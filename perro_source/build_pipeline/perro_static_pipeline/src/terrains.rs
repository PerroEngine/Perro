use crate::{StaticPipelineError, res_dir, static_dir};
use glam::{Mat4, Vec3};
use perro_io::walkdir::collect_file_paths;
use std::collections::{BTreeMap, HashMap};
use std::fmt::Write as _;
use std::fs;
use std::io;
use std::path::Path;

const DEFAULT_CHUNK_SIZE_METERS: f32 = 512.0;
const HEIGHTFIELD_EPSILON: f32 = 1.0e-4;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum ChunkMode {
    Unknown,
    Grid,
    Mesh,
}

#[derive(Clone)]
enum ChunkPayload {
    Grid {
        samples: Vec<(f32, f32, f32)>,
    },
    Mesh {
        vertices: Vec<[f32; 3]>,
        triangles: Vec<[usize; 3]>,
    },
}

#[derive(Clone)]
struct ParsedChunk {
    x: i32,
    z: i32,
    payload: ChunkPayload,
}

struct WorkingChunk {
    x: i32,
    z: i32,
    mode: ChunkMode,
    grid_samples: Vec<(f32, f32, f32)>,
    mesh_vertices: Vec<[f32; 3]>,
    mesh_triangles: Vec<[usize; 3]>,
}

impl WorkingChunk {
    fn new(x: i32, z: i32) -> Self {
        Self {
            x,
            z,
            mode: ChunkMode::Unknown,
            grid_samples: Vec::new(),
            mesh_vertices: Vec::new(),
            mesh_triangles: Vec::new(),
        }
    }

    fn add_grid_sample(&mut self, x: f32, z: f32, y: f32) -> Option<()> {
        match self.mode {
            ChunkMode::Unknown | ChunkMode::Grid => self.mode = ChunkMode::Grid,
            ChunkMode::Mesh => return None,
        }
        self.grid_samples.push((x, z, y));
        Some(())
    }

    fn add_mesh_vertex(&mut self, x: f32, y: f32, z: f32) -> Option<()> {
        match self.mode {
            ChunkMode::Unknown | ChunkMode::Mesh => self.mode = ChunkMode::Mesh,
            ChunkMode::Grid => return None,
        }
        self.mesh_vertices.push([x, y, z]);
        Some(())
    }

    fn add_mesh_triangle(&mut self, a: usize, b: usize, c: usize) -> Option<()> {
        match self.mode {
            ChunkMode::Unknown | ChunkMode::Mesh => self.mode = ChunkMode::Mesh,
            ChunkMode::Grid => return None,
        }
        self.mesh_triangles.push([a, b, c]);
        Some(())
    }

    fn finish(self) -> Option<ParsedChunk> {
        let payload = match self.mode {
            ChunkMode::Unknown | ChunkMode::Grid => ChunkPayload::Grid {
                samples: self.grid_samples,
            },
            ChunkMode::Mesh => {
                if self.mesh_vertices.len() < 3 || self.mesh_triangles.is_empty() {
                    return None;
                }
                if !mesh_is_single_height_per_xz(&self.mesh_vertices, HEIGHTFIELD_EPSILON) {
                    return None;
                }
                ChunkPayload::Mesh {
                    vertices: self.mesh_vertices,
                    triangles: self.mesh_triangles,
                }
            }
        };
        Some(ParsedChunk {
            x: self.x,
            z: self.z,
            payload,
        })
    }
}

struct TerrainFolderEntry {
    // higher priority replaces lower: glTF authoring source > raw ptchunk files
    priority: u8,
    chunks: Vec<ParsedChunk>,
}

pub fn generate_static_terrains(project_root: &Path) -> Result<(), StaticPipelineError> {
    let res_dir = res_dir(project_root);
    let static_dir = static_dir(project_root);
    fs::create_dir_all(&static_dir)?;

    let mut rel_paths = if res_dir.exists() {
        collect_file_paths(&res_dir, &res_dir)?
            .into_iter()
            .map(|rel| rel.replace('\\', "/"))
            .collect::<Vec<_>>()
    } else {
        Vec::new()
    };
    rel_paths.sort();

    let mut folders = BTreeMap::<String, TerrainFolderEntry>::new();

    for rel in rel_paths.iter().filter(|rel| {
        Path::new(rel)
            .extension()
            .and_then(|e| e.to_str())
            .is_some_and(|ext| ext.eq_ignore_ascii_case("ptchunk"))
    }) {
        let full = res_dir.join(rel);
        let src = fs::read_to_string(&full)?;
        let hint =
            coord_from_ptchunk_stem(Path::new(rel).file_stem().and_then(|s| s.to_str()))
                .ok_or_else(|| {
                    io::Error::other(format!(
                        "invalid .ptchunk filename `res://{rel}`; expected `<chunk_x>_<chunk_z>.ptchunk`"
                    ))
                })?;
        let mut chunks = parse_ptchunk_source(&src, Some(hint)).ok_or_else(|| {
            io::Error::other(format!("failed to parse terrain .ptchunk `res://{rel}`"))
        })?;
        chunks.sort_by_key(|c| (c.x, c.z));

        let folder = terrain_source_for_chunk_path(rel);
        upsert_terrain_folder(&mut folders, folder, 1, &mut chunks);
    }

    for rel in rel_paths.iter().filter(|rel| is_terrain_gltf_path(rel)) {
        let full = res_dir.join(rel);
        let mut chunks = import_gltf_terrain_chunks(&full).map_err(|err| {
            io::Error::other(format!("failed to parse terrain gltf `res://{rel}`: {err}"))
        })?;
        chunks.sort_by_key(|c| (c.x, c.z));

        let folder = terrain_source_for_chunk_path(rel);
        upsert_terrain_folder(&mut folders, folder, 2, &mut chunks);
    }

    let mut out = String::new();
    out.push_str("// Auto-generated by Perro Static Pipeline. Do not edit.\n");
    out.push_str("#![allow(unused_imports)]\n\n");

    let mut folder_rows: Vec<(String, Vec<ParsedChunk>)> = folders
        .into_iter()
        .map(|(folder, entry)| (folder, entry.chunks))
        .collect();
    folder_rows.sort_by(|a, b| a.0.cmp(&b.0));

    for (i, (_folder, chunks)) in folder_rows.iter().enumerate() {
        let const_name = format!("TERRAIN_{}", i);
        let literal = build_terrain_literal(chunks);
        let _ = writeln!(
            out,
            "static {const_name}: &str = \"{}\";",
            escape_str(&literal)
        );
    }

    out.push('\n');
    out.push_str("pub fn lookup_terrain(path: &str) -> Option<&'static str> {\n");
    out.push_str("    match path {\n");
    for (i, (folder, _)) in folder_rows.iter().enumerate() {
        let _ = writeln!(
            out,
            "        \"{}\" => Some(TERRAIN_{}),",
            escape_str(folder),
            i
        );
        let slash = if folder.ends_with('/') {
            folder.clone()
        } else {
            format!("{folder}/")
        };
        let _ = writeln!(
            out,
            "        \"{}\" => Some(TERRAIN_{}),",
            escape_str(&slash),
            i
        );
    }
    out.push_str("        _ => None,\n");
    out.push_str("    }\n");
    out.push_str("}\n");

    fs::write(static_dir.join("terrains.rs"), out)?;
    Ok(())
}

fn upsert_terrain_folder(
    folders: &mut BTreeMap<String, TerrainFolderEntry>,
    folder: String,
    priority: u8,
    chunks: &mut Vec<ParsedChunk>,
) {
    if let Some(existing) = folders.get_mut(&folder) {
        if priority < existing.priority {
            return;
        }
        if priority > existing.priority {
            existing.priority = priority;
            existing.chunks.clear();
        }
        existing.chunks.append(chunks);
        return;
    }
    folders.insert(
        folder,
        TerrainFolderEntry {
            priority,
            chunks: std::mem::take(chunks),
        },
    );
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

fn build_terrain_literal(chunks: &[ParsedChunk]) -> String {
    let mut out = String::new();
    for chunk in chunks {
        let _ = writeln!(out, "chunk = [{}, {}]", chunk.x, chunk.z);
        match &chunk.payload {
            ChunkPayload::Grid { samples } => {
                let mut sorted = samples.clone();
                sorted.sort_by(|a, b| a.1.total_cmp(&b.1).then_with(|| a.0.total_cmp(&b.0)));
                for (x, z, y) in sorted {
                    let _ = writeln!(out, "[{x:.6},{z:.6}] = {y:.6}");
                }
            }
            ChunkPayload::Mesh {
                vertices,
                triangles,
            } => {
                for v in vertices {
                    let _ = writeln!(out, "vertex = [{:.6},{:.6},{:.6}]", v[0], v[1], v[2]);
                }
                for tri in triangles {
                    let _ = writeln!(out, "tri = [{},{},{}]", tri[0], tri[1], tri[2]);
                }
            }
        }
        out.push('\n');
    }
    out
}

fn parse_ptchunk_source(
    source: &str,
    default_coord: Option<(i32, i32)>,
) -> Option<Vec<ParsedChunk>> {
    let mut current = default_coord;
    let mut chunks = Vec::<WorkingChunk>::new();
    if let Some((x, z)) = current {
        chunks.push(WorkingChunk::new(x, z));
    }

    for raw in source.lines() {
        let line = strip_line_comment(raw).trim();
        if line.is_empty() {
            continue;
        }

        if let Some((x, z)) = parse_coord_declaration(line) {
            current = Some((x, z));
            if !chunks.iter().any(|c| c.x == x && c.z == z) {
                chunks.push(WorkingChunk::new(x, z));
            }
            continue;
        }

        if let Some((gx, gz, y)) = parse_grid_sample_line(line) {
            let (cx, cz) = current?;
            let chunk = chunks
                .iter_mut()
                .find(|c| c.x == cx && c.z == cz)
                .expect("chunk exists");
            chunk.add_grid_sample(gx, gz, y)?;
            continue;
        }

        if let Some((x, y, z)) = parse_mesh_vertex_line(line) {
            let (cx, cz) = current?;
            let chunk = chunks
                .iter_mut()
                .find(|c| c.x == cx && c.z == cz)
                .expect("chunk exists");
            chunk.add_mesh_vertex(x, y, z)?;
            continue;
        }

        if let Some((a, b, c)) = parse_mesh_triangle_line(line) {
            let (cx, cz) = current?;
            let chunk = chunks
                .iter_mut()
                .find(|c| c.x == cx && c.z == cz)
                .expect("chunk exists");
            chunk.add_mesh_triangle(a, b, c)?;
            continue;
        }

        return None;
    }

    if chunks.is_empty() {
        return None;
    }
    let mut out = Vec::with_capacity(chunks.len());
    for chunk in chunks {
        out.push(chunk.finish()?);
    }
    Some(out)
}

fn parse_coord_declaration(line: &str) -> Option<(i32, i32)> {
    let trimmed = line.trim();
    let lower = trimmed.to_ascii_lowercase();
    if !lower.starts_with("coord") && !lower.starts_with("chunk") {
        return None;
    }
    let bracket_start = trimmed.find('[')?;
    let bracket_end = trimmed[bracket_start..].find(']')? + bracket_start;
    parse_pair_i32(&trimmed[(bracket_start + 1)..bracket_end])
}

fn parse_grid_sample_line(line: &str) -> Option<(f32, f32, f32)> {
    let trimmed = line.trim();
    if !trimmed.starts_with('[') {
        return None;
    }
    let close = trimmed.find(']')?;
    let (x, z) = parse_pair_f32(&trimmed[1..close])?;
    let mut rest = trimmed[(close + 1)..].trim();
    if let Some(after_eq) = rest.strip_prefix('=') {
        rest = after_eq.trim();
    }
    let y = rest.parse::<f32>().ok()?;
    Some((x, z, y))
}

fn parse_mesh_vertex_line(line: &str) -> Option<(f32, f32, f32)> {
    let trimmed = line.trim();
    let lower = trimmed.to_ascii_lowercase();
    if !lower.starts_with("vertex") && !lower.starts_with("vtx") && !lower.starts_with('v') {
        return None;
    }
    let bracket_start = trimmed.find('[')?;
    let bracket_end = trimmed[bracket_start..].find(']')? + bracket_start;
    parse_triplet_f32(&trimmed[(bracket_start + 1)..bracket_end])
}

fn parse_mesh_triangle_line(line: &str) -> Option<(usize, usize, usize)> {
    let trimmed = line.trim();
    let lower = trimmed.to_ascii_lowercase();
    if !lower.starts_with("triangle") && !lower.starts_with("tri") && !lower.starts_with("face") {
        return None;
    }
    let bracket_start = trimmed.find('[')?;
    let bracket_end = trimmed[bracket_start..].find(']')? + bracket_start;
    let (a_i32, b_i32, c_i32) = parse_triplet_i32(&trimmed[(bracket_start + 1)..bracket_end])?;
    Some((
        usize::try_from(a_i32).ok()?,
        usize::try_from(b_i32).ok()?,
        usize::try_from(c_i32).ok()?,
    ))
}

fn parse_pair_i32(text: &str) -> Option<(i32, i32)> {
    let mut parts = text.split(',').map(str::trim);
    let x = parts.next()?.parse::<i32>().ok()?;
    let z = parts.next()?.parse::<i32>().ok()?;
    if parts.next().is_some() {
        return None;
    }
    Some((x, z))
}

fn parse_pair_f32(text: &str) -> Option<(f32, f32)> {
    let mut parts = text.split(',').map(str::trim);
    let x = parts.next()?.parse::<f32>().ok()?;
    let z = parts.next()?.parse::<f32>().ok()?;
    if parts.next().is_some() {
        return None;
    }
    Some((x, z))
}

fn parse_triplet_i32(text: &str) -> Option<(i32, i32, i32)> {
    let mut parts = text.split(',').map(str::trim);
    let x = parts.next()?.parse::<i32>().ok()?;
    let y = parts.next()?.parse::<i32>().ok()?;
    let z = parts.next()?.parse::<i32>().ok()?;
    if parts.next().is_some() {
        return None;
    }
    Some((x, y, z))
}

fn parse_triplet_f32(text: &str) -> Option<(f32, f32, f32)> {
    let mut parts = text.split(',').map(str::trim);
    let x = parts.next()?.parse::<f32>().ok()?;
    let y = parts.next()?.parse::<f32>().ok()?;
    let z = parts.next()?.parse::<f32>().ok()?;
    if parts.next().is_some() {
        return None;
    }
    Some((x, y, z))
}

fn strip_line_comment(line: &str) -> &str {
    let no_hash = line.split('#').next().unwrap_or(line);
    no_hash.split("//").next().unwrap_or(no_hash)
}

fn coord_from_ptchunk_stem(stem: Option<&str>) -> Option<(i32, i32)> {
    let stem = stem?;
    parse_chunk_space_name(stem)
}

fn parse_chunk_space_name(stem: &str) -> Option<(i32, i32)> {
    let (x_text, z_text) = stem.split_once('_')?;
    if x_text.is_empty() || z_text.is_empty() || z_text.contains('_') {
        return None;
    }
    let x = parse_strict_i32(x_text)?;
    let z = parse_strict_i32(z_text)?;
    Some((x, z))
}

fn parse_strict_i32(text: &str) -> Option<i32> {
    let mut chars = text.chars();
    let first = chars.next()?;
    if first == '-' {
        if chars.clone().next().is_none() {
            return None;
        }
    } else if !first.is_ascii_digit() {
        return None;
    }
    if !chars.all(|ch| ch.is_ascii_digit()) {
        return None;
    }
    text.parse::<i32>().ok()
}

fn import_gltf_terrain_chunks(path: &Path) -> io::Result<Vec<ParsedChunk>> {
    let (doc, buffers, _images) = gltf::import(path).map_err(io::Error::other)?;
    let scene = doc
        .default_scene()
        .or_else(|| doc.scenes().next())
        .ok_or_else(|| io::Error::other("glTF has no scene"))?;

    let mut positions = Vec::<Vec3>::new();
    let mut triangles = Vec::<[u32; 3]>::new();
    for node in scene.nodes() {
        collect_node_meshes(
            &node,
            Mat4::IDENTITY,
            &buffers,
            &mut positions,
            &mut triangles,
        );
    }

    if positions.len() < 3 || triangles.is_empty() {
        return Err(io::Error::other("no mesh triangles found in glTF terrain"));
    }

    let chunked = chunk_mesh_positions(&positions, &triangles)?;
    let mut out = Vec::new();
    for ((cx, cz), chunk) in chunked {
        out.push(ParsedChunk {
            x: cx,
            z: cz,
            payload: ChunkPayload::Mesh {
                vertices: chunk.vertices,
                triangles: chunk.triangles,
            },
        });
    }
    if out.is_empty() {
        return Err(io::Error::other("glTF terrain produced no valid chunks"));
    }
    Ok(out)
}

fn collect_node_meshes(
    node: &gltf::Node,
    parent: Mat4,
    buffers: &[gltf::buffer::Data],
    positions: &mut Vec<Vec3>,
    triangles: &mut Vec<[u32; 3]>,
) {
    let local = Mat4::from_cols_array_2d(&node.transform().matrix());
    let world = parent * local;

    if let Some(mesh) = node.mesh() {
        for primitive in mesh.primitives() {
            let reader =
                primitive.reader(|buffer| buffers.get(buffer.index()).map(|d| d.0.as_slice()));
            let Some(pos_iter) = reader.read_positions() else {
                continue;
            };
            let local_positions: Vec<Vec3> = pos_iter.map(Vec3::from_array).collect();
            if local_positions.len() < 3 {
                continue;
            }

            let base = positions.len() as u32;
            for p in &local_positions {
                positions.push(world.transform_point3(*p));
            }

            if let Some(indices_reader) = reader.read_indices() {
                let mut flat: Vec<u32> = indices_reader.into_u32().collect();
                let tri_len = flat.len() / 3 * 3;
                flat.truncate(tri_len);
                for tri in flat.chunks_exact(3) {
                    let a = base + tri[0];
                    let b = base + tri[1];
                    let c = base + tri[2];
                    if a != b && b != c && a != c {
                        triangles.push([a, b, c]);
                    }
                }
            } else {
                let tri_len = local_positions.len() / 3 * 3;
                for i in (0..tri_len).step_by(3) {
                    triangles.push([base + i as u32, base + i as u32 + 1, base + i as u32 + 2]);
                }
            }
        }
    }

    for child in node.children() {
        collect_node_meshes(&child, world, buffers, positions, triangles);
    }
}

struct MeshChunkBuild {
    vertices: Vec<[f32; 3]>,
    triangles: Vec<[usize; 3]>,
    global_to_local: HashMap<u32, usize>,
    xz_to_height: HashMap<(i64, i64), f32>,
}

fn chunk_mesh_positions(
    positions: &[Vec3],
    triangles: &[[u32; 3]],
) -> io::Result<BTreeMap<(i32, i32), MeshChunkBuild>> {
    let mut out = BTreeMap::<(i32, i32), MeshChunkBuild>::new();
    let quant = 1.0 / HEIGHTFIELD_EPSILON.max(1.0e-4);

    for tri in triangles {
        let ia = usize::try_from(tri[0]).map_err(io::Error::other)?;
        let ib = usize::try_from(tri[1]).map_err(io::Error::other)?;
        let ic = usize::try_from(tri[2]).map_err(io::Error::other)?;
        let (Some(a), Some(b), Some(c)) = (positions.get(ia), positions.get(ib), positions.get(ic))
        else {
            continue;
        };

        let centroid = (*a + *b + *c) / 3.0;
        let coord = world_to_chunk_coord(centroid.x, centroid.z, DEFAULT_CHUNK_SIZE_METERS);
        let center_x = coord.0 as f32 * DEFAULT_CHUNK_SIZE_METERS;
        let center_z = coord.1 as f32 * DEFAULT_CHUNK_SIZE_METERS;

        let builder = out.entry(coord).or_insert_with(|| MeshChunkBuild {
            vertices: Vec::new(),
            triangles: Vec::new(),
            global_to_local: HashMap::new(),
            xz_to_height: HashMap::new(),
        });

        let mut local = [0usize; 3];
        for (i, global_idx) in tri.iter().enumerate() {
            if let Some(existing) = builder.global_to_local.get(global_idx).copied() {
                local[i] = existing;
                continue;
            }

            let g = positions
                .get(usize::try_from(*global_idx).map_err(io::Error::other)?)
                .ok_or_else(|| io::Error::other("triangle references out-of-range vertex"))?;
            let lx = g.x - center_x;
            let ly = g.y;
            let lz = g.z - center_z;

            let key = ((lx * quant).round() as i64, (lz * quant).round() as i64);
            if let Some(existing_y) = builder.xz_to_height.get(&key)
                && (*existing_y - ly).abs() > HEIGHTFIELD_EPSILON
            {
                return Err(io::Error::other(
                    "terrain glTF is not heightfield-like (multiple heights for same xz)",
                ));
            }
            builder.xz_to_height.insert(key, ly);

            let idx = builder.vertices.len();
            builder.vertices.push([lx, ly, lz]);
            builder.global_to_local.insert(*global_idx, idx);
            local[i] = idx;
        }

        if local[0] != local[1] && local[1] != local[2] && local[0] != local[2] {
            builder.triangles.push([local[0], local[1], local[2]]);
        }
    }

    out.retain(|_, chunk| chunk.vertices.len() >= 3 && !chunk.triangles.is_empty());
    if out.is_empty() {
        return Err(io::Error::other("no valid chunk meshes were built"));
    }
    Ok(out)
}

fn world_to_chunk_coord(world_x: f32, world_z: f32, chunk_size_meters: f32) -> (i32, i32) {
    let inv = 1.0 / chunk_size_meters;
    let cx = (world_x * inv + 0.5).floor() as i32;
    let cz = (world_z * inv + 0.5).floor() as i32;
    (cx, cz)
}

fn mesh_is_single_height_per_xz(vertices: &[[f32; 3]], epsilon: f32) -> bool {
    if !epsilon.is_finite() || epsilon < 0.0 {
        return false;
    }
    let mut by_xz = HashMap::<(i64, i64), f32>::new();
    let inv = 1.0 / epsilon.max(1.0e-4);
    for v in vertices {
        let kx = (v[0] * inv).round() as i64;
        let kz = (v[2] * inv).round() as i64;
        let key = (kx, kz);
        if let Some(existing_y) = by_xz.get(&key) {
            if (*existing_y - v[1]).abs() > epsilon {
                return false;
            }
        } else {
            by_xz.insert(key, v[1]);
        }
    }
    true
}

fn escape_str(s: &str) -> String {
    s.replace('\\', "\\\\")
        .replace('"', "\\\"")
        .replace('\r', "")
        .replace('\n', "\\n")
}
