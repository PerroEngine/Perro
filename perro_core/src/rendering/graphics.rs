#![allow(unused)]#![allow(dead_code)]
use std::{borrow::Cow, collections::HashMap, fmt, ops::Range, sync::Arc};

use bytemuck::cast_slice;
use wgpu::{
    util::DeviceExt, Adapter, BindGroupDescriptor, BindGroupEntry, BindGroupLayout, BindingResource, BlendComponent, BlendFactor, BlendOperation, BlendState, BufferBinding, BufferBindingType, BufferDescriptor, BufferSize, BufferUsages, Color, ColorTargetState, ColorWrites, CommandEncoderDescriptor, Device, DeviceDescriptor, Features, FragmentState, Instance, Limits, LoadOp, MemoryHints, Operations, PipelineLayout, PipelineLayoutDescriptor, PowerPreference, Queue, RenderPass, RenderPassColorAttachment, RenderPassDescriptor, RenderPipeline, RenderPipelineDescriptor, RequestAdapterOptions, ShaderModuleDescriptor, ShaderSource, StoreOp, Surface, SurfaceConfiguration, TextureFormat, TextureViewDescriptor, VertexAttribute, VertexBufferLayout, VertexFormat, VertexState, VertexStepMode
};
use winit::{dpi::PhysicalSize, event_loop::EventLoopProxy, window::Window};

use crate::{asset_io::{load_asset, resolve_path}, font::{Font, FontAtlas}, ui_elements::ui_container::CornerRadius, vertex::Vertex, ImageTexture, Transform2D, Vector2};

#[cfg(target_arch = "wasm32")]
pub type Rc<T> = std::rc::Rc<T>;
#[cfg(not(target_arch = "wasm32"))]
pub type Rc<T> = std::sync::Arc<T>;

#[cfg(target_arch = "wasm32")]
pub type SharedWindow = std::rc::Rc<Window>;
#[cfg(not(target_arch = "wasm32"))]
pub type SharedWindow = std::sync::Arc<Window>;

pub struct TextureManager {
    textures: HashMap<String, ImageTexture>,
    bind_groups: HashMap<String, wgpu::BindGroup>, // Cache bind groups too
}

pub const VIRTUAL_WIDTH: f32 = 1920.0;
pub const VIRTUAL_HEIGHT: f32 = 1080.0;
const MAX_INSTANCES: usize = 10000;

impl fmt::Debug for TextureManager {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("TextureManager")
            .field("textures_keys", &self.textures.keys().collect::<Vec<_>>())
            .finish()
    }
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

#[repr(C)]
#[derive(Clone, Copy, Debug, bytemuck::Pod, bytemuck::Zeroable)]
struct RectInstance {
    transform_0: [f32; 4],
    transform_1: [f32; 4],
    transform_2: [f32; 4],
    transform_3: [f32; 4],
    color: [f32; 4],
    size: [f32; 2],
    pivot: [f32; 2],
    // Packed corner radius: xy components of all 4 corners
    corner_radius_xy: [f32; 4], // [top_left.xy, top_right.xy]
    corner_radius_zw: [f32; 4], // [bottom_right.xy, bottom_left.xy]
    border_thickness: f32,
    is_border: u32,
    z_index: i32,
    _pad: f32,
}

#[repr(C)]
#[derive(Clone, Copy, Debug, bytemuck::Pod, bytemuck::Zeroable)]
struct TextureInstance {
    transform_0: [f32; 4],
    transform_1: [f32; 4],
    transform_2: [f32; 4],
    transform_3: [f32; 4],
    pivot: [f32; 2],
    z_index: i32,
    _pad: f32,
}

#[derive(Clone, Debug)]
struct CachedRect {
    instance: RectInstance,
}

#[derive(Clone, Debug)]
struct CachedTexture {
    instance: TextureInstance,
    texture_path: String,
}

#[repr(C)]
#[derive(Clone, Copy, Debug, bytemuck::Pod, bytemuck::Zeroable)]
pub struct FontInstance {
    transform_0: [f32; 4],
    transform_1: [f32; 4],
    transform_2: [f32; 4],
    transform_3: [f32; 4],
    color: [f32; 4],
    uv_offset: [f32; 2],
    uv_size: [f32; 2],
    z_index: i32,
    _pad: [f32; 3],
}

#[derive(Debug)]
pub struct TextRenderer<'a> {
    pub atlas: &'a FontAtlas,
    pub font_instance_buffer: wgpu::Buffer,
    pub text_instances: Vec<FontInstance>,
    pub text_instanced_pipeline: RenderPipeline,
    pub font_bind_group_layout: BindGroupLayout,
}

#[derive(Debug)]
pub struct Graphics {
    window: Rc<Window>,
    instance: Instance,
    surface: Surface<'static>,
    surface_config: SurfaceConfiguration,
    adapter: Adapter,
    device: Device,
    queue: Queue,

    texture_manager: TextureManager,

    // Camera uniform
    camera_buffer: wgpu::Buffer,
    camera_bind_group_layout: BindGroupLayout,
    camera_bind_group: wgpu::BindGroup,

    // Vertex buffer (shared quad)
    vertex_buffer: wgpu::Buffer,

    // Retained mode caches - using UUID directly (no string conversion!)
    cached_rects: HashMap<uuid::Uuid, CachedRect>,
    cached_textures: HashMap<uuid::Uuid, CachedTexture>,

    // Instance buffers
    rect_instance_buffer: wgpu::Buffer,
    texture_instance_buffer: wgpu::Buffer,

    // Instanced pipelines
    rect_instanced_pipeline: RenderPipeline,
    texture_instanced_pipeline: RenderPipeline,
    texture_bind_group_layout: BindGroupLayout,

    // Font rendering components
    font_atlas: Option<FontAtlas>,
    font_bind_group: Option<wgpu::BindGroup>,
    font_bind_group_layout: BindGroupLayout,
    font_instanced_pipeline: RenderPipeline,
    font_instance_buffer: wgpu::Buffer,

    // Text instance cache
    cached_text: HashMap<uuid::Uuid, Vec<FontInstance>>,
    text_instances_need_rebuild: bool,
    all_text_instances: Vec<FontInstance>,

    // Optimization flags
    instances_need_rebuild: bool,

    // Pre-built instance data (cached)
    rect_instances: Vec<RectInstance>,
    texture_groups: Vec<(String, Vec<TextureInstance>)>,
    texture_group_offsets: Vec<(usize, usize)>, // (start_offset, count) for each group
    
    // Pre-allocated temporary vectors to avoid per-frame allocations
    temp_texture_map: HashMap<String, Vec<TextureInstance>>,
    temp_sorted_groups: Vec<(String, Vec<TextureInstance>)>,
    temp_all_texture_instances: Vec<TextureInstance>,
    
    // Pre-computed buffer ranges to avoid recalculation
    texture_buffer_ranges: Vec<Range<u64>>,
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
                required_features: Features::empty(),
                required_limits: Limits::downlevel_webgl2_defaults()
                    .using_resolution(adapter.limits()),
                memory_hints: MemoryHints::Performance,
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

    // 4) Texture bind group layout
    let texture_bind_group_layout =
        device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("Texture BGL"),
            entries: &[
                wgpu::BindGroupLayoutEntry {
                    binding: 0,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Sampler(wgpu::SamplerBindingType::Filtering),
                    count: None,
                },
                wgpu::BindGroupLayoutEntry {
                    binding: 1,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Texture {
                        multisampled: false,
                        view_dimension: wgpu::TextureViewDimension::D2,
                        sample_type: wgpu::TextureSampleType::Float { filterable: true },
                    },
                    count: None,
                },
            ],
        });

    // 4.5) Font bind group layout
    let font_bind_group_layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
        label: Some("Font BGL"),
        entries: &[
            wgpu::BindGroupLayoutEntry {
                binding: 0,
                visibility: wgpu::ShaderStages::FRAGMENT,
                ty: wgpu::BindingType::Texture {
                    multisampled: false,
                    view_dimension: wgpu::TextureViewDimension::D2,
                    sample_type: wgpu::TextureSampleType::Float { filterable: true },
                },
                count: None,
            },
            wgpu::BindGroupLayoutEntry {
                binding: 1,
                visibility: wgpu::ShaderStages::FRAGMENT,
                ty: wgpu::BindingType::Sampler(wgpu::SamplerBindingType::Filtering),
                count: None,
            },
        ],
    });

    // 5) Instance buffers
    let rect_instance_buffer = device.create_buffer(&BufferDescriptor {
        label: Some("Rect Instance Buffer"),
        size: (std::mem::size_of::<RectInstance>() * MAX_INSTANCES) as u64,
        usage: BufferUsages::VERTEX | BufferUsages::COPY_DST,
        mapped_at_creation: false,
    });

    let texture_instance_buffer = device.create_buffer(&BufferDescriptor {
        label: Some("Texture Instance Buffer"),
        size: (std::mem::size_of::<TextureInstance>() * MAX_INSTANCES) as u64,
        usage: BufferUsages::VERTEX | BufferUsages::COPY_DST,
        mapped_at_creation: false,
    });

    // 5.5) Font instance buffer
    let font_instance_buffer = device.create_buffer(&BufferDescriptor {
        label: Some("Font Instance Buffer"),
        size: (std::mem::size_of::<FontInstance>() * MAX_INSTANCES) as u64,
        usage: BufferUsages::VERTEX | BufferUsages::COPY_DST,
        mapped_at_creation: false,
    });

    // 6) Instanced pipelines
    let rect_instanced_pipeline = create_rect_instanced_pipeline(
        &device,
        &camera_bind_group_layout,
        surface_config.format,
    );

    let texture_instanced_pipeline = create_texture_instanced_pipeline(
        &device,
        &texture_bind_group_layout,
        &camera_bind_group_layout,
        surface_config.format,
    );

    // 6.5) Font instanced pipeline
    let font_instanced_pipeline = create_font_pipeline(
        &device,
        &font_bind_group_layout,
        &camera_bind_group_layout,
        surface_config.format,
    );

    // 7) Quad vertex buffer
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

    // 8) Initialize camera data with pre-computed scaling
    let virtual_width = VIRTUAL_WIDTH;
    let virtual_height = VIRTUAL_HEIGHT;
    let window_width = surface_config.width as f32;
    let window_height = surface_config.height as f32;
    
    // Pre-compute aspect scaling on CPU
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
        scale_x * 2.0 / virtual_width,  // Pre-computed NDC scaling
        scale_y * 2.0 / virtual_height,
    ];
    queue.write_buffer(&camera_buffer, 0, bytemuck::cast_slice(&camera_data));

    // 9) Finalize Graphics
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
        cached_rects: HashMap::new(),
        cached_textures: HashMap::new(),
        rect_instance_buffer,
        texture_instance_buffer,
        rect_instanced_pipeline,
        texture_instanced_pipeline,
        texture_bind_group_layout,
        
        // Font system fields
        font_atlas: None,
        font_bind_group: None,
        font_bind_group_layout,
        font_instanced_pipeline,
        font_instance_buffer,
        cached_text: HashMap::new(),
        text_instances_need_rebuild: false,
        all_text_instances: Vec::new(),
        
        instances_need_rebuild: false,
        rect_instances: Vec::new(),
        texture_groups: Vec::new(),
        texture_group_offsets: Vec::new(),
        // Pre-allocate temporary vectors
        temp_texture_map: HashMap::new(),
        temp_sorted_groups: Vec::new(),
        temp_all_texture_instances: Vec::new(),
        texture_buffer_ranges: Vec::new(),
    };

    let _ = proxy.send_event(gfx);
}

fn create_rect_instanced_pipeline(
    device: &Device,
    camera_bgl: &BindGroupLayout,
    format: TextureFormat,
) -> RenderPipeline {
    let shader = device.create_shader_module(ShaderModuleDescriptor {
        label: Some("Rect Instanced Shader"),
        source: ShaderSource::Wgsl(Cow::Borrowed(include_str!("shaders/rect_instanced.wgsl"))),
    });

    let pipeline_layout = device.create_pipeline_layout(&PipelineLayoutDescriptor {
        label: Some("Rect Instanced Pipeline Layout"),
        bind_group_layouts: &[camera_bgl],
        push_constant_ranges: &[],
    });

    device.create_render_pipeline(&RenderPipelineDescriptor {
        label: Some("Rect Instanced Pipeline"),
        layout: Some(&pipeline_layout),
        vertex: VertexState {
            module: &shader,
            entry_point: Some("vs_main"),
            buffers: &[
                // Vertex buffer (position + uv)
                VertexBufferLayout {
                    array_stride: std::mem::size_of::<Vertex>() as _,
                    step_mode: VertexStepMode::Vertex,
                    attributes: &[
                        VertexAttribute {
                            offset: 0,
                            shader_location: 0,
                            format: VertexFormat::Float32x2,
                        },
                        VertexAttribute {
                            offset: std::mem::size_of::<[f32; 2]>() as _,
                            shader_location: 1,
                            format: VertexFormat::Float32x2,
                        },
                    ],
                },
                // Instance buffer - updated for packed corner radius
                VertexBufferLayout {
                    array_stride: std::mem::size_of::<RectInstance>() as _,
                    step_mode: VertexStepMode::Instance,
                    attributes: &[
                        VertexAttribute { offset: 0, shader_location: 2, format: VertexFormat::Float32x4 },
                        VertexAttribute { offset: 16, shader_location: 3, format: VertexFormat::Float32x4 },
                        VertexAttribute { offset: 32, shader_location: 4, format: VertexFormat::Float32x4 },
                        VertexAttribute { offset: 48, shader_location: 5, format: VertexFormat::Float32x4 },
                        VertexAttribute { offset: 64, shader_location: 6, format: VertexFormat::Float32x4 },
                        VertexAttribute { offset: 80, shader_location: 7, format: VertexFormat::Float32x2 },
                        VertexAttribute { offset: 88, shader_location: 8, format: VertexFormat::Float32x2 },
                        // Packed corner radius (2 vec4s instead of 4)
                        VertexAttribute { offset: 96, shader_location: 9, format: VertexFormat::Float32x4 },
                        VertexAttribute { offset: 112, shader_location: 10, format: VertexFormat::Float32x4 },
                        VertexAttribute { offset: 128, shader_location: 11, format: VertexFormat::Float32 },
                        VertexAttribute { offset: 132, shader_location: 12, format: VertexFormat::Uint32 },
                        VertexAttribute { offset: 136, shader_location: 13, format: VertexFormat::Sint32 },
                    ],
                },
            ],
            compilation_options: Default::default(),
        },
        fragment: Some(FragmentState {
            module: &shader,
            entry_point: Some("fs_main"),
            targets: &[Some(ColorTargetState {
                format,
                blend: Some(BlendState::ALPHA_BLENDING),
                write_mask: ColorWrites::ALL,
            })],
            compilation_options: Default::default(),
        }),
        primitive: Default::default(),
        depth_stencil: None,
        multisample: Default::default(),
        multiview: None,
        cache: None,
    })
}

fn create_texture_instanced_pipeline(
    device: &Device,
    texture_bgl: &BindGroupLayout,
    camera_bgl: &BindGroupLayout,
    format: TextureFormat,
) -> RenderPipeline {
    let shader = device.create_shader_module(ShaderModuleDescriptor {
        label: Some("Sprite Instanced Shader"),
        source: ShaderSource::Wgsl(Cow::Borrowed(include_str!("shaders/sprite_instanced.wgsl"))),
    });

    let pipeline_layout = device.create_pipeline_layout(&PipelineLayoutDescriptor {
        label: Some("Sprite Instanced Pipeline Layout"),
        bind_group_layouts: &[texture_bgl, camera_bgl],
        push_constant_ranges: &[],
    });

    device.create_render_pipeline(&RenderPipelineDescriptor {
        label: Some("Sprite Instanced Pipeline"),
        layout: Some(&pipeline_layout),
        vertex: VertexState {
            module: &shader,
            entry_point: Some("vs_main"),
            buffers: &[
                // Vertex buffer
                VertexBufferLayout {
                    array_stride: std::mem::size_of::<Vertex>() as _,
                    step_mode: VertexStepMode::Vertex,
                    attributes: &[
                        VertexAttribute {
                            offset: 0,
                            shader_location: 0,
                            format: VertexFormat::Float32x2,
                        },
                        VertexAttribute {
                            offset: std::mem::size_of::<[f32; 2]>() as _,
                            shader_location: 1,
                            format: VertexFormat::Float32x2,
                        },
                    ],
                },
                // Instance buffer
                VertexBufferLayout {
                    array_stride: std::mem::size_of::<TextureInstance>() as _,
                    step_mode: VertexStepMode::Instance,
                    attributes: &[
                        VertexAttribute { offset: 0, shader_location: 2, format: VertexFormat::Float32x4 },
                        VertexAttribute { offset: 16, shader_location: 3, format: VertexFormat::Float32x4 },
                        VertexAttribute { offset: 32, shader_location: 4, format: VertexFormat::Float32x4 },
                        VertexAttribute { offset: 48, shader_location: 5, format: VertexFormat::Float32x4 },
                        VertexAttribute { offset: 64, shader_location: 6, format: VertexFormat::Float32x2 },
                        VertexAttribute { offset: 72, shader_location: 7, format: VertexFormat::Sint32 },
                    ],
                },
            ],
            compilation_options: Default::default(),
        },
        fragment: Some(FragmentState {
            module: &shader,
            entry_point: Some("fs_main"),
            targets: &[Some(ColorTargetState {
                format,
                blend: Some(BlendState::ALPHA_BLENDING),
                write_mask: ColorWrites::ALL,
            })],
            compilation_options: Default::default(),
        }),
        primitive: Default::default(),
        depth_stencil: None,
        multisample: Default::default(),
        multiview: None,
        cache: None,
    })
}

pub fn create_font_pipeline(
    device: &wgpu::Device,
    font_texture_bind_group_layout: &BindGroupLayout,
    camera_bgl: &BindGroupLayout,
    format: wgpu::TextureFormat,
) -> RenderPipeline {
    let shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
        label: Some("Font Instanced Shader"),
        source: wgpu::ShaderSource::Wgsl(Cow::Borrowed(include_str!("shaders/font_instanced.wgsl"))),
    });

    let pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
        label: Some("Font Pipeline Layout"),
        bind_group_layouts: &[font_texture_bind_group_layout, camera_bgl],
        push_constant_ranges: &[],
    });

    device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
        label: Some("Font Instanced Pipeline"),
        layout: Some(&pipeline_layout),
        vertex: wgpu::VertexState {
            module: &shader,
            entry_point: Some("vs_main"),
            buffers: &[
                // Quad vertex buffer
                wgpu::VertexBufferLayout {
                    array_stride: std::mem::size_of::<Vertex>() as _,
                    step_mode: wgpu::VertexStepMode::Vertex,
                    attributes: &[
                        wgpu::VertexAttribute { offset: 0, shader_location: 0, format: wgpu::VertexFormat::Float32x2 },
                        wgpu::VertexAttribute { offset: 8, shader_location: 1, format: wgpu::VertexFormat::Float32x2 },
                    ],
                },
                // Instance buffer
                wgpu::VertexBufferLayout {
                    array_stride: std::mem::size_of::<FontInstance>() as _,
                    step_mode: wgpu::VertexStepMode::Instance,
                    attributes: &[
                        wgpu::VertexAttribute { offset: 0, shader_location: 2, format: wgpu::VertexFormat::Float32x4 },
                        wgpu::VertexAttribute { offset: 16, shader_location: 3, format: wgpu::VertexFormat::Float32x4 },
                        wgpu::VertexAttribute { offset: 32, shader_location: 4, format: wgpu::VertexFormat::Float32x4 },
                        wgpu::VertexAttribute { offset: 48, shader_location: 5, format: wgpu::VertexFormat::Float32x4 },
                        wgpu::VertexAttribute { offset: 64, shader_location: 6, format: wgpu::VertexFormat::Float32x4 },
                        wgpu::VertexAttribute { offset: 80, shader_location: 7, format: wgpu::VertexFormat::Float32x2 },
                        wgpu::VertexAttribute { offset: 88, shader_location: 8, format: wgpu::VertexFormat::Float32x2 },
                        wgpu::VertexAttribute { offset: 96, shader_location: 9, format: wgpu::VertexFormat::Sint32 },
                    ],
                },
            ],
            compilation_options: Default::default(),
        },
        fragment: Some(wgpu::FragmentState {
            module: &shader,
            entry_point: Some("fs_main"),
            targets: &[Some(wgpu::ColorTargetState {
                format,
                blend: Some(wgpu::BlendState::ALPHA_BLENDING),
                write_mask: wgpu::ColorWrites::ALL,
            })],
            compilation_options: Default::default(),
        }),
        primitive: Default::default(),
        depth_stencil: None,
        multisample: Default::default(),
        multiview: None,
        cache: None,
    })
}

impl Graphics {
    pub fn window(&self) -> &winit::window::Window {
        &self.window
    }

    pub fn resize(&mut self, size: PhysicalSize<u32>) {
        self.surface_config.width = size.width.max(1);
        self.surface_config.height = size.height.max(1);
        self.surface.configure(&self.device, &self.surface_config);

        let virtual_width = VIRTUAL_WIDTH;
        let virtual_height = VIRTUAL_HEIGHT;
        let window_width = self.surface_config.width as f32;
        let window_height = self.surface_config.height as f32;
        
        // Pre-compute aspect scaling on CPU
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
            scale_x * 2.0 / virtual_width,  // Pre-computed NDC scaling
            scale_y * 2.0 / virtual_height,
        ];
        self.queue
            .write_buffer(&self.camera_buffer, 0, bytemuck::cast_slice(&camera_data));
    }

    // Initialize font atlas (call this once with your font)
    pub fn initialize_font_atlas(&mut self, font_atlas: FontAtlas) {
        // Create texture from font atlas bitmap
        let atlas_texture = self.device.create_texture(&wgpu::TextureDescriptor {
            label: Some("Font Atlas"),
            size: wgpu::Extent3d {
                width: font_atlas.width,
                height: font_atlas.height,
                depth_or_array_layers: 1,
            },
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: wgpu::TextureFormat::R8Unorm, // Single channel for SDF
            usage: wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::COPY_DST,
            view_formats: &[],
        });

        // Upload atlas bitmap data
        self.queue.write_texture(
            wgpu::ImageCopyTexture {
                texture: &atlas_texture,
                mip_level: 0,
                origin: wgpu::Origin3d::ZERO,
                aspect: wgpu::TextureAspect::All,
            },
            &font_atlas.bitmap, // Assuming this is Vec<u8>
            wgpu::ImageDataLayout {
                offset: 0,
                bytes_per_row: Some(font_atlas.width),
                rows_per_image: Some(font_atlas.height),
            },
            wgpu::Extent3d {
                width: font_atlas.width,
                height: font_atlas.height,
                depth_or_array_layers: 1,
            },
        );

        let atlas_view = atlas_texture.create_view(&wgpu::TextureViewDescriptor::default());
        let atlas_sampler = self.device.create_sampler(&wgpu::SamplerDescriptor {
            label: Some("Font Atlas Sampler"),
            address_mode_u: wgpu::AddressMode::ClampToEdge,
            address_mode_v: wgpu::AddressMode::ClampToEdge,
            address_mode_w: wgpu::AddressMode::ClampToEdge,
            mag_filter: wgpu::FilterMode::Linear,
            min_filter: wgpu::FilterMode::Linear,
            mipmap_filter: wgpu::FilterMode::Nearest,
            ..Default::default()
        });

        // Create bind group
        let font_bind_group = self.device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("Font Bind Group"),
            layout: &self.font_bind_group_layout,
            entries: &[
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: wgpu::BindingResource::TextureView(&atlas_view),
                },
                wgpu::BindGroupEntry {
                    binding: 1,
                    resource: wgpu::BindingResource::Sampler(&atlas_sampler),
                },
            ],
        });

        self.font_atlas = Some(font_atlas);
        self.font_bind_group = Some(font_bind_group);
    }

    pub fn stop_rendering(&mut self, uuid: uuid::Uuid) {
        // Direct UUID usage - no string allocation!
        self.cached_rects.remove(&uuid);
        self.cached_textures.remove(&uuid);
        self.cached_text.remove(&uuid);
        
        self.instances_need_rebuild = true; // Mark as dirty
        self.text_instances_need_rebuild = true;
    }
    
    pub fn draw_rect(
        &mut self,
        uuid: uuid::Uuid,
        transform: Transform2D,
        size: Vector2,
        pivot: Vector2,
        color: crate::Color,
        corner_radius: Option<CornerRadius>,
        border_thickness: f32,
        is_border: bool,
        z_index: i32,
    ) {
        fn srgb_to_linear(c: f32) -> f32 {
            if c <= 0.04045 { c / 12.92 } else { ((c + 0.055) / 1.055).powf(2.4) }
        }

        let color_lin = [
            srgb_to_linear(color.r as f32 / 255.0),
            srgb_to_linear(color.g as f32 / 255.0),
            srgb_to_linear(color.b as f32 / 255.0),
            color.a as f32 / 255.0,
        ];

       let cr = corner_radius.unwrap_or_default();

        let sx = transform.scale.x.abs();
        let sy = transform.scale.y.abs();

        let scaled_size_x = size.x * sx;
        let scaled_size_y = size.y * sy;

        // ✅ Calculate max possible radius (half of the smaller dimension)
        let max_radius = (scaled_size_x.min(scaled_size_y)) * 0.5;
        
        // ✅ Convert each corner's 0-1 value to actual pixels
        let corner_radius_xy = [
            cr.top_left * max_radius,      // e.g., 0.5 * 25px = 12.5px
            cr.top_left * max_radius,      // same for both x and y (circular)
            cr.top_right * max_radius,     // e.g., 0.7 * 25px = 17.5px
            cr.top_right * max_radius,
        ];
        let corner_radius_zw = [
            cr.bottom_right * max_radius,  // each corner can be different
            cr.bottom_right * max_radius,
            cr.bottom_left * max_radius,
            cr.bottom_left * max_radius,
        ];

        // Border thickness in actual pixels
        let pixel_border_thickness = border_thickness;

        let mut xf_no_scale = transform.clone();
        xf_no_scale.scale = Vector2::new(1.0, 1.0);

        let transform_array = xf_no_scale.to_mat4().to_cols_array();

        let instance = RectInstance {
            transform_0: [transform_array[0], transform_array[1], transform_array[2], transform_array[3]],
            transform_1: [transform_array[4], transform_array[5], transform_array[6], transform_array[7]],
            transform_2: [transform_array[8], transform_array[9], transform_array[10], transform_array[11]],
            transform_3: [transform_array[12], transform_array[13], transform_array[14], transform_array[15]],
            color: color_lin,
            size: [scaled_size_x, scaled_size_y],
            pivot: [pivot.x, pivot.y],
            corner_radius_xy,
            corner_radius_zw,
            border_thickness: pixel_border_thickness,
            is_border: if is_border { 1 } else { 0 },
            z_index,
            _pad: 0.0,
        };

        self.cached_rects.insert(uuid, CachedRect { instance });
        self.instances_need_rebuild = true;
    }

    pub fn draw_texture(
        &mut self,
        uuid: uuid::Uuid,
        texture_path: &str,
        transform: Transform2D,
        pivot: Vector2,
        z_index: i32,
    ) {
        let transform_array = transform.to_mat4().to_cols_array();
        let instance = TextureInstance {
            transform_0: [transform_array[0], transform_array[1], transform_array[2], transform_array[3]],
            transform_1: [transform_array[4], transform_array[5], transform_array[6], transform_array[7]],
            transform_2: [transform_array[8], transform_array[9], transform_array[10], transform_array[11]],
            transform_3: [transform_array[12], transform_array[13], transform_array[14], transform_array[15]],
            pivot: [pivot.x, pivot.y],
            z_index,
            _pad: 0.0,
        };

        // Direct UUID usage - no string allocation!
        self.cached_textures.insert(
            uuid,
            CachedTexture {
                instance,
                texture_path: texture_path.to_string(),
            },
        );
        self.instances_need_rebuild = true; // Mark as dirty
    }

    // Fixed draw_text method
  // Simplified draw_text method - closer to your original
pub fn draw_text(
    &mut self,
    uuid: uuid::Uuid,
    text: &str,
    font_size: f32,
    transform: Transform2D,
    pivot: Vector2,
    color: crate::Color,
    z_index: i32,
) {
    if let Some(ref font_atlas) = self.font_atlas {
        let mut cursor_x = 0.0;
        let mut cursor_y = 0.0;
        let mut instances = Vec::new();

        // Convert color to linear space
        fn srgb_to_linear(c: f32) -> f32 {
            if c <= 0.04045 { c / 12.92 } else { ((c + 0.055) / 1.055).powf(2.4) }
        }

        let color_lin = [
            srgb_to_linear(color.r as f32 / 255.0),
            srgb_to_linear(color.g as f32 / 255.0),
            srgb_to_linear(color.b as f32 / 255.0),
            color.a as f32 / 255.0,
        ];

        for ch in text.chars() {
            if let Some(glyph) = font_atlas.glyphs.get(&ch) {
                // Calculate glyph position and size - keep it simple like your original
                let glyph_x = transform.position.x + cursor_x + (glyph.x_offset as f32 * font_size);
                let glyph_y = transform.position.y + cursor_y + (glyph.y_offset as f32 * font_size);
                let glyph_w = glyph.width as f32 * font_size;
                let glyph_h = glyph.height as f32 * font_size;

                // Create transform matrix for this glyph
                let glyph_transform = Transform2D {
                    position: Vector2::new(glyph_x, glyph_y),
                    rotation: transform.rotation,
                    scale: Vector2::new(glyph_w, glyph_h),
                };

                let transform_array = glyph_transform.to_mat4().to_cols_array();

                let instance = FontInstance {
                    transform_0: [transform_array[0], transform_array[1], transform_array[2], transform_array[3]],
                    transform_1: [transform_array[4], transform_array[5], transform_array[6], transform_array[7]],
                    transform_2: [transform_array[8], transform_array[9], transform_array[10], transform_array[11]],
                    transform_3: [transform_array[12], transform_array[13], transform_array[14], transform_array[15]],
                    color: color_lin,
                    uv_offset: [
                        glyph.x as f32 / font_atlas.width as f32,
                        glyph.y as f32 / font_atlas.height as f32,
                    ],
                    uv_size: [
                        glyph.width as f32 / font_atlas.width as f32,
                        glyph.height as f32 / font_atlas.height as f32,
                    ],
                    z_index,
                    _pad: [0.0; 3],
                };

                instances.push(instance);
                cursor_x += glyph.advance * font_size;
            }
        }

        println!("Generated {} font instances for text: '{}'", instances.len(), text);
        self.cached_text.insert(uuid, instances);
        self.text_instances_need_rebuild = true;
    } else {
        println!("No font atlas available!");
    }
}

    fn rebuild_instances(&mut self) {
        // Rebuild rect instances - reuse vector
        self.rect_instances.clear();
        self.rect_instances.extend(
            self.cached_rects
                .values()
                .map(|cached| cached.instance)
        );
        self.rect_instances.sort_by(|a, b| a.z_index.cmp(&b.z_index));

        // Upload rect instances to GPU once
        if !self.rect_instances.is_empty() {
            self.queue.write_buffer(
                &self.rect_instance_buffer,
                0,
                bytemuck::cast_slice(&self.rect_instances),
            );
        }

        // Rebuild texture groups using pre-allocated vectors
        self.texture_groups.clear();
        self.texture_group_offsets.clear();
        self.texture_buffer_ranges.clear();
        
        // Reuse pre-allocated vectors
        self.temp_all_texture_instances.clear();
        self.temp_texture_map.clear();
        
        for cached in self.cached_textures.values() {
            self.temp_texture_map
                .entry(cached.texture_path.clone())
                .or_default()
                .push(cached.instance);
        }

        // Sort texture groups by minimum z-index - reuse vector
        self.temp_sorted_groups.clear();
        self.temp_sorted_groups.extend(self.temp_texture_map.drain());
        self.temp_sorted_groups.sort_by(|a, b| {
            let min_z_a = a.1.iter().map(|c| c.z_index).min().unwrap_or(0);
            let min_z_b = b.1.iter().map(|c| c.z_index).min().unwrap_or(0);
            min_z_a.cmp(&min_z_b)
        });

        // Build one big buffer with all texture instances
        for (path, mut instances) in self.temp_sorted_groups.drain(..) {
            instances.sort_by(|a, b| a.z_index.cmp(&b.z_index));
            
            let start_offset = self.temp_all_texture_instances.len();
            let count = instances.len();
            
            // Pre-compute buffer ranges
            let start_byte = start_offset * std::mem::size_of::<TextureInstance>();
            let size_bytes = count * std::mem::size_of::<TextureInstance>();
            let range = (start_byte as u64)..((start_byte + size_bytes) as u64);
            
            self.temp_all_texture_instances.extend(instances.clone());
            self.texture_groups.push((path, instances));
            self.texture_group_offsets.push((start_offset, count));
            self.texture_buffer_ranges.push(range);
        }

        // Upload ALL texture instances to GPU once
        if !self.temp_all_texture_instances.is_empty() {
            self.queue.write_buffer(
                &self.texture_instance_buffer,
                0,
                bytemuck::cast_slice(&self.temp_all_texture_instances),
            );
        }
    }

    fn rebuild_text_instances(&mut self) {
        self.all_text_instances.clear();
        
        // Collect all text instances and sort by z-index
        for instances in self.cached_text.values() {
            self.all_text_instances.extend(instances.iter().cloned());
        }
        
        self.all_text_instances.sort_by(|a, b| a.z_index.cmp(&b.z_index));

        // Upload to GPU
        if !self.all_text_instances.is_empty() {
            self.queue.write_buffer(
                &self.font_instance_buffer,
                0,
                bytemuck::cast_slice(&self.all_text_instances),
            );
        }
    }

    pub fn draw_instances(&mut self, rpass: &mut RenderPass<'_>) {
        // Only rebuild if something changed
        if self.instances_need_rebuild {
            self.rebuild_instances();
            self.instances_need_rebuild = false;
        }
        
        if self.text_instances_need_rebuild {
            self.rebuild_text_instances();
            self.text_instances_need_rebuild = false;
        }

        // Fast path - just issue draw commands, no CPU work
        if !self.rect_instances.is_empty() {
            rpass.set_pipeline(&self.rect_instanced_pipeline);
            rpass.set_bind_group(0, &self.camera_bind_group, &[]);
            rpass.set_vertex_buffer(0, self.vertex_buffer.slice(..));
            rpass.set_vertex_buffer(1, self.rect_instance_buffer.slice(..));
            rpass.draw(0..6, 0..self.rect_instances.len() as u32);
        }

        // Draw texture groups - using pre-computed ranges
        for (i, (texture_path, _)) in self.texture_groups.iter().enumerate() {
            let (_, count) = self.texture_group_offsets[i];
            
            if count > 0 {
                // Get cached bind group (no creation)
                let tex_bg = self.texture_manager.get_or_create_bind_group(
                    texture_path,
                    &self.device,
                    &self.queue,
                    &self.texture_bind_group_layout,
                );

                // Draw this texture group using pre-computed buffer slice
                rpass.set_pipeline(&self.texture_instanced_pipeline);
                rpass.set_bind_group(0, tex_bg, &[]);
                rpass.set_bind_group(1, &self.camera_bind_group, &[]);
                rpass.set_vertex_buffer(0, self.vertex_buffer.slice(..));
                
                // Use pre-computed range - no calculation needed!
                let buffer_slice = self.texture_instance_buffer.slice(
                    self.texture_buffer_ranges[i].clone()
                );
                
                rpass.set_vertex_buffer(1, buffer_slice);
                rpass.draw(0..6, 0..count as u32);
            }
        }

        // Draw text
        if !self.all_text_instances.is_empty() && self.font_bind_group.is_some() {
            rpass.set_pipeline(&self.font_instanced_pipeline);
            rpass.set_bind_group(0, self.font_bind_group.as_ref().unwrap(), &[]);
            rpass.set_bind_group(1, &self.camera_bind_group, &[]);
            rpass.set_vertex_buffer(0, self.vertex_buffer.slice(..));
            rpass.set_vertex_buffer(1, self.font_instance_buffer.slice(..));
            rpass.draw(0..6, 0..self.all_text_instances.len() as u32);
        }
    }

    pub fn begin_frame(&mut self) -> (wgpu::SurfaceTexture, wgpu::TextureView, wgpu::CommandEncoder) {
        let frame = self.surface.get_current_texture().expect("Failed to get next frame");
        let view = frame.texture.create_view(&TextureViewDescriptor::default());
        let encoder = self.device.create_command_encoder(&CommandEncoderDescriptor {
            label: Some("Main Encoder"),
        });
        (frame, view, encoder)
    }

    pub fn end_frame(&mut self, frame: wgpu::SurfaceTexture, encoder: wgpu::CommandEncoder) {
        self.queue.submit(Some(encoder.finish()));
        frame.present();
    }
}