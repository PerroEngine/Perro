use crate::{BrushShape, ChunkConfig, ChunkCoord, ChunkError, InsertVertexResult, TerrainChunk};
use perro_structs::Vector3;
use std::collections::{HashMap, HashSet};

const BORDER_EPSILON: f32 = 1.0e-4;
const POSITION_KEY_SCALE: f32 = 10_000.0;

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct TerrainEditSummary {
    pub touched_chunks: Vec<ChunkCoord>,
    pub inserted_points: usize,
}

#[derive(Clone, Debug)]
pub struct TerrainData {
    chunk_size_meters: f32,
    origin_x: i32,
    origin_z: i32,
    width: usize,
    height: usize,
    cells: Vec<Option<TerrainChunk>>,
}

impl Default for TerrainData {
    fn default() -> Self {
        Self::new(64.0)
    }
}

impl TerrainData {
    pub fn new(chunk_size_meters: f32) -> Self {
        Self {
            chunk_size_meters,
            origin_x: 0,
            origin_z: 0,
            width: 0,
            height: 0,
            cells: Vec::new(),
        }
    }

    pub fn chunk_size_meters(&self) -> f32 {
        self.chunk_size_meters
    }

    pub fn chunk(&self, coord: ChunkCoord) -> Option<&TerrainChunk> {
        let idx = self.index_of(coord)?;
        self.cells.get(idx)?.as_ref()
    }

    pub fn chunk_mut(&mut self, coord: ChunkCoord) -> Option<&mut TerrainChunk> {
        let idx = self.index_of(coord)?;
        self.cells.get_mut(idx)?.as_mut()
    }

    pub fn ensure_chunk(&mut self, coord: ChunkCoord) -> &mut TerrainChunk {
        self.ensure_bounds_contains(coord);
        let idx = self
            .index_of(coord)
            .expect("index should exist after ensure_bounds_contains");
        if self.cells[idx].is_none() {
            self.cells[idx] = Some(TerrainChunk::new_flat(
                coord,
                ChunkConfig::new(self.chunk_size_meters),
            ));
        }
        self.cells[idx]
            .as_mut()
            .expect("chunk slot was just initialized")
    }

    pub fn insert_brush_world(
        &mut self,
        center_world: Vector3,
        brush_size_meters: f32,
        shape: BrushShape,
    ) -> Result<TerrainEditSummary, ChunkError> {
        let touched = self.overlapped_chunks(center_world, brush_size_meters);
        let mut summary = TerrainEditSummary::default();
        let mut touched_set: HashSet<ChunkCoord> = HashSet::new();

        for coord in touched {
            let local_center = self.world_to_chunk_local(center_world, coord);
            let chunk = self.ensure_chunk(coord);
            let results: Vec<InsertVertexResult> =
                chunk.insert_brush(local_center, brush_size_meters, shape)?;
            if !results.is_empty() {
                summary.inserted_points += results.len();
                touched_set.insert(coord);
            }
        }

        let mut touched_chunks: Vec<ChunkCoord> = touched_set.into_iter().collect();
        touched_chunks.sort_by_key(|c| (c.x, c.z));
        self.sync_seams_for_touched(&touched_chunks)?;
        summary.touched_chunks = touched_chunks;
        Ok(summary)
    }

    pub fn insert_vertex_world(
        &mut self,
        position_world: Vector3,
    ) -> Result<TerrainEditSummary, ChunkError> {
        let coord = self.world_to_chunk_coord(position_world.x, position_world.z);
        let local = self.world_to_chunk_local(position_world, coord);
        let chunk = self.ensure_chunk(coord);
        let _ = chunk.insert_vertex(local)?;

        let touched = vec![coord];
        self.sync_seams_for_touched(&touched)?;

        Ok(TerrainEditSummary {
            touched_chunks: touched,
            inserted_points: 1,
        })
    }

    fn sync_seams_for_touched(&mut self, touched: &[ChunkCoord]) -> Result<(), ChunkError> {
        let set: HashSet<ChunkCoord> = touched.iter().copied().collect();
        for coord in touched {
            let east = ChunkCoord::new(coord.x + 1, coord.z);
            if set.contains(&east) {
                self.sync_pair_seam(*coord, east, SharedBorder::Vertical)?;
            }
            let north = ChunkCoord::new(coord.x, coord.z + 1);
            if set.contains(&north) {
                self.sync_pair_seam(*coord, north, SharedBorder::Horizontal)?;
            }
        }
        Ok(())
    }

    fn sync_pair_seam(
        &mut self,
        a_coord: ChunkCoord,
        b_coord: ChunkCoord,
        border: SharedBorder,
    ) -> Result<(), ChunkError> {
        let a_points = self.border_points_world(a_coord, border.side_for_a(), BORDER_EPSILON)?;
        let b_points = self.border_points_world(b_coord, border.side_for_b(), BORDER_EPSILON)?;

        let mut merged: HashMap<(i32, i32), f32> = HashMap::new();
        for p in a_points.iter().chain(b_points.iter()) {
            let key = position_key(p.x, p.z);
            merged
                .entry(key)
                .and_modify(|y| *y = (*y + p.y) * 0.5)
                .or_insert(p.y);
        }
        if merged.is_empty() {
            return Ok(());
        }

        let mut targets: Vec<Vector3> = merged
            .into_iter()
            .map(|((kx, kz), y)| {
                Vector3::new(
                    (kx as f32) / POSITION_KEY_SCALE,
                    y,
                    (kz as f32) / POSITION_KEY_SCALE,
                )
            })
            .collect();
        targets.sort_by(|lhs, rhs| {
            lhs.x
                .partial_cmp(&rhs.x)
                .unwrap_or(std::cmp::Ordering::Equal)
                .then(lhs.z.partial_cmp(&rhs.z).unwrap_or(std::cmp::Ordering::Equal))
        });

        self.ensure_border_points_and_heights(a_coord, border.side_for_a(), &targets)?;
        self.ensure_border_points_and_heights(b_coord, border.side_for_b(), &targets)?;
        Ok(())
    }

    fn ensure_border_points_and_heights(
        &mut self,
        coord: ChunkCoord,
        side: BorderSide,
        targets_world: &[Vector3],
    ) -> Result<(), ChunkError> {
        for target_world in targets_world {
            let local = self.world_to_chunk_local(*target_world, coord);
            if !is_on_border_side(local, self.chunk_size_meters * 0.5, side, BORDER_EPSILON) {
                continue;
            }

            let chunk = self
                .chunk_mut(coord)
                .ok_or(ChunkError::PointOutsideMesh { x: local.x, z: local.z })?;
            let ids = find_vertex_ids_near_xz(chunk, local.x, local.z, BORDER_EPSILON);
            if ids.is_empty() {
                let _ = chunk.insert_vertex(local)?;
            }
            let ids2 = find_vertex_ids_near_xz(chunk, local.x, local.z, BORDER_EPSILON);
            for id in ids2 {
                let old = chunk.vertices()[id].position;
                chunk.set_vertex_position(id, Vector3::new(old.x, local.y, old.z))?;
            }
        }
        Ok(())
    }

    fn border_points_world(
        &self,
        coord: ChunkCoord,
        side: BorderSide,
        eps: f32,
    ) -> Result<Vec<Vector3>, ChunkError> {
        let chunk = self
            .chunk(coord)
            .ok_or(ChunkError::PointOutsideMesh { x: 0.0, z: 0.0 })?;
        let half = self.chunk_size_meters * 0.5;
        let mut out = Vec::new();
        for v in chunk.vertices() {
            if is_on_border_side(v.position, half, side, eps) {
                out.push(self.chunk_local_to_world(v.position, coord));
            }
        }
        Ok(out)
    }

    fn overlapped_chunks(&self, center_world: Vector3, brush_size_meters: f32) -> Vec<ChunkCoord> {
        let half = brush_size_meters * 0.5;
        let min_x = center_world.x - half;
        let max_x = center_world.x + half;
        let min_z = center_world.z - half;
        let max_z = center_world.z + half;

        let min_cx = self.world_to_chunk_coord(min_x, min_z).x;
        let max_cx = self.world_to_chunk_coord(max_x, min_z).x;
        let min_cz = self.world_to_chunk_coord(min_x, min_z).z;
        let max_cz = self.world_to_chunk_coord(min_x, max_z).z;

        let mut out = Vec::new();
        for cx in min_cx..=max_cx {
            for cz in min_cz..=max_cz {
                out.push(ChunkCoord::new(cx, cz));
            }
        }
        out
    }

    fn world_to_chunk_coord(&self, world_x: f32, world_z: f32) -> ChunkCoord {
        let inv = 1.0 / self.chunk_size_meters;
        let cx = (world_x * inv).floor() as i32;
        let cz = (world_z * inv).floor() as i32;
        ChunkCoord::new(cx, cz)
    }

    fn world_to_chunk_local(&self, world: Vector3, coord: ChunkCoord) -> Vector3 {
        let center_x = coord.x as f32 * self.chunk_size_meters + self.chunk_size_meters * 0.5;
        let center_z = coord.z as f32 * self.chunk_size_meters + self.chunk_size_meters * 0.5;
        Vector3::new(world.x - center_x, world.y, world.z - center_z)
    }

    fn chunk_local_to_world(&self, local: Vector3, coord: ChunkCoord) -> Vector3 {
        let center_x = coord.x as f32 * self.chunk_size_meters + self.chunk_size_meters * 0.5;
        let center_z = coord.z as f32 * self.chunk_size_meters + self.chunk_size_meters * 0.5;
        Vector3::new(local.x + center_x, local.y, local.z + center_z)
    }

    fn index_of(&self, coord: ChunkCoord) -> Option<usize> {
        if self.width == 0 || self.height == 0 {
            return None;
        }
        let rx = coord.x - self.origin_x;
        let rz = coord.z - self.origin_z;
        if rx < 0 || rz < 0 {
            return None;
        }
        let x = rx as usize;
        let z = rz as usize;
        if x >= self.width || z >= self.height {
            return None;
        }
        Some(z * self.width + x)
    }

    fn ensure_bounds_contains(&mut self, coord: ChunkCoord) {
        if self.width == 0 || self.height == 0 {
            self.origin_x = coord.x;
            self.origin_z = coord.z;
            self.width = 1;
            self.height = 1;
            self.cells = vec![None];
            return;
        }

        let current_min_x = self.origin_x;
        let current_min_z = self.origin_z;
        let current_max_x = self.origin_x + self.width as i32 - 1;
        let current_max_z = self.origin_z + self.height as i32 - 1;

        if coord.x >= current_min_x
            && coord.x <= current_max_x
            && coord.z >= current_min_z
            && coord.z <= current_max_z
        {
            return;
        }

        let new_min_x = current_min_x.min(coord.x);
        let new_min_z = current_min_z.min(coord.z);
        let new_max_x = current_max_x.max(coord.x);
        let new_max_z = current_max_z.max(coord.z);

        let new_width = (new_max_x - new_min_x + 1) as usize;
        let new_height = (new_max_z - new_min_z + 1) as usize;
        let mut new_cells: Vec<Option<TerrainChunk>> = vec![None; new_width * new_height];

        for old_z in 0..self.height {
            for old_x in 0..self.width {
                let old_idx = old_z * self.width + old_x;
                let old_coord = ChunkCoord::new(
                    self.origin_x + old_x as i32,
                    self.origin_z + old_z as i32,
                );
                let new_x = (old_coord.x - new_min_x) as usize;
                let new_z = (old_coord.z - new_min_z) as usize;
                let new_idx = new_z * new_width + new_x;
                new_cells[new_idx] = self.cells[old_idx].take();
            }
        }

        self.origin_x = new_min_x;
        self.origin_z = new_min_z;
        self.width = new_width;
        self.height = new_height;
        self.cells = new_cells;
    }
}

#[derive(Clone, Copy)]
enum SharedBorder {
    Vertical,
    Horizontal,
}

impl SharedBorder {
    fn side_for_a(self) -> BorderSide {
        match self {
            SharedBorder::Vertical => BorderSide::East,
            SharedBorder::Horizontal => BorderSide::North,
        }
    }

    fn side_for_b(self) -> BorderSide {
        match self {
            SharedBorder::Vertical => BorderSide::West,
            SharedBorder::Horizontal => BorderSide::South,
        }
    }
}

#[derive(Clone, Copy)]
enum BorderSide {
    East,
    West,
    North,
    South,
}

fn is_on_border_side(pos: Vector3, half: f32, side: BorderSide, eps: f32) -> bool {
    match side {
        BorderSide::East => (pos.x - half).abs() <= eps,
        BorderSide::West => (pos.x + half).abs() <= eps,
        BorderSide::North => (pos.z - half).abs() <= eps,
        BorderSide::South => (pos.z + half).abs() <= eps,
    }
}

fn find_vertex_ids_near_xz(chunk: &TerrainChunk, x: f32, z: f32, eps: f32) -> Vec<usize> {
    let mut ids = Vec::new();
    for (i, v) in chunk.vertices().iter().enumerate() {
        if (v.position.x - x).abs() <= eps && (v.position.z - z).abs() <= eps {
            ids.push(i);
        }
    }
    ids
}

fn position_key(x: f32, z: f32) -> (i32, i32) {
    (
        (x * POSITION_KEY_SCALE).round() as i32,
        (z * POSITION_KEY_SCALE).round() as i32,
    )
}
