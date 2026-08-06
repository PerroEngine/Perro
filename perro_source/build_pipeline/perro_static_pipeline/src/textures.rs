use crate::{
    CachedSource, ResFileTree, SourceCache, StaticPipelineError, asset_uri, embedded_dir,
    ensure_unique_hashes, res_dir, source_stat, static_dir, write_hash_const, write_if_changed,
    write_static_lookup_fn,
};
use perro_asset_formats::{
    ptex::{
        EXTENSION as PTEX_EXTENSION, FLAG_FORMAT_MASK as PTEX_FLAG_FORMAT_MASK,
        FLAG_FORMAT_R8 as PTEX_FLAG_FORMAT_R8, FLAG_FORMAT_RGB8 as PTEX_FLAG_FORMAT_RGB8,
        FLAG_FORMAT_RGBA8 as PTEX_FLAG_FORMAT_RGBA8, FLAG_HAS_MIPS as PTEX_FLAG_HAS_MIPS,
        FLAG_PAYLOAD_RAW as PTEX_FLAG_PAYLOAD_RAW, MAGIC as PTEX_MAGIC, VERSION as PTEX_VERSION,
    },
    source_ext,
};
use perro_graphics_assets::{SVG_RASTER_SCALE, decode_image_rgba, mip::build_rgba_levels_for_filter};
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
    let context = format!("textures svg_scale={SVG_RASTER_SCALE} ptex_v{PTEX_VERSION}");
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
    // One classify pass instead of two: an opaque grayscale image used to be
    // scanned twice before it was even written. Both answers fall out of the
    // same walk, and the loop stops early once neither can still hold.
    let mut is_opaque = true;
    let mut is_gray = true;
    for px in raw_rgba.chunks_exact(4) {
        if px[3] != 255 {
            // One transparent texel settles it: the payload stays RGBA8 and
            // grayness cannot change that.
            is_opaque = false;
            break;
        }
        is_gray &= px[0] == px[1] && px[1] == px[2];
    }

    if is_opaque && is_gray {
        // Byte-strided gather; `extend` per pixel could not use the known
        // output length.
        let packed = raw_rgba.chunks_exact(4).map(|px| px[0]).collect::<Vec<u8>>();
        (PTEX_FLAG_FORMAT_R8, packed)
    } else if is_opaque {
        let mut packed = vec![0u8; (raw_rgba.len() / 4) * 3];
        for (dst, px) in packed.chunks_exact_mut(3).zip(raw_rgba.chunks_exact(4)) {
            dst.copy_from_slice(&px[..3]);
        }
        (PTEX_FLAG_FORMAT_RGB8, packed)
    } else {
        (PTEX_FLAG_FORMAT_RGBA8, raw_rgba.to_vec())
    }
}

/// Pack mip levels 1..n back-to-back in the base level's pixel format.
///
/// Baked here so the runtime never downsamples on the render thread. The chain
/// uses the shared filter, so a baked file and a runtime-generated chain are
/// byte-identical.
fn pack_mip_chain(raw_rgba: &[u8], width: u32, height: u32, base_flags: u32) -> Vec<u8> {
    let levels = build_rgba_levels_for_filter(
        raw_rgba,
        width,
        height,
        perro_structs::TextureFilterMode::LinearMipmap,
    );
    let mut packed = Vec::new();
    for level in levels.iter().skip(1) {
        match base_flags & PTEX_FLAG_FORMAT_MASK {
            PTEX_FLAG_FORMAT_R8 => packed.extend(level.rgba.chunks_exact(4).map(|px| px[0])),
            PTEX_FLAG_FORMAT_RGB8 => {
                for px in level.rgba.chunks_exact(4) {
                    packed.extend_from_slice(&px[..3]);
                }
            }
            _ => packed.extend_from_slice(&level.rgba),
        }
    }
    packed
}

fn encode_ptex(raw_rgba: &[u8], width: u32, height: u32) -> io::Result<Vec<u8>> {
    let (format_flags, packed_raw) = pack_texture_payload(raw_rgba);
    // Base and mips compress separately so a consumer that only needs the base
    // level never inflates the chain.
    let base_compressed = compress_zlib_best(&packed_raw)?;
    let mut flags = format_flags;
    let base_payload: &[u8] = if base_compressed.len() < packed_raw.len() {
        &base_compressed
    } else {
        flags |= PTEX_FLAG_PAYLOAD_RAW;
        &packed_raw
    };

    // One RAW flag covers both payloads, so the chain always uses whatever
    // encoding the base settled on. A tiny texture can end up with a chain that
    // compresses to slightly more than its raw bytes; that is a few dozen bytes
    // and beats teaching the format two encodings.
    let mip_raw = pack_mip_chain(raw_rgba, width, height, format_flags);
    let mip_payload = if mip_raw.is_empty() {
        Vec::new()
    } else if flags & PTEX_FLAG_PAYLOAD_RAW != 0 {
        mip_raw.clone()
    } else {
        compress_zlib_best(&mip_raw)?
    };
    let mip_raw_len = mip_raw.len();
    if mip_raw_len > 0 {
        flags |= PTEX_FLAG_HAS_MIPS;
    }

    let mut ptex = Vec::with_capacity(32 + base_payload.len() + mip_payload.len());
    ptex.extend_from_slice(PTEX_MAGIC);
    ptex.extend_from_slice(&PTEX_VERSION.to_le_bytes());
    ptex.extend_from_slice(&width.to_le_bytes());
    ptex.extend_from_slice(&height.to_le_bytes());
    ptex.extend_from_slice(&flags.to_le_bytes());
    ptex.extend_from_slice(&(packed_raw.len() as u32).to_le_bytes());
    ptex.extend_from_slice(&(base_payload.len() as u32).to_le_bytes());
    ptex.extend_from_slice(&(mip_raw_len as u32).to_le_bytes());
    ptex.extend_from_slice(base_payload);
    ptex.extend_from_slice(&mip_payload);
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
    fn pack_texture_payload_picks_narrowest_format() {
        use super::{
            PTEX_FLAG_FORMAT_R8, PTEX_FLAG_FORMAT_RGB8, PTEX_FLAG_FORMAT_RGBA8,
            pack_texture_payload,
        };

        let gray = [7u8, 7, 7, 255, 9, 9, 9, 255];
        assert_eq!(
            pack_texture_payload(&gray),
            (PTEX_FLAG_FORMAT_R8, vec![7, 9])
        );

        let color = [7u8, 8, 9, 255, 1, 2, 3, 255];
        assert_eq!(
            pack_texture_payload(&color),
            (PTEX_FLAG_FORMAT_RGB8, vec![7, 8, 9, 1, 2, 3])
        );

        // Gray texels but one is transparent: alpha wins, nothing is dropped.
        let gray_with_alpha = [7u8, 7, 7, 255, 9, 9, 9, 128];
        assert_eq!(
            pack_texture_payload(&gray_with_alpha),
            (PTEX_FLAG_FORMAT_RGBA8, gray_with_alpha.to_vec())
        );

        // Transparency after a color texel must still reach RGBA8 (the early
        // exit runs before grayness is fully known).
        let color_then_alpha = [7u8, 8, 9, 255, 9, 9, 9, 0];
        assert_eq!(
            pack_texture_payload(&color_then_alpha),
            (PTEX_FLAG_FORMAT_RGBA8, color_then_alpha.to_vec())
        );
    }

    /// Where a texture cache miss actually spends its time: pack, then zlib.
    /// Run with `--release --ignored --nocapture`.
    #[test]
    #[ignore = "bench probe; run with --release --ignored --nocapture"]
    fn ptex_encode_stage_bench() {
        use std::time::Instant;

        if cfg!(debug_assertions) {
            eprintln!("warn run this with --release for useful numbers");
        }
        for dim in [512usize, 1024, 2048] {
            // Photographic-ish: smooth gradients plus banded detail, so neither
            // trivially compressible nor incompressible.
            let mut rgba = Vec::with_capacity(dim * dim * 4);
            for y in 0..dim {
                for x in 0..dim {
                    let r = ((x * 255) / dim) as u8;
                    let g = ((y * 255) / dim) as u8;
                    let b = (((x ^ y) & 0xff) as u8).wrapping_add((x / 16) as u8);
                    rgba.extend_from_slice(&[r, g, b, 255]);
                }
            }

            let start = Instant::now();
            let (_flags, packed) = super::pack_texture_payload(&rgba);
            let pack = start.elapsed();

            let mut line = format!(
                "ptex {dim}x{dim}: pack {:>7.1} ms",
                pack.as_secs_f64() * 1000.0
            );
            type Compressor = fn(&[u8]) -> std::io::Result<Vec<u8>>;
            let variants: [(&str, Compressor); 2] = [
                ("fast", perro_io::compress_zlib_fast),
                ("best", perro_io::compress_zlib_best),
            ];
            for (label, compress) in variants {
                let start = Instant::now();
                let out = compress(&packed).expect("compress");
                let elapsed = start.elapsed();
                let start = Instant::now();
                let back = perro_io::decompress_zlib_limited(&out, packed.len()).expect("inflate");
                let inflate = start.elapsed();
                assert_eq!(back.len(), packed.len());
                line.push_str(&format!(
                    " | zlib {label} {:>7.1} ms {:>8.1} KiB (inflate {:>6.1} ms)",
                    elapsed.as_secs_f64() * 1000.0,
                    out.len() as f64 / 1024.0,
                    inflate.as_secs_f64() * 1000.0
                ));
            }
            eprintln!("{line}");
        }
    }

    #[test]
    fn ptex_current_version_is_v2() {
        assert_eq!(PTEX_VERSION, 2);
    }

    #[test]
    fn encoded_ptex_carries_a_decodable_mip_chain() {
        use perro_graphics_assets::{decode_ptex, decode_ptex_mip_levels, mip};

        // Non-uniform so the chain is not trivially constant.
        let (w, h) = (8u32, 4u32);
        let rgba = (0..(w * h) as usize)
            .flat_map(|i| [(i * 7) as u8, (i * 13) as u8, (i * 29) as u8, 255])
            .collect::<Vec<u8>>();

        let ptex = super::encode_ptex(&rgba, w, h).expect("encode");
        let (decoded, dw, dh) = decode_ptex(&ptex).expect("decode base");
        assert_eq!((dw, dh), (w, h));
        assert_eq!(decoded, rgba);

        let baked = decode_ptex_mip_levels(&ptex).expect("decode mips");
        let generated = mip::build_rgba_levels_for_filter(
            &rgba,
            w,
            h,
            perro_structs::TextureFilterMode::LinearMipmap,
        );
        assert_eq!(baked.len(), generated.len() - 1);
        for (baked, generated) in baked.iter().zip(generated.iter().skip(1)) {
            assert_eq!((baked.width, baked.height), (generated.width, generated.height));
            assert_eq!(baked.rgba, generated.rgba, "baked chain must match runtime");
        }
    }

    #[test]
    fn ptex_v1_files_still_decode_without_mips() {
        use perro_graphics_assets::{decode_ptex, decode_ptex_mip_levels};

        let rgba = vec![9u8, 8, 7, 255, 6, 5, 4, 255];
        use super::{PTEX_FLAG_PAYLOAD_RAW, PTEX_MAGIC};
        let (flags, packed) = super::pack_texture_payload(&rgba);
        let mut v1 = Vec::new();
        v1.extend_from_slice(PTEX_MAGIC);
        v1.extend_from_slice(&1u32.to_le_bytes());
        v1.extend_from_slice(&2u32.to_le_bytes());
        v1.extend_from_slice(&1u32.to_le_bytes());
        v1.extend_from_slice(&(flags | PTEX_FLAG_PAYLOAD_RAW).to_le_bytes());
        v1.extend_from_slice(&(packed.len() as u32).to_le_bytes());
        v1.extend_from_slice(&packed);

        let (decoded, w, h) = decode_ptex(&v1).expect("decode v1");
        assert_eq!((w, h), (2, 1));
        assert_eq!(decoded, rgba);
        assert!(decode_ptex_mip_levels(&v1).is_none());
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
