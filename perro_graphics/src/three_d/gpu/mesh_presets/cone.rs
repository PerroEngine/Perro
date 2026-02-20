use super::common::push_triangle;
use super::super::MeshVertex;

pub(super) fn geometry(segments: u32) -> (Vec<MeshVertex>, Vec<u16>) {
    let seg = segments.max(3);
    let mut vertices = Vec::new();
    let mut indices = Vec::new();
    let apex = [0.0, 0.6, 0.0];
    let by = -0.5;
    let r = 0.5;
    for i in 0..seg {
        let a0 = i as f32 / seg as f32 * std::f32::consts::TAU;
        let a1 = (i + 1) as f32 / seg as f32 * std::f32::consts::TAU;
        let p0 = [r * a0.cos(), by, r * a0.sin()];
        let p1 = [r * a1.cos(), by, r * a1.sin()];
        push_triangle(&mut vertices, &mut indices, apex, p0, p1);
        push_triangle(&mut vertices, &mut indices, [0.0, by, 0.0], p1, p0);
    }
    (vertices, indices)
}
