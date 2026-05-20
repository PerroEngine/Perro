//! Shared built-in 3D mesh geometry for render and query paths.

pub const CUBE_SOURCE: &str = "__cube__";
pub const QUAD_SOURCE: &str = "__quad__";
pub const TRIANGULAR_PYRAMID_SOURCE: &str = "__tri_pyr__";
pub const SQUARE_PYRAMID_SOURCE: &str = "__sq_pyr__";
pub const SPHERE_SOURCE: &str = "__sphere__";
pub const TRIANGULAR_PRISM_SOURCE: &str = "__tri_prism__";
pub const CYLINDER_SOURCE: &str = "__cylinder__";
pub const CONE_SOURCE: &str = "__cone__";
pub const CAPSULE_SOURCE: &str = "__capsule__";

pub const BUILTIN_MESH_SOURCES: &[&str] = &[
    CUBE_SOURCE,
    QUAD_SOURCE,
    TRIANGULAR_PYRAMID_SOURCE,
    SQUARE_PYRAMID_SOURCE,
    SPHERE_SOURCE,
    TRIANGULAR_PRISM_SOURCE,
    CYLINDER_SOURCE,
    CONE_SOURCE,
    CAPSULE_SOURCE,
];

const ROUND_SEGMENTS: u32 = 36;
const CYLINDER_SEGMENTS: u32 = ROUND_SEGMENTS * 3;
const SPHERE_LATITUDE_BANDS: u32 = 24;
const CAPSULE_HEMISPHERE_BANDS: u32 = 14;

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct BuiltinMeshVertex {
    pub pos: [f32; 3],
    pub normal: [f32; 3],
    pub uv: [f32; 2],
}

#[derive(Clone, Debug, PartialEq)]
pub struct BuiltinMesh {
    pub vertices: Vec<BuiltinMeshVertex>,
    pub indices: Vec<u16>,
}

pub fn is_builtin_mesh_source(source: &str) -> bool {
    BUILTIN_MESH_SOURCES.contains(&source)
}

pub fn build_builtin_mesh(source: &str) -> Option<BuiltinMesh> {
    let (vertices, indices) = match source {
        CUBE_SOURCE => cube(),
        QUAD_SOURCE => quad(),
        TRIANGULAR_PYRAMID_SOURCE => triangular_pyramid(),
        SQUARE_PYRAMID_SOURCE => square_pyramid(),
        SPHERE_SOURCE => sphere(ROUND_SEGMENTS, SPHERE_LATITUDE_BANDS),
        TRIANGULAR_PRISM_SOURCE => triangular_prism(),
        CYLINDER_SOURCE => cylinder(CYLINDER_SEGMENTS),
        CONE_SOURCE => cone(ROUND_SEGMENTS),
        CAPSULE_SOURCE => capsule(ROUND_SEGMENTS, CAPSULE_HEMISPHERE_BANDS),
        _ => return None,
    };
    Some(BuiltinMesh { vertices, indices })
}

fn quad() -> (Vec<BuiltinMeshVertex>, Vec<u16>) {
    (
        vec![
            BuiltinMeshVertex {
                pos: [-0.5, -0.5, 0.0],
                normal: [0.0, 0.0, 1.0],
                uv: [0.0, 1.0],
            },
            BuiltinMeshVertex {
                pos: [0.5, -0.5, 0.0],
                normal: [0.0, 0.0, 1.0],
                uv: [1.0, 1.0],
            },
            BuiltinMeshVertex {
                pos: [0.5, 0.5, 0.0],
                normal: [0.0, 0.0, 1.0],
                uv: [1.0, 0.0],
            },
            BuiltinMeshVertex {
                pos: [-0.5, 0.5, 0.0],
                normal: [0.0, 0.0, 1.0],
                uv: [0.0, 0.0],
            },
        ],
        vec![0, 1, 2, 0, 2, 3],
    )
}

fn cube() -> (Vec<BuiltinMeshVertex>, Vec<u16>) {
    let mut vertices = Vec::new();
    let mut indices = Vec::new();
    push_quad(
        &mut vertices,
        &mut indices,
        [-0.5, -0.5, 0.5],
        [0.5, -0.5, 0.5],
        [0.5, 0.5, 0.5],
        [-0.5, 0.5, 0.5],
    );
    push_quad(
        &mut vertices,
        &mut indices,
        [0.5, -0.5, -0.5],
        [-0.5, -0.5, -0.5],
        [-0.5, 0.5, -0.5],
        [0.5, 0.5, -0.5],
    );
    push_quad(
        &mut vertices,
        &mut indices,
        [-0.5, -0.5, -0.5],
        [-0.5, -0.5, 0.5],
        [-0.5, 0.5, 0.5],
        [-0.5, 0.5, -0.5],
    );
    push_quad(
        &mut vertices,
        &mut indices,
        [0.5, -0.5, 0.5],
        [0.5, -0.5, -0.5],
        [0.5, 0.5, -0.5],
        [0.5, 0.5, 0.5],
    );
    push_quad(
        &mut vertices,
        &mut indices,
        [-0.5, 0.5, 0.5],
        [0.5, 0.5, 0.5],
        [0.5, 0.5, -0.5],
        [-0.5, 0.5, -0.5],
    );
    push_quad(
        &mut vertices,
        &mut indices,
        [-0.5, -0.5, -0.5],
        [0.5, -0.5, -0.5],
        [0.5, -0.5, 0.5],
        [-0.5, -0.5, 0.5],
    );
    (vertices, indices)
}

fn triangular_pyramid() -> (Vec<BuiltinMeshVertex>, Vec<u16>) {
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

fn square_pyramid() -> (Vec<BuiltinMeshVertex>, Vec<u16>) {
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

fn sphere(longitude_segments: u32, latitude_segments: u32) -> (Vec<BuiltinMeshVertex>, Vec<u16>) {
    let lon = longitude_segments.max(3);
    let lat = latitude_segments.max(2);
    let mut vertices = Vec::new();
    let mut indices = Vec::new();

    for y in 0..=lat {
        let v = y as f32 / lat as f32;
        let phi = v * std::f32::consts::PI;
        let sin_phi = phi.sin();
        let cos_phi = phi.cos();
        for x in 0..=lon {
            let u = x as f32 / lon as f32;
            let theta = u * std::f32::consts::TAU;
            let n = [sin_phi * theta.cos(), cos_phi, sin_phi * theta.sin()];
            vertices.push(BuiltinMeshVertex {
                pos: scale3(n, 0.5),
                normal: n,
                uv: [u, v],
            });
        }
    }

    let row = lon + 1;
    for y in 0..lat {
        for x in 0..lon {
            let i0 = y * row + x;
            let i1 = i0 + 1;
            let i2 = i0 + row;
            let i3 = i2 + 1;
            push_index_triangle_outward(&vertices, &mut indices, i0 as u16, i2 as u16, i1 as u16);
            push_index_triangle_outward(&vertices, &mut indices, i1 as u16, i2 as u16, i3 as u16);
        }
    }
    (vertices, indices)
}

fn triangular_prism() -> (Vec<BuiltinMeshVertex>, Vec<u16>) {
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

fn cylinder(segments: u32) -> (Vec<BuiltinMeshVertex>, Vec<u16>) {
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
        vertices.push(BuiltinMeshVertex {
            pos: p0,
            normal: n0,
            uv: [u0, 0.0],
        });
        vertices.push(BuiltinMeshVertex {
            pos: p1,
            normal: n1,
            uv: [u1, 0.0],
        });
        vertices.push(BuiltinMeshVertex {
            pos: p2,
            normal: n1,
            uv: [u1, 1.0],
        });
        vertices.push(BuiltinMeshVertex {
            pos: p3,
            normal: n0,
            uv: [u0, 1.0],
        });
        push_index_triangle_outward(&vertices, &mut indices, base, base + 1, base + 2);
        push_index_triangle_outward(&vertices, &mut indices, base, base + 2, base + 3);
        push_triangle(&mut vertices, &mut indices, [0.0, top_y, 0.0], p2, p3);
        push_triangle(&mut vertices, &mut indices, [0.0, bot_y, 0.0], p0, p1);
    }
    (vertices, indices)
}

fn cone(segments: u32) -> (Vec<BuiltinMeshVertex>, Vec<u16>) {
    let seg = segments.max(3);
    let mut vertices = Vec::new();
    let mut indices = Vec::new();
    let apex = [0.0, 0.6, 0.0];
    let by = -0.5;
    let r = 0.5;
    let height = apex[1] - by;
    for i in 0..seg {
        let a0 = i as f32 / seg as f32 * std::f32::consts::TAU;
        let a1 = (i + 1) as f32 / seg as f32 * std::f32::consts::TAU;
        let p0 = [r * a0.cos(), by, r * a0.sin()];
        let p1 = [r * a1.cos(), by, r * a1.sin()];
        let n0 = normalize3([height * a0.cos(), r, height * a0.sin()]);
        let n1 = normalize3([height * a1.cos(), r, height * a1.sin()]);
        let u0 = i as f32 / seg as f32;
        let u1 = (i + 1) as f32 / seg as f32;
        let base = vertices.len() as u16;
        vertices.push(BuiltinMeshVertex {
            pos: apex,
            normal: n0,
            uv: [u0, 1.0],
        });
        vertices.push(BuiltinMeshVertex {
            pos: p0,
            normal: n0,
            uv: [u0, 0.0],
        });
        vertices.push(BuiltinMeshVertex {
            pos: p1,
            normal: n1,
            uv: [u1, 0.0],
        });
        push_index_triangle_outward(&vertices, &mut indices, base, base + 1, base + 2);
        push_triangle(&mut vertices, &mut indices, [0.0, by, 0.0], p1, p0);
    }
    (vertices, indices)
}

fn capsule(longitude_segments: u32, hemisphere_rings: u32) -> (Vec<BuiltinMeshVertex>, Vec<u16>) {
    let lon = longitude_segments.max(6);
    let rings = hemisphere_rings.max(2);
    let mut vertices = Vec::new();
    let mut indices = Vec::new();

    for y in 0..=rings {
        let v = y as f32 / rings as f32;
        let phi = v * std::f32::consts::FRAC_PI_2;
        let sin_phi = phi.sin();
        let cos_phi = phi.cos();
        for x in 0..=lon {
            let u = x as f32 / lon as f32;
            let theta = u * std::f32::consts::TAU;
            let n = [sin_phi * theta.cos(), cos_phi, sin_phi * theta.sin()];
            let p = [n[0] * 0.5, n[1] * 0.5 + 0.25, n[2] * 0.5];
            vertices.push(BuiltinMeshVertex {
                pos: p,
                normal: n,
                uv: [u, v * 0.5],
            });
        }
    }

    let base_offset = vertices.len() as u16;
    for y in 0..=rings {
        let v = y as f32 / rings as f32;
        let phi = v * std::f32::consts::FRAC_PI_2;
        let sin_phi = phi.sin();
        let cos_phi = phi.cos();
        for x in 0..=lon {
            let u = x as f32 / lon as f32;
            let theta = u * std::f32::consts::TAU;
            let n = [sin_phi * theta.cos(), -cos_phi, sin_phi * theta.sin()];
            let p = [n[0] * 0.5, n[1] * 0.5 - 0.25, n[2] * 0.5];
            vertices.push(BuiltinMeshVertex {
                pos: p,
                normal: n,
                uv: [u, 0.5 + (v * 0.5)],
            });
        }
    }

    let row = lon + 1;
    for y in 0..rings {
        for x in 0..lon {
            let i0 = y * row + x;
            let i1 = i0 + 1;
            let i2 = i0 + row;
            let i3 = i2 + 1;
            push_index_triangle_outward(&vertices, &mut indices, i0 as u16, i2 as u16, i1 as u16);
            push_index_triangle_outward(&vertices, &mut indices, i1 as u16, i2 as u16, i3 as u16);
        }
    }

    let top_equator = rings * row;
    let bottom_equator = base_offset as u32 + rings * row;
    for x in 0..lon {
        let t0 = top_equator + x;
        let t1 = t0 + 1;
        let b0 = bottom_equator + x;
        let b1 = b0 + 1;
        push_index_triangle_outward(&vertices, &mut indices, t0 as u16, b0 as u16, t1 as u16);
        push_index_triangle_outward(&vertices, &mut indices, t1 as u16, b0 as u16, b1 as u16);
    }

    for y in 0..rings {
        for x in 0..lon {
            let i0 = base_offset as u32 + y * row + x;
            let i1 = i0 + 1;
            let i2 = i0 + row;
            let i3 = i2 + 1;
            push_index_triangle_outward(&vertices, &mut indices, i0 as u16, i1 as u16, i2 as u16);
            push_index_triangle_outward(&vertices, &mut indices, i1 as u16, i3 as u16, i2 as u16);
        }
    }
    (vertices, indices)
}

fn push_triangle(
    vertices: &mut Vec<BuiltinMeshVertex>,
    indices: &mut Vec<u16>,
    a: [f32; 3],
    b: [f32; 3],
    c: [f32; 3],
) {
    let av = a;
    let mut bv = b;
    let mut cv = c;
    let mut normal = normalize3(cross3(sub3(bv, av), sub3(cv, av)));
    let centroid = scale3(add3(add3(av, bv), cv), 1.0 / 3.0);
    if dot3(normal, centroid) < 0.0 {
        std::mem::swap(&mut bv, &mut cv);
        normal = normalize3(cross3(sub3(bv, av), sub3(cv, av)));
    }
    let auv = project_uv(a, normal);
    let buv = project_uv(bv, normal);
    let cuv = project_uv(cv, normal);
    let base = vertices.len() as u16;
    vertices.push(BuiltinMeshVertex {
        pos: a,
        normal,
        uv: auv,
    });
    vertices.push(BuiltinMeshVertex {
        pos: bv,
        normal,
        uv: buv,
    });
    vertices.push(BuiltinMeshVertex {
        pos: cv,
        normal,
        uv: cuv,
    });
    indices.extend_from_slice(&[base, base + 1, base + 2]);
}

fn push_quad(
    vertices: &mut Vec<BuiltinMeshVertex>,
    indices: &mut Vec<u16>,
    a: [f32; 3],
    b: [f32; 3],
    c: [f32; 3],
    d: [f32; 3],
) {
    push_triangle(vertices, indices, a, b, c);
    push_triangle(vertices, indices, a, c, d);
}

fn push_index_triangle_outward(
    vertices: &[BuiltinMeshVertex],
    indices: &mut Vec<u16>,
    i0: u16,
    i1: u16,
    i2: u16,
) {
    let p0 = vertices[i0 as usize].pos;
    let p1 = vertices[i1 as usize].pos;
    let p2 = vertices[i2 as usize].pos;
    let n = cross3(sub3(p1, p0), sub3(p2, p0));
    let centroid = scale3(add3(add3(p0, p1), p2), 1.0 / 3.0);
    if dot3(n, centroid) < 0.0 {
        indices.extend_from_slice(&[i0, i2, i1]);
    } else {
        indices.extend_from_slice(&[i0, i1, i2]);
    }
}

fn project_uv(pos: [f32; 3], normal: [f32; 3]) -> [f32; 2] {
    let ax = normal[0].abs();
    let ay = normal[1].abs();
    let az = normal[2].abs();
    if ay >= ax && ay >= az {
        [pos[0] + 0.5, pos[2] + 0.5]
    } else if ax >= az {
        [pos[2] + 0.5, pos[1] + 0.5]
    } else {
        [pos[0] + 0.5, pos[1] + 0.5]
    }
}

#[inline]
fn add3(a: [f32; 3], b: [f32; 3]) -> [f32; 3] {
    [a[0] + b[0], a[1] + b[1], a[2] + b[2]]
}

#[inline]
fn sub3(a: [f32; 3], b: [f32; 3]) -> [f32; 3] {
    [a[0] - b[0], a[1] - b[1], a[2] - b[2]]
}

#[inline]
fn scale3(v: [f32; 3], scale: f32) -> [f32; 3] {
    [v[0] * scale, v[1] * scale, v[2] * scale]
}

#[inline]
fn cross3(a: [f32; 3], b: [f32; 3]) -> [f32; 3] {
    [
        a[1] * b[2] - a[2] * b[1],
        a[2] * b[0] - a[0] * b[2],
        a[0] * b[1] - a[1] * b[0],
    ]
}

#[inline]
fn dot3(a: [f32; 3], b: [f32; 3]) -> f32 {
    a[0] * b[0] + a[1] * b[1] + a[2] * b[2]
}

#[inline]
fn normalize3(v: [f32; 3]) -> [f32; 3] {
    let len_sq = dot3(v, v);
    if len_sq > 1.0e-12 && len_sq.is_finite() {
        scale3(v, len_sq.sqrt().recip())
    } else {
        [0.0, 0.0, 0.0]
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_builtin_mesh_builds_nonempty_triangles() {
        for source in BUILTIN_MESH_SOURCES {
            let mesh = build_builtin_mesh(source).expect("builtin mesh");
            assert!(!mesh.vertices.is_empty(), "{source} vertices");
            assert!(!mesh.indices.is_empty(), "{source} indices");
            assert_eq!(mesh.indices.len() % 3, 0, "{source} triangles");
            for &index in &mesh.indices {
                assert!((index as usize) < mesh.vertices.len(), "{source} index");
            }
        }
    }

    #[test]
    fn source_check_matches_builder() {
        assert!(is_builtin_mesh_source(CUBE_SOURCE));
        assert!(build_builtin_mesh(CUBE_SOURCE).is_some());
        assert!(is_builtin_mesh_source(QUAD_SOURCE));
        assert!(build_builtin_mesh(QUAD_SOURCE).is_some());
        assert!(!is_builtin_mesh_source("__missing__"));
        assert!(build_builtin_mesh("__missing__").is_none());
    }

    #[test]
    fn quad_builtin_is_single_rect_face() {
        let mesh = build_builtin_mesh(QUAD_SOURCE).expect("quad");
        assert_eq!(mesh.vertices.len(), 4);
        assert_eq!(mesh.indices, vec![0, 1, 2, 0, 2, 3]);
        assert_eq!(mesh.vertices[0].uv, [0.0, 1.0]);
        assert_eq!(mesh.vertices[2].uv, [1.0, 0.0]);
    }
}
