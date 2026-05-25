use super::*;

pub(super) fn build_query_mesh_from_runtime_mesh(mesh: &Mesh3D) -> Option<QueryMeshData> {
    let vertices: Vec<Vec3> = mesh
        .vertices
        .iter()
        .map(|vertex| Vec3::from_array(vertex.position))
        .collect();
    let mut triangles = Vec::new();
    for (surface_index, range) in mesh.surface_ranges.iter().enumerate() {
        let start = range.index_start as usize;
        let end = start.saturating_add(range.index_count as usize);
        let Some(indices) = mesh.indices.get(start..end) else {
            continue;
        };
        for tri in indices.chunks_exact(3) {
            triangles.push(QueryTri {
                a: tri[0],
                b: tri[1],
                c: tri[2],
                surface_index: surface_index as u32,
            });
        }
    }
    if mesh.surface_ranges.is_empty() {
        for tri in mesh.indices.chunks_exact(3) {
            triangles.push(QueryTri {
                a: tri[0],
                b: tri[1],
                c: tri[2],
                surface_index: 0,
            });
        }
    }
    build_query_mesh_data(vertices, triangles)
}

pub(super) fn decode_gltf_query_mesh(bytes: &[u8], mesh_index: usize) -> Option<QueryMeshData> {
    let (doc, buffers, _images) = gltf::import_slice(bytes).ok()?;
    let mesh = doc.meshes().nth(mesh_index)?;

    let mut vertices = Vec::new();
    let mut triangles = Vec::new();
    for (surface_index, primitive) in mesh.primitives().enumerate() {
        let reader = primitive.reader(|buffer| buffers.get(buffer.index()).map(|d| d.0.as_slice()));
        let positions = reader.read_positions()?;
        let mut local_positions =
            GLTF_POS_SCRATCH.with(|scratch| std::mem::take(&mut *scratch.borrow_mut()));
        local_positions.clear();
        local_positions.extend(positions);
        if local_positions.len() < 3 {
            GLTF_POS_SCRATCH.with(|scratch| {
                local_positions.clear();
                *scratch.borrow_mut() = local_positions;
            });
            continue;
        }

        let base = vertices.len() as u32;
        for p in local_positions.iter().copied() {
            vertices.push(Vec3::new(p[0], p[1], p[2]));
        }

        if let Some(indices_reader) = reader.read_indices() {
            let mut flat =
                GLTF_INDEX_SCRATCH.with(|scratch| std::mem::take(&mut *scratch.borrow_mut()));
            flat.clear();
            flat.extend(indices_reader.into_u32());
            for tri in flat.chunks_exact(3) {
                let ia = tri[0] as usize;
                let ib = tri[1] as usize;
                let ic = tri[2] as usize;
                if ia >= local_positions.len()
                    || ib >= local_positions.len()
                    || ic >= local_positions.len()
                    || ia == ib
                    || ib == ic
                    || ia == ic
                {
                    continue;
                }
                triangles.push(QueryTri {
                    a: base + tri[0],
                    b: base + tri[1],
                    c: base + tri[2],
                    surface_index: surface_index as u32,
                });
            }
            GLTF_INDEX_SCRATCH.with(|scratch| {
                flat.clear();
                *scratch.borrow_mut() = flat;
            });
        } else {
            let tri_count = local_positions.len() / 3;
            for i in 0..tri_count {
                let idx = (i * 3) as u32;
                triangles.push(QueryTri {
                    a: base + idx,
                    b: base + idx + 1,
                    c: base + idx + 2,
                    surface_index: surface_index as u32,
                });
            }
        }
        GLTF_POS_SCRATCH.with(|scratch| {
            local_positions.clear();
            *scratch.borrow_mut() = local_positions;
        });
    }

    build_query_mesh_data(vertices, triangles)
}

pub(super) fn split_source_fragment(source: &str) -> (&str, Option<&str>) {
    let Some((path, selector)) = source.rsplit_once(':') else {
        return (source, None);
    };
    if path.is_empty() || selector.contains('/') || selector.contains('\\') {
        return (source, None);
    }
    if selector.contains('[') && selector.ends_with(']') {
        return (path, Some(selector));
    }
    (source, None)
}

pub(super) fn parse_fragment_index(fragment: Option<&str>, key: &str) -> Option<usize> {
    let fragment = fragment?;
    let (name, rest) = fragment.split_once('[')?;
    if name.trim() != key {
        return None;
    }
    let value = rest.strip_suffix(']')?.trim();
    value.parse::<usize>().ok()
}

pub(super) fn normalize_source_slashes(source: &str) -> std::borrow::Cow<'_, str> {
    if source.contains('\\') {
        std::borrow::Cow::Owned(source.replace('\\', "/"))
    } else {
        std::borrow::Cow::Borrowed(source)
    }
}

pub(super) fn normalized_static_mesh_lookup_alias(source: &str) -> Option<String> {
    let (path, fragment) = split_source_fragment(source);
    if !(path.ends_with(".glb") || path.ends_with(".gltf")) {
        return None;
    }
    match parse_fragment_index(fragment, "mesh") {
        Some(0) => Some(path.to_string()),
        Some(_) => None,
        None => Some(format!("{path}:mesh[0]")),
    }
}

pub(super) fn decode_pmesh_query(bytes: &[u8]) -> Option<QueryMeshData> {
    if bytes.len() < 41 || &bytes[0..5] != b"PMESH" {
        return None;
    }
    let version = u32::from_le_bytes(bytes[5..9].try_into().ok()?);
    if version != PMESH_VERSION {
        return None;
    }

    let flags = u32::from_le_bytes(bytes[9..13].try_into().ok()?);
    let vertex_count = u32::from_le_bytes(bytes[13..17].try_into().ok()?) as usize;
    let index_count = u32::from_le_bytes(bytes[17..21].try_into().ok()?) as usize;
    let surface_count = u32::from_le_bytes(bytes[21..25].try_into().ok()?) as usize;
    let meshlet_count = u32::from_le_bytes(bytes[25..29].try_into().ok()?) as usize;
    let lod_count = u32::from_le_bytes(bytes[29..33].try_into().ok()?) as usize;
    let raw_len = u32::from_le_bytes(bytes[33..37].try_into().ok()?) as usize;
    let payload_start = 41usize;

    let raw = decode_pmesh_payload(flags, &bytes[payload_start..])?;
    if raw.len() != raw_len {
        return None;
    }

    let has_normal = (flags & (1 << 0)) != 0;
    let has_uv0 = (flags & (1 << 1)) != 0;
    let has_joints = (flags & (1 << 2)) != 0;
    let has_weights = (flags & (1 << 3)) != 0;
    let weights_unorm8 = (flags & perro_asset_formats::pmesh::FLAG_WEIGHTS_UNORM8) != 0;
    let vertex_stride = 12
        + if has_normal { 12 } else { 0 }
        + if has_uv0 { 8 } else { 0 }
        + if has_joints { 8 } else { 0 }
        + if has_weights {
            if weights_unorm8 { 4 } else { 16 }
        } else {
            0
        };

    let vertex_bytes = vertex_count.checked_mul(vertex_stride)?;
    let index_bytes = index_count.checked_mul(4)?;
    let surface_bytes = surface_count.checked_mul(8)?;
    let meshlet_bytes = meshlet_count.checked_mul(24)?;
    let lod_bytes = lod_count.checked_mul(24)?;
    if raw.len() < vertex_bytes + index_bytes + surface_bytes {
        return None;
    }

    let mut vertices = Vec::with_capacity(vertex_count);
    for i in 0..vertex_count {
        let off = i * vertex_stride;
        let x = f32::from_le_bytes(raw[off..off + 4].try_into().ok()?);
        let y = f32::from_le_bytes(raw[off + 4..off + 8].try_into().ok()?);
        let z = f32::from_le_bytes(raw[off + 8..off + 12].try_into().ok()?);
        vertices.push(Vec3::new(x, y, z));
    }

    let mut indices = Vec::with_capacity(index_count);
    let index_start = vertex_bytes;
    for i in 0..index_count {
        let off = index_start + i * 4;
        indices.push(u32::from_le_bytes(raw[off..off + 4].try_into().ok()?));
    }

    let mut surface_ranges = Vec::with_capacity(surface_count);
    let surface_start = vertex_bytes + index_bytes;
    for i in 0..surface_count {
        let off = surface_start + i * 8;
        let start = u32::from_le_bytes(raw[off..off + 4].try_into().ok()?) as usize;
        let count = u32::from_le_bytes(raw[off + 4..off + 8].try_into().ok()?) as usize;
        surface_ranges.push((start, count));
    }
    if surface_ranges.is_empty() {
        surface_ranges.push((0, indices.len()));
    }
    if lod_count > 0 {
        let lod_start = vertex_bytes
            .checked_add(index_bytes)?
            .checked_add(surface_bytes)?
            .checked_add(meshlet_bytes)?;
        if raw.len() < lod_start.checked_add(lod_bytes)? || lod_start + 16 > raw.len() {
            return None;
        }
        let lod_surface_start =
            u32::from_le_bytes(raw[lod_start + 8..lod_start + 12].try_into().ok()?) as usize;
        let lod_surface_count =
            u32::from_le_bytes(raw[lod_start + 12..lod_start + 16].try_into().ok()?) as usize;
        let lod_surface_end = lod_surface_start
            .saturating_add(lod_surface_count)
            .min(surface_ranges.len());
        if lod_surface_start < lod_surface_end {
            surface_ranges = surface_ranges[lod_surface_start..lod_surface_end].to_vec();
        }
    }

    let mut triangles = Vec::new();
    for (surface_index, (start, count)) in surface_ranges.into_iter().enumerate() {
        let end = start.saturating_add(count).min(indices.len());
        let slice = &indices[start..end];
        for tri in slice.chunks_exact(3) {
            let ia = tri[0] as usize;
            let ib = tri[1] as usize;
            let ic = tri[2] as usize;
            if ia >= vertices.len()
                || ib >= vertices.len()
                || ic >= vertices.len()
                || ia == ib
                || ib == ic
                || ia == ic
            {
                continue;
            }
            triangles.push(QueryTri {
                a: tri[0],
                b: tri[1],
                c: tri[2],
                surface_index: surface_index as u32,
            });
        }
    }

    build_query_mesh_data(vertices, triangles)
}

pub(super) fn decode_pmesh_payload(flags: u32, payload: &[u8]) -> Option<Vec<u8>> {
    if (flags & PMESH_FLAG_PAYLOAD_RAW) != 0 {
        Some(payload.to_vec())
    } else {
        decompress_zlib(payload).ok()
    }
}

#[inline]
pub(super) fn ray_intersect_triangle(
    origin: Vec3,
    direction: Vec3,
    a: Vec3,
    b: Vec3,
    c: Vec3,
) -> Option<f32> {
    let ab = b - a;
    let ac = c - a;
    let pvec = direction.cross(ac);
    let det = ab.dot(pvec);
    if det.abs() <= 0.000001 {
        return None;
    }
    let inv_det = 1.0 / det;

    let tvec = origin - a;
    let u = tvec.dot(pvec) * inv_det;
    if !(0.0..=1.0).contains(&u) {
        return None;
    }

    let qvec = tvec.cross(ab);
    let v = direction.dot(qvec) * inv_det;
    if v < 0.0 || (u + v) > 1.0 {
        return None;
    }

    let t = ac.dot(qvec) * inv_det;
    if t < 0.0 {
        return None;
    }
    Some(t)
}

#[inline]
pub(super) fn closest_point_on_triangle(p: Vec3, a: Vec3, b: Vec3, c: Vec3) -> Vec3 {
    let ab = b - a;
    let ac = c - a;
    let ap = p - a;

    let d1 = ab.dot(ap);
    let d2 = ac.dot(ap);
    if d1 <= 0.0 && d2 <= 0.0 {
        return a;
    }

    let bp = p - b;
    let d3 = ab.dot(bp);
    let d4 = ac.dot(bp);
    if d3 >= 0.0 && d4 <= d3 {
        return b;
    }

    let vc = d1 * d4 - d3 * d2;
    if vc <= 0.0 && d1 >= 0.0 && d3 <= 0.0 {
        let v = d1 / (d1 - d3);
        return a + ab * v;
    }

    let cp = p - c;
    let d5 = ab.dot(cp);
    let d6 = ac.dot(cp);
    if d6 >= 0.0 && d5 <= d6 {
        return c;
    }

    let vb = d5 * d2 - d1 * d6;
    if vb <= 0.0 && d2 >= 0.0 && d6 <= 0.0 {
        let w = d2 / (d2 - d6);
        return a + ac * w;
    }

    let va = d3 * d6 - d5 * d4;
    if va <= 0.0 && (d4 - d3) >= 0.0 && (d5 - d6) >= 0.0 {
        let bc = c - b;
        let w = (d4 - d3) / ((d4 - d3) + (d5 - d6));
        return b + bc * w;
    }

    let denom = 1.0 / (va + vb + vc);
    let v = vb * denom;
    let w = vc * denom;
    a + ab * v + ac * w
}
