use crate::resources::ResourceStore;
use bytemuck::{Pod, Zeroable};
use perro_ids::TextureID;
use perro_render_bridge::{Camera2DState, Rect2DCommand};

#[derive(Debug, Clone, Copy)]
struct DrawPacket {
    texture: TextureID,
}

#[derive(Debug, Clone, Copy)]
struct RectPacket {
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
#[derive(Debug, Clone, Copy, Zeroable, Pod)]
pub struct RectInstanceGpu {
    pub center: [f32; 2],
    pub size: [f32; 2],
    pub color: [f32; 4],
    pub z_index: i32,
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
}

impl Renderer2D {
    pub fn new() -> Self {
        Self {
            queued_draws: Vec::new(),
            queued_rects: Vec::new(),
            camera: Camera2DState::default(),
            viewport: (0, 0),
            virtual_size: [DEFAULT_VIRTUAL_WIDTH, DEFAULT_VIRTUAL_HEIGHT],
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

    pub fn queue_texture(&mut self, texture: TextureID) {
        self.queued_draws.push(DrawPacket { texture });
    }

    pub fn queue_rect(&mut self, rect: Rect2DCommand) {
        self.queued_rects.push(RectPacket { rect });
    }

    pub fn drain_rect_instances(&mut self, out: &mut Vec<RectInstanceGpu>) -> Renderer2DStats {
        out.clear();
        let mut stats = Renderer2DStats::default();
        for RectPacket { rect } in self.queued_rects.drain(..) {
            if rect.size[0].is_finite()
                && rect.size[1].is_finite()
                && rect.center[0].is_finite()
                && rect.center[1].is_finite()
                && rect.color.iter().all(|v| v.is_finite())
                && rect.size[0] > 0.0
                && rect.size[1] > 0.0
            {
                out.push(RectInstanceGpu {
                    center: rect.center,
                    size: rect.size,
                    color: rect.color,
                    z_index: rect.z_index,
                });
                stats.accepted_rects = stats.accepted_rects.saturating_add(1);
            } else {
                stats.rejected_rects = stats.rejected_rects.saturating_add(1);
            }
        }
        stats
    }

    fn flush_texture_packets(&mut self, resources: &ResourceStore) -> Renderer2DStats {
        let mut stats = Renderer2DStats::default();
        for DrawPacket { texture } in self.queued_draws.drain(..) {
            if resources.has_texture(texture) {
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
        rect_out: &mut Vec<RectInstanceGpu>,
    ) -> (Camera2DUniform, Renderer2DStats) {
        let mut stats = self.flush_texture_packets(resources);
        let rect_stats = self.drain_rect_instances(rect_out);
        stats.accepted_rects = rect_stats.accepted_rects;
        stats.rejected_rects = rect_stats.rejected_rects;
        (self.camera_uniform(), stats)
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
