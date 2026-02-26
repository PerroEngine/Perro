use super::shaders::create_point_particles_shader_module;
use bytemuck::{Pod, Zeroable};
use glam::{Mat4, Quat, Vec3};
use perro_ids::NodeID;
use perro_render_bridge::{
    Camera3DState, CameraProjectionState, ParticlePath3D, PointParticles3DState,
};

#[repr(C)]
#[derive(Clone, Copy, Pod, Zeroable)]
struct CameraUniform {
    view_proj: [[f32; 4]; 4],
}

#[repr(C)]
#[derive(Clone, Copy, Pod, Zeroable)]
struct PointParticleGpu {
    world_pos: [f32; 3],
    size_alpha: [f32; 2],
    color: [f32; 4],
    emissive: [f32; 3],
}

pub struct PreparePointParticles3D<'a> {
    pub camera: Camera3DState,
    pub emitters: &'a [(NodeID, PointParticles3DState)],
    pub width: u32,
    pub height: u32,
}

pub struct GpuPointParticles3D {
    pipeline: wgpu::RenderPipeline,
    camera_buffer: wgpu::Buffer,
    camera_bg: wgpu::BindGroup,
    particle_buffer: wgpu::Buffer,
    particle_capacity: usize,
    staged: Vec<PointParticleGpu>,
}

impl GpuPointParticles3D {
    pub fn new(
        device: &wgpu::Device,
        color_format: wgpu::TextureFormat,
        sample_count: u32,
    ) -> Self {
        let camera_bgl = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("perro_particles3d_camera_bgl"),
            entries: &[wgpu::BindGroupLayoutEntry {
                binding: 0,
                visibility: wgpu::ShaderStages::VERTEX,
                ty: wgpu::BindingType::Buffer {
                    ty: wgpu::BufferBindingType::Uniform,
                    has_dynamic_offset: false,
                    min_binding_size: Some(
                        std::num::NonZeroU64::new(std::mem::size_of::<CameraUniform>() as u64)
                            .expect("camera uniform size must be non-zero"),
                    ),
                },
                count: None,
            }],
        });
        let camera_buffer = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("perro_particles3d_camera_buffer"),
            size: std::mem::size_of::<CameraUniform>() as u64,
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });
        let camera_bg = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("perro_particles3d_camera_bg"),
            layout: &camera_bgl,
            entries: &[wgpu::BindGroupEntry {
                binding: 0,
                resource: camera_buffer.as_entire_binding(),
            }],
        });
        let shader = create_point_particles_shader_module(device);
        let layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("perro_particles3d_pipeline_layout"),
            bind_group_layouts: &[&camera_bgl],
            immediate_size: 0,
        });
        let pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("perro_particles3d_pipeline"),
            layout: Some(&layout),
            vertex: wgpu::VertexState {
                module: &shader,
                entry_point: Some("vs_main"),
                buffers: &[wgpu::VertexBufferLayout {
                    array_stride: std::mem::size_of::<PointParticleGpu>() as u64,
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
                            format: wgpu::VertexFormat::Float32x2,
                        },
                        wgpu::VertexAttribute {
                            offset: 20,
                            shader_location: 2,
                            format: wgpu::VertexFormat::Float32x4,
                        },
                        wgpu::VertexAttribute {
                            offset: 36,
                            shader_location: 3,
                            format: wgpu::VertexFormat::Float32x3,
                        },
                    ],
                }],
                compilation_options: Default::default(),
            },
            fragment: Some(wgpu::FragmentState {
                module: &shader,
                entry_point: Some("fs_main"),
                targets: &[Some(wgpu::ColorTargetState {
                    format: color_format,
                    blend: Some(wgpu::BlendState::ALPHA_BLENDING),
                    write_mask: wgpu::ColorWrites::ALL,
                })],
                compilation_options: Default::default(),
            }),
            primitive: wgpu::PrimitiveState {
                topology: wgpu::PrimitiveTopology::PointList,
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
        });
        let particle_capacity = 1024usize;
        let particle_buffer = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("perro_particles3d_points"),
            size: (particle_capacity * std::mem::size_of::<PointParticleGpu>()) as u64,
            usage: wgpu::BufferUsages::VERTEX | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });
        Self {
            pipeline,
            camera_buffer,
            camera_bg,
            particle_buffer,
            particle_capacity,
            staged: Vec::new(),
        }
    }

    pub fn set_sample_count(
        &mut self,
        device: &wgpu::Device,
        color_format: wgpu::TextureFormat,
        sample_count: u32,
    ) {
        *self = Self::new(device, color_format, sample_count);
    }

    pub fn prepare(
        &mut self,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        frame: PreparePointParticles3D<'_>,
    ) {
        self.staged.clear();
        for (_, emitter) in frame.emitters {
            self.push_emitter_particles(emitter.clone());
        }
        if self.staged.is_empty() {
            return;
        }
        self.ensure_particle_capacity(device, self.staged.len());
        queue.write_buffer(&self.particle_buffer, 0, bytemuck::cast_slice(&self.staged));

        let uniform = CameraUniform {
            view_proj: compute_view_proj(frame.camera, frame.width, frame.height)
                .to_cols_array_2d(),
        };
        queue.write_buffer(&self.camera_buffer, 0, bytemuck::bytes_of(&uniform));
    }

    pub fn render_pass(&self, encoder: &mut wgpu::CommandEncoder, color_view: &wgpu::TextureView) {
        if self.staged.is_empty() {
            return;
        }
        let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
            label: Some("perro_particles3d_pass"),
            color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                view: color_view,
                resolve_target: None,
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
        pass.set_pipeline(&self.pipeline);
        pass.set_bind_group(0, &self.camera_bg, &[]);
        pass.set_vertex_buffer(0, self.particle_buffer.slice(..));
        pass.draw(0..self.staged.len() as u32, 0..1);
    }

    fn ensure_particle_capacity(&mut self, device: &wgpu::Device, needed: usize) {
        if needed <= self.particle_capacity {
            return;
        }
        let mut new_capacity = self.particle_capacity.max(1);
        while new_capacity < needed {
            new_capacity *= 2;
        }
        self.particle_buffer = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("perro_particles3d_points"),
            size: (new_capacity * std::mem::size_of::<PointParticleGpu>()) as u64,
            usage: wgpu::BufferUsages::VERTEX | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });
        self.particle_capacity = new_capacity;
    }

    fn push_emitter_particles(&mut self, emitter: PointParticles3DState) {
        if !emitter.active || emitter.max_particles == 0 || emitter.emission_rate <= 0.0 {
            return;
        }
        let model = Mat4::from_cols_array_2d(&emitter.model);
        let up = model.transform_vector3(Vec3::Y).normalize_or_zero();
        let origin = model.transform_point3(Vec3::ZERO);
        let time = emitter.simulation_time.max(0.0);
        let mut emit_count = (time * emitter.emission_rate) as u32;
        if !emitter.looping && emitter.duration > 0.0 {
            emit_count = ((time.min(emitter.duration)) * emitter.emission_rate) as u32;
        }
        emit_count = emit_count.min(emitter.max_particles.max(1));

        let life_min = emitter.lifetime_min.max(0.001);
        let life_max = emitter.lifetime_max.max(life_min);
        let speed_min = emitter.speed_min.max(0.0);
        let speed_max = emitter.speed_max.max(speed_min);
        let size_min = emitter.size_min.max(0.01);
        let size_max = emitter.size_max.max(size_min);

        for i in 0..emit_count {
            let h0 = hash01(emitter.seed ^ i);
            let h1 = hash01(emitter.seed.wrapping_add(0x9E37_79B9) ^ i.wrapping_mul(3));
            let h2 = hash01(emitter.seed.wrapping_add(0x7F4A_7C15) ^ i.wrapping_mul(7));
            let life = life_min + (life_max - life_min) * h0;
            let spawn_t = if emitter.prewarm {
                (i as f32) / emitter.emission_rate.max(1.0e-6)
            } else {
                (i as f32) / emitter.emission_rate.max(1.0e-6)
            };
            let local_t = if emitter.looping && emitter.duration > 0.0 {
                (time - spawn_t).rem_euclid(emitter.duration.max(1.0e-6))
            } else {
                time - spawn_t
            };
            if !(0.0..=life).contains(&local_t) {
                continue;
            }
            let age = (local_t / life).clamp(0.0, 1.0);
            let speed = speed_min + (speed_max - speed_min) * h1;
            let spread = emitter.spread_radians * (h2 * 2.0 - 1.0);
            let yaw = (h0 * std::f32::consts::TAU).sin_cos();
            let mut dir = Vec3::new(yaw.0, 1.0, yaw.1);
            dir = (Quat::from_axis_angle(Vec3::X, spread) * dir).normalize_or_zero();
            let mut pos = origin + up * 0.0 + dir * speed * local_t;
            let gravity = Vec3::from_array(emitter.gravity);
            pos += 0.5 * gravity * local_t * local_t;
            match emitter.profile.path {
                ParticlePath3D::Ballistic => {}
                ParticlePath3D::Spiral {
                    angular_velocity,
                    radius,
                } => {
                    let theta = local_t * angular_velocity + h0 * std::f32::consts::TAU;
                    pos += Vec3::new(theta.cos() * radius, 0.0, theta.sin() * radius);
                }
                ParticlePath3D::OrbitY {
                    angular_velocity,
                    radius,
                } => {
                    let theta = local_t * angular_velocity + h1 * std::f32::consts::TAU;
                    pos = origin
                        + Vec3::new(theta.cos() * radius, pos.y - origin.y, theta.sin() * radius);
                }
                ParticlePath3D::NoiseDrift {
                    amplitude,
                    frequency,
                } => {
                    let n = (local_t * frequency + h2 * 37.0).sin();
                    let m = (local_t * frequency * 1.37 + h1 * 17.0).cos();
                    pos += Vec3::new(n, m, n * m) * amplitude;
                }
                ParticlePath3D::Custom {
                    ref expr_x,
                    ref expr_y,
                    ref expr_z,
                } => {
                    let dx = eval_custom_expr(expr_x, age, local_t, &emitter.params).unwrap_or(0.0);
                    let dy = eval_custom_expr(expr_y, age, local_t, &emitter.params).unwrap_or(0.0);
                    let dz = eval_custom_expr(expr_z, age, local_t, &emitter.params).unwrap_or(0.0);
                    pos += Vec3::new(dx, dy, dz);
                }
            }
            let size = emitter.point_size * (size_min + (size_max - size_min) * h2);
            let color = lerp4(emitter.color_start, emitter.color_end, age);
            self.staged.push(PointParticleGpu {
                world_pos: pos.to_array(),
                size_alpha: [size, color[3]],
                color,
                emissive: emitter.emissive,
            });
        }
    }
}

fn compute_view_proj(camera: Camera3DState, width: u32, height: u32) -> Mat4 {
    let w = width.max(1) as f32;
    let h = height.max(1) as f32;
    let aspect = w / h;
    let proj = projection_matrix(camera.projection, aspect);
    let pos = Vec3::from_array(camera.position);
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
    proj * world.inverse()
}

fn projection_matrix(projection: CameraProjectionState, aspect: f32) -> Mat4 {
    match projection {
        CameraProjectionState::Perspective {
            fov_y_degrees,
            near,
            far,
        } => {
            let fov_y_radians = fov_y_degrees
                .to_radians()
                .clamp(10.0f32.to_radians(), 120.0f32.to_radians());
            Mat4::perspective_rh_gl(
                fov_y_radians,
                aspect.max(1.0e-6),
                near.max(1.0e-3),
                far.max(near + 1.0e-3),
            )
        }
        CameraProjectionState::Orthographic { size, near, far } => {
            let half_h = (size.abs() * 0.5).max(1.0e-3);
            let half_w = half_h * aspect.max(1.0e-6);
            Mat4::orthographic_rh(
                -half_w,
                half_w,
                -half_h,
                half_h,
                near.max(1.0e-3),
                far.max(near + 1.0e-3),
            )
        }
        CameraProjectionState::Frustum {
            left,
            right,
            bottom,
            top,
            near,
            far,
        } => Mat4::frustum_rh_gl(
            left,
            right,
            bottom,
            top,
            near.max(1.0e-3),
            far.max(near + 1.0e-3),
        ),
    }
}

#[inline]
fn lerp4(a: [f32; 4], b: [f32; 4], t: f32) -> [f32; 4] {
    [
        a[0] + (b[0] - a[0]) * t,
        a[1] + (b[1] - a[1]) * t,
        a[2] + (b[2] - a[2]) * t,
        a[3] + (b[3] - a[3]) * t,
    ]
}

#[inline]
fn hash01(seed: u32) -> f32 {
    let mut x = seed.wrapping_mul(747_796_405).wrapping_add(2_891_336_453);
    x = (x >> ((x >> 28) + 4)) ^ x;
    x = x.wrapping_mul(277_803_737);
    x = (x >> 22) ^ x;
    (x as f32) / (u32::MAX as f32)
}

fn eval_custom_expr(expr: &str, t: f32, life: f32, params: &[f32]) -> Option<f32> {
    let mut p = ExprParser {
        s: expr.as_bytes(),
        i: 0,
        t,
        life,
        params,
    };
    let out = p.parse_expr()?;
    p.skip_ws();
    (p.i == p.s.len()).then_some(out)
}

struct ExprParser<'a> {
    s: &'a [u8],
    i: usize,
    t: f32,
    life: f32,
    params: &'a [f32],
}

impl<'a> ExprParser<'a> {
    fn skip_ws(&mut self) {
        while self.i < self.s.len() && self.s[self.i].is_ascii_whitespace() {
            self.i += 1;
        }
    }

    fn parse_expr(&mut self) -> Option<f32> {
        let mut lhs = self.parse_term()?;
        loop {
            self.skip_ws();
            if self.eat(b'+') {
                lhs += self.parse_term()?;
            } else if self.eat(b'-') {
                lhs -= self.parse_term()?;
            } else {
                break;
            }
        }
        Some(lhs)
    }

    fn parse_term(&mut self) -> Option<f32> {
        let mut lhs = self.parse_power()?;
        loop {
            self.skip_ws();
            if self.eat(b'*') {
                lhs *= self.parse_power()?;
            } else if self.eat(b'/') {
                lhs /= self.parse_power()?;
            } else {
                break;
            }
        }
        Some(lhs)
    }

    fn parse_power(&mut self) -> Option<f32> {
        let mut lhs = self.parse_unary()?;
        self.skip_ws();
        while self.eat(b'^') {
            let rhs = self.parse_unary()?;
            lhs = lhs.powf(rhs);
            self.skip_ws();
        }
        Some(lhs)
    }

    fn parse_unary(&mut self) -> Option<f32> {
        self.skip_ws();
        if self.eat(b'+') {
            return self.parse_unary();
        }
        if self.eat(b'-') {
            return Some(-self.parse_unary()?);
        }
        self.parse_primary()
    }

    fn parse_primary(&mut self) -> Option<f32> {
        self.skip_ws();
        if self.eat(b'(') {
            let v = self.parse_expr()?;
            self.skip_ws();
            self.expect(b')')?;
            return Some(v);
        }
        if let Some(n) = self.parse_number() {
            return Some(n);
        }
        let ident = self.parse_ident()?;
        self.skip_ws();
        if ident == "params" && self.eat(b'[') {
            let idx = self.parse_expr()?.floor() as isize;
            self.skip_ws();
            self.expect(b']')?;
            if idx < 0 {
                return Some(0.0);
            }
            return Some(*self.params.get(idx as usize).unwrap_or(&0.0));
        }
        if self.eat(b'(') {
            let args = self.parse_args()?;
            return eval_func(ident.as_str(), &args);
        }
        match ident.as_str() {
            "t" => Some(self.t),
            "life" => Some(self.life),
            "pi" => Some(std::f32::consts::PI),
            _ => None,
        }
    }

    fn parse_args(&mut self) -> Option<Vec<f32>> {
        let mut args = Vec::new();
        self.skip_ws();
        if self.eat(b')') {
            return Some(args);
        }
        loop {
            args.push(self.parse_expr()?);
            self.skip_ws();
            if self.eat(b',') {
                continue;
            }
            self.expect(b')')?;
            break;
        }
        Some(args)
    }

    fn parse_ident(&mut self) -> Option<String> {
        self.skip_ws();
        let start = self.i;
        while self.i < self.s.len()
            && (self.s[self.i].is_ascii_alphanumeric() || self.s[self.i] == b'_')
        {
            self.i += 1;
        }
        (self.i > start).then(|| String::from_utf8_lossy(&self.s[start..self.i]).to_string())
    }

    fn parse_number(&mut self) -> Option<f32> {
        self.skip_ws();
        let start = self.i;
        let mut seen = false;
        while self.i < self.s.len() {
            let c = self.s[self.i];
            if c.is_ascii_digit() || c == b'.' {
                seen = true;
                self.i += 1;
            } else {
                break;
            }
        }
        if !seen {
            self.i = start;
            return None;
        }
        let s = std::str::from_utf8(&self.s[start..self.i]).ok()?;
        s.parse::<f32>().ok()
    }

    fn eat(&mut self, c: u8) -> bool {
        self.skip_ws();
        if self.i < self.s.len() && self.s[self.i] == c {
            self.i += 1;
            true
        } else {
            false
        }
    }

    fn expect(&mut self, c: u8) -> Option<()> {
        self.eat(c).then_some(())
    }
}

fn eval_func(name: &str, args: &[f32]) -> Option<f32> {
    match name {
        "sin" if args.len() == 1 => Some(args[0].sin()),
        "cos" if args.len() == 1 => Some(args[0].cos()),
        "tan" if args.len() == 1 => Some(args[0].tan()),
        "abs" if args.len() == 1 => Some(args[0].abs()),
        "sqrt" if args.len() == 1 => Some(args[0].max(0.0).sqrt()),
        "min" if args.len() == 2 => Some(args[0].min(args[1])),
        "max" if args.len() == 2 => Some(args[0].max(args[1])),
        "clamp" if args.len() == 3 => Some(args[0].clamp(args[1], args[2])),
        _ => None,
    }
}
