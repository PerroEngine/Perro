use bytemuck::{Pod, Zeroable};
use perro_ids::NodeID;
use perro_render_bridge::Water3DState;
use std::collections::HashMap;

const WORKGROUP: u32 = 64;
const MAX_PARTICLES_PER_WATER: u32 = 32_768;
const MAX_GRID_CELLS_PER_WATER: u32 = 65_536;

#[repr(C)]
#[derive(Clone, Copy, Pod, Zeroable)]
struct FlipWaterGpu {
    particle_offset_count: [u32; 4],
    grid_offset_dims_x: [u32; 4],
    dims_yz_pad: [u32; 4],
    size_depth_cell: [f32; 4],
    flow_splash: [f32; 4],
    splash_pos_radius: [f32; 4],
    deep_color: [f32; 4],
    shallow_color: [f32; 4],
    model_x: [f32; 4],
    model_y: [f32; 4],
    model_z: [f32; 4],
    model_w: [f32; 4],
}

#[repr(C)]
#[derive(Clone, Copy, Pod, Zeroable)]
struct FlipParamsGpu {
    water_count: u32,
    particle_count: u32,
    grid_count: u32,
    frame_seed: u32,
    delta_seconds: f32,
    gravity: f32,
    flip_ratio: f32,
    _pad: f32,
}

pub struct GpuWaterFlip {
    clear: wgpu::ComputePipeline,
    p2g: wgpu::ComputePipeline,
    grid: wgpu::ComputePipeline,
    g2p: wgpu::ComputePipeline,
    render: wgpu::RenderPipeline,
    compute_bgl: wgpu::BindGroupLayout,
    render_bgl: wgpu::BindGroupLayout,
    compute_bg: wgpu::BindGroup,
    render_bg: wgpu::BindGroup,
    waters: wgpu::Buffer,
    particles: wgpu::Buffer,
    accum: wgpu::Buffer,
    grid_velocity: wgpu::Buffer,
    params: wgpu::Buffer,
    water_capacity: usize,
    particle_capacity: usize,
    grid_capacity: usize,
    water_count: u32,
    particle_count: u32,
    grid_count: u32,
    max_particles_per_water: u32,
    max_grid_cells_per_water: u32,
    frame_seed: u32,
    staged: Vec<FlipWaterGpu>,
    impact_active: HashMap<NodeID, bool>,
    impact_epoch: HashMap<NodeID, u32>,
}

impl GpuWaterFlip {
    pub fn new(
        device: &wgpu::Device,
        color_format: wgpu::TextureFormat,
        sample_count: u32,
        camera_bgl: &wgpu::BindGroupLayout,
    ) -> Self {
        let compute_bgl = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("perro_water_flip_compute_bgl"),
            entries: &[
                storage_entry(0, true, wgpu::ShaderStages::COMPUTE),
                storage_entry(1, false, wgpu::ShaderStages::COMPUTE),
                storage_entry(2, false, wgpu::ShaderStages::COMPUTE),
                storage_entry(3, false, wgpu::ShaderStages::COMPUTE),
                uniform_entry(4, wgpu::ShaderStages::COMPUTE),
            ],
        });
        let render_bgl = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("perro_water_flip_render_bgl"),
            entries: &[
                storage_entry(0, true, wgpu::ShaderStages::VERTEX),
                storage_entry(1, true, wgpu::ShaderStages::VERTEX),
            ],
        });
        let shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("perro_water_flip_shader"),
            source: wgpu::ShaderSource::Wgsl(WATER_FLIP_WGSL.into()),
        });
        let render_shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("perro_water_flip_render_shader"),
            source: wgpu::ShaderSource::Wgsl(WATER_FLIP_RENDER_WGSL.into()),
        });
        let compute_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("perro_water_flip_compute_layout"),
            bind_group_layouts: &[Some(&compute_bgl)],
            immediate_size: 0,
        });
        let pipeline = |entry: &'static str| {
            device.create_compute_pipeline(&wgpu::ComputePipelineDescriptor {
                label: Some(entry),
                layout: Some(&compute_layout),
                module: &shader,
                entry_point: Some(entry),
                compilation_options: wgpu::PipelineCompilationOptions::default(),
                cache: None,
            })
        };
        let clear = pipeline("cs_clear_grid");
        let p2g = pipeline("cs_p2g");
        let grid = pipeline("cs_grid");
        let g2p = pipeline("cs_g2p");
        let render_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("perro_water_flip_render_layout"),
            bind_group_layouts: &[Some(&render_bgl), Some(camera_bgl)],
            immediate_size: 0,
        });
        let render = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("perro_water_flip_render"),
            layout: Some(&render_layout),
            vertex: wgpu::VertexState {
                module: &render_shader,
                entry_point: Some("vs_splash"),
                buffers: &[],
                compilation_options: wgpu::PipelineCompilationOptions::default(),
            },
            fragment: Some(wgpu::FragmentState {
                module: &render_shader,
                entry_point: Some("fs_splash"),
                targets: &[Some(wgpu::ColorTargetState {
                    format: color_format,
                    blend: Some(wgpu::BlendState::ALPHA_BLENDING),
                    write_mask: wgpu::ColorWrites::ALL,
                })],
                compilation_options: wgpu::PipelineCompilationOptions::default(),
            }),
            primitive: wgpu::PrimitiveState::default(),
            depth_stencil: Some(wgpu::DepthStencilState {
                format: crate::scene_depth_format(sample_count.max(1)),
                depth_write_enabled: Some(false),
                depth_compare: Some(wgpu::CompareFunction::LessEqual),
                stencil: wgpu::StencilState::default(),
                bias: wgpu::DepthBiasState::default(),
            }),
            multisample: wgpu::MultisampleState {
                count: sample_count.max(1),
                ..Default::default()
            },
            multiview_mask: None,
            cache: None,
        });
        let waters = storage_buffer(device, "perro_water_flip_waters", size_of::<FlipWaterGpu>());
        let particles = storage_buffer(device, "perro_water_flip_particles", 64);
        let accum = storage_buffer(device, "perro_water_flip_accum", 16);
        let grid_velocity = storage_buffer(device, "perro_water_flip_grid_velocity", 32);
        let params = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("perro_water_flip_params"),
            size: size_of::<FlipParamsGpu>() as u64,
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });
        let compute_bg = compute_bg(
            device,
            &compute_bgl,
            &waters,
            &particles,
            &accum,
            &grid_velocity,
            &params,
        );
        let render_bg = render_bg(device, &render_bgl, &waters, &particles);
        Self {
            clear,
            p2g,
            grid,
            g2p,
            render,
            compute_bgl,
            render_bgl,
            compute_bg,
            render_bg,
            waters,
            particles,
            accum,
            grid_velocity,
            params,
            water_capacity: 1,
            particle_capacity: 1,
            grid_capacity: 1,
            water_count: 0,
            particle_count: 0,
            grid_count: 0,
            max_particles_per_water: 0,
            max_grid_cells_per_water: 0,
            frame_seed: 0,
            staged: Vec::new(),
            impact_active: HashMap::new(),
            impact_epoch: HashMap::new(),
        }
    }

    pub fn prepare(
        &mut self,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        waters: &[(NodeID, Water3DState)],
        delta_seconds: f32,
    ) {
        self.staged.clear();
        let mut particle_offset = 0u32;
        let mut grid_offset = 0u32;
        self.max_particles_per_water = 0;
        self.max_grid_cells_per_water = 0;
        for (id, water) in waters.iter().filter(|(_, water)| !water.paused) {
            let (dims, particles) = flip_layout(water.resolution, water.depth);
            let cells = dims[0].saturating_mul(dims[1]).saturating_mul(dims[2]);
            self.max_particles_per_water = self.max_particles_per_water.max(particles);
            self.max_grid_cells_per_water = self.max_grid_cells_per_water.max(cells);
            let impact = strongest_impact(water);
            let active = impact.2 > 0.05;
            let was_active = self.impact_active.insert(*id, active).unwrap_or(false);
            let epoch = self.impact_epoch.entry(*id).or_insert(0);
            if active && !was_active {
                *epoch = epoch.wrapping_add(1).max(1);
            }
            self.staged.push(flip_water_gpu(
                water,
                particle_offset,
                particles,
                grid_offset,
                dims,
                impact,
                *epoch,
            ));
            particle_offset = particle_offset.saturating_add(particles);
            grid_offset = grid_offset.saturating_add(cells);
        }
        self.water_count = self.staged.len().min(u32::MAX as usize) as u32;
        self.particle_count = particle_offset;
        self.grid_count = grid_offset;
        if self.water_count == 0 {
            return;
        }
        if self.ensure_capacity(device) {
            self.compute_bg = compute_bg(
                device,
                &self.compute_bgl,
                &self.waters,
                &self.particles,
                &self.accum,
                &self.grid_velocity,
                &self.params,
            );
            self.render_bg = render_bg(device, &self.render_bgl, &self.waters, &self.particles);
        }
        queue.write_buffer(&self.waters, 0, bytemuck::cast_slice(&self.staged));
        self.frame_seed = self.frame_seed.wrapping_add(1);
        queue.write_buffer(
            &self.params,
            0,
            bytemuck::bytes_of(&FlipParamsGpu {
                water_count: self.water_count,
                particle_count: self.particle_count,
                grid_count: self.grid_count,
                frame_seed: self.frame_seed,
                delta_seconds: delta_seconds.clamp(0.0, 1.0 / 20.0),
                gravity: 9.81,
                flip_ratio: 0.94,
                _pad: 0.0,
            }),
        );
    }

    pub fn clear_active(&mut self) {
        self.water_count = 0;
        self.particle_count = 0;
        self.grid_count = 0;
        self.max_particles_per_water = 0;
        self.max_grid_cells_per_water = 0;
    }

    pub fn encode(&self, encoder: &mut wgpu::CommandEncoder) {
        if self.water_count == 0 {
            return;
        }
        let mut pass = encoder.begin_compute_pass(&wgpu::ComputePassDescriptor {
            label: Some("perro_water_flip_sim"),
            timestamp_writes: None,
        });
        pass.set_bind_group(0, &self.compute_bg, &[]);
        for (pipeline, count) in [
            (&self.clear, self.max_grid_cells_per_water),
            (&self.p2g, self.max_particles_per_water),
            (&self.grid, self.max_grid_cells_per_water),
            (&self.g2p, self.max_particles_per_water),
        ] {
            pass.set_pipeline(pipeline);
            pass.dispatch_workgroups(count.max(1).div_ceil(WORKGROUP), self.water_count, 1);
        }
    }

    pub fn render(
        &self,
        encoder: &mut wgpu::CommandEncoder,
        target: &wgpu::TextureView,
        depth: &wgpu::TextureView,
        camera: &wgpu::BindGroup,
    ) {
        if self.particle_count == 0 {
            return;
        }
        let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
            label: Some("perro_water_flip_splash_pass"),
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
                    load: wgpu::LoadOp::Load,
                    store: wgpu::StoreOp::Store,
                }),
                stencil_ops: None,
            }),
            timestamp_writes: None,
            occlusion_query_set: None,
            multiview_mask: None,
        });
        pass.set_pipeline(&self.render);
        pass.set_bind_group(0, &self.render_bg, &[]);
        pass.set_bind_group(1, camera, &[]);
        pass.draw(0..6, 0..self.particle_count);
    }

    fn ensure_capacity(&mut self, device: &wgpu::Device) -> bool {
        let mut changed = false;
        changed |= grow(
            device,
            &mut self.waters,
            &mut self.water_capacity,
            self.water_count as usize,
            size_of::<FlipWaterGpu>(),
            "perro_water_flip_waters",
        );
        changed |= grow(
            device,
            &mut self.particles,
            &mut self.particle_capacity,
            self.particle_count as usize,
            64,
            "perro_water_flip_particles",
        );
        changed |= grow(
            device,
            &mut self.accum,
            &mut self.grid_capacity,
            self.grid_count as usize,
            16,
            "perro_water_flip_accum",
        );
        if changed {
            self.grid_velocity = storage_buffer(
                device,
                "perro_water_flip_grid_velocity",
                self.grid_capacity.saturating_mul(32),
            );
        }
        changed
    }
}

fn flip_layout(resolution: [u32; 2], depth: f32) -> ([u32; 3], u32) {
    let x = resolution[0].clamp(8, 64);
    let z = resolution[1].clamp(8, 64);
    let y = ((x.min(z) as f32 * depth.max(0.25) / 16.0).round() as u32).clamp(4, 16);
    let mut dims = [x, y, z];
    while dims[0].saturating_mul(dims[1]).saturating_mul(dims[2]) > MAX_GRID_CELLS_PER_WATER {
        dims[0] = (dims[0] / 2).max(8);
        dims[2] = (dims[2] / 2).max(8);
    }
    let particles = dims[0]
        .saturating_mul(dims[2])
        .saturating_mul(3)
        .min(MAX_PARTICLES_PER_WATER);
    (dims, particles)
}

fn strongest_impact(water: &Water3DState) -> ([f32; 3], f32, f32) {
    water
        .impacts
        .iter()
        .max_by(|a, b| a.strength.total_cmp(&b.strength))
        .map(|i| {
            (
                i.position,
                i.radius.max(0.05),
                (i.strength + i.velocity[1].abs() * 0.1).max(0.0),
            )
        })
        .unwrap_or(([0.0; 3], 0.0, 0.0))
}

fn flip_water_gpu(
    w: &Water3DState,
    po: u32,
    pc: u32,
    go: u32,
    d: [u32; 3],
    impact: ([f32; 3], f32, f32),
    impact_epoch: u32,
) -> FlipWaterGpu {
    let cell = (w.size[0] / d[0] as f32).max(w.size[1] / d[2] as f32);
    FlipWaterGpu {
        particle_offset_count: [po, pc, impact_epoch, 0],
        grid_offset_dims_x: [go, d[0], 0, 0],
        dims_yz_pad: [d[1], d[2], 0, 0],
        size_depth_cell: [w.size[0], w.depth.max(0.01), w.size[1], cell],
        flow_splash: [w.flow[0], w.flow[1], impact.2, w.damping.clamp(0.0, 1.0)],
        splash_pos_radius: [impact.0[0], impact.0[1], impact.0[2], impact.1],
        deep_color: w.deep_color.into(),
        shallow_color: w.shallow_color.into(),
        model_x: w.model[0],
        model_y: w.model[1],
        model_z: w.model[2],
        model_w: w.model[3],
    }
}

fn storage_entry(
    binding: u32,
    read_only: bool,
    visibility: wgpu::ShaderStages,
) -> wgpu::BindGroupLayoutEntry {
    wgpu::BindGroupLayoutEntry {
        binding,
        visibility,
        ty: wgpu::BindingType::Buffer {
            ty: wgpu::BufferBindingType::Storage { read_only },
            has_dynamic_offset: false,
            min_binding_size: None,
        },
        count: None,
    }
}
fn uniform_entry(binding: u32, visibility: wgpu::ShaderStages) -> wgpu::BindGroupLayoutEntry {
    wgpu::BindGroupLayoutEntry {
        binding,
        visibility,
        ty: wgpu::BindingType::Buffer {
            ty: wgpu::BufferBindingType::Uniform,
            has_dynamic_offset: false,
            min_binding_size: None,
        },
        count: None,
    }
}
fn storage_buffer(device: &wgpu::Device, label: &'static str, bytes: usize) -> wgpu::Buffer {
    device.create_buffer(&wgpu::BufferDescriptor {
        label: Some(label),
        size: bytes.max(16) as u64,
        usage: wgpu::BufferUsages::STORAGE | wgpu::BufferUsages::COPY_DST,
        mapped_at_creation: false,
    })
}
fn grow(
    device: &wgpu::Device,
    buffer: &mut wgpu::Buffer,
    cap: &mut usize,
    needed: usize,
    stride: usize,
    label: &'static str,
) -> bool {
    if needed <= *cap {
        return false;
    }
    *cap = needed.next_power_of_two();
    *buffer = storage_buffer(device, label, cap.saturating_mul(stride));
    true
}
fn compute_bg(
    device: &wgpu::Device,
    layout: &wgpu::BindGroupLayout,
    w: &wgpu::Buffer,
    p: &wgpu::Buffer,
    a: &wgpu::Buffer,
    g: &wgpu::Buffer,
    params: &wgpu::Buffer,
) -> wgpu::BindGroup {
    device.create_bind_group(&wgpu::BindGroupDescriptor {
        label: Some("perro_water_flip_compute_bg"),
        layout,
        entries: &[
            wgpu::BindGroupEntry {
                binding: 0,
                resource: w.as_entire_binding(),
            },
            wgpu::BindGroupEntry {
                binding: 1,
                resource: p.as_entire_binding(),
            },
            wgpu::BindGroupEntry {
                binding: 2,
                resource: a.as_entire_binding(),
            },
            wgpu::BindGroupEntry {
                binding: 3,
                resource: g.as_entire_binding(),
            },
            wgpu::BindGroupEntry {
                binding: 4,
                resource: params.as_entire_binding(),
            },
        ],
    })
}
fn render_bg(
    device: &wgpu::Device,
    layout: &wgpu::BindGroupLayout,
    w: &wgpu::Buffer,
    p: &wgpu::Buffer,
) -> wgpu::BindGroup {
    device.create_bind_group(&wgpu::BindGroupDescriptor {
        label: Some("perro_water_flip_render_bg"),
        layout,
        entries: &[
            wgpu::BindGroupEntry {
                binding: 0,
                resource: w.as_entire_binding(),
            },
            wgpu::BindGroupEntry {
                binding: 1,
                resource: p.as_entire_binding(),
            },
        ],
    })
}
fn size_of<T>() -> usize {
    std::mem::size_of::<T>()
}

const WATER_FLIP_WGSL: &str = perro_macros::include_str_stripped!("water_shaders/water_flip.wgsl");
const WATER_FLIP_RENDER_WGSL: &str =
    perro_macros::include_str_stripped!("water_shaders/water_flip_render.wgsl");

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn flip_layout_caps_gpu_work() {
        let (dims, particles) = flip_layout([4096, 4096], 100.0);
        assert!(dims.into_iter().product::<u32>() <= MAX_GRID_CELLS_PER_WATER);
        assert!(particles <= MAX_PARTICLES_PER_WATER);
    }
    #[test]
    fn flip_layout_keeps_volume() {
        let (dims, particles) = flip_layout([24, 12], 4.0);
        assert!(dims[0] >= 8 && dims[1] >= 4 && dims[2] >= 8);
        assert!(particles >= dims[0] * dims[2]);
    }
    #[test]
    fn flip_shaders_validate() {
        for source in [WATER_FLIP_WGSL, WATER_FLIP_RENDER_WGSL] {
            let module = naga::front::wgsl::parse_str(source).expect("parse water FLIP WGSL");
            naga::valid::Validator::new(
                naga::valid::ValidationFlags::all(),
                naga::valid::Capabilities::all(),
            )
            .validate(&module)
            .expect("validate water FLIP WGSL");
        }
    }
}
