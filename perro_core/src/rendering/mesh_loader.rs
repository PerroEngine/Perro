//! Load mesh from GLTF/GLB bytes (dev path). Release uses static .pmesh.
//!
//! **Path syntax:** Use the model path alone if the file has one mesh; use `path:index` if it has
//! multiple. Internal mesh names (e.g. from the GLTF) are not documented and vary by exporter —
//! use index instead: `res://model.glb` (single mesh), `res://model.glb:0`, `res://model.glb:1`, …

use crate::rendering::renderer_3d::Vertex3D;

/// Normalize mesh path for cache keys: "res://model.glb" becomes "res://model.glb:0" so that
/// "model.glb" and "model.glb:0" resolve to the same entry and don't reload. Built-ins and
/// paths that already have a selector are returned unchanged.
pub fn normalize_mesh_path(path: &str) -> String {
    let path = path.trim();
    if path.starts_with("__") && path.ends_with("__") {
        return path.to_string(); // built-in
    }
    let (base, selector) = parse_mesh_path(path);
    if selector.is_some() {
        return path.to_string(); // already has :index
    }
    if base.ends_with(".glb") || base.ends_with(".gltf") {
        return format!("{}:0", base);
    }
    path.to_string()
}

/// Parse mesh path into (model_path, optional selector).
/// `res://model.glb` -> ("res://model.glb", None) — use first/only mesh.
/// `res://model.glb:0` or `res://model.glb:1` -> ("res://model.glb", Some("0")), ("res://model.glb", Some("1")) — mesh index.
pub fn parse_mesh_path(path: &str) -> (&str, Option<&str>) {
    let path = path.trim();
    // Only split on ':' if it looks like "something.glb:0" (model path ends with .glb/.gltf, selector = index)
    if let Some(last_colon) = path.rfind(':') {
        let after = path[last_colon + 1..].trim();
        let before = path[..last_colon].trim();
        if !after.is_empty()
            && !after.starts_with('/')
            && (before.ends_with(".glb") || before.ends_with(".gltf"))
        {
            return (before, Some(after));
        }
    }
    (path, None)
}

fn read_slice_u8(buf: &[u8], start: usize, count: usize, stride: usize) -> Vec<u32> {
    let mut out = Vec::with_capacity(count);
    for i in 0..count {
        let o = start + i * stride;
        out.push(buf.get(o).copied().unwrap_or(0) as u32);
    }
    out
}
fn read_slice_u16(buf: &[u8], start: usize, count: usize) -> Option<Vec<u32>> {
    let mut out = Vec::with_capacity(count);
    for i in 0..count {
        let o = start + i * 2;
        if o + 2 > buf.len() {
            return None;
        }
        out.push(u16::from_le_bytes(buf[o..o + 2].try_into().unwrap()) as u32);
    }
    Some(out)
}
fn read_slice_u32(buf: &[u8], start: usize, count: usize) -> Option<Vec<u32>> {
    let mut out = Vec::with_capacity(count);
    for i in 0..count {
        let o = start + i * 4;
        if o + 4 > buf.len() {
            return None;
        }
        out.push(u32::from_le_bytes(buf[o..o + 4].try_into().unwrap()));
    }
    Some(out)
}

/// Extract one mesh (first primitive) from a gltf::Mesh into Vertex3D + indices.
fn extract_primitive(
    primitive: gltf::Primitive<'_>,
    _blob: &[u8],
    buf_data: &[u8],
) -> Option<(Vec<Vertex3D>, Vec<u32>)> {
    // POSITION (required)
    let positions = primitive.get(&gltf::Semantic::Positions)?;
    let position_view = positions.view()?;
    let pos_start = position_view.offset() + positions.offset();
    let pos_stride = positions.size();
    let pos_count = positions.count();
    if pos_stride != 12 || pos_count == 0 {
        return None;
    }

    // NORMAL (optional)
    let normals: Vec<[f32; 3]> = primitive
        .get(&gltf::Semantic::Normals)
        .and_then(|acc| {
            let v = acc.view()?;
            let start = v.offset() + acc.offset();
            let stride = acc.size();
            let n = acc.count();
            let mut out = Vec::with_capacity(n);
            for i in 0..n {
                let off = start + i * stride;
                if off + 12 <= buf_data.len() {
                    out.push([
                        f32::from_le_bytes(buf_data[off..off + 4].try_into().unwrap()),
                        f32::from_le_bytes(buf_data[off + 4..off + 8].try_into().unwrap()),
                        f32::from_le_bytes(buf_data[off + 8..off + 12].try_into().unwrap()),
                    ]);
                } else {
                    out.push([0.0, 1.0, 0.0]);
                }
            }
            Some(out)
        })
        .unwrap_or_else(|| vec![[0.0, 1.0, 0.0]; pos_count]);

    let mut vertices = Vec::with_capacity(pos_count);
    for i in 0..pos_count {
        let off = pos_start + i * pos_stride;
        if off + 12 > buf_data.len() {
            break;
        }
        vertices.push(Vertex3D {
            position: [
                f32::from_le_bytes(buf_data[off..off + 4].try_into().unwrap()),
                f32::from_le_bytes(buf_data[off + 4..off + 8].try_into().unwrap()),
                f32::from_le_bytes(buf_data[off + 8..off + 12].try_into().unwrap()),
            ],
            normal: normals.get(i).copied().unwrap_or([0.0, 1.0, 0.0]),
        });
    }

    let indices = if let Some(ind_acc) = primitive.indices() {
        let ind_view = ind_acc.view()?;
        let ind_start = ind_view.offset() + ind_acc.offset();
        let count = ind_acc.count();
        match ind_acc.data_type() {
            gltf::accessor::DataType::U8 => read_slice_u8(buf_data, ind_start, count, 1),
            gltf::accessor::DataType::U16 => {
                read_slice_u16(buf_data, ind_start, count).unwrap_or_default()
            }
            gltf::accessor::DataType::U32 => {
                read_slice_u32(buf_data, ind_start, count).unwrap_or_default()
            }
            _ => (0..vertices.len() as u32).collect(),
        }
    } else {
        (0..vertices.len() as u32).collect()
    };

    if vertices.is_empty() {
        return None;
    }
    Some((vertices, indices))
}

/// Load one mesh from GLTF/GLB: by index (e.g. "0", "1") or by internal name; first mesh if selector is None.
/// Returns None if no mesh, or missing POSITION. Supports GLB (embedded bin).
pub fn load_gltf_mesh(bytes: &[u8], mesh_name: Option<&str>) -> Option<(Vec<Vertex3D>, Vec<u32>)> {
    let gltf = gltf::Gltf::from_slice(bytes).ok()?;
    let blob = gltf.blob.as_deref()?; // GLB: single buffer

    let mesh = if let Some(name) = mesh_name {
        // Try exact name match first
        gltf.meshes()
            .find(|m| m.name().as_deref() == Some(name))
            .or_else(|| {
                // Fallback: "0", "1", ... select mesh by index (res://model.glb:1 = second mesh)
                name.parse::<usize>()
                    .ok()
                    .and_then(|idx| gltf.meshes().nth(idx))
            })?
    } else {
        gltf.meshes().next()?
    };
    let primitive = mesh.primitives().next()?;
    extract_primitive(primitive, blob, blob)
}

/// List meshes in a GLTF/GLB. Use index in mesh_path: `res://model.glb:0`, `res://model.glb:1`, …
/// Returns (index, internal_name) for each mesh (internal names vary by exporter; prefer index).
pub fn list_gltf_mesh_names(bytes: &[u8]) -> Option<Vec<(usize, String)>> {
    let gltf = gltf::Gltf::from_slice(bytes).ok()?;
    let mut out = Vec::new();
    for (i, mesh) in gltf.meshes().enumerate() {
        let name = mesh
            .name()
            .map(String::from)
            .unwrap_or_else(|| format!("Mesh_{}", i));
        out.push((i, name));
    }
    if out.is_empty() {
        return None;
    }
    Some(out)
}

/// Load all meshes from a GLTF/GLB (for codegen: one static entry per mesh, keyed by model_path:mesh_name).
/// Returns (mesh_name, vertices, indices) for each mesh; name is mesh.name() or "Mesh_0", "Mesh_1", ...
pub fn load_gltf_model_all_meshes(bytes: &[u8]) -> Option<Vec<(String, Vec<Vertex3D>, Vec<u32>)>> {
    let gltf = gltf::Gltf::from_slice(bytes).ok()?;
    let blob = gltf.blob.as_deref()?;

    let mut out = Vec::new();
    for (i, mesh) in gltf.meshes().enumerate() {
        let name = mesh
            .name()
            .map(String::from)
            .unwrap_or_else(|| format!("Mesh_{}", i));
        let primitive = mesh.primitives().next()?;
        let (vertices, indices) = extract_primitive(primitive, blob, blob)?;
        out.push((name, vertices, indices));
    }
    if out.is_empty() {
        return None;
    }
    Some(out)
}
