//! 3D GPU renderer state, asset decode, batching, culling, and draw submission.

use super::{
    renderer::{
        DenseMultiMeshDraw3D, Draw3DInstance, Draw3DKind, Lighting3DState, MAX_POINT_LIGHTS,
        MAX_RAY_LIGHTS, MAX_SPOT_LIGHTS,
    },
    shaders::{
        build_custom_material_shader_with_prelude, build_custom_multimesh_material_shader,
        create_depth_prepass_shader_module_rigid,
        create_depth_prepass_shader_module_rigid_packed_lod,
        create_depth_prepass_shader_module_skinned, create_frustum_cull_shader_module,
        create_hiz_depth_copy_shader_module, create_hiz_downsample_shader_module,
        create_hiz_downsample_spd_shader_module, create_hiz_occlusion_cull_shader_module,
        create_mesh_blend_mask_shader_module_rigid,
        create_mesh_blend_mask_shader_module_rigid_packed_lod,
        create_mesh_blend_mask_shader_module_skinned, create_mesh_blend_screen_shader_module,
        create_mesh_shader_module_rigid, create_mesh_shader_module_rigid_packed_lod,
        create_mesh_shader_module_skinned, create_multimesh_cull_shader_module,
        create_multimesh_shader_module, create_sky_shader_module,
        create_sky_shader_module_from_source, create_toon_shader_module_rigid,
        create_toon_shader_module_skinned, create_unlit_shader_module_rigid,
        create_unlit_shader_module_skinned, prelude_rigid_wgsl, prelude_skinned_wgsl,
    },
};
use crate::backend::{
    OcclusionCullingMode, StaticMeshLookup, StaticShaderLookup, StaticTextureLookup,
};
use crate::resources::ResourceStore;
use ahash::{AHashMap, AHashSet};
use bytemuck::{Pod, Zeroable};
use glam::{Mat4, Quat, Vec3, Vec4};
use mesh_presets::build_builtin_mesh_buffer;
use perro_graphics_assets::{
    DecodedLod, DecodedMesh, DecodedMeshlet, MeshRange, MeshVertex as DecodedMeshVertex,
    gltf_texture_source_from_mesh_source, load_mesh_from_source,
    load_mesh_from_source_no_dynamic_lods, load_texture_rgba,
};
use perro_ids::MeshID;
use perro_io::load_asset;
use perro_render_bridge::{
    Camera3DState, CameraProjectionState, CustomMaterialLighting3D, DenseInstancePose3D,
    LODOptions3D, Material3D, MaterialParamOverride3D, MaterialParamOverrideValue3D,
    MeshBlendOptions3D, MeshSurfaceBinding3D, PointLight3DState, SpotLight3DState,
    StandardMaterial3D,
};
use perro_structs::BitMask;
use perro_structs::TextureFilterMode;
use std::{
    borrow::Cow,
    cmp::Ordering,
    ops::Range,
    sync::{Arc, mpsc, mpsc::TryRecvError},
    time::Duration,
};
use wgpu::util::DeviceExt;

type MeshVertex = DecodedMeshVertex;

const CUSTOM_MATERIAL_IMAGE_COUNT: usize = 8;
const MATERIAL_TEXTURE_SET_SIZE: usize = 1 + CUSTOM_MATERIAL_IMAGE_COUNT;
const CUSTOM_MATERIAL_TEXTURE_SLOT_BASE: u32 = 0x8000_0000;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
struct MaterialTextureKey {
    slots: [u32; MATERIAL_TEXTURE_SET_SIZE],
}

impl MaterialTextureKey {
    const fn empty() -> Self {
        Self {
            slots: [MATERIAL_TEXTURE_NONE; MATERIAL_TEXTURE_SET_SIZE],
        }
    }

    fn from_base(base: u32) -> Self {
        let mut key = Self::empty();
        key.slots[0] = base;
        key
    }

    fn state_hash(self) -> u64 {
        let mut hash = 0xcbf2_9ce4_8422_2325u64;
        for slot in self.slots {
            hash ^= slot as u64;
            hash = hash.wrapping_mul(0x1000_0000_01b3);
        }
        hash
    }
}

// Pose-pack cache entry: pinned source Arc + its packed geometry lanes.
type PosePackCacheEntry = (Arc<[DenseInstancePose3D]>, Arc<[MultiMeshPosePacked]>);

mod mesh_presets;
#[path = "gpu/paths/multimesh.rs"]
mod multimesh_path;
#[path = "gpu/paths/rigid.rs"]
mod rigid_path;
#[path = "gpu/paths/skinned.rs"]
mod skinned_path;
#[path = "gpu/texture_cache.rs"]
mod texture_cache;

use multimesh_path::{
    create_multimesh_blend_pipeline, create_multimesh_covered_pipeline,
    create_multimesh_depth_prepass_pipeline, create_multimesh_mask_pipeline,
    create_multimesh_pipeline, create_multimesh_shadow_depth_pipeline, pack_unorm4x8,
};
use rigid_path::{
    create_depth_prepass_pipeline_rigid, create_depth_prepass_pipeline_rigid_packed_lod,
    create_pipeline_overlay_rigid, create_pipeline_rigid, create_pipeline_rigid_blend,
    create_pipeline_rigid_packed_lod, create_pipeline_rigid_packed_lod_blend,
    create_shadow_depth_pipeline_rigid, create_shadow_depth_pipeline_rigid_packed_lod,
};
use skinned_path::{
    create_depth_prepass_pipeline_skinned, create_pipeline_overlay_skinned,
    create_pipeline_skinned, create_pipeline_skinned_blend, create_shadow_depth_pipeline_skinned,
};
use texture_cache::{
    CachedMaterialTexture, CachedMaterialTextureInput, create_cached_material_texture,
    create_external_material_texture, create_material_texture_bind_group,
};

#[path = "gpu/asset_bridge.rs"]
mod asset_bridge;
#[path = "gpu/buffers.rs"]
mod buffers;
#[path = "gpu/camera.rs"]
mod camera;
#[path = "gpu/culling.rs"]
mod culling;
#[path = "gpu/decals.rs"]
mod decals;
#[path = "gpu/draw.rs"]
mod draw;
#[path = "gpu/init.rs"]
mod init;
#[path = "gpu/mesh_blend_screen.rs"]
mod mesh_blend_screen;
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
#[path = "gpu/ssao.rs"]
mod ssao;
#[path = "gpu/targets.rs"]
mod targets;

use asset_bridge::*;
pub(crate) use asset_bridge::{load_mesh3d_from_source, validate_mesh_source};
use camera::*;
use decals::{create_decal_buffer, create_decal_texture_array};
use draw::*;
use sky::*;
use targets::*;

const DEPTH_PREPASS_FORMAT: wgpu::TextureFormat = wgpu::TextureFormat::Depth32Float;
const FRUSTUM_CULL_WORKGROUP_SIZE: u32 = 64;
const HIZ_WORKGROUP_SIZE_X: u32 = 8;
const HIZ_WORKGROUP_SIZE_Y: u32 = 8;
// SPD downsampler: dst mips written per dispatch (one src read + this many
// storage-texture writes per bind group). Each dispatch's 8x8 workgroup owns a
// 16x16 source region (2^HIZ_SPD_MIPS). Kept small enough that 1 sampled + this
// many storage textures stay within max_storage_textures_per_shader_stage.
const HIZ_SPD_MIPS: u32 = 4;
const HIZ_OCCLUSION_BIAS: f32 = 0.002;
const MATERIAL_TEXTURE_NONE: u32 = u32::MAX;
const PACKED_STANDARD_NORMAL_SCALE_MAX: f32 = 4.0;
const PACKED_TOON_RIM_STRENGTH_MAX: f32 = 4.0;
const PACKED_TOON_OUTLINE_WIDTH_MAX: f32 = 4.0;
const SHADOW_MAP_SIZE: u32 = 2048;
const SHADOW_SPOT_MAP_SIZE: u32 = 2048;
const SHADOW_POINT_MAP_SIZE: u32 = 1024;
const MAX_SHADOW_RAY_LIGHTS: usize = 1;
const MAX_SHADOW_RAY_CASCADES: usize = 4;
const MAX_SHADOW_SPOT_LIGHTS: usize = 4;
const MAX_SHADOW_POINT_LIGHTS: usize = 4;
const POINT_SHADOW_FACE_COUNT: usize = 6;
const SHADOW_CAMERA_COUNT: usize = MAX_SHADOW_RAY_LIGHTS * MAX_SHADOW_RAY_CASCADES
    + MAX_SHADOW_SPOT_LIGHTS
    + MAX_SHADOW_POINT_LIGHTS * POINT_SHADOW_FACE_COUNT;
const SHADOW_DEPTH_FORMAT: wgpu::TextureFormat = wgpu::TextureFormat::Depth32Float;
const SHADOW_MAP_DEPTH_BIAS_CONST: i32 = 2;
const SHADOW_MAP_DEPTH_BIAS_SLOPE: f32 = 2.0;
const MATERIAL_FLAG_MESHLET_DEBUG_VIEW: u32 = 1u32 << 0;
const MATERIAL_FLAG_FLAT_SHADING: u32 = 1u32 << 1;
const MATERIAL_FLAG_HAS_BASE_COLOR_TEXTURE: u32 = 1u32 << 2;
const MATERIAL_FLAG_MESH_BLEND: u32 = 1u32 << 3;
const MATERIAL_FLAG_NORMAL_BLEND: u32 = 1u32 << 4;
const MATERIAL_FLAG_RECEIVE_SHADOWS: u32 = 1u32 << 6;
// Surface carries a chromatic modulate: the standard shader re-applies the
// hue bias against the base color texture sample (0x100 in WGSL).
const MATERIAL_FLAG_MODULATE_BIAS: u32 = 1u32 << 8;
const MATERIAL_FLAG_HAS_METALLIC_ROUGHNESS_TEXTURE: u32 = 1u32 << 9;
const MATERIAL_FLAG_HAS_NORMAL_TEXTURE: u32 = 1u32 << 10;
const MATERIAL_FLAG_HAS_OCCLUSION_TEXTURE: u32 = 1u32 << 11;
const MATERIAL_FLAG_HAS_EMISSIVE_TEXTURE: u32 = 1u32 << 12;
const CUSTOM_PARAM_KIND_SCALAR: u32 = 0;
const CUSTOM_PARAM_KIND_VEC2: u32 = 1;
const CUSTOM_PARAM_KIND_VEC3: u32 = 2;
const CUSTOM_PARAM_KIND_VEC4: u32 = 3;
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
    inv_view_proj: [[f32; 4]; 4],
    // Hemisphere ambient: radiance from below (premultiplied), w unused.
    ground_color: [f32; 4],
    // Sky radiance at the horizon (premultiplied) for env reflections.
    sky_horizon_color: [f32; 4],
    // Frame globals for custom shaders: [time (wraps hourly), delta seconds,
    // frame index, 0..1 phase over 60s]. Zeroed in the dedup copy; the live
    // values are patched every frame at SCENE_GLOBALS_OFFSET so time does not
    // defeat the camera-uniform change gate.
    time_params: [f32; 4],
    // [width, height, 1/width, 1/height].
    resolution: [f32; 4],
}

// Tail of Scene3DUniform written every frame (time + resolution globals).
#[repr(C)]
#[derive(Clone, Copy, Pod, Zeroable)]
struct SceneGlobalsGpu {
    time_params: [f32; 4],
    resolution: [f32; 4],
}

const SCENE_GLOBALS_TAIL_BYTES: u64 = std::mem::size_of::<SceneGlobalsGpu>() as u64;
const SCENE_GLOBALS_OFFSET: u64 =
    std::mem::size_of::<Scene3DUniform>() as u64 - SCENE_GLOBALS_TAIL_BYTES;

#[repr(C)]
#[derive(Clone, Copy, Pod, Zeroable, PartialEq)]
struct SkyUniform {
    inv_view_proj: [[f32; 4]; 4],
    camera_pos: [f32; 4],
    day_colors: [[f32; 4]; 3],
    evening_colors: [[f32; 4]; 3],
    night_colors: [[f32; 4]; 3],
    horizon_colors: [[f32; 4]; 3],
    params0: [f32; 4], // time_of_day, day_weight, evening_weight, night_weight
    params1: [f32; 4], // time_seconds, reserved
}

const SKY_PARAMS1_X_OFFSET: u64 = std::mem::offset_of!(SkyUniform, params1) as u64;

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
    normal: [i16; 4],
    uv: [f32; 2],
    paint_uv: [f32; 2],
}

#[repr(C)]
#[derive(Clone, Copy, Pod, Zeroable)]
struct PackedRigidLodVertex {
    pos: [u16; 4],
    normal: [i8; 4],
    uv: [u16; 2],
    paint_uv: [f32; 2],
}

#[repr(C)]
#[derive(Clone, Copy, Pod, Zeroable)]
struct PackedLodParamGpu {
    pos_min: [f32; 4],
    pos_extent: [f32; 4],
    uv_min_extent: [f32; 4],
}

#[repr(C)]
#[derive(Clone, Copy, Pod, Zeroable)]
struct SkinnedMeshVertex {
    pos: [f32; 3],
    normal: [i16; 4],
    uv: [f32; 2],
    joints: [u16; 4],
    weights: perro_structs::UnitVector4,
    paint_uv: [f32; 2],
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

const MATERIAL_FLAG_MIRRORED_WINDING: u32 = 1u32 << 5;
// packed_pbr_params_1 carries a local color-bleed tint for this instance.
const MATERIAL_FLAG_LOCAL_BLEED: u32 = 1u32 << 7;

#[repr(C)]
#[derive(Clone, Copy, Pod, Zeroable)]
struct RigidInstanceMetaGpu {
    material: MaterialInstanceGpu,
    custom_params: [u32; 2], // custom_params_offset, custom_params_len
    packed_lod_param_id: u32,
}

#[repr(C)]
#[derive(Clone, Copy, Pod, Zeroable)]
struct SkinnedInstanceMetaGpu {
    material: MaterialInstanceGpu,
    skeleton_params: [u32; 4], // start, count, custom_params_offset, custom_params_len
}

#[repr(C)]
#[derive(Clone, Copy, Pod, Zeroable)]
struct MultiMeshInstanceGpu {
    position: [f32; 3],
    rotation: [i16; 4],
    scale: [f32; 3],
    draw_id: u32,
    blend_meta_id: u32,
}

// Build-independent packed pose lanes cached per pose-Arc (item 3). Skips the
// per-instance quaternion pack when a pose list survives a full rebuild.
#[derive(Clone, Copy)]
struct MultiMeshPosePacked {
    position: [f32; 3],
    rotation: [i16; 4],
    scale: [f32; 3],
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
    custom_params: [u32; 2],
    // Local color bleed tint (pack_local_bleed layout); 0 = none.
    packed_bleed: u32,
    _pad: u32,
}

#[repr(C)]
#[derive(Clone, Copy, Pod, Zeroable)]
struct BlendShapeDeltaGpu {
    position_delta: [f32; 4],
    normal_delta: [f32; 4],
}

#[repr(C)]
#[derive(Clone, Copy, Pod, Zeroable)]
struct BlendShapeInstanceMetaGpu {
    weight_range: [u32; 4], // weight_start, weight_count, reserved, reserved
    shape_range: [u32; 4],  // delta_start, target_count, vertex_start, vertex_count
}

#[inline]
fn pack_snorm16(v: f32) -> i16 {
    (v.clamp(-1.0, 1.0) * 32767.0).round() as i16
}

#[inline]
fn pack_normal_snorm16x4(normal: [f32; 3]) -> [i16; 4] {
    [
        pack_snorm16(normal[0]),
        pack_snorm16(normal[1]),
        pack_snorm16(normal[2]),
        0,
    ]
}

#[inline]
fn pack_quat_snorm16x4(rotation: [f32; 4]) -> [i16; 4] {
    [
        pack_snorm16(rotation[0]),
        pack_snorm16(rotation[1]),
        pack_snorm16(rotation[2]),
        pack_snorm16(rotation[3]),
    ]
}

#[inline]
fn pack_unorm16_local(value: f32, min: f32, extent: f32) -> u16 {
    if extent.abs() <= f32::EPSILON {
        return 0;
    }
    (((value - min) / extent).clamp(0.0, 1.0) * 65535.0).round() as u16
}

#[inline]
fn pack_snorm8(v: f32) -> i8 {
    (v.clamp(-1.0, 1.0) * 127.0).round() as i8
}

#[inline]
fn pack_normal_snorm8x4(normal: [f32; 3]) -> [i8; 4] {
    [
        pack_snorm8(normal[0]),
        pack_snorm8(normal[1]),
        pack_snorm8(normal[2]),
        0,
    ]
}

#[inline]
fn pack_rigid_mesh_vertex(v: &DecodedMeshVertex) -> RigidMeshVertex {
    RigidMeshVertex {
        pos: v.pos,
        normal: pack_normal_snorm16x4(v.normal),
        uv: v.uv,
        paint_uv: v.paint_uv,
    }
}

#[inline]
fn pack_skinned_mesh_vertex(v: &DecodedMeshVertex) -> SkinnedMeshVertex {
    SkinnedMeshVertex {
        pos: v.pos,
        normal: pack_normal_snorm16x4(v.normal),
        uv: v.uv,
        joints: v.joints,
        weights: v.weights,
        paint_uv: v.paint_uv,
    }
}

#[repr(C)]
#[derive(Clone, Copy, Pod, Zeroable, PartialEq)]
struct FrustumCullParamsGpu {
    planes: [[f32; 4]; 6],
    draw_count: u32,
    _pad: [u32; 3],
}

// Cull item split into a static half (bounds + flags, changes only when batch
// topology changes) and a dynamic half (model rows, changes on every transform
// update). Keeping them in separate storage buffers lets the transform fast
// path re-upload only the model rows.
#[repr(C)]
#[derive(Clone, Copy, Pod, Zeroable)]
struct FrustumCullStaticGpu {
    local_center_radius: [f32; 4],
    cull_flags: [u32; 4],
}

#[repr(C)]
#[derive(Clone, Copy, Pod, Zeroable)]
struct FrustumCullDynamicGpu {
    model_0: [f32; 4],
    model_1: [f32; 4],
    model_2: [f32; 4],
    model_3: [f32; 4],
}

// Per-batch static record consumed by the multimesh cull compute shader.
// Region [instance_start, instance_start+instance_cap) in visible_indices is
// reserved identical to the batch source range so firstInstance stays stable.
#[repr(C)]
#[derive(Clone, Copy, Pod, Zeroable)]
struct MultiMeshCullBatchGpu {
    instance_start: u32,
    instance_cap: u32,
    indirect_index: u32,
    mesh_radius_bits: u32,
}

#[repr(C)]
#[derive(Clone, Copy, Pod, Zeroable, PartialEq)]
struct MultiMeshCullParamsGpu {
    instance_count: u32,
    batch_count: u32,
    _pad1: u32,
    _pad2: u32,
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

// One per SPD downsample dispatch: how many dst mips it writes (1..=HIZ_SPD_MIPS)
// and the dimensions of its source mip (for NPOT edge clamping in the shader).
#[repr(C)]
#[derive(Clone, Copy, Pod, Zeroable, PartialEq)]
struct HizSpdParamsGpu {
    mip_count: u32,
    src_width: u32,
    src_height: u32,
    _pad: u32,
}

#[repr(C)]
#[derive(Clone, Copy, Pod, Zeroable, PartialEq)]
struct ShadowUniform {
    ray_light_view_proj: [[[f32; 4]; 4]; MAX_SHADOW_RAY_CASCADES],
    spot_light_view_proj: [[[f32; 4]; 4]; MAX_SHADOW_SPOT_LIGHTS],
    point_light_view_proj: [[[f32; 4]; 4]; MAX_SHADOW_POINT_LIGHTS * POINT_SHADOW_FACE_COUNT],
    params0: [f32; 4],    // enabled, strength, depth_bias, normal_bias
    ray_params: [f32; 4], // enabled, cascade_count, shadow_distance, reserved
    ray_splits: [f32; 4],
    ray_texel: [f32; 4], // world units per shadow texel, per cascade
    spot_params: [[f32; 4]; MAX_SHADOW_SPOT_LIGHTS], // enabled, light_index, layer, reserved
    point_params: [[f32; 4]; MAX_SHADOW_POINT_LIGHTS], // enabled, light_index, base_layer, range
    // Direct light-index -> shadow-slot lookup so the fragment shaders skip the
    // per-slot search. Each vec4 lane holds one light's slot (or -1.0 for none),
    // indexed by dense light index. MAX_{SPOT,POINT}_LIGHTS lanes total.
    spot_light_slots: [[f32; 4]; MAX_SPOT_LIGHTS.div_ceil(4)],
    point_light_slots: [[f32; 4]; MAX_POINT_LIGHTS.div_ceil(4)],
}

pub struct Gpu3D {
    color_format: wgpu::TextureFormat,
    camera_bgl: wgpu::BindGroupLayout,
    water_camera_bgl: wgpu::BindGroupLayout,
    rigid_camera_bgl: wgpu::BindGroupLayout,
    multimesh_bgl: wgpu::BindGroupLayout,
    material_texture_bgl: wgpu::BindGroupLayout,
    shadow_bgl: wgpu::BindGroupLayout,
    sky_bgl: wgpu::BindGroupLayout,
    material_pipeline_layout: wgpu::PipelineLayout,
    rigid_material_pipeline_layout: wgpu::PipelineLayout,
    multimesh_pipeline_layout: wgpu::PipelineLayout,
    sky_pipeline_layout: wgpu::PipelineLayout,
    sky_pipeline: wgpu::RenderPipeline,
    custom_sky_pipelines: AHashMap<u64, wgpu::RenderPipeline>,
    active_sky_pipeline_key: Option<u64>,
    pipeline_rigid_culled: wgpu::RenderPipeline,
    pipeline_rigid_double_sided: wgpu::RenderPipeline,
    pipeline_rigid_blend_culled: wgpu::RenderPipeline,
    pipeline_rigid_blend_double_sided: wgpu::RenderPipeline,
    pipeline_rigid_packed_lod_culled: wgpu::RenderPipeline,
    pipeline_rigid_packed_lod_double_sided: wgpu::RenderPipeline,
    pipeline_rigid_packed_lod_blend_culled: wgpu::RenderPipeline,
    pipeline_rigid_packed_lod_blend_double_sided: wgpu::RenderPipeline,
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
    pipeline_depth_prepass_rigid_packed_lod_culled: wgpu::RenderPipeline,
    pipeline_depth_prepass_rigid_packed_lod_double_sided: wgpu::RenderPipeline,
    pipeline_shadow_depth_culled: wgpu::RenderPipeline,
    pipeline_shadow_depth_double_sided: wgpu::RenderPipeline,
    pipeline_shadow_depth_rigid_culled: wgpu::RenderPipeline,
    pipeline_shadow_depth_rigid_double_sided: wgpu::RenderPipeline,
    pipeline_shadow_depth_rigid_packed_lod_culled: wgpu::RenderPipeline,
    pipeline_shadow_depth_rigid_packed_lod_double_sided: wgpu::RenderPipeline,
    pipeline_multimesh_culled: wgpu::RenderPipeline,
    pipeline_multimesh_double_sided: wgpu::RenderPipeline,
    pipeline_multimesh_blend_culled: wgpu::RenderPipeline,
    pipeline_multimesh_blend_double_sided: wgpu::RenderPipeline,
    pipeline_multimesh_mask_culled: wgpu::RenderPipeline,
    pipeline_multimesh_mask_double_sided: wgpu::RenderPipeline,
    // Prepass-covered variants (depth write off, LessEqual) used when unified
    // depth is active and the batch was primed in the depth prepass.
    pipeline_multimesh_covered: wgpu::RenderPipeline,
    pipeline_multimesh_covered_double_sided: wgpu::RenderPipeline,
    // Depth-only prepass pipelines for multimesh (Depth32Float, vertex only).
    pipeline_multimesh_depth_prepass_culled: wgpu::RenderPipeline,
    pipeline_multimesh_depth_prepass_double_sided: wgpu::RenderPipeline,
    // Shadow-depth pipelines for multimesh (biased, into a shadow layer).
    pipeline_multimesh_shadow_depth_culled: wgpu::RenderPipeline,
    pipeline_multimesh_shadow_depth_double_sided: wgpu::RenderPipeline,
    pipeline_mask_rigid_culled: wgpu::RenderPipeline,
    pipeline_mask_rigid_double_sided: wgpu::RenderPipeline,
    pipeline_mask_rigid_packed_lod_culled: wgpu::RenderPipeline,
    pipeline_mask_rigid_packed_lod_double_sided: wgpu::RenderPipeline,
    pipeline_mask_skinned_culled: wgpu::RenderPipeline,
    pipeline_mask_skinned_double_sided: wgpu::RenderPipeline,
    custom_pipelines: AHashMap<u32, CustomPipeline>,
    custom_pipelines_rigid: AHashMap<u32, CustomPipeline>,
    custom_pipelines_multimesh: AHashMap<u32, CustomPipeline>,
    custom_pipeline_tokens: AHashMap<u64, u32>,
    // Per custom-pipeline token: does the shader define a shade_vertex hook?
    // The shared depth-only passes (shadow depth + depth prepass) cannot run
    // the hook, so batch classification consults this to decide whether a
    // custom batch is depth-safe (see batch_depth_safe in draw.rs).
    custom_pipeline_vertex_hooks: AHashMap<u32, bool>,
    next_custom_pipeline_token: u32,
    camera_buffer: wgpu::Buffer,
    camera_bind_group: wgpu::BindGroup,
    water_camera_bind_group: wgpu::BindGroup,
    rigid_camera_bind_group: wgpu::BindGroup,
    shadow_camera_buffers: Vec<wgpu::Buffer>,
    shadow_camera_bind_groups: Vec<wgpu::BindGroup>,
    rigid_shadow_camera_bind_groups: Vec<wgpu::BindGroup>,
    // Per shadow layer multimesh draw bind groups: same layout as
    // multimesh_bgl but binding 0 = that layer's scene uniform and binding 8 =
    // the dedicated identity index buffer (never overwritten by camera cull).
    shadow_multimesh_bind_groups: Vec<wgpu::BindGroup>,
    // Identity visible-index buffer for the multimesh shadow path. Grown to the
    // multimesh instance count; never touched by the GPU cull compute pass.
    multimesh_shadow_identity_buffer: wgpu::Buffer,
    multimesh_shadow_identity_capacity: usize,
    shadow_buffer: wgpu::Buffer,
    shadow_bind_group: wgpu::BindGroup,
    _shadow_map_texture: wgpu::Texture,
    _shadow_map_view: wgpu::TextureView,
    shadow_layer_views: Vec<wgpu::TextureView>,
    _spot_shadow_map_texture: wgpu::Texture,
    _spot_shadow_map_view: wgpu::TextureView,
    spot_shadow_layer_views: Vec<wgpu::TextureView>,
    _point_shadow_map_texture: wgpu::Texture,
    _point_shadow_map_view: wgpu::TextureView,
    point_shadow_layer_views: Vec<wgpu::TextureView>,
    _shadow_map_sampler: wgpu::Sampler,
    mesh_blend_bgl: wgpu::BindGroupLayout,
    mesh_blend_bind_group: wgpu::BindGroup,
    // Screen-space mesh blend (seam pass) state.
    screen_blend_supported: bool,
    mesh_blend_screen_active: bool,
    mesh_blend_mask_batch_entries: Vec<MeshBlendMaskEntry>,
    _mesh_blend_mask_texture: wgpu::Texture,
    mesh_blend_mask_view: wgpu::TextureView,
    mesh_blend_mask_id_bgl: wgpu::BindGroupLayout,
    mesh_blend_mask_id_buffer: wgpu::Buffer,
    mesh_blend_mask_id_bind_group: wgpu::BindGroup,
    mesh_blend_mask_id_capacity: u64,
    mesh_blend_params_buffer: wgpu::Buffer,
    mesh_blend_seam_bgl: wgpu::BindGroupLayout,
    mesh_blend_seam_pipeline: wgpu::RenderPipeline,
    mesh_blend_seam_bind_group: Option<wgpu::BindGroup>,
    mesh_blend_scene_copy: Option<(wgpu::Texture, wgpu::TextureView)>,
    sky_buffer: wgpu::Buffer,
    sky_bind_group: wgpu::BindGroup,
    _sky_noise_texture: wgpu::Texture,
    _sky_noise_view: wgpu::TextureView,
    _sky_noise_sampler: wgpu::Sampler,
    skeleton_buffer: wgpu::Buffer,
    skeleton_capacity: usize,
    // Packed bone palettes: 3 affine rows per bone (w row is implicit 0,0,0,1
    // and never read by the skinning shaders).
    staged_skeletons: Vec<[[f32; 4]; 3]>,
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
    // texture slots (= texture index) backing stream sources (webcam/video):
    // built single-level so per-frame base writes update in place.
    stream_texture_slots: AHashSet<u32>,
    material_texture_bind_groups: AHashMap<MaterialTextureKey, wgpu::BindGroup>,
    custom_material_texture_slots: AHashMap<u64, u32>,
    next_custom_material_texture_slot: u32,
    texture_filter: TextureFilterMode,
    instance_transform_buffer: wgpu::Buffer,
    instance_transform_capacity: usize,
    staged_instance_transforms: Vec<TransformInstanceGpu>,
    rigid_instance_meta_buffer: wgpu::Buffer,
    rigid_instance_meta_capacity: usize,
    staged_rigid_instance_meta: Vec<RigidInstanceMetaGpu>,
    skinned_instance_meta_buffer: wgpu::Buffer,
    skinned_instance_meta_capacity: usize,
    staged_skinned_instance_meta: Vec<SkinnedInstanceMetaGpu>,
    blend_shape_delta_buffer: wgpu::Buffer,
    blend_shape_delta_capacity: usize,
    blend_shape_deltas: Vec<BlendShapeDeltaGpu>,
    blend_shape_weight_buffer: wgpu::Buffer,
    blend_shape_weight_capacity: usize,
    staged_blend_shape_weights: Vec<f32>,
    blend_shape_instance_meta_buffer: wgpu::Buffer,
    blend_shape_instance_meta_capacity: usize,
    staged_blend_shape_instance_meta: Vec<BlendShapeInstanceMetaGpu>,
    packed_lod_param_buffer: wgpu::Buffer,
    packed_lod_param_capacity: usize,
    packed_lod_params: Vec<PackedLodParamGpu>,
    decal_buffer: wgpu::Buffer,
    decal_buffer_capacity: usize,
    decal_texture: wgpu::Texture,
    decal_texture_view: wgpu::TextureView,
    decal_texture_layers: u32,
    decal_sampler: wgpu::Sampler,
    decal_layer_by_texture: AHashMap<perro_ids::TextureID, u32>,
    decal_sources_pending: bool,
    decal_count: u32,
    last_decals_revision: u64,
    multimesh_bind_group: wgpu::BindGroup,
    multimesh_draw_params_buffer: wgpu::Buffer,
    multimesh_draw_params_capacity: usize,
    staged_multimesh_draw_params: Vec<MultiMeshDrawParamGpu>,
    multimesh_instance_buffer: wgpu::Buffer,
    multimesh_instance_capacity: usize,
    staged_multimesh_instances: Vec<MultiMeshInstanceGpu>,
    multimesh_batches: Vec<MultiMeshBatch>,
    // GPU per-instance frustum cull for multimesh (item 1). Reuses the rigid
    // frustum params buffer for the planes. When inactive, visible_indices is
    // primed as identity so the same storage-fetch draw path is correct.
    multimesh_cull_pipeline: wgpu::ComputePipeline,
    multimesh_cull_finalize_pipeline: wgpu::ComputePipeline,
    // Second cull phase (frustum + hi-z) run after the depth prepass builds
    // this frame's pyramid; recompacts the same visible-index/indirect buffers
    // before the main pass reads them.
    multimesh_cull_hiz_pipeline: wgpu::ComputePipeline,
    multimesh_cull_bgl: wgpu::BindGroupLayout,
    multimesh_cull_bind_group: wgpu::BindGroup,
    multimesh_cull_params_buffer: wgpu::Buffer,
    // Per-instance batch id (index into multimesh_cull_batches).
    multimesh_instance_batch_buffer: wgpu::Buffer,
    staged_multimesh_instance_batch: Vec<u32>,
    // Per-batch cull records.
    multimesh_cull_batch_buffer: wgpu::Buffer,
    staged_multimesh_cull_batches: Vec<MultiMeshCullBatchGpu>,
    // Compacted / identity visible-instance indices consumed by the vertex shader.
    multimesh_visible_index_buffer: wgpu::Buffer,
    staged_multimesh_visible_identity: Vec<u32>,
    // Per-batch atomic append counters (u32 each), cleared each cull frame.
    multimesh_cull_counter_buffer: wgpu::Buffer,
    // Per-batch DrawIndexedIndirect records (instance_count written by cull).
    multimesh_indirect_buffer: wgpu::Buffer,
    multimesh_indirect_staging: Vec<DrawIndexedIndirectGpu>,
    // Shared capacity (instances) for visible_indices + instance_batch buffers.
    multimesh_cull_instance_capacity: usize,
    // Shared capacity (batches) for cull_batches + counters + indirect buffers.
    multimesh_cull_batch_capacity: usize,
    // True while the cull compute ran this frame (drives indirect draw path).
    multimesh_cull_active: bool,
    last_multimesh_cull_params: Option<MultiMeshCullParamsGpu>,
    frustum_cull_enabled: bool,
    frustum_cull_supported: bool,
    // When set, surviving culled draws are issued via multi_draw_indexed_indirect
    // per state group instead of one draw_indexed_indirect per batch.
    multi_draw_indirect_enabled: bool,
    frustum_cull_pipeline: wgpu::ComputePipeline,
    frustum_cull_bgl: wgpu::BindGroupLayout,
    frustum_cull_bind_group: wgpu::BindGroup,
    frustum_cull_params_buffer: wgpu::Buffer,
    frustum_cull_static_buffer: wgpu::Buffer,
    frustum_cull_dynamic_buffer: wgpu::Buffer,
    frustum_cull_items_capacity: usize,
    frustum_cull_static_staging: Vec<FrustumCullStaticGpu>,
    frustum_cull_dynamic_staging: Vec<FrustumCullDynamicGpu>,
    indirect_buffer: wgpu::Buffer,
    indirect_capacity: usize,
    indirect_staging: Vec<DrawIndexedIndirectGpu>,
    frustum_gpu_inputs_valid: bool,
    last_frustum_params: Option<FrustumCullParamsGpu>,
    last_hiz_params: Option<HizCullParamsGpu>,
    last_prepare_step_timing: Prepare3DStepTiming,
    draw_batches: Vec<DrawBatch>,
    opaque_batch_indices: Vec<usize>,
    alpha_batch_indices: Vec<usize>,
    mesh_blend_batch_indices: Vec<usize>,
    overlay_batch_indices: Vec<usize>,
    shadow_batch_indices: Vec<usize>,
    depth_prepass_batch_indices: Vec<usize>,
    mesh_blend_depth_batch_indices: Vec<usize>,
    has_shadow_casters: bool,
    mesh_blend_depth_active: bool,
    surface_entries_scratch: Vec<SurfaceEntry3D>,
    mesh_blend_scratch: Vec<ResolvedMeshBlend>,
    bleed_emitters_scratch: Vec<prepare::BleedEmitter>,
    bleed_occluders_scratch: Vec<prepare::BleedOccluder>,
    bleed_multimesh_bounds_scratch: Vec<Option<(Vec3, f32)>>,
    // Per mesh-blend source: (source batch index, range into
    // mesh_blend_receiver_indices). Precomputed in prepare so the source
    // depth passes skip the O(N) receiver scan.
    mesh_blend_source_receivers: Vec<(usize, Range<usize>)>,
    mesh_blend_receiver_indices: Vec<usize>,
    // Per draw batch: merged world sphere, cached once per receiver rebuild so
    // the source x target overlap loop never recomputes a batch's O(instances)
    // sphere per source. Reused across calls.
    mesh_blend_batch_spheres_scratch: Vec<Option<(Vec3, f32)>>,
    // Batch world spheres as of the last receiver-list build. On a transform-only
    // frame the receiver lists can be reused when no blend-relevant batch sphere
    // moved vs this snapshot (see rebuild_mesh_blend_receivers).
    mesh_blend_prev_spheres: Vec<Option<(Vec3, f32)>>,
    last_draws: Vec<Draw3DInstance>,
    last_draws_revision: u64,
    last_draw_instance_spans: Vec<Range<u32>>,
    last_draw_instance_span_ranges: Vec<Range<usize>>,
    // Per draw (build order): range of staged_multimesh_draw_params slots that
    // draw produced. Lets the transform-only fast path patch only the moved
    // multimesh draw's model rows w/o re-staging any instances.
    last_draw_multimesh_param_ranges: Vec<Range<u32>>,
    // Persistent scratch for compact_sorted_draw_batches: avoids a fresh alloc
    // per full rebuild. dst_* buffers are double-buffered via mem::swap with
    // the live staged_* vectors (old contents overwritten by extend_from_slice
    // after `clear`, capacity reused). spans_per_draw's inner Vecs are cleared
    // and reused in place rather than reallocated each rebuild.
    compact_instance_owner_scratch: Vec<u32>,
    compact_dst_transforms_scratch: Vec<TransformInstanceGpu>,
    compact_dst_rigid_meta_scratch: Vec<RigidInstanceMetaGpu>,
    compact_dst_skinned_meta_scratch: Vec<SkinnedInstanceMetaGpu>,
    compact_dst_batches_scratch: Vec<DrawBatch>,
    compact_spans_per_draw_scratch: Vec<Vec<Range<u32>>>,
    // Dedup for compact_sorted_draw_batches: meshlet batches of one draw share
    // one instance span, so the copy loop must copy that src region once and
    // repoint later batches at the existing dst region (else the shared span is
    // re-duplicated per meshlet, defeating the sharing). Maps an exact copied
    // src region (start, end) to its dst_start. Shared spans are always identical
    // across sharing batches, never partially overlapping.
    compact_src_region_dedup_scratch: AHashMap<(u32, u32), u32>,
    // Persistent scratch for compact_sorted_multimesh_batches (same
    // double-buffer swap pattern).
    compact_multimesh_dst_instances_scratch: Vec<MultiMeshInstanceGpu>,
    compact_multimesh_dst_batches_scratch: Vec<MultiMeshBatch>,
    // Item 3: skip pack_quat_snorm16x4 on full rebuilds for pose lists whose Arc
    // is unchanged. Keyed by pose-Arc pointer; holds the build-independent
    // geometry lanes (position/rotation/scale). draw_id + blend_meta_id are
    // build-order specific and stay recomputed per rebuild. The source Arc is
    // pinned in the value so the key pointer can never be reused by a different
    // allocation (ABA-safe) while the entry lives.
    multimesh_pose_pack_cache: AHashMap<usize, PosePackCacheEntry>,
    multimesh_pose_pack_cache_seen: ahash::AHashSet<usize>,
    last_scene: Option<Scene3DUniform>,
    last_shadow_scenes: Vec<Option<Scene3DUniform>>,
    // Frustum planes per shadow camera (cascade/spot/point-face), derived from
    // the shadow scenes each frame so shadow draws can CPU-cull off-view batches.
    shadow_camera_frustums: Vec<[Vec4; 6]>,
    last_shadow: Option<ShadowUniform>,
    // Per shadow layer (flat cascade/spot/point-face index) cache validity. A
    // valid layer retains prior depth contents and skips its render pass.
    shadow_layer_valid: Vec<bool>,
    // Set when any shadow-caster geometry moved this frame (full rebuild,
    // transform patch, or multimesh instance/pose upload). Invalidates all
    // shadow layers in update_shadow_state.
    shadow_casters_dirty: bool,
    // Scratch surviving-batch indices for the per-layer shadow caster cull.
    shadow_cull_scratch: Vec<usize>,
    shadow_pass_enabled: bool,
    ray_shadow_enabled: bool,
    spot_shadow_count: usize,
    point_shadow_count: usize,
    shadow_focus_center: Vec3,
    shadow_focus_radius: f32,
    last_sky: Option<SkyUniform>,
    last_sky_time_seconds: f32,
    sky_enabled: bool,
    mesh_vertices: Vec<SkinnedMeshVertex>,
    rigid_mesh_vertices: Vec<RigidMeshVertex>,
    packed_lod_vertices: Vec<PackedRigidLodVertex>,
    mesh_indices: Vec<u32>,
    packed_lod_indices: Vec<u32>,
    vertex_buffer: wgpu::Buffer,
    rigid_vertex_buffer: wgpu::Buffer,
    packed_lod_vertex_buffer: wgpu::Buffer,
    index_buffer: wgpu::Buffer,
    packed_lod_index_buffer: wgpu::Buffer,
    vertex_capacity: usize,
    rigid_vertex_capacity: usize,
    packed_lod_vertex_capacity: usize,
    index_capacity: usize,
    packed_lod_index_capacity: usize,
    builtin_mesh_ranges: AHashMap<&'static str, MeshRange>,
    builtin_mesh_bounds: AHashMap<&'static str, ([f32; 3], f32)>,
    builtin_meshlets: AHashMap<&'static str, Arc<[MeshletRange]>>,
    custom_mesh_ranges: AHashMap<MeshID, (u64, MeshAssetRange)>,
    depth_texture: wgpu::Texture,
    depth_view: wgpu::TextureView,
    depth_prepass_texture: wgpu::Texture,
    depth_prepass_view: wgpu::TextureView,
    ssao_pass: Option<ssao::SsaoPass>,
    _ssao_fallback_texture: wgpu::Texture,
    ssao_fallback_view: wgpu::TextureView,
    ssao_quality: crate::SsaoQuality,
    mesh_blend_depth_texture: wgpu::Texture,
    mesh_blend_depth_view: wgpu::TextureView,
    depth_size: (u32, u32),
    // True while encoding a frame whose main pass loads the prepass depth
    // (sample_count == 1 and the prepass ran) instead of re-rasterizing it.
    unified_depth_active: bool,
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
    // Single-pass downsampler (SPD): one dispatch per HIZ_SPD_MIPS-mip chunk,
    // replacing the per-mip serialized pass loop. `hiz_spd_supported` gates it on
    // the device having enough storage textures per stage; when false the old
    // per-mip path (hiz_downsample_bind_groups) runs instead.
    hiz_spd_supported: bool,
    hiz_spd_pipeline: wgpu::ComputePipeline,
    hiz_spd_bgl: wgpu::BindGroupLayout,
    // One entry per SPD dispatch. `_buffers` holds the per-dispatch uniforms so
    // they outlive the bind groups that reference them.
    hiz_spd_bind_groups: Vec<wgpu::BindGroup>,
    hiz_spd_params_buffers: Vec<wgpu::Buffer>,
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
    shadow_caster_debug_view: bool,
    disable_meshlet_shadows: bool,
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
    transform_only_kinds_scratch: Vec<draw::TransformOnlyDrawKind>,
    debug_point_instances_scratch: Vec<BuiltInstanceParts>,
    debug_edge_instances_scratch: Vec<BuiltInstanceParts>,
    camera_bind_group_generation: u32,
    multimesh_bind_group_generation: u32,
    perf_counters: RenderPerfCounters,
}

pub struct Prepare3D<'a> {
    pub resources: &'a ResourceStore,
    pub camera: Camera3DState,
    pub lighting: &'a Lighting3DState,
    pub draws: &'a [Draw3DInstance],
    pub draws_revision: u64,
    pub force_full_rebuild: bool,
    pub decals: &'a [(perro_ids::NodeID, perro_render_bridge::Decal3DState)],
    pub decals_revision: u64,
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
    pub ssao: crate::SsaoQuality,
    pub indirect_first_instance_enabled: bool,
    pub multi_draw_indirect_enabled: bool,
    pub texture_filter: TextureFilterMode,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
enum RenderBatchKind {
    Opaque,
    Alpha,
    MeshBlend,
    Overlay,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct RenderStateKey {
    pipeline_key: u64,
    texture_slot: u64,
    mesh_index_start: u32,
    mesh_base_vertex: i32,
    batch_kind: RenderBatchKind,
}

#[derive(Clone, Copy, Debug, Default)]
struct RenderPerfCounters {
    pipeline_switches: u32,
    texture_bind_group_switches: u32,
    camera_bind_group_switches: u32,
    draw_batches: u32,
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
    blend_shape_delta_start: u32,
    blend_shape_target_count: u32,
    blend_shape_vertex_start: u32,
    blend_shape_vertex_count: u32,
}

#[derive(Clone)]
struct MeshLodRange {
    full: MeshRange,
    surface_ranges: Arc<[MeshRange]>,
    meshlets: Arc<[MeshletRange]>,
    packed: Option<PackedMeshLodRange>,
}

#[derive(Clone)]
struct PackedMeshLodRange {
    full: MeshRange,
    surface_ranges: Arc<[MeshRange]>,
    param_index: u32,
}

struct MeshLodView<'a> {
    full: MeshRange,
    surface_ranges: &'a [MeshRange],
    meshlets: &'a [MeshletRange],
    packed: Option<&'a PackedMeshLodRange>,
}

#[derive(Clone)]
struct DrawBatch {
    state_key: u64,
    render_state: RenderStateKey,
    mesh: MeshRange,
    instance_start: u32,
    instance_count: u32,
    path: RenderPath3D,
    packed_lod: bool,
    double_sided: bool,
    material_kind: MaterialPipelineKind,
    alpha_mode: u8,
    draw_on_top: bool,
    base_color_texture_slot: u32,
    material_texture_key: MaterialTextureKey,
    local_center: [f32; 3],
    local_radius: f32,
    occlusion_query: Option<u32>,
    disable_hiz_occlusion: bool,
    casts_shadows: bool,
    receives_shadows: bool,
    mesh_blend: bool,
    mesh_blend_screen: bool,
    mesh_blend_params: u32,
    mesh_blend_depth: bool,
    blend_layers: u32,
    blend_mask: u32,
    order_index: u32,
}

#[derive(Clone)]
struct SurfaceEntry3D {
    range: MeshRange,
    packed_range: Option<MeshRange>,
    packed_lod_param_id: u32,
    material: Material3D,
    modulate_bias: bool,
}

#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Debug)]
enum RenderPath3D {
    Rigid,
    Skinned,
    MultiMesh,
}

#[derive(Clone)]
struct MultiMeshBatch {
    mesh: MeshRange,
    instance_start: u32,
    instance_count: u32,
    draw_param_index: u32,
    // Mesh local-bounds radius, used by the GPU cull to build instance spheres.
    mesh_local_radius: f32,
    double_sided: bool,
    mesh_blend: bool,
    mesh_blend_screen: bool,
    mesh_blend_params: u32,
    mesh_blend_depth: bool,
    blend_layers: u32,
    blend_mask: u32,
    casts_shadows: bool,
    material_kind: MaterialPipelineKind,
}

#[derive(Clone, Copy)]
enum MeshBlendMaskEntry {
    Draw { batch_index: usize, id: u32 },
    MultiMesh { batch_index: usize, id: u32 },
}

#[derive(Clone, Debug, PartialEq, Eq, Hash)]
enum MaterialPipelineKind {
    Standard,
    Unlit,
    Toon,
    Custom(u32),
}

impl MaterialPipelineKind {
    #[inline]
    fn uses_custom_shader(&self) -> bool {
        matches!(self, MaterialPipelineKind::Custom(_))
    }
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
// Below this total instance count, direct multimesh draw is cheaper than a
// compute cull pass (dispatch + readback overhead outweighs the vertex savings).
const MULTIMESH_CULL_MIN_INSTANCES: usize = 1024;
const DEPTH_PREPASS_MIN_BATCHES: usize = 32;
const DEPTH_PREPASS_MIN_INSTANCES: usize = 512;
const HIZ_DEBUG_READBACK_ENABLED: bool = false;
// Re-test occluded batches every frame so visibility recovers immediately when camera/object moves.
const OCCLUSION_PROBE_INTERVAL: u64 = 1;

#[cfg(test)]
mod tests {
    use super::{
        DrawBatch, DrawBatchPush, MATERIAL_TEXTURE_NONE, MaterialInstanceGpu, MaterialPipelineKind,
        MaterialTextureKey, MultiMeshDrawParamGpu, MultiMeshInstanceGpu, PackedLodParamGpu,
        PackedRigidLodVertex, RenderBatchKind, RenderPath3D, RigidInstanceMetaGpu, RigidMeshVertex,
        SkinnedInstanceMetaGpu, SkinnedMeshVertex, camera, compare_draw_batch_keys,
        draw_batch_state_key, push_draw_batch, render_state_key,
    };
    use glam::{Mat4, Quat, Vec3, Vec4};
    use perro_asset_formats::pmesh::{
        FLAG_HAS_JOINTS as PMESH_FLAG_HAS_JOINTS, FLAG_HAS_NORMAL as PMESH_FLAG_HAS_NORMAL,
        FLAG_HAS_UV0 as PMESH_FLAG_HAS_UV0, FLAG_HAS_WEIGHTS as PMESH_FLAG_HAS_WEIGHTS,
        FLAG_WEIGHTS_UNORM8 as PMESH_FLAG_WEIGHTS_UNORM8, VERSION_V1 as PMESH_VERSION_V1,
    };
    use perro_graphics_assets::{MeshRange, decode_pmesh, decode_ptex};
    use perro_render_bridge::CameraProjectionState;
    use perro_structs::BitMask;

    fn assert_approx(actual: f32, expected: f32) {
        assert!(
            (actual - expected).abs() <= 1.0e-4,
            "expected {expected}, got {actual}"
        );
    }

    fn projected_depth(proj: glam::Mat4, view_z: f32) -> f32 {
        let clip = proj * Vec4::new(0.0, 0.0, view_z, 1.0);
        clip.z / clip.w
    }

    fn next_unit(seed: &mut u32) -> f32 {
        *seed ^= *seed << 13;
        *seed ^= *seed >> 17;
        *seed ^= *seed << 5;
        (*seed as f32) / (u32::MAX as f32)
    }

    fn next_range(seed: &mut u32, min: f32, max: f32) -> f32 {
        min + (max - min) * next_unit(seed)
    }

    fn next_vec3(seed: &mut u32, min: f32, max: f32) -> Vec3 {
        Vec3::new(
            next_range(seed, min, max),
            next_range(seed, min, max),
            next_range(seed, min, max),
        )
    }

    #[test]
    fn packed_gpu_layouts_keep_expected_sizes() {
        assert_eq!(std::mem::size_of::<RigidMeshVertex>(), 36);
        assert_eq!(std::mem::size_of::<PackedRigidLodVertex>(), 24);
        assert_eq!(std::mem::size_of::<PackedLodParamGpu>(), 48);
        assert_eq!(std::mem::size_of::<SkinnedMeshVertex>(), 48);
        assert_eq!(std::mem::size_of::<MultiMeshInstanceGpu>(), 40);
        assert_eq!(std::mem::size_of::<MultiMeshDrawParamGpu>(), 80);
        assert_eq!(std::mem::size_of::<MaterialInstanceGpu>(), 20);
        assert_eq!(std::mem::size_of::<RigidInstanceMetaGpu>(), 32);
        assert_eq!(std::mem::size_of::<SkinnedInstanceMetaGpu>(), 36);
        assert_eq!(
            std::mem::offset_of!(RigidMeshVertex, normal),
            12,
            "rigid normal attr offset"
        );
        assert_eq!(
            std::mem::offset_of!(RigidMeshVertex, uv),
            20,
            "rigid uv attr offset"
        );
        assert_eq!(
            std::mem::offset_of!(SkinnedMeshVertex, joints),
            28,
            "skinned joints attr offset"
        );
        assert_eq!(
            std::mem::offset_of!(SkinnedMeshVertex, weights),
            36,
            "skinned weights attr offset"
        );
        assert_eq!(std::mem::offset_of!(RigidMeshVertex, paint_uv), 28);
        assert_eq!(std::mem::offset_of!(SkinnedMeshVertex, paint_uv), 40);
        assert_eq!(
            std::mem::offset_of!(MultiMeshInstanceGpu, scale),
            20,
            "multimesh scale attr offset"
        );
        assert_eq!(
            std::mem::offset_of!(MultiMeshInstanceGpu, draw_id),
            32,
            "multimesh draw id attr offset"
        );
        assert_eq!(
            std::mem::offset_of!(MultiMeshInstanceGpu, blend_meta_id),
            36,
            "multimesh blend meta attr offset"
        );
    }

    #[test]
    fn multimesh_vertex_layout_matches_gpu_structs() {
        let mesh_layout = super::multimesh_path::multimesh_mesh_vertex_layout();
        // Stride tracks RigidMeshVertex: pos(12) + normal(8) + uv(8) + paint_uv(8).
        assert_eq!(mesh_layout.array_stride, 36);
        assert_eq!(mesh_layout.attributes[0].offset, 0);
        assert_eq!(mesh_layout.attributes[0].shader_location, 0);
        assert_eq!(
            mesh_layout.attributes[0].format,
            wgpu::VertexFormat::Float32x3
        );
        assert_eq!(mesh_layout.attributes[1].offset, 12);
        assert_eq!(mesh_layout.attributes[1].shader_location, 1);
        assert_eq!(
            mesh_layout.attributes[1].format,
            wgpu::VertexFormat::Snorm16x4
        );
        // Instance data is now fetched from storage (GPU cull compaction), so it
        // is no longer a vertex buffer layout. The storage struct layout is
        // asserted by packed_gpu_layouts_keep_expected_sizes.
    }

    #[test]
    fn multimesh_instance_transform_matches_single_mesh_model_for_random_instances() {
        let node_model = Mat4::from_scale_rotation_translation(
            Vec3::new(1.4, 0.75, 1.1),
            Quat::from_euler(glam::EulerRot::XYZ, 0.23, -0.41, 0.17),
            Vec3::new(4.0, -2.0, 8.0),
        );
        let mut seed = 0x51d3_7a91u32;
        let instance_scale = 1.35;
        let local_points = [
            Vec3::new(-1.0, -0.5, 0.25),
            Vec3::new(0.35, 0.9, -0.75),
            Vec3::new(1.25, -0.2, 0.8),
        ];

        for _ in 0..128 {
            let position = next_vec3(&mut seed, -12.0, 12.0);
            let scale = next_vec3(&mut seed, 0.1, 3.0);
            let axis = next_vec3(&mut seed, -1.0, 1.0).normalize_or_zero();
            let axis = if axis.length_squared() > 0.0 {
                axis
            } else {
                Vec3::Y
            };
            let rotation = Quat::from_axis_angle(
                axis,
                next_range(&mut seed, -std::f32::consts::PI, std::f32::consts::PI),
            );
            let single_model = node_model
                * Mat4::from_scale_rotation_translation(scale * instance_scale, rotation, position);

            for point in local_points {
                let multimesh_local =
                    rotation.mul_vec3(point * (scale * instance_scale)) + position;
                let multimesh_world = node_model.transform_point3(multimesh_local);
                let single_world = single_model.transform_point3(point);
                assert!(
                    multimesh_world.distance(single_world) <= 0.0001,
                    "multimesh {multimesh_world:?} single {single_world:?}"
                );
            }
        }
    }

    #[test]
    fn perspective_fov_y_stays_vertical_at_reference_aspect() {
        let proj = camera::projection_matrix(
            CameraProjectionState::Perspective {
                fov_y_degrees: 60.0,
                near: 0.1,
                far: 1000.0,
            },
            16.0 / 9.0,
        );
        let fov = (2.0 * (1.0 / proj.y_axis.y).atan()).to_degrees();
        assert_approx(fov, 60.0);
    }

    #[test]
    fn perspective_fov_y_stays_vertical_at_ultrawide_aspect() {
        let proj = camera::projection_matrix(
            CameraProjectionState::Perspective {
                fov_y_degrees: 60.0,
                near: 0.1,
                far: 1000.0,
            },
            1864.0 / 768.0,
        );
        let fov = (2.0 * (1.0 / proj.y_axis.y).atan()).to_degrees();
        assert_approx(fov, 60.0);
    }

    #[test]
    fn perspective_depth_maps_to_wgpu_range() {
        let near = 0.1;
        let far = 1000.0;
        let proj = camera::projection_matrix(
            CameraProjectionState::Perspective {
                fov_y_degrees: 60.0,
                near,
                far,
            },
            16.0 / 9.0,
        );
        assert_approx(projected_depth(proj, -near), 0.0);
        assert_approx(projected_depth(proj, -far), 1.0);
    }

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
        raw.extend_from_slice(&[26u8, 51, 77, 101]);
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
        bytes.extend_from_slice(&PMESH_VERSION_V1.to_le_bytes());
        let flags = PMESH_FLAG_HAS_NORMAL
            | PMESH_FLAG_HAS_UV0
            | PMESH_FLAG_HAS_JOINTS
            | PMESH_FLAG_HAS_WEIGHTS
            | PMESH_FLAG_WEIGHTS_UNORM8;
        bytes.extend_from_slice(&flags.to_le_bytes());
        bytes.extend_from_slice(&1u32.to_le_bytes());
        bytes.extend_from_slice(&3u32.to_le_bytes());
        bytes.extend_from_slice(&1u32.to_le_bytes());
        bytes.extend_from_slice(&1u32.to_le_bytes());
        bytes.extend_from_slice(&0u32.to_le_bytes());
        bytes.extend_from_slice(&(raw.len() as u32).to_le_bytes());
        bytes.extend_from_slice(&0u32.to_le_bytes());
        bytes.extend_from_slice(&compressed);

        let decoded = decode_pmesh(&bytes).expect("decode v1 pmesh");
        assert_eq!(decoded.vertices.len(), 1);
        assert_eq!(decoded.indices, vec![0, 0, 0]);
        assert_eq!(decoded.vertices[0].pos, [1.0, 2.0, 3.0]);
        assert_eq!(decoded.vertices[0].normal, [0.0, 1.0, 0.0]);
        assert_eq!(decoded.vertices[0].uv, [0.25, 0.75]);
        assert_eq!(decoded.vertices[0].joints, [4, 5, 6, 7]);
        assert_eq!(decoded.vertices[0].weights.to_u8(), [26, 51, 77, 101]);
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
    fn decode_pmesh_rejects_unknown_versions() {
        for version in [3u32, 4, 5, 6, 7, 8] {
            let mut bytes = Vec::new();
            bytes.extend_from_slice(b"PMESH");
            bytes.extend_from_slice(&version.to_le_bytes());
            bytes.resize(33, 0);
            assert!(
                decode_pmesh(&bytes).is_none(),
                "unknown pmesh version {version} must reject"
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
                packed_lod: false,
                material_kind: MaterialPipelineKind::Standard,
                alpha_mode: 0,
                base_color_texture_slot: MATERIAL_TEXTURE_NONE,
                material_texture_key: MaterialTextureKey::from_base(MATERIAL_TEXTURE_NONE),
                local_bounds: ([1.0, 2.0, 3.0], 2.0),
                occlusion_query: None,
                disable_hiz_occlusion: false,
                casts_shadows: true,
                receives_shadows: true,
                mesh_blend: false,
                mesh_blend_screen: false,
                mesh_blend_params: 0,
                mesh_blend_depth: false,
                blend_layers: BitMask::ALL.bits(),
                blend_mask: BitMask::NONE.bits(),
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
                packed_lod: false,
                material_kind: MaterialPipelineKind::Standard,
                alpha_mode: 0,
                base_color_texture_slot: MATERIAL_TEXTURE_NONE,
                material_texture_key: MaterialTextureKey::from_base(MATERIAL_TEXTURE_NONE),
                local_bounds: ([9.0, 9.0, 9.0], 4.0),
                occlusion_query: None,
                disable_hiz_occlusion: false,
                casts_shadows: true,
                receives_shadows: true,
                mesh_blend: false,
                mesh_blend_screen: false,
                mesh_blend_params: 0,
                mesh_blend_depth: false,
                blend_layers: BitMask::ALL.bits(),
                blend_mask: BitMask::NONE.bits(),
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
                false,
                &MaterialPipelineKind::Standard
            )
        );
        // Merged bound is the tight enclosing sphere of both local bounds, not
        // an infinite sentinel; hi-z stays enabled (world sphere emitted at
        // cull upload handles multi-instance correctness).
        let center = Vec3::from(merged.local_center);
        let radius = merged.local_radius;
        assert!(radius < 1.0e8);
        assert!(center.distance(Vec3::new(1.0, 2.0, 3.0)) + 2.0 <= radius + 1.0e-3);
        assert!(center.distance(Vec3::new(9.0, 9.0, 9.0)) + 4.0 <= radius + 1.0e-3);
        assert!(!merged.disable_hiz_occlusion);
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
                packed_lod: false,
                material_kind: MaterialPipelineKind::Standard,
                alpha_mode: 0,
                base_color_texture_slot: MATERIAL_TEXTURE_NONE,
                material_texture_key: MaterialTextureKey::from_base(MATERIAL_TEXTURE_NONE),
                local_bounds: ([0.0, 0.0, 0.0], 1.0),
                occlusion_query: None,
                disable_hiz_occlusion: false,
                casts_shadows: true,
                receives_shadows: true,
                mesh_blend: false,
                mesh_blend_screen: false,
                mesh_blend_params: 0,
                mesh_blend_depth: false,
                blend_layers: BitMask::ALL.bits(),
                blend_mask: BitMask::NONE.bits(),
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
                packed_lod: false,
                material_kind: MaterialPipelineKind::Standard,
                alpha_mode: 0,
                base_color_texture_slot: MATERIAL_TEXTURE_NONE,
                material_texture_key: MaterialTextureKey::from_base(MATERIAL_TEXTURE_NONE),
                local_bounds: ([0.0, 0.0, 0.0], 1.0),
                occlusion_query: None,
                disable_hiz_occlusion: false,
                casts_shadows: true,
                receives_shadows: true,
                mesh_blend: false,
                mesh_blend_screen: false,
                mesh_blend_params: 0,
                mesh_blend_depth: false,
                blend_layers: BitMask::ALL.bits(),
                blend_mask: BitMask::NONE.bits(),
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
                packed_lod: false,
                material_kind: MaterialPipelineKind::Standard,
                alpha_mode: 0,
                base_color_texture_slot: MATERIAL_TEXTURE_NONE,
                material_texture_key: MaterialTextureKey::from_base(MATERIAL_TEXTURE_NONE),
                local_bounds: ([0.0, 0.0, 0.0], 1.0),
                occlusion_query: Some(11),
                disable_hiz_occlusion: false,
                casts_shadows: true,
                receives_shadows: true,
                mesh_blend: false,
                mesh_blend_screen: false,
                mesh_blend_params: 0,
                mesh_blend_depth: false,
                blend_layers: BitMask::ALL.bits(),
                blend_mask: BitMask::NONE.bits(),
            },
        );

        assert_eq!(batches.len(), 3);
    }

    #[test]
    fn meshlet_split_keeps_custom_shader_batch_state() {
        let mut batches = Vec::new();
        let material_kind = MaterialPipelineKind::Custom(77);
        let common = |mesh: MeshRange, instance_start: u32| DrawBatchPush {
            render_path: RenderPath3D::Rigid,
            mesh,
            instance_start,
            instance_count: 1,
            double_sided: false,
            packed_lod: false,
            material_kind: material_kind.clone(),
            alpha_mode: 0,
            base_color_texture_slot: MATERIAL_TEXTURE_NONE,
            material_texture_key: MaterialTextureKey::from_base(MATERIAL_TEXTURE_NONE),
            local_bounds: ([0.0, 0.0, 0.0], 1.0),
            occlusion_query: None,
            disable_hiz_occlusion: false,
            casts_shadows: true,
            receives_shadows: true,
            mesh_blend: false,
            mesh_blend_screen: false,
            mesh_blend_params: 0,
            mesh_blend_depth: false,
            blend_layers: BitMask::ALL.bits(),
            blend_mask: BitMask::NONE.bits(),
        };

        push_draw_batch(
            &mut batches,
            common(
                MeshRange {
                    index_start: 0,
                    index_count: 6,
                    base_vertex: 4,
                },
                0,
            ),
        );
        push_draw_batch(
            &mut batches,
            common(
                MeshRange {
                    index_start: 6,
                    index_count: 9,
                    base_vertex: 4,
                },
                1,
            ),
        );

        assert_eq!(batches.len(), 2);
        assert_eq!(batches[0].state_key, batches[1].state_key);
        assert_eq!(batches[0].material_kind, batches[1].material_kind);
        assert_eq!(batches[0].path, batches[1].path);
        assert_eq!(batches[0].alpha_mode, batches[1].alpha_mode);
        assert_eq!(
            batches[0].base_color_texture_slot,
            batches[1].base_color_texture_slot
        );
        assert_ne!(batches[0].mesh.index_start, batches[1].mesh.index_start);
        assert_eq!(batches[0].mesh.base_vertex, batches[1].mesh.base_vertex);
    }

    #[test]
    fn compare_draw_batch_keys_sorts_opaque_before_alpha_and_overlay() {
        let opaque = test_batch(0, false, 0, false, 2, 0);
        let alpha = test_batch(1, false, 2, false, 1, 1);
        let overlay = test_batch(2, true, 0, false, 0, 2);
        let mut batches = [overlay.clone(), alpha.clone(), opaque.clone()];

        batches.sort_unstable_by(compare_draw_batch_keys);

        assert_eq!(batches[0].render_state.batch_kind, RenderBatchKind::Opaque);
        assert_eq!(batches[1].render_state.batch_kind, RenderBatchKind::Alpha);
        assert_eq!(batches[2].render_state.batch_kind, RenderBatchKind::Overlay);
    }

    #[test]
    fn compare_draw_batch_keys_keep_alpha_submission_order() {
        let first = test_batch(0, false, 2, false, 1, 0);
        let second = test_batch(1, false, 2, false, 0, 1);
        let mut batches = [second.clone(), first.clone()];

        batches.sort_unstable_by(compare_draw_batch_keys);

        assert_eq!(batches[0].order_index, first.order_index);
        assert_eq!(batches[1].order_index, second.order_index);
    }

    fn test_batch(
        order_index: u32,
        draw_on_top: bool,
        alpha_mode: u8,
        mesh_blend: bool,
        texture_slot: u32,
        mesh_index_start: u32,
    ) -> DrawBatch {
        let material_kind = MaterialPipelineKind::Standard;
        let state_key = draw_batch_state_key(
            RenderPath3D::Rigid,
            draw_on_top,
            false,
            alpha_mode,
            false,
            &material_kind,
        );
        let material_texture_key = MaterialTextureKey::from_base(texture_slot);
        DrawBatch {
            state_key,
            render_state: render_state_key(
                state_key,
                material_texture_key.state_hash(),
                mesh_index_start,
                0,
                draw_on_top,
                alpha_mode,
                mesh_blend,
            ),
            mesh: MeshRange {
                index_start: mesh_index_start,
                index_count: 3,
                base_vertex: 0,
            },
            instance_start: order_index,
            instance_count: 1,
            path: RenderPath3D::Rigid,
            packed_lod: false,
            double_sided: false,
            material_kind,
            alpha_mode,
            draw_on_top,
            base_color_texture_slot: texture_slot,
            material_texture_key,
            local_center: [0.0; 3],
            local_radius: 1.0,
            occlusion_query: None,
            disable_hiz_occlusion: false,
            casts_shadows: true,
            receives_shadows: true,
            mesh_blend,
            mesh_blend_screen: false,
            mesh_blend_params: 0,
            mesh_blend_depth: false,
            blend_layers: BitMask::ALL.bits(),
            blend_mask: BitMask::NONE.bits(),
            order_index,
        }
    }
}
