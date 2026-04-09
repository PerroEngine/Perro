use crate::{
    BrushOp, BrushShape, ChunkConfig, ChunkCoord, ChunkError, InsertVertexResult, TerrainChunk,
    DEFAULT_CHUNK_SIZE_METERS,
};
use perro_structs::Vector3;
use std::collections::HashSet;

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct TerrainRayHit {
    pub chunk: ChunkCoord,
    pub position_world: Vector3,
    pub normal_world: Vector3,
    pub distance: f32,
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct TerrainEditSummary {
    pub touched_chunks: Vec<ChunkCoord>,
    /// Number of touched vertices in fixed-grid mode.
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
        Self::new(DEFAULT_CHUNK_SIZE_METERS)
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

    pub fn chunks(&self) -> impl Iterator<Item = (ChunkCoord, &TerrainChunk)> + '_ {
        self.cells.iter().enumerate().filter_map(|(idx, cell)| {
            let chunk = cell.as_ref()?;
            let x = idx % self.width.max(1);
            let z = idx / self.width.max(1);
            let coord = ChunkCoord::new(self.origin_x + x as i32, self.origin_z + z as i32);
            Some((coord, chunk))
        })
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

    pub fn set_chunk(&mut self, coord: ChunkCoord, mut chunk: TerrainChunk) {
        self.ensure_bounds_contains(coord);
        let idx = self
            .index_of(coord)
            .expect("index should exist after ensure_bounds_contains");
        chunk.coord = coord;
        chunk.config = ChunkConfig::new(self.chunk_size_meters);
        self.cells[idx] = Some(chunk);
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

    pub fn apply_brush_op_world(
        &mut self,
        center_world: Vector3,
        brush_size_meters: f32,
        shape: BrushShape,
        op: BrushOp,
    ) -> Result<TerrainEditSummary, ChunkError> {
        let touched = self.overlapped_chunks(center_world, brush_size_meters);
        let mut summary = TerrainEditSummary::default();
        let mut touched_set: HashSet<ChunkCoord> = HashSet::new();

        for coord in touched {
            let local_center = self.world_to_chunk_local(center_world, coord);
            let chunk = self.ensure_chunk(coord);
            let results: Vec<InsertVertexResult> =
                chunk.apply_brush_op(local_center, brush_size_meters, shape, op)?;
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

    pub fn raycast_world(
        &self,
        origin_world: Vector3,
        direction_world: Vector3,
        max_distance: f32,
    ) -> Option<TerrainRayHit> {
        if !max_distance.is_finite() || max_distance <= 0.0 {
            return None;
        }
        let direction = direction_world.normalized();
        if direction.length_squared() <= 1.0e-8 {
            return None;
        }

        let mut best: Option<TerrainRayHit> = None;
        for (coord, chunk) in self.chunks() {
            for tri in chunk.triangles() {
                let a = self.chunk_local_to_world(chunk.vertices()[tri.a].position, coord);
                let b = self.chunk_local_to_world(chunk.vertices()[tri.b].position, coord);
                let c = self.chunk_local_to_world(chunk.vertices()[tri.c].position, coord);

                if let Some((distance, position_world, normal_world)) =
                    ray_triangle_hit(origin_world, direction, max_distance, a, b, c)
                {
                    match best {
                        Some(current) if current.distance <= distance => {}
                        _ => {
                            best = Some(TerrainRayHit {
                                chunk: coord,
                                position_world,
                                normal_world,
                                distance,
                            });
                        }
                    }
                }
            }
        }
        best
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
        let a_chunk = self
            .chunk(a_coord)
            .ok_or(ChunkError::PointOutsideMesh { x: 0.0, z: 0.0 })?
            .clone();
        let b_chunk = self
            .chunk(b_coord)
            .ok_or(ChunkError::PointOutsideMesh { x: 0.0, z: 0.0 })?
            .clone();

        let (a_cells, b_cells) =
            match (a_chunk.grid_cells_per_side(), b_chunk.grid_cells_per_side()) {
                (Some(a_cells), Some(b_cells)) if a_cells == b_cells => (a_cells, b_cells),
                _ => return Ok(()),
            };

        let mut updates: Vec<(usize, f32)> = Vec::with_capacity(a_cells + 1);
        let mut updates_b: Vec<(usize, f32)> = Vec::with_capacity(b_cells + 1);

        for i in 0..=a_cells {
            let (a_id, b_id) = match border {
                SharedBorder::Vertical => {
                    (a_chunk.grid_index(a_cells, i), b_chunk.grid_index(0, i))
                }
                SharedBorder::Horizontal => {
                    (a_chunk.grid_index(i, a_cells), b_chunk.grid_index(i, 0))
                }
            };
            let (Some(a_id), Some(b_id)) = (a_id, b_id) else {
                continue;
            };
            let ay = a_chunk.vertices()[a_id].position.y;
            let by = b_chunk.vertices()[b_id].position.y;
            let merged = (ay + by) * 0.5;
            updates.push((a_id, merged));
            updates_b.push((b_id, merged));
        }

        if let Some(chunk) = self.chunk_mut(a_coord) {
            for (id, y) in updates {
                let old = chunk.vertices()[id].position;
                chunk.set_vertex_position(id, Vector3::new(old.x, y, old.z))?;
            }
        }
        if let Some(chunk) = self.chunk_mut(b_coord) {
            for (id, y) in updates_b {
                let old = chunk.vertices()[id].position;
                chunk.set_vertex_position(id, Vector3::new(old.x, y, old.z))?;
            }
        }
        Ok(())
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
        let cx = (world_x * inv + 0.5).floor() as i32;
        let cz = (world_z * inv + 0.5).floor() as i32;
        ChunkCoord::new(cx, cz)
    }

    fn world_to_chunk_local(&self, world: Vector3, coord: ChunkCoord) -> Vector3 {
        let center_x = coord.x as f32 * self.chunk_size_meters;
        let center_z = coord.z as f32 * self.chunk_size_meters;
        Vector3::new(world.x - center_x, world.y, world.z - center_z)
    }

    fn chunk_local_to_world(&self, local: Vector3, coord: ChunkCoord) -> Vector3 {
        let center_x = coord.x as f32 * self.chunk_size_meters;
        let center_z = coord.z as f32 * self.chunk_size_meters;
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
                let old_coord =
                    ChunkCoord::new(self.origin_x + old_x as i32, self.origin_z + old_z as i32);
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

fn ray_triangle_hit(
    origin: Vector3,
    direction: Vector3,
    max_distance: f32,
    a: Vector3,
    b: Vector3,
    c: Vector3,
) -> Option<(f32, Vector3, Vector3)> {
    let ab = sub(b, a);
    let ac = sub(c, a);
    let p = direction.cross(ac);
    let det = ab.dot(p);
    if det.abs() < 1.0e-6 {
        return None;
    }
    let inv_det = 1.0 / det;
    let tvec = sub(origin, a);
    let u = tvec.dot(p) * inv_det;
    if !(0.0..=1.0).contains(&u) {
        return None;
    }
    let q = tvec.cross(ab);
    let v = direction.dot(q) * inv_det;
    if v < 0.0 || u + v > 1.0 {
        return None;
    }
    let t = ac.dot(q) * inv_det;
    if t < 0.0 || t > max_distance {
        return None;
    }

    let hit = add(origin, mul(direction, t));
    let normal = ab.cross(ac).normalized();
    Some((t, hit, normal))
}

#[inline]
fn sub(a: Vector3, b: Vector3) -> Vector3 {
    Vector3::new(a.x - b.x, a.y - b.y, a.z - b.z)
}

#[inline]
fn add(a: Vector3, b: Vector3) -> Vector3 {
    Vector3::new(a.x + b.x, a.y + b.y, a.z + b.z)
}

#[inline]
fn mul(a: Vector3, s: f32) -> Vector3 {
    Vector3::new(a.x * s, a.y * s, a.z * s)
}
