#[derive(Debug, Clone, Copy, PartialEq)]
pub struct MeshletBounds {
    pub index_start: u32,
    pub index_count: u32,
    pub center: [f32; 3],
    pub radius: f32,
}

#[derive(Debug, Clone, PartialEq)]
pub struct MeshletPack {
    pub packed_indices: Vec<u32>,
    pub meshlets: Vec<MeshletBounds>,
}

pub fn pack_meshlets_from_positions(
    positions: &[[f32; 3]],
    indices: &[u32],
    triangles_per_meshlet: usize,
) -> MeshletPack {
    let tri_len = (indices.len() / 3) * 3;
    if tri_len == 0 || triangles_per_meshlet == 0 {
        return MeshletPack {
            packed_indices: indices.to_vec(),
            meshlets: Vec::new(),
        };
    }

    let tri_count = tri_len / 3;
    let mut cmin = [f32::INFINITY; 3];
    let mut cmax = [f32::NEG_INFINITY; 3];
    for tri_i in 0..tri_count {
        let base = tri_i * 3;
        let Some(a) = positions.get(indices[base] as usize) else {
            return MeshletPack {
                packed_indices: indices.to_vec(),
                meshlets: Vec::new(),
            };
        };
        let Some(b) = positions.get(indices[base + 1] as usize) else {
            return MeshletPack {
                packed_indices: indices.to_vec(),
                meshlets: Vec::new(),
            };
        };
        let Some(c) = positions.get(indices[base + 2] as usize) else {
            return MeshletPack {
                packed_indices: indices.to_vec(),
                meshlets: Vec::new(),
            };
        };
        let cx = (a[0] + b[0] + c[0]) * (1.0 / 3.0);
        let cy = (a[1] + b[1] + c[1]) * (1.0 / 3.0);
        let cz = (a[2] + b[2] + c[2]) * (1.0 / 3.0);
        cmin[0] = cmin[0].min(cx);
        cmin[1] = cmin[1].min(cy);
        cmin[2] = cmin[2].min(cz);
        cmax[0] = cmax[0].max(cx);
        cmax[1] = cmax[1].max(cy);
        cmax[2] = cmax[2].max(cz);
    }

    let span = [
        (cmax[0] - cmin[0]).max(1.0e-6),
        (cmax[1] - cmin[1]).max(1.0e-6),
        (cmax[2] - cmin[2]).max(1.0e-6),
    ];

    let mut keyed = Vec::with_capacity(tri_count);
    for tri_i in 0..tri_count {
        let base = tri_i * 3;
        let Some(a) = positions.get(indices[base] as usize) else {
            return MeshletPack {
                packed_indices: indices.to_vec(),
                meshlets: Vec::new(),
            };
        };
        let Some(b) = positions.get(indices[base + 1] as usize) else {
            return MeshletPack {
                packed_indices: indices.to_vec(),
                meshlets: Vec::new(),
            };
        };
        let Some(c) = positions.get(indices[base + 2] as usize) else {
            return MeshletPack {
                packed_indices: indices.to_vec(),
                meshlets: Vec::new(),
            };
        };
        let nx = (((a[0] + b[0] + c[0]) * (1.0 / 3.0) - cmin[0]) / span[0]).clamp(0.0, 1.0);
        let ny = (((a[1] + b[1] + c[1]) * (1.0 / 3.0) - cmin[1]) / span[1]).clamp(0.0, 1.0);
        let nz = (((a[2] + b[2] + c[2]) * (1.0 / 3.0) - cmin[2]) / span[2]).clamp(0.0, 1.0);
        keyed.push((morton3(nx, ny, nz), tri_i as u32));
    }
    keyed.sort_unstable_by_key(|item| item.0);

    let mut packed_indices = Vec::with_capacity(indices.len());
    for (_, tri) in keyed {
        let base = tri as usize * 3;
        packed_indices.push(indices[base]);
        packed_indices.push(indices[base + 1]);
        packed_indices.push(indices[base + 2]);
    }
    if tri_len < indices.len() {
        packed_indices.extend_from_slice(&indices[tri_len..]);
    }

    let chunk = triangles_per_meshlet * 3;
    let packed_tri_len = (packed_indices.len() / 3) * 3;
    let mut meshlets = Vec::with_capacity(packed_tri_len.div_ceil(chunk));
    let mut start = 0usize;
    while start < packed_tri_len {
        let end = (start + chunk).min(packed_tri_len);
        if let Some((center, radius)) = meshlet_bounds(positions, &packed_indices[start..end]) {
            meshlets.push(MeshletBounds {
                index_start: start as u32,
                index_count: (end - start) as u32,
                center,
                radius,
            });
        }
        start = end;
    }

    MeshletPack {
        packed_indices,
        meshlets,
    }
}

fn meshlet_bounds(positions: &[[f32; 3]], indices: &[u32]) -> Option<([f32; 3], f32)> {
    let mut minx = f32::INFINITY;
    let mut miny = f32::INFINITY;
    let mut minz = f32::INFINITY;
    let mut maxx = f32::NEG_INFINITY;
    let mut maxy = f32::NEG_INFINITY;
    let mut maxz = f32::NEG_INFINITY;
    for &idx in indices {
        let p = positions.get(idx as usize)?;
        minx = minx.min(p[0]);
        miny = miny.min(p[1]);
        minz = minz.min(p[2]);
        maxx = maxx.max(p[0]);
        maxy = maxy.max(p[1]);
        maxz = maxz.max(p[2]);
    }
    if !(minx.is_finite()
        && miny.is_finite()
        && minz.is_finite()
        && maxx.is_finite()
        && maxy.is_finite()
        && maxz.is_finite())
    {
        return None;
    }
    let cx = (minx + maxx) * 0.5;
    let cy = (miny + maxy) * 0.5;
    let cz = (minz + maxz) * 0.5;
    let mut radius_sq = 0.0f32;
    for &idx in indices {
        let p = positions.get(idx as usize)?;
        let dx = p[0] - cx;
        let dy = p[1] - cy;
        let dz = p[2] - cz;
        let d2 = dx * dx + dy * dy + dz * dz;
        if d2 > radius_sq {
            radius_sq = d2;
        }
    }
    Some(([cx, cy, cz], radius_sq.sqrt()))
}

#[inline]
fn morton3(nx: f32, ny: f32, nz: f32) -> u64 {
    let qx = (nx * 1023.0).round() as u32;
    let qy = (ny * 1023.0).round() as u32;
    let qz = (nz * 1023.0).round() as u32;
    interleave10(qx) | (interleave10(qy) << 1) | (interleave10(qz) << 2)
}

#[inline]
fn interleave10(v: u32) -> u64 {
    let mut x = (v & 0x3ff) as u64;
    x = (x | (x << 16)) & 0x30000ff;
    x = (x | (x << 8)) & 0x300f00f;
    x = (x | (x << 4)) & 0x30c30c3;
    x = (x | (x << 2)) & 0x9249249;
    x
}
