use bytemuck::{Pod, Zeroable};
use perro_asset_formats::pmesh::{
    FLAG_HAS_BLEND_SHAPE_NORMALS as PMESH_FLAG_HAS_BLEND_SHAPE_NORMALS,
    FLAG_HAS_JOINTS as PMESH_FLAG_HAS_JOINTS, FLAG_HAS_NORMAL as PMESH_FLAG_HAS_NORMAL,
    FLAG_HAS_UV0 as PMESH_FLAG_HAS_UV0, FLAG_HAS_UV1 as PMESH_FLAG_HAS_UV1,
    FLAG_HAS_WEIGHTS as PMESH_FLAG_HAS_WEIGHTS, FLAG_PAYLOAD_RAW as PMESH_FLAG_PAYLOAD_RAW,
    FLAG_WEIGHTS_UNORM8 as PMESH_FLAG_WEIGHTS_UNORM8, MAGIC as PMESH_MAGIC,
    VERSION as PMESH_VERSION, VERSION_V2 as PMESH_VERSION_V2,
};
use perro_io::{decompress_zlib, load_asset};
use perro_meshlets::{
    DEFAULT_LOD_TARGET_RATIOS, LodSurfaceRange, LodVertex, pack_meshlets_from_positions,
};
use perro_render_bridge::{
    Mesh3D, MeshSurfaceRange, RuntimeMeshBlendShape, RuntimeMeshBlendShapeVertex, RuntimeMeshData,
    RuntimeMeshVertex,
};
use perro_structs::UnitVector4;
use std::{borrow::Cow, collections::BTreeMap};

pub type StaticMeshBytesLookup = fn(path_hash: u64) -> &'static [u8];

const MESHLET_TRIANGLES: usize = 64;

#[repr(C)]
#[derive(Clone, Copy, Pod, Zeroable)]
pub struct MeshVertex {
    pub pos: [f32; 3],
    pub normal: [f32; 3],
    pub uv: [f32; 2],
    pub paint_uv: [f32; 2],
    pub joints: [u16; 4],
    pub weights: UnitVector4,
}

#[derive(Clone, Copy)]
pub struct MeshRange {
    pub index_start: u32,
    pub index_count: u32,
    pub base_vertex: i32,
}

pub struct DecodedMesh {
    pub vertices: Vec<MeshVertex>,
    pub indices: Vec<u32>,
    pub surface_ranges: Vec<MeshRange>,
    pub blend_shapes: Vec<MeshBlendShape>,
    pub meshlets: Vec<DecodedMeshlet>,
    pub lods: Vec<DecodedLod>,
    pub has_skinning: bool,
}

#[derive(Clone, Copy)]
pub struct MeshBlendShapeVertex {
    pub position_delta: [f32; 3],
    pub normal_delta: [f32; 3],
}

#[derive(Clone)]
pub struct MeshBlendShape {
    pub vertices: Vec<MeshBlendShapeVertex>,
    pub has_normal_deltas: bool,
}

#[derive(Clone, Copy)]
pub struct DecodedMeshlet {
    pub index_start: u32,
    pub index_count: u32,
    pub center: [f32; 3],
    pub radius: f32,
}

#[derive(Clone)]
pub struct DecodedLod {
    pub index_start: u32,
    pub index_count: u32,
    pub surface_start: u32,
    pub surface_count: u32,
    pub meshlet_start: u32,
    pub meshlet_count: u32,
}

pub fn load_mesh_from_source(
    source: &str,
    static_mesh_lookup: Option<StaticMeshBytesLookup>,
    runtime_mesh: Option<&RuntimeMeshData>,
    dev_meshlets: bool,
) -> Option<DecodedMesh> {
    let _ = dev_meshlets;
    load_mesh_from_source_inner(source, static_mesh_lookup, runtime_mesh, false, false)
}

pub fn load_mesh_from_source_no_dynamic_lods(
    source: &str,
    static_mesh_lookup: Option<StaticMeshBytesLookup>,
    runtime_mesh: Option<&RuntimeMeshData>,
) -> Option<DecodedMesh> {
    load_mesh_from_source_inner(source, static_mesh_lookup, runtime_mesh, false, false)
}

fn load_mesh_from_source_inner(
    source: &str,
    static_mesh_lookup: Option<StaticMeshBytesLookup>,
    runtime_mesh: Option<&RuntimeMeshData>,
    dev_meshlets: bool,
    build_lods: bool,
) -> Option<DecodedMesh> {
    let mut decoded = if let Some(mesh) = runtime_mesh {
        decode_runtime_mesh(mesh)?
    } else if let Some(lookup) = static_mesh_lookup {
        let normalized = normalize_source_slashes(source);
        let source_variants = if normalized.as_ref() == source {
            [source, source]
        } else {
            [source, normalized.as_ref()]
        };
        let mut static_decoded = None;
        let mut try_hash = |hash: u64| {
            if static_decoded.is_some() {
                return;
            }
            let bytes = lookup(hash);
            if bytes.is_empty() {
                return;
            }
            static_decoded = decode_pmesh(bytes);
        };

        try_hash(
            perro_ids::parse_hashed_source_uri(source)
                .unwrap_or_else(|| perro_ids::string_to_u64(source)),
        );
        if source_variants[1] != source_variants[0] {
            try_hash(
                perro_ids::parse_hashed_source_uri(source_variants[1])
                    .unwrap_or_else(|| perro_ids::string_to_u64(source_variants[1])),
            );
        }
        if let Some(alias) = normalized_static_mesh_lookup_alias(source) {
            try_hash(perro_ids::string_to_u64(alias.as_str()));
        }
        if source_variants[1] != source_variants[0]
            && let Some(alias) = normalized_static_mesh_lookup_alias(source_variants[1])
        {
            try_hash(perro_ids::string_to_u64(alias.as_str()));
        }

        if let Some(decoded) = static_decoded {
            decoded
        } else {
            load_mesh_from_asset_source(source)?
        }
    } else {
        load_mesh_from_asset_source(source)?
    };

    if decoded.has_skinning {
        decoded.lods.clear();
    } else if build_lods && decoded.lods.is_empty() {
        decoded = build_dynamic_lods(decoded, dev_meshlets);
    } else if build_lods && decoded.meshlets.is_empty() && dev_meshlets {
        let (packed_indices, meshlets) = build_meshlets(&decoded.vertices, &decoded.indices);
        decoded.indices = packed_indices;
        decoded.meshlets = meshlets;
        decoded.lods = vec![DecodedLod {
            index_start: 0,
            index_count: decoded.indices.len() as u32,
            surface_start: 0,
            surface_count: decoded.surface_ranges.len() as u32,
            meshlet_start: 0,
            meshlet_count: decoded.meshlets.len() as u32,
        }];
    }

    Some(decoded)
}

pub fn validate_mesh_source(
    source: &str,
    static_mesh_lookup: Option<StaticMeshBytesLookup>,
) -> Result<(), String> {
    if source.starts_with("__") {
        return Ok(());
    }
    if load_mesh_from_source_inner(source, static_mesh_lookup, None, false, false).is_some() {
        return Ok(());
    }
    Err(format!("mesh source failed to decode: {}", source))
}

pub fn load_mesh3d_from_source(
    source: &str,
    static_mesh_lookup: Option<StaticMeshBytesLookup>,
) -> Option<Mesh3D> {
    let decoded = load_mesh_from_source_inner(source, static_mesh_lookup, None, false, false)?;
    Some(mesh3d_from_decoded(decoded))
}

pub fn load_mesh3d_from_bytes(bytes: &[u8]) -> Option<Mesh3D> {
    let decoded = if bytes.starts_with(PMESH_MAGIC) {
        decode_pmesh(bytes)
    } else {
        decode_gltf_mesh(bytes, 0)
    }?;
    Some(mesh3d_from_decoded(decoded))
}

fn mesh3d_from_decoded(decoded: DecodedMesh) -> Mesh3D {
    Mesh3D {
        vertices: decoded
            .vertices
            .into_iter()
            .map(|v| RuntimeMeshVertex {
                position: v.pos,
                normal: v.normal,
                uv: v.uv,
                paint_uv: v.paint_uv,
                joints: v.joints,
                weights: v.weights,
            })
            .collect(),
        indices: decoded.indices,
        surface_ranges: decoded
            .surface_ranges
            .into_iter()
            .map(|range| MeshSurfaceRange {
                index_start: range.index_start,
                index_count: range.index_count,
            })
            .collect(),
        blend_shapes: decoded
            .blend_shapes
            .into_iter()
            .map(|shape| RuntimeMeshBlendShape {
                vertices: shape
                    .vertices
                    .into_iter()
                    .map(|v| RuntimeMeshBlendShapeVertex {
                        position_delta: v.position_delta,
                        normal_delta: v.normal_delta,
                    })
                    .collect(),
                has_normal_deltas: shape.has_normal_deltas,
            })
            .collect(),
    }
}

fn load_mesh_from_asset_source(source: &str) -> Option<DecodedMesh> {
    let (path, fragment) = split_source_fragment(source);
    if path.ends_with(".pmesh") {
        let bytes = load_asset(path).ok()?;
        decode_pmesh(&bytes)
    } else if path.ends_with(".glb") || path.ends_with(".gltf") {
        let mesh_index = parse_fragment_index(fragment, "mesh").unwrap_or(0);
        let bytes = load_asset(path).ok()?;
        decode_gltf_mesh(&bytes, mesh_index as usize)
    } else {
        None
    }
}

fn normalize_source_slashes(source: &str) -> Cow<'_, str> {
    if source.contains('\\') {
        Cow::Owned(source.replace('\\', "/"))
    } else {
        Cow::Borrowed(source)
    }
}

fn normalized_static_mesh_lookup_alias(source: &str) -> Option<String> {
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

fn decode_runtime_mesh(mesh: &RuntimeMeshData) -> Option<DecodedMesh> {
    if mesh.vertices.is_empty() || mesh.indices.is_empty() {
        return None;
    }
    if !mesh.indices.len().is_multiple_of(3) {
        return None;
    }
    let vertices: Vec<MeshVertex> = mesh
        .vertices
        .iter()
        .map(|v| MeshVertex {
            pos: v.position,
            normal: v.normal,
            uv: v.uv,
            paint_uv: v.paint_uv,
            joints: v.joints,
            weights: v.weights,
        })
        .collect();
    if vertices
        .iter()
        .any(|v| !v.pos.iter().all(|c| c.is_finite()))
    {
        return None;
    }
    if vertices
        .iter()
        .any(|v| !v.normal.iter().all(|c| c.is_finite()))
    {
        return None;
    }
    if vertices.iter().any(|v| !v.uv.iter().all(|c| c.is_finite())) {
        return None;
    }
    if vertices
        .iter()
        .any(|v| !v.paint_uv.iter().all(|c| c.is_finite()))
    {
        return None;
    }
    if mesh
        .indices
        .iter()
        .any(|&idx| (idx as usize) >= vertices.len())
    {
        return None;
    }
    let mut surface_ranges = Vec::new();
    if mesh.surface_ranges.is_empty() {
        surface_ranges.push(MeshRange {
            index_start: 0,
            index_count: mesh.indices.len() as u32,
            base_vertex: 0,
        });
    } else {
        for range in &mesh.surface_ranges {
            let end = range.index_start.checked_add(range.index_count)?;
            if end > mesh.indices.len() as u32 {
                return None;
            }
            if !range.index_count.is_multiple_of(3) {
                return None;
            }
            surface_ranges.push(MeshRange {
                index_start: range.index_start,
                index_count: range.index_count,
                base_vertex: 0,
            });
        }
    }
    Some(DecodedMesh {
        vertices,
        indices: mesh.indices.clone(),
        surface_ranges,
        blend_shapes: mesh
            .blend_shapes
            .iter()
            .filter(|shape| shape.vertices.len() == mesh.vertices.len())
            .map(|shape| MeshBlendShape {
                vertices: shape
                    .vertices
                    .iter()
                    .map(|v| MeshBlendShapeVertex {
                        position_delta: v.position_delta,
                        normal_delta: v.normal_delta,
                    })
                    .collect(),
                has_normal_deltas: shape.has_normal_deltas,
            })
            .collect(),
        meshlets: Vec::new(),
        lods: Vec::new(),
        has_skinning: mesh_vertices_have_skinning(&mesh.vertices),
    })
}

fn build_dynamic_lods(mut decoded: DecodedMesh, build_meshlets_for_lods: bool) -> DecodedMesh {
    let (vertices, indices) = dedup_mesh_vertices(decoded.vertices, decoded.indices);
    decoded.vertices = vertices;
    let base_surfaces = if decoded.surface_ranges.is_empty() {
        vec![MeshRange {
            index_start: 0,
            index_count: indices.len() as u32,
            base_vertex: 0,
        }]
    } else {
        decoded.surface_ranges
    };
    let lod_inputs = build_decoded_lod_sets(&decoded.vertices, &indices, &base_surfaces);
    let mut all_indices = Vec::new();
    let mut all_surfaces = Vec::new();
    let mut all_meshlets = Vec::new();
    let mut lods = Vec::new();
    for input in lod_inputs {
        let index_start = all_indices.len() as u32;
        let surface_start = all_surfaces.len() as u32;
        let meshlet_start = all_meshlets.len() as u32;
        let (packed_indices, packed_surfaces, mut packed_meshlets) = if build_meshlets_for_lods {
            pack_decoded_meshlets_with_surfaces(
                &decoded.vertices,
                &input.indices,
                &input.surface_ranges,
            )
        } else {
            (input.indices, input.surface_ranges, Vec::new())
        };
        let packed_index_start = all_indices.len() as u32;
        all_indices.extend(packed_indices);
        for mut surface in packed_surfaces {
            surface.index_start += packed_index_start;
            all_surfaces.push(surface);
        }
        for meshlet in &mut packed_meshlets {
            meshlet.index_start += packed_index_start;
        }
        let meshlet_count = packed_meshlets.len() as u32;
        all_meshlets.extend(packed_meshlets);
        lods.push(DecodedLod {
            index_start,
            index_count: (all_indices.len() as u32).saturating_sub(index_start),
            surface_start,
            surface_count: (all_surfaces.len() as u32).saturating_sub(surface_start),
            meshlet_start,
            meshlet_count,
        });
    }
    decoded.indices = all_indices;
    decoded.surface_ranges = all_surfaces;
    decoded.meshlets = all_meshlets;
    decoded.lods = lods;
    decoded
}

fn mesh_vertices_have_skinning(vertices: &[RuntimeMeshVertex]) -> bool {
    vertices.iter().any(|vertex| {
        vertex.joints.iter().any(|&joint| joint != 0)
            || vertex.weights != perro_structs::UnitVector4::new([1.0, 0.0, 0.0, 0.0])
    })
}

#[derive(Clone)]
struct DecodedLodInput {
    indices: Vec<u32>,
    surface_ranges: Vec<MeshRange>,
}

fn build_decoded_lod_sets(
    vertices: &[MeshVertex],
    indices: &[u32],
    surface_ranges: &[MeshRange],
) -> Vec<DecodedLodInput> {
    let lod_vertices = vertices
        .iter()
        .map(|vertex| LodVertex {
            position: vertex.pos,
            normal: vertex.normal,
            uv: vertex.uv,
        })
        .collect::<Vec<_>>();
    let lod_surfaces = surface_ranges
        .iter()
        .map(|range| LodSurfaceRange {
            index_start: range.index_start,
            index_count: range.index_count,
        })
        .collect::<Vec<_>>();
    perro_meshlets::build_lod_sets(
        &lod_vertices,
        indices,
        &lod_surfaces,
        &DEFAULT_LOD_TARGET_RATIOS,
    )
    .into_iter()
    .map(|lod| DecodedLodInput {
        indices: lod.indices,
        surface_ranges: lod
            .surface_ranges
            .into_iter()
            .map(|range| MeshRange {
                index_start: range.index_start,
                index_count: range.index_count,
                base_vertex: 0,
            })
            .collect(),
    })
    .collect()
}

fn pack_decoded_meshlets_with_surfaces(
    vertices: &[MeshVertex],
    indices: &[u32],
    surface_ranges: &[MeshRange],
) -> (Vec<u32>, Vec<MeshRange>, Vec<DecodedMeshlet>) {
    let mut packed_indices = Vec::new();
    let mut packed_ranges = Vec::new();
    let mut packed_meshlets = Vec::new();
    for surface in surface_ranges {
        let start = surface.index_start as usize;
        let end = start
            .saturating_add(surface.index_count as usize)
            .min(indices.len());
        let surface_indices = &indices[start..end];
        if surface_indices.is_empty() {
            continue;
        }
        let surface_start = packed_indices.len() as u32;
        let (mut surface_packed, mut surface_meshlets) = build_meshlets(vertices, surface_indices);
        for meshlet in &mut surface_meshlets {
            meshlet.index_start += surface_start;
        }
        packed_indices.append(&mut surface_packed);
        let surface_count = (packed_indices.len() as u32).saturating_sub(surface_start);
        packed_ranges.push(MeshRange {
            index_start: surface_start,
            index_count: surface_count,
            base_vertex: 0,
        });
        packed_meshlets.append(&mut surface_meshlets);
    }
    if packed_indices.is_empty() {
        let (packed, meshlets) = build_meshlets(vertices, indices);
        (
            packed,
            vec![MeshRange {
                index_start: 0,
                index_count: indices.len() as u32,
                base_vertex: 0,
            }],
            meshlets,
        )
    } else {
        (packed_indices, packed_ranges, packed_meshlets)
    }
}

fn dedup_mesh_vertices(
    vertices: Vec<MeshVertex>,
    indices: Vec<u32>,
) -> (Vec<MeshVertex>, Vec<u32>) {
    let mut out_vertices = Vec::<MeshVertex>::new();
    let mut out_indices = Vec::<u32>::with_capacity(indices.len());
    let mut map = BTreeMap::<MeshVertexKey, u32>::new();
    for idx in indices {
        let Some(vertex) = vertices.get(idx as usize).copied() else {
            continue;
        };
        let key = MeshVertexKey::from(vertex);
        let out_idx = if let Some(&existing) = map.get(&key) {
            existing
        } else {
            let next = out_vertices.len() as u32;
            out_vertices.push(vertex);
            map.insert(key, next);
            next
        };
        out_indices.push(out_idx);
    }
    if out_vertices.is_empty() {
        (vertices, out_indices)
    } else {
        (out_vertices, out_indices)
    }
}

#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
struct MeshVertexKey {
    pos: [u32; 3],
    normal: [u32; 3],
    uv: [u32; 2],
    paint_uv: [u32; 2],
    joints: [u16; 4],
    weights: [u8; 4],
}

impl From<MeshVertex> for MeshVertexKey {
    fn from(vertex: MeshVertex) -> Self {
        Self {
            pos: vertex.pos.map(f32::to_bits),
            normal: vertex.normal.map(f32::to_bits),
            uv: vertex.uv.map(f32::to_bits),
            paint_uv: vertex.paint_uv.map(f32::to_bits),
            joints: vertex.joints,
            weights: vertex.weights.to_u8(),
        }
    }
}

fn split_source_fragment(source: &str) -> (&str, Option<&str>) {
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

fn parse_fragment_index(fragment: Option<&str>, key: &str) -> Option<u32> {
    let fragment = fragment?;
    let (name, rest) = fragment.split_once('[')?;
    if name.trim() != key {
        return None;
    }
    let value = rest.strip_suffix(']')?.trim();
    value.parse::<u32>().ok()
}

fn build_meshlets(vertices: &[MeshVertex], indices: &[u32]) -> (Vec<u32>, Vec<DecodedMeshlet>) {
    let positions: Vec<[f32; 3]> = vertices.iter().map(|v| v.pos).collect();
    let packed = pack_meshlets_from_positions(&positions, indices, MESHLET_TRIANGLES);
    let meshlets = packed
        .meshlets
        .into_iter()
        .map(|m| DecodedMeshlet {
            index_start: m.index_start,
            index_count: m.index_count,
            center: m.center,
            radius: m.radius,
        })
        .collect();
    (packed.packed_indices, meshlets)
}

pub fn decode_pmesh(bytes: &[u8]) -> Option<DecodedMesh> {
    if bytes.len() < 37 || &bytes[0..5] != PMESH_MAGIC {
        return None;
    }
    let version = u32::from_le_bytes(bytes[5..9].try_into().ok()?);
    if version != PMESH_VERSION && version != PMESH_VERSION_V2 {
        return None;
    }
    let header_len = 41;
    if bytes.len() < header_len {
        return None;
    }
    let flags = u32::from_le_bytes(bytes[9..13].try_into().ok()?);
    let vertex_count = u32::from_le_bytes(bytes[13..17].try_into().ok()?) as usize;
    let index_count = u32::from_le_bytes(bytes[17..21].try_into().ok()?) as usize;
    let surface_count = u32::from_le_bytes(bytes[21..25].try_into().ok()?) as usize;
    let meshlet_count = u32::from_le_bytes(bytes[25..29].try_into().ok()?) as usize;
    let lod_count = u32::from_le_bytes(bytes[29..33].try_into().ok()?) as usize;
    let raw_len = u32::from_le_bytes(bytes[33..37].try_into().ok()?) as usize;
    let blend_shape_count = u32::from_le_bytes(bytes[37..41].try_into().ok()?) as usize;
    let raw = decode_static_payload(flags, &bytes[header_len..])?;
    if raw.len() != raw_len {
        return None;
    }

    let has_normal = (flags & PMESH_FLAG_HAS_NORMAL) != 0;
    let has_uv0 = (flags & PMESH_FLAG_HAS_UV0) != 0;
    let has_uv1 = (flags & PMESH_FLAG_HAS_UV1) != 0;
    let has_joints = (flags & PMESH_FLAG_HAS_JOINTS) != 0;
    let has_weights = (flags & PMESH_FLAG_HAS_WEIGHTS) != 0;
    let weights_unorm8 = (flags & PMESH_FLAG_WEIGHTS_UNORM8) != 0;
    let has_blend_shape_normals = (flags & PMESH_FLAG_HAS_BLEND_SHAPE_NORMALS) != 0;
    let vertex_stride = 12
        + if has_normal { 12 } else { 0 }
        + if has_uv0 { 8 } else { 0 }
        + if has_uv1 { 8 } else { 0 }
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
    let blend_shape_stride = 12 + if has_blend_shape_normals { 12 } else { 0 };
    let blend_shape_bytes = blend_shape_count
        .checked_mul(vertex_count)?
        .checked_mul(blend_shape_stride)?;
    let required = vertex_bytes
        .checked_add(index_bytes)?
        .checked_add(surface_bytes)?
        .checked_add(meshlet_bytes)?
        .checked_add(lod_bytes)?
        .checked_add(blend_shape_bytes)?;
    if raw.len() < required {
        return None;
    }

    let mut vertices = Vec::with_capacity(vertex_count);
    for i in 0..vertex_count {
        let off = i * vertex_stride;
        let mut cursor = off;
        let pos = [
            f32::from_le_bytes(raw[cursor..cursor + 4].try_into().ok()?),
            f32::from_le_bytes(raw[cursor + 4..cursor + 8].try_into().ok()?),
            f32::from_le_bytes(raw[cursor + 8..cursor + 12].try_into().ok()?),
        ];
        cursor += 12;
        let normal = if has_normal {
            let out = [
                f32::from_le_bytes(raw[cursor..cursor + 4].try_into().ok()?),
                f32::from_le_bytes(raw[cursor + 4..cursor + 8].try_into().ok()?),
                f32::from_le_bytes(raw[cursor + 8..cursor + 12].try_into().ok()?),
            ];
            cursor += 12;
            out
        } else {
            [0.0, 1.0, 0.0]
        };
        let uv = if has_uv0 {
            let out = [
                f32::from_le_bytes(raw[cursor..cursor + 4].try_into().ok()?),
                f32::from_le_bytes(raw[cursor + 4..cursor + 8].try_into().ok()?),
            ];
            cursor += 8;
            out
        } else {
            [0.0, 0.0]
        };
        let paint_uv = if has_uv1 {
            let out = [
                f32::from_le_bytes(raw[cursor..cursor + 4].try_into().ok()?),
                f32::from_le_bytes(raw[cursor + 4..cursor + 8].try_into().ok()?),
            ];
            cursor += 8;
            out
        } else {
            uv
        };
        let joints = if has_joints {
            let out = [
                u16::from_le_bytes(raw[cursor..cursor + 2].try_into().ok()?),
                u16::from_le_bytes(raw[cursor + 2..cursor + 4].try_into().ok()?),
                u16::from_le_bytes(raw[cursor + 4..cursor + 6].try_into().ok()?),
                u16::from_le_bytes(raw[cursor + 6..cursor + 8].try_into().ok()?),
            ];
            cursor += 8;
            out
        } else {
            [0, 0, 0, 0]
        };
        let weights = if has_weights {
            if weights_unorm8 {
                let bytes: [u8; 4] = raw[cursor..cursor + 4].try_into().ok()?;
                UnitVector4::from_u8(bytes)
            } else {
                let weights = [
                    f32::from_le_bytes(raw[cursor..cursor + 4].try_into().ok()?),
                    f32::from_le_bytes(raw[cursor + 4..cursor + 8].try_into().ok()?),
                    f32::from_le_bytes(raw[cursor + 8..cursor + 12].try_into().ok()?),
                    f32::from_le_bytes(raw[cursor + 12..cursor + 16].try_into().ok()?),
                ];
                quantize_skin_weights(weights)
            }
        } else {
            UnitVector4::from_u8([255, 0, 0, 0])
        };
        vertices.push(MeshVertex {
            pos,
            normal,
            uv,
            paint_uv,
            joints,
            weights,
        });
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
        surface_ranges.push(MeshRange {
            index_start: u32::from_le_bytes(raw[off..off + 4].try_into().ok()?),
            index_count: u32::from_le_bytes(raw[off + 4..off + 8].try_into().ok()?),
            base_vertex: 0,
        });
    }
    let mut meshlets = Vec::with_capacity(meshlet_count);
    let meshlet_start = vertex_bytes + index_bytes + surface_bytes;
    for i in 0..meshlet_count {
        let off = meshlet_start + i * 24;
        meshlets.push(DecodedMeshlet {
            index_start: u32::from_le_bytes(raw[off..off + 4].try_into().ok()?),
            index_count: u32::from_le_bytes(raw[off + 4..off + 8].try_into().ok()?),
            center: [
                f32::from_le_bytes(raw[off + 8..off + 12].try_into().ok()?),
                f32::from_le_bytes(raw[off + 12..off + 16].try_into().ok()?),
                f32::from_le_bytes(raw[off + 16..off + 20].try_into().ok()?),
            ],
            radius: f32::from_le_bytes(raw[off + 20..off + 24].try_into().ok()?),
        });
    }
    let lod_start = vertex_bytes + index_bytes + surface_bytes + meshlet_bytes;
    let mut lods = Vec::with_capacity(lod_count);
    for i in 0..lod_count {
        let off = lod_start + i * 24;
        let lod = DecodedLod {
            index_start: u32::from_le_bytes(raw[off..off + 4].try_into().ok()?),
            index_count: u32::from_le_bytes(raw[off + 4..off + 8].try_into().ok()?),
            surface_start: u32::from_le_bytes(raw[off + 8..off + 12].try_into().ok()?),
            surface_count: u32::from_le_bytes(raw[off + 12..off + 16].try_into().ok()?),
            meshlet_start: u32::from_le_bytes(raw[off + 16..off + 20].try_into().ok()?),
            meshlet_count: u32::from_le_bytes(raw[off + 20..off + 24].try_into().ok()?),
        };
        if (lod.index_start as usize).saturating_add(lod.index_count as usize) <= index_count
            && (lod.surface_start as usize).saturating_add(lod.surface_count as usize)
                <= surface_count
            && (lod.meshlet_start as usize).saturating_add(lod.meshlet_count as usize)
                <= meshlet_count
        {
            lods.push(lod);
        }
    }
    let blend_shape_start = vertex_bytes + index_bytes + surface_bytes + meshlet_bytes + lod_bytes;
    let mut blend_shapes = Vec::with_capacity(blend_shape_count);
    for shape_idx in 0..blend_shape_count {
        let shape_start = blend_shape_start + shape_idx * vertex_count * blend_shape_stride;
        let mut shape_vertices = Vec::with_capacity(vertex_count);
        for vertex_idx in 0..vertex_count {
            let mut cursor = shape_start + vertex_idx * blend_shape_stride;
            let position_delta = [
                f32::from_le_bytes(raw[cursor..cursor + 4].try_into().ok()?),
                f32::from_le_bytes(raw[cursor + 4..cursor + 8].try_into().ok()?),
                f32::from_le_bytes(raw[cursor + 8..cursor + 12].try_into().ok()?),
            ];
            cursor += 12;
            let normal_delta = if has_blend_shape_normals {
                [
                    f32::from_le_bytes(raw[cursor..cursor + 4].try_into().ok()?),
                    f32::from_le_bytes(raw[cursor + 4..cursor + 8].try_into().ok()?),
                    f32::from_le_bytes(raw[cursor + 8..cursor + 12].try_into().ok()?),
                ]
            } else {
                [0.0; 3]
            };
            shape_vertices.push(MeshBlendShapeVertex {
                position_delta,
                normal_delta,
            });
        }
        blend_shapes.push(MeshBlendShape {
            vertices: shape_vertices,
            has_normal_deltas: has_blend_shape_normals,
        });
    }
    if lods.is_empty() {
        lods.push(DecodedLod {
            index_start: 0,
            index_count: index_count as u32,
            surface_start: 0,
            surface_count: surface_count as u32,
            meshlet_start: 0,
            meshlet_count: meshlet_count as u32,
        });
    }

    Some(DecodedMesh {
        vertices,
        indices,
        surface_ranges,
        blend_shapes,
        meshlets,
        lods,
        has_skinning: has_joints || has_weights,
    })
}

pub fn decode_gltf_mesh(bytes: &[u8], mesh_index: usize) -> Option<DecodedMesh> {
    let (doc, buffers, _images) = gltf::import_slice(bytes).ok()?;
    let mesh = doc.meshes().nth(mesh_index)?;
    let mut vertices = Vec::new();
    let mut indices = Vec::new();
    let mut surface_ranges = Vec::new();
    let mut blend_shapes: Vec<MeshBlendShape> = Vec::new();
    let mut has_skinning = false;

    for primitive in mesh.primitives() {
        let reader = primitive.reader(|buffer| buffers.get(buffer.index()).map(|b| b.0.as_slice()));
        let Some(positions_iter) = reader.read_positions() else {
            continue;
        };
        let positions: Vec<[f32; 3]> = positions_iter.collect();
        if positions.is_empty() {
            continue;
        }
        let normals: Vec<[f32; 3]> = reader
            .read_normals()
            .map(|iter| iter.collect())
            .unwrap_or_default();
        let tex_coords: Vec<[f32; 2]> = reader
            .read_tex_coords(0)
            .map(|iter| iter.into_f32().collect())
            .unwrap_or_default();
        let paint_tex_coords: Vec<[f32; 2]> = reader
            .read_tex_coords(1)
            .map(|iter| iter.into_f32().collect())
            .unwrap_or_default();
        let joints: Vec<[u16; 4]> = reader
            .read_joints(0)
            .map(|iter| iter.into_u16().collect())
            .unwrap_or_default();
        if !joints.is_empty() {
            has_skinning = true;
        }
        let mut weights: Vec<[f32; 4]> = reader
            .read_weights(0)
            .map(|iter| iter.into_f32().collect())
            .unwrap_or_default();
        if !weights.is_empty() {
            has_skinning = true;
        }
        if weights.is_empty() && !joints.is_empty() {
            weights = vec![[1.0, 0.0, 0.0, 0.0]; joints.len()];
        }
        let primitive_vertex_count = positions.len();
        let primitive_blend_shapes = reader
            .read_morph_targets()
            .map(|(positions, normals, _tangents)| {
                let position_deltas: Vec<[f32; 3]> =
                    positions.map(|iter| iter.collect()).unwrap_or_default();
                let normal_deltas: Vec<[f32; 3]> =
                    normals.map(|iter| iter.collect()).unwrap_or_default();
                let has_normal_deltas = !normal_deltas.is_empty();
                let vertices = (0..primitive_vertex_count)
                    .map(|i| MeshBlendShapeVertex {
                        position_delta: position_deltas.get(i).copied().unwrap_or([0.0; 3]),
                        normal_delta: normal_deltas.get(i).copied().unwrap_or([0.0; 3]),
                    })
                    .collect::<Vec<_>>();
                MeshBlendShape {
                    vertices,
                    has_normal_deltas,
                }
            })
            .collect::<Vec<_>>();
        append_primitive_blend_shapes(
            &mut blend_shapes,
            &primitive_blend_shapes,
            vertices.len(),
            primitive_vertex_count,
        );
        let base_vertex = vertices.len() as u32;
        for (i, position) in positions.iter().copied().enumerate() {
            let joint = joints.get(i).copied().unwrap_or([0, 0, 0, 0]);
            let weight = weights.get(i).copied().unwrap_or([1.0, 0.0, 0.0, 0.0]);
            vertices.push(MeshVertex {
                pos: position,
                normal: normals.get(i).copied().unwrap_or([0.0, 1.0, 0.0]),
                uv: tex_coords.get(i).copied().unwrap_or([0.0, 0.0]),
                paint_uv: paint_tex_coords
                    .get(i)
                    .copied()
                    .unwrap_or_else(|| tex_coords.get(i).copied().unwrap_or([0.0, 0.0])),
                joints: joint,
                weights: quantize_skin_weights(weight),
            });
        }
        let surface_start = indices.len() as u32;
        if let Some(read_indices) = reader.read_indices() {
            indices.extend(read_indices.into_u32().map(|idx| idx + base_vertex));
        } else {
            indices.extend((0..positions.len() as u32).map(|idx| idx + base_vertex));
        }
        let surface_count = (indices.len() as u32).saturating_sub(surface_start);
        if surface_count > 0 {
            surface_ranges.push(MeshRange {
                index_start: surface_start,
                index_count: surface_count,
                base_vertex: 0,
            });
        }
    }

    if vertices.is_empty() || indices.is_empty() {
        return None;
    }
    Some(DecodedMesh {
        vertices,
        indices,
        surface_ranges,
        blend_shapes,
        meshlets: Vec::new(),
        lods: Vec::new(),
        has_skinning,
    })
}

fn append_primitive_blend_shapes(
    mesh_blend_shapes: &mut Vec<MeshBlendShape>,
    primitive_blend_shapes: &[MeshBlendShape],
    base_vertex_count: usize,
    primitive_vertex_count: usize,
) {
    for _ in mesh_blend_shapes.len()..primitive_blend_shapes.len() {
        mesh_blend_shapes.push(MeshBlendShape {
            vertices: vec![
                MeshBlendShapeVertex {
                    position_delta: [0.0; 3],
                    normal_delta: [0.0; 3],
                };
                base_vertex_count
            ],
            has_normal_deltas: false,
        });
    }
    for (idx, mesh_shape) in mesh_blend_shapes.iter_mut().enumerate() {
        if let Some(primitive_shape) = primitive_blend_shapes.get(idx) {
            mesh_shape.has_normal_deltas |= primitive_shape.has_normal_deltas;
            for vertex_idx in 0..primitive_vertex_count {
                mesh_shape.vertices.push(
                    primitive_shape.vertices.get(vertex_idx).copied().unwrap_or(
                        MeshBlendShapeVertex {
                            position_delta: [0.0; 3],
                            normal_delta: [0.0; 3],
                        },
                    ),
                );
            }
        } else {
            mesh_shape.vertices.extend(vec![
                MeshBlendShapeVertex {
                    position_delta: [0.0; 3],
                    normal_delta: [0.0; 3],
                };
                primitive_vertex_count
            ]);
        }
    }
}

fn decode_static_payload(flags: u32, payload: &[u8]) -> Option<Vec<u8>> {
    if (flags & PMESH_FLAG_PAYLOAD_RAW) != 0 {
        Some(payload.to_vec())
    } else {
        decompress_zlib(payload).ok()
    }
}

fn quantize_skin_weights(weights: [f32; 4]) -> UnitVector4 {
    let mut normalized = [0.0; 4];
    let mut sum = 0.0f32;
    for (dst, src) in normalized.iter_mut().zip(weights) {
        let lane = src.clamp(0.0, 1.0);
        *dst = lane;
        sum += lane;
    }
    if sum.is_finite() && sum > 0.0 {
        for lane in &mut normalized {
            *lane /= sum;
        }
    } else {
        normalized = [1.0, 0.0, 0.0, 0.0];
    }
    let mut bytes = UnitVector4::new(normalized).to_u8();
    let total = bytes.iter().map(|&v| v as i32).sum::<i32>();
    let delta = 255 - total;
    if delta != 0 {
        let mut max_idx = 0usize;
        for idx in 1..bytes.len() {
            if bytes[idx] > bytes[max_idx] {
                max_idx = idx;
            }
        }
        let fixed = (bytes[max_idx] as i32 + delta).clamp(0, 255) as u8;
        bytes[max_idx] = fixed;
    }
    UnitVector4::from_u8(bytes)
}
