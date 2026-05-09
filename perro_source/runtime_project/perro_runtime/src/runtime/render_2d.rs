use super::Runtime;
use ahash::{AHashMap, AHashSet};
use perro_ids::{NodeID, TextureID, parse_hashed_source_uri, string_to_u64};
use perro_nodes::{
    SceneNodeData, Shape2D, Triangle2DKind, particle_emitter_2d::ParticleEmitterSimMode2D,
};
use perro_particle_math::compile_expression;
use perro_render_bridge::{
    Camera2DState, Command2D, ParticlePath2D, ParticleProfile2D, ParticleSimulationMode2D,
    PointParticles2DState, RenderCommand, RenderRequestID, ResourceCommand, Sprite2DCommand,
    TileMap2DCommand,
};
use std::borrow::Cow;
use std::sync::Arc;

const PARTICLE_PATH_CACHE_MAX: usize = 256;

struct Sprite2DEmit {
    texture: TextureID,
    texture_region: Option<[f32; 4]>,
    model: [[f32; 3]; 3],
    z_index: i32,
}

#[derive(Clone, Debug, PartialEq)]
pub(crate) struct ParsedTileset2D {
    pub texture: String,
    pub tile_size: [f32; 2],
    pub columns: u32,
    pub rows: u32,
    pub tiles: AHashMap<i32, ParsedTile2D>,
}

#[derive(Clone, Debug, PartialEq)]
pub(crate) struct ParsedTile2D {
    pub atlas: [u32; 2],
    pub collision: bool,
    pub collision_shape: ParsedTileCollisionShape2D,
}

#[derive(Clone, Debug, PartialEq)]
pub(crate) enum ParsedTileCollisionShape2D {
    Auto,
    Shape {
        shape: Shape2D,
        offset: [f32; 2],
    },
    Polygon {
        points: std::sync::Arc<[perro_structs::Vector2]>,
        offset: [f32; 2],
    },
}

impl Runtime {
    fn sprite_texture_request(node: NodeID) -> RenderRequestID {
        RenderRequestID::new((node.as_u64() << 8) | 0x2D)
    }

    pub fn extract_render_2d_commands(&mut self) {
        let bootstrap_scan = self.render_2d.prev_visible.is_empty()
            && self.render_2d.retained_sprites.is_empty()
            && self.render_2d.last_camera.is_none();
        let has_extraction_work = self.dirty.has_any_dirty()
            || self.dirty.has_pending_transform_roots()
            || !self.render_2d.removed_nodes.is_empty()
            || bootstrap_scan;
        if !has_extraction_work {
            return;
        }

        self.propagate_pending_transform_dirty();
        self.refresh_dirty_global_transforms();

        let mut traversal_ids = std::mem::take(&mut self.render_2d.traversal_ids);
        traversal_ids.clear();
        traversal_ids.extend(
            self.dirty
                .dirty_indices()
                .iter()
                .filter_map(|&raw_index| self.nodes.slot_get(raw_index as usize).map(|(id, _)| id)),
        );
        if traversal_ids.is_empty() && bootstrap_scan {
            traversal_ids.extend(self.nodes.iter().map(|(id, _)| id));
        }
        let mut traversal_seen: AHashSet<NodeID> = traversal_ids.iter().copied().collect();
        let mut traversal_cursor = 0usize;
        while traversal_cursor < traversal_ids.len() {
            let node = traversal_ids[traversal_cursor];
            traversal_cursor += 1;
            if let Some(node_ref) = self.nodes.get(node) {
                for &child in node_ref.get_children_ids() {
                    if traversal_seen.insert(child) {
                        traversal_ids.push(child);
                    }
                }
            }
        }

        let mut visible_now = std::mem::take(&mut self.render_2d.visible_now);
        visible_now.clear();
        visible_now.extend(self.render_2d.prev_visible.iter().copied());
        let mut removed_nodes = std::mem::take(&mut self.render_2d.removed_nodes);
        for node in removed_nodes.drain(..) {
            visible_now.remove(&node);
        }
        self.render_2d.removed_nodes = removed_nodes;

        for node in traversal_ids.iter().copied() {
            visible_now.remove(&node);
            let effective_visible = self.is_effectively_visible(node);
            let sprite_data = self.nodes.get(node).and_then(|node| match &node.data {
                SceneNodeData::Sprite2D(sprite) => Some((
                    effective_visible && sprite.visible,
                    sprite.texture,
                    sprite.texture_region,
                    sprite.transform,
                    sprite.z_index,
                )),
                SceneNodeData::AnimatedSprite2D(sprite) => Some((
                    effective_visible && sprite.visible,
                    sprite.texture,
                    sprite.current_texture_region(),
                    sprite.transform,
                    sprite.z_index,
                )),
                _ => None,
            });
            if let Some((visible, texture, texture_region, local_transform, z_index)) = sprite_data
            {
                let model = self
                    .get_global_transform_2d(node)
                    .unwrap_or(local_transform)
                    .to_mat3()
                    .to_cols_array_2d();
                self.emit_sprite_2d(
                    node,
                    visible,
                    Sprite2DEmit {
                        texture,
                        texture_region,
                        model,
                        z_index,
                    },
                    &mut visible_now,
                );
            }

            let camera_data = self.nodes.get(node).and_then(|node| match &node.data {
                SceneNodeData::Camera2D(camera) if camera.active && effective_visible => Some((
                    camera.transform,
                    camera.zoom,
                    camera.post_processing.clone(),
                )),
                _ => None,
            });
            let camera_data = camera_data.map(|(local_transform, zoom, post_processing)| {
                let global = self
                    .get_global_transform_2d(node)
                    .unwrap_or(local_transform);
                Camera2DState {
                    position: [global.position.x, global.position.y],
                    rotation_radians: global.rotation,
                    zoom,
                    post_processing: Arc::from(post_processing.to_effects_vec()),
                }
            });
            if let Some(camera) = camera_data
                && self.render_2d.last_camera.as_ref() != Some(&camera)
            {
                self.queue_render_command(RenderCommand::TwoD(Command2D::SetCamera {
                    camera: camera.clone(),
                }));
                self.render_2d.last_camera = Some(camera);
            }

            let point_emitter_data = self.nodes.get(node).and_then(|node| match &node.data {
                SceneNodeData::ParticleEmitter2D(emitter) => Some((
                    effective_visible && emitter.visible,
                    emitter.profile.clone(),
                    emitter.sim_mode,
                    emitter.transform,
                    emitter.z_index,
                    emitter.active,
                    emitter.looping,
                    emitter.prewarm,
                    emitter.spawn_rate,
                    emitter.seed,
                    emitter.params.clone(),
                    emitter.internal_simulation_time,
                )),
                _ => None,
            });
            if let Some((
                visible,
                emitter_profile,
                emitter_sim_mode,
                emitter_transform,
                emitter_z_index,
                emitter_active,
                emitter_looping,
                emitter_prewarm,
                emitter_spawn_rate,
                emitter_seed,
                emitter_params,
                emitter_simulation_time,
            )) = point_emitter_data
            {
                if visible {
                    let profile =
                        resolve_particle_profile_2d(self, &emitter_profile).unwrap_or_default();
                    let lifetime_min = profile.lifetime_min.max(0.001);
                    let lifetime_max = profile.lifetime_max.max(lifetime_min);
                    if let Some(node_mut) = self.nodes.get_mut(node)
                        && let SceneNodeData::ParticleEmitter2D(emitter_mut) = &mut node_mut.data
                    {
                        emitter_mut.internal_lifetime_max = lifetime_max;
                    }
                    let model = self
                        .get_global_transform_2d(node)
                        .unwrap_or(emitter_transform)
                        .to_mat3()
                        .to_cols_array_2d();
                    self.queue_render_command(RenderCommand::TwoD(
                        Command2D::UpsertPointParticles {
                            node,
                            particles: Box::new(PointParticles2DState {
                                model,
                                z_index: emitter_z_index,
                                active: emitter_active,
                                looping: emitter_looping,
                                prewarm: emitter_prewarm,
                                alive_budget: derived_particle_budget(
                                    emitter_spawn_rate.max(0.0),
                                    lifetime_max,
                                ),
                                emission_rate: emitter_spawn_rate.max(0.0),
                                lifetime_min,
                                lifetime_max,
                                speed_min: profile.speed_min.max(0.0),
                                speed_max: profile.speed_max.max(profile.speed_min.max(0.0)),
                                spread_radians: profile
                                    .spread_radians
                                    .clamp(0.0, std::f32::consts::TAU),
                                size: profile.size.max(1.0),
                                size_min: profile.size_min.max(0.01),
                                size_max: profile.size_max.max(profile.size_min.max(0.01)),
                                force: profile.force,
                                color_start: profile.color_start,
                                color_end: profile.color_end,
                                seed: emitter_seed,
                                params: emitter_params,
                                simulation_time: emitter_simulation_time,
                                simulation_delta: 0.0,
                                profile,
                                sim_mode: resolve_particle_sim_mode_2d(emitter_sim_mode),
                            }),
                        },
                    ));
                    visible_now.insert(node);
                } else {
                    self.queue_render_command(RenderCommand::TwoD(Command2D::RemoveNode { node }));
                    self.render_2d.retained_sprites.remove(&node);
                }
            }

            let tilemap_data = self.nodes.get(node).and_then(|node| match &node.data {
                SceneNodeData::TileMap2D(tilemap) => Some((
                    effective_visible && tilemap.visible,
                    tilemap.tileset.clone(),
                    tilemap.width,
                    tilemap.height,
                    tilemap.empty_tile,
                    tilemap.tiles.clone(),
                    tilemap.transform,
                    tilemap.z_index,
                )),
                _ => None,
            });
            if let Some((
                visible,
                tileset_source,
                width,
                height,
                empty_tile,
                tiles,
                local_transform,
                z_index,
            )) = tilemap_data
            {
                if visible {
                    if let Some(tileset) = resolve_tileset_2d(self, &tileset_source)
                        && let Some(texture) =
                            self.resolve_tilemap_texture(node, tileset.texture.as_str())
                    {
                        let global = self
                            .get_global_transform_2d(node)
                            .unwrap_or(local_transform)
                            .to_mat3()
                            .to_cols_array_2d();
                        let sprites = build_tilemap_sprites(TilemapSpriteBuild {
                            texture,
                            base_model: global,
                            z_index,
                            width,
                            height,
                            empty_tile,
                            tiles: &tiles,
                            tileset: &tileset,
                        });
                        self.queue_render_command(RenderCommand::TwoD(Command2D::UpsertTileMap {
                            node,
                            tilemap: TileMap2DCommand {
                                texture,
                                sprites: Arc::from(sprites),
                            },
                        }));
                        visible_now.insert(node);
                    }
                } else {
                    self.queue_render_command(RenderCommand::TwoD(Command2D::RemoveNode { node }));
                    self.render_2d.retained_sprites.remove(&node);
                }
            }
        }
        self.remove_no_longer_visible_render_2d_nodes(&visible_now);

        std::mem::swap(&mut self.render_2d.prev_visible, &mut visible_now);
        visible_now.clear();
        self.render_2d.visible_now = visible_now;

        traversal_ids.clear();
        self.render_2d.traversal_ids = traversal_ids;
    }

    fn emit_sprite_2d(
        &mut self,
        node: NodeID,
        visible: bool,
        emit: Sprite2DEmit,
        visible_now: &mut AHashSet<NodeID>,
    ) {
        if !visible {
            return;
        }

        let Some(resolved_texture) = self.resolve_sprite_texture(node, emit.texture) else {
            return;
        };

        let (uv_min, uv_max, size) = sprite_region_uv(emit.texture_region);
        let sprite = Sprite2DCommand {
            texture: resolved_texture,
            model: emit.model,
            tint: [1.0, 1.0, 1.0, 1.0],
            uv_min,
            uv_max,
            size,
            z_index: emit.z_index,
        };
        let needs_upsert = self
            .render_2d
            .retained_sprites
            .get(&node)
            .is_none_or(|cached| *cached != sprite);
        if needs_upsert {
            self.queue_render_command(RenderCommand::TwoD(Command2D::UpsertSprite {
                node,
                sprite,
            }));
            self.render_2d.retained_sprites.insert(node, sprite);
        }
        visible_now.insert(node);
    }

    fn resolve_sprite_texture(
        &mut self,
        node: NodeID,
        mut texture: TextureID,
    ) -> Option<TextureID> {
        if texture.is_nil() {
            let request = Self::sprite_texture_request(node);
            if let Some(result) = self.take_render_result(request) {
                match result {
                    crate::RuntimeRenderResult::Texture(id) => {
                        texture = id;
                        if let Some(node) = self.nodes.get_mut(node) {
                            match &mut node.data {
                                SceneNodeData::Sprite2D(sprite) => sprite.texture = id,
                                SceneNodeData::AnimatedSprite2D(sprite) => sprite.texture = id,
                                _ => {}
                            }
                        }
                    }
                    crate::RuntimeRenderResult::Failed(_) => {}
                    crate::RuntimeRenderResult::Mesh(_)
                    | crate::RuntimeRenderResult::Material(_) => {}
                }
            }
        }

        if texture.is_nil() {
            let request = Self::sprite_texture_request(node);
            if !self.render.is_inflight(request) {
                let source = self
                    .render_2d
                    .texture_sources
                    .get(&node)
                    .cloned()
                    .unwrap_or_else(|| "__default__".to_string());
                self.render.mark_inflight(request);
                self.queue_render_command(RenderCommand::Resource(
                    ResourceCommand::CreateTexture {
                        request,
                        id: TextureID::nil(),
                        source,
                        reserved: false,
                    },
                ));
            }
            return None;
        }

        Some(texture)
    }

    fn resolve_tilemap_texture(&mut self, node: NodeID, source: &str) -> Option<TextureID> {
        let request = RenderRequestID::new((node.as_u64() << 8) | 0x71);
        if let Some(result) = self.take_render_result(request) {
            return match result {
                crate::RuntimeRenderResult::Texture(id) => Some(id),
                crate::RuntimeRenderResult::Failed(_) => None,
                crate::RuntimeRenderResult::Mesh(_) | crate::RuntimeRenderResult::Material(_) => {
                    None
                }
            };
        }
        if !self.render.is_inflight(request) {
            self.render.mark_inflight(request);
            self.queue_render_command(RenderCommand::Resource(ResourceCommand::CreateTexture {
                request,
                id: TextureID::nil(),
                source: source.to_string(),
                reserved: false,
            }));
        }
        None
    }

    fn remove_no_longer_visible_render_2d_nodes(&mut self, visible_now: &AHashSet<NodeID>) {
        for node in self.render_2d.prev_visible.iter().copied() {
            if !visible_now.contains(&node) {
                self.render_2d.removed_nodes.push(node);
            }
        }
        while let Some(node) = self.render_2d.removed_nodes.pop() {
            self.queue_render_command(RenderCommand::TwoD(Command2D::RemoveNode { node }));
            self.render_2d.retained_sprites.remove(&node);
        }
    }
}

pub(crate) fn resolve_tileset_2d(runtime: &mut Runtime, source: &str) -> Option<ParsedTileset2D> {
    if let Some(tileset) = runtime.render_2d.tileset_cache.get(source) {
        return Some(tileset.clone());
    }
    let static_text = if runtime.provider_mode() == crate::runtime_project::ProviderMode::Static {
        runtime
            .project()
            .and_then(|project| project.static_tileset_lookup)
            .map(|lookup| lookup(string_to_u64(source)))
            .filter(|text| !text.is_empty())
    } else {
        None
    };
    let bytes;
    let text = if let Some(text) = static_text {
        text
    } else {
        bytes = perro_io::load_asset(source).ok()?;
        std::str::from_utf8(&bytes).ok()?
    };
    let tileset = parse_ptileset_source(text)?;
    runtime
        .render_2d
        .tileset_cache
        .insert(source.to_string(), tileset.clone());
    Some(tileset)
}

struct TilemapSpriteBuild<'a> {
    texture: TextureID,
    width: u32,
    height: u32,
    z_index: i32,
    empty_tile: i32,
    base_model: [[f32; 3]; 3],
    tiles: &'a [i32],
    tileset: &'a ParsedTileset2D,
}

fn build_tilemap_sprites(build: TilemapSpriteBuild<'_>) -> Vec<Sprite2DCommand> {
    let max = (build.width as usize)
        .saturating_mul(build.height as usize)
        .min(build.tiles.len());
    let mut out = Vec::with_capacity(max);
    let [tw, th] = build.tileset.tile_size;
    for (idx, tile_id) in build.tiles.iter().take(max).copied().enumerate() {
        if tile_id == build.empty_tile {
            continue;
        }
        let Some(tile) = build.tileset.tiles.get(&tile_id) else {
            continue;
        };
        let x = (idx as u32 % build.width) as f32 * tw;
        let y = (idx as u32 / build.width) as f32 * th;
        let model = mul_mat3(build.base_model, translation_mat3(x, -y));
        let atlas_x = tile.atlas[0] as f32 * tw;
        let atlas_y = tile.atlas[1] as f32 * th;
        out.push(Sprite2DCommand {
            texture: build.texture,
            model,
            tint: [1.0, 1.0, 1.0, 1.0],
            uv_min: [atlas_x, atlas_y],
            uv_max: [atlas_x + tw, atlas_y + th],
            size: [tw, th],
            z_index: build.z_index,
        });
    }
    out
}

fn parse_ptileset_source(source: &str) -> Option<ParsedTileset2D> {
    let mut texture = String::new();
    let mut tile_size = [0.0, 0.0];
    let mut columns = 0u32;
    let mut rows = 0u32;
    let mut tiles = AHashMap::new();
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
            find_collision_shape_field(object).unwrap_or(ParsedTileCollisionShape2D::Auto);
        tiles.insert(
            id,
            ParsedTile2D {
                atlas,
                collision,
                collision_shape,
            },
        );
    }
    if texture.is_empty() || tile_size[0] <= 0.0 || tile_size[1] <= 0.0 {
        return None;
    }
    Some(ParsedTileset2D {
        texture,
        tile_size,
        columns,
        rows,
        tiles,
    })
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

fn find_collision_shape_field(text: &str) -> Option<ParsedTileCollisionShape2D> {
    let rest = text
        .split("collision_shape")
        .nth(1)?
        .split_once('=')?
        .1
        .trim();
    if rest.starts_with("\"auto\"") || rest.starts_with("auto") {
        return Some(ParsedTileCollisionShape2D::Auto);
    }
    if let Some(rect) = rest.split("rect").nth(1) {
        let body = rect.split_once('{')?.1.rsplit_once('}')?.0;
        let size = find_vec2_f32_field(body, "size")?;
        let offset = find_vec2_f32_field(body, "offset").unwrap_or([0.0, 0.0]);
        return Some(ParsedTileCollisionShape2D::Shape {
            shape: Shape2D::Quad {
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
        return Some(ParsedTileCollisionShape2D::Shape {
            shape: Shape2D::Circle { radius },
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
        return Some(ParsedTileCollisionShape2D::Shape {
            shape: Shape2D::Triangle {
                kind: Triangle2DKind::Isosceles,
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
        return Some(ParsedTileCollisionShape2D::Polygon {
            points: std::sync::Arc::from(points.into_boxed_slice()),
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

fn translation_mat3(x: f32, y: f32) -> [[f32; 3]; 3] {
    [[1.0, 0.0, 0.0], [0.0, 1.0, 0.0], [x, y, 1.0]]
}

fn mul_mat3(a: [[f32; 3]; 3], b: [[f32; 3]; 3]) -> [[f32; 3]; 3] {
    let mut out = [[0.0; 3]; 3];
    for c in 0..3 {
        for r in 0..3 {
            out[c][r] = a[0][r] * b[c][0] + a[1][r] * b[c][1] + a[2][r] * b[c][2];
        }
    }
    out
}

fn derived_particle_budget(spawn_rate: f32, lifetime_max: f32) -> u32 {
    if spawn_rate <= 0.0 || lifetime_max <= 0.0 {
        return 1;
    }
    let budget = (spawn_rate * lifetime_max).ceil() as u32 + 2;
    budget.clamp(1, 1_000_000)
}

fn resolve_particle_sim_mode_2d(mode: ParticleEmitterSimMode2D) -> ParticleSimulationMode2D {
    match mode {
        ParticleEmitterSimMode2D::Default | ParticleEmitterSimMode2D::Cpu => {
            ParticleSimulationMode2D::Cpu
        }
    }
}

fn resolve_particle_profile_2d(runtime: &mut Runtime, source: &str) -> Option<ParticleProfile2D> {
    let source = source.trim();
    if source.is_empty() {
        return None;
    }
    if let Some(path) = runtime.render_2d.particle_path_cache.get(source) {
        return Some(path.clone());
    }
    let parsed = if runtime.provider_mode() == crate::runtime_project::ProviderMode::Static {
        if let Some(inline) = source.strip_prefix("inline://") {
            parse_pparticle_source_2d(inline)?
        } else if let Some(lookup) = runtime
            .project()
            .and_then(|project| project.static_particle_lookup)
        {
            let source_hash =
                parse_hashed_source_uri(source).unwrap_or_else(|| string_to_u64(source));
            particle_profile_2d_from_3d(lookup(source_hash))
        } else {
            let bytes = perro_io::load_asset(source).ok()?;
            let text = std::str::from_utf8(&bytes).ok()?;
            parse_pparticle_source_2d(text)?
        }
    } else if let Some(inline) = source.strip_prefix("inline://") {
        parse_pparticle_source_2d(inline)?
    } else {
        let bytes = perro_io::load_asset(source).ok()?;
        let text = std::str::from_utf8(&bytes).ok()?;
        parse_pparticle_source_2d(text)?
    };
    if !runtime.render_2d.particle_path_cache.contains_key(source) {
        while runtime.render_2d.particle_path_cache.len() >= PARTICLE_PATH_CACHE_MAX {
            let Some(evict_key) = runtime.render_2d.particle_path_cache_order.pop_front() else {
                break;
            };
            runtime
                .render_2d
                .particle_path_cache
                .remove(evict_key.as_str());
        }
        runtime
            .render_2d
            .particle_path_cache_order
            .push_back(source.to_string());
    }
    runtime
        .render_2d
        .particle_path_cache
        .insert(source.to_string(), parsed.clone());
    Some(parsed)
}

fn particle_profile_2d_from_3d(
    profile: &perro_render_bridge::ParticleProfile3D,
) -> ParticleProfile2D {
    let path = match profile.path {
        perro_render_bridge::ParticlePath3D::None => ParticlePath2D::None,
        perro_render_bridge::ParticlePath3D::Ballistic => ParticlePath2D::Ballistic,
        perro_render_bridge::ParticlePath3D::Spiral {
            angular_velocity,
            radius,
        } => ParticlePath2D::Spiral {
            angular_velocity,
            radius,
        },
        perro_render_bridge::ParticlePath3D::NoiseDrift {
            amplitude,
            frequency,
        } => ParticlePath2D::NoiseDrift {
            amplitude,
            frequency,
        },
        perro_render_bridge::ParticlePath3D::FlatDisk { radius } => {
            ParticlePath2D::FlatDisk { radius }
        }
        perro_render_bridge::ParticlePath3D::OrbitY { .. }
        | perro_render_bridge::ParticlePath3D::Custom { .. }
        | perro_render_bridge::ParticlePath3D::CustomCompiled { .. } => ParticlePath2D::None,
    };
    ParticleProfile2D {
        path,
        expr_x_ops: profile.expr_x_ops.clone(),
        expr_y_ops: profile.expr_y_ops.clone(),
        lifetime_min: profile.lifetime_min,
        lifetime_max: profile.lifetime_max,
        speed_min: profile.speed_min,
        speed_max: profile.speed_max,
        spread_radians: profile.spread_radians,
        size: profile.size,
        size_min: profile.size_min,
        size_max: profile.size_max,
        force: [profile.force[0], profile.force[1]],
        color_start: profile.color_start,
        color_end: profile.color_end,
        spin_angular_velocity: profile.spin_angular_velocity,
    }
}

fn parse_pparticle_source_2d(source: &str) -> Option<ParticleProfile2D> {
    let mut profile = ParticleProfile2D::default();
    let mut preset: Option<String> = None;
    let mut preset_param_a = 1.0f32;
    let mut preset_param_b = 1.0f32;
    let mut expr_x = String::from("0.0");
    let mut expr_y = String::from("0.0");
    let mut has_expr_x = false;
    let mut has_expr_y = false;
    for line in source.lines() {
        let line = line.trim();
        if line.is_empty() || line.starts_with('#') || line.starts_with("//") {
            continue;
        }
        let (key, value) = line.split_once('=')?;
        let key = key.trim().to_ascii_lowercase();
        let value = value.trim();
        match key.as_str() {
            "preset" => preset = Some(value.to_ascii_lowercase()),
            "preset_param_a" => {
                preset_param_a = value.parse::<f32>().ok().unwrap_or(preset_param_a);
            }
            "preset_param_b" => {
                preset_param_b = value.parse::<f32>().ok().unwrap_or(preset_param_b);
            }
            "x" => {
                expr_x = value.to_string();
                has_expr_x = true;
            }
            "y" => {
                expr_y = value.to_string();
                has_expr_y = true;
            }
            "force" => {
                if let Some(v) = parse_vec2_or_vec3_literal_2d(value) {
                    profile.force = v;
                }
            }
            "force_x" => profile.force[0] = value.parse::<f32>().ok()?,
            "force_y" => profile.force[1] = value.parse::<f32>().ok()?,
            "lifetime_min" => {
                profile.lifetime_min = value.parse::<f32>().ok().unwrap_or(profile.lifetime_min);
            }
            "lifetime_max" => {
                profile.lifetime_max = value.parse::<f32>().ok().unwrap_or(profile.lifetime_max);
            }
            "speed_min" => {
                profile.speed_min = value.parse::<f32>().ok().unwrap_or(profile.speed_min)
            }
            "speed_max" => {
                profile.speed_max = value.parse::<f32>().ok().unwrap_or(profile.speed_max)
            }
            "spread_radians" => {
                profile.spread_radians =
                    value.parse::<f32>().ok().unwrap_or(profile.spread_radians);
            }
            "size" => profile.size = value.parse::<f32>().ok().unwrap_or(profile.size),
            "size_min" => profile.size_min = value.parse::<f32>().ok().unwrap_or(profile.size_min),
            "size_max" => profile.size_max = value.parse::<f32>().ok().unwrap_or(profile.size_max),
            "color_start" => {
                if let Some(v) = parse_vec4_literal_2d(value) {
                    profile.color_start = v;
                }
            }
            "color_end" => {
                if let Some(v) = parse_vec4_literal_2d(value) {
                    profile.color_end = v;
                }
            }
            "spin" => {
                profile.spin_angular_velocity = value
                    .parse::<f32>()
                    .ok()
                    .unwrap_or(profile.spin_angular_velocity);
            }
            _ => {}
        }
    }
    profile.path = match preset.as_deref() {
        None => ParticlePath2D::None,
        Some("ballistic") => ParticlePath2D::Ballistic,
        Some("spiral") => ParticlePath2D::Spiral {
            angular_velocity: preset_param_a,
            radius: preset_param_b.abs(),
        },
        Some("noise_drift") => ParticlePath2D::NoiseDrift {
            amplitude: preset_param_a.abs(),
            frequency: preset_param_b.abs(),
        },
        Some("flat_disk") => ParticlePath2D::FlatDisk {
            radius: preset_param_a.abs(),
        },
        Some("orbit_y") | Some(_) => ParticlePath2D::None,
    };
    if has_expr_x || has_expr_y {
        profile.expr_x_ops = Some(Cow::Owned(compile_expression(&expr_x).ok()?.ops().to_vec()));
        profile.expr_y_ops = Some(Cow::Owned(compile_expression(&expr_y).ok()?.ops().to_vec()));
    }
    Some(profile)
}

fn parse_vec2_or_vec3_literal_2d(raw: &str) -> Option<[f32; 2]> {
    let raw = raw.trim();
    let inner = raw.strip_prefix('(')?.strip_suffix(')')?;
    let mut it = inner.split(',').map(|v| v.trim().parse::<f32>().ok());
    Some([it.next()??, it.next()??])
}

fn parse_vec4_literal_2d(raw: &str) -> Option<[f32; 4]> {
    let raw = raw.trim();
    let inner = raw.strip_prefix('(')?.strip_suffix(')')?;
    let mut it = inner.split(',').map(|v| v.trim().parse::<f32>().ok());
    Some([it.next()??, it.next()??, it.next()??, it.next()??])
}

fn sprite_region_uv(region: Option<[f32; 4]>) -> ([f32; 2], [f32; 2], [f32; 2]) {
    let Some([x, y, w, h]) = region else {
        return ([0.0, 0.0], [1.0, 1.0], [0.0, 0.0]);
    };
    if !(x.is_finite() && y.is_finite() && w.is_finite() && h.is_finite()) || w <= 0.0 || h <= 0.0 {
        return ([0.0, 0.0], [1.0, 1.0], [0.0, 0.0]);
    }
    ([x, y], [x + w, y + h], [w, h])
}

#[cfg(test)]
#[path = "../../tests/unit/runtime_render_2d_tests.rs"]
mod tests;
