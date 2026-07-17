use super::*;

pub(super) fn build_render_chunks_3d(
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

pub(super) fn water_chunk_visible(
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

pub(super) fn water_render_chunk_vertex_count(
    water: &WaterGpu,
    chunk: &WaterRenderChunkGpu,
) -> u32 {
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

pub(super) fn water_render_chunk_distance_sq(
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
