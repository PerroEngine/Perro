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
    vertices: Vec<Vertex>,
    triangles: Vec<Triangle>,
}

impl TerrainChunk {
    pub fn new_flat(coord: ChunkCoord, config: ChunkConfig) -> Self {
        let size = config.size_meters;
        let v0 = Vertex::new(Vector3::new(0.0, 0.0, 0.0));
        let v1 = Vertex::new(Vector3::new(size, 0.0, 0.0));
        let v2 = Vertex::new(Vector3::new(0.0, 0.0, size));
        let v3 = Vertex::new(Vector3::new(size, 0.0, size));

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
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn flat_chunk_starts_with_4_vertices_and_2_triangles() {
        let c = TerrainChunk::new_flat_64m(ChunkCoord::new(0, 0));
        assert_eq!(c.vertex_count(), 4);
        assert_eq!(c.triangle_count(), 2);
        assert!(c.validate(1.0e-6).is_ok());
    }

    #[test]
    fn add_vertex_and_triangle_works() {
        let mut c = TerrainChunk::new_flat_64m(ChunkCoord::new(2, -1));
        let center = c.add_vertex(Vector3::new(32.0, 2.0, 32.0));
        let tri_id = c
            .add_triangle(0, 1, center)
            .expect("triangle should be valid");
        assert_eq!(tri_id, 2);
        assert_eq!(c.vertex_count(), 5);
        assert_eq!(c.triangle_count(), 3);
    }

    #[test]
    fn add_triangle_rejects_bad_indices() {
        let mut c = TerrainChunk::new_flat_64m(ChunkCoord::new(0, 0));
        let err = c
            .add_triangle(0, 1, 999)
            .expect_err("invalid index should fail");
        assert_eq!(err, ChunkError::InvalidVertexID { vertex_id: 999 });
    }

    #[test]
    fn validate_rejects_degenerate_triangles() {
        let mut c = TerrainChunk::new_flat_64m(ChunkCoord::new(0, 0));
        let v = c.add_vertex(Vector3::new(10.0, 0.0, 10.0));
        c.add_triangle(v, v, 0)
            .expect_err("duplicate index must fail early");

        c.triangles.push(Triangle::new(0, 0, 1));
        let err = c.validate(1.0e-6).expect_err("validation should fail");
        assert_eq!(
            err,
            ChunkError::DuplicateTriangleVertex { indices: [0, 0, 1] }
        );
    }

    #[test]
    fn set_vertex_position_updates_existing_vertex() {
        let mut c = TerrainChunk::new_flat_64m(ChunkCoord::new(0, 0));
        c.set_vertex_position(0, Vector3::new(-3.0, 5.0, 7.0))
            .expect("vertex should exist");
        assert_eq!(c.vertices()[0].position, Vector3::new(-3.0, 5.0, 7.0));
    }
}
