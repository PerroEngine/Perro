use super::{
    renderer::{
        Draw3DInstance, Draw3DKind, Lighting3DState, MAX_POINT_LIGHTS, MAX_RAY_LIGHTS,
        MAX_SPOT_LIGHTS,
    },
    shaders::{
        build_material_shader, create_depth_prepass_shader_module,
        create_frustum_cull_shader_module, create_hiz_depth_copy_shader_module,
        create_hiz_downsample_shader_module, create_hiz_occlusion_cull_shader_module,
        create_mesh_shader_module, create_sky_shader_module, create_toon_shader_module,
        create_unlit_shader_module,
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
use perro_io::{decompress_zlib, load_asset};
use perro_meshlets::pack_meshlets_from_positions;
use perro_render_bridge::{
    Camera3DState, CameraProjectionState, Material3D, MaterialParamOverride3D,
    MaterialParamOverrideValue3D, MeshSurfaceBinding3D, RuntimeMeshData, StandardMaterial3D,
};
use std::{
    borrow::Cow,
    cmp::Ordering,
    ops::Range,
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
const MATERIAL_TEXTURE_NONE: u32 = u32::MAX;
const MATERIAL_TEXTURE_FORMAT: wgpu::TextureFormat = wgpu::TextureFormat::Rgba8UnormSrgb;
const PTEX_MAGIC: &[u8; 4] = b"PTEX";
const SHADOW_MAP_SIZE: u32 = 4096;
const SHADOW_DEPTH_FORMAT: wgpu::TextureFormat = wgpu::TextureFormat::Depth32Float;
const SHADOW_MAP_DEPTH_BIAS_CONST: i32 = 2;
const SHADOW_MAP_DEPTH_BIAS_SLOPE: f32 = 2.0;
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
    uv: [f32; 2],
    joints: [u16; 4],
    weights: [f32; 4],
}

#[repr(C)]
#[derive(Clone, Copy, Pod, Zeroable)]
struct InstanceGpu {
    model_row_0: [f32; 4],
    model_row_1: [f32; 4],
    model_row_2: [f32; 4],
    color: [f32; 4],
    pbr_params: [f32; 4], // roughness, metallic, occlusion_strength, normal_scale
    emissive_factor: [f32; 3], // rgb
    material_params: [f32; 4], // alpha_mode, alpha_cutoff, double_sided, bitflags(debug/flat)
    skeleton_params: [u32; 4], // start, count, custom_params_offset, custom_params_len
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

#[repr(C)]
#[derive(Clone, Copy, Pod, Zeroable, PartialEq)]
struct ShadowUniform {
    light_view_proj: [[f32; 4]; 4],
    params0: [f32; 4], // enabled, strength, depth_bias, normal_bias
}

pub struct Gpu3D {
    color_format: wgpu::TextureFormat,
    camera_bgl: wgpu::BindGroupLayout,
    material_texture_bgl: wgpu::BindGroupLayout,
    shadow_bgl: wgpu::BindGroupLayout,
    sky_bgl: wgpu::BindGroupLayout,
    material_pipeline_layout: wgpu::PipelineLayout,
    sky_pipeline: wgpu::RenderPipeline,
    pipeline_culled: wgpu::RenderPipeline,
    pipeline_double_sided: wgpu::RenderPipeline,
    pipeline_unlit_culled: wgpu::RenderPipeline,
    pipeline_unlit_double_sided: wgpu::RenderPipeline,
    pipeline_toon_culled: wgpu::RenderPipeline,
    pipeline_toon_double_sided: wgpu::RenderPipeline,
    pipeline_overlay_culled: wgpu::RenderPipeline,
    pipeline_overlay_double_sided: wgpu::RenderPipeline,
    pipeline_depth_prepass_culled: wgpu::RenderPipeline,
    pipeline_depth_prepass_double_sided: wgpu::RenderPipeline,
    pipeline_shadow_depth_culled: wgpu::RenderPipeline,
    pipeline_shadow_depth_double_sided: wgpu::RenderPipeline,
    custom_pipelines: AHashMap<String, CustomPipeline>,
    camera_buffer: wgpu::Buffer,
    camera_bind_group: wgpu::BindGroup,
    shadow_camera_buffer: wgpu::Buffer,
    shadow_camera_bind_group: wgpu::BindGroup,
    shadow_buffer: wgpu::Buffer,
    shadow_bind_group: wgpu::BindGroup,
    _shadow_map_texture: wgpu::Texture,
    shadow_map_view: wgpu::TextureView,
    _shadow_map_sampler: wgpu::Sampler,
    sky_buffer: wgpu::Buffer,
    sky_bind_group: wgpu::BindGroup,
    skeleton_buffer: wgpu::Buffer,
    skeleton_capacity: usize,
    staged_skeletons: Vec<[[f32; 4]; 4]>,
    custom_params_buffer: wgpu::Buffer,
    custom_params_capacity: usize,
    staged_custom_params: Vec<[f32; 4]>,
    material_fallback_texture: Option<CachedMaterialTexture>,
    material_textures: AHashMap<u32, CachedMaterialTexture>,
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
    last_draws_revision: u64,
    last_draw_instance_ranges: Vec<Range<u32>>,
    last_scene: Option<Scene3DUniform>,
    last_shadow_scene: Option<Scene3DUniform>,
    last_shadow: Option<ShadowUniform>,
    shadow_pass_enabled: bool,
    shadow_focus_center: Vec3,
    shadow_focus_radius: f32,
    last_sky: Option<SkyUniform>,
    sky_enabled: bool,
    mesh_vertices: Vec<MeshVertex>,
    mesh_indices: Vec<u32>,
    vertex_buffer: wgpu::Buffer,
    index_buffer: wgpu::Buffer,
    vertex_capacity: usize,
    index_capacity: usize,
    builtin_mesh_ranges: AHashMap<&'static str, MeshRange>,
    builtin_mesh_bounds: AHashMap<&'static str, ([f32; 3], f32)>,
    builtin_meshlets: AHashMap<&'static str, Arc<[MeshletRange]>>,
    custom_mesh_ranges: AHashMap<String, MeshAssetRange>,
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
    surface_ranges: Arc<[MeshRange]>,
    meshlets: Arc<[MeshletRange]>,
    bounds_center: [f32; 3],
    bounds_radius: f32,
}

#[derive(Clone)]
struct DrawBatch {
    mesh: MeshRange,
    instance_start: u32,
    instance_count: u32,
    double_sided: bool,
    material_kind: MaterialPipelineKind,
    draw_on_top: bool,
    base_color_texture_slot: u32,
    local_center: [f32; 3],
    local_radius: f32,
    occlusion_query: Option<u32>,
    disable_hiz_occlusion: bool,
    casts_shadows: bool,
}

#[derive(Clone, Debug, PartialEq, Eq, Hash)]
enum MaterialPipelineKind {
    Standard,
    Unlit,
    Toon,
    Custom(String),
}

struct CustomPipeline {
    pipeline_culled: wgpu::RenderPipeline,
    pipeline_double_sided: wgpu::RenderPipeline,
}

struct CachedMaterialTexture {
    source: String,
    _texture: wgpu::Texture,
    _view: wgpu::TextureView,
    _sampler: wgpu::Sampler,
    bind_group: wgpu::BindGroup,
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
    surface_ranges: Vec<MeshRange>,
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
    fn ensure_custom_pipeline(
        &mut self,
        device: &wgpu::Device,
        shader_path: &str,
        static_shader_lookup: Option<StaticShaderLookup>,
    ) -> Option<&CustomPipeline> {
        if self.custom_pipelines.contains_key(shader_path) {
            return self.custom_pipelines.get(shader_path);
        }
        let src = if let Some(lookup) = static_shader_lookup {
            lookup(shader_path).map(Cow::Borrowed)
        } else {
            None
        }
        .or_else(|| {
            let bytes = load_asset(shader_path).ok()?;
            let src = std::str::from_utf8(&bytes).ok()?;
            Some(Cow::Owned(src.to_string()))
        })?;
        let wgsl = build_material_shader(src.as_ref());
        let shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("perro_mesh_custom"),
            source: wgpu::ShaderSource::Wgsl(wgsl.into()),
        });
        let pipeline_culled = create_pipeline(
            device,
            &self.material_pipeline_layout,
            &shader,
            self.color_format,
            self.sample_count,
            Some(wgpu::Face::Back),
        );
        let pipeline_double_sided = create_pipeline(
            device,
            &self.material_pipeline_layout,
            &shader,
            self.color_format,
            self.sample_count,
            None,
        );
        self.custom_pipelines.insert(
            shader_path.to_string(),
            CustomPipeline {
                pipeline_culled,
                pipeline_double_sided,
            },
        );
        self.custom_pipelines.get(shader_path)
    }

    fn material_pipeline_kind(
        &mut self,
        device: &wgpu::Device,
        material: &Material3D,
        static_shader_lookup: Option<StaticShaderLookup>,
    ) -> MaterialPipelineKind {
        match material {
            Material3D::Standard(_) => MaterialPipelineKind::Standard,
            Material3D::Unlit(_) => MaterialPipelineKind::Unlit,
            Material3D::Toon(_) => MaterialPipelineKind::Toon,
            Material3D::Custom(custom) => {
                let path = custom.shader_path.as_ref();
                if self
                    .ensure_custom_pipeline(device, path, static_shader_lookup)
                    .is_some()
                {
                    MaterialPipelineKind::Custom(path.to_string())
                } else {
                    MaterialPipelineKind::Standard
                }
            }
        }
    }

    fn pipeline_for_batch(&self, batch: &DrawBatch) -> &wgpu::RenderPipeline {
        if batch.draw_on_top {
            return if batch.double_sided {
                &self.pipeline_overlay_double_sided
            } else {
                &self.pipeline_overlay_culled
            };
        }
        match &batch.material_kind {
            MaterialPipelineKind::Standard => {
                if batch.double_sided {
                    &self.pipeline_double_sided
                } else {
                    &self.pipeline_culled
                }
            }
            MaterialPipelineKind::Unlit => {
                if batch.double_sided {
                    &self.pipeline_unlit_double_sided
                } else {
                    &self.pipeline_unlit_culled
                }
            }
            MaterialPipelineKind::Toon => {
                if batch.double_sided {
                    &self.pipeline_toon_double_sided
                } else {
                    &self.pipeline_toon_culled
                }
            }
            MaterialPipelineKind::Custom(path) => self
                .custom_pipelines
                .get(path)
                .map(|pipeline| {
                    if batch.double_sided {
                        &pipeline.pipeline_double_sided
                    } else {
                        &pipeline.pipeline_culled
                    }
                })
                .unwrap_or_else(|| {
                    if batch.double_sided {
                        &self.pipeline_double_sided
                    } else {
                        &self.pipeline_culled
                    }
                }),
        }
    }

    fn stage_custom_params(&mut self, material: &Material3D) -> (u32, u32) {
        match material {
            Material3D::Custom(custom) => {
                let offset = self.staged_custom_params.len() as u32;
                for param in custom.params.as_ref() {
                    self.staged_custom_params
                        .push(encode_custom_param_value(&param.value));
                }
                (offset, custom.params.len() as u32)
            }
            _ => (0, 0),
        }
    }

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
        let shader_unlit = create_unlit_shader_module(device);
        let shader_toon = create_toon_shader_module(device);
        let sky_shader = create_sky_shader_module(device);
        let camera_bgl = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("perro_camera3d_bgl"),
            entries: &[
                wgpu::BindGroupLayoutEntry {
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
                },
                wgpu::BindGroupLayoutEntry {
                    binding: 1,
                    visibility: wgpu::ShaderStages::VERTEX,
                    ty: wgpu::BindingType::Buffer {
                        ty: wgpu::BufferBindingType::Storage { read_only: true },
                        has_dynamic_offset: false,
                        min_binding_size: None,
                    },
                    count: None,
                },
                wgpu::BindGroupLayoutEntry {
                    binding: 2,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Buffer {
                        ty: wgpu::BufferBindingType::Storage { read_only: true },
                        has_dynamic_offset: false,
                        min_binding_size: None,
                    },
                    count: None,
                },
            ],
        });
        let material_texture_bgl =
            device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
                label: Some("perro_material_texture_bgl"),
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
                            sample_type: wgpu::TextureSampleType::Float { filterable: true },
                            view_dimension: wgpu::TextureViewDimension::D2,
                            multisampled: false,
                        },
                        count: None,
                    },
                ],
            });
        let shadow_bgl = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("perro_shadow3d_bgl"),
            entries: &[
                wgpu::BindGroupLayoutEntry {
                    binding: 0,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Buffer {
                        ty: wgpu::BufferBindingType::Uniform,
                        has_dynamic_offset: false,
                        min_binding_size: Some(
                            std::num::NonZeroU64::new(std::mem::size_of::<ShadowUniform>() as u64)
                                .expect("shadow uniform size must be non-zero"),
                        ),
                    },
                    count: None,
                },
                wgpu::BindGroupLayoutEntry {
                    binding: 1,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Texture {
                        sample_type: wgpu::TextureSampleType::Depth,
                        view_dimension: wgpu::TextureViewDimension::D2,
                        multisampled: false,
                    },
                    count: None,
                },
                wgpu::BindGroupLayoutEntry {
                    binding: 2,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Sampler(wgpu::SamplerBindingType::Comparison),
                    count: None,
                },
            ],
        });
        let sky_bgl = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("perro_sky3d_bgl"),
            entries: &[wgpu::BindGroupLayoutEntry {
                binding: 0,
                visibility: wgpu::ShaderStages::VERTEX_FRAGMENT,
                ty: wgpu::BindingType::Buffer {
                    ty: wgpu::BufferBindingType::Uniform,
                    has_dynamic_offset: false,
                    min_binding_size: Some(
                        std::num::NonZeroU64::new(std::mem::size_of::<SkyUniform>() as u64)
                            .expect("sky uniform size must be non-zero"),
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
        let shadow_camera_buffer = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("perro_shadow_camera3d_buffer"),
            size: std::mem::size_of::<Scene3DUniform>() as u64,
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });
        let shadow_buffer = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("perro_shadow3d_buffer"),
            size: std::mem::size_of::<ShadowUniform>() as u64,
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });
        let (shadow_map_texture, shadow_map_view) =
            create_shadow_map_texture(device, SHADOW_MAP_SIZE);
        let shadow_map_sampler = device.create_sampler(&wgpu::SamplerDescriptor {
            label: Some("perro_shadow3d_sampler"),
            address_mode_u: wgpu::AddressMode::ClampToEdge,
            address_mode_v: wgpu::AddressMode::ClampToEdge,
            address_mode_w: wgpu::AddressMode::ClampToEdge,
            mag_filter: wgpu::FilterMode::Linear,
            min_filter: wgpu::FilterMode::Linear,
            mipmap_filter: wgpu::MipmapFilterMode::Nearest,
            compare: Some(wgpu::CompareFunction::LessEqual),
            ..Default::default()
        });
        let sky_buffer = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("perro_sky3d_buffer"),
            size: std::mem::size_of::<SkyUniform>() as u64,
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });
        let skeleton_capacity = 1usize;
        let skeleton_buffer = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("perro_skeleton_palette_buffer"),
            size: (skeleton_capacity * std::mem::size_of::<[[f32; 4]; 4]>()) as u64,
            usage: wgpu::BufferUsages::STORAGE | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });
        let custom_params_capacity = 1usize;
        let custom_params_buffer = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("perro_custom_material_params"),
            size: (custom_params_capacity * std::mem::size_of::<[f32; 4]>()) as u64,
            usage: wgpu::BufferUsages::STORAGE | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });
        let camera_bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("perro_camera3d_bg"),
            layout: &camera_bgl,
            entries: &[
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: camera_buffer.as_entire_binding(),
                },
                wgpu::BindGroupEntry {
                    binding: 1,
                    resource: skeleton_buffer.as_entire_binding(),
                },
                wgpu::BindGroupEntry {
                    binding: 2,
                    resource: custom_params_buffer.as_entire_binding(),
                },
            ],
        });
        let shadow_camera_bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("perro_shadow_camera3d_bg"),
            layout: &camera_bgl,
            entries: &[
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: shadow_camera_buffer.as_entire_binding(),
                },
                wgpu::BindGroupEntry {
                    binding: 1,
                    resource: skeleton_buffer.as_entire_binding(),
                },
                wgpu::BindGroupEntry {
                    binding: 2,
                    resource: custom_params_buffer.as_entire_binding(),
                },
            ],
        });
        let shadow_bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("perro_shadow3d_bg"),
            layout: &shadow_bgl,
            entries: &[
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: shadow_buffer.as_entire_binding(),
                },
                wgpu::BindGroupEntry {
                    binding: 1,
                    resource: wgpu::BindingResource::TextureView(&shadow_map_view),
                },
                wgpu::BindGroupEntry {
                    binding: 2,
                    resource: wgpu::BindingResource::Sampler(&shadow_map_sampler),
                },
            ],
        });
        let sky_bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("perro_sky3d_bg"),
            layout: &sky_bgl,
            entries: &[wgpu::BindGroupEntry {
                binding: 0,
                resource: sky_buffer.as_entire_binding(),
            }],
        });

        let pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("perro_mesh_pipeline_layout"),
            bind_group_layouts: &[
                Some(&camera_bgl),
                Some(&material_texture_bgl),
                Some(&shadow_bgl),
            ],
            immediate_size: 0,
        });
        let depth_pipeline_layout =
            device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
                label: Some("perro_depth_pipeline_layout"),
                bind_group_layouts: &[Some(&camera_bgl)],
                immediate_size: 0,
            });
        let sky_pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("perro_sky3d_pipeline_layout"),
            bind_group_layouts: &[Some(&sky_bgl)],
            immediate_size: 0,
        });
        let sky_pipeline = create_sky_pipeline(
            device,
            &sky_pipeline_layout,
            &sky_shader,
            color_format,
            sample_count,
        );
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
        let pipeline_unlit_culled = create_pipeline(
            device,
            &pipeline_layout,
            &shader_unlit,
            color_format,
            sample_count,
            Some(wgpu::Face::Back),
        );
        let pipeline_unlit_double_sided = create_pipeline(
            device,
            &pipeline_layout,
            &shader_unlit,
            color_format,
            sample_count,
            None,
        );
        let pipeline_toon_culled = create_pipeline(
            device,
            &pipeline_layout,
            &shader_toon,
            color_format,
            sample_count,
            Some(wgpu::Face::Back),
        );
        let pipeline_toon_double_sided = create_pipeline(
            device,
            &pipeline_layout,
            &shader_toon,
            color_format,
            sample_count,
            None,
        );
        let pipeline_overlay_culled = create_pipeline_overlay(
            device,
            &pipeline_layout,
            &shader,
            color_format,
            sample_count,
            Some(wgpu::Face::Back),
        );
        let pipeline_overlay_double_sided = create_pipeline_overlay(
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
            &depth_pipeline_layout,
            &depth_prepass_shader,
            Some(wgpu::Face::Back),
        );
        let pipeline_depth_prepass_double_sided = create_depth_prepass_pipeline(
            device,
            &depth_pipeline_layout,
            &depth_prepass_shader,
            None,
        );
        let pipeline_shadow_depth_culled = create_shadow_depth_pipeline(
            device,
            &depth_pipeline_layout,
            &depth_prepass_shader,
            Some(wgpu::Face::Back),
        );
        let pipeline_shadow_depth_double_sided = create_shadow_depth_pipeline(
            device,
            &depth_pipeline_layout,
            &depth_prepass_shader,
            None,
        );

        let (vertices, indices, builtin_mesh_ranges, builtin_meshlets) =
            build_builtin_mesh_buffer();
        let builtin_mesh_bounds =
            compute_builtin_mesh_bounds(&vertices, &indices, &builtin_mesh_ranges);
        let vertex_capacity = vertices.len().max(1);
        let index_capacity = indices.len().max(1);
        let vertex_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("perro_builtin_mesh_vertices"),
            contents: bytemuck::cast_slice(&vertices),
            usage: wgpu::BufferUsages::VERTEX
                | wgpu::BufferUsages::COPY_DST
                | wgpu::BufferUsages::COPY_SRC,
        });
        let index_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("perro_builtin_mesh_indices"),
            contents: bytemuck::cast_slice(&indices),
            usage: wgpu::BufferUsages::INDEX
                | wgpu::BufferUsages::COPY_DST
                | wgpu::BufferUsages::COPY_SRC,
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
            bind_group_layouts: &[Some(&frustum_cull_bgl)],
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
                    bind_group_layouts: &[Some(&hiz_copy_bgl)],
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
                        bind_group_layouts: &[Some(&hiz_downsample_bgl)],
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
                    bind_group_layouts: &[Some(&hiz_cull_bgl)],
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

        let mut gpu = Self {
            color_format,
            camera_bgl,
            material_texture_bgl,
            shadow_bgl,
            sky_bgl,
            material_pipeline_layout: pipeline_layout,
            sky_pipeline,
            pipeline_culled,
            pipeline_double_sided,
            pipeline_unlit_culled,
            pipeline_unlit_double_sided,
            pipeline_toon_culled,
            pipeline_toon_double_sided,
            pipeline_overlay_culled,
            pipeline_overlay_double_sided,
            pipeline_depth_prepass_culled,
            pipeline_depth_prepass_double_sided,
            pipeline_shadow_depth_culled,
            pipeline_shadow_depth_double_sided,
            camera_buffer,
            camera_bind_group,
            shadow_camera_buffer,
            shadow_camera_bind_group,
            shadow_buffer,
            shadow_bind_group,
            _shadow_map_texture: shadow_map_texture,
            shadow_map_view,
            _shadow_map_sampler: shadow_map_sampler,
            sky_buffer,
            sky_bind_group,
            skeleton_buffer,
            skeleton_capacity,
            staged_skeletons: Vec::new(),
            custom_params_buffer,
            custom_params_capacity,
            staged_custom_params: Vec::new(),
            material_fallback_texture: None,
            material_textures: AHashMap::new(),
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
            last_draws_revision: u64::MAX,
            last_draw_instance_ranges: Vec::new(),
            last_scene: None,
            last_shadow_scene: None,
            last_shadow: None,
            shadow_pass_enabled: false,
            shadow_focus_center: Vec3::ZERO,
            shadow_focus_radius: 64.0,
            last_sky: None,
            sky_enabled: false,
            mesh_vertices: vertices,
            mesh_indices: indices,
            vertex_buffer,
            index_buffer,
            vertex_capacity,
            index_capacity,
            builtin_mesh_ranges,
            builtin_mesh_bounds,
            builtin_meshlets,
            custom_mesh_ranges: AHashMap::new(),
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
            hiz_copy_bind_group: None,
            hiz_downsample_bind_groups: Vec::new(),
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
            occlusion_state: AHashMap::new(),
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
            custom_pipelines: AHashMap::new(),
        };
        gpu.rebuild_hiz_bind_groups(device);
        gpu
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
        self.rebuild_hiz_bind_groups(device);
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
        if self.sample_count == sample_count && self.color_format == color_format {
            return;
        }
        let shader = create_mesh_shader_module(device);
        let shader_unlit = create_unlit_shader_module(device);
        let shader_toon = create_toon_shader_module(device);
        let sky_shader = create_sky_shader_module(device);
        let pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("perro_mesh_pipeline_layout"),
            bind_group_layouts: &[
                Some(&self.camera_bgl),
                Some(&self.material_texture_bgl),
                Some(&self.shadow_bgl),
            ],
            immediate_size: 0,
        });
        let depth_pipeline_layout =
            device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
                label: Some("perro_depth_pipeline_layout"),
                bind_group_layouts: &[Some(&self.camera_bgl)],
                immediate_size: 0,
            });
        let sky_pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("perro_sky3d_pipeline_layout"),
            bind_group_layouts: &[Some(&self.sky_bgl)],
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
        self.pipeline_unlit_culled = create_pipeline(
            device,
            &pipeline_layout,
            &shader_unlit,
            color_format,
            sample_count,
            Some(wgpu::Face::Back),
        );
        self.pipeline_unlit_double_sided = create_pipeline(
            device,
            &pipeline_layout,
            &shader_unlit,
            color_format,
            sample_count,
            None,
        );
        self.pipeline_toon_culled = create_pipeline(
            device,
            &pipeline_layout,
            &shader_toon,
            color_format,
            sample_count,
            Some(wgpu::Face::Back),
        );
        self.pipeline_toon_double_sided = create_pipeline(
            device,
            &pipeline_layout,
            &shader_toon,
            color_format,
            sample_count,
            None,
        );
        self.pipeline_overlay_culled = create_pipeline_overlay(
            device,
            &pipeline_layout,
            &shader,
            color_format,
            sample_count,
            Some(wgpu::Face::Back),
        );
        self.pipeline_overlay_double_sided = create_pipeline_overlay(
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
            &depth_pipeline_layout,
            &depth_prepass_shader,
            Some(wgpu::Face::Back),
        );
        self.pipeline_depth_prepass_double_sided = create_depth_prepass_pipeline(
            device,
            &depth_pipeline_layout,
            &depth_prepass_shader,
            None,
        );
        self.pipeline_shadow_depth_culled = create_shadow_depth_pipeline(
            device,
            &depth_pipeline_layout,
            &depth_prepass_shader,
            Some(wgpu::Face::Back),
        );
        self.pipeline_shadow_depth_double_sided = create_shadow_depth_pipeline(
            device,
            &depth_pipeline_layout,
            &depth_prepass_shader,
            None,
        );
        self.sky_pipeline = create_sky_pipeline(
            device,
            &sky_pipeline_layout,
            &sky_shader,
            color_format,
            sample_count,
        );
        self.material_pipeline_layout = pipeline_layout;
        self.color_format = color_format;
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
        self.rebuild_hiz_bind_groups(device);
        self.sample_count = sample_count;
        self.custom_pipelines.clear();
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
            if self.pending_hiz_debug_map_rx.is_some() {
                let _ = device.poll(wgpu::PollType::Poll);
            }
            if self.pending_hiz_debug_count > 0 && self.pending_hiz_debug_map_rx.is_none() {
                self.request_hiz_debug_map_async();
            }
            self.consume_hiz_debug_results();
        }
        if self.cpu_occlusion_enabled {
            if self.pending_occlusion_map_rx.is_some() {
                let _ = device.poll(wgpu::PollType::Poll);
            }
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
            draws_revision,
            width,
            height,
            static_texture_lookup,
            static_mesh_lookup,
            static_shader_lookup,
        } = frame;
        self.custom_mesh_ranges
            .retain(|source, _| resources.has_mesh_source(source));
        self.resize(device, width, height);
        self.ensure_material_fallback_texture(device, queue);
        self.frustum_cull_enabled = self.frustum_cull_supported;
        let (gpu_occlusion_enabled, cpu_occlusion_enabled) = occlusion_flags(self.occlusion_mode);
        self.gpu_occlusion_enabled = gpu_occlusion_enabled && self.frustum_cull_enabled;
        self.cpu_occlusion_enabled = cpu_occlusion_enabled;

        let uniform = build_scene_uniform(&camera, lighting, width, height);
        let sky_uniform = build_sky_uniform(&camera, lighting, width, height);
        self.sky_enabled = sky_uniform.is_some();
        if self.last_sky != sky_uniform {
            if let Some(sky) = sky_uniform {
                queue.write_buffer(&self.sky_buffer, 0, bytemuck::bytes_of(&sky));
            }
            self.last_sky = sky_uniform;
        }
        let draws_unchanged = self.last_draws_revision == draws_revision;
        let transform_only_semantic = !draws_unchanged
            && draws.len() == self.last_draws.len()
            && self
                .last_draws
                .iter()
                .zip(draws.iter())
                .all(|(prev, next)| {
                    prev.instance_mats.len() == 1
                        && next.instance_mats.len() == 1
                        && same_draw_except_model(prev, next)
                });
        let stable_instance_ranges = self.last_draw_instance_ranges.len() == draws.len()
            && self.last_draw_instance_ranges.iter().all(|range| {
                range.start <= range.end && (range.end as usize) <= self.staged_instances.len()
            });
        let transform_only_changed =
            !draws_unchanged && transform_only_semantic && stable_instance_ranges;
        let scene_changed = self.last_scene != Some(uniform) || !draws_unchanged;
        if self.last_scene != Some(uniform) {
            queue.write_buffer(&self.camera_buffer, 0, bytemuck::bytes_of(&uniform));
            self.last_scene = Some(uniform);
        }
        if self.cpu_occlusion_enabled && scene_changed {
            // Retained-mode correctness: when camera/transforms/resources update,
            // previous query visibility is stale and must not gate current frame.
            self.occlusion_state.clear();
        }
        let view_proj = compute_view_proj_mat(&camera, width, height);
        self.last_aspect = (width.max(1) as f32) / (height.max(1) as f32);
        self.last_proj_y_scale = projection_y_scale_from_projection(camera.projection);

        if draws_unchanged {
            if self.frustum_cull_enabled && !self.draw_batches.is_empty() {
                let frustum = extract_frustum_planes(view_proj);
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
            self.update_shadow_state(queue, &camera, lighting);
            self.last_total_drawn = self.staged_instances.len();
            return;
        }
        if transform_only_changed {
            let mut dirty_instance_spans: Vec<Range<u32>> = Vec::new();
            for (draw, range) in draws.iter().zip(self.last_draw_instance_ranges.iter()) {
                let Some(model) = draw.instance_mats.first() else {
                    continue;
                };
                if range.start >= range.end {
                    continue;
                }
                for instance in &mut self.staged_instances[range.start as usize..range.end as usize]
                {
                    instance.model_row_0 = [
                        model[0][0],
                        model[1][0],
                        model[2][0],
                        model[3][0],
                    ];
                    instance.model_row_1 = [
                        model[0][1],
                        model[1][1],
                        model[2][1],
                        model[3][1],
                    ];
                    instance.model_row_2 = [
                        model[0][2],
                        model[1][2],
                        model[2][2],
                        model[3][2],
                    ];
                }
                dirty_instance_spans.push(range.clone());
            }
            dirty_instance_spans.sort_unstable_by_key(|span| span.start);
            let mut merged_spans: Vec<Range<u32>> = Vec::new();
            for span in dirty_instance_spans {
                if let Some(last) = merged_spans.last_mut() && span.start <= last.end {
                    last.end = last.end.max(span.end);
                } else {
                    merged_spans.push(span);
                }
            }
            for span in merged_spans {
                let byte_start = span.start as u64 * std::mem::size_of::<InstanceGpu>() as u64;
                queue.write_buffer(
                    &self.instance_buffer,
                    byte_start,
                    bytemuck::cast_slice(
                        &self.staged_instances[span.start as usize..span.end as usize],
                    ),
                );
            }
            if self.frustum_cull_enabled && !self.draw_batches.is_empty() {
                let frustum = extract_frustum_planes(view_proj);
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

                self.frustum_cull_staging.clear();
                self.frustum_cull_staging.reserve(self.draw_batches.len());
                for batch in &self.draw_batches {
                    let instance = &self.staged_instances[batch.instance_start as usize];
                    let model_cols = model_cols_from_affine_rows(instance);
                    self.frustum_cull_staging.push(FrustumCullItemGpu {
                        model_0: model_cols[0],
                        model_1: model_cols[1],
                        model_2: model_cols[2],
                        model_3: model_cols[3],
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
                queue.write_buffer(
                    &self.frustum_cull_items_buffer,
                    0,
                    bytemuck::cast_slice(&self.frustum_cull_staging),
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
            self.update_shadow_state(queue, &camera, lighting);
            self.last_draws.clear();
            self.last_draws.extend_from_slice(draws);
            self.last_draws_revision = draws_revision;
            self.last_total_drawn = self.staged_instances.len();
            return;
        }

        self.last_draws.clear();
        self.last_draws.extend_from_slice(draws);
        self.last_draws_revision = draws_revision;

        self.staged_instances.clear();
        self.staged_instances.reserve(draws.len());
        self.staged_skeletons.clear();
        self.staged_custom_params.clear();
        self.draw_batches.clear();
        self.draw_batches.reserve(draws.len());
        self.last_draw_instance_ranges.clear();
        self.last_draw_instance_ranges.reserve(draws.len());
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
            let draw_instance_start = self.staged_instances.len() as u32;
            let is_debug_point = matches!(draw.kind, Draw3DKind::DebugPointCube);
            let is_debug_edge = matches!(draw.kind, Draw3DKind::DebugEdgeCylinder);
            let mesh_source = match draw.kind {
                Draw3DKind::Mesh(mesh) => resources.mesh_source(mesh).unwrap_or("__cube__"),
                Draw3DKind::DebugPointCube => "__cube__",
                Draw3DKind::DebugEdgeCylinder => "__cylinder__",
            };
            let mesh_asset = match draw.kind {
                Draw3DKind::Mesh(_) => self
                    .resolve_mesh_range(device, queue, resources, mesh_source, static_mesh_lookup)
                    .unwrap_or_else(|| default_mesh.clone()),
                Draw3DKind::DebugPointCube => self
                    .resolve_builtin_mesh_asset("__cube__")
                    .unwrap_or_else(|| default_mesh.clone()),
                Draw3DKind::DebugEdgeCylinder => self
                    .resolve_builtin_mesh_asset("__cylinder__")
                    .unwrap_or_else(|| default_mesh.clone()),
            };
            let mut surface_entries: Vec<(MeshRange, Material3D)> = match draw.kind {
                Draw3DKind::DebugPointCube => vec![(
                    mesh_asset.full,
                    Material3D::Standard(StandardMaterial3D {
                        base_color_factor: [1.0, 0.92, 0.2, 1.0],
                        roughness_factor: 0.35,
                        metallic_factor: 0.0,
                        emissive_factor: [0.35, 0.3, 0.06],
                        ..StandardMaterial3D::default()
                    }),
                )],
                Draw3DKind::DebugEdgeCylinder => vec![(
                    mesh_asset.full,
                    Material3D::Standard(StandardMaterial3D {
                        base_color_factor: [0.15, 0.95, 0.95, 1.0],
                        roughness_factor: 0.6,
                        metallic_factor: 0.0,
                        emissive_factor: [0.06, 0.3, 0.3],
                        ..StandardMaterial3D::default()
                    }),
                )],
                Draw3DKind::Mesh(_) => {
                    let mut out = Vec::new();
                    for (surface_index, surface) in draw.surfaces.iter().enumerate() {
                        let Some(range) = mesh_asset.surface_ranges.get(surface_index).copied()
                        else {
                            continue;
                        };
                        let base_material = surface
                            .material
                            .and_then(|id| resources.material(id))
                            .unwrap_or_default();
                        out.push((range, apply_surface_binding(base_material, surface)));
                    }
                    if out.is_empty() {
                        out.push((mesh_asset.full, Material3D::default()));
                    }
                    out
                }
            };
            if surface_entries.is_empty() {
                self.last_draw_instance_ranges
                    .push(draw_instance_start..(self.staged_instances.len() as u32));
                continue;
            }
            // CPU occlusion query mode works at object granularity.
            // Force whole-mesh batching in that mode so each object can be queried.
            let use_meshlets = !is_debug_point
                && !is_debug_edge
                && self.meshlets_enabled
                && !mesh_asset.meshlets.is_empty()
                && surface_entries.len() == 1
                && !self.cpu_occlusion_enabled;
            total_meshlets = total_meshlets.saturating_add(if use_meshlets {
                mesh_asset.meshlets.len()
            } else {
                1
            });

            // Keep casters available even when off-screen so directional shadow fitting
            // stays stable during camera orbit/rotation.

            if !use_meshlets {
                let occlusion_key = draw.node.as_u64();
                if self.cpu_occlusion_enabled
                    && !is_debug_point
                    && !is_debug_edge
                    && !self.should_probe_or_draw(occlusion_key)
                {
                    self.last_draw_instance_ranges
                        .push(draw_instance_start..(self.staged_instances.len() as u32));
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
                let (skeleton_start, skeleton_count) = if let Some(skeleton) = &draw.skeleton {
                    let start = self.staged_skeletons.len() as u32;
                    let count = skeleton.matrices.len() as u32;
                    self.staged_skeletons
                        .extend_from_slice(skeleton.matrices.as_ref());
                    (start, count)
                } else {
                    (0, 0)
                };
                let instance_mats = draw.instance_mats.as_ref();
                if is_debug_point {
                    let (_, material) = surface_entries.remove(0);
                    let (custom_params_offset, custom_params_len) =
                        self.stage_custom_params(&material);
                    let standard_params = material.standard_params();
                    self.ensure_material_texture_slot(
                        device,
                        queue,
                        resources,
                        standard_params.base_color_texture,
                        mesh_source,
                        static_texture_lookup,
                    );
                    if debug_point_instances.is_empty() {
                        debug_points_double_sided =
                            material.standard_params().double_sided || self.meshlet_debug_view;
                        debug_points_local_center = mesh_asset.bounds_center;
                        debug_points_local_radius = mesh_asset.bounds_radius;
                    }
                    for model in instance_mats.iter().copied() {
                        debug_point_instances.push(build_instance(
                            model,
                            &material,
                            self.meshlet_debug_view,
                            debug_color(draw.node.as_u64()),
                            skeleton_start,
                            skeleton_count,
                            custom_params_offset,
                            custom_params_len,
                        ));
                        debug_points_count = debug_points_count.saturating_add(1);
                    }
                } else if is_debug_edge {
                    let (_, material) = surface_entries.remove(0);
                    let (custom_params_offset, custom_params_len) =
                        self.stage_custom_params(&material);
                    let standard_params = material.standard_params();
                    self.ensure_material_texture_slot(
                        device,
                        queue,
                        resources,
                        standard_params.base_color_texture,
                        mesh_source,
                        static_texture_lookup,
                    );
                    if debug_edge_instances.is_empty() {
                        debug_edges_double_sided =
                            material.standard_params().double_sided || self.meshlet_debug_view;
                        debug_edges_local_center = mesh_asset.bounds_center;
                        debug_edges_local_radius = mesh_asset.bounds_radius;
                    }
                    for model in instance_mats.iter().copied() {
                        debug_edge_instances.push(build_instance(
                            model,
                            &material,
                            self.meshlet_debug_view,
                            debug_color(draw.node.as_u64()),
                            skeleton_start,
                            skeleton_count,
                            custom_params_offset,
                            custom_params_len,
                        ));
                        debug_edges_count = debug_edges_count.saturating_add(1);
                    }
                } else {
                    for (range, material) in surface_entries {
                        let standard_params = material.standard_params();
                        self.ensure_material_texture_slot(
                            device,
                            queue,
                            resources,
                            standard_params.base_color_texture,
                            mesh_source,
                            static_texture_lookup,
                        );
                        let material_kind =
                            self.material_pipeline_kind(device, &material, static_shader_lookup);
                        let (custom_params_offset, custom_params_len) =
                            self.stage_custom_params(&material);
                        let instance_start = self.staged_instances.len() as u32;
                        for model in instance_mats.iter().copied() {
                            self.staged_instances.push(build_instance(
                                model,
                                &material,
                                self.meshlet_debug_view,
                                debug_color(draw.node.as_u64()),
                                skeleton_start,
                                skeleton_count,
                                custom_params_offset,
                                custom_params_len,
                            ));
                        }
                        let instance_count =
                            (self.staged_instances.len() as u32).saturating_sub(instance_start);
                        if instance_count > 0 {
                            let multi_instance = instance_count > 1;
                            let occlusion_bounds = if multi_instance {
                                ([0.0, 0.0, 0.0], 1.0e9)
                            } else {
                                (mesh_asset.bounds_center, mesh_asset.bounds_radius)
                            };
                            push_draw_batch(
                                &mut self.draw_batches,
                                range,
                                instance_start,
                                instance_count,
                                standard_params.double_sided || self.meshlet_debug_view,
                                material_kind.clone(),
                                standard_params.base_color_texture,
                                occlusion_bounds,
                                occlusion_query,
                                multi_instance,
                                true,
                            );
                        }
                    }
                }
            } else {
                let (_, material) = &surface_entries[0];
                let standard_params = material.standard_params();
                self.ensure_material_texture_slot(
                    device,
                    queue,
                    resources,
                    standard_params.base_color_texture,
                    mesh_source,
                    static_texture_lookup,
                );
                let material_kind =
                    self.material_pipeline_kind(device, material, static_shader_lookup);
                let (custom_params_offset, custom_params_len) = self.stage_custom_params(material);
                let (skeleton_start, skeleton_count) = if let Some(skeleton) = &draw.skeleton {
                    let start = self.staged_skeletons.len() as u32;
                    let count = skeleton.matrices.len() as u32;
                    self.staged_skeletons
                        .extend_from_slice(skeleton.matrices.as_ref());
                    (start, count)
                } else {
                    (0, 0)
                };
                let instance_mats = draw.instance_mats.as_ref();
                for meshlet in mesh_asset.meshlets.iter().copied() {
                    // Keep meshlet casters for stable shadow fitting even when off-screen.
                    // CPU query occlusion at meshlet granularity self-occludes dynamic meshes.
                    // Keep meshlet occlusion GPU-driven only; CPU mode skips meshlet occlusion.
                    let occlusion_query = None;
                    let instance_start = self.staged_instances.len() as u32;
                    for model in instance_mats.iter().copied() {
                        self.staged_instances.push(build_instance(
                            model,
                            material,
                            self.meshlet_debug_view,
                            debug_color((draw.node.as_u64() << 32) ^ meshlet.index_start as u64),
                            skeleton_start,
                            skeleton_count,
                            custom_params_offset,
                            custom_params_len,
                        ));
                    }
                    let instance_count =
                        (self.staged_instances.len() as u32).saturating_sub(instance_start);
                    if instance_count == 0 {
                        continue;
                    }
                    let multi_instance = instance_count > 1;
                    // Use per-meshlet local bounds for tighter frustum/occlusion rejection.
                    let (occlusion_center, occlusion_radius) = if multi_instance {
                        ([0.0, 0.0, 0.0], 1.0e9)
                    } else {
                        (meshlet.center, meshlet.radius.max(0.001))
                    };
                    push_draw_batch(
                        &mut self.draw_batches,
                        MeshRange {
                            index_start: meshlet.index_start,
                            index_count: meshlet.index_count,
                            base_vertex: mesh_asset.full.base_vertex,
                        },
                        instance_start,
                        instance_count,
                        standard_params.double_sided || self.meshlet_debug_view,
                        material_kind.clone(),
                        standard_params.base_color_texture,
                        (occlusion_center, occlusion_radius),
                        occlusion_query,
                        multi_instance,
                        true,
                    );
                }
            }
            self.last_draw_instance_ranges
                .push(draw_instance_start..(self.staged_instances.len() as u32));
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
                material_kind: MaterialPipelineKind::Standard,
                draw_on_top: true,
                base_color_texture_slot: MATERIAL_TEXTURE_NONE,
                local_center: debug_points_local_center,
                local_radius: debug_points_local_radius.max(0.0),
                occlusion_query: None,
                disable_hiz_occlusion: true,
                casts_shadows: false,
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
                material_kind: MaterialPipelineKind::Standard,
                draw_on_top: true,
                base_color_texture_slot: MATERIAL_TEXTURE_NONE,
                local_center: debug_edges_local_center,
                local_radius: debug_edges_local_radius.max(0.0),
                occlusion_query: None,
                disable_hiz_occlusion: true,
                casts_shadows: false,
            });
        }
        self.draw_batches.sort_unstable_by(compare_draw_batch_keys);
        self.debug_frustum_visible_est = 0;
        for batch in &self.draw_batches {
            let model =
                model_cols_from_affine_rows(&self.staged_instances[batch.instance_start as usize]);
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
        self.ensure_skeleton_capacity(device, self.staged_skeletons.len().max(1));
        if !self.staged_skeletons.is_empty() {
            queue.write_buffer(
                &self.skeleton_buffer,
                0,
                bytemuck::cast_slice(&self.staged_skeletons),
            );
        }
        self.ensure_custom_params_capacity(device, self.staged_custom_params.len().max(1));
        if !self.staged_custom_params.is_empty() {
            queue.write_buffer(
                &self.custom_params_buffer,
                0,
                bytemuck::cast_slice(&self.staged_custom_params),
            );
        }
        self.ensure_frustum_cull_capacity(device, self.draw_batches.len());
        if self.frustum_cull_enabled && !self.draw_batches.is_empty() {
            self.indirect_staging
                .reserve(self.draw_batches.len() - self.indirect_staging.len());
            self.frustum_cull_staging
                .reserve(self.draw_batches.len() - self.frustum_cull_staging.len());
            for batch in &self.draw_batches {
                let model_cols = model_cols_from_affine_rows(
                    &self.staged_instances[batch.instance_start as usize],
                );
                self.indirect_staging.push(DrawIndexedIndirectGpu {
                    index_count: batch.mesh.index_count,
                    instance_count: batch.instance_count,
                    first_index: batch.mesh.index_start,
                    base_vertex: batch.mesh.base_vertex,
                    first_instance: batch.instance_start,
                });
                self.frustum_cull_staging.push(FrustumCullItemGpu {
                    model_0: model_cols[0],
                    model_1: model_cols[1],
                    model_2: model_cols[2],
                    model_3: model_cols[3],
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
        self.update_shadow_state(queue, &camera, lighting);
        self.last_total_meshlets = total_meshlets;
        self.last_total_drawn = self.staged_instances.len();
    }

    fn update_shadow_state(
        &mut self,
        queue: &wgpu::Queue,
        camera: &Camera3DState,
        lighting: &Lighting3DState,
    ) {
        let (shadow_scene, shadow_uniform, enabled, focus_center, focus_radius) =
            build_shadow_setup(
                camera,
                lighting,
                &self.draw_batches,
                &self.staged_instances,
                self.shadow_focus_center,
                self.shadow_focus_radius,
                self.depth_size.0,
                self.depth_size.1,
            );
        self.shadow_focus_center = focus_center;
        self.shadow_focus_radius = focus_radius;
        if self.last_shadow_scene != Some(shadow_scene) {
            queue.write_buffer(
                &self.shadow_camera_buffer,
                0,
                bytemuck::bytes_of(&shadow_scene),
            );
            self.last_shadow_scene = Some(shadow_scene);
        }
        if self.last_shadow != Some(shadow_uniform) {
            queue.write_buffer(&self.shadow_buffer, 0, bytemuck::bytes_of(&shadow_uniform));
            self.last_shadow = Some(shadow_uniform);
        }
        self.shadow_pass_enabled = enabled;
    }

    pub fn render_pass(
        &mut self,
        encoder: &mut wgpu::CommandEncoder,
        color_view: &wgpu::TextureView,
        clear_color: wgpu::Color,
        depth_prepass_needed: bool,
    ) {
        let hiz_active = self.gpu_occlusion_enabled && !self.draw_batches.is_empty();
        let depth_prepass_active = hiz_active || depth_prepass_needed;
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
        if self.shadow_pass_enabled && !self.draw_batches.is_empty() {
            let mut shadow_pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("perro_shadow3d_pass"),
                color_attachments: &[],
                depth_stencil_attachment: Some(wgpu::RenderPassDepthStencilAttachment {
                    view: &self.shadow_map_view,
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
            shadow_pass.set_bind_group(0, &self.shadow_camera_bind_group, &[]);
            shadow_pass.set_vertex_buffer(0, self.vertex_buffer.slice(..));
            shadow_pass.set_vertex_buffer(1, self.instance_buffer.slice(..));
            shadow_pass.set_index_buffer(self.index_buffer.slice(..), wgpu::IndexFormat::Uint32);
            let mut current_double_sided = None;
            for batch in &self.draw_batches {
                if batch.draw_on_top || !batch.casts_shadows {
                    continue;
                }
                if current_double_sided != Some(batch.double_sided) {
                    let pipeline = if batch.double_sided {
                        &self.pipeline_shadow_depth_double_sided
                    } else {
                        &self.pipeline_shadow_depth_culled
                    };
                    shadow_pass.set_pipeline(pipeline);
                    current_double_sided = Some(batch.double_sided);
                }
                let start = batch.mesh.index_start;
                let end = start + batch.mesh.index_count;
                let instances = batch.instance_start..batch.instance_start + batch.instance_count;
                shadow_pass.draw_indexed(start..end, batch.mesh.base_vertex, instances);
            }
            drop(shadow_pass);
        }
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
        if depth_prepass_active {
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
                if batch.draw_on_top {
                    continue;
                }
                if current_double_sided != Some(batch.double_sided) {
                    let pipeline = if batch.double_sided {
                        &self.pipeline_depth_prepass_double_sided
                    } else {
                        &self.pipeline_depth_prepass_culled
                    };
                    prepass.set_pipeline(pipeline);
                    current_double_sided = Some(batch.double_sided);
                }
                if self.frustum_cull_enabled {
                    let offset = (i * std::mem::size_of::<DrawIndexedIndirectGpu>()) as u64;
                    prepass.draw_indexed_indirect(&self.indirect_buffer, offset);
                } else {
                    let start = batch.mesh.index_start;
                    let end = start + batch.mesh.index_count;
                    let instances =
                        batch.instance_start..batch.instance_start + batch.instance_count;
                    prepass.draw_indexed(start..end, batch.mesh.base_vertex, instances);
                }
            }
            drop(prepass);
        }
        if hiz_active {
            self.build_hiz_from_depth(encoder);

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
        if self.sky_enabled {
            let mut sky_pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("perro_sky3d_pass"),
                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                    view: color_view,
                    resolve_target: None,
                    ops: wgpu::Operations {
                        load: wgpu::LoadOp::Clear(clear_color),
                        store: wgpu::StoreOp::Store,
                    },
                    depth_slice: None,
                })],
                depth_stencil_attachment: None,
                timestamp_writes: None,
                occlusion_query_set: None,
                multiview_mask: None,
            });
            sky_pass.set_pipeline(&self.sky_pipeline);
            sky_pass.set_bind_group(0, &self.sky_bind_group, &[]);
            sky_pass.draw(0..3, 0..1);
            drop(sky_pass);
        }
        let color_load = if self.sky_enabled {
            wgpu::LoadOp::Load
        } else {
            wgpu::LoadOp::Clear(clear_color)
        };
        let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
            label: Some("perro_mesh_pass"),
            color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                view: color_view,
                resolve_target: None,
                ops: wgpu::Operations {
                    load: color_load,
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
        pass.set_bind_group(1, self.fallback_material_texture_bind_group(), &[]);
        pass.set_bind_group(2, &self.shadow_bind_group, &[]);
        pass.set_vertex_buffer(0, self.vertex_buffer.slice(..));
        pass.set_vertex_buffer(1, self.instance_buffer.slice(..));
        pass.set_index_buffer(self.index_buffer.slice(..), wgpu::IndexFormat::Uint32);
        let mut current_pipeline_key: Option<(MaterialPipelineKind, bool, bool)> = None;
        let mut current_texture_slot = MATERIAL_TEXTURE_NONE;
        for (i, batch) in self.draw_batches.iter().enumerate() {
            let key = (
                batch.material_kind.clone(),
                batch.double_sided,
                batch.draw_on_top,
            );
            if current_pipeline_key.as_ref() != Some(&key) {
                let pipeline = self.pipeline_for_batch(batch);
                pass.set_pipeline(pipeline);
                current_pipeline_key = Some(key);
            }
            if current_texture_slot != batch.base_color_texture_slot {
                pass.set_bind_group(
                    1,
                    self.material_texture_bind_group(batch.base_color_texture_slot),
                    &[],
                );
                current_texture_slot = batch.base_color_texture_slot;
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

    pub fn depth_prepass_view(&self) -> &wgpu::TextureView {
        &self.depth_prepass_view
    }

    #[inline]
    pub fn draw_call_count(&self) -> u32 {
        self.draw_batches.len() as u32
    }

    fn fallback_material_texture_bind_group(&self) -> &wgpu::BindGroup {
        self.material_fallback_texture
            .as_ref()
            .map(|cached| &cached.bind_group)
            .expect("material fallback texture must be initialized in prepare")
    }

    fn material_texture_bind_group(&self, slot: u32) -> &wgpu::BindGroup {
        self.material_textures
            .get(&slot)
            .map(|cached| &cached.bind_group)
            .unwrap_or_else(|| self.fallback_material_texture_bind_group())
    }

    fn ensure_material_fallback_texture(&mut self, device: &wgpu::Device, queue: &wgpu::Queue) {
        if self.material_fallback_texture.is_some() {
            return;
        }
        let cached = create_cached_material_texture(
            device,
            queue,
            &self.material_texture_bgl,
            vec![255u8, 255, 255, 255],
            1,
            1,
            "__fallback__".to_string(),
        );
        self.material_fallback_texture = Some(cached);
    }

    fn ensure_material_texture_slot(
        &mut self,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        resources: &ResourceStore,
        slot: u32,
        mesh_source: &str,
        static_texture_lookup: Option<StaticTextureLookup>,
    ) {
        if slot == MATERIAL_TEXTURE_NONE {
            return;
        }
        self.ensure_material_fallback_texture(device, queue);

        // glTF material texture indices are model-local, not global texture IDs.
        // Prefer glTF-local texture source when mesh source is glTF/glb.
        let gltf_source = gltf_texture_source_from_mesh_source(mesh_source, slot);
        let global_source = resources.texture_source_by_index(slot).or_else(|| {
            slot.checked_add(1)
                .and_then(|next| resources.texture_source_by_index(next))
        });
        let source = if gltf_source.is_some() {
            gltf_source.or_else(|| global_source.map(ToString::to_string))
        } else {
            global_source
                .map(ToString::to_string)
                .or_else(|| gltf_source)
        };
        let Some(source) = source else {
            self.material_textures.remove(&slot);
            return;
        };
        if self
            .material_textures
            .get(&slot)
            .is_some_and(|cached| cached.source == source)
        {
            return;
        }

        let decoded = if source == "__default__" {
            Some((vec![255u8, 255, 255, 255], 1, 1))
        } else if let Some(lookup) = static_texture_lookup {
            if let Some(bytes) = lookup(source.as_str()) {
                decode_ptex(bytes)
            } else {
                load_texture_rgba(source.as_str())
            }
        } else {
            load_texture_rgba(source.as_str())
        };
        let Some((rgba, width, height)) = decoded else {
            self.material_textures.remove(&slot);
            return;
        };
        let cached = create_cached_material_texture(
            device,
            queue,
            &self.material_texture_bgl,
            rgba,
            width,
            height,
            source,
        );
        self.material_textures.insert(slot, cached);
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

    fn rebuild_camera_bind_groups(&mut self, device: &wgpu::Device) {
        self.camera_bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("perro_camera3d_bg"),
            layout: &self.camera_bgl,
            entries: &[
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: self.camera_buffer.as_entire_binding(),
                },
                wgpu::BindGroupEntry {
                    binding: 1,
                    resource: self.skeleton_buffer.as_entire_binding(),
                },
                wgpu::BindGroupEntry {
                    binding: 2,
                    resource: self.custom_params_buffer.as_entire_binding(),
                },
            ],
        });
        self.shadow_camera_bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("perro_shadow_camera3d_bg"),
            layout: &self.camera_bgl,
            entries: &[
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: self.shadow_camera_buffer.as_entire_binding(),
                },
                wgpu::BindGroupEntry {
                    binding: 1,
                    resource: self.skeleton_buffer.as_entire_binding(),
                },
                wgpu::BindGroupEntry {
                    binding: 2,
                    resource: self.custom_params_buffer.as_entire_binding(),
                },
            ],
        });
    }

    fn ensure_skeleton_capacity(&mut self, device: &wgpu::Device, needed: usize) {
        if needed <= self.skeleton_capacity {
            return;
        }
        let mut new_capacity = self.skeleton_capacity.max(1);
        while new_capacity < needed {
            new_capacity *= 2;
        }
        self.skeleton_buffer = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("perro_skeleton_palette_buffer"),
            size: (new_capacity * std::mem::size_of::<[[f32; 4]; 4]>()) as u64,
            usage: wgpu::BufferUsages::STORAGE | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });
        self.rebuild_camera_bind_groups(device);
        self.skeleton_capacity = new_capacity;
    }

    fn ensure_custom_params_capacity(&mut self, device: &wgpu::Device, needed: usize) {
        if needed <= self.custom_params_capacity {
            return;
        }
        let mut new_capacity = self.custom_params_capacity.max(1);
        while new_capacity < needed {
            new_capacity *= 2;
        }
        self.custom_params_buffer = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("perro_custom_material_params"),
            size: (new_capacity * std::mem::size_of::<[f32; 4]>()) as u64,
            usage: wgpu::BufferUsages::STORAGE | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });
        self.rebuild_camera_bind_groups(device);
        self.custom_params_capacity = new_capacity;
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

    fn build_hiz_from_depth(&self, encoder: &mut wgpu::CommandEncoder) {
        let Some(copy_bg) = self.hiz_copy_bind_group.as_ref() else {
            return;
        };
        {
            let mut pass = encoder.begin_compute_pass(&wgpu::ComputePassDescriptor {
                label: Some("perro_hiz_copy_pass"),
                timestamp_writes: None,
            });
            pass.set_pipeline(&self.hiz_copy_pipeline);
            pass.set_bind_group(0, copy_bg, &[]);
            let groups_x = self.hiz_size.0.div_ceil(HIZ_WORKGROUP_SIZE_X);
            let groups_y = self.hiz_size.1.div_ceil(HIZ_WORKGROUP_SIZE_Y);
            pass.dispatch_workgroups(groups_x, groups_y, 1);
        }
        let mut src_w = self.hiz_size.0;
        let mut src_h = self.hiz_size.1;
        for downsample_bg in &self.hiz_downsample_bind_groups {
            let dst_w = (src_w / 2).max(1);
            let dst_h = (src_h / 2).max(1);
            let mut pass = encoder.begin_compute_pass(&wgpu::ComputePassDescriptor {
                label: Some("perro_hiz_downsample_pass"),
                timestamp_writes: None,
            });
            pass.set_pipeline(&self.hiz_downsample_pipeline);
            pass.set_bind_group(0, downsample_bg, &[]);
            pass.dispatch_workgroups(
                dst_w.div_ceil(HIZ_WORKGROUP_SIZE_X),
                dst_h.div_ceil(HIZ_WORKGROUP_SIZE_Y),
                1,
            );
            src_w = dst_w;
            src_h = dst_h;
        }
    }

    fn rebuild_hiz_bind_groups(&mut self, device: &wgpu::Device) {
        if self.hiz_mip_views.is_empty() {
            self.hiz_copy_bind_group = None;
            self.hiz_downsample_bind_groups.clear();
            return;
        }

        self.hiz_copy_bind_group = Some(device.create_bind_group(&wgpu::BindGroupDescriptor {
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
        }));

        self.hiz_downsample_bind_groups.clear();
        self.hiz_downsample_bind_groups
            .reserve(self.hiz_mip_count.saturating_sub(1) as usize);
        for mip in 1..self.hiz_mip_count as usize {
            self.hiz_downsample_bind_groups
                .push(device.create_bind_group(&wgpu::BindGroupDescriptor {
                    label: Some("perro_hiz_downsample_bg"),
                    layout: &self.hiz_downsample_bgl,
                    entries: &[
                        wgpu::BindGroupEntry {
                            binding: 0,
                            resource: wgpu::BindingResource::TextureView(
                                &self.hiz_mip_views[mip - 1],
                            ),
                        },
                        wgpu::BindGroupEntry {
                            binding: 1,
                            resource: wgpu::BindingResource::TextureView(&self.hiz_mip_views[mip]),
                        },
                    ],
                }));
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
                surface_ranges: Arc::from([range]),
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
            surface_ranges: Arc::from([full]),
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
        let surface_ranges = if decoded.surface_ranges.is_empty() {
            vec![MeshRange {
                index_start,
                index_count,
                base_vertex: 0,
            }]
        } else {
            decoded
                .surface_ranges
                .iter()
                .copied()
                .map(|range| MeshRange {
                    index_start: index_start + range.index_start,
                    index_count: range.index_count,
                    base_vertex: 0,
                })
                .collect()
        };
        let added_vertices = decoded.vertices;
        let mut added_indices = Vec::with_capacity(decoded.indices.len());
        for idx in decoded.indices {
            added_indices.push(idx + base_vertex);
        }

        let new_vertex_len = self.mesh_vertices.len() + added_vertices.len();
        let new_index_len = self.mesh_indices.len() + added_indices.len();
        self.ensure_mesh_buffer_capacity(device, queue, new_vertex_len, new_index_len);

        let vertex_offset =
            self.mesh_vertices.len() as u64 * std::mem::size_of::<MeshVertex>() as u64;
        let index_offset = self.mesh_indices.len() as u64 * std::mem::size_of::<u32>() as u64;

        self.mesh_vertices.extend_from_slice(&added_vertices);
        self.mesh_indices.extend_from_slice(&added_indices);

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
            surface_ranges: Arc::from(surface_ranges),
            meshlets: Arc::from(meshlets),
            bounds_center,
            bounds_radius,
        })
    }

    fn ensure_mesh_buffer_capacity(
        &mut self,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        needed_vertices: usize,
        needed_indices: usize,
    ) {
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
            let old_vertex_buffer = self.vertex_buffer.clone();
            let old_index_buffer = self.index_buffer.clone();
            let old_vertex_size =
                self.mesh_vertices.len() as u64 * std::mem::size_of::<MeshVertex>() as u64;
            let old_index_size = self.mesh_indices.len() as u64 * std::mem::size_of::<u32>() as u64;
            self.vertex_buffer = device.create_buffer(&wgpu::BufferDescriptor {
                label: Some("perro_mesh_vertices"),
                size: (self.vertex_capacity * std::mem::size_of::<MeshVertex>()) as u64,
                usage: wgpu::BufferUsages::VERTEX
                    | wgpu::BufferUsages::COPY_DST
                    | wgpu::BufferUsages::COPY_SRC,
                mapped_at_creation: false,
            });
            self.index_buffer = device.create_buffer(&wgpu::BufferDescriptor {
                label: Some("perro_mesh_indices"),
                size: (self.index_capacity * std::mem::size_of::<u32>()) as u64,
                usage: wgpu::BufferUsages::INDEX
                    | wgpu::BufferUsages::COPY_DST
                    | wgpu::BufferUsages::COPY_SRC,
                mapped_at_creation: false,
            });
            if old_vertex_size > 0 || old_index_size > 0 {
                let mut encoder = device.create_command_encoder(&wgpu::CommandEncoderDescriptor {
                    label: Some("perro_mesh_buffer_growth_copy"),
                });
                if old_vertex_size > 0 {
                    encoder.copy_buffer_to_buffer(
                        &old_vertex_buffer,
                        0,
                        &self.vertex_buffer,
                        0,
                        old_vertex_size,
                    );
                }
                if old_index_size > 0 {
                    encoder.copy_buffer_to_buffer(
                        &old_index_buffer,
                        0,
                        &self.index_buffer,
                        0,
                        old_index_size,
                    );
                }
                queue.submit([encoder.finish()]);
            }
        }
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
            uv: v.uv,
            joints: v.joints,
            weights: v.weights,
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
    if vertices.iter().any(|v| !v.uv.iter().all(|c| c.is_finite())) {
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
        surface_ranges: Vec::new(),
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
    if version != 1 && version != 2 && version != 3 && version != 4 {
        return None;
    }
    let (vertex_count, index_count, surface_count, meshlet_count, raw_len, payload_start) =
        if version == 4 {
            if bytes.len() < 29 {
                return None;
            }
            (
                u32::from_le_bytes(bytes[9..13].try_into().ok()?) as usize,
                u32::from_le_bytes(bytes[13..17].try_into().ok()?) as usize,
                u32::from_le_bytes(bytes[17..21].try_into().ok()?) as usize,
                u32::from_le_bytes(bytes[21..25].try_into().ok()?) as usize,
                u32::from_le_bytes(bytes[25..29].try_into().ok()?) as usize,
                29usize,
            )
        } else {
            (
                u32::from_le_bytes(bytes[9..13].try_into().ok()?) as usize,
                u32::from_le_bytes(bytes[13..17].try_into().ok()?) as usize,
                0usize,
                u32::from_le_bytes(bytes[17..21].try_into().ok()?) as usize,
                u32::from_le_bytes(bytes[21..25].try_into().ok()?) as usize,
                25usize,
            )
        };
    let raw = decompress_zlib(&bytes[payload_start..]).ok()?;
    if raw.len() != raw_len {
        return None;
    }

    let vertex_stride = match version {
        1 => 24,
        2 => 48,
        3 => 56,
        _ => return None,
    };
    let vertex_bytes = vertex_count.checked_mul(vertex_stride)?;
    let index_bytes = index_count.checked_mul(4)?;
    let surface_bytes = surface_count.checked_mul(8)?;
    let meshlet_bytes = meshlet_count.checked_mul(24)?;
    let required = vertex_bytes
        .checked_add(index_bytes)?
        .checked_add(surface_bytes)?
        .checked_add(meshlet_bytes)?;
    if raw.len() < required {
        return None;
    }

    let mut vertices = Vec::with_capacity(vertex_count);
    for i in 0..vertex_count {
        let off = i * vertex_stride;
        let pos = [
            f32::from_le_bytes(raw[off..off + 4].try_into().ok()?),
            f32::from_le_bytes(raw[off + 4..off + 8].try_into().ok()?),
            f32::from_le_bytes(raw[off + 8..off + 12].try_into().ok()?),
        ];
        let normal = [
            f32::from_le_bytes(raw[off + 12..off + 16].try_into().ok()?),
            f32::from_le_bytes(raw[off + 16..off + 20].try_into().ok()?),
            f32::from_le_bytes(raw[off + 20..off + 24].try_into().ok()?),
        ];
        let uv = if version == 3 {
            [
                f32::from_le_bytes(raw[off + 24..off + 28].try_into().ok()?),
                f32::from_le_bytes(raw[off + 28..off + 32].try_into().ok()?),
            ]
        } else {
            [0.0, 0.0]
        };
        let (joints, weights) = if version == 3 {
            let joints = [
                u16::from_le_bytes(raw[off + 32..off + 34].try_into().ok()?),
                u16::from_le_bytes(raw[off + 34..off + 36].try_into().ok()?),
                u16::from_le_bytes(raw[off + 36..off + 38].try_into().ok()?),
                u16::from_le_bytes(raw[off + 38..off + 40].try_into().ok()?),
            ];
            let weights = [
                f32::from_le_bytes(raw[off + 40..off + 44].try_into().ok()?),
                f32::from_le_bytes(raw[off + 44..off + 48].try_into().ok()?),
                f32::from_le_bytes(raw[off + 48..off + 52].try_into().ok()?),
                f32::from_le_bytes(raw[off + 52..off + 56].try_into().ok()?),
            ];
            (joints, weights)
        } else if version == 2 {
            let joints = [
                u16::from_le_bytes(raw[off + 24..off + 26].try_into().ok()?),
                u16::from_le_bytes(raw[off + 26..off + 28].try_into().ok()?),
                u16::from_le_bytes(raw[off + 28..off + 30].try_into().ok()?),
                u16::from_le_bytes(raw[off + 30..off + 32].try_into().ok()?),
            ];
            let weights = [
                f32::from_le_bytes(raw[off + 32..off + 36].try_into().ok()?),
                f32::from_le_bytes(raw[off + 36..off + 40].try_into().ok()?),
                f32::from_le_bytes(raw[off + 40..off + 44].try_into().ok()?),
                f32::from_le_bytes(raw[off + 44..off + 48].try_into().ok()?),
            ];
            (joints, weights)
        } else {
            ([0, 0, 0, 0], [1.0, 0.0, 0.0, 0.0])
        };
        vertices.push(MeshVertex {
            pos,
            normal,
            uv,
            joints,
            weights,
        });
    }

    let mut indices = Vec::with_capacity(index_count);
    let index_start = vertex_bytes;
    for i in 0..index_count {
        let off = index_start + i * 4;
        indices.push(u32::from_le_bytes(raw[off..off + 4].try_into().ok()?));
    }
    let mut surface_ranges = Vec::with_capacity(surface_count);
    let surface_start = vertex_bytes + index_bytes;
    for i in 0..surface_count {
        let off = surface_start + i * 8;
        surface_ranges.push(MeshRange {
            index_start: u32::from_le_bytes(raw[off..off + 4].try_into().ok()?),
            index_count: u32::from_le_bytes(raw[off + 4..off + 8].try_into().ok()?),
            base_vertex: 0,
        });
    }
    let mut meshlets = Vec::with_capacity(meshlet_count);
    let meshlet_start = vertex_bytes + index_bytes + surface_bytes;
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
        surface_ranges,
        meshlets,
    })
}

fn decode_gltf_mesh(bytes: &[u8], mesh_index: usize) -> Option<DecodedMesh> {
    let (doc, buffers, _images) = gltf::import_slice(bytes).ok()?;
    let mesh = doc.meshes().nth(mesh_index)?;
    let mut vertices = Vec::new();
    let mut indices = Vec::new();
    let mut surface_ranges = Vec::new();

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
        let tex_coords: Vec<[f32; 2]> = reader
            .read_tex_coords(0)
            .map(|iter| iter.into_f32().collect())
            .unwrap_or_default();
        let joints: Vec<[u16; 4]> = reader
            .read_joints(0)
            .map(|iter| iter.into_u16().collect())
            .unwrap_or_default();
        let mut weights: Vec<[f32; 4]> = reader
            .read_weights(0)
            .map(|iter| iter.into_f32().collect())
            .unwrap_or_default();
        if weights.is_empty() && !joints.is_empty() {
            weights = vec![[1.0, 0.0, 0.0, 0.0]; joints.len()];
        }
        let base_vertex = vertices.len() as u32;
        for (i, position) in positions.iter().copied().enumerate() {
            let joint = joints.get(i).copied().unwrap_or([0, 0, 0, 0]);
            let mut weight = weights.get(i).copied().unwrap_or([1.0, 0.0, 0.0, 0.0]);
            let sum = weight.iter().copied().sum::<f32>();
            if sum > 0.0 {
                let inv = sum.recip();
                weight.iter_mut().for_each(|w| *w *= inv);
            } else {
                weight = [1.0, 0.0, 0.0, 0.0];
            }
            vertices.push(MeshVertex {
                pos: position,
                normal: normals.get(i).copied().unwrap_or([0.0, 1.0, 0.0]),
                uv: tex_coords.get(i).copied().unwrap_or([0.0, 0.0]),
                joints: joint,
                weights: weight,
            });
        }
        let surface_start = indices.len() as u32;
        if let Some(idx) = reader.read_indices() {
            indices.extend(idx.into_u32().map(|i| i + base_vertex));
        } else {
            indices.extend((0..positions.len() as u32).map(|i| i + base_vertex));
        }
        let surface_count = (indices.len() as u32).saturating_sub(surface_start);
        if surface_count > 0 {
            surface_ranges.push(MeshRange {
                index_start: surface_start,
                index_count: surface_count,
                base_vertex: 0,
            });
        }
    }
    if vertices.is_empty() || indices.is_empty() {
        return None;
    }
    Some(DecodedMesh {
        vertices,
        indices,
        surface_ranges,
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
    ranges: &AHashMap<&'static str, MeshRange>,
) -> AHashMap<&'static str, ([f32; 3], f32)> {
    let mut out = AHashMap::new();
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

fn create_shadow_map_texture(
    device: &wgpu::Device,
    size: u32,
) -> (wgpu::Texture, wgpu::TextureView) {
    let texture = device.create_texture(&wgpu::TextureDescriptor {
        label: Some("perro_shadow_map"),
        size: wgpu::Extent3d {
            width: size.max(1),
            height: size.max(1),
            depth_or_array_layers: 1,
        },
        mip_level_count: 1,
        sample_count: 1,
        dimension: wgpu::TextureDimension::D2,
        format: SHADOW_DEPTH_FORMAT,
        usage: wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::TEXTURE_BINDING,
        view_formats: &[],
    });
    let view = texture.create_view(&wgpu::TextureViewDescriptor::default());
    (texture, view)
}

fn create_cached_material_texture(
    device: &wgpu::Device,
    queue: &wgpu::Queue,
    layout: &wgpu::BindGroupLayout,
    rgba: Vec<u8>,
    width: u32,
    height: u32,
    source: String,
) -> CachedMaterialTexture {
    let width = width.max(1);
    let height = height.max(1);
    let texture = device.create_texture(&wgpu::TextureDescriptor {
        label: Some("perro_material_texture"),
        size: wgpu::Extent3d {
            width,
            height,
            depth_or_array_layers: 1,
        },
        mip_level_count: 1,
        sample_count: 1,
        dimension: wgpu::TextureDimension::D2,
        format: MATERIAL_TEXTURE_FORMAT,
        usage: wgpu::TextureUsages::TEXTURE_BINDING
            | wgpu::TextureUsages::COPY_DST
            | wgpu::TextureUsages::RENDER_ATTACHMENT,
        view_formats: &[],
    });
    queue.write_texture(
        wgpu::TexelCopyTextureInfo {
            texture: &texture,
            mip_level: 0,
            origin: wgpu::Origin3d::ZERO,
            aspect: wgpu::TextureAspect::All,
        },
        &rgba,
        wgpu::TexelCopyBufferLayout {
            offset: 0,
            bytes_per_row: Some(4 * width),
            rows_per_image: Some(height),
        },
        wgpu::Extent3d {
            width,
            height,
            depth_or_array_layers: 1,
        },
    );
    let view = texture.create_view(&wgpu::TextureViewDescriptor {
        label: Some("perro_material_texture_view"),
        ..Default::default()
    });
    let sampler = device.create_sampler(&wgpu::SamplerDescriptor {
        label: Some("perro_material_texture_sampler"),
        address_mode_u: wgpu::AddressMode::Repeat,
        address_mode_v: wgpu::AddressMode::Repeat,
        address_mode_w: wgpu::AddressMode::Repeat,
        mag_filter: wgpu::FilterMode::Linear,
        min_filter: wgpu::FilterMode::Linear,
        mipmap_filter: wgpu::MipmapFilterMode::Linear,
        anisotropy_clamp: 16,
        ..Default::default()
    });
    let bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
        label: Some("perro_material_texture_bg"),
        layout,
        entries: &[
            wgpu::BindGroupEntry {
                binding: 0,
                resource: wgpu::BindingResource::Sampler(&sampler),
            },
            wgpu::BindGroupEntry {
                binding: 1,
                resource: wgpu::BindingResource::TextureView(&view),
            },
        ],
    });
    CachedMaterialTexture {
        source,
        _texture: texture,
        _view: view,
        _sampler: sampler,
        bind_group,
    }
}

fn load_texture_rgba(source: &str) -> Option<(Vec<u8>, u32, u32)> {
    let (path, fragment) = split_source_fragment(source);
    if (path.ends_with(".glb") || path.ends_with(".gltf"))
        && let Some(texture_index) = parse_fragment_index(fragment, "tex")
            .or_else(|| parse_fragment_index(fragment, "texture"))
            .or_else(|| parse_fragment_index(fragment, "img"))
    {
        return decode_gltf_texture(path, texture_index as usize);
    }

    let bytes = load_asset(source).ok()?;
    let image = image::load_from_memory(&bytes).ok()?;
    let rgba = image.to_rgba8();
    let (w, h) = rgba.dimensions();
    Some((rgba.into_raw(), w.max(1), h.max(1)))
}

fn gltf_texture_source_from_mesh_source(mesh_source: &str, slot: u32) -> Option<String> {
    let (path, _) = split_source_fragment(mesh_source);
    if !(path.ends_with(".glb") || path.ends_with(".gltf")) {
        return None;
    }
    Some(format!("{path}:tex[{slot}]"))
}

fn decode_gltf_texture(source_path: &str, texture_index: usize) -> Option<(Vec<u8>, u32, u32)> {
    let bytes = load_asset(source_path).ok()?;
    let (doc, _buffers, images) = gltf::import_slice(&bytes).ok()?;
    let texture = doc.textures().nth(texture_index)?;
    let image_index = texture.source().index();
    let image = images.get(image_index)?;
    let (width, height) = (image.width.max(1), image.height.max(1));
    let rgba = match image.format {
        gltf::image::Format::R8G8B8A8 => image.pixels.clone(),
        gltf::image::Format::R8G8B8 => {
            let mut out = Vec::with_capacity((width * height * 4) as usize);
            for px in image.pixels.chunks_exact(3) {
                out.extend_from_slice(&[px[0], px[1], px[2], 255]);
            }
            out
        }
        gltf::image::Format::R8G8 => {
            let mut out = Vec::with_capacity((width * height * 4) as usize);
            for px in image.pixels.chunks_exact(2) {
                out.extend_from_slice(&[px[0], px[1], 0, 255]);
            }
            out
        }
        gltf::image::Format::R8 => {
            let mut out = Vec::with_capacity((width * height * 4) as usize);
            for &v in &image.pixels {
                out.extend_from_slice(&[v, v, v, 255]);
            }
            out
        }
        _ => return None,
    };
    Some((rgba, width, height))
}

fn decode_ptex(bytes: &[u8]) -> Option<(Vec<u8>, u32, u32)> {
    if bytes.len() < 20 || &bytes[0..4] != PTEX_MAGIC {
        return None;
    }
    let version = u32::from_le_bytes(bytes[4..8].try_into().ok()?);
    if version != 1 {
        return None;
    }
    let width = u32::from_le_bytes(bytes[8..12].try_into().ok()?);
    let height = u32::from_le_bytes(bytes[12..16].try_into().ok()?);
    let raw_len = u32::from_le_bytes(bytes[16..20].try_into().ok()?);
    if width == 0 || height == 0 {
        return None;
    }
    let expected_len = width.checked_mul(height)?.checked_mul(4)?;
    if raw_len != expected_len {
        return None;
    }
    let rgba = decompress_zlib(&bytes[20..]).ok()?;
    if rgba.len() != raw_len as usize {
        return None;
    }
    Some((rgba, width, height))
}

fn create_sky_pipeline(
    device: &wgpu::Device,
    pipeline_layout: &wgpu::PipelineLayout,
    shader: &wgpu::ShaderModule,
    color_format: wgpu::TextureFormat,
    sample_count: u32,
) -> wgpu::RenderPipeline {
    device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
        label: Some("perro_sky3d_pipeline"),
        layout: Some(pipeline_layout),
        vertex: wgpu::VertexState {
            module: shader,
            entry_point: Some("vs_main"),
            buffers: &[],
            compilation_options: Default::default(),
        },
        fragment: Some(wgpu::FragmentState {
            module: shader,
            entry_point: Some("fs_main"),
            targets: &[Some(wgpu::ColorTargetState {
                format: color_format,
                blend: Some(wgpu::BlendState::REPLACE),
                write_mask: wgpu::ColorWrites::ALL,
            })],
            compilation_options: Default::default(),
        }),
        primitive: wgpu::PrimitiveState {
            topology: wgpu::PrimitiveTopology::TriangleList,
            strip_index_format: None,
            front_face: wgpu::FrontFace::Ccw,
            cull_mode: None,
            unclipped_depth: false,
            polygon_mode: wgpu::PolygonMode::Fill,
            conservative: false,
        },
        depth_stencil: None,
        multisample: wgpu::MultisampleState {
            count: sample_count,
            mask: !0,
            alpha_to_coverage_enabled: false,
        },
        multiview_mask: None,
        cache: None,
    })
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
                        wgpu::VertexAttribute {
                            offset: 24,
                            shader_location: 12,
                            format: wgpu::VertexFormat::Float32x2,
                        },
                        wgpu::VertexAttribute {
                            offset: 32,
                            shader_location: 2,
                            format: wgpu::VertexFormat::Uint16x4,
                        },
                        wgpu::VertexAttribute {
                            offset: 40,
                            shader_location: 3,
                            format: wgpu::VertexFormat::Float32x4,
                        },
                    ],
                },
                wgpu::VertexBufferLayout {
                    array_stride: std::mem::size_of::<InstanceGpu>() as u64,
                    step_mode: wgpu::VertexStepMode::Instance,
                    attributes: &[
                        wgpu::VertexAttribute {
                            offset: 0,
                            shader_location: 4,
                            format: wgpu::VertexFormat::Float32x4,
                        },
                        wgpu::VertexAttribute {
                            offset: 16,
                            shader_location: 5,
                            format: wgpu::VertexFormat::Float32x4,
                        },
                        wgpu::VertexAttribute {
                            offset: 32,
                            shader_location: 6,
                            format: wgpu::VertexFormat::Float32x4,
                        },
                        wgpu::VertexAttribute {
                            offset: 48,
                            shader_location: 7,
                            format: wgpu::VertexFormat::Float32x4,
                        },
                        wgpu::VertexAttribute {
                            offset: 64,
                            shader_location: 8,
                            format: wgpu::VertexFormat::Float32x4,
                        },
                        wgpu::VertexAttribute {
                            offset: 80,
                            shader_location: 9,
                            format: wgpu::VertexFormat::Float32x3,
                        },
                        wgpu::VertexAttribute {
                            offset: 92,
                            shader_location: 10,
                            format: wgpu::VertexFormat::Float32x4,
                        },
                        wgpu::VertexAttribute {
                            offset: 108,
                            shader_location: 11,
                            format: wgpu::VertexFormat::Uint32x4,
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
                write_mask: wgpu::ColorWrites::RED
                    | wgpu::ColorWrites::GREEN
                    | wgpu::ColorWrites::BLUE,
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
            depth_write_enabled: Some(true),
            depth_compare: Some(wgpu::CompareFunction::LessEqual),
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

fn create_pipeline_overlay(
    device: &wgpu::Device,
    pipeline_layout: &wgpu::PipelineLayout,
    shader: &wgpu::ShaderModule,
    color_format: wgpu::TextureFormat,
    sample_count: u32,
    cull_mode: Option<wgpu::Face>,
) -> wgpu::RenderPipeline {
    device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
        label: Some("perro_mesh_pipeline_overlay"),
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
                        wgpu::VertexAttribute {
                            offset: 24,
                            shader_location: 12,
                            format: wgpu::VertexFormat::Float32x2,
                        },
                        wgpu::VertexAttribute {
                            offset: 32,
                            shader_location: 2,
                            format: wgpu::VertexFormat::Uint16x4,
                        },
                        wgpu::VertexAttribute {
                            offset: 40,
                            shader_location: 3,
                            format: wgpu::VertexFormat::Float32x4,
                        },
                    ],
                },
                wgpu::VertexBufferLayout {
                    array_stride: std::mem::size_of::<InstanceGpu>() as u64,
                    step_mode: wgpu::VertexStepMode::Instance,
                    attributes: &[
                        wgpu::VertexAttribute {
                            offset: 0,
                            shader_location: 4,
                            format: wgpu::VertexFormat::Float32x4,
                        },
                        wgpu::VertexAttribute {
                            offset: 16,
                            shader_location: 5,
                            format: wgpu::VertexFormat::Float32x4,
                        },
                        wgpu::VertexAttribute {
                            offset: 32,
                            shader_location: 6,
                            format: wgpu::VertexFormat::Float32x4,
                        },
                        wgpu::VertexAttribute {
                            offset: 48,
                            shader_location: 7,
                            format: wgpu::VertexFormat::Float32x4,
                        },
                        wgpu::VertexAttribute {
                            offset: 64,
                            shader_location: 8,
                            format: wgpu::VertexFormat::Float32x4,
                        },
                        wgpu::VertexAttribute {
                            offset: 80,
                            shader_location: 9,
                            format: wgpu::VertexFormat::Float32x3,
                        },
                        wgpu::VertexAttribute {
                            offset: 92,
                            shader_location: 10,
                            format: wgpu::VertexFormat::Float32x4,
                        },
                        wgpu::VertexAttribute {
                            offset: 108,
                            shader_location: 11,
                            format: wgpu::VertexFormat::Uint32x4,
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
                write_mask: wgpu::ColorWrites::RED
                    | wgpu::ColorWrites::GREEN
                    | wgpu::ColorWrites::BLUE,
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
            depth_write_enabled: Some(false),
            depth_compare: Some(wgpu::CompareFunction::Always),
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
                    attributes: &[
                        wgpu::VertexAttribute {
                            offset: 0,
                            shader_location: 0,
                            format: wgpu::VertexFormat::Float32x3,
                        },
                        wgpu::VertexAttribute {
                            offset: 32,
                            shader_location: 2,
                            format: wgpu::VertexFormat::Uint16x4,
                        },
                        wgpu::VertexAttribute {
                            offset: 40,
                            shader_location: 3,
                            format: wgpu::VertexFormat::Float32x4,
                        },
                    ],
                },
                wgpu::VertexBufferLayout {
                    array_stride: std::mem::size_of::<InstanceGpu>() as u64,
                    step_mode: wgpu::VertexStepMode::Instance,
                    attributes: &[
                        wgpu::VertexAttribute {
                            offset: 0,
                            shader_location: 4,
                            format: wgpu::VertexFormat::Float32x4,
                        },
                        wgpu::VertexAttribute {
                            offset: 16,
                            shader_location: 5,
                            format: wgpu::VertexFormat::Float32x4,
                        },
                        wgpu::VertexAttribute {
                            offset: 32,
                            shader_location: 6,
                            format: wgpu::VertexFormat::Float32x4,
                        },
                        wgpu::VertexAttribute {
                            offset: 48,
                            shader_location: 7,
                            format: wgpu::VertexFormat::Float32x4,
                        },
                        wgpu::VertexAttribute {
                            offset: 108,
                            shader_location: 11,
                            format: wgpu::VertexFormat::Uint32x4,
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
            depth_write_enabled: Some(true),
            depth_compare: Some(wgpu::CompareFunction::LessEqual),
            stencil: wgpu::StencilState::default(),
            bias: wgpu::DepthBiasState::default(),
        }),
        multisample: wgpu::MultisampleState::default(),
        multiview_mask: None,
        cache: None,
    })
}

fn create_shadow_depth_pipeline(
    device: &wgpu::Device,
    pipeline_layout: &wgpu::PipelineLayout,
    shader: &wgpu::ShaderModule,
    cull_mode: Option<wgpu::Face>,
) -> wgpu::RenderPipeline {
    device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
        label: Some("perro_shadow_depth_pipeline"),
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
                            offset: 32,
                            shader_location: 2,
                            format: wgpu::VertexFormat::Uint16x4,
                        },
                        wgpu::VertexAttribute {
                            offset: 40,
                            shader_location: 3,
                            format: wgpu::VertexFormat::Float32x4,
                        },
                    ],
                },
                wgpu::VertexBufferLayout {
                    array_stride: std::mem::size_of::<InstanceGpu>() as u64,
                    step_mode: wgpu::VertexStepMode::Instance,
                    attributes: &[
                        wgpu::VertexAttribute {
                            offset: 0,
                            shader_location: 4,
                            format: wgpu::VertexFormat::Float32x4,
                        },
                        wgpu::VertexAttribute {
                            offset: 16,
                            shader_location: 5,
                            format: wgpu::VertexFormat::Float32x4,
                        },
                        wgpu::VertexAttribute {
                            offset: 32,
                            shader_location: 6,
                            format: wgpu::VertexFormat::Float32x4,
                        },
                        wgpu::VertexAttribute {
                            offset: 48,
                            shader_location: 7,
                            format: wgpu::VertexFormat::Float32x4,
                        },
                        wgpu::VertexAttribute {
                            offset: 108,
                            shader_location: 11,
                            format: wgpu::VertexFormat::Uint32x4,
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
            format: SHADOW_DEPTH_FORMAT,
            depth_write_enabled: Some(true),
            depth_compare: Some(wgpu::CompareFunction::LessEqual),
            stencil: wgpu::StencilState::default(),
            bias: wgpu::DepthBiasState {
                constant: SHADOW_MAP_DEPTH_BIAS_CONST,
                slope_scale: SHADOW_MAP_DEPTH_BIAS_SLOPE,
                clamp: 0.0,
            },
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
    instance_start: u32,
    instance_count: u32,
    double_sided: bool,
    material_kind: MaterialPipelineKind,
    base_color_texture_slot: u32,
    local_bounds: ([f32; 3], f32),
    occlusion_query: Option<u32>,
    disable_hiz_occlusion: bool,
    casts_shadows: bool,
) {
    if instance_count == 0 {
        return;
    }
    let (local_center, local_radius) = local_bounds;
    draw_batches.push(DrawBatch {
        mesh,
        instance_start,
        instance_count,
        double_sided,
        material_kind,
        draw_on_top: false,
        base_color_texture_slot,
        local_center,
        local_radius: local_radius.max(0.0),
        occlusion_query,
        disable_hiz_occlusion,
        casts_shadows,
    });
}

#[inline]
fn build_instance(
    model: [[f32; 4]; 4],
    material: &perro_render_bridge::Material3D,
    debug_view: bool,
    debug_color: [f32; 4],
    skeleton_start: u32,
    skeleton_count: u32,
    custom_params_offset: u32,
    custom_params_len: u32,
) -> InstanceGpu {
    let (color, pbr_params, emissive_factor, debug_flags) = if debug_view {
        (debug_color, [0.5, 0.0, 1.0, 1.0], [0.0, 0.0, 0.0], 1u32)
    } else {
        match material {
            Material3D::Standard(params) => (
                params.base_color_factor,
                [
                    params.roughness_factor,
                    params.metallic_factor,
                    params.occlusion_strength,
                    params.normal_scale,
                ],
                params.emissive_factor,
                0u32,
            ),
            Material3D::Unlit(params) => (
                params.base_color_factor,
                [0.0, 0.0, 0.0, 0.0],
                params.emissive_factor,
                0u32,
            ),
            Material3D::Toon(params) => (
                params.base_color_factor,
                [
                    params.band_count as f32,
                    params.rim_strength,
                    params.outline_width,
                    0.0,
                ],
                params.emissive_factor,
                0u32,
            ),
            Material3D::Custom(_) => {
                let params = material.standard_params();
                (
                    params.base_color_factor,
                    [
                        params.roughness_factor,
                        params.metallic_factor,
                        params.occlusion_strength,
                        params.normal_scale,
                    ],
                    params.emissive_factor,
                    0u32,
                )
            }
        }
    };
    let params = material.standard_params();
    let material_flags = debug_flags | if params.flat_shading { 2u32 } else { 0u32 };

    InstanceGpu {
        model_row_0: [model[0][0], model[1][0], model[2][0], model[3][0]],
        model_row_1: [model[0][1], model[1][1], model[2][1], model[3][1]],
        model_row_2: [model[0][2], model[1][2], model[2][2], model[3][2]],
        color,
        pbr_params,
        emissive_factor,
        material_params: [
            params.alpha_mode as f32,
            params.alpha_cutoff,
            if params.double_sided { 1.0 } else { 0.0 },
            material_flags as f32,
        ],
        skeleton_params: [
            skeleton_start,
            skeleton_count,
            custom_params_offset,
            custom_params_len,
        ],
    }
}

#[inline]
fn model_cols_from_affine_rows(inst: &InstanceGpu) -> [[f32; 4]; 4] {
    [
        [
            inst.model_row_0[0],
            inst.model_row_1[0],
            inst.model_row_2[0],
            0.0,
        ],
        [
            inst.model_row_0[1],
            inst.model_row_1[1],
            inst.model_row_2[1],
            0.0,
        ],
        [
            inst.model_row_0[2],
            inst.model_row_1[2],
            inst.model_row_2[2],
            0.0,
        ],
        [
            inst.model_row_0[3],
            inst.model_row_1[3],
            inst.model_row_2[3],
            1.0,
        ],
    ]
}

#[inline]
fn encode_custom_param_value(value: &perro_render_bridge::CustomMaterialParamValue3D) -> [f32; 4] {
    match value {
        perro_render_bridge::CustomMaterialParamValue3D::F32(v) => [*v, 0.0, 0.0, 0.0],
        perro_render_bridge::CustomMaterialParamValue3D::I32(v) => [*v as f32, 0.0, 0.0, 0.0],
        perro_render_bridge::CustomMaterialParamValue3D::Bool(v) => {
            [if *v { 1.0 } else { 0.0 }, 0.0, 0.0, 0.0]
        }
        perro_render_bridge::CustomMaterialParamValue3D::Vec2(v) => [v[0], v[1], 0.0, 0.0],
        perro_render_bridge::CustomMaterialParamValue3D::Vec3(v) => [v[0], v[1], v[2], 0.0],
        perro_render_bridge::CustomMaterialParamValue3D::Vec4(v) => *v,
    }
}

fn apply_surface_binding(mut material: Material3D, surface: &MeshSurfaceBinding3D) -> Material3D {
    apply_modulate(&mut material, surface.modulate);
    apply_overrides(&mut material, &surface.overrides);
    material
}

fn apply_modulate(material: &mut Material3D, modulate: [f32; 4]) {
    match material {
        Material3D::Standard(m) => {
            for (dst, src) in m.base_color_factor.iter_mut().zip(modulate) {
                *dst *= src;
            }
        }
        Material3D::Unlit(m) => {
            for (dst, src) in m.base_color_factor.iter_mut().zip(modulate) {
                *dst *= src;
            }
        }
        Material3D::Toon(m) => {
            for (dst, src) in m.base_color_factor.iter_mut().zip(modulate) {
                *dst *= src;
            }
        }
        Material3D::Custom(_) => {}
    }
}

fn apply_overrides(material: &mut Material3D, overrides: &[MaterialParamOverride3D]) {
    if overrides.is_empty() {
        return;
    }
    match material {
        Material3D::Standard(standard) => {
            for ovr in overrides {
                apply_flat_shading_override(&ovr.name, &ovr.value, &mut standard.flat_shading);
            }
        }
        Material3D::Unlit(unlit) => {
            for ovr in overrides {
                apply_flat_shading_override(&ovr.name, &ovr.value, &mut unlit.flat_shading);
            }
        }
        Material3D::Toon(toon) => {
            for ovr in overrides {
                apply_flat_shading_override(&ovr.name, &ovr.value, &mut toon.flat_shading);
            }
        }
        Material3D::Custom(custom) => {
            let mut params = custom.params.clone().into_owned();
            for ovr in overrides {
                params.push(perro_render_bridge::CustomMaterialParam3D {
                    name: Some(ovr.name.clone()),
                    value: match ovr.value {
                        MaterialParamOverrideValue3D::F32(v) => {
                            perro_render_bridge::CustomMaterialParamValue3D::F32(v)
                        }
                        MaterialParamOverrideValue3D::I32(v) => {
                            perro_render_bridge::CustomMaterialParamValue3D::I32(v)
                        }
                        MaterialParamOverrideValue3D::Bool(v) => {
                            perro_render_bridge::CustomMaterialParamValue3D::Bool(v)
                        }
                        MaterialParamOverrideValue3D::Vec2(v) => {
                            perro_render_bridge::CustomMaterialParamValue3D::Vec2(v)
                        }
                        MaterialParamOverrideValue3D::Vec3(v) => {
                            perro_render_bridge::CustomMaterialParamValue3D::Vec3(v)
                        }
                        MaterialParamOverrideValue3D::Vec4(v) => {
                            perro_render_bridge::CustomMaterialParamValue3D::Vec4(v)
                        }
                    },
                });
            }
            custom.params = Cow::Owned(params);
        }
    }
}

fn apply_flat_shading_override(
    name: &str,
    value: &MaterialParamOverrideValue3D,
    flat_shading: &mut bool,
) {
    let Some(v) = override_bool(value) else {
        return;
    };
    match name {
        "flat_shading" | "flatShading" | "shade_flat" | "shadeFlat" => {
            *flat_shading = v;
        }
        "shade_smooth" | "shadeSmooth" => {
            *flat_shading = !v;
        }
        _ => {}
    }
}

fn override_bool(value: &MaterialParamOverrideValue3D) -> Option<bool> {
    match value {
        MaterialParamOverrideValue3D::Bool(v) => Some(*v),
        MaterialParamOverrideValue3D::I32(v) => Some(*v != 0),
        MaterialParamOverrideValue3D::F32(v) => Some(*v > 0.5),
        _ => None,
    }
}

#[inline]
fn compare_draw_batch_keys(a: &DrawBatch, b: &DrawBatch) -> Ordering {
    a.draw_on_top
        .cmp(&b.draw_on_top)
        .then_with(|| a.double_sided.cmp(&b.double_sided))
        .then_with(|| compare_material_pipeline_kind(&a.material_kind, &b.material_kind))
        .then_with(|| a.base_color_texture_slot.cmp(&b.base_color_texture_slot))
        .then_with(|| a.mesh.index_start.cmp(&b.mesh.index_start))
        .then_with(|| a.mesh.base_vertex.cmp(&b.mesh.base_vertex))
        .then_with(|| a.instance_start.cmp(&b.instance_start))
}

#[inline]
fn compare_material_pipeline_kind(a: &MaterialPipelineKind, b: &MaterialPipelineKind) -> Ordering {
    material_pipeline_kind_rank(a)
        .cmp(&material_pipeline_kind_rank(b))
        .then_with(|| match (a, b) {
            (MaterialPipelineKind::Custom(ka), MaterialPipelineKind::Custom(kb)) => ka.cmp(kb),
            _ => Ordering::Equal,
        })
}

#[inline]
fn material_pipeline_kind_rank(kind: &MaterialPipelineKind) -> u8 {
    match kind {
        MaterialPipelineKind::Standard => 0,
        MaterialPipelineKind::Unlit => 1,
        MaterialPipelineKind::Toon => 2,
        MaterialPipelineKind::Custom(_) => 3,
    }
}

#[inline]
fn same_draw_except_model(a: &Draw3DInstance, b: &Draw3DInstance) -> bool {
    a.node == b.node && a.kind == b.kind && a.surfaces == b.surfaces && a.skeleton == b.skeleton
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

fn compute_view_proj(camera: &Camera3DState, width: u32, height: u32) -> [[f32; 4]; 4] {
    compute_view_proj_mat(camera, width, height).to_cols_array_2d()
}

fn compute_view_proj_mat(camera: &Camera3DState, width: u32, height: u32) -> Mat4 {
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
    camera: &Camera3DState,
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
        ray_lights: [RayLightGpu {
            direction: [0.0, 0.0, -1.0, 0.0],
            color_intensity: [1.0, 1.0, 1.0, 0.0],
        }; MAX_RAY_LIGHTS],
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

    if let Some(sky) = lighting.sky.as_ref() {
        let day_color = sample_gradient(sky.day_colors.as_ref(), 0.55);
        let evening_color = sample_gradient(sky.evening_colors.as_ref(), 0.55);
        let night_color = sample_gradient(sky.night_colors.as_ref(), 0.55);
        let t_day = day_weight_from_time(sky.time.time_of_day);
        let t_evening = evening_weight_from_time(sky.time.time_of_day);
        let ambient_rgb = lerp3(
            lerp3(night_color, day_color, t_day),
            evening_color,
            t_evening,
        );
        let ambient_strength = (0.08 + 0.32 * t_day).max(0.0);
        scene.ambient_color = [
            ambient_rgb[0].max(0.0),
            ambient_rgb[1].max(0.0),
            ambient_rgb[2].max(0.0),
            ambient_strength,
        ];
    }

    if let Some(ambient) = lighting.ambient_light {
        scene.ambient_color = [
            ambient.color[0].max(0.0),
            ambient.color[1].max(0.0),
            ambient.color[2].max(0.0),
            ambient.intensity.max(0.0),
        ];
    }

    let mut ray_count = 0usize;
    let mut push_ray = |dir: Vec3, color: [f32; 3], intensity: f32| {
        if ray_count >= MAX_RAY_LIGHTS {
            return;
        }
        if intensity <= 1.0e-4 {
            return;
        }
        let d = dir.normalize_or_zero();
        if d.length_squared() <= 1.0e-6 || !d.is_finite() {
            return;
        }
        scene.ray_lights[ray_count] = RayLightGpu {
            direction: [d.x, d.y, d.z, 0.0],
            color_intensity: [
                color[0].max(0.0),
                color[1].max(0.0),
                color[2].max(0.0),
                intensity.max(0.0),
            ],
        };
        ray_count += 1;
    };

    if DEBUG_FORCE_WORLD_SUN_DIR {
        let d = Vec3::new(
            DEBUG_WORLD_SUN_DIR[0],
            DEBUG_WORLD_SUN_DIR[1],
            DEBUG_WORLD_SUN_DIR[2],
        )
        .normalize_or_zero();
        push_ray(d, [1.0, 0.98, 0.92], 1.0);
    }

    let has_explicit_rays = lighting
        .ray_lights
        .iter()
        .flatten()
        .any(|ray| ray.intensity > 1.0e-4);

    // Prefer authored directional lights when present.
    if !DEBUG_FORCE_WORLD_SUN_DIR {
        for ray in lighting.ray_lights.iter().flatten() {
            if !ray.cast_shadows {
                continue;
            }
            push_ray(Vec3::from(ray.direction), ray.color, ray.intensity);
        }
        for ray in lighting.ray_lights.iter().flatten() {
            if ray.cast_shadows {
                continue;
            }
            push_ray(Vec3::from(ray.direction), ray.color, ray.intensity);
        }
    }

    // Only synthesize sky sun/moon directional lights when no explicit rays exist.
    if !DEBUG_FORCE_WORLD_SUN_DIR
        && !has_explicit_rays
        && let Some(sky) = lighting.sky.as_ref()
    {
        let (sun_body_dir, moon_body_dir) =
            sun_moon_dirs_from_time(sky.time.time_of_day, sky.sky_angle);
        // Sky returns body position directions (origin -> sun/moon).
        // Ray-light direction stores light travel direction (light -> world), so invert.
        let sun_dir = -sun_body_dir;
        let moon_dir = -moon_body_dir;
        let day_amt = day_weight_from_time(sky.time.time_of_day).powf(1.20);
        let dusk_amt = evening_weight_from_time(sky.time.time_of_day) * (1.0 - day_amt * 0.55);
        let night_amt = (1.0 - day_amt).clamp(0.0, 1.0);

        let day_col = sample_gradient(sky.day_colors.as_ref(), 0.58);
        let eve_col = sample_gradient(sky.evening_colors.as_ref(), 0.52);
        let night_col = sample_gradient(sky.night_colors.as_ref(), 0.62);

        let sun_color = [
            day_col[0] + (eve_col[0] - day_col[0]) * (dusk_amt * 0.90),
            day_col[1] + (eve_col[1] - day_col[1]) * (dusk_amt * 0.90),
            day_col[2] + (eve_col[2] - day_col[2]) * (dusk_amt * 0.90),
        ];
        let sun_visibility = horizon_visibility(sun_body_dir.y);
        let sun_intensity =
            (((day_amt * 1.35) + (dusk_amt * 0.22)) * sky.sun_size.max(0.1) * sun_visibility)
                .max(0.0);

        let moon_color = [
            night_col[0] * 0.80,
            night_col[1] * 0.88,
            (night_col[2] * 1.05).max(0.0),
        ];
        let moon_visibility = horizon_visibility(moon_body_dir.y);
        let moon_intensity =
            ((night_amt * 0.18) * sky.moon_size.max(0.05) * moon_visibility).max(0.0);

        push_ray(sun_dir, sun_color, sun_intensity);
        push_ray(moon_dir, moon_color, moon_intensity);
    }

    scene.ambient_and_counts[0] = ray_count as f32;
    scene.ambient_and_counts[3] = if ray_count > 0 { 1.0 } else { 0.0 };
    if ray_count > 0 {
        scene.ray_light = scene.ray_lights[0];
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

fn build_shadow_setup(
    camera: &Camera3DState,
    lighting: &Lighting3DState,
    draw_batches: &[DrawBatch],
    staged_instances: &[InstanceGpu],
    fallback_focus_center: Vec3,
    fallback_focus_radius: f32,
    viewport_width: u32,
    viewport_height: u32,
) -> (Scene3DUniform, ShadowUniform, bool, Vec3, f32) {
    let mut shadow_scene = Scene3DUniform::zeroed();
    let mut shadow_uniform = ShadowUniform::zeroed();
    if TEMP_DISABLE_SHADOWS {
        return (
            shadow_scene,
            shadow_uniform,
            false,
            fallback_focus_center,
            fallback_focus_radius,
        );
    }

    let explicit_shadow_ray = lighting
        .ray_lights
        .iter()
        .flatten()
        .copied()
        .find(|light| light.cast_shadows && light.intensity > 1.0e-4);

    let sky_shadow_dir = lighting.sky.as_ref().and_then(|sky| {
        let (sun_body_dir, moon_body_dir) =
            sun_moon_dirs_from_time(sky.time.time_of_day, sky.sky_angle);
        let sun_dir = -sun_body_dir;
        let moon_dir = -moon_body_dir;
        let day_amt = day_weight_from_time(sky.time.time_of_day).powf(1.20);
        let dusk_amt = evening_weight_from_time(sky.time.time_of_day) * (1.0 - day_amt * 0.55);
        let night_amt = (1.0 - day_amt).clamp(0.0, 1.0);
        let sun_intensity = (((day_amt * 1.35) + (dusk_amt * 0.22))
            * sky.sun_size.max(0.1)
            * horizon_visibility(sun_body_dir.y))
        .max(0.0);
        let moon_intensity =
            ((night_amt * 0.18) * sky.moon_size.max(0.05) * horizon_visibility(moon_body_dir.y))
                .max(0.0);
        if sun_intensity > 1.0e-4 {
            Some(sun_dir)
        } else if moon_intensity > 1.0e-4 {
            Some(moon_dir)
        } else {
            None
        }
    });

    // Prefer authored directional lights when present.
    let dir = if DEBUG_FORCE_WORLD_SUN_DIR {
        Vec3::new(
            DEBUG_WORLD_SUN_DIR[0],
            DEBUG_WORLD_SUN_DIR[1],
            DEBUG_WORLD_SUN_DIR[2],
        )
        .normalize_or_zero()
    } else if let Some(ray) = explicit_shadow_ray {
        Vec3::from(ray.direction).normalize_or_zero()
    } else if let Some(dir) = sky_shadow_dir {
        dir.normalize_or_zero()
    } else {
        return (
            shadow_scene,
            shadow_uniform,
            false,
            fallback_focus_center,
            fallback_focus_radius,
        );
    };
    if dir.length_squared() <= 1.0e-6 || !dir.is_finite() {
        return (
            shadow_scene,
            shadow_uniform,
            false,
            fallback_focus_center,
            fallback_focus_radius,
        );
    }

    let has_casters = draw_batches
        .iter()
        .any(|batch| !batch.draw_on_top && batch.casts_shadows);
    if !has_casters {
        return (
            shadow_scene,
            shadow_uniform,
            false,
            fallback_focus_center,
            fallback_focus_radius,
        );
    }

    let (batch_focus_center, batch_focus_radius, has_batch_bounds) =
        compute_shadow_focus_bounds(camera, draw_batches, staged_instances);

    let Some(mut frustum_corners) =
        camera_frustum_corners_world(camera, viewport_width, viewport_height)
    else {
        return (
            shadow_scene,
            shadow_uniform,
            false,
            fallback_focus_center,
            fallback_focus_radius,
        );
    };

    // Clamp shadow coverage depth for stability/quality.
    let camera_pos = Vec3::from(camera.position);
    let max_shadow_distance = 220.0f32;
    for corner in &mut frustum_corners {
        let to = *corner - camera_pos;
        let d = to.length();
        if d.is_finite() && d > max_shadow_distance && d > 1.0e-4 {
            *corner = camera_pos + to * (max_shadow_distance / d);
        }
    }

    let mut focus_center = frustum_corners
        .iter()
        .copied()
        .fold(Vec3::ZERO, |acc, p| acc + p)
        / (frustum_corners.len() as f32);
    let mut focus_radius = frustum_corners
        .iter()
        .copied()
        .map(|p| (p - focus_center).length())
        .fold(0.0f32, f32::max)
        .clamp(10.0, 600.0);
    if has_batch_bounds {
        focus_center = focus_center.lerp(batch_focus_center, 0.35);
        focus_radius = focus_radius
            .max(batch_focus_radius * 0.85)
            .clamp(10.0, 600.0);
    }

    if fallback_focus_center.is_finite() && fallback_focus_radius.is_finite() {
        let t = 0.20;
        focus_center = fallback_focus_center.lerp(focus_center, t);
        let current = fallback_focus_radius.max(10.0);
        let target = focus_radius.max(10.0);
        focus_radius = (current + (target - current) * t).clamp(10.0, 600.0);
    }

    let up = if dir.dot(Vec3::Y).abs() > 0.95 {
        Vec3::Z
    } else {
        Vec3::Y
    };
    let distance = (focus_radius * 3.0).max(80.0);
    let mut eye = focus_center - dir * distance;
    let (right_axis, up_axis) = light_stable_axes(dir, up);

    let mut view = Mat4::look_at_rh(eye, focus_center, up);
    let Some((mut ls_min, mut ls_max)) = light_space_bounds(&frustum_corners, view) else {
        return (
            shadow_scene,
            shadow_uniform,
            false,
            focus_center,
            focus_radius,
        );
    };

    let mut span_x = (ls_max.x - ls_min.x).max(2.0);
    let mut span_y = (ls_max.y - ls_min.y).max(2.0);
    let xy_pad = (span_x.max(span_y) * 0.08).max(2.0);
    ls_min.x -= xy_pad;
    ls_max.x += xy_pad;
    ls_min.y -= xy_pad;
    ls_max.y += xy_pad;
    span_x = (ls_max.x - ls_min.x).max(2.0);
    span_y = (ls_max.y - ls_min.y).max(2.0);

    // Snap projection center in light-space texels for temporal stability.
    let wupt_x = (span_x / SHADOW_MAP_SIZE as f32).max(1.0e-6);
    let wupt_y = (span_y / SHADOW_MAP_SIZE as f32).max(1.0e-6);
    let center_ls_x = (ls_min.x + ls_max.x) * 0.5;
    let center_ls_y = (ls_min.y + ls_max.y) * 0.5;
    let snapped_ls_x = (center_ls_x / wupt_x).round() * wupt_x;
    let snapped_ls_y = (center_ls_y / wupt_y).round() * wupt_y;
    let center_delta =
        right_axis * (snapped_ls_x - center_ls_x) + up_axis * (snapped_ls_y - center_ls_y);
    focus_center += center_delta;
    eye += center_delta;
    view = Mat4::look_at_rh(eye, focus_center, up);

    let Some((mut ls_min, mut ls_max)) = light_space_bounds(&frustum_corners, view) else {
        return (
            shadow_scene,
            shadow_uniform,
            false,
            focus_center,
            focus_radius,
        );
    };
    let span_x = (ls_max.x - ls_min.x).max(2.0);
    let span_y = (ls_max.y - ls_min.y).max(2.0);
    let xy_pad = (span_x.max(span_y) * 0.08).max(2.0);
    ls_min.x -= xy_pad;
    ls_max.x += xy_pad;
    ls_min.y -= xy_pad;
    ls_max.y += xy_pad;

    let z_pad = (focus_radius * 0.45).max(12.0);
    let near = (-ls_max.z - z_pad).max(0.1);
    let far = (-ls_min.z + z_pad).max(near + 1.0);
    let proj = Mat4::orthographic_rh(ls_min.x, ls_max.x, ls_min.y, ls_max.y, near, far);
    let light_view_proj = proj * view;
    if !light_view_proj.is_finite() {
        return (
            shadow_scene,
            shadow_uniform,
            false,
            focus_center,
            focus_radius,
        );
    }

    shadow_scene.view_proj = light_view_proj.to_cols_array_2d();
    shadow_uniform.light_view_proj = shadow_scene.view_proj;
    // No falloff debug mode: very small constant bias for contact shadows.
    // params0 = [enabled, strength, depth_bias, normal_bias]
    shadow_uniform.params0 = [1.0, 1.0, 0.00002, 0.0];

    (
        shadow_scene,
        shadow_uniform,
        true,
        focus_center,
        focus_radius,
    )
}

fn camera_frustum_corners_world(
    camera: &Camera3DState,
    width: u32,
    height: u32,
) -> Option<Vec<Vec3>> {
    let view_proj = compute_view_proj_mat(camera, width, height);
    if !view_proj.is_finite() {
        return None;
    }
    let inv = view_proj.inverse();
    if !inv.is_finite() {
        return None;
    }
    let mut corners = Vec::with_capacity(8);
    for z in [-1.0f32, 1.0f32] {
        for y in [-1.0f32, 1.0f32] {
            for x in [-1.0f32, 1.0f32] {
                let clip = Vec4::new(x, y, z, 1.0);
                let world_h = inv * clip;
                if !world_h.is_finite() || world_h.w.abs() <= 1.0e-6 {
                    return None;
                }
                corners.push(world_h.truncate() / world_h.w);
            }
        }
    }
    Some(corners)
}

fn light_space_bounds(points_world: &[Vec3], light_view: Mat4) -> Option<(Vec3, Vec3)> {
    let mut it = points_world.iter().copied();
    let first = it.next()?;
    let first_ls = (light_view * first.extend(1.0)).truncate();
    if !first_ls.is_finite() {
        return None;
    }
    let mut min = first_ls;
    let mut max = first_ls;
    for p in it {
        let ls = (light_view * p.extend(1.0)).truncate();
        if !ls.is_finite() {
            continue;
        }
        min = min.min(ls);
        max = max.max(ls);
    }
    if !min.is_finite() || !max.is_finite() {
        None
    } else {
        Some((min, max))
    }
}

fn compute_shadow_focus_bounds(
    camera: &Camera3DState,
    draw_batches: &[DrawBatch],
    staged_instances: &[InstanceGpu],
) -> (Vec3, f32, bool) {
    let mut any = false;
    let mut min = Vec3::splat(f32::INFINITY);
    let mut max = Vec3::splat(f32::NEG_INFINITY);

    for batch in draw_batches {
        if batch.draw_on_top || !batch.casts_shadows {
            continue;
        }
        let start = batch.instance_start as usize;
        let end = (batch.instance_start + batch.instance_count) as usize;
        for inst in staged_instances.get(start..end).unwrap_or(&[]).iter() {
            let model_cols = model_cols_from_affine_rows(inst);
            let model = Mat4::from_cols_array_2d(&model_cols);
            if !model.is_finite() {
                continue;
            }
            let local_center = Vec3::new(
                batch.local_center[0],
                batch.local_center[1],
                batch.local_center[2],
            );
            let center_world = (model * local_center.extend(1.0)).truncate();
            let sx = Vec3::new(model.x_axis.x, model.x_axis.y, model.x_axis.z).length();
            let sy = Vec3::new(model.y_axis.x, model.y_axis.y, model.y_axis.z).length();
            let sz = Vec3::new(model.z_axis.x, model.z_axis.y, model.z_axis.z).length();
            let scale = sx.max(sy).max(sz).max(1.0e-6);
            let radius_world = (batch.local_radius.max(0.0) * scale).max(0.25);
            let r = Vec3::splat(radius_world);
            min = min.min(center_world - r);
            max = max.max(center_world + r);
            any = true;
        }
    }

    if !any {
        return (Vec3::from(camera.position), 64.0, false);
    }

    let center = (min + max) * 0.5;
    let radius = ((max - min) * 0.5).length().clamp(10.0, 600.0);
    (center, radius, true)
}

fn light_stable_axes(light_dir: Vec3, fallback_up: Vec3) -> (Vec3, Vec3) {
    let f = light_dir.normalize_or_zero();
    let mut right = f.cross(fallback_up).normalize_or_zero();
    if right.length_squared() <= 1.0e-6 {
        let alt_up = if f.dot(Vec3::Y).abs() > 0.95 {
            Vec3::X
        } else {
            Vec3::Y
        };
        right = f.cross(alt_up).normalize_or_zero();
    }
    let up = right.cross(f).normalize_or_zero();
    (right, up)
}

fn build_sky_uniform(
    camera: &Camera3DState,
    lighting: &Lighting3DState,
    width: u32,
    height: u32,
) -> Option<SkyUniform> {
    let sky = lighting.sky.as_ref()?;
    let view_proj = compute_view_proj_mat(camera, width, height);
    let inv_view_proj = view_proj.inverse();
    let inv = if inv_view_proj.is_finite() {
        inv_view_proj.to_cols_array_2d()
    } else {
        Mat4::IDENTITY.to_cols_array_2d()
    };
    let t_day = day_weight_from_time(sky.time.time_of_day);
    let day_colors = gradient_triplet(sky.day_colors.as_ref());
    let evening_colors = gradient_triplet(sky.evening_colors.as_ref());
    let night_colors = gradient_triplet(sky.night_colors.as_ref());
    Some(SkyUniform {
        inv_view_proj: inv,
        camera_pos: [
            camera.position[0],
            camera.position[1],
            camera.position[2],
            0.0,
        ],
        day_colors,
        evening_colors,
        night_colors,
        params0: [
            sky.cloud_size.max(0.0),
            sky.cloud_density.clamp(0.0, 1.0),
            sky.cloud_variance.clamp(0.0, 1.0),
            sky.time.time_of_day.rem_euclid(1.0),
        ],
        params1: [
            sky.star_size.max(0.0),
            sky.star_scatter.clamp(0.0, 1.0),
            sky.star_gleam.max(0.0),
            sky.sky_angle,
        ],
        params2: [
            sky.sun_size.max(0.0),
            sky.moon_size.max(0.0),
            t_day,
            lighting.sky_cloud_time_seconds.max(0.0),
        ],
        wind: [
            sky.cloud_wind_vector[0],
            sky.cloud_wind_vector[1],
            sky.style_blend.clamp(0.0, 1.0),
            0.0,
        ],
    })
}

fn gradient_triplet(colors: &[[f32; 3]]) -> [[f32; 4]; 3] {
    if colors.is_empty() {
        return [[0.0, 0.0, 0.0, 1.0]; 3];
    }
    if colors.len() == 1 {
        return [
            [colors[0][0], colors[0][1], colors[0][2], 1.0],
            [colors[0][0], colors[0][1], colors[0][2], 1.0],
            [colors[0][0], colors[0][1], colors[0][2], 1.0],
        ];
    }
    let first = colors[0];
    let middle = sample_gradient(colors, 0.5);
    let last = colors[colors.len() - 1];
    [
        [first[0], first[1], first[2], 1.0],
        [middle[0], middle[1], middle[2], 1.0],
        [last[0], last[1], last[2], 1.0],
    ]
}

fn day_weight_from_time(time_of_day: f32) -> f32 {
    let t = time_of_day.rem_euclid(1.0);
    let a = (t * std::f32::consts::TAU) - std::f32::consts::FRAC_PI_2;
    ((a.sin() + 1.0) * 0.5).clamp(0.0, 1.0)
}

fn evening_weight_from_time(time_of_day: f32) -> f32 {
    let t = time_of_day.rem_euclid(1.0);
    let dist = ((t - 0.75 + 0.5).rem_euclid(1.0) - 0.5).abs();
    (1.0 - (dist / 0.23)).clamp(0.0, 1.0)
}

fn horizon_visibility(y: f32) -> f32 {
    let t = ((y + 0.08) / 0.16).clamp(0.0, 1.0);
    t * t * (3.0 - 2.0 * t)
}

fn sun_moon_dirs_from_time(time_of_day: f32, sky_angle: f32) -> (Vec3, Vec3) {
    let t = time_of_day.rem_euclid(1.0);
    let theta = (t * std::f32::consts::TAU) - std::f32::consts::FRAC_PI_2 + sky_angle;
    let sun = Vec3::new(theta.cos(), theta.sin(), -0.25).normalize_or_zero();
    let moon = -sun;
    (sun, moon)
}

fn sample_gradient(colors: &[[f32; 3]], t: f32) -> [f32; 3] {
    if colors.is_empty() {
        return [0.0, 0.0, 0.0];
    }
    if colors.len() == 1 {
        return colors[0];
    }
    let n = colors.len() - 1;
    let f = t.clamp(0.0, 1.0) * n as f32;
    let i = f.floor() as usize;
    let j = (i + 1).min(n);
    let u = (f - i as f32).clamp(0.0, 1.0);
    lerp3(colors[i], colors[j], u)
}

fn lerp3(a: [f32; 3], b: [f32; 3], t: f32) -> [f32; 3] {
    [
        a[0] + (b[0] - a[0]) * t,
        a[1] + (b[1] - a[1]) * t,
        a[2] + (b[2] - a[2]) * t,
    ]
}
