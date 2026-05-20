use super::*;
use perro_structs::{AudioListenerOptions, BitMask};

#[derive(Debug, Clone, PartialEq)]
pub struct Camera2DState {
    pub position: [f32; 2],
    pub rotation_radians: f32,
    pub zoom: f32,
    pub render_mask: BitMask,
    pub post_processing: Arc<[PostProcessEffect]>,
    pub audio_options: AudioListenerOptions,
}

#[derive(Debug, Clone, PartialEq)]
pub enum CameraStreamSourceState {
    TwoD(Camera2DState),
    ThreeD(Camera3DState),
}

#[derive(Debug, Clone, PartialEq)]
pub struct CameraStreamState {
    pub source: CameraStreamSourceState,
    pub resolution: [u32; 2],
    pub aspect_ratio: f32,
    pub post_processing: Arc<[PostProcessEffect]>,
    pub output_texture: TextureID,
    pub sprites_2d: Arc<[Sprite2DCommand]>,
    pub lights_2d: Arc<[Light2DState]>,
    pub point_particles_2d: Arc<[(NodeID, PointParticles2DState)]>,
    pub waters_2d: Arc<[(NodeID, Water2DState)]>,
    pub draws_3d: Arc<[CameraStreamDraw3DState]>,
    pub lighting_3d: CameraStreamLighting3DState,
    pub point_particles_3d: Arc<[(NodeID, PointParticles3DState)]>,
    pub waters_3d: Arc<[(NodeID, Water3DState)]>,
}

#[derive(Debug, Clone, PartialEq, Default)]
pub struct CameraStreamLighting3DState {
    pub ambient_light: Option<AmbientLight3DState>,
    pub sky: Option<Sky3DState>,
    pub ray_lights: [Option<RayLight3DState>; 3],
    pub point_lights: [Option<PointLight3DState>; 8],
    pub spot_lights: [Option<SpotLight3DState>; 8],
}

#[derive(Debug, Clone, PartialEq)]
pub enum CameraStreamDraw3DState {
    Draw {
        mesh: MeshID,
        surfaces: Arc<[MeshSurfaceBinding3D]>,
        node: NodeID,
        model: [[f32; 4]; 4],
        skeleton: Option<SkeletonPalette>,
        meshlet_override: Option<bool>,
        lod: LODOptions3D,
        blend: MeshBlendOptions3D,
    },
    DrawMulti {
        mesh: MeshID,
        surfaces: Arc<[MeshSurfaceBinding3D]>,
        node: NodeID,
        instance_mats: Arc<[[[f32; 4]; 4]]>,
        skeleton: Option<SkeletonPalette>,
        meshlet_override: Option<bool>,
        lod: LODOptions3D,
        blend: MeshBlendOptions3D,
    },
    DrawMultiDense {
        mesh: MeshID,
        surfaces: Arc<[MeshSurfaceBinding3D]>,
        node: NodeID,
        node_model: [[f32; 4]; 4],
        instance_scale: f32,
        instances: Arc<[DenseInstancePose3D]>,
        meshlet_override: Option<bool>,
        lod: LODOptions3D,
        blend: MeshBlendOptions3D,
    },
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct CameraStream3DState {
    pub model: [[f32; 4]; 4],
    pub size: [f32; 2],
    pub tint: Color,
}

impl Default for Camera2DState {
    fn default() -> Self {
        Self {
            position: [0.0, 0.0],
            rotation_radians: 0.0,
            zoom: 1.0,
            render_mask: BitMask::NONE,
            post_processing: Arc::from([]),
            audio_options: AudioListenerOptions::new(),
        }
    }
}

#[derive(Debug, Clone, Copy)]
pub struct Rect2DCommand {
    pub center: [f32; 2],
    pub size: [f32; 2],
    pub color: Color,
    pub z_index: i32,
}

#[derive(Debug, Clone, PartialEq)]
pub struct DrawShape2DCommand {
    pub shape: DrawShape2D,
    pub position: [f32; 2],
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Sprite2DCommand {
    pub texture: TextureID,
    pub model: [[f32; 3]; 3],
    pub tint: Color,
    pub uv_min: [f32; 2],
    pub uv_max: [f32; 2],
    pub size: [f32; 2],
    pub z_index: i32,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct PointLight2DState {
    pub position: [f32; 2],
    pub color: [f32; 3],
    pub intensity: f32,
    pub range: f32,
    pub z_index: i32,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum WaterIdleModeState {
    #[default]
    Calm,
    Sine,
    Chop,
    Storm,
    River,
}

#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub enum WaterShapeState {
    #[default]
    Rect,
    Circle {
        radius: f32,
    },
    Cylinder {
        radius: f32,
        half_height: f32,
    },
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct WaterImpact2D {
    pub position: [f32; 2],
    pub velocity: [f32; 2],
    pub strength: f32,
    pub radius: f32,
    pub cavitation: f32,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct WaterContact2D {
    pub body: NodeID,
    pub position: [f32; 2],
    pub velocity: [f32; 2],
    pub radius: f32,
    pub foam_amount: f32,
    pub persist: f32,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum WaterCoastlineShape2D {
    Quad {
        center: [f32; 2],
        half_extents: [f32; 2],
        rotation: f32,
    },
    Circle {
        center: [f32; 2],
        radius: f32,
    },
    Triangle {
        points: [[f32; 2]; 3],
    },
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct WaterSampleState {
    pub node: NodeID,
    pub height: f32,
    pub velocity: [f32; 2],
    pub foam: f32,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct WaterBodyQueryState {
    pub water: NodeID,
    pub body: NodeID,
    pub point: u8,
    pub local: [f32; 2],
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct WaterBodySampleState {
    pub water: NodeID,
    pub body: NodeID,
    pub point: u8,
    pub local: [f32; 2],
    pub height: f32,
    pub velocity: [f32; 2],
    pub foam: f32,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct WaterLinkState {
    pub other: NodeID,
    pub overlap_min: [f32; 2],
    pub overlap_max: [f32; 2],
    pub blend_width: f32,
    pub wave_transfer: f32,
    pub flow_transfer: f32,
}

#[derive(Debug, Clone, PartialEq)]
pub struct Water2DState {
    pub model: [[f32; 3]; 3],
    pub z_index: i32,
    pub paused: bool,
    pub simulation_time: f32,
    pub simulation_delta: f32,
    pub size: [f32; 2],
    pub shape: WaterShapeState,
    pub resolution: [u32; 2],
    pub render_resolution: [u32; 2],
    pub depth: f32,
    pub flow: [f32; 2],
    pub wind: [f32; 2],
    pub idle_mode: WaterIdleModeState,
    pub wave_speed: f32,
    pub wave_scale: f32,
    pub wave_length: f32,
    pub damping: f32,
    pub wake_strength: f32,
    pub foam_strength: f32,
    pub sample_readback_rate: f32,
    pub lod_near_distance: f32,
    pub lod_mid_distance: f32,
    pub lod_far_distance: f32,
    pub lod_min_resolution: [u32; 2],
    pub collision_layers: BitMask,
    pub collision_mask: BitMask,
    pub deep_color: Color,
    pub shallow_color: Color,
    pub shallow_depth: f32,
    pub sky_bias_ratio: f32,
    pub transparency: f32,
    pub reflectivity: f32,
    pub roughness: f32,
    pub fresnel_power: f32,
    pub normal_strength: f32,
    pub ripple_scale: f32,
    pub foam_color: Color,
    pub foam_amount: f32,
    pub crest_foam_threshold: f32,
    pub caustic_strength: f32,
    pub refraction_strength: f32,
    pub scattering_strength: f32,
    pub distance_fog_strength: f32,
    pub coastline_foam_color: Color,
    pub coastline_foam_strength: f32,
    pub coastline_foam_width: f32,
    pub coastline_cutoff_softness: f32,
    pub coastline_wave_reflection: f32,
    pub coastline_wave_damping: f32,
    pub coastline_edge_noise: f32,
    pub debug: bool,
    pub links: Arc<[WaterLinkState]>,
    pub queries: Arc<[WaterBodyQueryState]>,
    pub impacts: Arc<[WaterImpact2D]>,
    pub coastline_shapes: Arc<[WaterCoastlineShape2D]>,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct AmbientLight2DState {
    pub color: [f32; 3],
    pub intensity: f32,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct RayLight2DState {
    pub direction: [f32; 2],
    pub color: [f32; 3],
    pub intensity: f32,
    pub z_index: i32,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct SpotLight2DState {
    pub position: [f32; 2],
    pub direction: [f32; 2],
    pub color: [f32; 3],
    pub intensity: f32,
    pub range: f32,
    pub inner_angle_radians: f32,
    pub outer_angle_radians: f32,
    pub z_index: i32,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Light2DState {
    Ambient(AmbientLight2DState),
    Ray(RayLight2DState),
    Point(PointLight2DState),
    Spot(SpotLight2DState),
}

#[derive(Debug, Clone, PartialEq)]
pub struct TileMap2DCommand {
    pub texture: TextureID,
    pub sprites: Arc<[Sprite2DCommand]>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct TileSet2D {
    pub texture: Cow<'static, str>,
    pub tile_size: [f32; 2],
    pub columns: u32,
    pub rows: u32,
    pub tiles: Cow<'static, [TileSetTile2D]>,
}

impl TileSet2D {
    pub const fn empty() -> Self {
        Self {
            texture: Cow::Borrowed(""),
            tile_size: [0.0, 0.0],
            columns: 0,
            rows: 0,
            tiles: Cow::Borrowed(&[]),
        }
    }

    pub fn tile(&self, id: i32) -> Option<&TileSetTile2D> {
        self.tiles.iter().find(|tile| tile.id == id)
    }

    pub fn is_empty(&self) -> bool {
        self.texture.is_empty() || self.tile_size[0] <= 0.0 || self.tile_size[1] <= 0.0
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct TileSetTile2D {
    pub id: i32,
    pub atlas: [u32; 2],
    pub collision: bool,
    pub collision_shape: TileSetCollisionShape2D,
}

#[derive(Debug, Clone, PartialEq)]
pub enum TileSetCollisionShape2D {
    Auto,
    Shape {
        shape: TileSetShape2D,
        offset: [f32; 2],
    },
    Polygon {
        points: Cow<'static, [perro_structs::Vector2]>,
        offset: [f32; 2],
    },
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum TileSetShape2D {
    Rect { width: f32, height: f32 },
    Circle { radius: f32 },
    Triangle { width: f32, height: f32 },
}

pub fn encode_tileset_2d_binary(tileset: &TileSet2D) -> Vec<u8> {
    let texture = tileset.texture.as_ref().as_bytes();
    let mut out = Vec::with_capacity(32 + texture.len() + tileset.tiles.len() * 32);
    out.extend_from_slice(TILESET2D_MAGIC);
    write_u32(&mut out, TILESET2D_VERSION);
    write_u32(&mut out, texture.len() as u32);
    out.extend_from_slice(texture);
    write_f32(&mut out, tileset.tile_size[0]);
    write_f32(&mut out, tileset.tile_size[1]);
    write_u32(&mut out, tileset.columns);
    write_u32(&mut out, tileset.rows);
    write_u32(&mut out, tileset.tiles.len() as u32);
    for tile in tileset.tiles.iter() {
        write_i32(&mut out, tile.id);
        write_u32(&mut out, tile.atlas[0]);
        write_u32(&mut out, tile.atlas[1]);
        out.push(u8::from(tile.collision));
        encode_tileset_collision_shape(&mut out, &tile.collision_shape);
    }
    out
}

pub fn decode_tileset_2d_binary(bytes: &[u8]) -> Option<TileSet2D> {
    let mut cursor = 0usize;
    if read_bytes(bytes, &mut cursor, TILESET2D_MAGIC.len())? != TILESET2D_MAGIC {
        return None;
    }
    let version = read_u32(bytes, &mut cursor)?;
    if version != TILESET2D_VERSION {
        return None;
    }
    let texture_len = read_u32(bytes, &mut cursor)? as usize;
    let texture = std::str::from_utf8(read_bytes(bytes, &mut cursor, texture_len)?)
        .ok()?
        .to_string();
    let tile_size = [read_f32(bytes, &mut cursor)?, read_f32(bytes, &mut cursor)?];
    let columns = read_u32(bytes, &mut cursor)?;
    let rows = read_u32(bytes, &mut cursor)?;
    let tile_count = read_u32(bytes, &mut cursor)? as usize;
    let mut tiles = Vec::with_capacity(tile_count);
    for _ in 0..tile_count {
        let id = read_i32(bytes, &mut cursor)?;
        let atlas = [read_u32(bytes, &mut cursor)?, read_u32(bytes, &mut cursor)?];
        let collision = read_u8(bytes, &mut cursor)? != 0;
        let collision_shape = decode_tileset_collision_shape(bytes, &mut cursor)?;
        tiles.push(TileSetTile2D {
            id,
            atlas,
            collision,
            collision_shape,
        });
    }
    if cursor != bytes.len() || texture.is_empty() || tile_size[0] <= 0.0 || tile_size[1] <= 0.0 {
        return None;
    }
    Some(TileSet2D {
        texture: Cow::Owned(texture),
        tile_size,
        columns,
        rows,
        tiles: Cow::Owned(tiles),
    })
}

fn encode_tileset_collision_shape(out: &mut Vec<u8>, shape: &TileSetCollisionShape2D) {
    match shape {
        TileSetCollisionShape2D::Auto => out.push(0),
        TileSetCollisionShape2D::Shape { shape, offset } => {
            out.push(1);
            match *shape {
                TileSetShape2D::Rect { width, height } => {
                    out.push(0);
                    write_f32(out, width);
                    write_f32(out, height);
                }
                TileSetShape2D::Circle { radius } => {
                    out.push(1);
                    write_f32(out, radius);
                }
                TileSetShape2D::Triangle { width, height } => {
                    out.push(2);
                    write_f32(out, width);
                    write_f32(out, height);
                }
            }
            write_f32(out, offset[0]);
            write_f32(out, offset[1]);
        }
        TileSetCollisionShape2D::Polygon { points, offset } => {
            out.push(2);
            write_u32(out, points.len() as u32);
            for point in points.iter() {
                write_f32(out, point.x);
                write_f32(out, point.y);
            }
            write_f32(out, offset[0]);
            write_f32(out, offset[1]);
        }
    }
}

fn decode_tileset_collision_shape(
    bytes: &[u8],
    cursor: &mut usize,
) -> Option<TileSetCollisionShape2D> {
    match read_u8(bytes, cursor)? {
        0 => Some(TileSetCollisionShape2D::Auto),
        1 => {
            let shape = match read_u8(bytes, cursor)? {
                0 => TileSetShape2D::Rect {
                    width: read_f32(bytes, cursor)?,
                    height: read_f32(bytes, cursor)?,
                },
                1 => TileSetShape2D::Circle {
                    radius: read_f32(bytes, cursor)?,
                },
                2 => TileSetShape2D::Triangle {
                    width: read_f32(bytes, cursor)?,
                    height: read_f32(bytes, cursor)?,
                },
                _ => return None,
            };
            let offset = [read_f32(bytes, cursor)?, read_f32(bytes, cursor)?];
            Some(TileSetCollisionShape2D::Shape { shape, offset })
        }
        2 => {
            let count = read_u32(bytes, cursor)? as usize;
            let mut points = Vec::with_capacity(count);
            for _ in 0..count {
                points.push(perro_structs::Vector2::new(
                    read_f32(bytes, cursor)?,
                    read_f32(bytes, cursor)?,
                ));
            }
            if points.len() < 3 {
                return None;
            }
            let offset = [read_f32(bytes, cursor)?, read_f32(bytes, cursor)?];
            Some(TileSetCollisionShape2D::Polygon {
                points: Cow::Owned(points),
                offset,
            })
        }
        _ => None,
    }
}

pub fn parse_ptileset_source(source: &str) -> Option<TileSet2D> {
    let mut texture = String::new();
    let mut tile_size = [0.0, 0.0];
    let mut columns = 0u32;
    let mut rows = 0u32;
    let mut tiles = Vec::new();
    let compact = source.replace('\n', " ");
    for raw in source.lines() {
        let line = raw.trim();
        if line.starts_with("texture") {
            texture = parse_quoted_value(line)?;
        } else if line.starts_with("tile_size") {
            tile_size = parse_vec2_u32(line).map(|v| [v[0] as f32, v[1] as f32])?;
        } else if line.starts_with("columns") {
            columns = parse_u32_after_eq(line)?;
        } else if line.starts_with("rows") {
            rows = parse_u32_after_eq(line)?;
        }
    }
    for object in extract_braced_objects(&compact) {
        let id = find_i32_field(object, "id")?;
        let atlas = find_vec2_field(object, "atlas")?;
        let collision = find_bool_field(object, "collision").unwrap_or(false);
        let collision_shape =
            find_collision_shape_field(object).unwrap_or(TileSetCollisionShape2D::Auto);
        tiles.push(TileSetTile2D {
            id,
            atlas,
            collision,
            collision_shape,
        });
    }
    if texture.is_empty() || tile_size[0] <= 0.0 || tile_size[1] <= 0.0 {
        return None;
    }
    tiles.sort_by_key(|tile| tile.id);
    Some(TileSet2D {
        texture: Cow::Owned(texture),
        tile_size,
        columns,
        rows,
        tiles: Cow::Owned(tiles),
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

fn read_bytes<'a>(bytes: &'a [u8], cursor: &mut usize, len: usize) -> Option<&'a [u8]> {
    let end = cursor.checked_add(len)?;
    let out = bytes.get(*cursor..end)?;
    *cursor = end;
    Some(out)
}

fn read_u8(bytes: &[u8], cursor: &mut usize) -> Option<u8> {
    let value = *bytes.get(*cursor)?;
    *cursor += 1;
    Some(value)
}

fn read_u32(bytes: &[u8], cursor: &mut usize) -> Option<u32> {
    let raw: [u8; 4] = read_bytes(bytes, cursor, 4)?.try_into().ok()?;
    Some(u32::from_le_bytes(raw))
}

fn read_i32(bytes: &[u8], cursor: &mut usize) -> Option<i32> {
    let raw: [u8; 4] = read_bytes(bytes, cursor, 4)?.try_into().ok()?;
    Some(i32::from_le_bytes(raw))
}

fn read_f32(bytes: &[u8], cursor: &mut usize) -> Option<f32> {
    let raw: [u8; 4] = read_bytes(bytes, cursor, 4)?.try_into().ok()?;
    Some(f32::from_le_bytes(raw))
}

fn extract_braced_objects(text: &str) -> Vec<&str> {
    let mut out = Vec::new();
    let mut depth = 0usize;
    let mut start = None;
    for (idx, ch) in text.char_indices() {
        match ch {
            '{' => {
                if depth == 0 {
                    start = Some(idx + ch.len_utf8());
                }
                depth += 1;
            }
            '}' => {
                depth = depth.saturating_sub(1);
                if depth == 0
                    && let Some(start_idx) = start.take()
                {
                    out.push(&text[start_idx..idx]);
                }
            }
            _ => {}
        }
    }
    out
}

fn parse_quoted_value(line: &str) -> Option<String> {
    let (_, rest) = line.split_once('=')?;
    let rest = rest.trim();
    Some(rest.strip_prefix('"')?.split('"').next()?.to_string())
}

fn parse_u32_after_eq(line: &str) -> Option<u32> {
    line.split_once('=')?.1.trim().parse().ok()
}

fn parse_vec2_u32(line: &str) -> Option<[u32; 2]> {
    let (_, rest) = line.split_once('=')?;
    parse_vec2_inner(rest)
}

fn find_i32_field(text: &str, key: &str) -> Option<i32> {
    let rest = text.split(key).nth(1)?.split_once('=')?.1.trim();
    rest.split(|c: char| c == ',' || c.is_whitespace())
        .find(|v| !v.is_empty())?
        .parse()
        .ok()
}

fn find_bool_field(text: &str, key: &str) -> Option<bool> {
    let rest = text.split(key).nth(1)?.split_once('=')?.1.trim();
    if rest.starts_with("true") {
        Some(true)
    } else if rest.starts_with("false") {
        Some(false)
    } else {
        None
    }
}

fn find_vec2_field(text: &str, key: &str) -> Option<[u32; 2]> {
    parse_vec2_inner(text.split(key).nth(1)?.split_once('=')?.1)
}

fn find_collision_shape_field(text: &str) -> Option<TileSetCollisionShape2D> {
    let rest = text
        .split("collision_shape")
        .nth(1)?
        .split_once('=')?
        .1
        .trim();
    if rest.starts_with("\"auto\"") || rest.starts_with("auto") {
        return Some(TileSetCollisionShape2D::Auto);
    }
    if let Some(rect) = rest.split("rect").nth(1) {
        let body = rect.split_once('{')?.1.rsplit_once('}')?.0;
        let size = find_vec2_f32_field(body, "size")?;
        let offset = find_vec2_f32_field(body, "offset").unwrap_or([0.0, 0.0]);
        return Some(TileSetCollisionShape2D::Shape {
            shape: TileSetShape2D::Rect {
                width: size[0],
                height: size[1],
            },
            offset,
        });
    }
    if let Some(circle) = rest.split("circle").nth(1) {
        let body = circle.split_once('{')?.1.rsplit_once('}')?.0;
        let radius = find_f32_field(body, "radius")?;
        let offset = find_vec2_f32_field(body, "offset").unwrap_or([0.0, 0.0]);
        return Some(TileSetCollisionShape2D::Shape {
            shape: TileSetShape2D::Circle { radius },
            offset,
        });
    }
    if let Some(triangle) = rest.split("triangle").nth(1) {
        let body = triangle.split_once('{')?.1.rsplit_once('}')?.0;
        let size = find_vec2_f32_field(body, "size").or_else(|| {
            Some([
                find_f32_field(body, "width")?,
                find_f32_field(body, "height")?,
            ])
        })?;
        let offset = find_vec2_f32_field(body, "offset").unwrap_or([0.0, 0.0]);
        return Some(TileSetCollisionShape2D::Shape {
            shape: TileSetShape2D::Triangle {
                width: size[0],
                height: size[1],
            },
            offset,
        });
    }
    if let Some(polygon) = rest.split("polygon").nth(1) {
        let body = polygon.split_once('{')?.1.rsplit_once('}')?.0;
        let points = find_vec2_f32_array_field(body, "points")?;
        let offset = find_vec2_f32_field(body, "offset").unwrap_or([0.0, 0.0]);
        return Some(TileSetCollisionShape2D::Polygon {
            points: Cow::Owned(points),
            offset,
        });
    }
    None
}

fn find_f32_field(text: &str, key: &str) -> Option<f32> {
    let rest = text.split(key).nth(1)?.split_once('=')?.1.trim();
    rest.split(|c: char| c == ',' || c.is_whitespace() || c == '}')
        .find(|v| !v.is_empty())?
        .parse()
        .ok()
}

fn find_vec2_f32_field(text: &str, key: &str) -> Option<[f32; 2]> {
    parse_vec2_f32_inner(text.split(key).nth(1)?.split_once('=')?.1)
}

fn find_vec2_f32_array_field(text: &str, key: &str) -> Option<Vec<perro_structs::Vector2>> {
    let rest = text.split(key).nth(1)?.split_once('=')?.1.trim();
    let inner = rest.strip_prefix('[')?.split_once(']')?.0;
    let mut points = Vec::new();
    for raw in inner.split(')').filter(|part| part.contains('(')) {
        let pair = raw.rsplit_once('(')?.1;
        let mut it = pair.split(',').map(|v| v.trim().parse::<f32>().ok());
        points.push(perro_structs::Vector2::new(it.next()??, it.next()??));
    }
    (points.len() >= 3).then_some(points)
}

fn parse_vec2_f32_inner(text: &str) -> Option<[f32; 2]> {
    let inner = text.trim().strip_prefix('(')?.split_once(')')?.0;
    let mut parts = inner.split(',').map(|v| v.trim().parse::<f32>().ok());
    Some([parts.next()??, parts.next()??])
}

fn parse_vec2_inner(text: &str) -> Option<[u32; 2]> {
    let inner = text.trim().strip_prefix('(')?.split_once(')')?.0;
    let mut parts = inner.split(',').map(|v| v.trim().parse::<u32>().ok());
    Some([parts.next()??, parts.next()??])
}

impl Default for Sprite2DCommand {
    fn default() -> Self {
        Self {
            texture: TextureID::nil(),
            model: [[1.0, 0.0, 0.0], [0.0, 1.0, 0.0], [0.0, 0.0, 1.0]],
            tint: Color::WHITE,
            uv_min: [0.0, 0.0],
            uv_max: [0.0, 0.0],
            size: [0.0, 0.0],
            z_index: 0,
        }
    }
}
