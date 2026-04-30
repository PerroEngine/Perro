use ahash::AHashMap;
use epaint::{
    AlphaFromCoverage, ClippedPrimitive, ClippedShape, Color32, CornerRadius, FontFamily, FontId,
    Fonts, Primitive, Rect, RectShape, Shape, Stroke, StrokeKind, TessellationOptions, Tessellator,
    pos2, text::FontDefinitions, textures::TexturesDelta, vec2,
};
use perro_ids::NodeID;
use perro_render_bridge::{UiCommand, UiRectState, UiTextAlignState};
use std::borrow::Cow;

#[derive(Clone, Debug, PartialEq)]
struct UiPanelDraw {
    rect: UiRectState,
    fill: [f32; 4],
    stroke: [f32; 4],
    stroke_width: f32,
    corner_radius: f32,
}

#[derive(Clone, Debug, PartialEq)]
struct UiLabelDraw {
    rect: UiRectState,
    text: Cow<'static, str>,
    color: [f32; 4],
    font_size: f32,
    h_align: UiTextAlignState,
    v_align: UiTextAlignState,
}

#[derive(Clone, Debug, PartialEq)]
struct UiButtonDraw {
    panel: UiPanelDraw,
    text: Cow<'static, str>,
    text_color: [f32; 4],
    disabled: bool,
}

#[derive(Clone, Debug, PartialEq)]
enum UiDraw {
    Panel(UiPanelDraw),
    Button(UiButtonDraw),
    Label(UiLabelDraw),
}

pub struct UiPaintFrame<'a> {
    pub primitives: &'a [ClippedPrimitive],
    pub textures_delta: &'a TexturesDelta,
    pub revision: u64,
}

pub struct UiRenderer {
    nodes: AHashMap<NodeID, UiDraw>,
    revision: u64,
    fonts: Fonts,
    shapes: Vec<ClippedShape>,
    primitives: Vec<ClippedPrimitive>,
    textures_delta: TexturesDelta,
    last_viewport: [f32; 2],
    paint_revision: u64,
}

impl Default for UiRenderer {
    fn default() -> Self {
        Self::new()
    }
}

impl UiRenderer {
    pub fn new() -> Self {
        Self {
            nodes: AHashMap::new(),
            revision: 0,
            fonts: Fonts::new(
                2048,
                AlphaFromCoverage::default(),
                FontDefinitions::default(),
            ),
            shapes: Vec::new(),
            primitives: Vec::new(),
            textures_delta: TexturesDelta::default(),
            last_viewport: [0.0, 0.0],
            paint_revision: u64::MAX,
        }
    }

    pub fn submit(&mut self, command: UiCommand) {
        match command {
            UiCommand::UpsertPanel {
                node,
                rect,
                fill,
                stroke,
                stroke_width,
                corner_radius,
            } => self.upsert(
                node,
                UiDraw::Panel(UiPanelDraw {
                    rect,
                    fill,
                    stroke,
                    stroke_width,
                    corner_radius,
                }),
            ),
            UiCommand::UpsertButton {
                node,
                rect,
                text,
                text_color,
                fill,
                stroke,
                stroke_width,
                corner_radius,
                disabled,
            } => self.upsert(
                node,
                UiDraw::Button(UiButtonDraw {
                    panel: UiPanelDraw {
                        rect,
                        fill,
                        stroke,
                        stroke_width,
                        corner_radius,
                    },
                    text,
                    text_color,
                    disabled,
                }),
            ),
            UiCommand::UpsertLabel {
                node,
                rect,
                text,
                color,
                font_size,
                h_align,
                v_align,
            } => self.upsert(
                node,
                UiDraw::Label(UiLabelDraw {
                    rect,
                    text,
                    color,
                    font_size,
                    h_align,
                    v_align,
                }),
            ),
            UiCommand::RemoveNode { node } => {
                if self.nodes.remove(&node).is_some() {
                    self.bump_revision();
                }
            }
            UiCommand::Clear => {
                if !self.nodes.is_empty() {
                    self.nodes.clear();
                    self.bump_revision();
                }
            }
        }
    }

    pub fn retained_count(&self) -> usize {
        self.nodes.len()
    }

    pub fn prepare_paint(&mut self, viewport: [f32; 2]) -> UiPaintFrame<'_> {
        let viewport = [viewport[0].max(1.0), viewport[1].max(1.0)];
        if self.paint_revision != self.revision || self.last_viewport != viewport {
            self.rebuild_primitives(viewport);
        }
        UiPaintFrame {
            primitives: &self.primitives,
            textures_delta: &self.textures_delta,
            revision: self.paint_revision,
        }
    }

    fn upsert(&mut self, node: NodeID, draw: UiDraw) {
        if self.nodes.get(&node) == Some(&draw) {
            return;
        }
        self.nodes.insert(node, draw);
        self.bump_revision();
    }

    fn bump_revision(&mut self) {
        self.revision = self.revision.wrapping_add(1);
    }

    fn rebuild_primitives(&mut self, viewport: [f32; 2]) {
        self.fonts.begin_pass(2048, AlphaFromCoverage::default());
        self.shapes.clear();
        self.primitives.clear();
        if self.shapes.capacity() < self.nodes.len() {
            self.shapes
                .reserve(self.nodes.len() - self.shapes.capacity());
        }

        let mut ordered: Vec<(NodeID, &UiDraw)> = self
            .nodes
            .iter()
            .map(|(node, draw)| (*node, draw))
            .collect();
        ordered.sort_unstable_by(|(a_node, a_draw), (b_node, b_draw)| {
            ui_rect(a_draw)
                .z_index
                .cmp(&ui_rect(b_draw).z_index)
                .then_with(|| a_node.as_u64().cmp(&b_node.as_u64()))
        });

        for (_, draw) in ordered {
            match draw {
                UiDraw::Panel(panel) => push_panel_shape(panel, viewport, &mut self.shapes),
                UiDraw::Button(button) => {
                    push_panel_shape(&button.panel, viewport, &mut self.shapes);
                    push_text_shape(
                        button.panel.rect,
                        viewport,
                        button.text.as_ref(),
                        16.0,
                        button.text_color,
                        UiTextAlignState::Center,
                        UiTextAlignState::Center,
                        &mut self.fonts,
                        &mut self.shapes,
                    );
                }
                UiDraw::Label(label) => push_text_shape(
                    label.rect,
                    viewport,
                    label.text.as_ref(),
                    label.font_size,
                    label.color,
                    label.h_align,
                    label.v_align,
                    &mut self.fonts,
                    &mut self.shapes,
                ),
            }
        }

        let mut tessellator = Tessellator::new(
            1.0,
            TessellationOptions::default(),
            self.fonts.font_image_size(),
            self.fonts.texture_atlas().prepared_discs(),
        );
        self.primitives = tessellator.tessellate_shapes(std::mem::take(&mut self.shapes));
        self.primitives
            .retain(|primitive| match &primitive.primitive {
                Primitive::Mesh(mesh) => !mesh.vertices.is_empty() && !mesh.indices.is_empty(),
                Primitive::Callback(_) => false,
            });

        self.textures_delta.clear();
        if let Some(delta) = self.fonts.font_image_delta() {
            self.textures_delta
                .set
                .push((epaint::TextureId::default(), delta));
        }
        self.paint_revision = self.revision;
        self.last_viewport = viewport;
    }
}

fn ui_rect(draw: &UiDraw) -> UiRectState {
    match draw {
        UiDraw::Panel(panel) => panel.rect,
        UiDraw::Button(button) => button.panel.rect,
        UiDraw::Label(label) => label.rect,
    }
}

fn push_panel_shape(panel: &UiPanelDraw, viewport: [f32; 2], out: &mut Vec<ClippedShape>) {
    if !valid_rect(panel.rect) || !valid_color(panel.fill) || !valid_color(panel.stroke) {
        return;
    }

    let (min, max) = panel.rect.screen_min_max(viewport);
    let rect = Rect::from_min_max(pos2(min[0], min[1]), pos2(max[0], max[1]));
    out.push(ClippedShape {
        clip_rect: viewport_rect(viewport),
        shape: Shape::Rect(RectShape::new(
            rect,
            CornerRadius::same(resolve_corner_radius(panel) as u8),
            color32(panel.fill),
            Stroke::new(panel.stroke_width.max(0.0), color32(panel.stroke)),
            StrokeKind::Inside,
        )),
    });
}

fn resolve_corner_radius(panel: &UiPanelDraw) -> f32 {
    if panel.corner_radius.is_infinite() {
        panel.rect.size[0].min(panel.rect.size[1]) * 0.5
    } else {
        panel.corner_radius.max(0.0)
    }
    .min(u8::MAX as f32)
}

fn push_text_shape(
    rect: UiRectState,
    viewport: [f32; 2],
    text: &str,
    font_size: f32,
    color: [f32; 4],
    h_align: UiTextAlignState,
    v_align: UiTextAlignState,
    fonts: &mut Fonts,
    out: &mut Vec<ClippedShape>,
) {
    if text.is_empty()
        || !valid_rect(rect)
        || !valid_color(color)
        || !font_size.is_finite()
        || font_size <= 0.0
    {
        return;
    }

    let (min, max) = rect.screen_min_max(viewport);
    let clip_rect = Rect::from_min_max(pos2(min[0], min[1]), pos2(max[0], max[1]));
    let galley = fonts.with_pixels_per_point(1.0).layout(
        text.to_string(),
        FontId::new(font_size, FontFamily::Proportional),
        color32(color),
        rect.size[0].max(1.0),
    );
    let text_size = galley.size();
    let x = match h_align {
        UiTextAlignState::Start => min[0],
        UiTextAlignState::Center => min[0] + (rect.size[0] - text_size.x).max(0.0) * 0.5,
        UiTextAlignState::End => max[0] - text_size.x,
    };
    let y = match v_align {
        UiTextAlignState::Start => min[1],
        UiTextAlignState::Center => min[1] + (rect.size[1] - text_size.y).max(0.0) * 0.5,
        UiTextAlignState::End => max[1] - text_size.y,
    };
    out.push(ClippedShape {
        clip_rect,
        shape: Shape::galley_with_override_text_color(pos2(x, y), galley, color32(color)),
    });
}

fn color32(color: [f32; 4]) -> Color32 {
    let r = (color[0].clamp(0.0, 1.0) * 255.0).round() as u8;
    let g = (color[1].clamp(0.0, 1.0) * 255.0).round() as u8;
    let b = (color[2].clamp(0.0, 1.0) * 255.0).round() as u8;
    let a = (color[3].clamp(0.0, 1.0) * 255.0).round() as u8;
    Color32::from_rgba_unmultiplied(r, g, b, a)
}

fn valid_rect(rect: UiRectState) -> bool {
    rect.center.iter().all(|v| v.is_finite())
        && rect.size.iter().all(|v| v.is_finite())
        && rect.size[0] > 0.0
        && rect.size[1] > 0.0
}

fn valid_color(color: [f32; 4]) -> bool {
    color.iter().all(|v| v.is_finite())
}

fn viewport_rect(viewport: [f32; 2]) -> Rect {
    Rect::from_min_size(pos2(0.0, 0.0), vec2(viewport[0], viewport[1]))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn panel_builds_epaint_mesh() {
        let mut renderer = UiRenderer::new();
        renderer.submit(UiCommand::UpsertPanel {
            node: NodeID::from_parts(1, 0),
            rect: UiRectState {
                center: [10.0, 20.0],
                size: [100.0, 50.0],
                z_index: 2,
            },
            fill: [0.1, 0.2, 0.3, 1.0],
            stroke: [1.0, 1.0, 1.0, 1.0],
            stroke_width: 2.0,
            corner_radius: 4.0,
        });

        let paint = renderer.prepare_paint([800.0, 600.0]);

        assert!(!paint.primitives.is_empty());
    }

    #[test]
    fn label_builds_text_mesh_and_font_delta() {
        let mut renderer = UiRenderer::new();
        renderer.submit(UiCommand::UpsertLabel {
            node: NodeID::from_parts(2, 0),
            rect: UiRectState {
                center: [0.0, 0.0],
                size: [200.0, 60.0],
                z_index: 0,
            },
            text: Cow::Borrowed("Run"),
            color: [1.0, 1.0, 1.0, 1.0],
            font_size: 18.0,
            h_align: UiTextAlignState::Center,
            v_align: UiTextAlignState::Center,
        });

        let paint = renderer.prepare_paint([800.0, 600.0]);

        assert!(!paint.primitives.is_empty());
        assert!(!paint.textures_delta.set.is_empty());
    }
}
