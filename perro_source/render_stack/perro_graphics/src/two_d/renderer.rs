use crate::resources::ResourceStore;
use ahash::AHashMap;
use bytemuck::{Pod, Zeroable};
use perro_ids::NodeID;
use perro_render_bridge::{Camera2DState, DrawShape2DCommand, Rect2DCommand, Sprite2DCommand};
use perro_structs::DrawShape2D;
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
    pub shape_kind: u32,
    pub thickness: f32,
    pub filled: u32,
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
    retained_sprites: AHashMap<NodeID, Sprite2DCommand>,
    retained_sprites_revision: u64,
    frame_shapes: Vec<RectInstanceGpu>,
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
            retained_sprites: AHashMap::new(),
            retained_sprites_revision: 0,
            frame_shapes: Vec::new(),
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

    pub fn queue_rect(&mut self, node: NodeID, rect: Rect2DCommand) {
        self.queued_rects.push(RectPacket { node, rect });
    }

    pub fn upsert_rect(&mut self, node: NodeID, rect: Rect2DCommand) {
        self.queued_rects.push(RectPacket { node, rect });
    }

    pub fn queue_shape(&mut self, draw: DrawShape2DCommand) {
        self.queued_shapes.push(draw);
    }

    pub fn remove_node(&mut self, node: NodeID) {
        self.remove_retained_rect(node);
        if self.retained_sprites.remove(&node).is_some() {
            self.retained_sprites_revision = self.retained_sprites_revision.wrapping_add(1);
        }
    }

    fn apply_queued_rect_updates(&mut self) -> Renderer2DStats {
        let queued = std::mem::take(&mut self.queued_rects);
        let mut stats = Renderer2DStats::default();
        for RectPacket { node, rect } in queued {
            if rect.size[0].is_finite()
                && rect.size[1].is_finite()
                && rect.center[0].is_finite()
                && rect.center[1].is_finite()
                && rect.color.iter().all(|v| v.is_finite())
                && rect.size[0] > 0.0
                && rect.size[1] > 0.0
            {
                self.upsert_retained_rect(
                    node,
                    RectInstanceGpu {
                        center: rect.center,
                        size: rect.size,
                        color: color_to_unorm8(rect.color),
                        z_index: rect.z_index,
                        shape_kind: 0,
                        thickness: 1.0,
                        filled: 1,
                    },
                );
                stats.accepted_rects = stats.accepted_rects.saturating_add(1);
            } else {
                self.remove_retained_rect(node);
                stats.rejected_rects = stats.rejected_rects.saturating_add(1);
            }
        }
        stats
    }

    fn flush_shape_packets(&mut self) {
        self.frame_shapes.clear();
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
                        || !color.iter().all(|v| v.is_finite())
                        || !thickness.is_finite()
                    {
                        continue;
                    }
                    self.frame_shapes.push(RectInstanceGpu {
                        center,
                        size: [radius * 2.0, radius * 2.0],
                        color: color_to_unorm8(color),
                        z_index: 900,
                        shape_kind: 1,
                        thickness: thickness.max(0.0),
                        filled: u32::from(filled),
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
                        || !color.iter().all(|v| v.is_finite())
                        || !thickness.is_finite()
                    {
                        continue;
                    }
                    self.frame_shapes.push(RectInstanceGpu {
                        center,
                        size: [size.x, size.y],
                        color: color_to_unorm8(color),
                        z_index: 900,
                        shape_kind: if filled { 0 } else { 2 },
                        thickness: thickness.max(0.0),
                        filled: u32::from(filled),
                    });
                }
            }
        }
    }

    fn flush_sprite_packets(&mut self, resources: &ResourceStore) -> Renderer2DStats {
        let mut stats = Renderer2DStats::default();
        let mut changed = false;
        for SpritePacket { node, sprite } in self.queued_sprites.drain(..) {
            if resources.has_texture(sprite.texture) {
                if self.retained_sprites.insert(node, sprite) != Some(sprite) {
                    changed = true;
                }
                stats.accepted_draws = stats.accepted_draws.saturating_add(1);
            } else {
                if let Some(retained) = self.retained_sprites.get_mut(&node) {
                    // Keep previous texture binding until replacement exists,
                    // but still apply latest transform/depth updates.
                    let old_model = retained.model;
                    let old_z = retained.z_index;
                    retained.model = sprite.model;
                    retained.z_index = sprite.z_index;
                    if retained.model != old_model || retained.z_index != old_z {
                        changed = true;
                    }
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
        self.retained_sprites.get(&node).copied()
    }

    pub fn retained_sprite_count(&self) -> usize {
        self.retained_sprites.len()
    }

    pub fn retained_sprites(&self) -> impl Iterator<Item = Sprite2DCommand> + '_ {
        self.retained_sprites.values().copied()
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
        let draw_count = self.retained_rects.len();
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

#[inline]
fn color_to_unorm8(color: [f32; 4]) -> [u8; 4] {
    [
        (color[0].clamp(0.0, 1.0) * 255.0).round() as u8,
        (color[1].clamp(0.0, 1.0) * 255.0).round() as u8,
        (color[2].clamp(0.0, 1.0) * 255.0).round() as u8,
        (color[3].clamp(0.0, 1.0) * 255.0).round() as u8,
    ]
}

#[cfg(test)]
#[path = "../../tests/unit/two_d_renderer_tests.rs"]
mod tests;
