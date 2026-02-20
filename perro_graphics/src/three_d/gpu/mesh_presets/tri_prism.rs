use super::common::{push_quad, push_triangle};
use super::super::MeshVertex;

pub(super) fn geometry() -> (Vec<MeshVertex>, Vec<u16>) {
    let mut vertices = Vec::new();
    let mut indices = Vec::new();
    let a0 = [-0.5, -0.5, -0.4];
    let a1 = [0.5, -0.5, -0.4];
    let a2 = [0.0, 0.5, -0.4];
    let b0 = [-0.5, -0.5, 0.4];
    let b1 = [0.5, -0.5, 0.4];
    let b2 = [0.0, 0.5, 0.4];
    push_triangle(&mut vertices, &mut indices, a0, a1, a2);
    push_triangle(&mut vertices, &mut indices, b0, b2, b1);
    push_quad(&mut vertices, &mut indices, a0, b0, b1, a1);
    push_quad(&mut vertices, &mut indices, a1, b1, b2, a2);
    push_quad(&mut vertices, &mut indices, a2, b2, b0, a0);
    (vertices, indices)
}
