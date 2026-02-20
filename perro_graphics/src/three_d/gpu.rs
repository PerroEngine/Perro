use super::{renderer::Draw3DInstance, shaders::create_mesh_shader_module};
use crate::resources::ResourceStore;
use bytemuck::{Pod, Zeroable};
use glam::{Mat4, Quat, Vec3};
use perro_render_bridge::Camera3DState;
use std::collections::HashMap;
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
    draw_calls: Vec<DrawCall>,
    last_camera: Option<Camera3DUniform>,
    vertex_buffer: wgpu::Buffer,
    index_buffer: wgpu::Buffer,
    mesh_ranges: HashMap<&'static str, MeshRange>,
    depth_texture: wgpu::Texture,
    depth_view: wgpu::TextureView,
    depth_size: (u32, u32),
    sample_count: u32,
}

#[derive(Clone, Copy)]
struct MeshRange {
    index_start: u32,
    index_count: u32,
    base_vertex: i32,
}

#[derive(Clone, Copy)]
struct DrawCall {
    mesh: MeshRange,
    instance: u32,
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
        let pipeline = create_pipeline(
            device,
            &pipeline_layout,
            &shader,
            color_format,
            sample_count,
        );

        let (vertices, indices, mesh_ranges) = build_builtin_mesh_buffer();
        let vertex_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("perro_builtin_mesh_vertices"),
            contents: bytemuck::cast_slice(&vertices),
            usage: wgpu::BufferUsages::VERTEX,
        });
        let index_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("perro_builtin_mesh_indices"),
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
            draw_calls: Vec::new(),
            last_camera: None,
            vertex_buffer,
            index_buffer,
            mesh_ranges,
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
        let (depth_texture, depth_view) = create_depth_texture(device, width, height, sample_count);
        self.depth_texture = depth_texture;
        self.depth_view = depth_view;
        self.depth_size = (width.max(1), height.max(1));
        self.sample_count = sample_count;
    }

    pub fn prepare(
        &mut self,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        resources: &ResourceStore,
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
        self.draw_calls.clear();
        self.draw_calls.reserve(draws.len());

        let default_mesh = self
            .mesh_ranges
            .get("__cube__")
            .copied()
            .expect("cube mesh preset must exist");
        for draw in draws {
            let source = resources.mesh_source(draw.mesh).unwrap_or("__cube__");
            let mesh = self.mesh_ranges.get(source).copied().unwrap_or(default_mesh);
            let instance = self.staged_instances.len() as u32;
            self.staged_instances.push(InstanceGpu {
                model_0: draw.model[0],
                model_1: draw.model[1],
                model_2: draw.model[2],
                model_3: draw.model[3],
                color: color_from_material(draw.material.index(), draw.material.generation()),
            });
            self.draw_calls.push(DrawCall { mesh, instance });
        }
        if !self.staged_instances.is_empty() {
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
        if self.draw_calls.is_empty() {
            return;
        }
        pass.set_pipeline(&self.pipeline);
        pass.set_bind_group(0, &self.camera_bind_group, &[]);
        pass.set_vertex_buffer(0, self.vertex_buffer.slice(..));
        pass.set_vertex_buffer(1, self.instance_buffer.slice(..));
        pass.set_index_buffer(self.index_buffer.slice(..), wgpu::IndexFormat::Uint16);
        for draw in &self.draw_calls {
            let start = draw.mesh.index_start;
            let end = start + draw.mesh.index_count;
            pass.draw_indexed(start..end, draw.mesh.base_vertex, draw.instance..draw.instance + 1);
        }
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

fn build_builtin_mesh_buffer() -> (Vec<MeshVertex>, Vec<u16>, HashMap<&'static str, MeshRange>) {
    let presets = [
        ("__cube__", cube_geometry()),
        ("__tri_pyr__", triangular_pyramid_geometry()),
        ("__sq_pyr__", square_pyramid_geometry()),
        ("__sphere__", sphere_geometry(20, 12)),
        ("__tri_prism__", tri_prism_geometry()),
        ("__cylinder__", cylinder_geometry(20)),
        ("__cone__", cone_geometry(20)),
        ("__capsule__", capsule_geometry(20, 8)),
    ];

    let mut all_vertices = Vec::new();
    let mut all_indices = Vec::new();
    let mut ranges = HashMap::new();

    for (name, (vertices, indices)) in presets {
        let base_vertex = all_vertices.len() as i32;
        let index_start = all_indices.len() as u32;
        let index_count = indices.len() as u32;
        all_vertices.extend(vertices);
        all_indices.extend(indices);
        ranges.insert(
            name,
            MeshRange {
                index_start,
                index_count,
                base_vertex,
            },
        );
    }

    (all_vertices, all_indices, ranges)
}

fn push_triangle(
    vertices: &mut Vec<MeshVertex>,
    indices: &mut Vec<u16>,
    a: [f32; 3],
    b: [f32; 3],
    c: [f32; 3],
) {
    let av = Vec3::from(a);
    let mut bv = Vec3::from(b);
    let mut cv = Vec3::from(c);
    let mut normal = (bv - av).cross(cv - av).normalize_or_zero();
    let centroid = (av + bv + cv) / 3.0;
    if normal.dot(centroid) < 0.0 {
        std::mem::swap(&mut bv, &mut cv);
        normal = (bv - av).cross(cv - av).normalize_or_zero();
    }
    let base = vertices.len() as u16;
    vertices.push(MeshVertex {
        pos: a,
        normal: normal.to_array(),
    });
    vertices.push(MeshVertex {
        pos: bv.to_array(),
        normal: normal.to_array(),
    });
    vertices.push(MeshVertex {
        pos: cv.to_array(),
        normal: normal.to_array(),
    });
    indices.extend_from_slice(&[base, base + 1, base + 2]);
}

fn push_quad(
    vertices: &mut Vec<MeshVertex>,
    indices: &mut Vec<u16>,
    a: [f32; 3],
    b: [f32; 3],
    c: [f32; 3],
    d: [f32; 3],
) {
    push_triangle(vertices, indices, a, b, c);
    push_triangle(vertices, indices, a, c, d);
}

fn cube_geometry() -> (Vec<MeshVertex>, Vec<u16>) {
    let mut vertices = Vec::new();
    let mut indices = Vec::new();
    push_quad(
        &mut vertices,
        &mut indices,
        [-0.5, -0.5, 0.5],
        [0.5, -0.5, 0.5],
        [0.5, 0.5, 0.5],
        [-0.5, 0.5, 0.5],
    );
    push_quad(
        &mut vertices,
        &mut indices,
        [0.5, -0.5, -0.5],
        [-0.5, -0.5, -0.5],
        [-0.5, 0.5, -0.5],
        [0.5, 0.5, -0.5],
    );
    push_quad(
        &mut vertices,
        &mut indices,
        [-0.5, -0.5, -0.5],
        [-0.5, -0.5, 0.5],
        [-0.5, 0.5, 0.5],
        [-0.5, 0.5, -0.5],
    );
    push_quad(
        &mut vertices,
        &mut indices,
        [0.5, -0.5, 0.5],
        [0.5, -0.5, -0.5],
        [0.5, 0.5, -0.5],
        [0.5, 0.5, 0.5],
    );
    push_quad(
        &mut vertices,
        &mut indices,
        [-0.5, 0.5, 0.5],
        [0.5, 0.5, 0.5],
        [0.5, 0.5, -0.5],
        [-0.5, 0.5, -0.5],
    );
    push_quad(
        &mut vertices,
        &mut indices,
        [-0.5, -0.5, -0.5],
        [0.5, -0.5, -0.5],
        [0.5, -0.5, 0.5],
        [-0.5, -0.5, 0.5],
    );
    (vertices, indices)
}

fn triangular_pyramid_geometry() -> (Vec<MeshVertex>, Vec<u16>) {
    let mut vertices = Vec::new();
    let mut indices = Vec::new();
    let p0 = [0.0, 0.6, 0.0];
    let p1 = [-0.5, -0.5, 0.5];
    let p2 = [0.5, -0.5, 0.5];
    let p3 = [0.0, -0.5, -0.6];
    push_triangle(&mut vertices, &mut indices, p0, p1, p2);
    push_triangle(&mut vertices, &mut indices, p0, p2, p3);
    push_triangle(&mut vertices, &mut indices, p0, p3, p1);
    push_triangle(&mut vertices, &mut indices, p1, p3, p2);
    (vertices, indices)
}

fn square_pyramid_geometry() -> (Vec<MeshVertex>, Vec<u16>) {
    let mut vertices = Vec::new();
    let mut indices = Vec::new();
    let top = [0.0, 0.65, 0.0];
    let b0 = [-0.5, -0.5, -0.5];
    let b1 = [0.5, -0.5, -0.5];
    let b2 = [0.5, -0.5, 0.5];
    let b3 = [-0.5, -0.5, 0.5];
    push_triangle(&mut vertices, &mut indices, top, b0, b1);
    push_triangle(&mut vertices, &mut indices, top, b1, b2);
    push_triangle(&mut vertices, &mut indices, top, b2, b3);
    push_triangle(&mut vertices, &mut indices, top, b3, b0);
    push_quad(&mut vertices, &mut indices, b0, b3, b2, b1);
    (vertices, indices)
}

fn tri_prism_geometry() -> (Vec<MeshVertex>, Vec<u16>) {
    let mut vertices = Vec::new();
    let mut indices = Vec::new();
    let a0 = [-0.5, -0.5, -0.4];
    let a1 = [0.5, -0.5, -0.4];
    let a2 = [0.0, 0.5, -0.4];
    let b0 = [-0.5, -0.5, 0.4];
    let b1 = [0.5, -0.5, 0.4];
    let b2 = [0.0, 0.5, 0.4];
    push_triangle(&mut vertices, &mut indices, a0, a1, a2);
    push_triangle(&mut vertices, &mut indices, b0, b2, b1);
    push_quad(&mut vertices, &mut indices, a0, b0, b1, a1);
    push_quad(&mut vertices, &mut indices, a1, b1, b2, a2);
    push_quad(&mut vertices, &mut indices, a2, b2, b0, a0);
    (vertices, indices)
}

fn sphere_geometry(longitude_segments: u32, latitude_segments: u32) -> (Vec<MeshVertex>, Vec<u16>) {
    let lon = longitude_segments.max(3);
    let lat = latitude_segments.max(2);
    let mut vertices = Vec::new();
    let mut indices = Vec::new();

    for y in 0..=lat {
        let v = y as f32 / lat as f32;
        let phi = v * std::f32::consts::PI;
        let sin_phi = phi.sin();
        let cos_phi = phi.cos();
        for x in 0..=lon {
            let u = x as f32 / lon as f32;
            let theta = u * std::f32::consts::TAU;
            let sin_theta = theta.sin();
            let cos_theta = theta.cos();
            let n = Vec3::new(sin_phi * cos_theta, cos_phi, sin_phi * sin_theta);
            vertices.push(MeshVertex {
                pos: (n * 0.5).to_array(),
                normal: n.to_array(),
            });
        }
    }

    let row = lon + 1;
    for y in 0..lat {
        for x in 0..lon {
            let i0 = y * row + x;
            let i1 = i0 + 1;
            let i2 = i0 + row;
            let i3 = i2 + 1;
            push_index_triangle_outward(&vertices, &mut indices, i0 as u16, i2 as u16, i1 as u16);
            push_index_triangle_outward(&vertices, &mut indices, i1 as u16, i2 as u16, i3 as u16);
        }
    }
    (vertices, indices)
}

fn cylinder_geometry(segments: u32) -> (Vec<MeshVertex>, Vec<u16>) {
    let seg = segments.max(3);
    let mut vertices = Vec::new();
    let mut indices = Vec::new();
    let top_y = 0.5;
    let bot_y = -0.5;
    let r = 0.5;

    for i in 0..seg {
        let a0 = i as f32 / seg as f32 * std::f32::consts::TAU;
        let a1 = (i + 1) as f32 / seg as f32 * std::f32::consts::TAU;
        let p0 = [r * a0.cos(), bot_y, r * a0.sin()];
        let p1 = [r * a1.cos(), bot_y, r * a1.sin()];
        let p2 = [r * a1.cos(), top_y, r * a1.sin()];
        let p3 = [r * a0.cos(), top_y, r * a0.sin()];
        push_quad(&mut vertices, &mut indices, p0, p1, p2, p3);
        push_triangle(&mut vertices, &mut indices, [0.0, top_y, 0.0], p2, p3);
        push_triangle(&mut vertices, &mut indices, [0.0, bot_y, 0.0], p0, p1);
    }
    (vertices, indices)
}

fn cone_geometry(segments: u32) -> (Vec<MeshVertex>, Vec<u16>) {
    let seg = segments.max(3);
    let mut vertices = Vec::new();
    let mut indices = Vec::new();
    let apex = [0.0, 0.6, 0.0];
    let by = -0.5;
    let r = 0.5;
    for i in 0..seg {
        let a0 = i as f32 / seg as f32 * std::f32::consts::TAU;
        let a1 = (i + 1) as f32 / seg as f32 * std::f32::consts::TAU;
        let p0 = [r * a0.cos(), by, r * a0.sin()];
        let p1 = [r * a1.cos(), by, r * a1.sin()];
        push_triangle(&mut vertices, &mut indices, apex, p0, p1);
        push_triangle(&mut vertices, &mut indices, [0.0, by, 0.0], p1, p0);
    }
    (vertices, indices)
}

fn capsule_geometry(longitude_segments: u32, hemisphere_rings: u32) -> (Vec<MeshVertex>, Vec<u16>) {
    let lon = longitude_segments.max(6);
    let rings = hemisphere_rings.max(2);
    let mut vertices = Vec::new();
    let mut indices = Vec::new();

    // Top hemisphere
    for y in 0..=rings {
        let v = y as f32 / rings as f32;
        let phi = v * std::f32::consts::FRAC_PI_2;
        let sin_phi = phi.sin();
        let cos_phi = phi.cos();
        for x in 0..=lon {
            let u = x as f32 / lon as f32;
            let theta = u * std::f32::consts::TAU;
            let n = Vec3::new(sin_phi * theta.cos(), cos_phi, sin_phi * theta.sin());
            let p = Vec3::new(n.x * 0.5, n.y * 0.5 + 0.25, n.z * 0.5);
            vertices.push(MeshVertex {
                pos: p.to_array(),
                normal: n.to_array(),
            });
        }
    }

    // Bottom hemisphere
    let base_offset = vertices.len() as u16;
    for y in 0..=rings {
        let v = y as f32 / rings as f32;
        let phi = v * std::f32::consts::FRAC_PI_2;
        let sin_phi = phi.sin();
        let cos_phi = phi.cos();
        for x in 0..=lon {
            let u = x as f32 / lon as f32;
            let theta = u * std::f32::consts::TAU;
            let n = Vec3::new(sin_phi * theta.cos(), -cos_phi, sin_phi * theta.sin());
            let p = Vec3::new(n.x * 0.5, n.y * 0.5 - 0.25, n.z * 0.5);
            vertices.push(MeshVertex {
                pos: p.to_array(),
                normal: n.to_array(),
            });
        }
    }

    let row = lon + 1;
    for y in 0..rings {
        for x in 0..lon {
            let i0 = y * row + x;
            let i1 = i0 + 1;
            let i2 = i0 + row;
            let i3 = i2 + 1;
            push_index_triangle_outward(&vertices, &mut indices, i0 as u16, i2 as u16, i1 as u16);
            push_index_triangle_outward(&vertices, &mut indices, i1 as u16, i2 as u16, i3 as u16);
        }
    }
    for y in 0..rings {
        for x in 0..lon {
            let i0 = base_offset as u32 + y * row + x;
            let i1 = i0 + 1;
            let i2 = i0 + row;
            let i3 = i2 + 1;
            push_index_triangle_outward(&vertices, &mut indices, i0 as u16, i1 as u16, i2 as u16);
            push_index_triangle_outward(&vertices, &mut indices, i1 as u16, i3 as u16, i2 as u16);
        }
    }
    (vertices, indices)
}

fn push_index_triangle_outward(
    vertices: &[MeshVertex],
    indices: &mut Vec<u16>,
    i0: u16,
    i1: u16,
    i2: u16,
) {
    let p0 = Vec3::from(vertices[i0 as usize].pos);
    let p1 = Vec3::from(vertices[i1 as usize].pos);
    let p2 = Vec3::from(vertices[i2 as usize].pos);
    let n = (p1 - p0).cross(p2 - p0);
    let centroid = (p0 + p1 + p2) / 3.0;
    if n.dot(centroid) < 0.0 {
        indices.extend_from_slice(&[i0, i2, i1]);
    } else {
        indices.extend_from_slice(&[i0, i1, i2]);
    }
}
