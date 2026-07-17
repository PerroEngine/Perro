use super::painter::{EpaintUiPainter, UiPaintFrame, UiPainter};
use ahash::AHashMap;
use perro_ids::{NodeID, TextureID};
use perro_render_bridge::{
    UiCommand, UiCornerRadiiState, UiDepthEffectState, UiFillKindState, UiImageScaleState,
    UiLinearGradientState, UiRectState, UiShapeKind, UiTextAlignState,
};
use perro_structs::Color;
use std::borrow::Cow;

#[derive(Clone, Debug, PartialEq)]
pub(crate) struct UiPanelDraw {
    pub(crate) rect: UiRectState,
    pub(crate) clip_rect: [f32; 4],
    pub(crate) fill: Color,
    pub(crate) fill_kind: UiFillKindState,
    pub(crate) gradient: UiLinearGradientState,
    pub(crate) stroke: Color,
    pub(crate) stroke_width: f32,
    pub(crate) corner_radii: UiCornerRadiiState,
    pub(crate) outer_shadow: UiDepthEffectState,
    pub(crate) inner_shadow: UiDepthEffectState,
    pub(crate) outer_highlight: UiDepthEffectState,
    pub(crate) inner_highlight: UiDepthEffectState,
}

#[derive(Clone, Debug, PartialEq)]
pub(crate) struct UiProgressBarDraw {
    pub(crate) rect: UiRectState,
    pub(crate) clip_rect: [f32; 4],
    pub(crate) value: f32,
    pub(crate) background_fill: Color,
    pub(crate) background_corner_radii: UiCornerRadiiState,
    pub(crate) fill: Color,
    pub(crate) fill_corner_radii: UiCornerRadiiState,
}

#[derive(Clone, Debug, PartialEq)]
pub(crate) struct UiLabelDraw {
    pub(crate) rect: UiRectState,
    pub(crate) clip_rect: [f32; 4],
    pub(crate) text: Cow<'static, str>,
    pub(crate) color: Color,
    pub(crate) font_size: f32,
    pub(crate) font: perro_ui::UiFont,
    pub(crate) wrap_width: Option<f32>,
    pub(crate) h_align: UiTextAlignState,
    pub(crate) v_align: UiTextAlignState,
    pub(crate) backdrop_color: Color,
    pub(crate) corner_radii: UiCornerRadiiState,
    pub(crate) padding: [f32; 4],
    pub(crate) projected_quad: Option<[[f32; 4]; 4]>,
    pub(crate) fit_content: bool,
}

#[derive(Clone, Debug, PartialEq)]
pub(crate) struct UiButtonDraw {
    pub(crate) panel: UiPanelDraw,
    pub(crate) disabled: bool,
}

#[derive(Clone, Debug, PartialEq)]
pub(crate) struct UiShapeDraw {
    pub(crate) rect: UiRectState,
    pub(crate) clip_rect: [f32; 4],
    pub(crate) kind: UiShapeKind,
    pub(crate) fill: Color,
    pub(crate) stroke: Color,
    pub(crate) stroke_width: f32,
}

#[derive(Clone, Debug, PartialEq)]
pub(crate) struct UiColorWheelDraw {
    pub(crate) rect: UiRectState,
    pub(crate) clip_rect: [f32; 4],
    pub(crate) mode: perro_render_bridge::UiColorPickerMode,
    pub(crate) selected: Color,
}

#[derive(Clone, Debug, PartialEq)]
pub(crate) struct UiCheckboxDraw {
    pub(crate) panel: UiPanelDraw,
    pub(crate) checked: bool,
    pub(crate) dot_fill: Color,
    pub(crate) disabled: bool,
}

#[derive(Clone, Debug, PartialEq)]
pub(crate) struct UiImageDraw {
    pub(crate) rect: UiRectState,
    pub(crate) clip_rect: [f32; 4],
    pub(crate) texture: TextureID,
    pub(crate) tint: Color,
    pub(crate) uv_min: [f32; 2],
    pub(crate) uv_max: [f32; 2],
    pub(crate) scale_mode: UiImageScaleState,
    pub(crate) h_align: UiTextAlignState,
    pub(crate) v_align: UiTextAlignState,
    pub(crate) aspect_ratio: f32,
    pub(crate) corner_radii: UiCornerRadiiState,
}

#[derive(Clone, Debug, PartialEq)]
pub(crate) struct UiNineSliceDraw {
    pub(crate) rect: UiRectState,
    pub(crate) clip_rect: [f32; 4],
    pub(crate) texture: TextureID,
    pub(crate) tint: Color,
    pub(crate) uv_min: [f32; 2],
    pub(crate) uv_max: [f32; 2],
    pub(crate) margins: [f32; 4],
    pub(crate) texture_size: [u32; 2],
}

#[derive(Clone, Debug, PartialEq)]
pub(crate) struct UiTextEditDraw {
    pub(crate) panel: UiPanelDraw,
    pub(crate) text: Cow<'static, str>,
    pub(crate) placeholder: Cow<'static, str>,
    pub(crate) color: Color,
    pub(crate) placeholder_color: Color,
    pub(crate) selection_color: Color,
    pub(crate) caret_color: Color,
    pub(crate) font_size: f32,
    pub(crate) font: perro_ui::UiFont,
    pub(crate) h_align: UiTextAlignState,
    pub(crate) v_align: UiTextAlignState,
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
    ProgressBar(UiProgressBarDraw),
    Shape(UiShapeDraw),
    ColorWheel(UiColorWheelDraw),
    Button(UiButtonDraw),
    Checkbox(UiCheckboxDraw),
    Image(UiImageDraw),
    NineSlice(UiNineSliceDraw),
    Label(UiLabelDraw),
    TextEdit(UiTextEditDraw),
}

pub struct UiRenderer {
    nodes: AHashMap<NodeID, UiDraw>,
    revision: u64,
    painter: EpaintUiPainter,
    static_font_lookup: Option<crate::StaticFontLookup>,
    default_font: perro_ui::UiFont,
    // Path hashes already handed to the painter; labels re-submit every
    // change, so gate here keeps registration off the per-frame path.
    registered_resource_fonts: ahash::AHashSet<u64>,
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
            static_font_lookup: None,
            default_font: perro_ui::UiFont::Default,
            registered_resource_fonts: ahash::AHashSet::new(),
        }
    }

    pub fn submit(&mut self, mut command: UiCommand) {
        match &mut command {
            UiCommand::UpsertLabel { font, .. } | UiCommand::UpsertTextEdit { font, .. }
                if matches!(font, perro_ui::UiFont::Default) =>
            {
                *font = self.default_font.clone();
            }
            _ => {}
        }
        match &command {
            UiCommand::UpsertLabel {
                font: perro_ui::UiFont::Resource(path),
                ..
            }
            | UiCommand::UpsertTextEdit {
                font: perro_ui::UiFont::Resource(path),
                ..
            } => {
                let hash = perro_ids::string_to_u64(path);
                if self.registered_resource_fonts.insert(hash) {
                    self.painter
                        .register_resource_font(path, self.static_font_lookup);
                }
            }
            _ => {}
        }
        match command {
            UiCommand::UpsertProgressBar {
                node,
                rect,
                clip_rect,
                value,
                background_fill,
                background_corner_radii,
                fill,
                fill_corner_radii,
            } => self.upsert(
                node,
                UiDraw::ProgressBar(UiProgressBarDraw {
                    rect,
                    clip_rect,
                    value: value.clamp(0.0, 1.0),
                    background_fill: background_fill.into(),
                    background_corner_radii,
                    fill: fill.into(),
                    fill_corner_radii,
                }),
            ),
            UiCommand::UpsertPanel {
                node,
                rect,
                clip_rect,
                fill,
                fill_kind,
                gradient,
                stroke,
                stroke_width,
                corner_radii,
                outer_shadow,
                inner_shadow,
                outer_highlight,
                inner_highlight,
            } => self.upsert(
                node,
                UiDraw::Panel(UiPanelDraw {
                    rect,
                    clip_rect,
                    fill: fill.into(),
                    fill_kind,
                    gradient,
                    stroke: stroke.into(),
                    stroke_width,
                    corner_radii,
                    outer_shadow,
                    inner_shadow,
                    outer_highlight,
                    inner_highlight,
                }),
            ),
            UiCommand::UpsertButton {
                node,
                rect,
                clip_rect,
                fill,
                fill_kind,
                gradient,
                stroke,
                stroke_width,
                corner_radii,
                outer_shadow,
                inner_shadow,
                outer_highlight,
                inner_highlight,
                disabled,
            } => self.upsert(
                node,
                UiDraw::Button(UiButtonDraw {
                    panel: UiPanelDraw {
                        rect,
                        clip_rect,
                        fill: fill.into(),
                        fill_kind,
                        gradient,
                        stroke: stroke.into(),
                        stroke_width,
                        corner_radii,
                        outer_shadow,
                        inner_shadow,
                        outer_highlight,
                        inner_highlight,
                    },
                    disabled,
                }),
            ),
            UiCommand::UpsertShape {
                node,
                rect,
                clip_rect,
                kind,
                fill,
                stroke,
                stroke_width,
            } => self.upsert(
                node,
                UiDraw::Shape(UiShapeDraw {
                    rect,
                    clip_rect,
                    kind,
                    fill: fill.into(),
                    stroke: stroke.into(),
                    stroke_width,
                }),
            ),
            UiCommand::UpsertColorWheel {
                node,
                rect,
                clip_rect,
                mode,
                selected,
            } => self.upsert(
                node,
                UiDraw::ColorWheel(UiColorWheelDraw {
                    rect,
                    clip_rect,
                    mode,
                    selected: selected.into(),
                }),
            ),
            UiCommand::UpsertCheckbox {
                node,
                rect,
                clip_rect,
                fill,
                fill_kind,
                gradient,
                stroke,
                stroke_width,
                corner_radii,
                outer_shadow,
                inner_shadow,
                outer_highlight,
                inner_highlight,
                checked,
                dot_fill,
                disabled,
            } => self.upsert(
                node,
                UiDraw::Checkbox(UiCheckboxDraw {
                    panel: UiPanelDraw {
                        rect,
                        clip_rect,
                        fill: fill.into(),
                        fill_kind,
                        gradient,
                        stroke: stroke.into(),
                        stroke_width,
                        corner_radii,
                        outer_shadow,
                        inner_shadow,
                        outer_highlight,
                        inner_highlight,
                    },
                    checked,
                    dot_fill: dot_fill.into(),
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
                font,
                wrap_width,
                h_align,
                v_align,
                backdrop_color,
                corner_radii,
                padding,
                projected_quad,
                fit_content,
            } => self.upsert(
                node,
                UiDraw::Label(UiLabelDraw {
                    rect,
                    clip_rect,
                    text,
                    color,
                    font_size,
                    font,
                    wrap_width,
                    h_align,
                    v_align,
                    backdrop_color,
                    corner_radii,
                    padding,
                    projected_quad,
                    fit_content,
                }),
            ),
            UiCommand::UpsertImage {
                node,
                rect,
                clip_rect,
                texture,
                tint,
                uv_min,
                uv_max,
                scale_mode,
                h_align,
                v_align,
                aspect_ratio,
                corner_radii,
            } => self.upsert(
                node,
                UiDraw::Image(UiImageDraw {
                    rect,
                    clip_rect,
                    texture,
                    tint,
                    uv_min,
                    uv_max,
                    scale_mode,
                    h_align,
                    v_align,
                    aspect_ratio,
                    corner_radii,
                }),
            ),
            UiCommand::UpsertNineSlice {
                node,
                rect,
                clip_rect,
                texture,
                tint,
                uv_min,
                uv_max,
                margins,
            } => self.upsert(
                node,
                UiDraw::NineSlice(UiNineSliceDraw {
                    rect,
                    clip_rect,
                    texture,
                    tint,
                    uv_min,
                    uv_max,
                    margins,
                    texture_size: [0, 0],
                }),
            ),
            UiCommand::UpsertTextEdit {
                node,
                rect,
                clip_rect,
                fill,
                fill_kind,
                gradient,
                stroke,
                stroke_width,
                corner_radii,
                outer_shadow,
                inner_shadow,
                outer_highlight,
                inner_highlight,
                text,
                placeholder,
                color,
                placeholder_color,
                selection_color,
                caret_color,
                font_size,
                font,
                h_align,
                v_align,
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
                        fill: fill.into(),
                        fill_kind,
                        gradient,
                        stroke: stroke.into(),
                        stroke_width,
                        corner_radii,
                        outer_shadow,
                        inner_shadow,
                        outer_highlight,
                        inner_highlight,
                    },
                    text,
                    placeholder,
                    color,
                    placeholder_color,
                    selection_color,
                    caret_color,
                    font_size,
                    font,
                    h_align,
                    v_align,
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

    pub(crate) fn set_static_font_lookup(&mut self, lookup: crate::StaticFontLookup) {
        self.static_font_lookup = Some(lookup);
    }

    pub(crate) fn set_default_font(&mut self, font: perro_ui::UiFont) {
        self.default_font = font;
    }

    pub(crate) fn set_nine_slice_texture_sizes(&mut self, sizes: &AHashMap<TextureID, [u32; 2]>) {
        let mut changed = false;
        for draw in self.nodes.values_mut() {
            let UiDraw::NineSlice(image) = draw else {
                continue;
            };
            let size = sizes.get(&image.texture).copied().unwrap_or([0, 0]);
            if image.texture_size != size {
                image.texture_size = size;
                changed = true;
            }
        }
        if changed {
            self.revision = self.revision.wrapping_add(1);
        }
    }

    pub fn retained_count(&self) -> usize {
        self.nodes.len()
    }

    pub fn prepare_paint(&mut self, viewport: [f32; 2]) -> UiPaintFrame<'_> {
        self.painter.paint(&self.nodes, self.revision, viewport)
    }

    pub fn image_textures(&self) -> impl Iterator<Item = TextureID> + '_ {
        self.nodes.values().filter_map(|draw| match draw {
            UiDraw::Image(image) => Some(image.texture),
            UiDraw::NineSlice(image) => Some(image.texture),
            _ => None,
        })
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
            clip_rect: [0.0, 0.0, 800.0, 600.0],
            fill: [0.1, 0.2, 0.3, 1.0],
            fill_kind: UiFillKindState::Solid,
            gradient: UiLinearGradientState::none(),
            stroke: [1.0, 1.0, 1.0, 1.0],
            stroke_width: 2.0,
            corner_radii: UiCornerRadiiState {
                tl: 0.2,
                tr: 0.2,
                br: 0.2,
                bl: 0.2,
            },
            outer_shadow: UiDepthEffectState::none(),
            inner_shadow: UiDepthEffectState::none(),
            outer_highlight: UiDepthEffectState::none(),
            inner_highlight: UiDepthEffectState::none(),
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
            clip_rect: [0.0, 0.0, 800.0, 600.0],
            text: Cow::Borrowed("Run"),
            color: Color::WHITE,
            font_size: 18.0,
            font: perro_ui::UiFont::Default,
            wrap_width: None,
            h_align: UiTextAlignState::Center,
            v_align: UiTextAlignState::Center,
            backdrop_color: Color::TRANSPARENT,
            corner_radii: UiCornerRadiiState::default(),
            padding: [0.0; 4],
            projected_quad: Some([
                [-0.2, 0.1, -2.0, 1.0],
                [0.225, 0.15, 0.0, 1.0],
                [0.175, -0.125, 0.0, 1.0],
                [-0.15, -0.1, 0.0, 1.0],
            ]),
            fit_content: true,
        });

        let paint = renderer.prepare_paint([800.0, 600.0]);

        assert!(!paint.primitives.is_empty());
        assert!(!paint.textures_delta.set.is_empty());
        for primitive in paint.primitives {
            if let epaint::Primitive::Mesh(mesh) = &primitive.primitive {
                assert!(mesh.vertices.iter().all(|vertex| {
                    (320.0..=490.0).contains(&vertex.pos.x)
                        && (270.0..=325.0).contains(&vertex.pos.y)
                }));
            }
        }
    }

    fn projected_label_command(quad: [[f32; 4]; 4]) -> UiCommand {
        UiCommand::UpsertLabel {
            node: NodeID::from_parts(7, 0),
            rect: UiRectState {
                center: [0.0, 0.0],
                size: [200.0, 60.0],
                pivot: [0.5, 0.5],
                rotation_radians: 0.0,
                z_index: 0,
            },
            clip_rect: [0.0, 0.0, 800.0, 600.0],
            text: Cow::Borrowed("Run"),
            color: Color::WHITE,
            font_size: 18.0,
            font: perro_ui::UiFont::Default,
            wrap_width: None,
            h_align: UiTextAlignState::Center,
            v_align: UiTextAlignState::Center,
            backdrop_color: Color::TRANSPARENT,
            corner_radii: UiCornerRadiiState::default(),
            padding: [0.0; 4],
            projected_quad: Some(quad),
            fit_content: true,
        }
    }

    fn collect_mesh_vertices(paint: &super::UiPaintFrame<'_>) -> Vec<[f32; 4]> {
        let mut out = Vec::new();
        for primitive in paint.primitives {
            if let epaint::Primitive::Mesh(mesh) = &primitive.primitive {
                for vertex in &mesh.vertices {
                    out.push([vertex.pos.x, vertex.pos.y, vertex.uv.x, vertex.uv.y]);
                }
            }
        }
        out
    }

    #[test]
    fn projected_label_camera_move_reprojects_cache_identically_to_fresh() {
        let quad_a = [
            [-0.2, 0.1, -2.0, 1.0],
            [0.225, 0.15, 0.0, 1.0],
            [0.175, -0.125, 0.0, 1.0],
            [-0.15, -0.1, 0.0, 1.0],
        ];
        let quad_b = [
            [-0.3, 0.2, 0.0, 1.0],
            [0.3, 0.2, 0.0, 1.0],
            [0.25, -0.2, 0.0, 1.0],
            [-0.25, -0.2, 0.0, 1.0],
        ];

        // Warm renderer: paint quad A first so quad B goes through the
        // cached-unprojected reprojection path.
        let mut warm = UiRenderer::new();
        warm.submit(projected_label_command(quad_a));
        warm.prepare_paint([800.0, 600.0]);
        warm.submit(projected_label_command(quad_b));
        let warm_paint = warm.prepare_paint([800.0, 600.0]);
        let warm_vertices = collect_mesh_vertices(&warm_paint);

        // Fresh renderer tessellates quad B from scratch.
        let mut fresh = UiRenderer::new();
        fresh.submit(projected_label_command(quad_b));
        let fresh_paint = fresh.prepare_paint([800.0, 600.0]);
        let fresh_vertices = collect_mesh_vertices(&fresh_paint);

        assert!(!warm_vertices.is_empty());
        assert_eq!(warm_vertices, fresh_vertices);
    }

    #[test]
    fn unchanged_label_reuses_cached_primitives_across_rebuilds() {
        let mut renderer = UiRenderer::new();
        renderer.submit(UiCommand::UpsertLabel {
            node: NodeID::from_parts(8, 0),
            rect: UiRectState {
                center: [0.0, 0.0],
                size: [200.0, 60.0],
                pivot: [0.5, 0.5],
                rotation_radians: 0.0,
                z_index: 0,
            },
            clip_rect: [0.0, 0.0, 800.0, 600.0],
            text: Cow::Borrowed("Score"),
            color: Color::WHITE,
            font_size: 18.0,
            font: perro_ui::UiFont::Default,
            wrap_width: None,
            h_align: UiTextAlignState::Center,
            v_align: UiTextAlignState::Center,
            backdrop_color: Color::TRANSPARENT,
            corner_radii: UiCornerRadiiState::default(),
            padding: [0.0; 4],
            projected_quad: None,
            fit_content: false,
        });
        let first_ptrs: Vec<*const epaint::ClippedPrimitive> = renderer
            .prepare_paint([800.0, 600.0])
            .primitives
            .iter()
            .map(std::sync::Arc::as_ptr)
            .collect();
        assert!(!first_ptrs.is_empty());

        // Mutate an unrelated node; the label's tessellation must be reused
        // (same Arc allocations), not rebuilt.
        renderer.submit(UiCommand::UpsertPanel {
            node: NodeID::from_parts(9, 0),
            rect: UiRectState {
                center: [100.0, 100.0],
                size: [50.0, 50.0],
                pivot: [0.5, 0.5],
                rotation_radians: 0.0,
                z_index: 1,
            },
            clip_rect: [0.0, 0.0, 800.0, 600.0],
            fill: [0.5, 0.1, 0.1, 1.0],
            fill_kind: UiFillKindState::Solid,
            gradient: UiLinearGradientState::none(),
            stroke: [0.0, 0.0, 0.0, 0.0],
            stroke_width: 0.0,
            corner_radii: UiCornerRadiiState::default(),
            outer_shadow: UiDepthEffectState::none(),
            inner_shadow: UiDepthEffectState::none(),
            outer_highlight: UiDepthEffectState::none(),
            inner_highlight: UiDepthEffectState::none(),
        });
        let second_ptrs: Vec<*const epaint::ClippedPrimitive> = renderer
            .prepare_paint([800.0, 600.0])
            .primitives
            .iter()
            .map(std::sync::Arc::as_ptr)
            .collect();

        assert!(
            first_ptrs.iter().any(|ptr| second_ptrs.contains(ptr)),
            "label primitives were re-tessellated instead of reused from cache"
        );
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
            clip_rect: [0.0, 0.0, 800.0, 600.0],
            fill: [0.1, 0.2, 0.3, 1.0],
            fill_kind: UiFillKindState::Solid,
            gradient: UiLinearGradientState::none(),
            stroke: [0.0, 0.0, 0.0, 0.0],
            stroke_width: 0.0,
            corner_radii: UiCornerRadiiState::default(),
            outer_shadow: UiDepthEffectState::none(),
            inner_shadow: UiDepthEffectState::none(),
            outer_highlight: UiDepthEffectState::none(),
            inner_highlight: UiDepthEffectState::none(),
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
            clip_rect: [0.0, 0.0, 800.0, 600.0],
            fill: [0.1, 0.2, 0.3, 1.0],
            fill_kind: UiFillKindState::Solid,
            gradient: UiLinearGradientState::none(),
            stroke: [0.0, 0.0, 0.0, 0.0],
            stroke_width: 0.0,
            corner_radii: UiCornerRadiiState::default(),
            outer_shadow: UiDepthEffectState::none(),
            inner_shadow: UiDepthEffectState::none(),
            outer_highlight: UiDepthEffectState::none(),
            inner_highlight: UiDepthEffectState::none(),
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
