use crate::{StaticPipelineError, embedded_dir, res_dir, static_dir};
use image::RgbaImage;
use perro_io::{compress_zlib_best, walkdir::collect_file_paths};
use perro_runtime::{
    LoadedTerrainSource, TerrainBakedChunkPhysics, TerrainBakedChunkTile, TerrainLayerRule,
    load_terrain_from_folder_source,
};
use perro_terrain::{ChunkConfig, TerrainChunk, TerrainData, Triangle, Vertex};
use rayon::prelude::*;
use perro_structs::Vector3;
use std::collections::hash_map::DefaultHasher;
use std::collections::{BTreeSet, HashMap, HashSet};
use std::fmt::Write as _;
use std::fs;
use std::io;
use std::hash::{Hash, Hasher};
use std::path::{Path, PathBuf};

const HEIGHTFIELD_EPSILON: f32 = 1.0e-4;
const POSITION_QUANT_EPSILON: f32 = 1.0e-5;
const PTERRB_MAGIC: &[u8; 8] = b"PTERRB1\0";
const PTERRB_FLAG_ZLIB: u32 = 1;
const TERRAIN_BAKED_TILE_DIR: &str = "__perro_terrain_baked";

struct TerrainRef {
    lookup_key: String,
    embedded_rel_path: String,
}

struct TerrainAsset {
    entry: TerrainRef,
    bytes: Vec<u8>,
}

struct GeneratedTerrainTile {
    rel_path: String,
    ptex_bytes: Vec<u8>,
}

pub fn generate_static_terrains(project_root: &Path) -> Result<(), StaticPipelineError> {
    let res_dir = res_dir(project_root);
    let static_dir = static_dir(project_root);
    let embedded_textures_dir = embedded_dir(project_root).join("textures");
    let embedded_terrains_dir = embedded_dir(project_root).join("terrains");
    let baked_tile_dir = embedded_textures_dir.join(TERRAIN_BAKED_TILE_DIR);
    fs::create_dir_all(&static_dir)?;
    fs::create_dir_all(&embedded_textures_dir)?;
    fs::create_dir_all(&embedded_terrains_dir)?;
    if baked_tile_dir.exists() {
        fs::remove_dir_all(&baked_tile_dir)?;
    }

    let mut rel_paths = if res_dir.exists() {
        collect_file_paths(&res_dir, &res_dir)?
            .into_iter()
            .map(|rel| rel.replace('\\', "/"))
            .collect::<Vec<_>>()
    } else {
        Vec::new()
    };
    rel_paths.sort();

    let mut folders = BTreeSet::<String>::new();
    for rel in rel_paths.iter().filter(|rel| {
        Path::new(rel)
            .extension()
            .and_then(|e| e.to_str())
            .is_some_and(|ext| ext.eq_ignore_ascii_case("ptchunk"))
    }) {
        folders.insert(terrain_source_for_chunk_path(rel));
    }
    for rel in rel_paths.iter().filter(|rel| is_terrain_gltf_path(rel)) {
        folders.insert(terrain_source_for_chunk_path(rel));
    }

    let mut terrain_assets = Vec::<TerrainAsset>::new();
    let mut generated_tiles = Vec::<GeneratedTerrainTile>::new();
    for (i, folder) in folders.into_iter().enumerate() {
        let disk_path = terrain_source_to_disk_path(&res_dir, &folder);
        let source = disk_path.to_string_lossy().to_string();
        let Some(mut loaded) = load_terrain_from_folder_source(&source) else {
            return Err(StaticPipelineError::Io(io::Error::other(format!(
                "failed to load terrain source at `{}`",
                disk_path.display()
            ))));
        };
        loaded.terrain = optimize_terrain_meshes(&loaded.terrain)?;
        let baked = bake_static_terrain_chunk_tiles(&res_dir, &folder, &loaded)?;
        if !baked.tiles.is_empty() {
            loaded.settings.baked_chunk_tiles = baked.tiles;
            loaded.settings.baked_chunk_physics = baked.physics;
            generated_tiles.extend(baked.generated_textures);
        } else {
            loaded.settings.baked_chunk_tiles.clear();
            loaded.settings.baked_chunk_physics.clear();
        }

        let raw = encode_loaded_terrain_source(&loaded)?;
        let compressed = compress_zlib_best(&raw)?;
        let mut bytes = Vec::with_capacity(16 + compressed.len());
        bytes.extend_from_slice(PTERRB_MAGIC);
        bytes.extend_from_slice(&PTERRB_FLAG_ZLIB.to_le_bytes());
        bytes.extend_from_slice(&(raw.len() as u32).to_le_bytes());
        bytes.extend_from_slice(&compressed);

        let embedded_rel_path = format!("terrain_{i}.pterrb");
        terrain_assets.push(TerrainAsset {
            entry: TerrainRef {
                lookup_key: folder,
                embedded_rel_path,
            },
            bytes,
        });
    }

    terrain_assets.sort_by(|a, b| a.entry.embedded_rel_path.cmp(&b.entry.embedded_rel_path));
    let mut terrain_refs = Vec::<TerrainRef>::with_capacity(terrain_assets.len());
    for asset in terrain_assets {
        let output_path = embedded_terrains_dir.join(&asset.entry.embedded_rel_path);
        if let Some(parent) = output_path.parent() {
            fs::create_dir_all(parent)?;
        }
        fs::write(&output_path, asset.bytes)?;
        terrain_refs.push(asset.entry);
    }

    if !generated_tiles.is_empty() {
        fs::create_dir_all(&baked_tile_dir)?;
        generated_tiles.sort_by(|a, b| a.rel_path.cmp(&b.rel_path));
        for tile in generated_tiles {
            let output = baked_tile_dir.join(&tile.rel_path);
            if let Some(parent) = output.parent() {
                fs::create_dir_all(parent)?;
            }
            fs::write(&output, tile.ptex_bytes)?;
        }
    }

    terrain_refs.sort_by(|a, b| a.lookup_key.cmp(&b.lookup_key));
    terrain_refs.dedup_by(|a, b| a.lookup_key == b.lookup_key);

    let mut out = String::new();
    out.push_str("// Auto-generated by Perro Static Pipeline. Do not edit.\n");
    out.push_str("#![allow(unused_imports)]\n");
    out.push_str("#![allow(dead_code)]\n\n");
    out.push('\n');

    for (index, terrain_ref) in terrain_refs.iter().enumerate() {
        let include_path = format!(
            "../../embedded/terrains/{}",
            escape_str(&terrain_ref.embedded_rel_path)
        );
        let _ = writeln!(
            out,
            "static TERRAIN_BLOB_{index}: &[u8] = include_bytes!(\"{include_path}\");"
        );
    }

    if !terrain_refs.is_empty() {
        out.push('\n');
    }
    out.push_str("pub fn lookup_terrain(path: &str) -> Option<&'static [u8]> {\n");
    out.push_str("    match path {\n");
    for (index, terrain_ref) in terrain_refs.iter().enumerate() {
        let _ = writeln!(
            out,
            "        \"{}\" => Some(TERRAIN_BLOB_{index}),",
            escape_str(&terrain_ref.lookup_key)
        );
        let slash = if terrain_ref.lookup_key.ends_with('/') {
            terrain_ref.lookup_key.to_string()
        } else {
            format!("{}/", terrain_ref.lookup_key)
        };
        let _ = writeln!(
            out,
            "        \"{}\" => Some(TERRAIN_BLOB_{index}),",
            escape_str(&slash)
        );
    }
    out.push_str("        _ => None,\n");
    out.push_str("    }\n");
    out.push_str("}\n");

    fs::write(static_dir.join("terrains.rs"), out)?;
    Ok(())
}

fn encode_loaded_terrain_source(loaded: &LoadedTerrainSource) -> io::Result<Vec<u8>> {
    let mut out = Vec::<u8>::new();

    write_f32(&mut out, loaded.terrain.chunk_size_meters());
    let chunks = loaded.terrain.chunks().collect::<Vec<_>>();
    write_u32(&mut out, chunks.len() as u32);
    for (coord, chunk) in chunks {
        write_i32(&mut out, coord.x);
        write_i32(&mut out, coord.z);
        write_u32(&mut out, chunk.vertices().len() as u32);
        write_u32(&mut out, chunk.triangles().len() as u32);
        for v in chunk.vertices() {
            write_f32(&mut out, v.position.x);
            write_f32(&mut out, v.position.y);
            write_f32(&mut out, v.position.z);
        }
        for t in chunk.triangles() {
            write_u32(&mut out, t.a as u32);
            write_u32(&mut out, t.b as u32);
            write_u32(&mut out, t.c as u32);
        }
    }

    let settings = &loaded.settings;
    write_opt_f32(&mut out, settings.sample_rate);
    // Legacy reserved slot kept for blob layout compatibility.
    write_opt_f32(&mut out, None);

    write_u32(&mut out, settings.layers.len() as u32);
    for layer in &settings.layers {
        write_u32(&mut out, layer.index as u32);
        write_opt_string(&mut out, layer.name.as_deref())?;
        out.push(layer.color.r);
        out.push(layer.color.g);
        out.push(layer.color.b);
        out.push(layer.color_tolerance);
        write_opt_string(&mut out, layer.texture_source.as_deref())?;
        write_f32(&mut out, layer.texture_tile_meters);
        write_f32(&mut out, layer.texture_rotation_degrees);
        out.push(layer.texture_hard_cut as u8);
        write_u32(&mut out, layer.blend_with.len() as u32);
        for idx in &layer.blend_with {
            write_u32(&mut out, *idx as u32);
        }
        write_opt_f32(&mut out, layer.friction);
        write_opt_f32(&mut out, layer.restitution);
    }

    write_u32(&mut out, settings.layer_blendings.len() as u32);
    for (a, b) in &settings.layer_blendings {
        write_u32(&mut out, *a as u32);
        write_u32(&mut out, *b as u32);
    }

    write_u32(&mut out, settings.baked_chunk_tiles.len() as u32);
    for tile in &settings.baked_chunk_tiles {
        write_i32(&mut out, tile.chunk_x);
        write_i32(&mut out, tile.chunk_z);
        write_opt_string(&mut out, Some(tile.texture_source.as_str()))?;
        write_f32(&mut out, tile.uv_min[0]);
        write_f32(&mut out, tile.uv_min[1]);
        write_f32(&mut out, tile.uv_max[0]);
        write_f32(&mut out, tile.uv_max[1]);
    }
    write_u32(&mut out, settings.baked_chunk_physics.len() as u32);
    for chunk in &settings.baked_chunk_physics {
        write_i32(&mut out, chunk.chunk_x);
        write_i32(&mut out, chunk.chunk_z);
        write_u32(&mut out, chunk.triangle_layers.len() as u32);
        for layer in &chunk.triangle_layers {
            write_i32(&mut out, *layer);
        }
    }

    Ok(out)
}

struct BakedTerrainTilesResult {
    tiles: Vec<TerrainBakedChunkTile>,
    physics: Vec<TerrainBakedChunkPhysics>,
    generated_textures: Vec<GeneratedTerrainTile>,
}

fn bake_static_terrain_chunk_tiles(
    res_dir: &Path,
    terrain_source: &str,
    loaded: &LoadedTerrainSource,
) -> io::Result<BakedTerrainTilesResult> {
    let rules = loaded.settings.layers.as_slice();
    if rules.is_empty() {
        return Ok(BakedTerrainTilesResult {
            tiles: Vec::new(),
            physics: Vec::new(),
            generated_textures: Vec::new(),
        });
    }

    let Some(map_source) = terrain_map_candidate_source(terrain_source) else {
        return Ok(BakedTerrainTilesResult {
            tiles: Vec::new(),
            physics: Vec::new(),
            generated_textures: Vec::new(),
        });
    };
    let Some(map_image) = load_image_from_source(res_dir, &map_source) else {
        return Ok(BakedTerrainTilesResult {
            tiles: Vec::new(),
            physics: Vec::new(),
            generated_textures: Vec::new(),
        });
    };

    let mut chunks = loaded.terrain.chunks().collect::<Vec<_>>();
    chunks.sort_unstable_by_key(|(coord, _)| (coord.x, coord.z));
    if chunks.is_empty() {
        return Ok(BakedTerrainTilesResult {
            tiles: Vec::new(),
            physics: Vec::new(),
            generated_textures: Vec::new(),
        });
    }
    let chunk_size = loaded.terrain.chunk_size_meters();
    let Some(terrain_bounds) = terrain_world_bounds_from_chunks(chunk_size, &chunks) else {
        return Ok(BakedTerrainTilesResult {
            tiles: Vec::new(),
            physics: Vec::new(),
            generated_textures: Vec::new(),
        });
    };

    let layer_textures = load_layer_textures_from_rules(res_dir, rules);
    let (map_w, map_h) = map_image.dimensions();
    if map_w == 0 || map_h == 0 {
        return Ok(BakedTerrainTilesResult {
            tiles: Vec::new(),
            physics: Vec::new(),
            generated_textures: Vec::new(),
        });
    }
    let (terrain_min_x, terrain_max_x, terrain_min_z, terrain_max_z) = terrain_bounds;
    let span_x = (terrain_max_x - terrain_min_x).max(1.0e-3);
    let span_z = (terrain_max_z - terrain_min_z).max(1.0e-3);
    let upscale = perro_runtime::terrain_bake::terrain_layer_bake_upscale(
        rules,
        loaded.settings.sample_rate,
    );
    let smoothed_map = perro_runtime::terrain_bake::build_smoothed_terrain_map(&map_image, upscale);

    let mut base_hash = DefaultHasher::new();
    terrain_source.hash(&mut base_hash);
    map_source.hash(&mut base_hash);
    hash_layer_rules(&mut base_hash, rules);
    loaded
        .settings
        .sample_rate
        .unwrap_or(0.0)
        .to_bits()
        .hash(&mut base_hash);
    upscale.hash(&mut base_hash);
    map_w.hash(&mut base_hash);
    map_h.hash(&mut base_hash);
    chunk_size.to_bits().hash(&mut base_hash);
    terrain_min_x.to_bits().hash(&mut base_hash);
    terrain_max_x.to_bits().hash(&mut base_hash);
    terrain_min_z.to_bits().hash(&mut base_hash);
    terrain_max_z.to_bits().hash(&mut base_hash);
    let base_hash = format!("{:016x}", base_hash.finish());

    const BORDER: u32 = 1;

    let per_chunk = chunks
        .par_iter()
        .map(|(coord, chunk)| -> io::Result<Option<(TerrainBakedChunkTile, TerrainBakedChunkPhysics, GeneratedTerrainTile)>> {
            let Some((chunk_min_x, chunk_max_x, chunk_min_z, chunk_max_z)) =
                terrain_chunk_world_bounds(chunk_size, *coord, chunk)
            else {
                return Ok(None);
            };
            let u0 = ((chunk_min_x - terrain_min_x) / span_x).clamp(0.0, 1.0);
            let u1 = ((chunk_max_x - terrain_min_x) / span_x).clamp(0.0, 1.0);
            let v0 = ((chunk_min_z - terrain_min_z) / span_z).clamp(0.0, 1.0);
            let v1 = ((chunk_max_z - terrain_min_z) / span_z).clamp(0.0, 1.0);

            let mut x0 = (u0 * map_w as f32).floor() as u32;
            let mut x1 = (u1 * map_w as f32).ceil() as u32;
            let mut y0 = (v0 * map_h as f32).floor() as u32;
            let mut y1 = (v1 * map_h as f32).ceil() as u32;
            if x1 <= x0 {
                x1 = (x0 + 1).min(map_w);
            }
            if y1 <= y0 {
                y1 = (y0 + 1).min(map_h);
            }
            x0 = x0.min(map_w.saturating_sub(1));
            y0 = y0.min(map_h.saturating_sub(1));
            x1 = x1.max(x0 + 1).min(map_w);
            y1 = y1.max(y0 + 1).min(map_h);

            let px0 = x0.saturating_sub(BORDER);
            let py0 = y0.saturating_sub(BORDER);
            let px1 = (x1 + BORDER).min(map_w);
            let py1 = (y1 + BORDER).min(map_h);
            let w = px1.saturating_sub(px0).max(1);
            let h = py1.saturating_sub(py0).max(1);
            let out_w = w.saturating_mul(upscale).max(1);
            let out_h = h.saturating_mul(upscale).max(1);

            let baked_tile = perro_runtime::terrain_bake::build_layered_terrain_chunk_tile(
                &map_image,
                smoothed_map.as_ref(),
                &layer_textures,
                rules,
                terrain_bounds,
                px0,
                py0,
                out_w,
                out_h,
                upscale,
            );
            let baked_ptex = encode_ptex_from_rgba(&baked_tile)?;

            let x0_local = x0.saturating_sub(px0) as f32 * upscale as f32;
            let y0_local = y0.saturating_sub(py0) as f32 * upscale as f32;
            let x1_local = x1.saturating_sub(px0) as f32 * upscale as f32;
            let y1_local = y1.saturating_sub(py0) as f32 * upscale as f32;
            let uv_min = [(x0_local + 0.5) / out_w as f32, (y0_local + 0.5) / out_h as f32];
            let uv_max = [
                (x1_local - 0.5).max(uv_min[0] * out_w as f32 + 1.0e-4) / out_w as f32,
                (y1_local - 0.5).max(uv_min[1] * out_h as f32 + 1.0e-4) / out_h as f32,
            ];
            let tile = TerrainBakedChunkTile {
                chunk_x: coord.x,
                chunk_z: coord.z,
                texture_source: format!(
                    "bin://{}/{}/{}_{}.png",
                    TERRAIN_BAKED_TILE_DIR, base_hash, coord.x, coord.z
                ),
                uv_min,
                uv_max,
            };
            let tri_layers = bake_chunk_triangle_layers(
                &map_image,
                rules,
                terrain_bounds,
                chunk_size,
                *coord,
                chunk,
            );
            let physics = TerrainBakedChunkPhysics {
                chunk_x: coord.x,
                chunk_z: coord.z,
                triangle_layers: tri_layers,
            };
            Ok(Some((
                tile,
                physics,
                GeneratedTerrainTile {
                    rel_path: format!("{}/{}_{}.ptex", base_hash, coord.x, coord.z),
                    ptex_bytes: baked_ptex,
                },
            )))
        })
        .collect::<io::Result<Vec<_>>>()?;
    let mut tiles = Vec::with_capacity(per_chunk.len());
    let mut physics = Vec::with_capacity(per_chunk.len());
    let mut generated_textures = Vec::with_capacity(per_chunk.len());
    for per_chunk in per_chunk {
        let Some((tile, phys, generated_tile)) = per_chunk else {
            continue;
        };
        tiles.push(tile);
        physics.push(phys);
        generated_textures.push(generated_tile);
    }

    Ok(BakedTerrainTilesResult {
        tiles,
        physics,
        generated_textures,
    })
}

fn write_u32(out: &mut Vec<u8>, value: u32) {
    out.extend_from_slice(&value.to_le_bytes());
}

fn write_i32(out: &mut Vec<u8>, value: i32) {
    out.extend_from_slice(&value.to_le_bytes());
}

fn write_f32(out: &mut Vec<u8>, value: f32) {
    out.extend_from_slice(&value.to_le_bytes());
}

fn write_opt_f32(out: &mut Vec<u8>, value: Option<f32>) {
    if let Some(v) = value {
        out.push(1);
        write_f32(out, v);
    } else {
        out.push(0);
    }
}

fn write_opt_string(out: &mut Vec<u8>, value: Option<&str>) -> io::Result<()> {
    if let Some(v) = value {
        out.push(1);
        let bytes = v.as_bytes();
        let len = u32::try_from(bytes.len()).map_err(io::Error::other)?;
        write_u32(out, len);
        out.extend_from_slice(bytes);
    } else {
        out.push(0);
    }
    Ok(())
}

fn terrain_source_to_disk_path(res_dir: &Path, source: &str) -> PathBuf {
    let rel = source
        .trim()
        .trim_start_matches("res://")
        .trim_start_matches('/');
    if rel.is_empty() {
        return res_dir.to_path_buf();
    }
    res_dir.join(rel)
}

fn terrain_source_for_chunk_path(rel_chunk_path: &str) -> String {
    let rel = rel_chunk_path.replace('\\', "/");
    let folder = Path::new(&rel)
        .parent()
        .map(path_to_slash_string)
        .unwrap_or_default();
    if folder.is_empty() {
        "res://".to_string()
    } else {
        format!("res://{folder}")
    }
}

fn path_to_slash_string(path: &Path) -> String {
    path.components()
        .map(|c| c.as_os_str().to_string_lossy().to_string())
        .collect::<Vec<_>>()
        .join("/")
}

fn is_terrain_gltf_path(rel: &str) -> bool {
    let path = Path::new(rel);
    let Some(file_name) = path.file_name().and_then(|s| s.to_str()) else {
        return false;
    };
    if !file_name.eq_ignore_ascii_case("terrain.glb")
        && !file_name.eq_ignore_ascii_case("terrain.gltf")
    {
        return false;
    }
    path.extension()
        .and_then(|e| e.to_str())
        .is_some_and(|ext| ext.eq_ignore_ascii_case("glb") || ext.eq_ignore_ascii_case("gltf"))
}

fn terrain_map_candidate_source(terrain_source: &str) -> Option<String> {
    let source = terrain_source.trim();
    if source.is_empty() {
        return None;
    }
    let mut base = source
        .trim_end_matches('/')
        .trim_end_matches('\\')
        .to_string();
    let lower = base.to_ascii_lowercase();
    if lower.ends_with(".glb") || lower.ends_with(".gltf") || lower.ends_with(".ptchunk") {
        if let Some((head, _)) = base.rsplit_once(['/', '\\']) {
            base = head.to_string();
        } else {
            base.clear();
        }
    }
    if base.is_empty() {
        return Some("terrain_map.png".to_string());
    }
    let sep = if base.contains('\\') && !base.contains('/') {
        '\\'
    } else {
        '/'
    };
    Some(format!("{base}{sep}terrain_map.png"))
}

fn load_image_from_source(res_dir: &Path, source: &str) -> Option<image::RgbaImage> {
    let rel = source
        .trim()
        .trim_start_matches("res://")
        .trim_start_matches('/');
    let path = res_dir.join(rel);
    let bytes = fs::read(path).ok()?;
    let image = image::load_from_memory(&bytes).ok()?.to_rgba8();
    if image.width() == 0 || image.height() == 0 {
        return None;
    }
    Some(image)
}

fn load_layer_textures_from_rules(
    res_dir: &Path,
    rules: &[TerrainLayerRule],
) -> Vec<Option<image::RgbaImage>> {
    rules
        .iter()
        .map(|rule| {
            rule.texture_source
                .as_deref()
                .and_then(|source| load_image_from_source(res_dir, source))
        })
        .collect()
}

fn terrain_world_bounds_from_chunks(
    chunk_size: f32,
    chunks: &[(perro_terrain::ChunkCoord, &TerrainChunk)],
) -> Option<(f32, f32, f32, f32)> {
    let mut min_x = f32::INFINITY;
    let mut max_x = f32::NEG_INFINITY;
    let mut min_z = f32::INFINITY;
    let mut max_z = f32::NEG_INFINITY;
    for (coord, chunk) in chunks {
        let base_x = coord.x as f32 * chunk_size;
        let base_z = coord.z as f32 * chunk_size;
        for vertex in chunk.vertices() {
            min_x = min_x.min(base_x + vertex.position.x);
            max_x = max_x.max(base_x + vertex.position.x);
            min_z = min_z.min(base_z + vertex.position.z);
            max_z = max_z.max(base_z + vertex.position.z);
        }
    }
    if !min_x.is_finite() || !max_x.is_finite() || !min_z.is_finite() || !max_z.is_finite() {
        return None;
    }
    Some((min_x, max_x, min_z, max_z))
}

fn terrain_chunk_world_bounds(
    chunk_size: f32,
    coord: perro_terrain::ChunkCoord,
    chunk: &TerrainChunk,
) -> Option<(f32, f32, f32, f32)> {
    let base_x = coord.x as f32 * chunk_size;
    let base_z = coord.z as f32 * chunk_size;
    let mut min_x = f32::INFINITY;
    let mut max_x = f32::NEG_INFINITY;
    let mut min_z = f32::INFINITY;
    let mut max_z = f32::NEG_INFINITY;
    for vertex in chunk.vertices() {
        min_x = min_x.min(base_x + vertex.position.x);
        max_x = max_x.max(base_x + vertex.position.x);
        min_z = min_z.min(base_z + vertex.position.z);
        max_z = max_z.max(base_z + vertex.position.z);
    }
    if !min_x.is_finite() || !max_x.is_finite() || !min_z.is_finite() || !max_z.is_finite() {
        return None;
    }
    Some((min_x, max_x, min_z, max_z))
}

fn bake_chunk_triangle_layers(
    terrain_map: &image::RgbaImage,
    layer_rules: &[TerrainLayerRule],
    terrain_bounds: (f32, f32, f32, f32),
    chunk_size: f32,
    coord: perro_terrain::ChunkCoord,
    chunk: &TerrainChunk,
) -> Vec<i32> {
    let mut out = Vec::with_capacity(chunk.triangles().len());
    for tri in chunk.triangles() {
        if tri.a >= chunk.vertices().len() || tri.b >= chunk.vertices().len() || tri.c >= chunk.vertices().len() {
            out.push(-1);
            continue;
        }
        let va = chunk.vertices()[tri.a].position;
        let vb = chunk.vertices()[tri.b].position;
        let vc = chunk.vertices()[tri.c].position;
        let world_x = coord.x as f32 * chunk_size + (va.x + vb.x + vc.x) / 3.0;
        let world_z = coord.z as f32 * chunk_size + (va.z + vb.z + vc.z) / 3.0;
        let layer = classify_layer_from_world_xz(terrain_map, layer_rules, terrain_bounds, world_x, world_z)
            .map(|idx| idx as i32)
            .unwrap_or(-1);
        out.push(layer);
    }
    out
}

fn classify_layer_from_world_xz(
    terrain_map: &image::RgbaImage,
    layer_rules: &[TerrainLayerRule],
    terrain_bounds: (f32, f32, f32, f32),
    world_x: f32,
    world_z: f32,
) -> Option<usize> {
    let (min_x, max_x, min_z, max_z) = terrain_bounds;
    let span_x = (max_x - min_x).max(1.0e-3);
    let span_z = (max_z - min_z).max(1.0e-3);
    let u = ((world_x - min_x) / span_x).clamp(0.0, 1.0);
    let v = ((world_z - min_z) / span_z).clamp(0.0, 1.0);
    let x = (u * terrain_map.width().saturating_sub(1) as f32).round() as u32;
    let y = (v * terrain_map.height().saturating_sub(1) as f32).round() as u32;
    let pixel = *terrain_map.get_pixel(x, y);
    layer_rules
        .iter()
        .enumerate()
        .find_map(|(i, rule)| terrain_layer_color_matches(pixel, rule).then_some(i))
}

fn hash_layer_rules(hasher: &mut DefaultHasher, rules: &[TerrainLayerRule]) {
    rules.len().hash(hasher);
    for rule in rules {
        rule.index.hash(hasher);
        rule.color.r.hash(hasher);
        rule.color.g.hash(hasher);
        rule.color.b.hash(hasher);
        rule.color_tolerance.hash(hasher);
        rule.name.as_deref().unwrap_or("").hash(hasher);
        rule.texture_source.as_deref().unwrap_or("").hash(hasher);
        rule.texture_tile_meters.to_bits().hash(hasher);
        rule.texture_rotation_degrees.to_bits().hash(hasher);
        rule.texture_hard_cut.hash(hasher);
        for b in &rule.blend_with {
            b.hash(hasher);
        }
        rule.friction.unwrap_or(-1.0).to_bits().hash(hasher);
        rule.restitution.unwrap_or(-1.0).to_bits().hash(hasher);
    }
}

fn terrain_layer_color_matches(pixel: image::Rgba<u8>, rule: &TerrainLayerRule) -> bool {
    let dr = (pixel[0] as i16 - rule.color.r as i16).unsigned_abs() as u8;
    let dg = (pixel[1] as i16 - rule.color.g as i16).unsigned_abs() as u8;
    let db = (pixel[2] as i16 - rule.color.b as i16).unsigned_abs() as u8;
    let tol = rule.color_tolerance;
    dr <= tol && dg <= tol && db <= tol
}

fn encode_ptex_from_rgba(image: &RgbaImage) -> io::Result<Vec<u8>> {
    let (width, height) = image.dimensions();
    let raw = image.as_raw();
    let compressed = compress_zlib_best(raw)?;
    let mut out = Vec::with_capacity(20 + compressed.len());
    out.extend_from_slice(b"PTEX");
    out.extend_from_slice(&1u32.to_le_bytes());
    out.extend_from_slice(&width.to_le_bytes());
    out.extend_from_slice(&height.to_le_bytes());
    out.extend_from_slice(&(raw.len() as u32).to_le_bytes());
    out.extend_from_slice(&compressed);
    Ok(out)
}

fn optimize_terrain_meshes(input: &TerrainData) -> Result<TerrainData, StaticPipelineError> {
    let mut out = TerrainData::new(input.chunk_size_meters());
    let chunk_size = input.chunk_size_meters();
    for (coord, chunk) in input.chunks() {
        let vertices = chunk
            .vertices()
            .iter()
            .map(|v| [v.position.x, v.position.y, v.position.z])
            .collect::<Vec<_>>();
        let triangles = chunk
            .triangles()
            .iter()
            .map(|t| [t.a, t.b, t.c])
            .collect::<Vec<_>>();

        let Some((vertices, triangles)) = optimize_chunk_mesh(vertices, triangles) else {
            continue;
        };

        let chunk = TerrainChunk::from_mesh(
            coord,
            ChunkConfig::new(chunk_size),
            vertices
                .into_iter()
                .map(|v| Vertex::new(Vector3::new(v[0], v[1], v[2])))
                .collect(),
            triangles
                .into_iter()
                .map(|t| Triangle::new(t[0], t[1], t[2]))
                .collect(),
        )
        .map_err(|err| io::Error::other(format!("terrain mesh optimization produced invalid chunk: {err:?}")))?;
        out.set_chunk(coord, chunk);
    }
    Ok(out)
}

fn optimize_chunk_mesh(
    vertices: Vec<[f32; 3]>,
    triangles: Vec<[usize; 3]>,
) -> Option<(Vec<[f32; 3]>, Vec<[usize; 3]>)> {
    if vertices.len() < 3 || triangles.is_empty() {
        return None;
    }

    let mut filtered = Vec::<[usize; 3]>::new();
    let mut seen = HashSet::<(usize, usize, usize)>::new();
    for tri in triangles {
        let [a, b, c] = tri;
        if a >= vertices.len() || b >= vertices.len() || c >= vertices.len() {
            continue;
        }
        if a == b || b == c || a == c {
            continue;
        }
        if triangle_area(vertices[a], vertices[b], vertices[c]) <= HEIGHTFIELD_EPSILON {
            continue;
        }
        let mut key = [a, b, c];
        key.sort_unstable();
        let key = (key[0], key[1], key[2]);
        if seen.insert(key) {
            filtered.push([a, b, c]);
        }
    }
    if filtered.is_empty() {
        return None;
    }

    let filtered = collapse_horizontal_coplanar_rect_regions(&vertices, filtered);
    let (vertices, triangles) = compact_vertices(vertices, filtered)?;
    let (vertices, triangles) = dedupe_vertices(vertices, triangles)?;

    if let Some(flattened) = collapse_if_coplanar_xz_rect(&vertices) {
        return Some(flattened);
    }

    Some((vertices, triangles))
}

fn collapse_horizontal_coplanar_rect_regions(
    vertices: &[[f32; 3]],
    triangles: Vec<[usize; 3]>,
) -> Vec<[usize; 3]> {
    if triangles.len() < 4 {
        return triangles;
    }

    let mut edge_to_tris = HashMap::<(usize, usize), Vec<usize>>::new();
    for (i, tri) in triangles.iter().enumerate() {
        for (a, b) in tri_edges(*tri) {
            edge_to_tris.entry(edge_key(a, b)).or_default().push(i);
        }
    }

    let mut neighbors = vec![Vec::<usize>::new(); triangles.len()];
    for tri_ids in edge_to_tris.values() {
        if tri_ids.len() < 2 {
            continue;
        }
        for a in 0..tri_ids.len() {
            for b in (a + 1)..tri_ids.len() {
                let ia = tri_ids[a];
                let ib = tri_ids[b];
                neighbors[ia].push(ib);
                neighbors[ib].push(ia);
            }
        }
    }

    let mut visited = vec![false; triangles.len()];
    let mut remove = vec![false; triangles.len()];
    let mut additions = Vec::<[usize; 3]>::new();

    for start in 0..triangles.len() {
        if visited[start] {
            continue;
        }
        let tri0 = triangles[start];
        let Some((plane_n, plane_d)) = plane_from_triangle(vertices, tri0) else {
            visited[start] = true;
            continue;
        };
        let mut stack = vec![start];
        let mut component = Vec::<usize>::new();
        visited[start] = true;
        while let Some(cur) = stack.pop() {
            component.push(cur);
            for &n in &neighbors[cur] {
                if !visited[n] && triangle_matches_plane(vertices, triangles[n], plane_n, plane_d) {
                    visited[n] = true;
                    stack.push(n);
                }
            }
        }
        if component.len() < 2 {
            continue;
        }
        if let Some(replacement) = collapse_rect_component(vertices, &triangles, &component) {
            for &ix in &component {
                remove[ix] = true;
            }
            additions.extend(replacement);
        }
    }

    if !remove.iter().any(|v| *v) {
        return triangles;
    }

    let mut out = Vec::<[usize; 3]>::with_capacity(triangles.len() + additions.len());
    for (i, tri) in triangles.into_iter().enumerate() {
        if !remove[i] {
            out.push(tri);
        }
    }
    out.extend(additions);
    out
}

fn collapse_rect_component(
    vertices: &[[f32; 3]],
    triangles: &[[usize; 3]],
    component: &[usize],
) -> Option<Vec<[usize; 3]>> {
    let mut edge_counts = HashMap::<(usize, usize), u32>::new();
    let mut normal_sum = [0.0f32; 3];

    for &tri_ix in component {
        let tri = triangles[tri_ix];
        for (a, b) in tri_edges(tri) {
            *edge_counts.entry(edge_key(a, b)).or_default() += 1;
        }
        let a = vertices[tri[0]];
        let b = vertices[tri[1]];
        let c = vertices[tri[2]];
        let n = triangle_normal(a, b, c);
        normal_sum[0] += n[0];
        normal_sum[1] += n[1];
        normal_sum[2] += n[2];
    }
    if vec3_len(normal_sum) <= 1.0e-8 {
        return None;
    }

    let boundary_edges: Vec<(usize, usize)> = edge_counts
        .into_iter()
        .filter_map(|(edge, count)| (count == 1).then_some(edge))
        .collect();
    if boundary_edges.len() < 4 {
        return None;
    }

    let mut boundary_vertices = HashSet::<usize>::new();
    for (a, b) in &boundary_edges {
        boundary_vertices.insert(*a);
        boundary_vertices.insert(*b);
    }
    if boundary_vertices.len() < 4 {
        return None;
    }

    let mut points = Vec::<(f32, f32, usize)>::new();
    let mut uniq = HashMap::<(i64, i64), usize>::new();
    let inv = 1.0 / POSITION_QUANT_EPSILON.max(1.0e-6);
    for &vid in &boundary_vertices {
        let v = vertices[vid];
        let key = ((v[0] * inv).round() as i64, (v[2] * inv).round() as i64);
        uniq.entry(key).or_insert_with(|| {
            points.push((v[0], v[2], vid));
            vid
        });
    }
    if points.len() < 4 {
        return None;
    }

    let hull = convex_hull_indices(&points);
    if hull.len() != 4 {
        return None;
    }

    let mut min_x = f32::INFINITY;
    let mut max_x = f32::NEG_INFINITY;
    let mut min_z = f32::INFINITY;
    let mut max_z = f32::NEG_INFINITY;
    for &h in &hull {
        let (x, z, _) = points[h];
        min_x = min_x.min(x);
        max_x = max_x.max(x);
        min_z = min_z.min(z);
        max_z = max_z.max(z);
    }
    if (max_x - min_x) <= HEIGHTFIELD_EPSILON || (max_z - min_z) <= HEIGHTFIELD_EPSILON {
        return None;
    }

    for &(x, z, _) in &points {
        if x < min_x - HEIGHTFIELD_EPSILON
            || x > max_x + HEIGHTFIELD_EPSILON
            || z < min_z - HEIGHTFIELD_EPSILON
            || z > max_z + HEIGHTFIELD_EPSILON
        {
            return None;
        }
        let on_x = nearly_eq(x, min_x) || nearly_eq(x, max_x);
        let on_z = nearly_eq(z, min_z) || nearly_eq(z, max_z);
        if !on_x && !on_z {
            return None;
        }
    }

    let c00 = find_vertex_by_xz(vertices, &boundary_vertices, min_x, min_z)?;
    let c10 = find_vertex_by_xz(vertices, &boundary_vertices, max_x, min_z)?;
    let c11 = find_vertex_by_xz(vertices, &boundary_vertices, max_x, max_z)?;
    let c01 = find_vertex_by_xz(vertices, &boundary_vertices, min_x, max_z)?;

    if c00 == c10 || c10 == c11 || c11 == c01 || c00 == c11 || c10 == c01 || c00 == c01 {
        return None;
    }

    let opt_a = vec![[c00, c10, c11], [c00, c11, c01]];
    let opt_b = vec![[c00, c11, c10], [c00, c01, c11]];
    let score = |tris: &[[usize; 3]]| -> f32 {
        tris.iter()
            .map(|tri| {
                let n = triangle_normal(vertices[tri[0]], vertices[tri[1]], vertices[tri[2]]);
                dot3(n, normal_sum)
            })
            .sum()
    };
    let tris = if score(&opt_a) >= score(&opt_b) {
        opt_a
    } else {
        opt_b
    };

    let Some((plane_n, plane_d)) = plane_from_triangle(vertices, triangles[component[0]]) else {
        return None;
    };
    for tri in &tris {
        if triangle_area(vertices[tri[0]], vertices[tri[1]], vertices[tri[2]]) <= HEIGHTFIELD_EPSILON {
            return None;
        }
        if !triangle_matches_plane(vertices, *tri, plane_n, plane_d) {
            return None;
        }
    }

    Some(tris)
}

fn plane_from_triangle(vertices: &[[f32; 3]], tri: [usize; 3]) -> Option<([f32; 3], f32)> {
    let a = vertices[tri[0]];
    let b = vertices[tri[1]];
    let c = vertices[tri[2]];
    let n = triangle_normal(a, b, c);
    let len = vec3_len(n);
    if len <= 1.0e-8 {
        return None;
    }
    let inv = 1.0 / len;
    let nn = [n[0] * inv, n[1] * inv, n[2] * inv];
    let d = -dot3(nn, a);
    Some((nn, d))
}

fn triangle_matches_plane(
    vertices: &[[f32; 3]],
    tri: [usize; 3],
    plane_n: [f32; 3],
    plane_d: f32,
) -> bool {
    let a = vertices[tri[0]];
    let b = vertices[tri[1]];
    let c = vertices[tri[2]];
    let n = triangle_normal(a, b, c);
    let len = vec3_len(n);
    if len <= 1.0e-8 {
        return false;
    }
    let inv = 1.0 / len;
    let nn = [n[0] * inv, n[1] * inv, n[2] * inv];
    if dot3(nn, plane_n).abs() < 0.999 {
        return false;
    }
    let dist = |p: [f32; 3]| (dot3(plane_n, p) + plane_d).abs();
    dist(a) <= HEIGHTFIELD_EPSILON
        && dist(b) <= HEIGHTFIELD_EPSILON
        && dist(c) <= HEIGHTFIELD_EPSILON
}

fn triangle_normal(a: [f32; 3], b: [f32; 3], c: [f32; 3]) -> [f32; 3] {
    let ab = [b[0] - a[0], b[1] - a[1], b[2] - a[2]];
    let ac = [c[0] - a[0], c[1] - a[1], c[2] - a[2]];
    [
        ab[1] * ac[2] - ab[2] * ac[1],
        ab[2] * ac[0] - ab[0] * ac[2],
        ab[0] * ac[1] - ab[1] * ac[0],
    ]
}

fn tri_edges(tri: [usize; 3]) -> [(usize, usize); 3] {
    [(tri[0], tri[1]), (tri[1], tri[2]), (tri[2], tri[0])]
}

fn edge_key(a: usize, b: usize) -> (usize, usize) {
    if a <= b { (a, b) } else { (b, a) }
}

fn nearly_eq(a: f32, b: f32) -> bool {
    (a - b).abs() <= HEIGHTFIELD_EPSILON
}

fn find_vertex_by_xz(
    vertices: &[[f32; 3]],
    pool: &HashSet<usize>,
    x: f32,
    z: f32,
) -> Option<usize> {
    pool.iter().copied().find(|&vid| {
        let v = vertices[vid];
        nearly_eq(v[0], x) && nearly_eq(v[2], z)
    })
}

fn convex_hull_indices(points: &[(f32, f32, usize)]) -> Vec<usize> {
    if points.len() < 3 {
        return Vec::new();
    }
    let mut ids: Vec<usize> = (0..points.len()).collect();
    ids.sort_by(|&a, &b| {
        points[a]
            .0
            .total_cmp(&points[b].0)
            .then_with(|| points[a].1.total_cmp(&points[b].1))
    });

    let mut lower: Vec<usize> = Vec::new();
    for &i in &ids {
        while lower.len() >= 2 {
            let l = lower.len();
            let a = lower[l - 2];
            let b = lower[l - 1];
            if orient2d(points[a], points[b], points[i]) <= 0.0 {
                lower.pop();
            } else {
                break;
            }
        }
        lower.push(i);
    }

    let mut upper: Vec<usize> = Vec::new();
    for &i in ids.iter().rev() {
        while upper.len() >= 2 {
            let l = upper.len();
            let a = upper[l - 2];
            let b = upper[l - 1];
            if orient2d(points[a], points[b], points[i]) <= 0.0 {
                upper.pop();
            } else {
                break;
            }
        }
        upper.push(i);
    }

    if !lower.is_empty() {
        lower.pop();
    }
    if !upper.is_empty() {
        upper.pop();
    }
    lower.extend(upper);
    lower
}

fn orient2d(a: (f32, f32, usize), b: (f32, f32, usize), c: (f32, f32, usize)) -> f32 {
    (b.0 - a.0) * (c.1 - a.1) - (b.1 - a.1) * (c.0 - a.0)
}

fn dot3(a: [f32; 3], b: [f32; 3]) -> f32 {
    a[0] * b[0] + a[1] * b[1] + a[2] * b[2]
}

fn vec3_len(v: [f32; 3]) -> f32 {
    (v[0] * v[0] + v[1] * v[1] + v[2] * v[2]).sqrt()
}

fn compact_vertices(
    vertices: Vec<[f32; 3]>,
    triangles: Vec<[usize; 3]>,
) -> Option<(Vec<[f32; 3]>, Vec<[usize; 3]>)> {
    let mut used = vec![false; vertices.len()];
    for [a, b, c] in &triangles {
        used[*a] = true;
        used[*b] = true;
        used[*c] = true;
    }

    let mut remap = vec![usize::MAX; vertices.len()];
    let mut out_vertices = Vec::new();
    for (i, v) in vertices.into_iter().enumerate() {
        if !used[i] {
            continue;
        }
        remap[i] = out_vertices.len();
        out_vertices.push(v);
    }

    let mut out_triangles = Vec::with_capacity(triangles.len());
    for [a, b, c] in triangles {
        let na = remap[a];
        let nb = remap[b];
        let nc = remap[c];
        if na == usize::MAX || nb == usize::MAX || nc == usize::MAX {
            continue;
        }
        if na == nb || nb == nc || na == nc {
            continue;
        }
        out_triangles.push([na, nb, nc]);
    }

    if out_vertices.len() < 3 || out_triangles.is_empty() {
        return None;
    }
    Some((out_vertices, out_triangles))
}

fn dedupe_vertices(
    vertices: Vec<[f32; 3]>,
    triangles: Vec<[usize; 3]>,
) -> Option<(Vec<[f32; 3]>, Vec<[usize; 3]>)> {
    let inv = 1.0 / POSITION_QUANT_EPSILON.max(1.0e-6);
    let mut map = HashMap::<(i64, i64, i64), usize>::new();
    let mut remap = vec![0usize; vertices.len()];
    let mut out_vertices = Vec::<[f32; 3]>::new();

    for (i, v) in vertices.iter().enumerate() {
        let key = (
            (v[0] * inv).round() as i64,
            (v[1] * inv).round() as i64,
            (v[2] * inv).round() as i64,
        );
        let idx = if let Some(existing) = map.get(&key).copied() {
            existing
        } else {
            let idx = out_vertices.len();
            out_vertices.push(*v);
            map.insert(key, idx);
            idx
        };
        remap[i] = idx;
    }

    let mut out_triangles = Vec::with_capacity(triangles.len());
    for [a, b, c] in triangles {
        let na = remap[a];
        let nb = remap[b];
        let nc = remap[c];
        if na == nb || nb == nc || na == nc {
            continue;
        }
        if triangle_area(out_vertices[na], out_vertices[nb], out_vertices[nc]) <= HEIGHTFIELD_EPSILON {
            continue;
        }
        out_triangles.push([na, nb, nc]);
    }

    compact_vertices(out_vertices, out_triangles)
}

fn collapse_if_coplanar_xz_rect(
    vertices: &[[f32; 3]],
) -> Option<(Vec<[f32; 3]>, Vec<[usize; 3]>)> {
    if vertices.len() < 3 {
        return None;
    }
    let a = vertices[0];
    let mut plane: Option<([f32; 3], f32)> = None;
    'outer: for i in 1..vertices.len() {
        for j in (i + 1)..vertices.len() {
            let n = triangle_normal(a, vertices[i], vertices[j]);
            let len = vec3_len(n);
            if len > 1.0e-8 {
                let inv = 1.0 / len;
                let nn = [n[0] * inv, n[1] * inv, n[2] * inv];
                plane = Some((nn, -dot3(nn, a)));
                break 'outer;
            }
        }
    }
    let Some((plane_n, plane_d)) = plane else {
        return None;
    };
    if vertices
        .iter()
        .any(|v| (dot3(plane_n, *v) + plane_d).abs() > HEIGHTFIELD_EPSILON)
    {
        return None;
    }

    let mut min_x = f32::INFINITY;
    let mut max_x = f32::NEG_INFINITY;
    let mut min_z = f32::INFINITY;
    let mut max_z = f32::NEG_INFINITY;
    for v in vertices {
        min_x = min_x.min(v[0]);
        max_x = max_x.max(v[0]);
        min_z = min_z.min(v[2]);
        max_z = max_z.max(v[2]);
    }

    let span_x = max_x - min_x;
    let span_z = max_z - min_z;
    if span_x <= HEIGHTFIELD_EPSILON || span_z <= HEIGHTFIELD_EPSILON {
        return None;
    }

    let c00 = vertices
        .iter()
        .copied()
        .find(|v| nearly_eq(v[0], min_x) && nearly_eq(v[2], min_z))?;
    let c10 = vertices
        .iter()
        .copied()
        .find(|v| nearly_eq(v[0], max_x) && nearly_eq(v[2], min_z))?;
    let c11 = vertices
        .iter()
        .copied()
        .find(|v| nearly_eq(v[0], max_x) && nearly_eq(v[2], max_z))?;
    let c01 = vertices
        .iter()
        .copied()
        .find(|v| nearly_eq(v[0], min_x) && nearly_eq(v[2], max_z))?;

    let verts = vec![c00, c10, c11, c01];
    let t_a = vec![[0usize, 1usize, 2usize], [0usize, 2usize, 3usize]];
    let t_b = vec![[0usize, 2usize, 1usize], [0usize, 3usize, 2usize]];
    let score = |tris: &[[usize; 3]]| -> f32 {
        tris.iter()
            .map(|tri| dot3(triangle_normal(verts[tri[0]], verts[tri[1]], verts[tri[2]]), plane_n))
            .sum()
    };
    let tris = if score(&t_a) >= score(&t_b) { t_a } else { t_b };

    Some((
        verts,
        tris,
    ))
}

fn triangle_area(a: [f32; 3], b: [f32; 3], c: [f32; 3]) -> f32 {
    let ab = [b[0] - a[0], b[1] - a[1], b[2] - a[2]];
    let ac = [c[0] - a[0], c[1] - a[1], c[2] - a[2]];
    let cross = [
        ab[1] * ac[2] - ab[2] * ac[1],
        ab[2] * ac[0] - ab[0] * ac[2],
        ab[0] * ac[1] - ab[1] * ac[0],
    ];
    (cross[0] * cross[0] + cross[1] * cross[1] + cross[2] * cross[2]).sqrt()
}

fn escape_str(s: &str) -> String {
    s.replace('\\', "\\\\")
        .replace('"', "\\\"")
        .replace('\r', "")
        .replace('\n', "\\n")
}
