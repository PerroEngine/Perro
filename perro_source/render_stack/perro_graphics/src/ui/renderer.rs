use crate::two_d::renderer::RectInstanceGpu;
use ahash::AHashMap;
use epaint::{Color32, CornerRadius, Rect, Stroke, pos2, vec2};
use perro_ids::NodeID;
use perro_render_bridge::{UiCommand, UiRectState};
use std::borrow::Cow;

const UI_Z_BIAS: i32 = 10_000;

#[derive(Clone, Debug, PartialEq)]
struct UiPanelDraw {
    rect: UiRectState,
    fill: [f32; 4],
    stroke: [f32; 4],
    stroke_width: f32,
    corner_radius: f32,
}

#[derive(Clone, Debug, PartialEq)]
struct EguiPaintRect {
    rect: Rect,
    fill: Color32,
    stroke: Stroke,
    corner_radius: CornerRadius,
}

#[derive(Clone, Debug, PartialEq)]
struct UiLabelDraw {
    rect: UiRectState,
    text: Cow<'static, str>,
    color: [f32; 4],
    font_size: f32,
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

#[derive(Default)]
pub struct UiRenderer {
    nodes: AHashMap<NodeID, UiDraw>,
    revision: u64,
    primitives: Vec<RectInstanceGpu>,
    primitives_revision: u64,
}

impl UiRenderer {
    pub fn new() -> Self {
        Self {
            nodes: AHashMap::new(),
            revision: 0,
            primitives: Vec::new(),
            primitives_revision: u64::MAX,
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
            } => self.upsert(
                node,
                UiDraw::Label(UiLabelDraw {
                    rect,
                    text,
                    color,
                    font_size,
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

    pub fn primitives(&mut self) -> &[RectInstanceGpu] {
        if self.primitives_revision != self.revision {
            self.rebuild_primitives();
        }
        &self.primitives
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

    fn rebuild_primitives(&mut self) {
        self.primitives.clear();
        if self.primitives.capacity() < self.nodes.len() {
            self.primitives
                .reserve(self.nodes.len() - self.primitives.capacity());
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
                UiDraw::Panel(panel) => push_panel_primitives(panel, &mut self.primitives),
                UiDraw::Button(button) => {
                    push_panel_primitives(&button.panel, &mut self.primitives)
                }
                UiDraw::Label(label) => {
                    let _ = (
                        label.rect,
                        label.text.as_ref(),
                        label.color,
                        label.font_size,
                    );
                }
            }
        }
        self.primitives_revision = self.revision;
    }
}

fn ui_rect(draw: &UiDraw) -> UiRectState {
    match draw {
        UiDraw::Panel(panel) => panel.rect,
        UiDraw::Button(button) => button.panel.rect,
        UiDraw::Label(label) => label.rect,
    }
}

fn push_panel_primitives(panel: &UiPanelDraw, out: &mut Vec<RectInstanceGpu>) {
    let rect = panel.rect;
    if !valid_rect(rect) || !valid_color(panel.fill) {
        return;
    }

    let paint = panel.to_epaint_rect();
    out.push(rect_instance(rect, panel.fill, 0, 1.0, true));

    if paint.stroke.width <= 0.0 || !paint.stroke.width.is_finite() || !valid_color(panel.stroke) {
        return;
    }

    let half_w = rect.size[0] * 0.5;
    let half_h = rect.size[1] * 0.5;
    let thickness = paint.stroke.width.min(half_w).min(half_h).max(0.0);
    if thickness <= 0.0 {
        return;
    }

    let edge_z = UiRectState {
        z_index: rect.z_index.saturating_add(1),
        ..rect
    };
    out.push(rect_instance(
        UiRectState {
            center: [rect.center[0], rect.center[1] + half_h - thickness * 0.5],
            size: [rect.size[0], thickness],
            ..edge_z
        },
        panel.stroke,
        0,
        1.0,
        true,
    ));
    out.push(rect_instance(
        UiRectState {
            center: [rect.center[0], rect.center[1] - half_h + thickness * 0.5],
            size: [rect.size[0], thickness],
            ..edge_z
        },
        panel.stroke,
        0,
        1.0,
        true,
    ));
    out.push(rect_instance(
        UiRectState {
            center: [rect.center[0] - half_w + thickness * 0.5, rect.center[1]],
            size: [thickness, rect.size[1]],
            ..edge_z
        },
        panel.stroke,
        0,
        1.0,
        true,
    ));
    out.push(rect_instance(
        UiRectState {
            center: [rect.center[0] + half_w - thickness * 0.5, rect.center[1]],
            size: [thickness, rect.size[1]],
            ..edge_z
        },
        panel.stroke,
        0,
        1.0,
        true,
    ));
}

impl UiPanelDraw {
    fn to_epaint_rect(&self) -> EguiPaintRect {
        let min = pos2(
            self.rect.center[0] - self.rect.size[0] * 0.5,
            self.rect.center[1] - self.rect.size[1] * 0.5,
        );
        EguiPaintRect {
            rect: Rect::from_min_size(min, vec2(self.rect.size[0], self.rect.size[1])),
            fill: color32(self.fill),
            stroke: Stroke::new(self.stroke_width.max(0.0), color32(self.stroke)),
            corner_radius: CornerRadius::same(self.corner_radius.max(0.0) as u8),
        }
    }
}

fn color32(color: [f32; 4]) -> Color32 {
    let r = (color[0].clamp(0.0, 1.0) * 255.0).round() as u8;
    let g = (color[1].clamp(0.0, 1.0) * 255.0).round() as u8;
    let b = (color[2].clamp(0.0, 1.0) * 255.0).round() as u8;
    let a = (color[3].clamp(0.0, 1.0) * 255.0).round() as u8;
    Color32::from_rgba_unmultiplied(r, g, b, a)
}

fn rect_instance(
    rect: UiRectState,
    color: [f32; 4],
    shape_kind: u32,
    thickness: f32,
    filled: bool,
) -> RectInstanceGpu {
    RectInstanceGpu {
        center: rect.center,
        size: rect.size,
        color,
        z_index: rect.z_index.saturating_add(UI_Z_BIAS),
        shape_kind,
        thickness,
        filled: u32::from(filled),
        _pad: 0,
    }
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn panel_builds_fill_and_stroke_primitives() {
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

        let primitives = renderer.primitives();

        assert_eq!(primitives.len(), 5);
        assert_eq!(primitives[0].center, [10.0, 20.0]);
        assert_eq!(primitives[0].size, [100.0, 50.0]);
        assert_eq!(primitives[0].z_index, UI_Z_BIAS + 2);
    }

    #[test]
    fn clear_removes_primitives() {
        let mut renderer = UiRenderer::new();
        renderer.submit(UiCommand::UpsertButton {
            node: NodeID::from_parts(2, 0),
            rect: UiRectState {
                center: [0.0, 0.0],
                size: [24.0, 12.0],
                z_index: 0,
            },
            text: Cow::Borrowed("Run"),
            text_color: [1.0, 1.0, 1.0, 1.0],
            fill: [0.0, 0.0, 0.0, 1.0],
            stroke: [1.0, 1.0, 1.0, 1.0],
            stroke_width: 1.0,
            corner_radius: 0.0,
            disabled: false,
        });
        assert!(!renderer.primitives().is_empty());

        renderer.submit(UiCommand::Clear);

        assert!(renderer.primitives().is_empty());
    }
}
