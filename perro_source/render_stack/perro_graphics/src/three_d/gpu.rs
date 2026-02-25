use super::{
    renderer::{Draw3DInstance, Lighting3DState, MAX_POINT_LIGHTS, MAX_SPOT_LIGHTS},
    shaders::create_mesh_shader_module,
};
use crate::backend::StaticMeshLookup;
use crate::resources::ResourceStore;
use bytemuck::{Pod, Zeroable};
use glam::{Mat4, Quat, Vec3, Vec4};
use mesh_presets::build_builtin_mesh_buffer;
use perro_io::{decompress_zlib, load_asset};
use perro_meshlets::pack_meshlets_from_positions;
use perro_render_bridge::Camera3DState;
use std::{
    collections::HashMap,
    sync::{Arc, mpsc, mpsc::TryRecvError},
    time::Instant,
};
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
    pbr_params: [f32; 4], // roughness, metallic, occlusion_strength, normal_scale
    emissive_factor: [f32; 3], // rgb
    material_params: [f32; 4], // alpha_mode, alpha_cutoff, double_sided, reserved
}

pub struct Gpu3D {
    camera_bgl: wgpu::BindGroupLayout,
    pipeline_culled: wgpu::RenderPipeline,
    pipeline_double_sided: wgpu::RenderPipeline,
    camera_buffer: wgpu::Buffer,
    camera_bind_group: wgpu::BindGroup,
    instance_buffer: wgpu::Buffer,
    instance_capacity: usize,
    staged_instances: Vec<InstanceGpu>,
    draw_batches: Vec<DrawBatch>,
    last_draws: Vec<Draw3DInstance>,
    last_scene: Option<Scene3DUniform>,
    mesh_vertices: Vec<MeshVertex>,
    mesh_indices: Vec<u32>,
    vertex_buffer: wgpu::Buffer,
    index_buffer: wgpu::Buffer,
    vertex_capacity: usize,
    index_capacity: usize,
    builtin_mesh_ranges: HashMap<&'static str, MeshRange>,
    builtin_meshlets: HashMap<&'static str, Arc<[MeshletRange]>>,
    custom_mesh_ranges: HashMap<String, MeshAssetRange>,
    depth_texture: wgpu::Texture,
    depth_view: wgpu::TextureView,
    depth_size: (u32, u32),
    sample_count: u32,
    meshlets_enabled: bool,
    dev_meshlets: bool,
    meshlet_debug_view: bool,
    occlusion_enabled: bool,
    last_total_meshlets: usize,
    last_total_drawn: usize,
    occlusion_frame: u64,
    occlusion_state: HashMap<u64, OcclusionState>,
    occlusion_query_set: Option<wgpu::QuerySet>,
    occlusion_query_capacity: u32,
    occlusion_resolve_buffer: Option<wgpu::Buffer>,
    occlusion_readback_buffer: Option<wgpu::Buffer>,
    occlusion_query_keys_this_frame: Vec<u64>,
    pending_occlusion_query_keys: Vec<u64>,
    pending_occlusion_query_count: u32,
    pending_occlusion_map_rx: Option<mpsc::Receiver<Result<(), wgpu::BufferAsyncError>>>,
    last_occlusion_queried: u32,
    last_occlusion_visible: u32,
    last_occlusion_culled: u32,
    occlusion_print_cooldown: u32,
}

pub struct Prepare3D<'a> {
    pub resources: &'a ResourceStore,
    pub camera: Camera3DState,
    pub lighting: &'a Lighting3DState,
    pub draws: &'a [Draw3DInstance],
    pub width: u32,
    pub height: u32,
    pub static_mesh_lookup: Option<StaticMeshLookup>,
}

#[derive(Clone, Copy)]
struct MeshRange {
    index_start: u32,
    index_count: u32,
    base_vertex: i32,
}

#[derive(Clone, Copy)]
struct MeshletRange {
    index_start: u32,
    index_count: u32,
    center: [f32; 3],
    radius: f32,
}

#[derive(Clone)]
struct MeshAssetRange {
    full: MeshRange,
    meshlets: Arc<[MeshletRange]>,
}

#[derive(Clone, Copy)]
struct DrawBatch {
    mesh: MeshRange,
    instance_start: u32,
    instance_count: u32,
    double_sided: bool,
    occlusion_key: u64,
    occlusion_query: Option<u32>,
}

#[derive(Clone, Copy)]
struct OcclusionState {
    visible_last_frame: bool,
    last_test_frame: u64,
}

const PMESH_MAGIC: &[u8; 5] = b"PMESH";
// Re-test occluded batches frequently so visibility recovers quickly when camera/object moves.
const OCCLUSION_PROBE_INTERVAL: u64 = 3;

#[derive(Clone)]
struct DecodedMesh {
    vertices: Vec<MeshVertex>,
    indices: Vec<u32>,
    meshlets: Vec<DecodedMeshlet>,
}

#[derive(Clone, Copy)]
struct DecodedMeshlet {
    index_start: u32,
    index_count: u32,
    center: [f32; 3],
    radius: f32,
}

impl Gpu3D {
    pub fn new(
        device: &wgpu::Device,
        color_format: wgpu::TextureFormat,
        sample_count: u32,
        width: u32,
        height: u32,
        meshlets_enabled: bool,
        dev_meshlets: bool,
        meshlet_debug_view: bool,
        occlusion_enabled: bool,
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
        let pipeline_culled = create_pipeline(
            device,
            &pipeline_layout,
            &shader,
            color_format,
            sample_count,
            Some(wgpu::Face::Back),
        );
        let pipeline_double_sided = create_pipeline(
            device,
            &pipeline_layout,
            &shader,
            color_format,
            sample_count,
            None,
        );

        let (vertices, indices, builtin_mesh_ranges, builtin_meshlets) =
            build_builtin_mesh_buffer();
        let vertex_capacity = vertices.len().max(1);
        let index_capacity = indices.len().max(1);
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
            pipeline_culled,
            pipeline_double_sided,
            camera_buffer,
            camera_bind_group,
            instance_buffer,
            instance_capacity,
            staged_instances: Vec::new(),
            draw_batches: Vec::new(),
            last_draws: Vec::new(),
            last_scene: None,
            mesh_vertices: vertices,
            mesh_indices: indices,
            vertex_buffer,
            index_buffer,
            vertex_capacity,
            index_capacity,
            builtin_mesh_ranges,
            builtin_meshlets,
            custom_mesh_ranges: HashMap::new(),
            depth_texture,
            depth_view,
            depth_size: (width.max(1), height.max(1)),
            sample_count,
            meshlets_enabled,
            dev_meshlets,
            meshlet_debug_view,
            occlusion_enabled,
            last_total_meshlets: 0,
            last_total_drawn: 0,
            occlusion_frame: 0,
            occlusion_state: HashMap::new(),
            occlusion_query_set: None,
            occlusion_query_capacity: 0,
            occlusion_resolve_buffer: None,
            occlusion_readback_buffer: None,
            occlusion_query_keys_this_frame: Vec::new(),
            pending_occlusion_query_keys: Vec::new(),
            pending_occlusion_query_count: 0,
            pending_occlusion_map_rx: None,
            last_occlusion_queried: 0,
            last_occlusion_visible: 0,
            last_occlusion_culled: 0,
            occlusion_print_cooldown: 0,
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
        self.pipeline_culled = create_pipeline(
            device,
            &pipeline_layout,
            &shader,
            color_format,
            sample_count,
            Some(wgpu::Face::Back),
        );
        self.pipeline_double_sided = create_pipeline(
            device,
            &pipeline_layout,
            &shader,
            color_format,
            sample_count,
            None,
        );
        let (depth_texture, depth_view) = create_depth_texture(device, width, height, sample_count);
        self.depth_texture = depth_texture;
        self.depth_view = depth_view;
        self.depth_size = (width.max(1), height.max(1));
        self.sample_count = sample_count;
    }

    pub fn prepare(&mut self, device: &wgpu::Device, queue: &wgpu::Queue, frame: Prepare3D<'_>) {
        if self.occlusion_enabled {
            let _ = device.poll(wgpu::PollType::Poll);
            if self.pending_occlusion_query_count > 0 && self.pending_occlusion_map_rx.is_none() {
                self.request_occlusion_map_async();
            }
            self.consume_occlusion_results();
            self.occlusion_frame = self.occlusion_frame.wrapping_add(1);
        }
        self.occlusion_query_keys_this_frame.clear();
        let occlusion_capture_this_frame = self.occlusion_enabled
            && self.pending_occlusion_query_count == 0
            && self.pending_occlusion_map_rx.is_none();

        let Prepare3D {
            resources,
            camera,
            lighting,
            draws,
            width,
            height,
            static_mesh_lookup,
        } = frame;
        self.custom_mesh_ranges
            .retain(|source, _| resources.has_mesh_source(source));
        self.resize(device, width, height);
        self.ensure_instance_capacity(device, draws.len());

        let uniform = build_scene_uniform(camera, lighting, width, height);
        if self.last_scene != Some(uniform) {
            queue.write_buffer(&self.camera_buffer, 0, bytemuck::bytes_of(&uniform));
            self.last_scene = Some(uniform);
        }
        let view_proj = compute_view_proj_mat(camera, width, height);

        if self.last_draws.as_slice() == draws {
            return;
        }
        self.last_draws.clear();
        self.last_draws.extend_from_slice(draws);

        self.staged_instances.clear();
        self.staged_instances.reserve(draws.len());
        self.draw_batches.clear();
        self.draw_batches.reserve(draws.len());
        let mut total_meshlets = 0usize;
        let frustum = extract_frustum_planes(view_proj);
        let default_mesh = self
            .resolve_builtin_mesh_asset("__cube__")
            .expect("cube mesh preset must exist");
        for draw in draws {
            let source = resources.mesh_source(draw.mesh).unwrap_or("__cube__");
            let mesh_asset = self
                .resolve_mesh_range(device, queue, source, static_mesh_lookup)
                .unwrap_or_else(|| default_mesh.clone());
            let material = resources.material(draw.material).unwrap_or_default();
            let use_meshlets = self.meshlets_enabled && !mesh_asset.meshlets.is_empty();
            total_meshlets = total_meshlets.saturating_add(if use_meshlets {
                mesh_asset.meshlets.len()
            } else {
                1
            });

            // For non-meshlet draws, keep coarse center culling.
            // For meshlet-enabled draws, rely on per-meshlet bounds culling.
            if !use_meshlets && !draw_center_in_frustum(draw, view_proj) {
                continue;
            }

            if !use_meshlets {
                let occlusion_key = draw.node.as_u64();
                if self.occlusion_enabled && !self.should_probe_or_draw(occlusion_key) {
                    continue;
                }
                let occlusion_query = if occlusion_capture_this_frame {
                    Some(self.push_occlusion_query_key(occlusion_key))
                } else {
                    None
                };
                let instance = self.staged_instances.len() as u32;
                self.staged_instances.push(build_instance(
                    draw.model,
                    &material,
                    self.meshlet_debug_view,
                    debug_color(draw.node.as_u64()),
                ));
                push_draw_batch(
                    &mut self.draw_batches,
                    mesh_asset.full,
                    instance,
                    material.double_sided,
                    occlusion_key,
                    occlusion_query,
                );
            } else {
                for meshlet in mesh_asset.meshlets.iter().copied() {
                    if meshlet_in_frustum(draw.model, meshlet, &frustum) {
                        let occlusion_key =
                            (draw.node.as_u64() << 32) ^ u64::from(meshlet.index_start);
                        if self.occlusion_enabled && !self.should_probe_or_draw(occlusion_key) {
                            continue;
                        }
                        let occlusion_query = if occlusion_capture_this_frame {
                            Some(self.push_occlusion_query_key(occlusion_key))
                        } else {
                            None
                        };
                        let instance = self.staged_instances.len() as u32;
                        self.staged_instances.push(build_instance(
                            draw.model,
                            &material,
                            self.meshlet_debug_view,
                            debug_color((draw.node.as_u64() << 32) ^ meshlet.index_start as u64),
                        ));
                        push_draw_batch(
                            &mut self.draw_batches,
                            MeshRange {
                                index_start: meshlet.index_start,
                                index_count: meshlet.index_count,
                                base_vertex: mesh_asset.full.base_vertex,
                            },
                            instance,
                            material.double_sided,
                            occlusion_key,
                            occlusion_query,
                        );
                    }
                }
            }
        }
        if occlusion_capture_this_frame {
            self.ensure_occlusion_query_capacity(
                device,
                self.occlusion_query_keys_this_frame.len() as u32,
            );
        }
        if !self.staged_instances.is_empty() {
            queue.write_buffer(
                &self.instance_buffer,
                0,
                bytemuck::cast_slice(&self.staged_instances),
            );
        }
        self.last_total_meshlets = total_meshlets;
        self.last_total_drawn = self.staged_instances.len();
    }

    pub fn render_pass(
        &mut self,
        encoder: &mut wgpu::CommandEncoder,
        color_view: &wgpu::TextureView,
        clear_color: wgpu::Color,
    ) {
        let query_count = if self.occlusion_enabled
            && self.pending_occlusion_query_count == 0
            && self.pending_occlusion_map_rx.is_none()
        {
            self.occlusion_query_keys_this_frame.len() as u32
        } else {
            0
        };
        let query_set = if query_count > 0 {
            self.occlusion_query_set.as_ref()
        } else {
            None
        };
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
            occlusion_query_set: query_set,
            multiview_mask: None,
        });
        if self.draw_batches.is_empty() {
            return;
        }
        pass.set_bind_group(0, &self.camera_bind_group, &[]);
        pass.set_vertex_buffer(0, self.vertex_buffer.slice(..));
        pass.set_vertex_buffer(1, self.instance_buffer.slice(..));
        pass.set_index_buffer(self.index_buffer.slice(..), wgpu::IndexFormat::Uint32);
        let mut current_double_sided = None;
        for batch in &self.draw_batches {
            if current_double_sided != Some(batch.double_sided) {
                let pipeline = if batch.double_sided {
                    &self.pipeline_double_sided
                } else {
                    &self.pipeline_culled
                };
                pass.set_pipeline(pipeline);
                current_double_sided = Some(batch.double_sided);
            }
            let start = batch.mesh.index_start;
            let end = start + batch.mesh.index_count;
            let instances = batch.instance_start..batch.instance_start + batch.instance_count;
            if let Some(query_index) = batch.occlusion_query {
                pass.begin_occlusion_query(query_index);
                pass.draw_indexed(start..end, batch.mesh.base_vertex, instances);
                pass.end_occlusion_query();
            } else {
                pass.draw_indexed(start..end, batch.mesh.base_vertex, instances);
            }
        }
        drop(pass);

        if query_count > 0
            && let (Some(query_set), Some(resolve), Some(readback)) = (
                self.occlusion_query_set.as_ref(),
                self.occlusion_resolve_buffer.as_ref(),
                self.occlusion_readback_buffer.as_ref(),
            )
        {
            let byte_len = u64::from(query_count) * 8;
            encoder.resolve_query_set(query_set, 0..query_count, resolve, 0);
            encoder.copy_buffer_to_buffer(resolve, 0, readback, 0, byte_len);

            self.pending_occlusion_query_count = query_count;
            self.pending_occlusion_query_keys.clear();
            self.pending_occlusion_query_keys
                .extend(self.occlusion_query_keys_this_frame.iter().copied());
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

    fn should_probe_or_draw(&self, key: u64) -> bool {
        let Some(state) = self.occlusion_state.get(&key) else {
            return true;
        };
        state.visible_last_frame
            || self.occlusion_frame.saturating_sub(state.last_test_frame)
                >= OCCLUSION_PROBE_INTERVAL
    }

    fn push_occlusion_query_key(&mut self, key: u64) -> u32 {
        let query = self.occlusion_query_keys_this_frame.len() as u32;
        self.occlusion_query_keys_this_frame.push(key);
        query
    }

    fn ensure_occlusion_query_capacity(&mut self, device: &wgpu::Device, needed: u32) {
        if !self.occlusion_enabled {
            return;
        }
        if needed == 0 || needed <= self.occlusion_query_capacity {
            return;
        }
        let mut capacity = self.occlusion_query_capacity.max(64);
        while capacity < needed {
            capacity = capacity.saturating_mul(2);
        }
        self.occlusion_query_set = Some(device.create_query_set(&wgpu::QuerySetDescriptor {
            label: Some("perro_occlusion_query_set"),
            ty: wgpu::QueryType::Occlusion,
            count: capacity,
        }));
        let byte_len = u64::from(capacity) * 8;
        self.occlusion_resolve_buffer = Some(device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("perro_occlusion_resolve"),
            size: byte_len,
            usage: wgpu::BufferUsages::QUERY_RESOLVE | wgpu::BufferUsages::COPY_SRC,
            mapped_at_creation: false,
        }));
        self.occlusion_readback_buffer = Some(device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("perro_occlusion_readback"),
            size: byte_len,
            usage: wgpu::BufferUsages::COPY_DST | wgpu::BufferUsages::MAP_READ,
            mapped_at_creation: false,
        }));
        self.occlusion_query_capacity = capacity;
    }

    fn request_occlusion_map_async(&mut self) {
        if self.pending_occlusion_query_count == 0 || self.pending_occlusion_map_rx.is_some() {
            return;
        }
        let Some(readback) = self.occlusion_readback_buffer.as_ref() else {
            return;
        };
        let byte_len = u64::from(self.pending_occlusion_query_count) * 8;
        let (tx, rx) = mpsc::channel();
        readback
            .slice(0..byte_len)
            .map_async(wgpu::MapMode::Read, move |result| {
                let _ = tx.send(result);
            });
        self.pending_occlusion_map_rx = Some(rx);
    }

    fn consume_occlusion_results(&mut self) {
        if !self.occlusion_enabled {
            return;
        }
        let query_count = self.pending_occlusion_query_count as usize;
        if query_count == 0 {
            return;
        }
        let Some(rx) = self.pending_occlusion_map_rx.as_ref() else {
            return;
        };
        let Some(readback) = self.occlusion_readback_buffer.as_ref() else {
            self.pending_occlusion_query_count = 0;
            self.pending_occlusion_query_keys.clear();
            self.pending_occlusion_map_rx = None;
            return;
        };
        match rx.try_recv() {
            Ok(Ok(())) => {
                let byte_len = (query_count * 8) as u64;
                let data = readback.slice(0..byte_len).get_mapped_range();
                let mut visible = 0u32;
                for (i, bytes) in data.chunks_exact(8).enumerate() {
                    let samples =
                        u64::from_le_bytes(bytes.try_into().expect("8-byte occlusion sample"));
                    if samples > 0 {
                        visible = visible.saturating_add(1);
                    }
                    if let Some(key) = self.pending_occlusion_query_keys.get(i).copied() {
                        self.occlusion_state.insert(
                            key,
                            OcclusionState {
                                visible_last_frame: samples > 0,
                                last_test_frame: self.occlusion_frame,
                            },
                        );
                    }
                }
                drop(data);
                readback.unmap();
                self.last_occlusion_queried = query_count as u32;
                self.last_occlusion_visible = visible;
                self.last_occlusion_culled = (query_count as u32).saturating_sub(visible);
                if self.occlusion_print_cooldown == 0 {
                    println!(
                        "[perro][3d][occlusion] queried={} visible={} culled={}",
                        self.last_occlusion_queried,
                        self.last_occlusion_visible,
                        self.last_occlusion_culled
                    );
                    self.occlusion_print_cooldown = 20;
                } else {
                    self.occlusion_print_cooldown = self.occlusion_print_cooldown.saturating_sub(1);
                }
                self.pending_occlusion_query_count = 0;
                self.pending_occlusion_query_keys.clear();
                self.pending_occlusion_map_rx = None;
            }
            Ok(Err(_)) | Err(TryRecvError::Disconnected) => {
                readback.unmap();
                self.pending_occlusion_query_count = 0;
                self.pending_occlusion_query_keys.clear();
                self.pending_occlusion_map_rx = None;
            }
            Err(TryRecvError::Empty) => {}
        }
    }

    fn resolve_mesh_range(
        &mut self,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        source: &str,
        static_mesh_lookup: Option<StaticMeshLookup>,
    ) -> Option<MeshAssetRange> {
        if let Some(range) = self.builtin_mesh_ranges.get(source).copied() {
            return Some(MeshAssetRange {
                full: range,
                meshlets: self
                    .builtin_meshlets
                    .get(source)
                    .cloned()
                    .unwrap_or_else(|| Arc::from([])),
            });
        }
        if let Some(range) = self.custom_mesh_ranges.get(source).cloned() {
            return Some(range);
        }
        let decoded = load_mesh_from_source(
            source,
            static_mesh_lookup,
            self.meshlets_enabled && self.dev_meshlets,
        )?;
        let range = self.append_mesh_data(device, queue, source, decoded)?;
        self.custom_mesh_ranges
            .insert(source.to_string(), range.clone());
        Some(range)
    }

    fn resolve_builtin_mesh_asset(&self, source: &str) -> Option<MeshAssetRange> {
        let full = self.builtin_mesh_ranges.get(source).copied()?;
        let meshlets = self
            .builtin_meshlets
            .get(source)
            .cloned()
            .unwrap_or_else(|| Arc::from([]));
        Some(MeshAssetRange { full, meshlets })
    }

    fn append_mesh_data(
        &mut self,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        _source: &str,
        decoded: DecodedMesh,
    ) -> Option<MeshAssetRange> {
        if decoded.vertices.is_empty() || decoded.indices.is_empty() {
            return None;
        }
        let base_vertex = self.mesh_vertices.len() as u32;
        let index_start = self.mesh_indices.len() as u32;
        let index_count = decoded.indices.len() as u32;

        let added_vertices = decoded.vertices;
        let mut added_indices = Vec::with_capacity(decoded.indices.len());
        for idx in decoded.indices {
            added_indices.push(idx + base_vertex);
        }

        let new_vertex_len = self.mesh_vertices.len() + added_vertices.len();
        let new_index_len = self.mesh_indices.len() + added_indices.len();
        let grew = self.ensure_mesh_buffer_capacity(device, new_vertex_len, new_index_len);

        let vertex_offset =
            self.mesh_vertices.len() as u64 * std::mem::size_of::<MeshVertex>() as u64;
        let index_offset = self.mesh_indices.len() as u64 * std::mem::size_of::<u32>() as u64;

        self.mesh_vertices.extend_from_slice(&added_vertices);
        self.mesh_indices.extend_from_slice(&added_indices);

        let _ = grew;
        queue.write_buffer(
            &self.vertex_buffer,
            vertex_offset,
            bytemuck::cast_slice(&added_vertices),
        );
        queue.write_buffer(
            &self.index_buffer,
            index_offset,
            bytemuck::cast_slice(&added_indices),
        );

        let full = MeshRange {
            index_start,
            index_count,
            base_vertex: 0,
        };

        let meshlets: Vec<MeshletRange> = decoded
            .meshlets
            .into_iter()
            .filter_map(|meshlet| {
                if meshlet.index_count == 0 {
                    return None;
                }
                Some(MeshletRange {
                    index_start: index_start + meshlet.index_start,
                    index_count: meshlet.index_count,
                    center: meshlet.center,
                    radius: meshlet.radius.max(0.0),
                })
            })
            .collect();

        Some(MeshAssetRange {
            full,
            meshlets: Arc::from(meshlets),
        })
    }

    fn ensure_mesh_buffer_capacity(
        &mut self,
        device: &wgpu::Device,
        needed_vertices: usize,
        needed_indices: usize,
    ) -> bool {
        let mut grew = false;

        if needed_vertices > self.vertex_capacity {
            let mut cap = self.vertex_capacity.max(1);
            while cap < needed_vertices {
                cap *= 2;
            }
            self.vertex_capacity = cap;
            grew = true;
        }

        if needed_indices > self.index_capacity {
            let mut cap = self.index_capacity.max(1);
            while cap < needed_indices {
                cap *= 2;
            }
            self.index_capacity = cap;
            grew = true;
        }

        if grew {
            let mut vertex_bytes =
                vec![0u8; self.vertex_capacity * std::mem::size_of::<MeshVertex>()];
            let used_vertex_bytes = bytemuck::cast_slice(&self.mesh_vertices);
            vertex_bytes[..used_vertex_bytes.len()].copy_from_slice(used_vertex_bytes);
            self.vertex_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
                label: Some("perro_mesh_vertices"),
                contents: &vertex_bytes,
                usage: wgpu::BufferUsages::VERTEX | wgpu::BufferUsages::COPY_DST,
            });

            let mut index_bytes = vec![0u8; self.index_capacity * std::mem::size_of::<u32>()];
            let used_index_bytes = bytemuck::cast_slice(&self.mesh_indices);
            index_bytes[..used_index_bytes.len()].copy_from_slice(used_index_bytes);
            self.index_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
                label: Some("perro_mesh_indices"),
                contents: &index_bytes,
                usage: wgpu::BufferUsages::INDEX | wgpu::BufferUsages::COPY_DST,
            });
        }

        grew
    }
}

fn load_mesh_from_source(
    source: &str,
    static_mesh_lookup: Option<StaticMeshLookup>,
    dev_meshlets: bool,
) -> Option<DecodedMesh> {
    let mut decoded = if let Some(lookup) = static_mesh_lookup
        && let Some(bytes) = lookup(source)
        && let Some(decoded) = decode_pmesh(bytes)
    {
        decoded
    } else {
        let (path, fragment) = split_source_fragment(source);
        if path.ends_with(".pmesh") {
            let bytes = load_asset(path).ok()?;
            decode_pmesh(&bytes)?
        } else if path.ends_with(".glb") || path.ends_with(".gltf") {
            let mesh_index = parse_fragment_index(fragment, "mesh").unwrap_or(0);
            let bytes = load_asset(path).ok()?;
            decode_gltf_mesh(&bytes, mesh_index as usize)?
        } else {
            return None;
        }
    };

    if decoded.meshlets.is_empty() && dev_meshlets {
        let start = Instant::now();
        let (packed_indices, meshlets) = build_meshlets(&decoded.vertices, &decoded.indices);
        decoded.indices = packed_indices;
        decoded.meshlets = meshlets;
        let elapsed_ms = start.elapsed().as_secs_f64() * 1000.0;
        eprintln!(
            "[perro][3d][meshlets] generated source={} meshlets={} tris={} took_ms={:.2}",
            source,
            decoded.meshlets.len(),
            decoded.indices.len() / 3,
            elapsed_ms
        );
    }

    Some(decoded)
}

pub(crate) fn validate_mesh_source(
    source: &str,
    static_mesh_lookup: Option<StaticMeshLookup>,
) -> Result<(), String> {
    if source.starts_with("__") {
        return Ok(());
    }
    if load_mesh_from_source(source, static_mesh_lookup, false).is_some() {
        return Ok(());
    }
    Err(format!("mesh source failed to decode: {}", source))
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

const MESHLET_TRIANGLES: usize = 64;

fn build_meshlets(vertices: &[MeshVertex], indices: &[u32]) -> (Vec<u32>, Vec<DecodedMeshlet>) {
    let positions: Vec<[f32; 3]> = vertices.iter().map(|v| v.pos).collect();
    let packed = pack_meshlets_from_positions(&positions, indices, MESHLET_TRIANGLES);
    let meshlets = packed
        .meshlets
        .into_iter()
        .map(|m| DecodedMeshlet {
            index_start: m.index_start,
            index_count: m.index_count,
            center: m.center,
            radius: m.radius,
        })
        .collect();
    (packed.packed_indices, meshlets)
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
    let meshlet_count = u32::from_le_bytes(bytes[17..21].try_into().ok()?) as usize;
    let raw_len = u32::from_le_bytes(bytes[21..25].try_into().ok()?) as usize;
    let raw = decompress_zlib(&bytes[25..]).ok()?;
    if raw.len() != raw_len {
        return None;
    }

    let vertex_bytes = vertex_count.checked_mul(24)?;
    let index_bytes = index_count.checked_mul(4)?;
    let meshlet_bytes = meshlet_count.checked_mul(24)?;
    let required = vertex_bytes
        .checked_add(index_bytes)?
        .checked_add(meshlet_bytes)?;
    if raw.len() < required {
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
    let mut meshlets = Vec::with_capacity(meshlet_count);
    let meshlet_start = vertex_bytes + index_bytes;
    for i in 0..meshlet_count {
        let off = meshlet_start + i * 24;
        meshlets.push(DecodedMeshlet {
            index_start: u32::from_le_bytes(raw[off..off + 4].try_into().ok()?),
            index_count: u32::from_le_bytes(raw[off + 4..off + 8].try_into().ok()?),
            center: [
                f32::from_le_bytes(raw[off + 8..off + 12].try_into().ok()?),
                f32::from_le_bytes(raw[off + 12..off + 16].try_into().ok()?),
                f32::from_le_bytes(raw[off + 16..off + 20].try_into().ok()?),
            ],
            radius: f32::from_le_bytes(raw[off + 20..off + 24].try_into().ok()?),
        });
    }

    Some(DecodedMesh {
        vertices,
        indices,
        meshlets,
    })
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
    Some(DecodedMesh {
        vertices,
        indices,
        meshlets: Vec::new(),
    })
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
    cull_mode: Option<wgpu::Face>,
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
                        wgpu::VertexAttribute {
                            offset: 96,
                            shader_location: 8,
                            format: wgpu::VertexFormat::Float32x3,
                        },
                        wgpu::VertexAttribute {
                            offset: 108,
                            shader_location: 9,
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
            cull_mode,
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

#[inline]
fn push_draw_batch(
    draw_batches: &mut Vec<DrawBatch>,
    mesh: MeshRange,
    instance: u32,
    double_sided: bool,
    occlusion_key: u64,
    occlusion_query: Option<u32>,
) {
    if let Some(batch) = draw_batches.last_mut()
        && batch.mesh.index_start == mesh.index_start
        && batch.mesh.index_count == mesh.index_count
        && batch.mesh.base_vertex == mesh.base_vertex
        && batch.double_sided == double_sided
        && batch.occlusion_key == occlusion_key
        && batch.occlusion_query == occlusion_query
        && batch.instance_start + batch.instance_count == instance
    {
        batch.instance_count += 1;
        return;
    }
    draw_batches.push(DrawBatch {
        mesh,
        instance_start: instance,
        instance_count: 1,
        double_sided,
        occlusion_key,
        occlusion_query,
    });
}

#[inline]
fn build_instance(
    model: [[f32; 4]; 4],
    material: &perro_render_bridge::Material3D,
    debug_view: bool,
    debug_color: [f32; 4],
) -> InstanceGpu {
    let (color, pbr_params, emissive_factor, debug_flag) = if debug_view {
        (debug_color, [0.5, 0.0, 1.0, 1.0], [0.0, 0.0, 0.0], 1.0)
    } else {
        (
            material.base_color_factor,
            [
                material.roughness_factor,
                material.metallic_factor,
                material.occlusion_strength,
                material.normal_scale,
            ],
            material.emissive_factor,
            0.0,
        )
    };

    InstanceGpu {
        model_0: model[0],
        model_1: model[1],
        model_2: model[2],
        model_3: model[3],
        color,
        pbr_params,
        emissive_factor,
        material_params: [
            material.alpha_mode as f32,
            material.alpha_cutoff,
            if material.double_sided { 1.0 } else { 0.0 },
            debug_flag,
        ],
    }
}

#[inline]
fn debug_color(seed: u64) -> [f32; 4] {
    let mut x = seed ^ 0x9E37_79B9_7F4A_7C15;
    x ^= x >> 30;
    x = x.wrapping_mul(0xBF58_476D_1CE4_E5B9);
    x ^= x >> 27;
    x = x.wrapping_mul(0x94D0_49BB_1331_11EB);
    x ^= x >> 31;

    let h = ((x & 0xFFFF) as f32) / 65535.0;
    hsv_to_rgb(h, 0.75, 0.95)
}

fn hsv_to_rgb(h: f32, s: f32, v: f32) -> [f32; 4] {
    let h = (h.fract() * 6.0).max(0.0);
    let i = h.floor() as i32;
    let f = h - i as f32;
    let p = v * (1.0 - s);
    let q = v * (1.0 - s * f);
    let t = v * (1.0 - s * (1.0 - f));
    let (r, g, b) = match i.rem_euclid(6) {
        0 => (v, t, p),
        1 => (q, v, p),
        2 => (p, v, t),
        3 => (p, q, v),
        4 => (t, p, v),
        _ => (v, p, q),
    };
    [r, g, b, 1.0]
}

fn compute_view_proj(camera: Camera3DState, width: u32, height: u32) -> [[f32; 4]; 4] {
    compute_view_proj_mat(camera, width, height).to_cols_array_2d()
}

fn compute_view_proj_mat(camera: Camera3DState, width: u32, height: u32) -> Mat4 {
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
    proj * view
}

#[inline]
fn draw_center_in_frustum(draw: &Draw3DInstance, view_proj: Mat4) -> bool {
    let center = Vec4::new(draw.model[3][0], draw.model[3][1], draw.model[3][2], 1.0);
    if !center.is_finite() {
        return false;
    }
    let clip = view_proj * center;
    if !clip.is_finite() || clip.w <= 0.0 {
        return false;
    }
    clip.x >= -clip.w
        && clip.x <= clip.w
        && clip.y >= -clip.w
        && clip.y <= clip.w
        && clip.z >= -clip.w
        && clip.z <= clip.w
}

fn extract_frustum_planes(view_proj: Mat4) -> [Vec4; 6] {
    let r0 = Vec4::new(
        view_proj.x_axis.x,
        view_proj.y_axis.x,
        view_proj.z_axis.x,
        view_proj.w_axis.x,
    );
    let r1 = Vec4::new(
        view_proj.x_axis.y,
        view_proj.y_axis.y,
        view_proj.z_axis.y,
        view_proj.w_axis.y,
    );
    let r2 = Vec4::new(
        view_proj.x_axis.z,
        view_proj.y_axis.z,
        view_proj.z_axis.z,
        view_proj.w_axis.z,
    );
    let r3 = Vec4::new(
        view_proj.x_axis.w,
        view_proj.y_axis.w,
        view_proj.z_axis.w,
        view_proj.w_axis.w,
    );
    [
        normalize_plane(r3 + r0),
        normalize_plane(r3 - r0),
        normalize_plane(r3 + r1),
        normalize_plane(r3 - r1),
        normalize_plane(r3 + r2),
        normalize_plane(r3 - r2),
    ]
}

#[inline]
fn normalize_plane(plane: Vec4) -> Vec4 {
    let n = plane.truncate();
    let len = n.length();
    if len > 1.0e-6 && len.is_finite() {
        plane / len
    } else {
        plane
    }
}

fn meshlet_in_frustum(model: [[f32; 4]; 4], meshlet: MeshletRange, planes: &[Vec4; 6]) -> bool {
    let model = Mat4::from_cols_array_2d(&model);
    if !model.is_finite() {
        return false;
    }
    let center_local = Vec4::new(meshlet.center[0], meshlet.center[1], meshlet.center[2], 1.0);
    let center_world = model * center_local;
    if !center_world.is_finite() {
        return false;
    }
    let sx = Vec3::new(model.x_axis.x, model.x_axis.y, model.x_axis.z).length();
    let sy = Vec3::new(model.y_axis.x, model.y_axis.y, model.y_axis.z).length();
    let sz = Vec3::new(model.z_axis.x, model.z_axis.y, model.z_axis.z).length();
    let scale = sx.max(sy).max(sz).max(1.0e-6);
    let radius_world = meshlet.radius.max(0.0) * scale;
    let center = center_world.truncate();

    for plane in planes {
        let d = plane.truncate().dot(center) + plane.w;
        if d < -radius_world {
            return false;
        }
    }
    true
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
