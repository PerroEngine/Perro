use super::*;

pub(super) fn raster_coastline_2d(
    out: &mut [[f32; 4]],
    resolution: [u32; 2],
    water: &Water2DState,
    node: NodeID,
    cache: &mut HashMap<NodeID, CachedCoastline>,
) {
    let width = resolution[0].clamp(1, 256) as usize;
    let height = resolution[1].clamp(1, 256) as usize;
    if water.coastline_shapes.is_empty() {
        cache.remove(&node);
        raster_impacts_2d(out, width, height, water);
        return;
    }
    let foam_width = water.coastline_foam_width.max(0.001);
    let softness = water.coastline_cutoff_softness.max(0.001);

    let mut hasher = coastline_hasher();
    width.hash(&mut hasher);
    height.hash(&mut hasher);
    water.size[0].to_bits().hash(&mut hasher);
    water.size[1].to_bits().hash(&mut hasher);
    foam_width.to_bits().hash(&mut hasher);
    softness.to_bits().hash(&mut hasher);
    water.coastline_shapes.len().hash(&mut hasher);
    for shape in water.coastline_shapes.iter() {
        hash_coastline_shape_2d(shape, &mut hasher);
    }
    let signature = hasher.finish();

    let cell_count = width * height;
    let entry = cache.entry(node).or_insert_with(|| CachedCoastline {
        signature,
        base: Vec::new(),
    });
    // Rebuild the static field only when the shapes/params/grid changed.
    if entry.signature != signature || entry.base.len() != cell_count {
        entry.signature = signature;
        entry.base.clear();
        entry.base.reserve(cell_count);
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
                let (solid, foam_edge, spill_energy) =
                    coastline_fill(signed_min, foam_width, softness);
                entry.base.push([solid, edge.max(foam_edge), spill_energy]);
            }
        }
    }
    let base = &entry.base;

    // Blend the frame-varying impacts wake over the cached static field.
    for y in 0..height {
        for x in 0..width {
            let i = y * width + x;
            let [solid, edge_foam, spill_energy] = base[i];
            let fx = x as f32 / (width.saturating_sub(1).max(1) as f32);
            let fy = y as f32 / (height.saturating_sub(1).max(1) as f32);
            let p = [(fx - 0.5) * water.size[0], (fy - 0.5) * water.size[1]];
            let mut wake = 0.0f32;
            for impact in water.impacts.iter() {
                let dx = p[0] - impact.position[0];
                let dy = p[1] - impact.position[1];
                let radius = impact.radius.max(0.001);
                let t = ((dx * dx + dy * dy) / (radius * radius).max(0.000001)).clamp(0.0, 1.0);
                let push = 1.0 - t;
                let ring = (1.0 - ((t - 0.72).abs() / 0.28).clamp(0.0, 1.0)).powi(2);
                let strength = impact.strength * (1.0 / 180.0);
                // displaced water: crater under the body, crown on the ring
                wake += ring * (strength * 0.30 + impact.cavitation * 0.92)
                    - push * (strength * 0.70 + impact.cavitation * 0.28);
            }
            let wake = wake.clamp(-1.0, 1.0);
            out[i] = [solid, edge_foam, wake, spill_energy.max(wake.abs())];
        }
    }
}

pub(super) fn coastline_hasher() -> ahash::AHasher {
    ahash::RandomState::with_seeds(0xc0a5_0001, 0xc0a5_0002, 0xc0a5_0003, 0xc0a5_0004)
        .build_hasher()
}

pub(super) fn hash_coastline_shape_2d(shape: &WaterCoastlineShape2D, hasher: &mut ahash::AHasher) {
    match shape {
        WaterCoastlineShape2D::Quad {
            center,
            half_extents,
            rotation,
        } => {
            0u8.hash(hasher);
            hash_f32_slice(center, hasher);
            hash_f32_slice(half_extents, hasher);
            rotation.to_bits().hash(hasher);
        }
        WaterCoastlineShape2D::Circle { center, radius } => {
            1u8.hash(hasher);
            hash_f32_slice(center, hasher);
            radius.to_bits().hash(hasher);
        }
        WaterCoastlineShape2D::Triangle { points } => {
            2u8.hash(hasher);
            for point in points {
                hash_f32_slice(point, hasher);
            }
        }
    }
}

pub(super) fn hash_coastline_shape_3d(shape: &WaterCoastlineShape3D, hasher: &mut ahash::AHasher) {
    match shape {
        WaterCoastlineShape3D::Box {
            center,
            half_extents,
            axis_x,
            axis_z,
        } => {
            0u8.hash(hasher);
            hash_f32_slice(center, hasher);
            hash_f32_slice(half_extents, hasher);
            hash_f32_slice(axis_x, hasher);
            hash_f32_slice(axis_z, hasher);
        }
        WaterCoastlineShape3D::Sphere { center, radius } => {
            1u8.hash(hasher);
            hash_f32_slice(center, hasher);
            radius.to_bits().hash(hasher);
        }
        WaterCoastlineShape3D::Cylinder {
            center,
            radius,
            half_height,
        } => {
            2u8.hash(hasher);
            hash_f32_slice(center, hasher);
            radius.to_bits().hash(hasher);
            half_height.to_bits().hash(hasher);
        }
        WaterCoastlineShape3D::Triangle { points } => {
            3u8.hash(hasher);
            for point in points {
                hash_f32_slice(point, hasher);
            }
        }
    }
}

pub(super) fn hash_f32_slice(values: &[f32], hasher: &mut ahash::AHasher) {
    for value in values {
        value.to_bits().hash(hasher);
    }
}

pub(super) fn raster_impacts_2d(
    out: &mut [[f32; 4]],
    width: usize,
    height: usize,
    water: &Water2DState,
) {
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
                // displaced water: crater under the body, crown on the ring
                let crown = ring * (strength * 0.44 + impact.cavitation * 1.08);
                let push = amount * (strength * 0.70 + impact.cavitation * 0.28);
                let cell = &mut out[y * width + x];
                cell[2] = (cell[2] + crown - push).clamp(-1.0, 1.0);
                cell[3] = cell[3].max((ring * 1.20 + amount * 0.22).clamp(0.0, 1.0));
            }
        }
    }
}

pub(super) fn signed_distance_2d(p: [f32; 2], shape: WaterCoastlineShape2D) -> f32 {
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

pub(super) fn point_in_triangle(p: [f32; 2], t: [[f32; 2]; 3]) -> bool {
    let s1 = cross2(p, t[0], t[1]);
    let s2 = cross2(p, t[1], t[2]);
    let s3 = cross2(p, t[2], t[0]);
    (s1 >= 0.0 && s2 >= 0.0 && s3 >= 0.0) || (s1 <= 0.0 && s2 <= 0.0 && s3 <= 0.0)
}

pub(super) fn cross2(p: [f32; 2], a: [f32; 2], b: [f32; 2]) -> f32 {
    (p[0] - a[0]) * (b[1] - a[1]) - (p[1] - a[1]) * (b[0] - a[0])
}

pub(super) fn distance_segment(p: [f32; 2], a: [f32; 2], b: [f32; 2]) -> f32 {
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

pub(super) fn raster_coastline_3d(
    out: &mut [[f32; 4]],
    resolution: [u32; 2],
    water: &Water3DState,
    node: NodeID,
    cache: &mut HashMap<NodeID, CachedCoastline>,
) {
    let width = resolution[0].clamp(1, 256) as usize;
    let height = resolution[1].clamp(1, 256) as usize;
    if water.coastline_shapes.is_empty() {
        cache.remove(&node);
        raster_impacts_3d(out, width, height, water);
        return;
    }
    let foam_width = water.coastline_foam_width.max(0.001);
    let softness = water.coastline_cutoff_softness.max(0.001);

    let mut hasher = coastline_hasher();
    width.hash(&mut hasher);
    height.hash(&mut hasher);
    water.size[0].to_bits().hash(&mut hasher);
    water.size[1].to_bits().hash(&mut hasher);
    foam_width.to_bits().hash(&mut hasher);
    softness.to_bits().hash(&mut hasher);
    water.coastline_shapes.len().hash(&mut hasher);
    for shape in water.coastline_shapes.iter() {
        hash_coastline_shape_3d(shape, &mut hasher);
    }
    let signature = hasher.finish();

    let cell_count = width * height;
    let entry = cache.entry(node).or_insert_with(|| CachedCoastline {
        signature,
        base: Vec::new(),
    });
    // Rebuild the static field only when the shapes/params/grid changed.
    if entry.signature != signature || entry.base.len() != cell_count {
        entry.signature = signature;
        entry.base.clear();
        entry.base.reserve(cell_count);
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
                let (solid, foam_edge, spill_energy) =
                    coastline_fill(signed_min, foam_width, softness);
                entry.base.push([solid, edge.max(foam_edge), spill_energy]);
            }
        }
    }
    let base = &entry.base;

    // Blend the frame-varying impacts wake over the cached static field.
    for y in 0..height {
        for x in 0..width {
            let i = y * width + x;
            let [solid, edge_foam, spill_energy] = base[i];
            let fx = x as f32 / (width.saturating_sub(1).max(1) as f32);
            let fy = y as f32 / (height.saturating_sub(1).max(1) as f32);
            let p = [(fx - 0.5) * water.size[0], (fy - 0.5) * water.size[1]];
            let mut wake = 0.0f32;
            for impact in water.impacts.iter() {
                let dx = p[0] - impact.position[0];
                let dz = p[1] - impact.position[2];
                let radius = impact.radius.max(0.001);
                let t = ((dx * dx + dz * dz) / (radius * radius).max(0.000001)).clamp(0.0, 1.0);
                let push = 1.0 - t;
                let ring = (1.0 - ((t - 0.72).abs() / 0.28).clamp(0.0, 1.0)).powi(2);
                let strength = impact.strength * (1.0 / 180.0);
                // displaced water: crater under the body, crown on the ring
                wake += ring * (strength * 0.30 + impact.cavitation * 0.92)
                    - push * (strength * 0.70 + impact.cavitation * 0.28);
            }
            let wake = wake.clamp(-1.0, 1.0);
            out[i] = [solid, edge_foam, wake, spill_energy.max(wake.abs())];
        }
    }
}

pub(super) fn coastline_fill(signed: f32, foam_width: f32, softness: f32) -> (f32, f32, f32) {
    let inset = WATER_COASTLINE_INSET_METERS.max(softness);
    let block_t = ((-signed - inset) / softness.max(0.001)).clamp(0.0, 1.0);
    let solid = block_t * block_t * (3.0 - 2.0 * block_t);
    let foam_edge = 1.0 - (signed.abs() / foam_width.max(0.001)).clamp(0.0, 1.0);
    let spill_t = ((-signed) / inset).clamp(0.0, 1.0);
    let spill_energy = (1.0 - spill_t * 0.70) * (1.0 - solid);
    (solid, foam_edge.max(0.0), spill_energy.clamp(0.0, 1.0))
}

pub(super) fn raster_impacts_3d(
    out: &mut [[f32; 4]],
    width: usize,
    height: usize,
    water: &Water3DState,
) {
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
                // displaced water: crater under the body, crown on the ring
                let crown = ring * (strength * 0.44 + impact.cavitation * 1.08);
                let push = amount * (strength * 0.70 + impact.cavitation * 0.28);
                let cell = &mut out[y * width + x];
                cell[2] = (cell[2] + crown - push).clamp(-1.0, 1.0);
                cell[3] = cell[3].max((ring * 1.20 + amount * 0.22).clamp(0.0, 1.0));
            }
        }
    }
}

pub(super) fn signed_distance_3d_xz(p: [f32; 2], shape: WaterCoastlineShape3D) -> f32 {
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
