use perro_asset_formats::dlc::{DlcAssetAccess, DlcAssetKind};
use perro_static_pipeline::StaticAssetInventoryRecord;
use sha2::{Digest, Sha256};
use std::fmt::Write as FmtWrite;

fn engine_abi_fingerprint() -> Result<[u8; 32], CompilerError> {
    let engine_root = engine_root_dir();
    let mut files = Vec::<PathBuf>::new();
    walk_dir(&engine_root.join("perro_source"), &mut |path| {
        if path.is_file()
            && path
                .extension()
                .and_then(|value| value.to_str())
                .is_some_and(|ext| matches!(ext, "rs" | "toml" | "wgsl"))
        {
            files.push(path.to_path_buf());
        }
        Ok(())
    })?;
    files.push(engine_root.join("Cargo.toml"));
    files.push(engine_root.join("Cargo.lock"));
    files.sort();
    files.dedup();

    let mut source = Sha256::new();
    for path in files {
        let relative = path.strip_prefix(&engine_root).unwrap_or(&path);
        source.update(relative.to_string_lossy().replace('\\', "/").as_bytes());
        source.update(b"\0");
        source.update(fs::read(path)?);
        source.update(b"\0");
    }
    let source_hash = source.finalize();

    let rustc = env::var_os("RUSTC").unwrap_or_else(|| "rustc".into());
    let output = Command::new(rustc).arg("-vV").output()?;
    if !output.status.success() {
        return Err(CompilerError::SceneParse("rustc -vV failed".to_string()));
    }
    let rustc_verbose = String::from_utf8(output.stdout)
        .map_err(|err| CompilerError::SceneParse(format!("rustc -vV output invalid: {err}")))?;
    let target = rustc_verbose
        .lines()
        .find_map(|line| line.strip_prefix("host: "))
        .unwrap_or("unknown");

    let mut canonical = Sha256::new();
    canonical.update(hex_bytes(&source_hash).as_bytes());
    canonical.update(b"\n");
    canonical.update(rustc_verbose.trim_end().as_bytes());
    canonical.update(b"\n");
    canonical.update(target.as_bytes());
    canonical.update(b"\n");
    canonical.update(b"features=");
    canonical.update(b"\n");
    canonical.update(b"dlc-typed-lookup-schema-v1");
    Ok(canonical.finalize().into())
}

fn hex_bytes(bytes: &[u8]) -> String {
    const HEX: &[u8; 16] = b"0123456789abcdef";
    let mut out = String::with_capacity(bytes.len() * 2);
    for byte in bytes {
        out.push(HEX[(byte >> 4) as usize] as char);
        out.push(HEX[(byte & 0x0f) as usize] as char);
    }
    out
}

fn append_dlc_registry_source(
    src: &mut String,
    inventory: &[StaticAssetInventoryRecord],
    fingerprint: [u8; 32],
) {
    src.push_str(
        "#[derive(Clone, Copy)]\nstruct DlcRegistryRecord {\n    kind: DlcAssetKind,\n    flags: DlcAssetFlags,\n    access: DlcAssetAccess,\n    path_hash: u64,\n    path: &'static str,\n}\n\n",
    );
    src.push_str("static DLC_REGISTRY: &[DlcRegistryRecord] = &[\n");
    for record in inventory {
        let _ = writeln!(
            src,
            "    DlcRegistryRecord {{ kind: DlcAssetKind::from_raw({}), flags: DlcAssetFlags::from_raw({}), access: DlcAssetAccess::from_raw({}), path_hash: {}u64, path: \"{}\" }},",
            record.kind.raw(),
            record.flags.raw(),
            record.access.raw(),
            perro_ids::string_to_u64(&record.path),
            escape_generated_str(&record.path),
        );
    }
    src.push_str("];\n\n");
    let fingerprint = fingerprint
        .iter()
        .map(|byte| byte.to_string())
        .collect::<Vec<_>>()
        .join(", ");
    let _ = writeln!(
        src,
        "static DLC_REGISTRY_API: DlcRegistryApiV1 = DlcRegistryApiV1::new([{fingerprint}], registry_len_v1, registry_get_v1, registry_find_v1, registry_lookup_bytes_v1);\n"
    );
    src.push_str(
        "fn registry_entry(record: &DlcRegistryRecord) -> DlcRegistryEntryV1 {\n    DlcRegistryEntryV1 { kind: record.kind, flags: record.flags, access: record.access, reserved: 0, path_hash: record.path_hash, path_ptr: record.path.as_ptr(), path_len: record.path.len() }\n}\n\n",
    );
    src.push_str("unsafe extern \"C\" fn registry_len_v1() -> usize { DLC_REGISTRY.len() }\n\n");
    src.push_str(
        "/// # Safety\n/// `entry_out` must be null or writable for one entry.\nunsafe extern \"C\" fn registry_get_v1(index: usize, entry_out: *mut DlcRegistryEntryV1) -> bool {\n    if entry_out.is_null() { return false; }\n    let Some(record) = DLC_REGISTRY.get(index) else { return false; };\n    // SAFETY: Caller supplies writable output storage.\n    unsafe { *entry_out = registry_entry(record); }\n    true\n}\n\n",
    );
    src.push_str(
        "/// # Safety\n/// `entry_out` must be null or writable for one entry.\nunsafe extern \"C\" fn registry_find_v1(kind: DlcAssetKind, path_hash: u64, entry_out: *mut DlcRegistryEntryV1) -> bool {\n    if entry_out.is_null() { return false; }\n    let Some(record) = DLC_REGISTRY.iter().find(|record| record.kind == kind && record.path_hash == path_hash) else { return false; };\n    // SAFETY: Caller supplies writable output storage.\n    unsafe { *entry_out = registry_entry(record); }\n    true\n}\n\n",
    );
    src.push_str(
        "fn registry_bytes(kind: DlcAssetKind, path_hash: u64) -> Option<&'static [u8]> {\n    let record = DLC_REGISTRY.iter().find(|record| record.kind == kind && record.path_hash == path_hash && record.access == DlcAssetAccess::BYTES)?;\n    let bytes = match record.kind.raw() {\n        4 => static_assets::tilesets::lookup_tileset(path_hash),\n        8 => static_assets::meshes::lookup_mesh(path_hash),\n        9 => static_assets::collision_trimeshes::lookup_collision_trimesh(path_hash),\n        10 => static_assets::skeletons::lookup_skeleton(path_hash),\n        11 => static_assets::textures::lookup_texture(path_hash),\n        12 => static_assets::shaders::lookup_shader(path_hash).as_bytes(),\n        13 => static_assets::audios::lookup_audio(path_hash),\n        17 => static_assets::navmeshes::lookup_navmesh(path_hash),\n        _ => return None,\n    };\n    Some(bytes)\n}\n\n",
    );
    src.push_str(
        "/// # Safety\n/// Output pointers must be null or writable for this call.\nunsafe extern \"C\" fn registry_lookup_bytes_v1(kind: DlcAssetKind, path_hash: u64, data_out: *mut *const u8, data_len_out: *mut usize) -> bool {\n    if data_out.is_null() || data_len_out.is_null() { return false; }\n    let Some(bytes) = registry_bytes(kind, path_hash) else { return false; };\n    // SAFETY: Caller supplies writable output storage.\n    unsafe { *data_out = bytes.as_ptr(); *data_len_out = bytes.len(); }\n    true\n}\n\n",
    );
    src.push_str(
        "#[unsafe(no_mangle)]\npub extern \"C\" fn perro_dlc_pack_registry_api(requested_version: u32) -> *const DlcRegistryApiV1 {\n    if requested_version == REGISTRY_ABI_VERSION { &DLC_REGISTRY_API } else { core::ptr::null() }\n}\n\n",
    );
    src.push_str(
        "#[unsafe(no_mangle)]\npub extern \"C\" fn perro_dlc_pack_registry_len() -> usize { DLC_REGISTRY.len() }\n\n",
    );
    src.push_str(
        "#[unsafe(no_mangle)]\n/// # Safety\n/// Output pointers must be null or writable for this call.\npub unsafe extern \"C\" fn perro_dlc_pack_registry_get(index: usize, path_hash_out: *mut u64, path_out: *mut *const u8, path_len_out: *mut usize, data_out: *mut *const u8, data_len_out: *mut usize) -> bool {\n    if path_hash_out.is_null() || path_out.is_null() || path_len_out.is_null() || data_out.is_null() || data_len_out.is_null() { return false; }\n    let Some(record) = DLC_REGISTRY.get(index) else { return false; };\n    let bytes = registry_bytes(record.kind, record.path_hash);\n    // SAFETY: Caller supplies writable output storage.\n    unsafe {\n        *path_hash_out = record.path_hash; *path_out = record.path.as_ptr(); *path_len_out = record.path.len();\n        *data_out = bytes.map_or(core::ptr::null(), |value| value.as_ptr());\n        *data_len_out = bytes.map_or(0, |value| value.len());\n    }\n    true\n}\n",
    );
}

fn escape_generated_str(input: &str) -> String {
    input
        .replace('\\', "\\\\")
        .replace('"', "\\\"")
        .replace('\n', "\\n")
        .replace('\r', "\\r")
}

fn registry_fingerprint(
    inventory: &[StaticAssetInventoryRecord],
) -> Result<[u8; 32], CompilerError> {
    if inventory
        .iter()
        .any(|record| record.access == DlcAssetAccess::ENGINE_LOCAL)
    {
        engine_abi_fingerprint()
    } else {
        Ok([0; 32])
    }
}

fn validate_registry_kind_coverage(
    inventory: &[StaticAssetInventoryRecord],
) -> Result<(), CompilerError> {
    for record in inventory {
        if record.kind == DlcAssetKind::UNKNOWN || record.kind.raw() > DlcAssetKind::NAVMESH.raw() {
            return Err(CompilerError::SceneParse(format!(
                "unsupported DLC registry kind {} for `{}`",
                record.kind.raw(),
                record.path
            )));
        }
    }
    Ok(())
}
