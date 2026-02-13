use super::{renderer::Draw3DInstance, shaders::create_mesh_shader_module};
use bytemuck::{Pod, Zeroable};
use glam::{Mat4, Quat, Vec3};
use perro_render_bridge::Camera3DState;
use wgpu::util::DeviceExt;

const DEPTH_FORMAT: wgpu::TextureFormat = wgpu::TextureFormat::Depth24Plus;

#[repr(C)]
#[derive(Clone, Copy, Pod, Zeroable, PartialEq)]
struct Camera3DUniform {
    view_proj: [[f32; 4]; 4],
}

#[repr(C)]
#[derive(Clone, Copy, Pod, Zeroable)]
struct MeshVertex {
    pos: [f32; 3],
    normal: [f32; 3],
}

#[repr(C)]
#[derive(Clone, Copy, Pod, Zeroable)]
struct InstanceGpu {
    model_0: [f32; 4],
    model_1: [f32; 4],
    model_2: [f32; 4],
    model_3: [f32; 4],
    color: [f32; 4],
}

pub struct Gpu3D {
    camera_bgl: wgpu::BindGroupLayout,
    pipeline: wgpu::RenderPipeline,
    camera_buffer: wgpu::Buffer,
    camera_bind_group: wgpu::BindGroup,
    instance_buffer: wgpu::Buffer,
    instance_capacity: usize,
    staged_instances: Vec<InstanceGpu>,
    draw_count: usize,
    last_camera: Option<Camera3DUniform>,
    vertex_buffer: wgpu::Buffer,
    index_buffer: wgpu::Buffer,
    index_count: u32,
    depth_texture: wgpu::Texture,
    depth_view: wgpu::TextureView,
    depth_size: (u32, u32),
    sample_count: u32,
}

impl Gpu3D {
    pub fn new(
        device: &wgpu::Device,
        color_format: wgpu::TextureFormat,
        sample_count: u32,
        width: u32,
        height: u32,
    ) -> Self {
        let shader = create_mesh_shader_module(device);
        let camera_bgl = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("perro_camera3d_bgl"),
            entries: &[wgpu::BindGroupLayoutEntry {
                binding: 0,
                visibility: wgpu::ShaderStages::VERTEX,
                ty: wgpu::BindingType::Buffer {
                    ty: wgpu::BufferBindingType::Uniform,
                    has_dynamic_offset: false,
                    min_binding_size: Some(
                        std::num::NonZeroU64::new(std::mem::size_of::<Camera3DUniform>() as u64)
                            .expect("camera uniform size must be non-zero"),
                    ),
                },
                count: None,
            }],
        });
        let camera_buffer = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("perro_camera3d_buffer"),
            size: std::mem::size_of::<Camera3DUniform>() as u64,
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });
        let camera_bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("perro_camera3d_bg"),
            layout: &camera_bgl,
            entries: &[wgpu::BindGroupEntry {
                binding: 0,
                resource: camera_buffer.as_entire_binding(),
            }],
        });

        let pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("perro_mesh_pipeline_layout"),
            bind_group_layouts: &[&camera_bgl],
            immediate_size: 0,
        });
        let pipeline = create_pipeline(device, &pipeline_layout, &shader, color_format, sample_count);

        let (vertices, indices) = cube_geometry();
        let vertex_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("perro_cube_vertices"),
            contents: bytemuck::cast_slice(&vertices),
            usage: wgpu::BufferUsages::VERTEX,
        });
        let index_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("perro_cube_indices"),
            contents: bytemuck::cast_slice(&indices),
            usage: wgpu::BufferUsages::INDEX,
        });

        let instance_capacity = 256usize;
        let instance_buffer = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("perro_mesh_instances"),
            size: (instance_capacity * std::mem::size_of::<InstanceGpu>()) as u64,
            usage: wgpu::BufferUsages::VERTEX | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });

        let (depth_texture, depth_view) = create_depth_texture(device, width, height, sample_count);

        Self {
            camera_bgl,
            pipeline,
            camera_buffer,
            camera_bind_group,
            instance_buffer,
            instance_capacity,
            staged_instances: Vec::new(),
            draw_count: 0,
            last_camera: None,
            vertex_buffer,
            index_buffer,
            index_count: indices.len() as u32,
            depth_texture,
            depth_view,
            depth_size: (width.max(1), height.max(1)),
            sample_count,
        }
    }

    pub fn resize(&mut self, device: &wgpu::Device, width: u32, height: u32) {
        let width = width.max(1);
        let height = height.max(1);
        if self.depth_size == (width, height) {
            return;
        }
        let (depth_texture, depth_view) =
            create_depth_texture(device, width, height, self.sample_count);
        self.depth_texture = depth_texture;
        self.depth_view = depth_view;
        self.depth_size = (width, height);
    }

    pub fn set_sample_count(
        &mut self,
        device: &wgpu::Device,
        color_format: wgpu::TextureFormat,
        sample_count: u32,
        width: u32,
        height: u32,
    ) {
        let sample_count = sample_count.max(1);
        if self.sample_count == sample_count {
            return;
        }
        let shader = create_mesh_shader_module(device);
        let pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("perro_mesh_pipeline_layout"),
            bind_group_layouts: &[&self.camera_bgl],
            immediate_size: 0,
        });
        self.pipeline = create_pipeline(
            device,
            &pipeline_layout,
            &shader,
            color_format,
            sample_count,
        );
        let (depth_texture, depth_view) =
            create_depth_texture(device, width, height, sample_count);
        self.depth_texture = depth_texture;
        self.depth_view = depth_view;
        self.depth_size = (width.max(1), height.max(1));
        self.sample_count = sample_count;
    }

    pub fn prepare(
        &mut self,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        camera: Camera3DState,
        draws: &[Draw3DInstance],
        width: u32,
        height: u32,
    ) {
        self.resize(device, width, height);
        self.ensure_instance_capacity(device, draws.len());

        let uniform = Camera3DUniform {
            view_proj: compute_view_proj(camera, width, height),
        };
        if self.last_camera != Some(uniform) {
            queue.write_buffer(&self.camera_buffer, 0, bytemuck::bytes_of(&uniform));
            self.last_camera = Some(uniform);
        }

        self.staged_instances.clear();
        self.staged_instances.reserve(draws.len());
        for draw in draws {
            self.staged_instances.push(InstanceGpu {
                model_0: draw.model[0],
                model_1: draw.model[1],
                model_2: draw.model[2],
                model_3: draw.model[3],
                color: color_from_material(draw.material.index(), draw.material.generation()),
            });
        }
        self.draw_count = self.staged_instances.len();
        if self.draw_count > 0 {
            queue.write_buffer(
                &self.instance_buffer,
                0,
                bytemuck::cast_slice(&self.staged_instances),
            );
        }
    }

    pub fn render_pass(
        &self,
        encoder: &mut wgpu::CommandEncoder,
        color_view: &wgpu::TextureView,
        clear_color: wgpu::Color,
    ) {
        let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
            label: Some("perro_mesh_pass"),
            color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                view: color_view,
                resolve_target: None,
                ops: wgpu::Operations {
                    load: wgpu::LoadOp::Clear(clear_color),
                    store: wgpu::StoreOp::Store,
                },
                depth_slice: None,
            })],
            depth_stencil_attachment: Some(wgpu::RenderPassDepthStencilAttachment {
                view: &self.depth_view,
                depth_ops: Some(wgpu::Operations {
                    load: wgpu::LoadOp::Clear(1.0),
                    store: wgpu::StoreOp::Store,
                }),
                stencil_ops: None,
            }),
            timestamp_writes: None,
            occlusion_query_set: None,
            multiview_mask: None,
        });
        if self.draw_count == 0 {
            return;
        }
        pass.set_pipeline(&self.pipeline);
        pass.set_bind_group(0, &self.camera_bind_group, &[]);
        pass.set_vertex_buffer(0, self.vertex_buffer.slice(..));
        pass.set_vertex_buffer(1, self.instance_buffer.slice(..));
        pass.set_index_buffer(self.index_buffer.slice(..), wgpu::IndexFormat::Uint16);
        pass.draw_indexed(0..self.index_count, 0, 0..self.draw_count as u32);
    }

    fn ensure_instance_capacity(&mut self, device: &wgpu::Device, needed: usize) {
        if needed <= self.instance_capacity {
            return;
        }
        let mut new_capacity = self.instance_capacity.max(1);
        while new_capacity < needed {
            new_capacity *= 2;
        }
        self.instance_buffer = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("perro_mesh_instances"),
            size: (new_capacity * std::mem::size_of::<InstanceGpu>()) as u64,
            usage: wgpu::BufferUsages::VERTEX | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });
        self.instance_capacity = new_capacity;
    }
}

fn create_depth_texture(
    device: &wgpu::Device,
    width: u32,
    height: u32,
    sample_count: u32,
) -> (wgpu::Texture, wgpu::TextureView) {
    let depth_texture = device.create_texture(&wgpu::TextureDescriptor {
        label: Some("perro_depth3d"),
        size: wgpu::Extent3d {
            width: width.max(1),
            height: height.max(1),
            depth_or_array_layers: 1,
        },
        mip_level_count: 1,
        sample_count,
        dimension: wgpu::TextureDimension::D2,
        format: DEPTH_FORMAT,
        usage: wgpu::TextureUsages::RENDER_ATTACHMENT,
        view_formats: &[],
    });
    let depth_view = depth_texture.create_view(&wgpu::TextureViewDescriptor::default());
    (depth_texture, depth_view)
}

fn create_pipeline(
    device: &wgpu::Device,
    pipeline_layout: &wgpu::PipelineLayout,
    shader: &wgpu::ShaderModule,
    color_format: wgpu::TextureFormat,
    sample_count: u32,
) -> wgpu::RenderPipeline {
    device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
        label: Some("perro_mesh_pipeline"),
        layout: Some(pipeline_layout),
        vertex: wgpu::VertexState {
            module: shader,
            entry_point: Some("vs_main"),
            buffers: &[
                wgpu::VertexBufferLayout {
                    array_stride: std::mem::size_of::<MeshVertex>() as u64,
                    step_mode: wgpu::VertexStepMode::Vertex,
                    attributes: &[
                        wgpu::VertexAttribute {
                            offset: 0,
                            shader_location: 0,
                            format: wgpu::VertexFormat::Float32x3,
                        },
                        wgpu::VertexAttribute {
                            offset: 12,
                            shader_location: 1,
                            format: wgpu::VertexFormat::Float32x3,
                        },
                    ],
                },
                wgpu::VertexBufferLayout {
                    array_stride: std::mem::size_of::<InstanceGpu>() as u64,
                    step_mode: wgpu::VertexStepMode::Instance,
                    attributes: &[
                        wgpu::VertexAttribute {
                            offset: 0,
                            shader_location: 2,
                            format: wgpu::VertexFormat::Float32x4,
                        },
                        wgpu::VertexAttribute {
                            offset: 16,
                            shader_location: 3,
                            format: wgpu::VertexFormat::Float32x4,
                        },
                        wgpu::VertexAttribute {
                            offset: 32,
                            shader_location: 4,
                            format: wgpu::VertexFormat::Float32x4,
                        },
                        wgpu::VertexAttribute {
                            offset: 48,
                            shader_location: 5,
                            format: wgpu::VertexFormat::Float32x4,
                        },
                        wgpu::VertexAttribute {
                            offset: 64,
                            shader_location: 6,
                            format: wgpu::VertexFormat::Float32x4,
                        },
                    ],
                },
            ],
            compilation_options: Default::default(),
        },
        fragment: Some(wgpu::FragmentState {
            module: shader,
            entry_point: Some("fs_main"),
            targets: &[Some(wgpu::ColorTargetState {
                format: color_format,
                blend: Some(wgpu::BlendState::ALPHA_BLENDING),
                write_mask: wgpu::ColorWrites::ALL,
            })],
            compilation_options: Default::default(),
        }),
        primitive: wgpu::PrimitiveState {
            topology: wgpu::PrimitiveTopology::TriangleList,
            strip_index_format: None,
            front_face: wgpu::FrontFace::Ccw,
            cull_mode: Some(wgpu::Face::Back),
            unclipped_depth: false,
            polygon_mode: wgpu::PolygonMode::Fill,
            conservative: false,
        },
        depth_stencil: Some(wgpu::DepthStencilState {
            format: DEPTH_FORMAT,
            depth_write_enabled: true,
            depth_compare: wgpu::CompareFunction::LessEqual,
            stencil: wgpu::StencilState::default(),
            bias: wgpu::DepthBiasState::default(),
        }),
        multisample: wgpu::MultisampleState {
            count: sample_count,
            mask: !0,
            alpha_to_coverage_enabled: false,
        },
        multiview_mask: None,
        cache: None,
    })
}

fn compute_view_proj(camera: Camera3DState, width: u32, height: u32) -> [[f32; 4]; 4] {
    let w = width.max(1) as f32;
    let h = height.max(1) as f32;
    let aspect = w / h;

    let zoom = if camera.zoom.is_finite() && camera.zoom > 0.0 {
        camera.zoom
    } else {
        1.0
    };
    let fov_y_radians = (60.0f32 / zoom)
        .to_radians()
        .clamp(10.0f32.to_radians(), 120.0f32.to_radians());
    let proj = Mat4::perspective_rh_gl(fov_y_radians, aspect, 0.1, 500.0);

    let pos = Vec3::from(camera.position);
    let rot_raw = Quat::from_xyzw(
        camera.rotation[0],
        camera.rotation[1],
        camera.rotation[2],
        camera.rotation[3],
    );
    let rot = if rot_raw.is_finite() && rot_raw.length_squared() > 1.0e-6 {
        rot_raw.normalize()
    } else {
        Quat::IDENTITY
    };
    let world = Mat4::from_rotation_translation(rot, pos);
    let view = world.inverse();
    (proj * view).to_cols_array_2d()
}

fn color_from_material(index: u32, generation: u32) -> [f32; 4] {
    let mut x = index ^ (generation.rotate_left(16));
    x ^= x >> 17;
    x = x.wrapping_mul(0xed5ad4bb);
    x ^= x >> 11;
    x = x.wrapping_mul(0xac4c1b51);
    x ^= x >> 15;

    let r = ((x & 0xFF) as f32 / 255.0) * 0.55 + 0.35;
    let g = (((x >> 8) & 0xFF) as f32 / 255.0) * 0.55 + 0.35;
    let b = (((x >> 16) & 0xFF) as f32 / 255.0) * 0.55 + 0.35;
    [r, g, b, 1.0]
}

fn cube_geometry() -> ([MeshVertex; 24], [u16; 36]) {
    let vertices = [
        MeshVertex { pos: [-0.5, -0.5, 0.5], normal: [0.0, 0.0, 1.0] },
        MeshVertex { pos: [0.5, -0.5, 0.5], normal: [0.0, 0.0, 1.0] },
        MeshVertex { pos: [0.5, 0.5, 0.5], normal: [0.0, 0.0, 1.0] },
        MeshVertex { pos: [-0.5, 0.5, 0.5], normal: [0.0, 0.0, 1.0] },
        MeshVertex { pos: [0.5, -0.5, -0.5], normal: [0.0, 0.0, -1.0] },
        MeshVertex { pos: [-0.5, -0.5, -0.5], normal: [0.0, 0.0, -1.0] },
        MeshVertex { pos: [-0.5, 0.5, -0.5], normal: [0.0, 0.0, -1.0] },
        MeshVertex { pos: [0.5, 0.5, -0.5], normal: [0.0, 0.0, -1.0] },
        MeshVertex { pos: [-0.5, -0.5, -0.5], normal: [-1.0, 0.0, 0.0] },
        MeshVertex { pos: [-0.5, -0.5, 0.5], normal: [-1.0, 0.0, 0.0] },
        MeshVertex { pos: [-0.5, 0.5, 0.5], normal: [-1.0, 0.0, 0.0] },
        MeshVertex { pos: [-0.5, 0.5, -0.5], normal: [-1.0, 0.0, 0.0] },
        MeshVertex { pos: [0.5, -0.5, 0.5], normal: [1.0, 0.0, 0.0] },
        MeshVertex { pos: [0.5, -0.5, -0.5], normal: [1.0, 0.0, 0.0] },
        MeshVertex { pos: [0.5, 0.5, -0.5], normal: [1.0, 0.0, 0.0] },
        MeshVertex { pos: [0.5, 0.5, 0.5], normal: [1.0, 0.0, 0.0] },
        MeshVertex { pos: [-0.5, 0.5, 0.5], normal: [0.0, 1.0, 0.0] },
        MeshVertex { pos: [0.5, 0.5, 0.5], normal: [0.0, 1.0, 0.0] },
        MeshVertex { pos: [0.5, 0.5, -0.5], normal: [0.0, 1.0, 0.0] },
        MeshVertex { pos: [-0.5, 0.5, -0.5], normal: [0.0, 1.0, 0.0] },
        MeshVertex { pos: [-0.5, -0.5, -0.5], normal: [0.0, -1.0, 0.0] },
        MeshVertex { pos: [0.5, -0.5, -0.5], normal: [0.0, -1.0, 0.0] },
        MeshVertex { pos: [0.5, -0.5, 0.5], normal: [0.0, -1.0, 0.0] },
        MeshVertex { pos: [-0.5, -0.5, 0.5], normal: [0.0, -1.0, 0.0] },
    ];
    let indices = [
        0, 1, 2, 0, 2, 3,
        4, 5, 6, 4, 6, 7,
        8, 9, 10, 8, 10, 11,
        12, 13, 14, 12, 14, 15,
        16, 17, 18, 16, 18, 19,
        20, 21, 22, 20, 22, 23,
    ];
    (vertices, indices)
}
