use super::*;

pub struct MeshQueryModule<'rt, R: NodeAPI + ?Sized> {
    rt: &'rt mut R,
}

impl<'rt, R: NodeAPI + ?Sized> MeshQueryModule<'rt, R> {
    pub fn new(rt: &'rt mut R) -> Self {
        Self { rt }
    }

    pub fn instance_surface_at_global_point(
        &mut self,
        node_id: NodeID,
        global_point: Vector3,
    ) -> Option<MeshSurfaceHit3D> {
        self.rt
            .mesh_instance_surface_at_global_point(node_id, global_point)
    }

    pub fn instance_surface_global_point(
        &mut self,
        node_id: NodeID,
        triangle_index: u32,
        barycentric: Vector3,
    ) -> Option<Vector3> {
        self.rt
            .mesh_instance_surface_global_point(node_id, triangle_index, barycentric)
    }

    pub fn instance_surface_on_global_ray(
        &mut self,
        node_id: NodeID,
        ray_origin: Vector3,
        ray_direction: Vector3,
        max_distance: f32,
    ) -> Option<MeshSurfaceHit3D> {
        self.rt.mesh_instance_surface_on_global_ray(
            node_id,
            ray_origin,
            ray_direction,
            max_distance,
        )
    }

    pub fn instance_surfaces_on_global_rays(
        &mut self,
        node_id: NodeID,
        rays: &[MeshSurfaceRay3D],
        resolve_material: bool,
    ) -> Vec<Option<MeshSurfaceHit3D>> {
        self.rt
            .mesh_instance_surfaces_on_global_rays(node_id, rays, resolve_material)
    }

    pub fn instance_material_regions(
        &mut self,
        node_id: NodeID,
        material: MaterialID,
    ) -> Vec<MeshMaterialRegion3D> {
        self.rt.mesh_instance_material_regions(node_id, material)
    }

    pub fn data_surface_at_local_point(
        &mut self,
        mesh_id: MeshID,
        local_point: Vector3,
    ) -> Option<MeshDataSurfaceHit3D> {
        self.rt
            .mesh_data_surface_at_local_point(mesh_id, local_point)
    }

    pub fn data_surface_on_local_ray(
        &mut self,
        mesh_id: MeshID,
        ray_origin_local: Vector3,
        ray_direction_local: Vector3,
        max_distance: f32,
    ) -> Option<MeshDataSurfaceHit3D> {
        self.rt.mesh_data_surface_on_local_ray(
            mesh_id,
            ray_origin_local,
            ray_direction_local,
            max_distance,
        )
    }

    pub fn data_surface_regions(
        &mut self,
        mesh_id: MeshID,
        surface_index: u32,
    ) -> Vec<MeshDataSurfaceRegion3D> {
        self.rt.mesh_data_surface_regions(mesh_id, surface_index)
    }
}

mod access_macros;
mod node_macros;
mod query_macros;
mod transform_macros;
pub use query_macros::CameraRay3D;
