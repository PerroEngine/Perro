use bytemuck::{Pod, Zeroable};
use glam::{Mat4, Vec3, Vec4};
use perro_ids::NodeID;
use perro_render_bridge::{
    Water2DState, Water3DState, WaterBodyQueryState, WaterBodySampleState, WaterCoastlineShape2D,
    WaterCoastlineShape3D, WaterIdleModeState, WaterSampleState, WaterShapeState,
};
use std::collections::HashMap;
use std::sync::mpsc;

const WATER_WORKGROUP_SIZE: u32 = 64;
const WATER_MAX_RENDER_RESOLUTION: u32 = 1024;
const WATER_FLAG_DEBUG: u32 = 1 << 0;
const WATER_FLAG_PAUSED: u32 = 1 << 1;
const WATER_COASTLINE_INSET_METERS: f32 = 1.0;
const WATER_CHUNK_QUADS: u32 = 128;
const WATER_3D_MAX_RENDER_RESOLUTION: u32 = 256;

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
    readback_mapped_bytes: u64,
    readback_pending_rx: Option<mpsc::Receiver<Result<(), wgpu::BufferAsyncError>>>,
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
}

#[derive(Clone, Copy, Debug)]
struct WaterReadbackQuery {
    query: WaterBodyQueryState,
    frac: [f32; 2],
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

impl GpuWater {
    pub fn new(
        device: &wgpu::Device,
        color_format: wgpu::TextureFormat,
        sample_count: u32,
        camera_bgl: &wgpu::BindGroupLayout,
        camera_3d_bgl: &wgpu::BindGroupLayout,
        scene_depth_view: &wgpu::TextureView,
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
            entries: &[wgpu::BindGroupLayoutEntry {
                binding: 0,
                visibility: wgpu::ShaderStages::FRAGMENT,
                ty: wgpu::BindingType::Texture {
                    sample_type: wgpu::TextureSampleType::Depth,
                    view_dimension: wgpu::TextureViewDimension::D2,
                    multisampled: false,
                },
                count: None,
            }],
        });
        let shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("perro_water_gpu_shader"),
            source: wgpu::ShaderSource::Wgsl(WATER_WGSL.into()),
        });
        let render_wgsl = water_render_wgsl();
        let render_shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("perro_water_render_shader"),
            source: wgpu::ShaderSource::Wgsl(render_wgsl.into()),
        });
        let render_shader_3d = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("perro_water_3d_render_shader"),
            source: wgpu::ShaderSource::Wgsl(WATER_3D_RENDER_WGSL.into()),
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
        let render_layout_2d = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("perro_water_2d_render_layout"),
            bind_group_layouts: &[Some(&render_bgl), Some(camera_bgl)],
            immediate_size: 0,
        });
        let render_layout_3d = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("perro_water_3d_render_layout"),
            bind_group_layouts: &[Some(&render_bgl), Some(camera_3d_bgl), Some(&depth_bgl)],
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
                format: wgpu::TextureFormat::Depth24Plus,
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
            &water_buffer,
            &cell_buffer_a,
            &cell_buffer_b,
            &coastline_buffer,
            &params_buffer,
            "perro_water_gpu_bg_ab",
        );
        let compute_bind_group_ba = make_compute_bind_group(
            device,
            &compute_bgl,
            &water_buffer,
            &cell_buffer_b,
            &cell_buffer_a,
            &coastline_buffer,
            &params_buffer,
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
        let depth_bind_group =
            make_depth_bind_group(device, &depth_bgl, scene_depth_view, "perro_water_depth_bg");
        Self {
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
            readback_mapped_bytes: 0,
            readback_pending_rx: None,
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
            "perro_water_depth_bg",
        );
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
                );
            }
            self.staged_waters.push(water_gpu_3d(
                *node,
                water,
                lod.grid,
                offset as u32,
                cells as u32,
                lod.ripple_blend,
                ctx.sky_color,
            ));
            let water_idx = (self.staged_waters.len().saturating_sub(1)) as u32;
            if lod.grid.render[0] > 0 && lod.grid.render[1] > 0 {
                let staged = *self.staged_waters.last().expect("staged water");
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
        for ((node, state), water) in waters_2d.iter().zip(self.staged_waters.iter()) {
            if !self.readback_scheduled_nodes.contains(node) {
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
            if !self.readback_scheduled_nodes.contains(node) {
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
    }

    pub fn clear_active(&mut self) {
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
    }

    pub fn encode(&self, encoder: &mut wgpu::CommandEncoder) {
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
        if self.render_3d_chunk_count == 0 || self.max_3d_chunk_vertices == 0 {
            return;
        }
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

    pub fn encode_readback(&mut self, encoder: &mut wgpu::CommandEncoder) {
        self.readback_copy_encoded = false;
        if self.water_count == 0 || self.readback_pending_rx.is_some() {
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
        if self.water_count == 0
            || self.readback_pending_rx.is_some()
            || !self.readback_copy_encoded
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
        let byte_count = (needed_samples * std::mem::size_of::<[f32; 4]>()) as u64;
        let slice = self.readback_buffer.slice(0..byte_count);
        let (tx, rx) = mpsc::channel();
        slice.map_async(wgpu::MapMode::Read, move |result| {
            let _ = tx.send(result);
        });
        self.readback_pending_rx = Some(rx);
        self.readback_mapped_bytes = byte_count;
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
            &self.water_buffer,
            &self.cell_buffer_a,
            &self.cell_buffer_b,
            &self.coastline_buffer,
            &self.params_buffer,
            "perro_water_gpu_bg_ab",
        );
        self.compute_bind_group_ba = make_compute_bind_group(
            device,
            &self.compute_bgl,
            &self.water_buffer,
            &self.cell_buffer_b,
            &self.cell_buffer_a,
            &self.coastline_buffer,
            &self.params_buffer,
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
        if needed_samples <= self.readback_capacity {
            return;
        }
        let mut cap = self.readback_capacity.max(64);
        while cap < needed_samples {
            cap *= 2;
        }
        self.readback_buffer = readback_buffer(device, cap);
        self.readback_capacity = cap;
        self.readback_pending_rx = None;
    }

    fn poll_readback(&mut self, device: &wgpu::Device) {
        let Some(rx) = self.readback_pending_rx.as_ref() else {
            return;
        };
        let _ = device.poll(wgpu::PollType::Poll);
        match rx.try_recv() {
            Ok(Ok(())) => {
                let slice = self.readback_buffer.slice(0..self.readback_mapped_bytes);
                let data = slice.get_mapped_range();
                let cells: &[[f32; 4]] = bytemuck::cast_slice(&data);
                self.readback_samples.clear();
                self.readback_body_samples.clear();
                for (idx, node) in self
                    .readback_nodes
                    .iter()
                    .take(self.readback_water_sample_count)
                    .enumerate()
                {
                    let cell = cells.get(idx).copied().unwrap_or([0.0; 4]);
                    self.readback_samples.push(WaterSampleState {
                        node: *node,
                        height: cell[0],
                        velocity: [cell[1], 0.0],
                        foam: cell[2],
                    });
                }
                let mut query_base = self.readback_water_sample_count;
                for sample in self.readback_queries.iter() {
                    let c00 = cells.get(query_base).copied().unwrap_or([0.0; 4]);
                    let c10 = cells.get(query_base + 1).copied().unwrap_or(c00);
                    let c01 = cells.get(query_base + 2).copied().unwrap_or(c00);
                    let c11 = cells.get(query_base + 3).copied().unwrap_or(c10);
                    query_base += 4;
                    let cell = water_lerp_cell(c00, c10, c01, c11, sample.frac);
                    let query = sample.query;
                    self.readback_body_samples.push(WaterBodySampleState {
                        water: query.water,
                        body: query.body,
                        point: query.point,
                        local: query.local,
                        height: cell[0],
                        velocity: [cell[1], 0.0],
                        foam: cell[2],
                    });
                }
                drop(data);
                self.readback_buffer.unmap();
                self.readback_pending_rx = None;
            }
            Ok(Err(_)) | Err(mpsc::TryRecvError::Disconnected) => {
                self.readback_buffer.unmap();
                self.readback_pending_rx = None;
            }
            Err(mpsc::TryRecvError::Empty) => {}
        }
    }
}

fn empty_buffer(device: &wgpu::Device, label: &str, count: usize, water: bool) -> wgpu::Buffer {
    let elem = if water {
        std::mem::size_of::<WaterGpu>()
    } else {
        std::mem::size_of::<[f32; 4]>()
    };
    device.create_buffer(&wgpu::BufferDescriptor {
        label: Some(label),
        size: (count.max(1) * elem) as u64,
        usage: wgpu::BufferUsages::STORAGE
            | wgpu::BufferUsages::COPY_DST
            | wgpu::BufferUsages::COPY_SRC,
        mapped_at_creation: false,
    })
}

fn readback_buffer(device: &wgpu::Device, cell_count: usize) -> wgpu::Buffer {
    device.create_buffer(&wgpu::BufferDescriptor {
        label: Some("perro_water_gpu_readback"),
        size: (cell_count.max(1) * std::mem::size_of::<[f32; 4]>()) as u64,
        usage: wgpu::BufferUsages::COPY_DST | wgpu::BufferUsages::MAP_READ,
        mapped_at_creation: false,
    })
}

fn readback_interval_seconds(rate_hz: f32) -> f32 {
    if !rate_hz.is_finite() || rate_hz <= 0.0 {
        return 0.0;
    }
    1.0 / rate_hz.clamp(1.0, 240.0)
}

fn water_adaptive_readback_interval(
    base_rate_hz: f32,
    ripple_blend: f32,
    has_queries: bool,
    has_impacts: bool,
) -> f32 {
    let active_scale = if has_queries || has_impacts || ripple_blend >= 0.85 {
        1.0
    } else if ripple_blend >= 0.45 {
        0.5
    } else {
        0.25
    };
    readback_interval_seconds(base_rate_hz * active_scale)
}

fn make_compute_bind_group(
    device: &wgpu::Device,
    layout: &wgpu::BindGroupLayout,
    waters: &wgpu::Buffer,
    cells: &wgpu::Buffer,
    next_cells: &wgpu::Buffer,
    coastline: &wgpu::Buffer,
    params: &wgpu::Buffer,
    label: &'static str,
) -> wgpu::BindGroup {
    device.create_bind_group(&wgpu::BindGroupDescriptor {
        label: Some(label),
        layout,
        entries: &[
            wgpu::BindGroupEntry {
                binding: 0,
                resource: waters.as_entire_binding(),
            },
            wgpu::BindGroupEntry {
                binding: 1,
                resource: cells.as_entire_binding(),
            },
            wgpu::BindGroupEntry {
                binding: 2,
                resource: params.as_entire_binding(),
            },
            wgpu::BindGroupEntry {
                binding: 3,
                resource: coastline.as_entire_binding(),
            },
            wgpu::BindGroupEntry {
                binding: 5,
                resource: next_cells.as_entire_binding(),
            },
        ],
    })
}

struct RenderBindGroupBuffers<'a> {
    waters: &'a wgpu::Buffer,
    cells: &'a wgpu::Buffer,
    coastline: &'a wgpu::Buffer,
    render_chunks: &'a wgpu::Buffer,
    params: &'a wgpu::Buffer,
}

fn make_render_bind_group(
    device: &wgpu::Device,
    layout: &wgpu::BindGroupLayout,
    buffers: RenderBindGroupBuffers<'_>,
    label: &'static str,
) -> wgpu::BindGroup {
    device.create_bind_group(&wgpu::BindGroupDescriptor {
        label: Some(label),
        layout,
        entries: &[
            wgpu::BindGroupEntry {
                binding: 0,
                resource: buffers.waters.as_entire_binding(),
            },
            wgpu::BindGroupEntry {
                binding: 1,
                resource: buffers.cells.as_entire_binding(),
            },
            wgpu::BindGroupEntry {
                binding: 2,
                resource: buffers.params.as_entire_binding(),
            },
            wgpu::BindGroupEntry {
                binding: 3,
                resource: buffers.coastline.as_entire_binding(),
            },
            wgpu::BindGroupEntry {
                binding: 4,
                resource: buffers.render_chunks.as_entire_binding(),
            },
        ],
    })
}

fn make_depth_bind_group(
    device: &wgpu::Device,
    layout: &wgpu::BindGroupLayout,
    scene_depth_view: &wgpu::TextureView,
    label: &str,
) -> wgpu::BindGroup {
    device.create_bind_group(&wgpu::BindGroupDescriptor {
        label: Some(label),
        layout,
        entries: &[wgpu::BindGroupEntry {
            binding: 0,
            resource: wgpu::BindingResource::TextureView(scene_depth_view),
        }],
    })
}

fn build_render_chunks_3d(
    out: &mut Vec<WaterRenderChunkGpu>,
    water_idx: u32,
    water: &Water3DState,
    gpu: WaterGpu,
    planes: &[[f32; 4]; 6],
) {
    match water.shape {
        WaterShapeState::Circle { .. } | WaterShapeState::Cylinder { .. } => {
            if water_chunk_visible(gpu, [0.5, 0.5], [1.0, 1.0], planes) {
                out.push(WaterRenderChunkGpu {
                    water_idx,
                    render_width: gpu.flags[0].max(2),
                    render_height: gpu.flags[1].max(2),
                    flags: WATER_CHUNK_FLAG_DRAW_SIDES | WATER_CHUNK_FLAG_CIRCLE,
                    uv_origin: [0.0, 0.0],
                    uv_scale: [1.0, 1.0],
                });
            }
        }
        WaterShapeState::Rect => {
            let width = gpu.flags[0].max(2);
            let height = gpu.flags[1].max(2);
            let quad_width = width.saturating_sub(1);
            let quad_height = height.saturating_sub(1);
            let chunks_x = quad_width.div_ceil(WATER_CHUNK_QUADS).max(1);
            let chunks_y = quad_height.div_ceil(WATER_CHUNK_QUADS).max(1);
            for cy in 0..chunks_y {
                for cx in 0..chunks_x {
                    let start_x = cx * WATER_CHUNK_QUADS;
                    let start_y = cy * WATER_CHUNK_QUADS;
                    let chunk_quads_x = (quad_width.saturating_sub(start_x)).min(WATER_CHUNK_QUADS);
                    let chunk_quads_y =
                        (quad_height.saturating_sub(start_y)).min(WATER_CHUNK_QUADS);
                    let chunk_width = chunk_quads_x + 1;
                    let chunk_height = chunk_quads_y + 1;
                    let uv_origin = [
                        start_x as f32 / quad_width.max(1) as f32,
                        start_y as f32 / quad_height.max(1) as f32,
                    ];
                    let uv_scale = [
                        chunk_quads_x as f32 / quad_width.max(1) as f32,
                        chunk_quads_y as f32 / quad_height.max(1) as f32,
                    ];
                    if !water_chunk_visible(gpu, uv_origin, uv_scale, planes) {
                        continue;
                    }
                    out.push(WaterRenderChunkGpu {
                        water_idx,
                        render_width: chunk_width.max(2),
                        render_height: chunk_height.max(2),
                        flags: if cx == 0 && cy == 0 {
                            WATER_CHUNK_FLAG_DRAW_SIDES
                        } else {
                            0
                        },
                        uv_origin,
                        uv_scale,
                    });
                }
            }
        }
    }
}

fn water_chunk_visible(
    water: WaterGpu,
    uv_origin: [f32; 2],
    uv_scale: [f32; 2],
    planes: &[[f32; 4]; 6],
) -> bool {
    let center_uv = [
        uv_origin[0] + uv_scale[0] * 0.5,
        uv_origin[1] + uv_scale[1] * 0.5,
    ];
    let center_local = Vec4::new(
        (center_uv[0] - 0.5) * water.size_depth_time[0],
        0.0,
        (center_uv[1] - 0.5) * water.size_depth_time[1],
        1.0,
    );
    let model =
        Mat4::from_cols_array_2d(&[water.model_x, water.model_y, water.model_z, water.model_w]);
    if !model.is_finite() {
        return true;
    }
    let center_world = model * center_local;
    let sx = Vec3::new(water.model_x[0], water.model_x[1], water.model_x[2]).length();
    let sy = Vec3::new(water.model_y[0], water.model_y[1], water.model_y[2]).length();
    let sz = Vec3::new(water.model_z[0], water.model_z[1], water.model_z[2]).length();
    let chunk_half_x = water.size_depth_time[0].abs() * uv_scale[0] * 0.5;
    let chunk_half_z = water.size_depth_time[1].abs() * uv_scale[1] * 0.5;
    let depth = water.size_depth_time[2].abs().max(0.5);
    let radius_local =
        (chunk_half_x * chunk_half_x + chunk_half_z * chunk_half_z + depth * depth).sqrt();
    let radius = radius_local * sx.max(sy).max(sz).max(1.0e-6);
    for plane in planes {
        let p = Vec4::from_array(*plane);
        let dist = p.x * center_world.x + p.y * center_world.y + p.z * center_world.z + p.w;
        if dist < -radius {
            return false;
        }
    }
    true
}

fn water_render_chunk_vertex_count(water: &WaterGpu, chunk: &WaterRenderChunkGpu) -> u32 {
    if chunk.flags & WATER_CHUNK_FLAG_CIRCLE != 0 {
        return water_3d_vertex_count(water);
    }
    let surface = chunk
        .render_width
        .saturating_sub(1)
        .saturating_mul(chunk.render_height.saturating_sub(1))
        .saturating_mul(6);
    if chunk.flags & WATER_CHUNK_FLAG_DRAW_SIDES != 0 {
        surface.saturating_add(water_3d_side_vertex_count(water))
    } else {
        surface
    }
}

fn water_render_chunk_distance_sq(
    water: &WaterGpu,
    chunk: &WaterRenderChunkGpu,
    camera: [f32; 3],
) -> f32 {
    let uv = [
        chunk.uv_origin[0] + chunk.uv_scale[0] * 0.5,
        chunk.uv_origin[1] + chunk.uv_scale[1] * 0.5,
    ];
    let local_x = (uv[0] - 0.5) * water.size_depth_time[0];
    let local_z = (uv[1] - 0.5) * water.size_depth_time[1];
    let world = [
        water.model_w[0] + water.model_x[0] * local_x + water.model_z[0] * local_z,
        water.model_w[1] + water.model_x[1] * local_x + water.model_z[1] * local_z,
        water.model_w[2] + water.model_x[2] * local_x + water.model_z[2] * local_z,
    ];
    let dx = world[0] - camera[0];
    let dy = world[1] - camera[1];
    let dz = world[2] - camera[2];
    dx * dx + dy * dy + dz * dz
}

fn raster_coastline_2d(out: &mut [[f32; 4]], resolution: [u32; 2], water: &Water2DState) {
    let width = resolution[0].clamp(1, 256) as usize;
    let height = resolution[1].clamp(1, 256) as usize;
    if water.coastline_shapes.is_empty() {
        raster_impacts_2d(out, width, height, water);
        return;
    }
    let foam_width = water.coastline_foam_width.max(0.001);
    let softness = water.coastline_cutoff_softness.max(0.001);
    for y in 0..height {
        for x in 0..width {
            let fx = x as f32 / (width.saturating_sub(1).max(1) as f32);
            let fy = y as f32 / (height.saturating_sub(1).max(1) as f32);
            let p = [(fx - 0.5) * water.size[0], (fy - 0.5) * water.size[1]];
            let mut signed_min = f32::INFINITY;
            let mut edge = 0.0f32;
            for shape in water.coastline_shapes.iter() {
                let signed = signed_distance_2d(p, *shape);
                signed_min = signed_min.min(signed);
                edge = edge.max(1.0 - (signed / foam_width).clamp(0.0, 1.0));
            }
            let (solid, foam_edge, spill_energy) = coastline_fill(signed_min, foam_width, softness);
            let mut wake = 0.0f32;
            for impact in water.impacts.iter() {
                let dx = p[0] - impact.position[0];
                let dy = p[1] - impact.position[1];
                let radius = impact.radius.max(0.001);
                let t = ((dx * dx + dy * dy) / (radius * radius).max(0.000001)).clamp(0.0, 1.0);
                let push = 1.0 - t;
                let ring = (1.0 - ((t - 0.72).abs() / 0.28).clamp(0.0, 1.0)).powi(2);
                let strength = impact.strength * (1.0 / 180.0);
                wake += push * (strength * 0.70 + impact.cavitation * 0.28)
                    + ring * (strength * 0.30 + impact.cavitation * 0.92);
            }
            let wake = wake.clamp(0.0, 1.0);
            out[y * width + x] = [solid, edge.max(foam_edge), wake, spill_energy.max(wake)];
        }
    }
}

fn raster_impacts_2d(out: &mut [[f32; 4]], width: usize, height: usize, water: &Water2DState) {
    out.fill([0.0; 4]);
    if water.impacts.is_empty() {
        return;
    }
    let max_x = width.saturating_sub(1).max(1) as f32;
    let max_y = height.saturating_sub(1).max(1) as f32;
    let inv_x = max_x / water.size[0].abs().max(0.001);
    let inv_y = max_y / water.size[1].abs().max(0.001);
    for impact in water.impacts.iter() {
        let radius = impact.radius.max(0.001);
        let radius_sq = (radius * radius).max(0.000001);
        let inv_radius_sq = 1.0 / radius_sq;
        let min_x = (((impact.position[0] - radius) / water.size[0]) + 0.5) * max_x;
        let max_xf = (((impact.position[0] + radius) / water.size[0]) + 0.5) * max_x;
        let min_y = (((impact.position[1] - radius) / water.size[1]) + 0.5) * max_y;
        let max_yf = (((impact.position[1] + radius) / water.size[1]) + 0.5) * max_y;
        let x0 = min_x.floor().clamp(0.0, max_x) as usize;
        let x1 = max_xf.ceil().clamp(0.0, max_x) as usize;
        let y0 = min_y.floor().clamp(0.0, max_y) as usize;
        let y1 = max_yf.ceil().clamp(0.0, max_y) as usize;
        for y in y0..=y1 {
            let py = (y as f32 / inv_y) - water.size[1] * 0.5;
            for x in x0..=x1 {
                let px = (x as f32 / inv_x) - water.size[0] * 0.5;
                let dx = px - impact.position[0];
                let dy = py - impact.position[1];
                let dist_sq = dx * dx + dy * dy;
                if dist_sq > radius_sq {
                    continue;
                }
                let t = (dist_sq * inv_radius_sq).clamp(0.0, 1.0);
                let amount = 1.0 - t;
                if amount <= 0.0 {
                    continue;
                }
                let outline_width = (0.20 / radius).clamp(0.08, 0.42);
                let ring_center = (1.0 - outline_width * 0.65).clamp(0.42, 0.96);
                let ring =
                    (1.0 - ((t - ring_center).abs() / outline_width).clamp(0.0, 1.0)).powi(2);
                let strength = impact.strength * (1.0 / 180.0);
                let wake = amount * (strength * 0.70 + impact.cavitation * 0.28)
                    + ring * (strength * 0.44 + impact.cavitation * 1.08);
                let cell = &mut out[y * width + x];
                cell[2] = (cell[2] + wake).clamp(0.0, 1.0);
                cell[3] = cell[3].max((ring * 1.20 + amount * 0.22).clamp(0.0, 1.0));
            }
        }
    }
}

fn signed_distance_2d(p: [f32; 2], shape: WaterCoastlineShape2D) -> f32 {
    match shape {
        WaterCoastlineShape2D::Circle { center, radius } => {
            let dx = p[0] - center[0];
            let dy = p[1] - center[1];
            (dx * dx + dy * dy).sqrt() - radius
        }
        WaterCoastlineShape2D::Quad {
            center,
            half_extents,
            rotation,
        } => {
            let s = rotation.sin();
            let c = rotation.cos();
            let dx = p[0] - center[0];
            let dy = p[1] - center[1];
            let lx = (dx * c + dy * s).abs() - half_extents[0];
            let ly = (-dx * s + dy * c).abs() - half_extents[1];
            let ox = lx.max(0.0);
            let oy = ly.max(0.0);
            (ox * ox + oy * oy).sqrt() + lx.max(ly).min(0.0)
        }
        WaterCoastlineShape2D::Triangle { points } => {
            let inside = point_in_triangle(p, points);
            let d = distance_segment(p, points[0], points[1])
                .min(distance_segment(p, points[1], points[2]))
                .min(distance_segment(p, points[2], points[0]));
            if inside { -d } else { d }
        }
    }
}

fn point_in_triangle(p: [f32; 2], t: [[f32; 2]; 3]) -> bool {
    let s1 = cross2(p, t[0], t[1]);
    let s2 = cross2(p, t[1], t[2]);
    let s3 = cross2(p, t[2], t[0]);
    (s1 >= 0.0 && s2 >= 0.0 && s3 >= 0.0) || (s1 <= 0.0 && s2 <= 0.0 && s3 <= 0.0)
}

fn cross2(p: [f32; 2], a: [f32; 2], b: [f32; 2]) -> f32 {
    (p[0] - a[0]) * (b[1] - a[1]) - (p[1] - a[1]) * (b[0] - a[0])
}

fn distance_segment(p: [f32; 2], a: [f32; 2], b: [f32; 2]) -> f32 {
    let vx = b[0] - a[0];
    let vy = b[1] - a[1];
    let wx = p[0] - a[0];
    let wy = p[1] - a[1];
    let denom = (vx * vx + vy * vy).max(0.0001);
    let t = ((wx * vx + wy * vy) / denom).clamp(0.0, 1.0);
    let dx = p[0] - (a[0] + vx * t);
    let dy = p[1] - (a[1] + vy * t);
    (dx * dx + dy * dy).sqrt()
}

fn raster_coastline_3d(out: &mut [[f32; 4]], resolution: [u32; 2], water: &Water3DState) {
    let width = resolution[0].clamp(1, 256) as usize;
    let height = resolution[1].clamp(1, 256) as usize;
    if water.coastline_shapes.is_empty() {
        raster_impacts_3d(out, width, height, water);
        return;
    }
    let foam_width = water.coastline_foam_width.max(0.001);
    let softness = water.coastline_cutoff_softness.max(0.001);
    for y in 0..height {
        for x in 0..width {
            let fx = x as f32 / (width.saturating_sub(1).max(1) as f32);
            let fy = y as f32 / (height.saturating_sub(1).max(1) as f32);
            let p = [(fx - 0.5) * water.size[0], (fy - 0.5) * water.size[1]];
            let mut signed_min = f32::INFINITY;
            let mut edge = 0.0f32;
            for shape in water.coastline_shapes.iter() {
                let signed = signed_distance_3d_xz(p, *shape);
                signed_min = signed_min.min(signed);
                edge = edge.max(1.0 - (signed / foam_width).clamp(0.0, 1.0));
            }
            let (solid, foam_edge, spill_energy) = coastline_fill(signed_min, foam_width, softness);
            let mut wake = 0.0f32;
            for impact in water.impacts.iter() {
                let dx = p[0] - impact.position[0];
                let dz = p[1] - impact.position[2];
                let radius = impact.radius.max(0.001);
                let t = ((dx * dx + dz * dz) / (radius * radius).max(0.000001)).clamp(0.0, 1.0);
                let push = 1.0 - t;
                let ring = (1.0 - ((t - 0.72).abs() / 0.28).clamp(0.0, 1.0)).powi(2);
                let strength = impact.strength * (1.0 / 180.0);
                wake += push * (strength * 0.70 + impact.cavitation * 0.28)
                    + ring * (strength * 0.30 + impact.cavitation * 0.92);
            }
            let wake = wake.clamp(0.0, 1.0);
            out[y * width + x] = [solid, edge.max(foam_edge), wake, spill_energy.max(wake)];
        }
    }
}

fn coastline_fill(signed: f32, foam_width: f32, softness: f32) -> (f32, f32, f32) {
    let inset = WATER_COASTLINE_INSET_METERS.max(softness);
    let block_t = ((-signed - inset) / softness.max(0.001)).clamp(0.0, 1.0);
    let solid = block_t * block_t * (3.0 - 2.0 * block_t);
    let foam_edge = 1.0 - (signed.abs() / foam_width.max(0.001)).clamp(0.0, 1.0);
    let spill_t = ((-signed) / inset).clamp(0.0, 1.0);
    let spill_energy = (1.0 - spill_t * 0.70) * (1.0 - solid);
    (solid, foam_edge.max(0.0), spill_energy.clamp(0.0, 1.0))
}

fn raster_impacts_3d(out: &mut [[f32; 4]], width: usize, height: usize, water: &Water3DState) {
    out.fill([0.0; 4]);
    if water.impacts.is_empty() {
        return;
    }
    let max_x = width.saturating_sub(1).max(1) as f32;
    let max_y = height.saturating_sub(1).max(1) as f32;
    let inv_x = max_x / water.size[0].abs().max(0.001);
    let inv_y = max_y / water.size[1].abs().max(0.001);
    for impact in water.impacts.iter() {
        let radius = impact.radius.max(0.001);
        let radius_sq = (radius * radius).max(0.000001);
        let inv_radius_sq = 1.0 / radius_sq;
        let min_x = (((impact.position[0] - radius) / water.size[0]) + 0.5) * max_x;
        let max_xf = (((impact.position[0] + radius) / water.size[0]) + 0.5) * max_x;
        let min_y = (((impact.position[2] - radius) / water.size[1]) + 0.5) * max_y;
        let max_yf = (((impact.position[2] + radius) / water.size[1]) + 0.5) * max_y;
        let x0 = min_x.floor().clamp(0.0, max_x) as usize;
        let x1 = max_xf.ceil().clamp(0.0, max_x) as usize;
        let y0 = min_y.floor().clamp(0.0, max_y) as usize;
        let y1 = max_yf.ceil().clamp(0.0, max_y) as usize;
        for y in y0..=y1 {
            let pz = (y as f32 / inv_y) - water.size[1] * 0.5;
            for x in x0..=x1 {
                let px = (x as f32 / inv_x) - water.size[0] * 0.5;
                let dx = px - impact.position[0];
                let dz = pz - impact.position[2];
                let dist_sq = dx * dx + dz * dz;
                if dist_sq > radius_sq {
                    continue;
                }
                let t = (dist_sq * inv_radius_sq).clamp(0.0, 1.0);
                let amount = 1.0 - t;
                if amount <= 0.0 {
                    continue;
                }
                let outline_width = (0.20 / radius).clamp(0.08, 0.42);
                let ring_center = (1.0 - outline_width * 0.65).clamp(0.42, 0.96);
                let ring =
                    (1.0 - ((t - ring_center).abs() / outline_width).clamp(0.0, 1.0)).powi(2);
                let strength = impact.strength * (1.0 / 180.0);
                let wake = amount * (strength * 0.70 + impact.cavitation * 0.28)
                    + ring * (strength * 0.44 + impact.cavitation * 1.08);
                let cell = &mut out[y * width + x];
                cell[2] = (cell[2] + wake).clamp(0.0, 1.0);
                cell[3] = cell[3].max((ring * 1.20 + amount * 0.22).clamp(0.0, 1.0));
            }
        }
    }
}

fn signed_distance_3d_xz(p: [f32; 2], shape: WaterCoastlineShape3D) -> f32 {
    match shape {
        WaterCoastlineShape3D::Box {
            center,
            half_extents,
            axis_x,
            axis_z,
        } => {
            let dx = p[0] - center[0];
            let dz = p[1] - center[2];
            let local_x = dx * axis_x[0] + dz * axis_x[1];
            let local_z = dx * axis_z[0] + dz * axis_z[1];
            let lx = local_x.abs() - half_extents[0];
            let ly = local_z.abs() - half_extents[2];
            let ox = lx.max(0.0);
            let oy = ly.max(0.0);
            (ox * ox + oy * oy).sqrt() + lx.max(ly).min(0.0)
        }
        WaterCoastlineShape3D::Sphere { center, radius }
        | WaterCoastlineShape3D::Cylinder { center, radius, .. } => {
            let dx = p[0] - center[0];
            let dz = p[1] - center[2];
            (dx * dx + dz * dz).sqrt() - radius
        }
        WaterCoastlineShape3D::Triangle { points } => {
            let tri = [
                [points[0][0], points[0][2]],
                [points[1][0], points[1][2]],
                [points[2][0], points[2][2]],
            ];
            let inside = point_in_triangle(p, tri);
            let d = distance_segment(p, tri[0], tri[1])
                .min(distance_segment(p, tri[1], tri[2]))
                .min(distance_segment(p, tri[2], tri[0]));
            if inside { -d } else { d }
        }
    }
}

fn water_cell_count(resolution: [u32; 2]) -> usize {
    if resolution[0] == 0 || resolution[1] == 0 {
        return 0;
    }
    let x = resolution[0].clamp(1, 256) as usize;
    let y = resolution[1].clamp(1, 256) as usize;
    x.saturating_mul(y)
}

fn water_center_cell_offset(water: &WaterGpu) -> usize {
    let width = water.sim[2].max(1);
    let height = water.sim[3].max(1);
    let center = (height / 2).saturating_mul(width).saturating_add(width / 2);
    water.sim[0].saturating_add(center.min(water.sim[1].saturating_sub(1))) as usize
}

#[derive(Clone, Copy, Debug, PartialEq)]
struct WaterQuerySampleOffsets {
    offsets: [usize; 4],
    frac: [f32; 2],
}

fn water_query_sample_offsets(water: &WaterGpu, local: [f32; 2]) -> WaterQuerySampleOffsets {
    let width = water.sim[2].max(1);
    let height = water.sim[3].max(1);
    let sx = water.size_depth_time[0].max(0.001);
    let sy = water.size_depth_time[1].max(0.001);
    let u = (local[0] / sx + 0.5).clamp(0.0, 1.0);
    let v = (local[1] / sy + 0.5).clamp(0.0, 1.0);
    let x = u * width.saturating_sub(1).max(1) as f32;
    let y = v * height.saturating_sub(1).max(1) as f32;
    let x0 = x.floor() as u32;
    let y0 = y.floor() as u32;
    let x1 = (x0 + 1).min(width - 1);
    let y1 = (y0 + 1).min(height - 1);
    WaterQuerySampleOffsets {
        offsets: [
            water_query_offset_from_xy(water, width, x0, y0),
            water_query_offset_from_xy(water, width, x1, y0),
            water_query_offset_from_xy(water, width, x0, y1),
            water_query_offset_from_xy(water, width, x1, y1),
        ],
        frac: [x.fract(), y.fract()],
    }
}

fn water_query_offset_from_xy(water: &WaterGpu, width: u32, x: u32, y: u32) -> usize {
    let cell = y
        .saturating_mul(width)
        .saturating_add(x)
        .min(water.sim[1].saturating_sub(1));
    water.sim[0].saturating_add(cell) as usize
}

fn water_lerp_cell(
    c00: [f32; 4],
    c10: [f32; 4],
    c01: [f32; 4],
    c11: [f32; 4],
    frac: [f32; 2],
) -> [f32; 4] {
    let tx = frac[0].clamp(0.0, 1.0);
    let ty = frac[1].clamp(0.0, 1.0);
    let mut out = [0.0; 4];
    for i in 0..4 {
        let a = c00[i] + (c10[i] - c00[i]) * tx;
        let b = c01[i] + (c11[i] - c01[i]) * tx;
        out[i] = a + (b - a) * ty;
    }
    out
}

fn water_3d_vertex_count(water: &WaterGpu) -> u32 {
    if water.sim[1] == 0 {
        return 0;
    }
    let width = water.flags[0].clamp(1, WATER_MAX_RENDER_RESOLUTION);
    let height = water.flags[1].clamp(1, WATER_MAX_RENDER_RESOLUTION);
    if water.shape[0] >= 0.5 {
        let segments = width
            .max(height)
            .saturating_mul(4)
            .clamp(16, WATER_MAX_RENDER_RESOLUTION);
        let rings = width
            .min(height)
            .saturating_div(2)
            .clamp(1, WATER_MAX_RENDER_RESOLUTION / 2);
        return rings
            .saturating_mul(segments)
            .saturating_mul(6)
            .saturating_add(segments.saturating_mul(6));
    }
    let surface = width
        .saturating_sub(1)
        .saturating_mul(height.saturating_sub(1))
        .saturating_mul(6);
    let side = water_3d_side_vertex_count(water);
    surface.saturating_add(side)
}

fn water_3d_side_vertex_count(water: &WaterGpu) -> u32 {
    let width = water.flags[0].clamp(1, WATER_MAX_RENDER_RESOLUTION);
    let height = water.flags[1].clamp(1, WATER_MAX_RENDER_RESOLUTION);
    width
        .saturating_sub(1)
        .saturating_add(height.saturating_sub(1))
        .saturating_mul(2)
        .saturating_mul(6)
}

fn water_lod_2d(water: &Water2DState, camera: [f32; 2]) -> WaterLodDecision {
    let pos = [water.model[2][0], water.model[2][1]];
    water_lod(
        water.resolution,
        water.render_resolution,
        water.size,
        [
            water.lod_near_distance,
            water.lod_mid_distance,
            water.lod_far_distance,
        ],
        water.lod_min_resolution,
        pos,
        camera,
    )
}

fn water_lod_3d(water: &Water3DState, camera: [f32; 3]) -> WaterLodDecision {
    let pos = water.model[3];
    let radius = water_lod_shape_radius(water.shape, water.size);
    let lod = water_lod_from_distance(
        water.resolution,
        water.render_resolution,
        [
            water.lod_near_distance,
            water.lod_mid_distance,
            water.lod_far_distance,
        ],
        water.lod_min_resolution,
        water_lod_surface_distance([pos[0], pos[2]], [camera[0], camera[2]], radius),
        WATER_3D_MAX_RENDER_RESOLUTION,
        4.0,
    );
    WaterLodDecision {
        grid: WaterGridResolution {
            sim: [
                water.resolution[0].clamp(1, 256),
                water.resolution[1].clamp(1, 256),
            ],
            render: lod.grid.render,
        },
        ripple_blend: lod.ripple_blend,
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
struct WaterLodDecision {
    grid: WaterGridResolution,
    ripple_blend: f32,
}

fn water_lod(
    sim_resolution: [u32; 2],
    render_resolution: [u32; 2],
    _size: [f32; 2],
    distances: [f32; 3],
    min_resolution: [u32; 2],
    water_pos: [f32; 2],
    camera_pos: [f32; 2],
) -> WaterLodDecision {
    let dx = water_pos[0] - camera_pos[0];
    let dy = water_pos[1] - camera_pos[1];
    let distance = (dx * dx + dy * dy).sqrt();
    water_lod_from_distance(
        sim_resolution,
        render_resolution,
        distances,
        min_resolution,
        distance,
        WATER_MAX_RENDER_RESOLUTION,
        3.0,
    )
}

fn water_lod_from_distance(
    sim_resolution: [u32; 2],
    render_resolution: [u32; 2],
    distances: [f32; 3],
    min_resolution: [u32; 2],
    distance: f32,
    max_render_resolution: u32,
    render_lod_strength: f32,
) -> WaterLodDecision {
    let near = distances[0].max(5.0);
    let mid = distances[1].max(near);
    let far = distances[2].max(mid);
    let (lod_t, ripple_blend) = if distance <= near {
        (0.0, 1.0)
    } else if distance <= mid {
        let span = (mid - near).max(0.001);
        let t = smooth01(((distance - near) / span).clamp(0.0, 1.0));
        (t * 0.42, 1.0 - t * 0.18)
    } else if distance <= far {
        let span = (far - mid).max(0.001);
        let t = smooth01(((distance - mid) / span).clamp(0.0, 1.0));
        (0.42 + t * 0.58, 0.82 - t * 0.42)
    } else {
        return WaterLodDecision {
            grid: WaterGridResolution {
                sim: [0, 0],
                render: [0, 0],
            },
            ripple_blend: 0.0,
        };
    };
    let q = lod_t * lod_t * (3.0 - 2.0 * lod_t);
    let sim_div = 1.0 + q * 3.5;
    let render_div = 1.0 + q * render_lod_strength.max(0.0);
    WaterLodDecision {
        grid: WaterGridResolution {
            sim: [
                ((sim_resolution[0] as f32 / sim_div).round() as u32)
                    .clamp(min_resolution[0].clamp(1, 256), 256),
                ((sim_resolution[1] as f32 / sim_div).round() as u32)
                    .clamp(min_resolution[1].clamp(1, 256), 256),
            ],
            render: [
                ((render_resolution[0] as f32 / render_div).round() as u32)
                    .clamp(2, max_render_resolution),
                ((render_resolution[1] as f32 / render_div).round() as u32)
                    .clamp(2, max_render_resolution),
            ],
        },
        ripple_blend,
    }
}

fn smooth01(t: f32) -> f32 {
    t * t * (3.0 - 2.0 * t)
}

fn water_lod_shape_radius(shape: WaterShapeState, size: [f32; 2]) -> f32 {
    match shape {
        WaterShapeState::Rect => size[0].max(size[1]) * 0.5,
        WaterShapeState::Circle { radius } | WaterShapeState::Cylinder { radius, .. } => radius,
    }
}

fn water_lod_surface_distance(water_pos: [f32; 2], camera_pos: [f32; 2], radius: f32) -> f32 {
    let dx = water_pos[0] - camera_pos[0];
    let dz = water_pos[1] - camera_pos[1];
    ((dx * dx + dz * dz).sqrt() - radius.max(0.0)).max(0.0)
}

fn water_gpu_2d(
    node: NodeID,
    water: &Water2DState,
    resolution: WaterGridResolution,
    cell_offset: u32,
    cell_count: u32,
    ripple_blend: f32,
) -> WaterGpu {
    water_gpu_common(
        node,
        2,
        water.idle_mode,
        water.size,
        water.depth,
        water.flow,
        water.wind,
        water.shape,
        resolution,
        water.wave_speed,
        water.wave_scale,
        water.wave_length,
        water.damping,
        water.wake_strength,
        water.foam_strength,
        water.collision_layers.bits(),
        water.collision_mask.bits(),
        water.coastline_foam_color.into(),
        water.deep_color.into(),
        water.shallow_color.into(),
        water.shallow_depth,
        [0.0, 0.0, 0.0, 0.0],
        water.foam_color.into(),
        [
            water.transparency,
            water.reflectivity,
            water.roughness,
            water.fresnel_power,
        ],
        [
            water.normal_strength,
            water.ripple_scale,
            water.foam_amount,
            water.crest_foam_threshold,
        ],
        [
            water.caustic_strength,
            water.refraction_strength,
            water.scattering_strength,
            water.distance_fog_strength,
        ],
        [
            water.coastline_foam_strength,
            water.coastline_foam_width,
            water.coastline_cutoff_softness,
            water.coastline_wave_damping,
        ],
        water.coastline_wave_reflection,
        water.coastline_edge_noise,
        water.debug,
        water.paused,
        water.model,
        [0.0, 0.0, 0.0, 1.0],
        water.z_index,
        cell_offset,
        cell_count,
        ripple_blend,
    )
}

fn water_gpu_3d(
    node: NodeID,
    water: &Water3DState,
    resolution: WaterGridResolution,
    cell_offset: u32,
    cell_count: u32,
    ripple_blend: f32,
    sky_color: [f32; 3],
) -> WaterGpu {
    water_gpu_common(
        node,
        3,
        water.idle_mode,
        water.size,
        water.depth,
        water.flow,
        water.wind,
        water.shape,
        resolution,
        water.wave_speed,
        water.wave_scale,
        water.wave_length,
        water.damping,
        water.wake_strength,
        water.foam_strength,
        water.collision_layers.bits(),
        water.collision_mask.bits(),
        water.coastline_foam_color.into(),
        water.deep_color.into(),
        water.shallow_color.into(),
        water.shallow_depth,
        [
            sky_color[0],
            sky_color[1],
            sky_color[2],
            water.sky_bias_ratio.clamp(0.0, 1.0),
        ],
        water.foam_color.into(),
        [
            water.transparency,
            water.reflectivity,
            water.roughness,
            water.fresnel_power,
        ],
        [
            water.normal_strength,
            water.ripple_scale,
            water.foam_amount,
            water.crest_foam_threshold,
        ],
        [
            water.caustic_strength,
            water.refraction_strength,
            water.scattering_strength,
            water.distance_fog_strength,
        ],
        [
            water.coastline_foam_strength,
            water.coastline_foam_width,
            water.coastline_cutoff_softness,
            water.coastline_wave_damping,
        ],
        water.coastline_wave_reflection,
        water.coastline_edge_noise,
        water.debug,
        water.paused,
        [
            [water.model[0][0], water.model[0][1], water.model[0][2]],
            [water.model[1][0], water.model[1][1], water.model[1][2]],
            [water.model[2][0], water.model[2][1], water.model[2][2]],
        ],
        water.model[3],
        0,
        cell_offset,
        cell_count,
        ripple_blend,
    )
}

#[allow(clippy::too_many_arguments)]
fn water_gpu_common(
    node: NodeID,
    kind: u32,
    idle_mode: WaterIdleModeState,
    size: [f32; 2],
    depth: f32,
    flow: [f32; 2],
    wind: [f32; 2],
    shape: WaterShapeState,
    resolution: WaterGridResolution,
    wave_speed: f32,
    wave_scale: f32,
    wave_length: f32,
    damping: f32,
    wake_strength: f32,
    foam_strength: f32,
    _collision_layers: u32,
    _collision_mask: u32,
    coastline_foam_color: [f32; 4],
    deep_color: [f32; 4],
    shallow_color: [f32; 4],
    shallow_depth: f32,
    sky_color_bias: [f32; 4],
    foam_color: [f32; 4],
    visual0: [f32; 4],
    visual1: [f32; 4],
    visual2: [f32; 4],
    coastline: [f32; 4],
    coastline_wave_reflection: f32,
    coastline_edge_noise: f32,
    debug: bool,
    paused: bool,
    model: [[f32; 3]; 3],
    model_w: [f32; 4],
    z_index: i32,
    cell_offset: u32,
    cell_count: u32,
    ripple_blend: f32,
) -> WaterGpu {
    WaterGpu {
        node: node.index(),
        kind,
        idle_mode: idle_mode as u32,
        z_index,
        size_depth_time: [size[0], size[1], depth, shallow_depth.max(-1.0)],
        flow_wind: [flow[0], flow[1], wind[0], wind[1]],
        wave: [wave_speed, wave_scale, damping, wake_strength],
        flags: [
            resolution.render[0].clamp(1, WATER_MAX_RENDER_RESOLUTION),
            resolution.render[1].clamp(1, WATER_MAX_RENDER_RESOLUTION),
            (u32::from(debug) * WATER_FLAG_DEBUG) | (u32::from(paused) * WATER_FLAG_PAUSED),
            foam_strength.to_bits(),
        ],
        deep_color,
        shallow_color,
        sky_color_bias,
        foam_color,
        visual0,
        visual1,
        visual2,
        wave_profile: [wave_length.max(0.001), 0.0, 0.0, 0.0],
        coastline_foam_color,
        coastline,
        shape: water_shape_gpu(shape, size, depth),
        sim: [
            cell_offset,
            cell_count,
            resolution.sim[0].clamp(1, 256),
            resolution.sim[1].clamp(1, 256),
        ],
        model_x: [
            model[0][0],
            model[0][1],
            model[0][2],
            ripple_blend.clamp(0.0, 1.0),
        ],
        model_y: [
            model[1][0],
            model[1][1],
            model[1][2],
            coastline_wave_reflection.clamp(0.0, 2.0),
        ],
        model_z: [
            model[2][0],
            model[2][1],
            model[2][2],
            coastline_edge_noise.clamp(0.0, 1.0),
        ],
        model_w,
    }
}

fn water_shape_gpu(shape: WaterShapeState, size: [f32; 2], depth: f32) -> [f32; 4] {
    match shape {
        WaterShapeState::Rect => [0.0, size[0], size[1], depth],
        WaterShapeState::Circle { radius } => [1.0, radius, depth, 0.0],
        WaterShapeState::Cylinder {
            radius,
            half_height,
        } => [2.0, radius, half_height, 0.0],
    }
}

const WATER_WGSL: &str = perro_macros::include_str_stripped!("water_shaders/water_compute.wgsl");
const WATER_3D_RENDER_WGSL: &str =
    perro_macros::include_str_stripped!("water_shaders/water_3d_render.wgsl");

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
mod tests {
    use super::*;
    use std::sync::Arc;

    fn test_water_2d() -> Water2DState {
        Water2DState {
            model: [[1.0, 0.0, 0.0], [0.0, 1.0, 0.0], [0.0, 0.0, 1.0]],
            z_index: 0,
            paused: false,
            simulation_time: 0.0,
            simulation_delta: 1.0 / 60.0,
            size: [16.0, 16.0],
            shape: WaterShapeState::Rect,
            resolution: [8, 8],
            render_resolution: [16, 16],
            depth: 4.0,
            flow: [0.0, 0.0],
            wind: [1.0, 0.0],
            idle_mode: WaterIdleModeState::Calm,
            wave_speed: 1.0,
            wave_scale: 1.0,
            wave_length: 18.0,
            damping: 0.985,
            wake_strength: 1.35,
            foam_strength: 0.9,
            sample_readback_rate: 30.0,
            lod_near_distance: 128.0,
            lod_mid_distance: 384.0,
            lod_far_distance: 896.0,
            lod_min_resolution: [4, 4],
            collision_layers: perro_structs::BitMask::ALL,
            collision_mask: perro_structs::BitMask::NONE,
            deep_color: perro_structs::Color::new(0.02, 0.16, 0.28, 0.94),
            shallow_color: perro_structs::Color::new(0.08, 0.46, 0.62, 0.74),
            shallow_depth: -1.0,
            sky_bias_ratio: 0.0,
            transparency: 0.24,
            reflectivity: 0.46,
            roughness: 0.18,
            fresnel_power: 5.0,
            normal_strength: 1.15,
            ripple_scale: 1.0,
            foam_color: perro_structs::Color::new(0.86, 0.96, 1.0, 1.0),
            foam_amount: 0.72,
            crest_foam_threshold: 0.58,
            caustic_strength: 0.20,
            refraction_strength: 0.12,
            scattering_strength: 0.18,
            distance_fog_strength: 0.32,
            coastline_foam_color: perro_structs::Color::new(0.9, 0.97, 1.0, 1.0),
            coastline_foam_strength: 0.75,
            coastline_foam_width: 1.5,
            coastline_cutoff_softness: 0.25,
            coastline_wave_reflection: 0.45,
            coastline_wave_damping: 0.35,
            coastline_edge_noise: 0.2,
            debug: false,
            links: Arc::from([perro_render_bridge::WaterLinkState {
                other: NodeID::from_parts(99, 0),
                overlap_min: [-1.0, -1.0],
                overlap_max: [1.0, 1.0],
                blend_width: 1.0,
                wave_transfer: 1.0,
                flow_transfer: 1.0,
            }]),
            queries: Arc::from([]),
            impacts: Arc::from([perro_render_bridge::WaterImpact2D {
                position: [0.0, 0.0],
                velocity: [1.0, 0.0],
                strength: 2.0,
                radius: 2.0,
                cavitation: 0.5,
            }]),
            coastline_shapes: Arc::from([]),
        }
    }

    fn test_water_3d() -> Water3DState {
        Water3DState {
            model: [
                [1.0, 0.0, 0.0, 0.0],
                [0.0, 1.0, 0.0, 0.0],
                [0.0, 0.0, 1.0, 0.0],
                [0.0, 0.0, 0.0, 1.0],
            ],
            paused: false,
            simulation_time: 0.0,
            simulation_delta: 1.0 / 60.0,
            size: [16.0, 16.0],
            shape: WaterShapeState::Rect,
            resolution: [8, 8],
            render_resolution: [16, 16],
            depth: 4.0,
            flow: [0.0, 0.0],
            wind: [1.0, 0.0],
            idle_mode: WaterIdleModeState::Calm,
            wave_speed: 1.0,
            wave_scale: 1.0,
            wave_length: 18.0,
            damping: 0.985,
            wake_strength: 1.35,
            foam_strength: 0.9,
            sample_readback_rate: 30.0,
            lod_near_distance: 128.0,
            lod_mid_distance: 384.0,
            lod_far_distance: 896.0,
            lod_min_resolution: [4, 4],
            collision_layers: perro_structs::BitMask::ALL,
            collision_mask: perro_structs::BitMask::NONE,
            deep_color: perro_structs::Color::new(0.02, 0.16, 0.28, 0.94),
            shallow_color: perro_structs::Color::new(0.08, 0.46, 0.62, 0.74),
            shallow_depth: -1.0,
            sky_bias_ratio: 0.0,
            transparency: 0.24,
            reflectivity: 0.46,
            roughness: 0.18,
            fresnel_power: 5.0,
            normal_strength: 1.15,
            ripple_scale: 1.0,
            foam_color: perro_structs::Color::new(0.86, 0.96, 1.0, 1.0),
            foam_amount: 0.72,
            crest_foam_threshold: 0.58,
            caustic_strength: 0.20,
            refraction_strength: 0.12,
            scattering_strength: 0.18,
            distance_fog_strength: 0.32,
            coastline_foam_color: perro_structs::Color::new(0.9, 0.97, 1.0, 1.0),
            coastline_foam_strength: 0.75,
            coastline_foam_width: 1.5,
            coastline_cutoff_softness: 0.25,
            coastline_wave_reflection: 0.45,
            coastline_wave_damping: 0.35,
            coastline_edge_noise: 0.2,
            debug: false,
            links: Arc::from([]),
            queries: Arc::from([]),
            impacts: Arc::from([perro_render_bridge::WaterImpact3D {
                position: [0.0, 0.0, 0.0],
                velocity: [1.0, 0.0, 0.0],
                strength: 2.0,
                radius: 2.0,
                cavitation: 0.5,
            }]),
            coastline_shapes: Arc::from([]),
        }
    }

    #[test]
    fn water_wgsl_parses() {
        naga::front::wgsl::parse_str(WATER_WGSL).expect("water wgsl should parse");
        let render_wgsl = water_render_wgsl();
        naga::front::wgsl::parse_str(&render_wgsl).expect("water render wgsl should parse");
        naga::front::wgsl::parse_str(WATER_3D_RENDER_WGSL)
            .expect("water 3d render wgsl should parse");
        assert!(!WATER_3D_RENDER_WGSL.contains("water_screen_contact_outline"));
        assert!(!WATER_3D_RENDER_WGSL.contains("outline_white"));
        assert!(WATER_3D_RENDER_WGSL.contains("water_analytic_wave"));
        assert!(WATER_3D_RENDER_WGSL.contains("water_depth_thickness"));
        assert!(!WATER_3D_RENDER_WGSL.contains("water_surface_contact_foam"));
        assert!(WATER_3D_RENDER_WGSL.contains("vec4<f32>(w.model_x.xyz, 0.0)"));
        assert!(WATER_3D_RENDER_WGSL.contains("vec4<f32>(w.model_y.xyz, 0.0)"));
        assert!(WATER_3D_RENDER_WGSL.contains("vec4<f32>(w.model_z.xyz, 0.0)"));
        assert!(WATER_3D_RENDER_WGSL.contains("let width = max(w.sim.z, 1u);"));
        assert!(WATER_3D_RENDER_WGSL.contains("let width = max(w.flags.x, 1u);"));
        assert!(WATER_3D_RENDER_WGSL.contains("water_circle_surface_vertex"));
        assert!(WATER_3D_RENDER_WGSL.contains("water_circle_side_vertex"));
        assert!(WATER_3D_RENDER_WGSL.contains("horizontal_segments * 2u + vertical_segments"));
        assert!(WATER_3D_RENDER_WGSL.contains("vec2<u32>(0u, 0u),"));
        assert!(WATER_3D_RENDER_WGSL.contains("vec2<u32>(1u, 1u),"));
        assert!(WATER_3D_RENDER_WGSL.contains("vec2<u32>(1u, 0u),"));
        assert!(WATER_3D_RENDER_WGSL.contains("vec2<u32>(0u, 1u),"));
    }

    #[test]
    fn rect_water_3d_side_vertices_follow_grid_edges() {
        let mut water = water_gpu_3d(
            NodeID::from_parts(1, 0),
            &test_water_3d(),
            WaterGridResolution {
                sim: [8, 6],
                render: [8, 6],
            },
            0,
            water_cell_count([8, 6]) as u32,
            1.0,
            [0.0, 0.0, 0.0],
        );
        water.shape = [0.0, 16.0, 16.0, 4.0];

        let surface = (8 - 1) * (6 - 1) * 6;
        let side = ((8 - 1) + (6 - 1)) * 2 * 6;
        assert_eq!(water_3d_vertex_count(&water), surface + side);
    }

    #[test]
    fn rotated_box_coastline_distance_uses_shape_axes() {
        let shape = WaterCoastlineShape3D::Box {
            center: [0.0, 0.0, 0.0],
            half_extents: [4.0, 1.0, 1.0],
            axis_x: [0.0, 1.0],
            axis_z: [-1.0, 0.0],
        };

        assert!(signed_distance_3d_xz([0.0, 3.5], shape) < 0.0);
        assert!(signed_distance_3d_xz([3.5, 0.0], shape) > 0.0);
    }

    #[test]
    fn coastline_fill_keeps_foam_inside_one_meter_before_cutoff() {
        let (edge_solid, edge_foam, edge_energy) = coastline_fill(-0.25, 1.5, 0.25);
        assert!(edge_solid < 0.01);
        assert!(edge_foam > 0.8);
        assert!(edge_energy > 0.7);

        let (deep_solid, deep_foam, deep_energy) = coastline_fill(-1.5, 1.5, 0.25);
        assert!(deep_solid > 0.9);
        assert!(deep_foam <= 0.01);
        assert!(deep_energy < 0.1);
    }

    #[test]
    fn water_lod_resolution_clamps_with_distance() {
        assert_eq!(
            water_lod(
                [256, 256],
                [512, 512],
                [64.0, 64.0],
                [128.0, 384.0, 896.0],
                [32, 32],
                [0.0, 0.0],
                [0.0, 0.0]
            ),
            WaterLodDecision {
                grid: WaterGridResolution {
                    sim: [256, 256],
                    render: [512, 512],
                },
                ripple_blend: 1.0,
            }
        );
        let mid = water_lod(
            [256, 256],
            [512, 512],
            [64.0, 64.0],
            [128.0, 384.0, 896.0],
            [32, 32],
            [512.0, 0.0],
            [0.0, 0.0],
        );
        assert_eq!(mid.grid.sim, [91, 91]);
        assert_eq!(mid.grid.render, [201, 201]);
        assert!(mid.ripple_blend > 0.75 && mid.ripple_blend < 0.85);
        let high = water_lod(
            [4096, 4096],
            [4096, 4096],
            [64.0, 64.0],
            [128.0, 384.0, 896.0],
            [32, 32],
            [0.0, 0.0],
            [0.0, 0.0],
        );
        assert_eq!(high.grid.sim, [256, 256]);
        assert_eq!(high.grid.render, [1024, 1024]);
        assert_eq!(
            water_lod(
                [256, 256],
                [512, 512],
                [64.0, 64.0],
                [128.0, 384.0, 896.0],
                [32, 32],
                [2048.0, 0.0],
                [0.0, 0.0]
            ),
            WaterLodDecision {
                grid: WaterGridResolution {
                    sim: [0, 0],
                    render: [0, 0],
                },
                ripple_blend: 0.0,
            }
        );
        assert_eq!(water_cell_count([0, 0]), 0);
        assert_eq!(water_cell_count([1, 1]), 1);
    }

    #[test]
    fn water_lod_3d_keeps_simulation_active_while_render_lods() {
        let mut water = test_water_3d();
        water.resolution = [4096, 2048];
        water.render_resolution = [256, 256];
        let near = water_lod_3d(&water, [0.0, 2.0, 0.0]);
        let mid = water_lod_3d(&water, [260.0, 2.0, 0.0]);
        let culled = water_lod_3d(&water, [100_000.0, 2.0, 100_000.0]);

        assert_eq!(near.grid.sim, [256, 256]);
        assert_eq!(mid.grid.sim, near.grid.sim);
        assert_eq!(culled.grid.sim, near.grid.sim);
        assert!(mid.grid.render[0] < near.grid.render[0]);
        assert_eq!(culled.grid.render, [0, 0]);
        assert!(mid.ripple_blend < near.ripple_blend);
        assert_eq!(culled.ripple_blend, 0.0);
    }

    #[test]
    fn water_readback_interval_uses_rate() {
        assert_eq!(readback_interval_seconds(0.0), 0.0);
        assert!((readback_interval_seconds(60.0) - (1.0 / 60.0)).abs() < 1.0e-6);
        assert!((readback_interval_seconds(30.0) - (1.0 / 30.0)).abs() < 1.0e-6);
        assert!((readback_interval_seconds(15.0) - (1.0 / 15.0)).abs() < 1.0e-6);
    }

    #[test]
    fn water_query_offsets_sample_four_cells_for_bilinear_height() {
        let water = water_gpu_3d(
            NodeID::from_parts(1, 0),
            &test_water_3d(),
            WaterGridResolution {
                sim: [4, 4],
                render: [4, 4],
            },
            10,
            16,
            1.0,
            [0.0, 0.0, 0.0],
        );
        let sample = water_query_sample_offsets(&water, [0.0, 0.0]);
        assert_eq!(sample.offsets, [15, 16, 19, 20]);
        assert_eq!(sample.frac, [0.5, 0.5]);
        let cell = water_lerp_cell(
            [0.0, 0.0, 0.0, 0.0],
            [2.0, 0.0, 0.0, 0.0],
            [4.0, 0.0, 0.0, 0.0],
            [6.0, 0.0, 0.0, 0.0],
            sample.frac,
        );
        assert_eq!(cell[0], 3.0);
    }

    #[test]
    fn water_gpu_2d_staging_accepts_linked_water_state() {
        let water = test_water_2d();
        let staged = water_gpu_2d(
            NodeID::from_parts(7, 0),
            &water,
            WaterGridResolution {
                sim: water.resolution,
                render: water.resolution,
            },
            4,
            64,
            1.0,
        );
        assert_eq!(staged.node, 7);
        assert_eq!(staged.sim, [4, 64, 8, 8]);
        assert_eq!(staged.kind, 2);
        assert_eq!(staged.flags[2] & WATER_FLAG_PAUSED, 0);
        let mut paused = water;
        paused.paused = true;
        let paused_staged = water_gpu_2d(
            NodeID::from_parts(7, 0),
            &paused,
            WaterGridResolution {
                sim: paused.resolution,
                render: paused.resolution,
            },
            4,
            64,
            1.0,
        );
        assert_ne!(paused_staged.flags[2] & WATER_FLAG_PAUSED, 0);
    }

    #[test]
    fn water_gpu_raster_impacts_2d_and_3d_write_wake_cells() {
        let water_2d = test_water_2d();
        let mut cells_2d = vec![[0.0; 4]; 64];
        raster_impacts_2d(&mut cells_2d, 8, 8, &water_2d);
        assert!(cells_2d.iter().any(|cell| cell[2] > 0.0 && cell[3] > 0.0));

        let water_3d = test_water_3d();
        let mut cells_3d = vec![[0.0; 4]; 64];
        raster_impacts_3d(&mut cells_3d, 8, 8, &water_3d);
        assert!(cells_3d.iter().any(|cell| cell[2] > 0.0 && cell[3] > 0.0));
    }
}
