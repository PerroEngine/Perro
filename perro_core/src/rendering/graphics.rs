#![allow(unused)]#![allow(dead_code)]
use std::{borrow::Cow, collections::HashMap, fmt, path::PathBuf, sync::Arc};

use bytemuck::cast_slice;
use wgpu::{
    util::DeviceExt, Adapter, BindGroupDescriptor, BindGroupEntry, BindGroupLayout, BindingResource, BlendComponent, BlendFactor, BlendOperation, BlendState, BufferBinding, BufferBindingType, BufferDescriptor, BufferSize, BufferUsages, Color, ColorTargetState, ColorWrites, CommandEncoderDescriptor, Device, DeviceDescriptor, Features, FragmentState, Instance, Limits, LoadOp, MemoryHints, Operations, PipelineLayout, PipelineLayoutDescriptor, PowerPreference, Queue, RenderPass, RenderPassColorAttachment, RenderPassDescriptor, RenderPipeline, RenderPipelineDescriptor, RequestAdapterOptions, ShaderModuleDescriptor, ShaderSource, StoreOp, Surface, SurfaceConfiguration, TextureFormat, TextureViewDescriptor, VertexAttribute, VertexBufferLayout, VertexFormat, VertexState, VertexStepMode
};
use winit::{dpi::PhysicalSize, event_loop::EventLoopProxy, window::Window};

use crate::{resolve_res_path, ui_elements::ui_panel::CornerRadius, vertex::Vertex, ImageTexture, Transform2D, Vector2};

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
}


const VIRTUAL_WIDTH: f32 = 1920.0;
const VIRTUAL_HEIGHT: f32 = 1080.0;

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
        }
    }

    pub fn get_or_load_texture_sync(
        &mut self,
        path: &str,
        device: &Device,
        queue: &Queue,
    ) -> &ImageTexture {
        let actual_path = resolve_res_path(path);
        let key = actual_path.to_string_lossy().to_string();
        if !self.textures.contains_key(&key) {
            let img_bytes =
                std::fs::read(&actual_path).expect("Failed to read image file");
            let img = image::load_from_memory(&img_bytes)
                .expect("Failed to decode image");
            let img_texture = ImageTexture::from_image(&img, device, queue);
            self.textures.insert(key.clone(), img_texture);
        }
        self.textures.get(&key).unwrap()
    }
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

    /// Big uniform buffer for all sprite transforms
    transform_buffer: wgpu::Buffer,
    uniform_bind_group_layout: BindGroupLayout,
    uniform_bind_group: wgpu::BindGroup,
    min_offset: u32,
    next_offset: u32,

    texture_bind_group_layout: BindGroupLayout,
    pipeline_layout: PipelineLayout,
    render_pipeline: RenderPipeline,

    vertex_buffer: wgpu::Buffer,

    // ▼—— NEW FIELDS FOR SOLID‐COLOR QUADS ——▼
    color_bg: wgpu::BindGroup,
    color_pipeline: RenderPipeline,
    aspect_buffer: wgpu::Buffer,
    rect_uniform_buffer: wgpu::Buffer,
    rect_bgl: BindGroupLayout,
    rect_bg: wgpu::BindGroup,
    rect_uniform_size: u64,
    next_rect_offset: u32,
    // ▲———————————————————————————————▲
}

#[repr(C)]
#[derive(Clone, Copy, bytemuck::Pod, bytemuck::Zeroable)]
struct RectUniform {
    transform: [[f32; 4]; 4],
    color: [f32; 4],
    size: [f32; 2],
    pivot: [f32; 2],
    corner_radius: [[f32; 4]; 4],
    border_thickness: f32,
    is_border: u32,
    _pad: [f32; 2],
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
    let surface_config = surface
        .get_default_config(&adapter, w, h)
        .unwrap();
    #[cfg(not(target_arch = "wasm32"))]
    surface.configure(&device, &surface_config);

    // 3) Dynamic‐offset UBO for transforms (textured quads)
    const MAX_SPRITES: u32 = 1024;
    let min_offset = device.limits().min_uniform_buffer_offset_alignment as u32;
    let big_ubo_size = (min_offset as u64) * (MAX_SPRITES as u64);
    let transform_buffer = device.create_buffer(&BufferDescriptor {
        label: Some("Big Transform UBO"),
        size: big_ubo_size,
        usage: BufferUsages::UNIFORM | BufferUsages::COPY_DST,
        mapped_at_creation: false,
    });
    let uniform_bind_group_layout =
        device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("Dynamic-UBO BGL"),
            entries: &[wgpu::BindGroupLayoutEntry {
                binding: 0,
                visibility: wgpu::ShaderStages::VERTEX,
                ty: wgpu::BindingType::Buffer {
                    ty: BufferBindingType::Uniform,
                    has_dynamic_offset: true,
                    min_binding_size: BufferSize::new(64),
                },
                count: None,
            }],
        });
    let uniform_bind_group = device.create_bind_group(&BindGroupDescriptor {
        label: Some("Dynamic-UBO BG"),
        layout: &uniform_bind_group_layout,
        entries: &[BindGroupEntry {
            binding: 0,
            resource: BindingResource::Buffer(BufferBinding {
                buffer: &transform_buffer,
                offset: 0,
                size: BufferSize::new(64),
            }),
        }],
    });

    // 4) Textured‐quad pipeline
    let texture_bind_group_layout =
        device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("Texture BGL"),
            entries: &[
                wgpu::BindGroupLayoutEntry {
                    binding: 0,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Sampler(
                        wgpu::SamplerBindingType::Filtering,
                    ),
                    count: None,
                },
                wgpu::BindGroupLayoutEntry {
                    binding: 1,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Texture {
                        multisampled: false,
                        view_dimension: wgpu::TextureViewDimension::D2,
                        sample_type: wgpu::TextureSampleType::Float {
                            filterable: true,
                        },
                    },
                    count: None,
                },
            ],
        });
    let pipeline_layout = device.create_pipeline_layout(
        &wgpu::PipelineLayoutDescriptor {
            label: Some("Texture Pipeline Layout"),
            bind_group_layouts: &[
                &texture_bind_group_layout,
                &uniform_bind_group_layout,
            ],
            push_constant_ranges: &[],
        },
    );
    let render_pipeline =
        create_pipeline(&device, &pipeline_layout, surface_config.format);

    // 5) Dynamic UBO for solid-color quads
    const MAX_RECTS: u32 = 1024;
    let min_offset_rect = device.limits().min_uniform_buffer_offset_alignment as u64;
    let rect_uniform_size = ((std::mem::size_of::<RectUniform>() as u64 + min_offset_rect - 1)
        / min_offset_rect)
        * min_offset_rect;

    let rect_uniform_buffer = device.create_buffer(&wgpu::BufferDescriptor {
        label: Some("Rect Dynamic UBO"),
        size: rect_uniform_size * MAX_RECTS as u64,
        usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
        mapped_at_creation: false,
    });

    let rect_bgl = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
        label: Some("Rect Dynamic BGL"),
        entries: &[wgpu::BindGroupLayoutEntry {
            binding: 0,
            visibility: wgpu::ShaderStages::VERTEX | wgpu::ShaderStages::FRAGMENT,
            ty: wgpu::BindingType::Buffer {
                ty: wgpu::BufferBindingType::Uniform,
                has_dynamic_offset: true,
                min_binding_size: BufferSize::new(std::mem::size_of::<RectUniform>() as u64),
            },
            count: None,
        }],
    });

    let rect_bg = device.create_bind_group(&wgpu::BindGroupDescriptor {
        label: Some("Rect Dynamic BG"),
        layout: &rect_bgl,
        entries: &[wgpu::BindGroupEntry {
            binding: 0,
            resource: wgpu::BindingResource::Buffer(wgpu::BufferBinding {
                buffer: &rect_uniform_buffer,
                offset: 0,
                size: BufferSize::new(std::mem::size_of::<RectUniform>() as u64),
            }),
        }],
    });

    // 6) Global aspect ratio uniform
    let aspect_buffer = device.create_buffer(&wgpu::BufferDescriptor {
    label: Some("Camera UBO"),
    size: 16, // vec4<f32> = 16 bytes
    usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
    mapped_at_creation: false,
});

    let aspect_bgl = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
    label: Some("Aspect Ratio BGL"),
    entries: &[wgpu::BindGroupLayoutEntry {
        binding: 0,
        visibility: wgpu::ShaderStages::VERTEX,
        ty: wgpu::BindingType::Buffer {
            ty: wgpu::BufferBindingType::Uniform,
            has_dynamic_offset: false,
            min_binding_size: BufferSize::new(16), // vec4<f32> = 16 bytes
        },
        count: None,
    }],
});

let aspect_bg = device.create_bind_group(&wgpu::BindGroupDescriptor {
    label: Some("Aspect Ratio BG"),
    layout: &aspect_bgl,
    entries: &[wgpu::BindGroupEntry {
        binding: 0,
        resource: wgpu::BindingResource::Buffer(wgpu::BufferBinding {
            buffer: &aspect_buffer,
            offset: 0,
            size: BufferSize::new(16), // ✅ vec4<f32> = 16 bytes
        }),
    }],
});

    // 7) Color quad pipeline
    let shader_color = device.create_shader_module(ShaderModuleDescriptor {
        label: Some("Color Shader"),
        source: ShaderSource::Wgsl(Cow::Borrowed(
            include_str!("shaders/color_quad.wgsl"),
        )),
    });
    let pipeline_layout_color = device.create_pipeline_layout(
        &PipelineLayoutDescriptor {
            label: Some("Color Pipeline Layout"),
            bind_group_layouts: &[&rect_bgl, &aspect_bgl],
            push_constant_ranges: &[],
        },
    );
    let color_pipeline = device.create_render_pipeline(
        &RenderPipelineDescriptor {
            label: Some("Color Quad Pipeline"),
            layout: Some(&pipeline_layout_color),
            vertex: VertexState {
                module: &shader_color,
                entry_point: Some("vs_main"),
                buffers: &[VertexBufferLayout {
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
                }],
                compilation_options: Default::default(),
            },
            fragment: Some(FragmentState {
                module: &shader_color,
                entry_point: Some("fs_main"),
                targets: &[Some(ColorTargetState {
                    format: surface_config.format,
                    blend: Some(BlendState::REPLACE),
                    write_mask: ColorWrites::ALL,
                })],
                compilation_options: Default::default(),
            }),
            primitive: Default::default(),
            depth_stencil: None,
            multisample: Default::default(),
            multiview: None,
            cache: None,
        },
    );

    // 8) Quad vertex buffer
    let vertices: &[Vertex] = &[
        Vertex { position: [-0.5, -0.5], uv: [0.0, 1.0] },
        Vertex { position: [ 0.5, -0.5], uv: [1.0, 1.0] },
        Vertex { position: [ 0.5,  0.5], uv: [1.0, 0.0] },
        Vertex { position: [-0.5, -0.5], uv: [0.0, 1.0] },
        Vertex { position: [ 0.5,  0.5], uv: [1.0, 0.0] },
        Vertex { position: [-0.5,  0.5], uv: [0.0, 0.0] },
    ];
    let vertex_buffer = device.create_buffer_init(
        &wgpu::util::BufferInitDescriptor {
            label: Some("Vertex Buffer"),
            contents: bytemuck::cast_slice(vertices),
            usage: BufferUsages::VERTEX,
        },
    );

let virtual_width = VIRTUAL_WIDTH;
let virtual_height = VIRTUAL_HEIGHT;
let window_width = surface_config.width as f32;
let window_height = surface_config.height as f32;

let camera_data = [virtual_width, virtual_height, window_width, window_height];
queue.write_buffer(&aspect_buffer, 0, bytemuck::cast_slice(&camera_data));

    // 10) Finalize Graphics
    let gfx = Graphics {
        window: window.clone(),
        instance,
        surface,
        surface_config,
        adapter,
        device,
        queue,
        texture_manager: TextureManager::new(),
        transform_buffer,
        uniform_bind_group_layout,
        uniform_bind_group,
        min_offset,
        next_offset: 0,
        texture_bind_group_layout,
        pipeline_layout,
        render_pipeline,
        rect_uniform_buffer,
        rect_bgl,
        rect_bg,
        rect_uniform_size,
        next_rect_offset: 0,
        color_bg: aspect_bg, // store aspect bind group here
        color_pipeline,
        aspect_buffer,
        vertex_buffer,
    };
    
    let _ = proxy.send_event(gfx);
}
fn create_pipeline(
    device: &Device,
    pipeline_layout: &PipelineLayout,
    swap_chain_format: TextureFormat,
) -> RenderPipeline {
    let vertex_buffer_layout = VertexBufferLayout {
        array_stride: std::mem::size_of::<Vertex>() as wgpu::BufferAddress,
        step_mode: VertexStepMode::Vertex,
        attributes: &[
            VertexAttribute {
                offset: 0,
                shader_location: 0,
                format: VertexFormat::Float32x2,
            },
            VertexAttribute {
                offset: std::mem::size_of::<[f32; 2]>()
                    as wgpu::BufferAddress,
                shader_location: 1,
                format: VertexFormat::Float32x2,
            },
        ],
    };

    let shader = device.create_shader_module(ShaderModuleDescriptor {
        label: Some("Texture Shader"),
        source: ShaderSource::Wgsl(Cow::Borrowed(
            include_str!("shaders/texture_shader.wgsl"),
        )),
    });

    device.create_render_pipeline(&RenderPipelineDescriptor {
        label: Some("Textured Quad Pipeline"),
        layout: Some(pipeline_layout),
        vertex: VertexState {
            module: &shader,
            entry_point: Some("vs_main"),
            buffers: &[vertex_buffer_layout],
            compilation_options: Default::default(),
        },
        fragment: Some(FragmentState {
            module: &shader,
            entry_point: Some("fs_main"),
            targets: &[Some(ColorTargetState {
                format: swap_chain_format,
                blend: Some(BlendState {
                    color: BlendComponent {
                        src_factor: BlendFactor::SrcAlpha,
                        dst_factor: BlendFactor::OneMinusSrcAlpha,
                        operation: BlendOperation::Add,
                    },
                    alpha: BlendComponent {
                        src_factor: BlendFactor::One,
                        dst_factor: BlendFactor::OneMinusSrcAlpha,
                        operation: BlendOperation::Add,
                    },
                }),
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

impl Graphics {

    pub fn window(&self) -> &winit::window::Window {
        &self.window   
    }

pub fn resize(&mut self, size: PhysicalSize<u32>) {
    self.surface_config.width = size.width.max(1);
    self.surface_config.height = size.height.max(1);
    self.surface.configure(&self.device, &self.surface_config);

    // Virtual resolution (locked)
    let virtual_width = VIRTUAL_WIDTH;
    let virtual_height = VIRTUAL_HEIGHT;

    // Actual window resolution
    let window_width = self.surface_config.width as f32;
    let window_height = self.surface_config.height as f32;

    // Send both to GPU
    let camera_data = [virtual_width, virtual_height, window_width, window_height];
    self.queue
        .write_buffer(&self.aspect_buffer, 0, bytemuck::cast_slice(&camera_data));
}

    pub fn begin_frame(
        &mut self,
    ) -> (wgpu::SurfaceTexture, wgpu::TextureView, wgpu::CommandEncoder) {

        self.next_offset = 0;

        let frame = self
            .surface
            .get_current_texture()
            .expect("Failed to get next frame");
        let view = frame
            .texture
            .create_view(&TextureViewDescriptor::default());
        let encoder = self.device.create_command_encoder(
            &CommandEncoderDescriptor {
                label: Some("Main Encoder"),
            },
        );
        (frame, view, encoder)
    }

    pub fn end_frame(
        &mut self,
        frame: wgpu::SurfaceTexture,
        encoder: wgpu::CommandEncoder,
    ) {
        self.queue.submit(Some(encoder.finish()));
        frame.present();
    }

    pub fn draw_triangle(&mut self) {
        let (frame, view, mut enc) = self.begin_frame();
        {
            let mut rpass = enc.begin_render_pass(&RenderPassDescriptor {
                label: Some("Triangle Pass"),
                color_attachments: &[Some(RenderPassColorAttachment {
                    view: &view,
                    resolve_target: None,
                    ops: Operations {
                        load: LoadOp::Clear(Color::GREEN),
                        store: StoreOp::Store,
                    },
                })],
                depth_stencil_attachment: None,
                timestamp_writes: None,
                occlusion_query_set: None,
            });
            rpass.set_pipeline(&self.render_pipeline);
            rpass.draw(0..3, 0..1);
        }
        self.end_frame(frame, enc);
    }

    

    pub fn draw_image_in_pass<'a>(
        &mut self,
        rpass: &mut wgpu::RenderPass<'a>,
        texture_path: &str,
        transform: Transform2D,
        pivot: Vector2
    ) {
        // texture bind group
        let tex = self.texture_manager.get_or_load_texture_sync(
            texture_path,
            &self.device,
            &self.queue,
        );
        let tex_bg = self.device.create_bind_group(&BindGroupDescriptor {
            layout: &self.texture_bind_group_layout,
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
            label: Some("Sprite Texture BG"),
        });

    const MAT_SIZE: u64 = 64;

    // ---- wrap if the next write would exceed the buffer ---------------
    if self.next_offset as u64 + MAT_SIZE > self.transform_buffer.size() {
        self.next_offset = 0;                    // ring-buffer wrap
    }
    let offset = self.next_offset as u64;
    self.next_offset += self.min_offset;         // advance by 256 B chunk
    // -------------------------------------------------------------------

    // write transform
    let mat: [f32; 16] = transform.to_mat4().to_cols_array();
    self.queue
        .write_buffer(&self.transform_buffer, offset, bytemuck::cast_slice(&mat));

    // bind + draw
    rpass.set_pipeline(&self.render_pipeline);
    rpass.set_bind_group(0, &tex_bg, &[]);
    rpass.set_bind_group(1, &self.uniform_bind_group, &[offset as u32]);
    rpass.set_vertex_buffer(0, self.vertex_buffer.slice(..));
    rpass.draw(0..6, 0..1);
    }

pub fn draw_rect(
    &mut self,
    pass: &mut RenderPass<'_>,
    transform: Transform2D,
    size: Vector2,
    pivot: Vector2,
    color: crate::Color,
    corner_radius: Option<CornerRadius>,
) {
    // --- 1) Wrap dynamic offset if needed ---
    if self.next_rect_offset as u64 + self.rect_uniform_size > self.rect_uniform_buffer.size() {
        self.next_rect_offset = 0;
    }
    let offset = self.next_rect_offset as u64;
    self.next_rect_offset += self.rect_uniform_size as u32;

    // --- 2) Convert sRGB to linear ---
    fn srgb_to_linear(c: f32) -> f32 {
        if c <= 0.04045 {
            c / 12.92
        } else {
            ((c + 0.055) / 1.055).powf(2.4)
        }
    }

    let color_lin = [
        srgb_to_linear(color.r as f32 / 255.0),
        srgb_to_linear(color.g as f32 / 255.0),
        srgb_to_linear(color.b as f32 / 255.0),
        color.a as f32 / 255.0,
    ];



    // --- 4) Size in virtual pixels ---
    let size_data = [size.x, size.y];

    // --- 5) Corner radius normalization ---
    let half_w = size.x * 0.5;
    let half_h = size.y * 0.5;
    let scale_factor = 3.0;
    let cr = corner_radius.unwrap_or_default();
    let clamp_norm = |val: f32| -> f32 { (val * scale_factor).clamp(0.0, 0.5) };

    let cr_data = [
        [clamp_norm(cr.top_left / half_w), clamp_norm(cr.top_left / half_h), 0.0, 0.0],
        [clamp_norm(cr.top_right / half_w), clamp_norm(cr.top_right / half_h), 0.0, 0.0],
        [clamp_norm(cr.bottom_right / half_w), clamp_norm(cr.bottom_right / half_h), 0.0, 0.0],
        [clamp_norm(cr.bottom_left / half_w), clamp_norm(cr.bottom_left / half_h), 0.0, 0.0],
    ];

    // --- 6) Build the uniform struct ---
    let rect_uniform = RectUniform {
    transform: transform.to_mat4().to_cols_array_2d(),
    color: color_lin,
    size: size_data,
    pivot: [pivot.x, pivot.y],
    corner_radius: cr_data,
    border_thickness: 0.0, // ✅ no border
    is_border: 0,          // ✅ tell shader this is a fill
    _pad: [0.0; 2],
};
    // --- 7) Upload to GPU ---
    self.queue
        .write_buffer(&self.rect_uniform_buffer, offset, cast_slice(&[rect_uniform]));

    // --- 8) Draw ---
    pass.set_pipeline(&self.color_pipeline);
    pass.set_bind_group(0, &self.rect_bg, &[offset as u32]);
    pass.set_bind_group(1, &self.color_bg, &[]);
    pass.set_vertex_buffer(0, self.vertex_buffer.slice(..));
    pass.draw(0..6, 0..1);
}

pub fn draw_border(
    &mut self,
    pass: &mut RenderPass<'_>,
    transform: Transform2D,
    size: Vector2,
    pivot: Vector2,
    color: crate::Color,
    thickness: f32,
    corner_radius: Option<CornerRadius>,
) {
    if self.next_rect_offset as u64 + self.rect_uniform_size > self.rect_uniform_buffer.size() {
        self.next_rect_offset = 0;
    }
    let offset = self.next_rect_offset as u64;
    self.next_rect_offset += self.rect_uniform_size as u32;

    fn srgb_to_linear(c: f32) -> f32 {
        if c <= 0.04045 {
            c / 12.92
        } else {
            ((c + 0.055) / 1.055).powf(2.4)
        }
    }

    let color_lin = [
        srgb_to_linear(color.r as f32 / 255.0),
        srgb_to_linear(color.g as f32 / 255.0),
        srgb_to_linear(color.b as f32 / 255.0),
        color.a as f32 / 255.0,
    ];

    let size_data = [size.x, size.y];
    let half_w = size.x * 0.5;
    let half_h = size.y * 0.5;
    let scale_factor = 3.0;
    let cr = corner_radius.unwrap_or_default();
    let clamp_norm = |val: f32| -> f32 { (val * scale_factor).clamp(0.0, 0.5) };

    let cr_data = [
        [clamp_norm(cr.top_left / half_w), clamp_norm(cr.top_left / half_h), 0.0, 0.0],
        [clamp_norm(cr.top_right / half_w), clamp_norm(cr.top_right / half_h), 0.0, 0.0],
        [clamp_norm(cr.bottom_right / half_w), clamp_norm(cr.bottom_right / half_h), 0.0, 0.0],
        [clamp_norm(cr.bottom_left / half_w), clamp_norm(cr.bottom_left / half_h), 0.0, 0.0],
    ];

    let rect_uniform = RectUniform {
        transform: transform.to_mat4().to_cols_array_2d(),
        color: color_lin,
        size: size_data,
        pivot: [pivot.x, pivot.y],
        corner_radius: cr_data,
        border_thickness: thickness,
        is_border: 1,
        _pad: [0.0; 2],
    };

    self.queue
        .write_buffer(&self.rect_uniform_buffer, offset, cast_slice(&[rect_uniform]));

    pass.set_pipeline(&self.color_pipeline);
    pass.set_bind_group(0, &self.rect_bg, &[offset as u32]);
    pass.set_bind_group(1, &self.color_bg, &[]);
    pass.set_vertex_buffer(0, self.vertex_buffer.slice(..));
    pass.draw(0..6, 0..1);
}
}