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

        let dir = r2::Vector::new(direction.x, direction.y);
        let dir_len = dir.length();
        if dir_len <= 0.000_001 || !dir_len.is_finite() {
            return None;
        }
        let dir = dir / dir_len;

        let world = self.world_2d.as_ref()?;

        let ray = r2::Ray::new(r2::Vector::new(origin.x, origin.y), dir);
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
        let query_pipeline = query_pipeline_2d(world, query_filter);
        let (collider, hit) = query_pipeline.cast_ray_and_get_normal(&ray, max_distance, true)?;
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

        let dir = r3::Vector::new(direction.x, direction.y, direction.z);
        let dir_len = dir.length();
        if dir_len <= 0.000_001 || !dir_len.is_finite() {
            return None;
        }
        let dir = dir / dir_len;

        let world = self.world_3d.as_ref()?;

        let ray = r3::Ray::new(r3::Vector::new(origin.x, origin.y, origin.z), dir);
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
        let query_pipeline = query_pipeline_3d(world, query_filter);
        let (collider, hit) = query_pipeline.cast_ray_and_get_normal(&ray, max_distance, true)?;
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
        let dir = r2::Vector::new(direction.x, direction.y);
        let dir_len = dir.length();
        if dir_len <= 0.000_001 || !dir_len.is_finite() {
            return None;
        }

        let world = self.world_2d.as_mut()?;
        let shape = shared_shape_2d(shape)?;
        let shape_pos = r2::Pose::new(r2::Vector::new(origin.x, origin.y), 0.0);
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
        let query_pipeline = query_pipeline_2d(world, query_filter);
        let (collider, hit) = query_pipeline.cast_shape(
            &shape_pos,
            shape_vel,
            shape.as_ref(),
            rapier2d::parry::query::ShapeCastOptions::with_max_time_of_impact(1.0),
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
        let dir = r3::Vector::new(direction.x, direction.y, direction.z);
        let dir_len = dir.length();
        if dir_len <= 0.000_001 || !dir_len.is_finite() {
            return None;
        }

        let world = self.world_3d.as_mut()?;
        let shape = shared_shape_3d(shape)?;
        let shape_pos = r3::Pose::translation(origin.x, origin.y, origin.z);
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
        let query_pipeline = query_pipeline_3d(world, query_filter);
        let (collider, hit) = query_pipeline.cast_shape(
            &shape_pos,
            shape_vel,
            shape.as_ref(),
            rapier3d::parry::query::ShapeCastOptions::with_max_time_of_impact(1.0),
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
        let mut recovery = r2::Vector::ZERO;
        // scratch hit buf: kp alloc across calls (up to 16 fills / char / frame).
        let mut hits = std::mem::take(&mut self.recovery_hits_2d);
        for _ in 0..RECOVERY_ITERATIONS {
            let mut iter_push = r2::Vector::ZERO;
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
                let shape_pos = r2::Pose::from_translation(recovery) * start_orig * local_pos;
                let shape = collider.shape();
                hits.clear();
                let query_pipeline = query_pipeline_2d(world, query_filter);
                hits.extend(
                    query_pipeline
                        .intersect_shape(shape_pos, shape)
                        .map(|(handle, _)| handle),
                );
                for other_handle in hits.iter().copied() {
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
                        iter_push -= contact.normal1 * depth;
                    }
                }
            }
            if !penetrating {
                break;
            }
            recovery += iter_push;
            let recovery_len = recovery.length();
            if recovery_len > RECOVERY_MAX_2D {
                recovery *= RECOVERY_MAX_2D / recovery_len;
                break;
            }
        }
        hits.clear();
        self.recovery_hits_2d = hits;

        let start = r2::Pose::from_translation(recovery) * start_orig;
        let delta = r2::Vector::new(
            target.x - start.translation.x,
            target.y - start.translation.y,
        );
        let distance = delta.length();
        if distance <= 0.000_001 {
            return Some(PhysicsMoveResult2D {
                position: Vector2::new(start.translation.x, start.translation.y),
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
            let query_pipeline = query_pipeline_2d(world, query_filter);
            let Some((hit_collider, hit)) = query_pipeline.cast_shape(
                &shape_pos,
                delta,
                collider.shape(),
                rapier2d::parry::query::ShapeCastOptions::with_max_time_of_impact(1.0),
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
            start.translation.x + dir.x * travel,
            start.translation.y + dir.y * travel,
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
        let mut recovery = r3::Vector::ZERO;
        // scratch hit buf: kp alloc across calls (up to 16 fills / char / frame).
        let mut hits = std::mem::take(&mut self.recovery_hits_3d);
        for _ in 0..RECOVERY_ITERATIONS {
            let mut iter_push = r3::Vector::ZERO;
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
                let shape_pos = r3::Pose::from_translation(recovery) * start_orig * local_pos;
                let shape = collider.shape();
                hits.clear();
                let query_pipeline = query_pipeline_3d(world, query_filter);
                hits.extend(
                    query_pipeline
                        .intersect_shape(shape_pos, shape)
                        .map(|(handle, _)| handle),
                );
                for other_handle in hits.iter().copied() {
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
                        iter_push -= contact.normal1 * depth;
                    }
                }
            }
            if !penetrating {
                break;
            }
            recovery += iter_push;
            let recovery_len = recovery.length();
            if recovery_len > RECOVERY_MAX_3D {
                recovery *= RECOVERY_MAX_3D / recovery_len;
                break;
            }
        }
        hits.clear();
        self.recovery_hits_3d = hits;

        let start = r3::Pose::from_translation(recovery) * start_orig;
        let delta = r3::Vector::new(
            target.x - start.translation.x,
            target.y - start.translation.y,
            target.z - start.translation.z,
        );
        let distance = delta.length();
        if distance <= 0.000_001 {
            return Some(PhysicsMoveResult3D {
                position: Vector3::new(
                    start.translation.x,
                    start.translation.y,
                    start.translation.z,
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
            let query_pipeline = query_pipeline_3d(world, query_filter);
            let Some((hit_collider, hit)) = query_pipeline.cast_shape(
                &shape_pos,
                delta,
                collider.shape(),
                rapier3d::parry::query::ShapeCastOptions::with_max_time_of_impact(1.0),
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
            start.translation.x + dir.x * travel,
            start.translation.y + dir.y * travel,
            start.translation.z + dir.z * travel,
        );
        Some(PhysicsMoveResult3D {
            position,
            hit,
            clipped,
        })
    }

    /// write resolved move pose straight into rapier body + refresh signature,
    /// replicate what next full sync_world_2d would do 4 this one body.
    /// let move_and_slide / apply_gravity skip O(bodies) collect+sync per iter.
    /// ret false when body absent (caller fall back to full invalidate).
    /// `global` = re-read node global aft set (== collect input) so pose +
    /// sig match sync_world exactly; rot kp (move only translates).
    pub fn commit_moved_body_2d(
        &mut self,
        body_id: NodeID,
        global: perro_structs::Transform2D,
        sync_signature: u64,
    ) -> bool {
        let Some(world) = self.world_2d.as_mut() else {
            return false;
        };
        let Some(state) = world.body_map.get_mut(&body_id) else {
            return false;
        };
        let Some(rb) = world.bodies.get_mut(state.handle) else {
            return false;
        };
        // set_position(., true) wake body -> mirror sync_world_2d wake.
        rb.set_position(transform_to_iso2(global), true);
        let woke_dynamic = rb.is_dynamic();
        state.sync_signature = sync_signature;
        patch_body_colliders_bvh_2d(
            &world.bodies,
            &mut world.colliders,
            &mut world.broad_phase,
            &world.integration_parameters,
            state.handle,
        );
        // O(1) idle upkeep: kinematic/static commit can't chg dynamic sleep, so
        // cached val stay valid. dynamic commit wake the body (! yet in rapier's
        // active set) => force next step; post-step refresh re-derives cache.
        if woke_dynamic {
            self.world_2d_idle_cached = false;
        }
        true
    }

    /// 3d twin of [`Self::commit_moved_body_2d`].
    pub fn commit_moved_body_3d(
        &mut self,
        body_id: NodeID,
        global: perro_structs::Transform3D,
        sync_signature: u64,
    ) -> bool {
        let Some(world) = self.world_3d.as_mut() else {
            return false;
        };
        let Some(state) = world.body_map.get_mut(&body_id) else {
            return false;
        };
        let Some(rb) = world.bodies.get_mut(state.handle) else {
            return false;
        };
        rb.set_position(transform_to_iso3(global), true);
        let woke_dynamic = rb.is_dynamic();
        state.sync_signature = sync_signature;
        patch_body_colliders_bvh_3d(
            &world.bodies,
            &mut world.colliders,
            &mut world.broad_phase,
            &world.integration_parameters,
            state.handle,
        );
        // see commit_moved_body_2d idle-cache note.
        if woke_dynamic {
            self.world_3d_idle_cached = false;
        }
        true
    }

    pub fn contacts_2d(&self, body_id: NodeID) -> Vec<PhysicsContact2D> {
        let mut out = Vec::new();
        self.contacts_2d_into(body_id, &mut out);
        out
    }

    /// alloc-free variant of [`Self::contacts_2d`]: clear + fill `out`.
    /// walk only this body's collider contact pairs (narrow-phase graph),
    /// ! the whole world's pair list.
    pub fn contacts_2d_into(&self, body_id: NodeID, out: &mut Vec<PhysicsContact2D>) {
        out.clear();
        let Some(world) = self.world_2d.as_ref() else {
            return;
        };
        let Some(state) = world.body_map.get(&body_id) else {
            return;
        };
        for collider_handle in &state.colliders {
            for pair in world.narrow_phase.contact_pairs_with(*collider_handle) {
                if !pair.has_any_active_contact() {
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
        }
    }

    pub fn contacts_3d(&self, body_id: NodeID) -> Vec<PhysicsContact3D> {
        let mut out = Vec::new();
        self.contacts_3d_into(body_id, &mut out);
        out
    }

    /// 3d twin of [`Self::contacts_2d_into`].
    pub fn contacts_3d_into(&self, body_id: NodeID, out: &mut Vec<PhysicsContact3D>) {
        out.clear();
        let Some(world) = self.world_3d.as_ref() else {
            return;
        };
        let Some(state) = world.body_map.get(&body_id) else {
            return;
        };
        for collider_handle in &state.colliders {
            for pair in world.narrow_phase.contact_pairs_with(*collider_handle) {
                if !pair.has_any_active_contact() {
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
        }
    }
}

/// Propagate a just-moved body's pose to its colliders + patch their leaves in
/// the broad-phase BVH (O(k log n)). Replaces the old dirty-flag full-tree
/// rebuild, which re-ran O(n log n) per moved body per query batch — up to
/// ~5x per character per frame via move_and_slide.
pub(crate) fn patch_body_colliders_bvh_2d(
    bodies: &r2::RigidBodySet,
    colliders: &mut r2::ColliderSet,
    broad_phase: &mut r2::DefaultBroadPhase,
    params: &r2::IntegrationParameters,
    handle: r2::RigidBodyHandle,
) {
    let Some(rb) = bodies.get(handle) else {
        return;
    };
    let pose = *rb.position();
    for &collider_handle in rb.colliders() {
        let Some(collider) = colliders.get_mut(collider_handle) else {
            continue;
        };
        let rel = collider
            .position_wrt_parent()
            .copied()
            .unwrap_or_else(r2::Pose::identity);
        collider.set_position(pose * rel);
        let aabb = collider.compute_aabb();
        broad_phase.set_aabb(params, collider_handle, aabb);
    }
}

/// Flush removed-collider leaves out of the broad-phase BVH so pre-step
/// queries never resolve stale (or slot-reused) leaves. No-op when empty.
pub(crate) fn flush_removed_colliders_bvh_2d(
    broad_phase: &mut r2::DefaultBroadPhase,
    colliders: &r2::ColliderSet,
    bodies: &r2::RigidBodySet,
    params: &r2::IntegrationParameters,
    removed: &mut Vec<r2::ColliderHandle>,
) {
    if removed.is_empty() {
        return;
    }
    let mut events = Vec::new();
    broad_phase.update(params, colliders, bodies, &[], removed, &mut events);
    removed.clear();
}

/// 3d twin of [`flush_removed_colliders_bvh_2d`].
pub(crate) fn flush_removed_colliders_bvh_3d(
    broad_phase: &mut r3::DefaultBroadPhase,
    colliders: &r3::ColliderSet,
    bodies: &r3::RigidBodySet,
    params: &r3::IntegrationParameters,
    removed: &mut Vec<r3::ColliderHandle>,
) {
    if removed.is_empty() {
        return;
    }
    let mut events = Vec::new();
    broad_phase.update(params, colliders, bodies, &[], removed, &mut events);
    removed.clear();
}

/// 3d twin of [`patch_body_colliders_bvh_2d`].
pub(crate) fn patch_body_colliders_bvh_3d(
    bodies: &r3::RigidBodySet,
    colliders: &mut r3::ColliderSet,
    broad_phase: &mut r3::DefaultBroadPhase,
    params: &r3::IntegrationParameters,
    handle: r3::RigidBodyHandle,
) {
    let Some(rb) = bodies.get(handle) else {
        return;
    };
    let pose = *rb.position();
    for &collider_handle in rb.colliders() {
        let Some(collider) = colliders.get_mut(collider_handle) else {
            continue;
        };
        let rel = collider
            .position_wrt_parent()
            .copied()
            .unwrap_or_else(r3::Pose::identity);
        collider.set_position(pose * rel);
        let aabb = collider.compute_aabb();
        broad_phase.set_aabb(params, collider_handle, aabb);
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
        let world = system
            .world_2d
            .as_mut()
            .expect("test or bench setup must succeed");
        let handle = world
            .body_map
            .get(&id)
            .expect("test or bench setup must succeed")
            .handle;
        let rb = world
            .bodies
            .get_mut(handle)
            .expect("test or bench setup must succeed");
        rb.set_position(r2::Pose::translation(pos.x, pos.y), true);
        patch_body_colliders_bvh_2d(
            &world.bodies,
            &mut world.colliders,
            &mut world.broad_phase,
            &world.integration_parameters,
            handle,
        );
    }

    fn body_teleport_3d(system: &mut PhysicsSystem, id: NodeID, pos: Vector3) {
        let world = system
            .world_3d
            .as_mut()
            .expect("test or bench setup must succeed");
        let handle = world
            .body_map
            .get(&id)
            .expect("test or bench setup must succeed")
            .handle;
        let rb = world
            .bodies
            .get_mut(handle)
            .expect("test or bench setup must succeed");
        rb.set_position(r3::Pose::translation(pos.x, pos.y, pos.z), true);
        patch_body_colliders_bvh_3d(
            &world.bodies,
            &mut world.colliders,
            &mut world.broad_phase,
            &world.integration_parameters,
            handle,
        );
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
                .expect("test or bench setup must succeed");
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
            .expect("test or bench setup must succeed");
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
            .expect("test or bench setup must succeed");
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
                .expect("test or bench setup must succeed");
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
                .expect("test or bench setup must succeed");
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
            .expect("test or bench setup must succeed");
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
            .expect("test or bench setup must succeed");
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
                .expect("test or bench setup must succeed");
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
