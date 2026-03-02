use perro_ids::TerrainID;
use perro_structs::Vector3;
use perro_terrain::{BrushOp, BrushShape, TerrainEditSummary, TerrainRayHit};

pub trait TerrainAPI {
    fn terrain_brush_op(
        &self,
        terrain: TerrainID,
        center_world: Vector3,
        brush_size_meters: f32,
        shape: BrushShape,
        op: BrushOp,
    ) -> Option<TerrainEditSummary>;

    fn terrain_raycast(
        &self,
        terrain: TerrainID,
        origin_world: Vector3,
        direction_world: Vector3,
        max_distance: f32,
    ) -> Option<TerrainRayHit>;
}

pub struct TerrainModule<'res, R: TerrainAPI + ?Sized> {
    api: &'res R,
}

impl<'res, R: TerrainAPI + ?Sized> TerrainModule<'res, R> {
    pub fn new(api: &'res R) -> Self {
        Self { api }
    }

    pub fn brush_op(
        &self,
        terrain: TerrainID,
        center_world: Vector3,
        brush_size_meters: f32,
        shape: BrushShape,
        op: BrushOp,
    ) -> Option<TerrainEditSummary> {
        self.api
            .terrain_brush_op(terrain, center_world, brush_size_meters, shape, op)
    }

    pub fn raycast(
        &self,
        terrain: TerrainID,
        origin_world: Vector3,
        direction_world: Vector3,
        max_distance: f32,
    ) -> Option<TerrainRayHit> {
        self.api
            .terrain_raycast(terrain, origin_world, direction_world, max_distance)
    }
}

