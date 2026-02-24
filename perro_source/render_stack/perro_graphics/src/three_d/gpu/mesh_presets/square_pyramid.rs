use super::super::MeshVertex;
use super::common::{push_quad, push_triangle};

pub(super) fn geometry() -> (Vec<MeshVertex>, Vec<u16>) {
    let mut vertices = Vec::new();
    let mut indices = Vec::new();
    let top = [0.0, 0.65, 0.0];
    let b0 = [-0.5, -0.5, -0.5];
    let b1 = [0.5, -0.5, -0.5];
    let b2 = [0.5, -0.5, 0.5];
    let b3 = [-0.5, -0.5, 0.5];
    push_triangle(&mut vertices, &mut indices, top, b0, b1);
    push_triangle(&mut vertices, &mut indices, top, b1, b2);
    push_triangle(&mut vertices, &mut indices, top, b2, b3);
    push_triangle(&mut vertices, &mut indices, top, b3, b0);
    push_quad(&mut vertices, &mut indices, b0, b3, b2, b1);
    (vertices, indices)
}
