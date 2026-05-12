use super::*;

impl PhysicsSystem {
    pub fn prepared_audio_raycast_2d(
        &self,
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

        let world = self.world_2d.as_ref()?;
        let ray = r2::Ray::new(na2::Point2::new(origin.x, origin.y), dir);
        let excluded = filter.exclude_nodes.as_slice();
        let mask = filter.mask.bits();
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

    pub fn prepared_audio_raycast_3d(
        &self,
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

        let world = self.world_3d.as_ref()?;
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

    pub fn cast_prepared_audio_rays(
        &self,
        inputs: &[AudioRaycastInput],
        outputs: &mut [AudioRaycastResult],
        parallel: bool,
    ) {
        let world_2d = self.world_2d.as_ref();
        let world_3d = self.world_3d.as_ref();
        let cast = |input: &AudioRaycastInput| match *input {
            AudioRaycastInput::TwoD {
                origin,
                direction,
                max_distance,
                mask,
            } => AudioRaycastResult::TwoD(world_2d.and_then(|world| {
                prepared_audio_raycast_2d_in_world(world, origin, direction, max_distance, mask)
            })),
            AudioRaycastInput::ThreeD {
                origin,
                direction,
                max_distance,
                include_areas,
            } => AudioRaycastResult::ThreeD(world_3d.and_then(|world| {
                prepared_audio_raycast_3d_in_world(
                    world,
                    origin,
                    direction,
                    max_distance,
                    include_areas,
                )
            })),
        };

        if parallel {
            outputs
                .par_iter_mut()
                .zip(inputs.par_iter())
                .for_each(|(out, input)| *out = cast(input));
        } else {
            for (out, input) in outputs.iter_mut().zip(inputs.iter()) {
                *out = cast(input);
            }
        }
    }
}
