use crate::Runtime;
use perro_runtime_api::sub_apis::{NavMeshAPI, NavMeshPath3D, NavMeshPathOptions};
use perro_structs::{BitMask, Vector3};

impl NavMeshAPI for Runtime {
    fn navmesh_find_path_3d(
        &mut self,
        navmesh: perro_ids::NavMeshID,
        start: Vector3,
        end: Vector3,
        opts: NavMeshPathOptions,
    ) -> NavMeshPath3D {
        let Some((data, graph)) = self.resource_api.navmesh_query_data(navmesh, opts.layers) else {
            return NavMeshPath3D::failed();
        };
        crate::runtime::navmesh::find_path_3d_prepared(&data, &graph, start, end, opts)
    }

    fn navmesh_project_point_3d(
        &mut self,
        navmesh: perro_ids::NavMeshID,
        point: Vector3,
        max_distance: f32,
    ) -> Option<Vector3> {
        let (data, _) = self
            .resource_api
            .navmesh_query_data(navmesh, BitMask::ALL)?;
        crate::runtime::navmesh::project_point_3d(&data, point, max_distance, BitMask::ALL)
    }
}
