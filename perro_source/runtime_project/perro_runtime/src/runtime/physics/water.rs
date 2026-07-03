use ahash::AHashMap;
use glam::{Mat3, Mat4, Vec3};
use perro_ids::NodeID;
use perro_nodes::{Shape2D, Shape3D, WaterShape};
use perro_structs::{BitMask, Vector2, Vector3};
pub(super) const WATER_FORCE_PAR_BODY_THRESHOLD: usize = 512;
pub(super) const WATER_WAVE_FOLLOW_DT: f32 = 1.0 / 60.0;
pub(super) const WATER_BODY_SAMPLE_TTL: f32 = 0.20;
pub(super) const WATER_QUERY_LOCAL_EPS: f32 = 0.35;
pub(super) const WATER_QUERY_MAX_PER_WATER: usize = 128;

pub(super) fn water_force_lod(
    near_distance: f32,
    mid_distance: f32,
    far_distance: f32,
    water_pos: Vector2,
    camera_pos: Vector2,
) -> (f32, f32) {
    let distance = Vector2::distance(water_pos, camera_pos);
    let near = near_distance.max(0.0);
    let mid = mid_distance.max(near);
    let far = far_distance.max(mid);
    if distance <= near {
        return (1.0, 0.0);
    }
    if distance <= mid {
        let t = ((distance - near) / (mid - near).max(0.001)).clamp(0.0, 1.0);
        return (1.0 - t * 0.25, 0.02 * t);
    }
    if distance <= far {
        let t = ((distance - mid) / (far - mid).max(0.001)).clamp(0.0, 1.0);
        return (0.75 - t * 0.35, 0.02 + 0.18 * t);
    }
    (0.25, 0.5)
}

pub(crate) fn water_target_submerged(density: f32) -> f32 {
    (0.12 + density.max(0.0) * 0.11).clamp(0.12, 0.58)
}

pub(super) fn water_buoyancy_cap(mass: f32, submerged: f32, depth: f32, buoyancy: f32) -> f32 {
    let deep_recovery = (submerged - depth.max(0.0)).max(0.0) * 10.5;
    let lift_recovery = buoyancy.max(0.0) * 16.0;
    let mass = mass.max(0.001);
    mass * mass.sqrt() * (9.81 + lift_recovery + deep_recovery)
}

#[derive(Clone, Copy)]
pub(super) struct RuntimeWater2D {
    pub(super) id: NodeID,
    pub(super) half: Vector2,
    pub(super) transform: Mat3,
    pub(super) inv_transform: Mat3,
    pub(super) normal: Vector2,
    pub(super) min_x: f32,
    pub(super) max_x: f32,
    pub(super) surface: perro_nodes::WaterSurfaceParams,
}

#[derive(Clone, Copy)]
pub(super) struct RuntimeWater3D {
    pub(super) id: NodeID,
    pub(super) half: Vector2,
    pub(super) transform: Mat4,
    pub(super) inv_transform: Mat4,
    pub(super) normal: Vector3,
    pub(super) min_x: f32,
    pub(super) max_x: f32,
    pub(super) surface: perro_nodes::WaterSurfaceParams,
}

pub(super) struct RuntimeWaterIndex2D {
    pub(super) waters: Vec<RuntimeWater2D>,
    pub(super) bins: Vec<Vec<usize>>,
    pub(super) origin_x: f32,
    pub(super) inv_cell_width: f32,
}

pub(super) struct RuntimeWaterIndex3D {
    pub(super) waters: Vec<RuntimeWater3D>,
    pub(super) bins: Vec<Vec<usize>>,
    pub(super) origin_x: f32,
    pub(super) inv_cell_width: f32,
}

#[derive(Clone, Copy)]
pub(super) struct RuntimeWaterBody2D {
    pub(super) id: NodeID,
    pub(super) pos: Vector2,
    pub(super) velocity: Vector2,
    pub(super) mass: f32,
    pub(super) density: f32,
    pub(super) float_radius: f32,
    pub(super) sleeping: bool,
    pub(super) collision_layers: BitMask,
    pub(super) collision_mask: BitMask,
}

#[derive(Clone, Copy)]
pub(super) struct RuntimeWaterBody3D {
    pub(super) id: NodeID,
    pub(super) pos: Vector3,
    pub(super) velocity: Vector3,
    pub(super) mass: f32,
    pub(super) density: f32,
    pub(super) float_radius: f32,
    pub(super) sleeping: bool,
    pub(super) collision_layers: BitMask,
    pub(super) collision_mask: BitMask,
}

#[derive(Clone, Copy)]
pub(super) struct WaterCandidate2D {
    pub(super) water: RuntimeWater2D,
    pub(super) local: Vector2,
    pub(super) surface_point: Vector2,
    pub(super) normal: Vector2,
    pub(super) wave_dir: Vector2,
    pub(super) sample: perro_nodes::WaterPhysicsSample,
    pub(super) weight: f32,
}

#[derive(Clone, Copy)]
pub(super) struct WaterCandidate3D {
    pub(super) water: RuntimeWater3D,
    pub(super) local: Vector3,
    pub(super) surface_point: Vector3,
    pub(super) normal: Vector3,
    pub(super) wave_dir: Vector3,
    pub(super) sample: perro_nodes::WaterPhysicsSample,
    pub(super) weight: f32,
}

#[derive(Clone, Copy)]
pub(super) struct BlendedWaterSample2D {
    pub(super) pos: Vector2,
    pub(super) normal: Vector2,
    pub(super) wave_dir: Vector2,
    pub(super) submerged: f32,
    pub(super) surface: perro_nodes::WaterSurfaceParams,
    pub(super) sample: perro_nodes::WaterPhysicsSample,
    pub(super) lod_weight: f32,
}

#[derive(Clone, Copy)]
pub(super) struct BlendedWaterSample3D {
    pub(super) pos: Vector3,
    pub(super) normal: Vector3,
    pub(super) wave_dir: Vector3,
    pub(super) submerged: f32,
    pub(super) surface: perro_nodes::WaterSurfaceParams,
    pub(super) sample: perro_nodes::WaterPhysicsSample,
    pub(super) lod_weight: f32,
}

#[derive(Clone, Copy)]
pub(super) struct WaterBodyForce2D {
    pub(super) id: NodeID,
    pub(super) force: Vector2,
    pub(super) impulse: Vector2,
}

#[derive(Clone, Copy)]
pub(super) struct WaterBodyForce3D {
    pub(super) id: NodeID,
    pub(super) force: Vector3,
    pub(super) impulse: Vector3,
}

pub(super) fn water_shape_2d(shape: WaterShape) -> Shape2D {
    match shape {
        WaterShape::Circle { radius } | WaterShape::Cylinder { radius, .. } => {
            Shape2D::Circle { radius }
        }
        WaterShape::Rect { size } => Shape2D::Quad {
            width: size.x,
            height: size.y,
        },
        WaterShape::Box { size } => Shape2D::Quad {
            width: size.x,
            height: size.z,
        },
    }
}

pub(super) fn blend_water_candidates_2d(
    candidates: Vec<WaterCandidate2D>,
) -> Vec<BlendedWaterSample2D> {
    if candidates.len() <= 1 {
        return candidates
            .into_iter()
            .map(blended_water_sample_2d)
            .collect();
    }
    let mut used = vec![false; candidates.len()];
    let mut out = Vec::new();
    for start in 0..candidates.len() {
        if used[start] {
            continue;
        }
        let mut group = vec![start];
        used[start] = true;
        let mut cursor = 0;
        while cursor < group.len() {
            let a = group[cursor];
            for b in 0..candidates.len() {
                if used[b] {
                    continue;
                }
                if water_linked_2d(candidates[a].water, candidates[b].water) {
                    used[b] = true;
                    group.push(b);
                }
            }
            cursor += 1;
        }
        out.push(blend_water_group_2d(&candidates, &group));
    }
    out
}

pub(super) fn blend_water_candidates_3d(
    candidates: Vec<WaterCandidate3D>,
) -> Vec<BlendedWaterSample3D> {
    if candidates.len() <= 1 {
        return candidates
            .into_iter()
            .map(blended_water_sample_3d)
            .collect();
    }
    let mut used = vec![false; candidates.len()];
    let mut out = Vec::new();
    for start in 0..candidates.len() {
        if used[start] {
            continue;
        }
        let mut group = vec![start];
        used[start] = true;
        let mut cursor = 0;
        while cursor < group.len() {
            let a = group[cursor];
            for b in 0..candidates.len() {
                if used[b] {
                    continue;
                }
                if water_linked_3d(candidates[a].water, candidates[b].water) {
                    used[b] = true;
                    group.push(b);
                }
            }
            cursor += 1;
        }
        out.push(blend_water_group_3d(&candidates, &group));
    }
    out
}

pub(super) fn water_forces_for_body_2d(
    body: RuntimeWaterBody2D,
    water_index: &RuntimeWaterIndex2D,
    water_samples: &AHashMap<NodeID, perro_nodes::WaterPhysicsSample>,
    water_body_samples: &AHashMap<
        crate::runtime::WaterBodySampleKey,
        crate::runtime::WaterBodySampleCache,
    >,
    elapsed: f32,
    camera_pos: Vector2,
) -> Vec<WaterBodyForce2D> {
    let samples = blended_water_samples_2d(WaterBlendQuery2D {
        point: body.pos,
        body_layers: body.collision_layers,
        body_mask: body.collision_mask,
        water_index,
        water_samples,
        water_body_samples,
        body_id: body.id,
        point_id: 0,
        elapsed,
    });
    let mut forces = Vec::with_capacity(samples.len());
    for blend in samples {
        let float_radius = body.float_radius.max(0.0);
        let submerged = (blend.submerged + float_radius).max(0.0);
        if submerged <= 0.0 {
            continue;
        }
        let mass = body.mass.max(0.001);
        let target_submerged = (float_radius * 2.0 * body.density.clamp(0.05, 0.95))
            .max(water_target_submerged(body.density));
        let contact = (submerged / target_submerged.max(0.001)).clamp(0.0, 1.5);
        // critically damped spring toward the wave surface: damping matched to
        // stiffness so the body sticks to the surface instead of bouncing
        let stiffness = blend.surface.physics.buoyancy.max(0.05) * 9.5;
        let damping_ratio = 0.9 + blend.surface.physics.drag.max(0.0) * 0.35;
        let damping = 2.0 * stiffness.sqrt() * damping_ratio;
        let rel_vel = body.velocity.dot(blend.normal) - blend.sample.velocity.y;
        let err = (submerged - target_submerged).clamp(-3.0, 3.0);
        let support = 9.81 * contact.min(1.0);
        let accel_y = support + stiffness * err - damping * rel_vel;
        let current_speed = blend.sample.velocity.x;
        let wave_speed = (current_speed + blend.sample.velocity.y.abs() * 0.012)
            * blend.surface.physics.wake_strength.max(0.0)
            * contact;
        let target_wave_speed = wave_speed.clamp(-1.5, 1.5);
        let body_wave_speed = body.velocity.dot(blend.wave_dir);
        let wave_drive = blend.wave_dir
            * ((target_wave_speed - body_wave_speed).clamp(-2.0, 2.0)
                * mass
                * contact
                * blend.surface.physics.drag.max(0.05)
                * 5.0);
        // surf push: a rising wave face shoves the body along wave travel
        let phase_speed =
            (blend.surface.wave.length.max(0.25) * blend.surface.wave.speed.max(0.0) * 0.2
                / std::f32::consts::TAU)
                .max(0.35);
        let slope = (blend.sample.velocity.y / phase_speed).clamp(-1.3, 1.3);
        let surf_push = blend.wave_dir
            * (9.81
                * slope
                * mass
                * contact.min(1.0)
                * blend.surface.physics.wake_strength.clamp(0.0, 2.5)
                * 0.45);
        let (scale, deadzone) = water_force_lod(
            blend.surface.lod.near_distance,
            blend.surface.lod.mid_distance,
            blend.surface.lod.far_distance,
            blend.pos,
            camera_pos,
        );
        let cap = water_buoyancy_cap(
            mass,
            submerged,
            blend.surface.shape.depth(blend.surface.depth),
            blend.surface.physics.buoyancy,
        );
        let force_y = (mass * accel_y).clamp(-cap, cap) * scale * blend.lod_weight;
        let mass_scale = (1.0 / mass.max(1.0).sqrt()).clamp(0.35, 1.0);
        let force = blend.normal * force_y
            + (wave_drive + surf_push) * scale * blend.lod_weight * mass_scale;
        if force.length() >= deadzone {
            forces.push(WaterBodyForce2D {
                id: body.id,
                force,
                impulse: Vector2::ZERO,
            });
        }
    }
    forces
}

pub(super) fn water_forces_for_body_3d(
    body: RuntimeWaterBody3D,
    water_index: &RuntimeWaterIndex3D,
    water_samples: &AHashMap<NodeID, perro_nodes::WaterPhysicsSample>,
    water_body_samples: &AHashMap<
        crate::runtime::WaterBodySampleKey,
        crate::runtime::WaterBodySampleCache,
    >,
    elapsed: f32,
    camera_pos: Vector2,
) -> Vec<WaterBodyForce3D> {
    let radius = body.float_radius.max(0.5);
    let sample_points = [
        (0u8, body.pos),
        (1u8, body.pos + Vector3::new(-radius * 0.75, 0.0, 0.0)),
        (2u8, body.pos + Vector3::new(radius * 0.75, 0.0, 0.0)),
        (3u8, body.pos + Vector3::new(0.0, 0.0, -radius * 0.75)),
        (4u8, body.pos + Vector3::new(0.0, 0.0, radius * 0.75)),
    ];
    let sample_count = if body.sleeping {
        1
    } else {
        sample_points.len()
    };
    let mut total_force = Vector3::ZERO;
    for (point_id, point_pos) in sample_points.into_iter().take(sample_count) {
        let samples = blended_water_samples_3d(WaterBlendQuery3D {
            point: point_pos,
            body_layers: body.collision_layers,
            body_mask: body.collision_mask,
            water_index,
            water_samples,
            water_body_samples,
            body_id: body.id,
            point_id,
            elapsed,
        });
        for blend in samples {
            let float_radius = body.float_radius.max(0.0);
            let submerged = (blend.submerged + float_radius).max(0.0);
            if submerged <= 0.0 {
                continue;
            }
            let mass = body.mass.max(0.001) / sample_count as f32;
            let target_submerged = (float_radius * 2.0 * body.density.clamp(0.05, 0.95))
                .max(water_target_submerged(body.density));
            let contact = (submerged / target_submerged.max(0.001)).clamp(0.0, 1.5);
            // critically damped spring toward the wave surface: damping matched
            // to stiffness so the body rides the surface instead of bouncing
            let stiffness = blend.surface.physics.buoyancy.max(0.05) * 9.5;
            let damping_ratio = 0.9 + blend.surface.physics.drag.max(0.0) * 0.35;
            let damping = 2.0 * stiffness.sqrt() * damping_ratio;
            let rel_vel = body.velocity.dot(blend.normal) - blend.sample.velocity.y;
            let err = (submerged - target_submerged).clamp(-3.0, 3.0);
            let support = 9.81 * contact.min(1.0);
            let accel_y = support + stiffness * err - damping * rel_vel;
            let current_speed = blend.sample.velocity.x;
            let wave_speed = (current_speed + blend.sample.velocity.y.abs() * 0.018)
                * blend.surface.physics.wake_strength.max(0.0)
                * contact;
            let target_wave_speed = wave_speed.clamp(-2.0, 2.0);
            let body_wave_speed = body.velocity.dot(blend.wave_dir);
            let wave_drive = blend.wave_dir
                * ((target_wave_speed - body_wave_speed).clamp(-2.5, 2.5)
                    * mass
                    * contact
                    * blend.surface.physics.drag.max(0.08)
                    * 7.5);
            // surf push: a rising wave face shoves the body along wave travel
            let phase_speed =
                (blend.surface.wave.length.max(0.25) * blend.surface.wave.speed.max(0.0) * 0.2
                    / std::f32::consts::TAU)
                    .max(0.35);
            let slope = (blend.sample.velocity.y / phase_speed).clamp(-1.3, 1.3);
            let surf_push = blend.wave_dir
                * (9.81
                    * slope
                    * mass
                    * contact.min(1.0)
                    * blend.surface.physics.wake_strength.clamp(0.0, 2.5)
                    * 0.45);
            let water_pos_2d = Vector2::new(blend.pos.x, blend.pos.z);
            let (scale, deadzone) = water_force_lod(
                blend.surface.lod.near_distance,
                blend.surface.lod.mid_distance,
                blend.surface.lod.far_distance,
                water_pos_2d,
                camera_pos,
            );
            let cap = water_buoyancy_cap(
                mass,
                submerged,
                blend.surface.shape.depth(blend.surface.depth),
                blend.surface.physics.buoyancy,
            );
            let force_y = (mass * accel_y).clamp(-cap, cap) * scale * blend.lod_weight;
            let mass_scale = (1.0 / body.mass.max(1.0).sqrt()).clamp(0.35, 1.0);
            let force = blend.normal * force_y
                + (wave_drive + surf_push) * scale * blend.lod_weight * mass_scale;
            if force.length() >= deadzone {
                total_force += force;
            }
        }
    }
    let mut forces = Vec::new();
    if total_force.length_squared() > 0.0 {
        forces.push(WaterBodyForce3D {
            id: body.id,
            force: total_force,
            impulse: Vector3::ZERO,
        });
    }
    forces
}

pub(super) fn water_body_splashes_2d(
    bodies: &[RuntimeWaterBody2D],
    water_index: &RuntimeWaterIndex2D,
    water_body_samples: &AHashMap<
        crate::runtime::WaterBodySampleKey,
        crate::runtime::WaterBodySampleCache,
    >,
    elapsed: f32,
) -> Vec<crate::runtime::ForceWaterImpact2D> {
    let mut impacts = Vec::new();
    let empty_samples = AHashMap::new();
    for body in bodies {
        if body.sleeping {
            continue;
        }
        for sample in blended_water_samples_2d(WaterBlendQuery2D {
            point: body.pos,
            body_layers: body.collision_layers,
            body_mask: body.collision_mask,
            water_index,
            water_samples: &empty_samples,
            water_body_samples,
            body_id: body.id,
            point_id: 0,
            elapsed,
        }) {
            let target = water_target_submerged(body.density);
            if sample.submerged <= 0.0 || sample.submerged > target * 2.25 {
                continue;
            }
            // real drop-ins only: gentle bobbing must not spawn impact spikes
            let rel_down = sample.sample.velocity.y - body.velocity.dot(sample.normal);
            if rel_down <= 1.1 {
                continue;
            }
            let strength = perro_nodes::water_impact_strength(
                body.mass.max(body.density),
                sample.normal * rel_down,
                sample.surface.physics.wake_strength,
            );
            if strength <= 0.0 {
                continue;
            }
            impacts.push(crate::runtime::ForceWaterImpact2D {
                position: sample.pos,
                force: -sample.normal * rel_down * body.mass.max(0.001),
                strength: strength.min(512.0),
                radius: body.mass.max(body.density).sqrt().clamp(0.65, 4.0),
                cavitation: (strength / 128.0).clamp(0.0, 1.0),
            });
        }
    }
    impacts
}

pub(super) fn water_body_splashes_3d(
    bodies: &[RuntimeWaterBody3D],
    water_index: &RuntimeWaterIndex3D,
    water_body_samples: &AHashMap<
        crate::runtime::WaterBodySampleKey,
        crate::runtime::WaterBodySampleCache,
    >,
    elapsed: f32,
) -> Vec<crate::runtime::ForceWaterImpact3D> {
    let mut impacts = Vec::new();
    let empty_samples = AHashMap::new();
    for body in bodies {
        if body.sleeping {
            continue;
        }
        for sample in blended_water_samples_3d(WaterBlendQuery3D {
            point: body.pos,
            body_layers: body.collision_layers,
            body_mask: body.collision_mask,
            water_index,
            water_samples: &empty_samples,
            water_body_samples,
            body_id: body.id,
            point_id: 0,
            elapsed,
        }) {
            let target = water_target_submerged(body.density);
            if sample.submerged <= 0.0 || sample.submerged > target * 2.25 {
                continue;
            }
            // real drop-ins only: gentle bobbing must not spawn impact spikes
            let rel_down = sample.sample.velocity.y - body.velocity.dot(sample.normal);
            if rel_down <= 1.1 {
                continue;
            }
            let strength = perro_nodes::water_impact_strength(
                body.mass.max(body.density),
                Vector2::new(0.0, rel_down),
                sample.surface.physics.wake_strength,
            );
            if strength <= 0.0 {
                continue;
            }
            impacts.push(crate::runtime::ForceWaterImpact3D {
                position: sample.pos,
                force: -sample.normal * rel_down * body.mass.max(0.001),
                strength: strength.min(512.0),
                radius: body.mass.max(body.density).sqrt().clamp(0.65, 4.0),
                cavitation: (strength / 128.0).clamp(0.0, 1.0),
            });
        }
    }
    impacts
}

pub(super) struct WaterBlendQuery2D<'a> {
    pub(super) point: Vector2,
    pub(super) body_layers: BitMask,
    pub(super) body_mask: BitMask,
    pub(super) water_index: &'a RuntimeWaterIndex2D,
    pub(super) water_samples: &'a AHashMap<NodeID, perro_nodes::WaterPhysicsSample>,
    pub(super) water_body_samples:
        &'a AHashMap<crate::runtime::WaterBodySampleKey, crate::runtime::WaterBodySampleCache>,
    pub(super) body_id: NodeID,
    pub(super) point_id: u8,
    pub(super) elapsed: f32,
}

pub(super) fn blended_water_samples_2d(query: WaterBlendQuery2D<'_>) -> Vec<BlendedWaterSample2D> {
    let mut first = None;
    let mut candidates: Vec<WaterCandidate2D> = Vec::new();
    let Some(bin) = query.water_index.bin(query.point.x) else {
        return Vec::new();
    };
    for &idx in bin {
        let water = query.water_index.waters[idx];
        if water.surface.collision_mask.intersects(query.body_layers)
            || query.body_mask.intersects(water.surface.collision_layers)
        {
            continue;
        }
        let local = water_local_point_2d(water.inv_transform, query.point);
        if local.x.abs() > water.half.x || local.y.abs() > water.half.y {
            continue;
        }
        if !water.surface.shape.contains_surface(local) {
            continue;
        }
        let sample = water_physics_sample_for_body_cached(
            &water.surface,
            local,
            query.elapsed,
            lookup_water_body_sample(
                query.water_body_samples,
                water.id,
                query.body_id,
                query.point_id,
                local,
                query.elapsed,
            ),
            query.water_samples.get(&water.id).copied(),
        );
        let surface_point =
            water_global_point_2d(water.transform, Vector2::new(local.x, sample.height));
        let candidate = WaterCandidate2D {
            water,
            local,
            surface_point,
            normal: water.normal,
            wave_dir: water_wave_dir_2d(water.transform, water.surface),
            sample,
            weight: water_blend_weight(water.surface.shape, local),
        };
        if let Some(existing) = first {
            if candidates.is_empty() {
                candidates.push(existing);
            }
            candidates.push(candidate);
        } else {
            first = Some(candidate);
        }
    }
    if candidates.is_empty() {
        return first.map(blended_water_sample_2d).into_iter().collect();
    }
    blend_water_candidates_2d(candidates)
}

pub(super) struct WaterBlendQuery3D<'a> {
    pub(super) point: Vector3,
    pub(super) body_layers: BitMask,
    pub(super) body_mask: BitMask,
    pub(super) water_index: &'a RuntimeWaterIndex3D,
    pub(super) water_samples: &'a AHashMap<NodeID, perro_nodes::WaterPhysicsSample>,
    pub(super) water_body_samples:
        &'a AHashMap<crate::runtime::WaterBodySampleKey, crate::runtime::WaterBodySampleCache>,
    pub(super) body_id: NodeID,
    pub(super) point_id: u8,
    pub(super) elapsed: f32,
}

pub(super) fn blended_water_samples_3d(query: WaterBlendQuery3D<'_>) -> Vec<BlendedWaterSample3D> {
    let mut first = None;
    let mut candidates: Vec<WaterCandidate3D> = Vec::new();
    let Some(bin) = query.water_index.bin(query.point.x) else {
        return Vec::new();
    };
    for &idx in bin {
        let water = query.water_index.waters[idx];
        if water.surface.collision_mask.intersects(query.body_layers)
            || query.body_mask.intersects(water.surface.collision_layers)
        {
            continue;
        }
        let local3 = water_local_point_3d(water.inv_transform, query.point);
        let local = Vector2::new(local3.x, local3.z);
        if local.x.abs() > water.half.x || local.y.abs() > water.half.y {
            continue;
        }
        if !water.surface.shape.contains_surface(local) {
            continue;
        }
        let sample = water_physics_sample_for_body_cached(
            &water.surface,
            local,
            query.elapsed,
            lookup_water_body_sample(
                query.water_body_samples,
                water.id,
                query.body_id,
                query.point_id,
                local,
                query.elapsed,
            ),
            query.water_samples.get(&water.id).copied(),
        );
        let surface_point = water_global_point_3d(
            water.transform,
            Vector3::new(local3.x, sample.height, local3.z),
        );
        let candidate = WaterCandidate3D {
            water,
            local: local3,
            surface_point,
            normal: water.normal,
            wave_dir: water_wave_dir_3d(water.transform, water.surface),
            sample,
            weight: water_blend_weight(water.surface.shape, local),
        };
        if let Some(existing) = first {
            if candidates.is_empty() {
                candidates.push(existing);
            }
            candidates.push(candidate);
        } else {
            first = Some(candidate);
        }
    }
    if candidates.is_empty() {
        return first.map(blended_water_sample_3d).into_iter().collect();
    }
    blend_water_candidates_3d(candidates)
}

pub(super) fn blended_water_sample_2d(candidate: WaterCandidate2D) -> BlendedWaterSample2D {
    BlendedWaterSample2D {
        pos: candidate.surface_point,
        normal: candidate.normal,
        wave_dir: candidate.wave_dir,
        submerged: candidate.sample.height - candidate.local.y,
        surface: candidate.water.surface,
        sample: candidate.sample,
        lod_weight: 1.0,
    }
}

pub(super) fn blended_water_sample_3d(candidate: WaterCandidate3D) -> BlendedWaterSample3D {
    BlendedWaterSample3D {
        pos: candidate.surface_point,
        normal: candidate.normal,
        wave_dir: candidate.wave_dir,
        submerged: candidate.sample.height - candidate.local.y,
        surface: candidate.water.surface,
        sample: candidate.sample,
        lod_weight: 1.0,
    }
}

impl RuntimeWaterIndex2D {
    pub(super) fn new(waters: Vec<RuntimeWater2D>) -> Self {
        let (bins, origin_x, inv_cell_width) =
            build_water_bins(waters.iter().map(|water| (water.min_x, water.max_x)));
        Self {
            waters,
            bins,
            origin_x,
            inv_cell_width,
        }
    }

    pub(super) fn bin(&self, point_x: f32) -> Option<&[usize]> {
        water_bin(&self.bins, self.origin_x, self.inv_cell_width, point_x)
    }
}

impl RuntimeWaterIndex3D {
    pub(super) fn new(waters: Vec<RuntimeWater3D>) -> Self {
        let (bins, origin_x, inv_cell_width) =
            build_water_bins(waters.iter().map(|water| (water.min_x, water.max_x)));
        Self {
            waters,
            bins,
            origin_x,
            inv_cell_width,
        }
    }

    pub(super) fn bin(&self, point_x: f32) -> Option<&[usize]> {
        water_bin(&self.bins, self.origin_x, self.inv_cell_width, point_x)
    }
}

pub(super) fn build_water_bins(
    waters: impl Iterator<Item = (f32, f32)> + Clone,
) -> (Vec<Vec<usize>>, f32, f32) {
    let mut min_x = f32::INFINITY;
    let mut max_x = f32::NEG_INFINITY;
    let mut max_width = 0.0f32;
    let mut count = 0usize;
    for (water_min_x, water_max_x) in waters.clone() {
        min_x = min_x.min(water_min_x);
        max_x = max_x.max(water_max_x);
        max_width = max_width.max(water_max_x - water_min_x);
        count += 1;
    }
    if count == 0 || !min_x.is_finite() || !max_x.is_finite() {
        return (Vec::new(), 0.0, 1.0);
    }
    let cell_width = (max_width * 0.5).max(1.0);
    let inv_cell_width = 1.0 / cell_width;
    let bin_count = (((max_x - min_x) * inv_cell_width).ceil() as usize)
        .saturating_add(1)
        .max(1);
    let mut bins = vec![Vec::new(); bin_count];
    for (idx, (water_min_x, water_max_x)) in waters.enumerate() {
        let first = (((water_min_x - min_x) * inv_cell_width).floor() as isize)
            .clamp(0, bin_count.saturating_sub(1) as isize) as usize;
        let last = (((water_max_x - min_x) * inv_cell_width).floor() as isize)
            .clamp(0, bin_count.saturating_sub(1) as isize) as usize;
        for bin in &mut bins[first..=last] {
            bin.push(idx);
        }
    }
    (bins, min_x, inv_cell_width)
}

pub(super) fn water_bin(
    bins: &[Vec<usize>],
    origin_x: f32,
    inv_cell_width: f32,
    point_x: f32,
) -> Option<&[usize]> {
    if bins.is_empty() {
        return None;
    }
    let idx = ((point_x - origin_x) * inv_cell_width).floor() as isize;
    if idx < 0 || idx as usize >= bins.len() {
        return None;
    }
    Some(&bins[idx as usize])
}

pub(super) fn water_local_point_2d(inv_transform: Mat3, point: Vector2) -> Vector2 {
    let p = inv_transform * glam::Vec3::new(point.x, point.y, 1.0);
    Vector2::new(p.x, p.y)
}

pub(super) fn water_global_point_2d(transform: Mat3, point: Vector2) -> Vector2 {
    let p = transform * glam::Vec3::new(point.x, point.y, 1.0);
    Vector2::new(p.x, p.y)
}

pub(super) fn water_local_point_3d(inv_transform: Mat4, point: Vector3) -> Vector3 {
    inv_transform.transform_point3(point.into()).into()
}

pub(super) fn water_global_point_3d(transform: Mat4, point: Vector3) -> Vector3 {
    transform.transform_point3(point.into()).into()
}

#[cfg_attr(not(test), allow(dead_code))]
pub(crate) fn water_physics_sample_for_body(
    surface: &perro_nodes::WaterSurfaceParams,
    local: Vector2,
    elapsed: f32,
) -> perro_nodes::WaterPhysicsSample {
    water_physics_sample_for_body_cached(surface, local, elapsed, None, None)
}

/// GPU readback caches hold the sim deviation from the analytic idle surface.
/// Height/velocity combine that deviation with the live idle waves so physics
/// tracks exactly what the render shader displaces.
pub(crate) fn water_physics_sample_for_body_cached(
    surface: &perro_nodes::WaterSurfaceParams,
    local: Vector2,
    elapsed: f32,
    body_cached: Option<crate::runtime::WaterBodySampleCache>,
    cached: Option<perro_nodes::WaterPhysicsSample>,
) -> perro_nodes::WaterPhysicsSample {
    let idle_now = perro_nodes::analytic_idle_water_height(surface, local, elapsed);
    let idle_prev =
        perro_nodes::analytic_idle_water_height(surface, local, elapsed - WATER_WAVE_FOLLOW_DT);
    let idle_velocity_y = (idle_now - idle_prev) / WATER_WAVE_FOLLOW_DT;
    let flow_speed = surface.flow.dot(water_wave_local_dir(*surface));
    let (deviation, deviation_velocity_y, foam) = if let Some(body_cached) = body_cached {
        (body_cached.height, body_cached.velocity.y, body_cached.foam)
    } else if let Some(cached) = cached {
        (cached.height, cached.velocity.y, cached.foam)
    } else {
        (0.0, 0.0, 0.0)
    };
    perro_nodes::WaterPhysicsSample {
        height: idle_now + deviation,
        velocity: Vector2::new(flow_speed, idle_velocity_y + deviation_velocity_y),
        foam,
    }
}

pub(crate) fn lookup_water_body_sample(
    water_body_samples: &AHashMap<
        crate::runtime::WaterBodySampleKey,
        crate::runtime::WaterBodySampleCache,
    >,
    water: NodeID,
    body: NodeID,
    point: u8,
    local: Vector2,
    elapsed: f32,
) -> Option<crate::runtime::WaterBodySampleCache> {
    let key = crate::runtime::WaterBodySampleKey { water, body, point };
    let sample = water_body_samples.get(&key).copied()?;
    if elapsed - sample.sample_time > WATER_BODY_SAMPLE_TTL {
        return None;
    }
    if (sample.local - local).length() > WATER_QUERY_LOCAL_EPS {
        return None;
    }
    Some(sample)
}

pub(super) fn water_wave_local_dir(surface: perro_nodes::WaterSurfaceParams) -> Vector2 {
    let dir = if surface.flow.length_squared() > 1.0e-6 {
        surface.flow
    } else {
        surface.wind
    };
    water_normalize_2d(-dir)
}

pub(super) fn water_wave_dir_2d(
    transform: Mat3,
    surface: perro_nodes::WaterSurfaceParams,
) -> Vector2 {
    let dir = water_wave_local_dir(surface);
    let v = transform * glam::Vec3::new(dir.x, dir.y, 0.0);
    water_normalize_2d(Vector2::new(v.x, v.y))
}

pub(super) fn water_wave_dir_3d(
    transform: Mat4,
    surface: perro_nodes::WaterSurfaceParams,
) -> Vector3 {
    let dir = water_wave_local_dir(surface);
    let v = transform.transform_vector3(Vec3::new(dir.x, 0.0, dir.y));
    water_normalize_3d(Vector3::new(v.x, 0.0, v.z))
}

pub(super) fn water_normal_2d(transform: Mat3) -> Vector2 {
    let up = transform * glam::Vec3::new(0.0, 1.0, 0.0);
    water_normalize_2d(Vector2::new(up.x, up.y))
}

pub(super) fn water_normal_3d(transform: Mat4) -> Vector3 {
    water_normalize_3d(transform.transform_vector3(Vec3::Y).into())
}

pub(super) fn water_normalize_2d(v: Vector2) -> Vector2 {
    let len = (v.x * v.x + v.y * v.y).sqrt();
    if len > 1.0e-6 {
        v / len
    } else {
        Vector2::new(0.0, 1.0)
    }
}

pub(super) fn water_normalize_3d(v: Vector3) -> Vector3 {
    let len = (v.x * v.x + v.y * v.y + v.z * v.z).sqrt();
    if len > 1.0e-6 {
        v / len
    } else {
        Vector3::new(0.0, 1.0, 0.0)
    }
}

pub(super) fn water_world_x_bounds_2d(transform: Mat3, half: Vector2) -> (f32, f32) {
    let mut min_x = f32::INFINITY;
    let mut max_x = f32::NEG_INFINITY;
    for point in [
        Vector2::new(-half.x, -half.y),
        Vector2::new(half.x, -half.y),
        Vector2::new(-half.x, half.y),
        Vector2::new(half.x, half.y),
    ] {
        let p = water_global_point_2d(transform, point);
        min_x = min_x.min(p.x);
        max_x = max_x.max(p.x);
    }
    (min_x, max_x)
}

pub(super) fn water_world_x_bounds_3d(transform: Mat4, half: Vector2, depth: f32) -> (f32, f32) {
    let mut min_x = f32::INFINITY;
    let mut max_x = f32::NEG_INFINITY;
    for y in [0.0, -depth] {
        for point in [
            Vector3::new(-half.x, y, -half.y),
            Vector3::new(half.x, y, -half.y),
            Vector3::new(-half.x, y, half.y),
            Vector3::new(half.x, y, half.y),
        ] {
            let p = water_global_point_3d(transform, point);
            min_x = min_x.min(p.x);
            max_x = max_x.max(p.x);
        }
    }
    (min_x, max_x)
}

pub(super) fn register_water_query_candidates_2d(
    out: &mut AHashMap<NodeID, Vec<crate::runtime::PendingWaterQuery>>,
    water_index: &RuntimeWaterIndex2D,
    body: RuntimeWaterBody2D,
    point: u8,
    pos: Vector2,
) {
    let Some(bin) = water_index.bin(pos.x) else {
        return;
    };
    for &idx in bin {
        let water = water_index.waters[idx];
        if water
            .surface
            .collision_mask
            .intersects(body.collision_layers)
            || body
                .collision_mask
                .intersects(water.surface.collision_layers)
        {
            continue;
        }
        let local = water_local_point_2d(water.inv_transform, pos);
        if !water.surface.shape.contains_surface(local) {
            continue;
        }
        let list = out.entry(water.id).or_default();
        if list.len() >= WATER_QUERY_MAX_PER_WATER
            || list
                .iter()
                .any(|query| query.body == body.id && query.point == point)
        {
            continue;
        }
        list.push(crate::runtime::PendingWaterQuery {
            body: body.id,
            point,
            local,
        });
    }
}

pub(super) fn register_water_query_candidates_3d(
    out: &mut AHashMap<NodeID, Vec<crate::runtime::PendingWaterQuery>>,
    water_index: &RuntimeWaterIndex3D,
    body: RuntimeWaterBody3D,
    point: u8,
    pos: Vector3,
) {
    let Some(bin) = water_index.bin(pos.x) else {
        return;
    };
    for &idx in bin {
        let water = water_index.waters[idx];
        if water
            .surface
            .collision_mask
            .intersects(body.collision_layers)
            || body
                .collision_mask
                .intersects(water.surface.collision_layers)
        {
            continue;
        }
        let local = water_local_point_3d(water.inv_transform, pos);
        let local_xz = Vector2::new(local.x, local.z);
        if !water.surface.shape.contains_surface(local_xz) {
            continue;
        }
        let list = out.entry(water.id).or_default();
        if list.len() >= WATER_QUERY_MAX_PER_WATER
            || list
                .iter()
                .any(|query| query.body == body.id && query.point == point)
        {
            continue;
        }
        list.push(crate::runtime::PendingWaterQuery {
            body: body.id,
            point,
            local: local_xz,
        });
    }
}

pub(super) fn sample_water_id_2d(
    point: Vector2,
    water_index: &RuntimeWaterIndex2D,
    surface_point: Vector2,
) -> Option<NodeID> {
    let bin = water_index.bin(point.x)?;
    let mut best = None;
    let mut best_dist = f32::INFINITY;
    for &idx in bin {
        let water = water_index.waters[idx];
        let local = water_local_point_2d(water.inv_transform, point);
        if !water.surface.shape.contains_surface(local) {
            continue;
        }
        let dist = (surface_point
            - water_global_point_2d(water.transform, Vector2::new(local.x, surface_point.y)))
        .length();
        if dist < best_dist {
            best = Some(water.id);
            best_dist = dist;
        }
    }
    best
}

pub(super) fn sample_water_id_3d(
    point: Vector3,
    water_index: &RuntimeWaterIndex3D,
    surface_point: Vector3,
) -> Option<NodeID> {
    let bin = water_index.bin(point.x)?;
    let mut best = None;
    let mut best_dist = f32::INFINITY;
    for &idx in bin {
        let water = water_index.waters[idx];
        let local = water_local_point_3d(water.inv_transform, point);
        if !water
            .surface
            .shape
            .contains_surface(Vector2::new(local.x, local.z))
        {
            continue;
        }
        let dist = (surface_point
            - water_global_point_3d(
                water.transform,
                Vector3::new(local.x, surface_point.y, local.z),
            ))
        .length();
        if dist < best_dist {
            best = Some(water.id);
            best_dist = dist;
        }
    }
    best
}

pub(super) fn blend_water_group_2d(
    candidates: &[WaterCandidate2D],
    group: &[usize],
) -> BlendedWaterSample2D {
    if group.len() == 1 {
        let candidate = candidates[group[0]];
        return blended_water_sample_2d(candidate);
    }
    let mut total = 0.0;
    let mut pos = Vector2::ZERO;
    let mut normal = Vector2::ZERO;
    let mut wave_dir = Vector2::ZERO;
    let mut submerged = 0.0;
    let mut sample = perro_nodes::WaterPhysicsSample::default();
    let mut surface = candidates[group[0]].water.surface;
    let mut buoyancy = 0.0;
    let mut drag = 0.0;
    for &idx in group {
        let candidate = candidates[idx];
        let w = candidate.weight.max(0.001);
        total += w;
        pos += candidate.surface_point * w;
        normal += candidate.normal * w;
        wave_dir += candidate.wave_dir * w;
        submerged += (candidate.sample.height - candidate.local.y) * w;
        sample.height += candidate.surface_point.y * w;
        sample.velocity +=
            candidate.sample.velocity * w * candidate.water.surface.link.flow_transfer;
        sample.foam += candidate.sample.foam * w * candidate.water.surface.link.wave_transfer;
        buoyancy += candidate.water.surface.physics.buoyancy * w;
        drag += candidate.water.surface.physics.drag * w;
    }
    let inv = 1.0 / total.max(0.001);
    pos *= inv;
    normal = water_normalize_2d(normal);
    wave_dir = water_normalize_2d(wave_dir);
    submerged *= inv;
    sample.height *= inv;
    sample.velocity *= inv;
    sample.foam *= inv;
    surface.physics.buoyancy = buoyancy * inv;
    surface.physics.drag = drag * inv;
    BlendedWaterSample2D {
        pos,
        normal,
        wave_dir,
        submerged,
        surface,
        sample,
        lod_weight: 1.0,
    }
}

pub(super) fn blend_water_group_3d(
    candidates: &[WaterCandidate3D],
    group: &[usize],
) -> BlendedWaterSample3D {
    if group.len() == 1 {
        let candidate = candidates[group[0]];
        return blended_water_sample_3d(candidate);
    }
    let mut total = 0.0;
    let mut pos = Vector3::ZERO;
    let mut normal = Vector3::ZERO;
    let mut wave_dir = Vector3::ZERO;
    let mut submerged = 0.0;
    let mut sample = perro_nodes::WaterPhysicsSample::default();
    let mut surface = candidates[group[0]].water.surface;
    let mut buoyancy = 0.0;
    let mut drag = 0.0;
    for &idx in group {
        let candidate = candidates[idx];
        let w = candidate.weight.max(0.001);
        total += w;
        pos += candidate.surface_point * w;
        normal += candidate.normal * w;
        wave_dir += candidate.wave_dir * w;
        submerged += (candidate.sample.height - candidate.local.y) * w;
        sample.height += candidate.surface_point.y * w;
        sample.velocity +=
            candidate.sample.velocity * w * candidate.water.surface.link.flow_transfer;
        sample.foam += candidate.sample.foam * w * candidate.water.surface.link.wave_transfer;
        buoyancy += candidate.water.surface.physics.buoyancy * w;
        drag += candidate.water.surface.physics.drag * w;
    }
    let inv = 1.0 / total.max(0.001);
    pos *= inv;
    normal = water_normalize_3d(normal);
    wave_dir = water_normalize_3d(wave_dir);
    submerged *= inv;
    sample.height *= inv;
    sample.velocity *= inv;
    sample.foam *= inv;
    surface.physics.buoyancy = buoyancy * inv;
    surface.physics.drag = drag * inv;
    BlendedWaterSample3D {
        pos,
        normal,
        wave_dir,
        submerged,
        surface,
        sample,
        lod_weight: 1.0,
    }
}

pub(super) fn water_link_allowed(
    a: perro_nodes::WaterSurfaceParams,
    b: perro_nodes::WaterSurfaceParams,
) -> bool {
    !a.link.link_mask.intersects(b.link.link_layers)
        && !b.link.link_mask.intersects(a.link.link_layers)
}

pub(super) fn water_linked_2d(a: RuntimeWater2D, b: RuntimeWater2D) -> bool {
    water_link_allowed(a.surface, b.surface)
}

pub(super) fn water_linked_3d(a: RuntimeWater3D, b: RuntimeWater3D) -> bool {
    water_link_allowed(a.surface, b.surface)
}

pub(super) fn water_blend_weight(shape: WaterShape, local: Vector2) -> f32 {
    let t = match shape {
        WaterShape::Rect { size } => {
            let half = size * 0.5;
            let edge = (half.x - local.x.abs()).min(half.y - local.y.abs());
            let width = (half.x.min(half.y) * 0.25).max(0.5);
            (edge / width).clamp(0.0, 1.0)
        }
        WaterShape::Box { size } => {
            let half = Vector2::new(size.x, size.z) * 0.5;
            let edge = (half.x - local.x.abs()).min(half.y - local.y.abs());
            let width = (half.x.min(half.y) * 0.25).max(0.5);
            (edge / width).clamp(0.0, 1.0)
        }
        WaterShape::Circle { radius } | WaterShape::Cylinder { radius, .. } => {
            let edge = radius - local.length();
            let width = (radius * 0.25).max(0.5);
            (edge / width).clamp(0.0, 1.0)
        }
    };
    smoothstep(t)
}

pub(super) fn smoothstep(t: f32) -> f32 {
    let t = t.clamp(0.0, 1.0);
    t * t * (3.0 - 2.0 * t)
}

pub(super) fn water_shape_3d(shape: WaterShape, fallback_depth: f32) -> (Shape3D, f32) {
    match shape {
        WaterShape::Cylinder {
            radius,
            half_height,
        } => (
            Shape3D::Cylinder {
                radius,
                half_height,
            },
            -half_height,
        ),
        WaterShape::Circle { radius } => {
            let half_height = fallback_depth.max(0.001) * 0.5;
            (
                Shape3D::Cylinder {
                    radius,
                    half_height,
                },
                -half_height,
            )
        }
        WaterShape::Box { size } => (
            Shape3D::Cube {
                size: Vector3::new(size.x, size.y.max(0.001), size.z),
            },
            -size.y.max(0.001) * 0.5,
        ),
        WaterShape::Rect { size } => {
            let depth = fallback_depth.max(0.001);
            (
                Shape3D::Cube {
                    size: Vector3::new(size.x, depth, size.y),
                },
                -depth * 0.5,
            )
        }
    }
}
