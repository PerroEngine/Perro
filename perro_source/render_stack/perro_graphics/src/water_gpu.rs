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
const WATER_CHUNK_QUADS: u32 = 48;

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
    compute_bind_group: wgpu::BindGroup,
    render_bind_group: wgpu::BindGroup,
    depth_bind_group: wgpu::BindGroup,
    water_buffer: wgpu::Buffer,
    cell_buffer: wgpu::Buffer,
    coastline_buffer: wgpu::Buffer,
    render_chunk_buffer: wgpu::Buffer,
    params_buffer: wgpu::Buffer,
    readback_buffer: wgpu::Buffer,
    water_capacity: usize,
    cell_capacity: usize,
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
    readback_queries: Vec<WaterBodyQueryState>,
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
                        ty: wgpu::BufferBindingType::Storage { read_only: false },
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
                cull_mode: Some(wgpu::Face::Back),
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
                depth_write_enabled: Some(false),
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
        let cell_buffer = empty_buffer(device, "perro_water_gpu_cells", 64, false);
        let coastline_buffer = empty_buffer(device, "perro_water_gpu_coastline", 64, false);
        let render_chunk_buffer = empty_buffer(device, "perro_water_gpu_render_chunks", 1, true);
        let readback_buffer = readback_buffer(device, 1);
        let params_buffer = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("perro_water_gpu_params"),
            size: std::mem::size_of::<WaterParamsGpu>() as u64,
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });
        let compute_bind_group = make_compute_bind_group(
            device,
            &compute_bgl,
            &water_buffer,
            &cell_buffer,
            &coastline_buffer,
            &params_buffer,
            "perro_water_gpu_bg",
        );
        let render_bind_group = make_render_bind_group(
            device,
            &render_bgl,
            &water_buffer,
            &cell_buffer,
            &coastline_buffer,
            &render_chunk_buffer,
            &params_buffer,
            "perro_water_render_bg",
        );
        let depth_bind_group = make_depth_bind_group(
            device,
            &depth_bgl,
            scene_depth_view,
            "perro_water_depth_bg",
        );
        Self {
            compute_pipeline,
            render_pipeline_2d,
            render_pipeline_3d,
            compute_bgl,
            render_bgl,
            depth_bgl,
            compute_bind_group,
            render_bind_group,
            depth_bind_group,
            water_buffer,
            cell_buffer,
            coastline_buffer,
            render_chunk_buffer,
            params_buffer,
            readback_buffer,
            water_capacity: 1,
            cell_capacity: 64,
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
            let staged = *self.staged_waters.last().expect("staged water");
            build_render_chunks_3d(
                &mut self.staged_render_chunks,
                water_idx,
                water,
                staged,
                &ctx.camera_3d_frustum_planes,
            );
            cell_needed = cell_needed.saturating_add(cells);
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
            self.compute_bind_group = make_compute_bind_group(
                device,
                &self.compute_bgl,
                &self.water_buffer,
                &self.cell_buffer,
                &self.coastline_buffer,
                &self.params_buffer,
                "perro_water_gpu_bg",
            );
            self.render_bind_group = make_render_bind_group(
                device,
                &self.render_bgl,
                &self.water_buffer,
                &self.cell_buffer,
                &self.coastline_buffer,
                &self.render_chunk_buffer,
                &self.params_buffer,
                "perro_water_render_bg",
            );
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
                self.readback_queries.push(*query);
                self.readback_offsets
                    .push(water_query_cell_offset(water, query.local));
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
                self.readback_queries.push(*query);
                self.readback_offsets
                    .push(water_query_cell_offset(water, query.local));
                debug_assert_eq!(query.water, *node);
            }
        }
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
        pass.set_bind_group(0, &self.compute_bind_group, &[]);
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
        pass.set_bind_group(0, &self.render_bind_group, &[]);
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
        pass.set_bind_group(0, &self.render_bind_group, &[]);
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
                &self.cell_buffer,
                offset as u64 * elem,
                &self.readback_buffer,
                idx as u64 * elem,
                elem,
            );
        }
        self.readback_accum_seconds =
            (self.readback_accum_seconds - self.readback_interval_seconds).max(0.0);
        for node in self.readback_scheduled_nodes.iter().copied() {
            let Some(interval) = self.readback_water_interval.get(&node).copied() else {
                continue;
            };
            let Some(accum) = self.readback_water_accum.get_mut(&node) else {
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
            self.cell_buffer = empty_buffer(device, "perro_water_gpu_cells", cap, false);
            self.coastline_buffer = empty_buffer(device, "perro_water_gpu_coastline", cap, false);
            self.cell_capacity = cap;
            rebuilt = true;
        }
        if needed_waters > self.readback_capacity {
            let mut cap = self.readback_capacity.max(64);
            while cap < needed_waters {
                cap *= 2;
            }
            self.readback_buffer = readback_buffer(device, cap);
            self.readback_capacity = cap;
            self.readback_pending_rx = None;
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
                for (idx, query) in self.readback_queries.iter().enumerate() {
                    let cell = cells
                        .get(self.readback_water_sample_count + idx)
                        .copied()
                        .unwrap_or([0.0; 4]);
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
    let active_scale = if has_queries || has_impacts {
        1.0
    } else if ripple_blend >= 0.85 {
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
        ],
    })
}

fn make_render_bind_group(
    device: &wgpu::Device,
    layout: &wgpu::BindGroupLayout,
    waters: &wgpu::Buffer,
    cells: &wgpu::Buffer,
    coastline: &wgpu::Buffer,
    render_chunks: &wgpu::Buffer,
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
                binding: 4,
                resource: render_chunks.as_entire_binding(),
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

fn water_query_cell_offset(water: &WaterGpu, local: [f32; 2]) -> usize {
    let width = water.sim[2].max(1);
    let height = water.sim[3].max(1);
    let sx = water.size_depth_time[0].max(0.001);
    let sy = water.size_depth_time[1].max(0.001);
    let u = (local[0] / sx + 0.5).clamp(0.0, 0.999_999);
    let v = (local[1] / sy + 0.5).clamp(0.0, 0.999_999);
    let x = (u * width as f32).floor() as u32;
    let y = (v * height as f32).floor() as u32;
    let cell = y
        .saturating_mul(width)
        .saturating_add(x)
        .min(water.sim[1].saturating_sub(1));
    water.sim[0].saturating_add(cell) as usize
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
    let pos = [water.model[3][0], water.model[3][2]];
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
        [camera[0], camera[2]],
    )
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
    let near = distances[0].max(5.0);
    let mid = distances[1].max(near);
    let far = distances[2].max(mid);
    let (lod_t, ripple_blend) = if distance <= near {
        (0.0, 1.0)
    } else if distance <= mid {
        let span = (mid - near).max(0.001);
        let t = ((distance - near) / span).clamp(0.0, 1.0);
        (t * 0.5, 1.0 - t * 0.35)
    } else if distance <= far {
        let span = (far - mid).max(0.001);
        let t = ((distance - mid) / span).clamp(0.0, 1.0);
        (0.5 + t * 0.5, 0.65 - t * 0.45)
    } else {
        return WaterLodDecision {
            grid: WaterGridResolution {
                sim: [0, 0],
                render: [0, 0],
            },
            ripple_blend: 0.0,
        };
    };
    let q = lod_t * lod_t;
    let (early_sim_div, early_render_div, early_ripple) = if distance <= 5.0 {
        (1.0, 1.0, 1.0)
    } else if distance <= 10.0 {
        (1.0 / 0.95, 1.0 / 0.975, 0.95)
    } else if distance <= 15.0 {
        (1.0 / 0.90, 1.0 / 0.95, 0.90)
    } else if distance <= 20.0 {
        (1.0 / 0.75, 1.0 / 0.875, 0.82)
    } else {
        (1.0, 1.0, 1.0)
    };
    let sim_div = early_sim_div * (1.0 + q * 7.0);
    let render_div = early_render_div * (1.0 + q * 3.0);
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
                    .clamp(1, WATER_MAX_RENDER_RESOLUTION),
                ((render_resolution[1] as f32 / render_div).round() as u32)
                    .clamp(1, WATER_MAX_RENDER_RESOLUTION),
            ],
        },
        ripple_blend: ripple_blend.min(early_ripple),
    }
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

const WATER_WGSL: &str = r#"
struct Water {
    node: u32,
    kind: u32,
    idle_mode: u32,
    z_index: i32,
    size_depth_time: vec4<f32>,
    flow_wind: vec4<f32>,
    wave: vec4<f32>,
    flags: vec4<u32>,
    deep_color: vec4<f32>,
    shallow_color: vec4<f32>,
    sky_color_bias: vec4<f32>,
    foam_color: vec4<f32>,
    visual0: vec4<f32>,
    visual1: vec4<f32>,
    visual2: vec4<f32>,
    wave_profile: vec4<f32>,
    coastline_foam_color: vec4<f32>,
    coastline: vec4<f32>,
    shape: vec4<f32>,
    sim: vec4<u32>,
    model_x: vec4<f32>,
    model_y: vec4<f32>,
    model_z: vec4<f32>,
    model_w: vec4<f32>,
}

struct Params {
    water_count: u32,
    water_2d_count: u32,
    cell_count: u32,
    _pad: u32,
    time_seconds: f32,
    delta_seconds: f32,
    _pad1: vec2<f32>,
}

struct Camera2D {
    view: mat4x4<f32>,
    ndc_scale: vec2<f32>,
    pad: vec2<f32>,
}

@group(0) @binding(0)
var<storage, read> waters: array<Water>;
@group(0) @binding(1)
var<storage, read_write> cells: array<vec4<f32>>;
@group(0) @binding(2)
var<uniform> params: Params;
@group(0) @binding(3)
var<storage, read> coastline_cells: array<vec4<f32>>;
@group(1) @binding(0)
var<uniform> camera: Camera2D;

fn water_shape_alpha(w: Water, uv: vec2<f32>) -> f32 {
    if w.shape.x < 0.5 {
        return 1.0;
    }
    let local = (uv - vec2<f32>(0.5, 0.5)) * w.size_depth_time.xy;
    let r = w.shape.y;
    if dot(local, local) <= r * r {
        return 1.0;
    }
    return 0.0;
}

fn water_crest_wave(v: f32) -> f32 {
    return pow(max(v, 0.0), 3.0) - pow(max(-v, 0.0), 1.35) * 0.30;
}

fn water_idle_height(w: Water, local: vec2<f32>, t: f32) -> f32 {
    let phase = t * w.wave.x * 0.2;
    let wave_coord = local / max(w.wave_profile.x, 0.001);
    let tau = 6.2831853;
    if w.idle_mode == 0u {
        return 0.0;
    }
    if w.idle_mode == 1u {
        let wind = normalize(select(vec2<f32>(1.0, 0.0), w.flow_wind.zw, length(w.flow_wind.zw) > 0.0001));
        let cross = vec2<f32>(-wind.y, wind.x);
        let a = sin(dot(wave_coord, wind) * tau + phase);
        let b = sin(dot(wave_coord, cross) * tau * 1.73 - phase * 0.61);
        let c = sin((wave_coord.x * 0.37 + wave_coord.y * 0.91) * tau * 2.37 + phase * 1.41);
        return (water_crest_wave(a) * 0.52 + b * 0.24 + water_crest_wave(c) * 0.24) * w.wave.y;
    }
    if w.idle_mode == 2u {
        let wind = normalize(select(vec2<f32>(1.0, 0.0), w.flow_wind.zw, length(w.flow_wind.zw) > 0.0001));
        let cross = vec2<f32>(-wind.y, wind.x);
        let a = sin(dot(wave_coord, wind) * tau * 0.72 + phase * 0.84);
        let b = cos(dot(wave_coord, cross) * tau * 1.21 - phase * 1.17);
        let c = sin((wave_coord.x * 0.74 + wave_coord.y * 1.36) * tau * 1.83 + phase * 1.46);
        let d = cos((wave_coord.x * -1.19 + wave_coord.y * 0.48) * tau * 2.79 - phase * 2.08);
        let crest_a = pow(max(a, 0.0), 3.0) - pow(max(-a, 0.0), 1.4) * 0.42;
        let crest_b = pow(max(c, 0.0), 4.0) - pow(max(-c, 0.0), 1.3) * 0.28;
        return (crest_a * 0.42 + b * 0.20 + crest_b * 0.25 + d * 0.13) * w.wave.y * 1.45;
    }
    if w.idle_mode == 3u {
        let wind = normalize(select(vec2<f32>(1.0, 0.0), w.flow_wind.zw, length(w.flow_wind.zw) > 0.0001));
        let cross = vec2<f32>(-wind.y, wind.x);
        let a = sin(dot(wave_coord, wind) * tau * 0.58 + phase * 0.77);
        let b = cos(dot(wave_coord, cross) * tau * 1.02 - phase * 0.91);
        let c = sin((wave_coord.x * 1.43 + wave_coord.y * 0.61) * tau * 1.74 + phase * 1.37);
        let d = cos((wave_coord.x * -0.51 + wave_coord.y * 1.18) * tau * 2.52 - phase * 1.91);
        let swell_a = pow(max(0.0, sin(dot(wave_coord, wind) * tau * 0.39 + phase * 0.63)), 5.0);
        let swell_b = pow(max(0.0, sin(dot(wave_coord, cross) * tau * 0.64 - phase * 1.09 + 1.7)), 4.0);
        let chop = (pow(max(a, 0.0), 3.0) * 0.30 - pow(max(-a, 0.0), 1.35) * 0.16)
            + (b * 0.12 + c * 0.14 + d * 0.10);
        return (chop + swell_a * 0.82 + swell_b * 0.56) * w.wave.y * 1.65;
    }
    let flow = normalize(select(vec2<f32>(1.0, 0.0), w.flow_wind.xy, length(w.flow_wind.xy) > 0.0001));
    let cross = vec2<f32>(-flow.y, flow.x);
    let a = sin(dot(wave_coord, flow) * tau * 1.6 - phase * 1.5);
    let b = sin(dot(wave_coord, cross) * tau * 2.4 + phase * 0.55);
    return (a * 0.76 + b * 0.24) * w.wave.y * 0.45;
}

fn water_coast_diffuse(w: Water, local_idx: u32, width: u32) -> f32 {
    let height = max(w.sim.w, 1u);
    let x = local_idx % width;
    let y = local_idx / width;
    let xl = x - select(0u, 1u, x > 0u);
    let xr = min(x + 1u, width - 1u);
    let yd = y - select(0u, 1u, y > 0u);
    let yu = min(y + 1u, height - 1u);
    let left = coastline_cells[w.sim.x + min(y * width + xl, w.sim.y - 1u)].y;
    let right = coastline_cells[w.sim.x + min(y * width + xr, w.sim.y - 1u)].y;
    let down = coastline_cells[w.sim.x + min(yd * width + x, w.sim.y - 1u)].y;
    let up = coastline_cells[w.sim.x + min(yu * width + x, w.sim.y - 1u)].y;
    return (left + right + down + up) * 0.25;
}

fn water_coast_normal(w: Water, local_idx: u32, width: u32) -> vec2<f32> {
    let height = max(w.sim.w, 1u);
    let x = local_idx % width;
    let y = local_idx / width;
    let xl = x - select(0u, 1u, x > 0u);
    let xr = min(x + 1u, width - 1u);
    let yd = y - select(0u, 1u, y > 0u);
    let yu = min(y + 1u, height - 1u);
    let left = coastline_cells[w.sim.x + min(y * width + xl, w.sim.y - 1u)].x;
    let right = coastline_cells[w.sim.x + min(y * width + xr, w.sim.y - 1u)].x;
    let down = coastline_cells[w.sim.x + min(yd * width + x, w.sim.y - 1u)].x;
    let up = coastline_cells[w.sim.x + min(yu * width + x, w.sim.y - 1u)].x;
    let grad = vec2<f32>(right - left, up - down);
    let len = length(grad);
    if len <= 0.0001 {
        return vec2<f32>(0.0, 0.0);
    }
    return grad / len;
}

@compute @workgroup_size(64)
fn cs_main(@builtin(global_invocation_id) gid: vec3<u32>) {
    let water_idx = gid.y;
    if water_idx >= params.water_count {
        return;
    }
    let w = waters[water_idx];
    let local_idx = gid.x;
    if w.sim.y == 0u || local_idx >= w.sim.y {
        return;
    }
    if (w.flags.z & 2u) != 0u {
        return;
    }
    let cell_idx = w.sim.x + local_idx;
    let width = max(w.sim.z, 1u);
    let x_cell = local_idx % width;
    let y_cell = local_idx / width;
    let fx = f32(x_cell) / max(f32(width - 1u), 1.0);
    let fy = f32(y_cell) / max(f32(max(w.sim.w, 1u) - 1u), 1.0);
    if water_shape_alpha(w, vec2<f32>(fx, fy)) <= 0.0 {
        cells[cell_idx] = vec4<f32>(0.0);
        return;
    }
    let t = params.time_seconds;
    let phase = t * w.wave.x * 0.2;
    let local = (vec2<f32>(fx, fy) - vec2<f32>(0.5, 0.5)) * w.size_depth_time.xy;
    let idle = water_idle_height(w, local, t);
    let coast = coastline_cells[cell_idx];
    if coast.x > 0.985 {
        cells[cell_idx] = vec4<f32>(0.0, 0.0, 1.0, 1.0);
        return;
    }
    let edge = max(0.0, 1.0 - min(min(fx, 1.0 - fx), min(fy, 1.0 - fy)) * max(w.coastline.y, 0.001) * 8.0);
    let neighbor_shore = water_coast_diffuse(w, local_idx, width);
    let coast_normal = water_coast_normal(w, local_idx, width);
    let shore = max(edge, max(coast.y, neighbor_shore * 0.64)) * (1.0 - coast.x * 0.30);
    let wake = coast.z * w.wave.w * 1.45;
    if shore <= 0.0 && wake <= 0.0 && coast.w <= 0.0 && abs(idle) <= 0.00001 {
        cells[cell_idx] = vec4<f32>(0.0);
        return;
    }
    let edge_noise = (sin((local.x * 0.31 + local.y * 0.47) + phase * 7.0) + sin((local.x * -0.53 + local.y * 0.29) - phase * 4.3)) * 0.34 * w.model_z.w;
    let spill = clamp(coast.w, 0.0, 1.0);
    let diffusion = max(neighbor_shore - coast.y, 0.0) * 0.45 + spill * 0.20;
    let wave_dir = normalize(select(vec2<f32>(1.0, 0.0), w.flow_wind.zw + w.flow_wind.xy * 0.35, length(w.flow_wind.zw + w.flow_wind.xy * 0.35) > 0.0001));
    let coast_push = max(dot(-wave_dir, coast_normal), 0.0);
    let coast_slide = abs(dot(vec2<f32>(-wave_dir.y, wave_dir.x), coast_normal));
    let crash_wave = max(0.0, water_crest_wave(sin((local.x * 0.19 - local.y * 0.23) + phase * 5.5 + edge_noise)));
    let reflected = shore * (0.35 + coast_push * 0.95 + coast_slide * 0.18)
        * water_crest_wave(sin((local.x * -0.27 + local.y * 0.18) - phase * 4.1 - edge_noise))
        * w.model_y.w
        * w.wave.y
        * w.model_z.w;
    let crash_up = shore * (1.0 + coast_push * 0.85) * pow(crash_wave, 2.4) * w.model_y.w * w.wave.y * 2.10;
    let crash_down = -shore * pow(max(-crash_wave, 0.0), 1.2) * w.wave.y * 0.34;
    let crash = (crash_up + crash_down + max(reflected, 0.0) * 0.54) * (0.60 + spill * 0.52) + diffusion * w.wave.y * 0.74;
    let prev = cells[cell_idx].x
        * w.wave.z
        * (1.0 - shore * (0.52 + coast_push * 0.26) * w.coastline.w)
        * (0.64 + spill * 0.24 + coast_slide * 0.06);
    let crest_norm = idle / max(w.wave.y, 0.001);
    let crest_line = smoothstep(0.50, 0.86, crest_norm) * (1.0 - smoothstep(1.10, 1.80, crest_norm));
    let wave_foam = crest_line * bitcast<f32>(w.flags.w) * 0.34;
    let impact_foam = smoothstep(0.08, 0.96, wake + abs(crash)) * bitcast<f32>(w.flags.w) * 0.46;
    let shore_foam = smoothstep(0.24, 1.42, crash + shore * 0.50) * (1.0 - smoothstep(1.55, 2.65, crash)) * w.coastline.x * bitcast<f32>(w.flags.w);
    let foam = clamp(wave_foam + impact_foam + shore_foam + spill * max(wake, shore) * 0.22, 0.0, 1.0);
    let height = mix(prev + idle * (0.030 + shore * w.model_y.w * 0.18) + wake * 0.30 + crash, idle + wake * 0.28 + crash, 0.44 + spill * 0.14);
    cells[cell_idx] = vec4<f32>(height, idle, foam, shore);
}

struct Water2DVertexOut {
    @builtin(position) clip_pos: vec4<f32>,
    @location(0) uv: vec2<f32>,
    @location(1) water_idx: u32,
}

fn quad_pos(vertex_idx: u32) -> vec2<f32> {
    var p = array<vec2<f32>, 6>(
        vec2<f32>(-0.5, -0.5),
        vec2<f32>( 0.5, -0.5),
        vec2<f32>( 0.5,  0.5),
        vec2<f32>(-0.5, -0.5),
        vec2<f32>( 0.5,  0.5),
        vec2<f32>(-0.5,  0.5),
    );
    return p[vertex_idx];
}

@vertex
fn vs_water_2d(
    @builtin(vertex_index) vertex_idx: u32,
    @builtin(instance_index) water_idx: u32,
) -> Water2DVertexOut {
    let w = waters[water_idx];
    let local = quad_pos(vertex_idx);
    let scaled = local * w.size_depth_time.xy;
    let model = mat3x3<f32>(w.model_x.xyz, w.model_y.xyz, w.model_z.xyz);
    let world_xy = (model * vec3<f32>(scaled, 1.0)).xy;
    let view = camera.view * vec4<f32>(world_xy, 0.0, 1.0);
    let depth = 1.0 - f32(w.z_index) * 0.001;

    var out: Water2DVertexOut;
    out.clip_pos = vec4<f32>(view.xy * camera.ndc_scale, depth, 1.0);
    out.uv = local + vec2<f32>(0.5, 0.5);
    out.water_idx = water_idx;
    return out;
}

@fragment
fn fs_water_2d(in: Water2DVertexOut) -> @location(0) vec4<f32> {
    let w = waters[in.water_idx];
    if water_shape_alpha(w, in.uv) <= 0.0 {
        return vec4<f32>(0.0);
    }
    let t = params.time_seconds;
    let idle = sin((in.uv.x + in.uv.y + t * w.wave.x * 0.2) * 6.2831853) * 0.5 + 0.5;
    var ripple = vec4<f32>(0.0);
    if w.sim.y > 0u {
        let width = max(w.sim.z, 1u);
        let height = max(w.sim.w, 1u);
        let x = u32(clamp(in.uv.x, 0.0, 1.0) * f32(max(width - 1u, 1u)));
        let y = u32(clamp(in.uv.y, 0.0, 1.0) * f32(max(height - 1u, 1u)));
        let local_idx = min(y * width + x, w.sim.y - 1u);
        let cell_idx = w.sim.x + local_idx;
        ripple = cells[cell_idx] * w.model_x.w;
        if coastline_cells[cell_idx].x > 0.985 {
            return vec4<f32>(0.0, 0.0, 0.0, 0.0);
        }
    }
    let edge = max(0.0, 1.0 - min(min(in.uv.x, 1.0 - in.uv.x), min(in.uv.y, 1.0 - in.uv.y)) * max(w.coastline.y, 0.001) * 8.0);
    let crest = smoothstep(1.05, 2.7, abs(ripple.x)) * bitcast<f32>(w.flags.w) * 0.22;
    let foam = clamp(ripple.z * 0.86 + max(edge, ripple.w) * w.coastline.x * 0.16 + crest, 0.0, 1.0);
    let auto_shallow_depth = max(max(w.size_depth_time.x, w.size_depth_time.y) * 0.25, 0.001);
    let shallow_depth = select(auto_shallow_depth, max(w.size_depth_time.w, 0.001), w.size_depth_time.w >= 0.0);
    let depth_t = clamp(w.size_depth_time.z / shallow_depth, 0.0, 1.0);
    let shallow_t = clamp(1.0 - depth_t + idle * 0.10 + foam * 0.12, 0.0, 1.0);
    let surface_t = clamp(shallow_t + abs(ripple.x) * 0.16 + foam * 0.10, 0.0, 1.0);
    let water_rgb = mix(w.deep_color.rgb, w.shallow_color.rgb, surface_t);
    let sky_rgb = mix(water_rgb, w.sky_color_bias.rgb, w.sky_color_bias.w);
    let color = mix(sky_rgb, w.coastline_foam_color.rgb, foam * 0.42);
    let alpha = mix(w.deep_color.a, w.shallow_color.a, shallow_t);
    return vec4<f32>(color, clamp(alpha + foam * 0.12, 0.0, 1.0));
}
"#;

const WATER_3D_RENDER_WGSL: &str = r#"
struct Water {
    node: u32,
    kind: u32,
    idle_mode: u32,
    z_index: i32,
    size_depth_time: vec4<f32>,
    flow_wind: vec4<f32>,
    wave: vec4<f32>,
    flags: vec4<u32>,
    deep_color: vec4<f32>,
    shallow_color: vec4<f32>,
    sky_color_bias: vec4<f32>,
    foam_color: vec4<f32>,
    visual0: vec4<f32>,
    visual1: vec4<f32>,
    visual2: vec4<f32>,
    wave_profile: vec4<f32>,
    coastline_foam_color: vec4<f32>,
    coastline: vec4<f32>,
    shape: vec4<f32>,
    sim: vec4<u32>,
    model_x: vec4<f32>,
    model_y: vec4<f32>,
    model_z: vec4<f32>,
    model_w: vec4<f32>,
}

struct Params {
    water_count: u32,
    water_2d_count: u32,
    cell_count: u32,
    _pad: u32,
    time_seconds: f32,
    delta_seconds: f32,
    _pad1: vec2<f32>,
}

struct RayLightGpu {
    direction: vec4<f32>,
    color_intensity: vec4<f32>,
}

struct PointLightGpu {
    position_range: vec4<f32>,
    color_intensity: vec4<f32>,
}

struct SpotLightGpu {
    position_range: vec4<f32>,
    direction_outer_cos: vec4<f32>,
    color_intensity: vec4<f32>,
    inner_cos_pad: vec4<f32>,
}

struct Scene3D {
    view_proj: mat4x4<f32>,
    ambient_and_counts: vec4<f32>,
    camera_pos: vec4<f32>,
    ambient_color: vec4<f32>,
    ray_light: RayLightGpu,
    ray_lights: array<RayLightGpu, 3>,
    point_lights: array<PointLightGpu, 8>,
    spot_lights: array<SpotLightGpu, 8>,
    inv_view_proj: mat4x4<f32>,
}

@group(0) @binding(0)
var<storage, read> waters: array<Water>;
@group(0) @binding(1)
var<storage, read> cells: array<vec4<f32>>;
@group(0) @binding(2)
var<uniform> params: Params;
@group(0) @binding(3)
var<storage, read> coastline_cells: array<vec4<f32>>;
struct WaterRenderChunk {
    water_idx: u32,
    render_width: u32,
    render_height: u32,
    flags: u32,
    uv_origin: vec2<f32>,
    uv_scale: vec2<f32>,
}
@group(0) @binding(4)
var<storage, read> render_chunks: array<WaterRenderChunk>;
@group(1) @binding(0)
var<uniform> scene: Scene3D;
@group(2) @binding(0)
var scene_depth_tex: texture_depth_2d;

struct Water3DVertexOut {
    @builtin(position) clip_pos: vec4<f32>,
    @location(0) uv: vec2<f32>,
    @location(1) water_idx: u32,
    @location(2) world_pos: vec3<f32>,
    @location(3) side_t: f32,
}

fn water_shape_alpha(w: Water, uv: vec2<f32>) -> f32 {
    let local = (uv - vec2<f32>(0.5, 0.5)) * w.size_depth_time.xy;
    if w.shape.x < 0.5 {
        return 1.0;
    }
    let r = w.shape.y;
    if dot(local, local) <= r * r {
        return 1.0;
    }
    return 0.0;
}

fn water_cell(w: Water, uv: vec2<f32>) -> vec4<f32> {
    if w.sim.y == 0u {
        return vec4<f32>(0.0);
    }
    let width = max(w.sim.z, 1u);
    let height = max(w.sim.w, 1u);
    let p = clamp(uv, vec2<f32>(0.0), vec2<f32>(1.0)) * vec2<f32>(f32(max(width - 1u, 1u)), f32(max(height - 1u, 1u)));
    let x0 = u32(floor(p.x));
    let y0 = u32(floor(p.y));
    let x1 = min(x0 + 1u, width - 1u);
    let y1 = min(y0 + 1u, height - 1u);
    let tx = fract(p.x);
    let ty = fract(p.y);
    let i00 = min(y0 * width + x0, w.sim.y - 1u);
    let i10 = min(y0 * width + x1, w.sim.y - 1u);
    let i01 = min(y1 * width + x0, w.sim.y - 1u);
    let i11 = min(y1 * width + x1, w.sim.y - 1u);
    let a = mix(cells[w.sim.x + i00], cells[w.sim.x + i10], tx);
    let b = mix(cells[w.sim.x + i01], cells[w.sim.x + i11], tx);
    return mix(a, b, ty) * w.model_x.w;
}

fn water_coast_solid(w: Water, uv: vec2<f32>) -> bool {
    if w.sim.y == 0u {
        return false;
    }
    let width = max(w.sim.z, 1u);
    let height = max(w.sim.w, 1u);
    let p = clamp(uv, vec2<f32>(0.0), vec2<f32>(1.0)) * vec2<f32>(f32(max(width - 1u, 1u)), f32(max(height - 1u, 1u)));
    let x0 = u32(floor(p.x));
    let y0 = u32(floor(p.y));
    let x1 = min(x0 + 1u, width - 1u);
    let y1 = min(y0 + 1u, height - 1u);
    let tx = fract(p.x);
    let ty = fract(p.y);
    let i00 = min(y0 * width + x0, w.sim.y - 1u);
    let i10 = min(y0 * width + x1, w.sim.y - 1u);
    let i01 = min(y1 * width + x0, w.sim.y - 1u);
    let i11 = min(y1 * width + x1, w.sim.y - 1u);
    let a = mix(coastline_cells[w.sim.x + i00].x, coastline_cells[w.sim.x + i10].x, tx);
    let b = mix(coastline_cells[w.sim.x + i01].x, coastline_cells[w.sim.x + i11].x, tx);
    return mix(a, b, ty) > 0.985;
}

fn water_coast_sample(w: Water, uv: vec2<f32>) -> vec4<f32> {
    if w.sim.y == 0u {
        return vec4<f32>(0.0);
    }
    let width = max(w.sim.z, 1u);
    let height = max(w.sim.w, 1u);
    let p = clamp(uv, vec2<f32>(0.0), vec2<f32>(1.0)) * vec2<f32>(f32(max(width - 1u, 1u)), f32(max(height - 1u, 1u)));
    let x0 = u32(floor(p.x));
    let y0 = u32(floor(p.y));
    let x1 = min(x0 + 1u, width - 1u);
    let y1 = min(y0 + 1u, height - 1u);
    let tx = fract(p.x);
    let ty = fract(p.y);
    let i00 = min(y0 * width + x0, w.sim.y - 1u);
    let i10 = min(y0 * width + x1, w.sim.y - 1u);
    let i01 = min(y1 * width + x0, w.sim.y - 1u);
    let i11 = min(y1 * width + x1, w.sim.y - 1u);
    let a = mix(coastline_cells[w.sim.x + i00], coastline_cells[w.sim.x + i10], tx);
    let b = mix(coastline_cells[w.sim.x + i01], coastline_cells[w.sim.x + i11], tx);
    return mix(a, b, ty);
}

fn water_surface_height(w: Water, uv: vec2<f32>) -> f32 {
    let ripple = water_cell(w, uv);
    return ripple.x + ripple.y * 0.72;
}

fn water_ridge_wave(v: f32) -> f32 {
    let s = sin(v);
    return pow(max(s, 0.0), 8.0) - pow(max(-s, 0.0), 0.78) * 0.18;
}

fn water_visual_wave_height(w: Water, uv: vec2<f32>, t: f32) -> f32 {
    let local = (uv - vec2<f32>(0.5, 0.5)) * w.size_depth_time.xy;
    let wind = normalize(select(vec2<f32>(1.0, 0.0), w.flow_wind.zw, length(w.flow_wind.zw) > 0.0001));
    let cross = vec2<f32>(-wind.y, wind.x);
    let ripple_scale = max(w.visual1.y, 0.001);
    let p = local / max(ripple_scale, 0.001);
    let break_n = water_fbm(local * 0.16 + vec2<f32>(t * 0.08, -t * 0.05));
    let shard = water_fbm(local * 0.075 + vec2<f32>(3.7, 9.2)) * 6.2831853;
    let break_dir = normalize(vec2<f32>(
        cos(shard + water_fbm(local * 0.11 + vec2<f32>(11.0, 2.0)) * 1.2),
        sin(shard + water_fbm(local * 0.09 + vec2<f32>(5.0, 19.0)) * 1.2),
    ));
    let a = water_ridge_wave(dot(p, wind) * 0.42 + t * w.wave.x * 1.9 + break_n * 1.6);
    let b = water_ridge_wave(dot(p, cross) * 0.86 - t * w.wave.x * 2.6 + 1.4 + dot(local, break_dir) * 0.15);
    let c = water_ridge_wave((p.x * 0.58 + p.y * 0.35) + t * w.wave.x * 3.6 + shard * 0.44);
    let d = water_ridge_wave(dot(p, break_dir) * 1.42 - t * w.wave.x * 3.2 + break_n * 2.8);
    let diag_a = water_ridge_wave(dot(p, normalize(vec2<f32>(0.72, 0.69))) * 1.05 + t * w.wave.x * 2.9 + break_n);
    let diag_b = water_ridge_wave(dot(p, normalize(vec2<f32>(-0.64, 0.77))) * 1.24 - t * w.wave.x * 2.4 + shard * 0.27);
    let cut_n = water_ridged_fbm(local * 0.28 + vec2<f32>(t * 0.04, -t * 0.03));
    let fracture = (break_n - 0.5) * 0.14 + (cut_n - 0.5) * 0.10;
    let ridge = a * 0.32 + b * 0.20 + c * 0.14 + d * 0.15 + diag_a * 0.12 + diag_b * 0.11 + fracture;
    let snap = sign(ridge) * pow(abs(ridge), 1.46);
    return snap * w.wave.y * 0.24 * clamp(w.visual1.x, 0.0, 1.4);
}

fn water_height_visual(w: Water, uv: vec2<f32>, t: f32) -> f32 {
    return water_surface_height(w, uv) + water_visual_wave_height(w, uv, t);
}

fn water_height_geometry(w: Water, uv: vec2<f32>, t: f32) -> f32 {
    let du = vec2<f32>(1.0 / max(f32(w.flags.x), 2.0), 0.0);
    let dv = vec2<f32>(0.0, 1.0 / max(f32(w.flags.y), 2.0));
    let center = water_surface_height(w, uv);
    let neighbor_avg = (
        water_surface_height(w, uv - du)
        + water_surface_height(w, uv + du)
        + water_surface_height(w, uv - dv)
        + water_surface_height(w, uv + dv)
    ) * 0.25;
    let smoothed = mix(center, neighbor_avg, 0.42);
    return smoothed + water_visual_wave_height(w, uv, t) * 0.24;
}

fn water_visual_normal(w: Water, uv: vec2<f32>, t: f32) -> vec3<f32> {
    let du = vec2<f32>(1.0 / max(f32(w.flags.x), 2.0), 0.0);
    let dv = vec2<f32>(0.0, 1.0 / max(f32(w.flags.y), 2.0));
    let h_l = water_height_visual(w, uv - du, t);
    let h_r = water_height_visual(w, uv + du, t);
    let h_d = water_height_visual(w, uv - dv, t);
    let h_u = water_height_visual(w, uv + dv, t);
    let sx = max(w.size_depth_time.x * du.x * 2.0, 0.001);
    let sz = max(w.size_depth_time.y * dv.y * 2.0, 0.001);
    let normal_scale = clamp(w.visual1.x, 0.0, 1.35) * 0.62;
    return normalize(vec3<f32>((h_l - h_r) * normal_scale, (sx + sz) * 1.18, (h_d - h_u) * normal_scale));
}

fn water_visual_normal_fast(w: Water, uv: vec2<f32>) -> vec3<f32> {
    let du = vec2<f32>(1.0 / max(f32(w.flags.x), 2.0), 0.0);
    let dv = vec2<f32>(0.0, 1.0 / max(f32(w.flags.y), 2.0));
    let h_l = water_surface_height(w, uv - du);
    let h_r = water_surface_height(w, uv + du);
    let h_d = water_surface_height(w, uv - dv);
    let h_u = water_surface_height(w, uv + dv);
    let sx = max(w.size_depth_time.x * du.x * 2.0, 0.001);
    let sz = max(w.size_depth_time.y * dv.y * 2.0, 0.001);
    let normal_scale = clamp(w.visual1.x, 0.0, 1.1) * 0.42;
    return normalize(vec3<f32>((h_l - h_r) * normal_scale, (sx + sz) * 1.12, (h_d - h_u) * normal_scale));
}

fn water_hash(p: vec2<f32>) -> f32 {
    return fract(sin(dot(p, vec2<f32>(127.1, 311.7))) * 43758.5453);
}

fn water_hash2(p: vec2<f32>) -> vec2<f32> {
    let x = fract(sin(dot(p, vec2<f32>(127.1, 311.7))) * 43758.5453);
    let y = fract(sin(dot(p, vec2<f32>(269.5, 183.3))) * 43758.5453);
    return vec2<f32>(x, y);
}

fn water_grad2(p: vec2<f32>) -> vec2<f32> {
    let h = water_hash2(p) * 2.0 - 1.0;
    let len2 = max(dot(h, h), 1.0e-4);
    return h * inverseSqrt(len2);
}

fn water_noise(p: vec2<f32>) -> f32 {
    let i = floor(p);
    let f = fract(p);
    let u = f * f * f * (f * (f * 6.0 - 15.0) + 10.0);
    let a = water_hash(i);
    let b = water_hash(i + vec2<f32>(1.0, 0.0));
    let c = water_hash(i + vec2<f32>(0.0, 1.0));
    let d = water_hash(i + vec2<f32>(1.0, 1.0));
    return mix(mix(a, b, u.x), mix(c, d, u.x), u.y);
}

fn water_perlin_noise(p: vec2<f32>) -> f32 {
    let i = floor(p);
    let f = fract(p);
    let u = f * f * f * (f * (f * 6.0 - 15.0) + 10.0);
    let g00 = water_grad2(i);
    let g10 = water_grad2(i + vec2<f32>(1.0, 0.0));
    let g01 = water_grad2(i + vec2<f32>(0.0, 1.0));
    let g11 = water_grad2(i + vec2<f32>(1.0, 1.0));
    let a = dot(g00, f - vec2<f32>(0.0, 0.0));
    let b = dot(g10, f - vec2<f32>(1.0, 0.0));
    let c = dot(g01, f - vec2<f32>(0.0, 1.0));
    let d = dot(g11, f - vec2<f32>(1.0, 1.0));
    return mix(mix(a, b, u.x), mix(c, d, u.x), u.y) * 0.5 + 0.5;
}

fn water_fbm(p: vec2<f32>) -> f32 {
    var q = p;
    var amp = 0.52;
    var sum = 0.0;
    var norm = 0.0;
    for (var i = 0u; i < 4u; i = i + 1u) {
        sum += water_noise(q) * amp;
        norm += amp;
        q = vec2<f32>(q.x * 1.72 + q.y * 1.11, q.x * -1.04 + q.y * 1.83) + vec2<f32>(17.0, 9.0);
        amp *= 0.52;
    }
    return sum / max(norm, 0.001);
}

fn water_perlin_fbm(p: vec2<f32>) -> f32 {
    var q = p;
    var amp = 0.54;
    var sum = 0.0;
    var norm = 0.0;
    for (var i = 0u; i < 5u; i = i + 1u) {
        sum += water_perlin_noise(q) * amp;
        norm += amp;
        q = vec2<f32>(q.x * 1.84 - q.y * 0.74, q.x * 0.92 + q.y * 1.67) + vec2<f32>(7.0, 19.0);
        amp *= 0.53;
    }
    return sum / max(norm, 0.001);
}

fn water_ridged_fbm(p: vec2<f32>) -> f32 {
    var q = p;
    var amp = 0.56;
    var sum = 0.0;
    var norm = 0.0;
    for (var i = 0u; i < 4u; i = i + 1u) {
        let n = abs(water_noise(q) * 2.0 - 1.0);
        sum += (1.0 - n) * amp;
        norm += amp;
        q = vec2<f32>(q.x * 1.91 - q.y * 0.73, q.x * 0.86 + q.y * 1.68) + vec2<f32>(4.0, 23.0);
        amp *= 0.50;
    }
    return sum / max(norm, 0.001);
}

fn water_hex_noise(p: vec2<f32>) -> f32 {
    let rot_a = vec2<f32>(p.x * 0.5 - p.y * 0.8660254, p.x * 0.8660254 + p.y * 0.5);
    let rot_b = vec2<f32>(p.x * 0.5 + p.y * 0.8660254, -p.x * 0.8660254 + p.y * 0.5);
    let a = water_noise(p);
    let b = water_noise(rot_a + vec2<f32>(11.0, 7.0));
    let c = water_noise(rot_b + vec2<f32>(23.0, 17.0));
    let cell = water_noise(p * 0.19 + vec2<f32>(3.0, 5.0));
    return mix((a + b + c) * 0.33333334, max(a, max(b, c)), 0.22 + cell * 0.18);
}

fn water_hex_ridged_fbm(p: vec2<f32>) -> f32 {
    var q = p;
    var amp = 0.58;
    var sum = 0.0;
    var norm = 0.0;
    for (var i = 0u; i < 4u; i = i + 1u) {
        let n = abs(water_hex_noise(q) * 2.0 - 1.0);
        sum += (1.0 - n) * amp;
        norm += amp;
        q = vec2<f32>(q.x * 1.73 - q.y * 0.94, q.x * 0.88 + q.y * 1.61) + vec2<f32>(13.0, 29.0);
        amp *= 0.52;
    }
    return sum / max(norm, 0.001);
}

fn water_line_layer(p: vec2<f32>, dir: vec2<f32>, t: f32, scale: f32) -> vec3<f32> {
    let n = water_hex_noise(p * 0.21 + dir * t * 0.08);
    let q = dot(p, dir) * scale + n * 1.35 + t * 0.18;
    let band = abs(fract(q) - 0.5) * 2.0;
    let line = 1.0 - smoothstep(0.035, 0.13, band);
    let broken =
        smoothstep(0.30, 0.74, water_hex_noise(p * 0.53 + vec2<f32>(t * 0.05, -t * 0.04)));
    let dark = smoothstep(0.62, 0.95, band)
        * smoothstep(0.42, 0.88, water_hex_noise(p * 0.12 - dir * t * 0.03));
    return vec3<f32>(line * broken, dark, n);
}

fn water_schlick_fresnel(cos_theta: f32, power: f32) -> f32 {
    let f0 = 0.020;
    let grazing = 1.0 - clamp(cos_theta, 0.0, 1.0);
    let edge = pow(grazing, max(power, 0.001));
    let shoulder = smoothstep(0.18, 0.88, grazing);
    return f0 + (1.0 - f0) * (edge * 0.68 + shoulder * 0.18);
}

fn water_scene_world_from_depth(coord: vec2<i32>, dims_u: vec2<u32>, depth: f32) -> vec3<f32> {
    let uv = (vec2<f32>(coord) + vec2<f32>(0.5)) / vec2<f32>(dims_u);
    let ndc_xy = vec2<f32>(uv.x * 2.0 - 1.0, 1.0 - uv.y * 2.0);
    let ndc = vec4<f32>(ndc_xy, depth * 2.0 - 1.0, 1.0);
    let world_h = scene.inv_view_proj * ndc;
    return world_h.xyz / max(abs(world_h.w), 1.0e-5);
}

fn water_screen_contact_outline(in: Water3DVertexOut) -> vec2<f32> {
    let dims_u = textureDimensions(scene_depth_tex);
    let dims = vec2<i32>(i32(dims_u.x), i32(dims_u.y));
    let coord = vec2<i32>(floor(in.clip_pos.xy));
    if any(coord < vec2<i32>(0)) || any(coord >= dims) {
        return vec2<f32>(0.0, 1.0e9);
    }
    let water_view_dist = distance(in.world_pos, scene.camera_pos.xyz);
    var nearest_delta = 1.0e9;
    var edge = 0.0;
    for (var oy = -2; oy <= 2; oy = oy + 1) {
        for (var ox = -2; ox <= 2; ox = ox + 1) {
            let sample_coord = clamp(coord + vec2<i32>(ox, oy), vec2<i32>(0), dims - vec2<i32>(1));
            let scene_depth = textureLoad(scene_depth_tex, sample_coord, 0);
            if scene_depth >= 0.999999 {
                continue;
            }
            let scene_world = water_scene_world_from_depth(sample_coord, dims_u, scene_depth);
            let delta = water_view_dist - distance(scene_world, scene.camera_pos.xyz);
            if delta <= 0.0 {
                continue;
            }
            nearest_delta = min(nearest_delta, delta);
            let pixel_dist = length(vec2<f32>(f32(ox), f32(oy)));
            let radius_fade = 1.0 - smoothstep(0.0, 2.8, pixel_dist);
            let gap_fade = 1.0 - smoothstep(0.01, 0.30, delta);
            edge = max(edge, gap_fade * (0.42 + radius_fade * 0.58));
        }
    }
    return vec2<f32>(edge, nearest_delta);
}

struct WaterVertexLocal {
    position: vec3<f32>,
    uv: vec2<f32>,
    side_t: f32,
    valid: bool,
}

fn water_surface_vertex(w: Water, vertex_idx: u32) -> WaterVertexLocal {
    let width = max(w.flags.x, 1u);
    let height = max(w.flags.y, 1u);
    let quad_width = width - 1u;
    let quad_height = height - 1u;
    let quad_count = quad_width * quad_height;
    let cell = vertex_idx / 6u;
    if w.sim.y == 0u || quad_count == 0u || cell >= quad_count {
        return WaterVertexLocal(vec3<f32>(0.0), vec2<f32>(0.0), 0.0, false);
    }
    var corner = array<vec2<u32>, 6>(
        vec2<u32>(0u, 0u),
        vec2<u32>(1u, 1u),
        vec2<u32>(1u, 0u),
        vec2<u32>(0u, 0u),
        vec2<u32>(0u, 1u),
        vec2<u32>(1u, 1u),
    );
    let cx = cell % quad_width;
    let cy = cell / quad_width;
    let c = corner[vertex_idx % 6u];
    let uv = vec2<f32>(f32(cx + c.x) / f32(quad_width), f32(cy + c.y) / f32(quad_height));
    let pos = vec3<f32>(
        (uv.x - 0.5) * w.size_depth_time.x,
        water_height_geometry(w, uv, params.time_seconds),
        (uv.y - 0.5) * w.size_depth_time.y,
    );
    return WaterVertexLocal(pos, uv, 0.0, true);
}

fn water_chunk_surface_vertex(w: Water, chunk: WaterRenderChunk, vertex_idx: u32) -> WaterVertexLocal {
    let width = max(chunk.render_width, 2u);
    let height = max(chunk.render_height, 2u);
    let quad_width = width - 1u;
    let quad_height = height - 1u;
    let quad_count = quad_width * quad_height;
    let cell = vertex_idx / 6u;
    if w.sim.y == 0u || quad_count == 0u || cell >= quad_count {
        return WaterVertexLocal(vec3<f32>(0.0), vec2<f32>(0.0), 0.0, false);
    }
    var corner = array<vec2<u32>, 6>(
        vec2<u32>(0u, 0u),
        vec2<u32>(1u, 1u),
        vec2<u32>(1u, 0u),
        vec2<u32>(0u, 0u),
        vec2<u32>(0u, 1u),
        vec2<u32>(1u, 1u),
    );
    let cx = cell % quad_width;
    let cy = cell / quad_width;
    let c = corner[vertex_idx % 6u];
    let local_uv = vec2<f32>(f32(cx + c.x) / f32(quad_width), f32(cy + c.y) / f32(quad_height));
    let uv = chunk.uv_origin + local_uv * chunk.uv_scale;
    let pos = vec3<f32>(
        (uv.x - 0.5) * w.size_depth_time.x,
        water_height_geometry(w, uv, params.time_seconds),
        (uv.y - 0.5) * w.size_depth_time.y,
    );
    return WaterVertexLocal(pos, uv, 0.0, true);
}

fn water_rect_side_vertex(w: Water, side_idx: u32) -> WaterVertexLocal {
    let width = max(w.flags.x, 1u);
    let height = max(w.flags.y, 1u);
    let horizontal_segments = width - 1u;
    let vertical_segments = height - 1u;
    let side_count = horizontal_segments * 2u + vertical_segments * 2u;
    let cell = side_idx / 6u;
    if side_count == 0u || cell >= side_count {
        return WaterVertexLocal(vec3<f32>(0.0), vec2<f32>(0.0), 1.0, false);
    }
    var corner = array<vec2<f32>, 6>(
        vec2<f32>(0.0, 0.0),
        vec2<f32>(1.0, 0.0),
        vec2<f32>(1.0, 1.0),
        vec2<f32>(0.0, 0.0),
        vec2<f32>(1.0, 1.0),
        vec2<f32>(0.0, 1.0),
    );
    let c = corner[side_idx % 6u];
    let top_t = 1.0 - c.y;
    var uv = vec2<f32>(0.0, 0.0);

    if cell < horizontal_segments {
        let edge_t = (f32(cell) + c.x) / f32(horizontal_segments);
        uv = vec2<f32>(edge_t, 0.0);
    } else if cell < horizontal_segments + vertical_segments {
        let seg = cell - horizontal_segments;
        let edge_t = (f32(seg) + c.x) / f32(vertical_segments);
        uv = vec2<f32>(1.0, edge_t);
    } else if cell < horizontal_segments * 2u + vertical_segments {
        let seg = cell - horizontal_segments - vertical_segments;
        let edge_t = (f32(seg) + c.x) / f32(horizontal_segments);
        uv = vec2<f32>(1.0 - edge_t, 1.0);
    } else {
        let seg = cell - horizontal_segments * 2u - vertical_segments;
        let edge_t = (f32(seg) + c.x) / f32(vertical_segments);
        uv = vec2<f32>(0.0, 1.0 - edge_t);
    }
    let top = water_height_geometry(w, uv, params.time_seconds);
    let y = mix(-max(w.size_depth_time.z, 0.001), top, top_t);
    let pos = vec3<f32>(
        (uv.x - 0.5) * w.size_depth_time.x,
        y,
        (uv.y - 0.5) * w.size_depth_time.y,
    );
    return WaterVertexLocal(pos, uv, 1.0, true);
}

fn water_circle_counts(w: Water) -> vec2<u32> {
    let width = max(w.flags.x, 1u);
    let height = max(w.flags.y, 1u);
    let segments = clamp(max(width, height) * 4u, 16u, 512u);
    let rings = clamp(min(width, height) / 2u, 1u, 512u);
    return vec2<u32>(segments, rings);
}

fn water_circle_point(w: Water, angle_t: f32, radius_t: f32) -> WaterVertexLocal {
    let angle = angle_t * 6.2831853;
    let local_xz = vec2<f32>(cos(angle), sin(angle)) * w.shape.y * radius_t;
    let uv = local_xz / w.size_depth_time.xy + vec2<f32>(0.5, 0.5);
    let pos = vec3<f32>(local_xz.x, water_height_geometry(w, uv, params.time_seconds), local_xz.y);
    return WaterVertexLocal(pos, uv, 0.0, true);
}

fn water_circle_surface_vertex(w: Water, vertex_idx: u32) -> WaterVertexLocal {
    let counts = water_circle_counts(w);
    let segments = counts.x;
    let rings = counts.y;
    let cell = vertex_idx / 6u;
    if w.sim.y == 0u || cell >= segments * rings {
        return WaterVertexLocal(vec3<f32>(0.0), vec2<f32>(0.0), 0.0, false);
    }
    var corner = array<vec2<u32>, 6>(
        vec2<u32>(0u, 0u),
        vec2<u32>(1u, 1u),
        vec2<u32>(1u, 0u),
        vec2<u32>(0u, 0u),
        vec2<u32>(0u, 1u),
        vec2<u32>(1u, 1u),
    );
    let seg = cell % segments;
    let ring = cell / segments;
    let c = corner[vertex_idx % 6u];
    let angle_t = f32((seg + c.x) % segments) / f32(segments);
    let radius_t = f32(ring + c.y) / f32(rings);
    return water_circle_point(w, angle_t, radius_t);
}

fn water_circle_side_vertex(w: Water, side_idx: u32) -> WaterVertexLocal {
    let counts = water_circle_counts(w);
    let segments = counts.x;
    let side = side_idx / 6u;
    if side >= segments {
        return WaterVertexLocal(vec3<f32>(0.0), vec2<f32>(0.0), 1.0, false);
    }
    var corner = array<vec2<f32>, 6>(
        vec2<f32>(0.0, 0.0),
        vec2<f32>(1.0, 0.0),
        vec2<f32>(1.0, 1.0),
        vec2<f32>(0.0, 0.0),
        vec2<f32>(1.0, 1.0),
        vec2<f32>(0.0, 1.0),
    );
    let c = corner[side_idx % 6u];
    let angle_t = (f32(side) + c.x) / f32(segments);
    let top = water_circle_point(w, angle_t, 1.0);
    let y = mix(-max(w.size_depth_time.z, 0.001), top.position.y, 1.0 - c.y);
    return WaterVertexLocal(vec3<f32>(top.position.x, y, top.position.z), top.uv, 1.0, true);
}

@vertex
fn vs_water_3d(
    @builtin(vertex_index) vertex_idx: u32,
    @builtin(instance_index) chunk_idx: u32,
) -> Water3DVertexOut {
    let chunk = render_chunks[chunk_idx];
    let water_idx = chunk.water_idx;
    let w = waters[water_idx];
    var surface_vertex_count = (max(chunk.render_width, 2u) - 1u) * (max(chunk.render_height, 2u) - 1u) * 6u;
    var local_vertex = water_chunk_surface_vertex(w, chunk, vertex_idx);
    if (chunk.flags & 2u) != 0u {
        let counts = water_circle_counts(w);
        surface_vertex_count = counts.x * counts.y * 6u;
        local_vertex = water_circle_surface_vertex(w, vertex_idx);
        if vertex_idx >= surface_vertex_count {
            local_vertex = water_circle_side_vertex(w, vertex_idx - surface_vertex_count);
        }
    } else if vertex_idx >= surface_vertex_count && (chunk.flags & 1u) != 0u {
        local_vertex = water_rect_side_vertex(w, vertex_idx - surface_vertex_count);
    } else if vertex_idx >= surface_vertex_count {
        local_vertex.valid = false;
    }
    let scaled = vec4<f32>(local_vertex.position, 1.0);
    let model = mat4x4<f32>(
        vec4<f32>(w.model_x.xyz, 0.0),
        vec4<f32>(w.model_y.xyz, 0.0),
        vec4<f32>(w.model_z.xyz, 0.0),
        w.model_w,
    );
    let world = model * scaled;

    var out: Water3DVertexOut;
    out.clip_pos = select(vec4<f32>(2.0, 2.0, 1.0, 1.0), scene.view_proj * world, local_vertex.valid);
    out.uv = local_vertex.uv;
    out.water_idx = water_idx;
    out.world_pos = world.xyz;
    out.side_t = local_vertex.side_t;
    return out;
}

@fragment
fn fs_water_3d(in: Water3DVertexOut) -> @location(0) vec4<f32> {
    let w = waters[in.water_idx];
    if in.side_t <= 0.5 && water_shape_alpha(w, in.uv) <= 0.0 {
        return vec4<f32>(0.0);
    }
    let t = params.time_seconds;
    let idle = sin((in.uv.x + in.uv.y + t * w.wave.x * 0.2) * 6.2831853) * 0.5 + 0.5;
    var ripple = water_cell(w, in.uv);
    if water_coast_solid(w, in.uv) {
        return vec4<f32>(0.0);
    }
    let view_dist = distance(scene.camera_pos.xyz, in.world_pos);
    let far_t = clamp((view_dist - 140.0) / 220.0, 0.0, 1.0);
    let normal = normalize(mix(water_visual_normal(w, in.uv, t), water_visual_normal_fast(w, in.uv), far_t));
    let view_dir = normalize(scene.camera_pos.xyz - in.world_pos);
    let fresnel_base = water_schlick_fresnel(dot(normal, view_dir), w.visual0.w);
    let screen_contact = water_screen_contact_outline(in);
    let screen_outline = screen_contact.x;
    let screen_outline_core = 1.0 - smoothstep(0.01, 0.09, screen_contact.y);
    let screen_contact_foam = smoothstep(0.18, 0.92, screen_outline) * (0.42 + screen_outline_core * 0.58);
    let slope = 1.0 - clamp(normal.y, 0.0, 1.0);
    let edge = max(0.0, 1.0 - min(min(in.uv.x, 1.0 - in.uv.x), min(in.uv.y, 1.0 - in.uv.y)) * max(w.coastline.y, 0.001) * 8.0);
    let auto_shallow_depth = max(max(w.size_depth_time.x, w.size_depth_time.y) * 0.25, 0.001);
    let shallow_depth = select(auto_shallow_depth, max(w.size_depth_time.w, 0.001), w.size_depth_time.w >= 0.0);
    let depth_t = clamp(w.size_depth_time.z / shallow_depth, 0.0, 1.0);
    if view_dist >= 320.0 {
        let coast_outline = max(max(edge * 0.20, ripple.w * 0.35), screen_outline);
        let foam = clamp(
            (smoothstep(0.32, 0.90, ripple.z) * 0.08 + screen_contact_foam * w.coastline.x * 0.96)
                * w.visual1.z,
            0.0,
            1.0,
        );
        let foam_aa = max(fwidth(foam), 0.01);
        let foam_blend = smoothstep(0.06 - foam_aa, 0.72 + foam_aa, foam);
        let shallow_t = clamp(1.0 - depth_t + idle * 0.02 + foam * 0.02, 0.0, 1.0);
        let fresnel = fresnel_base * (0.24 + screen_outline * 0.22 + screen_outline_core * 0.08);
        let water_rgb = mix(w.deep_color.rgb, w.shallow_color.rgb, shallow_t);
        let reflected = mix(water_rgb, w.sky_color_bias.rgb, max(w.sky_color_bias.w, w.visual0.y * fresnel * 0.62));
        let fog_t = clamp(view_dist / 620.0, 0.0, 1.0) * w.visual2.w;
        let outline_aa = max(fwidth(screen_outline), 0.01);
        let outline_white = smoothstep(0.34 - outline_aa, 0.80 + outline_aa, screen_outline);
        let color = mix(mix(reflected, w.deep_color.rgb, fog_t), vec3<f32>(0.94), outline_white * max(foam_blend, screen_contact_foam * 0.72));
        let alpha = mix(w.deep_color.a, w.shallow_color.a, shallow_t) * (1.0 - clamp(w.visual0.x, 0.0, 1.0) * 0.72);
        let side_color = mix(w.deep_color.rgb, color, 0.22);
        let final_color = mix(color, side_color, in.side_t);
        let final_alpha = mix(alpha + foam_blend * 0.04 + fresnel * w.visual0.y * 0.03, w.deep_color.a * 0.82, in.side_t);
        return vec4<f32>(final_color, clamp(final_alpha, 0.0, 1.0));
    }
    if view_dist >= 220.0 {
        let coast_outline = max(max(edge * 0.18, ripple.w * 0.30), screen_outline);
        let crest_seed = ripple.y / max(w.wave.y, 0.001) + slope * 2.2;
        let crest = smoothstep(max(w.visual1.w, 0.001), max(w.visual1.w + 0.16, 0.002), crest_seed)
            * (1.0 - smoothstep(max(w.visual1.w + 0.24, 0.003), max(w.visual1.w + 0.52, 0.004), crest_seed))
            * bitcast<f32>(w.flags.w);
        let foam = clamp(
            (smoothstep(0.22, 0.88, ripple.z) * 0.10 + screen_contact_foam * w.coastline.x * 0.92 + crest * 0.24)
                * w.visual1.z,
            0.0,
            1.0,
        );
        let foam_aa = max(fwidth(foam), 0.01);
        let foam_blend = smoothstep(0.06 - foam_aa, 0.74 + foam_aa, foam);
        let shallow_t = clamp(1.0 - depth_t + idle * 0.04 + foam * 0.02, 0.0, 1.0);
        let fresnel = fresnel_base * (0.22 + screen_outline * 0.20 + screen_outline_core * 0.08);
        let water_rgb = mix(w.deep_color.rgb, w.shallow_color.rgb, shallow_t);
        let reflected = mix(water_rgb, w.sky_color_bias.rgb, max(w.sky_color_bias.w, w.visual0.y * fresnel * 0.58));
        let fog_t = clamp(view_dist / 620.0, 0.0, 1.0) * w.visual2.w;
        let foam_rgb = mix(w.coastline_foam_color.rgb, w.foam_color.rgb, w.foam_color.a);
        let outline_aa = max(fwidth(screen_outline), 0.01);
        let outline_white = smoothstep(0.32 - outline_aa, 0.78 + outline_aa, screen_outline);
        let color = mix(
            mix(mix(reflected, w.deep_color.rgb, fog_t), foam_rgb, foam_blend * 0.26),
            vec3<f32>(0.94),
            outline_white * max(foam_blend, screen_contact_foam * 0.72),
        );
        let alpha = mix(w.deep_color.a, w.shallow_color.a, shallow_t) * (1.0 - clamp(w.visual0.x, 0.0, 1.0) * 0.72);
        let side_color = mix(w.deep_color.rgb, color, 0.28);
        let final_color = mix(color, side_color, in.side_t);
        let final_alpha = mix(alpha + foam_blend * 0.05 + fresnel * w.visual0.y * 0.04, w.deep_color.a * 0.82, in.side_t);
        return vec4<f32>(final_color, clamp(final_alpha, 0.0, 1.0));
    }
    let local = (in.uv - vec2<f32>(0.5, 0.5)) * w.size_depth_time.xy;
    let world_uv = in.world_pos.xz;
    let wave_flow = normalize(select(vec2<f32>(1.0, 0.0), w.flow_wind.zw + w.flow_wind.xy * 0.35, length(w.flow_wind.zw + w.flow_wind.xy * 0.35) > 0.0001));
    let wave_push = vec2<f32>(normal.x, normal.z) * 0.018 * clamp(w.visual1.x, 0.0, 1.4);
    let foam_drift = wave_flow
        * (0.010 * sin(t * w.wave.x * 1.7 + ripple.y)
            + 0.006 * water_hex_ridged_fbm(local * (0.21 - far_t * 0.08) + t * 0.07));
    let coast_anim = water_coast_sample(w, in.uv + wave_push + foam_drift);
    let coast_outline = max(screen_outline, max(edge * 0.18, max(coast_anim.y * 0.20, ripple.w * 0.18)));
    let foam_break =
        water_hex_ridged_fbm(local * 1.35 + wave_flow * t * 0.42 + vec2<f32>(ripple.y * 0.23, -ripple.x * 0.17));
    let foam_cut = smoothstep(0.54, 0.91, foam_break);
    let foam_thread = smoothstep(
        0.64,
        0.94,
        water_hex_ridged_fbm(local * 3.9 - wave_flow * t * 0.68 + vec2<f32>(ripple.x * 0.31, ripple.y * 0.19)),
    );
    let impact_core = smoothstep(0.20, 0.86, ripple.z) * foam_cut;
    let impact_lace = impact_core * foam_thread;
    let fresnel_break = water_perlin_fbm(world_uv * mix(0.070, 0.040, far_t) + wave_flow * t * 0.028 + normal.xz * 1.7);
    let fresnel = fresnel_base * (0.32 + screen_outline * 0.34 + slope * 0.18 + fresnel_break * 0.16);
    let crest_seed = ripple.y / max(w.wave.y, 0.001) + slope * 2.4;
    let crest_base = smoothstep(max(w.visual1.w, 0.001), max(w.visual1.w + 0.18, 0.002), crest_seed)
        * (1.0 - smoothstep(max(w.visual1.w + 0.20, 0.003), max(w.visual1.w + 0.58, 0.004), crest_seed))
        * bitcast<f32>(w.flags.w) * 0.64;
    let crest = crest_base * 0.18;
    let outline_foam = screen_contact_foam * w.coastline.x * bitcast<f32>(w.flags.w);
    let foam = clamp((impact_core * 0.10 + impact_lace * 0.14 + outline_foam * 0.92 + crest) * w.visual1.z, 0.0, 1.0);
    let caustic_seed = water_fbm(in.uv * max(w.size_depth_time.xy, vec2<f32>(1.0)) * 0.42 + vec2<f32>(t * 0.18, -t * 0.13));
    let caustic = smoothstep(0.62, 0.92, caustic_seed) * (1.0 - depth_t) * w.visual2.x;
    let sun_dir = normalize(select(vec3<f32>(0.0, 1.0, 0.0), -scene.ray_light.direction.xyz, length(scene.ray_light.direction.xyz) > 0.001));
    let scatter = (1.0 - depth_t) * w.visual2.z * max(dot(normal, sun_dir), 0.0);
    let basin = water_perlin_fbm(world_uv * mix(0.060, 0.034, far_t) + vec2<f32>(t * 0.012, -t * 0.008));
    let shoal = water_perlin_fbm(world_uv * mix(0.130, 0.075, far_t) + vec2<f32>(4.3, 8.1));
    let macro_break = water_ridged_fbm(world_uv * mix(0.095, 0.050, far_t) - wave_flow * t * 0.032 + normal.xz * 0.6);
    let lowlight_noise = water_perlin_fbm(world_uv * mix(0.18, 0.10, far_t) - wave_flow * t * 0.045 + vec2<f32>(3.0, 17.0));
    let highlight_noise = water_perlin_fbm(world_uv * mix(0.34, 0.21, far_t) + wave_flow * t * 0.14 + vec2<f32>(9.0, 2.0));
    let micro_break = water_ridged_fbm(world_uv * mix(0.52, 0.30, far_t) + wave_flow * t * 0.11 + vec2<f32>(14.0, 5.0));
    let dark_patch = smoothstep(0.42, 0.86, basin * 0.54 + lowlight_noise * 0.36 + macro_break * 0.22) * (1.0 - foam) * 0.34;
    let light_patch = smoothstep(0.50, 0.88, shoal * 0.46 + highlight_noise * 0.40 + micro_break * 0.20) * (1.0 - depth_t) * 0.46;
    let scratch_ripple = (highlight_noise - lowlight_noise) * 0.08 + (micro_break - 0.5) * 0.06;
    let shallow_t = clamp(1.0 - depth_t + idle * 0.06 + foam * 0.035 + caustic * 0.12 + light_patch * 0.14 - dark_patch * 0.25, 0.0, 1.0);
    let surface_t = clamp(shallow_t + abs(ripple.x + scratch_ripple) * 0.12 + foam * 0.025 + clamp(view_dist / 256.0, 0.0, 1.0) * 0.04, 0.0, 1.0);
    let depth_rgb = mix(w.deep_color.rgb * (0.74 - dark_patch * 0.18), w.deep_color.rgb, depth_t);
    let water_rgb = mix(depth_rgb, w.shallow_color.rgb + vec3<f32>(light_patch * 0.18), surface_t);
    let refract_tint = vec3<f32>(caustic * 0.22 + w.visual2.y * (1.0 - depth_t) * 0.08 + light_patch * 0.06);
    let reflected = mix(water_rgb, w.sky_color_bias.rgb, max(w.sky_color_bias.w, w.visual0.y * fresnel * 0.34));
    let rough_blend = clamp(w.visual0.z, 0.0, 1.0);
    let half_dir = normalize(view_dir + sun_dir);
    let spec_line = pow(max(dot(normal, half_dir), 0.0), mix(128.0, 36.0, rough_blend)) * 0.08 * w.visual0.y * (1.0 - screen_outline * 0.85);
    let fresnel_tint = vec3<f32>(0.16, 0.22, 0.26) * fresnel * w.visual0.y;
    let lit_water = mix(reflected, water_rgb, rough_blend * 0.48) + refract_tint + scatter + fresnel_tint + vec3<f32>(spec_line + micro_break * 0.025) - vec3<f32>(dark_patch * 0.10);
    let fog_t = clamp(view_dist / 620.0, 0.0, 1.0) * w.visual2.w;
    let fogged = mix(lit_water, w.deep_color.rgb, fog_t);
    let foam_rgb = mix(w.coastline_foam_color.rgb, w.foam_color.rgb, w.foam_color.a);
    let foam_aa = max(fwidth(foam), 0.01);
    let foam_blend = smoothstep(0.05 - foam_aa, 0.76 + foam_aa, foam);
    let outline_mask = screen_outline;
    let outline_aa = max(fwidth(outline_mask), 0.01);
    let outline_white = smoothstep(0.24 - outline_aa, 0.74 + outline_aa, outline_mask);
    let color = mix(mix(fogged, foam_rgb, foam_blend * 0.34), vec3<f32>(0.94), outline_white * max(foam_blend, screen_contact_foam * 0.76));
    let alpha = mix(w.deep_color.a, w.shallow_color.a, shallow_t) * (1.0 - clamp(w.visual0.x, 0.0, 1.0) * 0.72);
    let side_color = mix(w.deep_color.rgb, color, 0.35);
    let final_color = mix(color, side_color, in.side_t);
    let final_alpha = mix(alpha + foam_blend * 0.08 + fresnel * w.visual0.y * 0.06, w.deep_color.a * 0.82, in.side_t);
    return vec4<f32>(final_color, clamp(final_alpha, 0.0, 1.0));
}
"#;

fn water_render_wgsl() -> String {
    WATER_WGSL
        .replace("var<storage, read_write> cells", "var<storage, read> cells")
        .replace(
            "cells[cell_idx] = vec4<f32>(0.0);",
            "let render_only_shape_skip = cell_idx;",
        )
        .replace(
            "cells[cell_idx] = vec4<f32>(0.0, 0.0, 1.0, 1.0);",
            "let render_only_coast_skip = cell_idx;",
        )
        .replace(
            "cells[cell_idx] = vec4<f32>(0.0);",
            "let render_only_empty_skip = cell_idx;",
        )
        .replace(
            "cells[cell_idx] = vec4<f32>(height, idle, foam, shore);",
            "let render_only_wave_skip = height + idle + foam + shore;",
        )
        .replace(
            "let edge_noise = (sin((local.x * 0.31 + local.y * 0.47) + phase * 7.0) + sin((local.x * -0.53 + local.y * 0.29) - phase * 4.3)) * 0.5 * w.model_z.w;\n    let crash_wave = max(0.0, sin((local.x * 0.19 - local.y * 0.23) + phase * 5.5 + edge_noise));\n    let crash = shore * pow(crash_wave, 4.2) * w.model_y.w * w.wave.y * 2.25;\n    let prev = cells[cell_idx].x * w.wave.z * (1.0 - shore * w.coastline.w * 0.35);\n    let crest_line = smoothstep(0.78, 1.0, idle / max(w.wave.y, 0.001)) * (1.0 - smoothstep(1.0, 1.75, idle / max(w.wave.y, 0.001)));\n    let wave_foam = crest_line * bitcast<f32>(w.flags.w) * 0.20;\n    let impact_foam = smoothstep(0.10, 1.35, wake + crash) * bitcast<f32>(w.flags.w) * 0.42;\n    let shore_foam = smoothstep(0.45, 1.65, crash) * (1.0 - smoothstep(1.65, 2.8, crash)) * w.coastline.x * bitcast<f32>(w.flags.w);\n    let foam = clamp(wave_foam + impact_foam + shore_foam + coast.w * wake * 0.26, 0.0, 1.0);\n    let height = mix(prev + idle * (0.018 + shore * w.model_y.w * 0.12) + wake * 0.16 + crash, idle + wake * 0.18 + crash, 0.62);\n    cells[cell_idx] = vec4<f32>(height, idle, foam, shore);",
            "let render_only_unused = idle + shore + wake + coast.w;",
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
        assert!(WATER_3D_RENDER_WGSL.contains("vec4<f32>(w.model_x.xyz, 0.0)"));
        assert!(WATER_3D_RENDER_WGSL.contains("vec4<f32>(w.model_y.xyz, 0.0)"));
        assert!(WATER_3D_RENDER_WGSL.contains("vec4<f32>(w.model_z.xyz, 0.0)"));
        assert!(WATER_3D_RENDER_WGSL.contains("let width = max(w.sim.z, 1u);"));
        assert!(WATER_3D_RENDER_WGSL.contains("let width = max(w.flags.x, 1u);"));
        assert!(WATER_3D_RENDER_WGSL.contains("water_circle_surface_vertex"));
        assert!(WATER_3D_RENDER_WGSL.contains("water_circle_side_vertex"));
        assert!(WATER_3D_RENDER_WGSL.contains("horizontal_segments * 2u + vertical_segments"));
        assert!(
            WATER_3D_RENDER_WGSL.contains(
                "vec2<u32>(0u, 0u),\n        vec2<u32>(1u, 1u),\n        vec2<u32>(1u, 0u)"
            )
        );
        assert!(
            WATER_3D_RENDER_WGSL.contains(
                "vec2<u32>(0u, 0u),\n        vec2<u32>(0u, 1u),\n        vec2<u32>(1u, 1u)"
            )
        );
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
        assert_eq!(mid.grid.sim, [69, 69]);
        assert_eq!(mid.grid.render, [236, 236]);
        assert!(mid.ripple_blend > 0.5 && mid.ripple_blend < 0.6);
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
    fn water_readback_interval_uses_rate() {
        assert_eq!(readback_interval_seconds(0.0), 0.0);
        assert!((readback_interval_seconds(60.0) - (1.0 / 60.0)).abs() < 1.0e-6);
        assert!((readback_interval_seconds(30.0) - (1.0 / 30.0)).abs() < 1.0e-6);
        assert!((readback_interval_seconds(15.0) - (1.0 / 15.0)).abs() < 1.0e-6);
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
