/// Gets global transform for a 2D spatial node.
/// Usage: `get_global_transform_2d!(ctx, node_id) -> Option<Transform2D>`.
#[macro_export]
macro_rules! get_global_transform_2d {
    ($ctx:expr, $id:expr) => {
        $ctx.Nodes().get_global_transform_2d($id)
    };
}

/// Gets global transform for a 3D spatial node.
/// Usage: `get_global_transform_3d!(ctx, node_id) -> Option<Transform3D>`.
#[macro_export]
macro_rules! get_global_transform_3d {
    ($ctx:expr, $id:expr) => {
        $ctx.Nodes().get_global_transform_3d($id)
    };
}

/// Gets local transform for a 2D spatial node.
/// Usage: `get_local_transform_2d!(ctx, node_id) -> Option<Transform2D>`.
#[macro_export]
macro_rules! get_local_transform_2d {
    ($ctx:expr, $id:expr) => {
        $ctx.Nodes().get_local_transform_2d($id)
    };
}

/// Gets local transform for a 3D spatial node.
/// Usage: `get_local_transform_3d!(ctx, node_id) -> Option<Transform3D>`.
#[macro_export]
macro_rules! get_local_transform_3d {
    ($ctx:expr, $id:expr) => {
        $ctx.Nodes().get_local_transform_3d($id)
    };
}

/// Sets global transform for a 2D spatial node.
/// Usage: `set_global_transform_2d!(ctx, node_id, transform) -> bool`.
#[macro_export]
macro_rules! set_global_transform_2d {
    ($ctx:expr, $id:expr, $transform:expr) => {
        $ctx.Nodes().set_global_transform_2d($id, $transform)
    };
}

/// Sets global transform for a 3D spatial node.
/// Usage: `set_global_transform_3d!(ctx, node_id, transform) -> bool`.
#[macro_export]
macro_rules! set_global_transform_3d {
    ($ctx:expr, $id:expr, $transform:expr) => {
        $ctx.Nodes().set_global_transform_3d($id, $transform)
    };
}

/// Sets local transform for a 2D spatial node.
/// Usage: `set_local_transform_2d!(ctx, node_id, transform) -> bool`.
#[macro_export]
macro_rules! set_local_transform_2d {
    ($ctx:expr, $id:expr, $transform:expr) => {
        $ctx.Nodes().set_local_transform_2d($id, $transform)
    };
}

/// Sets local transform for a 3D spatial node.
/// Usage: `set_local_transform_3d!(ctx, node_id, transform) -> bool`.
#[macro_export]
macro_rules! set_local_transform_3d {
    ($ctx:expr, $id:expr, $transform:expr) => {
        $ctx.Nodes().set_local_transform_3d($id, $transform)
    };
}

/// Gets local position for a 2D spatial node.
/// Usage: `get_local_pos_2d!(ctx, node_id) -> Option<Vector2>`.
#[macro_export]
macro_rules! get_local_pos_2d {
    ($ctx:expr, $id:expr) => {
        $ctx.Nodes().get_local_pos_2d($id)
    };
}

/// Gets local position for a 3D spatial node.
/// Usage: `get_local_pos_3d!(ctx, node_id) -> Option<Vector3>`.
#[macro_export]
macro_rules! get_local_pos_3d {
    ($ctx:expr, $id:expr) => {
        $ctx.Nodes().get_local_pos_3d($id)
    };
}

/// Sets local position for a 2D spatial node.
/// Usage: `set_local_pos_2d!(ctx, node_id, pos) -> bool`.
#[macro_export]
macro_rules! set_local_pos_2d {
    ($ctx:expr, $id:expr, $pos:expr) => {
        $ctx.Nodes().set_local_pos_2d($id, $pos)
    };
}

/// Sets local position for a 3D spatial node.
/// Usage: `set_local_pos_3d!(ctx, node_id, pos) -> bool`.
#[macro_export]
macro_rules! set_local_pos_3d {
    ($ctx:expr, $id:expr, $pos:expr) => {
        $ctx.Nodes().set_local_pos_3d($id, $pos)
    };
}

/// Gets global position for a 2D spatial node.
/// Usage: `get_global_pos_2d!(ctx, node_id) -> Option<Vector2>`.
#[macro_export]
macro_rules! get_global_pos_2d {
    ($ctx:expr, $id:expr) => {
        $ctx.Nodes().get_global_pos_2d($id)
    };
}

/// Gets global position for a 3D spatial node.
/// Usage: `get_global_pos_3d!(ctx, node_id) -> Option<Vector3>`.
#[macro_export]
macro_rules! get_global_pos_3d {
    ($ctx:expr, $id:expr) => {
        $ctx.Nodes().get_global_pos_3d($id)
    };
}

/// Sets global position for a 2D spatial node.
/// Usage: `set_global_pos_2d!(ctx, node_id, pos) -> bool`.
#[macro_export]
macro_rules! set_global_pos_2d {
    ($ctx:expr, $id:expr, $pos:expr) => {
        $ctx.Nodes().set_global_pos_2d($id, $pos)
    };
}

/// Sets global position for a 3D spatial node.
/// Usage: `set_global_pos_3d!(ctx, node_id, pos) -> bool`.
#[macro_export]
macro_rules! set_global_pos_3d {
    ($ctx:expr, $id:expr, $pos:expr) => {
        $ctx.Nodes().set_global_pos_3d($id, $pos)
    };
}

/// Gets local rotation for a 2D spatial node.
/// Usage: `get_local_rot_2d!(ctx, node_id) -> Option<f32>`.
#[macro_export]
macro_rules! get_local_rot_2d {
    ($ctx:expr, $id:expr) => {
        $ctx.Nodes().get_local_rot_2d($id)
    };
}

/// Gets local rotation for a 3D spatial node.
/// Usage: `get_local_rot_3d!(ctx, node_id) -> Option<Quaternion>`.
#[macro_export]
macro_rules! get_local_rot_3d {
    ($ctx:expr, $id:expr) => {
        $ctx.Nodes().get_local_rot_3d($id)
    };
}

/// Sets local rotation for a 2D spatial node.
/// Usage: `set_local_rot_2d!(ctx, node_id, rot) -> bool`.
#[macro_export]
macro_rules! set_local_rot_2d {
    ($ctx:expr, $id:expr, $rot:expr) => {
        $ctx.Nodes().set_local_rot_2d($id, $rot)
    };
}

/// Sets local rotation for a 3D spatial node.
/// Usage: `set_local_rot_3d!(ctx, node_id, rot) -> bool`.
#[macro_export]
macro_rules! set_local_rot_3d {
    ($ctx:expr, $id:expr, $rot:expr) => {
        $ctx.Nodes().set_local_rot_3d($id, $rot)
    };
}

/// Gets global rotation for a 2D spatial node.
/// Usage: `get_global_rot_2d!(ctx, node_id) -> Option<f32>`.
#[macro_export]
macro_rules! get_global_rot_2d {
    ($ctx:expr, $id:expr) => {
        $ctx.Nodes().get_global_rot_2d($id)
    };
}

/// Gets global rotation for a 3D spatial node.
/// Usage: `get_global_rot_3d!(ctx, node_id) -> Option<Quaternion>`.
#[macro_export]
macro_rules! get_global_rot_3d {
    ($ctx:expr, $id:expr) => {
        $ctx.Nodes().get_global_rot_3d($id)
    };
}

/// Sets global rotation for a 2D spatial node.
/// Usage: `set_global_rot_2d!(ctx, node_id, rot) -> bool`.
#[macro_export]
macro_rules! set_global_rot_2d {
    ($ctx:expr, $id:expr, $rot:expr) => {
        $ctx.Nodes().set_global_rot_2d($id, $rot)
    };
}

/// Sets global rotation for a 3D spatial node.
/// Usage: `set_global_rot_3d!(ctx, node_id, rot) -> bool`.
#[macro_export]
macro_rules! set_global_rot_3d {
    ($ctx:expr, $id:expr, $rot:expr) => {
        $ctx.Nodes().set_global_rot_3d($id, $rot)
    };
}

/// Gets local scale for a 2D spatial node.
/// Usage: `get_local_scale_2d!(ctx, node_id) -> Option<Vector2>`.
#[macro_export]
macro_rules! get_local_scale_2d {
    ($ctx:expr, $id:expr) => {
        $ctx.Nodes().get_local_scale_2d($id)
    };
}

/// Gets local scale for a 3D spatial node.
/// Usage: `get_local_scale_3d!(ctx, node_id) -> Option<Vector3>`.
#[macro_export]
macro_rules! get_local_scale_3d {
    ($ctx:expr, $id:expr) => {
        $ctx.Nodes().get_local_scale_3d($id)
    };
}

/// Sets local scale for a 2D spatial node.
/// Usage: `set_local_scale_2d!(ctx, node_id, scale) -> bool`.
#[macro_export]
macro_rules! set_local_scale_2d {
    ($ctx:expr, $id:expr, $scale:expr) => {
        $ctx.Nodes().set_local_scale_2d($id, $scale)
    };
}

/// Sets local scale for a 3D spatial node.
/// Usage: `set_local_scale_3d!(ctx, node_id, scale) -> bool`.
#[macro_export]
macro_rules! set_local_scale_3d {
    ($ctx:expr, $id:expr, $scale:expr) => {
        $ctx.Nodes().set_local_scale_3d($id, $scale)
    };
}

/// Gets global scale for a 2D spatial node.
/// Usage: `get_global_scale_2d!(ctx, node_id) -> Option<Vector2>`.
#[macro_export]
macro_rules! get_global_scale_2d {
    ($ctx:expr, $id:expr) => {
        $ctx.Nodes().get_global_scale_2d($id)
    };
}

/// Gets global scale for a 3D spatial node.
/// Usage: `get_global_scale_3d!(ctx, node_id) -> Option<Vector3>`.
#[macro_export]
macro_rules! get_global_scale_3d {
    ($ctx:expr, $id:expr) => {
        $ctx.Nodes().get_global_scale_3d($id)
    };
}

/// Sets global scale for a 2D spatial node.
/// Usage: `set_global_scale_2d!(ctx, node_id, scale) -> bool`.
#[macro_export]
macro_rules! set_global_scale_2d {
    ($ctx:expr, $id:expr, $scale:expr) => {
        $ctx.Nodes().set_global_scale_2d($id, $scale)
    };
}

/// Sets global scale for a 3D spatial node.
/// Usage: `set_global_scale_3d!(ctx, node_id, scale) -> bool`.
#[macro_export]
macro_rules! set_global_scale_3d {
    ($ctx:expr, $id:expr, $scale:expr) => {
        $ctx.Nodes().set_global_scale_3d($id, $scale)
    };
}

/// Converts local 2D point to global point.
/// Usage: `to_global_point_2d!(ctx, node_id, local_point) -> Option<Vector2>`.
#[macro_export]
macro_rules! to_global_point_2d {
    ($ctx:expr, $id:expr, $point:expr) => {
        $ctx.Nodes().to_global_point_2d($id, $point)
    };
}

/// Converts global 2D point to local point.
/// Usage: `to_local_point_2d!(ctx, node_id, global_point) -> Option<Vector2>`.
#[macro_export]
macro_rules! to_local_point_2d {
    ($ctx:expr, $id:expr, $point:expr) => {
        $ctx.Nodes().to_local_point_2d($id, $point)
    };
}

/// Converts local 3D point to global point.
/// Usage: `to_global_point_3d!(ctx, node_id, local_point) -> Option<Vector3>`.
#[macro_export]
macro_rules! to_global_point_3d {
    ($ctx:expr, $id:expr, $point:expr) => {
        $ctx.Nodes().to_global_point_3d($id, $point)
    };
}

/// Converts global 3D point to local point.
/// Usage: `to_local_point_3d!(ctx, node_id, global_point) -> Option<Vector3>`.
#[macro_export]
macro_rules! to_local_point_3d {
    ($ctx:expr, $id:expr, $point:expr) => {
        $ctx.Nodes().to_local_point_3d($id, $point)
    };
}

/// Converts local 2D transform to global transform.
/// Usage: `to_global_transform_2d!(ctx, node_id, local_transform) -> Option<Transform2D>`.
#[macro_export]
macro_rules! to_global_transform_2d {
    ($ctx:expr, $id:expr, $transform:expr) => {
        $ctx.Nodes().to_global_transform_2d($id, $transform)
    };
}

/// Converts global 2D transform to local transform.
/// Usage: `to_local_transform_2d!(ctx, node_id, global_transform) -> Option<Transform2D>`.
#[macro_export]
macro_rules! to_local_transform_2d {
    ($ctx:expr, $id:expr, $transform:expr) => {
        $ctx.Nodes().to_local_transform_2d($id, $transform)
    };
}

/// Converts local 3D transform to global transform.
/// Usage: `to_global_transform_3d!(ctx, node_id, local_transform) -> Option<Transform3D>`.
#[macro_export]
macro_rules! to_global_transform_3d {
    ($ctx:expr, $id:expr, $transform:expr) => {
        $ctx.Nodes().to_global_transform_3d($id, $transform)
    };
}

/// Converts global 3D transform to local transform.
/// Usage: `to_local_transform_3d!(ctx, node_id, global_transform) -> Option<Transform3D>`.
#[macro_export]
macro_rules! to_local_transform_3d {
    ($ctx:expr, $id:expr, $transform:expr) => {
        $ctx.Nodes().to_local_transform_3d($id, $transform)
    };
}

/// Finds nearest mesh surface at a global-space point for a mesh instance node.
/// Usage: `mesh_instance_surface_at_global_point_3d!(ctx, node_id, global_point) -> Option<MeshSurfaceHit3D>`.
#[macro_export]
macro_rules! mesh_instance_surface_at_global_point_3d {
    ($ctx:expr, $id:expr, $point:expr) => {
        $ctx.MeshQuery()
            .instance_surface_at_global_point($id, $point)
    };
}

/// Resolves a mesh query triangle + barycentric coordinate to global space.
/// Usage:
/// `mesh_instance_surface_global_point_3d!(ctx, node_id, triangle_index, barycentric) -> Option<Vector3>`.
#[macro_export]
macro_rules! mesh_instance_surface_global_point_3d {
    ($ctx:expr, $id:expr, $triangle:expr, $barycentric:expr) => {
        $ctx.MeshQuery()
            .instance_surface_global_point($id, $triangle, $barycentric)
    };
}

/// Finds first mesh surface hit along a global-space ray for a mesh instance node.
/// Usage:
/// `mesh_instance_surface_on_global_ray_3d!(ctx, node_id, ray_origin, ray_direction, max_distance) -> Option<MeshSurfaceHit3D>`.
#[macro_export]
macro_rules! mesh_instance_surface_on_global_ray_3d {
    ($ctx:expr, $id:expr, $origin:expr, $direction:expr, $max_distance:expr) => {
        $ctx.MeshQuery()
            .instance_surface_on_global_ray($id, $origin, $direction, $max_distance)
    };
}

/// Finds mesh surface hits for many global-space rays against one mesh instance node.
/// Usage:
/// `mesh_instance_surfaces_on_global_rays_3d!(ctx, node_id, rays, resolve_material) -> Vec<Option<MeshSurfaceHit3D>>`.
#[macro_export]
macro_rules! mesh_instance_surfaces_on_global_rays_3d {
    ($ctx:expr, $id:expr, $rays:expr, $resolve_material:expr) => {
        $ctx.MeshQuery()
            .instance_surfaces_on_global_rays($id, $rays, $resolve_material)
    };
}

/// Returns mesh instance regions that use the target material.
/// Usage: `mesh_instance_material_regions_3d!(ctx, node_id, material_id) -> Vec<MeshMaterialRegion3D>`.
#[macro_export]
macro_rules! mesh_instance_material_regions_3d {
    ($ctx:expr, $id:expr, $material:expr) => {
        $ctx.MeshQuery().instance_material_regions($id, $material)
    };
}

/// Finds nearest raw mesh-data surface at a mesh-local point.
#[macro_export]
macro_rules! mesh_data_surface_at_local_point_3d {
    ($ctx:expr, $mesh_id:expr, $point_local:expr) => {
        $ctx.MeshQuery()
            .data_surface_at_local_point($mesh_id, $point_local)
    };
}

/// Finds raw mesh-data surface hit on a mesh-local ray.
#[macro_export]
macro_rules! mesh_data_surface_on_local_ray_3d {
    ($ctx:expr, $mesh_id:expr, $origin_local:expr, $direction_local:expr, $max_distance:expr) => {
        $ctx.MeshQuery().data_surface_on_local_ray(
            $mesh_id,
            $origin_local,
            $direction_local,
            $max_distance,
        )
    };
}

/// Returns raw mesh-data regions for one surface index.
#[macro_export]
macro_rules! mesh_data_surface_regions_3d {
    ($ctx:expr, $mesh_id:expr, $surface_index:expr) => {
        $ctx.MeshQuery()
            .data_surface_regions($mesh_id, $surface_index)
    };
}
