use crate::{ChunkError, TerrainChunk, VertexID};
use perro_structs::Vector3;

pub const DEFAULT_AREA_EPSILON: f32 = 1.0e-6;
pub const DEFAULT_NORMAL_EPSILON: f32 = 1.0e-4;
pub const DEFAULT_DISTANCE_EPSILON: f32 = 1.0e-4;

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct InsertVertexResult {
    pub inserted_vertex_id: VertexID,
    pub removed_as_coplanar: bool,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct BatchInsertSummary {
    pub attempted: usize,
    pub inserted: usize,
    pub removed_as_coplanar: usize,
    pub skipped_outside_mesh: usize,
}

impl TerrainChunk {
    pub fn insert_vertex(&mut self, position: Vector3) -> Result<InsertVertexResult, ChunkError> {
        self.insert_vertex_with_tolerances(
            position,
            DEFAULT_AREA_EPSILON,
            DEFAULT_NORMAL_EPSILON,
            DEFAULT_DISTANCE_EPSILON,
        )
    }

    pub fn insert_vertex_with_tolerances(
        &mut self,
        position: Vector3,
        _area_epsilon: f32,
        _normal_epsilon: f32,
        distance_epsilon: f32,
    ) -> Result<InsertVertexResult, ChunkError> {
        let Some((gx, gz, snapped_x, snapped_z)) = self.snap_local_xz_to_grid(position.x, position.z) else {
            return Err(ChunkError::PointOutsideMesh {
                x: position.x,
                z: position.z,
            });
        };

        let id = TerrainChunk::grid_index(gx, gz);
        let current = self.vertices[id].position;
        let same_height = (current.y - position.y).abs() <= distance_epsilon;
        self.vertices[id].position = Vector3::new(snapped_x, position.y, snapped_z);

        Ok(InsertVertexResult {
            inserted_vertex_id: id,
            removed_as_coplanar: same_height,
        })
    }

    pub fn insert_vertices_batch(
        &mut self,
        points: &[Vector3],
    ) -> Result<BatchInsertSummary, ChunkError> {
        let mut summary = BatchInsertSummary {
            attempted: points.len(),
            ..BatchInsertSummary::default()
        };
        for point in points {
            match self.insert_vertex(*point) {
                Ok(result) => {
                    summary.inserted += 1;
                    if result.removed_as_coplanar {
                        summary.removed_as_coplanar += 1;
                    }
                }
                Err(ChunkError::PointOutsideMesh { .. }) => {
                    summary.skipped_outside_mesh += 1;
                }
                Err(err) => return Err(err),
            }
        }
        Ok(summary)
    }

}
