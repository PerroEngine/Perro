use super::common::push_triangle;
use super::super::MeshVertex;

pub(super) fn geometry() -> (Vec<MeshVertex>, Vec<u16>) {
    let mut vertices = Vec::new();
    let mut indices = Vec::new();
    let p0 = [0.0, 0.6, 0.0];
    let p1 = [-0.5, -0.5, 0.5];
    let p2 = [0.5, -0.5, 0.5];
    let p3 = [0.0, -0.5, -0.6];
    push_triangle(&mut vertices, &mut indices, p0, p1, p2);
    push_triangle(&mut vertices, &mut indices, p0, p2, p3);
    push_triangle(&mut vertices, &mut indices, p0, p3, p1);
    push_triangle(&mut vertices, &mut indices, p1, p3, p2);
    (vertices, indices)
}
