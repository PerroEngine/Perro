use crate::resources::ResourceStore;
use ahash::AHashMap;
use bytemuck::{Pod, Zeroable};
use perro_ids::NodeID;
use perro_particle_math::{ParticleEvalInput, eval_ops_particle};
use perro_render_bridge::{
    AmbientLight2DState, Camera2DState, DrawShape2DCommand, Light2DState, ParticlePath2D,
    PointLight2DState, PointParticles2DState, RayLight2DState, Rect2DCommand, ShadowCaster2DState,
    SpotLight2DState, Sprite2DCommand, TileMap2DCommand, Water2DState,
};
use perro_structs::{DrawShape2D, UnitVector4};
use std::ops::Range;

#[derive(Debug, Clone, Copy)]
struct SpritePacket {
    node: NodeID,
    sprite: Sprite2DCommand,
}

#[derive(Debug, Clone, Copy)]
struct RectPacket {
    node: NodeID,
    rect: Rect2DCommand,
}

#[derive(Debug, Clone, Copy, Default)]
pub struct Renderer2DStats {
    pub accepted_draws: u32,
    pub rejected_draws: u32,
    pub accepted_rects: u32,
    pub rejected_rects: u32,
}

#[repr(C)]
#[derive(Debug, Clone, Copy, PartialEq, Zeroable, Pod)]
pub struct Camera2DUniform {
    pub view: [[f32; 4]; 4],
    pub ndc_scale: [f32; 2],
    pub pad: [f32; 2],
}

#[repr(C)]
#[derive(Debug, Clone, Copy, PartialEq, Zeroable, Pod)]
pub struct RectInstanceGpu {
    pub center: [f32; 2],
    pub size: [f32; 2],
    pub color: [u8; 4],
    pub z_index: i32,
    // Packed shape_kind (high bits) + filled flag (low bit): kind * 2 + filled.
    // Fold saves 4 bytes of instance stride vs a separate `filled: u32`.
    pub shape_kind: u32,
    pub thickness: f32,
}

#[inline]
fn pack_shape_kind(kind: u32, filled: bool) -> u32 {
    kind * 2 + u32::from(filled)
}

#[derive(Debug, Clone, Default)]
pub struct RectUploadPlan {
    pub full_reupload: bool,
    pub dirty_ranges: Vec<Range<usize>>,
    pub draw_count: usize,
}

pub const DEFAULT_VIRTUAL_WIDTH: f32 = 1920.0;
pub const DEFAULT_VIRTUAL_HEIGHT: f32 = 1080.0;

#[derive(Default)]
pub struct Renderer2D {
    queued_sprites: Vec<SpritePacket>,
    queued_rects: Vec<RectPacket>,
    queued_shapes: Vec<DrawShape2DCommand>,
    camera: Camera2DState,
    viewport: (u32, u32),
    virtual_size: [f32; 2],
    retained_rects: Vec<RectInstanceGpu>,
    retained_nodes: Vec<NodeID>,
    node_to_rect_index: AHashMap<NodeID, usize>,
    rect_dirty_ranges: Vec<Range<usize>>,
    rect_structure_dirty: bool,
    retained_sprites: Vec<SpritePacket>,
    node_to_sprite_index: AHashMap<NodeID, usize>,
    retained_tilemaps: AHashMap<NodeID, TileMap2DCommand>,
    retained_point_particles: AHashMap<NodeID, PointParticles2DState>,
    retained_waters: AHashMap<NodeID, Water2DState>,
    retained_waters_revision: u64,
    retained_lights: AHashMap<NodeID, Light2DState>,
    retained_point_lights_revision: u64,
    retained_shadow_casters: AHashMap<NodeID, ShadowCaster2DState>,
    retained_shadow_casters_revision: u64,
    retained_sprites_revision: u64,
    frame_shapes: Vec<RectInstanceGpu>,
    frame_sprites: Vec<Sprite2DCommand>,
    // Last frame's immediate sprites, kept to detect byte-identical frames and
    // skip the revision bump that would defeat the sprite staging cache.
    prev_frame_sprites: Vec<Sprite2DCommand>,
    particle_eval_stack: Vec<f32>,
}

impl Renderer2D {
    pub fn new() -> Self {
        Self {
            queued_sprites: Vec::new(),
            queued_rects: Vec::new(),
            queued_shapes: Vec::new(),
            camera: Camera2DState::default(),
            viewport: (0, 0),
            virtual_size: [DEFAULT_VIRTUAL_WIDTH, DEFAULT_VIRTUAL_HEIGHT],
            retained_rects: Vec::new(),
            retained_nodes: Vec::new(),
            node_to_rect_index: AHashMap::new(),
            rect_dirty_ranges: Vec::new(),
            rect_structure_dirty: false,
            retained_sprites: Vec::new(),
            node_to_sprite_index: AHashMap::new(),
            retained_tilemaps: AHashMap::new(),
            retained_point_particles: AHashMap::new(),
            retained_waters: AHashMap::new(),
            retained_waters_revision: 0,
            retained_lights: AHashMap::new(),
            retained_point_lights_revision: 0,
            retained_shadow_casters: AHashMap::new(),
            retained_shadow_casters_revision: 0,
            retained_sprites_revision: 0,
            frame_shapes: Vec::new(),
            frame_sprites: Vec::new(),
            prev_frame_sprites: Vec::new(),
            particle_eval_stack: Vec::new(),
        }
    }

    #[inline]
    pub fn set_viewport(&mut self, width: u32, height: u32) {
        self.viewport = (width, height);
    }

    #[inline]
    pub fn set_camera(&mut self, camera: Camera2DState) {
        self.camera = camera;
    }

    #[inline]
    pub fn set_virtual_viewport(&mut self, width: f32, height: f32) {
        if width.is_finite() && height.is_finite() && width > 0.0 && height > 0.0 {
            self.virtual_size = [width, height];
        }
    }

    #[inline]
    pub fn camera_uniform(&self) -> Camera2DUniform {
        let view = compute_view_matrix(&self.camera);
        let ndc_scale = ndc_scale(self.viewport, self.virtual_size, self.camera.zoom);
        Camera2DUniform {
            view,
            ndc_scale,
            pad: [0.0, 0.0],
        }
    }

    pub fn queue_sprite(&mut self, node: NodeID, sprite: Sprite2DCommand) {
        self.queued_sprites.push(SpritePacket { node, sprite });
    }

    pub fn reserve_queued_sprites(&mut self, additional: usize) {
        self.queued_sprites.reserve(additional);
    }

    pub fn queue_rect(&mut self, node: NodeID, rect: Rect2DCommand) {
        self.queued_rects.push(RectPacket { node, rect });
    }

    pub fn reserve_queued_rects(&mut self, additional: usize) {
        self.queued_rects.reserve(additional);
    }

    pub fn upsert_rect(&mut self, node: NodeID, rect: Rect2DCommand) {
        self.queued_rects.push(RectPacket { node, rect });
    }

    pub fn queue_shape(&mut self, draw: DrawShape2DCommand) {
        self.queued_shapes.push(draw);
    }

    pub fn queue_point_particles(&mut self, node: NodeID, particles: PointParticles2DState) {
        self.retained_point_particles.insert(node, particles);
    }

    pub fn upsert_water(&mut self, node: NodeID, water: Water2DState) {
        match self.retained_waters.get_mut(&node) {
            Some(existing) if *existing == water => {}
            Some(existing) => {
                *existing = water;
                self.retained_waters_revision = self.retained_waters_revision.wrapping_add(1);
            }
            None => {
                self.retained_waters.insert(node, water);
                self.retained_waters_revision = self.retained_waters_revision.wrapping_add(1);
            }
        }
    }

    pub fn set_ambient_light(&mut self, node: NodeID, light: AmbientLight2DState) {
        self.set_light(node, Light2DState::Ambient(light));
    }

    pub fn set_ray_light(&mut self, node: NodeID, light: RayLight2DState) {
        self.set_light(node, Light2DState::Ray(light));
    }

    pub fn set_point_light(&mut self, node: NodeID, light: PointLight2DState) {
        self.set_light(node, Light2DState::Point(light));
    }

    pub fn set_spot_light(&mut self, node: NodeID, light: SpotLight2DState) {
        self.set_light(node, Light2DState::Spot(light));
    }

    pub fn upsert_shadow_caster(&mut self, node: NodeID, caster: ShadowCaster2DState) {
        if self.retained_shadow_casters.insert(node, caster) != Some(caster) {
            self.retained_shadow_casters_revision =
                self.retained_shadow_casters_revision.wrapping_add(1);
        }
    }

    fn set_light(&mut self, node: NodeID, light: Light2DState) {
        if self.retained_lights.insert(node, light) != Some(light) {
            self.retained_point_lights_revision =
                self.retained_point_lights_revision.wrapping_add(1);
        }
    }

    pub fn upsert_tilemap(&mut self, node: NodeID, tilemap: TileMap2DCommand) {
        let sprites_changed = self
            .retained_tilemaps
            .get(&node)
            .is_none_or(|old| old.sprites != tilemap.sprites);
        let casters_changed = self
            .retained_tilemaps
            .get(&node)
            .is_none_or(|old| old.shadow_casters != tilemap.shadow_casters);
        self.retained_tilemaps.insert(node, tilemap);
        if sprites_changed {
            self.retained_sprites_revision = self.retained_sprites_revision.wrapping_add(1);
        }
        if casters_changed {
            self.retained_shadow_casters_revision =
                self.retained_shadow_casters_revision.wrapping_add(1);
        }
    }

    pub fn remove_node(&mut self, node: NodeID) {
        self.remove_retained_rect(node);
        self.retained_point_particles.remove(&node);
        if self.retained_waters.remove(&node).is_some() {
            self.retained_waters_revision = self.retained_waters_revision.wrapping_add(1);
        }
        if self.retained_lights.remove(&node).is_some() {
            self.retained_point_lights_revision =
                self.retained_point_lights_revision.wrapping_add(1);
        }
        if self.retained_shadow_casters.remove(&node).is_some() {
            self.retained_shadow_casters_revision =
                self.retained_shadow_casters_revision.wrapping_add(1);
        }
        if let Some(tilemap) = self.retained_tilemaps.remove(&node) {
            self.retained_sprites_revision = self.retained_sprites_revision.wrapping_add(1);
            if !tilemap.shadow_casters.is_empty() {
                self.retained_shadow_casters_revision =
                    self.retained_shadow_casters_revision.wrapping_add(1);
            }
        }
        self.remove_retained_sprite(node);
    }

    fn apply_queued_rect_updates(&mut self) -> Renderer2DStats {
        let queued = std::mem::take(&mut self.queued_rects);
        let mut stats = Renderer2DStats::default();
        for RectPacket { node, rect } in queued {
            if let Some(rect) = retained_rect_instance(rect) {
                self.upsert_retained_rect(node, rect);
                stats.accepted_rects = stats.accepted_rects.saturating_add(1);
            } else {
                self.remove_retained_rect(node);
                stats.rejected_rects = stats.rejected_rects.saturating_add(1);
            }
        }
        stats
    }

    fn flush_shape_packets(&mut self) {
        // Retain last frame's immediate sprites for the change check below.
        // Swap (no clone) so the old buffer becomes this frame's scratch.
        std::mem::swap(&mut self.frame_sprites, &mut self.prev_frame_sprites);
        self.frame_shapes.clear();
        self.frame_sprites.clear();
        if self.frame_shapes.capacity() < self.queued_shapes.len() {
            self.frame_shapes
                .reserve(self.queued_shapes.len() - self.frame_shapes.capacity());
        }
        for draw in self.queued_shapes.drain(..) {
            let center = normalized_screen_to_virtual_centered(draw.position, self.virtual_size);
            match draw.shape {
                DrawShape2D::Circle {
                    radius,
                    color,
                    filled,
                    thickness,
                } => {
                    if !radius.is_finite()
                        || radius <= 0.0
                        || !color.to_rgba().iter().all(|v| v.is_finite())
                        || !thickness.is_finite()
                    {
                        continue;
                    }
                    self.frame_shapes.push(RectInstanceGpu {
                        center,
                        size: [radius * 2.0, radius * 2.0],
                        color: color_to_unorm8(color.into()),
                        z_index: 900,
                        shape_kind: pack_shape_kind(1, filled),
                        thickness: thickness.max(0.0),
                    });
                }
                DrawShape2D::Rect {
                    size,
                    color,
                    filled,
                    thickness,
                } => {
                    if !size.x.is_finite()
                        || !size.y.is_finite()
                        || size.x <= 0.0
                        || size.y <= 0.0
                        || !color.to_rgba().iter().all(|v| v.is_finite())
                        || !thickness.is_finite()
                    {
                        continue;
                    }
                    self.frame_shapes.push(RectInstanceGpu {
                        center,
                        size: [size.x, size.y],
                        color: color_to_unorm8(color.into()),
                        z_index: 900,
                        shape_kind: pack_shape_kind(if filled { 0 } else { 2 }, filled),
                        thickness: thickness.max(0.0),
                    });
                }
                DrawShape2D::Line {
                    end,
                    color,
                    thickness,
                } => {
                    let end =
                        normalized_screen_to_virtual_centered([end.x, end.y], self.virtual_size);
                    append_line_rect(
                        &mut self.frame_shapes,
                        center,
                        end,
                        color.into(),
                        thickness,
                        900,
                    );
                }
                DrawShape2D::Polyline {
                    points,
                    color,
                    thickness,
                    closed,
                } => {
                    append_polyline_rects(
                        &mut self.frame_shapes,
                        points.as_ref(),
                        color.into(),
                        thickness,
                        closed,
                        self.virtual_size,
                        900,
                    );
                }
                DrawShape2D::Path {
                    points,
                    color,
                    thickness,
                } => {
                    append_polyline_rects(
                        &mut self.frame_shapes,
                        points.as_ref(),
                        color.into(),
                        thickness,
                        false,
                        self.virtual_size,
                        900,
                    );
                }
                DrawShape2D::Sprite {
                    texture,
                    size,
                    tint,
                    texture_region,
                } => {
                    if texture.is_nil()
                        || !size.x.is_finite()
                        || !size.y.is_finite()
                        || size.x <= 0.0
                        || size.y <= 0.0
                        || !tint.to_rgba().iter().all(|v| v.is_finite())
                    {
                        continue;
                    }
                    let (uv_min, uv_max, resolved_size) =
                        sprite_region_uv(texture_region, [size.x, size.y]);
                    self.frame_sprites.push(Sprite2DCommand {
                        texture,
                        model: translation_scale_mat3(center, resolved_size),
                        tint,
                        uv_min,
                        uv_max,
                        uv_normalized: false,
                        size: resolved_size,
                        z_index: 900,
                    });
                }
            }
        }
        // Bump only when the immediate set actually changed; byte-identical
        // frames keep the revision so the staging cache holds.
        if self.frame_sprites != self.prev_frame_sprites {
            self.retained_sprites_revision = self.retained_sprites_revision.wrapping_add(1);
        }
        self.flush_point_particles();
    }

    fn flush_point_particles(&mut self) {
        // Split borrow: take the mutable scratch fields out so the shared
        // borrow of retained_point_particles has no overlap. Zero clones.
        let mut frame_shapes = std::mem::take(&mut self.frame_shapes);
        let mut eval_stack = std::mem::take(&mut self.particle_eval_stack);
        for emitter in self.retained_point_particles.values() {
            append_point_particles(&mut frame_shapes, &mut eval_stack, emitter);
        }
        self.frame_shapes = frame_shapes;
        self.particle_eval_stack = eval_stack;
    }

    fn flush_sprite_packets(&mut self, resources: &ResourceStore) -> Renderer2DStats {
        let queued = std::mem::take(&mut self.queued_sprites);
        if let Some((stats, changed)) =
            self.try_apply_sequential_sprite_packets(queued.as_slice(), resources)
        {
            if changed {
                self.retained_sprites_revision = self.retained_sprites_revision.wrapping_add(1);
            }
            return stats;
        }
        let mut stats = Renderer2DStats::default();
        let mut changed = false;
        for SpritePacket { node, sprite } in queued {
            if resources.has_texture(sprite.texture) {
                changed |= self.upsert_retained_sprite(node, sprite);
                stats.accepted_draws = stats.accepted_draws.saturating_add(1);
            } else {
                if let Some(retained) = self.retained_sprite_mut(node) {
                    // Keep previous texture binding until replacement exists,
                    // but still apply latest transform/depth updates.
                    changed |= update_unready_retained_sprite(retained, sprite);
                }
                stats.rejected_draws = stats.rejected_draws.saturating_add(1);
            }
        }
        if changed {
            self.retained_sprites_revision = self.retained_sprites_revision.wrapping_add(1);
        }
        stats
    }

    pub fn prepare_frame(
        &mut self,
        resources: &ResourceStore,
    ) -> (Camera2DUniform, Renderer2DStats, RectUploadPlan) {
        let mut stats = self.flush_sprite_packets(resources);
        let rect_stats = self.apply_queued_rect_updates();
        self.flush_shape_packets();
        stats.accepted_rects = rect_stats.accepted_rects;
        stats.rejected_rects = rect_stats.rejected_rects;
        let plan = self.build_upload_plan();
        (self.camera_uniform(), stats, plan)
    }

    pub fn retained_rects(&self) -> &[RectInstanceGpu] {
        &self.retained_rects
    }

    pub fn frame_shapes(&self) -> &[RectInstanceGpu] {
        &self.frame_shapes
    }

    pub fn retained_sprite(&self, node: NodeID) -> Option<Sprite2DCommand> {
        let idx = *self.node_to_sprite_index.get(&node)?;
        Some(self.retained_sprites.get(idx)?.sprite)
    }

    pub fn retained_sprite_count(&self) -> usize {
        self.retained_sprites.len()
            + self
                .retained_tilemaps
                .values()
                .map(|tilemap| tilemap.sprites.len())
                .sum::<usize>()
            + self.frame_sprites.len()
    }

    pub fn retained_sprites(&self) -> impl Iterator<Item = Sprite2DCommand> + '_ {
        self.retained_sprites
            .iter()
            .map(|packet| packet.sprite)
            .chain(
                self.retained_tilemaps
                    .values()
                    .flat_map(|tilemap| tilemap.sprites.iter().copied()),
            )
            .chain(self.frame_sprites.iter().copied())
    }

    pub fn lights(&self) -> impl Iterator<Item = Light2DState> + '_ {
        self.retained_lights.values().copied()
    }

    pub fn shadow_casters(&self) -> impl Iterator<Item = ShadowCaster2DState> + '_ {
        self.retained_shadow_casters.values().copied().chain(
            self.retained_tilemaps
                .values()
                .flat_map(|tilemap| tilemap.shadow_casters.iter().copied()),
        )
    }

    pub fn retained_waters(&self) -> impl Iterator<Item = (NodeID, Water2DState)> + '_ {
        self.retained_waters
            .iter()
            .map(|(node, water)| (*node, water.clone()))
    }

    #[inline]
    pub fn retained_water_count(&self) -> usize {
        self.retained_waters.len()
    }

    #[inline]
    pub fn retained_waters_revision(&self) -> u64 {
        self.retained_waters_revision
    }

    #[inline]
    pub fn light_count(&self) -> usize {
        self.retained_lights.len()
    }

    #[inline]
    pub fn retained_point_lights_revision(&self) -> u64 {
        self.retained_point_lights_revision
    }

    #[inline]
    pub fn retained_shadow_casters_revision(&self) -> u64 {
        self.retained_shadow_casters_revision
    }

    pub fn camera(&self) -> Camera2DState {
        self.camera.clone()
    }

    #[inline]
    pub fn retained_sprites_revision(&self) -> u64 {
        self.retained_sprites_revision
    }

    fn upsert_retained_rect(&mut self, node: NodeID, rect: RectInstanceGpu) {
        if let Some(&idx) = self.node_to_rect_index.get(&node) {
            if self.retained_rects[idx] != rect {
                self.retained_rects[idx] = rect;
                self.mark_rect_dirty(idx);
            }
            return;
        }

        let idx = self.retained_rects.len();
        self.retained_rects.push(rect);
        self.retained_nodes.push(node);
        self.node_to_rect_index.insert(node, idx);
        self.rect_structure_dirty = true;
    }

    fn upsert_retained_sprite(&mut self, node: NodeID, sprite: Sprite2DCommand) -> bool {
        if let Some(&idx) = self.node_to_sprite_index.get(&node) {
            if self.retained_sprites[idx].sprite != sprite {
                self.retained_sprites[idx].sprite = sprite;
                return true;
            }
            return false;
        }

        let idx = self.retained_sprites.len();
        self.retained_sprites.push(SpritePacket { node, sprite });
        self.node_to_sprite_index.insert(node, idx);
        true
    }

    fn try_apply_sequential_sprite_packets(
        &mut self,
        queued: &[SpritePacket],
        resources: &ResourceStore,
    ) -> Option<(Renderer2DStats, bool)> {
        if queued.len() != self.retained_sprites.len() {
            return None;
        }
        if !queued
            .iter()
            .zip(self.retained_sprites.iter())
            .all(|(queued, retained)| queued.node == retained.node)
        {
            return None;
        }

        let mut stats = Renderer2DStats::default();
        let mut changed = false;
        for (queued, retained) in queued.iter().zip(self.retained_sprites.iter_mut()) {
            if resources.has_texture(queued.sprite.texture) {
                if retained.sprite != queued.sprite {
                    retained.sprite = queued.sprite;
                    changed = true;
                }
                stats.accepted_draws = stats.accepted_draws.saturating_add(1);
            } else {
                changed |= update_unready_retained_sprite(&mut retained.sprite, queued.sprite);
                stats.rejected_draws = stats.rejected_draws.saturating_add(1);
            }
        }
        Some((stats, changed))
    }

    fn retained_sprite_mut(&mut self, node: NodeID) -> Option<&mut Sprite2DCommand> {
        let idx = *self.node_to_sprite_index.get(&node)?;
        Some(&mut self.retained_sprites.get_mut(idx)?.sprite)
    }

    fn remove_retained_sprite(&mut self, node: NodeID) {
        let Some(removed_idx) = self.node_to_sprite_index.remove(&node) else {
            return;
        };

        let last = self.retained_sprites.len() - 1;
        self.retained_sprites.swap_remove(removed_idx);
        if removed_idx != last {
            let moved_node = self.retained_sprites[removed_idx].node;
            self.node_to_sprite_index.insert(moved_node, removed_idx);
        }
        self.retained_sprites_revision = self.retained_sprites_revision.wrapping_add(1);
    }

    fn remove_retained_rect(&mut self, node: NodeID) {
        let Some(removed_idx) = self.node_to_rect_index.remove(&node) else {
            return;
        };

        let last = self.retained_rects.len() - 1;
        self.retained_rects.swap_remove(removed_idx);
        self.retained_nodes.swap_remove(removed_idx);

        if removed_idx != last {
            let moved_node = self.retained_nodes[removed_idx];
            self.node_to_rect_index.insert(moved_node, removed_idx);
        }
        self.rect_structure_dirty = true;
    }

    fn mark_rect_dirty(&mut self, idx: usize) {
        self.rect_dirty_ranges.push(idx..(idx + 1));
    }

    fn build_upload_plan(&mut self) -> RectUploadPlan {
        let draw_count = self.retained_rects.len() + self.frame_shapes.len();
        if !self.frame_shapes.is_empty() {
            self.rect_structure_dirty = false;
            self.rect_dirty_ranges.clear();
            return RectUploadPlan {
                full_reupload: true,
                dirty_ranges: Vec::new(),
                draw_count,
            };
        }
        if self.rect_structure_dirty {
            self.rect_structure_dirty = false;
            self.rect_dirty_ranges.clear();
            return RectUploadPlan {
                full_reupload: true,
                dirty_ranges: Vec::new(),
                draw_count,
            };
        }

        if self.rect_dirty_ranges.is_empty() {
            return RectUploadPlan {
                full_reupload: false,
                dirty_ranges: Vec::new(),
                draw_count,
            };
        }

        let dirty_ranges = coalesce_ranges(std::mem::take(&mut self.rect_dirty_ranges));
        RectUploadPlan {
            full_reupload: false,
            dirty_ranges,
            draw_count,
        }
    }
}

pub fn camera_2d_uniform_from_state(
    camera: &Camera2DState,
    width: u32,
    height: u32,
) -> Camera2DUniform {
    let view = compute_view_matrix(camera);
    let ndc_scale = ndc_scale(
        (width.max(1), height.max(1)),
        [width.max(1) as f32, height.max(1) as f32],
        camera.zoom,
    );
    Camera2DUniform {
        view,
        ndc_scale,
        pad: [0.0, 0.0],
    }
}

#[inline]
fn ndc_scale(viewport: (u32, u32), virtual_size: [f32; 2], zoom: f32) -> [f32; 2] {
    let width = viewport.0.max(1) as f32;
    let height = viewport.1.max(1) as f32;
    let vw = virtual_size[0].max(1.0);
    let vh = virtual_size[1].max(1.0);
    let zoom = if zoom.is_finite() && zoom > 0.0 {
        zoom
    } else {
        1.0
    };

    // Aspect-fit virtual viewport into actual window.
    let sx = width / vw;
    let sy = height / vh;
    let scale = sx.min(sy);
    let world_to_window = scale * zoom;

    [
        (2.0 * world_to_window) / width,
        (2.0 * world_to_window) / height,
    ]
}

#[inline]
fn compute_view_matrix(camera: &Camera2DState) -> [[f32; 4]; 4] {
    let angle = -camera.rotation_radians;
    let c = angle.cos();
    let s = angle.sin();
    let tx = -camera.position[0];
    let ty = -camera.position[1];

    [
        [c, s, 0.0, 0.0],
        [-s, c, 0.0, 0.0],
        [0.0, 0.0, 1.0, 0.0],
        [tx * c - ty * s, tx * s + ty * c, 0.0, 1.0],
    ]
}

fn coalesce_ranges(mut ranges: Vec<Range<usize>>) -> Vec<Range<usize>> {
    if ranges.len() <= 1 {
        return ranges;
    }
    ranges.sort_by_key(|r| r.start);
    let mut merged = Vec::with_capacity(ranges.len());
    let mut current = ranges.remove(0);
    for range in ranges {
        if range.start <= current.end {
            current.end = current.end.max(range.end);
        } else {
            merged.push(current);
            current = range;
        }
    }
    merged.push(current);
    merged
}

#[inline]
fn normalized_screen_to_virtual_centered(pos: [f32; 2], virtual_size: [f32; 2]) -> [f32; 2] {
    let vx = virtual_size[0].max(1.0);
    let vy = virtual_size[1].max(1.0);
    // Position is normalized screen-space: (0.5, 0.5) is the center.
    // X grows right, Y grows upward.
    [(pos[0] - 0.5) * vx, (pos[1] - 0.5) * vy]
}

fn append_polyline_rects(
    out: &mut Vec<RectInstanceGpu>,
    points: &[perro_structs::Vector2],
    color: [f32; 4],
    thickness: f32,
    closed: bool,
    virtual_size: [f32; 2],
    z_index: i32,
) {
    if points.len() < 2 {
        return;
    }
    for pair in points.windows(2) {
        let a = normalized_screen_to_virtual_centered([pair[0].x, pair[0].y], virtual_size);
        let b = normalized_screen_to_virtual_centered([pair[1].x, pair[1].y], virtual_size);
        append_line_rect(out, a, b, color, thickness, z_index);
    }
    if closed {
        let a = points[points.len() - 1];
        let b = points[0];
        append_line_rect(
            out,
            normalized_screen_to_virtual_centered([a.x, a.y], virtual_size),
            normalized_screen_to_virtual_centered([b.x, b.y], virtual_size),
            color,
            thickness,
            z_index,
        );
    }
}

fn append_line_rect(
    out: &mut Vec<RectInstanceGpu>,
    start: [f32; 2],
    end: [f32; 2],
    color: [f32; 4],
    thickness: f32,
    z_index: i32,
) {
    let dx = end[0] - start[0];
    let dy = end[1] - start[1];
    let len = (dx * dx + dy * dy).sqrt();
    if !len.is_finite()
        || len <= 0.0
        || !thickness.is_finite()
        || thickness <= 0.0
        || !color.iter().all(|v| v.is_finite())
    {
        return;
    }
    out.push(RectInstanceGpu {
        center: [(start[0] + end[0]) * 0.5, (start[1] + end[1]) * 0.5],
        size: [dx, dy],
        color: color_to_unorm8(color),
        z_index,
        shape_kind: pack_shape_kind(3, true),
        thickness,
    });
}

fn sprite_region_uv(
    region: Option<[f32; 4]>,
    fallback_size: [f32; 2],
) -> ([f32; 2], [f32; 2], [f32; 2]) {
    let Some([x, y, w, h]) = region else {
        return ([0.0, 0.0], [1.0, 1.0], fallback_size);
    };
    if !(x.is_finite() && y.is_finite() && w.is_finite() && h.is_finite()) || w <= 0.0 || h <= 0.0 {
        return ([0.0, 0.0], [1.0, 1.0], fallback_size);
    }
    ([x, y], [x + w, y + h], [fallback_size[0], fallback_size[1]])
}

fn translation_scale_mat3(center: [f32; 2], size: [f32; 2]) -> [[f32; 3]; 3] {
    [
        [size[0], 0.0, 0.0],
        [0.0, size[1], 0.0],
        [center[0], center[1], 1.0],
    ]
}

pub(crate) fn append_point_particles(
    out: &mut Vec<RectInstanceGpu>,
    stack: &mut Vec<f32>,
    emitter: &PointParticles2DState,
) {
    if !emitter.active || emitter.alive_budget == 0 || emitter.emission_rate <= 0.0 {
        return;
    }
    let lifetime_min = emitter.lifetime_min.max(0.001);
    let lifetime_max = emitter.lifetime_max.max(lifetime_min);
    let period = (emitter.alive_budget as f32 / emitter.emission_rate.max(0.001)).max(lifetime_max);
    let sim_time = if emitter.prewarm {
        emitter.simulation_time + lifetime_max
    } else {
        emitter.simulation_time
    };

    for i in 0..emitter.alive_budget {
        let base_spawn = i as f32 / emitter.emission_rate.max(0.001);
        let spawn_time = if emitter.looping && sim_time >= base_spawn {
            let cycles = ((sim_time - base_spawn) / period).floor();
            base_spawn + cycles * period
        } else {
            base_spawn
        };
        let life = sim_time - spawn_time;
        if life < 0.0 || life > lifetime_max {
            continue;
        }

        let h0 = hash01(emitter.seed ^ i);
        let h1 = hash01(emitter.seed.wrapping_add(0x9E37_79B9) ^ i.wrapping_mul(3));
        let h2 = hash01(emitter.seed.wrapping_add(0x7F4A_7C15) ^ i.wrapping_mul(7));
        let h3 = hash01(emitter.seed.wrapping_add(0x94D0_49BB) ^ i.wrapping_mul(11));
        let lifetime = lifetime_min + (lifetime_max - lifetime_min) * h0;
        if life > lifetime {
            continue;
        }
        let t = (life / lifetime).clamp(0.0, 1.0);
        let speed = emitter.speed_min + (emitter.speed_max - emitter.speed_min) * h1;
        let angle = (h2 - 0.5) * emitter.spread_radians;
        let dir = [angle.sin(), angle.cos(), 0.0];
        let vel = [dir[0] * speed, dir[1] * speed, 0.0];
        let mut local = eval_particle_pos_2d(
            emitter,
            stack,
            i,
            t,
            life,
            lifetime,
            spawn_time,
            speed,
            dir,
            vel,
            [h0, h1, h2],
            h3,
        );
        local[0] += 0.5 * emitter.force[0] * life * life;
        local[1] += 0.5 * emitter.force[1] * life * life;

        let world = transform_point_2d(emitter.model, local);
        let size = (emitter.size * (emitter.size_min + (emitter.size_max - emitter.size_min) * h3))
            .max(1.0);
        let color = lerp_color(emitter.color_start.into(), emitter.color_end.into(), t);
        if !world[0].is_finite() || !world[1].is_finite() || !size.is_finite() {
            continue;
        }
        out.push(RectInstanceGpu {
            center: world,
            size: [size, size],
            color: color_to_unorm8(color),
            z_index: emitter.z_index,
            shape_kind: pack_shape_kind(1, true),
            thickness: 1.0,
        });
    }
}

#[allow(clippy::too_many_arguments)]
fn eval_particle_pos_2d(
    emitter: &PointParticles2DState,
    stack: &mut Vec<f32>,
    particle_key: u32,
    t: f32,
    life: f32,
    lifetime: f32,
    spawn_time: f32,
    speed: f32,
    dir: [f32; 3],
    vel: [f32; 3],
    rand: [f32; 3],
    ring_u: f32,
) -> [f32; 2] {
    if let (Some(x_ops), Some(y_ops)) = (
        emitter.profile.expr_x_ops.as_ref(),
        emitter.profile.expr_y_ops.as_ref(),
    ) {
        let input = ParticleEvalInput {
            t,
            life,
            lifetime,
            spawn_time,
            emitter_time: emitter.simulation_time,
            speed,
            particle_id: particle_key as f32,
            dir,
            vel,
            rand,
            seed: particle_key as f32,
            ring_u,
            index01: particle_key as f32 / emitter.alive_budget.max(1) as f32,
            emitter_pos: [emitter.model[2][0], emitter.model[2][1], 0.0],
            prev_pos: [0.0, 0.0, 0.0],
            params: &emitter.params,
        };
        return [
            eval_ops_particle(x_ops, &input, stack).unwrap_or(0.0),
            eval_ops_particle(y_ops, &input, stack).unwrap_or(0.0),
        ];
    }

    match emitter.profile.path {
        ParticlePath2D::None => [0.0, 0.0],
        ParticlePath2D::Ballistic => [vel[0] * life, vel[1] * life],
        ParticlePath2D::Spiral {
            angular_velocity,
            radius,
        } => {
            let a = angular_velocity * life + ring_u * std::f32::consts::TAU;
            [a.cos() * radius * t, a.sin() * radius * t]
        }
        ParticlePath2D::NoiseDrift {
            amplitude,
            frequency,
        } => [
            (life * frequency + rand[0] * 10.0).sin() * amplitude,
            vel[1] * life + (life * frequency + rand[1] * 10.0).cos() * amplitude,
        ],
        ParticlePath2D::FlatDisk { radius } => {
            let a = ring_u * std::f32::consts::TAU;
            [a.cos() * radius * rand[0], a.sin() * radius * rand[0]]
        }
        ParticlePath2D::Custom { .. } | ParticlePath2D::CustomCompiled { .. } => [0.0, 0.0],
    }
}

fn transform_point_2d(model: [[f32; 3]; 3], p: [f32; 2]) -> [f32; 2] {
    [
        model[0][0] * p[0] + model[1][0] * p[1] + model[2][0],
        model[0][1] * p[0] + model[1][1] * p[1] + model[2][1],
    ]
}

fn lerp_color(a: [f32; 4], b: [f32; 4], t: f32) -> [f32; 4] {
    [
        a[0] + (b[0] - a[0]) * t,
        a[1] + (b[1] - a[1]) * t,
        a[2] + (b[2] - a[2]) * t,
        a[3] + (b[3] - a[3]) * t,
    ]
}

fn hash01(seed: u32) -> f32 {
    let mut x = seed.wrapping_add(0x9E37_79B9);
    x ^= x >> 16;
    x = x.wrapping_mul(0x7FEB_352D);
    x ^= x >> 15;
    x = x.wrapping_mul(0x846C_A68B);
    x ^= x >> 16;
    (x as f32) * (1.0 / u32::MAX as f32)
}

#[inline]
fn color_to_unorm8(color: [f32; 4]) -> [u8; 4] {
    UnitVector4::new(color).to_u8()
}

fn retained_rect_instance(rect: Rect2DCommand) -> Option<RectInstanceGpu> {
    if !(rect.size[0].is_finite()
        && rect.size[1].is_finite()
        && rect.center[0].is_finite()
        && rect.center[1].is_finite()
        && rect.color.to_rgba().iter().all(|v| v.is_finite())
        && rect.size[0] > 0.0
        && rect.size[1] > 0.0)
    {
        return None;
    }
    Some(RectInstanceGpu {
        center: rect.center,
        size: rect.size,
        color: color_to_unorm8(rect.color.into()),
        z_index: rect.z_index,
        shape_kind: pack_shape_kind(0, true),
        thickness: 1.0,
    })
}

fn update_unready_retained_sprite(retained: &mut Sprite2DCommand, sprite: Sprite2DCommand) -> bool {
    let old_model = retained.model;
    let old_z = retained.z_index;
    retained.model = sprite.model;
    retained.z_index = sprite.z_index;
    retained.model != old_model || retained.z_index != old_z
}

#[cfg(test)]
#[path = "../../tests/unit/two_d_renderer_tests.rs"]
mod tests;
