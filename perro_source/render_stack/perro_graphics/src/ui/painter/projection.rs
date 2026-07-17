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

pub(super) fn project_label_primitives(
    primitives: &mut [ClippedPrimitive],
    source: UiRectState,
    quad: [[f32; 4]; 4],
    viewport: [f32; 2],
) {
    let (min, max) = source.screen_min_max(viewport);
    let width = (max[0] - min[0]).max(0.001);
    let height = (max[1] - min[1]).max(0.001);
    for primitive in primitives {
        primitive.clip_rect = Rect::EVERYTHING;
        if let Primitive::Mesh(mesh) = &mut primitive.primitive {
            let old = std::mem::replace(mesh, Mesh::with_texture(mesh.texture_id));
            for triangle in old.indices.chunks_exact(3) {
                let mut polygon = Vec::with_capacity(4);
                for &index in triangle {
                    let vertex = old.vertices[index as usize];
                    let u = ((vertex.pos.x - min[0]) / width).clamp(0.0, 1.0);
                    let v = ((vertex.pos.y - min[1]) / height).clamp(0.0, 1.0);
                    polygon.push(ProjectedLabelVertex {
                        clip: bilerp_clip_quad(quad, u, v),
                        uv: vertex.uv,
                        color: vertex.color,
                    });
                }
                clip_label_polygon_near(&mut polygon);
                for index in 1..polygon.len().saturating_sub(1) {
                    push_projected_label_triangle(
                        mesh,
                        [polygon[0], polygon[index], polygon[index + 1]],
                        viewport,
                    );
                }
            }
        }
    }
}

#[derive(Clone, Copy)]
pub(super) struct ProjectedLabelVertex {
    clip: [f32; 4],
    uv: epaint::Pos2,
    color: Color32,
}

pub(super) fn bilerp_clip_quad(quad: [[f32; 4]; 4], u: f32, v: f32) -> [f32; 4] {
    std::array::from_fn(|axis| {
        let top = quad[0][axis] + (quad[1][axis] - quad[0][axis]) * u;
        let bottom = quad[3][axis] + (quad[2][axis] - quad[3][axis]) * u;
        top + (bottom - top) * v
    })
}

pub(super) fn clip_label_polygon_near(polygon: &mut Vec<ProjectedLabelVertex>) {
    let input = std::mem::take(polygon);
    if input.is_empty() {
        return;
    }
    let mut previous = *input.last().unwrap();
    let mut previous_distance = previous.clip[2] + previous.clip[3];
    for current in input {
        let current_distance = current.clip[2] + current.clip[3];
        let previous_inside = previous_distance >= 1.0e-5;
        let current_inside = current_distance >= 1.0e-5;
        if previous_inside != current_inside {
            let t = ((1.0e-5 - previous_distance) / (current_distance - previous_distance))
                .clamp(0.0, 1.0);
            polygon.push(lerp_projected_label_vertex(previous, current, t));
        }
        if current_inside {
            polygon.push(current);
        }
        previous = current;
        previous_distance = current_distance;
    }
}

pub(super) fn lerp_projected_label_vertex(
    a: ProjectedLabelVertex,
    b: ProjectedLabelVertex,
    t: f32,
) -> ProjectedLabelVertex {
    ProjectedLabelVertex {
        clip: std::array::from_fn(|i| a.clip[i] + (b.clip[i] - a.clip[i]) * t),
        uv: a.uv + (b.uv - a.uv) * t,
        color: a.color,
    }
}

pub(super) fn push_projected_label_triangle(
    mesh: &mut Mesh,
    triangle: [ProjectedLabelVertex; 3],
    viewport: [f32; 2],
) {
    if triangle.iter().any(|vertex| vertex.clip[3].abs() <= 1.0e-6) {
        return;
    }
    let base = mesh.vertices.len() as u32;
    for vertex in triangle {
        let ndc_x = vertex.clip[0] / vertex.clip[3];
        let ndc_y = vertex.clip[1] / vertex.clip[3];
        mesh.vertices.push(Vertex {
            pos: pos2(
                (ndc_x * 0.5 + 0.5) * viewport[0],
                (0.5 - ndc_y * 0.5) * viewport[1],
            ),
            uv: vertex.uv,
            color: vertex.color,
        });
    }
    mesh.indices.extend_from_slice(&[base, base + 1, base + 2]);
}
