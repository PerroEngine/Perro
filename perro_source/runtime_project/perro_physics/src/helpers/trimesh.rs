use super::*;

pub struct TrimeshLoadCtx<'a> {
    pub(crate) provider_mode: PhysicsProviderMode,
    pub(crate) static_mesh_lookup: Option<StaticBytesLookup>,
    pub(crate) static_collision_trimesh_lookup: Option<StaticBytesLookup>,
    pub(crate) trimesh_cache: &'a mut AHashMap<u64, TriMeshData>,
}

pub fn load_trimesh_from_source(
    source: &str,
    scale: [f32; 3],
    ctx: &mut TrimeshLoadCtx<'_>,
) -> Option<TriMeshData> {
    let source = source.trim();
    if source.is_empty() {
        return None;
    }
    let [sx, sy, sz] = scale;

    let cache_key = trimesh_cache_key(source, sx, sy, sz, ctx.provider_mode);
    if let Some(cached) = ctx.trimesh_cache.get(&cache_key) {
        return Some(cached.clone());
    }

    if ctx.provider_mode == PhysicsProviderMode::Static
        && let Some(lookup) = ctx.static_collision_trimesh_lookup
    {
        let source_hash = parse_hashed_source_uri(source).unwrap_or_else(|| string_to_u64(source));
        let bytes = lookup(source_hash);
        if !bytes.is_empty()
            && let Some(decoded) = decode_pmesh_trimesh(bytes, sx, sy, sz)
        {
            let simplified = simplify_trimesh_data(decoded.0, decoded.1)?;
            ctx.trimesh_cache.insert(cache_key, simplified.clone());
            return Some(simplified);
        }

        let normalized = normalize_source_slashes(source);
        if normalized.as_ref() != source {
            let bytes = lookup(string_to_u64(normalized.as_ref()));
            if !bytes.is_empty()
                && let Some(decoded) = decode_pmesh_trimesh(bytes, sx, sy, sz)
            {
                let simplified = simplify_trimesh_data(decoded.0, decoded.1)?;
                ctx.trimesh_cache.insert(cache_key, simplified.clone());
                return Some(simplified);
            }
        }
        if let Some(alias) = normalized_static_mesh_lookup_alias(source) {
            let bytes = lookup(string_to_u64(alias.as_str()));
            if !bytes.is_empty()
                && let Some(decoded) = decode_pmesh_trimesh(bytes, sx, sy, sz)
            {
                let simplified = simplify_trimesh_data(decoded.0, decoded.1)?;
                ctx.trimesh_cache.insert(cache_key, simplified.clone());
                return Some(simplified);
            }
        }
        if normalized.as_ref() != source
            && let Some(alias) = normalized_static_mesh_lookup_alias(normalized.as_ref())
        {
            let bytes = lookup(string_to_u64(alias.as_str()));
            if !bytes.is_empty()
                && let Some(decoded) = decode_pmesh_trimesh(bytes, sx, sy, sz)
            {
                let simplified = simplify_trimesh_data(decoded.0, decoded.1)?;
                ctx.trimesh_cache.insert(cache_key, simplified.clone());
                return Some(simplified);
            }
        }
    }

    if ctx.provider_mode == PhysicsProviderMode::Static
        && let Some(lookup) = ctx.static_mesh_lookup
    {
        let source_hash = parse_hashed_source_uri(source).unwrap_or_else(|| string_to_u64(source));
        let bytes = lookup(source_hash);
        if !bytes.is_empty()
            && let Some(decoded) = decode_pmesh_trimesh(bytes, sx, sy, sz)
        {
            let simplified = simplify_trimesh_data(decoded.0, decoded.1)?;
            ctx.trimesh_cache.insert(cache_key, simplified.clone());
            return Some(simplified);
        }
    }

    let (path, fragment) = split_source_fragment(source);
    let mesh_index = if fragment.is_some() {
        parse_fragment_index(fragment, "mesh")?
    } else {
        0
    };

    let bytes = load_asset(path).ok()?;
    if path.ends_with(".pmesh") {
        let loaded = decode_pmesh_trimesh(&bytes, sx, sy, sz)?;
        let simplified = simplify_trimesh_data(loaded.0, loaded.1)?;
        ctx.trimesh_cache.insert(cache_key, simplified.clone());
        return Some(simplified);
    }
    if path.ends_with(".glb") || path.ends_with(".gltf") {
        let loaded = load_trimesh_from_gltf_bytes(&bytes, mesh_index, sx, sy, sz)?;
        let simplified = simplify_trimesh_data(loaded.0, loaded.1)?;
        ctx.trimesh_cache.insert(cache_key, simplified.clone());
        return Some(simplified);
    }
    None
}

pub fn normalize_source_slashes(source: &str) -> std::borrow::Cow<'_, str> {
    if source.contains('\\') {
        std::borrow::Cow::Owned(source.replace('\\', "/"))
    } else {
        std::borrow::Cow::Borrowed(source)
    }
}

pub fn normalized_static_mesh_lookup_alias(source: &str) -> Option<String> {
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

pub fn decode_pmesh_trimesh(bytes: &[u8], sx: f32, sy: f32, sz: f32) -> Option<TriMeshData> {
    if bytes.len() < 33 || &bytes[0..5] != b"PMESH" {
        return None;
    }
    let version = u32::from_le_bytes(bytes[5..9].try_into().ok()?);
    if version != PMESH_VERSION {
        return None;
    }
    if let Some(render_trimesh) = decode_render_pmesh_trimesh(bytes, sx, sy, sz) {
        return Some(render_trimesh);
    }
    let flags = u32::from_le_bytes(bytes[9..13].try_into().ok()?);
    let vertex_count = u32::from_le_bytes(bytes[13..17].try_into().ok()?) as usize;
    let index_count = u32::from_le_bytes(bytes[17..21].try_into().ok()?) as usize;
    let raw_len = u32::from_le_bytes(bytes[29..33].try_into().ok()?) as usize;
    let payload_start = 33usize;

    let raw = decode_pmesh_payload(flags, &bytes[payload_start..])?;
    if raw.len() != raw_len {
        return None;
    }

    let index_u16 = (flags & PMESH_FLAG_INDEX_U16) != 0;
    let vertex_stride = 12usize;
    let vertex_bytes = vertex_count.checked_mul(vertex_stride)?;
    let index_bytes = index_count.checked_mul(if index_u16 { 2 } else { 4 })?;
    if raw.len() < vertex_bytes + index_bytes {
        return None;
    }

    let mut vertices = Vec::with_capacity(vertex_count);
    for i in 0..vertex_count {
        let off = i * vertex_stride;
        let x = f32::from_le_bytes(raw[off..off + 4].try_into().ok()?);
        let y = f32::from_le_bytes(raw[off + 4..off + 8].try_into().ok()?);
        let z = f32::from_le_bytes(raw[off + 8..off + 12].try_into().ok()?);
        vertices.push(na3::Point3::new(x * sx, y * sy, z * sz));
    }

    let mut triangles = Vec::new();
    let index_start = vertex_bytes;
    for tri_idx in (0..index_count / 3).map(|i| i * 3) {
        let ia = read_trimesh_index(raw.as_slice(), index_start, tri_idx, index_u16)?;
        let ib = read_trimesh_index(raw.as_slice(), index_start, tri_idx + 1, index_u16)?;
        let ic = read_trimesh_index(raw.as_slice(), index_start, tri_idx + 2, index_u16)?;
        let a = ia as usize;
        let b = ib as usize;
        let c = ic as usize;
        if a >= vertices.len()
            || b >= vertices.len()
            || c >= vertices.len()
            || a == b
            || b == c
            || a == c
        {
            continue;
        }
        triangles.push([ia, ib, ic]);
    }

    if vertices.len() < 3 || triangles.is_empty() {
        return None;
    }
    Some((vertices, triangles))
}

pub fn decode_render_pmesh_trimesh(bytes: &[u8], sx: f32, sy: f32, sz: f32) -> Option<TriMeshData> {
    if bytes.len() < 37 {
        return None;
    }
    let flags = u32::from_le_bytes(bytes[9..13].try_into().ok()?);
    let vertex_count = u32::from_le_bytes(bytes[13..17].try_into().ok()?) as usize;
    let index_count = u32::from_le_bytes(bytes[17..21].try_into().ok()?) as usize;
    let surface_count = u32::from_le_bytes(bytes[21..25].try_into().ok()?) as usize;
    let meshlet_count = u32::from_le_bytes(bytes[25..29].try_into().ok()?) as usize;
    let lod_count = u32::from_le_bytes(bytes[29..33].try_into().ok()?) as usize;
    let raw_len = u32::from_le_bytes(bytes[33..37].try_into().ok()?) as usize;
    let raw = decode_pmesh_payload(flags, &bytes[37..])?;
    if raw.len() != raw_len {
        return None;
    }
    let has_normal = (flags & (1 << 0)) != 0;
    let has_uv0 = (flags & (1 << 1)) != 0;
    let has_joints = (flags & (1 << 2)) != 0;
    let has_weights = (flags & (1 << 3)) != 0;
    let stride = 12
        + if has_normal { 12 } else { 0 }
        + if has_uv0 { 8 } else { 0 }
        + if has_joints { 8 } else { 0 }
        + if has_weights { 16 } else { 0 };
    let vertex_bytes = vertex_count.checked_mul(stride)?;
    let index_bytes = index_count.checked_mul(4)?;
    let surface_bytes = surface_count.checked_mul(8)?;
    let meshlet_bytes = meshlet_count.checked_mul(24)?;
    let lod_start = vertex_bytes
        .checked_add(index_bytes)?
        .checked_add(surface_bytes)?
        .checked_add(meshlet_bytes)?;
    if raw.len() < lod_start {
        return None;
    }
    let (lod_index_start, lod_index_count) = if lod_count > 0 && raw.len() >= lod_start + 24 {
        (
            u32::from_le_bytes(raw[lod_start..lod_start + 4].try_into().ok()?) as usize,
            u32::from_le_bytes(raw[lod_start + 4..lod_start + 8].try_into().ok()?) as usize,
        )
    } else {
        (0, index_count)
    };
    let mut vertices = Vec::with_capacity(vertex_count);
    for i in 0..vertex_count {
        let off = i * stride;
        vertices.push(na3::Point3::new(
            f32::from_le_bytes(raw[off..off + 4].try_into().ok()?) * sx,
            f32::from_le_bytes(raw[off + 4..off + 8].try_into().ok()?) * sy,
            f32::from_le_bytes(raw[off + 8..off + 12].try_into().ok()?) * sz,
        ));
    }
    let index_start = vertex_bytes + lod_index_start.saturating_mul(4);
    let index_end = index_start
        .saturating_add(lod_index_count.saturating_mul(4))
        .min(vertex_bytes + index_bytes);
    let mut triangles = Vec::new();
    for off in (index_start..index_end).step_by(12) {
        if off + 12 > raw.len() {
            break;
        }
        let ia = u32::from_le_bytes(raw[off..off + 4].try_into().ok()?);
        let ib = u32::from_le_bytes(raw[off + 4..off + 8].try_into().ok()?);
        let ic = u32::from_le_bytes(raw[off + 8..off + 12].try_into().ok()?);
        let a = ia as usize;
        let b = ib as usize;
        let c = ic as usize;
        if a < vertices.len()
            && b < vertices.len()
            && c < vertices.len()
            && a != b
            && b != c
            && a != c
        {
            triangles.push([ia, ib, ic]);
        }
    }
    if vertices.len() < 3 || triangles.is_empty() {
        return None;
    }
    Some((vertices, triangles))
}

pub fn decode_pmesh_payload(flags: u32, payload: &[u8]) -> Option<Vec<u8>> {
    if (flags & PMESH_FLAG_PAYLOAD_RAW) != 0 {
        Some(payload.to_vec())
    } else {
        decompress_zlib(payload).ok()
    }
}

pub fn read_trimesh_index(
    raw: &[u8],
    index_start: usize,
    index: usize,
    index_u16: bool,
) -> Option<u32> {
    if index_u16 {
        let off = index_start + index * 2;
        Some(u16::from_le_bytes(raw[off..off + 2].try_into().ok()?) as u32)
    } else {
        let off = index_start + index * 4;
        Some(u32::from_le_bytes(raw[off..off + 4].try_into().ok()?))
    }
}

pub fn load_trimesh_from_gltf_bytes(
    bytes: &[u8],
    mesh_index: usize,
    sx: f32,
    sy: f32,
    sz: f32,
) -> Option<TriMeshData> {
    let (doc, buffers, _images) = gltf::import_slice(bytes).ok()?;
    let mesh = doc.meshes().nth(mesh_index)?;

    let mut vertices = Vec::<na3::Point3<f32>>::new();
    let mut triangles = Vec::<[u32; 3]>::new();

    for primitive in mesh.primitives() {
        let reader = primitive.reader(|buffer| buffers.get(buffer.index()).map(|d| d.0.as_slice()));
        let Some(pos_iter) = reader.read_positions() else {
            continue;
        };

        let local_positions: Vec<[f32; 3]> = pos_iter.collect();
        if local_positions.len() < 3 {
            continue;
        }

        let Ok(base) = u32::try_from(vertices.len()) else {
            return None;
        };
        for p in &local_positions {
            vertices.push(na3::Point3::new(p[0] * sx, p[1] * sy, p[2] * sz));
        }

        if let Some(indices_reader) = reader.read_indices() {
            let mut flat: Vec<u32> = indices_reader.into_u32().collect();
            let tri_len = flat.len() / 3 * 3;
            flat.truncate(tri_len);
            for tri in flat.chunks_exact(3) {
                let ia = tri[0] as usize;
                let ib = tri[1] as usize;
                let ic = tri[2] as usize;
                if ia >= local_positions.len()
                    || ib >= local_positions.len()
                    || ic >= local_positions.len()
                {
                    continue;
                }
                let a = base + tri[0];
                let b = base + tri[1];
                let c = base + tri[2];
                if a != b && b != c && a != c {
                    triangles.push([a, b, c]);
                }
            }
        } else {
            for i in (0..local_positions.len() / 3 * 3).step_by(3) {
                let a = base + i as u32;
                let b = base + i as u32 + 1;
                let c = base + i as u32 + 2;
                triangles.push([a, b, c]);
            }
        }
    }

    if vertices.len() < 3 || triangles.is_empty() {
        return None;
    }
    Some((vertices, triangles))
}

pub fn trimesh_cache_key(
    source: &str,
    sx: f32,
    sy: f32,
    sz: f32,
    provider_mode: PhysicsProviderMode,
) -> u64 {
    string_to_u64(&format!(
        "{source}|{:08x}|{:08x}|{:08x}|{}",
        sx.to_bits(),
        sy.to_bits(),
        sz.to_bits(),
        provider_mode as u8
    ))
}
