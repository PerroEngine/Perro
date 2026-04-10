use glam::{Mat4, Vec3};
use perro_io::{ResolvedPath, decompress_zlib, resolve_path};
use perro_structs::Vector3;
use perro_terrain::{
    ChunkConfig, ChunkCoord, DEFAULT_CHUNK_SIZE_METERS, TerrainChunk, TerrainData, Triangle, Vertex,
};
use std::collections::HashMap;
use std::fs;
use std::path::{Path, PathBuf};

const HEIGHTFIELD_EPSILON: f32 = 1.0e-4;
const PTERRB_MAGIC: &[u8; 8] = b"PTERRB1\0";
const PTERRB_FLAG_ZLIB: u32 = 1;

#[derive(Clone)]
struct ParsedChunk {
    coord: ChunkCoord,
    chunk: TerrainChunk,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct TerrainLayerColor {
    pub r: u8,
    pub g: u8,
    pub b: u8,
}

impl TerrainLayerColor {
    pub const fn new(r: u8, g: u8, b: u8) -> Self {
        Self { r, g, b }
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct TerrainLayerRule {
    pub index: usize,
    pub name: Option<String>,
    pub color: TerrainLayerColor,
    pub color_tolerance: u8,
    pub texture_source: Option<String>,
    pub texture_tile_meters: f32,
    pub texture_rotation_degrees: f32,
    pub texture_hard_cut: bool,
    pub blend_with: Vec<usize>,
    pub friction: Option<f32>,
    pub restitution: Option<f32>,
}

#[derive(Clone, Debug)]
pub struct TerrainBakedChunkTile {
    pub chunk_x: i32,
    pub chunk_z: i32,
    pub texture_source: String,
    pub uv_min: [f32; 2],
    pub uv_max: [f32; 2],
}

#[derive(Clone, Debug)]
pub struct TerrainBakedChunkPhysics {
    pub chunk_x: i32,
    pub chunk_z: i32,
    pub triangle_layers: Vec<i32>,
}

#[derive(Clone, Debug)]
pub struct TerrainSourceSettings {
    pub sample_rate: Option<f32>,
    pub layers: Vec<TerrainLayerRule>,
    pub layer_blendings: Vec<(usize, usize)>,
    pub baked_chunk_tiles: Vec<TerrainBakedChunkTile>,
    pub baked_chunk_physics: Vec<TerrainBakedChunkPhysics>,
}

impl Default for TerrainSourceSettings {
    fn default() -> Self {
        Self {
            sample_rate: None,
            layers: Vec::new(),
            layer_blendings: Vec::new(),
            baked_chunk_tiles: Vec::new(),
            baked_chunk_physics: Vec::new(),
        }
    }
}

#[derive(Clone, Debug)]
pub struct LoadedTerrainSource {
    pub terrain: TerrainData,
    pub settings: TerrainSourceSettings,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum WorkingChunkMode {
    Unknown,
    Grid,
    Mesh,
}

struct WorkingChunk {
    coord: ChunkCoord,
    mode: WorkingChunkMode,
    grid_samples: Vec<(f32, f32, f32)>,
    mesh_vertices: Vec<Vector3>,
    mesh_triangles: Vec<[usize; 3]>,
}

impl WorkingChunk {
    fn new(coord: ChunkCoord) -> Self {
        Self {
            coord,
            mode: WorkingChunkMode::Unknown,
            grid_samples: Vec::new(),
            mesh_vertices: Vec::new(),
            mesh_triangles: Vec::new(),
        }
    }

    fn set_grid_sample(&mut self, x: f32, z: f32, y: f32) -> Option<()> {
        match self.mode {
            WorkingChunkMode::Unknown | WorkingChunkMode::Grid => {
                self.mode = WorkingChunkMode::Grid;
            }
            WorkingChunkMode::Mesh => return None,
        }

        self.grid_samples.push((x, z, y));
        Some(())
    }

    fn push_mesh_vertex(&mut self, v: Vector3) -> Option<()> {
        match self.mode {
            WorkingChunkMode::Unknown | WorkingChunkMode::Mesh => {
                self.mode = WorkingChunkMode::Mesh;
            }
            WorkingChunkMode::Grid => return None,
        }
        self.mesh_vertices.push(v);
        Some(())
    }

    fn push_mesh_triangle(&mut self, tri: [usize; 3]) -> Option<()> {
        match self.mode {
            WorkingChunkMode::Unknown | WorkingChunkMode::Mesh => {
                self.mode = WorkingChunkMode::Mesh;
            }
            WorkingChunkMode::Grid => return None,
        }
        self.mesh_triangles.push(tri);
        Some(())
    }

    fn finish(self, chunk_size_meters: f32) -> Option<ParsedChunk> {
        let config = ChunkConfig::new(chunk_size_meters);
        let chunk = match self.mode {
            WorkingChunkMode::Unknown | WorkingChunkMode::Grid => {
                if self.grid_samples.is_empty() {
                    TerrainChunk::new_flat(self.coord, config)
                } else {
                    let (vertices, triangles) = build_mesh_from_height_samples(&self.grid_samples)?;
                    let chunk =
                        TerrainChunk::from_mesh(self.coord, config, vertices, triangles).ok()?;
                    if !chunk.has_single_height_per_xz(HEIGHTFIELD_EPSILON) {
                        return None;
                    }
                    chunk
                }
            }
            WorkingChunkMode::Mesh => {
                if self.mesh_vertices.is_empty() || self.mesh_triangles.is_empty() {
                    return None;
                }
                let vertices: Vec<Vertex> =
                    self.mesh_vertices.into_iter().map(Vertex::new).collect();
                let triangles: Vec<Triangle> = self
                    .mesh_triangles
                    .into_iter()
                    .map(|t| Triangle::new(t[0], t[1], t[2]))
                    .collect();
                let chunk =
                    TerrainChunk::from_mesh(self.coord, config, vertices, triangles).ok()?;
                if !chunk.has_single_height_per_xz(HEIGHTFIELD_EPSILON) {
                    return None;
                }
                chunk
            }
        };
        Some(ParsedChunk {
            coord: self.coord,
            chunk,
        })
    }
}

pub fn load_terrain_literal(source: &str) -> Option<TerrainData> {
    let chunks = parse_terrain_kv(source, None, DEFAULT_CHUNK_SIZE_METERS)?;
    build_terrain_data(DEFAULT_CHUNK_SIZE_METERS, &chunks)
}

pub fn load_terrain_from_folder_source(source: &str) -> Option<LoadedTerrainSource> {
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

pub fn decode_loaded_terrain_blob(blob: &[u8]) -> Option<LoadedTerrainSource> {
    if blob.len() < 16 || &blob[..8] != PTERRB_MAGIC {
        return None;
    }
    let flags = u32::from_le_bytes(blob.get(8..12)?.try_into().ok()?);
    let raw_len = u32::from_le_bytes(blob.get(12..16)?.try_into().ok()?) as usize;
    let payload = blob.get(16..)?;
    let raw = if (flags & PTERRB_FLAG_ZLIB) != 0 {
        let decoded = decompress_zlib(payload).ok()?;
        if decoded.len() != raw_len {
            return None;
        }
        decoded
    } else {
        if payload.len() != raw_len {
            return None;
        }
        payload.to_vec()
    };

    let mut rd = TerrainBlobReader {
        bytes: &raw,
        cursor: 0,
    };

    let chunk_size_meters = rd.read_f32()?;
    if !chunk_size_meters.is_finite() || chunk_size_meters <= 0.0 {
        return None;
    }
    let chunk_count = rd.read_u32()? as usize;
    let mut terrain = TerrainData::new(chunk_size_meters);
    for _ in 0..chunk_count {
        let coord = ChunkCoord::new(rd.read_i32()?, rd.read_i32()?);
        let vertex_count = rd.read_u32()? as usize;
        let tri_count = rd.read_u32()? as usize;

        let mut vertices = Vec::with_capacity(vertex_count);
        for _ in 0..vertex_count {
            vertices.push(Vertex::new(Vector3::new(
                rd.read_f32()?,
                rd.read_f32()?,
                rd.read_f32()?,
            )));
        }

        let mut triangles = Vec::with_capacity(tri_count);
        for _ in 0..tri_count {
            triangles.push(Triangle::new(
                rd.read_u32()? as usize,
                rd.read_u32()? as usize,
                rd.read_u32()? as usize,
            ));
        }

        let chunk = TerrainChunk::from_mesh(
            coord,
            ChunkConfig::new(chunk_size_meters),
            vertices,
            triangles,
        )
        .ok()?;
        terrain.set_chunk(coord, chunk);
    }

    let sample_rate = rd.read_opt_f32()?;
    let _reserved_legacy = rd.read_opt_f32()?;

    let layer_count = rd.read_u32()? as usize;
    let mut layers = Vec::with_capacity(layer_count);
    for _ in 0..layer_count {
        let index = rd.read_u32()? as usize;
        let name = rd.read_opt_string()?;
        let color = TerrainLayerColor::new(rd.read_u8()?, rd.read_u8()?, rd.read_u8()?);
        let color_tolerance = rd.read_u8()?;
        let texture_source = rd.read_opt_string()?;
        let texture_tile_meters = rd.read_f32()?;
        let texture_rotation_degrees = rd.read_f32()?;
        let texture_hard_cut = rd.read_u8()? != 0;
        let blend_count = rd.read_u32()? as usize;
        let mut blend_with = Vec::with_capacity(blend_count);
        for _ in 0..blend_count {
            blend_with.push(rd.read_u32()? as usize);
        }
        let friction = rd.read_opt_f32()?;
        let restitution = rd.read_opt_f32()?;
        layers.push(TerrainLayerRule {
            index,
            name,
            color,
            color_tolerance,
            texture_source,
            texture_tile_meters,
            texture_rotation_degrees,
            texture_hard_cut,
            blend_with,
            friction,
            restitution,
        });
    }

    let pair_count = rd.read_u32()? as usize;
    let mut layer_blendings = Vec::with_capacity(pair_count);
    for _ in 0..pair_count {
        layer_blendings.push((rd.read_u32()? as usize, rd.read_u32()? as usize));
    }

    let mut baked_chunk_tiles = Vec::new();
    if rd.cursor < rd.bytes.len() {
        let tile_count = rd.read_u32()? as usize;
        baked_chunk_tiles = Vec::with_capacity(tile_count);
        for _ in 0..tile_count {
            baked_chunk_tiles.push(TerrainBakedChunkTile {
                chunk_x: rd.read_i32()?,
                chunk_z: rd.read_i32()?,
                texture_source: rd.read_opt_string()??,
                uv_min: [rd.read_f32()?, rd.read_f32()?],
                uv_max: [rd.read_f32()?, rd.read_f32()?],
            });
        }
    }
    let mut baked_chunk_physics = Vec::new();
    if rd.cursor < rd.bytes.len() {
        let chunk_count = rd.read_u32()? as usize;
        baked_chunk_physics = Vec::with_capacity(chunk_count);
        for _ in 0..chunk_count {
            let chunk_x = rd.read_i32()?;
            let chunk_z = rd.read_i32()?;
            let tri_count = rd.read_u32()? as usize;
            let mut triangle_layers = Vec::with_capacity(tri_count);
            for _ in 0..tri_count {
                triangle_layers.push(rd.read_i32()?);
            }
            baked_chunk_physics.push(TerrainBakedChunkPhysics {
                chunk_x,
                chunk_z,
                triangle_layers,
            });
        }
    }

    if rd.cursor != rd.bytes.len() {
        return None;
    }

    Some(LoadedTerrainSource {
        terrain,
        settings: TerrainSourceSettings {
            sample_rate,
            layers,
            layer_blendings,
            baked_chunk_tiles,
            baked_chunk_physics,
        },
    })
}

struct TerrainBlobReader<'a> {
    bytes: &'a [u8],
    cursor: usize,
}

impl<'a> TerrainBlobReader<'a> {
    fn read_exact(&mut self, len: usize) -> Option<&'a [u8]> {
        let end = self.cursor.checked_add(len)?;
        let slice = self.bytes.get(self.cursor..end)?;
        self.cursor = end;
        Some(slice)
    }

    fn read_u8(&mut self) -> Option<u8> {
        Some(*self.read_exact(1)?.first()?)
    }

    fn read_u32(&mut self) -> Option<u32> {
        Some(u32::from_le_bytes(self.read_exact(4)?.try_into().ok()?))
    }

    fn read_i32(&mut self) -> Option<i32> {
        Some(i32::from_le_bytes(self.read_exact(4)?.try_into().ok()?))
    }

    fn read_f32(&mut self) -> Option<f32> {
        Some(f32::from_le_bytes(self.read_exact(4)?.try_into().ok()?))
    }

    fn read_opt_f32(&mut self) -> Option<Option<f32>> {
        match self.read_u8()? {
            0 => Some(None),
            1 => Some(Some(self.read_f32()?)),
            _ => None,
        }
    }

    fn read_opt_string(&mut self) -> Option<Option<String>> {
        match self.read_u8()? {
            0 => Some(None),
            1 => {
                let len = self.read_u32()? as usize;
                let bytes = self.read_exact(len)?;
                Some(Some(String::from_utf8(bytes.to_vec()).ok()?))
            }
            _ => None,
        }
    }
}

fn load_terrain_from_disk_path(path: &Path) -> Option<LoadedTerrainSource> {
    if path.is_file() {
        let ext = path.extension().and_then(|s| s.to_str())?;
        if ext.eq_ignore_ascii_case("glb") || ext.eq_ignore_ascii_case("gltf") {
            return load_terrain_from_gltf_file(path, DEFAULT_CHUNK_SIZE_METERS).map(|terrain| {
                LoadedTerrainSource {
                    terrain,
                    settings: TerrainSourceSettings::default(),
                }
            });
        }
        if ext.eq_ignore_ascii_case("ptchunk") {
            let text = fs::read_to_string(path).ok()?;
            let hint = coord_from_ptchunk_path(path)?;
            let chunks = parse_terrain_kv(&text, Some(hint), DEFAULT_CHUNK_SIZE_METERS)?;
            return build_terrain_data(DEFAULT_CHUNK_SIZE_METERS, &chunks).map(|terrain| {
                LoadedTerrainSource {
                    terrain,
                    settings: TerrainSourceSettings::default(),
                }
            });
        }
        return None;
    }
    if !path.is_dir() {
        return None;
    }

    let settings = load_terrain_folder_settings(path);

    for candidate in ["terrain.glb", "terrain.gltf"] {
        let gltf_path = path.join(candidate);
        if gltf_path.is_file() {
            return load_terrain_from_gltf_file(&gltf_path, DEFAULT_CHUNK_SIZE_METERS)
                .map(|terrain| LoadedTerrainSource { terrain, settings });
        }
    }

    let mut paths = Vec::new();
    collect_ptchunk_paths(path, &mut paths).ok()?;
    paths.sort();

    let mut chunks = Vec::new();
    for chunk_path in paths {
        let text = fs::read_to_string(&chunk_path).ok()?;
        let hint = coord_from_ptchunk_path(&chunk_path)?;
        let mut parsed = parse_terrain_kv(&text, Some(hint), DEFAULT_CHUNK_SIZE_METERS)?;
        chunks.append(&mut parsed);
    }
    build_terrain_data(DEFAULT_CHUNK_SIZE_METERS, &chunks)
        .map(|terrain| LoadedTerrainSource { terrain, settings })
}

fn load_terrain_folder_settings(dir: &Path) -> TerrainSourceSettings {
    let settings_path = dir.join("settings.pterr");
    if !settings_path.is_file() {
        return TerrainSourceSettings::default();
    }
    let Ok(text) = fs::read_to_string(&settings_path) else {
        return TerrainSourceSettings::default();
    };
    parse_terrain_settings(&text)
}

fn parse_terrain_settings(source: &str) -> TerrainSourceSettings {
    let mut out = TerrainSourceSettings::default();
    let mut layers = HashMap::<usize, TerrainLayerRuleBuilder>::new();
    let mut global_blendings = Vec::<(usize, usize)>::new();
    for raw in source.lines() {
        let line = raw.trim();
        if line.is_empty() || line.starts_with('#') || line.starts_with("//") {
            continue;
        }
        let Some((key, value)) = line.split_once('=') else {
            continue;
        };
        let key = key.trim().to_ascii_lowercase();
        let value = value.trim();

        if key == "layer_blendings" || key == "layer_blending" {
            global_blendings.extend(parse_layer_blending_pairs(value));
            continue;
        }

        if let Some((layer_index, field)) = parse_layer_key(&key) {
            let entry = layers.entry(layer_index).or_default();
            match field {
                "name" => {
                    if !value.is_empty() {
                        entry.name = Some(value.to_string());
                    }
                }
                "color" | "rgb" | "hex" | "match_color" | "source_color" => {
                    if let Some(color) = parse_layer_color(value) {
                        entry.color = Some(color);
                    }
                }
                "color_tolerance" | "tolerance" | "match_tolerance" | "source_tolerance" => {
                    if let Ok(parsed) = value.parse::<f32>()
                        && parsed.is_finite()
                        && parsed >= 0.0
                    {
                        entry.color_tolerance = Some(parsed.round().clamp(0.0, 255.0) as u8);
                    }
                }
                "texture" | "texture_source" => {
                    if !value.is_empty() {
                        entry.texture_source = Some(value.to_string());
                    }
                }
                "texture_tile_meters" | "tile_meters" | "tile_size" | "tile" => {
                    if let Ok(parsed) = value.parse::<f32>()
                        && parsed.is_finite()
                        && parsed > 0.0
                    {
                        entry.texture_tile_meters = Some(parsed);
                    }
                }
                "texture_rotation_degrees" | "rotation_degrees" | "rotation" => {
                    if let Ok(parsed) = value.parse::<f32>()
                        && parsed.is_finite()
                    {
                        entry.texture_rotation_degrees = Some(parsed);
                    }
                }
                "hard_cut" | "sample_nearest" | "texture_hard_cut" => {
                    if let Some(parsed_bool) = parse_bool_token(value) {
                        entry.texture_hard_cut = Some(parsed_bool);
                    }
                }
                "blending" | "blend_with" | "layer_blendings" => {
                    entry.blend_with.extend(parse_layer_blend_list(value));
                }
                "filter" | "sample_filter" => {
                    let token = value.trim().to_ascii_lowercase();
                    match token.as_str() {
                        "nearest" | "point" | "hard" => entry.texture_hard_cut = Some(true),
                        "linear" | "smooth" => entry.texture_hard_cut = Some(false),
                        _ => {}
                    }
                }
                "friction" => {
                    if let Ok(parsed) = value.parse::<f32>()
                        && parsed.is_finite()
                        && parsed >= 0.0
                    {
                        entry.friction = Some(parsed);
                    }
                }
                "restitution" | "bounce" => {
                    if let Ok(parsed) = value.parse::<f32>()
                        && parsed.is_finite()
                        && parsed >= 0.0
                    {
                        entry.restitution = Some(parsed);
                    }
                }
                _ => {}
            }
            continue;
        }

        if let Ok(parsed) = value.parse::<f32>()
            && parsed.is_finite()
            && parsed > 0.0
        {
            match key.as_str() {
                "sample_rate"
                | "terrain_sample_rate"
                | "pixels_per_meter"
                | "terrain_pixels_per_meter"
                | "ppm" => {
                    out.sample_rate = Some(parsed.clamp(1.0, 12.0));
                }
                _ => {}
            }
        }
    }
    let mut sorted_layers = layers.into_iter().collect::<Vec<_>>();
    sorted_layers.sort_unstable_by_key(|(index, _)| *index);
    out.layers = sorted_layers
        .into_iter()
        .filter_map(|(index, layer)| layer.finish(index))
        .collect();
    out.layer_blendings = global_blendings;
    for (a, b) in &out.layer_blendings {
        if let Some(layer_a) = out.layers.iter_mut().find(|layer| layer.index == *a)
            && !layer_a.blend_with.contains(b)
        {
            layer_a.blend_with.push(*b);
        }
        if let Some(layer_b) = out.layers.iter_mut().find(|layer| layer.index == *b)
            && !layer_b.blend_with.contains(a)
        {
            layer_b.blend_with.push(*a);
        }
    }
    for layer in &mut out.layers {
        layer.blend_with.sort_unstable();
        layer.blend_with.dedup();
    }
    out
}

#[derive(Default)]
struct TerrainLayerRuleBuilder {
    name: Option<String>,
    color: Option<TerrainLayerColor>,
    color_tolerance: Option<u8>,
    texture_source: Option<String>,
    texture_tile_meters: Option<f32>,
    texture_rotation_degrees: Option<f32>,
    texture_hard_cut: Option<bool>,
    blend_with: Vec<usize>,
    friction: Option<f32>,
    restitution: Option<f32>,
}

impl TerrainLayerRuleBuilder {
    fn finish(self, index: usize) -> Option<TerrainLayerRule> {
        let color = self.color?;
        Some(TerrainLayerRule {
            index,
            name: self.name,
            color,
            color_tolerance: self.color_tolerance.unwrap_or(0),
            texture_source: self.texture_source,
            texture_tile_meters: self.texture_tile_meters.unwrap_or(6.0),
            texture_rotation_degrees: self.texture_rotation_degrees.unwrap_or(0.0),
            texture_hard_cut: self.texture_hard_cut.unwrap_or(false),
            blend_with: self.blend_with,
            friction: self.friction,
            restitution: self.restitution,
        })
    }
}

fn parse_layer_key(key: &str) -> Option<(usize, &str)> {
    let trimmed = key.trim();
    let rest = trimmed
        .strip_prefix("layer.")
        .or_else(|| trimmed.strip_prefix("layers."))
        .or_else(|| trimmed.strip_prefix("layer"))
        .or_else(|| trimmed.strip_prefix("layers"))?;
    let (index_text, field) = rest.split_once('.')?;
    let index = index_text.trim().parse::<usize>().ok()?;
    Some((index, field.trim()))
}

fn parse_layer_color(value: &str) -> Option<TerrainLayerColor> {
    let value = value.trim();
    if let Some(hex) = value.strip_prefix('#') {
        return parse_hex_rgb(hex);
    }
    if let Some(hex) = value
        .strip_prefix("0x")
        .or_else(|| value.strip_prefix("0X"))
    {
        return parse_hex_rgb(hex);
    }

    let cleaned = value.trim_start_matches('[').trim_end_matches(']');
    let comps = cleaned
        .split(',')
        .map(|v| v.trim())
        .filter(|v| !v.is_empty())
        .collect::<Vec<_>>();
    if comps.len() != 3 {
        return None;
    }
    let mut out = [0u8; 3];
    for (i, comp) in comps.iter().enumerate() {
        if let Ok(parsed_int) = comp.parse::<i32>() {
            out[i] = parsed_int.clamp(0, 255) as u8;
            continue;
        }
        let parsed = comp.parse::<f32>().ok()?;
        if !parsed.is_finite() {
            return None;
        }
        if parsed <= 1.0 {
            out[i] = (parsed.clamp(0.0, 1.0) * 255.0).round() as u8;
        } else {
            out[i] = parsed.round().clamp(0.0, 255.0) as u8;
        }
    }
    Some(TerrainLayerColor::new(out[0], out[1], out[2]))
}

fn parse_hex_rgb(value: &str) -> Option<TerrainLayerColor> {
    let hex = value.trim();
    if hex.len() != 6 {
        return None;
    }
    let parsed = u32::from_str_radix(hex, 16).ok()?;
    Some(TerrainLayerColor::new(
        ((parsed >> 16) & 0xFF) as u8,
        ((parsed >> 8) & 0xFF) as u8,
        (parsed & 0xFF) as u8,
    ))
}

fn parse_bool_token(value: &str) -> Option<bool> {
    let token = value.trim().to_ascii_lowercase();
    match token.as_str() {
        "true" | "1" | "yes" | "on" => Some(true),
        "false" | "0" | "no" | "off" => Some(false),
        _ => None,
    }
}

fn parse_layer_blend_list(value: &str) -> Vec<usize> {
    value
        .replace(['[', ']', ';'], ",")
        .split(',')
        .filter_map(|token| token.trim().parse::<usize>().ok())
        .collect()
}

fn parse_layer_blending_pairs(value: &str) -> Vec<(usize, usize)> {
    // Preferred format: array of pairs/tuples, e.g.:
    // layer_blendings = [(0,1), (1,2)]   or   [[0,1], [1,2]]
    let mut out = Vec::new();
    let mut current = String::new();
    let mut depth = 0i32;
    for ch in value.chars() {
        match ch {
            '(' | '[' => {
                depth += 1;
                if depth == 2 {
                    current.clear();
                } else if depth > 2 {
                    current.push(ch);
                }
            }
            ')' | ']' => {
                if depth == 2 {
                    let nums = parse_layer_blend_list(&current);
                    if nums.len() == 2 && nums[0] != nums[1] {
                        out.push((nums[0], nums[1]));
                    }
                    current.clear();
                } else if depth > 2 {
                    current.push(ch);
                }
                depth = (depth - 1).max(0);
            }
            _ => {
                if depth >= 2 {
                    current.push(ch);
                }
            }
        }
    }
    if !out.is_empty() {
        return out;
    }

    // Backward-compatible fallback: flat numeric stream grouped by 2.
    let nums = parse_layer_blend_list(value);
    for pair in nums.chunks(2) {
        if pair.len() == 2 && pair[0] != pair[1] {
            out.push((pair[0], pair[1]));
        }
    }
    out
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

fn parse_terrain_kv(
    source: &str,
    default_coord: Option<ChunkCoord>,
    chunk_size_meters: f32,
) -> Option<Vec<ParsedChunk>> {
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

        if let Some((x, z, y)) = parse_grid_sample_line(line) {
            let coord = current_coord?;
            let chunk = working
                .iter_mut()
                .find(|chunk| chunk.coord == coord)
                .expect("chunk exists after ensure");
            chunk.set_grid_sample(x, z, y)?;
            continue;
        }

        if let Some(v) = parse_mesh_vertex_line(line) {
            let coord = current_coord?;
            let chunk = working
                .iter_mut()
                .find(|chunk| chunk.coord == coord)
                .expect("chunk exists after ensure");
            chunk.push_mesh_vertex(v)?;
            continue;
        }

        if let Some(tri) = parse_mesh_triangle_line(line) {
            let coord = current_coord?;
            let chunk = working
                .iter_mut()
                .find(|chunk| chunk.coord == coord)
                .expect("chunk exists after ensure");
            chunk.push_mesh_triangle(tri)?;
            continue;
        }

        return None;
    }

    if working.is_empty() {
        return None;
    }

    let mut out = Vec::with_capacity(working.len());
    for chunk in working {
        out.push(chunk.finish(chunk_size_meters)?);
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

fn parse_mesh_vertex_line(line: &str) -> Option<Vector3> {
    let trimmed = line.trim();
    let lower = trimmed.to_ascii_lowercase();
    if !lower.starts_with("vertex") && !lower.starts_with("vtx") && !lower.starts_with("v") {
        return None;
    }
    let bracket_start = trimmed.find('[')?;
    let bracket_end = trimmed[bracket_start..].find(']')? + bracket_start;
    let (x, y, z) = parse_triplet_f32(&trimmed[(bracket_start + 1)..bracket_end])?;
    Some(Vector3::new(x, y, z))
}

fn parse_mesh_triangle_line(line: &str) -> Option<[usize; 3]> {
    let trimmed = line.trim();
    let lower = trimmed.to_ascii_lowercase();
    if !lower.starts_with("triangle") && !lower.starts_with("tri") && !lower.starts_with("face") {
        return None;
    }
    let bracket_start = trimmed.find('[')?;
    let bracket_end = trimmed[bracket_start..].find(']')? + bracket_start;
    let (a, b, c) = parse_triplet_i32(&trimmed[(bracket_start + 1)..bracket_end])?;
    Some([
        usize::try_from(a).ok()?,
        usize::try_from(b).ok()?,
        usize::try_from(c).ok()?,
    ])
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
        terrain.set_chunk(chunk.coord, chunk.chunk.clone());
    }
    Some(terrain)
}

fn load_terrain_from_gltf_file(path: &Path, chunk_size_meters: f32) -> Option<TerrainData> {
    let (doc, buffers, _images) = gltf::import(path).ok()?;
    let scene = doc.default_scene().or_else(|| doc.scenes().next())?;

    let mut positions = Vec::<Vec3>::new();
    let mut triangles = Vec::<[u32; 3]>::new();

    for node in scene.nodes() {
        collect_gltf_scene_node_meshes(
            &node,
            Mat4::IDENTITY,
            &buffers,
            &mut positions,
            &mut triangles,
        );
    }

    if positions.is_empty() || triangles.is_empty() {
        return None;
    }

    let chunks = chunk_mesh_positions(&positions, &triangles, chunk_size_meters)?;
    let mut terrain = TerrainData::new(chunk_size_meters);
    for (coord, chunk) in chunks {
        terrain.set_chunk(coord, chunk);
    }
    Some(terrain)
}

fn collect_gltf_scene_node_meshes(
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
                for i in (0..local_positions.len() / 3 * 3).step_by(3) {
                    triangles.push([base + i as u32, base + i as u32 + 1, base + i as u32 + 2]);
                }
            }
        }
    }

    for child in node.children() {
        collect_gltf_scene_node_meshes(&child, world, buffers, positions, triangles);
    }
}

struct ChunkMeshBuilder {
    coord: ChunkCoord,
    map_global_to_local: HashMap<u32, usize>,
    xz_to_height: HashMap<(i64, i64), f32>,
    vertices: Vec<Vertex>,
    triangles: Vec<Triangle>,
}

fn chunk_mesh_positions(
    positions: &[Vec3],
    triangles: &[[u32; 3]],
    chunk_size_meters: f32,
) -> Option<Vec<(ChunkCoord, TerrainChunk)>> {
    let mut builders = HashMap::<ChunkCoord, ChunkMeshBuilder>::new();
    let xz_quant = 1.0 / HEIGHTFIELD_EPSILON.max(1.0e-4);

    for tri in triangles {
        let ia = usize::try_from(tri[0]).ok()?;
        let ib = usize::try_from(tri[1]).ok()?;
        let ic = usize::try_from(tri[2]).ok()?;
        let (Some(a), Some(b), Some(c)) = (positions.get(ia), positions.get(ib), positions.get(ic))
        else {
            continue;
        };
        let centroid = (*a + *b + *c) / 3.0;
        let coord = world_to_chunk_coord(centroid.x, centroid.z, chunk_size_meters);
        let center_x = coord.x as f32 * chunk_size_meters;
        let center_z = coord.z as f32 * chunk_size_meters;

        let builder = builders.entry(coord).or_insert_with(|| ChunkMeshBuilder {
            coord,
            map_global_to_local: HashMap::new(),
            xz_to_height: HashMap::new(),
            vertices: Vec::new(),
            triangles: Vec::new(),
        });

        let mut local_ids = [0usize; 3];
        for (corner, global_idx) in tri.iter().enumerate() {
            if let Some(existing) = builder.map_global_to_local.get(global_idx).copied() {
                local_ids[corner] = existing;
                continue;
            }

            let g = positions.get(usize::try_from(*global_idx).ok()?)?;
            let local = Vector3::new(g.x - center_x, g.y, g.z - center_z);
            let key = (
                (local.x * xz_quant).round() as i64,
                (local.z * xz_quant).round() as i64,
            );
            if let Some(existing_y) = builder.xz_to_height.get(&key)
                && (*existing_y - local.y).abs() > HEIGHTFIELD_EPSILON
            {
                return None;
            }
            builder.xz_to_height.insert(key, local.y);

            let local_id = builder.vertices.len();
            builder.vertices.push(Vertex::new(local));
            builder.map_global_to_local.insert(*global_idx, local_id);
            local_ids[corner] = local_id;
        }

        if local_ids[0] != local_ids[1]
            && local_ids[1] != local_ids[2]
            && local_ids[0] != local_ids[2]
        {
            builder
                .triangles
                .push(Triangle::new(local_ids[0], local_ids[1], local_ids[2]));
        }
    }

    let mut out = Vec::new();
    for (_coord, build) in builders {
        if build.vertices.len() < 3 || build.triangles.is_empty() {
            continue;
        }
        let chunk = TerrainChunk::from_mesh(
            build.coord,
            ChunkConfig::new(chunk_size_meters),
            build.vertices,
            build.triangles,
        )
        .ok()?;
        if !chunk.has_single_height_per_xz(HEIGHTFIELD_EPSILON) {
            return None;
        }
        out.push((build.coord, chunk));
    }
    if out.is_empty() {
        return None;
    }
    Some(out)
}

fn build_mesh_from_height_samples(
    samples: &[(f32, f32, f32)],
) -> Option<(Vec<Vertex>, Vec<Triangle>)> {
    let mut dedup = HashMap::<(i64, i64), (f32, f32, f32)>::new();
    let quant = 1.0 / HEIGHTFIELD_EPSILON.max(1.0e-4);
    for (x, z, y) in samples.iter().copied() {
        if !x.is_finite() || !z.is_finite() || !y.is_finite() {
            return None;
        }
        let key = ((x * quant).round() as i64, (z * quant).round() as i64);
        if let Some((_, _, ey)) = dedup.get(&key).copied() {
            if (ey - y).abs() > HEIGHTFIELD_EPSILON {
                return None;
            }
        } else {
            dedup.insert(key, (x, z, y));
        }
    }

    if dedup.len() < 3 {
        return None;
    }

    let unique: Vec<(f32, f32, f32)> = dedup.into_values().collect();
    let points: Vec<(f64, f64)> = unique
        .iter()
        .map(|(x, z, _)| (*x as f64, *z as f64))
        .collect();
    let tri_ix = delaunay_triangulate_2d(&points)?;
    if tri_ix.is_empty() {
        return None;
    }

    let vertices: Vec<Vertex> = unique
        .iter()
        .map(|(x, z, y)| Vertex::new(Vector3::new(*x, *y, *z)))
        .collect();
    let triangles: Vec<Triangle> = tri_ix
        .into_iter()
        .map(|[a, b, c]| Triangle::new(a, b, c))
        .collect();
    Some((vertices, triangles))
}

#[derive(Clone, Copy, PartialEq, Eq, Hash)]
struct Edge2 {
    a: usize,
    b: usize,
}

impl Edge2 {
    fn new(a: usize, b: usize) -> Self {
        if a <= b {
            Self { a, b }
        } else {
            Self { a: b, b: a }
        }
    }
}

fn delaunay_triangulate_2d(points: &[(f64, f64)]) -> Option<Vec<[usize; 3]>> {
    if points.len() < 3 {
        return None;
    }

    let mut min_x = f64::INFINITY;
    let mut min_y = f64::INFINITY;
    let mut max_x = f64::NEG_INFINITY;
    let mut max_y = f64::NEG_INFINITY;
    for (x, y) in points.iter().copied() {
        min_x = min_x.min(x);
        min_y = min_y.min(y);
        max_x = max_x.max(x);
        max_y = max_y.max(y);
    }
    let dx = max_x - min_x;
    let dy = max_y - min_y;
    let delta = dx.max(dy).max(1.0);
    let mid_x = (min_x + max_x) * 0.5;
    let mid_y = (min_y + max_y) * 0.5;

    let mut all_points = points.to_vec();
    let super_a = all_points.len();
    all_points.push((mid_x - 20.0 * delta, mid_y - delta));
    let super_b = all_points.len();
    all_points.push((mid_x, mid_y + 20.0 * delta));
    let super_c = all_points.len();
    all_points.push((mid_x + 20.0 * delta, mid_y - delta));

    let mut tris = vec![[super_a, super_b, super_c]];
    for p_idx in 0..points.len() {
        let p = all_points[p_idx];
        let mut bad = Vec::<usize>::new();
        for (t_idx, tri) in tris.iter().enumerate() {
            if circumcircle_contains(
                all_points[tri[0]],
                all_points[tri[1]],
                all_points[tri[2]],
                p,
            ) {
                bad.push(t_idx);
            }
        }
        if bad.is_empty() {
            continue;
        }

        let mut edges = HashMap::<Edge2, usize>::new();
        for &idx in &bad {
            let t = tris[idx];
            *edges.entry(Edge2::new(t[0], t[1])).or_default() += 1;
            *edges.entry(Edge2::new(t[1], t[2])).or_default() += 1;
            *edges.entry(Edge2::new(t[2], t[0])).or_default() += 1;
        }

        bad.sort_unstable();
        for idx in bad.into_iter().rev() {
            tris.swap_remove(idx);
        }

        for (edge, count) in edges {
            if count != 1 {
                continue;
            }
            let mut tri = [edge.a, edge.b, p_idx];
            let pa = all_points[tri[0]];
            let pb = all_points[tri[1]];
            let pc = all_points[tri[2]];
            if orient2d(pa, pb, pc) < 0.0 {
                tri.swap(0, 1);
            }
            if tri[0] != tri[1] && tri[1] != tri[2] && tri[0] != tri[2] {
                tris.push(tri);
            }
        }
    }

    tris.retain(|t| t[0] < points.len() && t[1] < points.len() && t[2] < points.len());
    Some(tris)
}

fn orient2d(a: (f64, f64), b: (f64, f64), c: (f64, f64)) -> f64 {
    (b.0 - a.0) * (c.1 - a.1) - (b.1 - a.1) * (c.0 - a.0)
}

fn circumcircle_contains(a: (f64, f64), b: (f64, f64), c: (f64, f64), p: (f64, f64)) -> bool {
    let ax = a.0 - p.0;
    let ay = a.1 - p.1;
    let bx = b.0 - p.0;
    let by = b.1 - p.1;
    let cx = c.0 - p.0;
    let cy = c.1 - p.1;

    let det = (ax * ax + ay * ay) * (bx * cy - by * cx) - (bx * bx + by * by) * (ax * cy - ay * cx)
        + (cx * cx + cy * cy) * (ax * by - ay * bx);
    if orient2d(a, b, c) > 0.0 {
        det > 1.0e-12
    } else {
        det < -1.0e-12
    }
}

fn world_to_chunk_coord(world_x: f32, world_z: f32, chunk_size_meters: f32) -> ChunkCoord {
    let inv = 1.0 / chunk_size_meters;
    let cx = (world_x * inv + 0.5).floor() as i32;
    let cz = (world_z * inv + 0.5).floor() as i32;
    ChunkCoord::new(cx, cz)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_terrain_settings_reads_layer_rules_sorted_by_index() {
        let src = r#"
            sample_rate = 2.0

            layer.2.color = #224466
            layer.2.texture = res://terrain/rock.png
            layer.2.friction = 1.2

            layer.0.match_color = 128, 200, 64
            layer.0.match_tolerance = 2
            layer.0.name = fairway
            layer.0.tile_meters = 5.0
            layer.0.rotation_degrees = 15
            layer.0.restitution = 0.03
        "#;

        let parsed = parse_terrain_settings(src);
        assert_eq!(parsed.sample_rate, Some(2.0));
        assert_eq!(parsed.layers.len(), 2);

        let l0 = &parsed.layers[0];
        assert_eq!(l0.index, 0);
        assert_eq!(l0.name.as_deref(), Some("fairway"));
        assert_eq!(l0.color, TerrainLayerColor::new(128, 200, 64));
        assert_eq!(l0.color_tolerance, 2);
        assert_eq!(l0.texture_tile_meters, 5.0);
        assert_eq!(l0.texture_rotation_degrees, 15.0);
        assert_eq!(l0.restitution, Some(0.03));

        let l1 = &parsed.layers[1];
        assert_eq!(l1.index, 2);
        assert_eq!(l1.color, TerrainLayerColor::new(0x22, 0x44, 0x66));
        assert_eq!(l1.texture_source.as_deref(), Some("res://terrain/rock.png"));
        assert_eq!(l1.friction, Some(1.2));
    }

    #[test]
    fn parse_terrain_settings_accepts_rgb_float_01() {
        let src = r#"
            layer.0.color = 0.5, 0.25, 1.0
        "#;
        let parsed = parse_terrain_settings(src);
        assert_eq!(parsed.layers.len(), 1);
        let c = parsed.layers[0].color;
        assert_eq!(c, TerrainLayerColor::new(128, 64, 255));
    }

    #[test]
    fn parse_terrain_settings_accepts_layer_blendings_tuples() {
        let src = r#"
            layer.0.color = #369528
            layer.1.color = #21411c
            layer.2.color = #c8b27a
            layer_blendings = [(0,1), (1,2)]
        "#;
        let parsed = parse_terrain_settings(src);
        assert_eq!(parsed.layer_blendings, vec![(0, 1), (1, 2)]);
        let l0 = parsed.layers.iter().find(|l| l.index == 0).unwrap();
        let l1 = parsed.layers.iter().find(|l| l.index == 1).unwrap();
        let l2 = parsed.layers.iter().find(|l| l.index == 2).unwrap();
        assert!(l0.blend_with.contains(&1));
        assert!(l1.blend_with.contains(&0));
        assert!(l1.blend_with.contains(&2));
        assert!(l2.blend_with.contains(&1));
    }

    #[test]
    fn parse_terrain_settings_clamps_sample_rate() {
        let src = r#"
            sample_rate = 32
        "#;
        let parsed = parse_terrain_settings(src);
        assert_eq!(parsed.sample_rate, Some(12.0));
    }

    #[test]
    fn parse_terrain_settings_legacy_ppm_alias_maps_to_sample_rate() {
        let src = r#"
            ppm = 3
        "#;
        let parsed = parse_terrain_settings(src);
        assert_eq!(parsed.sample_rate, Some(3.0));
    }
}
