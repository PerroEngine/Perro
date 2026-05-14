use bytemuck::{Pod, Zeroable};
use perro_ids::NodeID;
use perro_render_bridge::{
    Water2DState, Water3DState, WaterCoastlineShape2D, WaterCoastlineShape3D, WaterIdleModeState,
    WaterSampleState, WaterShapeState,
};
use std::sync::mpsc;

const WATER_WORKGROUP_SIZE: u32 = 64;

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
    coastline_foam_color: [f32; 4],
    coastline: [f32; 4],
    shape: [f32; 4],
    sim: [u32; 4],
    model_x: [f32; 4],
    model_y: [f32; 4],
    model_z: [f32; 4],
    model_w: [f32; 4],
}

#[repr(C)]
#[derive(Clone, Copy, Pod, Zeroable)]
struct WaterParamsGpu {
    water_count: u32,
    water_2d_count: u32,
    cell_count: u32,
    frame_index: u32,
}

pub struct GpuWater {
    compute_pipeline: wgpu::ComputePipeline,
    render_pipeline_2d: wgpu::RenderPipeline,
    render_pipeline_3d: wgpu::RenderPipeline,
    compute_bgl: wgpu::BindGroupLayout,
    render_bgl: wgpu::BindGroupLayout,
    compute_bind_group: wgpu::BindGroup,
    render_bind_group: wgpu::BindGroup,
    water_buffer: wgpu::Buffer,
    cell_buffer: wgpu::Buffer,
    coastline_buffer: wgpu::Buffer,
    params_buffer: wgpu::Buffer,
    readback_buffer: wgpu::Buffer,
    water_capacity: usize,
    cell_capacity: usize,
    active_cell_count: usize,
    max_cells_per_water: usize,
    water_count: u32,
    water_2d_count: u32,
    frame_index: u32,
    readback_capacity: usize,
    readback_mapped_bytes: u64,
    readback_pending_rx: Option<mpsc::Receiver<Result<(), wgpu::BufferAsyncError>>>,
    readback_nodes: Vec<NodeID>,
    readback_offsets: Vec<usize>,
    readback_samples: Vec<WaterSampleState>,
    readback_frame: u32,
    readback_period_frames: u32,
    readback_copy_encoded: bool,
}

#[derive(Clone, Copy, Debug)]
pub struct WaterPrepareContext {
    pub camera_2d_position: [f32; 2],
    pub camera_3d_position: [f32; 3],
    pub sky_color: [f32; 3],
}

impl GpuWater {
    pub fn new(
        device: &wgpu::Device,
        color_format: wgpu::TextureFormat,
        sample_count: u32,
        camera_bgl: &wgpu::BindGroupLayout,
        camera_3d_bgl: &wgpu::BindGroupLayout,
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
            ],
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
            bind_group_layouts: &[Some(&render_bgl), Some(camera_3d_bgl)],
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
                cull_mode: None,
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
        let readback_buffer = readback_buffer(device, 1);
        let params_buffer = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("perro_water_gpu_params"),
            size: std::mem::size_of::<WaterParamsGpu>() as u64,
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });
        let compute_bind_group = make_bind_group(
            device,
            &compute_bgl,
            &water_buffer,
            &cell_buffer,
            &coastline_buffer,
            &params_buffer,
            "perro_water_gpu_bg",
        );
        let render_bind_group = make_bind_group(
            device,
            &render_bgl,
            &water_buffer,
            &cell_buffer,
            &coastline_buffer,
            &params_buffer,
            "perro_water_render_bg",
        );
        Self {
            compute_pipeline,
            render_pipeline_2d,
            render_pipeline_3d,
            compute_bgl,
            render_bgl,
            compute_bind_group,
            render_bind_group,
            water_buffer,
            cell_buffer,
            coastline_buffer,
            params_buffer,
            readback_buffer,
            water_capacity: 1,
            cell_capacity: 64,
            active_cell_count: 0,
            max_cells_per_water: 64,
            water_count: 0,
            water_2d_count: 0,
            frame_index: 0,
            readback_capacity: 1,
            readback_mapped_bytes: 0,
            readback_pending_rx: None,
            readback_nodes: Vec::new(),
            readback_offsets: Vec::new(),
            readback_samples: Vec::new(),
            readback_frame: 0,
            readback_period_frames: 2,
            readback_copy_encoded: false,
        }
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
        self.frame_index = self.frame_index.wrapping_add(1);
        let needed = waters_2d.len() + waters_3d.len();
        self.water_count = needed.min(u32::MAX as usize) as u32;
        self.water_2d_count = waters_2d.len().min(u32::MAX as usize) as u32;
        if self.water_count == 0 {
            self.active_cell_count = 0;
            self.max_cells_per_water = 0;
            return;
        }
        let mut staged = Vec::with_capacity(needed);
        let mut coastline_cells = Vec::new();
        let mut cell_needed = 0usize;
        let mut readback_rate = 0.0f32;
        for (node, water) in waters_2d {
            readback_rate = readback_rate.max(water.sample_readback_rate);
            let lod = water_lod_2d(water, ctx.camera_2d_position);
            let cells = water_cell_count(lod.resolution);
            let offset = cell_needed;
            if cells > 0 {
                coastline_cells.resize(offset.saturating_add(cells), [0.0; 4]);
                raster_coastline_2d(
                    &mut coastline_cells[offset..offset + cells],
                    lod.resolution,
                    water,
                );
            }
            staged.push(water_gpu_2d(
                *node,
                water,
                lod.resolution,
                offset as u32,
                cells as u32,
                lod.ripple_blend,
            ));
            cell_needed = cell_needed.saturating_add(cells);
        }
        for (node, water) in waters_3d {
            readback_rate = readback_rate.max(water.sample_readback_rate);
            let lod = water_lod_3d(water, ctx.camera_3d_position);
            let cells = water_cell_count(lod.resolution);
            let offset = cell_needed;
            if cells > 0 {
                coastline_cells.resize(offset.saturating_add(cells), [0.0; 4]);
                raster_coastline_3d(
                    &mut coastline_cells[offset..offset + cells],
                    lod.resolution,
                    water,
                );
            }
            staged.push(water_gpu_3d(
                *node,
                water,
                lod.resolution,
                offset as u32,
                cells as u32,
                lod.ripple_blend,
                ctx.sky_color,
            ));
            cell_needed = cell_needed.saturating_add(cells);
        }
        cell_needed = cell_needed.max(WATER_WORKGROUP_SIZE as usize);
        self.active_cell_count = cell_needed;
        self.max_cells_per_water = staged
            .iter()
            .map(|water| water.sim[1] as usize)
            .max()
            .unwrap_or(WATER_WORKGROUP_SIZE as usize);
        self.readback_period_frames = readback_period_frames(readback_rate);
        let rebuilt = self.ensure_capacity(device, needed, cell_needed);
        if rebuilt {
            self.compute_bind_group = make_bind_group(
                device,
                &self.compute_bgl,
                &self.water_buffer,
                &self.cell_buffer,
                &self.coastline_buffer,
                &self.params_buffer,
                "perro_water_gpu_bg",
            );
            self.render_bind_group = make_bind_group(
                device,
                &self.render_bgl,
                &self.water_buffer,
                &self.cell_buffer,
                &self.coastline_buffer,
                &self.params_buffer,
                "perro_water_render_bg",
            );
        }
        queue.write_buffer(&self.water_buffer, 0, bytemuck::cast_slice(&staged));
        if !coastline_cells.is_empty() {
            queue.write_buffer(
                &self.coastline_buffer,
                0,
                bytemuck::cast_slice(&coastline_cells),
            );
        }
        let params = WaterParamsGpu {
            water_count: self.water_count,
            water_2d_count: self.water_2d_count,
            cell_count: cell_needed.min(u32::MAX as usize) as u32,
            frame_index: self.frame_index,
        };
        queue.write_buffer(&self.params_buffer, 0, bytemuck::bytes_of(&params));
        self.readback_nodes.clear();
        self.readback_offsets.clear();
        for ((node, _), water) in waters_2d.iter().zip(staged.iter()) {
            if water.sim[1] > 0 {
                self.readback_nodes.push(*node);
                self.readback_offsets.push(water.sim[0] as usize);
            }
        }
        for ((node, _), water) in waters_3d.iter().zip(staged.iter().skip(waters_2d.len())) {
            if water.sim[1] > 0 {
                self.readback_nodes.push(*node);
                self.readback_offsets.push(water.sim[0] as usize);
            }
        }
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
        let water_3d_count = self.water_count.saturating_sub(self.water_2d_count);
        if water_3d_count == 0 {
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
        pass.draw(0..6, self.water_2d_count..self.water_count);
    }

    pub fn encode_readback(&mut self, encoder: &mut wgpu::CommandEncoder) {
        self.readback_copy_encoded = false;
        if self.water_count == 0 || self.readback_pending_rx.is_some() {
            return;
        }
        self.readback_frame = self.readback_frame.wrapping_add(1);
        if self.readback_period_frames == 0
            || !self
                .readback_frame
                .is_multiple_of(self.readback_period_frames)
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

    fn ensure_capacity(
        &mut self,
        device: &wgpu::Device,
        needed_waters: usize,
        needed_cells: usize,
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
                for (idx, node) in self.readback_nodes.iter().enumerate() {
                    let cell = cells.get(idx).copied().unwrap_or([0.0; 4]);
                    self.readback_samples.push(WaterSampleState {
                        node: *node,
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

fn readback_period_frames(rate_hz: f32) -> u32 {
    if !rate_hz.is_finite() || rate_hz <= 0.0 {
        return 0;
    }
    (60.0 / rate_hz.clamp(1.0, 240.0)).ceil().max(1.0) as u32
}

fn make_bind_group(
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

fn raster_coastline_2d(out: &mut [[f32; 4]], resolution: [u32; 2], water: &Water2DState) {
    let width = resolution[0].clamp(1, 256) as usize;
    let height = resolution[1].clamp(1, 256) as usize;
    if water.coastline_shapes.is_empty() {
        raster_impacts_2d(out, width, height, water);
        return;
    }
    let foam_width = water.coastline_foam_width.max(0.001);
    for y in 0..height {
        for x in 0..width {
            let fx = x as f32 / (width.saturating_sub(1).max(1) as f32);
            let fy = y as f32 / (height.saturating_sub(1).max(1) as f32);
            let p = [(fx - 0.5) * water.size[0], (fy - 0.5) * water.size[1]];
            let mut solid = 0.0f32;
            let mut edge = 0.0f32;
            for shape in water.coastline_shapes.iter() {
                let signed = signed_distance_2d(p, *shape);
                if signed <= 0.0 {
                    solid = 1.0;
                    edge = 1.0;
                    break;
                }
                edge = edge.max(1.0 - (signed / foam_width).clamp(0.0, 1.0));
            }
            let mut wake = 0.0f32;
            for impact in water.impacts.iter() {
                let dx = p[0] - impact.position[0];
                let dy = p[1] - impact.position[1];
                let dist = (dx * dx + dy * dy).sqrt();
                let radius = impact.radius.max(0.001);
                wake += (1.0 - (dist / radius).clamp(0.0, 1.0)) * impact.strength / 256.0;
                wake += (1.0 - (dist / radius).clamp(0.0, 1.0)) * impact.cavitation;
            }
            out[y * width + x] = [solid, edge, wake.clamp(0.0, 1.0), wake.clamp(0.0, 1.0)];
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
                let dist = (dx * dx + dy * dy).sqrt();
                let amount = 1.0 - (dist / radius).clamp(0.0, 1.0);
                if amount <= 0.0 {
                    continue;
                }
                let wake = amount * (impact.strength / 256.0 + impact.cavitation);
                let cell = &mut out[y * width + x];
                cell[2] = (cell[2] + wake).clamp(0.0, 1.0);
                cell[3] = cell[2];
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
    for y in 0..height {
        for x in 0..width {
            let fx = x as f32 / (width.saturating_sub(1).max(1) as f32);
            let fy = y as f32 / (height.saturating_sub(1).max(1) as f32);
            let p = [(fx - 0.5) * water.size[0], (fy - 0.5) * water.size[1]];
            let mut solid = 0.0f32;
            let mut edge = 0.0f32;
            for shape in water.coastline_shapes.iter() {
                let signed = signed_distance_3d_xz(p, *shape);
                if signed <= 0.0 {
                    solid = 1.0;
                    edge = 1.0;
                    break;
                }
                edge = edge.max(1.0 - (signed / foam_width).clamp(0.0, 1.0));
            }
            let mut wake = 0.0f32;
            for impact in water.impacts.iter() {
                let dx = p[0] - impact.position[0];
                let dz = p[1] - impact.position[2];
                let dist = (dx * dx + dz * dz).sqrt();
                let radius = impact.radius.max(0.001);
                wake += (1.0 - (dist / radius).clamp(0.0, 1.0)) * impact.strength / 256.0;
                wake += (1.0 - (dist / radius).clamp(0.0, 1.0)) * impact.cavitation;
            }
            out[y * width + x] = [solid, edge, wake.clamp(0.0, 1.0), wake.clamp(0.0, 1.0)];
        }
    }
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
                let dist = (dx * dx + dz * dz).sqrt();
                let amount = 1.0 - (dist / radius).clamp(0.0, 1.0);
                if amount <= 0.0 {
                    continue;
                }
                let wake = amount * (impact.strength / 256.0 + impact.cavitation);
                let cell = &mut out[y * width + x];
                cell[2] = (cell[2] + wake).clamp(0.0, 1.0);
                cell[3] = cell[2];
            }
        }
    }
}

fn signed_distance_3d_xz(p: [f32; 2], shape: WaterCoastlineShape3D) -> f32 {
    match shape {
        WaterCoastlineShape3D::Box {
            center,
            half_extents,
        } => {
            let lx = (p[0] - center[0]).abs() - half_extents[0];
            let ly = (p[1] - center[2]).abs() - half_extents[2];
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

fn water_lod_2d(water: &Water2DState, camera: [f32; 2]) -> WaterLodDecision {
    let pos = [water.model[2][0], water.model[2][1]];
    water_lod(
        water.resolution,
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
    resolution: [u32; 2],
    ripple_blend: f32,
}

fn water_lod(
    resolution: [u32; 2],
    size: [f32; 2],
    distances: [f32; 3],
    min_resolution: [u32; 2],
    water_pos: [f32; 2],
    camera_pos: [f32; 2],
) -> WaterLodDecision {
    let dx = water_pos[0] - camera_pos[0];
    let dy = water_pos[1] - camera_pos[1];
    let distance = (dx * dx + dy * dy).sqrt();
    let radius = size[0].max(size[1]).max(1.0);
    let near = distances[0].max(radius * 2.0);
    let mid = distances[1].max(near);
    let far = distances[2].max(mid);
    let (div, ripple_blend) = if distance <= near {
        (1, 1.0)
    } else if distance <= mid {
        let span = (mid - near).max(0.001);
        let t = ((distance - near) / span).clamp(0.0, 1.0);
        (2, 1.0 - t * 0.35)
    } else if distance <= far {
        let span = (far - mid).max(0.001);
        let t = ((distance - mid) / span).clamp(0.0, 1.0);
        (4, 0.65 - t * 0.45)
    } else {
        return WaterLodDecision {
            resolution: [0, 0],
            ripple_blend: 0.0,
        };
    };
    WaterLodDecision {
        resolution: [
            (resolution[0] / div).clamp(min_resolution[0].clamp(1, 256), 256),
            (resolution[1] / div).clamp(min_resolution[1].clamp(1, 256), 256),
        ],
        ripple_blend,
    }
}

fn water_gpu_2d(
    node: NodeID,
    water: &Water2DState,
    resolution: [u32; 2],
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
        water.damping,
        water.wake_strength,
        water.foam_strength,
        water.collision_layers.bits(),
        water.collision_mask.bits(),
        water.coastline_foam_color,
        water.deep_color,
        water.shallow_color,
        water.shallow_depth,
        [0.0, 0.0, 0.0, 0.0],
        [
            water.coastline_foam_strength,
            water.coastline_foam_width,
            water.coastline_cutoff_softness,
            water.coastline_wave_damping,
        ],
        water.debug,
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
    resolution: [u32; 2],
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
        water.damping,
        water.wake_strength,
        water.foam_strength,
        water.collision_layers.bits(),
        water.collision_mask.bits(),
        water.coastline_foam_color,
        water.deep_color,
        water.shallow_color,
        water.shallow_depth,
        [
            sky_color[0],
            sky_color[1],
            sky_color[2],
            water.sky_bias_ratio.clamp(0.0, 1.0),
        ],
        [
            water.coastline_foam_strength,
            water.coastline_foam_width,
            water.coastline_cutoff_softness,
            water.coastline_wave_damping,
        ],
        water.debug,
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
    resolution: [u32; 2],
    wave_speed: f32,
    wave_scale: f32,
    damping: f32,
    wake_strength: f32,
    foam_strength: f32,
    collision_layers: u32,
    collision_mask: u32,
    coastline_foam_color: [f32; 4],
    deep_color: [f32; 4],
    shallow_color: [f32; 4],
    shallow_depth: f32,
    sky_color_bias: [f32; 4],
    coastline: [f32; 4],
    debug: bool,
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
            collision_layers,
            collision_mask,
            u32::from(debug),
            foam_strength.to_bits(),
        ],
        deep_color,
        shallow_color,
        sky_color_bias,
        coastline_foam_color,
        coastline,
        shape: water_shape_gpu(shape, size, depth),
        sim: [
            cell_offset,
            cell_count,
            resolution[0].clamp(1, 256),
            resolution[1].clamp(1, 256),
        ],
        model_x: [
            model[0][0],
            model[0][1],
            model[0][2],
            ripple_blend.clamp(0.0, 1.0),
        ],
        model_y: [model[1][0], model[1][1], model[1][2], 0.0],
        model_z: [model[2][0], model[2][1], model[2][2], 0.0],
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
    frame_index: u32,
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
    let t = f32(params.frame_index) * 0.016;
    let phase = fract(fx + fy + w.flow_wind.z * 0.01 + t * w.wave.x * 0.15915494);
    let idle = (1.0 - abs(phase * 2.0 - 1.0) * 2.0) * w.wave.y;
    let coast = coastline_cells[cell_idx];
    if coast.x > 0.5 {
        cells[cell_idx] = vec4<f32>(0.0, 0.0, 1.0, 1.0);
        return;
    }
    let edge = max(0.0, 1.0 - min(min(fx, 1.0 - fx), min(fy, 1.0 - fy)) * max(w.coastline.y, 0.001) * 8.0);
    let shore = max(edge, coast.y);
    let wake = coast.z * w.wave.w;
    if shore <= 0.0 && wake <= 0.0 && coast.w <= 0.0 {
        cells[cell_idx] = vec4<f32>(0.0);
        return;
    }
    let prev = cells[cell_idx].x * w.wave.z * (1.0 - w.coastline.w * 0.08);
    let foam = clamp((shore + wake + coast.w) * abs(prev + idle + wake) * w.coastline.x + bitcast<f32>(w.flags.w) * 0.05, 0.0, 1.0);
    cells[cell_idx] = vec4<f32>(prev + idle * 0.015 * (1.0 - shore * w.coastline.w) + wake * 0.08, idle, foam, shore);
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
    let t = f32(params.frame_index) * 0.016;
    let idle = sin((in.uv.x + in.uv.y + t * w.wave.x) * 6.2831853) * 0.5 + 0.5;
    var ripple = vec4<f32>(0.0);
    if w.sim.y > 0u {
        let width = max(w.sim.z, 1u);
        let height = max(w.sim.w, 1u);
        let x = u32(clamp(in.uv.x, 0.0, 1.0) * f32(max(width - 1u, 1u)));
        let y = u32(clamp(in.uv.y, 0.0, 1.0) * f32(max(height - 1u, 1u)));
        let local_idx = min(y * width + x, w.sim.y - 1u);
        let cell_idx = w.sim.x + local_idx;
        ripple = cells[cell_idx] * w.model_x.w;
        if coastline_cells[cell_idx].x > 0.5 {
            return vec4<f32>(0.0, 0.0, 0.0, 0.0);
        }
    }
    let edge = max(0.0, 1.0 - min(min(in.uv.x, 1.0 - in.uv.x), min(in.uv.y, 1.0 - in.uv.y)) * max(w.coastline.y, 0.001) * 8.0);
    let foam = clamp(ripple.z + max(edge, ripple.w) * w.coastline.x + abs(ripple.x) * 0.35, 0.0, 1.0);
    let auto_shallow_depth = max(max(w.size_depth_time.x, w.size_depth_time.y) * 0.25, 0.001);
    let shallow_depth = select(auto_shallow_depth, max(w.size_depth_time.w, 0.001), w.size_depth_time.w >= 0.0);
    let depth_t = clamp(w.size_depth_time.z / shallow_depth, 0.0, 1.0);
    let shallow_t = clamp(1.0 - depth_t + idle * 0.18 + foam * 0.25, 0.0, 1.0);
    let surface_t = clamp(shallow_t + abs(ripple.x) * 0.22 + foam * 0.18, 0.0, 1.0);
    let water_rgb = mix(w.deep_color.rgb, w.shallow_color.rgb, surface_t);
    let sky_rgb = mix(water_rgb, w.sky_color_bias.rgb, w.sky_color_bias.w);
    let color = mix(sky_rgb, w.coastline_foam_color.rgb, foam * 0.65);
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
    frame_index: u32,
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
}

@group(0) @binding(0)
var<storage, read> waters: array<Water>;
@group(0) @binding(1)
var<storage, read> cells: array<vec4<f32>>;
@group(0) @binding(2)
var<uniform> params: Params;
@group(0) @binding(3)
var<storage, read> coastline_cells: array<vec4<f32>>;
@group(1) @binding(0)
var<uniform> scene: Scene3D;

struct Water3DVertexOut {
    @builtin(position) clip_pos: vec4<f32>,
    @location(0) uv: vec2<f32>,
    @location(1) water_idx: u32,
    @location(2) world_pos: vec3<f32>,
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

@vertex
fn vs_water_3d(
    @builtin(vertex_index) vertex_idx: u32,
    @builtin(instance_index) water_idx: u32,
) -> Water3DVertexOut {
    let w = waters[water_idx];
    let local = quad_pos(vertex_idx);
    let scaled = vec4<f32>(local.x * w.size_depth_time.x, 0.0, local.y * w.size_depth_time.y, 1.0);
    let model = mat4x4<f32>(w.model_x, w.model_y, w.model_z, w.model_w);
    let world = model * scaled;

    var out: Water3DVertexOut;
    out.clip_pos = scene.view_proj * world;
    out.uv = local + vec2<f32>(0.5, 0.5);
    out.water_idx = water_idx;
    out.world_pos = world.xyz;
    return out;
}

@fragment
fn fs_water_3d(in: Water3DVertexOut) -> @location(0) vec4<f32> {
    let w = waters[in.water_idx];
    if water_shape_alpha(w, in.uv) <= 0.0 {
        return vec4<f32>(0.0);
    }
    let t = f32(params.frame_index) * 0.016;
    let idle = sin((in.uv.x + in.uv.y + t * w.wave.x) * 6.2831853) * 0.5 + 0.5;
    var ripple = vec4<f32>(0.0);
    if w.sim.y > 0u {
        let width = max(w.sim.z, 1u);
        let height = max(w.sim.w, 1u);
        let x = u32(clamp(in.uv.x, 0.0, 1.0) * f32(max(width - 1u, 1u)));
        let y = u32(clamp(in.uv.y, 0.0, 1.0) * f32(max(height - 1u, 1u)));
        let local_idx = min(y * width + x, w.sim.y - 1u);
        let cell_idx = w.sim.x + local_idx;
        ripple = cells[cell_idx] * w.model_x.w;
        if coastline_cells[cell_idx].x > 0.5 {
            return vec4<f32>(0.0);
        }
    }
    let edge = max(0.0, 1.0 - min(min(in.uv.x, 1.0 - in.uv.x), min(in.uv.y, 1.0 - in.uv.y)) * max(w.coastline.y, 0.001) * 8.0);
    let foam = clamp(ripple.z + max(edge, ripple.w) * w.coastline.x + abs(ripple.x) * 0.35, 0.0, 1.0);
    let auto_shallow_depth = max(max(w.size_depth_time.x, w.size_depth_time.y) * 0.25, 0.001);
    let shallow_depth = select(auto_shallow_depth, max(w.size_depth_time.w, 0.001), w.size_depth_time.w >= 0.0);
    let depth_t = clamp(w.size_depth_time.z / shallow_depth, 0.0, 1.0);
    let view_dist = distance(scene.camera_pos.xyz, in.world_pos);
    let shallow_t = clamp(1.0 - depth_t + idle * 0.18 + foam * 0.25, 0.0, 1.0);
    let surface_t = clamp(shallow_t + abs(ripple.x) * 0.22 + foam * 0.18 + clamp(view_dist / 256.0, 0.0, 1.0) * 0.08, 0.0, 1.0);
    let water_rgb = mix(w.deep_color.rgb, w.shallow_color.rgb, surface_t);
    let sky_rgb = mix(water_rgb, w.sky_color_bias.rgb, w.sky_color_bias.w);
    let color = mix(sky_rgb, w.coastline_foam_color.rgb, foam * 0.65);
    let alpha = mix(w.deep_color.a, w.shallow_color.a, shallow_t);
    return vec4<f32>(color, clamp(alpha + foam * 0.12, 0.0, 1.0));
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
            "cells[cell_idx] = vec4<f32>(prev + idle * 0.015 * (1.0 - shore * w.coastline.w) + wake * 0.08, idle, foam, shore);",
            "let render_only_unused = prev + idle + foam + shore + wake;",
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
            size: [16.0, 16.0],
            shape: WaterShapeState::Rect,
            resolution: [8, 8],
            depth: 4.0,
            flow: [0.0, 0.0],
            wind: [1.0, 0.0],
            idle_mode: WaterIdleModeState::Calm,
            wave_speed: 1.0,
            wave_scale: 1.0,
            damping: 0.985,
            wake_strength: 1.0,
            foam_strength: 0.65,
            sample_readback_rate: 30.0,
            lod_near_distance: 128.0,
            lod_mid_distance: 384.0,
            lod_far_distance: 896.0,
            lod_min_resolution: [4, 4],
            collision_layers: perro_structs::BitMask::ALL,
            collision_mask: perro_structs::BitMask::NONE,
            deep_color: [0.02, 0.16, 0.28, 0.86],
            shallow_color: [0.08, 0.46, 0.62, 0.48],
            shallow_depth: -1.0,
            sky_bias_ratio: 0.0,
            coastline_foam_color: [0.9, 0.97, 1.0, 1.0],
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
            size: [16.0, 16.0],
            shape: WaterShapeState::Rect,
            resolution: [8, 8],
            depth: 4.0,
            flow: [0.0, 0.0],
            wind: [1.0, 0.0],
            idle_mode: WaterIdleModeState::Calm,
            wave_speed: 1.0,
            wave_scale: 1.0,
            damping: 0.985,
            wake_strength: 1.0,
            foam_strength: 0.65,
            sample_readback_rate: 30.0,
            lod_near_distance: 128.0,
            lod_mid_distance: 384.0,
            lod_far_distance: 896.0,
            lod_min_resolution: [4, 4],
            collision_layers: perro_structs::BitMask::ALL,
            collision_mask: perro_structs::BitMask::NONE,
            deep_color: [0.02, 0.16, 0.28, 0.86],
            shallow_color: [0.08, 0.46, 0.62, 0.48],
            shallow_depth: -1.0,
            sky_bias_ratio: 0.0,
            coastline_foam_color: [0.9, 0.97, 1.0, 1.0],
            coastline_foam_strength: 0.75,
            coastline_foam_width: 1.5,
            coastline_cutoff_softness: 0.25,
            coastline_wave_reflection: 0.45,
            coastline_wave_damping: 0.35,
            coastline_edge_noise: 0.2,
            debug: false,
            links: Arc::from([]),
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
    }

    #[test]
    fn water_lod_resolution_clamps_with_distance() {
        assert_eq!(
            water_lod(
                [256, 256],
                [64.0, 64.0],
                [128.0, 384.0, 896.0],
                [32, 32],
                [0.0, 0.0],
                [0.0, 0.0]
            ),
            WaterLodDecision {
                resolution: [256, 256],
                ripple_blend: 1.0,
            }
        );
        let mid = water_lod(
            [256, 256],
            [64.0, 64.0],
            [128.0, 384.0, 896.0],
            [32, 32],
            [512.0, 0.0],
            [0.0, 0.0],
        );
        assert_eq!(mid.resolution, [64, 64]);
        assert!(mid.ripple_blend > 0.5 && mid.ripple_blend < 0.6);
        assert_eq!(
            water_lod(
                [256, 256],
                [64.0, 64.0],
                [128.0, 384.0, 896.0],
                [32, 32],
                [2048.0, 0.0],
                [0.0, 0.0]
            ),
            WaterLodDecision {
                resolution: [0, 0],
                ripple_blend: 0.0,
            }
        );
        assert_eq!(water_cell_count([0, 0]), 0);
        assert_eq!(water_cell_count([1, 1]), 1);
    }

    #[test]
    fn water_readback_period_uses_rate() {
        assert_eq!(readback_period_frames(0.0), 0);
        assert_eq!(readback_period_frames(60.0), 1);
        assert_eq!(readback_period_frames(30.0), 2);
        assert_eq!(readback_period_frames(15.0), 4);
    }

    #[test]
    fn water_gpu_2d_staging_accepts_linked_water_state() {
        let water = test_water_2d();
        let staged = water_gpu_2d(
            NodeID::from_parts(7, 0),
            &water,
            water.resolution,
            4,
            64,
            1.0,
        );
        assert_eq!(staged.node, 7);
        assert_eq!(staged.sim, [4, 64, 8, 8]);
        assert_eq!(staged.kind, 2);
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
