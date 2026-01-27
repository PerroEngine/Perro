use std::{borrow::Cow, time::Instant};
use rustc_hash::FxHashMap;
use crate::uid32::{Uid32, TextureID, NodeID};

use wgpu::{
    Adapter, Backends, BindGroupDescriptor, BindGroupEntry, BindGroupLayout, BindingResource, BufferBinding,
    BufferBindingType, BufferDescriptor, BufferSize, BufferUsages,
    Device, DeviceDescriptor, Features, Instance, InstanceDescriptor, Limits, MemoryHints, PowerPreference, Queue,
    RenderPass, RequestAdapterOptions, SurfaceConfiguration, TextureFormat,
    util::DeviceExt,
};
use winit::{dpi::PhysicalSize, event_loop::EventLoopProxy, window::Window};

use crate::{
    Camera2D, Camera3D, Transform3D,
    asset_io::load_asset,
    font::FontAtlas,
    renderer_2d::Renderer2D,
    renderer_3d::{MaterialUniform, Mesh, Renderer3D},
    renderer_prim::PrimitiveRenderer,
    renderer_ui::RendererUI,
    runtime::get_static_textures,
    structs2d::ImageTexture,
    vertex::Vertex,
    nodes::ui::egui_integration::EguiIntegration,
};

use crate::rendering::image_loader;

#[cfg(target_arch = "wasm32")]
pub type Rc<T> = std::rc::Rc<T>;
#[cfg(not(target_arch = "wasm32"))]
pub type Rc<T> = std::sync::Arc<T>;

#[cfg(target_arch = "wasm32")]
pub type SharedWindow = std::rc::Rc<Window>;
#[cfg(not(target_arch = "wasm32"))]
pub type SharedWindow = std::sync::Arc<Window>;

pub const VIRTUAL_WIDTH: f32 = 1920.0;
pub const VIRTUAL_HEIGHT: f32 = 1080.0;

#[derive(Debug)]
pub struct TextureManager {
    textures: FxHashMap<String, ImageTexture>, // path -> texture (primary storage)
    path_to_id: FxHashMap<String, TextureID>, // path -> id (new)
    id_to_path: FxHashMap<TextureID, String>, // id -> path (for reverse lookup)
    bind_groups: FxHashMap<String, wgpu::BindGroup>,
    // OPTIMIZED: Cache bind group layout to avoid recreating for each texture
    cached_bind_group_layout: Option<wgpu::BindGroupLayout>,
}

impl TextureManager {
    pub fn new() -> Self {
        Self {
            textures: FxHashMap::default(),
            path_to_id: FxHashMap::default(),
            id_to_path: FxHashMap::default(),
            bind_groups: FxHashMap::default(),
            cached_bind_group_layout: None,
        }
    }

    /// Get or create the cached bind group layout for textures
    pub fn get_bind_group_layout(&mut self, device: &Device) -> &wgpu::BindGroupLayout {
        if self.cached_bind_group_layout.is_none() {
            self.cached_bind_group_layout = Some(crate::structs2d::create_texture_bind_group_layout(device));
        }
        self.cached_bind_group_layout.as_ref().unwrap()
    }

    pub fn get_or_load_texture_sync(
        &mut self,
        path: &str,
        device: &Device,
        queue: &Queue,
    ) -> &ImageTexture {
        // Optimize: check with &str first, only allocate String if we need to insert
        if !self.textures.contains_key(path) {
            let key = path.to_string();
            let start = Instant::now();

            // Runtime mode: check static textures first, then fall back to disk/BRK
            // Dev mode: static textures will be None, so it loads from disk/BRK
            let img_texture = if let Some(static_textures) = get_static_textures() {
                if let Some(static_data) = static_textures.get(path) {
                    println!(
                        "🖼️ Loading static texture: {} ({}x{})",
                        path, static_data.width, static_data.height
                    );
                    // Use pre-decoded RGBA8 data to create ImageTexture
                    let texture = static_data.to_image_texture(device, queue);
                    let elapsed = start.elapsed();
                    println!(
                        "⏱️ Static texture loaded in {:.2}ms",
                        elapsed.as_secs_f64() * 1000.0
                    );
                    texture
                } else {
                    // Not in static textures, load from disk/BRK
                    let load_start = Instant::now();
                    let img_bytes = load_asset(path).expect("Failed to read image file");
                    let load_elapsed = load_start.elapsed();

                    let decode_start = Instant::now();
                    let img = image::load_from_memory(&img_bytes).expect("Failed to decode image");
                    let decode_elapsed = decode_start.elapsed();

                    println!(
                        "🖼️ Loading texture: {} ({}x{})",
                        path,
                        img.width(),
                        img.height()
                    );

                    let upload_start = Instant::now();
                    let texture = ImageTexture::from_image(&img, device, queue);
                    let upload_elapsed = upload_start.elapsed();

                    let total_elapsed = start.elapsed();
                    println!(
                        "⏱️ Runtime texture loaded in {:.2}ms total (load: {:.2}ms, decode: {:.2}ms, upload: {:.2}ms)",
                        total_elapsed.as_secs_f64() * 1000.0,
                        load_elapsed.as_secs_f64() * 1000.0,
                        decode_elapsed.as_secs_f64() * 1000.0,
                        upload_elapsed.as_secs_f64() * 1000.0
                    );
                    texture
                }
            } else {
                // Dev mode: no static textures, load from disk/BRK with optimized decoder
                let load_start = Instant::now();
                let img_bytes = load_asset(path).expect("Failed to read image file");
                let load_elapsed = load_start.elapsed();

                let decode_start = Instant::now();
                // Use optimized fast decoder (format-specific decoders for PNG/JPEG)
                let (rgba, width, height) = image_loader::load_and_decode_image_fast(&img_bytes, path)
                    .expect("Failed to decode image");
                let decode_elapsed = decode_start.elapsed();

                println!(
                    "🖼️ Loading texture: {} ({}x{})",
                    path,
                    width,
                    height
                );

                let upload_start = Instant::now();
                // Use direct RGBA8 path (avoids DynamicImage conversion)
                let texture = ImageTexture::from_rgba8(&rgba, device, queue);
                let upload_elapsed = upload_start.elapsed();

                let total_elapsed = start.elapsed();
                println!(
                    "⏱️ Dev texture loaded in {:.2}ms total (load: {:.2}ms, decode: {:.2}ms, upload: {:.2}ms)",
                    total_elapsed.as_secs_f64() * 1000.0,
                    load_elapsed.as_secs_f64() * 1000.0,
                    decode_elapsed.as_secs_f64() * 1000.0,
                    upload_elapsed.as_secs_f64() * 1000.0
                );
                texture
            };
            
            // Get or create Uid32 for this path (Texture namespace)
            let texture_id = *self.path_to_id.entry(key.clone()).or_insert_with(|| TextureID::new());
            self.id_to_path.insert(texture_id, key.clone());
            
            // Store by path (textures_by_id will look up through path)
            self.textures.insert(key.clone(), img_texture);
        }
        // Optimize: use path directly for lookup (no String allocation needed)
        self.textures.get(path).unwrap()
    }

    /// Get or load texture by path and return its UUID
    /// This is the main API method for scripts - returns the UUID handle
    /// Returns an error if the texture cannot be loaded
    pub fn get_or_load_texture_id(
        &mut self,
        path: &str,
        device: &Device,
        queue: &Queue,
    ) -> Result<TextureID, String> {
        // Check if already loaded
        if let Some(id) = self.path_to_id.get(path) {
            return Ok(*id);
        }
        
        // Try to load the texture
        let key = path.to_string();
        let start = Instant::now();

        // Runtime mode: check static textures first, then fall back to disk/BRK
        // Dev mode: static textures will be None, so it loads from disk/BRK
        let img_texture = if let Some(static_textures) = get_static_textures() {
            if let Some(static_data) = static_textures.get(path) {
                println!(
                    "🖼️ Loading static texture: {} ({}x{})",
                    path, static_data.width, static_data.height
                );
                // Use pre-decoded RGBA8 data to create ImageTexture
                let texture = static_data.to_image_texture(device, queue);
                let elapsed = start.elapsed();
                println!(
                    "⏱️ Static texture loaded in {:.2}ms",
                    elapsed.as_secs_f64() * 1000.0
                );
                texture
            } else {
                // Not in static textures, load from disk/BRK
                let load_start = Instant::now();
                let img_bytes = load_asset(path)
                    .map_err(|e| format!("Failed to read image file '{}': {}", path, e))?;
                let load_elapsed = load_start.elapsed();

                let decode_start = Instant::now();
                let img = image::load_from_memory(&img_bytes)
                    .map_err(|e| format!("Failed to decode image '{}': {}", path, e))?;
                let decode_elapsed = decode_start.elapsed();

                println!(
                    "🖼️ Loading texture: {} ({}x{})",
                    path,
                    img.width(),
                    img.height()
                );

                let upload_start = Instant::now();
                let texture = ImageTexture::from_image(&img, device, queue);
                let upload_elapsed = upload_start.elapsed();

                let total_elapsed = start.elapsed();
                println!(
                    "⏱️ Runtime texture loaded in {:.2}ms total (load: {:.2}ms, decode: {:.2}ms, upload: {:.2}ms)",
                    total_elapsed.as_secs_f64() * 1000.0,
                    load_elapsed.as_secs_f64() * 1000.0,
                    decode_elapsed.as_secs_f64() * 1000.0,
                    upload_elapsed.as_secs_f64() * 1000.0
                );
                texture
            }
        } else {
            // Dev mode: no static textures, load from disk/BRK with optimized decoder
            let load_start = Instant::now();
            let img_bytes = load_asset(path)
                .map_err(|e| format!("Failed to read image file '{}': {}", path, e))?;
            let load_elapsed = load_start.elapsed();

            let decode_start = Instant::now();
            // Use optimized fast decoder (format-specific decoders for PNG/JPEG)
            let (rgba, width, height) = image_loader::load_and_decode_image_fast(&img_bytes, path)
                .map_err(|e| format!("Failed to decode image '{}': {}", path, e))?;
            let decode_elapsed = decode_start.elapsed();

            println!(
                "🖼️ Loading texture: {} ({}x{})",
                path,
                width,
                height
            );

            let upload_start = Instant::now();
            // Use direct RGBA8 path (avoids DynamicImage conversion)
            let texture = ImageTexture::from_rgba8(&rgba, device, queue);
            let upload_elapsed = upload_start.elapsed();

            let total_elapsed = start.elapsed();
            println!(
                "⏱️ Dev texture loaded in {:.2}ms total (load: {:.2}ms, decode: {:.2}ms, upload: {:.2}ms)",
                total_elapsed.as_secs_f64() * 1000.0,
                load_elapsed.as_secs_f64() * 1000.0,
                decode_elapsed.as_secs_f64() * 1000.0,
                upload_elapsed.as_secs_f64() * 1000.0
            );
            texture
        };
        
        // Get or create TextureID for this path
        let texture_id = *self.path_to_id.entry(key.clone()).or_insert_with(|| TextureID::new());
        self.id_to_path.insert(texture_id, key.clone());
        
        // Store by path (textures_by_id will look up through path)
        self.textures.insert(key.clone(), img_texture);
        
        Ok(texture_id)
    }

    /// Get texture by TextureID (for script access)
    /// Returns a reference if the texture exists
    /// Looks up path from ID, then gets texture by path
    pub fn get_texture_by_id(&self, id: &TextureID) -> Option<&ImageTexture> {
        // Look up path from ID, then get texture by path
        if let Some(path) = self.id_to_path.get(id) {
            self.textures.get(path)
        } else {
            None
        }
    }

    /// Get texture path from TextureID (for rendering fallback)
    pub fn get_texture_path_from_id(&self, id: &TextureID) -> Option<&str> {
        self.id_to_path.get(id).map(|s| s.as_str())
    }

    /// Create texture from bytes and return UUID
    /// Creates a synthetic path for programmatically created textures
    pub fn create_texture_from_bytes(
        &mut self,
        rgba_bytes: &[u8],
        width: u32,
        height: u32,
        device: &Device,
        queue: &Queue,
    ) -> TextureID {
        let texture = ImageTexture::from_rgba8_bytes(rgba_bytes, width, height, device, queue);
        let texture_id = TextureID::new();
        // Create a synthetic path for programmatically created textures
        let synthetic_path = format!("__synthetic__{}", texture_id);
        self.path_to_id.insert(synthetic_path.clone(), texture_id);
        self.id_to_path.insert(texture_id, synthetic_path.clone());
        self.textures.insert(synthetic_path, texture);
        texture_id
    }

    /// Get texture size if texture is already loaded (doesn't load if missing)
    pub fn get_texture_size_if_loaded(&self, path: &str) -> Option<crate::Vector2> {
        self.textures.get(path).map(|tex| {
            crate::Vector2::new(tex.width as f32, tex.height as f32)
        })
    }

    /// Get texture size by UUID (for script access)
    pub fn get_texture_size_by_id(&self, id: &TextureID) -> Option<crate::Vector2> {
        self.get_texture_by_id(id).map(|tex| {
            crate::Vector2::new(tex.width as f32, tex.height as f32)
        })
    }

    pub fn get_or_create_bind_group(
        &mut self,
        path: &str,
        device: &Device,
        queue: &Queue,
        layout: &BindGroupLayout,
    ) -> &wgpu::BindGroup {
        // Optimize: check with &str first, only allocate String if we need to insert
        if !self.bind_groups.contains_key(path) {
            let key = path.to_string();
            let tex = self.get_or_load_texture_sync(path, device, queue);
            let bind_group = device.create_bind_group(&BindGroupDescriptor {
                layout,
                entries: &[
                    BindGroupEntry {
                        binding: 0,
                        resource: wgpu::BindingResource::Sampler(&tex.sampler),
                    },
                    BindGroupEntry {
                        binding: 1,
                        resource: wgpu::BindingResource::TextureView(&tex.view),
                    },
                ],
                label: Some("Texture Instance BG"),
            });
            self.bind_groups.insert(key, bind_group);
        }
        // Optimize: use path directly for lookup (no String allocation needed)
        self.bind_groups.get(path).unwrap()
    }
}

pub struct MeshManager {
    pub meshes: FxHashMap<String, Mesh>,
}

impl MeshManager {
    pub fn new() -> Self {
        Self {
            meshes: FxHashMap::default(),
        }
    }

    pub fn get_or_load_mesh(
        &mut self,
        path: &str,
        device: &Device,
        _queue: &Queue,
    ) -> Option<&Mesh> {
        // Optimize: check with &str first, only allocate String if we need to insert
        if !self.meshes.contains_key(path) {
            let key = path.to_string();
            // Load mesh from file
            if let Some(mesh) = Self::load_mesh_from_file(path, device) {
                println!("🔷 Loading mesh: {}", path);
                self.meshes.insert(key, mesh);
            } else {
                println!("⚠️ Failed to load mesh: {}", path);
                return None;
            }
        }

        // Optimize: use path directly for lookup (no String allocation needed)
        self.meshes.get(path)
    }

    pub fn load_mesh_from_file(path: &str, device: &Device) -> Option<Mesh> {
        match path {
            "__cube__" => Some(crate::renderer_3d::Renderer3D::create_cube_mesh(device)),
            "__sphere__" => Some(crate::renderer_3d::Renderer3D::create_sphere_mesh(device)),
            "__plane__" => Some(crate::renderer_3d::Renderer3D::create_plane_mesh(device)),
            "__cylinder__" => Some(crate::renderer_3d::Renderer3D::create_cylinder_mesh(device)),
            "__capsule__" => Some(crate::renderer_3d::Renderer3D::create_capsule_mesh(device)),
            "__cone__" => Some(crate::renderer_3d::Renderer3D::create_cone_mesh(device)),
            "__s_pyramid__" => Some(crate::renderer_3d::Renderer3D::create_square_pyramid_mesh(
                device,
            )),
            "__t_pyramid__" => {
                Some(crate::renderer_3d::Renderer3D::create_triangular_pyramid_mesh(device))
            }

            // Future: load actual .glb or .gltf
            _ => {
                // TODO: Implement real GLB/GLTF loading
                None
            }
        }
    }

    pub fn add_mesh(&mut self, name: String, mesh: Mesh) {
        self.meshes.insert(name, mesh);
    }
}

/// Material manager that handles path → slot mapping
pub struct MaterialManager {
    /// All loaded materials by path
    materials: FxHashMap<String, MaterialUniform>,

    /// Cache of path → GPU slot ID
    path_to_slot: FxHashMap<String, u32>,
}

impl MaterialManager {
    pub fn new() -> Self {
        Self {
            materials: FxHashMap::default(),
            path_to_slot: FxHashMap::default(),
        }
    }

    /// Load or get a material by path
    pub fn load_material(&mut self, path: &str) -> MaterialUniform {
        if let Some(mat) = self.materials.get(path) {
            return *mat;
        }

        // Create different materials based on path for testing
        let material = match path {
            "__default__" => MaterialUniform {
                base_color: [1.0, 1.0, 1.0, 1.0],
                metallic: 0.0,
                roughness: 0.5,
                _pad0: [0.0; 2],
                emissive: [0.1, 0.1, 0.1, 0.1],
            },
            "__red__" => MaterialUniform {
                base_color: [1.0, 0.2, 0.2, 1.0],
                metallic: 0.0,
                roughness: 0.5,
                _pad0: [0.0; 2],
                emissive: [0.1, 0.0, 0.0, 0.0],
            },
            "__blue__" => MaterialUniform {
                base_color: [0.07, 0.5, 0.9, 1.0],
                metallic: 0.0,
                roughness: 0.5,
                _pad0: [0.0; 2],
                emissive: [0.0, 0.0, 0.1, 0.0],
            },
            "__green__" => MaterialUniform {
                base_color: [0.2, 1.0, 0.2, 1.0],
                metallic: 0.0,
                roughness: 0.5,
                _pad0: [0.0; 2],
                emissive: [0.0, 0.1, 0.0, 0.0],
            },
            _ => MaterialUniform {
                base_color: [1.0, 1.0, 1.0, 1.0],
                metallic: 0.0,
                roughness: 0.5,
                _pad0: [0.0; 2],
                emissive: [0.0, 0.0, 0.0, 0.0],
            },
        };

        // Optimize: only allocate String when inserting (HashMap key)
        self.materials.insert(path.to_string(), material);
        material
    }

    /// Upload material to renderer and get its slot ID (idempotent)
    pub fn upload_to_renderer(&mut self, path: &str, renderer: &mut Renderer3D) -> Option<u32> {
        // Check if already uploaded
        if let Some(&slot) = self.path_to_slot.get(path) {
            return Some(slot);
        }

        // Load the material data
        let material = self.load_material(path);

        // Create deterministic Uid32 from path
        let mat_uuid = Uid32::from_string(path);

        // Queue to renderer
        let slot = renderer.queue_material(mat_uuid, material);

        // Cache the slot (only allocate String when inserting)
        self.path_to_slot.insert(path.to_string(), slot);

        Some(slot)
    }

    /// Get slot ID without uploading (returns None if not uploaded yet)
    pub fn get_slot(&self, path: &str) -> Option<u32> {
        // Optimize: use &str for lookup (no String allocation)
        self.path_to_slot.get(path).copied()
    }

    /// Register a material directly (useful for embedded GLTF materials)
    pub fn register_material(&mut self, path: String, material: MaterialUniform) {
        self.materials.insert(path, material);
    }

    /// Get or upload material and return slot ID (most common method)
    pub fn get_or_upload_material(&mut self, path: &str, renderer: &mut Renderer3D) -> u32 {
        // Check if already uploaded (this will catch the default material)
        if let Some(&slot) = self.path_to_slot.get(path) {
            return slot;
        }

        // Load the material data
        let material = self.load_material(path);

        // Create deterministic Uid32 from path
        let mat_uuid = Uid32::from_string(path);

        // Queue to renderer
        let slot = renderer.queue_material(mat_uuid, material);

        // Cache the slot (only allocate String when inserting)
        self.path_to_slot.insert(path.to_string(), slot);

        slot
    }

    /// Batch resolve multiple material paths at once (more efficient)
    pub fn resolve_material_paths(
        &mut self,
        paths: &[&str],
        renderer: &mut Renderer3D,
    ) -> Vec<u32> {
        paths
            .iter()
            .map(|path| self.get_or_upload_material(path, renderer))
            .collect()
    }
}

pub struct Graphics {
    // Core WGPU resources
    window: Rc<Window>,
    #[allow(dead_code)]
    instance: Instance,
    pub surface: wgpu::Surface<'static>,
    pub surface_config: SurfaceConfiguration,
    #[allow(dead_code)]
    adapter: Adapter,
    pub device: Device,
    pub queue: Queue,

    // Shared rendering resources
    pub texture_manager: TextureManager,
    pub mesh_manager: MeshManager,
    pub material_manager: MaterialManager,
    pub camera_buffer: wgpu::Buffer,
    pub camera_bind_group_layout: BindGroupLayout,
    pub camera_bind_group: wgpu::BindGroup,
    // UI camera uses window size directly (no virtual resolution)
    pub ui_camera_buffer: wgpu::Buffer,
    pub ui_camera_bind_group: wgpu::BindGroup,
    pub vertex_buffer: wgpu::Buffer,

    pub camera3d: Camera3D,
    pub camera3d_buffer: wgpu::Buffer,
    pub camera3d_bind_group_layout: BindGroupLayout,
    pub camera3d_bind_group: wgpu::BindGroup,

    // Specialized renderers
    pub renderer_prim: PrimitiveRenderer,
    pub renderer_2d: Renderer2D,
    pub renderer_ui: RendererUI,
    pub renderer_3d: Renderer3D,
    
    // egui integration for native text rendering
    pub egui_integration: EguiIntegration,
    pub egui_renderer: Option<egui_wgpu::Renderer>,

    pub depth_texture: wgpu::Texture,
    pub depth_view: wgpu::TextureView,

    // Cached render state
    #[allow(dead_code)]
    cached_operations: wgpu::Operations<wgpu::Color>,

    // OPTIMIZED: Cache camera 3D matrices to avoid recalculating in render()
    cached_camera3d_view: Option<glam::Mat4>,
    cached_camera3d_proj: Option<glam::Mat4>,
}
fn initialize_material_system(renderer_3d: &mut Renderer3D, queue: &Queue) -> MaterialManager {
    let mut material_manager = MaterialManager::new();

    // Guarantee that the default material exists (once)
    material_manager.get_or_upload_material("__default__", renderer_3d);

    // Upload whatever is pending (only the default at startup)
    renderer_3d.upload_materials_to_gpu(queue);

    material_manager
}pub async fn create_graphics(window: SharedWindow, proxy: EventLoopProxy<Graphics>) {
    
    
    // GPU-aware backend selection: probe all backends, detect GPU vendors, choose best match
    // Different GPUs work better with different backends:
    // - Intel integrated: DX12 (best on Windows), Vulkan (fallback)
    // - NVIDIA: Vulkan (excellent support), DX12 (good support)
    // - AMD: Vulkan (excellent support), DX12 (good support)
    // - Apple Silicon: Metal only
    // Note: OpenGL backend disabled due to wgpu-hal 28.0.0 compatibility issue
    
    // Get list of available backends for this platform
    // Note: OpenGL backend disabled due to wgpu-hal 28.0.0 compatibility issue
    #[cfg(windows)]
    let available_backends = vec![
        ("DX12", Backends::DX12),
        ("Vulkan", Backends::VULKAN),
    ];
    #[cfg(target_os = "macos")]
    let available_backends = vec![
        ("Metal", Backends::METAL),
        ("Vulkan", Backends::VULKAN),
    ];
    #[cfg(target_os = "linux")]
    let available_backends = vec![
        ("Vulkan", Backends::VULKAN),
    ];
    #[cfg(not(any(windows, target_os = "macos", target_os = "linux")))]
    let available_backends = vec![
        ("Vulkan", Backends::VULKAN),
    ];
    
    // Collect all available adapters from all backends
    struct AdapterCandidate {
        backend_name: &'static str,
        backend: Backends,
        info: wgpu::AdapterInfo,
    }
    
    let mut candidates: Vec<AdapterCandidate> = Vec::new();
    
    for (backend_name, backends) in available_backends.iter() {
        let test_instance = Instance::new(&InstanceDescriptor {
            backends: *backends,
            ..Default::default()
        });
        
        let test_surface = match test_instance.create_surface(Rc::clone(&window)) {
            Ok(s) => s,
            Err(e) => {
                eprintln!("⚠️  Failed to create surface for {} backend: {:?}", backend_name, e);
                continue;
            }
        };
        
        // Try to get adapters (prefer discrete, then integrated)
        let mut seen_names = std::collections::HashSet::new();
        
        // Try high performance (discrete GPU)
        if let Ok(adap) = test_instance
            .request_adapter(&RequestAdapterOptions {
                power_preference: PowerPreference::HighPerformance,
                force_fallback_adapter: false,
                compatible_surface: Some(&test_surface),
            })
            .await
        {
            let info = adap.get_info();
            if seen_names.insert(info.name.clone()) {
                candidates.push(AdapterCandidate {
                    backend_name,
                    backend: *backends,
                    info,
                });
            }
        }
        
        // Try default (might get integrated)
        if let Ok(adap) = test_instance
            .request_adapter(&RequestAdapterOptions {
                power_preference: PowerPreference::default(),
                force_fallback_adapter: false,
                compatible_surface: Some(&test_surface),
            })
            .await
        {
            let info = adap.get_info();
            if seen_names.insert(info.name.clone()) {
                candidates.push(AdapterCandidate {
                    backend_name,
                    backend: *backends,
                    info,
                });
            }
        }
    }
    
    if candidates.is_empty() {
        panic!("No GPU adapter found (hardware or software)");
    }
    
    // Score each candidate based on GPU vendor and backend compatibility
    let scored: Vec<_> = candidates.into_iter().map(|cand| {
        let mut score = 0i32;
        let name_lower = cand.info.name.to_lowercase();
        
        // Prefer discrete GPUs over integrated
        match cand.info.device_type {
            wgpu::DeviceType::DiscreteGpu => score += 1000,
            wgpu::DeviceType::IntegratedGpu => score += 100,
            wgpu::DeviceType::Cpu => score += 1, // Software renderer
            _ => {}
        }
        
        // GPU vendor-specific backend preferences
        // Note: OpenGL backend disabled due to wgpu-hal 28.0.0 compatibility issue
        if name_lower.contains("intel") {
            // Intel: DX12 (best on Windows), Vulkan (fallback)
            match cand.backend_name {
                "DX12" => score += 300,
                "Vulkan" => score += 200,
                _ => {}
            }
        } else if name_lower.contains("nvidia") {
            // NVIDIA: Vulkan excellent support, DX12 good support
            match cand.backend_name {
                "Vulkan" => score += 300,
                "DX12" => score += 250,
                _ => {}
            }
        } else if name_lower.contains("amd") || name_lower.contains("radeon") {
            // AMD: Vulkan excellent support, DX12 good support
            match cand.backend_name {
                "Vulkan" => score += 400,
                "DX12" => score += 250,
                _ => {}
            }
        } else if name_lower.contains("apple") || name_lower.contains("m1") || 
                  name_lower.contains("m2") || name_lower.contains("m3") {
            // Apple Silicon: Metal only
            match cand.backend_name {
                "Metal" => score += 1000,
                _ => score -= 500, // Don't use other backends on Apple
            }
        } else {
            // Unknown GPU: prefer Vulkan, then Metal
            match cand.backend_name {
                "Vulkan" => score += 250,
                "Metal" => score += 200,
                _ => {}
            }
        }
        
        (score, cand)
    }).collect();
    
    // Sort by score (highest first) and take the best one
    let mut scored_sorted = scored;
    scored_sorted.sort_by(|a, b| b.0.cmp(&a.0));
    
    let (_, best_candidate) = scored_sorted.into_iter().next().unwrap();
    
    // Create fresh instance and surface for the selected backend
    // IMPORTANT: Must be done immediately before requesting adapter to avoid invalidation
    let instance = Instance::new(&InstanceDescriptor {
        backends: best_candidate.backend,
        ..Default::default()
    });
    
    let surface = instance.create_surface(Rc::clone(&window))
        .unwrap_or_else(|e| {
            panic!(
                "Failed to create graphics surface with {} backend: {:?}\n\
                This usually happens on VMs or systems without proper graphics drivers.\n\
                Try installing graphics drivers or enabling hardware acceleration in your VM settings.",
                best_candidate.backend_name, e
            );
        });
    
    // Request the adapter again with the fresh instance/surface
    // This prevents the adapter from becoming invalid
    let adapter = instance
        .request_adapter(&RequestAdapterOptions {
            power_preference: if best_candidate.info.device_type == wgpu::DeviceType::DiscreteGpu {
                PowerPreference::HighPerformance
            } else {
                PowerPreference::default()
            },
            force_fallback_adapter: false,
            compatible_surface: Some(&surface),
        })
        .await
        .expect("Failed to request adapter for selected backend");
    
    let backend_used = best_candidate.backend_name;
    let adapter_name = adapter.get_info().name.clone();
    
    // Check if it's integrated graphics (often less stable)
    let is_integrated = matches!(adapter.get_info().device_type, wgpu::DeviceType::IntegratedGpu);
    
    // Use more conservative limits for integrated GPUs to avoid driver crashes
    let device_limits = if is_integrated {
        // More conservative limits for integrated GPUs
        wgpu::Limits::downlevel_webgl2_defaults()
            .using_resolution(adapter.limits())
    } else {
        wgpu::Limits::downlevel_webgl2_defaults()
            .using_resolution(adapter.limits())
    };
    
    let (device, queue) = adapter
        .request_device(
            &DeviceDescriptor {
                label: None,
                required_features: wgpu::Features::empty(),
                required_limits: device_limits,
                memory_hints: if is_integrated {
                    wgpu::MemoryHints::Performance // Use performance hints for integrated
                } else {
                    wgpu::MemoryHints::Performance
                },
                experimental_features: wgpu::ExperimentalFeatures::disabled(),
                trace: wgpu::Trace::Off,
            },
        )
        .await
        .expect("Failed to get device");

    // Choose emoji based on backend
    let backend_emoji = match backend_used {
        "Vulkan" => "⚡",
        "DX12" => "🎮",
        "Metal" => "🍎",  
        _ => "💻",
    };
    println!("{} {} on {} backend", backend_emoji, adapter_name, backend_used);

    // 2) Surface config
    let size = window.inner_size();
    let (w, h) = (size.width.max(1), size.height.max(1));
    let mut surface_config = surface.get_default_config(&adapter, w, h).unwrap();
    
    // OPTIMIZED: Select best available present mode with proper fallback
    // Priority: Immediate (no VSync) > Mailbox (adaptive VSync) > Fifo (standard VSync)
    // Since we're doing frame pacing at the application level, we don't need VSync
    // Using Immediate prevents double-limiting and pixel waving/jitter
    let surface_caps = surface.get_capabilities(&adapter);
    let preferred_present_mode = if surface_caps.present_modes.contains(&wgpu::PresentMode::Immediate) {
        wgpu::PresentMode::Immediate
    } else if surface_caps.present_modes.contains(&wgpu::PresentMode::Mailbox) {
        wgpu::PresentMode::Mailbox
    } else if surface_caps.present_modes.contains(&wgpu::PresentMode::Fifo) {
        wgpu::PresentMode::Fifo
    } else {
        // Fallback to default (should always be Fifo, which is guaranteed to be supported)
        surface_config.present_mode
    };
    
    surface_config.present_mode = preferred_present_mode;
    
    let present_mode_name = match preferred_present_mode {
        wgpu::PresentMode::Mailbox => "Mailbox (adaptive VSync)",
        wgpu::PresentMode::Fifo => "Fifo (standard VSync)",
        wgpu::PresentMode::FifoRelaxed => "FifoRelaxed",
        wgpu::PresentMode::Immediate => "Immediate (no VSync)",
        _ => "Unknown",
    };
    println!("📺 Present mode: {}", present_mode_name);
    
    #[cfg(not(target_arch = "wasm32"))]
    surface.configure(&device, &surface_config);

    // 3) Camera uniform buffer (for 2D)
    let camera_buffer = device.create_buffer(&BufferDescriptor {
        label: Some("Camera UBO"),
        size: 96,
        usage: BufferUsages::UNIFORM | BufferUsages::COPY_DST,
        mapped_at_creation: false,
    });

    let camera_bind_group_layout =
        device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("Camera BGL"),
            entries: &[wgpu::BindGroupLayoutEntry {
                binding: 0,
                visibility: wgpu::ShaderStages::VERTEX,
                ty: wgpu::BindingType::Buffer {
                    ty: BufferBindingType::Uniform,
                    has_dynamic_offset: false,
                    min_binding_size: BufferSize::new(96),
                },
                count: None,
            }],
        });

    let camera_bind_group = device.create_bind_group(&BindGroupDescriptor {
        label: Some("Camera BG"),
        layout: &camera_bind_group_layout,
        entries: &[BindGroupEntry {
            binding: 0,
            resource: BindingResource::Buffer(BufferBinding {
                buffer: &camera_buffer,
                offset: 0,
                size: BufferSize::new(96),
            }),
        }],
    });

    // 3.5) UI Camera uniform buffer (uses window size directly, no virtual resolution)
    let ui_camera_buffer = device.create_buffer(&BufferDescriptor {
        label: Some("UI Camera UBO"),
        size: 96,
        usage: BufferUsages::UNIFORM | BufferUsages::COPY_DST,
        mapped_at_creation: false,
    });

    let ui_camera_bind_group = device.create_bind_group(&BindGroupDescriptor {
        label: Some("UI Camera BG"),
        layout: &camera_bind_group_layout, // Reuse the same layout
        entries: &[BindGroupEntry {
            binding: 0,
            resource: BindingResource::Buffer(BufferBinding {
                buffer: &ui_camera_buffer,
                offset: 0,
                size: BufferSize::new(96),
            }),
        }],
    });

    // Initialize UI camera: stretches to fill window (no black bars) while maintaining coordinate system
    // Virtual size defines the coordinate space (e.g., -540 is always left edge of 1080-wide space)
    // NDC scale maps virtual coordinates directly to window, stretching to fill
    let ui_virtual_width = VIRTUAL_WIDTH;
    let ui_virtual_height = VIRTUAL_HEIGHT;
    let window_width = surface_config.width as f32;
    let window_height = surface_config.height as f32;
    
    #[repr(C)]
    #[derive(Clone, Copy)]
    struct CameraUniform {
        virtual_size: [f32; 2],
        ndc_scale: [f32; 2],
        zoom: f32,
        _pad0: f32,
        _pad1: [f32; 2],
        view: [[f32; 4]; 4],
    }
    
    unsafe impl bytemuck::Pod for CameraUniform {}
    unsafe impl bytemuck::Zeroable for CameraUniform {}
    
    // UI camera: stretch to fill window (no black bars)
    // Scale virtual coordinates by window/virtual ratio using view matrix, then convert to NDC
        // Use f64 for precision to avoid rounding errors that cause gaps
        let ui_scale_x = (window_width as f64 / ui_virtual_width as f64) as f32;
        let ui_scale_y = (window_height as f64 / ui_virtual_height as f64) as f32;
        let ui_view = glam::Mat4::from_scale(glam::vec3(ui_scale_x, ui_scale_y, 1.0));
    // Use f64 for precision to avoid rounding errors
    let ui_ndc_scale = glam::vec2(
        (2.0_f64 / window_width as f64) as f32,
        (2.0_f64 / window_height as f64) as f32
    );
    
    let ui_cam_uniform = CameraUniform {
        virtual_size: [ui_virtual_width, ui_virtual_height], // Keep virtual size for coordinate system
        ndc_scale: ui_ndc_scale.into(),
        zoom: 0.0,
        _pad0: 0.0,
        _pad1: [0.0, 0.0],
        view: ui_view.to_cols_array_2d(),
    };
    
    queue.write_buffer(&ui_camera_buffer, 0, bytemuck::bytes_of(&ui_cam_uniform));

    // 4) 3D Camera uniform buffer (128 bytes: 2x mat4)
    let camera3d_buffer = device.create_buffer(&BufferDescriptor {
        label: Some("Camera3D UBO"),
        size: 128,
        usage: BufferUsages::UNIFORM | BufferUsages::COPY_DST,
        mapped_at_creation: false,
    });

    let camera3d_bind_group_layout =
        device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("Camera3D BGL"),
            entries: &[wgpu::BindGroupLayoutEntry {
                binding: 0,
                visibility: wgpu::ShaderStages::VERTEX | wgpu::ShaderStages::FRAGMENT,
                ty: wgpu::BindingType::Buffer {
                    ty: BufferBindingType::Uniform,
                    has_dynamic_offset: false,
                    min_binding_size: BufferSize::new(128),
                },
                count: None,
            }],
        });

    let camera3d_bind_group = device.create_bind_group(&BindGroupDescriptor {
        label: Some("Camera3D BG"),
        layout: &camera3d_bind_group_layout,
        entries: &[BindGroupEntry {
            binding: 0,
            resource: BindingResource::Buffer(BufferBinding {
                buffer: &camera3d_buffer,
                offset: 0,
                size: BufferSize::new(128),
            }),
        }],
    });

    let mut initial_camera_3d = Camera3D::new();
    initial_camera_3d.name = Cow::Borrowed("MainCamera3D");

    // Initialize 3D camera matrices
    let view = glam::Mat4::look_at_rh(
        glam::vec3(3.0, 3.0, 3.0),
        glam::vec3(0.0, 0.0, 0.0),
        glam::vec3(0.0, 1.0, 0.0),
    );

    let aspect_ratio = surface_config.width as f32 / surface_config.height as f32;
    let projection = glam::Mat4::perspective_rh(45.0_f32.to_radians(), aspect_ratio, 0.1, 100.0);

    // OPTIMIZED: Initialize cached camera matrices
    let cached_camera3d_view = Some(view);
    let cached_camera3d_proj = Some(projection);

    let camera3d_uniform = crate::renderer_3d::Camera3DUniform {
        view: view.to_cols_array_2d(),
        projection: projection.to_cols_array_2d(),
    };

    queue.write_buffer(&camera3d_buffer, 0, bytemuck::bytes_of(&camera3d_uniform));

    // 5) Quad vertex buffer
    let vertices: &[Vertex] = &[
        Vertex {
            position: [-0.5, -0.5],
            uv: [0.0, 1.0],
        },
        Vertex {
            position: [0.5, -0.5],
            uv: [1.0, 1.0],
        },
        Vertex {
            position: [0.5, 0.5],
            uv: [1.0, 0.0],
        },
        Vertex {
            position: [-0.5, -0.5],
            uv: [0.0, 1.0],
        },
        Vertex {
            position: [0.5, 0.5],
            uv: [1.0, 0.0],
        },
        Vertex {
            position: [-0.5, 0.5],
            uv: [0.0, 0.0],
        },
    ];
    let vertex_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
        label: Some("Vertex Buffer"),
        contents: bytemuck::cast_slice(vertices),
        usage: BufferUsages::VERTEX,
    });

    // 6) Initialize 2D camera data
    let virtual_width = VIRTUAL_WIDTH;
    let virtual_height = VIRTUAL_HEIGHT;
    let window_width = surface_config.width as f32;
    let window_height = surface_config.height as f32;

    let virtual_aspect = virtual_width / virtual_height;
    let window_aspect = window_width / window_height;

    let (scale_x, scale_y) = if window_aspect > virtual_aspect {
        (virtual_aspect / window_aspect, 1.0)
    } else {
        (1.0, window_aspect / virtual_aspect)
    };

    let camera_data = [
        virtual_width,
        virtual_height,
        scale_x * 2.0 / virtual_width,
        scale_y * 2.0 / virtual_height,
    ];
    queue.write_buffer(&camera_buffer, 0, bytemuck::cast_slice(&camera_data));

    // 7) Create renderers
    let mut renderer_3d =
        Renderer3D::new(&device, &camera3d_bind_group_layout, surface_config.format);
    let renderer_prim =
        PrimitiveRenderer::new(&device, &camera_bind_group_layout, surface_config.format);
    let renderer_2d = Renderer2D::new();
    let renderer_ui = RendererUI::new();

    // 8) Initialize material system with default material
    let material_manager = initialize_material_system(&mut renderer_3d, &queue);

    // 9) Create depth texture
    let depth_texture = device.create_texture(&wgpu::TextureDescriptor {
        label: Some("Depth Texture"),
        size: wgpu::Extent3d {
            width: surface_config.width,
            height: surface_config.height,
            depth_or_array_layers: 1,
        },
        mip_level_count: 1,
        sample_count: 1,
        dimension: wgpu::TextureDimension::D2,
        format: wgpu::TextureFormat::Depth32Float,
        usage: wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::TEXTURE_BINDING,
        view_formats: &[],
    });

    let depth_view = depth_texture.create_view(&wgpu::TextureViewDescriptor::default());
    
    // Initialize egui integration
    let egui_integration = EguiIntegration::new();
    
    // Initialize egui-wgpu renderer
    // egui-wgpu 0.33.3 uses its own wgpu re-exports (egui_wgpu::wgpu)
    // We need to use those types, but wgpu types are compatible
    // Create renderer lazily in render() to avoid type issues
    let egui_renderer = None; // Will be initialized lazily in render() 
    
    let gfx = Graphics {
        window: window.clone(),
        instance,
        surface,
        surface_config,
        adapter,
        device,
        queue,
        texture_manager: TextureManager::new(),
        mesh_manager: MeshManager::new(),
        material_manager,
        camera_buffer,
        camera_bind_group_layout,
        camera_bind_group,
        ui_camera_buffer,
        ui_camera_bind_group,
        vertex_buffer,

        camera3d: initial_camera_3d,
        camera3d_buffer,
        camera3d_bind_group_layout,
        camera3d_bind_group,

        renderer_prim,
        renderer_2d,
        renderer_ui,
        renderer_3d,

        depth_texture,
        depth_view,
        
        egui_integration,
        egui_renderer,

        cached_operations: wgpu::Operations {
            load: wgpu::LoadOp::Clear(wgpu::Color::BLACK),
            store: wgpu::StoreOp::Store,
        },

        // OPTIMIZED: Initialize cached camera matrices with initial values
        cached_camera3d_view,
        cached_camera3d_proj,
    };
    let _ = proxy.send_event(gfx);
}

/// Synchronous version of create_graphics for use during initialization
/// Returns Graphics directly instead of sending via proxy
pub fn create_graphics_sync(window: SharedWindow) -> Graphics {
    
    
    // GPU-aware backend selection: probe all backends, detect GPU vendors, choose best match
    // Different GPUs work better with different backends:
    // - Intel integrated: DX12 (best on Windows), Vulkan (fallback)
    // - NVIDIA: Vulkan (excellent support), DX12 (good support)
    // - AMD: Vulkan (excellent support), DX12 (good support)
    // - Apple Silicon: Metal only
    // Note: OpenGL backend disabled due to wgpu-hal 28.0.0 compatibility issue
    
    // Get list of available backends for this platform
    #[cfg(windows)]
    let available_backends = vec![
        ("DX12", Backends::DX12),
        ("Vulkan", Backends::VULKAN),
    ];
    #[cfg(target_os = "macos")]
    let available_backends = vec![
        ("Metal", Backends::METAL),
        ("Vulkan", Backends::VULKAN),
    ];
    #[cfg(target_os = "linux")]
    let available_backends = vec![
        ("Vulkan", Backends::VULKAN),
    ];
    #[cfg(not(any(windows, target_os = "macos", target_os = "linux")))]
    let available_backends = vec![
        ("Vulkan", Backends::VULKAN),
    ];
    
    // Collect all available adapters from all backends
    struct AdapterCandidate {
        backend_name: &'static str,
        backend: Backends,
        info: wgpu::AdapterInfo,
    }
    
    let mut candidates: Vec<AdapterCandidate> = Vec::new();
    
    for (backend_name, backends) in available_backends.iter() {
        let instance = Instance::new(&InstanceDescriptor {
            backends: *backends,
            ..Default::default()
        });
        
        // Try to create surface - if it fails, skip this backend
        let surface = match instance.create_surface(window.clone()) {
            Ok(s) => s,
            Err(e) => {
                eprintln!("⚠️  Failed to create surface for {} backend: {:?}", backend_name, e);
                continue;
            }
        };
        
        let adapter_options = RequestAdapterOptions {
            power_preference: PowerPreference::HighPerformance,
            compatible_surface: Some(&surface),
            force_fallback_adapter: false,
        };
        
        if let Ok(adapter) = pollster::block_on(instance.request_adapter(&adapter_options)) {
            let info = adapter.get_info();
            candidates.push(AdapterCandidate {
                backend_name,
                backend: *backends,
                info,
            });
        }
    }
    
    // Score each candidate based on GPU vendor and backend compatibility
    if candidates.is_empty() {
        panic!("No GPU adapter found (hardware or software)");
    }
    
    let scored: Vec<_> = candidates.into_iter().map(|cand| {
        let mut score = 0i32;
        let name_lower = cand.info.name.to_lowercase();
        
        // Prefer discrete GPUs over integrated
        match cand.info.device_type {
            wgpu::DeviceType::DiscreteGpu => score += 1000,
            wgpu::DeviceType::IntegratedGpu => score += 100,
            wgpu::DeviceType::Cpu => score += 1, // Software renderer
            _ => {}
        }
        
        // GPU vendor-specific backend preferences
        // Note: OpenGL backend disabled due to wgpu-hal 28.0.0 compatibility issue
        if name_lower.contains("intel") {
            // Intel: DX12 (best on Windows), Vulkan (fallback)
            match cand.backend_name {
                "DX12" => score += 300,
                "Vulkan" => score += 200,
                _ => {}
            }
        } else if name_lower.contains("nvidia") {
            // NVIDIA: Vulkan excellent support, DX12 good support
            match cand.backend_name {
                "Vulkan" => score += 300,
                "DX12" => score += 250,
                _ => {}
            }
        } else if name_lower.contains("amd") || name_lower.contains("radeon") {
            // AMD: Vulkan excellent support, DX12 good support
            match cand.backend_name {
                "Vulkan" => score += 400,
                "DX12" => score += 250,
                _ => {}
            }
        } else if name_lower.contains("apple") || name_lower.contains("m1") || 
                  name_lower.contains("m2") || name_lower.contains("m3") {
            // Apple Silicon: Metal only
            match cand.backend_name {
                "Metal" => score += 1000,
                _ => score -= 500, // Don't use other backends on Apple
            }
        } else {
            // Unknown GPU: prefer Vulkan, then Metal
            match cand.backend_name {
                "Vulkan" => score += 250,
                "Metal" => score += 200,
                _ => {}
            }
        }
        
        (score, cand)
    }).collect();
    
    // Sort by score (highest first) and take the best one
    let mut scored_sorted = scored;
    scored_sorted.sort_by(|a, b| b.0.cmp(&a.0));
    
    let (best_score, best_candidate) = scored_sorted.into_iter().next().unwrap();
    
    
    let chosen_backend = best_candidate.backend;
    let _chosen_backend_name = best_candidate.backend_name;
    
    // Create instance with chosen backend
    let instance = Instance::new(&InstanceDescriptor {
        backends: chosen_backend,
        ..Default::default()
    });
    
    // Create surface
    let surface = instance.create_surface(window.clone())
        .unwrap_or_else(|e| {
            panic!(
                "Failed to create graphics surface with {} backend: {:?}\n\
                This usually happens on VMs or systems without proper graphics drivers.\n\
                Try installing graphics drivers or enabling hardware acceleration in your VM settings.",
                best_candidate.backend_name, e
            );
        });
    
    // Request adapter
    let adapter = pollster::block_on(instance.request_adapter(&RequestAdapterOptions {
        power_preference: PowerPreference::HighPerformance,
        compatible_surface: Some(&surface),
        force_fallback_adapter: false,
    }))
    .expect("Failed to find an appropriate adapter");
    
    // Get device and queue
    let (device, queue) = pollster::block_on(adapter.request_device(
        &DeviceDescriptor {
            label: None,
            required_features: Features::empty(),
            required_limits: Limits::default(),
            memory_hints: MemoryHints::default(),
            experimental_features: wgpu::ExperimentalFeatures::disabled(),
            trace: wgpu::Trace::Off,
        },
    ))
    .expect("Failed to create device");
    
    // Configure surface
    let surface_caps = surface.get_capabilities(&adapter);
    let surface_format = surface_caps
        .formats
        .iter()
        .copied()
        .find(|f| matches!(f, TextureFormat::Bgra8UnormSrgb | TextureFormat::Rgba8UnormSrgb))
        .unwrap_or(surface_caps.formats[0]);
    
    let size = window.inner_size();
    let surface_config = SurfaceConfiguration {
        usage: wgpu::TextureUsages::RENDER_ATTACHMENT,
        format: surface_format,
        width: size.width,
        height: size.height,
        present_mode: surface_caps.present_modes[0],
        alpha_mode: surface_caps.alpha_modes[0],
        view_formats: vec![],
        desired_maximum_frame_latency: 2,
    };
    surface.configure(&device, &surface_config);
    
    // Create camera buffers and bind groups (same as async version)
    let camera_buffer = device.create_buffer(&BufferDescriptor {
        label: Some("Camera Buffer"),
        size: 96,
        usage: BufferUsages::UNIFORM | BufferUsages::COPY_DST,
        mapped_at_creation: false,
    });
    
    let camera_bind_group_layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
        label: Some("Camera Bind Group Layout"),
        entries: &[wgpu::BindGroupLayoutEntry {
            binding: 0,
            visibility: wgpu::ShaderStages::VERTEX,
            ty: wgpu::BindingType::Buffer {
                ty: BufferBindingType::Uniform,
                has_dynamic_offset: false,
                min_binding_size: BufferSize::new(96),
            },
            count: None,
        }],
    });
    
    let camera_bind_group = device.create_bind_group(&BindGroupDescriptor {
        label: Some("Camera Bind Group"),
        layout: &camera_bind_group_layout,
        entries: &[BindGroupEntry {
            binding: 0,
            resource: BindingResource::Buffer(BufferBinding {
                buffer: &camera_buffer,
                offset: 0,
                size: BufferSize::new(96),
            }),
        }],
    });
    
    // 3D camera setup
    let mut initial_camera_3d = Camera3D::new();
    initial_camera_3d.name = Cow::Borrowed("MainCamera3D");
    
    let camera3d_buffer = device.create_buffer(&BufferDescriptor {
        label: Some("Camera3D Buffer"),
        size: std::mem::size_of::<crate::renderer_3d::Camera3DUniform>() as u64,
        usage: BufferUsages::UNIFORM | BufferUsages::COPY_DST,
        mapped_at_creation: false,
    });
    
    let camera3d_bind_group_layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
        label: Some("Camera3D Bind Group Layout"),
        entries: &[wgpu::BindGroupLayoutEntry {
            binding: 0,
            visibility: wgpu::ShaderStages::VERTEX | wgpu::ShaderStages::FRAGMENT,
            ty: wgpu::BindingType::Buffer {
                ty: BufferBindingType::Uniform,
                has_dynamic_offset: false,
                min_binding_size: BufferSize::new(std::mem::size_of::<crate::renderer_3d::Camera3DUniform>() as u64),
            },
            count: None,
        }],
    });
    
    let camera3d_bind_group = device.create_bind_group(&BindGroupDescriptor {
        label: Some("Camera3D Bind Group"),
        layout: &camera3d_bind_group_layout,
        entries: &[BindGroupEntry {
            binding: 0,
            resource: BindingResource::Buffer(BufferBinding {
                buffer: &camera3d_buffer,
                offset: 0,
                size: BufferSize::new(std::mem::size_of::<crate::renderer_3d::Camera3DUniform>() as u64),
            }),
        }],
    });
    
    // Initialize 3D camera matrices
    let view = glam::Mat4::look_at_rh(
        glam::vec3(3.0, 3.0, 3.0),
        glam::vec3(0.0, 0.0, 0.0),
        glam::vec3(0.0, 1.0, 0.0),
    );
    
    let aspect_ratio = surface_config.width as f32 / surface_config.height as f32;
    let projection = glam::Mat4::perspective_rh(45.0_f32.to_radians(), aspect_ratio, 0.1, 100.0);
    
    let camera3d_uniform = crate::renderer_3d::Camera3DUniform {
        view: view.to_cols_array_2d(),
        projection: projection.to_cols_array_2d(),
    };
    queue.write_buffer(&camera3d_buffer, 0, bytemuck::bytes_of(&camera3d_uniform));
    
    let cached_camera3d_view = Some(view);
    let cached_camera3d_proj = Some(projection);
    
    // Quad vertex buffer
    let vertices: &[Vertex] = &[
        Vertex {
            position: [-0.5, -0.5],
            uv: [0.0, 1.0],
        },
        Vertex {
            position: [0.5, -0.5],
            uv: [1.0, 1.0],
        },
        Vertex {
            position: [0.5, 0.5],
            uv: [1.0, 0.0],
        },
        Vertex {
            position: [-0.5, -0.5],
            uv: [0.0, 1.0],
        },
        Vertex {
            position: [0.5, 0.5],
            uv: [1.0, 0.0],
        },
        Vertex {
            position: [-0.5, 0.5],
            uv: [0.0, 0.0],
        },
    ];
    let vertex_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
        label: Some("Vertex Buffer"),
        contents: bytemuck::cast_slice(vertices),
        usage: BufferUsages::VERTEX,
    });
    
    // Initialize 2D camera data
    let virtual_width = VIRTUAL_WIDTH;
    let virtual_height = VIRTUAL_HEIGHT;
    let window_width = surface_config.width as f32;
    let window_height = surface_config.height as f32;
    
    let virtual_aspect = virtual_width / virtual_height;
    let window_aspect = window_width / window_height;
    
    let (scale_x, scale_y) = if window_aspect > virtual_aspect {
        (virtual_aspect / window_aspect, 1.0)
    } else {
        (1.0, window_aspect / virtual_aspect)
    };
    
    #[repr(C)]
    #[derive(Clone, Copy)]
    struct CameraUniform {
        virtual_size: [f32; 2],
        ndc_scale: [f32; 2],
        zoom: f32,
        _pad0: f32,
        _pad1: [f32; 2],
        view: [[f32; 4]; 4],
    }
    
    unsafe impl bytemuck::Pod for CameraUniform {}
    unsafe impl bytemuck::Zeroable for CameraUniform {}
    
    let ndc_scale = glam::vec2(scale_x * 2.0 / virtual_width, scale_y * 2.0 / virtual_height);
    let view = glam::Mat4::IDENTITY; // Initial identity view matrix
    
    let cam_uniform = CameraUniform {
        virtual_size: [virtual_width, virtual_height],
        ndc_scale: ndc_scale.into(),
        zoom: 0.0, // 0.0 = no zoom (UI should not be zoomed)
        _pad0: 0.0,
        _pad1: [0.0, 0.0],
        view: view.to_cols_array_2d(),
    };
    
    queue.write_buffer(&camera_buffer, 0, bytemuck::bytes_of(&cam_uniform));
    
    // Initialize UI camera (uses window size directly, no virtual resolution)
    let ui_camera_buffer = device.create_buffer(&BufferDescriptor {
        label: Some("UI Camera UBO"),
        size: 96,
        usage: BufferUsages::UNIFORM | BufferUsages::COPY_DST,
        mapped_at_creation: false,
    });

    let ui_camera_bind_group = device.create_bind_group(&BindGroupDescriptor {
        label: Some("UI Camera BG"),
        layout: &camera_bind_group_layout, // Reuse the same layout
        entries: &[BindGroupEntry {
            binding: 0,
            resource: BindingResource::Buffer(BufferBinding {
                buffer: &ui_camera_buffer,
                offset: 0,
                size: BufferSize::new(96),
            }),
        }],
    });

    // UI camera: stretches to fill window (no black bars) while maintaining coordinate system
    // Virtual size defines the coordinate space (e.g., -540 is always left edge of 1080-wide space)
    // NDC scale maps virtual coordinates directly to window, stretching to fill
    let ui_virtual_width = VIRTUAL_WIDTH;
    let ui_virtual_height = VIRTUAL_HEIGHT;
    
    // UI camera: stretch to fill window (no black bars)
    // Scale virtual coordinates by window/virtual ratio using view matrix, then convert to NDC
        // Use f64 for precision to avoid rounding errors that cause gaps
        let ui_scale_x = (window_width as f64 / ui_virtual_width as f64) as f32;
        let ui_scale_y = (window_height as f64 / ui_virtual_height as f64) as f32;
        let ui_view = glam::Mat4::from_scale(glam::vec3(ui_scale_x, ui_scale_y, 1.0));
    // Use f64 for precision to avoid rounding errors
    let ui_ndc_scale = glam::vec2(
        (2.0_f64 / window_width as f64) as f32,
        (2.0_f64 / window_height as f64) as f32
    );
    
    let ui_cam_uniform = CameraUniform {
        virtual_size: [ui_virtual_width, ui_virtual_height], // Keep virtual size for coordinate system
        ndc_scale: ui_ndc_scale.into(),
        zoom: 0.0,
        _pad0: 0.0,
        _pad1: [0.0, 0.0],
        view: ui_view.to_cols_array_2d(),
    };
    
    queue.write_buffer(&ui_camera_buffer, 0, bytemuck::bytes_of(&ui_cam_uniform));
    
    // Create renderers
    let mut renderer_3d =
        Renderer3D::new(&device, &camera3d_bind_group_layout, surface_config.format);
    let renderer_prim =
        PrimitiveRenderer::new(&device, &camera_bind_group_layout, surface_config.format);
    let renderer_2d = Renderer2D::new();
    let renderer_ui = RendererUI::new();
    
    // Initialize material system with default material
    let material_manager = initialize_material_system(&mut renderer_3d, &queue);
    
    // Create depth texture
    let depth_texture = device.create_texture(&wgpu::TextureDescriptor {
        label: Some("Depth Texture"),
        size: wgpu::Extent3d {
            width: surface_config.width,
            height: surface_config.height,
            depth_or_array_layers: 1,
        },
        mip_level_count: 1,
        sample_count: 1,
        dimension: wgpu::TextureDimension::D2,
        format: wgpu::TextureFormat::Depth32Float,
        usage: wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::TEXTURE_BINDING,
        view_formats: &[],
    });
    
    let depth_view = depth_texture.create_view(&wgpu::TextureViewDescriptor::default());
    
    Graphics {
        window: window.clone(),
        instance,
        surface,
        surface_config,
        adapter,
        device,
        queue,
        texture_manager: TextureManager::new(),
        mesh_manager: MeshManager::new(),
        material_manager,
        camera_buffer,
        camera_bind_group_layout,
        camera_bind_group,
        ui_camera_buffer,
        ui_camera_bind_group,
        vertex_buffer,
        camera3d: initial_camera_3d,
        camera3d_buffer,
        camera3d_bind_group_layout,
        camera3d_bind_group,
        renderer_prim,
        renderer_2d,
        renderer_ui,
        renderer_3d,
        depth_texture,
        depth_view,
        
        egui_integration: EguiIntegration::new(),
        egui_renderer: None,

        cached_operations: wgpu::Operations {
            load: wgpu::LoadOp::Clear(wgpu::Color::BLACK),
            store: wgpu::StoreOp::Store,
        },
        cached_camera3d_view,
        cached_camera3d_proj,
    }
}

impl Graphics {
    pub fn window(&self) -> &winit::window::Window {
        &self.window
    }

    pub fn resize(&mut self, size: PhysicalSize<u32>) {
        // OPTIMIZED: Wait for all pending work to complete before resizing
        // This ensures we don't resize while GPU is still using old resources
        let _ = self.device.poll(wgpu::PollType::Wait {
            submission_index: None,
            timeout: None,
        });

        self.surface_config.width = size.width.max(1);
        self.surface_config.height = size.height.max(1);
        self.surface.configure(&self.device, &self.surface_config);
        self.update_camera_uniform();
        self.update_ui_camera_uniform();

        // Recreate depth texture with new size
        self.depth_texture = self.device.create_texture(&wgpu::TextureDescriptor {
            label: Some("Depth Texture"),
            size: wgpu::Extent3d {
                width: self.surface_config.width,
                height: self.surface_config.height,
                depth_or_array_layers: 1,
            },
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: wgpu::TextureFormat::Depth32Float,
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::TEXTURE_BINDING,
            view_formats: &[],
        });
        self.depth_view = self
            .depth_texture
            .create_view(&wgpu::TextureViewDescriptor::default());
    }

    fn update_camera_uniform(&self) {
        let virtual_width = VIRTUAL_WIDTH;
        let virtual_height = VIRTUAL_HEIGHT;
        let window_width = self.surface_config.width as f32;
        let window_height = self.surface_config.height as f32;

        let virtual_aspect = virtual_width / virtual_height;
        let window_aspect = window_width / window_height;

        let (scale_x, scale_y) = if window_aspect > virtual_aspect {
            (virtual_aspect / window_aspect, 1.0)
        } else {
            (1.0, window_aspect / virtual_aspect)
        };

        let camera_data = [
            virtual_width,
            virtual_height,
            scale_x * 2.0 / virtual_width,
            scale_y * 2.0 / virtual_height,
        ];

        self.queue
            .write_buffer(&self.camera_buffer, 0, bytemuck::cast_slice(&camera_data));
    }

    fn update_ui_camera_uniform(&self) {
        // UI camera: stretches to fill window (no black bars) while maintaining coordinate system
        // Virtual size defines the coordinate space (e.g., -540 is always left edge of 1080-wide space)
        // NDC scale maps virtual coordinates directly to window, stretching to fill
        let ui_virtual_width = VIRTUAL_WIDTH;
        let ui_virtual_height = VIRTUAL_HEIGHT;
        let window_width = self.surface_config.width as f32;
        let window_height = self.surface_config.height as f32;

        #[repr(C)]
        #[derive(Clone, Copy)]
        struct CameraUniform {
            virtual_size: [f32; 2],
            ndc_scale: [f32; 2],
            zoom: f32,
            _pad0: f32,
            _pad1: [f32; 2],
            view: [[f32; 4]; 4],
        }

        unsafe impl bytemuck::Pod for CameraUniform {}
        unsafe impl bytemuck::Zeroable for CameraUniform {}

        // UI camera: stretch to fill window (no black bars)
        // Scale virtual coordinates by window/virtual ratio using view matrix, then convert to NDC
        // view matrix scales: virtual_pos * (window/virtual) = window_pos
        // ndc_scale converts: window_pos * (2.0/window) = NDC
        // Use f64 for precision to avoid rounding errors that cause gaps
        let ui_scale_x = (window_width as f64 / ui_virtual_width as f64) as f32;
        let ui_scale_y = (window_height as f64 / ui_virtual_height as f64) as f32;
        let ui_view = glam::Mat4::from_scale(glam::vec3(ui_scale_x, ui_scale_y, 1.0));
        let ui_ndc_scale = glam::vec2(
            (2.0_f64 / window_width as f64) as f32,
            (2.0_f64 / window_height as f64) as f32
        );

        let ui_cam_uniform = CameraUniform {
            virtual_size: [ui_virtual_width, ui_virtual_height], // Keep virtual size for coordinate system
            ndc_scale: ui_ndc_scale.into(),
            zoom: 0.0,
            _pad0: 0.0,
            _pad1: [0.0, 0.0],
            view: ui_view.to_cols_array_2d(),
        };

        self.queue
            .write_buffer(&self.ui_camera_buffer, 0, bytemuck::bytes_of(&ui_cam_uniform));
    }

    pub fn update_camera_2d(&mut self, cam: &Camera2D) {
        // Pass zoom directly to shader (0.0 = normal, positive = zoom in, negative = zoom out)
        // The shader now divides positions by (1.0 + zoom), so:
        //   - zoom = 0.0: divide by 1.0 = normal
        //   - zoom > 0.0: divide by >1.0 = positions smaller = zoom IN
        //   - zoom < 0.0: divide by <1.0 = positions larger = zoom OUT
        let zoom = cam.zoom;
        let t = &cam.transform;

        let rotation = glam::Mat4::from_rotation_z(t.rotation);
        let translation =
            glam::Mat4::from_translation(glam::vec3(-t.position.x, -t.position.y, 0.0));
        let view = rotation * translation;

        let vw = VIRTUAL_WIDTH;
        let vh = VIRTUAL_HEIGHT;
        let ndc_scale = glam::vec2(2.0 / vw, 2.0 / vh);

        #[repr(C)]
        #[derive(Clone, Copy)]
        struct CameraUniform {
            virtual_size: [f32; 2],
            ndc_scale: [f32; 2],
            zoom: f32,
            _pad0: f32,
            _pad1: [f32; 2],
            view: [[f32; 4]; 4],
        }

        unsafe impl bytemuck::Pod for CameraUniform {}
        unsafe impl bytemuck::Zeroable for CameraUniform {}

        let cam_uniform = CameraUniform {
            virtual_size: [vw, vh],
            ndc_scale: ndc_scale.into(),
            zoom,
            _pad0: 0.0,
            _pad1: [0.0, 0.0],
            view: view.to_cols_array_2d(),
        };

        self.queue
            .write_buffer(&self.camera_buffer, 0, bytemuck::bytes_of(&cam_uniform));
        
        // OPTIMIZED: Update viewport culling info in primitive renderer
        self.renderer_prim.update_camera_2d(t.position, t.rotation, zoom);
    }

    pub fn update_camera_3d(&mut self, cam: &Camera3D) {
        // Save the active camera reference for later use (clone values)
        self.camera3d = cam.clone();

        let t = &cam.transform;

        let translation = glam::Mat4::from_translation(t.position.to_glam_public());
        let rotation = glam::Mat4::from_quat(t.rotation.to_glam_public());

        let model = translation * rotation;
        let view = model.inverse();

        let aspect_ratio = self.surface_config.width as f32 / self.surface_config.height as f32;
        let projection = glam::Mat4::perspective_rh(
            cam.fov.unwrap_or(45.0_f32.to_radians()),
            aspect_ratio,
            cam.near.unwrap_or(0.1),
            cam.far.unwrap_or(100.0),
        );

        // OPTIMIZED: Cache matrices for use in render() to avoid recalculation
        self.cached_camera3d_view = Some(view);
        self.cached_camera3d_proj = Some(projection);

        let camera3d_uniform = crate::renderer_3d::Camera3DUniform {
            view: view.to_cols_array_2d(),
            projection: projection.to_cols_array_2d(),
        };

        self.queue.write_buffer(
            &self.camera3d_buffer,
            0,
            bytemuck::bytes_of(&camera3d_uniform),
        );

        self.renderer_3d
            .maybe_update_culling(&self.mesh_manager, &view, &projection, &self.queue);
    }

    #[allow(dead_code)]
    pub fn initialize_font_atlas(&mut self, _font_atlas: FontAtlas) {
        // DEPRECATED: Native text rendering initializes glyph atlas on-demand
        // This method is kept for compatibility but does nothing
    }

    pub fn stop_rendering(&mut self, uuid: NodeID) {
        self.renderer_prim.stop_rendering(uuid.as_uid32());
    }

    /// Queue a 3D mesh with automatic material resolution
    pub fn queue_mesh_3d(
        &mut self,
        uuid: NodeID,
        mesh_path: &str,
        material_path: &str,
        transform: Transform3D,
    ) {
        self.renderer_3d.queue_mesh(
            uuid,
            mesh_path,
            transform,
            Some(material_path),
            &mut self.mesh_manager,
            &mut self.material_manager,
            &self.device,
            &self.queue,
        );
    }

    /// Batch queue multiple meshes (more efficient for scenes with many objects)
    pub fn queue_meshes_3d(&mut self, meshes: &[(NodeID, &str, &str, Transform3D)]) {
        // Pre-resolve all unique material paths
        let unique_materials: std::collections::HashSet<&str> =
            meshes.iter().map(|(_, _, mat, _)| *mat).collect();
        let material_paths: Vec<&str> = unique_materials.into_iter().collect();
        self.material_manager
            .resolve_material_paths(&material_paths, &mut self.renderer_3d);

        // Now queue all meshes (material IDs are cached)
        for (uuid, mesh_path, material_path, transform) in meshes {
            self.renderer_3d.queue_mesh(
                *uuid,
                mesh_path,
                *transform,
                Some(material_path),
                &mut self.mesh_manager,
                &mut self.material_manager,
                &self.device,
                &self.queue,
            );
        }
    }

    /// Main render method that coordinates all renderers
    pub fn render(&mut self, rpass: &mut RenderPass<'_>) {
        // Upload any dirty materials/lights before rendering
        {
            #[cfg(feature = "profiling")]
            let _span = tracing::span!(tracing::Level::INFO, "upload_materials_to_gpu").entered();
            self.renderer_3d.upload_materials_to_gpu(&self.queue);
        }
        {
            #[cfg(feature = "profiling")]
            let _span = tracing::span!(tracing::Level::INFO, "upload_lights_to_gpu").entered();
            self.renderer_3d.upload_lights_to_gpu(&self.queue);
        }

        // OPTIMIZED: Use cached camera matrices instead of recalculating
        // Fallback to calculation if cache is invalid (shouldn't happen in normal flow)
        let (view, proj) = if let (Some(v), Some(p)) = (self.cached_camera3d_view, self.cached_camera3d_proj) {
            (v, p)
        } else {
            // Fallback: recalculate if cache is missing (shouldn't happen normally)
            let t = &self.camera3d.transform;
            let translation = glam::Mat4::from_translation(t.position.to_glam_public());
            let rotation = glam::Mat4::from_quat(t.rotation.to_glam_public());
            let view = (translation * rotation).inverse();
            let aspect_ratio = self.surface_config.width as f32 / self.surface_config.height as f32;
            let proj = glam::Mat4::perspective_rh(
                self.camera3d.fov.unwrap_or(45.0_f32.to_radians()),
                aspect_ratio,
                self.camera3d.near.unwrap_or(0.1),
                self.camera3d.far.unwrap_or(100.0),
            );
            (view, proj)
        };

        {
            #[cfg(feature = "profiling")]
            let _span = tracing::span!(tracing::Level::INFO, "renderer_3d_render").entered();
            self.renderer_3d.render(
                rpass,
                &self.mesh_manager,
                &self.camera3d_bind_group,
                &view,
                &proj,
                &self.device,
                &self.queue,
            );
        }

        // Render 2D world objects
        {
            #[cfg(feature = "profiling")]
            let _span = tracing::span!(tracing::Level::INFO, "renderer_2d_render").entered();
            self.renderer_2d.render(
                &mut self.renderer_prim,
                rpass,
                &mut self.texture_manager,
                &self.device,
                &self.queue,
                &self.camera_bind_group,
                &self.vertex_buffer,
            );
        }

        // Render UI on top (panels, images - still using old system)
        {
            #[cfg(feature = "profiling")]
            let _span = tracing::span!(tracing::Level::INFO, "renderer_ui_render").entered();
            self.renderer_ui.render(
                &mut self.renderer_prim,
                rpass,
                &mut self.texture_manager,
                &self.device,
                &self.queue,
                &self.ui_camera_bind_group, // Use UI camera that fits window directly
                &self.vertex_buffer,
            );
        }
        
        // TODO: Render egui UI elements here
        // egui rendering is prepared in render_ui() and will be rendered here
        // For now, egui context is updated but not yet rendered to screen
    }

    pub fn begin_frame(
        &mut self,
    ) -> (
        wgpu::SurfaceTexture,
        wgpu::TextureView,
        wgpu::CommandEncoder,
    ) {
        // OPTIMIZED: Use loop instead of recursion to avoid stack growth on repeated errors
        loop {
            match self.surface.get_current_texture() {
                Ok(frame) => {
                    let view = frame
                        .texture
                        .create_view(&wgpu::TextureViewDescriptor::default());
                    let encoder = self
                        .device
                        .create_command_encoder(&wgpu::CommandEncoderDescriptor {
                            label: Some("Main Encoder"),
                        });
                    return (frame, view, encoder);
                }
                Err(wgpu::SurfaceError::Lost | wgpu::SurfaceError::Outdated) => {
                    // Surface needs reconfiguration - reconfigure and retry
                    self.surface.configure(&self.device, &self.surface_config);
                    // Continue loop to retry
                }
                Err(wgpu::SurfaceError::OutOfMemory) => {
                    eprintln!("OutOfMemory: GPU may be lost");
                    std::process::exit(1);
                }
                Err(e) => {
                    eprintln!("Surface error: {:?}, retrying...", e);
                    // Continue loop to retry (may be transient)
                }
            }
        }
    }

    pub fn end_frame(&mut self, frame: wgpu::SurfaceTexture, encoder: wgpu::CommandEncoder) {
        self.queue.submit(Some(encoder.finish()));
        frame.present();

        // OPTIMIZED: Use Poll instead of Wait at end of frame for better throughput
        // Wait is used in resize() where we need to ensure completion, but Poll is
        // more efficient for regular frame rendering where we don't need to block
        let _ = self.device.poll(wgpu::PollType::Poll);
    }

    /// Ensure egui renderer is initialized
    fn ensure_egui_renderer(&mut self) {
        if self.egui_renderer.is_none() {
            use egui_wgpu::{Renderer, RendererOptions};
            // Now that we're using wgpu 27.0.1 (same as egui-wgpu), types are compatible
            let renderer = Renderer::new(
                &self.device,
                self.surface_config.format,
                RendererOptions {
                    msaa_samples: 1,
                    depth_stencil_format: None,
                    ..Default::default()
                },
            );
            self.egui_renderer = Some(renderer);
            log::info!("🎨 [EGUI] Renderer initialized");
        }
    }

    /// Render egui UI output to the screen
    /// Must be called after the main render pass, before end_frame
    pub fn render_egui(
        &mut self,
        encoder: &mut wgpu::CommandEncoder,
        view: &wgpu::TextureView,
    ) {
        // Ensure renderer is initialized first
        self.ensure_egui_renderer();

        // Get the output if available - early return if no output
        let full_output = match &self.egui_integration.last_output {
            Some(output) => output,
            None => return,
        };

        // Update textures if needed
        if let Some(renderer) = &mut self.egui_renderer {
            for (id, image_delta) in &full_output.textures_delta.set {
                renderer.update_texture(
                    &self.device,
                    &self.queue,
                    *id,
                    image_delta,
                );
            }

            // Remove old textures
            for id in &full_output.textures_delta.free {
                renderer.free_texture(id);
            }
        } else {
            log::warn!("🎨 [EGUI] Renderer not initialized, skipping render");
            return;
        }

        // Paint egui shapes to screen
        let screen_descriptor = egui_wgpu::ScreenDescriptor {
            size_in_pixels: [self.surface_config.width, self.surface_config.height],
            pixels_per_point: self.egui_integration.context.pixels_per_point(),
        };

        // Paint the shapes
        let paint_jobs = self.egui_integration.context.tessellate(
            full_output.shapes.clone(),
            self.egui_integration.context.pixels_per_point(),
        );

        // Skip rendering if there are no paint jobs
        if paint_jobs.is_empty() {
            return;
        }

        // Update buffers first
        if let Some(renderer) = &mut self.egui_renderer {
            renderer.update_buffers(
                &self.device,
                &self.queue,
                encoder,
                &paint_jobs,
                &screen_descriptor,
            );

            // Create render pass for egui and render
            let rpass_descriptor = wgpu::RenderPassDescriptor {
                label: Some("egui_render_pass"),
                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                    view,
                    resolve_target: None,
                    ops: wgpu::Operations {
                        load: wgpu::LoadOp::Load, // Load existing content (don't clear)
                        store: wgpu::StoreOp::Store,
                    },
                    depth_slice: None,
                })],
                depth_stencil_attachment: None,
                timestamp_writes: None,
                occlusion_query_set: None,
            };
            
            // SAFETY: egui-wgpu re-exports wgpu, so types should be compatible
            // We need to convert the render pass to egui_wgpu's wgpu type
            let mut rpass = encoder.begin_render_pass(&rpass_descriptor);
            // Use egui_wgpu's wgpu types explicitly
            use egui_wgpu::wgpu as egui_wgpu_types;
            let rpass_egui: &mut egui_wgpu_types::RenderPass<'_> = unsafe {
                std::mem::transmute::<&mut wgpu::RenderPass<'_>, &mut egui_wgpu_types::RenderPass<'_>>(&mut rpass)
            };
            renderer.render(
                rpass_egui,
                &paint_jobs,
                &screen_descriptor,
            );
        }
    }
}
