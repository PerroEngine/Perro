//! Navigation runtime API.

use perro_ids::NavMeshID;
use perro_structs::{BitMask, Vector3};

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct NavMeshPathOptions {
    pub layers: BitMask,
    pub max_snap_distance: f32,
    pub max_points: u32,
    pub simplify: bool,
}

impl Default for NavMeshPathOptions {
    fn default() -> Self {
        Self {
            layers: BitMask::ALL,
            max_snap_distance: 1.0,
            max_points: 256,
            simplify: true,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct NavMeshAreaCost {
    pub area: u8,
    pub multiplier: f32,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum NavMeshObstacle3D {
    Circle { center: Vector3, radius: f32 },
    Aabb { min: Vector3, max: Vector3 },
}

#[derive(Clone, Debug, PartialEq)]
pub struct NavMeshQueryOptions {
    pub path: NavMeshPathOptions,
    pub area_costs: Vec<NavMeshAreaCost>,
    pub obstacles: Vec<NavMeshObstacle3D>,
    pub use_off_mesh_links: bool,
}

impl Default for NavMeshQueryOptions {
    fn default() -> Self {
        Self {
            path: NavMeshPathOptions::default(),
            area_costs: Vec::new(),
            obstacles: Vec::new(),
            use_off_mesh_links: true,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum NavMeshPathStatus {
    Complete,
    Partial,
    Failed,
}

#[derive(Clone, Debug, PartialEq)]
pub struct NavMeshPath3D {
    pub status: NavMeshPathStatus,
    pub points: Vec<Vector3>,
    pub distance: f32,
}

impl NavMeshPath3D {
    pub fn failed() -> Self {
        Self {
            status: NavMeshPathStatus::Failed,
            points: Vec::new(),
            distance: 0.0,
        }
    }
}

pub trait NavMeshAPI {
    fn navmesh_find_path_3d(
        &mut self,
        navmesh: NavMeshID,
        start: Vector3,
        end: Vector3,
        opts: NavMeshPathOptions,
    ) -> NavMeshPath3D;

    fn navmesh_project_point_3d(
        &mut self,
        navmesh: NavMeshID,
        point: Vector3,
        max_distance: f32,
    ) -> Option<Vector3>;

    fn navmesh_find_path_query_3d(
        &mut self,
        navmesh: NavMeshID,
        start: Vector3,
        end: Vector3,
        query: NavMeshQueryOptions,
    ) -> NavMeshPath3D {
        self.navmesh_find_path_3d(navmesh, start, end, query.path)
    }
}

pub struct NavMeshModule<'rt, R: NavMeshAPI + ?Sized> {
    rt: &'rt mut R,
}

impl<'rt, R: NavMeshAPI + ?Sized> NavMeshModule<'rt, R> {
    pub fn new(rt: &'rt mut R) -> Self {
        Self { rt }
    }

    #[inline]
    pub fn find_path_3d(
        &mut self,
        navmesh: NavMeshID,
        start: Vector3,
        end: Vector3,
        opts: NavMeshPathOptions,
    ) -> NavMeshPath3D {
        self.rt.navmesh_find_path_3d(navmesh, start, end, opts)
    }

    #[inline]
    pub fn find_path_query_3d(
        &mut self,
        navmesh: NavMeshID,
        start: Vector3,
        end: Vector3,
        query: NavMeshQueryOptions,
    ) -> NavMeshPath3D {
        self.rt
            .navmesh_find_path_query_3d(navmesh, start, end, query)
    }

    #[inline]
    pub fn project_point_3d(
        &mut self,
        navmesh: NavMeshID,
        point: Vector3,
        max_distance: f32,
    ) -> Option<Vector3> {
        self.rt
            .navmesh_project_point_3d(navmesh, point, max_distance)
    }
}

#[macro_export]
macro_rules! navmesh_find_path_3d {
    ($run:expr, $navmesh:expr, $start:expr, $end:expr) => {
        $run.NavMesh().find_path_3d(
            $navmesh,
            $start,
            $end,
            $crate::sub_apis::NavMeshPathOptions::default(),
        )
    };
    ($run:expr, $navmesh:expr, $start:expr, $end:expr, $opts:expr) => {
        $run.NavMesh().find_path_3d($navmesh, $start, $end, $opts)
    };
}
