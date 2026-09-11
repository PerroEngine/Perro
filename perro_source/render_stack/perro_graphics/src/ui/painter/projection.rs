use super::*;

pub(super) fn rotate_primitives(
    primitives: &mut [ClippedPrimitive],
    rotations: &[(f32, epaint::Pos2)],
) {
    for (primitive, &(rotation, origin)) in primitives.iter_mut().zip(rotations) {
        if !rotation.is_finite() || rotation == 0.0 {
            continue;
        }
        let rot = Rot2::from_angle(-rotation);
        primitive.clip_rect = Rect::EVERYTHING;
        if let Primitive::Mesh(mesh) = &mut primitive.primitive {
            mesh.rotate(rot, origin);
        }
    }
}

// Map local glyph vertices directly to homogeneous world clip coordinates.
// GPU clipping and perspective interpolation require the original z and w;
// dividing on the CPU and submitting a UI vertex (w = 1) loses both.
pub(super) fn project_label_primitives(
    primitives: &[Arc<ClippedPrimitive>],
    source: UiRectState,
    quad: [[f32; 4]; 4],
    depth_test: bool,
) -> Vec<UiWorldProjection> {
    let (min, max) = source.screen_min_max([0.0; 2]);
    let width = (max[0] - min[0]).max(0.001);
    let height = (max[1] - min[1]).max(0.001);
    primitives
        .iter()
        .map(|primitive| {
            let clip_positions = match &primitive.primitive {
                Primitive::Mesh(mesh) => mesh
                    .vertices
                    .iter()
                    .map(|vertex| {
                        let u = ((vertex.pos.x - min[0]) / width).clamp(0.0, 1.0);
                        let v = ((vertex.pos.y - min[1]) / height).clamp(0.0, 1.0);
                        bilerp_clip_quad(quad, u, v)
                    })
                    .collect(),
                _ => Arc::from([]),
            };
            UiWorldProjection {
                clip_positions,
                depth_test,
            }
        })
        .collect()
}

pub(super) fn bilerp_clip_quad(quad: [[f32; 4]; 4], u: f32, v: f32) -> [f32; 4] {
    std::array::from_fn(|axis| {
        let top = quad[0][axis] + (quad[1][axis] - quad[0][axis]) * u;
        let bottom = quad[3][axis] + (quad[2][axis] - quad[3][axis]) * u;
        top + (bottom - top) * v
    })
}
