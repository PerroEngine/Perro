use perro_structs::Vector3;
use std::collections::HashMap;

pub type VertexID = usize;
pub type TriangleID = usize;
/// Legacy fixed-grid topology constant kept for compatibility with existing tools.
pub const CHUNK_GRID_CELLS_PER_SIDE: usize = 64;
pub const CHUNK_GRID_VERTICES_PER_SIDE: usize = CHUNK_GRID_CELLS_PER_SIDE + 1;
pub const DEFAULT_CHUNK_SIZE_METERS: f32 = 512.0;

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Vertex {
    pub position: Vector3,
}

impl Vertex {
    pub const fn new(position: Vector3) -> Self {
        Self { position }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Triangle {
    pub a: VertexID,
    pub b: VertexID,
    pub c: VertexID,
}

impl Triangle {
    pub const fn new(a: VertexID, b: VertexID, c: VertexID) -> Self {
        Self { a, b, c }
    }

    pub const fn indices(self) -> [VertexID; 3] {
        [self.a, self.b, self.c]
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct ChunkCoord {
    pub x: i32,
    pub z: i32,
}

impl ChunkCoord {
    pub const fn new(x: i32, z: i32) -> Self {
        Self { x, z }
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct ChunkConfig {
    pub size_meters: f32,
}

impl ChunkConfig {
    pub const fn new(size_meters: f32) -> Self {
        Self { size_meters }
    }
}

impl Default for ChunkConfig {
    fn default() -> Self {
        Self {
            size_meters: DEFAULT_CHUNK_SIZE_METERS,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ChunkTopology {
    Grid { cells_per_side: usize },
    ArbitraryMesh,
}

#[derive(Clone, Debug, PartialEq)]
pub struct TerrainChunk {
    pub coord: ChunkCoord,
    pub config: ChunkConfig,
    pub(crate) topology: ChunkTopology,
    pub(crate) vertices: Vec<Vertex>,
    pub(crate) triangles: Vec<Triangle>,
}

impl TerrainChunk {
    /// Builds a square grid chunk. The default density is 1 vertex per meter.
    pub fn new_flat(coord: ChunkCoord, config: ChunkConfig) -> Self {
        let cells_per_side = config.size_meters.round().max(1.0) as usize;
        Self::new_flat_with_cells(coord, config, cells_per_side)
    }

    pub fn new_flat_with_cells(
        coord: ChunkCoord,
        config: ChunkConfig,
        cells_per_side: usize,
    ) -> Self {
        let cells_per_side = cells_per_side.max(1);
        let size = config.size_meters;
        let half = size * 0.5;
        let verts_per_side = cells_per_side + 1;
        let step = size / cells_per_side as f32;

        let mut vertices = Vec::with_capacity(verts_per_side * verts_per_side);
        for z in 0..verts_per_side {
            let z_world = -half + z as f32 * step;
            for x in 0..verts_per_side {
                let x_world = -half + x as f32 * step;
                vertices.push(Vertex::new(Vector3::new(x_world, 0.0, z_world)));
            }
        }

        let mut triangles = Vec::with_capacity(cells_per_side * cells_per_side * 2);
        for z in 0..cells_per_side {
            for x in 0..cells_per_side {
                let i00 = z * verts_per_side + x;
                let i10 = i00 + 1;
                let i01 = i00 + verts_per_side;
                let i11 = i01 + 1;

                triangles.push(Triangle::new(i00, i10, i01));
                triangles.push(Triangle::new(i01, i10, i11));
            }
        }

        Self {
            coord,
            config,
            topology: ChunkTopology::Grid { cells_per_side },
            vertices,
            triangles,
        }
    }

    pub fn new_flat_64m(coord: ChunkCoord) -> Self {
        Self::new_flat(coord, ChunkConfig::new(64.0))
    }

    pub fn from_mesh(
        coord: ChunkCoord,
        config: ChunkConfig,
        vertices: Vec<Vertex>,
        triangles: Vec<Triangle>,
    ) -> Result<Self, ChunkError> {
        let chunk = Self {
            coord,
            config,
            topology: ChunkTopology::ArbitraryMesh,
            vertices,
            triangles,
        };
        chunk.validate(1.0e-6)?;
        Ok(chunk)
    }

    pub const fn topology(&self) -> ChunkTopology {
        self.topology
    }

    pub fn grid_cells_per_side(&self) -> Option<usize> {
        match self.topology {
            ChunkTopology::Grid { cells_per_side } => Some(cells_per_side),
            ChunkTopology::ArbitraryMesh => None,
        }
    }

    pub fn vertices(&self) -> &[Vertex] {
        &self.vertices
    }

    pub fn triangles(&self) -> &[Triangle] {
        &self.triangles
    }

    pub fn vertex_count(&self) -> usize {
        self.vertices.len()
    }

    pub fn triangle_count(&self) -> usize {
        self.triangles.len()
    }

    pub fn nearest_vertex_id_by_xz(&self, x: f32, z: f32) -> Option<VertexID> {
        let mut best: Option<(usize, f32)> = None;
        for (id, vertex) in self.vertices.iter().enumerate() {
            let dx = vertex.position.x - x;
            let dz = vertex.position.z - z;
            let d2 = dx * dx + dz * dz;
            match best {
                Some((_, best_d2)) if best_d2 <= d2 => {}
                _ => best = Some((id, d2)),
            }
        }
        best.map(|(id, _)| id)
    }

    pub fn has_single_height_per_xz(&self, epsilon: f32) -> bool {
        if !epsilon.is_finite() || epsilon < 0.0 {
            return false;
        }
        let mut by_xz: HashMap<(i64, i64), f32> = HashMap::new();
        let inv = 1.0 / epsilon.max(1.0e-4);
        for v in &self.vertices {
            let kx = (v.position.x * inv).round() as i64;
            let kz = (v.position.z * inv).round() as i64;
            let key = (kx, kz);
            if let Some(existing_y) = by_xz.get(&key) {
                if (*existing_y - v.position.y).abs() > epsilon {
                    return false;
                }
            } else {
                by_xz.insert(key, v.position.y);
            }
        }
        true
    }

    /// Grid spacing in local meters between adjacent vertices.
    pub(crate) fn grid_step_meters(&self) -> Option<f32> {
        let cells = self.grid_cells_per_side()?;
        Some(self.config.size_meters / cells as f32)
    }

    pub(crate) fn snap_local_xz_to_grid(&self, x: f32, z: f32) -> Option<(usize, usize, f32, f32)> {
        let cells_per_side = self.grid_cells_per_side()?;
        let half = self.config.size_meters * 0.5;
        let step = self.grid_step_meters()?;
        let gx = ((x + half) / step).round();
        let gz = ((z + half) / step).round();
        if !gx.is_finite() || !gz.is_finite() {
            return None;
        }
        let gx_i = gx as i32;
        let gz_i = gz as i32;
        if gx_i < 0 || gz_i < 0 || gx_i > cells_per_side as i32 || gz_i > cells_per_side as i32 {
            return None;
        }
        let gx_u = gx_i as usize;
        let gz_u = gz_i as usize;
        let sx = -half + gx_u as f32 * step;
        let sz = -half + gz_u as f32 * step;
        Some((gx_u, gz_u, sx, sz))
    }

    pub fn grid_index(&self, grid_x: usize, grid_z: usize) -> Option<usize> {
        let cells = self.grid_cells_per_side()?;
        let verts = cells + 1;
        if grid_x > cells || grid_z > cells {
            return None;
        }
        Some(grid_z * verts + grid_x)
    }

    /// Legacy API: runtime terrain editing should use snapped grid height edits (`insert_vertex`).
    pub fn add_vertex(&mut self, position: Vector3) -> VertexID {
        let id = self.vertices.len();
        self.vertices.push(Vertex::new(position));
        id
    }

    pub fn set_vertex_position(
        &mut self,
        vertex_id: VertexID,
        position: Vector3,
    ) -> Result<(), ChunkError> {
        let Some(vertex) = self.vertices.get_mut(vertex_id) else {
            return Err(ChunkError::InvalidVertexID { vertex_id });
        };
        vertex.position = position;
        Ok(())
    }

    /// Legacy API: runtime terrain editing should keep fixed topology and avoid adding triangles.
    pub fn add_triangle(
        &mut self,
        a: VertexID,
        b: VertexID,
        c: VertexID,
    ) -> Result<TriangleID, ChunkError> {
        let tri = Triangle::new(a, b, c);
        Self::validate_triangle_indices(tri, self.vertices.len())?;

        let id = self.triangles.len();
        self.triangles.push(tri);
        Ok(id)
    }

    pub fn triangle_normal(&self, tri_id: TriangleID) -> Result<Vector3, ChunkError> {
        let Some(tri) = self.triangles.get(tri_id).copied() else {
            return Err(ChunkError::InvalidTriangleID {
                triangle_id: tri_id,
            });
        };
        let a = self.vertices[tri.a].position;
        let b = self.vertices[tri.b].position;
        let c = self.vertices[tri.c].position;

        let ab = Vector3::new(b.x - a.x, b.y - a.y, b.z - a.z);
        let ac = Vector3::new(c.x - a.x, c.y - a.y, c.z - a.z);
        Ok(ab.cross(ac).normalized())
    }

    pub fn validate(&self, area_epsilon: f32) -> Result<(), ChunkError> {
        if !self.config.size_meters.is_finite() || self.config.size_meters <= 0.0 {
            return Err(ChunkError::InvalidChunkSize {
                size_meters: self.config.size_meters,
            });
        }

        for (tri_id, tri) in self.triangles.iter().copied().enumerate() {
            Self::validate_triangle_indices(tri, self.vertices.len())?;

            let area2 = self.triangle_area2(tri);
            if area2 <= area_epsilon {
                return Err(ChunkError::DegenerateTriangle {
                    triangle_id: tri_id,
                });
            }
        }
        Ok(())
    }

    fn validate_triangle_indices(tri: Triangle, vertex_count: usize) -> Result<(), ChunkError> {
        for idx in tri.indices() {
            if idx >= vertex_count {
                return Err(ChunkError::InvalidVertexID { vertex_id: idx });
            }
        }
        if tri.a == tri.b || tri.b == tri.c || tri.a == tri.c {
            return Err(ChunkError::DuplicateTriangleVertex {
                indices: tri.indices(),
            });
        }
        Ok(())
    }

    fn triangle_area2(&self, tri: Triangle) -> f32 {
        let a = self.vertices[tri.a].position;
        let b = self.vertices[tri.b].position;
        let c = self.vertices[tri.c].position;

        let ab = Vector3::new(b.x - a.x, b.y - a.y, b.z - a.z);
        let ac = Vector3::new(c.x - a.x, c.y - a.y, c.z - a.z);
        ab.cross(ac).length()
    }
}

#[derive(Clone, Debug, PartialEq)]
pub enum ChunkError {
    InvalidChunkSize { size_meters: f32 },
    InvalidVertexID { vertex_id: VertexID },
    InvalidTriangleID { triangle_id: TriangleID },
    DuplicateTriangleVertex { indices: [VertexID; 3] },
    DegenerateTriangle { triangle_id: TriangleID },
    PointOutsideMesh { x: f32, z: f32 },
}
