use super::super::MeshVertex;
use super::common::push_index_triangle_outward;
use glam::Vec3;

pub(super) fn geometry(
    longitude_segments: u32,
    hemisphere_rings: u32,
) -> (Vec<MeshVertex>, Vec<u16>) {
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
            let n = Vec3::new(sin_phi * theta.cos(), cos_phi, sin_phi * theta.sin());
            let p = Vec3::new(n.x * 0.5, n.y * 0.5 + 0.25, n.z * 0.5);
            vertices.push(MeshVertex {
                pos: p.to_array(),
                normal: n.to_array(),
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
            let n = Vec3::new(sin_phi * theta.cos(), -cos_phi, sin_phi * theta.sin());
            let p = Vec3::new(n.x * 0.5, n.y * 0.5 - 0.25, n.z * 0.5);
            vertices.push(MeshVertex {
                pos: p.to_array(),
                normal: n.to_array(),
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
