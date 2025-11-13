use std::{borrow::Cow, collections::HashMap, fmt, ops::Range, sync::Arc};

use bytemuck::cast_slice;
use wgpu::{
    util::DeviceExt, Adapter, BindGroupDescriptor, BindGroupEntry, BindGroupLayout, 
    BindingResource, BufferBinding, BufferBindingType, BufferDescriptor, BufferSize, 
    BufferUsages, CommandEncoderDescriptor, Device, DeviceDescriptor, Features, 
    Instance, Limits, MemoryHints, Queue, RenderPass, RequestAdapterOptions, 
    SurfaceConfiguration, TextureFormat, TextureViewDescriptor, PowerPreference
};
use winit::{dpi::PhysicalSize, event_loop::EventLoopProxy, window::Window};

use crate::{
    asset_io::load_asset,
    font::FontAtlas,
    renderer_prim::PrimitiveRenderer,
    renderer_2d::Renderer2D,
    renderer_ui::RendererUI,
    renderer_3d::Renderer3D,
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
    pub camera_buffer: wgpu::Buffer,
    pub camera_bind_group_layout: BindGroupLayout,
    pub camera_bind_group: wgpu::BindGroup,
    pub vertex_buffer: wgpu::Buffer,

    // Specialized renderers
    pub renderer_prim: PrimitiveRenderer,
    pub renderer_2d: Renderer2D,
    pub renderer_ui: RendererUI,
    pub renderer_3d: Renderer3D,

    // Cached render state
    cached_operations: wgpu::Operations<wgpu::Color>,
}

pub async fn create_graphics(
    window: SharedWindow,
    proxy: EventLoopProxy<Graphics>,
) {
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

    // 3) Camera uniform buffer
    let camera_buffer = device.create_buffer(&BufferDescriptor {
        label: Some("Camera UBO"),
        size: 16, // vec4<f32> = 16 bytes
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
                    min_binding_size: BufferSize::new(16),
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
                size: BufferSize::new(16),
            }),
        }],
    });

    // 4) Quad vertex buffer
    let vertices: &[Vertex] = &[
        Vertex { position: [-0.5, -0.5], uv: [0.0, 1.0] },
        Vertex { position: [0.5, -0.5], uv: [1.0, 1.0] },
        Vertex { position: [0.5, 0.5], uv: [1.0, 0.0] },
        Vertex { position: [-0.5, -0.5], uv: [0.0, 1.0] },
        Vertex { position: [0.5, 0.5], uv: [1.0, 0.0] },
        Vertex { position: [-0.5, 0.5], uv: [0.0, 0.0] },
    ];
    let vertex_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
        label: Some("Vertex Buffer"),
        contents: bytemuck::cast_slice(vertices),
        usage: BufferUsages::VERTEX,
    });

    // 5) Initialize camera data
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

    // 6) Create renderers

    let renderer_3d = Renderer3D::new(&device, &camera_bind_group_layout, surface_config.format);
    let renderer_prim = PrimitiveRenderer::new(
        &device, 
        &camera_bind_group_layout, 
        surface_config.format
    );
    let renderer_2d = Renderer2D::new();
    let renderer_ui = RendererUI::new();

    let gfx = Graphics {
        window: window.clone(),
        instance,
        surface,
        surface_config,
        adapter,
        device,
        queue,
        texture_manager: TextureManager::new(),
        camera_buffer,
        camera_bind_group_layout,
        camera_bind_group,
        vertex_buffer,
        renderer_prim,
        renderer_2d,
        renderer_ui,
        renderer_3d,
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
        
        self.queue.write_buffer(
            &self.camera_buffer, 
            0, 
            bytemuck::cast_slice(&camera_data)
        );
    }

    pub fn initialize_font_atlas(&mut self, font_atlas: FontAtlas) {
        self.renderer_prim.initialize_font_atlas(
            &self.device, 
            &self.queue, 
            font_atlas
        );
    }

    pub fn stop_rendering(&mut self, uuid: uuid::Uuid) {
        self.renderer_prim.stop_rendering(uuid);
    }

    /// Main render method that coordinates all renderers
    pub fn render(&mut self, rpass: &mut RenderPass<'_>) {
        // Render 3D world first (when implemented)
        self.renderer_3d.render(
            rpass,
            &self.device,
            &self.queue,
            &self.camera_bind_group,
            &self.vertex_buffer,
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

    pub fn begin_frame(&mut self) -> (wgpu::SurfaceTexture, wgpu::TextureView, wgpu::CommandEncoder) {
        let frame = self.surface.get_current_texture().expect("Failed to get next frame");
        let view = frame.texture.create_view(&wgpu::TextureViewDescriptor::default());
        let encoder = self.device.create_command_encoder(&wgpu::CommandEncoderDescriptor {
            label: Some("Main Encoder"),
        });
        (frame, view, encoder)
    }

    pub fn end_frame(&mut self, frame: wgpu::SurfaceTexture, encoder: wgpu::CommandEncoder) {
        self.queue.submit(Some(encoder.finish()));
        frame.present();
    }
}