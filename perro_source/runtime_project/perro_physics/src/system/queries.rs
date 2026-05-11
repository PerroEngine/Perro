use super::*;

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
        world.query_pipeline.update(&world.colliders);

        let ray = r2::Ray::new(na2::Point2::new(origin.x, origin.y), dir);
        let excluded = filter.exclude_nodes.as_slice();
        let mask = filter.mask;
        let predicate = |handle, collider: &r2::Collider| {
            (collider.collision_groups().memberships.bits() & mask) != 0
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
        world.query_pipeline.update(&world.colliders);

        let ray = r3::Ray::new(na3::Point3::new(origin.x, origin.y, origin.z), dir);
        let filter = if include_areas {
            r3::QueryFilter::new()
        } else {
            r3::QueryFilter::new().exclude_sensors()
        };
        let (collider, hit) = world.query_pipeline.cast_ray_and_get_normal(
            &world.bodies,
            &world.colliders,
            &ray,
            max_distance,
            true,
            filter,
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
        world.query_pipeline.update(&world.colliders);

        let shape_pos = na2::Isometry2::new(na2::Vector2::new(origin.x, origin.y), 0.0);
        let shape_vel = dir / dir_len * max_distance;
        let excluded = filter.exclude_nodes.as_slice();
        let mask = filter.mask;
        let predicate = |handle, collider: &r2::Collider| {
            (collider.collision_groups().memberships.bits() & mask) != 0
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
        world.query_pipeline.update(&world.colliders);

        let shape_pos = na3::Isometry3::translation(origin.x, origin.y, origin.z);
        let shape_vel = dir / dir_len * max_distance;
        let excluded = filter.exclude_nodes.as_slice();
        let mask = filter.mask;
        let predicate = |handle, collider: &r3::Collider| {
            (collider.collision_groups().memberships.bits() & mask) != 0
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
        if let Some(world) = self.world_2d.as_mut() {
            world.query_pipeline.update(&world.colliders);
        }
    }

    pub fn update_query_pipeline_3d(&mut self) {
        if let Some(world) = self.world_3d.as_mut() {
            world.query_pipeline.update(&world.colliders);
        }
    }
}
