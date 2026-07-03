use super::*;

/// Max depenetration recovery iterations per move_body call (Godot uses 4).
const RECOVERY_ITERATIONS: usize = 4;
/// Min skin push floor when margin is smaller.
const RECOVERY_SKIN_2D: f32 = 0.001;
const RECOVERY_SKIN_3D: f32 = 0.001;
/// Clamp total recovery per call to avoid popping thru geometry.
const RECOVERY_MAX_2D: f32 = 0.2;
const RECOVERY_MAX_3D: f32 = 0.2;

impl PhysicsSystem {
    pub fn raycast_2d(
        &mut self,
        origin: Vector2,
        direction: Vector2,
        max_distance: f32,
        filter: &PhysicsQueryFilter,
    ) -> Option<PhysicsRayHit2D> {
        if max_distance <= 0.0 || !max_distance.is_finite() {
            return None;
        }

        let dir = na2::Vector2::new(direction.x, direction.y);
        let dir_len = dir.norm();
        if dir_len <= 0.000_001 || !dir_len.is_finite() {
            return None;
        }
        let dir = dir / dir_len;

        let world = self.world_2d.as_mut()?;
        if self.query_pipeline_dirty_2d {
            world.query_pipeline.update(&world.colliders);
            self.query_pipeline_dirty_2d = false;
        }

        let ray = r2::Ray::new(na2::Point2::new(origin.x, origin.y), dir);
        let excluded = filter.exclude_nodes.as_slice();
        let layers = filter.layers.bits();
        let mask = filter.mask.bits();
        let predicate = |handle, collider: &r2::Collider| {
            let collider_layers = collider.collision_groups().memberships.bits();
            (collider_layers & layers) != 0
                && (collider_layers & mask) == 0
                && world
                    .collider_owners
                    .get(&handle)
                    .map(|node| !excluded.contains(node))
                    .unwrap_or(true)
        };
        let query_filter = query_filter_2d(filter).predicate(&predicate);
        let (collider, hit) = world.query_pipeline.cast_ray_and_get_normal(
            &world.bodies,
            &world.colliders,
            &ray,
            max_distance,
            true,
            query_filter,
        )?;
        let node = *world.collider_owners.get(&collider)?;
        let point = ray.point_at(hit.time_of_impact);

        Some(PhysicsRayHit2D {
            node,
            point: Vector2::new(point.x, point.y),
            normal: Vector2::new(hit.normal.x, hit.normal.y),
            distance: hit.time_of_impact,
        })
    }

    pub fn raycast_3d(
        &mut self,
        origin: Vector3,
        direction: Vector3,
        max_distance: f32,
        include_areas: bool,
    ) -> Option<PhysicsRayHit3D> {
        self.raycast_3d_filtered(
            origin,
            direction,
            max_distance,
            &PhysicsQueryFilter {
                include_areas,
                ..PhysicsQueryFilter::default()
            },
        )
    }

    pub fn raycast_3d_filtered(
        &mut self,
        origin: Vector3,
        direction: Vector3,
        max_distance: f32,
        filter: &PhysicsQueryFilter,
    ) -> Option<PhysicsRayHit3D> {
        if max_distance <= 0.0 || !max_distance.is_finite() {
            return None;
        }

        let dir = na3::Vector3::new(direction.x, direction.y, direction.z);
        let dir_len = dir.norm();
        if dir_len <= 0.000_001 || !dir_len.is_finite() {
            return None;
        }
        let dir = dir / dir_len;

        let world = self.world_3d.as_mut()?;
        if self.query_pipeline_dirty_3d {
            world.query_pipeline.update(&world.colliders);
            self.query_pipeline_dirty_3d = false;
        }

        let ray = r3::Ray::new(na3::Point3::new(origin.x, origin.y, origin.z), dir);
        let excluded = filter.exclude_nodes.as_slice();
        let layers = filter.layers.bits();
        let mask = filter.mask.bits();
        let predicate = |handle, collider: &r3::Collider| {
            let collider_layers = collider.collision_groups().memberships.bits();
            (collider_layers & layers) != 0
                && (collider_layers & mask) == 0
                && world
                    .collider_owners
                    .get(&handle)
                    .map(|node| !excluded.contains(node))
                    .unwrap_or(true)
        };
        let query_filter = query_filter_3d(filter).predicate(&predicate);
        let (collider, hit) = world.query_pipeline.cast_ray_and_get_normal(
            &world.bodies,
            &world.colliders,
            &ray,
            max_distance,
            true,
            query_filter,
        )?;
        let node = *world.collider_owners.get(&collider)?;
        let point = ray.point_at(hit.time_of_impact);

        Some(PhysicsRayHit3D {
            node,
            point: Vector3::new(point.x, point.y, point.z),
            normal: Vector3::new(hit.normal.x, hit.normal.y, hit.normal.z),
            distance: hit.time_of_impact,
        })
    }

    pub fn shape_cast_2d(
        &mut self,
        shape: Shape2D,
        origin: Vector2,
        direction: Vector2,
        max_distance: f32,
        filter: &PhysicsQueryFilter,
    ) -> Option<PhysicsShapeHit2D> {
        if max_distance <= 0.0 || !max_distance.is_finite() {
            return None;
        }
        let dir = na2::Vector2::new(direction.x, direction.y);
        let dir_len = dir.norm();
        if dir_len <= 0.000_001 || !dir_len.is_finite() {
            return None;
        }

        let world = self.world_2d.as_mut()?;
        let shape = shared_shape_2d(shape)?;
        if self.query_pipeline_dirty_2d {
            world.query_pipeline.update(&world.colliders);
            self.query_pipeline_dirty_2d = false;
        }

        let shape_pos = na2::Isometry2::new(na2::Vector2::new(origin.x, origin.y), 0.0);
        let shape_vel = dir / dir_len * max_distance;
        let excluded = filter.exclude_nodes.as_slice();
        let layers = filter.layers.bits();
        let mask = filter.mask.bits();
        let predicate = |handle, collider: &r2::Collider| {
            let collider_layers = collider.collision_groups().memberships.bits();
            (collider_layers & layers) != 0
                && (collider_layers & mask) == 0
                && world
                    .collider_owners
                    .get(&handle)
                    .map(|node| !excluded.contains(node))
                    .unwrap_or(true)
        };
        let query_filter = query_filter_2d(filter).predicate(&predicate);
        let (collider, hit) = world.query_pipeline.cast_shape(
            &world.bodies,
            &world.colliders,
            &shape_pos,
            &shape_vel,
            shape.as_ref(),
            rapier2d::parry::query::ShapeCastOptions::with_max_time_of_impact(1.0),
            query_filter,
        )?;
        let node = *world.collider_owners.get(&collider)?;
        let point = hit.transform1_by(&shape_pos).witness1;

        Some(PhysicsShapeHit2D {
            node,
            point: Vector2::new(point.x, point.y),
            normal: Vector2::new(hit.normal1.x, hit.normal1.y),
            distance: hit.time_of_impact * max_distance,
        })
    }

    pub fn shape_cast_3d(
        &mut self,
        shape: Shape3D,
        origin: Vector3,
        direction: Vector3,
        max_distance: f32,
        filter: &PhysicsQueryFilter,
    ) -> Option<PhysicsShapeHit3D> {
        if max_distance <= 0.0 || !max_distance.is_finite() {
            return None;
        }
        let dir = na3::Vector3::new(direction.x, direction.y, direction.z);
        let dir_len = dir.norm();
        if dir_len <= 0.000_001 || !dir_len.is_finite() {
            return None;
        }

        let world = self.world_3d.as_mut()?;
        let shape = shared_shape_3d(shape)?;
        if self.query_pipeline_dirty_3d {
            world.query_pipeline.update(&world.colliders);
            self.query_pipeline_dirty_3d = false;
        }

        let shape_pos = na3::Isometry3::translation(origin.x, origin.y, origin.z);
        let shape_vel = dir / dir_len * max_distance;
        let excluded = filter.exclude_nodes.as_slice();
        let layers = filter.layers.bits();
        let mask = filter.mask.bits();
        let predicate = |handle, collider: &r3::Collider| {
            let collider_layers = collider.collision_groups().memberships.bits();
            (collider_layers & layers) != 0
                && (collider_layers & mask) == 0
                && world
                    .collider_owners
                    .get(&handle)
                    .map(|node| !excluded.contains(node))
                    .unwrap_or(true)
        };
        let query_filter = query_filter_3d(filter).predicate(&predicate);
        let (collider, hit) = world.query_pipeline.cast_shape(
            &world.bodies,
            &world.colliders,
            &shape_pos,
            &shape_vel,
            shape.as_ref(),
            rapier3d::parry::query::ShapeCastOptions::with_max_time_of_impact(1.0),
            query_filter,
        )?;
        let node = *world.collider_owners.get(&collider)?;
        let point = hit.transform1_by(&shape_pos).witness1;

        Some(PhysicsShapeHit3D {
            node,
            point: Vector3::new(point.x, point.y, point.z),
            normal: Vector3::new(hit.normal1.x, hit.normal1.y, hit.normal1.z),
            distance: hit.time_of_impact * max_distance,
        })
    }

    pub fn move_body_2d(
        &mut self,
        body_id: NodeID,
        target: Vector2,
        margin: f32,
        filter: &PhysicsQueryFilter,
    ) -> Option<PhysicsMoveResult2D> {
        if !target.x.is_finite() || !target.y.is_finite() {
            return None;
        }
        let world = self.world_2d.as_mut()?;
        if self.query_pipeline_dirty_2d {
            world.query_pipeline.update(&world.colliders);
            self.query_pipeline_dirty_2d = false;
        }
        let state = world.body_map.get(&body_id)?;
        let rb = world.bodies.get(state.handle)?;
        let start_orig = *rb.position();

        let excluded = filter.exclude_nodes.as_slice();
        let layers = filter.layers.bits();
        let mask = filter.mask.bits();
        let predicate = |handle, collider: &r2::Collider| {
            let Some(owner) = world.collider_owners.get(&handle).copied() else {
                return true;
            };
            if owner == body_id || excluded.contains(&owner) {
                return false;
            }
            let collider_layers = collider.collision_groups().memberships.bits();
            (collider_layers & layers) != 0 && (collider_layers & mask) == 0
        };
        let query_filter = query_filter_2d(filter).predicate(&predicate);

        // Godot-style depenetration: push body out of any pre-existing overlap
        // before sweeping, so a body starting inside a static collider is not
        // stuck w/ time_of_impact=0 in every direction.
        let skin = margin.max(RECOVERY_SKIN_2D);
        let mut recovery = na2::Vector2::zeros();
        for _ in 0..RECOVERY_ITERATIONS {
            let mut iter_push = na2::Vector2::zeros();
            let mut penetrating = false;
            for collider_handle in &state.colliders {
                let Some(collider) = world.colliders.get(*collider_handle) else {
                    continue;
                };
                if collider.is_sensor() {
                    continue;
                }
                let local_pos = collider
                    .position_wrt_parent()
                    .copied()
                    .unwrap_or_else(|| *collider.position());
                let shape_pos = na2::Translation2::from(recovery) * start_orig * local_pos;
                let shape = collider.shape();
                let mut hits: Vec<r2::ColliderHandle> = Vec::new();
                world.query_pipeline.intersections_with_shape(
                    &world.bodies,
                    &world.colliders,
                    &shape_pos,
                    shape,
                    query_filter,
                    |handle| {
                        hits.push(handle);
                        true
                    },
                );
                for other_handle in hits {
                    let Some(other) = world.colliders.get(other_handle) else {
                        continue;
                    };
                    let Ok(Some(contact)) = rapier2d::parry::query::contact(
                        &shape_pos,
                        shape,
                        other.position(),
                        other.shape(),
                        skin,
                    ) else {
                        continue;
                    };
                    if contact.dist < skin {
                        penetrating = true;
                        let depth = skin - contact.dist;
                        iter_push -= contact.normal1.into_inner() * depth;
                    }
                }
            }
            if !penetrating {
                break;
            }
            recovery += iter_push;
            let recovery_len = recovery.norm();
            if recovery_len > RECOVERY_MAX_2D {
                recovery *= RECOVERY_MAX_2D / recovery_len;
                break;
            }
        }

        let start = na2::Translation2::from(recovery) * start_orig;
        let delta = na2::Vector2::new(
            target.x - start.translation.vector.x,
            target.y - start.translation.vector.y,
        );
        let distance = delta.norm();
        if distance <= 0.000_001 {
            return Some(PhysicsMoveResult2D {
                position: Vector2::new(start.translation.vector.x, start.translation.vector.y),
                hit: None,
                clipped: false,
            });
        }

        let mut best: Option<PhysicsShapeHit2D> = None;
        for collider_handle in &state.colliders {
            let Some(collider) = world.colliders.get(*collider_handle) else {
                continue;
            };
            if collider.is_sensor() {
                continue;
            }
            let local_pos = collider
                .position_wrt_parent()
                .copied()
                .unwrap_or_else(|| *collider.position());
            let shape_pos = start * local_pos;
            let Some((hit_collider, hit)) = world.query_pipeline.cast_shape(
                &world.bodies,
                &world.colliders,
                &shape_pos,
                &delta,
                collider.shape(),
                rapier2d::parry::query::ShapeCastOptions::with_max_time_of_impact(1.0),
                query_filter,
            ) else {
                continue;
            };
            let node = *world.collider_owners.get(&hit_collider)?;
            let point = hit.transform1_by(&shape_pos).witness1;
            let hit_out = PhysicsShapeHit2D {
                node,
                point: Vector2::new(point.x, point.y),
                normal: Vector2::new(hit.normal1.x, hit.normal1.y),
                distance: hit.time_of_impact * distance,
            };
            if best.is_none_or(|best| hit_out.distance < best.distance) {
                best = Some(hit_out);
            }
        }

        let hit = best;
        let clipped = hit.is_some();
        let travel = hit
            .map(|hit| (hit.distance - margin.max(0.0)).max(0.0))
            .unwrap_or(distance);
        let dir = delta / distance;
        let position = Vector2::new(
            start.translation.vector.x + dir.x * travel,
            start.translation.vector.y + dir.y * travel,
        );
        Some(PhysicsMoveResult2D {
            position,
            hit,
            clipped,
        })
    }

    pub fn move_body_3d(
        &mut self,
        body_id: NodeID,
        target: Vector3,
        margin: f32,
        filter: &PhysicsQueryFilter,
    ) -> Option<PhysicsMoveResult3D> {
        if !target.x.is_finite() || !target.y.is_finite() || !target.z.is_finite() {
            return None;
        }
        let world = self.world_3d.as_mut()?;
        if self.query_pipeline_dirty_3d {
            world.query_pipeline.update(&world.colliders);
            self.query_pipeline_dirty_3d = false;
        }
        let state = world.body_map.get(&body_id)?;
        let rb = world.bodies.get(state.handle)?;
        let start_orig = *rb.position();

        let excluded = filter.exclude_nodes.as_slice();
        let layers = filter.layers.bits();
        let mask = filter.mask.bits();
        let predicate = |handle, collider: &r3::Collider| {
            let Some(owner) = world.collider_owners.get(&handle).copied() else {
                return true;
            };
            if owner == body_id || excluded.contains(&owner) {
                return false;
            }
            let collider_layers = collider.collision_groups().memberships.bits();
            (collider_layers & layers) != 0 && (collider_layers & mask) == 0
        };
        let query_filter = query_filter_3d(filter).predicate(&predicate);

        // Godot-style depenetration: push body out of any pre-existing overlap
        // before sweeping, so a body starting inside a static collider is not
        // stuck w/ time_of_impact=0 in every direction.
        let skin = margin.max(RECOVERY_SKIN_3D);
        let mut recovery = na3::Vector3::zeros();
        for _ in 0..RECOVERY_ITERATIONS {
            let mut iter_push = na3::Vector3::zeros();
            let mut penetrating = false;
            for collider_handle in &state.colliders {
                let Some(collider) = world.colliders.get(*collider_handle) else {
                    continue;
                };
                if collider.is_sensor() {
                    continue;
                }
                let local_pos = collider
                    .position_wrt_parent()
                    .copied()
                    .unwrap_or_else(|| *collider.position());
                let shape_pos = na3::Translation3::from(recovery) * start_orig * local_pos;
                let shape = collider.shape();
                let mut hits: Vec<r3::ColliderHandle> = Vec::new();
                world.query_pipeline.intersections_with_shape(
                    &world.bodies,
                    &world.colliders,
                    &shape_pos,
                    shape,
                    query_filter,
                    |handle| {
                        hits.push(handle);
                        true
                    },
                );
                for other_handle in hits {
                    let Some(other) = world.colliders.get(other_handle) else {
                        continue;
                    };
                    let Ok(Some(contact)) = rapier3d::parry::query::contact(
                        &shape_pos,
                        shape,
                        other.position(),
                        other.shape(),
                        skin,
                    ) else {
                        continue;
                    };
                    if contact.dist < skin {
                        penetrating = true;
                        let depth = skin - contact.dist;
                        iter_push -= contact.normal1.into_inner() * depth;
                    }
                }
            }
            if !penetrating {
                break;
            }
            recovery += iter_push;
            let recovery_len = recovery.norm();
            if recovery_len > RECOVERY_MAX_3D {
                recovery *= RECOVERY_MAX_3D / recovery_len;
                break;
            }
        }

        let start = na3::Translation3::from(recovery) * start_orig;
        let delta = na3::Vector3::new(
            target.x - start.translation.vector.x,
            target.y - start.translation.vector.y,
            target.z - start.translation.vector.z,
        );
        let distance = delta.norm();
        if distance <= 0.000_001 {
            return Some(PhysicsMoveResult3D {
                position: Vector3::new(
                    start.translation.vector.x,
                    start.translation.vector.y,
                    start.translation.vector.z,
                ),
                hit: None,
                clipped: false,
            });
        }

        let mut best: Option<PhysicsShapeHit3D> = None;
        for collider_handle in &state.colliders {
            let Some(collider) = world.colliders.get(*collider_handle) else {
                continue;
            };
            if collider.is_sensor() {
                continue;
            }
            let local_pos = collider
                .position_wrt_parent()
                .copied()
                .unwrap_or_else(|| *collider.position());
            let shape_pos = start * local_pos;
            let Some((hit_collider, hit)) = world.query_pipeline.cast_shape(
                &world.bodies,
                &world.colliders,
                &shape_pos,
                &delta,
                collider.shape(),
                rapier3d::parry::query::ShapeCastOptions::with_max_time_of_impact(1.0),
                query_filter,
            ) else {
                continue;
            };
            let node = *world.collider_owners.get(&hit_collider)?;
            let point = hit.transform1_by(&shape_pos).witness1;
            let hit_out = PhysicsShapeHit3D {
                node,
                point: Vector3::new(point.x, point.y, point.z),
                normal: Vector3::new(hit.normal1.x, hit.normal1.y, hit.normal1.z),
                distance: hit.time_of_impact * distance,
            };
            if best.is_none_or(|best| hit_out.distance < best.distance) {
                best = Some(hit_out);
            }
        }

        let hit = best;
        let clipped = hit.is_some();
        let travel = hit
            .map(|hit| (hit.distance - margin.max(0.0)).max(0.0))
            .unwrap_or(distance);
        let dir = delta / distance;
        let position = Vector3::new(
            start.translation.vector.x + dir.x * travel,
            start.translation.vector.y + dir.y * travel,
            start.translation.vector.z + dir.z * travel,
        );
        Some(PhysicsMoveResult3D {
            position,
            hit,
            clipped,
        })
    }

    pub fn contacts_2d(&self, body_id: NodeID) -> Vec<PhysicsContact2D> {
        let Some(world) = self.world_2d.as_ref() else {
            return Vec::new();
        };
        let mut out = Vec::new();
        for pair in world.narrow_phase.contact_pairs() {
            if !pair.has_any_active_contact {
                continue;
            }
            let Some(&a) = world.collider_owners.get(&pair.collider1) else {
                continue;
            };
            let Some(&b) = world.collider_owners.get(&pair.collider2) else {
                continue;
            };
            let other = if a == body_id {
                b
            } else if b == body_id {
                a
            } else {
                continue;
            };
            for manifold in &pair.manifolds {
                let normal = if a == body_id {
                    manifold.data.normal
                } else {
                    -manifold.data.normal
                };
                for contact in &manifold.data.solver_contacts {
                    out.push(PhysicsContact2D {
                        node: other,
                        point: Vector2::new(contact.point.x, contact.point.y),
                        normal: Vector2::new(normal.x, normal.y),
                        impulse: contact.warmstart_impulse,
                    });
                }
            }
        }
        out
    }

    pub fn contacts_3d(&self, body_id: NodeID) -> Vec<PhysicsContact3D> {
        let Some(world) = self.world_3d.as_ref() else {
            return Vec::new();
        };
        let mut out = Vec::new();
        for pair in world.narrow_phase.contact_pairs() {
            if !pair.has_any_active_contact {
                continue;
            }
            let Some(&a) = world.collider_owners.get(&pair.collider1) else {
                continue;
            };
            let Some(&b) = world.collider_owners.get(&pair.collider2) else {
                continue;
            };
            let other = if a == body_id {
                b
            } else if b == body_id {
                a
            } else {
                continue;
            };
            for manifold in &pair.manifolds {
                let normal = if a == body_id {
                    manifold.data.normal
                } else {
                    -manifold.data.normal
                };
                for contact in &manifold.data.solver_contacts {
                    out.push(PhysicsContact3D {
                        node: other,
                        point: Vector3::new(contact.point.x, contact.point.y, contact.point.z),
                        normal: Vector3::new(normal.x, normal.y, normal.z),
                        impulse: contact.warmstart_impulse,
                    });
                }
            }
        }
        out
    }

    pub fn update_query_pipeline_2d(&mut self) {
        if !self.query_pipeline_dirty_2d {
            return;
        }
        if let Some(world) = self.world_2d.as_mut() {
            world.query_pipeline.update(&world.colliders);
            self.query_pipeline_dirty_2d = false;
        }
    }

    pub fn update_query_pipeline_3d(&mut self) {
        if !self.query_pipeline_dirty_3d {
            return;
        }
        if let Some(world) = self.world_3d.as_mut() {
            world.query_pipeline.update(&world.colliders);
            self.query_pipeline_dirty_3d = false;
        }
    }
}

#[cfg(test)]
mod recovery_tests {
    use super::*;
    use crate::{
        BodyDesc2D, BodyDesc3D, BodyKind, PhysicsAssetContext, PhysicsProviderMode, ShapeDesc2D,
        ShapeDesc3D, ShapeKind2D, ShapeKind3D,
    };
    use perro_nodes::{Shape2D, Shape3D};
    use perro_structs::{BitMask, Quaternion, Transform2D, Transform3D};

    fn asset_context() -> PhysicsAssetContext {
        PhysicsAssetContext {
            provider_mode: PhysicsProviderMode::Dynamic,
            static_mesh_lookup: None,
            static_collision_trimesh_lookup: None,
        }
    }

    fn shape_2d(shape: Shape2D) -> ShapeDesc2D {
        ShapeDesc2D {
            local: Transform2D::IDENTITY,
            shape: ShapeKind2D::Primitive(shape),
            sensor: false,
            collision_layers: BitMask::ALL,
            collision_mask: BitMask::ALL,
            friction: 0.7,
            restitution: 0.0,
            density: 1.0,
        }
    }

    fn shape_3d(shape: Shape3D) -> ShapeDesc3D {
        ShapeDesc3D {
            local: Transform3D::IDENTITY,
            shape: ShapeKind3D::Primitive(shape),
            sensor: false,
            collision_layers: BitMask::ALL,
            collision_mask: BitMask::ALL,
            friction: 0.7,
            restitution: 0.0,
            density: 1.0,
        }
    }

    fn static_2d(id: NodeID, pos: Vector2, shape: Shape2D) -> BodyDesc2D {
        BodyDesc2D {
            id,
            kind: BodyKind::Static,
            enabled: true,
            global: Transform2D::new(pos, 0.0, Vector2::ONE),
            rigid: None,
            sync_signature: id.as_u64(),
            shape_signature: 1,
            shapes: vec![shape_2d(shape)],
        }
    }

    fn char_2d(id: NodeID, pos: Vector2, shape: Shape2D) -> BodyDesc2D {
        BodyDesc2D {
            id,
            kind: BodyKind::Character,
            enabled: true,
            global: Transform2D::new(pos, 0.0, Vector2::ONE),
            rigid: None,
            sync_signature: id.as_u64(),
            shape_signature: 2,
            shapes: vec![shape_2d(shape)],
        }
    }

    fn static_3d(id: NodeID, pos: Vector3, shape: Shape3D) -> BodyDesc3D {
        BodyDesc3D {
            id,
            kind: BodyKind::Static,
            enabled: true,
            global: Transform3D::new(pos, Quaternion::IDENTITY, Vector3::ONE),
            rigid: None,
            sync_signature: id.as_u64(),
            shape_signature: 1,
            shapes: vec![shape_3d(shape)],
        }
    }

    fn char_3d(id: NodeID, pos: Vector3, shape: Shape3D) -> BodyDesc3D {
        BodyDesc3D {
            id,
            kind: BodyKind::Character,
            enabled: true,
            global: Transform3D::new(pos, Quaternion::IDENTITY, Vector3::ONE),
            rigid: None,
            sync_signature: id.as_u64(),
            shape_signature: 2,
            shapes: vec![shape_3d(shape)],
        }
    }

    fn filter() -> PhysicsQueryFilter {
        PhysicsQueryFilter::default()
    }

    fn body_teleport_2d(system: &mut PhysicsSystem, id: NodeID, pos: Vector2) {
        let world = system.world_2d.as_mut().unwrap();
        let handle = world.body_map.get(&id).unwrap().handle;
        let rb = world.bodies.get_mut(handle).unwrap();
        rb.set_position(na2::Isometry2::translation(pos.x, pos.y), true);
        system.query_pipeline_dirty_2d = true;
    }

    fn body_teleport_3d(system: &mut PhysicsSystem, id: NodeID, pos: Vector3) {
        let world = system.world_3d.as_mut().unwrap();
        let handle = world.body_map.get(&id).unwrap().handle;
        let rb = world.bodies.get_mut(handle).unwrap();
        rb.set_position(na3::Isometry3::translation(pos.x, pos.y, pos.z), true);
        system.query_pipeline_dirty_3d = true;
    }

    // ---- 2D ----

    #[test]
    fn recovers_from_overlap_and_moves_2d() {
        let mut system = PhysicsSystem::new();
        // static box at origin, half-extent 1.0.
        let wall = static_2d(
            NodeID::new(1),
            Vector2::new(0.0, 0.0),
            Shape2D::Quad {
                width: 2.0,
                height: 2.0,
            },
        );
        // char body straddling the +x face (x=1.0): penetrates 0.3, mostly outside.
        let body = char_2d(
            NodeID::new(2),
            Vector2::new(1.1, 0.0),
            Shape2D::Circle { radius: 0.4 },
        );
        system.sync_world_2d(&[wall, body], |_, _| {});

        // Pure sweep would freeze at time_of_impact=0. Recovery pushes out along the
        // +x face normal. Clamp is 0.2/call, so a couple calls fully depenetrate.
        let mut pos = Vector2::new(1.1, 0.0);
        let mut prev_x = pos.x;
        let mut cleared = false;
        for _ in 0..8 {
            let res = system
                .move_body_2d(
                    NodeID::new(2),
                    Vector2::new(pos.x + 0.01, pos.y),
                    0.001,
                    &filter(),
                )
                .unwrap();
            assert!(
                res.position.x >= prev_x - 0.001,
                "recovery went wrong direction, x={}",
                res.position.x
            );
            // drive the character body pose to the resolved position.
            body_teleport_2d(&mut system, NodeID::new(2), res.position);
            prev_x = res.position.x;
            pos = res.position;
            // clear once outside the +x face by at least the radius.
            if pos.x >= 1.0 + 0.4 - 0.001 {
                cleared = true;
                break;
            }
        }
        assert!(cleared, "body never recovered out of wall, x={}", pos.x);

        // now free of the wall, a move further +x succeeds fully.
        let res = system
            .move_body_2d(
                NodeID::new(2),
                Vector2::new(pos.x + 1.0, pos.y),
                0.001,
                &filter(),
            )
            .unwrap();
        assert!(
            (res.position.x - (pos.x + 1.0)).abs() < 0.01,
            "free move blocked, x={}",
            res.position.x
        );
    }

    #[test]
    fn resting_contact_slides_2d() {
        let mut system = PhysicsSystem::new();
        // floor top at y=0.
        let floor = static_2d(
            NodeID::new(1),
            Vector2::new(0.0, -1.0),
            Shape2D::Quad {
                width: 40.0,
                height: 2.0,
            },
        );
        // circle resting just on top, touching w/ margin.
        let body = char_2d(
            NodeID::new(2),
            Vector2::new(0.0, 0.4),
            Shape2D::Circle { radius: 0.4 },
        );
        system.sync_world_2d(&[floor, body], |_, _| {});

        // slide +x along the floor.
        let res = system
            .move_body_2d(NodeID::new(2), Vector2::new(1.0, 0.4), 0.001, &filter())
            .unwrap();
        // full horizontal travel, no vertical pop.
        assert!(res.position.x > 0.9, "slide blocked, x={}", res.position.x);
        assert!(
            (res.position.y - 0.4).abs() < 0.05,
            "unexpected vertical pop, y={}",
            res.position.y
        );
    }

    #[test]
    fn deep_overlap_no_tunnel_2d() {
        let mut system = PhysicsSystem::new();
        // thin wall centered at x=0, half-thickness 0.05.
        let wall = static_2d(
            NodeID::new(1),
            Vector2::new(0.0, 0.0),
            Shape2D::Quad {
                width: 0.1,
                height: 4.0,
            },
        );
        // body straddling the wall from the -x side.
        let body = char_2d(
            NodeID::new(2),
            Vector2::new(-0.02, 0.0),
            Shape2D::Circle { radius: 0.4 },
        );
        system.sync_world_2d(&[wall, body], |_, _| {});

        let mut x = -0.02;
        for _ in 0..12 {
            let res = system
                .move_body_2d(NodeID::new(2), Vector2::new(x, 0.0), 0.001, &filter())
                .unwrap();
            x = res.position.x;
            body_teleport_2d(&mut system, NodeID::new(2), Vector2::new(x, 0.0));
            // never tunnel to the far (+x) side of the thin wall.
            assert!(x <= 0.05 + 0.4, "tunneled through thin wall, x={x}");
        }
        // recovered fully to -x side (out of the wall).
        assert!(
            x <= -0.05 - 0.4 + 0.01,
            "did not recover clear of wall, x={x}"
        );
    }

    // ---- 3D ----

    #[test]
    fn recovers_from_overlap_and_moves_3d() {
        let mut system = PhysicsSystem::new();
        let wall = static_3d(
            NodeID::new(1),
            Vector3::new(0.0, 0.0, 0.0),
            Shape3D::Cube {
                size: Vector3::new(2.0, 2.0, 2.0),
            },
        );
        // straddle +x face (x=1.0): penetrate 0.3, mostly outside.
        let body = char_3d(
            NodeID::new(2),
            Vector3::new(1.1, 0.0, 0.0),
            Shape3D::Sphere { radius: 0.4 },
        );
        system.sync_world_3d(&[wall, body], asset_context(), |_, _| {});

        let mut pos = Vector3::new(1.1, 0.0, 0.0);
        let mut prev_x = pos.x;
        let mut cleared = false;
        for _ in 0..8 {
            let res = system
                .move_body_3d(
                    NodeID::new(2),
                    Vector3::new(pos.x + 0.01, pos.y, pos.z),
                    0.001,
                    &filter(),
                )
                .unwrap();
            assert!(
                res.position.x >= prev_x - 0.001,
                "recovery went wrong direction, x={}",
                res.position.x
            );
            body_teleport_3d(&mut system, NodeID::new(2), res.position);
            prev_x = res.position.x;
            pos = res.position;
            if pos.x >= 1.0 + 0.4 - 0.001 {
                cleared = true;
                break;
            }
        }
        assert!(cleared, "body never recovered out of wall, x={}", pos.x);

        let res = system
            .move_body_3d(
                NodeID::new(2),
                Vector3::new(pos.x + 1.0, pos.y, pos.z),
                0.001,
                &filter(),
            )
            .unwrap();
        assert!(
            (res.position.x - (pos.x + 1.0)).abs() < 0.01,
            "free move blocked, x={}",
            res.position.x
        );
    }

    #[test]
    fn resting_contact_slides_3d() {
        let mut system = PhysicsSystem::new();
        let floor = static_3d(
            NodeID::new(1),
            Vector3::new(0.0, -1.0, 0.0),
            Shape3D::Cube {
                size: Vector3::new(40.0, 2.0, 40.0),
            },
        );
        // rest just above the floor w/ a thin margin gap (floor top y=0).
        let body = char_3d(
            NodeID::new(2),
            Vector3::new(0.0, 0.405, 0.0),
            Shape3D::Sphere { radius: 0.4 },
        );
        system.sync_world_3d(&[floor, body], asset_context(), |_, _| {});

        let res = system
            .move_body_3d(
                NodeID::new(2),
                Vector3::new(1.0, 0.405, 0.0),
                0.001,
                &filter(),
            )
            .unwrap();
        assert!(res.position.x > 0.9, "slide blocked, x={}", res.position.x);
        assert!(
            (res.position.y - 0.405).abs() < 0.05,
            "unexpected vertical pop, y={}",
            res.position.y
        );
    }

    #[test]
    fn deep_overlap_no_tunnel_3d() {
        let mut system = PhysicsSystem::new();
        let wall = static_3d(
            NodeID::new(1),
            Vector3::new(0.0, 0.0, 0.0),
            Shape3D::Cube {
                size: Vector3::new(0.1, 4.0, 4.0),
            },
        );
        let body = char_3d(
            NodeID::new(2),
            Vector3::new(-0.02, 0.0, 0.0),
            Shape3D::Sphere { radius: 0.4 },
        );
        system.sync_world_3d(&[wall, body], asset_context(), |_, _| {});

        let mut x = -0.02;
        for _ in 0..12 {
            let res = system
                .move_body_3d(NodeID::new(2), Vector3::new(x, 0.0, 0.0), 0.001, &filter())
                .unwrap();
            x = res.position.x;
            body_teleport_3d(&mut system, NodeID::new(2), Vector3::new(x, 0.0, 0.0));
            assert!(x <= 0.05 + 0.4, "tunneled through thin wall, x={x}");
        }
        assert!(
            x <= -0.05 - 0.4 + 0.01,
            "did not recover clear of wall, x={x}"
        );
    }
}
