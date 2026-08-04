use super::*;

/// Target world size of one render chunk. Chunk count derives from body size,
/// so a big body splits into enough chunks for per-chunk LOD to bite.
pub(super) const WATER_CHUNK_WORLD_TARGET: f32 = 12.0;
pub(super) const WATER_MAX_CHUNKS_PER_AXIS: u32 = 8;
/// Max LOD ratio btw neighbour chunks. Edge snapping stays exact at any power
/// of 2 ratio; this cap only keeps the degenerate edge slivers short.
pub(super) const WATER_CHUNK_MAX_LOD_RATIO: u32 = 4;
/// Distance floor for the projected-size math. Camera sitting on a body would
/// otherwise divide by ~0.
pub(super) const WATER_CHUNK_MIN_DISTANCE: f32 = 0.5;

/// Chunk grid for a body, derived from world size.
pub(super) fn water_chunk_counts(world_size: [f32; 2]) -> [u32; 2] {
    std::array::from_fn(|axis| {
        let extent = world_size[axis].abs().max(0.001);
        ((extent / WATER_CHUNK_WORLD_TARGET).ceil() as u32).clamp(1, WATER_MAX_CHUNKS_PER_AXIS)
    })
}

/// Surface quads per chunk axis to hit the tier's target triangle edge length
/// in PIXELS. Screen-space, so body size, camera distance, window size and
/// `render_scale` all fold into one number. Always a power of 2: neighbour LOD
/// ratios stay integer, which is what makes the edge snap crack-free.
pub(super) fn water_chunk_quads(
    quality: WaterQuality,
    chunk_extent: f32,
    distance: f32,
    lod_scale: [f32; 2],
) -> u32 {
    let max_quads = quality.max_chunk_quads().max(1);
    let focal_px = lod_scale[0].max(0.0);
    let ortho_px = lod_scale[1].max(0.0);
    if focal_px <= 0.0 && ortho_px <= 0.0 {
        // No projection info (headless / test): stay at the tier ceiling.
        return max_quads;
    }
    let px_per_world = if ortho_px > 0.0 {
        ortho_px
    } else {
        focal_px / distance.max(WATER_CHUNK_MIN_DISTANCE)
    };
    let target = quality.target_edge_pixels().max(1.0);
    let needed = chunk_extent.abs() * px_per_world / target;
    if !needed.is_finite() || needed <= 1.0 {
        return 1;
    }
    let raw = (needed.ceil() as u32).clamp(1, max_quads);
    raw.next_power_of_two().clamp(1, max_quads)
}

/// Raise any chunk more than [`WATER_CHUNK_MAX_LOD_RATIO`] coarser than a
/// neighbour. Keeps snapped edges from collapsing into long slivers.
pub(super) fn clamp_chunk_lod_ratio(quads: &mut [u32], counts: [u32; 2]) {
    let (nx, ny) = (counts[0] as usize, counts[1] as usize);
    for _ in 0..WATER_MAX_CHUNKS_PER_AXIS {
        let mut changed = false;
        for cy in 0..ny {
            for cx in 0..nx {
                let idx = cy * nx + cx;
                let mut want = quads[idx];
                let mut consider = |n: usize| {
                    want = want.max((quads[n] / WATER_CHUNK_MAX_LOD_RATIO).max(1));
                };
                if cx > 0 {
                    consider(idx - 1);
                }
                if cx + 1 < nx {
                    consider(idx + 1);
                }
                if cy > 0 {
                    consider(idx - nx);
                }
                if cy + 1 < ny {
                    consider(idx + nx);
                }
                let want = want.next_power_of_two();
                if want > quads[idx] {
                    quads[idx] = want;
                    changed = true;
                }
            }
        }
        if !changed {
            break;
        }
    }
}

/// Snap ratio for one edge: how many of this chunk's quad steps fit in one
/// neighbour step. 1 = same LOD (or finer neighbour, which snaps its own side).
fn edge_snap_ratio(self_quads: u32, neighbour: Option<u32>) -> u32 {
    match neighbour {
        Some(other) if other < self_quads => (self_quads / other.max(1)).clamp(1, 255),
        _ => 1,
    }
}

#[allow(clippy::too_many_arguments)]
pub(super) fn build_render_chunks_3d(
    out: &mut Vec<WaterRenderChunkGpu>,
    quads_scratch: &mut Vec<u32>,
    water_idx: u32,
    water: &Water3DState,
    gpu: WaterGpu,
    camera: [f32; 3],
    lod_scale: [f32; 2],
    planes: &[[f32; 4]; 6],
) {
    match water.shape {
        WaterShapeState::Circle { .. } | WaterShapeState::Cylinder { .. } => {
            if water_chunk_visible(gpu, [0.5, 0.5], [1.0, 1.0], planes) {
                out.push(WaterRenderChunkGpu {
                    water_idx,
                    quads: gpu.flags[0].max(2),
                    flags: WATER_CHUNK_FLAG_CIRCLE,
                    edge_snap: WATER_CHUNK_EDGE_SNAP_NONE,
                    chunk: [0, 0],
                    chunks: [1, 1],
                });
            }
        }
        WaterShapeState::Rect => {
            let scale_x = Vec3::new(gpu.model_x[0], gpu.model_x[1], gpu.model_x[2]).length();
            let scale_z = Vec3::new(gpu.model_z[0], gpu.model_z[1], gpu.model_z[2]).length();
            let size = [gpu.size_depth_time[0].abs(), gpu.size_depth_time[1].abs()];
            let world = [size[0] * scale_x, size[1] * scale_z];
            let counts = water_chunk_counts(world);
            let (nx, ny) = (counts[0], counts[1]);
            let chunk_extent = (world[0] / nx as f32).max(world[1] / ny as f32);
            // Conservative OBB radius: chunk half extents along both surface
            // axes, so a chunk the camera straddles reads distance ~0 and
            // saturates to the tier ceiling instead of under-tessellating.
            let radius =
                (size[0] * scale_x / (2.0 * nx as f32)) + (size[1] * scale_z / (2.0 * ny as f32));
            quads_scratch.clear();
            quads_scratch.resize((nx as usize) * (ny as usize), 1);
            for cy in 0..ny {
                for cx in 0..nx {
                    let local_x = ((cx as f32 + 0.5) / nx as f32 - 0.5) * gpu.size_depth_time[0];
                    let local_z = ((cy as f32 + 0.5) / ny as f32 - 0.5) * gpu.size_depth_time[1];
                    let center = [
                        gpu.model_w[0] + gpu.model_x[0] * local_x + gpu.model_z[0] * local_z,
                        gpu.model_w[1] + gpu.model_x[1] * local_x + gpu.model_z[1] * local_z,
                        gpu.model_w[2] + gpu.model_x[2] * local_x + gpu.model_z[2] * local_z,
                    ];
                    let dx = center[0] - camera[0];
                    let dy = center[1] - camera[1];
                    let dz = center[2] - camera[2];
                    let distance = ((dx * dx + dy * dy + dz * dz).sqrt() - radius).max(0.0);
                    quads_scratch[(cy * nx + cx) as usize] =
                        water_chunk_quads(water.quality, chunk_extent, distance, lod_scale);
                }
            }
            clamp_chunk_lod_ratio(quads_scratch, counts);
            let at = |cx: u32, cy: u32| quads_scratch[(cy * nx + cx) as usize];
            for cy in 0..ny {
                for cx in 0..nx {
                    let uv_origin = [cx as f32 / nx as f32, cy as f32 / ny as f32];
                    let uv_scale = [1.0 / nx as f32, 1.0 / ny as f32];
                    if !water_chunk_visible(gpu, uv_origin, uv_scale, planes) {
                        continue;
                    }
                    let quads = at(cx, cy);
                    // Snap ratios come from the full grid, not the visible set:
                    // a culled neighbour still defines the shared edge.
                    let neg_u = (cx > 0).then(|| at(cx - 1, cy));
                    let pos_u = (cx + 1 < nx).then(|| at(cx + 1, cy));
                    let neg_v = (cy > 0).then(|| at(cx, cy - 1));
                    let pos_v = (cy + 1 < ny).then(|| at(cx, cy + 1));
                    let edge_snap = edge_snap_ratio(quads, neg_u)
                        | edge_snap_ratio(quads, pos_u) << 8
                        | edge_snap_ratio(quads, neg_v) << 16
                        | edge_snap_ratio(quads, pos_v) << 24;
                    let mut flags = 0;
                    if neg_u.is_none() {
                        flags |= WATER_CHUNK_FLAG_EDGE_NEG_U;
                    }
                    if pos_u.is_none() {
                        flags |= WATER_CHUNK_FLAG_EDGE_POS_U;
                    }
                    if neg_v.is_none() {
                        flags |= WATER_CHUNK_FLAG_EDGE_NEG_V;
                    }
                    if pos_v.is_none() {
                        flags |= WATER_CHUNK_FLAG_EDGE_POS_V;
                    }
                    out.push(WaterRenderChunkGpu {
                        water_idx,
                        quads,
                        flags,
                        edge_snap,
                        chunk: [cx, cy],
                        chunks: [nx, ny],
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

/// Border edges this chunk draws a side wall for.
pub(super) fn water_chunk_side_edges(chunk: &WaterRenderChunkGpu) -> u32 {
    (chunk.flags & WATER_CHUNK_EDGE_MASK).count_ones()
}

pub(super) fn water_render_chunk_vertex_count(
    water: &WaterGpu,
    chunk: &WaterRenderChunkGpu,
) -> u32 {
    if chunk.flags & WATER_CHUNK_FLAG_CIRCLE != 0 {
        return water_3d_vertex_count(water);
    }
    let quads = chunk.quads.max(1);
    let surface = quads.saturating_mul(quads).saturating_mul(6);
    // Sides ride the border chunk's own edge tessellation, so the wall top edge
    // shares the surface border vertices exactly.
    let sides = water_chunk_side_edges(chunk)
        .saturating_mul(quads)
        .saturating_mul(6);
    surface.saturating_add(sides)
}

pub(super) fn water_render_chunk_distance_sq(
    water: &WaterGpu,
    chunk: &WaterRenderChunkGpu,
    camera: [f32; 3],
) -> f32 {
    let nx = chunk.chunks[0].max(1) as f32;
    let ny = chunk.chunks[1].max(1) as f32;
    let uv = [
        (chunk.chunk[0] as f32 + 0.5) / nx,
        (chunk.chunk[1] as f32 + 0.5) / ny,
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
