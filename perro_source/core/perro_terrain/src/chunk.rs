use perro_structs::Vector3;

pub type VertexID = usize;
pub type TriangleID = usize;
/// Fixed chunk topology: 64 cells per side => 1 meter per cell at 64m chunk size.
pub const CHUNK_GRID_CELLS_PER_SIDE: usize = 64;
pub const CHUNK_GRID_VERTICES_PER_SIDE: usize = CHUNK_GRID_CELLS_PER_SIDE + 1;

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
        Self { size_meters: 64.0 }
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct TerrainChunk {
    pub coord: ChunkCoord,
    pub config: ChunkConfig,
    pub(crate) vertices: Vec<Vertex>,
    pub(crate) triangles: Vec<Triangle>,
}

impl TerrainChunk {
    /// Builds a fixed-topology grid chunk. Topology is static; editing mutates vertex heights.
    pub fn new_flat(coord: ChunkCoord, config: ChunkConfig) -> Self {
        let size = config.size_meters;
        let half = size * 0.5;
        let step = size / CHUNK_GRID_CELLS_PER_SIDE as f32;

        let mut vertices = Vec::with_capacity(CHUNK_GRID_VERTICES_PER_SIDE * CHUNK_GRID_VERTICES_PER_SIDE);
        for z in 0..CHUNK_GRID_VERTICES_PER_SIDE {
            let z_world = -half + z as f32 * step;
            for x in 0..CHUNK_GRID_VERTICES_PER_SIDE {
                let x_world = -half + x as f32 * step;
                vertices.push(Vertex::new(Vector3::new(x_world, 0.0, z_world)));
            }
        }

        let mut triangles = Vec::with_capacity(CHUNK_GRID_CELLS_PER_SIDE * CHUNK_GRID_CELLS_PER_SIDE * 2);
        for z in 0..CHUNK_GRID_CELLS_PER_SIDE {
            for x in 0..CHUNK_GRID_CELLS_PER_SIDE {
                let i00 = z * CHUNK_GRID_VERTICES_PER_SIDE + x;
                let i10 = i00 + 1;
                let i01 = i00 + CHUNK_GRID_VERTICES_PER_SIDE;
                let i11 = i01 + 1;

                triangles.push(Triangle::new(i00, i10, i01));
                triangles.push(Triangle::new(i01, i10, i11));
            }
        }

        Self {
            coord,
            config,
            vertices,
            triangles,
        }
    }

    pub fn new_flat_64m(coord: ChunkCoord) -> Self {
        Self::new_flat(coord, ChunkConfig::default())
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

    /// Grid spacing in local meters between adjacent vertices.
    pub(crate) fn grid_step_meters(&self) -> f32 {
        self.config.size_meters / CHUNK_GRID_CELLS_PER_SIDE as f32
    }

    pub(crate) fn snap_local_xz_to_grid(&self, x: f32, z: f32) -> Option<(usize, usize, f32, f32)> {
        let half = self.config.size_meters * 0.5;
        let step = self.grid_step_meters();
        let gx = ((x + half) / step).round();
        let gz = ((z + half) / step).round();
        if !gx.is_finite() || !gz.is_finite() {
            return None;
        }
        let gx_i = gx as i32;
        let gz_i = gz as i32;
        if gx_i < 0
            || gz_i < 0
            || gx_i > CHUNK_GRID_CELLS_PER_SIDE as i32
            || gz_i > CHUNK_GRID_CELLS_PER_SIDE as i32
        {
            return None;
        }
        let gx_u = gx_i as usize;
        let gz_u = gz_i as usize;
        let sx = -half + gx_u as f32 * step;
        let sz = -half + gz_u as f32 * step;
        Some((gx_u, gz_u, sx, sz))
    }

    pub(crate) const fn grid_index(grid_x: usize, grid_z: usize) -> usize {
        grid_z * CHUNK_GRID_VERTICES_PER_SIDE + grid_x
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
