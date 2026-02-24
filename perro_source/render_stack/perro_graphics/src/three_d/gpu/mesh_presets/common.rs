use super::super::MeshVertex;
use glam::Vec3;

pub(super) fn push_triangle(
    vertices: &mut Vec<MeshVertex>,
    indices: &mut Vec<u16>,
    a: [f32; 3],
    b: [f32; 3],
    c: [f32; 3],
) {
    let av = Vec3::from(a);
    let mut bv = Vec3::from(b);
    let mut cv = Vec3::from(c);
    let mut normal = (bv - av).cross(cv - av).normalize_or_zero();
    let centroid = (av + bv + cv) / 3.0;
    if normal.dot(centroid) < 0.0 {
        std::mem::swap(&mut bv, &mut cv);
        normal = (bv - av).cross(cv - av).normalize_or_zero();
    }
    let base = vertices.len() as u16;
    vertices.push(MeshVertex {
        pos: a,
        normal: normal.to_array(),
    });
    vertices.push(MeshVertex {
        pos: bv.to_array(),
        normal: normal.to_array(),
    });
    vertices.push(MeshVertex {
        pos: cv.to_array(),
        normal: normal.to_array(),
    });
    indices.extend_from_slice(&[base, base + 1, base + 2]);
}

pub(super) fn push_quad(
    vertices: &mut Vec<MeshVertex>,
    indices: &mut Vec<u16>,
    a: [f32; 3],
    b: [f32; 3],
    c: [f32; 3],
    d: [f32; 3],
) {
    push_triangle(vertices, indices, a, b, c);
    push_triangle(vertices, indices, a, c, d);
}

pub(super) fn push_index_triangle_outward(
    vertices: &[MeshVertex],
    indices: &mut Vec<u16>,
    i0: u16,
    i1: u16,
    i2: u16,
) {
    let p0 = Vec3::from(vertices[i0 as usize].pos);
    let p1 = Vec3::from(vertices[i1 as usize].pos);
    let p2 = Vec3::from(vertices[i2 as usize].pos);
    let n = (p1 - p0).cross(p2 - p0);
    let centroid = (p0 + p1 + p2) / 3.0;
    if n.dot(centroid) < 0.0 {
        indices.extend_from_slice(&[i0, i2, i1]);
    } else {
        indices.extend_from_slice(&[i0, i1, i2]);
    }
}
