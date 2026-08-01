use super::*;

pub fn prepared_audio_raycast_2d_in_world(
    world: &PhysicsWorld2D,
    origin: Vector2,
    direction: Vector2,
    max_distance: f32,
    layers: BitMask,
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
    let ray = r2::Ray::new(r2::Vector::new(origin.x, origin.y), dir);
    let predicate = |handle, collider: &r2::Collider| {
        (collider.collision_groups().memberships.bits() & layers.bits()) != 0
            && world.collider_owners.contains_key(&handle)
    };
    let query_filter = r2::QueryFilter::new()
        .exclude_sensors()
        .predicate(&predicate);
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

pub fn prepared_audio_raycast_3d_in_world(
    world: &PhysicsWorld3D,
    origin: Vector3,
    direction: Vector3,
    max_distance: f32,
    include_areas: bool,
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
    let ray = r3::Ray::new(r3::Vector::new(origin.x, origin.y, origin.z), dir);
    let filter = if include_areas {
        r3::QueryFilter::new()
    } else {
        r3::QueryFilter::new().exclude_sensors()
    };
    let query_pipeline = query_pipeline_3d(world, filter);
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
