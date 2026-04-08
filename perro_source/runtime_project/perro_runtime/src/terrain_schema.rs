use perro_io::{ResolvedPath, resolve_path};
use perro_structs::Vector3;
use perro_terrain::{CHUNK_GRID_VERTICES_PER_SIDE, ChunkCoord, TerrainData};
use std::fs;
use std::path::{Path, PathBuf};

const HEIGHT_COUNT: usize = CHUNK_GRID_VERTICES_PER_SIDE * CHUNK_GRID_VERTICES_PER_SIDE;

#[derive(Clone)]
struct ParsedChunk {
    coord: ChunkCoord,
    heights: Vec<f32>,
}

struct WorkingChunk {
    coord: ChunkCoord,
    heights: Vec<f32>,
    assigned: Vec<bool>,
    assigned_count: usize,
}

impl WorkingChunk {
    fn new(coord: ChunkCoord) -> Self {
        Self {
            coord,
            heights: vec![0.0; HEIGHT_COUNT],
            assigned: vec![false; HEIGHT_COUNT],
            assigned_count: 0,
        }
    }

    fn set(&mut self, x: usize, z: usize, y: f32) {
        let idx = z * CHUNK_GRID_VERTICES_PER_SIDE + x;
        if !self.assigned[idx] {
            self.assigned[idx] = true;
            self.assigned_count += 1;
        }
        self.heights[idx] = y;
    }

    fn finish(self) -> Option<ParsedChunk> {
        Some(ParsedChunk {
            coord: self.coord,
            heights: self.heights,
        })
    }
}

pub fn load_terrain_literal(source: &str) -> Option<TerrainData> {
    let chunks = parse_terrain_kv(source, None)?;
    build_terrain_data(64.0, &chunks)
}

pub fn load_terrain_from_folder_source(source: &str) -> Option<TerrainData> {
    let source = source.trim();
    if source.is_empty() {
        return None;
    }
    let resolved = resolve_path(source);
    let ResolvedPath::Disk(path) = resolved else {
        return None;
    };
    load_terrain_from_disk_path(&path)
}

fn load_terrain_from_disk_path(path: &Path) -> Option<TerrainData> {
    if path.is_file() {
        let text = fs::read_to_string(path).ok()?;
        let hint = coord_from_ptchunk_path(path)?;
        let chunks = parse_terrain_kv(&text, Some(hint))?;
        return build_terrain_data(64.0, &chunks);
    }
    if !path.is_dir() {
        return None;
    }

    let mut paths = Vec::new();
    collect_ptchunk_paths(path, &mut paths).ok()?;
    paths.sort();

    let mut chunks = Vec::new();
    for chunk_path in paths {
        let text = fs::read_to_string(&chunk_path).ok()?;
        let hint = coord_from_ptchunk_path(&chunk_path)?;
        let mut parsed = parse_terrain_kv(&text, Some(hint))?;
        chunks.append(&mut parsed);
    }
    build_terrain_data(64.0, &chunks)
}

fn collect_ptchunk_paths(dir: &Path, out: &mut Vec<PathBuf>) -> std::io::Result<()> {
    for entry in fs::read_dir(dir)? {
        let entry = entry?;
        let path = entry.path();
        if path.is_dir() {
            collect_ptchunk_paths(&path, out)?;
            continue;
        }
        if path
            .extension()
            .and_then(|e| e.to_str())
            .is_some_and(|ext| ext.eq_ignore_ascii_case("ptchunk"))
        {
            out.push(path);
        }
    }
    Ok(())
}

fn parse_terrain_kv(source: &str, default_coord: Option<ChunkCoord>) -> Option<Vec<ParsedChunk>> {
    let mut working: Vec<WorkingChunk> = Vec::new();
    let mut current_coord = default_coord;

    if let Some(coord) = current_coord {
        ensure_working_chunk(&mut working, coord);
    }

    for raw in source.lines() {
        let line = strip_line_comment(raw).trim();
        if line.is_empty() {
            continue;
        }

        if let Some(coord) = parse_coord_declaration(line) {
            current_coord = Some(coord);
            ensure_working_chunk(&mut working, coord);
            continue;
        }

        let (x, z, y) = parse_sample_line(line)?;
        let coord = current_coord?;
        let chunk = working
            .iter_mut()
            .find(|chunk| chunk.coord == coord)
            .expect("chunk exists after ensure");
        chunk.set(x, z, y);
    }

    if working.is_empty() {
        return None;
    }

    let mut out = Vec::with_capacity(working.len());
    for chunk in working {
        out.push(chunk.finish()?);
    }
    Some(out)
}

fn ensure_working_chunk(chunks: &mut Vec<WorkingChunk>, coord: ChunkCoord) {
    if chunks.iter().any(|chunk| chunk.coord == coord) {
        return;
    }
    chunks.push(WorkingChunk::new(coord));
}

fn parse_coord_declaration(line: &str) -> Option<ChunkCoord> {
    let trimmed = line.trim();
    let lower = trimmed.to_ascii_lowercase();
    if !lower.starts_with("coord") && !lower.starts_with("chunk") {
        return None;
    }

    let bracket_start = trimmed.find('[')?;
    let bracket_end = trimmed[bracket_start..].find(']')? + bracket_start;
    let inside = &trimmed[(bracket_start + 1)..bracket_end];
    let (x, z) = parse_pair_i32(inside)?;
    Some(ChunkCoord::new(x, z))
}

fn parse_sample_line(line: &str) -> Option<(usize, usize, f32)> {
    let trimmed = line.trim();
    let open = trimmed.find('[')?;
    if open != 0 {
        return None;
    }
    let close = trimmed.find(']')?;
    let inside = &trimmed[1..close];
    let (x_i32, z_i32) = parse_pair_i32(inside)?;
    let x = usize::try_from(x_i32).ok()?;
    let z = usize::try_from(z_i32).ok()?;
    if x >= CHUNK_GRID_VERTICES_PER_SIDE || z >= CHUNK_GRID_VERTICES_PER_SIDE {
        return None;
    }

    let mut rest = trimmed[(close + 1)..].trim();
    if let Some(after_eq) = rest.strip_prefix('=') {
        rest = after_eq.trim();
    }
    if rest.is_empty() {
        return None;
    }
    let y = rest.parse::<f32>().ok()?;
    Some((x, z, y))
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

fn strip_line_comment(line: &str) -> &str {
    let no_hash = line.split('#').next().unwrap_or(line);
    no_hash.split("//").next().unwrap_or(no_hash)
}

fn coord_from_ptchunk_path(path: &Path) -> Option<ChunkCoord> {
    let stem = path.file_stem()?.to_string_lossy();
    let (x, z) = parse_chunk_space_name(&stem)?;
    Some(ChunkCoord::new(x, z))
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

fn build_terrain_data(chunk_size_meters: f32, chunks: &[ParsedChunk]) -> Option<TerrainData> {
    if chunks.is_empty() {
        return None;
    }
    let mut terrain = TerrainData::new(chunk_size_meters);
    for chunk in chunks {
        let dst = terrain.ensure_chunk(chunk.coord);
        for (i, y) in chunk.heights.iter().copied().enumerate() {
            let old = dst.vertices()[i].position;
            dst.set_vertex_position(i, Vector3::new(old.x, y, old.z))
                .ok()?;
        }
    }
    Some(terrain)
}
