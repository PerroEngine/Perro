//! 3D GPU renderer state, asset decode, batching, culling, and draw submission.

use super::{
    renderer::{
        Draw3DInstance, Draw3DKind, Lighting3DState, MAX_POINT_LIGHTS, MAX_RAY_LIGHTS,
        MAX_SPOT_LIGHTS,
    },
    shaders::{
        build_material_shader_with_prelude, create_depth_prepass_shader_module_rigid,
        create_depth_prepass_shader_module_skinned, create_frustum_cull_shader_module,
        create_hiz_depth_copy_shader_module, create_hiz_downsample_shader_module,
        create_hiz_occlusion_cull_shader_module, create_mesh_shader_module_rigid,
        create_mesh_shader_module_skinned, create_multimesh_shader_module,
        create_sky_shader_module, create_toon_shader_module_rigid,
        create_toon_shader_module_skinned, create_unlit_shader_module_rigid,
        create_unlit_shader_module_skinned,
    },
};
use crate::backend::{
    OcclusionCullingMode, StaticMeshLookup, StaticShaderLookup, StaticTextureLookup,
};
use crate::resources::ResourceStore;
use ahash::AHashMap;
use bytemuck::{Pod, Zeroable};
use glam::{Mat4, Quat, Vec3, Vec4};
use mesh_presets::build_builtin_mesh_buffer;
use perro_graphics_assets::{
    DecodedLod, DecodedMesh, DecodedMeshlet, MeshRange, MeshVertex, decode_ptex,
    gltf_texture_source_from_mesh_source, load_mesh_from_source, load_texture_rgba,
};
use perro_ids::MeshID;
use perro_io::load_asset;
use perro_render_bridge::{
    Camera3DState, CameraProjectionState, LODOptions3D, Material3D, MaterialParamOverride3D,
    MaterialParamOverrideValue3D, MeshBlendOptions3D, MeshSurfaceBinding3D, StandardMaterial3D,
};
use std::{
    borrow::Cow,
    cmp::Ordering,
    ops::Range,
    sync::{Arc, mpsc, mpsc::TryRecvError},
    time::Duration,
};
use wgpu::util::DeviceExt;

mod mesh_presets;
#[path = "gpu/paths/multimesh.rs"]
mod multimesh_path;
#[path = "gpu/paths/rigid.rs"]
mod rigid_path;
#[path = "gpu/paths/skinned.rs"]
mod skinned_path;
#[path = "gpu/texture_cache.rs"]
mod texture_cache;

use multimesh_path::{create_multimesh_blend_pipeline, create_multimesh_pipeline, pack_unorm4x8};
use rigid_path::{
    create_depth_prepass_pipeline_rigid, create_pipeline_overlay_rigid, create_pipeline_rigid,
    create_pipeline_rigid_blend, create_shadow_depth_pipeline_rigid,
};
use skinned_path::{
    create_depth_prepass_pipeline_skinned, create_pipeline_overlay_skinned,
    create_pipeline_skinned, create_pipeline_skinned_blend, create_shadow_depth_pipeline_skinned,
};
use texture_cache::{CachedMaterialTexture, create_cached_material_texture};

#[path = "gpu/asset_bridge.rs"]
mod asset_bridge;
#[path = "gpu/buffers.rs"]
mod buffers;
#[path = "gpu/camera.rs"]
mod camera;
#[path = "gpu/culling.rs"]
mod culling;
#[path = "gpu/draw.rs"]
mod draw;
#[path = "gpu/init.rs"]
mod init;
#[path = "gpu/pipelines.rs"]
mod pipelines;
#[path = "gpu/prepare.rs"]
mod prepare;
#[path = "gpu/render_pass.rs"]
mod render_pass;
#[path = "gpu/resize.rs"]
mod resize;
#[path = "gpu/shadows.rs"]
mod shadows;
#[path = "gpu/sky.rs"]
mod sky;
#[path = "gpu/targets.rs"]
mod targets;

use asset_bridge::*;
pub(crate) use asset_bridge::{load_mesh3d_from_source, validate_mesh_source};
use camera::*;
use draw::*;
use sky::*;
use targets::*;

const DEPTH_FORMAT: wgpu::TextureFormat = wgpu::TextureFormat::Depth24Plus;
const DEPTH_PREPASS_FORMAT: wgpu::TextureFormat = wgpu::TextureFormat::Depth32Float;
const FRUSTUM_CULL_WORKGROUP_SIZE: u32 = 64;
const HIZ_WORKGROUP_SIZE_X: u32 = 8;
const HIZ_WORKGROUP_SIZE_Y: u32 = 8;
const HIZ_OCCLUSION_BIAS: f32 = 0.002;
const MATERIAL_TEXTURE_NONE: u32 = u32::MAX;
const PACKED_STANDARD_NORMAL_SCALE_MAX: f32 = 4.0;
const PACKED_TOON_RIM_STRENGTH_MAX: f32 = 4.0;
const PACKED_TOON_OUTLINE_WIDTH_MAX: f32 = 4.0;
const SHADOW_MAP_SIZE: u32 = 4096;
const SHADOW_DEPTH_FORMAT: wgpu::TextureFormat = wgpu::TextureFormat::Depth32Float;
const SHADOW_MAP_DEPTH_BIAS_CONST: i32 = 2;
const SHADOW_MAP_DEPTH_BIAS_SLOPE: f32 = 2.0;
const MATERIAL_FLAG_MESHLET_DEBUG_VIEW: u32 = 1u32 << 0;
const MATERIAL_FLAG_FLAT_SHADING: u32 = 1u32 << 1;
const MATERIAL_FLAG_HAS_BASE_COLOR_TEXTURE: u32 = 1u32 << 2;
const MATERIAL_FLAG_MESH_BLEND: u32 = 1u32 << 3;
const MATERIAL_FLAG_NORMAL_BLEND: u32 = 1u32 << 4;
const CUSTOM_PARAM_KIND_SCALAR: u32 = 0;
const CUSTOM_PARAM_KIND_VEC2: u32 = 1;
const CUSTOM_PARAM_KIND_VEC3: u32 = 2;
const CUSTOM_PARAM_KIND_VEC4: u32 = 3;
const TEMP_DISABLE_SHADOWS: bool = true;
// Debug lock: force a fixed world-space directional light vector.
// Set to false after validating shadow stability.
const DEBUG_FORCE_WORLD_SUN_DIR: bool = false;
const DEBUG_WORLD_SUN_DIR: [f32; 3] = [-0.45, -0.85, -0.28];

#[repr(C)]
#[derive(Clone, Copy, Pod, Zeroable, PartialEq)]
struct Scene3DUniform {
    view_proj: [[f32; 4]; 4],
    ambient_and_counts: [f32; 4],
    camera_pos: [f32; 4],
    ambient_color: [f32; 4],
    ray_light: RayLightGpu,
    ray_lights: [RayLightGpu; MAX_RAY_LIGHTS],
    point_lights: [PointLightGpu; MAX_POINT_LIGHTS],
    spot_lights: [SpotLightGpu; MAX_SPOT_LIGHTS],
}

#[repr(C)]
#[derive(Clone, Copy, Pod, Zeroable, PartialEq)]
struct SkyUniform {
    inv_view_proj: [[f32; 4]; 4],
    camera_pos: [f32; 4],
    day_colors: [[f32; 4]; 3],
    evening_colors: [[f32; 4]; 3],
    night_colors: [[f32; 4]; 3],
    params0: [f32; 4], // cloud_size, cloud_density, cloud_variance, time_of_day
    params1: [f32; 4], // star_size, star_scatter, star_gleam, sky_angle
    params2: [f32; 4], // sun_size, moon_size, day_weight, cloud_time_seconds
    wind: [f32; 4],    // x,y = cloud wind, z = style_blend (0 toon, 1 realistic), w = reserved
}

const SKY_PARAMS2_W_OFFSET: u64 =
    std::mem::offset_of!(SkyUniform, params2) as u64 + (3 * std::mem::size_of::<f32>() as u64);

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
struct RigidMeshVertex {
    pos: [f32; 3],
    normal: [f32; 3],
    uv: [f32; 2],
}

#[repr(C)]
#[derive(Clone, Copy, Pod, Zeroable)]
struct TransformInstanceGpu {
    model_row_0: [f32; 4],
    model_row_1: [f32; 4],
    model_row_2: [f32; 4],
}

#[repr(C)]
#[derive(Clone, Copy, Pod, Zeroable)]
struct MaterialInstanceGpu {
    packed_color: u32,
    packed_pbr_params_0: u32, // lane payload (standard/toon path-specific)
    packed_pbr_params_1: u32, // reserved extension word
    packed_emissive: u32,
    packed_material_params: u32, // alpha_mode/alpha_cutoff/double_sided/flags
}

#[repr(C)]
#[derive(Clone, Copy, Pod, Zeroable)]
struct RigidInstanceMetaGpu {
    custom_params: [u32; 2], // custom_params_offset, custom_params_len
}

#[repr(C)]
#[derive(Clone, Copy, Pod, Zeroable)]
struct SkinnedInstanceMetaGpu {
    skeleton_params: [u32; 4], // start, count, custom_params_offset, custom_params_len
}

#[repr(C)]
#[derive(Clone, Copy, Pod, Zeroable)]
struct MultiMeshInstanceGpu {
    position: [f32; 3],
    rotation: [f32; 4],
    draw_id: u32,
}

#[repr(C)]
#[derive(Clone, Copy, Pod, Zeroable)]
struct MultiMeshDrawParamGpu {
    model_row_0: [f32; 4],
    model_row_1: [f32; 4],
    model_row_2: [f32; 4],
    packed_color: u32,
    packed_emissive: u32,
    scale_bits: u32,
    packed_blend_params: u32,
}

#[repr(C)]
#[derive(Clone, Copy, Pod, Zeroable, PartialEq)]
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
#[derive(Clone, Copy, Pod, Zeroable, PartialEq)]
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

#[repr(C)]
#[derive(Clone, Copy, Pod, Zeroable, PartialEq)]
struct ShadowUniform {
    light_view_proj: [[f32; 4]; 4],
    params0: [f32; 4], // enabled, strength, depth_bias, normal_bias
}

pub struct Gpu3D {
    color_format: wgpu::TextureFormat,
    camera_bgl: wgpu::BindGroupLayout,
    rigid_camera_bgl: wgpu::BindGroupLayout,
    multimesh_bgl: wgpu::BindGroupLayout,
    material_texture_bgl: wgpu::BindGroupLayout,
    shadow_bgl: wgpu::BindGroupLayout,
    sky_bgl: wgpu::BindGroupLayout,
    material_pipeline_layout: wgpu::PipelineLayout,
    rigid_material_pipeline_layout: wgpu::PipelineLayout,
    sky_pipeline: wgpu::RenderPipeline,
    pipeline_rigid_culled: wgpu::RenderPipeline,
    pipeline_rigid_double_sided: wgpu::RenderPipeline,
    pipeline_rigid_blend_culled: wgpu::RenderPipeline,
    pipeline_rigid_blend_double_sided: wgpu::RenderPipeline,
    pipeline_rigid_unlit_culled: wgpu::RenderPipeline,
    pipeline_rigid_unlit_double_sided: wgpu::RenderPipeline,
    pipeline_rigid_unlit_blend_culled: wgpu::RenderPipeline,
    pipeline_rigid_unlit_blend_double_sided: wgpu::RenderPipeline,
    pipeline_rigid_toon_culled: wgpu::RenderPipeline,
    pipeline_rigid_toon_double_sided: wgpu::RenderPipeline,
    pipeline_rigid_toon_blend_culled: wgpu::RenderPipeline,
    pipeline_rigid_toon_blend_double_sided: wgpu::RenderPipeline,
    pipeline_rigid_overlay_culled: wgpu::RenderPipeline,
    pipeline_rigid_overlay_double_sided: wgpu::RenderPipeline,
    pipeline_culled: wgpu::RenderPipeline,
    pipeline_double_sided: wgpu::RenderPipeline,
    pipeline_blend_culled: wgpu::RenderPipeline,
    pipeline_blend_double_sided: wgpu::RenderPipeline,
    pipeline_unlit_culled: wgpu::RenderPipeline,
    pipeline_unlit_double_sided: wgpu::RenderPipeline,
    pipeline_unlit_blend_culled: wgpu::RenderPipeline,
    pipeline_unlit_blend_double_sided: wgpu::RenderPipeline,
    pipeline_toon_culled: wgpu::RenderPipeline,
    pipeline_toon_double_sided: wgpu::RenderPipeline,
    pipeline_toon_blend_culled: wgpu::RenderPipeline,
    pipeline_toon_blend_double_sided: wgpu::RenderPipeline,
    pipeline_overlay_culled: wgpu::RenderPipeline,
    pipeline_overlay_double_sided: wgpu::RenderPipeline,
    pipeline_depth_prepass_culled: wgpu::RenderPipeline,
    pipeline_depth_prepass_double_sided: wgpu::RenderPipeline,
    pipeline_depth_prepass_rigid_culled: wgpu::RenderPipeline,
    pipeline_depth_prepass_rigid_double_sided: wgpu::RenderPipeline,
    pipeline_shadow_depth_culled: wgpu::RenderPipeline,
    pipeline_shadow_depth_double_sided: wgpu::RenderPipeline,
    pipeline_shadow_depth_rigid_culled: wgpu::RenderPipeline,
    pipeline_shadow_depth_rigid_double_sided: wgpu::RenderPipeline,
    pipeline_multimesh_culled: wgpu::RenderPipeline,
    pipeline_multimesh_double_sided: wgpu::RenderPipeline,
    pipeline_multimesh_blend_culled: wgpu::RenderPipeline,
    pipeline_multimesh_blend_double_sided: wgpu::RenderPipeline,
    custom_pipelines: AHashMap<u32, CustomPipeline>,
    custom_pipelines_rigid: AHashMap<u32, CustomPipeline>,
    custom_pipeline_tokens: AHashMap<String, u32>,
    next_custom_pipeline_token: u32,
    camera_buffer: wgpu::Buffer,
    camera_bind_group: wgpu::BindGroup,
    rigid_camera_bind_group: wgpu::BindGroup,
    shadow_camera_buffer: wgpu::Buffer,
    shadow_camera_bind_group: wgpu::BindGroup,
    rigid_shadow_camera_bind_group: wgpu::BindGroup,
    shadow_buffer: wgpu::Buffer,
    shadow_bind_group: wgpu::BindGroup,
    _shadow_map_texture: wgpu::Texture,
    shadow_map_view: wgpu::TextureView,
    _shadow_map_sampler: wgpu::Sampler,
    mesh_blend_bgl: wgpu::BindGroupLayout,
    mesh_blend_bind_group: wgpu::BindGroup,
    sky_buffer: wgpu::Buffer,
    sky_bind_group: wgpu::BindGroup,
    _sky_noise_texture: wgpu::Texture,
    _sky_noise_view: wgpu::TextureView,
    _sky_noise_sampler: wgpu::Sampler,
    skeleton_buffer: wgpu::Buffer,
    skeleton_capacity: usize,
    staged_skeletons: Vec<[[f32; 4]; 4]>,
    custom_params_meta_buffer: wgpu::Buffer,
    custom_params_meta_capacity: usize,
    staged_custom_params_meta: Vec<u32>,
    custom_params_meta_uploaded: usize,
    custom_params_values_buffer: wgpu::Buffer,
    custom_params_values_capacity: usize,
    staged_custom_params_values: Vec<f32>,
    custom_params_values_uploaded: usize,
    staged_custom_params_dedupe: AHashMap<Vec<u32>, (u32, u32)>,
    staged_custom_params_key_scratch: Vec<u32>,
    staged_custom_params_meta_scratch: Vec<u32>,
    staged_custom_params_values_scratch: Vec<f32>,
    material_fallback_texture: Option<CachedMaterialTexture>,
    material_textures: AHashMap<u32, CachedMaterialTexture>,
    instance_transform_buffer: wgpu::Buffer,
    instance_transform_capacity: usize,
    staged_instance_transforms: Vec<TransformInstanceGpu>,
    instance_material_buffer: wgpu::Buffer,
    instance_material_capacity: usize,
    staged_instance_materials: Vec<MaterialInstanceGpu>,
    rigid_instance_meta_buffer: wgpu::Buffer,
    rigid_instance_meta_capacity: usize,
    staged_rigid_instance_meta: Vec<RigidInstanceMetaGpu>,
    skinned_instance_meta_buffer: wgpu::Buffer,
    skinned_instance_meta_capacity: usize,
    staged_skinned_instance_meta: Vec<SkinnedInstanceMetaGpu>,
    multimesh_bind_group: wgpu::BindGroup,
    multimesh_draw_params_buffer: wgpu::Buffer,
    multimesh_draw_params_capacity: usize,
    staged_multimesh_draw_params: Vec<MultiMeshDrawParamGpu>,
    multimesh_instance_buffer: wgpu::Buffer,
    multimesh_instance_capacity: usize,
    staged_multimesh_instances: Vec<MultiMeshInstanceGpu>,
    multimesh_batches: Vec<MultiMeshBatch>,
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
    frustum_gpu_inputs_valid: bool,
    last_frustum_params: Option<FrustumCullParamsGpu>,
    last_hiz_params: Option<HizCullParamsGpu>,
    last_prepare_step_timing: Prepare3DStepTiming,
    draw_batches: Vec<DrawBatch>,
    has_shadow_casters: bool,
    surface_entries_scratch: Vec<(MeshRange, Material3D)>,
    mesh_blend_scratch: Vec<ResolvedMeshBlend>,
    last_draws: Vec<Draw3DInstance>,
    last_draws_revision: u64,
    last_draw_instance_spans: Vec<Range<u32>>,
    last_draw_instance_span_ranges: Vec<Range<usize>>,
    last_scene: Option<Scene3DUniform>,
    last_shadow_scene: Option<Scene3DUniform>,
    last_shadow: Option<ShadowUniform>,
    shadow_pass_enabled: bool,
    shadow_focus_center: Vec3,
    shadow_focus_radius: f32,
    last_sky: Option<SkyUniform>,
    last_sky_cloud_time_seconds: f32,
    sky_enabled: bool,
    mesh_vertices: Vec<MeshVertex>,
    rigid_mesh_vertices: Vec<RigidMeshVertex>,
    mesh_indices: Vec<u32>,
    vertex_buffer: wgpu::Buffer,
    rigid_vertex_buffer: wgpu::Buffer,
    index_buffer: wgpu::Buffer,
    vertex_capacity: usize,
    rigid_vertex_capacity: usize,
    index_capacity: usize,
    builtin_mesh_ranges: AHashMap<&'static str, MeshRange>,
    builtin_mesh_bounds: AHashMap<&'static str, ([f32; 3], f32)>,
    builtin_meshlets: AHashMap<&'static str, Arc<[MeshletRange]>>,
    custom_mesh_ranges: AHashMap<MeshID, (u64, MeshAssetRange)>,
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
    hiz_copy_bind_group: Option<wgpu::BindGroup>,
    hiz_downsample_bind_groups: Vec<wgpu::BindGroup>,
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
    occlusion_state: AHashMap<u64, OcclusionState>,
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
    dirty_instance_spans_scratch: Vec<Range<u32>>,
    merged_instance_spans_scratch: Vec<Range<u32>>,
    dirty_cull_batch_spans_scratch: Vec<Range<usize>>,
    debug_point_instances_scratch: Vec<BuiltInstanceParts>,
    debug_edge_instances_scratch: Vec<BuiltInstanceParts>,
}

pub struct Prepare3D<'a> {
    pub resources: &'a ResourceStore,
    pub camera: Camera3DState,
    pub lighting: &'a Lighting3DState,
    pub draws: &'a [Draw3DInstance],
    pub draws_revision: u64,
    pub width: u32,
    pub height: u32,
    pub static_texture_lookup: Option<StaticTextureLookup>,
    pub static_mesh_lookup: Option<StaticMeshLookup>,
    pub static_shader_lookup: Option<StaticShaderLookup>,
}

#[derive(Clone, Copy, Debug, Default)]
pub struct Prepare3DStepTiming {
    pub frustum_prep: Duration,
    pub hiz_prep: Duration,
    pub indirect_prep: Duration,
    pub cull_input_prep: Duration,
    pub frustum_skipped: u32,
    pub hiz_skipped: u32,
    pub indirect_skipped: u32,
    pub cull_input_skipped: u32,
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
struct MeshletRange {
    index_start: u32,
    index_count: u32,
    center: [f32; 3],
    radius: f32,
}

#[derive(Clone)]
struct MeshAssetRange {
    full: MeshRange,
    surface_ranges: Arc<[MeshRange]>,
    meshlets: Arc<[MeshletRange]>,
    lods: Arc<[MeshLodRange]>,
    bounds_center: [f32; 3],
    bounds_radius: f32,
}

#[derive(Clone)]
struct MeshLodRange {
    full: MeshRange,
    surface_ranges: Arc<[MeshRange]>,
    meshlets: Arc<[MeshletRange]>,
}

struct MeshLodView<'a> {
    full: MeshRange,
    surface_ranges: &'a [MeshRange],
    meshlets: &'a [MeshletRange],
}

#[derive(Clone)]
struct DrawBatch {
    state_key: u64,
    mesh: MeshRange,
    instance_start: u32,
    instance_count: u32,
    path: RenderPath3D,
    double_sided: bool,
    material_kind: MaterialPipelineKind,
    alpha_mode: u8,
    draw_on_top: bool,
    base_color_texture_slot: u32,
    local_center: [f32; 3],
    local_radius: f32,
    occlusion_query: Option<u32>,
    disable_hiz_occlusion: bool,
    casts_shadows: bool,
    mesh_blend: bool,
}

#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Debug)]
enum RenderPath3D {
    Rigid,
    Skinned,
}

#[derive(Clone)]
struct MultiMeshBatch {
    mesh: MeshRange,
    instance_start: u32,
    instance_count: u32,
    draw_param_index: u32,
    double_sided: bool,
    mesh_blend: bool,
}

#[derive(Clone, Debug, PartialEq, Eq, Hash)]
enum MaterialPipelineKind {
    Standard,
    Unlit,
    Toon,
    Custom(u32),
}

struct CustomPipeline {
    pipeline_culled: wgpu::RenderPipeline,
    pipeline_double_sided: wgpu::RenderPipeline,
    pipeline_blend_culled: wgpu::RenderPipeline,
    pipeline_blend_double_sided: wgpu::RenderPipeline,
}

#[derive(Clone, Copy)]
struct OcclusionState {
    visible_last_frame: bool,
    last_test_frame: u64,
}

const CULL_FLAG_DISABLE_HIZ_OCCLUSION: u32 = 1u32;
const LOD_DISTANCE_RADIUS_SCALES: [f32; 5] = [36.0, 54.0, 72.0, 108.0, 144.0];
const FRUSTUM_CULL_MIN_BATCHES: usize = 96;
const FRUSTUM_CULL_MIN_INSTANCES: usize = 1024;
const FRUSTUM_CULL_HIGH_VISIBLE_RATIO: f32 = 0.9;
const FRUSTUM_CULL_HIGH_VISIBLE_MIN_SAMPLES: u32 = 24;
const FRUSTUM_CULL_HIGH_VISIBLE_MIN_BATCHES: usize = 160;
const FRUSTUM_CULL_HIGH_VISIBLE_MIN_INSTANCES: usize = 2048;
const HIZ_OCCLUSION_MIN_BATCHES: usize = 80;
const HIZ_OCCLUSION_MIN_INSTANCES: usize = 1024;
const DEPTH_PREPASS_MIN_BATCHES: usize = 32;
const DEPTH_PREPASS_MIN_INSTANCES: usize = 512;
const HIZ_DEBUG_READBACK_ENABLED: bool = false;
// Re-test occluded batches every frame so visibility recovers immediately when camera/object moves.
const OCCLUSION_PROBE_INTERVAL: u64 = 1;

#[cfg(test)]
mod tests {
    use super::{
        DrawBatchPush, MATERIAL_TEXTURE_NONE, MaterialPipelineKind, RenderPath3D,
        draw_batch_state_key, push_draw_batch,
    };
    use perro_asset_formats::pmesh::{
        FLAG_HAS_JOINTS as PMESH_FLAG_HAS_JOINTS, FLAG_HAS_NORMAL as PMESH_FLAG_HAS_NORMAL,
        FLAG_HAS_UV0 as PMESH_FLAG_HAS_UV0, FLAG_HAS_WEIGHTS as PMESH_FLAG_HAS_WEIGHTS,
        VERSION as PMESH_VERSION,
    };
    use perro_graphics_assets::{MeshRange, decode_pmesh, decode_ptex};

    #[test]
    fn decode_pmesh_accepts_v1_render_payload_with_all_attributes() {
        let mut raw = Vec::new();
        raw.extend_from_slice(&1.0f32.to_le_bytes());
        raw.extend_from_slice(&2.0f32.to_le_bytes());
        raw.extend_from_slice(&3.0f32.to_le_bytes());
        raw.extend_from_slice(&0.0f32.to_le_bytes());
        raw.extend_from_slice(&1.0f32.to_le_bytes());
        raw.extend_from_slice(&0.0f32.to_le_bytes());
        raw.extend_from_slice(&0.25f32.to_le_bytes());
        raw.extend_from_slice(&0.75f32.to_le_bytes());
        raw.extend_from_slice(&4u16.to_le_bytes());
        raw.extend_from_slice(&5u16.to_le_bytes());
        raw.extend_from_slice(&6u16.to_le_bytes());
        raw.extend_from_slice(&7u16.to_le_bytes());
        raw.extend_from_slice(&0.1f32.to_le_bytes());
        raw.extend_from_slice(&0.2f32.to_le_bytes());
        raw.extend_from_slice(&0.3f32.to_le_bytes());
        raw.extend_from_slice(&0.4f32.to_le_bytes());
        raw.extend_from_slice(&0u32.to_le_bytes());
        raw.extend_from_slice(&0u32.to_le_bytes());
        raw.extend_from_slice(&0u32.to_le_bytes());
        raw.extend_from_slice(&0u32.to_le_bytes());
        raw.extend_from_slice(&3u32.to_le_bytes());
        raw.extend_from_slice(&0u32.to_le_bytes());
        raw.extend_from_slice(&3u32.to_le_bytes());
        raw.extend_from_slice(&9.0f32.to_le_bytes());
        raw.extend_from_slice(&8.0f32.to_le_bytes());
        raw.extend_from_slice(&7.0f32.to_le_bytes());
        raw.extend_from_slice(&6.0f32.to_le_bytes());

        let compressed = perro_io::compress_zlib_best(&raw).expect("compress pmesh payload");
        let mut bytes = Vec::new();
        bytes.extend_from_slice(b"PMESH");
        bytes.extend_from_slice(&PMESH_VERSION.to_le_bytes());
        let flags = PMESH_FLAG_HAS_NORMAL
            | PMESH_FLAG_HAS_UV0
            | PMESH_FLAG_HAS_JOINTS
            | PMESH_FLAG_HAS_WEIGHTS;
        bytes.extend_from_slice(&flags.to_le_bytes());
        bytes.extend_from_slice(&1u32.to_le_bytes());
        bytes.extend_from_slice(&3u32.to_le_bytes());
        bytes.extend_from_slice(&1u32.to_le_bytes());
        bytes.extend_from_slice(&1u32.to_le_bytes());
        bytes.extend_from_slice(&0u32.to_le_bytes());
        bytes.extend_from_slice(&(raw.len() as u32).to_le_bytes());
        bytes.extend_from_slice(&compressed);

        let decoded = decode_pmesh(&bytes).expect("decode v1 pmesh");
        assert_eq!(decoded.vertices.len(), 1);
        assert_eq!(decoded.indices, vec![0, 0, 0]);
        assert_eq!(decoded.vertices[0].pos, [1.0, 2.0, 3.0]);
        assert_eq!(decoded.vertices[0].normal, [0.0, 1.0, 0.0]);
        assert_eq!(decoded.vertices[0].uv, [0.25, 0.75]);
        assert_eq!(decoded.vertices[0].joints, [4, 5, 6, 7]);
        assert_eq!(decoded.vertices[0].weights, [0.1, 0.2, 0.3, 0.4]);
        assert_eq!(decoded.surface_ranges.len(), 1);
        assert_eq!(decoded.surface_ranges[0].index_start, 0);
        assert_eq!(decoded.surface_ranges[0].index_count, 3);
        assert_eq!(decoded.meshlets.len(), 1);
        assert_eq!(decoded.meshlets[0].index_start, 0);
        assert_eq!(decoded.meshlets[0].index_count, 3);
        assert_eq!(decoded.meshlets[0].center, [9.0, 8.0, 7.0]);
        assert_eq!(decoded.meshlets[0].radius, 6.0);
    }

    #[test]
    fn decode_pmesh_rejects_non_v1() {
        for version in [2u32, 3, 4, 5, 6, 7, 8] {
            let mut bytes = Vec::new();
            bytes.extend_from_slice(b"PMESH");
            bytes.extend_from_slice(&version.to_le_bytes());
            bytes.resize(33, 0);
            assert!(
                decode_pmesh(&bytes).is_none(),
                "non-v1 pmesh version {version} must reject"
            );
        }
    }

    #[test]
    fn decode_ptex_accepts_v1_rgb_payload() {
        let raw_rgb = vec![10u8, 20, 30, 40, 50, 60];
        let compressed = perro_io::compress_zlib_best(&raw_rgb).expect("compress ptex payload");
        let mut bytes = Vec::new();
        bytes.extend_from_slice(b"PTEX");
        bytes.extend_from_slice(&1u32.to_le_bytes());
        bytes.extend_from_slice(&2u32.to_le_bytes());
        bytes.extend_from_slice(&1u32.to_le_bytes());
        bytes.extend_from_slice(&1u32.to_le_bytes()); // rgb8
        bytes.extend_from_slice(&(raw_rgb.len() as u32).to_le_bytes());
        bytes.extend_from_slice(&compressed);

        let decoded = decode_ptex(&bytes).expect("decode v1 ptex");
        assert_eq!(decoded.1, 2);
        assert_eq!(decoded.2, 1);
        assert_eq!(decoded.0, vec![10u8, 20, 30, 255, 40, 50, 60, 255]);
    }

    #[test]
    fn decode_ptex_rejects_non_v1() {
        for version in [2u32, 3, 4, 5] {
            let mut bytes = Vec::new();
            bytes.extend_from_slice(b"PTEX");
            bytes.extend_from_slice(&version.to_le_bytes());
            bytes.extend_from_slice(&1u32.to_le_bytes());
            bytes.extend_from_slice(&1u32.to_le_bytes());
            bytes.extend_from_slice(&0u32.to_le_bytes());
            bytes.extend_from_slice(&0u32.to_le_bytes());
            assert!(
                decode_ptex(&bytes).is_none(),
                "non-v1 ptex version {version} must reject"
            );
        }
    }

    #[test]
    fn push_draw_batch_merges_compatible_adjacent_ranges() {
        let mut batches = Vec::new();
        let mesh = MeshRange {
            index_start: 4,
            index_count: 36,
            base_vertex: 0,
        };

        push_draw_batch(
            &mut batches,
            DrawBatchPush {
                render_path: RenderPath3D::Rigid,
                mesh,
                instance_start: 0,
                instance_count: 1,
                double_sided: false,
                material_kind: MaterialPipelineKind::Standard,
                alpha_mode: 0,
                base_color_texture_slot: MATERIAL_TEXTURE_NONE,
                local_bounds: ([1.0, 2.0, 3.0], 2.0),
                occlusion_query: None,
                disable_hiz_occlusion: false,
                casts_shadows: true,
                mesh_blend: false,
            },
        );
        push_draw_batch(
            &mut batches,
            DrawBatchPush {
                render_path: RenderPath3D::Rigid,
                mesh,
                instance_start: 1,
                instance_count: 2,
                double_sided: false,
                material_kind: MaterialPipelineKind::Standard,
                alpha_mode: 0,
                base_color_texture_slot: MATERIAL_TEXTURE_NONE,
                local_bounds: ([9.0, 9.0, 9.0], 4.0),
                occlusion_query: None,
                disable_hiz_occlusion: false,
                casts_shadows: true,
                mesh_blend: false,
            },
        );

        assert_eq!(batches.len(), 1);
        let merged = &batches[0];
        assert_eq!(merged.instance_start, 0);
        assert_eq!(merged.instance_count, 3);
        assert_eq!(
            merged.state_key,
            draw_batch_state_key(
                RenderPath3D::Rigid,
                false,
                false,
                0,
                &MaterialPipelineKind::Standard
            )
        );
        assert_eq!(merged.local_center, [0.0, 0.0, 0.0]);
        assert_eq!(merged.local_radius, 1.0e9);
        assert!(merged.disable_hiz_occlusion);
    }

    #[test]
    fn push_draw_batch_keeps_separate_batches_when_not_mergeable() {
        let mut batches = Vec::new();
        let mesh = MeshRange {
            index_start: 7,
            index_count: 12,
            base_vertex: 0,
        };

        push_draw_batch(
            &mut batches,
            DrawBatchPush {
                render_path: RenderPath3D::Rigid,
                mesh,
                instance_start: 0,
                instance_count: 1,
                double_sided: false,
                material_kind: MaterialPipelineKind::Standard,
                alpha_mode: 0,
                base_color_texture_slot: MATERIAL_TEXTURE_NONE,
                local_bounds: ([0.0, 0.0, 0.0], 1.0),
                occlusion_query: None,
                disable_hiz_occlusion: false,
                casts_shadows: true,
                mesh_blend: false,
            },
        );
        push_draw_batch(
            &mut batches,
            DrawBatchPush {
                render_path: RenderPath3D::Rigid,
                mesh,
                instance_start: 2,
                instance_count: 1,
                double_sided: false,
                material_kind: MaterialPipelineKind::Standard,
                alpha_mode: 0,
                base_color_texture_slot: MATERIAL_TEXTURE_NONE,
                local_bounds: ([0.0, 0.0, 0.0], 1.0),
                occlusion_query: None,
                disable_hiz_occlusion: false,
                casts_shadows: true,
                mesh_blend: false,
            },
        );
        push_draw_batch(
            &mut batches,
            DrawBatchPush {
                render_path: RenderPath3D::Rigid,
                mesh,
                instance_start: 3,
                instance_count: 1,
                double_sided: false,
                material_kind: MaterialPipelineKind::Standard,
                alpha_mode: 0,
                base_color_texture_slot: MATERIAL_TEXTURE_NONE,
                local_bounds: ([0.0, 0.0, 0.0], 1.0),
                occlusion_query: Some(11),
                disable_hiz_occlusion: false,
                casts_shadows: true,
                mesh_blend: false,
            },
        );

        assert_eq!(batches.len(), 3);
    }
}
