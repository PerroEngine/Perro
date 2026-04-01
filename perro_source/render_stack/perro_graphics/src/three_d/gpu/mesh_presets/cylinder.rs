use super::super::MeshVertex;
use super::common::{push_index_triangle_outward, push_triangle};

pub(super) fn geometry(segments: u32) -> (Vec<MeshVertex>, Vec<u16>) {
    let seg = segments.max(3);
    let mut vertices = Vec::new();
    let mut indices = Vec::new();
    let top_y = 0.5;
    let bot_y = -0.5;
    let r = 0.5;

    for i in 0..seg {
        let a0 = i as f32 / seg as f32 * std::f32::consts::TAU;
        let a1 = (i + 1) as f32 / seg as f32 * std::f32::consts::TAU;
        let p0 = [r * a0.cos(), bot_y, r * a0.sin()];
        let p1 = [r * a1.cos(), bot_y, r * a1.sin()];
        let p2 = [r * a1.cos(), top_y, r * a1.sin()];
        let p3 = [r * a0.cos(), top_y, r * a0.sin()];
        let n0 = [a0.cos(), 0.0, a0.sin()];
        let n1 = [a1.cos(), 0.0, a1.sin()];
        let u0 = i as f32 / seg as f32;
        let u1 = (i + 1) as f32 / seg as f32;
        let base = vertices.len() as u16;
        vertices.push(MeshVertex {
            pos: p0,
            normal: n0,
            uv: [u0, 0.0],
            joints: [0, 0, 0, 0],
            weights: [1.0, 0.0, 0.0, 0.0],
        });
        vertices.push(MeshVertex {
            pos: p1,
            normal: n1,
            uv: [u1, 0.0],
            joints: [0, 0, 0, 0],
            weights: [1.0, 0.0, 0.0, 0.0],
        });
        vertices.push(MeshVertex {
            pos: p2,
            normal: n1,
            uv: [u1, 1.0],
            joints: [0, 0, 0, 0],
            weights: [1.0, 0.0, 0.0, 0.0],
        });
        vertices.push(MeshVertex {
            pos: p3,
            normal: n0,
            uv: [u0, 1.0],
            joints: [0, 0, 0, 0],
            weights: [1.0, 0.0, 0.0, 0.0],
        });
        push_index_triangle_outward(&vertices, &mut indices, base, base + 1, base + 2);
        push_index_triangle_outward(&vertices, &mut indices, base, base + 2, base + 3);
        push_triangle(&mut vertices, &mut indices, [0.0, top_y, 0.0], p2, p3);
        push_triangle(&mut vertices, &mut indices, [0.0, bot_y, 0.0], p0, p1);
    }
    (vertices, indices)
}
