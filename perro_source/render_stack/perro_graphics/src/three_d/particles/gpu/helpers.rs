use super::*;

pub(super) fn emitter_emission_count(
    emitter: &PointParticles3DState,
    max_alive_budget: u32,
) -> u32 {
    if max_alive_budget == 0 {
        return 0;
    }
    let time = emitter.simulation_time.max(0.0);
    let spawned = (time * emitter.emission_rate.max(0.0)) as u32;
    if emitter.looping && emitter.prewarm {
        max_alive_budget
    } else {
        spawned.min(max_alive_budget)
    }
}

pub(super) fn gpu_path_params(path: &ParticlePath3D) -> Option<(u32, f32, f32)> {
    match path {
        ParticlePath3D::None => Some((0, 0.0, 0.0)),
        ParticlePath3D::Ballistic => Some((1, 0.0, 0.0)),
        ParticlePath3D::Spiral {
            angular_velocity,
            radius,
        } => Some((2, *angular_velocity, *radius)),
        ParticlePath3D::OrbitY {
            angular_velocity,
            radius,
        } => Some((3, *angular_velocity, *radius)),
        ParticlePath3D::NoiseDrift {
            amplitude,
            frequency,
        } => Some((4, *amplitude, *frequency)),
        ParticlePath3D::FlatDisk { radius } => Some((5, 0.0, *radius)),
        ParticlePath3D::Custom { .. } => None,
        ParticlePath3D::CustomCompiled { .. } => None,
    }
}

pub(super) fn append_gpu_ops(dst: &mut Vec<GpuExprOp>, ops: &[Op]) -> (u32, u32) {
    let offset = dst.len() as u32;
    for op in ops {
        dst.push(encode_gpu_op(op));
    }
    (offset, ops.len() as u32)
}

pub(super) fn encode_gpu_op(op: &Op) -> GpuExprOp {
    let (opcode, arg) = match op {
        Op::Const(v) => (0u32, v.to_bits()),
        Op::T => (1u32, 0u32),
        Op::Life => (2u32, 0u32),
        Op::Id => (3u32, 0u32),
        Op::Rand => (4u32, 0u32),
        Op::Rand2 => (5u32, 0u32),
        Op::Rand3 => (6u32, 0u32),
        Op::Param => (7u32, 0u32),
        Op::Add => (8u32, 0u32),
        Op::Sub => (9u32, 0u32),
        Op::Mul => (10u32, 0u32),
        Op::Div => (11u32, 0u32),
        Op::Pow => (12u32, 0u32),
        Op::Neg => (13u32, 0u32),
        Op::Sin => (14u32, 0u32),
        Op::Cos => (15u32, 0u32),
        Op::Tan => (16u32, 0u32),
        Op::Abs => (17u32, 0u32),
        Op::Sqrt => (18u32, 0u32),
        Op::Min => (19u32, 0u32),
        Op::Max => (20u32, 0u32),
        Op::Clamp => (21u32, 0u32),
        Op::Speed => (22u32, 0u32),
        Op::Lifetime => (23u32, 0u32),
        Op::AgeLeft => (24u32, 0u32),
        Op::Age01 => (25u32, 0u32),
        Op::SpawnTime => (26u32, 0u32),
        Op::EmitterTime => (27u32, 0u32),
        Op::DirX => (28u32, 0u32),
        Op::DirY => (29u32, 0u32),
        Op::DirZ => (30u32, 0u32),
        Op::VelX => (31u32, 0u32),
        Op::VelY => (32u32, 0u32),
        Op::VelZ => (33u32, 0u32),
        Op::Seed => (34u32, 0u32),
        Op::RingU => (35u32, 0u32),
        Op::Index01 => (36u32, 0u32),
        Op::EmitterX => (37u32, 0u32),
        Op::EmitterY => (38u32, 0u32),
        Op::EmitterZ => (39u32, 0u32),
        Op::PrevX => (40u32, 0u32),
        Op::PrevY => (41u32, 0u32),
        Op::PrevZ => (42u32, 0u32),
        Op::Hash => (43u32, 0u32),
    };
    GpuExprOp {
        words: [opcode, arg, 0, 0],
    }
}

pub(super) fn compute_view_proj(camera: &Camera3DState, width: u32, height: u32) -> Mat4 {
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

pub(super) fn push_instance_range(
    ranges: &mut Vec<InstanceRange>,
    start: u32,
    count: u32,
    path_kind: u32,
) {
    if count == 0 {
        return;
    }
    if let Some(last) = ranges.last_mut() {
        let last_end = last.start.saturating_add(last.count);
        if last_end == start && last.path_kind == path_kind {
            last.count = last.count.saturating_add(count);
            return;
        }
    }
    ranges.push(InstanceRange {
        start,
        count,
        path_kind,
    });
}

pub(super) fn gpu_compute_particles_enabled() -> bool {
    std::env::var("PERRO_ENABLE_GPU_COMPUTE_PARTICLES")
        .ok()
        .as_deref()
        .map(|v| matches!(v, "1" | "true" | "TRUE" | "on" | "ON"))
        .unwrap_or(true)
}

pub(super) fn append_emitter_map_entries(
    map: &mut Vec<u32>,
    emitter_index: u32,
    count: u32,
    fingerprint: &mut u64,
) {
    if count == 0 {
        return;
    }
    let old_len = map.len();
    map.resize(old_len + count as usize, emitter_index);
    for _ in 0..count {
        hash_u32(fingerprint, emitter_index);
    }
}

pub(super) fn write_spawn_origin_dirty_ranges(
    queue: &wgpu::Queue,
    buffer: &wgpu::Buffer,
    all_origins: &[[f32; 4]],
    dirty_slots: &mut Vec<u32>,
) {
    dirty_slots.sort_unstable();
    dirty_slots.dedup();
    let mut i = 0usize;
    while i < dirty_slots.len() {
        let start = dirty_slots[i];
        let mut end = start;
        i += 1;
        while i < dirty_slots.len() {
            let slot = dirty_slots[i];
            if slot == end.saturating_add(1) {
                end = slot;
                i += 1;
            } else {
                break;
            }
        }
        let start_idx = start as usize;
        let end_idx = end as usize + 1;
        let byte_offset = (start_idx * std::mem::size_of::<[f32; 4]>()) as u64;
        queue.write_buffer(
            buffer,
            byte_offset,
            bytemuck::cast_slice(&all_origins[start_idx..end_idx]),
        );
    }
    dirty_slots.clear();
}

#[inline]
pub(super) fn hash_u32(fingerprint: &mut u64, value: u32) {
    *fingerprint ^= value as u64;
    *fingerprint = fingerprint.wrapping_mul(0x0000_0100_0000_01B3);
}

pub(super) fn projection_matrix(projection: CameraProjectionState, aspect: f32) -> Mat4 {
    match projection {
        CameraProjectionState::Perspective {
            fov_y_degrees,
            near,
            far,
        } => {
            let fov_y_radians = adjusted_perspective_fov_y_radians(fov_y_degrees, aspect);
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

pub(super) const CAMERA_FOV_REFERENCE_ASPECT: f32 = 16.0 / 9.0;

pub(super) fn adjusted_perspective_fov_y_radians(fov_y_degrees: f32, aspect: f32) -> f32 {
    let base_fov_y_radians = if fov_y_degrees.is_finite() {
        fov_y_degrees
            .to_radians()
            .clamp(10.0f32.to_radians(), 120.0f32.to_radians())
    } else {
        60.0f32.to_radians()
    };
    let safe_aspect = aspect.max(1.0e-6);
    let base_aspect = CAMERA_FOV_REFERENCE_ASPECT.max(1.0e-6);
    let base_tan_y = (base_fov_y_radians * 0.5).tan().max(1.0e-6);
    let diagonal_tan = base_tan_y * (1.0 + base_aspect * base_aspect).sqrt();
    let adjusted_tan_y = diagonal_tan / (1.0 + safe_aspect * safe_aspect).sqrt();
    (2.0 * adjusted_tan_y.atan()).clamp(10.0f32.to_radians(), 120.0f32.to_radians())
}

#[inline]
pub(super) fn lerp4(a: [f32; 4], b: [f32; 4], t: f32) -> [f32; 4] {
    [
        a[0] + (b[0] - a[0]) * t,
        a[1] + (b[1] - a[1]) * t,
        a[2] + (b[2] - a[2]) * t,
        a[3] + (b[3] - a[3]) * t,
    ]
}

#[inline]
pub(super) fn hash01(seed: u32) -> f32 {
    let mut x = seed.wrapping_mul(747_796_405).wrapping_add(2_891_336_453);
    x = (x >> ((x >> 28) + 4)) ^ x;
    x = x.wrapping_mul(277_803_737);
    x = (x >> 22) ^ x;
    (x as f32) / (u32::MAX as f32)
}
