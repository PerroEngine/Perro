use super::painter::{EpaintUiPainter, UiPaintFrame, UiPainter};
use ahash::AHashMap;
use perro_ids::NodeID;
use perro_render_bridge::{UiCommand, UiRectState, UiTextAlignState};
use std::borrow::Cow;

#[derive(Clone, Debug, PartialEq)]
pub(crate) struct UiPanelDraw {
    pub(crate) rect: UiRectState,
    pub(crate) clip_rect: [f32; 4],
    pub(crate) fill: [f32; 4],
    pub(crate) stroke: [f32; 4],
    pub(crate) stroke_width: f32,
    pub(crate) corner_radius: f32,
}

#[derive(Clone, Debug, PartialEq)]
pub(crate) struct UiLabelDraw {
    pub(crate) rect: UiRectState,
    pub(crate) clip_rect: [f32; 4],
    pub(crate) text: Cow<'static, str>,
    pub(crate) color: [f32; 4],
    pub(crate) font_size: f32,
    pub(crate) h_align: UiTextAlignState,
    pub(crate) v_align: UiTextAlignState,
}

#[derive(Clone, Debug, PartialEq)]
pub(crate) struct UiButtonDraw {
    pub(crate) panel: UiPanelDraw,
    pub(crate) disabled: bool,
}

#[derive(Clone, Debug, PartialEq)]
pub(crate) struct UiTextEditDraw {
    pub(crate) panel: UiPanelDraw,
    pub(crate) text: Cow<'static, str>,
    pub(crate) placeholder: Cow<'static, str>,
    pub(crate) color: [f32; 4],
    pub(crate) placeholder_color: [f32; 4],
    pub(crate) selection_color: [f32; 4],
    pub(crate) caret_color: [f32; 4],
    pub(crate) font_size: f32,
    pub(crate) padding: [f32; 4],
    pub(crate) scroll: [f32; 2],
    pub(crate) caret: usize,
    pub(crate) anchor: usize,
    pub(crate) focused: bool,
    pub(crate) multiline: bool,
}

#[derive(Clone, Debug, PartialEq)]
pub(crate) enum UiDraw {
    Panel(UiPanelDraw),
    Button(UiButtonDraw),
    Label(UiLabelDraw),
    TextEdit(UiTextEditDraw),
}

pub struct UiRenderer {
    nodes: AHashMap<NodeID, UiDraw>,
    revision: u64,
    painter: EpaintUiPainter,
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
            painter: EpaintUiPainter::new(),
        }
    }

    pub fn submit(&mut self, command: UiCommand) {
        match command {
            UiCommand::UpsertPanel {
                node,
                rect,
                clip_rect,
                fill,
                stroke,
                stroke_width,
                corner_radius,
            } => self.upsert(
                node,
                UiDraw::Panel(UiPanelDraw {
                    rect,
                    clip_rect,
                    fill,
                    stroke,
                    stroke_width,
                    corner_radius,
                }),
            ),
            UiCommand::UpsertButton {
                node,
                rect,
                clip_rect,
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
                        clip_rect,
                        fill,
                        stroke,
                        stroke_width,
                        corner_radius,
                    },
                    disabled,
                }),
            ),
            UiCommand::UpsertLabel {
                node,
                rect,
                clip_rect,
                text,
                color,
                font_size,
                h_align,
                v_align,
            } => self.upsert(
                node,
                UiDraw::Label(UiLabelDraw {
                    rect,
                    clip_rect,
                    text,
                    color,
                    font_size,
                    h_align,
                    v_align,
                }),
            ),
            UiCommand::UpsertTextEdit {
                node,
                rect,
                clip_rect,
                fill,
                stroke,
                stroke_width,
                corner_radius,
                text,
                placeholder,
                color,
                placeholder_color,
                selection_color,
                caret_color,
                font_size,
                padding,
                scroll,
                caret,
                anchor,
                focused,
                multiline,
            } => self.upsert(
                node,
                UiDraw::TextEdit(UiTextEditDraw {
                    panel: UiPanelDraw {
                        rect,
                        clip_rect,
                        fill,
                        stroke,
                        stroke_width,
                        corner_radius,
                    },
                    text,
                    placeholder,
                    color,
                    placeholder_color,
                    selection_color,
                    caret_color,
                    font_size,
                    padding,
                    scroll,
                    caret,
                    anchor,
                    focused,
                    multiline,
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
        self.painter.paint(&self.nodes, self.revision, viewport)
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
                pivot: [0.5, 0.5],
                rotation_radians: 0.0,
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
                pivot: [0.5, 0.5],
                rotation_radians: 0.0,
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

    #[test]
    fn panel_rotation_changes_mesh_bounds() {
        let mut renderer = UiRenderer::new();
        renderer.submit(UiCommand::UpsertPanel {
            node: NodeID::from_parts(3, 0),
            rect: UiRectState {
                center: [0.0, 0.0],
                size: [100.0, 50.0],
                pivot: [0.5, 0.5],
                rotation_radians: std::f32::consts::FRAC_PI_2,
                z_index: 0,
            },
            fill: [0.1, 0.2, 0.3, 1.0],
            stroke: [0.0, 0.0, 0.0, 0.0],
            stroke_width: 0.0,
            corner_radius: 0.0,
        });

        let paint = renderer.prepare_paint([800.0, 600.0]);
        let mut min = [f32::INFINITY, f32::INFINITY];
        let mut max = [f32::NEG_INFINITY, f32::NEG_INFINITY];
        for primitive in paint.primitives {
            if let epaint::Primitive::Mesh(mesh) = &primitive.primitive {
                for vertex in &mesh.vertices {
                    min[0] = min[0].min(vertex.pos.x);
                    min[1] = min[1].min(vertex.pos.y);
                    max[0] = max[0].max(vertex.pos.x);
                    max[1] = max[1].max(vertex.pos.y);
                }
            }
        }

        let width = max[0] - min[0];
        let height = max[1] - min[1];
        assert!(width < 60.0, "width={width}");
        assert!(height > 90.0, "height={height}");
    }

    #[test]
    fn panel_rotation_uses_pivot_origin() {
        let mut renderer = UiRenderer::new();
        renderer.submit(UiCommand::UpsertPanel {
            node: NodeID::from_parts(4, 0),
            rect: UiRectState {
                center: [0.0, 0.0],
                size: [100.0, 50.0],
                pivot: [0.0, 0.5],
                rotation_radians: std::f32::consts::PI,
                z_index: 0,
            },
            fill: [0.1, 0.2, 0.3, 1.0],
            stroke: [0.0, 0.0, 0.0, 0.0],
            stroke_width: 0.0,
            corner_radius: 0.0,
        });

        let paint = renderer.prepare_paint([800.0, 600.0]);
        let mut min_x = f32::INFINITY;
        let mut max_x = f32::NEG_INFINITY;
        for primitive in paint.primitives {
            if let epaint::Primitive::Mesh(mesh) = &primitive.primitive {
                for vertex in &mesh.vertices {
                    min_x = min_x.min(vertex.pos.x);
                    max_x = max_x.max(vertex.pos.x);
                }
            }
        }

        assert!(min_x < 260.0, "min_x={min_x}");
        assert!(max_x < 360.0, "max_x={max_x}");
    }
}
