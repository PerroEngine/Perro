use super::renderer::{UiDraw, UiLabelDraw, UiPanelDraw, UiTextEditDraw};
use ahash::AHashMap;
use epaint::{
    AlphaFromCoverage, ClippedPrimitive, ClippedShape, Color32, CornerRadius, FontFamily, FontId,
    Fonts, Galley, Primitive, Rect, RectShape, Shape, Stroke, StrokeKind, TessellationOptions,
    Tessellator, emath::Rot2, pos2, text::FontDefinitions, textures::TexturesDelta, vec2,
};
use perro_ids::NodeID;
use perro_render_bridge::{UiRectState, UiTextAlignState};

pub struct UiPaintFrame<'a> {
    pub primitives: &'a [ClippedPrimitive],
    pub textures_delta: &'a TexturesDelta,
    pub texture_size: [u32; 2],
    pub revision: u64,
}

pub(crate) trait UiPainter {
    fn paint<'a>(
        &'a mut self,
        nodes: &AHashMap<NodeID, UiDraw>,
        revision: u64,
        viewport: [f32; 2],
    ) -> UiPaintFrame<'a>;
}

pub(crate) struct EpaintUiPainter {
    fonts: Fonts,
    shapes: Vec<ClippedShape>,
    shape_rotations: Vec<(f32, epaint::Pos2)>,
    primitives: Vec<ClippedPrimitive>,
    textures_delta: TexturesDelta,
    last_viewport: [f32; 2],
    paint_revision: u64,
}

impl Default for EpaintUiPainter {
    fn default() -> Self {
        Self::new()
    }
}

impl EpaintUiPainter {
    pub fn new() -> Self {
        Self {
            fonts: Fonts::new(
                2048,
                AlphaFromCoverage::default(),
                FontDefinitions::default(),
            ),
            shapes: Vec::new(),
            shape_rotations: Vec::new(),
            primitives: Vec::new(),
            textures_delta: TexturesDelta::default(),
            last_viewport: [0.0, 0.0],
            paint_revision: u64::MAX,
        }
    }

    fn rebuild_primitives(
        &mut self,
        nodes: &AHashMap<NodeID, UiDraw>,
        revision: u64,
        viewport: [f32; 2],
    ) {
        self.fonts.begin_pass(2048, AlphaFromCoverage::default());
        self.shapes.clear();
        self.shape_rotations.clear();
        self.primitives.clear();
        if self.shapes.capacity() < nodes.len() {
            self.shapes.reserve(nodes.len() - self.shapes.capacity());
        }

        let mut ordered: Vec<(NodeID, &UiDraw)> =
            nodes.iter().map(|(node, draw)| (*node, draw)).collect();
        ordered.sort_unstable_by(|(a_node, a_draw), (b_node, b_draw)| {
            ui_rect(a_draw)
                .z_index
                .cmp(&ui_rect(b_draw).z_index)
                .then_with(|| a_node.as_u64().cmp(&b_node.as_u64()))
        });

        for (_, draw) in ordered {
            let shape_start = self.shapes.len();
            match draw {
                UiDraw::Panel(panel) => push_panel_shape(panel, viewport, &mut self.shapes),
                UiDraw::Button(button) => {
                    push_panel_shape(&button.panel, viewport, &mut self.shapes)
                }
                UiDraw::Label(label) => {
                    push_label_shape(label, viewport, &mut self.fonts, &mut self.shapes)
                }
                UiDraw::TextEdit(edit) => {
                    push_panel_shape(&edit.panel, viewport, &mut self.shapes);
                    push_text_edit_shapes(edit, viewport, &mut self.fonts, &mut self.shapes);
                }
            }
            let rect = ui_rect(draw);
            let rotation = rect.rotation_radians;
            let origin = screen_pivot(rect, viewport);
            self.shape_rotations
                .extend((shape_start..self.shapes.len()).map(|_| (rotation, origin)));
        }
        let mut tessellator = Tessellator::new(
            1.0,
            TessellationOptions::default(),
            self.fonts.font_image_size(),
            self.fonts.texture_atlas().prepared_discs(),
        );
        self.primitives = tessellator.tessellate_shapes(std::mem::take(&mut self.shapes));
        rotate_primitives(&mut self.primitives, &self.shape_rotations);
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
        self.paint_revision = revision;
        self.last_viewport = viewport;
    }
}

impl UiPainter for EpaintUiPainter {
    fn paint<'a>(
        &'a mut self,
        nodes: &AHashMap<NodeID, UiDraw>,
        revision: u64,
        viewport: [f32; 2],
    ) -> UiPaintFrame<'a> {
        let viewport = [viewport[0].max(1.0), viewport[1].max(1.0)];
        if self.paint_revision != revision || self.last_viewport != viewport {
            self.rebuild_primitives(nodes, revision, viewport);
        }
        UiPaintFrame {
            primitives: &self.primitives,
            textures_delta: &self.textures_delta,
            texture_size: font_texture_size(&self.fonts),
            revision: self.paint_revision,
        }
    }
}

fn font_texture_size(fonts: &Fonts) -> [u32; 2] {
    let size = fonts.font_image_size();
    [
        size[0].min(u32::MAX as usize) as u32,
        size[1].min(u32::MAX as usize) as u32,
    ]
}

fn ui_rect(draw: &UiDraw) -> UiRectState {
    match draw {
        UiDraw::Panel(panel) => panel.rect,
        UiDraw::Button(button) => button.panel.rect,
        UiDraw::Label(label) => label.rect,
        UiDraw::TextEdit(edit) => edit.panel.rect,
    }
}

fn push_label_shape(
    label: &UiLabelDraw,
    viewport: [f32; 2],
    fonts: &mut Fonts,
    out: &mut Vec<ClippedShape>,
) {
    push_text_shape(
        TextShapeInput {
            rect: label.rect,
            viewport,
            clip_rect: clip_rect_from_state(label.clip_rect, viewport),
            text: label.text.as_ref(),
            font_size: label.font_size,
            color: label.color,
            h_align: label.h_align,
            v_align: label.v_align,
        },
        fonts,
        out,
    );
}

fn push_panel_shape(panel: &UiPanelDraw, viewport: [f32; 2], out: &mut Vec<ClippedShape>) {
    if !valid_rect(panel.rect) || !valid_color(panel.fill) || !valid_color(panel.stroke) {
        return;
    }

    let (min, max) = panel.rect.screen_min_max(viewport);
    let rect = Rect::from_min_max(pos2(min[0], min[1]), pos2(max[0], max[1]));
    out.push(ClippedShape {
        clip_rect: clip_rect_from_state(panel.clip_rect, viewport),
        shape: Shape::Rect(RectShape::new(
            rect,
            CornerRadius::same(resolve_corner_radius(panel) as u8),
            color32(panel.fill),
            Stroke::new(panel.stroke_width.max(0.0), color32(panel.stroke)),
            StrokeKind::Inside,
        )),
    });
}

fn push_text_edit_shapes(
    edit: &UiTextEditDraw,
    viewport: [f32; 2],
    fonts: &mut Fonts,
    out: &mut Vec<ClippedShape>,
) {
    let panel = &edit.panel;
    if !valid_rect(panel.rect) || !edit.font_size.is_finite() || edit.font_size <= 0.0 {
        return;
    }

    let (min, max) = panel.rect.screen_min_max(viewport);
    let content_min = pos2(min[0] + edit.padding[0], min[1] + edit.padding[1]);
    let content_max = pos2(max[0] - edit.padding[2], max[1] - edit.padding[3]);
    if content_max.x <= content_min.x || content_max.y <= content_min.y {
        return;
    }
    let clip_rect = Rect::from_min_max(content_min, content_max)
        .intersect(clip_rect_from_state(panel.clip_rect, viewport));
    let draw_pos = pos2(
        content_min.x - edit.scroll[0],
        content_min.y - edit.scroll[1],
    );
    let body = if edit.text.is_empty() {
        edit.placeholder.as_ref()
    } else {
        edit.text.as_ref()
    };
    let color = if edit.text.is_empty() {
        edit.placeholder_color
    } else {
        edit.color
    };
    let wrap_width = if edit.multiline {
        (content_max.x - content_min.x).max(1.0)
    } else {
        f32::INFINITY
    };
    let edit_galley = if edit.focused {
        Some(fonts.with_pixels_per_point(1.0).layout(
            edit.text.to_string(),
            FontId::new(edit.font_size, FontFamily::Monospace),
            color32(edit.color),
            wrap_width,
        ))
    } else {
        None
    };
    if edit.focused {
        if let Some(galley) = edit_galley.as_deref() {
            push_selection_shapes(edit, galley, clip_rect, draw_pos, out);
        }
    }
    if !body.is_empty() && valid_color(color) {
        let galley = fonts.with_pixels_per_point(1.0).layout(
            body.to_string(),
            FontId::new(edit.font_size, FontFamily::Monospace),
            color32(color),
            wrap_width,
        );
        out.push(ClippedShape {
            clip_rect,
            shape: Shape::galley_with_override_text_color(draw_pos, galley, color32(color)),
        });
    }

    if !edit.focused {
        return;
    }
    if let Some(galley) = edit_galley.as_deref() {
        push_caret_shape(edit, galley, clip_rect, draw_pos, out);
    }
}

fn push_selection_shapes(
    edit: &UiTextEditDraw,
    galley: &Galley,
    clip_rect: Rect,
    origin: epaint::Pos2,
    out: &mut Vec<ClippedShape>,
) {
    if edit.caret == edit.anchor || edit.text.is_empty() || !valid_color(edit.selection_color) {
        return;
    }
    let (start, end) = if edit.caret < edit.anchor {
        (edit.caret, edit.anchor)
    } else {
        (edit.anchor, edit.caret)
    };
    for row in galley_text_rows(edit.text.as_ref(), galley) {
        let sel_start = start.max(row.start).min(row.end);
        let sel_end = end.max(row.start).min(row.end);
        if sel_start >= sel_end {
            continue;
        }
        let placed = &galley.rows[row.index];
        let x0 = origin.x
            + placed.pos.x
            + placed.x_offset(byte_col(edit.text.as_ref(), row.start, sel_start));
        let x1 = origin.x
            + placed.pos.x
            + placed.x_offset(byte_col(edit.text.as_ref(), row.start, sel_end));
        let y0 = origin.y + placed.pos.y;
        let line_h = placed.height().max(1.0);
        let rect = Rect::from_min_max(pos2(x0, y0), pos2(x1.max(x0 + 1.0), y0 + line_h));
        out.push(ClippedShape {
            clip_rect,
            shape: Shape::Rect(RectShape::filled(
                rect,
                CornerRadius::ZERO,
                color32(edit.selection_color),
            )),
        });
    }
}

fn push_caret_shape(
    edit: &UiTextEditDraw,
    galley: &Galley,
    clip_rect: Rect,
    origin: epaint::Pos2,
    out: &mut Vec<ClippedShape>,
) {
    if !valid_color(edit.caret_color) {
        return;
    }
    let caret = clamp_char_boundary(edit.text.as_ref(), edit.caret);
    for row in galley_text_rows(edit.text.as_ref(), galley) {
        if caret >= row.start && caret <= row.end {
            let placed = &galley.rows[row.index];
            let x = origin.x
                + placed.pos.x
                + placed.x_offset(byte_col(edit.text.as_ref(), row.start, caret));
            let y = origin.y + placed.pos.y;
            let line_h = placed.height().max(1.0);
            let rect = Rect::from_min_max(pos2(x, y), pos2(x + 1.5, y + line_h));
            out.push(ClippedShape {
                clip_rect,
                shape: Shape::Rect(RectShape::filled(
                    rect,
                    CornerRadius::ZERO,
                    color32(edit.caret_color),
                )),
            });
            break;
        }
    }
}

fn rotate_primitives(primitives: &mut [ClippedPrimitive], rotations: &[(f32, epaint::Pos2)]) {
    for (primitive, &(rotation, origin)) in primitives.iter_mut().zip(rotations) {
        if !rotation.is_finite() || rotation == 0.0 {
            continue;
        }
        let rot = Rot2::from_angle(-rotation);
        primitive.clip_rect = Rect::EVERYTHING;
        if let Primitive::Mesh(mesh) = &mut primitive.primitive {
            mesh.rotate(rot, origin);
        }
    }
}

fn screen_center(rect: UiRectState, viewport: [f32; 2]) -> epaint::Pos2 {
    pos2(
        viewport[0] * 0.5 + rect.center[0],
        viewport[1] * 0.5 - rect.center[1],
    )
}

fn screen_pivot(rect: UiRectState, viewport: [f32; 2]) -> epaint::Pos2 {
    let center = screen_center(rect, viewport);
    pos2(
        center.x + (rect.pivot[0] - 0.5) * rect.size[0],
        center.y - (rect.pivot[1] - 0.5) * rect.size[1],
    )
}

fn resolve_corner_radius(panel: &UiPanelDraw) -> f32 {
    if panel.corner_radius.is_infinite() {
        panel.rect.size[0].min(panel.rect.size[1]) * 0.5
    } else {
        panel.corner_radius.max(0.0)
    }
    .min(u8::MAX as f32)
}

struct TextShapeInput<'a> {
    rect: UiRectState,
    viewport: [f32; 2],
    clip_rect: Rect,
    text: &'a str,
    font_size: f32,
    color: [f32; 4],
    h_align: UiTextAlignState,
    v_align: UiTextAlignState,
}

fn push_text_shape(input: TextShapeInput<'_>, fonts: &mut Fonts, out: &mut Vec<ClippedShape>) {
    let TextShapeInput {
        rect,
        viewport,
        clip_rect,
        text,
        font_size,
        color,
        h_align,
        v_align,
    } = input;
    if text.is_empty()
        || !valid_rect(rect)
        || !valid_color(color)
        || !font_size.is_finite()
        || font_size <= 0.0
    {
        return;
    }

    let (min, max) = rect.screen_min_max(viewport);
    let clip_rect = Rect::from_min_max(pos2(min[0], min[1]), pos2(max[0], max[1]))
        .intersect(clip_rect);
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

fn clip_rect_from_state(clip: [f32; 4], viewport: [f32; 2]) -> Rect {
    let fallback = viewport_rect(viewport);
    let min_x = clip[0].max(0.0).min(viewport[0]);
    let min_y = clip[1].max(0.0).min(viewport[1]);
    let max_x = clip[2].max(min_x).min(viewport[0]);
    let max_y = clip[3].max(min_y).min(viewport[1]);
    if !min_x.is_finite() || !min_y.is_finite() || !max_x.is_finite() || !max_y.is_finite() {
        return fallback;
    }
    Rect::from_min_max(pos2(min_x, min_y), pos2(max_x, max_y))
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

#[derive(Clone, Copy)]
struct GalleyTextRow {
    index: usize,
    start: usize,
    end: usize,
}

fn galley_text_rows(text: &str, galley: &Galley) -> Vec<GalleyTextRow> {
    let mut out = Vec::with_capacity(galley.rows.len());
    let mut start = 0usize;
    for (index, row) in galley.rows.iter().enumerate() {
        let end = advance_chars(text, start, row.char_count_excluding_newline());
        out.push(GalleyTextRow { index, start, end });
        start = if row.ends_with_newline {
            advance_chars(text, end, 1)
        } else {
            end
        };
    }
    if out.is_empty() {
        out.push(GalleyTextRow {
            index: 0,
            start: 0,
            end: 0,
        });
    }
    out
}

fn byte_col(text: &str, start: usize, end: usize) -> usize {
    let start = clamp_char_boundary(text, start);
    let end = clamp_char_boundary(text, end);
    text[start.min(end)..end].chars().count()
}

fn advance_chars(text: &str, start: usize, count: usize) -> usize {
    let start = clamp_char_boundary(text, start);
    if count == 0 {
        return start;
    }
    text[start..]
        .char_indices()
        .nth(count)
        .map(|(idx, _)| start + idx)
        .unwrap_or(text.len())
}

fn clamp_char_boundary(text: &str, mut index: usize) -> usize {
    index = index.min(text.len());
    while index > 0 && !text.is_char_boundary(index) {
        index -= 1;
    }
    index
}
