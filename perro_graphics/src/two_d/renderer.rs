use crate::resources::ResourceStore;
use bytemuck::{Pod, Zeroable};
use perro_ids::{NodeID, TextureID};
use perro_render_bridge::{Camera2DState, Rect2DCommand};
use std::{collections::HashMap, ops::Range};

#[derive(Debug, Clone, Copy)]
struct DrawPacket {
    node: NodeID,
    texture: TextureID,
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
#[derive(Debug, Clone, Copy, Zeroable, Pod)]
#[allow(dead_code)]
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
    pub color: [f32; 4],
    pub z_index: i32,
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
    queued_draws: Vec<DrawPacket>,
    queued_rects: Vec<RectPacket>,
    camera: Camera2DState,
    viewport: (u32, u32),
    virtual_size: [f32; 2],
    retained_rects: Vec<RectInstanceGpu>,
    retained_nodes: Vec<NodeID>,
    node_to_rect_index: HashMap<NodeID, usize>,
    rect_dirty_ranges: Vec<Range<usize>>,
    rect_structure_dirty: bool,
    retained_textures: HashMap<NodeID, TextureID>,
}

impl Renderer2D {
    pub fn new() -> Self {
        Self {
            queued_draws: Vec::new(),
            queued_rects: Vec::new(),
            camera: Camera2DState::default(),
            viewport: (0, 0),
            virtual_size: [DEFAULT_VIRTUAL_WIDTH, DEFAULT_VIRTUAL_HEIGHT],
            retained_rects: Vec::new(),
            retained_nodes: Vec::new(),
            node_to_rect_index: HashMap::new(),
            rect_dirty_ranges: Vec::new(),
            rect_structure_dirty: false,
            retained_textures: HashMap::new(),
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
        let view = compute_view_matrix(self.camera);
        let ndc_scale = ndc_scale(self.viewport, self.virtual_size, self.camera.zoom);
        Camera2DUniform {
            view,
            ndc_scale,
            pad: [0.0, 0.0],
        }
    }

    pub fn queue_texture(&mut self, node: NodeID, texture: TextureID) {
        self.queued_draws.push(DrawPacket { node, texture });
    }

    pub fn queue_rect(&mut self, node: NodeID, rect: Rect2DCommand) {
        self.queued_rects.push(RectPacket { node, rect });
    }

    pub fn upsert_rect(&mut self, node: NodeID, rect: Rect2DCommand) {
        self.queued_rects.push(RectPacket { node, rect });
    }

    pub fn remove_node(&mut self, node: NodeID) {
        self.remove_retained_rect(node);
        self.retained_textures.remove(&node);
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
                    color: rect.color,
                    z_index: rect.z_index,
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

    fn flush_texture_packets(&mut self, resources: &ResourceStore) -> Renderer2DStats {
        let mut stats = Renderer2DStats::default();
        for DrawPacket { node, texture } in self.queued_draws.drain(..) {
            if resources.has_texture(texture) {
                self.retained_textures.insert(node, texture);
                stats.accepted_draws = stats.accepted_draws.saturating_add(1);
            } else {
                stats.rejected_draws = stats.rejected_draws.saturating_add(1);
            }
        }
        stats
    }

    pub fn prepare_frame(
        &mut self,
        resources: &ResourceStore,
    ) -> (Camera2DUniform, Renderer2DStats, RectUploadPlan) {
        let mut stats = self.flush_texture_packets(resources);
        let rect_stats = self.apply_queued_rect_updates();
        stats.accepted_rects = rect_stats.accepted_rects;
        stats.rejected_rects = rect_stats.rejected_rects;
        let plan = self.build_upload_plan();
        (self.camera_uniform(), stats, plan)
    }

    pub fn retained_rects(&self) -> &[RectInstanceGpu] {
        &self.retained_rects
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

    [(2.0 * world_to_window) / width, (2.0 * world_to_window) / height]
}

#[inline]
fn compute_view_matrix(camera: Camera2DState) -> [[f32; 4]; 4] {
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
