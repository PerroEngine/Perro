use super::{
    renderer::{Draw3DInstance, Lighting3DState, MAX_POINT_LIGHTS, MAX_SPOT_LIGHTS},
    shaders::create_mesh_shader_module,
};
use crate::backend::StaticMeshLookup;
use crate::resources::ResourceStore;
use bytemuck::{Pod, Zeroable};
use glam::{Mat4, Quat, Vec3};
use mesh_presets::build_builtin_mesh_buffer;
use perro_io::{decompress_zlib, load_asset};
use perro_render_bridge::{Camera3DState, Material3D};
use std::collections::HashMap;
use wgpu::util::DeviceExt;

mod mesh_presets;

const DEPTH_FORMAT: wgpu::TextureFormat = wgpu::TextureFormat::Depth24Plus;

#[repr(C)]
#[derive(Clone, Copy, Pod, Zeroable, PartialEq)]
struct Scene3DUniform {
    view_proj: [[f32; 4]; 4],
    ambient_and_counts: [f32; 4],
    camera_pos: [f32; 4],
    ambient_color: [f32; 4],
    ray_light: RayLightGpu,
    point_lights: [PointLightGpu; MAX_POINT_LIGHTS],
    spot_lights: [SpotLightGpu; MAX_SPOT_LIGHTS],
}

#[repr(C)]
#[derive(Clone, Copy, Pod, Zeroable, PartialEq)]
struct RayLightGpu {
    direction: [f32; 4],
    color_intensity: [f32; 4],
}

#[repr(C)]
#[derive(Clone, Copy, Pod, Zeroable, PartialEq)]
struct PointLightGpu {
    position_range: [f32; 4],
    color_intensity: [f32; 4],
}

#[repr(C)]
#[derive(Clone, Copy, Pod, Zeroable, PartialEq)]
struct SpotLightGpu {
    position_range: [f32; 4],
    direction_outer_cos: [f32; 4],
    color_intensity: [f32; 4],
    inner_cos_pad: [f32; 4],
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
    pbr_params: [f32; 4], // roughness, metallic, occlusion_strength, emissive_strength
}

pub struct Gpu3D {
    camera_bgl: wgpu::BindGroupLayout,
    pipeline: wgpu::RenderPipeline,
    camera_buffer: wgpu::Buffer,
    camera_bind_group: wgpu::BindGroup,
    instance_buffer: wgpu::Buffer,
    instance_capacity: usize,
    staged_instances: Vec<InstanceGpu>,
    draw_batches: Vec<DrawBatch>,
    last_scene: Option<Scene3DUniform>,
    mesh_vertices: Vec<MeshVertex>,
    mesh_indices: Vec<u32>,
    vertex_buffer: wgpu::Buffer,
    index_buffer: wgpu::Buffer,
    builtin_mesh_ranges: HashMap<&'static str, MeshRange>,
    custom_mesh_ranges: HashMap<String, MeshRange>,
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
struct DrawBatch {
    mesh: MeshRange,
    instance_start: u32,
    instance_count: u32,
}

const PMESH_MAGIC: &[u8; 5] = b"PMESH";

#[derive(Clone)]
struct DecodedMesh {
    vertices: Vec<MeshVertex>,
    indices: Vec<u32>,
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
                visibility: wgpu::ShaderStages::VERTEX_FRAGMENT,
                ty: wgpu::BindingType::Buffer {
                    ty: wgpu::BufferBindingType::Uniform,
                    has_dynamic_offset: false,
                    min_binding_size: Some(
                        std::num::NonZeroU64::new(std::mem::size_of::<Scene3DUniform>() as u64)
                            .expect("camera uniform size must be non-zero"),
                    ),
                },
                count: None,
            }],
        });
        let camera_buffer = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("perro_camera3d_buffer"),
            size: std::mem::size_of::<Scene3DUniform>() as u64,
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

        let (vertices, indices, builtin_mesh_ranges) = build_builtin_mesh_buffer();
        let vertex_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("perro_builtin_mesh_vertices"),
            contents: bytemuck::cast_slice(&vertices),
            usage: wgpu::BufferUsages::VERTEX | wgpu::BufferUsages::COPY_DST,
        });
        let index_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("perro_builtin_mesh_indices"),
            contents: bytemuck::cast_slice(&indices),
            usage: wgpu::BufferUsages::INDEX | wgpu::BufferUsages::COPY_DST,
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
            draw_batches: Vec::new(),
            last_scene: None,
            mesh_vertices: vertices,
            mesh_indices: indices,
            vertex_buffer,
            index_buffer,
            builtin_mesh_ranges,
            custom_mesh_ranges: HashMap::new(),
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
        lighting: &Lighting3DState,
        draws: &[Draw3DInstance],
        width: u32,
        height: u32,
        static_mesh_lookup: Option<StaticMeshLookup>,
    ) {
        self.resize(device, width, height);
        self.ensure_instance_capacity(device, draws.len());

        let uniform = build_scene_uniform(camera, lighting, width, height);
        if self.last_scene != Some(uniform) {
            queue.write_buffer(&self.camera_buffer, 0, bytemuck::bytes_of(&uniform));
            self.last_scene = Some(uniform);
        }

        self.staged_instances.clear();
        self.staged_instances.reserve(draws.len());
        self.draw_batches.clear();
        self.draw_batches.reserve(draws.len());

        let default_mesh = self
            .builtin_mesh_ranges
            .get("__cube__")
            .copied()
            .expect("cube mesh preset must exist");
        for draw in draws {
            let source = resources.mesh_source(draw.mesh).unwrap_or("__cube__");
            let mesh = self
                .resolve_mesh_range(device, queue, source, static_mesh_lookup)
                .unwrap_or(default_mesh);
            let material = resources
                .material(draw.material)
                .unwrap_or_else(Material3D::default);
            let instance = self.staged_instances.len() as u32;
            self.staged_instances.push(InstanceGpu {
                model_0: draw.model[0],
                model_1: draw.model[1],
                model_2: draw.model[2],
                model_3: draw.model[3],
                color: material.base_color_factor,
                pbr_params: [
                    material.roughness_factor,
                    material.metallic_factor,
                    material.occlusion_strength,
                    material
                        .emissive_factor
                        .iter()
                        .copied()
                        .fold(0.0_f32, f32::max),
                ],
            });
            if let Some(batch) = self.draw_batches.last_mut() {
                if batch.mesh.index_start == mesh.index_start
                    && batch.mesh.index_count == mesh.index_count
                    && batch.mesh.base_vertex == mesh.base_vertex
                    && batch.instance_start + batch.instance_count == instance
                {
                    batch.instance_count += 1;
                    continue;
                }
            }
            self.draw_batches.push(DrawBatch {
                mesh,
                instance_start: instance,
                instance_count: 1,
            });
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
        if self.draw_batches.is_empty() {
            return;
        }
        pass.set_pipeline(&self.pipeline);
        pass.set_bind_group(0, &self.camera_bind_group, &[]);
        pass.set_vertex_buffer(0, self.vertex_buffer.slice(..));
        pass.set_vertex_buffer(1, self.instance_buffer.slice(..));
        pass.set_index_buffer(self.index_buffer.slice(..), wgpu::IndexFormat::Uint32);
        for batch in &self.draw_batches {
            let start = batch.mesh.index_start;
            let end = start + batch.mesh.index_count;
            let instances = batch.instance_start..batch.instance_start + batch.instance_count;
            pass.draw_indexed(start..end, batch.mesh.base_vertex, instances);
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

    fn resolve_mesh_range(
        &mut self,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        source: &str,
        static_mesh_lookup: Option<StaticMeshLookup>,
    ) -> Option<MeshRange> {
        if let Some(range) = self.builtin_mesh_ranges.get(source).copied() {
            return Some(range);
        }
        if let Some(range) = self.custom_mesh_ranges.get(source).copied() {
            return Some(range);
        }
        let decoded = load_mesh_from_source(source, static_mesh_lookup)?;
        let range = self.append_mesh_data(device, queue, source, decoded)?;
        self.custom_mesh_ranges.insert(source.to_string(), range);
        Some(range)
    }

    fn append_mesh_data(
        &mut self,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        _source: &str,
        decoded: DecodedMesh,
    ) -> Option<MeshRange> {
        if decoded.vertices.is_empty() || decoded.indices.is_empty() {
            return None;
        }
        let base_vertex = self.mesh_vertices.len() as u32;
        let index_start = self.mesh_indices.len() as u32;
        let index_count = decoded.indices.len() as u32;

        self.mesh_vertices.extend_from_slice(&decoded.vertices);
        self.mesh_indices
            .extend(decoded.indices.iter().copied().map(|idx| idx + base_vertex));

        self.vertex_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("perro_mesh_vertices"),
            contents: bytemuck::cast_slice(&self.mesh_vertices),
            usage: wgpu::BufferUsages::VERTEX | wgpu::BufferUsages::COPY_DST,
        });
        self.index_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("perro_mesh_indices"),
            contents: bytemuck::cast_slice(&self.mesh_indices),
            usage: wgpu::BufferUsages::INDEX | wgpu::BufferUsages::COPY_DST,
        });
        // Queue is intentionally unused: we rebuild immutable buffers for now.
        let _ = queue;

        Some(MeshRange {
            index_start,
            index_count,
            base_vertex: 0,
        })
    }
}

fn load_mesh_from_source(
    source: &str,
    static_mesh_lookup: Option<StaticMeshLookup>,
) -> Option<DecodedMesh> {
    if let Some(lookup) = static_mesh_lookup {
        if let Some(bytes) = lookup(source) {
            if let Some(decoded) = decode_pmesh(bytes) {
                return Some(decoded);
            }
        }
    }

    let (path, fragment) = split_source_fragment(source);
    if path.ends_with(".pmesh") {
        let bytes = load_asset(path).ok()?;
        return decode_pmesh(&bytes);
    }
    if path.ends_with(".glb") || path.ends_with(".gltf") {
        let mesh_index = parse_fragment_index(fragment, "mesh").unwrap_or(0);
        let bytes = load_asset(path).ok()?;
        return decode_gltf_mesh(&bytes, mesh_index as usize);
    }
    None
}

fn split_source_fragment(source: &str) -> (&str, Option<&str>) {
    let Some((path, selector)) = source.rsplit_once(':') else {
        return (source, None);
    };
    if path.is_empty() || selector.contains('/') || selector.contains('\\') {
        return (source, None);
    }
    if selector.contains('[') && selector.ends_with(']') {
        return (path, Some(selector));
    }
    (source, None)
}

fn parse_fragment_index(fragment: Option<&str>, key: &str) -> Option<u32> {
    let fragment = fragment?;
    let (name, rest) = fragment.split_once('[')?;
    if name.trim() != key {
        return None;
    }
    let value = rest.strip_suffix(']')?.trim();
    value.parse::<u32>().ok()
}

fn decode_pmesh(bytes: &[u8]) -> Option<DecodedMesh> {
    if bytes.len() < 25 || &bytes[0..5] != PMESH_MAGIC {
        return None;
    }
    let version = u32::from_le_bytes(bytes[5..9].try_into().ok()?);
    if version != 1 {
        return None;
    }
    let vertex_count = u32::from_le_bytes(bytes[9..13].try_into().ok()?) as usize;
    let index_count = u32::from_le_bytes(bytes[13..17].try_into().ok()?) as usize;
    let _material_count = u32::from_le_bytes(bytes[17..21].try_into().ok()?) as usize;
    let raw_len = u32::from_le_bytes(bytes[21..25].try_into().ok()?) as usize;
    let raw = decompress_zlib(&bytes[25..]).ok()?;
    if raw.len() != raw_len {
        return None;
    }

    let vertex_bytes = vertex_count.checked_mul(24)?;
    let index_bytes = index_count.checked_mul(4)?;
    if raw.len() < vertex_bytes + index_bytes {
        return None;
    }

    let mut vertices = Vec::with_capacity(vertex_count);
    for i in 0..vertex_count {
        let off = i * 24;
        vertices.push(MeshVertex {
            pos: [
                f32::from_le_bytes(raw[off..off + 4].try_into().ok()?),
                f32::from_le_bytes(raw[off + 4..off + 8].try_into().ok()?),
                f32::from_le_bytes(raw[off + 8..off + 12].try_into().ok()?),
            ],
            normal: [
                f32::from_le_bytes(raw[off + 12..off + 16].try_into().ok()?),
                f32::from_le_bytes(raw[off + 16..off + 20].try_into().ok()?),
                f32::from_le_bytes(raw[off + 20..off + 24].try_into().ok()?),
            ],
        });
    }

    let mut indices = Vec::with_capacity(index_count);
    let index_start = vertex_bytes;
    for i in 0..index_count {
        let off = index_start + i * 4;
        indices.push(u32::from_le_bytes(raw[off..off + 4].try_into().ok()?));
    }
    Some(DecodedMesh { vertices, indices })
}

fn decode_gltf_mesh(bytes: &[u8], mesh_index: usize) -> Option<DecodedMesh> {
    let (doc, buffers, _images) = gltf::import_slice(bytes).ok()?;
    let mesh = doc.meshes().nth(mesh_index)?;
    let primitive = mesh.primitives().next()?;
    let reader = primitive.reader(|buffer| buffers.get(buffer.index()).map(|b| b.0.as_slice()));

    let positions = reader.read_positions()?;
    let normals: Vec<[f32; 3]> = reader
        .read_normals()
        .map(|iter| iter.collect())
        .unwrap_or_default();
    let mut vertices = Vec::new();
    for (i, position) in positions.enumerate() {
        vertices.push(MeshVertex {
            pos: position,
            normal: normals.get(i).copied().unwrap_or([0.0, 1.0, 0.0]),
        });
    }
    if vertices.is_empty() {
        return None;
    }

    let indices = if let Some(idx) = reader.read_indices() {
        idx.into_u32().collect()
    } else {
        (0..vertices.len() as u32).collect()
    };
    Some(DecodedMesh { vertices, indices })
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
                        wgpu::VertexAttribute {
                            offset: 80,
                            shader_location: 7,
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

fn build_scene_uniform(
    camera: Camera3DState,
    lighting: &Lighting3DState,
    width: u32,
    height: u32,
) -> Scene3DUniform {
    let mut scene = Scene3DUniform {
        view_proj: compute_view_proj(camera, width, height),
        ambient_and_counts: [0.0, 0.0, 0.0, 0.0],
        camera_pos: [
            camera.position[0],
            camera.position[1],
            camera.position[2],
            0.0,
        ],
        ambient_color: [1.0, 1.0, 1.0, 0.0],
        ray_light: RayLightGpu {
            direction: [0.0, 0.0, -1.0, 0.0],
            color_intensity: [1.0, 1.0, 1.0, 0.0],
        },
        point_lights: [PointLightGpu {
            position_range: [0.0, 0.0, 0.0, 1.0],
            color_intensity: [0.0, 0.0, 0.0, 0.0],
        }; MAX_POINT_LIGHTS],
        spot_lights: [SpotLightGpu {
            position_range: [0.0, 0.0, 0.0, 1.0],
            direction_outer_cos: [0.0, 0.0, -1.0, -1.0],
            color_intensity: [0.0, 0.0, 0.0, 0.0],
            inner_cos_pad: [1.0, 0.0, 0.0, 0.0],
        }; MAX_SPOT_LIGHTS],
    };

    if let Some(ambient) = lighting.ambient_light {
        scene.ambient_color = [
            ambient.color[0].max(0.0),
            ambient.color[1].max(0.0),
            ambient.color[2].max(0.0),
            ambient.intensity.max(0.0),
        ];
    }

    if let Some(ray) = lighting.ray_light {
        let dir = Vec3::from(ray.direction).normalize_or_zero();
        scene.ray_light = RayLightGpu {
            direction: [dir.x, dir.y, dir.z, 0.0],
            color_intensity: [
                ray.color[0].max(0.0),
                ray.color[1].max(0.0),
                ray.color[2].max(0.0),
                ray.intensity.max(0.0),
            ],
        };
        scene.ambient_and_counts[3] = 1.0;
    }

    let mut point_count = 0.0f32;
    for (dst, src) in scene
        .point_lights
        .iter_mut()
        .zip(lighting.point_lights.iter().flatten())
    {
        dst.position_range = [
            src.position[0],
            src.position[1],
            src.position[2],
            src.range.max(0.001),
        ];
        dst.color_intensity = [
            src.color[0].max(0.0),
            src.color[1].max(0.0),
            src.color[2].max(0.0),
            src.intensity.max(0.0),
        ];
        point_count += 1.0;
    }
    scene.ambient_and_counts[1] = point_count;

    let mut spot_count = 0.0f32;
    for (dst, src) in scene
        .spot_lights
        .iter_mut()
        .zip(lighting.spot_lights.iter().flatten())
    {
        let dir = Vec3::from(src.direction).normalize_or_zero();
        let inner = src.inner_angle_radians.max(0.0);
        let outer = src.outer_angle_radians.max(inner + 1.0e-4);
        dst.position_range = [
            src.position[0],
            src.position[1],
            src.position[2],
            src.range.max(0.001),
        ];
        dst.direction_outer_cos = [dir.x, dir.y, dir.z, outer.cos()];
        dst.color_intensity = [
            src.color[0].max(0.0),
            src.color[1].max(0.0),
            src.color[2].max(0.0),
            src.intensity.max(0.0),
        ];
        dst.inner_cos_pad = [inner.cos(), 0.0, 0.0, 0.0];
        spot_count += 1.0;
    }
    scene.ambient_and_counts[2] = spot_count;

    scene
}
