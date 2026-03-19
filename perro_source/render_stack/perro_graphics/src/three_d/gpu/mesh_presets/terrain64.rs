use super::super::MeshVertex;
use perro_terrain::{ChunkCoord, TerrainChunk};

pub(super) fn geometry() -> (Vec<MeshVertex>, Vec<u16>) {
    let chunk = TerrainChunk::new_flat_64m(ChunkCoord::new(0, 0));
    let vertices = chunk
        .vertices()
        .iter()
        .map(|vertex| MeshVertex {
            pos: [vertex.position.x, vertex.position.y, vertex.position.z],
            normal: [0.0, 1.0, 0.0],
            joints: [0, 0, 0, 0],
            weights: [1.0, 0.0, 0.0, 0.0],
        })
        .collect::<Vec<_>>();

    let mut indices = Vec::with_capacity(chunk.triangles().len() * 3);
    for tri in chunk.triangles() {
        let a = tri.a as u16;
        let mut b = tri.b as u16;
        let mut c = tri.c as u16;

        let pa = vertices[a as usize].pos;
        let pb = vertices[b as usize].pos;
        let pc = vertices[c as usize].pos;
        let ab = [pb[0] - pa[0], pb[1] - pa[1], pb[2] - pa[2]];
        let ac = [pc[0] - pa[0], pc[1] - pa[1], pc[2] - pa[2]];
        let ny = ab[2] * ac[0] - ab[0] * ac[2];
        if ny < 0.0 {
            std::mem::swap(&mut b, &mut c);
        }

        indices.extend_from_slice(&[a, b, c]);
    }

    (vertices, indices)
}
