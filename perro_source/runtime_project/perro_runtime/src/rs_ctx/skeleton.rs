use super::core::RuntimeResourceApi;
use perro_asset_formats::pskel::{
    BONE_FLAG_HAS_INV_POS as PSKEL_BONE_FLAG_HAS_INV_POS,
    BONE_FLAG_HAS_INV_ROT as PSKEL_BONE_FLAG_HAS_INV_ROT,
    BONE_FLAG_HAS_INV_SCALE as PSKEL_BONE_FLAG_HAS_INV_SCALE,
    BONE_FLAG_HAS_PARENT as PSKEL_BONE_FLAG_HAS_PARENT,
    BONE_FLAG_HAS_REST_POS as PSKEL_BONE_FLAG_HAS_REST_POS,
    BONE_FLAG_HAS_REST_ROT as PSKEL_BONE_FLAG_HAS_REST_ROT,
    BONE_FLAG_HAS_REST_SCALE as PSKEL_BONE_FLAG_HAS_REST_SCALE,
    FLAG_PAYLOAD_RAW as PSKEL_FLAG_PAYLOAD_RAW, MAGIC as PSKEL_MAGIC, VERSION as PSKEL_VERSION,
    VERSION_2D as PSKEL_VERSION_2D,
};
use perro_ids::{parse_hashed_source_uri, string_to_u64};
use perro_io::{decompress_zlib, load_asset};
use perro_nodes::{skeleton_2d::Bone2D, skeleton_3d::Bone3D};
use perro_resource_api::sub_apis::SkeletonAPI;
use perro_structs::{Quaternion, Transform2D, Transform3D, Vector2, Vector3};
use std::collections::HashMap;

impl SkeletonAPI for RuntimeResourceApi {
    fn load_bones_2d(&self, source: &str) -> Vec<Bone2D> {
        if source.is_empty() {
            return Vec::new();
        }
        self.poll_skeleton_bone_loads();

        {
            let cache = self
                .skeleton_bones_2d_cache
                .lock()
                .expect("skeleton 2d cache mutex poisoned");
            if let Some(cached) = cache.get(source) {
                return cached.clone();
            }
        }

        if self.static_skeleton_lookup.is_some() {
            let bones = load_bones_2d_static(self, source).unwrap_or_default();
            let mut cache = self
                .skeleton_bones_2d_cache
                .lock()
                .expect("skeleton 2d cache mutex poisoned");
            cache.insert(source.to_string(), bones.clone());
            return bones;
        }

        self.queue_skeleton_2d_load(source);
        Vec::new()
    }

    fn load_bones_3d(&self, source: &str) -> Vec<Bone3D> {
        if source.is_empty() {
            return Vec::new();
        }
        self.poll_skeleton_bone_loads();

        {
            let cache = self
                .skeleton_bones_3d_cache
                .lock()
                .expect("skeleton 3d cache mutex poisoned");
            if let Some(cached) = cache.get(source) {
                return cached.clone();
            }
        }

        if self.static_skeleton_lookup.is_some() {
            let bones = load_bones_3d_static(self, source).unwrap_or_default();
            let mut cache = self
                .skeleton_bones_3d_cache
                .lock()
                .expect("skeleton 3d cache mutex poisoned");
            cache.insert(source.to_string(), bones.clone());
            return bones;
        }

        self.queue_skeleton_3d_load(source);
        Vec::new()
    }

    fn load_bones_2d_from_bytes(&self, bytes: &[u8]) -> Vec<Bone2D> {
        decode_pskel_2d(bytes).unwrap_or_default()
    }

    fn load_bones_3d_from_bytes(&self, bytes: &[u8]) -> Vec<Bone3D> {
        decode_pskel(bytes).unwrap_or_default()
    }

    fn load_bones(&self, source: &str) -> Vec<Bone3D> {
        self.load_bones_3d(source)
    }
}

impl RuntimeResourceApi {
    pub(crate) fn poll_skeleton_bone_loads(&self) {
        while let Ok(result) = self
            .skeleton_2d_load_rx
            .lock()
            .expect("skeleton 2d load rx mutex poisoned")
            .try_recv()
        {
            self.skeleton_bones_2d_pending
                .lock()
                .expect("skeleton 2d pending mutex poisoned")
                .remove(result.source.as_str());
            self.skeleton_bones_2d_cache
                .lock()
                .expect("skeleton 2d cache mutex poisoned")
                .insert(result.source, result.bones);
        }
        while let Ok(result) = self
            .skeleton_3d_load_rx
            .lock()
            .expect("skeleton 3d load rx mutex poisoned")
            .try_recv()
        {
            self.skeleton_bones_3d_pending
                .lock()
                .expect("skeleton 3d pending mutex poisoned")
                .remove(result.source.as_str());
            self.skeleton_bones_3d_cache
                .lock()
                .expect("skeleton 3d cache mutex poisoned")
                .insert(result.source, result.bones);
        }
    }

    pub(crate) fn cached_bones_2d(&self, source: &str) -> Option<Vec<Bone2D>> {
        self.poll_skeleton_bone_loads();
        self.skeleton_bones_2d_cache
            .lock()
            .expect("skeleton 2d cache mutex poisoned")
            .get(source)
            .cloned()
    }

    pub(crate) fn cached_bones_3d(&self, source: &str) -> Option<Vec<Bone3D>> {
        self.poll_skeleton_bone_loads();
        self.skeleton_bones_3d_cache
            .lock()
            .expect("skeleton 3d cache mutex poisoned")
            .get(source)
            .cloned()
    }

    pub(crate) fn is_skeleton_2d_pending(&self, source: &str) -> bool {
        self.skeleton_bones_2d_pending
            .lock()
            .expect("skeleton 2d pending mutex poisoned")
            .contains(source)
    }

    pub(crate) fn is_skeleton_3d_pending(&self, source: &str) -> bool {
        self.skeleton_bones_3d_pending
            .lock()
            .expect("skeleton 3d pending mutex poisoned")
            .contains(source)
    }

    fn queue_skeleton_2d_load(&self, source: &str) {
        {
            let mut pending = self
                .skeleton_bones_2d_pending
                .lock()
                .expect("skeleton 2d pending mutex poisoned");
            if !pending.insert(source.to_string()) {
                return;
            }
        }
        let source = source.to_string();
        let tx = self.skeleton_2d_load_tx.clone();
        #[cfg(not(target_arch = "wasm32"))]
        rayon::spawn(move || {
            let bones = load_bones_2d_dynamic(source.as_str()).unwrap_or_default();
            let _ = tx.send(super::core::AsyncSkeleton2DLoadResult { source, bones });
        });
        #[cfg(target_arch = "wasm32")]
        {
            let bones = load_bones_2d_dynamic(source.as_str()).unwrap_or_default();
            let _ = tx.send(super::core::AsyncSkeleton2DLoadResult { source, bones });
        }
    }

    fn queue_skeleton_3d_load(&self, source: &str) {
        {
            let mut pending = self
                .skeleton_bones_3d_pending
                .lock()
                .expect("skeleton 3d pending mutex poisoned");
            if !pending.insert(source.to_string()) {
                return;
            }
        }
        let source = source.to_string();
        let tx = self.skeleton_3d_load_tx.clone();
        #[cfg(not(target_arch = "wasm32"))]
        rayon::spawn(move || {
            let bones = load_bones_3d_dynamic(source.as_str()).unwrap_or_default();
            let _ = tx.send(super::core::AsyncSkeleton3DLoadResult { source, bones });
        });
        #[cfg(target_arch = "wasm32")]
        {
            let bones = load_bones_3d_dynamic(source.as_str()).unwrap_or_default();
            let _ = tx.send(super::core::AsyncSkeleton3DLoadResult { source, bones });
        }
    }
}

fn load_bones_2d_static(api: &RuntimeResourceApi, source: &str) -> Option<Vec<Bone2D>> {
    let path_hash = parse_hashed_source_uri(source).unwrap_or_else(|| string_to_u64(source));
    if let Some(lookup) = api.static_skeleton_lookup {
        let bytes = lookup(path_hash);
        if bytes.is_empty() {
            return Some(Vec::new());
        }
        return decode_pskel_2d(bytes).ok();
    }
    None
}

fn load_bones_2d_dynamic(source: &str) -> Option<Vec<Bone2D>> {
    if source.ends_with(".pskel2d") {
        let bytes = load_asset(source).ok()?;
        if bytes.starts_with(PSKEL_MAGIC) {
            return decode_pskel_2d(&bytes).ok();
        }
        if let Ok(text) = std::str::from_utf8(&bytes) {
            return parse_pskel2d_text(text).ok();
        }
    }

    None
}

fn load_bones_3d_static(api: &RuntimeResourceApi, source: &str) -> Option<Vec<Bone3D>> {
    let path_hash = parse_hashed_source_uri(source).unwrap_or_else(|| string_to_u64(source));
    if let Some(lookup) = api.static_skeleton_lookup {
        let bytes = lookup(path_hash);
        if bytes.is_empty() {
            return Some(Vec::new());
        }
        return decode_pskel(bytes).ok();
    }
    None
}

fn load_bones_3d_dynamic(source: &str) -> Option<Vec<Bone3D>> {
    if source.ends_with(".pskel") || source.ends_with(".pskel3d") {
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
            pose: rest,
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
    if bytes.len() < 21 {
        return Err("pskel too small".to_string());
    }
    let flags = u32::from_le_bytes(bytes[17..21].try_into().unwrap());
    let payload_start = 21usize;
    let raw = decode_pskel_payload(flags, &bytes[payload_start..])?;
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
        let flags = read_u32(&raw, &mut cursor)?;
        let parent = if (flags & PSKEL_BONE_FLAG_HAS_PARENT) != 0 {
            read_i32(&raw, &mut cursor)?
        } else {
            -1
        };
        let mut rest = Transform3D::IDENTITY;
        let mut inv_bind = Transform3D::IDENTITY;
        if (flags & PSKEL_BONE_FLAG_HAS_REST_POS) != 0 {
            rest.position = read_vec3(&raw, &mut cursor)?;
        }
        if (flags & PSKEL_BONE_FLAG_HAS_REST_SCALE) != 0 {
            rest.scale = read_vec3(&raw, &mut cursor)?;
        }
        if (flags & PSKEL_BONE_FLAG_HAS_REST_ROT) != 0 {
            rest.rotation = read_quat(&raw, &mut cursor)?;
        }
        if (flags & PSKEL_BONE_FLAG_HAS_INV_POS) != 0 {
            inv_bind.position = read_vec3(&raw, &mut cursor)?;
        }
        if (flags & PSKEL_BONE_FLAG_HAS_INV_SCALE) != 0 {
            inv_bind.scale = read_vec3(&raw, &mut cursor)?;
        }
        if (flags & PSKEL_BONE_FLAG_HAS_INV_ROT) != 0 {
            inv_bind.rotation = read_quat(&raw, &mut cursor)?;
        };
        bones.push(Bone3D {
            name: name.into(),
            parent,
            rest,
            pose: rest,
            inv_bind,
        });
    }

    Ok(bones)
}

fn decode_pskel_2d(bytes: &[u8]) -> Result<Vec<Bone2D>, String> {
    if bytes.len() < 21 {
        return Err("pskel2d too small".to_string());
    }
    if &bytes[..5] != PSKEL_MAGIC {
        return Err("invalid pskel2d magic".to_string());
    }
    let version = u32::from_le_bytes(bytes[5..9].try_into().unwrap());
    if version != PSKEL_VERSION_2D {
        return Err(format!("unsupported pskel2d version {version}"));
    }
    let bone_count = u32::from_le_bytes(bytes[9..13].try_into().unwrap()) as usize;
    let raw_size = u32::from_le_bytes(bytes[13..17].try_into().unwrap()) as usize;
    let flags = u32::from_le_bytes(bytes[17..21].try_into().unwrap());
    let raw = decode_pskel_payload(flags, &bytes[21..])?;
    if raw.len() != raw_size {
        return Err("pskel2d raw size mismatch".to_string());
    }

    let mut cursor = 0usize;
    let mut bones = Vec::with_capacity(bone_count);
    for _ in 0..bone_count {
        let name_len = read_u32(&raw, &mut cursor)? as usize;
        let name_bytes = read_bytes(&raw, &mut cursor, name_len)?;
        let name = std::str::from_utf8(name_bytes)
            .map_err(|_| "invalid bone name utf8".to_string())?
            .to_string();
        let flags = read_u32(&raw, &mut cursor)?;
        let parent = if (flags & PSKEL_BONE_FLAG_HAS_PARENT) != 0 {
            read_i32(&raw, &mut cursor)?
        } else {
            -1
        };
        let mut rest = Transform2D::IDENTITY;
        let mut inv_bind = Transform2D::IDENTITY;
        if (flags & PSKEL_BONE_FLAG_HAS_REST_POS) != 0 {
            rest.position = read_vec2(&raw, &mut cursor)?;
        }
        if (flags & PSKEL_BONE_FLAG_HAS_REST_SCALE) != 0 {
            rest.scale = read_vec2(&raw, &mut cursor)?;
        }
        if (flags & PSKEL_BONE_FLAG_HAS_REST_ROT) != 0 {
            rest.rotation = read_f32(&raw, &mut cursor)?;
        }
        if (flags & PSKEL_BONE_FLAG_HAS_INV_POS) != 0 {
            inv_bind.position = read_vec2(&raw, &mut cursor)?;
        }
        if (flags & PSKEL_BONE_FLAG_HAS_INV_SCALE) != 0 {
            inv_bind.scale = read_vec2(&raw, &mut cursor)?;
        }
        if (flags & PSKEL_BONE_FLAG_HAS_INV_ROT) != 0 {
            inv_bind.rotation = read_f32(&raw, &mut cursor)?;
        }
        bones.push(Bone2D {
            name: name.into(),
            parent,
            rest,
            pose: rest,
            inv_bind,
        });
    }

    Ok(bones)
}

fn decode_pskel_payload(flags: u32, payload: &[u8]) -> Result<Vec<u8>, String> {
    if (flags & PSKEL_FLAG_PAYLOAD_RAW) != 0 {
        Ok(payload.to_vec())
    } else {
        decompress_zlib(payload).map_err(|err| err.to_string())
    }
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
                pose: Transform3D::IDENTITY,
                inv_bind: Transform3D::IDENTITY,
            });
            continue;
        }
        if line.eq_ignore_ascii_case("[/bone]") {
            if let Some(mut bone) = current.take() {
                bone.pose = bone.rest;
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
            "rest_rot_deg" | "rest_rotation_deg" => {
                bone.rest.rotation = parse_euler_degrees_quat(value, line_no + 1)?
            }
            "inv_pos" => bone.inv_bind.position = parse_vec3(value, line_no + 1)?,
            "inv_scale" => bone.inv_bind.scale = parse_vec3(value, line_no + 1)?,
            "inv_rot" => bone.inv_bind.rotation = parse_quat(value, line_no + 1)?,
            "inv_rot_deg" | "inv_rotation_deg" => {
                bone.inv_bind.rotation = parse_euler_degrees_quat(value, line_no + 1)?
            }
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

fn parse_pskel2d_text(source: &str) -> Result<Vec<Bone2D>, String> {
    let mut bones = Vec::<Bone2D>::new();
    let mut current: Option<Bone2D> = None;

    for (line_no, raw_line) in source.lines().enumerate() {
        let line = raw_line.trim();
        if line.is_empty() || line.starts_with('#') || line.starts_with("//") {
            continue;
        }
        if let Some(name) = parse_bone_start(line) {
            if current.is_some() {
                return Err(format!("pskel2d: nested bone at line {}", line_no + 1));
            }
            current = Some(Bone2D {
                name: name.into(),
                parent: -1,
                rest: Transform2D::IDENTITY,
                pose: Transform2D::IDENTITY,
                inv_bind: Transform2D::IDENTITY,
            });
            continue;
        }
        if line.eq_ignore_ascii_case("[/bone]") {
            if let Some(mut bone) = current.take() {
                bone.pose = bone.rest;
                bones.push(bone);
                continue;
            }
            return Err(format!(
                "pskel2d: closing [/bone] without open at line {}",
                line_no + 1
            ));
        }

        let Some((key, value)) = line.split_once('=') else {
            return Err(format!("pskel2d: invalid line {}: {line}", line_no + 1));
        };
        let key = key.trim();
        let value = value.trim();

        let Some(bone) = current.as_mut() else {
            return Err(format!(
                "pskel2d: field outside bone at line {}",
                line_no + 1
            ));
        };

        match key {
            "parent" => {
                bone.parent = value
                    .parse::<i32>()
                    .map_err(|_| format!("pskel2d: invalid parent at line {}", line_no + 1))?;
            }
            "rest_pos" => bone.rest.position = parse_vec2(value, line_no + 1)?,
            "rest_scale" => bone.rest.scale = parse_vec2(value, line_no + 1)?,
            "rest_rot" => {
                bone.rest.rotation = value
                    .parse::<f32>()
                    .map_err(|_| format!("pskel2d: invalid rotation at line {}", line_no + 1))?;
            }
            "rest_rot_deg" | "rest_rotation_deg" => {
                bone.rest.rotation = value.parse::<f32>().map_err(|_| {
                    format!("pskel2d: invalid rotation degrees at line {}", line_no + 1)
                })? * std::f32::consts::PI
                    / 180.0;
            }
            "inv_pos" => bone.inv_bind.position = parse_vec2(value, line_no + 1)?,
            "inv_scale" => bone.inv_bind.scale = parse_vec2(value, line_no + 1)?,
            "inv_rot" => {
                bone.inv_bind.rotation = value
                    .parse::<f32>()
                    .map_err(|_| format!("pskel2d: invalid rotation at line {}", line_no + 1))?;
            }
            "inv_rot_deg" | "inv_rotation_deg" => {
                bone.inv_bind.rotation = value.parse::<f32>().map_err(|_| {
                    format!("pskel2d: invalid rotation degrees at line {}", line_no + 1)
                })? * std::f32::consts::PI
                    / 180.0;
            }
            _ => {
                return Err(format!(
                    "pskel2d: unknown field `{key}` at line {}",
                    line_no + 1
                ));
            }
        }
    }

    if current.is_some() {
        return Err("pskel2d: missing [/bone] at end".to_string());
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

fn parse_vec2(value: &str, line_no: usize) -> Result<Vector2, String> {
    let nums = parse_tuple(value, 2, line_no)?;
    Ok(Vector2::new(nums[0], nums[1]))
}

fn parse_quat(value: &str, line_no: usize) -> Result<Quaternion, String> {
    let nums = parse_tuple(value, 4, line_no)?;
    Ok(Quaternion::new(nums[0], nums[1], nums[2], nums[3]))
}

fn parse_euler_degrees_quat(value: &str, line_no: usize) -> Result<Quaternion, String> {
    let nums = parse_tuple(value, 3, line_no)?;
    let mut out = Quaternion::IDENTITY;
    out.rotate_xyz(
        nums[0].to_radians(),
        nums[1].to_radians(),
        nums[2].to_radians(),
    );
    out.normalize();
    Ok(out)
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

fn read_vec3(raw: &[u8], cursor: &mut usize) -> Result<Vector3, String> {
    Ok(Vector3::new(
        read_f32(raw, cursor)?,
        read_f32(raw, cursor)?,
        read_f32(raw, cursor)?,
    ))
}

fn read_vec2(raw: &[u8], cursor: &mut usize) -> Result<Vector2, String> {
    Ok(Vector2::new(read_f32(raw, cursor)?, read_f32(raw, cursor)?))
}

fn read_quat(raw: &[u8], cursor: &mut usize) -> Result<Quaternion, String> {
    Ok(Quaternion::new(
        read_f32(raw, cursor)?,
        read_f32(raw, cursor)?,
        read_f32(raw, cursor)?,
        read_f32(raw, cursor)?,
    ))
}

#[cfg(test)]
mod tests {
    use super::{
        RuntimeResourceApi, decode_pskel, decode_pskel_2d, parse_pskel_text, parse_pskel2d_text,
    };
    use perro_resource_api::sub_apis::SkeletonAPI;
    use perro_structs::{Quaternion, Vector2, Vector3};

    #[test]
    fn decode_pskel_accepts_v1_compressed_payload() {
        let mut raw = Vec::new();
        raw.extend_from_slice(&4u32.to_le_bytes());
        raw.extend_from_slice(b"root");
        raw.extend_from_slice(&0u32.to_le_bytes()); // all defaults
        let compressed = perro_io::compress_zlib_best(&raw).expect("compress pskel payload");

        let mut bytes = Vec::new();
        bytes.extend_from_slice(b"PSKEL");
        bytes.extend_from_slice(&1u32.to_le_bytes());
        bytes.extend_from_slice(&1u32.to_le_bytes());
        bytes.extend_from_slice(&(raw.len() as u32).to_le_bytes());
        bytes.extend_from_slice(&0u32.to_le_bytes());
        bytes.extend_from_slice(&compressed);

        let bones = decode_pskel(&bytes).expect("decode pskel v1");
        assert_eq!(bones.len(), 1);
        assert_eq!(bones[0].name.as_ref(), "root");
        assert_eq!(bones[0].parent, -1);
        assert_eq!(bones[0].rest.position, Vector3::ZERO);
        assert_eq!(bones[0].rest.scale, Vector3::ONE);
        assert_eq!(bones[0].rest.rotation, Quaternion::IDENTITY);
        assert_eq!(bones[0].inv_bind.position, Vector3::ZERO);
        assert_eq!(bones[0].inv_bind.scale, Vector3::ONE);
        assert_eq!(bones[0].inv_bind.rotation, Quaternion::IDENTITY);
    }

    #[test]
    fn decode_pskel_accepts_v1_raw_payload() {
        let mut raw = Vec::new();
        raw.extend_from_slice(&4u32.to_le_bytes());
        raw.extend_from_slice(b"root");
        raw.extend_from_slice(&0u32.to_le_bytes()); // all defaults

        let mut bytes = Vec::new();
        bytes.extend_from_slice(b"PSKEL");
        bytes.extend_from_slice(&1u32.to_le_bytes());
        bytes.extend_from_slice(&1u32.to_le_bytes());
        bytes.extend_from_slice(&(raw.len() as u32).to_le_bytes());
        bytes.extend_from_slice(&(1u32 << 31).to_le_bytes());
        bytes.extend_from_slice(&raw);

        let bones = decode_pskel(&bytes).expect("decode pskel v1 raw");
        assert_eq!(bones.len(), 1);
        assert_eq!(bones[0].name.as_ref(), "root");
        assert_eq!(bones[0].parent, -1);
    }

    #[test]
    fn decode_pskel2d_accepts_v1_raw_payload() {
        let mut raw = Vec::new();
        raw.extend_from_slice(&4u32.to_le_bytes());
        raw.extend_from_slice(b"root");
        raw.extend_from_slice(&(1u32 << 1).to_le_bytes());
        raw.extend_from_slice(&2.0f32.to_le_bytes());
        raw.extend_from_slice(&3.0f32.to_le_bytes());

        let mut bytes = Vec::new();
        bytes.extend_from_slice(b"PSKEL");
        bytes.extend_from_slice(&1u32.to_le_bytes());
        bytes.extend_from_slice(&1u32.to_le_bytes());
        bytes.extend_from_slice(&(raw.len() as u32).to_le_bytes());
        bytes.extend_from_slice(&(1u32 << 31).to_le_bytes());
        bytes.extend_from_slice(&raw);

        let bones = decode_pskel_2d(&bytes).expect("decode pskel2d v1 raw");
        assert_eq!(bones.len(), 1);
        assert_eq!(bones[0].name.as_ref(), "root");
        assert_eq!(bones[0].parent, -1);
        assert_eq!(bones[0].rest.position, Vector2::new(2.0, 3.0));
        assert_eq!(bones[0].rest.scale, Vector2::ONE);
    }

    #[test]
    fn parse_text_pskel_rotation_degrees() {
        let bones = parse_pskel_text(
            r#"
            [bone "Root"]
                parent = -1
                rest_rot_deg = (0, 90, 0)
                inv_rot_deg = (0, 0, 90)
            [/bone]
            "#,
        )
        .expect("parse pskel");

        assert!((bones[0].rest.rotation.y.abs() - std::f32::consts::FRAC_1_SQRT_2).abs() < 1e-5);
        assert!(
            (bones[0].inv_bind.rotation.z.abs() - std::f32::consts::FRAC_1_SQRT_2).abs() < 1e-5
        );
    }

    #[test]
    fn parse_text_pskel2d_rotation_degrees() {
        let bones = parse_pskel2d_text(
            r#"
            [bone "Root"]
                parent = -1
                rest_rot_deg = 90
                inv_rot_deg = 180
            [/bone]
            "#,
        )
        .expect("parse pskel2d");

        assert!((bones[0].rest.rotation - std::f32::consts::FRAC_PI_2).abs() < 1e-5);
        assert!((bones[0].inv_bind.rotation - std::f32::consts::PI).abs() < 1e-5);
    }

    #[test]
    fn decode_pskel_rejects_non_v1() {
        for version in [2u32, 3, 4, 5] {
            let mut bytes = Vec::new();
            bytes.extend_from_slice(b"PSKEL");
            bytes.extend_from_slice(&version.to_le_bytes());
            bytes.extend_from_slice(&0u32.to_le_bytes());
            bytes.extend_from_slice(&0u32.to_le_bytes());
            assert!(
                decode_pskel(&bytes).is_err(),
                "non-v1 pskel version {version} must reject"
            );
        }
    }

    #[test]
    fn dynamic_skeleton_load_returns_empty_and_completes_async() {
        let api = RuntimeResourceApi::new(None, None, None, None, None, None, None, None);
        let source = "res://missing_async_skeleton.pskel";

        assert!(api.load_bones_3d(source).is_empty());
        assert!(api.is_skeleton_3d_pending(source));

        for _ in 0..50 {
            api.poll_skeleton_bone_loads();
            if !api.is_skeleton_3d_pending(source) {
                break;
            }
            std::thread::sleep(std::time::Duration::from_millis(2));
        }

        assert!(!api.is_skeleton_3d_pending(source));
        assert!(api.cached_bones_3d(source).is_some());
    }
}
