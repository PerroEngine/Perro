use super::water_flip_gpu::GpuWaterFlip;
use crate::gpu_shrink::{ShrinkTracker, shrink_buffer_preserving};
use bytemuck::{Pod, Zeroable};
use glam::{Mat4, Vec3, Vec4};
use perro_ids::NodeID;
use perro_render_bridge::{
    Water2DState, Water3DState, WaterBodyQueryState, WaterBodySampleState, WaterCoastlineShape2D,
    WaterCoastlineShape3D, WaterIdleModeState, WaterSampleState, WaterShapeState,
};
use perro_structs::WaterQuality;
use std::collections::HashMap;
use std::hash::{BuildHasher, Hash, Hasher};
use std::sync::mpsc;

const WATER_WORKGROUP_SIZE: u32 = 64;
const WATER_MAX_RENDER_RESOLUTION: u32 = 1024;
const WATER_FLAG_DEBUG: u32 = 1 << 0;
const WATER_FLAG_PAUSED: u32 = 1 << 1;
const WATER_COASTLINE_INSET_METERS: f32 = 1.0;
const WATER_3D_MAX_RENDER_RESOLUTION: u32 = 256;
// Frames without any 3D water before the scene-color refraction copy target
// (half-res non-MSAA, full-res MSAA) releases back to 1x1 (2D-only water
// never reads it).
const WATER_SCENE_COLOR_IDLE_RELEASE_FRAMES: u32 = 120;

#[inline]
fn scene_color_capture_cache_hit(cached_key: Option<u64>, source_view_key: u64) -> bool {
    cached_key == Some(source_view_key)
}

#[repr(C)]
#[derive(Clone, Copy, Pod, Zeroable)]
struct WaterGpu {
    node: u32,
    kind: u32,
    idle_mode: u32,
    z_index: i32,
    size_depth_time: [f32; 4],
    flow_wind: [f32; 4],
    wave: [f32; 4],
    flags: [u32; 4],
    deep_color: [f32; 4],
    shallow_color: [f32; 4],
    sky_color_bias: [f32; 4],
    foam_color: [f32; 4],
    visual0: [f32; 4],
    visual1: [f32; 4],
    visual2: [f32; 4],
    wave_profile: [f32; 4],
    coastline_foam_color: [f32; 4],
    coastline: [f32; 4],
    shape: [f32; 4],
    sim: [u32; 4],
    model_x: [f32; 4],
    model_y: [f32; 4],
    model_z: [f32; 4],
    model_w: [f32; 4],
}

#[derive(Clone, Copy, Debug, PartialEq)]
struct WaterGridResolution {
    sim: [u32; 2],
    render: [u32; 2],
}

#[repr(C)]
#[derive(Clone, Copy, Pod, Zeroable)]
struct WaterParamsGpu {
    water_count: u32,
    water_2d_count: u32,
    cell_count: u32,
    render_flags: u32,
    time_seconds: f32,
    delta_seconds: f32,
    _pad1: [f32; 2],
}

// `render_flags` bit 0: the 3D water pass attaches a private depth target
// (1 sample; see Gpu3D::water_depth_attachment), so the 3D water shader has to
// reject fragments behind scene geometry itself. Clear under MSAA, where the
// pass still attaches the real scene depth.
const WATER_RENDER_FLAG_SCENE_DEPTH_REJECT: u32 = 1 << 0;
const WATER_RENDER_FLAG_SCENE_GEOMETRY: u32 = 1 << 1;

/// One render chunk = one instance of the 3D water draw.
///
/// `chunk`/`chunks` are integer grid coords, not float uv: a shared edge then
/// resolves to bit-identical uv in both neighbours, which is half of the
/// crack-free story (the other half is `edge_snap`).
#[repr(C)]
#[derive(Clone, Copy, Debug, PartialEq, Eq, Pod, Zeroable)]
struct WaterRenderChunkGpu {
    water_idx: u32,
    /// Surface quads per axis. Power of 2.
    quads: u32,
    flags: u32,
    /// 4 x u8 snap ratio for the -u/+u/-v/+v edges. >1 = neighbour is coarser,
    /// so this chunk snaps its boundary vertices onto the neighbour's knots.
    edge_snap: u32,
    chunk: [u32; 2],
    chunks: [u32; 2],
}

// Body-border edges draw a side wall at that edge, at this chunk's own
// tessellation, so the wall top shares the surface border vertices exactly.
const WATER_CHUNK_FLAG_EDGE_NEG_U: u32 = 1 << 0;
const WATER_CHUNK_FLAG_EDGE_POS_U: u32 = 1 << 1;
const WATER_CHUNK_FLAG_EDGE_NEG_V: u32 = 1 << 2;
const WATER_CHUNK_FLAG_EDGE_POS_V: u32 = 1 << 3;
const WATER_CHUNK_EDGE_MASK: u32 = 0b1111;
const WATER_CHUNK_FLAG_CIRCLE: u32 = 1 << 4;
const WATER_CHUNK_EDGE_SNAP_NONE: u32 = 0x0101_0101;

/// One `draw` per distinct chunk vertex count. Instances are sorted so equal
/// counts are contiguous; without this every chunk would pay the max chunk's
/// vertex count and per-chunk LOD would save no vertex work at all.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct WaterChunkDrawGroup {
    vertex_count: u32,
    first_instance: u32,
    instance_count: u32,
}

pub struct GpuWater {
    flip_3d: GpuWaterFlip,
    compute_pipeline: wgpu::ComputePipeline,
    render_pipeline_2d: wgpu::RenderPipeline,
    render_pipeline_3d: wgpu::RenderPipeline,
    compute_bgl: wgpu::BindGroupLayout,
    render_bgl: wgpu::BindGroupLayout,
    depth_bgl: wgpu::BindGroupLayout,
    compute_bind_group_ab: wgpu::BindGroup,
    compute_bind_group_ba: wgpu::BindGroup,
    render_bind_group_a: wgpu::BindGroup,
    render_bind_group_b: wgpu::BindGroup,
    depth_bind_group: wgpu::BindGroup,
    scene_color_texture: wgpu::Texture,
    scene_color_view: wgpu::TextureView,
    scene_color_format: wgpu::TextureFormat,
    scene_color_size: [u32; 2],
    scene_color_idle_frames: u32,
    scene_color_blit: SceneColorBlit,
    scene_color_capture_bind_group: Option<wgpu::BindGroup>,
    scene_color_capture_view_key: u64,
    #[cfg(test)]
    scene_color_capture_bind_group_creations: u32,
    // Last scene depth view bound alongside scene color, retained so the idle
    // release can rebuild the depth bind group without a caller-provided view.
    scene_depth_view: wgpu::TextureView,
    sample_count: u32,
    water_buffer: wgpu::Buffer,
    cell_buffer_a: wgpu::Buffer,
    cell_buffer_b: wgpu::Buffer,
    coastline_buffer: wgpu::Buffer,
    render_chunk_buffer: wgpu::Buffer,
    params_buffer: wgpu::Buffer,
    readback_buffer: wgpu::Buffer,
    water_capacity: usize,
    cell_capacity: usize,
    // Sized from the actual coastline sample count, not the doubled cell
    // capacity, so cell growth does not triple-allocate coastline storage.
    coastline_capacity: usize,
    cell_shrink: ShrinkTracker,
    coastline_shrink: ShrinkTracker,
    active_cell_buffer_b: bool,
    render_chunk_capacity: usize,
    active_cell_count: usize,
    max_cells_per_water: usize,
    max_3d_chunk_vertices: u32,
    water_count: u32,
    water_2d_count: u32,
    render_3d_chunk_count: u32,
    readback_capacity: usize,
    readback_pending: Option<PendingWaterReadback>,
    readback_nodes: Vec<NodeID>,
    readback_offsets: Vec<usize>,
    readback_samples: Vec<WaterSampleState>,
    readback_queries: Vec<WaterReadbackQuery>,
    readback_body_samples: Vec<WaterBodySampleState>,
    readback_water_sample_count: usize,
    readback_interval_seconds: f32,
    readback_accum_seconds: f32,
    readback_water_accum: HashMap<NodeID, f32>,
    readback_water_interval: HashMap<NodeID, f32>,
    readback_scheduled_nodes: Vec<NodeID>,
    // Retired snapshot buffers from completed readbacks; `request_readback`
    // swaps them back in so pending snapshots reuse capacity instead of
    // cloning the node/query vecs per encode.
    readback_nodes_pool: Vec<NodeID>,
    readback_queries_pool: Vec<WaterReadbackQuery>,
    readback_copy_encoded: bool,
    staged_waters: Vec<WaterGpu>,
    staged_render_chunks: Vec<WaterRenderChunkGpu>,
    staged_chunk_draws: Vec<WaterChunkDrawGroup>,
    chunk_quads_scratch: Vec<u32>,
    coastline_cells_scratch: Vec<[f32; 4]>,
    // Per-water cached static coastline field (solid/edge/spill from the
    // coastline shapes), keyed by a content signature. Only the dynamic impacts
    // wake is re-blended each frame, so static coastlines skip the expensive
    // per-cell signed-distance raster.
    coastline_cache: HashMap<NodeID, CachedCoastline>,
    // Forces the next coastline upload regardless of per-water gating. Set
    // whenever the destination buffer identity changes (growth / GC shrink) or
    // the scratch layout is reset.
    coastline_force_upload: bool,
    // Bytes actually pushed to `coastline_buffer`; the idle gate is asserted
    // against this in tests.
    coastline_upload_bytes: u64,
}

/// Cached static per-cell coastline field for one water node. `base` holds
/// `[solid, edge (foam), spill_energy]` derived only from the coastline shapes
/// and grid; the frame-varying impacts wake is blended on top per prepare.
struct CachedCoastline {
    signature: u64,
    base: Vec<[f32; 3]>,
    /// `(content signature, (scratch offset, cell count))` of the last raster
    /// this node wrote. When both still match, the persistent scratch already
    /// holds the identical bytes, so the blend loop and the coastline upload
    /// are both skipped. Safe because the coastline buffer is bound
    /// `storage, read` in every water shader - the GPU never writes it back.
    written: Option<(u64, (usize, usize))>,
}

impl CachedCoastline {
    fn new() -> Self {
        Self {
            signature: 0,
            base: Vec::new(),
            written: None,
        }
    }
}

#[derive(Clone, Copy, Debug)]
struct WaterReadbackQuery {
    query: WaterBodyQueryState,
    frac: [f32; 2],
}

struct PendingWaterReadback {
    rx: mpsc::Receiver<Result<(), wgpu::BufferAsyncError>>,
    mapped_bytes: u64,
    nodes: Vec<NodeID>,
    queries: Vec<WaterReadbackQuery>,
    water_sample_count: usize,
}

#[derive(Clone, Copy, Debug)]
pub struct WaterPrepareContext {
    pub camera_3d_position: [f32; 3],
    pub camera_3d_frustum_planes: [[f32; 4]; 6],
    pub camera_3d_lod_scale: [f32; 2],
    pub sky_color: [f32; 3],
    pub time_seconds: f32,
    pub delta_seconds: f32,
    pub scene_geometry_present: bool,
}

// Render pipelines depend on color format, sample count, and the scene depth
// format (derived from the sample count), so set_sample_count rebuilds them
// through this shared helper without touching simulation state.
#[allow(clippy::too_many_arguments)]
fn create_water_render_pipelines(
    device: &wgpu::Device,
    color_format: wgpu::TextureFormat,
    sample_count: u32,
    render_bgl: &wgpu::BindGroupLayout,
    depth_bgl: &wgpu::BindGroupLayout,
    camera_bgl: &wgpu::BindGroupLayout,
    camera_3d_bgl: &wgpu::BindGroupLayout,
) -> (wgpu::RenderPipeline, wgpu::RenderPipeline) {
    let render_wgsl = water_render_wgsl();
    let render_shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
        label: Some("perro_water_render_shader"),
        source: wgpu::ShaderSource::Wgsl(render_wgsl.into()),
    });
    let render_shader_3d = device.create_shader_module(wgpu::ShaderModuleDescriptor {
        label: Some("perro_water_3d_render_shader"),
        source: wgpu::ShaderSource::Wgsl(WATER_3D_RENDER_WGSL.into()),
    });
    let render_layout_2d = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
        label: Some("perro_water_2d_render_layout"),
        bind_group_layouts: &[Some(render_bgl), Some(camera_bgl)],
        immediate_size: 0,
    });
    let render_layout_3d = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
        label: Some("perro_water_3d_render_layout"),
        bind_group_layouts: &[Some(render_bgl), Some(camera_3d_bgl), Some(depth_bgl)],
        immediate_size: 0,
    });
    let render_pipeline_2d = crate::pipeline_cache::create_render_pipeline(
        device,
        wgpu::RenderPipelineDescriptor {
            label: Some("perro_water_2d_pipeline"),
            layout: Some(&render_layout_2d),
            vertex: wgpu::VertexState {
                module: &render_shader,
                entry_point: Some("vs_water_2d"),
                buffers: &[],
                compilation_options: wgpu::PipelineCompilationOptions::default(),
            },
            fragment: Some(wgpu::FragmentState {
                module: &render_shader,
                entry_point: Some("fs_water_2d"),
                targets: &[Some(wgpu::ColorTargetState {
                    format: color_format,
                    blend: Some(wgpu::BlendState::ALPHA_BLENDING),
                    write_mask: wgpu::ColorWrites::ALL,
                })],
                compilation_options: wgpu::PipelineCompilationOptions::default(),
            }),
            primitive: wgpu::PrimitiveState {
                topology: wgpu::PrimitiveTopology::TriangleList,
                strip_index_format: None,
                front_face: wgpu::FrontFace::Ccw,
                cull_mode: None,
                polygon_mode: wgpu::PolygonMode::Fill,
                unclipped_depth: false,
                conservative: false,
            },
            depth_stencil: None,
            multisample: wgpu::MultisampleState {
                count: sample_count.max(1),
                mask: !0,
                alpha_to_coverage_enabled: false,
            },
            multiview_mask: None,
            cache: None,
        },
    );
    let render_pipeline_3d = crate::pipeline_cache::create_render_pipeline(
        device,
        wgpu::RenderPipelineDescriptor {
            label: Some("perro_water_3d_pipeline"),
            layout: Some(&render_layout_3d),
            vertex: wgpu::VertexState {
                module: &render_shader_3d,
                entry_point: Some("vs_water_3d"),
                buffers: &[],
                compilation_options: wgpu::PipelineCompilationOptions::default(),
            },
            fragment: Some(wgpu::FragmentState {
                module: &render_shader_3d,
                entry_point: Some("fs_water_3d"),
                targets: &[Some(wgpu::ColorTargetState {
                    format: color_format,
                    blend: Some(wgpu::BlendState::ALPHA_BLENDING),
                    write_mask: wgpu::ColorWrites::ALL,
                })],
                compilation_options: wgpu::PipelineCompilationOptions::default(),
            }),
            primitive: wgpu::PrimitiveState {
                topology: wgpu::PrimitiveTopology::TriangleList,
                strip_index_format: None,
                front_face: wgpu::FrontFace::Ccw,
                cull_mode: Some(wgpu::Face::Back),
                polygon_mode: wgpu::PolygonMode::Fill,
                unclipped_depth: false,
                conservative: false,
            },
            depth_stencil: Some(wgpu::DepthStencilState {
                // Matches the 3D scene depth target this pipeline attaches.
                format: crate::scene_depth_format(sample_count.max(1)),
                depth_write_enabled: Some(true),
                depth_compare: Some(wgpu::CompareFunction::LessEqual),
                stencil: wgpu::StencilState::default(),
                bias: wgpu::DepthBiasState::default(),
            }),
            multisample: wgpu::MultisampleState {
                count: sample_count.max(1),
                mask: !0,
                alpha_to_coverage_enabled: false,
            },
            multiview_mask: None,
            cache: None,
        },
    );
    (render_pipeline_2d, render_pipeline_3d)
}

impl GpuWater {
    #[allow(clippy::too_many_arguments)] // GPU init inputs map 1:1 to renderer resources.
    pub fn new(
        device: &wgpu::Device,
        color_format: wgpu::TextureFormat,
        sample_count: u32,
        camera_bgl: &wgpu::BindGroupLayout,
        camera_3d_bgl: &wgpu::BindGroupLayout,
        scene_depth_view: &wgpu::TextureView,
        _width: u32,
        _height: u32,
    ) -> Self {
        let compute_bgl = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("perro_water_gpu_bgl"),
            entries: &[
                wgpu::BindGroupLayoutEntry {
                    binding: 0,
                    visibility: wgpu::ShaderStages::COMPUTE,
                    ty: wgpu::BindingType::Buffer {
                        ty: wgpu::BufferBindingType::Storage { read_only: true },
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
                        ty: wgpu::BufferBindingType::Uniform,
                        has_dynamic_offset: false,
                        min_binding_size: None,
                    },
                    count: None,
                },
                wgpu::BindGroupLayoutEntry {
                    binding: 3,
                    visibility: wgpu::ShaderStages::COMPUTE,
                    ty: wgpu::BindingType::Buffer {
                        ty: wgpu::BufferBindingType::Storage { read_only: true },
                        has_dynamic_offset: false,
                        min_binding_size: None,
                    },
                    count: None,
                },
                wgpu::BindGroupLayoutEntry {
                    binding: 5,
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
        let render_bgl = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("perro_water_render_bgl"),
            entries: &[
                wgpu::BindGroupLayoutEntry {
                    binding: 0,
                    visibility: wgpu::ShaderStages::VERTEX | wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Buffer {
                        ty: wgpu::BufferBindingType::Storage { read_only: true },
                        has_dynamic_offset: false,
                        min_binding_size: None,
                    },
                    count: None,
                },
                wgpu::BindGroupLayoutEntry {
                    binding: 1,
                    visibility: wgpu::ShaderStages::VERTEX | wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Buffer {
                        ty: wgpu::BufferBindingType::Storage { read_only: true },
                        has_dynamic_offset: false,
                        min_binding_size: None,
                    },
                    count: None,
                },
                wgpu::BindGroupLayoutEntry {
                    binding: 2,
                    visibility: wgpu::ShaderStages::VERTEX | wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Buffer {
                        ty: wgpu::BufferBindingType::Uniform,
                        has_dynamic_offset: false,
                        min_binding_size: None,
                    },
                    count: None,
                },
                wgpu::BindGroupLayoutEntry {
                    binding: 3,
                    visibility: wgpu::ShaderStages::VERTEX | wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Buffer {
                        ty: wgpu::BufferBindingType::Storage { read_only: true },
                        has_dynamic_offset: false,
                        min_binding_size: None,
                    },
                    count: None,
                },
                wgpu::BindGroupLayoutEntry {
                    binding: 4,
                    visibility: wgpu::ShaderStages::VERTEX | wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Buffer {
                        ty: wgpu::BufferBindingType::Storage { read_only: true },
                        has_dynamic_offset: false,
                        min_binding_size: None,
                    },
                    count: None,
                },
            ],
        });
        let depth_bgl = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("perro_water_depth_bgl"),
            entries: &[
                wgpu::BindGroupLayoutEntry {
                    binding: 0,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Texture {
                        sample_type: wgpu::TextureSampleType::Depth,
                        view_dimension: wgpu::TextureViewDimension::D2,
                        multisampled: false,
                    },
                    count: None,
                },
                wgpu::BindGroupLayoutEntry {
                    binding: 1,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Texture {
                        sample_type: wgpu::TextureSampleType::Float { filterable: false },
                        view_dimension: wgpu::TextureViewDimension::D2,
                        multisampled: false,
                    },
                    count: None,
                },
            ],
        });
        // Built after `depth_bgl`: the splash pass shares it, to sample scene
        // depth on the private-depth path.
        let flip_3d = GpuWaterFlip::new(
            device,
            color_format,
            sample_count,
            camera_3d_bgl,
            &depth_bgl,
        );
        let shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("perro_water_gpu_shader"),
            source: wgpu::ShaderSource::Wgsl(WATER_WGSL.into()),
        });
        let layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("perro_water_gpu_pipeline_layout"),
            bind_group_layouts: &[Some(&compute_bgl)],
            immediate_size: 0,
        });
        let compute_pipeline = crate::pipeline_cache::create_compute_pipeline(
            device,
            wgpu::ComputePipelineDescriptor {
                label: Some("perro_water_gpu_pipeline"),
                layout: Some(&layout),
                module: &shader,
                entry_point: Some("cs_main"),
                compilation_options: wgpu::PipelineCompilationOptions::default(),
                cache: None,
            },
        );
        let (render_pipeline_2d, render_pipeline_3d) = create_water_render_pipelines(
            device,
            color_format,
            sample_count,
            &render_bgl,
            &depth_bgl,
            camera_bgl,
            camera_3d_bgl,
        );
        let water_buffer = empty_buffer(device, "perro_water_gpu_waters", 1, true);
        let cell_buffer_a = empty_buffer(device, "perro_water_gpu_cells_a", 64, false);
        let cell_buffer_b = empty_buffer(device, "perro_water_gpu_cells_b", 64, false);
        let coastline_buffer = empty_buffer(device, "perro_water_gpu_coastline", 64, false);
        let render_chunk_buffer = empty_buffer(device, "perro_water_gpu_render_chunks", 1, true);
        let readback_buffer = readback_buffer(device, 1);
        let params_buffer = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("perro_water_gpu_params"),
            size: std::mem::size_of::<WaterParamsGpu>() as u64,
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });
        let compute_bind_group_ab = make_compute_bind_group(
            device,
            &compute_bgl,
            ComputeBindGroupBuffers {
                waters: &water_buffer,
                cells: &cell_buffer_a,
                next_cells: &cell_buffer_b,
                coastline: &coastline_buffer,
                params: &params_buffer,
            },
            "perro_water_gpu_bg_ab",
        );
        let compute_bind_group_ba = make_compute_bind_group(
            device,
            &compute_bgl,
            ComputeBindGroupBuffers {
                waters: &water_buffer,
                cells: &cell_buffer_b,
                next_cells: &cell_buffer_a,
                coastline: &coastline_buffer,
                params: &params_buffer,
            },
            "perro_water_gpu_bg_ba",
        );
        let render_bind_group_a = make_render_bind_group(
            device,
            &render_bgl,
            RenderBindGroupBuffers {
                waters: &water_buffer,
                cells: &cell_buffer_a,
                coastline: &coastline_buffer,
                render_chunks: &render_chunk_buffer,
                params: &params_buffer,
            },
            "perro_water_render_bg_a",
        );
        let render_bind_group_b = make_render_bind_group(
            device,
            &render_bgl,
            RenderBindGroupBuffers {
                waters: &water_buffer,
                cells: &cell_buffer_b,
                coastline: &coastline_buffer,
                render_chunks: &render_chunk_buffer,
                params: &params_buffer,
            },
            "perro_water_render_bg_b",
        );
        // Allocated lazily: only 3D water refraction reads scene color, so it
        // starts at 1x1 and promotes via set_scene_color_size on first use.
        let (scene_color_texture, scene_color_view) =
            create_scene_color_texture(device, color_format, 1, 1);
        let scene_color_blit = create_scene_color_blit(device, color_format);
        let depth_bind_group = make_depth_bind_group(
            device,
            &depth_bgl,
            scene_depth_view,
            &scene_color_view,
            "perro_water_depth_bg",
        );
        Self {
            flip_3d,
            compute_pipeline,
            render_pipeline_2d,
            render_pipeline_3d,
            compute_bgl,
            render_bgl,
            depth_bgl,
            compute_bind_group_ab,
            compute_bind_group_ba,
            render_bind_group_a,
            render_bind_group_b,
            depth_bind_group,
            scene_color_texture,
            scene_color_view,
            scene_color_format: color_format,
            scene_color_size: [1, 1],
            scene_color_idle_frames: 0,
            scene_color_blit,
            scene_color_capture_bind_group: None,
            scene_color_capture_view_key: 0,
            #[cfg(test)]
            scene_color_capture_bind_group_creations: 0,
            scene_depth_view: scene_depth_view.clone(),
            sample_count: sample_count.max(1),
            water_buffer,
            cell_buffer_a,
            cell_buffer_b,
            coastline_buffer,
            render_chunk_buffer,
            params_buffer,
            readback_buffer,
            water_capacity: 1,
            cell_capacity: 64,
            coastline_capacity: 64,
            cell_shrink: ShrinkTracker::default(),
            coastline_shrink: ShrinkTracker::default(),
            active_cell_buffer_b: false,
            render_chunk_capacity: 1,
            active_cell_count: 0,
            max_cells_per_water: 64,
            max_3d_chunk_vertices: 30,
            water_count: 0,
            water_2d_count: 0,
            render_3d_chunk_count: 0,
            readback_capacity: 1,
            readback_pending: None,
            readback_nodes: Vec::new(),
            readback_offsets: Vec::new(),
            readback_samples: Vec::new(),
            readback_queries: Vec::new(),
            readback_body_samples: Vec::new(),
            readback_water_sample_count: 0,
            readback_interval_seconds: 1.0 / 30.0,
            readback_accum_seconds: 0.0,
            readback_water_accum: HashMap::new(),
            readback_water_interval: HashMap::new(),
            readback_scheduled_nodes: Vec::new(),
            readback_nodes_pool: Vec::new(),
            readback_queries_pool: Vec::new(),
            readback_copy_encoded: false,
            staged_waters: Vec::new(),
            staged_render_chunks: Vec::new(),
            staged_chunk_draws: Vec::new(),
            chunk_quads_scratch: Vec::new(),
            coastline_cells_scratch: Vec::new(),
            coastline_cache: HashMap::new(),
            coastline_force_upload: true,
            coastline_upload_bytes: 0,
        }
    }

    /// Total bytes written to the coastline storage buffer since creation.
    /// Stays flat while every water's coastline field is idle. Profiling hook,
    /// also the assertion target for the idle-water upload test.
    #[cfg_attr(not(test), allow(dead_code))]
    pub fn coastline_upload_bytes(&self) -> u64 {
        self.coastline_upload_bytes
    }

    pub fn set_scene_depth_view(
        &mut self,
        device: &wgpu::Device,
        scene_depth_view: &wgpu::TextureView,
    ) {
        self.scene_depth_view = scene_depth_view.clone();
        self.depth_bind_group = make_depth_bind_group(
            device,
            &self.depth_bgl,
            scene_depth_view,
            &self.scene_color_view,
            "perro_water_depth_bg",
        );
    }

    pub fn set_scene_color_size(
        &mut self,
        device: &wgpu::Device,
        scene_depth_view: &wgpu::TextureView,
        width: u32,
        height: u32,
    ) {
        // Non-MSAA captures fill the copy through a downsample blit, so the
        // target lives at half the render resolution (4x less VRAM + copy
        // bandwidth). MSAA resolve targets must match the source dimensions,
        // so the MSAA capture keeps the copy at full resolution.
        let size = if self.sample_count == 1 {
            [width.max(1).div_ceil(2), height.max(1).div_ceil(2)]
        } else {
            [width.max(1), height.max(1)]
        };
        self.scene_color_idle_frames = 0;
        if self.scene_color_size == size {
            return;
        }
        (self.scene_color_texture, self.scene_color_view) =
            create_scene_color_texture(device, self.scene_color_format, size[0], size[1]);
        self.scene_color_size = size;
        self.set_scene_depth_view(device, scene_depth_view);
    }

    /// Per-frame tick while no 3D water exists. Releases the scene color copy
    /// target back to 1x1 after enough idle frames.
    pub fn note_scene_color_idle(&mut self, device: &wgpu::Device) {
        if self.scene_color_size == [1, 1] {
            self.scene_color_idle_frames = 0;
            return;
        }
        self.scene_color_idle_frames = self.scene_color_idle_frames.saturating_add(1);
        if self.scene_color_idle_frames < WATER_SCENE_COLOR_IDLE_RELEASE_FRAMES {
            return;
        }
        self.scene_color_idle_frames = 0;
        // The bind group owns its sampled source view. Release it with the
        // idle copy target so a resized main/stream target is not kept alive.
        self.scene_color_capture_bind_group = None;
        (self.scene_color_texture, self.scene_color_view) =
            create_scene_color_texture(device, self.scene_color_format, 1, 1);
        self.scene_color_size = [1, 1];
        let depth_view = self.scene_depth_view.clone();
        self.set_scene_depth_view(device, &depth_view);
    }

    pub fn capture_scene_color(
        &mut self,
        device: &wgpu::Device,
        encoder: &mut wgpu::CommandEncoder,
        source_view: &wgpu::TextureView,
        source_view_key: u64,
    ) {
        // Only the 3D water surface pipeline samples scene color (refraction /
        // SSR); skip the capture when it draws nothing this frame.
        if self.render_3d_chunk_count == 0 || self.max_3d_chunk_vertices == 0 {
            return;
        }
        if self.sample_count == 1 {
            // Downsample blit into the half-res copy target: one linear-tap
            // fullscreen triangle instead of a full-res 1:1 texture copy.
            if !scene_color_capture_cache_hit(
                self.scene_color_capture_bind_group
                    .as_ref()
                    .map(|_| self.scene_color_capture_view_key),
                source_view_key,
            ) {
                self.scene_color_capture_bind_group =
                    Some(device.create_bind_group(&wgpu::BindGroupDescriptor {
                        label: Some("perro_water_scene_color_blit_bg"),
                        layout: &self.scene_color_blit.bgl,
                        entries: &[
                            wgpu::BindGroupEntry {
                                binding: 0,
                                resource: wgpu::BindingResource::TextureView(source_view),
                            },
                            wgpu::BindGroupEntry {
                                binding: 1,
                                resource: wgpu::BindingResource::Sampler(
                                    &self.scene_color_blit.sampler,
                                ),
                            },
                        ],
                    }));
                self.scene_color_capture_view_key = source_view_key;
                #[cfg(test)]
                {
                    self.scene_color_capture_bind_group_creations = self
                        .scene_color_capture_bind_group_creations
                        .saturating_add(1);
                }
            }
            let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("perro_water_scene_color_downsample"),
                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                    view: &self.scene_color_view,
                    resolve_target: None,
                    ops: wgpu::Operations {
                        load: wgpu::LoadOp::Clear(wgpu::Color::TRANSPARENT),
                        store: wgpu::StoreOp::Store,
                    },
                    depth_slice: None,
                })],
                depth_stencil_attachment: None,
                timestamp_writes: None,
                occlusion_query_set: None,
                multiview_mask: None,
            });
            pass.set_pipeline(&self.scene_color_blit.pipeline);
            pass.set_bind_group(
                0,
                self.scene_color_capture_bind_group
                    .as_ref()
                    .expect("scene color capture bind group"),
                &[],
            );
            pass.draw(0..3, 0..1);
            return;
        }
        // MSAA: a resolve target must match the attachment dimensions, so the
        // copy target stays full-res here and capture remains a resolve pass.
        let _resolve_pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
            label: Some("perro_water_scene_color_resolve"),
            color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                view: source_view,
                resolve_target: Some(&self.scene_color_view),
                ops: wgpu::Operations {
                    load: wgpu::LoadOp::Load,
                    store: wgpu::StoreOp::Store,
                },
                depth_slice: None,
            })],
            depth_stencil_attachment: None,
            timestamp_writes: None,
            occlusion_query_set: None,
            multiview_mask: None,
        });
    }

    // Rebuild the render pipelines for a new MSAA sample count (and the scene
    // depth format tied to it) while keeping all simulation state.
    pub fn set_sample_count(
        &mut self,
        device: &wgpu::Device,
        color_format: wgpu::TextureFormat,
        sample_count: u32,
        camera_bgl: &wgpu::BindGroupLayout,
        camera_3d_bgl: &wgpu::BindGroupLayout,
    ) {
        let (render_pipeline_2d, render_pipeline_3d) = create_water_render_pipelines(
            device,
            color_format,
            sample_count,
            &self.render_bgl,
            &self.depth_bgl,
            camera_bgl,
            camera_3d_bgl,
        );
        self.render_pipeline_2d = render_pipeline_2d;
        self.render_pipeline_3d = render_pipeline_3d;
        // The splash pipeline attaches the same depth target, so it tracks the
        // sample count too (format + scene-occlusion path).
        self.flip_3d.set_sample_count(
            device,
            color_format,
            sample_count,
            camera_3d_bgl,
            &self.depth_bgl,
        );
        self.sample_count = sample_count.max(1);
        // MSAA capture does not use the blit bind group. Drop its source view;
        // a later 1x capture rebuilds against the caller's current generation.
        self.scene_color_capture_bind_group = None;
    }

    pub fn prepare(
        &mut self,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        waters_2d: &[(NodeID, Water2DState)],
        waters_3d: &[(NodeID, Water3DState)],
        ctx: WaterPrepareContext,
    ) {
        self.poll_readback(device);
        if waters_3d.is_empty() {
            self.note_scene_color_idle(device);
        } else {
            self.scene_color_idle_frames = 0;
        }
        let all_paused = waters_2d.iter().all(|(_, water)| water.paused)
            && waters_3d.iter().all(|(_, water)| water.paused);
        let needed = waters_2d.len() + waters_3d.len();
        self.water_count = needed.min(u32::MAX as usize) as u32;
        self.water_2d_count = waters_2d.len().min(u32::MAX as usize) as u32;
        if self.water_count == 0 {
            self.active_cell_count = 0;
            self.max_cells_per_water = 0;
            self.max_3d_chunk_vertices = 0;
            self.render_3d_chunk_count = 0;
            self.staged_chunk_draws.clear();
            self.readback_accum_seconds = 0.0;
            self.coastline_cache.clear();
            self.coastline_cells_scratch.clear();
            self.coastline_force_upload = true;
            self.readback_water_interval.clear();
            self.readback_water_accum.clear();
            // Nothing is drawn or simulated; let the GC tick reclaim the cell
            // and coastline buffers.
            self.cell_shrink.note_used(0);
            self.coastline_shrink.note_used(0);
            return;
        }
        if !all_paused {
            self.readback_accum_seconds += ctx.delta_seconds.max(0.0);
        }
        self.staged_waters.clear();
        if self.staged_waters.capacity() < needed {
            self.staged_waters
                .reserve(needed - self.staged_waters.capacity());
        }
        self.staged_render_chunks.clear();
        self.staged_chunk_draws.clear();
        // The coastline scratch is persistent: it mirrors the GPU buffer, so a
        // water whose field did not change keeps last frame's bytes in place
        // and neither the blend loop nor the upload runs.
        let mut coastline_dirty = self.coastline_force_upload;
        self.coastline_force_upload = false;
        let mut cell_needed = 0usize;
        let mut readback_rate = 0.0f32;
        for (node, water) in waters_2d {
            readback_rate = readback_rate.max(water.sample_readback_rate);
            let lod = water_lod_2d(water);
            let cells = water_cell_count(lod.grid.sim);
            let offset = cell_needed;
            if cells > 0 {
                let end = offset.saturating_add(cells);
                if self.coastline_cells_scratch.len() < end {
                    self.coastline_cells_scratch.resize(end, [0.0; 4]);
                    coastline_dirty = true;
                }
                coastline_dirty |= raster_coastline_2d(
                    &mut self.coastline_cells_scratch[offset..end],
                    lod.grid.sim,
                    water,
                    *node,
                    &mut self.coastline_cache,
                    (offset, cells),
                );
            }
            self.staged_waters.push(water_gpu_2d(
                *node,
                water,
                lod.grid,
                offset as u32,
                cells as u32,
                lod.ripple_blend,
            ));
            cell_needed = cell_needed.saturating_add(cells);
        }
        for (node, water) in waters_3d {
            readback_rate = readback_rate.max(water.sample_readback_rate);
            let lod = water_lod_3d(water, ctx.camera_3d_position, ctx.camera_3d_lod_scale);
            let cells = water_cell_count(lod.grid.sim);
            let offset = cell_needed;
            if cells > 0 {
                let end = offset.saturating_add(cells);
                if self.coastline_cells_scratch.len() < end {
                    self.coastline_cells_scratch.resize(end, [0.0; 4]);
                    coastline_dirty = true;
                }
                coastline_dirty |= raster_coastline_3d(
                    &mut self.coastline_cells_scratch[offset..end],
                    lod.grid.sim,
                    water,
                    *node,
                    &mut self.coastline_cache,
                    (offset, cells),
                );
            }
            let staged = water_gpu_3d(
                *node,
                water,
                lod.grid,
                offset as u32,
                cells as u32,
                lod.ripple_blend,
                ctx.sky_color,
            );
            self.staged_waters.push(staged);
            let water_idx = (self.staged_waters.len().saturating_sub(1)) as u32;
            if lod.grid.render[0] > 0 && lod.grid.render[1] > 0 {
                build_render_chunks_3d(
                    &mut self.staged_render_chunks,
                    &mut self.chunk_quads_scratch,
                    water_idx,
                    water,
                    staged,
                    ctx.camera_3d_position,
                    ctx.camera_3d_lod_scale,
                    &ctx.camera_3d_frustum_planes,
                );
            }
            cell_needed = cell_needed.saturating_add(cells);
        }
        // Total coastline cells staged this frame. Any length change means the
        // slot layout moved, so re-upload the whole run.
        if self.coastline_cells_scratch.len() != cell_needed {
            self.coastline_cells_scratch.resize(cell_needed, [0.0; 4]);
            coastline_dirty = true;
        }
        // Drop per-node caches for waters no longer present this frame. Same
        // node set keeps every map's size equal to the active water count. Only
        // scan for stale entries after a removal/replacement makes one larger.
        if self.coastline_cache.len() > needed
            || self.readback_water_interval.len() > needed
            || self.readback_water_accum.len() > needed
        {
            let live = |node: &NodeID| {
                waters_2d.iter().any(|(n, _)| n == node) || waters_3d.iter().any(|(n, _)| n == node)
            };
            self.coastline_cache.retain(|node, _| live(node));
            self.readback_water_interval.retain(|node, _| live(node));
            self.readback_water_accum.retain(|node, _| live(node));
        }
        // Group key first, distance second. Equal vertex counts must be
        // contiguous so `render_3d` can issue one draw per LOD; within a group
        // near-to-far order survives because vertex count falls with distance.
        self.staged_render_chunks.sort_by(|a, b| {
            let va = water_render_chunk_vertex_count(&self.staged_waters[a.water_idx as usize], a);
            let vb = water_render_chunk_vertex_count(&self.staged_waters[b.water_idx as usize], b);
            vb.cmp(&va).then_with(|| {
                let da = water_render_chunk_distance_sq(
                    &self.staged_waters[a.water_idx as usize],
                    a,
                    ctx.camera_3d_position,
                );
                let db = water_render_chunk_distance_sq(
                    &self.staged_waters[b.water_idx as usize],
                    b,
                    ctx.camera_3d_position,
                );
                da.total_cmp(&db)
            })
        });
        self.staged_chunk_draws.clear();
        for (index, chunk) in self.staged_render_chunks.iter().enumerate() {
            let vertex_count = water_render_chunk_vertex_count(
                &self.staged_waters[chunk.water_idx as usize],
                chunk,
            );
            if vertex_count == 0 {
                continue;
            }
            match self.staged_chunk_draws.last_mut() {
                Some(group)
                    if group.vertex_count == vertex_count
                        && group.first_instance + group.instance_count == index as u32 =>
                {
                    group.instance_count += 1;
                }
                _ => self.staged_chunk_draws.push(WaterChunkDrawGroup {
                    vertex_count,
                    first_instance: index as u32,
                    instance_count: 1,
                }),
            }
        }
        cell_needed = cell_needed.max(WATER_WORKGROUP_SIZE as usize);
        self.active_cell_count = cell_needed;
        self.max_cells_per_water = self
            .staged_waters
            .iter()
            .map(|water| water.sim[1] as usize)
            .max()
            .unwrap_or(WATER_WORKGROUP_SIZE as usize);
        self.max_3d_chunk_vertices = self
            .staged_chunk_draws
            .iter()
            .map(|group| group.vertex_count)
            .max()
            .unwrap_or(0);
        self.render_3d_chunk_count = self.staged_render_chunks.len().min(u32::MAX as usize) as u32;
        self.readback_interval_seconds = readback_interval_seconds(readback_rate);
        let rebuilt = self.ensure_capacity(
            device,
            needed,
            cell_needed,
            self.coastline_cells_scratch.len(),
            self.staged_render_chunks.len(),
        );
        if rebuilt {
            // Any buffer in the set may have been recreated (empty contents),
            // so the coastline run must be pushed again.
            self.rebuild_cell_bind_groups(device);
            coastline_dirty = true;
        }
        queue.write_buffer(
            &self.water_buffer,
            0,
            bytemuck::cast_slice(&self.staged_waters),
        );
        if !self.staged_render_chunks.is_empty() {
            queue.write_buffer(
                &self.render_chunk_buffer,
                0,
                bytemuck::cast_slice(&self.staged_render_chunks),
            );
        }
        if coastline_dirty && !self.coastline_cells_scratch.is_empty() {
            let bytes: &[u8] = bytemuck::cast_slice(&self.coastline_cells_scratch);
            self.coastline_upload_bytes = self
                .coastline_upload_bytes
                .saturating_add(bytes.len() as u64);
            queue.write_buffer(&self.coastline_buffer, 0, bytes);
        }
        let params = WaterParamsGpu {
            water_count: self.water_count,
            water_2d_count: self.water_2d_count,
            cell_count: cell_needed.min(u32::MAX as usize) as u32,
            render_flags: (if self.sample_count <= 1 {
                WATER_RENDER_FLAG_SCENE_DEPTH_REJECT
            } else {
                0
            }) | if ctx.scene_geometry_present {
                WATER_RENDER_FLAG_SCENE_GEOMETRY
            } else {
                0
            },
            time_seconds: ctx.time_seconds.max(0.0),
            delta_seconds: ctx.delta_seconds.max(0.0),
            _pad1: [0.0; 2],
        };
        queue.write_buffer(&self.params_buffer, 0, bytemuck::bytes_of(&params));
        self.readback_nodes.clear();
        self.readback_offsets.clear();
        self.readback_queries.clear();
        self.readback_scheduled_nodes.clear();
        for ((node, state), water) in waters_2d.iter().zip(self.staged_waters.iter()) {
            let interval = water_adaptive_readback_interval(
                state.sample_readback_rate,
                water.wave[3],
                !state.queries.is_empty(),
                !state.impacts.is_empty(),
            );
            self.readback_water_interval.insert(*node, interval);
            let accum = self.readback_water_accum.entry(*node).or_insert(0.0);
            if !all_paused {
                *accum += ctx.delta_seconds.max(0.0);
            }
            let scheduled = interval > 0.0 && *accum + 1.0e-6 >= interval;
            if water.sim[1] > 0 && scheduled {
                self.readback_nodes.push(*node);
                self.readback_offsets.push(water_center_cell_offset(water));
                self.readback_scheduled_nodes.push(*node);
            }
        }
        for ((node, state), water) in waters_3d
            .iter()
            .zip(self.staged_waters.iter().skip(waters_2d.len()))
        {
            let interval = water_adaptive_readback_interval(
                state.sample_readback_rate,
                water.wave[3],
                !state.queries.is_empty(),
                !state.impacts.is_empty(),
            );
            self.readback_water_interval.insert(*node, interval);
            let accum = self.readback_water_accum.entry(*node).or_insert(0.0);
            if !all_paused {
                *accum += ctx.delta_seconds.max(0.0);
            }
            let scheduled = interval > 0.0 && *accum + 1.0e-6 >= interval;
            if water.sim[1] > 0 && scheduled {
                self.readback_nodes.push(*node);
                self.readback_offsets.push(water_center_cell_offset(water));
                self.readback_scheduled_nodes.push(*node);
            }
        }
        self.readback_water_sample_count = self.readback_nodes.len();
        // Sort once so the membership checks below are O(log n) binary searches
        // rather than O(n) scans per water.
        self.readback_scheduled_nodes
            .sort_unstable_by_key(|node| node.as_u64());
        for ((node, state), water) in waters_2d.iter().zip(self.staged_waters.iter()) {
            if self
                .readback_scheduled_nodes
                .binary_search_by_key(&node.as_u64(), |n| n.as_u64())
                .is_err()
            {
                continue;
            }
            for query in state.queries.iter() {
                let sample = water_query_sample_offsets(water, query.local);
                self.readback_queries.push(WaterReadbackQuery {
                    query: *query,
                    frac: sample.frac,
                });
                self.readback_offsets.extend(sample.offsets);
                debug_assert_eq!(query.water, *node);
            }
        }
        for ((node, state), water) in waters_3d
            .iter()
            .zip(self.staged_waters.iter().skip(waters_2d.len()))
        {
            if self
                .readback_scheduled_nodes
                .binary_search_by_key(&node.as_u64(), |n| n.as_u64())
                .is_err()
            {
                continue;
            }
            for query in state.queries.iter() {
                let sample = water_query_sample_offsets(water, query.local);
                self.readback_queries.push(WaterReadbackQuery {
                    query: *query,
                    frac: sample.frac,
                });
                self.readback_offsets.extend(sample.offsets);
                debug_assert_eq!(query.water, *node);
            }
        }
        self.ensure_readback_capacity(device, self.readback_offsets.len());
        self.flip_3d
            .prepare(device, queue, waters_3d, ctx.delta_seconds);
    }

    pub fn clear_active(&mut self) {
        self.flip_3d.clear_active();
        self.water_count = 0;
        self.water_2d_count = 0;
        self.active_cell_count = 0;
        self.max_cells_per_water = 0;
        self.max_3d_chunk_vertices = 0;
        self.readback_accum_seconds = 0.0;
        self.render_3d_chunk_count = 0;
        self.readback_nodes.clear();
        self.readback_offsets.clear();
        self.readback_queries.clear();
        self.readback_body_samples.clear();
        self.readback_water_sample_count = 0;
        self.readback_scheduled_nodes.clear();
        self.readback_copy_encoded = false;
        self.staged_render_chunks.clear();
        self.staged_chunk_draws.clear();
        self.coastline_cache.clear();
        self.coastline_cells_scratch.clear();
        self.coastline_force_upload = true;
        self.readback_water_interval.clear();
        self.readback_water_accum.clear();
    }

    pub fn encode(&self, encoder: &mut wgpu::CommandEncoder) {
        self.flip_3d.encode(encoder);
        if self.water_count == 0 {
            return;
        }
        if self.max_cells_per_water == 0 {
            return;
        }
        let mut pass = encoder.begin_compute_pass(&wgpu::ComputePassDescriptor {
            label: Some("perro_water_gpu_sim"),
            timestamp_writes: None,
        });
        pass.set_pipeline(&self.compute_pipeline);
        pass.set_bind_group(0, self.compute_bind_group(), &[]);
        let workgroups_x = self
            .max_cells_per_water
            .max(WATER_WORKGROUP_SIZE as usize)
            .div_ceil(WATER_WORKGROUP_SIZE as usize) as u32;
        let x_groups = workgroups_x.min(65_535);
        pass.dispatch_workgroups(x_groups, self.water_count, 1);
    }

    #[cfg(test)]
    pub(crate) fn flip_particle_count(&self) -> u32 {
        self.flip_3d.particle_count()
    }

    pub fn render_2d(
        &self,
        encoder: &mut wgpu::CommandEncoder,
        target: &wgpu::TextureView,
        resolve_target: Option<&wgpu::TextureView>,
        camera_bind_group: &wgpu::BindGroup,
        clear: Option<wgpu::Color>,
    ) {
        if self.water_2d_count == 0 {
            return;
        }
        let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
            label: Some("perro_water_2d_pass"),
            color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                view: target,
                resolve_target,
                ops: wgpu::Operations {
                    load: clear.map_or(wgpu::LoadOp::Load, wgpu::LoadOp::Clear),
                    store: wgpu::StoreOp::Store,
                },
                depth_slice: None,
            })],
            depth_stencil_attachment: None,
            timestamp_writes: None,
            occlusion_query_set: None,
            multiview_mask: None,
        });
        pass.set_pipeline(&self.render_pipeline_2d);
        pass.set_bind_group(0, self.render_bind_group(), &[]);
        pass.set_bind_group(1, camera_bind_group, &[]);
        pass.draw(0..6, 0..self.water_2d_count);
    }

    pub fn render_3d(
        &self,
        encoder: &mut wgpu::CommandEncoder,
        target: &wgpu::TextureView,
        depth: &wgpu::TextureView,
        camera_bind_group: &wgpu::BindGroup,
        clear_depth: bool,
    ) {
        if self.render_3d_chunk_count > 0 && self.max_3d_chunk_vertices > 0 {
            let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("perro_water_3d_pass"),
                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                    view: target,
                    resolve_target: None,
                    ops: wgpu::Operations {
                        load: wgpu::LoadOp::Load,
                        store: wgpu::StoreOp::Store,
                    },
                    depth_slice: None,
                })],
                depth_stencil_attachment: Some(wgpu::RenderPassDepthStencilAttachment {
                    view: depth,
                    depth_ops: Some(wgpu::Operations {
                        load: if clear_depth {
                            wgpu::LoadOp::Clear(1.0)
                        } else {
                            wgpu::LoadOp::Load
                        },
                        store: wgpu::StoreOp::Store,
                    }),
                    stencil_ops: None,
                }),
                timestamp_writes: None,
                occlusion_query_set: None,
                multiview_mask: None,
            });
            pass.set_pipeline(&self.render_pipeline_3d);
            pass.set_bind_group(0, self.render_bind_group(), &[]);
            pass.set_bind_group(1, camera_bind_group, &[]);
            pass.set_bind_group(2, &self.depth_bind_group, &[]);
            // One draw per LOD group. `instance_index` in WGSL is absolute, so
            // the chunk lookup still indexes the shared buffer directly.
            for group in &self.staged_chunk_draws {
                pass.draw(
                    0..group.vertex_count,
                    group.first_instance..group.first_instance + group.instance_count,
                );
            }
        }
        self.flip_3d.render(
            encoder,
            target,
            depth,
            camera_bind_group,
            &self.depth_bind_group,
        );
    }

    pub fn encode_readback(&mut self, encoder: &mut wgpu::CommandEncoder) {
        self.readback_copy_encoded = false;
        if self.water_count == 0 || self.readback_pending.is_some() {
            return;
        }
        if self.readback_interval_seconds <= 0.0
            || self.readback_accum_seconds + 1.0e-6 < self.readback_interval_seconds
        {
            return;
        }
        if self.readback_offsets.is_empty() {
            return;
        }
        let needed_samples = self.readback_offsets.len();
        if needed_samples > self.readback_capacity {
            return;
        }
        let elem = std::mem::size_of::<[f32; 4]>() as u64;
        for (idx, offset) in self.readback_offsets.iter().copied().enumerate() {
            encoder.copy_buffer_to_buffer(
                self.render_cell_buffer(),
                offset as u64 * elem,
                &self.readback_buffer,
                idx as u64 * elem,
                elem,
            );
        }
        self.readback_accum_seconds =
            (self.readback_accum_seconds - self.readback_interval_seconds).max(0.0);
        for node in &self.readback_scheduled_nodes {
            let Some(interval) = self.readback_water_interval.get(node).copied() else {
                continue;
            };
            let Some(accum) = self.readback_water_accum.get_mut(node) else {
                continue;
            };
            *accum = (*accum - interval).max(0.0);
        }
        self.readback_copy_encoded = true;
    }

    pub fn request_readback(&mut self) {
        if self.water_count == 0 || self.readback_pending.is_some() || !self.readback_copy_encoded {
            return;
        }
        if self.readback_offsets.is_empty() {
            return;
        }
        let needed_samples = self.readback_offsets.len();
        if needed_samples > self.readback_capacity {
            return;
        }
        let byte_count = (needed_samples * std::mem::size_of::<[f32; 4]>()) as u64;
        let slice = self.readback_buffer.slice(0..byte_count);
        let (tx, rx) = mpsc::channel();
        slice.map_async(wgpu::MapMode::Read, move |result| {
            let _ = tx.send(result);
        });
        // Move the staged snapshot into the pending readback and hand the live
        // vecs pooled (empty) buffers; prepare() rebuilds them from scratch
        // every frame before the next encode, so nothing reads them after this
        // point in the frame.
        let mut nodes = std::mem::take(&mut self.readback_nodes_pool);
        nodes.clear();
        std::mem::swap(&mut nodes, &mut self.readback_nodes);
        let mut queries = std::mem::take(&mut self.readback_queries_pool);
        queries.clear();
        std::mem::swap(&mut queries, &mut self.readback_queries);
        self.readback_pending = Some(PendingWaterReadback {
            rx,
            mapped_bytes: byte_count,
            nodes,
            queries,
            water_sample_count: self.readback_water_sample_count,
        });
        self.readback_copy_encoded = false;
    }

    pub fn finish_frame(&mut self) {
        if self.water_count != 0 {
            self.active_cell_buffer_b = !self.active_cell_buffer_b;
        }
    }

    pub fn drain_samples(&mut self, out: &mut Vec<WaterSampleState>) {
        out.append(&mut self.readback_samples);
    }

    pub fn drain_body_samples(&mut self, out: &mut Vec<WaterBodySampleState>) {
        out.append(&mut self.readback_body_samples);
    }

    fn ensure_capacity(
        &mut self,
        device: &wgpu::Device,
        needed_waters: usize,
        needed_cells: usize,
        needed_coastline: usize,
        needed_render_chunks: usize,
    ) -> bool {
        self.cell_shrink.note_used(needed_cells);
        self.coastline_shrink.note_used(needed_coastline);
        let mut rebuilt = false;
        if needed_waters > self.water_capacity {
            let mut cap = self.water_capacity.max(1);
            while cap < needed_waters {
                cap *= 2;
            }
            self.water_buffer = empty_buffer(device, "perro_water_gpu_waters", cap, true);
            self.water_capacity = cap;
            rebuilt = true;
        }
        if needed_cells > self.cell_capacity {
            let mut cap = self.cell_capacity.max(64);
            while cap < needed_cells {
                cap *= 2;
            }
            self.cell_buffer_a = empty_buffer(device, "perro_water_gpu_cells_a", cap, false);
            self.cell_buffer_b = empty_buffer(device, "perro_water_gpu_cells_b", cap, false);
            self.cell_capacity = cap;
            self.active_cell_buffer_b = false;
            rebuilt = true;
        }
        // Coastline is per-water static data rewritten by every prepare; size
        // it from the actual coastline sample count instead of riding the
        // doubled cell capacity.
        if needed_coastline > self.coastline_capacity {
            let mut cap = self.coastline_capacity.max(64);
            while cap < needed_coastline {
                cap *= 2;
            }
            self.coastline_buffer = empty_buffer(device, "perro_water_gpu_coastline", cap, false);
            self.coastline_capacity = cap;
            rebuilt = true;
        }
        if needed_waters > self.readback_capacity {
            self.ensure_readback_capacity(device, needed_waters);
        }
        if needed_render_chunks > self.render_chunk_capacity {
            let mut cap = self.render_chunk_capacity.max(1);
            while cap < needed_render_chunks {
                cap *= 2;
            }
            self.render_chunk_buffer =
                empty_buffer(device, "perro_water_gpu_render_chunks", cap, true);
            self.render_chunk_capacity = cap;
            rebuilt = true;
        }
        rebuilt
    }

    /// Periodic GC tick: shrink the cell ping-pong buffers and coastline
    /// buffer once usage stays far below capacity. Content is preserved with a
    /// prefix copy (the sim state lives in the prefix `[0, active_cell_count)`),
    /// so the ping-pong parity and in-flight readbacks stay valid.
    pub fn shrink_tick(&mut self, device: &wgpu::Device, queue: &wgpu::Queue) {
        let elem = std::mem::size_of::<[f32; 4]>() as u64;
        let mut rebuilt = false;
        if let Some(new_cap) = self.cell_shrink.tick(self.cell_capacity, 64) {
            let new_size = new_cap as u64 * elem;
            self.cell_buffer_a = shrink_buffer_preserving(
                device,
                queue,
                &self.cell_buffer_a,
                "perro_water_gpu_cells_a",
                new_size,
                wgpu::BufferUsages::STORAGE
                    | wgpu::BufferUsages::COPY_DST
                    | wgpu::BufferUsages::COPY_SRC,
            );
            self.cell_buffer_b = shrink_buffer_preserving(
                device,
                queue,
                &self.cell_buffer_b,
                "perro_water_gpu_cells_b",
                new_size,
                wgpu::BufferUsages::STORAGE
                    | wgpu::BufferUsages::COPY_DST
                    | wgpu::BufferUsages::COPY_SRC,
            );
            self.cell_capacity = new_cap;
            rebuilt = true;
        }
        if let Some(new_cap) = self.coastline_shrink.tick(self.coastline_capacity, 64) {
            self.coastline_buffer = shrink_buffer_preserving(
                device,
                queue,
                &self.coastline_buffer,
                "perro_water_gpu_coastline",
                new_cap as u64 * elem,
                wgpu::BufferUsages::STORAGE
                    | wgpu::BufferUsages::COPY_DST
                    | wgpu::BufferUsages::COPY_SRC,
            );
            self.coastline_capacity = new_cap;
            rebuilt = true;
            // Prefix-preserving copy, but the buffer identity changed; force the
            // next prepare to re-push rather than trusting the copy.
            self.coastline_force_upload = true;
        }
        if rebuilt {
            self.rebuild_cell_bind_groups(device);
        }
    }

    fn compute_bind_group(&self) -> &wgpu::BindGroup {
        if self.active_cell_buffer_b {
            &self.compute_bind_group_ba
        } else {
            &self.compute_bind_group_ab
        }
    }

    fn render_bind_group(&self) -> &wgpu::BindGroup {
        if self.active_cell_buffer_b {
            &self.render_bind_group_a
        } else {
            &self.render_bind_group_b
        }
    }

    fn render_cell_buffer(&self) -> &wgpu::Buffer {
        if self.active_cell_buffer_b {
            &self.cell_buffer_a
        } else {
            &self.cell_buffer_b
        }
    }

    fn rebuild_cell_bind_groups(&mut self, device: &wgpu::Device) {
        self.compute_bind_group_ab = make_compute_bind_group(
            device,
            &self.compute_bgl,
            ComputeBindGroupBuffers {
                waters: &self.water_buffer,
                cells: &self.cell_buffer_a,
                next_cells: &self.cell_buffer_b,
                coastline: &self.coastline_buffer,
                params: &self.params_buffer,
            },
            "perro_water_gpu_bg_ab",
        );
        self.compute_bind_group_ba = make_compute_bind_group(
            device,
            &self.compute_bgl,
            ComputeBindGroupBuffers {
                waters: &self.water_buffer,
                cells: &self.cell_buffer_b,
                next_cells: &self.cell_buffer_a,
                coastline: &self.coastline_buffer,
                params: &self.params_buffer,
            },
            "perro_water_gpu_bg_ba",
        );
        self.render_bind_group_a = make_render_bind_group(
            device,
            &self.render_bgl,
            RenderBindGroupBuffers {
                waters: &self.water_buffer,
                cells: &self.cell_buffer_a,
                coastline: &self.coastline_buffer,
                render_chunks: &self.render_chunk_buffer,
                params: &self.params_buffer,
            },
            "perro_water_render_bg_a",
        );
        self.render_bind_group_b = make_render_bind_group(
            device,
            &self.render_bgl,
            RenderBindGroupBuffers {
                waters: &self.water_buffer,
                cells: &self.cell_buffer_b,
                coastline: &self.coastline_buffer,
                render_chunks: &self.render_chunk_buffer,
                params: &self.params_buffer,
            },
            "perro_water_render_bg_b",
        );
    }

    fn ensure_readback_capacity(&mut self, device: &wgpu::Device, needed_samples: usize) {
        if needed_samples <= self.readback_capacity || self.readback_pending.is_some() {
            return;
        }
        let mut cap = self.readback_capacity.max(64);
        while cap < needed_samples {
            cap *= 2;
        }
        self.readback_buffer = readback_buffer(device, cap);
        self.readback_capacity = cap;
    }

    fn poll_readback(&mut self, device: &wgpu::Device) {
        let Some(pending) = self.readback_pending.as_ref() else {
            return;
        };
        let _ = device.poll(wgpu::PollType::Poll);
        match pending.rx.try_recv() {
            Ok(Ok(())) => {
                let pending = self
                    .readback_pending
                    .take()
                    .expect("water readback pending after ready result");
                let slice = self.readback_buffer.slice(0..pending.mapped_bytes);
                let Ok(data) = slice.get_mapped_range() else {
                    // Range failure after a successful map means the buffer
                    // is destroyed (device loss); unmap would panic.
                    self.retire_readback_snapshot(pending);
                    return;
                };
                let cells: &[[f32; 4]] = bytemuck::cast_slice(&data);
                decode_water_readback(
                    cells,
                    &pending.nodes,
                    pending.water_sample_count,
                    &pending.queries,
                    &mut self.readback_samples,
                    &mut self.readback_body_samples,
                );
                drop(data);
                self.readback_buffer.unmap();
                self.retire_readback_snapshot(pending);
            }
            // Failed map: the buffer never reached the mapped state (device
            // loss destroys it); unmapping trips wgpu's destroyed-buffer
            // validation and panics the app.
            Ok(Err(_)) | Err(mpsc::TryRecvError::Disconnected) => {
                if let Some(pending) = self.readback_pending.take() {
                    self.retire_readback_snapshot(pending);
                }
            }
            Err(mpsc::TryRecvError::Empty) => {}
        }
    }

    /// Return a completed snapshot's buffers to the pool so the next
    /// `request_readback` reuses their capacity instead of allocating.
    fn retire_readback_snapshot(&mut self, pending: PendingWaterReadback) {
        self.readback_nodes_pool = pending.nodes;
        self.readback_queries_pool = pending.queries;
    }
}

#[path = "water_gpu/resources.rs"]
mod resources;
use resources::*;
#[path = "water_gpu/chunks.rs"]
mod chunks;
use chunks::*;
#[path = "water_gpu/coastline.rs"]
mod coastline;
use coastline::*;
#[path = "water_gpu/sampling.rs"]
mod sampling;
use sampling::*;
#[path = "water_gpu/params.rs"]
mod params;
use params::*;

fn water_render_wgsl() -> String {
    WATER_WGSL
        .replace(
            "next_cells[cell_idx] = vec4<f32>(0.0);",
            "let render_only_shape_skip = cell_idx;",
        )
        .replace(
            "next_cells[cell_idx] = vec4<f32>(0.0, 0.0, 1.0, 1.0);",
            "let render_only_coast_skip = cell_idx;",
        )
        .replace(
            "next_cells[cell_idx] = vec4<f32>(0.0);",
            "let render_only_empty_skip = cell_idx;",
        )
        .replace(
            "next_cells[cell_idx] = vec4<f32>(blended_height, velocity, foam, shore);",
            "let render_only_wave_skip = blended_height + velocity + foam + shore;",
        )
}

#[cfg(test)]
#[path = "water_gpu/tests.rs"]
mod test_suite;
