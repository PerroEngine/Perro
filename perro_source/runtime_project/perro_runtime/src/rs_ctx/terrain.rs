use super::core::RuntimeResourceApi;
use perro_ids::TerrainID;
use perro_resource_context::sub_apis::TerrainAPI;
use perro_structs::Vector3;
use perro_terrain::{BrushOp, BrushShape, TerrainEditSummary, TerrainRayHit};

impl TerrainAPI for RuntimeResourceApi {
    fn terrain_brush_op(
        &self,
        terrain: TerrainID,
        center_world: Vector3,
        brush_size_meters: f32,
        shape: BrushShape,
        op: BrushOp,
    ) -> Option<TerrainEditSummary> {
        let mut store = self
            .terrain_store
            .lock()
            .expect("terrain store mutex poisoned");
        let data = store.get_mut(terrain)?;
        data.apply_brush_op_world(center_world, brush_size_meters, shape, op)
            .ok()
    }

    fn terrain_raycast(
        &self,
        terrain: TerrainID,
        origin_world: Vector3,
        direction_world: Vector3,
        max_distance: f32,
    ) -> Option<TerrainRayHit> {
        let store = self
            .terrain_store
            .lock()
            .expect("terrain store mutex poisoned");
        let data = store.get(terrain)?;
        data.raycast_world(origin_world, direction_world, max_distance)
    }
}
