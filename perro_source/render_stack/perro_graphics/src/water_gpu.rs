use bytemuck::{Pod, Zeroable};
use perro_ids::NodeID;
use perro_render_bridge::{Water2DState, Water3DState, WaterIdleModeState, WaterSampleState};
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
    sim: [u32; 4],
    model_x: [f32; 4],
    model_y: [f32; 4],
    model_z: [f32; 4],
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
    compute_bgl: wgpu::BindGroupLayout,
    render_bgl: wgpu::BindGroupLayout,
    compute_bind_group: wgpu::BindGroup,
    render_bind_group: wgpu::BindGroup,
    water_buffer: wgpu::Buffer,
    cell_buffer: wgpu::Buffer,
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
}

impl GpuWater {
    pub fn new(
        device: &wgpu::Device,
        color_format: wgpu::TextureFormat,
        sample_count: u32,
        camera_bgl: &wgpu::BindGroupLayout,
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
        let render_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("perro_water_2d_render_layout"),
            bind_group_layouts: &[Some(&render_bgl), Some(camera_bgl)],
            immediate_size: 0,
        });
        let render_pipeline_2d = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("perro_water_2d_pipeline"),
            layout: Some(&render_layout),
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
        let water_buffer = empty_buffer(device, "perro_water_gpu_waters", 1, true);
        let cell_buffer = empty_buffer(device, "perro_water_gpu_cells", 64, false);
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
            &params_buffer,
            "perro_water_gpu_bg",
        );
        let render_bind_group = make_bind_group(
            device,
            &render_bgl,
            &water_buffer,
            &cell_buffer,
            &params_buffer,
            "perro_water_render_bg",
        );
        Self {
            compute_pipeline,
            render_pipeline_2d,
            compute_bgl,
            render_bgl,
            compute_bind_group,
            render_bind_group,
            water_buffer,
            cell_buffer,
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
        }
    }

    pub fn prepare(
        &mut self,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        waters_2d: &[(NodeID, Water2DState)],
        waters_3d: &[(NodeID, Water3DState)],
        camera_2d_position: [f32; 2],
        camera_3d_position: [f32; 3],
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
        let mut cell_needed = 0usize;
        for (node, water) in waters_2d {
            let resolution = water_lod_resolution_2d(water, camera_2d_position);
            let cells = water_cell_count(resolution);
            staged.push(water_gpu_2d(
                *node,
                water,
                resolution,
                cell_needed as u32,
                cells as u32,
            ));
            cell_needed = cell_needed.saturating_add(cells);
        }
        for (node, water) in waters_3d {
            let resolution = water_lod_resolution_3d(water, camera_3d_position);
            let cells = water_cell_count(resolution);
            staged.push(water_gpu_3d(
                *node,
                water,
                resolution,
                cell_needed as u32,
                cells as u32,
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
        let rebuilt = self.ensure_capacity(device, needed, cell_needed);
        if rebuilt {
            self.compute_bind_group = make_bind_group(
                device,
                &self.compute_bgl,
                &self.water_buffer,
                &self.cell_buffer,
                &self.params_buffer,
                "perro_water_gpu_bg",
            );
            self.render_bind_group = make_bind_group(
                device,
                &self.render_bgl,
                &self.water_buffer,
                &self.cell_buffer,
                &self.params_buffer,
                "perro_water_render_bg",
            );
        }
        queue.write_buffer(&self.water_buffer, 0, bytemuck::cast_slice(&staged));
        let params = WaterParamsGpu {
            water_count: self.water_count,
            water_2d_count: self.water_2d_count,
            cell_count: cell_needed.min(u32::MAX as usize) as u32,
            frame_index: self.frame_index,
        };
        queue.write_buffer(&self.params_buffer, 0, bytemuck::bytes_of(&params));
        self.readback_nodes.clear();
        self.readback_offsets.clear();
        self.readback_nodes
            .extend(waters_2d.iter().map(|(node, _)| *node));
        self.readback_nodes
            .extend(waters_3d.iter().map(|(node, _)| *node));
        self.readback_offsets
            .extend(staged.iter().map(|water| water.sim[0] as usize));
    }

    pub fn encode(&self, encoder: &mut wgpu::CommandEncoder) {
        if self.water_count == 0 {
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
        pass.set_pipeline(&self.render_pipeline_2d);
        pass.set_bind_group(0, &self.render_bind_group, &[]);
        pass.set_bind_group(1, camera_bind_group, &[]);
        pass.draw(0..6, 0..self.water_2d_count);
    }

    pub fn encode_readback(&mut self, encoder: &mut wgpu::CommandEncoder) {
        if self.water_count == 0 || self.readback_pending_rx.is_some() {
            return;
        }
        let needed_samples = self.water_count as usize;
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
    }

    pub fn request_readback(&mut self) {
        if self.water_count == 0 || self.readback_pending_rx.is_some() {
            return;
        }
        let needed_samples = self.water_count as usize;
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

fn make_bind_group(
    device: &wgpu::Device,
    layout: &wgpu::BindGroupLayout,
    waters: &wgpu::Buffer,
    cells: &wgpu::Buffer,
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
        ],
    })
}

fn water_cell_count(resolution: [u32; 2]) -> usize {
    let x = resolution[0].clamp(8, 256) as usize;
    let y = resolution[1].clamp(8, 256) as usize;
    x.saturating_mul(y).max(WATER_WORKGROUP_SIZE as usize)
}

fn water_lod_resolution_2d(water: &Water2DState, camera: [f32; 2]) -> [u32; 2] {
    let pos = [water.model[2][0], water.model[2][1]];
    water_lod_resolution(water.resolution, water.size, pos, camera)
}

fn water_lod_resolution_3d(water: &Water3DState, camera: [f32; 3]) -> [u32; 2] {
    let pos = [water.model[3][0], water.model[3][2]];
    water_lod_resolution(water.resolution, water.size, pos, [camera[0], camera[2]])
}

fn water_lod_resolution(
    resolution: [u32; 2],
    size: [f32; 2],
    water_pos: [f32; 2],
    camera_pos: [f32; 2],
) -> [u32; 2] {
    let dx = water_pos[0] - camera_pos[0];
    let dy = water_pos[1] - camera_pos[1];
    let distance = (dx * dx + dy * dy).sqrt();
    let radius = size[0].max(size[1]).max(1.0);
    let div = if distance <= radius * 2.0 {
        1
    } else if distance <= radius * 6.0 {
        2
    } else if distance <= radius * 14.0 {
        4
    } else {
        8
    };
    [
        (resolution[0] / div).clamp(32, 256),
        (resolution[1] / div).clamp(32, 256),
    ]
}

fn water_gpu_2d(
    node: NodeID,
    water: &Water2DState,
    resolution: [u32; 2],
    cell_offset: u32,
    cell_count: u32,
) -> WaterGpu {
    water_gpu_common(
        node,
        2,
        water.idle_mode,
        water.size,
        water.depth,
        water.flow,
        water.wind,
        resolution,
        water.wave_speed,
        water.wave_scale,
        water.damping,
        water.wake_strength,
        water.foam_strength,
        water.shoreline_mask,
        water.static_body_wakes,
        water.debug,
        water.model,
        water.z_index,
        cell_offset,
        cell_count,
    )
}

fn water_gpu_3d(
    node: NodeID,
    water: &Water3DState,
    resolution: [u32; 2],
    cell_offset: u32,
    cell_count: u32,
) -> WaterGpu {
    water_gpu_common(
        node,
        3,
        water.idle_mode,
        water.size,
        water.depth,
        water.flow,
        water.wind,
        resolution,
        water.wave_speed,
        water.wave_scale,
        water.damping,
        water.wake_strength,
        water.foam_strength,
        water.shoreline_mask,
        water.static_body_wakes,
        water.debug,
        [[1.0, 0.0, 0.0], [0.0, 1.0, 0.0], [0.0, 0.0, 1.0]],
        0,
        cell_offset,
        cell_count,
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
    resolution: [u32; 2],
    wave_speed: f32,
    wave_scale: f32,
    damping: f32,
    wake_strength: f32,
    foam_strength: f32,
    shoreline_mask: bool,
    static_body_wakes: bool,
    debug: bool,
    model: [[f32; 3]; 3],
    z_index: i32,
    cell_offset: u32,
    cell_count: u32,
) -> WaterGpu {
    WaterGpu {
        node: node.index(),
        kind,
        idle_mode: idle_mode as u32,
        z_index,
        size_depth_time: [size[0], size[1], depth, 0.0],
        flow_wind: [flow[0], flow[1], wind[0], wind[1]],
        wave: [wave_speed, wave_scale, damping, wake_strength],
        flags: [
            u32::from(shoreline_mask),
            u32::from(static_body_wakes),
            u32::from(debug),
            foam_strength.to_bits(),
        ],
        sim: [
            cell_offset,
            cell_count,
            resolution[0].clamp(8, 256),
            resolution[1].clamp(8, 256),
        ],
        model_x: [model[0][0], model[0][1], model[0][2], 0.0],
        model_y: [model[1][0], model[1][1], model[1][2], 0.0],
        model_z: [model[2][0], model[2][1], model[2][2], 0.0],
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
    sim: vec4<u32>,
    model_x: vec4<f32>,
    model_y: vec4<f32>,
    model_z: vec4<f32>,
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
@group(1) @binding(0)
var<uniform> camera: Camera2D;

@compute @workgroup_size(64)
fn cs_main(@builtin(global_invocation_id) gid: vec3<u32>) {
    let water_idx = gid.y;
    if water_idx >= params.water_count {
        return;
    }
    let w = waters[water_idx];
    let local_idx = gid.x;
    if local_idx >= w.sim.y {
        return;
    }
    let cell_idx = w.sim.x + local_idx;
    let width = max(w.sim.z, 8u);
    let x_cell = local_idx % width;
    let y_cell = local_idx / width;
    let fx = f32(x_cell) / max(f32(width - 1u), 1.0);
    let fy = f32(y_cell) / max(f32(max(w.sim.w, 1u) - 1u), 1.0);
    let t = f32(params.frame_index) * 0.016;
    let idle = sin((fx + fy + w.flow_wind.z * 0.05) * 6.2831853 + t * w.wave.x) * w.wave.y;
    let prev = cells[cell_idx].x * w.wave.z;
    cells[cell_idx] = vec4<f32>(prev + idle * 0.015, idle, bitcast<f32>(w.flags.w), 1.0);
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
    let cell_idx = w.sim.x + u32(clamp(in.uv.x, 0.0, 1.0) * f32(max(w.sim.y - 1u, 1u)));
    let ripple = cells[cell_idx];
    let t = f32(params.frame_index) * 0.016;
    let idle = sin((in.uv.x + in.uv.y + t * w.wave.x) * 6.2831853) * 0.5 + 0.5;
    let foam = clamp(ripple.z + abs(ripple.x) * 0.35, 0.0, 1.0);
    let deep = vec3<f32>(0.02, 0.18, 0.30);
    let shallow = vec3<f32>(0.08, 0.46, 0.62);
    let color = mix(deep, shallow, idle * 0.35 + foam * 0.25);
    return vec4<f32>(mix(color, vec3<f32>(0.9, 0.97, 1.0), foam * 0.65), 0.72);
}
"#;

fn water_render_wgsl() -> String {
    WATER_WGSL
        .replace("var<storage, read_write> cells", "var<storage, read> cells")
        .replace(
            "cells[cell_idx] = vec4<f32>(prev + idle * 0.015, idle, bitcast<f32>(w.flags.w), 1.0);",
            "let render_only_unused = prev + idle;",
        )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn water_wgsl_parses() {
        naga::front::wgsl::parse_str(WATER_WGSL).expect("water wgsl should parse");
        let render_wgsl = water_render_wgsl();
        naga::front::wgsl::parse_str(&render_wgsl).expect("water render wgsl should parse");
    }

    #[test]
    fn water_lod_resolution_clamps_with_distance() {
        assert_eq!(
            water_lod_resolution([256, 256], [64.0, 64.0], [0.0, 0.0], [0.0, 0.0]),
            [256, 256]
        );
        assert_eq!(
            water_lod_resolution([256, 256], [64.0, 64.0], [512.0, 0.0], [0.0, 0.0]),
            [64, 64]
        );
        assert_eq!(
            water_lod_resolution([256, 256], [64.0, 64.0], [2048.0, 0.0], [0.0, 0.0]),
            [32, 32]
        );
    }
}
