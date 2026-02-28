use perro_structs::Vector3;

pub type VertexID = usize;
pub type TriangleID = usize;

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

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
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
    pub fn new_flat(coord: ChunkCoord, config: ChunkConfig) -> Self {
        let size = config.size_meters;
        let half = size * 0.5;
        let v0 = Vertex::new(Vector3::new(-half, 0.0, -half));
        let v1 = Vertex::new(Vector3::new(half, 0.0, -half));
        let v2 = Vertex::new(Vector3::new(-half, 0.0, half));
        let v3 = Vertex::new(Vector3::new(half, 0.0, half));

        let vertices = vec![v0, v1, v2, v3];
        let triangles = vec![Triangle::new(0, 1, 2), Triangle::new(2, 1, 3)];

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
            return Err(ChunkError::InvalidTriangleID { triangle_id: tri_id });
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
