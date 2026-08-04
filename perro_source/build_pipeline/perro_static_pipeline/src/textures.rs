use crate::{
    CachedSource, ResFileTree, SourceCache, StaticPipelineError, asset_uri, embedded_dir,
    ensure_unique_hashes, res_dir, source_stat, static_dir, write_hash_const, write_if_changed,
    write_static_lookup_fn,
};
use perro_asset_formats::{
    ptex::{
        EXTENSION as PTEX_EXTENSION, FLAG_FORMAT_MASK as PTEX_FLAG_FORMAT_MASK,
        FLAG_FORMAT_R8 as PTEX_FLAG_FORMAT_R8, FLAG_FORMAT_RGB8 as PTEX_FLAG_FORMAT_RGB8,
        FLAG_FORMAT_RGBA8 as PTEX_FLAG_FORMAT_RGBA8, FLAG_PAYLOAD_RAW as PTEX_FLAG_PAYLOAD_RAW,
        MAGIC as PTEX_MAGIC, VERSION as PTEX_VERSION,
    },
    source_ext,
};
use perro_graphics_assets::{SVG_RASTER_SCALE, decode_image_rgba};
use perro_io::compress_zlib_best;
use rayon::prelude::*;
use std::{
    collections::HashSet,
    fmt::Write as _,
    fs, io,
    path::{Path, PathBuf},
};

pub fn generate_static_textures(
    project_root: &Path,
    res_tree: &ResFileTree,
) -> Result<(), StaticPipelineError> {
    let res_dir = res_dir(project_root);
    let static_dir = static_dir(project_root);
    let embedded_textures_dir = embedded_dir(project_root).join("textures");
    fs::create_dir_all(&static_dir)?;
    fs::create_dir_all(&embedded_textures_dir)?;

    let mut texture_inputs = res_tree
        .filter_ext(|ext| source_ext::contains(source_ext::IMAGE, ext))
        .into_iter()
        .map(|rel| {
            let full = res_dir.join(&rel);
            let uri = asset_uri(&rel);
            (rel, uri, full)
        })
        .collect::<Vec<(String, String, PathBuf)>>();
    texture_inputs.sort_by(|a, b| a.1.cmp(&b.1));
    texture_inputs.dedup_by(|a, b| a.1 == b.1);

    // Split cache hits from sources that need a real decode + compress pass.
    // The SVG raster scale changes decoded output bytes without touching the
    // source file stat key, so it must live in the cache context: a future
    // scale change rebakes instead of serving stale rasters.
    let context = format!("textures svg_scale={SVG_RASTER_SCALE}");
    let mut cache = SourceCache::open(&embedded_textures_dir, &context);
    let mut textures = Vec::<(String, String)>::with_capacity(texture_inputs.len());
    let mut baked_texture_uris = HashSet::<String>::new();
    let mut misses = Vec::<(String, String, PathBuf, u64, u128)>::new();
    for (rel, res_path, full_path) in texture_inputs {
        let stat = source_stat(&full_path);
        if let Some((len, mtime)) = stat
            && let Some(hit) = cache.lookup(&rel, len, mtime)
            && let Some(row) = hit.rows.first()
            && row.len() == 2
        {
            textures.push((row[0].clone(), row[1].clone()));
            continue;
        }
        let (len, mtime) = stat.unwrap_or((0, 0));
        misses.push((rel, res_path, full_path, len, mtime));
    }

    let encoded = misses
        .into_par_iter()
        .map(|(rel, res_path, full_path, len, mtime)| -> io::Result<_> {
            let file_bytes = fs::read(&full_path)?;
            let (raw_rgba, width, height) = decode_image_rgba(&file_bytes)
                .ok_or_else(|| io::Error::other(format!("failed to decode image `{res_path}`")))?;
            let ptex = encode_ptex(&raw_rgba, width, height)?;
            Ok((rel, res_path, len, mtime, ptex))
        })
        .collect::<io::Result<Vec<_>>>()?;

    for (rel, res_path, len, mtime, ptex) in encoded {
        // Path-hash names stay stable when other textures come and go, so an
        // added asset does not rename (and re-fingerprint) every blob.
        let rel_ptex = format!(
            "texture_{:016x}.{PTEX_EXTENSION}",
            perro_ids::string_to_u64(&res_path)
        );
        write_if_changed(&embedded_textures_dir.join(&rel_ptex), &ptex)?;
        cache.store(
            &rel,
            len,
            mtime,
            CachedSource {
                rows: vec![vec![res_path.clone(), rel_ptex.clone()]],
                files: vec![rel_ptex.clone()],
            },
        );
        textures.push((res_path, rel_ptex));
    }

    for job in crate::materials::collect_shader_bake_jobs(project_root, res_tree)? {
        baked_texture_uris.insert(job.texture_uri.clone());
        let rel_ptex = format!(
            "texture_{:016x}.{PTEX_EXTENSION}",
            perro_ids::string_to_u64(&job.texture_uri)
        );
        let cache_key = format!("__shader_bake__/{}", job.material_uri);
        let fingerprint = job.fingerprint();
        let hit = cache.lookup(&cache_key, fingerprint, 0);
        if hit.is_none() {
            let rgba = crate::shader_bake::bake_shader_texture(&job)?;
            let ptex = encode_ptex(&rgba, job.resolution[0], job.resolution[1])?;
            write_if_changed(&embedded_textures_dir.join(&rel_ptex), &ptex)?;
            cache.store(
                &cache_key,
                fingerprint,
                0,
                CachedSource {
                    rows: vec![vec![job.texture_uri.clone(), rel_ptex.clone()]],
                    files: vec![rel_ptex.clone()],
                },
            );
        }
        textures.push((job.texture_uri, rel_ptex));
    }
    cache.finish()?;

    textures.sort_by(|a, b| a.0.cmp(&b.0));
    ensure_unique_hashes("texture", textures.iter().map(|(path, _)| path.as_str()))?;

    let mut out = String::new();
    out.push_str("// Auto-generated by Perro Static Pipeline. Do not edit.\n");
    out.push_str("#![allow(unused_imports)]\n\n");
    for (index, (_res_path, rel_ptex)) in textures.iter().enumerate() {
        let include_path = format!("../../embedded/textures/{}", escape_str(rel_ptex));
        let _ = writeln!(
            out,
            "static TEXTURE_{index}: &[u8] = include_bytes!(\"{include_path}\");"
        );
    }
    if !textures.is_empty() {
        out.push('\n');
    }
    out.push_str("static EMPTY_TEXTURE: &[u8] = b\"\";\n\n");
    for (index, (res_path, _)) in textures.iter().enumerate() {
        write_hash_const(&mut out, &format!("TEXTURE_HASH_{index}"), res_path);
    }
    if !textures.is_empty() {
        out.push('\n');
    }
    let lookup_entries = textures
        .iter()
        .enumerate()
        .map(|(index, (res_path, _))| {
            (
                perro_ids::string_to_u64(res_path),
                format!("TEXTURE_HASH_{index}"),
                format!("TEXTURE_{index}"),
            )
        })
        .collect::<Vec<_>>();
    write_static_lookup_fn(
        &mut out,
        "lookup_texture",
        "TEXTURE_TABLE",
        "TextureEntry",
        "&'static [u8]",
        "EMPTY_TEXTURE",
        &lookup_entries,
    );

    write_if_changed(&static_dir.join("textures.rs"), out.as_bytes())?;
    crate::record_static_assets(
        perro_asset_formats::dlc::DlcAssetKind::TEXTURE,
        perro_asset_formats::dlc::DlcAssetAccess::BYTES,
        textures
            .iter()
            .map(|(path, _)| (path.as_str(), baked_texture_uris.contains(path))),
    );
    Ok(())
}

fn escape_str(input: &str) -> String {
    let mut out = String::with_capacity(input.len());
    for ch in input.chars() {
        match ch {
            '\\' => out.push_str("\\\\"),
            '"' => out.push_str("\\\""),
            '\n' => out.push_str("\\n"),
            '\r' => out.push_str("\\r"),
            '\t' => out.push_str("\\t"),
            _ => out.push(ch),
        }
    }
    out
}

fn pack_texture_payload(raw_rgba: &[u8]) -> (u32, Vec<u8>) {
    let is_opaque = raw_rgba.chunks_exact(4).all(|px| px[3] == 255);
    let is_gray_opaque = is_opaque
        && raw_rgba
            .chunks_exact(4)
            .all(|px| px[0] == px[1] && px[1] == px[2]);

    if is_gray_opaque {
        let mut packed = Vec::with_capacity(raw_rgba.len() / 4);
        for px in raw_rgba.chunks_exact(4) {
            packed.push(px[0]);
        }
        (PTEX_FLAG_FORMAT_R8, packed)
    } else if is_opaque {
        let mut packed = Vec::with_capacity((raw_rgba.len() / 4) * 3);
        for px in raw_rgba.chunks_exact(4) {
            packed.extend_from_slice(&px[..3]);
        }
        (PTEX_FLAG_FORMAT_RGB8, packed)
    } else {
        (PTEX_FLAG_FORMAT_RGBA8, raw_rgba.to_vec())
    }
}

fn encode_ptex(raw_rgba: &[u8], width: u32, height: u32) -> io::Result<Vec<u8>> {
    let (mut flags, packed_raw) = pack_texture_payload(raw_rgba);
    let compressed = compress_zlib_best(&packed_raw)?;
    let payload: &[u8] = if compressed.len() < packed_raw.len() {
        &compressed
    } else {
        flags |= PTEX_FLAG_PAYLOAD_RAW;
        &packed_raw
    };
    let mut ptex = Vec::with_capacity(24 + payload.len());
    ptex.extend_from_slice(PTEX_MAGIC);
    ptex.extend_from_slice(&PTEX_VERSION.to_le_bytes());
    ptex.extend_from_slice(&width.to_le_bytes());
    ptex.extend_from_slice(&height.to_le_bytes());
    ptex.extend_from_slice(
        &((flags & PTEX_FLAG_FORMAT_MASK) | (flags & PTEX_FLAG_PAYLOAD_RAW)).to_le_bytes(),
    );
    ptex.extend_from_slice(&(packed_raw.len() as u32).to_le_bytes());
    ptex.extend_from_slice(payload);
    Ok(ptex)
}

#[cfg(test)]
mod tests {
    use super::{PTEX_VERSION, generate_static_textures};
    use perro_graphics_assets::decode_ptex;
    use std::{
        fs,
        time::{SystemTime, UNIX_EPOCH},
    };

    #[test]
    fn ptex_current_version_is_v1() {
        assert_eq!(PTEX_VERSION, 1);
    }

    #[test]
    fn generate_static_textures_bakes_svg_to_ptex() {
        let unique = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("time")
            .as_nanos();
        let root = std::env::temp_dir().join(format!(
            "perro_static_svg_texture_{}_{}",
            std::process::id(),
            unique
        ));
        fs::create_dir_all(root.join("res")).expect("create res");
        fs::write(
            root.join("res").join("icon.svg"),
            br#"<svg xmlns="http://www.w3.org/2000/svg" width="2" height="2"><rect width="2" height="2" fill="red"/></svg>"#,
        )
        .expect("write svg");

        generate_static_textures(&root, &crate::ResFileTree::scan(&root).expect("res scan"))
            .expect("generate textures");
        let ptex_name = format!(
            "texture_{:016x}.ptex",
            perro_ids::string_to_u64("res://icon.svg")
        );
        let ptex = fs::read(
            root.join(".perro")
                .join("project")
                .join("embedded")
                .join("textures")
                .join(ptex_name),
        )
        .expect("read ptex");
        let (rgba, width, height) = decode_ptex(&ptex).expect("decode ptex");
        assert_eq!((width, height), (4, 4));
        assert_eq!(rgba.len(), 4 * 4 * 4);

        let _ = fs::remove_dir_all(root);
    }

    #[test]
    #[ignore = "requires GPU adapter"]
    fn static_pipeline_bakes_wgsl_material_to_ptex() {
        let unique = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("time")
            .as_nanos();
        let root = std::env::temp_dir().join(format!("perro_shader_bake_{unique}"));
        fs::create_dir_all(root.join("res/materials")).expect("create materials");
        fs::create_dir_all(root.join("res/shaders")).expect("create shaders");
        fs::write(
            root.join("res/materials/background.pmat"),
            "type = \"custom\"\nshader_path = \"res://shaders/background.wgsl\"\nrelease_bake = true\nbake_resolution = (2, 2)\n",
        )
        .expect("write material");
        fs::write(
            root.join("res/shaders/background.wgsl"),
            "fn shade_material(in: FragmentInput) -> vec4<f32> { return vec4<f32>(in.uv, 0.25, 1.0); }\nfn bake_texture(in: BakeInput) -> vec4<f32> { return vec4<f32>(in.uv, 0.25, 1.0); }\n",
        )
        .expect("write shader");
        let tree = crate::ResFileTree::scan(&root).expect("res scan");

        generate_static_textures(&root, &tree).expect("bake texture");
        crate::materials::generate_static_materials(&root, &tree).expect("gen materials");
        crate::shaders::generate_static_shaders(&root, &tree).expect("gen shaders");

        let texture_uri = crate::materials::baked_texture_uri("res://materials/background.pmat");
        let ptex_name = format!(
            "texture_{:016x}.ptex",
            perro_ids::string_to_u64(&texture_uri)
        );
        let ptex = fs::read(
            root.join(".perro/project/embedded/textures")
                .join(ptex_name),
        )
        .expect("read baked ptex");
        let (rgba, width, height) = decode_ptex(&ptex).expect("decode baked ptex");
        assert_eq!((width, height), (2, 2));
        assert_eq!(rgba.len(), 16);
        assert!(
            root.join(".perro/project/embedded/shaders/__perro_baked__/sample_texture.wgsl")
                .exists()
        );

        let _ = fs::remove_dir_all(root);
    }
}
