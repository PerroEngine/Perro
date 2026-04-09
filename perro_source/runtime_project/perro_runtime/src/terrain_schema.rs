use glam::{Mat4, Vec3};
use perro_io::{ResolvedPath, resolve_path};
use perro_structs::Vector3;
use perro_terrain::{
    ChunkConfig, ChunkCoord, DEFAULT_CHUNK_SIZE_METERS, TerrainChunk, TerrainData, Triangle, Vertex,
};
use std::collections::HashMap;
use std::fs;
use std::path::{Path, PathBuf};

const HEIGHTFIELD_EPSILON: f32 = 1.0e-4;

#[derive(Clone)]
struct ParsedChunk {
    coord: ChunkCoord,
    chunk: TerrainChunk,
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
        let ext = path.extension().and_then(|s| s.to_str())?;
        if ext.eq_ignore_ascii_case("glb") || ext.eq_ignore_ascii_case("gltf") {
            return load_terrain_from_gltf_file(path, DEFAULT_CHUNK_SIZE_METERS);
        }
        if ext.eq_ignore_ascii_case("ptchunk") {
            let text = fs::read_to_string(path).ok()?;
            let hint = coord_from_ptchunk_path(path)?;
            let chunks = parse_terrain_kv(&text, Some(hint), DEFAULT_CHUNK_SIZE_METERS)?;
            return build_terrain_data(DEFAULT_CHUNK_SIZE_METERS, &chunks);
        }
        return None;
    }
    if !path.is_dir() {
        return None;
    }

    for candidate in ["terrain.glb", "terrain.gltf"] {
        let gltf_path = path.join(candidate);
        if gltf_path.is_file() {
            return load_terrain_from_gltf_file(&gltf_path, DEFAULT_CHUNK_SIZE_METERS);
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
            if circumcircle_contains(all_points[tri[0]], all_points[tri[1]], all_points[tri[2]], p)
            {
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

    let det = (ax * ax + ay * ay) * (bx * cy - by * cx)
        - (bx * bx + by * by) * (ax * cy - ay * cx)
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
