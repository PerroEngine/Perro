use super::common::push_index_triangle_outward;
use super::super::MeshVertex;
use glam::Vec3;

pub(super) fn geometry(longitude_segments: u32, latitude_segments: u32) -> (Vec<MeshVertex>, Vec<u16>) {
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
            let n = Vec3::new(sin_phi * theta.cos(), cos_phi, sin_phi * theta.sin());
            vertices.push(MeshVertex {
                pos: (n * 0.5).to_array(),
                normal: n.to_array(),
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
