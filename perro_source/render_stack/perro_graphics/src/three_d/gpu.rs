use super::{
    renderer::{Draw3DInstance, Draw3DKind, Lighting3DState, MAX_POINT_LIGHTS, MAX_SPOT_LIGHTS},
    shaders::{
        create_depth_prepass_shader_module, create_frustum_cull_shader_module,
        create_hiz_depth_copy_shader_module, create_hiz_downsample_shader_module,
        create_hiz_occlusion_cull_shader_module, create_mesh_shader_module,
    },
};
use crate::backend::{OcclusionCullingMode, StaticMeshLookup};
use crate::resources::ResourceStore;
use bytemuck::{Pod, Zeroable};
use glam::{Mat4, Quat, Vec3, Vec4};
use mesh_presets::build_builtin_mesh_buffer;
use perro_io::{decompress_zlib, load_asset};
use perro_meshlets::pack_meshlets_from_positions;
use perro_render_bridge::{Camera3DState, CameraProjectionState, Material3D, RuntimeMeshData};
use std::{
    collections::HashMap,
    sync::{Arc, mpsc, mpsc::TryRecvError},
};
use wgpu::util::DeviceExt;

mod mesh_presets;

const DEPTH_FORMAT: wgpu::TextureFormat = wgpu::TextureFormat::Depth24Plus;
const DEPTH_PREPASS_FORMAT: wgpu::TextureFormat = wgpu::TextureFormat::Depth32Float;
const FRUSTUM_CULL_WORKGROUP_SIZE: u32 = 64;
const HIZ_WORKGROUP_SIZE_X: u32 = 8;
const HIZ_WORKGROUP_SIZE_Y: u32 = 8;
const HIZ_OCCLUSION_BIAS: f32 = 0.002;

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

#[repr(C)]
#[derive(Clone, Copy, Pod, Zeroable)]
struct FrustumCullParamsGpu {
    planes: [[f32; 4]; 6],
    draw_count: u32,
    _pad: [u32; 3],
}

#[repr(C)]
#[derive(Clone, Copy, Pod, Zeroable)]
struct FrustumCullItemGpu {
    model_0: [f32; 4],
    model_1: [f32; 4],
    model_2: [f32; 4],
    model_3: [f32; 4],
    local_center_radius: [f32; 4],
    cull_flags: [u32; 4],
}

#[repr(C)]
#[derive(Clone, Copy, Pod, Zeroable)]
struct DrawIndexedIndirectGpu {
    index_count: u32,
    instance_count: u32,
    first_index: u32,
    base_vertex: i32,
    first_instance: u32,
}

#[repr(C)]
#[derive(Clone, Copy, Pod, Zeroable)]
struct HizCullParamsGpu {
    view_proj: [[f32; 4]; 4],
    draw_count: u32,
    hiz_mip_count: u32,
    hiz_width: u32,
    hiz_height: u32,
    aspect: f32,
    proj_y_scale: f32,
    depth_bias: f32,
    _pad: u32,
}

pub struct Gpu3D {
    camera_bgl: wgpu::BindGroupLayout,
    pipeline_culled: wgpu::RenderPipeline,
    pipeline_double_sided: wgpu::RenderPipeline,
    pipeline_depth_prepass_culled: wgpu::RenderPipeline,
    pipeline_depth_prepass_double_sided: wgpu::RenderPipeline,
    camera_buffer: wgpu::Buffer,
    camera_bind_group: wgpu::BindGroup,
    instance_buffer: wgpu::Buffer,
    instance_capacity: usize,
    staged_instances: Vec<InstanceGpu>,
    frustum_cull_enabled: bool,
    frustum_cull_supported: bool,
    frustum_cull_pipeline: wgpu::ComputePipeline,
    frustum_cull_bgl: wgpu::BindGroupLayout,
    frustum_cull_bind_group: wgpu::BindGroup,
    frustum_cull_params_buffer: wgpu::Buffer,
    frustum_cull_items_buffer: wgpu::Buffer,
    frustum_cull_items_capacity: usize,
    frustum_cull_staging: Vec<FrustumCullItemGpu>,
    indirect_buffer: wgpu::Buffer,
    indirect_capacity: usize,
    indirect_staging: Vec<DrawIndexedIndirectGpu>,
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
    builtin_mesh_bounds: HashMap<&'static str, ([f32; 3], f32)>,
    builtin_meshlets: HashMap<&'static str, Arc<[MeshletRange]>>,
    custom_mesh_ranges: HashMap<String, MeshAssetRange>,
    depth_texture: wgpu::Texture,
    depth_view: wgpu::TextureView,
    depth_prepass_texture: wgpu::Texture,
    depth_prepass_view: wgpu::TextureView,
    depth_size: (u32, u32),
    gpu_occlusion_enabled: bool,
    hiz_texture: wgpu::Texture,
    hiz_mip_views: Vec<wgpu::TextureView>,
    hiz_sample_view: wgpu::TextureView,
    hiz_size: (u32, u32),
    hiz_mip_count: u32,
    hiz_copy_pipeline: wgpu::ComputePipeline,
    hiz_downsample_pipeline: wgpu::ComputePipeline,
    hiz_cull_pipeline: wgpu::ComputePipeline,
    hiz_copy_bgl: wgpu::BindGroupLayout,
    hiz_downsample_bgl: wgpu::BindGroupLayout,
    hiz_cull_bgl: wgpu::BindGroupLayout,
    hiz_cull_params: wgpu::Buffer,
    hiz_cull_bind_group: wgpu::BindGroup,
    hiz_debug_readback_buffer: wgpu::Buffer,
    pending_hiz_debug_count: u32,
    pending_hiz_debug_frustum_visible_est: u32,
    pending_hiz_debug_map_rx: Option<mpsc::Receiver<Result<(), wgpu::BufferAsyncError>>>,
    debug_frustum_visible_est: u32,
    last_aspect: f32,
    last_proj_y_scale: f32,
    sample_count: u32,
    occlusion_mode: OcclusionCullingMode,
    meshlets_enabled: bool,
    dev_meshlets: bool,
    meshlet_debug_view: bool,
    cpu_occlusion_enabled: bool,
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

pub struct Gpu3DConfig {
    pub sample_count: u32,
    pub width: u32,
    pub height: u32,
    pub meshlets_enabled: bool,
    pub dev_meshlets: bool,
    pub meshlet_debug_view: bool,
    pub occlusion_culling: OcclusionCullingMode,
    pub indirect_first_instance_enabled: bool,
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
    bounds_center: [f32; 3],
    bounds_radius: f32,
}

#[derive(Clone, Copy)]
struct DrawBatch {
    mesh: MeshRange,
    instance_start: u32,
    instance_count: u32,
    double_sided: bool,
    local_center: [f32; 3],
    local_radius: f32,
    occlusion_query: Option<u32>,
    disable_hiz_occlusion: bool,
}

#[derive(Clone, Copy)]
struct OcclusionState {
    visible_last_frame: bool,
    last_test_frame: u64,
}

const PMESH_MAGIC: &[u8; 5] = b"PMESH";
const CULL_FLAG_DISABLE_HIZ_OCCLUSION: u32 = 1u32;
// Re-test occluded batches every frame so visibility recovers immediately when camera/object moves.
const OCCLUSION_PROBE_INTERVAL: u64 = 1;

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
        config: Gpu3DConfig,
    ) -> Self {
        let Gpu3DConfig {
            sample_count,
            width,
            height,
            meshlets_enabled,
            dev_meshlets,
            meshlet_debug_view,
            occlusion_culling,
            indirect_first_instance_enabled,
        } = config;
        let (gpu_occlusion_enabled, cpu_occlusion_enabled) = occlusion_flags(occlusion_culling);
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
        let depth_prepass_shader = create_depth_prepass_shader_module(device);
        let pipeline_depth_prepass_culled = create_depth_prepass_pipeline(
            device,
            &pipeline_layout,
            &depth_prepass_shader,
            Some(wgpu::Face::Back),
        );
        let pipeline_depth_prepass_double_sided =
            create_depth_prepass_pipeline(device, &pipeline_layout, &depth_prepass_shader, None);

        let (vertices, indices, builtin_mesh_ranges, builtin_meshlets) =
            build_builtin_mesh_buffer();
        let builtin_mesh_bounds =
            compute_builtin_mesh_bounds(&vertices, &indices, &builtin_mesh_ranges);
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

        let frustum_cull_enabled = indirect_first_instance_enabled;
        let frustum_shader = create_frustum_cull_shader_module(device);
        let frustum_cull_bgl = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("perro_frustum_cull_bgl"),
            entries: &[
                wgpu::BindGroupLayoutEntry {
                    binding: 0,
                    visibility: wgpu::ShaderStages::COMPUTE,
                    ty: wgpu::BindingType::Buffer {
                        ty: wgpu::BufferBindingType::Uniform,
                        has_dynamic_offset: false,
                        min_binding_size: Some(
                            std::num::NonZeroU64::new(
                                std::mem::size_of::<FrustumCullParamsGpu>() as u64
                            )
                            .expect("frustum cull params size must be non-zero"),
                        ),
                    },
                    count: None,
                },
                wgpu::BindGroupLayoutEntry {
                    binding: 1,
                    visibility: wgpu::ShaderStages::COMPUTE,
                    ty: wgpu::BindingType::Buffer {
                        ty: wgpu::BufferBindingType::Storage { read_only: true },
                        has_dynamic_offset: false,
                        min_binding_size: None,
                    },
                    count: None,
                },
                wgpu::BindGroupLayoutEntry {
                    binding: 2,
                    visibility: wgpu::ShaderStages::COMPUTE,
                    ty: wgpu::BindingType::Buffer {
                        ty: wgpu::BufferBindingType::Storage { read_only: false },
                        has_dynamic_offset: false,
                        min_binding_size: None,
                    },
                    count: None,
                },
            ],
        });
        let frustum_cull_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("perro_frustum_cull_layout"),
            bind_group_layouts: &[&frustum_cull_bgl],
            immediate_size: 0,
        });
        let frustum_cull_pipeline =
            device.create_compute_pipeline(&wgpu::ComputePipelineDescriptor {
                label: Some("perro_frustum_cull_pipeline"),
                layout: Some(&frustum_cull_layout),
                module: &frustum_shader,
                entry_point: Some("cs_main"),
                compilation_options: Default::default(),
                cache: None,
            });
        let frustum_cull_params_buffer = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("perro_frustum_cull_params"),
            size: std::mem::size_of::<FrustumCullParamsGpu>() as u64,
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });
        let frustum_cull_items_capacity = 256usize;
        let frustum_cull_items_buffer = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("perro_frustum_cull_items"),
            size: (frustum_cull_items_capacity * std::mem::size_of::<FrustumCullItemGpu>()) as u64,
            usage: wgpu::BufferUsages::STORAGE | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });
        let indirect_capacity = 256usize;
        let indirect_buffer = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("perro_draw_indirect"),
            size: (indirect_capacity * std::mem::size_of::<DrawIndexedIndirectGpu>()) as u64,
            usage: wgpu::BufferUsages::INDIRECT
                | wgpu::BufferUsages::STORAGE
                | wgpu::BufferUsages::COPY_DST
                | wgpu::BufferUsages::COPY_SRC,
            mapped_at_creation: false,
        });
        let hiz_debug_readback_buffer = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("perro_hiz_indirect_readback"),
            size: (indirect_capacity * std::mem::size_of::<DrawIndexedIndirectGpu>()) as u64,
            usage: wgpu::BufferUsages::COPY_DST | wgpu::BufferUsages::MAP_READ,
            mapped_at_creation: false,
        });
        let frustum_cull_bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("perro_frustum_cull_bg"),
            layout: &frustum_cull_bgl,
            entries: &[
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: frustum_cull_params_buffer.as_entire_binding(),
                },
                wgpu::BindGroupEntry {
                    binding: 1,
                    resource: frustum_cull_items_buffer.as_entire_binding(),
                },
                wgpu::BindGroupEntry {
                    binding: 2,
                    resource: indirect_buffer.as_entire_binding(),
                },
            ],
        });

        let (depth_texture, depth_view) = create_depth_texture(device, width, height, sample_count);
        let (depth_prepass_texture, depth_prepass_view) =
            create_depth_prepass_texture(device, width, height);
        let (hiz_texture, hiz_mip_views, hiz_sample_view, hiz_mip_count, hiz_size) =
            create_hiz_texture(device, width, height);

        let hiz_copy_bgl = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("perro_hiz_copy_bgl"),
            entries: &[
                wgpu::BindGroupLayoutEntry {
                    binding: 0,
                    visibility: wgpu::ShaderStages::COMPUTE,
                    ty: wgpu::BindingType::Texture {
                        sample_type: wgpu::TextureSampleType::Depth,
                        view_dimension: wgpu::TextureViewDimension::D2,
                        multisampled: false,
                    },
                    count: None,
                },
                wgpu::BindGroupLayoutEntry {
                    binding: 1,
                    visibility: wgpu::ShaderStages::COMPUTE,
                    ty: wgpu::BindingType::StorageTexture {
                        access: wgpu::StorageTextureAccess::WriteOnly,
                        format: wgpu::TextureFormat::R32Float,
                        view_dimension: wgpu::TextureViewDimension::D2,
                    },
                    count: None,
                },
            ],
        });
        let hiz_downsample_bgl =
            device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
                label: Some("perro_hiz_downsample_bgl"),
                entries: &[
                    wgpu::BindGroupLayoutEntry {
                        binding: 0,
                        visibility: wgpu::ShaderStages::COMPUTE,
                        ty: wgpu::BindingType::Texture {
                            sample_type: wgpu::TextureSampleType::Float { filterable: false },
                            view_dimension: wgpu::TextureViewDimension::D2,
                            multisampled: false,
                        },
                        count: None,
                    },
                    wgpu::BindGroupLayoutEntry {
                        binding: 1,
                        visibility: wgpu::ShaderStages::COMPUTE,
                        ty: wgpu::BindingType::StorageTexture {
                            access: wgpu::StorageTextureAccess::WriteOnly,
                            format: wgpu::TextureFormat::R32Float,
                            view_dimension: wgpu::TextureViewDimension::D2,
                        },
                        count: None,
                    },
                ],
            });
        let hiz_cull_bgl = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("perro_hiz_cull_bgl"),
            entries: &[
                wgpu::BindGroupLayoutEntry {
                    binding: 0,
                    visibility: wgpu::ShaderStages::COMPUTE,
                    ty: wgpu::BindingType::Buffer {
                        ty: wgpu::BufferBindingType::Uniform,
                        has_dynamic_offset: false,
                        min_binding_size: None,
                    },
                    count: None,
                },
                wgpu::BindGroupLayoutEntry {
                    binding: 1,
                    visibility: wgpu::ShaderStages::COMPUTE,
                    ty: wgpu::BindingType::Buffer {
                        ty: wgpu::BufferBindingType::Storage { read_only: true },
                        has_dynamic_offset: false,
                        min_binding_size: None,
                    },
                    count: None,
                },
                wgpu::BindGroupLayoutEntry {
                    binding: 2,
                    visibility: wgpu::ShaderStages::COMPUTE,
                    ty: wgpu::BindingType::Buffer {
                        ty: wgpu::BufferBindingType::Storage { read_only: false },
                        has_dynamic_offset: false,
                        min_binding_size: None,
                    },
                    count: None,
                },
                wgpu::BindGroupLayoutEntry {
                    binding: 3,
                    visibility: wgpu::ShaderStages::COMPUTE,
                    ty: wgpu::BindingType::Texture {
                        sample_type: wgpu::TextureSampleType::Float { filterable: false },
                        view_dimension: wgpu::TextureViewDimension::D2,
                        multisampled: false,
                    },
                    count: None,
                },
            ],
        });

        let hiz_copy_pipeline = device.create_compute_pipeline(&wgpu::ComputePipelineDescriptor {
            label: Some("perro_hiz_copy_pipeline"),
            layout: Some(
                &device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
                    label: Some("perro_hiz_copy_layout"),
                    bind_group_layouts: &[&hiz_copy_bgl],
                    immediate_size: 0,
                }),
            ),
            module: &create_hiz_depth_copy_shader_module(device),
            entry_point: Some("cs_main"),
            compilation_options: Default::default(),
            cache: None,
        });
        let hiz_downsample_pipeline =
            device.create_compute_pipeline(&wgpu::ComputePipelineDescriptor {
                label: Some("perro_hiz_downsample_pipeline"),
                layout: Some(
                    &device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
                        label: Some("perro_hiz_downsample_layout"),
                        bind_group_layouts: &[&hiz_downsample_bgl],
                        immediate_size: 0,
                    }),
                ),
                module: &create_hiz_downsample_shader_module(device),
                entry_point: Some("cs_main"),
                compilation_options: Default::default(),
                cache: None,
            });
        let hiz_cull_pipeline = device.create_compute_pipeline(&wgpu::ComputePipelineDescriptor {
            label: Some("perro_hiz_cull_pipeline"),
            layout: Some(
                &device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
                    label: Some("perro_hiz_cull_layout"),
                    bind_group_layouts: &[&hiz_cull_bgl],
                    immediate_size: 0,
                }),
            ),
            module: &create_hiz_occlusion_cull_shader_module(device),
            entry_point: Some("cs_main"),
            compilation_options: Default::default(),
            cache: None,
        });

        let hiz_cull_params = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("perro_hiz_cull_params"),
            size: std::mem::size_of::<HizCullParamsGpu>() as u64,
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });
        let hiz_cull_bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("perro_hiz_cull_bg"),
            layout: &hiz_cull_bgl,
            entries: &[
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: hiz_cull_params.as_entire_binding(),
                },
                wgpu::BindGroupEntry {
                    binding: 1,
                    resource: frustum_cull_items_buffer.as_entire_binding(),
                },
                wgpu::BindGroupEntry {
                    binding: 2,
                    resource: indirect_buffer.as_entire_binding(),
                },
                wgpu::BindGroupEntry {
                    binding: 3,
                    resource: wgpu::BindingResource::TextureView(&hiz_sample_view),
                },
            ],
        });

        Self {
            camera_bgl,
            pipeline_culled,
            pipeline_double_sided,
            pipeline_depth_prepass_culled,
            pipeline_depth_prepass_double_sided,
            camera_buffer,
            camera_bind_group,
            instance_buffer,
            instance_capacity,
            staged_instances: Vec::new(),
            frustum_cull_enabled,
            frustum_cull_supported: frustum_cull_enabled,
            frustum_cull_pipeline,
            frustum_cull_bgl,
            frustum_cull_bind_group,
            frustum_cull_params_buffer,
            frustum_cull_items_buffer,
            frustum_cull_items_capacity,
            frustum_cull_staging: Vec::new(),
            indirect_buffer,
            indirect_capacity,
            indirect_staging: Vec::new(),
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
            builtin_mesh_bounds,
            builtin_meshlets,
            custom_mesh_ranges: HashMap::new(),
            depth_texture,
            depth_view,
            depth_prepass_texture,
            depth_prepass_view,
            depth_size: (width.max(1), height.max(1)),
            gpu_occlusion_enabled,
            hiz_texture,
            hiz_mip_views,
            hiz_sample_view,
            hiz_size,
            hiz_mip_count,
            hiz_copy_pipeline,
            hiz_downsample_pipeline,
            hiz_cull_pipeline,
            hiz_copy_bgl,
            hiz_downsample_bgl,
            hiz_cull_bgl,
            hiz_cull_params,
            hiz_cull_bind_group,
            hiz_debug_readback_buffer,
            pending_hiz_debug_count: 0,
            pending_hiz_debug_frustum_visible_est: 0,
            pending_hiz_debug_map_rx: None,
            debug_frustum_visible_est: 0,
            last_aspect: (width.max(1) as f32) / (height.max(1) as f32),
            last_proj_y_scale: projection_y_scale_from_projection(CameraProjectionState::default()),
            sample_count,
            occlusion_mode: occlusion_culling,
            meshlets_enabled,
            dev_meshlets,
            meshlet_debug_view,
            cpu_occlusion_enabled,
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
        let (depth_prepass_texture, depth_prepass_view) =
            create_depth_prepass_texture(device, width, height);
        self.depth_prepass_texture = depth_prepass_texture;
        self.depth_prepass_view = depth_prepass_view;
        self.depth_size = (width, height);
        let (hiz_texture, hiz_mip_views, hiz_sample_view, hiz_mip_count, hiz_size) =
            create_hiz_texture(device, width, height);
        self.hiz_texture = hiz_texture;
        self.hiz_mip_views = hiz_mip_views;
        self.hiz_sample_view = hiz_sample_view;
        self.hiz_mip_count = hiz_mip_count;
        self.hiz_size = hiz_size;
        self.hiz_cull_bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("perro_hiz_cull_bg"),
            layout: &self.hiz_cull_bgl,
            entries: &[
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: self.hiz_cull_params.as_entire_binding(),
                },
                wgpu::BindGroupEntry {
                    binding: 1,
                    resource: self.frustum_cull_items_buffer.as_entire_binding(),
                },
                wgpu::BindGroupEntry {
                    binding: 2,
                    resource: self.indirect_buffer.as_entire_binding(),
                },
                wgpu::BindGroupEntry {
                    binding: 3,
                    resource: wgpu::BindingResource::TextureView(&self.hiz_sample_view),
                },
            ],
        });
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
        let depth_prepass_shader = create_depth_prepass_shader_module(device);
        self.pipeline_depth_prepass_culled = create_depth_prepass_pipeline(
            device,
            &pipeline_layout,
            &depth_prepass_shader,
            Some(wgpu::Face::Back),
        );
        self.pipeline_depth_prepass_double_sided =
            create_depth_prepass_pipeline(device, &pipeline_layout, &depth_prepass_shader, None);
        let (depth_texture, depth_view) = create_depth_texture(device, width, height, sample_count);
        self.depth_texture = depth_texture;
        self.depth_view = depth_view;
        let (depth_prepass_texture, depth_prepass_view) =
            create_depth_prepass_texture(device, width, height);
        self.depth_prepass_texture = depth_prepass_texture;
        self.depth_prepass_view = depth_prepass_view;
        self.depth_size = (width.max(1), height.max(1));
        let (hiz_texture, hiz_mip_views, hiz_sample_view, hiz_mip_count, hiz_size) =
            create_hiz_texture(device, width, height);
        self.hiz_texture = hiz_texture;
        self.hiz_mip_views = hiz_mip_views;
        self.hiz_sample_view = hiz_sample_view;
        self.hiz_mip_count = hiz_mip_count;
        self.hiz_size = hiz_size;
        self.sample_count = sample_count;
        let (gpu_occlusion_enabled, cpu_occlusion_enabled) = occlusion_flags(self.occlusion_mode);
        self.gpu_occlusion_enabled = gpu_occlusion_enabled;
        self.cpu_occlusion_enabled = cpu_occlusion_enabled;
        self.hiz_cull_bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("perro_hiz_cull_bg"),
            layout: &self.hiz_cull_bgl,
            entries: &[
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: self.hiz_cull_params.as_entire_binding(),
                },
                wgpu::BindGroupEntry {
                    binding: 1,
                    resource: self.frustum_cull_items_buffer.as_entire_binding(),
                },
                wgpu::BindGroupEntry {
                    binding: 2,
                    resource: self.indirect_buffer.as_entire_binding(),
                },
                wgpu::BindGroupEntry {
                    binding: 3,
                    resource: wgpu::BindingResource::TextureView(&self.hiz_sample_view),
                },
            ],
        });
    }

    pub fn prepare(&mut self, device: &wgpu::Device, queue: &wgpu::Queue, frame: Prepare3D<'_>) {
        if self.gpu_occlusion_enabled {
            let _ = device.poll(wgpu::PollType::Poll);
            if self.pending_hiz_debug_count > 0 && self.pending_hiz_debug_map_rx.is_none() {
                self.request_hiz_debug_map_async();
            }
            self.consume_hiz_debug_results();
        }
        if self.cpu_occlusion_enabled {
            let _ = device.poll(wgpu::PollType::Poll);
            if self.pending_occlusion_query_count > 0 && self.pending_occlusion_map_rx.is_none() {
                self.request_occlusion_map_async();
            }
            self.consume_occlusion_results();
            self.occlusion_frame = self.occlusion_frame.wrapping_add(1);
        }
        self.occlusion_query_keys_this_frame.clear();
        let occlusion_capture_this_frame = self.cpu_occlusion_enabled
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
        self.frustum_cull_enabled = self.frustum_cull_supported;
        let (gpu_occlusion_enabled, cpu_occlusion_enabled) = occlusion_flags(self.occlusion_mode);
        self.gpu_occlusion_enabled = gpu_occlusion_enabled && self.frustum_cull_enabled;
        self.cpu_occlusion_enabled = cpu_occlusion_enabled;

        let uniform = build_scene_uniform(camera, lighting, width, height);
        let scene_changed = self.last_scene != Some(uniform) || self.last_draws.as_slice() != draws;
        if self.last_scene != Some(uniform) {
            queue.write_buffer(&self.camera_buffer, 0, bytemuck::bytes_of(&uniform));
            self.last_scene = Some(uniform);
        }
        if self.cpu_occlusion_enabled && scene_changed {
            // Retained-mode correctness: when camera/transforms/resources update,
            // previous query visibility is stale and must not gate current frame.
            self.occlusion_state.clear();
        }
        let view_proj = compute_view_proj_mat(camera, width, height);
        self.last_aspect = (width.max(1) as f32) / (height.max(1) as f32);
        self.last_proj_y_scale = projection_y_scale_from_projection(camera.projection);

        self.last_draws.clear();
        self.last_draws.extend_from_slice(draws);

        self.staged_instances.clear();
        self.staged_instances.reserve(draws.len());
        self.draw_batches.clear();
        self.draw_batches.reserve(draws.len());
        self.frustum_cull_staging.clear();
        self.indirect_staging.clear();
        let mut total_meshlets = 0usize;
        let frustum = extract_frustum_planes(view_proj);
        let default_mesh = self
            .resolve_builtin_mesh_asset("__cube__")
            .expect("cube mesh preset must exist");
        let mut debug_points_start: Option<u32> = None;
        let mut debug_points_count: u32 = 0;
        let mut debug_points_double_sided = false;
        let mut debug_points_local_center = [0.0f32; 3];
        let mut debug_points_local_radius = 0.0f32;
        let mut debug_point_instances: Vec<InstanceGpu> = Vec::new();
        let mut debug_edges_start: Option<u32> = None;
        let mut debug_edges_count: u32 = 0;
        let mut debug_edges_double_sided = false;
        let mut debug_edges_local_center = [0.0f32; 3];
        let mut debug_edges_local_radius = 0.0f32;
        let mut debug_edge_instances: Vec<InstanceGpu> = Vec::new();

        for draw in draws {
            let is_debug_point = matches!(draw.kind, Draw3DKind::DebugPointCube);
            let is_debug_edge = matches!(draw.kind, Draw3DKind::DebugEdgeCylinder);
            let (mesh_asset, is_terrain_mesh) = match draw.kind {
                Draw3DKind::Mesh(mesh) => {
                    let source = resources.mesh_source(mesh).unwrap_or("__cube__");
                    let is_terrain = source.starts_with("__terrain");
                    let asset = self
                        .resolve_mesh_range(device, queue, resources, source, static_mesh_lookup)
                        .unwrap_or_else(|| default_mesh.clone());
                    (asset, is_terrain)
                }
                Draw3DKind::Terrain64 => (
                    self.resolve_builtin_mesh_asset("__terrain64__")
                        .unwrap_or_else(|| default_mesh.clone()),
                    true,
                ),
                Draw3DKind::DebugPointCube => (
                    self.resolve_builtin_mesh_asset("__cube__")
                        .unwrap_or_else(|| default_mesh.clone()),
                    false,
                ),
                Draw3DKind::DebugEdgeCylinder => (
                    self.resolve_builtin_mesh_asset("__cylinder__")
                        .unwrap_or_else(|| default_mesh.clone()),
                    false,
                ),
            };
            let material = match draw.kind {
                Draw3DKind::Terrain64 => Material3D {
                    base_color_factor: [0.32, 0.56, 0.29, 1.0],
                    roughness_factor: 0.92,
                    metallic_factor: 0.0,
                    ..Material3D::default()
                },
                Draw3DKind::DebugPointCube => Material3D {
                    base_color_factor: [1.0, 0.92, 0.2, 1.0],
                    roughness_factor: 0.35,
                    metallic_factor: 0.0,
                    emissive_factor: [0.35, 0.3, 0.06],
                    ..Material3D::default()
                },
                Draw3DKind::DebugEdgeCylinder => Material3D {
                    base_color_factor: [0.15, 0.95, 0.95, 1.0],
                    roughness_factor: 0.6,
                    metallic_factor: 0.0,
                    emissive_factor: [0.06, 0.3, 0.3],
                    ..Material3D::default()
                },
                Draw3DKind::Mesh(_) => draw
                    .material
                    .and_then(|id| resources.material(id))
                    .unwrap_or_default(),
            };
            // CPU occlusion query mode works at object granularity.
            // Force whole-mesh batching in that mode so each object can be queried.
            let use_meshlets = !is_debug_point
                && !is_debug_edge
                && self.meshlets_enabled
                && !mesh_asset.meshlets.is_empty()
                && !self.cpu_occlusion_enabled;
            total_meshlets = total_meshlets.saturating_add(if use_meshlets {
                mesh_asset.meshlets.len()
            } else {
                1
            });

            // CPU fallback frustum culling should use mesh bounds, not object center.
            // Center-only tests pop large meshes when their origin exits the screen.
            if !self.frustum_cull_enabled
                && !use_meshlets
                && !bounds_in_frustum(
                    draw.model,
                    mesh_asset.bounds_center,
                    mesh_asset.bounds_radius,
                    &frustum,
                )
            {
                continue;
            }

            if !use_meshlets {
                let occlusion_key = draw.node.as_u64();
                if self.cpu_occlusion_enabled && !self.should_probe_or_draw(occlusion_key) {
                    continue;
                }
                let occlusion_query =
                    if (is_debug_point || is_debug_edge) && self.cpu_occlusion_enabled {
                        // Debug primitives are batched into shared instanced draws, so per-object CPU
                        // occlusion queries are not meaningful for these draws.
                        None
                    } else if occlusion_capture_this_frame {
                        Some(self.push_occlusion_query_key(occlusion_key))
                    } else {
                        None
                    };
                let built_instance = build_instance(
                    draw.model,
                    &material,
                    self.meshlet_debug_view,
                    debug_color(draw.node.as_u64()),
                );
                if is_debug_point {
                    if debug_point_instances.is_empty() {
                        debug_points_double_sided =
                            material.double_sided || self.meshlet_debug_view;
                        debug_points_local_center = mesh_asset.bounds_center;
                        debug_points_local_radius = mesh_asset.bounds_radius;
                    }
                    debug_point_instances.push(built_instance);
                    debug_points_count = debug_points_count.saturating_add(1);
                } else if is_debug_edge {
                    if debug_edge_instances.is_empty() {
                        debug_edges_double_sided = material.double_sided || self.meshlet_debug_view;
                        debug_edges_local_center = mesh_asset.bounds_center;
                        debug_edges_local_radius = mesh_asset.bounds_radius;
                    }
                    debug_edge_instances.push(built_instance);
                    debug_edges_count = debug_edges_count.saturating_add(1);
                } else {
                    let instance = self.staged_instances.len() as u32;
                    self.staged_instances.push(built_instance);
                    push_draw_batch(
                        &mut self.draw_batches,
                        mesh_asset.full,
                        instance,
                        material.double_sided || self.meshlet_debug_view,
                        mesh_asset.bounds_center,
                        mesh_asset.bounds_radius,
                        occlusion_query,
                        is_terrain_mesh,
                    );
                }
            } else {
                for meshlet in mesh_asset.meshlets.iter().copied() {
                    if !self.frustum_cull_enabled
                        && !meshlet_in_frustum(draw.model, meshlet, &frustum)
                    {
                        continue;
                    }
                    // CPU query occlusion at meshlet granularity self-occludes dynamic meshes.
                    // Keep meshlet occlusion GPU-driven only; CPU mode skips meshlet occlusion.
                    let occlusion_query = None;
                    let instance = self.staged_instances.len() as u32;
                    self.staged_instances.push(build_instance(
                        draw.model,
                        &material,
                        self.meshlet_debug_view,
                        debug_color((draw.node.as_u64() << 32) ^ meshlet.index_start as u64),
                    ));
                    // Conservative retained-mode behavior: keep meshlet batches gated by
                    // whole-object bounds so visible objects do not lose arbitrary meshlets.
                    let occlusion_center = mesh_asset.bounds_center;
                    let occlusion_radius = mesh_asset.bounds_radius;
                    push_draw_batch(
                        &mut self.draw_batches,
                        MeshRange {
                            index_start: meshlet.index_start,
                            index_count: meshlet.index_count,
                            base_vertex: mesh_asset.full.base_vertex,
                        },
                        instance,
                        material.double_sided || self.meshlet_debug_view,
                        occlusion_center,
                        occlusion_radius,
                        occlusion_query,
                        is_terrain_mesh,
                    );
                }
            }
        }
        if !debug_point_instances.is_empty() {
            debug_points_start = Some(self.staged_instances.len() as u32);
            self.staged_instances.extend(debug_point_instances);
        }
        if !debug_edge_instances.is_empty() {
            debug_edges_start = Some(self.staged_instances.len() as u32);
            self.staged_instances.extend(debug_edge_instances);
        }
        if let Some(instance_start) = debug_points_start
            && debug_points_count > 0
        {
            self.draw_batches.push(DrawBatch {
                mesh: default_mesh.full,
                instance_start,
                instance_count: debug_points_count,
                double_sided: debug_points_double_sided,
                local_center: debug_points_local_center,
                local_radius: debug_points_local_radius.max(0.0),
                occlusion_query: None,
                disable_hiz_occlusion: false,
            });
        }
        if let Some(instance_start) = debug_edges_start
            && debug_edges_count > 0
        {
            let debug_edge_mesh = self
                .resolve_builtin_mesh_asset("__cylinder__")
                .unwrap_or_else(|| default_mesh.clone());
            self.draw_batches.push(DrawBatch {
                mesh: debug_edge_mesh.full,
                instance_start,
                instance_count: debug_edges_count,
                double_sided: debug_edges_double_sided,
                local_center: debug_edges_local_center,
                local_radius: debug_edges_local_radius.max(0.0),
                occlusion_query: None,
                disable_hiz_occlusion: false,
            });
        }
        self.debug_frustum_visible_est = 0;
        for batch in &self.draw_batches {
            let model = [
                self.staged_instances[batch.instance_start as usize].model_0,
                self.staged_instances[batch.instance_start as usize].model_1,
                self.staged_instances[batch.instance_start as usize].model_2,
                self.staged_instances[batch.instance_start as usize].model_3,
            ];
            if bounds_in_frustum(model, batch.local_center, batch.local_radius, &frustum) {
                self.debug_frustum_visible_est = self.debug_frustum_visible_est.saturating_add(1);
            }
        }
        if occlusion_capture_this_frame {
            self.ensure_occlusion_query_capacity(
                device,
                self.occlusion_query_keys_this_frame.len() as u32,
            );
        }
        self.ensure_instance_capacity(device, self.staged_instances.len());
        if !self.staged_instances.is_empty() {
            queue.write_buffer(
                &self.instance_buffer,
                0,
                bytemuck::cast_slice(&self.staged_instances),
            );
        }
        self.ensure_frustum_cull_capacity(device, self.draw_batches.len());
        if self.frustum_cull_enabled && !self.draw_batches.is_empty() {
            self.indirect_staging
                .reserve(self.draw_batches.len() - self.indirect_staging.len());
            self.frustum_cull_staging
                .reserve(self.draw_batches.len() - self.frustum_cull_staging.len());
            for batch in &self.draw_batches {
                self.indirect_staging.push(DrawIndexedIndirectGpu {
                    index_count: batch.mesh.index_count,
                    instance_count: batch.instance_count,
                    first_index: batch.mesh.index_start,
                    base_vertex: batch.mesh.base_vertex,
                    first_instance: batch.instance_start,
                });
                self.frustum_cull_staging.push(FrustumCullItemGpu {
                    model_0: self.staged_instances[batch.instance_start as usize].model_0,
                    model_1: self.staged_instances[batch.instance_start as usize].model_1,
                    model_2: self.staged_instances[batch.instance_start as usize].model_2,
                    model_3: self.staged_instances[batch.instance_start as usize].model_3,
                    local_center_radius: [
                        batch.local_center[0],
                        batch.local_center[1],
                        batch.local_center[2],
                        batch.local_radius.max(0.0),
                    ],
                    cull_flags: [
                        if batch.disable_hiz_occlusion {
                            CULL_FLAG_DISABLE_HIZ_OCCLUSION
                        } else {
                            0
                        },
                        0,
                        0,
                        0,
                    ],
                });
            }
            let mut planes = [[0.0f32; 4]; 6];
            for (dst, plane) in planes.iter_mut().zip(frustum.iter()) {
                *dst = [plane.x, plane.y, plane.z, plane.w];
            }
            let cull_params = FrustumCullParamsGpu {
                planes,
                draw_count: self.draw_batches.len() as u32,
                _pad: [0; 3],
            };
            queue.write_buffer(
                &self.frustum_cull_params_buffer,
                0,
                bytemuck::bytes_of(&cull_params),
            );
            queue.write_buffer(
                &self.frustum_cull_items_buffer,
                0,
                bytemuck::cast_slice(&self.frustum_cull_staging),
            );
            queue.write_buffer(
                &self.indirect_buffer,
                0,
                bytemuck::cast_slice(&self.indirect_staging),
            );
            if self.gpu_occlusion_enabled {
                let hiz_params = HizCullParamsGpu {
                    view_proj: uniform.view_proj,
                    draw_count: self.draw_batches.len() as u32,
                    hiz_mip_count: self.hiz_mip_count,
                    hiz_width: self.hiz_size.0,
                    hiz_height: self.hiz_size.1,
                    aspect: self.last_aspect,
                    proj_y_scale: self.last_proj_y_scale,
                    depth_bias: HIZ_OCCLUSION_BIAS,
                    _pad: 0,
                };
                queue.write_buffer(&self.hiz_cull_params, 0, bytemuck::bytes_of(&hiz_params));
            }
        }
        self.last_total_meshlets = total_meshlets;
        self.last_total_drawn = self.staged_instances.len();
    }

    pub fn render_pass(
        &mut self,
        device: &wgpu::Device,
        encoder: &mut wgpu::CommandEncoder,
        color_view: &wgpu::TextureView,
        clear_color: wgpu::Color,
    ) {
        let hiz_active = self.gpu_occlusion_enabled && !self.draw_batches.is_empty();
        let query_count = if self.cpu_occlusion_enabled
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
        if self.frustum_cull_enabled && !self.draw_batches.is_empty() {
            let mut cull_pass = encoder.begin_compute_pass(&wgpu::ComputePassDescriptor {
                label: Some("perro_frustum_cull_pass"),
                timestamp_writes: None,
            });
            cull_pass.set_pipeline(&self.frustum_cull_pipeline);
            cull_pass.set_bind_group(0, &self.frustum_cull_bind_group, &[]);
            let groups = (self.draw_batches.len() as u32).div_ceil(FRUSTUM_CULL_WORKGROUP_SIZE);
            cull_pass.dispatch_workgroups(groups, 1, 1);
        }
        if hiz_active {
            let mut prepass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("perro_depth_prepass"),
                color_attachments: &[],
                depth_stencil_attachment: Some(wgpu::RenderPassDepthStencilAttachment {
                    view: &self.depth_prepass_view,
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
            prepass.set_bind_group(0, &self.camera_bind_group, &[]);
            prepass.set_vertex_buffer(0, self.vertex_buffer.slice(..));
            prepass.set_vertex_buffer(1, self.instance_buffer.slice(..));
            prepass.set_index_buffer(self.index_buffer.slice(..), wgpu::IndexFormat::Uint32);
            let mut current_double_sided = None;
            for (i, batch) in self.draw_batches.iter().enumerate() {
                if current_double_sided != Some(batch.double_sided) {
                    let pipeline = if batch.double_sided {
                        &self.pipeline_depth_prepass_double_sided
                    } else {
                        &self.pipeline_depth_prepass_culled
                    };
                    prepass.set_pipeline(pipeline);
                    current_double_sided = Some(batch.double_sided);
                }
                let offset = (i * std::mem::size_of::<DrawIndexedIndirectGpu>()) as u64;
                prepass.draw_indexed_indirect(&self.indirect_buffer, offset);
            }
            drop(prepass);

            self.build_hiz_from_depth(device, encoder);

            let mut cull_pass = encoder.begin_compute_pass(&wgpu::ComputePassDescriptor {
                label: Some("perro_hiz_occlusion_cull_pass"),
                timestamp_writes: None,
            });
            cull_pass.set_pipeline(&self.hiz_cull_pipeline);
            cull_pass.set_bind_group(0, &self.hiz_cull_bind_group, &[]);
            let groups = (self.draw_batches.len() as u32).div_ceil(FRUSTUM_CULL_WORKGROUP_SIZE);
            cull_pass.dispatch_workgroups(groups, 1, 1);
            drop(cull_pass);

            if self.pending_hiz_debug_count == 0 && self.pending_hiz_debug_map_rx.is_none() {
                let count = self.draw_batches.len() as u32;
                if count > 0 {
                    let byte_len =
                        u64::from(count) * std::mem::size_of::<DrawIndexedIndirectGpu>() as u64;
                    encoder.copy_buffer_to_buffer(
                        &self.indirect_buffer,
                        0,
                        &self.hiz_debug_readback_buffer,
                        0,
                        byte_len,
                    );
                    self.pending_hiz_debug_count = count;
                    self.pending_hiz_debug_frustum_visible_est = self.debug_frustum_visible_est;
                }
            }
        }
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
        for (i, batch) in self.draw_batches.iter().enumerate() {
            if current_double_sided != Some(batch.double_sided) {
                let pipeline = if batch.double_sided {
                    &self.pipeline_double_sided
                } else {
                    &self.pipeline_culled
                };
                pass.set_pipeline(pipeline);
                current_double_sided = Some(batch.double_sided);
            }
            if let Some(query_index) = batch.occlusion_query {
                pass.begin_occlusion_query(query_index);
                if self.frustum_cull_enabled {
                    let offset = (i * std::mem::size_of::<DrawIndexedIndirectGpu>()) as u64;
                    pass.draw_indexed_indirect(&self.indirect_buffer, offset);
                } else {
                    let start = batch.mesh.index_start;
                    let end = start + batch.mesh.index_count;
                    let instances =
                        batch.instance_start..batch.instance_start + batch.instance_count;
                    pass.draw_indexed(start..end, batch.mesh.base_vertex, instances);
                }
                pass.end_occlusion_query();
            } else if self.frustum_cull_enabled {
                let offset = (i * std::mem::size_of::<DrawIndexedIndirectGpu>()) as u64;
                pass.draw_indexed_indirect(&self.indirect_buffer, offset);
            } else {
                let start = batch.mesh.index_start;
                let end = start + batch.mesh.index_count;
                let instances = batch.instance_start..batch.instance_start + batch.instance_count;
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

    pub fn depth_view(&self) -> &wgpu::TextureView {
        &self.depth_view
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

    fn ensure_frustum_cull_capacity(&mut self, device: &wgpu::Device, needed: usize) {
        if needed == 0 || needed <= self.frustum_cull_items_capacity {
            return;
        }
        let mut new_capacity = self.frustum_cull_items_capacity.max(1);
        while new_capacity < needed {
            new_capacity *= 2;
        }
        self.frustum_cull_items_buffer = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("perro_frustum_cull_items"),
            size: (new_capacity * std::mem::size_of::<FrustumCullItemGpu>()) as u64,
            usage: wgpu::BufferUsages::STORAGE | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });
        self.indirect_buffer = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("perro_draw_indirect"),
            size: (new_capacity * std::mem::size_of::<DrawIndexedIndirectGpu>()) as u64,
            usage: wgpu::BufferUsages::INDIRECT
                | wgpu::BufferUsages::STORAGE
                | wgpu::BufferUsages::COPY_DST
                | wgpu::BufferUsages::COPY_SRC,
            mapped_at_creation: false,
        });
        self.hiz_debug_readback_buffer = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("perro_hiz_indirect_readback"),
            size: (new_capacity * std::mem::size_of::<DrawIndexedIndirectGpu>()) as u64,
            usage: wgpu::BufferUsages::COPY_DST | wgpu::BufferUsages::MAP_READ,
            mapped_at_creation: false,
        });
        self.pending_hiz_debug_count = 0;
        self.pending_hiz_debug_map_rx = None;
        self.frustum_cull_bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("perro_frustum_cull_bg"),
            layout: &self.frustum_cull_bgl,
            entries: &[
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: self.frustum_cull_params_buffer.as_entire_binding(),
                },
                wgpu::BindGroupEntry {
                    binding: 1,
                    resource: self.frustum_cull_items_buffer.as_entire_binding(),
                },
                wgpu::BindGroupEntry {
                    binding: 2,
                    resource: self.indirect_buffer.as_entire_binding(),
                },
            ],
        });
        self.hiz_cull_bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("perro_hiz_cull_bg"),
            layout: &self.hiz_cull_bgl,
            entries: &[
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: self.hiz_cull_params.as_entire_binding(),
                },
                wgpu::BindGroupEntry {
                    binding: 1,
                    resource: self.frustum_cull_items_buffer.as_entire_binding(),
                },
                wgpu::BindGroupEntry {
                    binding: 2,
                    resource: self.indirect_buffer.as_entire_binding(),
                },
                wgpu::BindGroupEntry {
                    binding: 3,
                    resource: wgpu::BindingResource::TextureView(&self.hiz_sample_view),
                },
            ],
        });
        self.frustum_cull_items_capacity = new_capacity;
        self.indirect_capacity = new_capacity;
    }

    fn build_hiz_from_depth(&self, device: &wgpu::Device, encoder: &mut wgpu::CommandEncoder) {
        if self.hiz_mip_views.is_empty() {
            return;
        }
        let copy_bg = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("perro_hiz_copy_bg"),
            layout: &self.hiz_copy_bgl,
            entries: &[
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: wgpu::BindingResource::TextureView(&self.depth_prepass_view),
                },
                wgpu::BindGroupEntry {
                    binding: 1,
                    resource: wgpu::BindingResource::TextureView(&self.hiz_mip_views[0]),
                },
            ],
        });
        {
            let mut pass = encoder.begin_compute_pass(&wgpu::ComputePassDescriptor {
                label: Some("perro_hiz_copy_pass"),
                timestamp_writes: None,
            });
            pass.set_pipeline(&self.hiz_copy_pipeline);
            pass.set_bind_group(0, &copy_bg, &[]);
            let groups_x = self.hiz_size.0.div_ceil(HIZ_WORKGROUP_SIZE_X);
            let groups_y = self.hiz_size.1.div_ceil(HIZ_WORKGROUP_SIZE_Y);
            pass.dispatch_workgroups(groups_x, groups_y, 1);
        }
        let mut src_w = self.hiz_size.0;
        let mut src_h = self.hiz_size.1;
        for mip in 1..self.hiz_mip_count as usize {
            let downsample_bg = device.create_bind_group(&wgpu::BindGroupDescriptor {
                label: Some("perro_hiz_downsample_bg"),
                layout: &self.hiz_downsample_bgl,
                entries: &[
                    wgpu::BindGroupEntry {
                        binding: 0,
                        resource: wgpu::BindingResource::TextureView(&self.hiz_mip_views[mip - 1]),
                    },
                    wgpu::BindGroupEntry {
                        binding: 1,
                        resource: wgpu::BindingResource::TextureView(&self.hiz_mip_views[mip]),
                    },
                ],
            });
            let dst_w = (src_w / 2).max(1);
            let dst_h = (src_h / 2).max(1);
            let mut pass = encoder.begin_compute_pass(&wgpu::ComputePassDescriptor {
                label: Some("perro_hiz_downsample_pass"),
                timestamp_writes: None,
            });
            pass.set_pipeline(&self.hiz_downsample_pipeline);
            pass.set_bind_group(0, &downsample_bg, &[]);
            pass.dispatch_workgroups(
                dst_w.div_ceil(HIZ_WORKGROUP_SIZE_X),
                dst_h.div_ceil(HIZ_WORKGROUP_SIZE_Y),
                1,
            );
            src_w = dst_w;
            src_h = dst_h;
        }
    }

    fn request_hiz_debug_map_async(&mut self) {
        if self.pending_hiz_debug_count == 0 || self.pending_hiz_debug_map_rx.is_some() {
            return;
        }
        let byte_len = u64::from(self.pending_hiz_debug_count)
            * std::mem::size_of::<DrawIndexedIndirectGpu>() as u64;
        let (tx, rx) = mpsc::channel();
        self.hiz_debug_readback_buffer.slice(0..byte_len).map_async(
            wgpu::MapMode::Read,
            move |result| {
                let _ = tx.send(result);
            },
        );
        self.pending_hiz_debug_map_rx = Some(rx);
    }

    fn consume_hiz_debug_results(&mut self) {
        let count = self.pending_hiz_debug_count as usize;
        if count == 0 {
            return;
        }
        let Some(rx) = self.pending_hiz_debug_map_rx.as_ref() else {
            return;
        };
        match rx.try_recv() {
            Ok(Ok(())) => {
                let byte_len = (count * std::mem::size_of::<DrawIndexedIndirectGpu>()) as u64;
                let data = self
                    .hiz_debug_readback_buffer
                    .slice(0..byte_len)
                    .get_mapped_range();
                let mut visible = 0u32;
                for bytes in data.chunks_exact(std::mem::size_of::<DrawIndexedIndirectGpu>()) {
                    let cmd = bytemuck::from_bytes::<DrawIndexedIndirectGpu>(bytes);
                    if cmd.instance_count > 0 {
                        visible = visible.saturating_add(1);
                    }
                }
                drop(data);
                self.hiz_debug_readback_buffer.unmap();

                let _total_batches = self.pending_hiz_debug_count;
                let _frustum_visible_est = self.pending_hiz_debug_frustum_visible_est;
                let _visible = visible;
                self.pending_hiz_debug_count = 0;
                self.pending_hiz_debug_frustum_visible_est = 0;
                self.pending_hiz_debug_map_rx = None;
            }
            Ok(Err(_)) | Err(TryRecvError::Disconnected) => {
                self.hiz_debug_readback_buffer.unmap();
                self.pending_hiz_debug_count = 0;
                self.pending_hiz_debug_frustum_visible_est = 0;
                self.pending_hiz_debug_map_rx = None;
            }
            Err(TryRecvError::Empty) => {}
        }
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
        if !self.cpu_occlusion_enabled {
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
        if !self.cpu_occlusion_enabled {
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
        resources: &ResourceStore,
        source: &str,
        static_mesh_lookup: Option<StaticMeshLookup>,
    ) -> Option<MeshAssetRange> {
        if let Some(range) = self.builtin_mesh_ranges.get(source).copied() {
            let (bounds_center, bounds_radius) = self
                .builtin_mesh_bounds
                .get(source)
                .copied()
                .unwrap_or(([0.0, 0.0, 0.0], 1.0));
            return Some(MeshAssetRange {
                full: range,
                meshlets: self
                    .builtin_meshlets
                    .get(source)
                    .cloned()
                    .unwrap_or_else(|| Arc::from([])),
                bounds_center,
                bounds_radius,
            });
        }
        if let Some(range) = self.custom_mesh_ranges.get(source).cloned() {
            return Some(range);
        }
        let decoded = load_mesh_from_source(
            source,
            static_mesh_lookup,
            resources.runtime_mesh_data(source),
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
        let (bounds_center, bounds_radius) = self
            .builtin_mesh_bounds
            .get(source)
            .copied()
            .unwrap_or(([0.0, 0.0, 0.0], 1.0));
        Some(MeshAssetRange {
            full,
            meshlets,
            bounds_center,
            bounds_radius,
        })
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

        let (bounds_center, bounds_radius) = mesh_bounds_from_vertices(&decoded.vertices)?;
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
            bounds_center,
            bounds_radius,
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
    runtime_mesh: Option<&RuntimeMeshData>,
    dev_meshlets: bool,
) -> Option<DecodedMesh> {
    let mut decoded = if let Some(mesh) = runtime_mesh {
        decode_runtime_mesh(mesh)?
    } else if let Some(lookup) = static_mesh_lookup
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
        let (packed_indices, meshlets) = build_meshlets(&decoded.vertices, &decoded.indices);
        decoded.indices = packed_indices;
        decoded.meshlets = meshlets;
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
    if load_mesh_from_source(source, static_mesh_lookup, None, false).is_some() {
        return Ok(());
    }
    Err(format!("mesh source failed to decode: {}", source))
}

fn decode_runtime_mesh(mesh: &RuntimeMeshData) -> Option<DecodedMesh> {
    if mesh.vertices.is_empty() || mesh.indices.is_empty() {
        return None;
    }
    if !mesh.indices.len().is_multiple_of(3) {
        return None;
    }
    let vertices: Vec<MeshVertex> = mesh
        .vertices
        .iter()
        .map(|v| MeshVertex {
            pos: v.position,
            normal: v.normal,
        })
        .collect();
    if vertices
        .iter()
        .any(|v| !v.pos.iter().all(|c| c.is_finite()))
    {
        return None;
    }
    if vertices
        .iter()
        .any(|v| !v.normal.iter().all(|c| c.is_finite()))
    {
        return None;
    }
    if mesh
        .indices
        .iter()
        .any(|&idx| (idx as usize) >= vertices.len())
    {
        return None;
    }
    Some(DecodedMesh {
        vertices,
        indices: mesh.indices.clone(),
        meshlets: Vec::new(),
    })
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
    let mut vertices = Vec::new();
    let mut indices = Vec::new();

    for primitive in mesh.primitives() {
        let reader = primitive.reader(|buffer| buffers.get(buffer.index()).map(|b| b.0.as_slice()));
        let Some(positions_iter) = reader.read_positions() else {
            continue;
        };
        let positions: Vec<[f32; 3]> = positions_iter.collect();
        if positions.is_empty() {
            continue;
        }
        let normals: Vec<[f32; 3]> = reader
            .read_normals()
            .map(|iter| iter.collect())
            .unwrap_or_default();
        let base_vertex = vertices.len() as u32;
        for (i, position) in positions.iter().copied().enumerate() {
            vertices.push(MeshVertex {
                pos: position,
                normal: normals.get(i).copied().unwrap_or([0.0, 1.0, 0.0]),
            });
        }
        if let Some(idx) = reader.read_indices() {
            indices.extend(idx.into_u32().map(|i| i + base_vertex));
        } else {
            indices.extend((0..positions.len() as u32).map(|i| i + base_vertex));
        }
    }
    if vertices.is_empty() || indices.is_empty() {
        return None;
    }
    Some(DecodedMesh {
        vertices,
        indices,
        meshlets: Vec::new(),
    })
}

fn create_hiz_texture(
    device: &wgpu::Device,
    width: u32,
    height: u32,
) -> (
    wgpu::Texture,
    Vec<wgpu::TextureView>,
    wgpu::TextureView,
    u32,
    (u32, u32),
) {
    let width = width.max(1);
    let height = height.max(1);
    let max_dim = width.max(height);
    let mip_count = (u32::BITS - max_dim.leading_zeros()).max(1);
    let texture = device.create_texture(&wgpu::TextureDescriptor {
        label: Some("perro_hiz_texture"),
        size: wgpu::Extent3d {
            width,
            height,
            depth_or_array_layers: 1,
        },
        mip_level_count: mip_count,
        sample_count: 1,
        dimension: wgpu::TextureDimension::D2,
        format: wgpu::TextureFormat::R32Float,
        usage: wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::STORAGE_BINDING,
        view_formats: &[],
    });
    let mut mip_views = Vec::with_capacity(mip_count as usize);
    for mip in 0..mip_count {
        mip_views.push(texture.create_view(&wgpu::TextureViewDescriptor {
            label: Some("perro_hiz_mip_view"),
            format: Some(wgpu::TextureFormat::R32Float),
            dimension: Some(wgpu::TextureViewDimension::D2),
            usage: Some(
                wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::STORAGE_BINDING,
            ),
            aspect: wgpu::TextureAspect::All,
            base_mip_level: mip,
            mip_level_count: Some(1),
            base_array_layer: 0,
            array_layer_count: Some(1),
        }));
    }
    let sample_view = texture.create_view(&wgpu::TextureViewDescriptor {
        label: Some("perro_hiz_sample_view"),
        format: Some(wgpu::TextureFormat::R32Float),
        dimension: Some(wgpu::TextureViewDimension::D2),
        usage: Some(wgpu::TextureUsages::TEXTURE_BINDING),
        aspect: wgpu::TextureAspect::All,
        base_mip_level: 0,
        mip_level_count: Some(mip_count),
        base_array_layer: 0,
        array_layer_count: Some(1),
    });
    (texture, mip_views, sample_view, mip_count, (width, height))
}

fn compute_builtin_mesh_bounds(
    vertices: &[MeshVertex],
    indices: &[u32],
    ranges: &HashMap<&'static str, MeshRange>,
) -> HashMap<&'static str, ([f32; 3], f32)> {
    let mut out = HashMap::new();
    for (name, range) in ranges {
        let start = range.index_start as usize;
        let end = start
            .saturating_add(range.index_count as usize)
            .min(indices.len());
        let mut pts = Vec::with_capacity(end.saturating_sub(start));
        for idx in &indices[start..end] {
            let vertex_index = range.base_vertex as i64 + *idx as i64;
            if vertex_index < 0 {
                continue;
            }
            let Some(v) = vertices.get(vertex_index as usize) else {
                continue;
            };
            pts.push(v.pos);
        }
        if let Some((c, r)) = mesh_bounds_from_positions(&pts) {
            out.insert(*name, (c, r));
        }
    }
    out
}

fn mesh_bounds_from_vertices(vertices: &[MeshVertex]) -> Option<([f32; 3], f32)> {
    let positions: Vec<[f32; 3]> = vertices.iter().map(|v| v.pos).collect();
    mesh_bounds_from_positions(&positions)
}

fn mesh_bounds_from_positions(positions: &[[f32; 3]]) -> Option<([f32; 3], f32)> {
    let mut it = positions.iter().copied();
    let first = it.next()?;
    let mut min = Vec3::from(first);
    let mut max = Vec3::from(first);
    for p in it {
        let v = Vec3::from(p);
        min = min.min(v);
        max = max.max(v);
    }
    let center = (min + max) * 0.5;
    let mut radius = 0.0f32;
    for p in positions {
        let d = (Vec3::from(*p) - center).length();
        if d > radius {
            radius = d;
        }
    }
    Some(([center.x, center.y, center.z], radius))
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
        usage: wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::TEXTURE_BINDING,
        view_formats: &[],
    });
    let depth_view = depth_texture.create_view(&wgpu::TextureViewDescriptor::default());
    (depth_texture, depth_view)
}

fn create_depth_prepass_texture(
    device: &wgpu::Device,
    width: u32,
    height: u32,
) -> (wgpu::Texture, wgpu::TextureView) {
    let texture = device.create_texture(&wgpu::TextureDescriptor {
        label: Some("perro_depth_prepass"),
        size: wgpu::Extent3d {
            width: width.max(1),
            height: height.max(1),
            depth_or_array_layers: 1,
        },
        mip_level_count: 1,
        sample_count: 1,
        dimension: wgpu::TextureDimension::D2,
        format: DEPTH_PREPASS_FORMAT,
        usage: wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::TEXTURE_BINDING,
        view_formats: &[],
    });
    let view = texture.create_view(&wgpu::TextureViewDescriptor::default());
    (texture, view)
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

fn create_depth_prepass_pipeline(
    device: &wgpu::Device,
    pipeline_layout: &wgpu::PipelineLayout,
    shader: &wgpu::ShaderModule,
    cull_mode: Option<wgpu::Face>,
) -> wgpu::RenderPipeline {
    device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
        label: Some("perro_depth_prepass_pipeline"),
        layout: Some(pipeline_layout),
        vertex: wgpu::VertexState {
            module: shader,
            entry_point: Some("vs_main"),
            buffers: &[
                wgpu::VertexBufferLayout {
                    array_stride: std::mem::size_of::<MeshVertex>() as u64,
                    step_mode: wgpu::VertexStepMode::Vertex,
                    attributes: &[wgpu::VertexAttribute {
                        offset: 0,
                        shader_location: 0,
                        format: wgpu::VertexFormat::Float32x3,
                    }],
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
                    ],
                },
            ],
            compilation_options: Default::default(),
        },
        fragment: None,
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
            format: DEPTH_PREPASS_FORMAT,
            depth_write_enabled: true,
            depth_compare: wgpu::CompareFunction::LessEqual,
            stencil: wgpu::StencilState::default(),
            bias: wgpu::DepthBiasState::default(),
        }),
        multisample: wgpu::MultisampleState::default(),
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
    local_center: [f32; 3],
    local_radius: f32,
    occlusion_query: Option<u32>,
    disable_hiz_occlusion: bool,
) {
    draw_batches.push(DrawBatch {
        mesh,
        instance_start: instance,
        instance_count: 1,
        double_sided,
        local_center,
        local_radius: local_radius.max(0.0),
        occlusion_query,
        disable_hiz_occlusion,
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

    let proj = projection_matrix(camera.projection, aspect);

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

fn projection_matrix(projection: CameraProjectionState, aspect: f32) -> Mat4 {
    match projection {
        CameraProjectionState::Perspective {
            fov_y_degrees,
            near,
            far,
        } => {
            let fov_y_radians = if fov_y_degrees.is_finite() {
                fov_y_degrees
                    .to_radians()
                    .clamp(10.0f32.to_radians(), 120.0f32.to_radians())
            } else {
                60.0f32.to_radians()
            };
            let near = sanitize_near(near);
            let far = sanitize_far(far, near);
            Mat4::perspective_rh_gl(fov_y_radians, aspect.max(1.0e-6), near, far)
        }
        CameraProjectionState::Orthographic { size, near, far } => {
            let half_h = if size.is_finite() {
                (size.abs() * 0.5).max(1.0e-3)
            } else {
                5.0
            };
            let half_w = half_h * aspect.max(1.0e-6);
            let near = sanitize_near(near);
            let far = sanitize_far(far, near);
            Mat4::orthographic_rh(-half_w, half_w, -half_h, half_h, near, far)
        }
        CameraProjectionState::Frustum {
            left,
            right,
            bottom,
            top,
            near,
            far,
        } => {
            let near = sanitize_near(near);
            let far = sanitize_far(far, near);
            let (left, right) = sanitize_range(left, right, -1.0, 1.0);
            let (bottom, top) = sanitize_range(bottom, top, -1.0, 1.0);
            Mat4::frustum_rh_gl(left, right, bottom, top, near, far)
        }
    }
}

fn projection_y_scale_from_projection(projection: CameraProjectionState) -> f32 {
    match projection {
        CameraProjectionState::Perspective { fov_y_degrees, .. } => {
            let fov_y_radians = if fov_y_degrees.is_finite() {
                fov_y_degrees
                    .to_radians()
                    .clamp(10.0f32.to_radians(), 120.0f32.to_radians())
            } else {
                60.0f32.to_radians()
            };
            1.0 / (fov_y_radians * 0.5).tan().max(1.0e-6)
        }
        CameraProjectionState::Orthographic { size, .. } => {
            let half_h = if size.is_finite() {
                (size.abs() * 0.5).max(1.0e-3)
            } else {
                5.0
            };
            1.0 / half_h
        }
        CameraProjectionState::Frustum {
            bottom, top, near, ..
        } => {
            let near = sanitize_near(near);
            let (bottom, top) = sanitize_range(bottom, top, -1.0, 1.0);
            (2.0 * near / (top - bottom).abs().max(1.0e-6)).max(1.0e-6)
        }
    }
}

fn sanitize_near(near: f32) -> f32 {
    if near.is_finite() {
        near.max(1.0e-3)
    } else {
        0.1
    }
}

fn sanitize_far(far: f32, near: f32) -> f32 {
    if far.is_finite() {
        far.max(near + 1.0e-3)
    } else {
        (near + 1000.0).max(near + 1.0e-3)
    }
}

fn sanitize_range(min: f32, max: f32, fallback_min: f32, fallback_max: f32) -> (f32, f32) {
    let mut a = if min.is_finite() { min } else { fallback_min };
    let mut b = if max.is_finite() { max } else { fallback_max };
    if (b - a).abs() < 1.0e-6 {
        a = fallback_min;
        b = fallback_max;
    }
    if b < a {
        std::mem::swap(&mut a, &mut b);
    }
    (a, b)
}

// Returns (gpu_occlusion_enabled, cpu_occlusion_enabled).
fn occlusion_flags(mode: OcclusionCullingMode) -> (bool, bool) {
    match mode {
        OcclusionCullingMode::Cpu => (false, true),
        OcclusionCullingMode::Gpu => (true, false),
        OcclusionCullingMode::Off => (false, false),
    }
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
    bounds_in_frustum(model, meshlet.center, meshlet.radius, planes)
}

fn bounds_in_frustum(
    model: [[f32; 4]; 4],
    local_center: [f32; 3],
    local_radius: f32,
    planes: &[Vec4; 6],
) -> bool {
    let model = Mat4::from_cols_array_2d(&model);
    if !model.is_finite() {
        return false;
    }
    let center_local = Vec4::new(local_center[0], local_center[1], local_center[2], 1.0);
    let center_world = model * center_local;
    if !center_world.is_finite() {
        return false;
    }
    let sx = Vec3::new(model.x_axis.x, model.x_axis.y, model.x_axis.z).length();
    let sy = Vec3::new(model.y_axis.x, model.y_axis.y, model.y_axis.z).length();
    let sz = Vec3::new(model.z_axis.x, model.z_axis.y, model.z_axis.z).length();
    let scale = sx.max(sy).max(sz).max(1.0e-6);
    let radius_world = local_radius.max(0.0) * scale;
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
