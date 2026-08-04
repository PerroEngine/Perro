use super::*;

pub(super) fn water_cell_count(resolution: [u32; 2]) -> usize {
    if resolution[0] == 0 || resolution[1] == 0 {
        return 0;
    }
    let x = resolution[0].clamp(1, 256) as usize;
    let y = resolution[1].clamp(1, 256) as usize;
    x.saturating_mul(y)
}

pub(super) fn water_center_cell_offset(water: &WaterGpu) -> usize {
    let width = water.sim[2].max(1);
    let height = water.sim[3].max(1);
    let center = (height / 2).saturating_mul(width).saturating_add(width / 2);
    water.sim[0].saturating_add(center.min(water.sim[1].saturating_sub(1))) as usize
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub(super) struct WaterQuerySampleOffsets {
    pub(super) offsets: [usize; 4],
    pub(super) frac: [f32; 2],
}

pub(super) fn water_query_sample_offsets(
    water: &WaterGpu,
    local: [f32; 2],
) -> WaterQuerySampleOffsets {
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

pub(super) fn water_query_offset_from_xy(water: &WaterGpu, width: u32, x: u32, y: u32) -> usize {
    let cell = y
        .saturating_mul(width)
        .saturating_add(x)
        .min(water.sim[1].saturating_sub(1));
    water.sim[0].saturating_add(cell) as usize
}

pub(super) fn water_lerp_cell(
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

pub(super) fn water_3d_vertex_count(water: &WaterGpu) -> u32 {
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

pub(super) fn water_3d_side_vertex_count(water: &WaterGpu) -> u32 {
    let width = water.flags[0].clamp(1, WATER_MAX_RENDER_RESOLUTION);
    let height = water.flags[1].clamp(1, WATER_MAX_RENDER_RESOLUTION);
    width
        .saturating_sub(1)
        .saturating_add(height.saturating_sub(1))
        .saturating_mul(2)
        .saturating_mul(6)
}

pub(super) fn water_lod_2d(water: &Water2DState) -> WaterLodDecision {
    // 2D water rasterizes as one screen quad, so only the sim grid matters.
    let sim = water.quality.sim_resolution();
    WaterLodDecision {
        grid: WaterGridResolution { sim, render: sim },
        ripple_blend: 1.0,
    }
}

pub(super) fn water_lod_3d(
    water: &Water3DState,
    camera: [f32; 3],
    projection_scale: [f32; 2],
) -> WaterLodDecision {
    let sim = water.quality.sim_resolution();
    let render = water_body_reference_resolution(water, camera, projection_scale);
    WaterLodDecision {
        grid: WaterGridResolution { sim, render },
        ripple_blend: water_ripple_blend(water, camera, projection_scale, sim),
    }
}

/// Body-level reference mesh resolution. Rect bodies tessellate per chunk and
/// ignore this; the circle/cylinder path still needs one segment count, and it
/// rides the same screen-space rule.
pub(super) fn water_body_reference_resolution(
    water: &Water3DState,
    camera: [f32; 3],
    projection_scale: [f32; 2],
) -> [u32; 2] {
    let ceiling = (water.quality.max_chunk_quads() * WATER_MAX_CHUNKS_PER_AXIS)
        .clamp(2, WATER_3D_MAX_RENDER_RESOLUTION);
    let px_per_world = water_px_per_world(water, camera, projection_scale);
    let Some(px_per_world) = px_per_world else {
        return [ceiling, ceiling];
    };
    let scale_x = Vec3::new(water.model[0][0], water.model[0][1], water.model[0][2])
        .length()
        .max(1.0e-6);
    let scale_z = Vec3::new(water.model[2][0], water.model[2][1], water.model[2][2])
        .length()
        .max(1.0e-6);
    let world_axes = [water.size[0].abs() * scale_x, water.size[1].abs() * scale_z];
    let target = water.quality.target_edge_pixels().max(1.0);
    std::array::from_fn(|axis| {
        let segments = (world_axes[axis] * px_per_world / target).ceil();
        if !segments.is_finite() {
            return ceiling;
        }
        (segments as u32).clamp(2, ceiling)
    })
}

/// Pixels per world unit at the body's nearest surface point. `None` = no
/// projection info (headless / test) -> callers stay at the tier ceiling.
fn water_px_per_world(
    water: &Water3DState,
    camera: [f32; 3],
    projection_scale: [f32; 2],
) -> Option<f32> {
    let focal_px = projection_scale[0].max(0.0);
    let ortho_px = projection_scale[1].max(0.0);
    if ortho_px > 0.0 {
        return Some(ortho_px);
    }
    if focal_px <= 0.0 {
        return None;
    }
    let pos = water.model[3];
    let radius = water_lod_shape_radius(water.shape, water.size);
    let distance = water_lod_surface_distance([pos[0], pos[2]], [camera[0], camera[2]], radius)
        .max(WATER_CHUNK_MIN_DISTANCE);
    Some(focal_px / distance)
}

/// Fade sim ripples out once a sim cell stops covering a pixel. Same
/// screen-space rule as tessellation, so it tracks window size + render scale.
fn water_ripple_blend(
    water: &Water3DState,
    camera: [f32; 3],
    projection_scale: [f32; 2],
    sim: [u32; 2],
) -> f32 {
    let Some(px_per_world) = water_px_per_world(water, camera, projection_scale) else {
        return 1.0;
    };
    let scale_x = Vec3::new(water.model[0][0], water.model[0][1], water.model[0][2])
        .length()
        .max(1.0e-6);
    let scale_z = Vec3::new(water.model[2][0], water.model[2][1], water.model[2][2])
        .length()
        .max(1.0e-6);
    let cell_world = (water.size[0].abs() * scale_x / sim[0].max(1) as f32)
        .min(water.size[1].abs() * scale_z / sim[1].max(1) as f32);
    let cell_px = cell_world * px_per_world;
    if !cell_px.is_finite() {
        return 1.0;
    }
    smooth01(((cell_px - 0.5) / 2.5).clamp(0.0, 1.0))
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub(super) struct WaterLodDecision {
    pub(super) grid: WaterGridResolution,
    pub(super) ripple_blend: f32,
}

pub(super) fn smooth01(t: f32) -> f32 {
    t * t * (3.0 - 2.0 * t)
}

pub(super) fn water_lod_shape_radius(shape: WaterShapeState, size: [f32; 2]) -> f32 {
    match shape {
        WaterShapeState::Rect => size[0].max(size[1]) * 0.5,
        WaterShapeState::Circle { radius } | WaterShapeState::Cylinder { radius, .. } => radius,
    }
}

pub(super) fn water_lod_surface_distance(
    water_pos: [f32; 2],
    camera_pos: [f32; 2],
    radius: f32,
) -> f32 {
    let dx = water_pos[0] - camera_pos[0];
    let dz = water_pos[1] - camera_pos[1];
    ((dx * dx + dz * dz).sqrt() - radius.max(0.0)).max(0.0)
}
