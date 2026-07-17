use super::water_flip_gpu::GpuWaterFlip;
use bytemuck::{Pod, Zeroable};
use glam::{Mat4, Vec3, Vec4};
use perro_ids::NodeID;
use perro_render_bridge::{
    Water2DState, Water3DState, WaterBodyQueryState, WaterBodySampleState, WaterCoastlineShape2D,
    WaterCoastlineShape3D, WaterIdleModeState, WaterSampleState, WaterShapeState,
};
use std::collections::HashMap;
use std::hash::{BuildHasher, Hash, Hasher};
use std::sync::mpsc;

const WATER_WORKGROUP_SIZE: u32 = 64;
const WATER_MAX_RENDER_RESOLUTION: u32 = 1024;
const WATER_FLAG_DEBUG: u32 = 1 << 0;
const WATER_FLAG_PAUSED: u32 = 1 << 1;
const WATER_COASTLINE_INSET_METERS: f32 = 1.0;
const WATER_CHUNK_QUADS: u32 = 128;
const WATER_3D_MAX_RENDER_RESOLUTION: u32 = 256;
// Keep silhouette tessellation dense; fragment normals alone cannot hide long
// low-poly edges against the horizon. Far mesh stays >=57% per axis (~3x cut).
const WATER_3D_RENDER_LOD_STRENGTH: f32 = 0.75;

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
    _pad: u32,
    time_seconds: f32,
    delta_seconds: f32,
    _pad1: [f32; 2],
}

#[repr(C)]
#[derive(Clone, Copy, Pod, Zeroable)]
struct WaterRenderChunkGpu {
    water_idx: u32,
    render_width: u32,
    render_height: u32,
    flags: u32,
    uv_origin: [f32; 2],
    uv_scale: [f32; 2],
}

const WATER_CHUNK_FLAG_DRAW_SIDES: u32 = 1 << 0;
const WATER_CHUNK_FLAG_CIRCLE: u32 = 1 << 1;

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
    readback_copy_encoded: bool,
    staged_waters: Vec<WaterGpu>,
    staged_render_chunks: Vec<WaterRenderChunkGpu>,
    coastline_cells_scratch: Vec<[f32; 4]>,
    // Per-water cached static coastline field (solid/edge/spill from the
    // coastline shapes), keyed by a content signature. Only the dynamic impacts
    // wake is re-blended each frame, so static coastlines skip the expensive
    // per-cell signed-distance raster.
    coastline_cache: HashMap<NodeID, CachedCoastline>,
}

/// Cached static per-cell coastline field for one water node. `base` holds
/// `[solid, edge (foam), spill_energy]` derived only from the coastline shapes
/// and grid; the frame-varying impacts wake is blended on top per prepare.
struct CachedCoastline {
    signature: u64,
    base: Vec<[f32; 3]>,
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
    pub camera_2d_position: [f32; 2],
    pub camera_3d_position: [f32; 3],
    pub camera_3d_frustum_planes: [[f32; 4]; 6],
    pub sky_color: [f32; 3],
    pub time_seconds: f32,
    pub delta_seconds: f32,
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
    let render_pipeline_2d = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
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
    });
    let render_pipeline_3d = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
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
    });
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
        width: u32,
        height: u32,
    ) -> Self {
        let flip_3d = GpuWaterFlip::new(device, color_format, sample_count, camera_3d_bgl);
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
        let shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("perro_water_gpu_shader"),
            source: wgpu::ShaderSource::Wgsl(WATER_WGSL.into()),
        });
        let layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("perro_water_gpu_pipeline_layout"),
            bind_group_layouts: &[Some(&compute_bgl)],
            immediate_size: 0,
        });
        let compute_pipeline = device.create_compute_pipeline(&wgpu::ComputePipelineDescriptor {
            label: Some("perro_water_gpu_pipeline"),
            layout: Some(&layout),
            module: &shader,
            entry_point: Some("cs_main"),
            compilation_options: wgpu::PipelineCompilationOptions::default(),
            cache: None,
        });
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
        let (scene_color_texture, scene_color_view) =
            create_scene_color_texture(device, color_format, width, height);
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
            scene_color_size: [width.max(1), height.max(1)],
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
            readback_copy_encoded: false,
            staged_waters: Vec::new(),
            staged_render_chunks: Vec::new(),
            coastline_cells_scratch: Vec::new(),
            coastline_cache: HashMap::new(),
        }
    }

    pub fn set_scene_depth_view(
        &mut self,
        device: &wgpu::Device,
        scene_depth_view: &wgpu::TextureView,
    ) {
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
        let size = [width.max(1), height.max(1)];
        if self.scene_color_size == size {
            return;
        }
        (self.scene_color_texture, self.scene_color_view) =
            create_scene_color_texture(device, self.scene_color_format, size[0], size[1]);
        self.scene_color_size = size;
        self.set_scene_depth_view(device, scene_depth_view);
    }

    pub fn capture_scene_color(
        &self,
        encoder: &mut wgpu::CommandEncoder,
        source_texture: &wgpu::Texture,
        source_view: &wgpu::TextureView,
    ) {
        if self.sample_count == 1 {
            encoder.copy_texture_to_texture(
                source_texture.as_image_copy(),
                self.scene_color_texture.as_image_copy(),
                wgpu::Extent3d {
                    width: self.scene_color_size[0],
                    height: self.scene_color_size[1],
                    depth_or_array_layers: 1,
                },
            );
            return;
        }
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
        self.sample_count = sample_count.max(1);
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
            self.readback_accum_seconds = 0.0;
            self.coastline_cache.clear();
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
        self.coastline_cells_scratch.clear();
        self.staged_render_chunks.clear();
        let mut cell_needed = 0usize;
        let mut readback_rate = 0.0f32;
        for (node, water) in waters_2d {
            readback_rate = readback_rate.max(water.sample_readback_rate);
            let lod = water_lod_2d(water, ctx.camera_2d_position);
            let cells = water_cell_count(lod.grid.sim);
            let offset = cell_needed;
            if cells > 0 {
                self.coastline_cells_scratch
                    .resize(offset.saturating_add(cells), [0.0; 4]);
                raster_coastline_2d(
                    &mut self.coastline_cells_scratch[offset..offset + cells],
                    lod.grid.sim,
                    water,
                    *node,
                    &mut self.coastline_cache,
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
            let lod = water_lod_3d(water, ctx.camera_3d_position);
            let cells = water_cell_count(lod.grid.sim);
            let offset = cell_needed;
            if cells > 0 {
                self.coastline_cells_scratch
                    .resize(offset.saturating_add(cells), [0.0; 4]);
                raster_coastline_3d(
                    &mut self.coastline_cells_scratch[offset..offset + cells],
                    lod.grid.sim,
                    water,
                    *node,
                    &mut self.coastline_cache,
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
                    water_idx,
                    water,
                    staged,
                    &ctx.camera_3d_frustum_planes,
                );
            }
            cell_needed = cell_needed.saturating_add(cells);
        }
        // Drop cached coastlines for waters no longer present this frame.
        // Same node set keeps cache size equal to active water count. Only scan
        // for stale entries after a removal/replacement makes it larger.
        if self.coastline_cache.len() > needed {
            self.coastline_cache.retain(|node, _| {
                waters_2d.iter().any(|(n, _)| n == node) || waters_3d.iter().any(|(n, _)| n == node)
            });
        }
        self.staged_render_chunks.sort_by(|a, b| {
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
        });
        cell_needed = cell_needed.max(WATER_WORKGROUP_SIZE as usize);
        self.active_cell_count = cell_needed;
        self.max_cells_per_water = self
            .staged_waters
            .iter()
            .map(|water| water.sim[1] as usize)
            .max()
            .unwrap_or(WATER_WORKGROUP_SIZE as usize);
        self.max_3d_chunk_vertices = self
            .staged_render_chunks
            .iter()
            .map(|chunk| {
                water_render_chunk_vertex_count(
                    &self.staged_waters[chunk.water_idx as usize],
                    chunk,
                )
            })
            .max()
            .unwrap_or(0);
        self.render_3d_chunk_count = self.staged_render_chunks.len().min(u32::MAX as usize) as u32;
        self.readback_interval_seconds = readback_interval_seconds(readback_rate);
        let rebuilt =
            self.ensure_capacity(device, needed, cell_needed, self.staged_render_chunks.len());
        if rebuilt {
            self.rebuild_cell_bind_groups(device);
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
        if !self.coastline_cells_scratch.is_empty() {
            queue.write_buffer(
                &self.coastline_buffer,
                0,
                bytemuck::cast_slice(&self.coastline_cells_scratch),
            );
        }
        let params = WaterParamsGpu {
            water_count: self.water_count,
            water_2d_count: self.water_2d_count,
            cell_count: cell_needed.min(u32::MAX as usize) as u32,
            _pad: 0,
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
        self.coastline_cache.clear();
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
            pass.draw(0..self.max_3d_chunk_vertices, 0..self.render_3d_chunk_count);
        }
        self.flip_3d
            .render(encoder, target, depth, camera_bind_group);
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
        self.readback_pending = Some(PendingWaterReadback {
            rx,
            mapped_bytes: byte_count,
            nodes: self.readback_nodes.clone(),
            queries: self.readback_queries.clone(),
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
        needed_render_chunks: usize,
    ) -> bool {
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
            self.coastline_buffer = empty_buffer(device, "perro_water_gpu_coastline", cap, false);
            self.cell_capacity = cap;
            self.active_cell_buffer_b = false;
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
                    self.readback_buffer.unmap();
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
            }
            Ok(Err(_)) | Err(mpsc::TryRecvError::Disconnected) => {
                self.readback_buffer.unmap();
                self.readback_pending = None;
            }
            Err(mpsc::TryRecvError::Empty) => {}
        }
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
