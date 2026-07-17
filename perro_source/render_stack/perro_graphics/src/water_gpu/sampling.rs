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

pub(super) fn water_lod_2d(water: &Water2DState, camera: [f32; 2]) -> WaterLodDecision {
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

pub(super) fn water_lod_3d(water: &Water3DState, camera: [f32; 3]) -> WaterLodDecision {
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
        WATER_3D_RENDER_LOD_STRENGTH,
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
pub(super) struct WaterLodDecision {
    pub(super) grid: WaterGridResolution,
    pub(super) ripple_blend: f32,
}

pub(super) fn water_lod(
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

pub(super) fn water_lod_from_distance(
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
