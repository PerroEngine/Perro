use std::{borrow::Cow, collections::HashMap, fmt, ops::Range, sync::Arc, time::Instant};

use bytemuck::cast_slice;
use wgpu::{
    Adapter, Backends, BindGroupDescriptor, BindGroupEntry, BindGroupLayout, BindingResource, BufferBinding,
    BufferBindingType, BufferDescriptor, BufferSize, BufferUsages, CommandEncoderDescriptor,
    Device, DeviceDescriptor, Features, Instance, InstanceDescriptor, Limits, MemoryHints, PowerPreference, Queue,
    RenderPass, RequestAdapterOptions, SurfaceConfiguration, TextureFormat, TextureViewDescriptor,
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
};

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
    textures: HashMap<String, ImageTexture>,
    bind_groups: HashMap<String, wgpu::BindGroup>,
}

impl TextureManager {
    pub fn new() -> Self {
        Self {
            textures: HashMap::new(),
            bind_groups: HashMap::new(),
        }
    }

    pub fn get_or_load_texture_sync(
        &mut self,
        path: &str,
        device: &Device,
        queue: &Queue,
    ) -> &ImageTexture {
        let key = path.to_string();
        if !self.textures.contains_key(&key) {
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
                // Dev mode: no static textures, load from disk/BRK
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
                    "⏱️ Dev texture loaded in {:.2}ms total (load: {:.2}ms, decode: {:.2}ms, upload: {:.2}ms)",
                    total_elapsed.as_secs_f64() * 1000.0,
                    load_elapsed.as_secs_f64() * 1000.0,
                    decode_elapsed.as_secs_f64() * 1000.0,
                    upload_elapsed.as_secs_f64() * 1000.0
                );
                texture
            };
            self.textures.insert(key.clone(), img_texture);
        }
        self.textures.get(&key).unwrap()
    }

    pub fn get_or_create_bind_group(
        &mut self,
        path: &str,
        device: &Device,
        queue: &Queue,
        layout: &BindGroupLayout,
    ) -> &wgpu::BindGroup {
        let key = path.to_string();
        if !self.bind_groups.contains_key(&key) {
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
            self.bind_groups.insert(key.clone(), bind_group);
        }
        self.bind_groups.get(&key).unwrap()
    }
}

pub struct MeshManager {
    pub meshes: HashMap<String, Mesh>,
}

impl MeshManager {
    pub fn new() -> Self {
        Self {
            meshes: HashMap::new(),
        }
    }

    pub fn get_or_load_mesh(
        &mut self,
        path: &str,
        device: &Device,
        queue: &Queue,
    ) -> Option<&Mesh> {
        let key = path.to_string();

        if !self.meshes.contains_key(&key) {
            // Load mesh from file
            if let Some(mesh) = Self::load_mesh_from_file(path, device) {
                println!("🔷 Loading mesh: {}", path);
                self.meshes.insert(key.clone(), mesh);
            } else {
                println!("⚠️ Failed to load mesh: {}", path);
                return None;
            }
        }

        self.meshes.get(&key)
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
    materials: HashMap<String, MaterialUniform>,

    /// Cache of path → GPU slot ID
    path_to_slot: HashMap<String, u32>,
}

impl MaterialManager {
    pub fn new() -> Self {
        Self {
            materials: HashMap::new(),
            path_to_slot: HashMap::new(),
        }
    }

    /// Load or get a material by path
    pub fn load_material(&mut self, path: &str) -> MaterialUniform {
        if let Some(mat) = self.materials.get(path) {
            return *mat;
        }

        // Create different materials based on path for testing
        println!("📦 Loading material: {}", path);
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

        // Create deterministic UUID from path
        let mat_uuid = uuid::Uuid::new_v5(&uuid::Uuid::NAMESPACE_OID, path.as_bytes());

        // Queue to renderer
        let slot = renderer.queue_material(mat_uuid, material);

        // Cache the slot
        self.path_to_slot.insert(path.to_string(), slot);

        Some(slot)
    }

    /// Get slot ID without uploading (returns None if not uploaded yet)
    pub fn get_slot(&self, path: &str) -> Option<u32> {
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

        // Create deterministic UUID from path
        let mat_uuid = uuid::Uuid::new_v5(&uuid::Uuid::NAMESPACE_OID, path.as_bytes());

        // Queue to renderer
        let slot = renderer.queue_material(mat_uuid, material);

        // Cache the slot
        self.path_to_slot.insert(path.to_string(), slot);

        println!("📦 Material '{}' assigned to slot {}", path, slot);
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
    instance: Instance,
    pub surface: wgpu::Surface<'static>,
    pub surface_config: SurfaceConfiguration,
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

    pub depth_texture: wgpu::Texture,
    pub depth_view: wgpu::TextureView,

    // Cached render state
    cached_operations: wgpu::Operations<wgpu::Color>,
}
fn initialize_material_system(renderer_3d: &mut Renderer3D, queue: &Queue) -> MaterialManager {
    let mut material_manager = MaterialManager::new();

    // Guarantee that the default material exists (once)
    material_manager.get_or_upload_material("__default__", renderer_3d);

    // Upload whatever is pending (only the default at startup)
    renderer_3d.upload_materials_to_gpu(queue);

    material_manager
}pub async fn create_graphics(window: SharedWindow, proxy: EventLoopProxy<Graphics>) {
    use std::io::Write;
    
    // GPU-aware backend selection: probe all backends, detect GPU vendors, choose best match
    // Different GPUs work better with different backends:
    // - Intel integrated: DX12 > Vulkan (Vulkan drivers crash)
    // - NVIDIA: Vulkan ≈ DX12 (both good, Vulkan often slightly better)
    // - AMD: Vulkan > DX12 (Vulkan usually better)
    // - Apple Silicon: Metal only
    
    println!("🔍 Probing available GPUs and backends...");
    std::io::stdout().flush().unwrap();
    
    // Get list of available backends for this platform
    #[cfg(windows)]
    let available_backends = vec![
        ("DX12", Backends::DX12),
        ("Vulkan", Backends::VULKAN),
        ("OpenGL", Backends::GL),
    ];
    #[cfg(target_os = "macos")]
    let available_backends = vec![
        ("Metal", Backends::METAL),
        ("Vulkan", Backends::VULKAN),
        ("OpenGL", Backends::GL),
    ];
    #[cfg(target_os = "linux")]
    let available_backends = vec![
        ("Vulkan", Backends::VULKAN),
        ("OpenGL", Backends::GL),
    ];
    #[cfg(not(any(windows, target_os = "macos", target_os = "linux")))]
    let available_backends = vec![
        ("Vulkan", Backends::VULKAN),
        ("OpenGL", Backends::GL),
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
            Err(_) => continue,
        };
        
        // Try to get adapters (prefer discrete, then integrated)
        let mut seen_names = std::collections::HashSet::new();
        
        // Try high performance (discrete GPU)
        if let Some(adap) = test_instance
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
        if let Some(adap) = test_instance
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
    
    // Also try WARP (software renderer) as last resort on Windows
    #[cfg(windows)]
    {
        let warp_instance = Instance::new(&InstanceDescriptor {
            backends: Backends::DX12,
            ..Default::default()
        });
        if let Ok(warp_surface) = warp_instance.create_surface(Rc::clone(&window)) {
            if let Some(warp_adap) = warp_instance
                .request_adapter(&RequestAdapterOptions {
                    power_preference: PowerPreference::LowPower,
                    force_fallback_adapter: true, // This enables WARP
                    compatible_surface: Some(&warp_surface),
                })
                .await
            {
                let info = warp_adap.get_info();
                candidates.push(AdapterCandidate {
                    backend_name: "WARP (Software)",
                    backend: Backends::DX12,
                    info,
                });
            }
        }
    }
    
    if candidates.is_empty() {
        panic!("No GPU adapter found (hardware or software)");
    }
    
    println!("🔍 Found {} GPU adapter(s):", candidates.len());
    for (i, cand) in candidates.iter().enumerate() {
        println!("   {}. {} on {} backend (Type: {:?})", 
            i + 1, cand.info.name, cand.backend_name, cand.info.device_type);
    }
    std::io::stdout().flush().unwrap();
    
    // Score each candidate based on GPU vendor and backend compatibility
    let scored: Vec<_> = candidates.into_iter().map(|cand| {
        let mut score = 0i32;
        let name_lower = cand.info.name.to_lowercase();
        
        // Prefer discrete GPUs over integrated
        match cand.info.device_type {
            wgpu::DeviceType::DiscreteGpu => score += 1000,
            wgpu::DeviceType::IntegratedGpu => score += 100,
            wgpu::DeviceType::Cpu => score += 1, // WARP/software
            _ => {}
        }
        
        // GPU vendor-specific backend preferences
        if name_lower.contains("intel") {
            // Intel: DX12 >> Vulkan (Vulkan drivers crash on integrated)
            match cand.backend_name {
                "DX12" => score += 500,
                "Vulkan" => score -= 200, // Heavy penalty for Intel + Vulkan
                "OpenGL" => score += 200,
                _ => {}
            }
        } else if name_lower.contains("nvidia") {
            // NVIDIA: Vulkan ≈ DX12 (both excellent, Vulkan often slightly better)
            match cand.backend_name {
                "Vulkan" => score += 300,
                "DX12" => score += 280,
                "OpenGL" => score += 100,
                _ => {}
            }
        } else if name_lower.contains("amd") || name_lower.contains("radeon") {
            // AMD: Vulkan > DX12 (Vulkan usually better)
            match cand.backend_name {
                "Vulkan" => score += 400,
                "DX12" => score += 200,
                "OpenGL" => score += 100,
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
            // Unknown GPU: prefer Vulkan, then DX12, then GL
            match cand.backend_name {
                "Vulkan" => score += 250,
                "DX12" => score += 200,
                "Metal" => score += 200,
                "OpenGL" => score += 100,
                _ => {}
            }
        }
        
        (score, cand)
    }).collect();
    
    // Sort by score (highest first) and take the best one
    let mut scored_sorted = scored;
    scored_sorted.sort_by(|a, b| b.0.cmp(&a.0));
    
    let (score, best_candidate) = scored_sorted.into_iter().next().unwrap();
    
    println!("✅ Selected: {} on {} backend (score: {})", 
        best_candidate.info.name, best_candidate.backend_name, score);
    std::io::stdout().flush().unwrap();
    
    // Warn about known problematic combinations
    let name_lower = best_candidate.info.name.to_lowercase();
    let is_intel_integrated = matches!(best_candidate.info.device_type, wgpu::DeviceType::IntegratedGpu)
        && name_lower.contains("intel");
    
    if is_intel_integrated && best_candidate.backend_name == "Vulkan" {
        println!("⚠ WARNING: Intel integrated GPU with Vulkan - known to crash!");
        println!("   DX12 would be more stable. Consider updating Intel GPU drivers.");
        std::io::stdout().flush().unwrap();
    }
    
    // Create fresh instance and surface for the selected backend
    // IMPORTANT: Must be done immediately before requesting adapter to avoid invalidation
    println!("🔧 Creating instance and surface for {} backend...", best_candidate.backend_name);
    std::io::stdout().flush().unwrap();
    
    let instance = Instance::new(&InstanceDescriptor {
        backends: best_candidate.backend,
        ..Default::default()
    });
    
    let surface = instance.create_surface(Rc::clone(&window))
        .expect("Failed to create surface for selected adapter");
    
    println!("🔧 Requesting adapter...");
    std::io::stdout().flush().unwrap();
    
    // Request the adapter again with the fresh instance/surface
    // This prevents the adapter from becoming invalid
    let adapter = instance
        .request_adapter(&RequestAdapterOptions {
            power_preference: if best_candidate.info.device_type == wgpu::DeviceType::DiscreteGpu {
                PowerPreference::HighPerformance
            } else {
                PowerPreference::default()
            },
            force_fallback_adapter: best_candidate.backend_name == "WARP (Software)",
            compatible_surface: Some(&surface),
        })
        .await
        .expect("Failed to request adapter for selected backend");
    
    let backend_used = best_candidate.backend_name;
    
    // Print adapter info for debugging GPU driver issues
    println!("🔧 GPU Adapter Info:");
    println!("   Backend Used: {}", backend_used);
    println!("   Name: {:?}", adapter.get_info().name);
    println!("   Backend: {:?}", adapter.get_info().backend);
    println!("   Device Type: {:?}", adapter.get_info().device_type);
    
    // Check if it's integrated graphics (often less stable)
    let is_integrated = matches!(adapter.get_info().device_type, wgpu::DeviceType::IntegratedGpu);
    if is_integrated {
        println!("   ⚠ WARNING: Using integrated GPU - enabling compatibility mode");
        println!("   Consider using a discrete GPU if available");
    }
    
    println!("   If you're experiencing crashes, try updating your GPU drivers.");
    std::io::stdout().flush().unwrap();
    
    // Use more conservative limits for integrated GPUs to avoid driver crashes
    let device_limits = if is_integrated {
        // More conservative limits for integrated GPUs
        wgpu::Limits::downlevel_webgl2_defaults()
            .using_resolution(adapter.limits())
    } else {
        wgpu::Limits::downlevel_webgl2_defaults()
            .using_resolution(adapter.limits())
    };
    
    println!("🔧 Requesting device...");
    std::io::stdout().flush().unwrap();
    
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
            },
            None,
        )
        .await
        .expect("Failed to get device");

    println!("✅ Device and queue created successfully");
    std::io::stdout().flush().unwrap();

    // 2) Surface config
    let size = window.inner_size();
    let (w, h) = (size.width.max(1), size.height.max(1));
    let mut surface_config = surface.get_default_config(&adapter, w, h).unwrap();
    surface_config.present_mode = wgpu::PresentMode::Immediate;
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

    let initial_camera_3d = Camera3D::new("MainCamera3D");

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
    println!("🔧 Creating Renderer3D...");
    std::io::stdout().flush().unwrap();
    let mut renderer_3d =
        Renderer3D::new(&device, &camera3d_bind_group_layout, surface_config.format);
    println!("✅ Renderer3D created successfully");
    std::io::stdout().flush().unwrap();
    
    println!("🔧 Creating PrimitiveRenderer...");
    std::io::stdout().flush().unwrap();
    let renderer_prim =
        PrimitiveRenderer::new(&device, &camera_bind_group_layout, surface_config.format);
    println!("✅ PrimitiveRenderer created successfully");
    std::io::stdout().flush().unwrap();
    
    println!("🔧 Creating Renderer2D...");
    std::io::stdout().flush().unwrap();
    let renderer_2d = Renderer2D::new();
    println!("✅ Renderer2D created successfully");
    std::io::stdout().flush().unwrap();
    
    println!("🔧 Creating RendererUI...");
    std::io::stdout().flush().unwrap();
    let renderer_ui = RendererUI::new();
    println!("✅ RendererUI created successfully");
    std::io::stdout().flush().unwrap();

    // 8) Initialize material system with default material
    println!("🔧 Initializing material system...");
    std::io::stdout().flush().unwrap();
    let material_manager = initialize_material_system(&mut renderer_3d, &queue);
    println!("✅ Material system initialized");
    std::io::stdout().flush().unwrap();

    // 9) Create depth texture
    println!("🔧 Creating depth texture...");
    std::io::stdout().flush().unwrap();
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
    println!("✅ Depth texture created");
    std::io::stdout().flush().unwrap();

    println!("🔧 Constructing Graphics struct...");
    std::io::stdout().flush().unwrap();
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

        cached_operations: wgpu::Operations {
            load: wgpu::LoadOp::Clear(wgpu::Color::BLACK),
            store: wgpu::StoreOp::Store,
        },
    };
    println!("✅ Graphics struct constructed");
    std::io::stdout().flush().unwrap();

    println!("🔧 Sending Graphics event...");
    std::io::stdout().flush().unwrap();
    let _ = proxy.send_event(gfx);
    println!("✅ Graphics event sent successfully");
    std::io::stdout().flush().unwrap();
}

impl Graphics {
    pub fn window(&self) -> &winit::window::Window {
        &self.window
    }

    pub fn resize(&mut self, size: PhysicalSize<u32>) {
        self.device.poll(wgpu::Maintain::Wait);

        self.surface_config.width = size.width.max(1);
        self.surface_config.height = size.height.max(1);
        self.surface.configure(&self.device, &self.surface_config);
        self.update_camera_uniform();

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

    pub fn update_camera_2d(&self, cam: &Camera2D) {
        let zoom = cam.zoom();
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
    }

    pub fn update_camera_3d(&mut self, cam: &Camera3D) {
        // Save the active camera reference for later use (clone values)
        self.camera3d = cam.clone();

        let t = &cam.transform;

        let translation = glam::Mat4::from_translation(t.position.to_glam());
        let rotation = glam::Mat4::from_quat(t.rotation.to_glam());

        let model = translation * rotation;
        let view = model.inverse();

        let aspect_ratio = self.surface_config.width as f32 / self.surface_config.height as f32;
        let projection = glam::Mat4::perspective_rh(
            cam.fov.unwrap_or(45.0_f32.to_radians()),
            aspect_ratio,
            cam.near.unwrap_or(0.1),
            cam.far.unwrap_or(100.0),
        );

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

    pub fn initialize_font_atlas(&mut self, font_atlas: FontAtlas) {
        self.renderer_prim
            .initialize_font_atlas(&self.device, &self.queue, font_atlas);
    }

    pub fn stop_rendering(&mut self, uuid: uuid::Uuid) {
        self.renderer_prim.stop_rendering(uuid);
    }

    /// Queue a 3D mesh with automatic material resolution
    pub fn queue_mesh_3d(
        &mut self,
        uuid: uuid::Uuid,
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
    pub fn queue_meshes_3d(&mut self, meshes: &[(uuid::Uuid, &str, &str, Transform3D)]) {
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
        self.renderer_3d.upload_materials_to_gpu(&self.queue);
        self.renderer_3d.upload_lights_to_gpu(&self.queue);

        let t = &self.camera3d.transform;
        let translation = glam::Mat4::from_translation(t.position.to_glam());
        let rotation = glam::Mat4::from_quat(t.rotation.to_glam());
        let view = (translation * rotation).inverse();

        let aspect_ratio = self.surface_config.width as f32 / self.surface_config.height as f32;
        let proj = glam::Mat4::perspective_rh(
            self.camera3d.fov.unwrap_or(45.0_f32.to_radians()),
            aspect_ratio,
            self.camera3d.near.unwrap_or(0.1),
            self.camera3d.far.unwrap_or(100.0),
        );

        self.renderer_3d.render(
            rpass,
            &self.mesh_manager,
            &self.camera3d_bind_group,
            &view,
            &proj,
            &self.device,
            &self.queue,
        );

        // Render 2D world objects
        self.renderer_2d.render(
            &mut self.renderer_prim,
            rpass,
            &mut self.texture_manager,
            &self.device,
            &self.queue,
            &self.camera_bind_group,
            &self.vertex_buffer,
        );

        // Render UI on top
        self.renderer_ui.render(
            &mut self.renderer_prim,
            rpass,
            &mut self.texture_manager,
            &self.device,
            &self.queue,
            &self.camera_bind_group,
            &self.vertex_buffer,
        );
    }

    pub fn begin_frame(
        &mut self,
    ) -> (
        wgpu::SurfaceTexture,
        wgpu::TextureView,
        wgpu::CommandEncoder,
    ) {
        let frame = match self.surface.get_current_texture() {
            Ok(frame) => frame,
            Err(wgpu::SurfaceError::Lost | wgpu::SurfaceError::Outdated) => {
                self.surface.configure(&self.device, &self.surface_config);
                return self.begin_frame();
            }
            Err(wgpu::SurfaceError::OutOfMemory) => {
                eprintln!("OutOfMemory: GPU may be lost");
                std::process::exit(1);
            }
            Err(e) => {
                eprintln!("Surface error: {:?}", e);
                return self.begin_frame();
            }
        };
        let view = frame
            .texture
            .create_view(&wgpu::TextureViewDescriptor::default());
        let encoder = self
            .device
            .create_command_encoder(&wgpu::CommandEncoderDescriptor {
                label: Some("Main Encoder"),
            });
        (frame, view, encoder)
    }

    pub fn end_frame(&mut self, frame: wgpu::SurfaceTexture, encoder: wgpu::CommandEncoder) {
        self.queue.submit(Some(encoder.finish()));
        frame.present();

        self.device.poll(wgpu::Maintain::Poll);
    }
}
