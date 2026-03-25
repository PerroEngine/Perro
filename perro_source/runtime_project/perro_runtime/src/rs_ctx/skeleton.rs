use super::core::RuntimeResourceApi;
use perro_io::{decompress_zlib, load_asset};
use perro_nodes::skeleton_3d::Bone3D;
use perro_resource_context::sub_apis::SkeletonAPI;
use perro_structs::{Quaternion, Transform3D, Vector3};
use std::collections::HashMap;

const PSKEL_MAGIC: &[u8; 5] = b"PSKEL";
const PSKEL_VERSION: u32 = 1;

impl SkeletonAPI for RuntimeResourceApi {
    fn load_bones(&self, source: &str) -> Vec<Bone3D> {
        if source.is_empty() {
            return Vec::new();
        }

        {
            let cache = self
                .skeleton_bones_cache
                .lock()
                .expect("skeleton cache mutex poisoned");
            if let Some(cached) = cache.get(source) {
                return cached.clone();
            }
        }

        let bones = load_bones_uncached(self, source).unwrap_or_default();

        let mut cache = self
            .skeleton_bones_cache
            .lock()
            .expect("skeleton cache mutex poisoned");
        cache.insert(source.to_string(), bones.clone());
        bones
    }
}

fn load_bones_uncached(api: &RuntimeResourceApi, source: &str) -> Option<Vec<Bone3D>> {
    if let Some(bytes) = api.static_skeleton_lookup.and_then(|lookup| lookup(source)) {
        return decode_pskel(bytes).ok();
    }

    if source.ends_with(".pskel") {
        let bytes = load_asset(source).ok()?;
        if bytes.starts_with(PSKEL_MAGIC) {
            return decode_pskel(&bytes).ok();
        }
        if let Ok(text) = std::str::from_utf8(&bytes) {
            return parse_pskel_text(text).ok();
        }
        return None;
    }

    if let Some((base_path, skin_index)) = split_gltf_skin_source(source) {
        let bytes = load_asset(base_path).ok()?;
        return load_bones_from_gltf(&bytes, skin_index).ok();
    }

    None
}

fn split_gltf_skin_source(source: &str) -> Option<(&str, usize)> {
    let (base, suffix) = source.rsplit_once(":skeleton[")?;
    if !suffix.ends_with(']') {
        return None;
    }
    let index_str = suffix.trim_end_matches(']');
    let skin_index = index_str.parse::<usize>().ok()?;
    Some((base, skin_index))
}

fn load_bones_from_gltf(bytes: &[u8], skin_index: usize) -> Result<Vec<Bone3D>, String> {
    let (doc, buffers, _images) =
        gltf::import_slice(bytes).map_err(|err| format!("gltf import failed: {err}"))?;

    let skin = doc
        .skins()
        .nth(skin_index)
        .ok_or_else(|| format!("skin index {skin_index} not found"))?;

    let joints: Vec<_> = skin.joints().collect();
    if joints.is_empty() {
        return Ok(Vec::new());
    }

    let mut parent_by_node = vec![None::<usize>; doc.nodes().len()];
    for node in doc.nodes() {
        let parent_index = node.index();
        for child in node.children() {
            parent_by_node[child.index()] = Some(parent_index);
        }
    }

    let mut joint_index_by_node = HashMap::<usize, usize>::new();
    for (index, joint) in joints.iter().enumerate() {
        joint_index_by_node.insert(joint.index(), index);
    }

    let inv_bind_mats = if let Some(accessor) = skin.inverse_bind_matrices() {
        match read_inv_bind_mats(&accessor, &buffers) {
            Ok(mats) => mats
                .into_iter()
                .map(transform_from_matrix)
                .collect::<Vec<_>>(),
            Err(_) => vec![Transform3D::IDENTITY; joints.len()],
        }
    } else {
        vec![Transform3D::IDENTITY; joints.len()]
    };

    let mut bones = Vec::<Bone3D>::with_capacity(joints.len());
    for (joint_index, joint) in joints.iter().enumerate() {
        let name = joint
            .name()
            .map(|n| n.to_string())
            .unwrap_or_else(|| format!("Bone{joint_index}"));
        let parent = resolve_joint_parent(joint.index(), &parent_by_node, &joint_index_by_node);
        let rest = transform_from_node(joint);
        let inv_bind = inv_bind_mats
            .get(joint_index)
            .copied()
            .unwrap_or(Transform3D::IDENTITY);
        bones.push(Bone3D {
            name: name.into(),
            parent,
            rest,
            inv_bind,
        });
    }

    Ok(bones)
}

fn resolve_joint_parent(
    joint_node_index: usize,
    parent_by_node: &[Option<usize>],
    joint_index_by_node: &HashMap<usize, usize>,
) -> i32 {
    let mut cursor = parent_by_node.get(joint_node_index).copied().flatten();
    while let Some(parent_idx) = cursor {
        if let Some(joint_index) = joint_index_by_node.get(&parent_idx) {
            return *joint_index as i32;
        }
        cursor = parent_by_node.get(parent_idx).copied().flatten();
    }
    -1
}

fn transform_from_node(node: &gltf::Node) -> Transform3D {
    match node.transform() {
        gltf::scene::Transform::Decomposed {
            translation,
            rotation,
            scale,
        } => Transform3D::new(
            Vector3::new(translation[0], translation[1], translation[2]),
            Quaternion::new(rotation[0], rotation[1], rotation[2], rotation[3]),
            Vector3::new(scale[0], scale[1], scale[2]),
        ),
        gltf::scene::Transform::Matrix { matrix } => transform_from_matrix(matrix),
    }
}

fn transform_from_matrix(matrix: [[f32; 4]; 4]) -> Transform3D {
    // glTF matrices are column-major. Basis vectors are columns 0..2, translation is column 3.
    let x = Vector3::new(matrix[0][0], matrix[0][1], matrix[0][2]);
    let y = Vector3::new(matrix[1][0], matrix[1][1], matrix[1][2]);
    let z = Vector3::new(matrix[2][0], matrix[2][1], matrix[2][2]);
    let position = Vector3::new(matrix[3][0], matrix[3][1], matrix[3][2]);

    let sx = x.length();
    let sy = y.length();
    let sz = z.length();
    let scale = Vector3::new(sx, sy, sz);

    let nx = if sx > 0.0 {
        Vector3::new(x.x / sx, x.y / sx, x.z / sx)
    } else {
        Vector3::new(1.0, 0.0, 0.0)
    };
    let ny = if sy > 0.0 {
        Vector3::new(y.x / sy, y.y / sy, y.z / sy)
    } else {
        Vector3::new(0.0, 1.0, 0.0)
    };
    let nz = if sz > 0.0 {
        Vector3::new(z.x / sz, z.y / sz, z.z / sz)
    } else {
        Vector3::new(0.0, 0.0, 1.0)
    };

    let rotation = quat_from_basis(nx, ny, nz);

    Transform3D::new(position, rotation, scale)
}

fn quat_from_basis(x: Vector3, y: Vector3, z: Vector3) -> Quaternion {
    let m00 = x.x;
    let m01 = y.x;
    let m02 = z.x;
    let m10 = x.y;
    let m11 = y.y;
    let m12 = z.y;
    let m20 = x.z;
    let m21 = y.z;
    let m22 = z.z;

    let trace = m00 + m11 + m22;
    if trace > 0.0 {
        let s = (trace + 1.0).sqrt() * 2.0;
        let w = 0.25 * s;
        let x = (m21 - m12) / s;
        let y = (m02 - m20) / s;
        let z = (m10 - m01) / s;
        Quaternion::new(x, y, z, w)
    } else if m00 > m11 && m00 > m22 {
        let s = (1.0 + m00 - m11 - m22).sqrt() * 2.0;
        let w = (m21 - m12) / s;
        let x = 0.25 * s;
        let y = (m01 + m10) / s;
        let z = (m02 + m20) / s;
        Quaternion::new(x, y, z, w)
    } else if m11 > m22 {
        let s = (1.0 + m11 - m00 - m22).sqrt() * 2.0;
        let w = (m02 - m20) / s;
        let x = (m01 + m10) / s;
        let y = 0.25 * s;
        let z = (m12 + m21) / s;
        Quaternion::new(x, y, z, w)
    } else {
        let s = (1.0 + m22 - m00 - m11).sqrt() * 2.0;
        let w = (m10 - m01) / s;
        let x = (m02 + m20) / s;
        let y = (m12 + m21) / s;
        let z = 0.25 * s;
        Quaternion::new(x, y, z, w)
    }
}

fn decode_pskel(bytes: &[u8]) -> Result<Vec<Bone3D>, String> {
    if bytes.len() < 5 + 4 * 3 {
        return Err("pskel too small".to_string());
    }
    if &bytes[..5] != PSKEL_MAGIC {
        return Err("invalid pskel magic".to_string());
    }
    let version = u32::from_le_bytes(bytes[5..9].try_into().unwrap());
    if version != PSKEL_VERSION {
        return Err(format!("unsupported pskel version {version}"));
    }
    let bone_count = u32::from_le_bytes(bytes[9..13].try_into().unwrap()) as usize;
    let raw_size = u32::from_le_bytes(bytes[13..17].try_into().unwrap()) as usize;
    let raw = decompress_zlib(&bytes[17..]).map_err(|err| err.to_string())?;
    if raw.len() != raw_size {
        return Err("pskel raw size mismatch".to_string());
    }

    let mut cursor = 0usize;
    let mut bones = Vec::with_capacity(bone_count);
    for _ in 0..bone_count {
        let name_len = read_u32(&raw, &mut cursor)? as usize;
        let name_bytes = read_bytes(&raw, &mut cursor, name_len)?;
        let name = std::str::from_utf8(name_bytes)
            .map_err(|_| "invalid bone name utf8".to_string())?
            .to_string();
        let parent = read_i32(&raw, &mut cursor)?;
        let rest = read_transform(&raw, &mut cursor)?;
        let inv_bind = read_transform(&raw, &mut cursor)?;
        bones.push(Bone3D {
            name: name.into(),
            parent,
            rest,
            inv_bind,
        });
    }

    Ok(bones)
}

fn read_inv_bind_mats(
    accessor: &gltf::Accessor,
    buffers: &[gltf::buffer::Data],
) -> Result<Vec<[[f32; 4]; 4]>, String> {
    if accessor.sparse().is_some() {
        return Err("sparse inverse bind matrices not supported".to_string());
    }
    let view = accessor
        .view()
        .ok_or("inverse bind accessor missing view")?;
    let buffer = buffers
        .get(view.buffer().index())
        .ok_or("inverse bind buffer missing")?;
    if accessor.data_type() != gltf::accessor::DataType::F32
        || accessor.dimensions() != gltf::accessor::Dimensions::Mat4
    {
        return Err("inverse bind accessor must be F32 MAT4".to_string());
    }
    let stride = view.stride().unwrap_or(64);
    let base = view.offset() + accessor.offset();
    let count = accessor.count();
    let bytes = buffer.0.as_slice();
    let mut out = Vec::with_capacity(count);

    for i in 0..count {
        let start = base + i * stride;
        if start + 64 > bytes.len() {
            return Err("inverse bind buffer out of bounds".to_string());
        }
        let mut mat = [[0.0f32; 4]; 4];

        (0..4).enumerate().for_each(|(col_idx, col)| {
            (0..4).enumerate().for_each(|(row_idx, row)| {
                let idx = start + (col * 4 + row) * 4;
                let raw = bytes[idx..idx + 4].try_into().unwrap();
                mat[col_idx][row_idx] = f32::from_le_bytes(raw);
            });
        });

        out.push(mat);
    }
    Ok(out)
}

fn parse_pskel_text(source: &str) -> Result<Vec<Bone3D>, String> {
    let mut bones = Vec::<Bone3D>::new();
    let mut current: Option<Bone3D> = None;

    for (line_no, raw_line) in source.lines().enumerate() {
        let line = raw_line.trim();
        if line.is_empty() || line.starts_with('#') || line.starts_with("//") {
            continue;
        }
        if let Some(name) = parse_bone_start(line) {
            if current.is_some() {
                return Err(format!("pskel: nested bone at line {}", line_no + 1));
            }
            current = Some(Bone3D {
                name: name.into(),
                parent: -1,
                rest: Transform3D::IDENTITY,
                inv_bind: Transform3D::IDENTITY,
            });
            continue;
        }
        if line.eq_ignore_ascii_case("[/bone]") {
            if let Some(bone) = current.take() {
                bones.push(bone);
                continue;
            }
            return Err(format!(
                "pskel: closing [/bone] without open at line {}",
                line_no + 1
            ));
        }

        let Some((key, value)) = line.split_once('=') else {
            return Err(format!("pskel: invalid line {}: {line}", line_no + 1));
        };
        let key = key.trim();
        let value = value.trim();

        let Some(bone) = current.as_mut() else {
            return Err(format!("pskel: field outside bone at line {}", line_no + 1));
        };

        match key {
            "parent" => {
                bone.parent = value
                    .parse::<i32>()
                    .map_err(|_| format!("pskel: invalid parent at line {}", line_no + 1))?;
            }
            "rest_pos" => bone.rest.position = parse_vec3(value, line_no + 1)?,
            "rest_scale" => bone.rest.scale = parse_vec3(value, line_no + 1)?,
            "rest_rot" => bone.rest.rotation = parse_quat(value, line_no + 1)?,
            "inv_pos" => bone.inv_bind.position = parse_vec3(value, line_no + 1)?,
            "inv_scale" => bone.inv_bind.scale = parse_vec3(value, line_no + 1)?,
            "inv_rot" => bone.inv_bind.rotation = parse_quat(value, line_no + 1)?,
            _ => {
                return Err(format!(
                    "pskel: unknown field `{key}` at line {}",
                    line_no + 1
                ));
            }
        }
    }

    if current.is_some() {
        return Err("pskel: missing [/bone] at end".to_string());
    }

    Ok(bones)
}

fn parse_bone_start(line: &str) -> Option<String> {
    let line = line.trim();
    if !line.starts_with("[bone") || !line.ends_with(']') {
        return None;
    }
    let inner = line
        .trim_start_matches("[bone")
        .trim_end_matches(']')
        .trim();
    if inner.is_empty() {
        return None;
    }
    if let Some(stripped) = inner.strip_prefix('"').and_then(|s| s.strip_suffix('"')) {
        return Some(stripped.to_string());
    }
    Some(inner.to_string())
}

fn parse_vec3(value: &str, line_no: usize) -> Result<Vector3, String> {
    let nums = parse_tuple(value, 3, line_no)?;
    Ok(Vector3::new(nums[0], nums[1], nums[2]))
}

fn parse_quat(value: &str, line_no: usize) -> Result<Quaternion, String> {
    let nums = parse_tuple(value, 4, line_no)?;
    Ok(Quaternion::new(nums[0], nums[1], nums[2], nums[3]))
}

fn parse_tuple(value: &str, count: usize, line_no: usize) -> Result<Vec<f32>, String> {
    let trimmed = value.trim();
    let inner = trimmed
        .strip_prefix('(')
        .and_then(|s| s.strip_suffix(')'))
        .ok_or_else(|| format!("pskel: expected tuple at line {line_no}"))?;
    let parts: Vec<_> = inner.split(',').map(|s| s.trim()).collect();
    if parts.len() != count {
        return Err(format!("pskel: expected {count} values at line {line_no}"));
    }
    let mut out = Vec::with_capacity(count);
    for part in parts {
        let value = part
            .parse::<f32>()
            .map_err(|_| format!("pskel: invalid number at line {line_no}"))?;
        out.push(value);
    }
    Ok(out)
}

fn read_u32(raw: &[u8], cursor: &mut usize) -> Result<u32, String> {
    let end = *cursor + 4;
    if end > raw.len() {
        return Err("pskel read_u32 out of bounds".to_string());
    }
    let value = u32::from_le_bytes(raw[*cursor..end].try_into().unwrap());
    *cursor = end;
    Ok(value)
}

fn read_i32(raw: &[u8], cursor: &mut usize) -> Result<i32, String> {
    let end = *cursor + 4;
    if end > raw.len() {
        return Err("pskel read_i32 out of bounds".to_string());
    }
    let value = i32::from_le_bytes(raw[*cursor..end].try_into().unwrap());
    *cursor = end;
    Ok(value)
}

fn read_f32(raw: &[u8], cursor: &mut usize) -> Result<f32, String> {
    let end = *cursor + 4;
    if end > raw.len() {
        return Err("pskel read_f32 out of bounds".to_string());
    }
    let value = f32::from_le_bytes(raw[*cursor..end].try_into().unwrap());
    *cursor = end;
    Ok(value)
}

fn read_bytes<'a>(raw: &'a [u8], cursor: &mut usize, len: usize) -> Result<&'a [u8], String> {
    let end = *cursor + len;
    if end > raw.len() {
        return Err("pskel read_bytes out of bounds".to_string());
    }
    let slice = &raw[*cursor..end];
    *cursor = end;
    Ok(slice)
}

fn read_transform(raw: &[u8], cursor: &mut usize) -> Result<Transform3D, String> {
    let px = read_f32(raw, cursor)?;
    let py = read_f32(raw, cursor)?;
    let pz = read_f32(raw, cursor)?;
    let sx = read_f32(raw, cursor)?;
    let sy = read_f32(raw, cursor)?;
    let sz = read_f32(raw, cursor)?;
    let rx = read_f32(raw, cursor)?;
    let ry = read_f32(raw, cursor)?;
    let rz = read_f32(raw, cursor)?;
    let rw = read_f32(raw, cursor)?;
    Ok(Transform3D::new(
        Vector3::new(px, py, pz),
        Quaternion::new(rx, ry, rz, rw),
        Vector3::new(sx, sy, sz),
    ))
}
