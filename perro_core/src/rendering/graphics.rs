use std::{borrow::Cow, collections::HashMap, fmt, ops::Range, sync::Arc};

use bytemuck::cast_slice;
use wgpu::{
    Adapter, BindGroupDescriptor, BindGroupEntry, BindGroupLayout, BindingResource, BufferBinding,
    BufferBindingType, BufferDescriptor, BufferSize, BufferUsages, CommandEncoderDescriptor,
    Device, DeviceDescriptor, Features, Instance, Limits, MemoryHints, PowerPreference, Queue,
    RenderPass, RequestAdapterOptions, SurfaceConfiguration, TextureFormat, TextureViewDescriptor,
    util::DeviceExt,
};
use winit::{dpi::PhysicalSize, event_loop::EventLoopProxy, window::Window};

use crate::{
    Camera2D, Camera3D,
    asset_io::load_asset,
    font::FontAtlas,
    renderer_2d::Renderer2D,
    renderer_3d::{Mesh, Renderer3D},
    renderer_prim::PrimitiveRenderer,
    renderer_ui::RendererUI,
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
            let img_bytes = load_asset(path).expect("Failed to read image file");
            let img = image::load_from_memory(&img_bytes).expect("Failed to decode image");
            println!(
                "🖼️ Loading texture: {} ({}x{})",
                path,
                img.width(),
                img.height()
            );
            let img_texture = ImageTexture::from_image(&img, device, queue);
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

    fn load_mesh_from_file(path: &str, device: &Device) -> Option<Mesh> {
        // TODO: Implement .glb/.gltf loading
        // For now, return built-in meshes
        if path == "cube" || path.contains("cube") {
            Some(crate::renderer_3d::Renderer3D::create_cube_mesh(device))
        } else {
            Some(crate::renderer_3d::Renderer3D::create_cube_mesh(device))
        }
    }

    pub fn add_mesh(&mut self, name: String, mesh: Mesh) {
        self.meshes.insert(name, mesh);
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
    pub camera_buffer: wgpu::Buffer,
    pub camera_bind_group_layout: BindGroupLayout,
    pub camera_bind_group: wgpu::BindGroup,
    pub vertex_buffer: wgpu::Buffer,

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

pub async fn create_graphics(window: SharedWindow, proxy: EventLoopProxy<Graphics>) {
    // 1) Instance / Surface / Adapter / Device+Queue
    let instance = Instance::default();
    let surface = instance.create_surface(Rc::clone(&window)).unwrap();
    let adapter = instance
        .request_adapter(&RequestAdapterOptions {
            power_preference: PowerPreference::default(),
            force_fallback_adapter: false,
            compatible_surface: Some(&surface),
        })
        .await
        .expect("No GPU adapter");
    let (device, queue) = adapter
        .request_device(
            &DeviceDescriptor {
                label: None,
                required_features: wgpu::Features::empty(),
                required_limits: wgpu::Limits::downlevel_webgl2_defaults()
                    .using_resolution(adapter.limits()),
                memory_hints: wgpu::MemoryHints::Performance,
            },
            None,
        )
        .await
        .expect("Failed to get device");

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
                // CHANGE THIS: Add FRAGMENT stage visibility
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

    // Initialize 3D camera matrices
    let view = glam::Mat4::look_at_rh(
        glam::vec3(3.0, 3.0, 3.0), // Camera position (back up so we can see the cube)
        glam::vec3(0.0, 0.0, 0.0), // Look at origin
        glam::vec3(0.0, 1.0, 0.0), // Up vector
    );

    let aspect_ratio = surface_config.width as f32 / surface_config.height as f32;
    let projection = glam::Mat4::perspective_rh(
        45.0_f32.to_radians(), // Field of view
        aspect_ratio,
        0.1,   // Near plane
        100.0, // Far plane
    );

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
    let renderer_3d = Renderer3D::new(&device, &camera3d_bind_group_layout, surface_config.format);
    let renderer_prim =
        PrimitiveRenderer::new(&device, &camera_bind_group_layout, surface_config.format);
    let renderer_2d = Renderer2D::new();
    let renderer_ui = RendererUI::new();

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
        camera_buffer,
        camera_bind_group_layout,
        camera_bind_group,
        vertex_buffer,

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

    let _ = proxy.send_event(gfx);
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
        // Translate first, then rotate: world-space panning works
        let view = rotation * translation;

        let vw = super::VIRTUAL_WIDTH;
        let vh = super::VIRTUAL_HEIGHT;
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

    pub fn update_camera_3d(&self, cam: &Camera3D) {
        let t = &cam.transform;

        // Use the quaternion directly instead of converting to/from Euler angles
        let translation = glam::Mat4::from_translation(t.position.to_glam());
        let rotation = glam::Mat4::from_quat(t.rotation.to_glam());

        // Build the model (camera) transform
        let model = translation * rotation;

        // View matrix is the inverse of the camera's model transform
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
    }
    pub fn initialize_font_atlas(&mut self, font_atlas: FontAtlas) {
        self.renderer_prim
            .initialize_font_atlas(&self.device, &self.queue, font_atlas);
    }

    pub fn stop_rendering(&mut self, uuid: uuid::Uuid) {
        self.renderer_prim.stop_rendering(uuid);
    }

    /// Main render method that coordinates all renderers
    pub fn render(&mut self, rpass: &mut RenderPass<'_>) {
        self.renderer_3d.upload_lights_to_gpu(&self.queue);

        self.renderer_3d.render(
            rpass,
            &self.mesh_manager,
            &self.camera3d_bind_group,
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

        // Prevent GPU race conditions between frames (especially on Windows/Vulkan)
        self.device.poll(wgpu::Maintain::Poll);
    }
}
