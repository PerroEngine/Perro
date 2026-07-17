use super::renderer::{
    UiCheckboxDraw, UiColorWheelDraw, UiDraw, UiImageDraw, UiLabelDraw, UiNineSliceDraw,
    UiPanelDraw, UiProgressBarDraw, UiShapeDraw, UiTextEditDraw,
};
use ab_glyph::Font as _;
use ahash::AHashMap;
use epaint::{
    AlphaFromCoverage, CircleShape, ClippedPrimitive, ClippedShape, Color32, CornerRadius,
    FontFamily, FontId, Fonts, Galley, Mesh, Primitive, Rect, RectShape, Shape, Stroke, StrokeKind,
    TessellationOptions, Tessellator, TextureAtlas, TextureId, Vertex,
    emath::{Align, Rot2},
    pos2,
    text::{FontData, FontDefinitions, LayoutJob},
    textures::TexturesDelta,
    vec2,
};
use perro_ids::NodeID;
use perro_render_bridge::{
    UiColorPickerMode, UiCornerRadiiState, UiDepthEffectState, UiFillKindState, UiImageScaleState,
    UiLinearGradientState, UiRectState, UiShapeKind, UiTextAlignState,
};
use perro_structs::Color;
use perro_ui::{UiFont, UiSystemFont};
use std::sync::Arc;

const UI_RASTER_SCALE: f32 = 3.0;
const UI_FONT_ATLAS_SIZE: usize = 4096;
const UI_HARFBUZZ_ATLAS_SIZE: usize = 4096;
const UI_HARFBUZZ_TEXTURE_ID: TextureId = TextureId::Managed(1);
const UI_SYSTEM_FONT_PREFIX: &str = "perro-system";
const UI_CYRILLIC_FONT_FAMILY: &str = "perro-cyrillic";
const UI_ARABIC_FONT_FAMILY: &str = "perro-arabic";
const UI_HEBREW_FONT_FAMILY: &str = "perro-hebrew";
const UI_INDIC_FONT_FAMILY: &str = "perro-indic";
const UI_THAI_FONT_FAMILY: &str = "perro-thai";
const UI_SE_ASIAN_FONT_FAMILY: &str = "perro-se-asian";
const UI_JAPANESE_FONT_FAMILY: &str = "perro-japanese";
const UI_CHINESE_FONT_FAMILY: &str = "perro-chinese";
const UI_KOREAN_FONT_FAMILY: &str = "perro-korean";

const UI_CYRILLIC_FONT_FAMILIES: &[&str] = &[
    "Segoe UI",
    "Arial",
    "Helvetica",
    "Noto Sans",
    "DejaVu Sans",
    "Liberation Sans",
];

const UI_ARABIC_FONT_FAMILIES: &[&str] = &[
    "Segoe UI",
    "Segoe UI Historic",
    "Arial",
    "Tahoma",
    "Noto Sans Arabic",
    "Noto Naskh Arabic",
    "DejaVu Sans",
    "Liberation Sans",
];

const UI_HEBREW_FONT_FAMILIES: &[&str] = &[
    "Segoe UI",
    "Arial",
    "Tahoma",
    "Noto Sans Hebrew",
    "DejaVu Sans",
    "Liberation Sans",
];

const UI_INDIC_FONT_FAMILIES: &[&str] = &[
    "Nirmala UI",
    "Mangal",
    "Noto Sans Devanagari",
    "Noto Sans Bengali",
    "Noto Sans Tamil",
    "Noto Sans Telugu",
    "Noto Sans Kannada",
    "Noto Sans Malayalam",
];

const UI_THAI_FONT_FAMILIES: &[&str] = &[
    "Leelawadee UI",
    "Tahoma",
    "Arial",
    "Noto Sans Thai",
    "DejaVu Sans",
];

const UI_SE_ASIAN_FONT_FAMILIES: &[&str] = &[
    "Leelawadee UI",
    "Khmer UI",
    "Myanmar Text",
    "Lao UI",
    "Noto Sans Khmer",
    "Noto Sans Myanmar",
    "Noto Sans Lao",
    "Noto Sans Tibetan",
    "Noto Sans",
];

const UI_JAPANESE_FONT_FAMILIES: &[&str] = &[
    "Hiragino Maru Gothic ProN",
    "M PLUS Rounded 1c",
    "Kosugi Maru",
    "Meiryo",
    "Yu Gothic",
    "Yu Gothic UI",
    "Hiragino Sans",
    "Hiragino Kaku Gothic ProN",
    "Noto Sans CJK JP",
    "Noto Sans JP",
    "Source Han Sans JP",
    "TakaoGothic",
    "IPAGothic",
];

const UI_CHINESE_FONT_FAMILIES: &[&str] = &[
    "Microsoft YaHei",
    "Microsoft JhengHei",
    "SimSun",
    "PingFang SC",
    "PingFang TC",
    "Noto Sans CJK SC",
    "Noto Sans CJK TC",
    "Noto Sans SC",
    "Noto Sans TC",
    "Source Han Sans SC",
    "Source Han Sans TC",
    "WenQuanYi Micro Hei",
];

const UI_KOREAN_FONT_FAMILIES: &[&str] = &[
    "Malgun Gothic",
    "Apple SD Gothic Neo",
    "Noto Sans CJK KR",
    "Noto Sans KR",
    "Source Han Sans KR",
    "NanumGothic",
];

pub struct UiPaintFrame<'a> {
    pub primitives: &'a [Arc<ClippedPrimitive>],
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

/// Per-node cached tessellation output. Reused across rebuilds when the node's
/// own draw data + viewport are unchanged, so only mutated nodes re-tessellate.
/// Text nodes cache too: their glyph UVs stay valid for as long as the font
/// atlas keeps its size, and the whole cache drops whenever
/// `font_image_size()` differs from the size the cache was built against
/// (growth doubles the height; epaint's fill>0.8 recreation resets it, so
/// either event changes the size).
///
/// For a world-projected label (`projected_quad`) the cache keeps the
/// UNPROJECTED tessellation with the quad stripped from `draw`; a camera move
/// then only re-runs the cheap projection instead of re-layout + re-tessellate.
///
/// Primitives are shared behind `Arc` so reusing an unchanged node is a
/// refcount bump instead of a deep clone of its vertex/index meshes.
struct CachedNode {
    draw: UiDraw,
    viewport: [f32; 2],
    primitives: Vec<Arc<ClippedPrimitive>>,
}

/// Per-node staging entry for the two-phase rebuild: shapes are staged for
/// every node first, then tessellated together once the font atlas is final.
enum NodeTess {
    Cached,
    /// Cached unprojected label tessellation; only the projection quad
    /// changed (or matched), so phase three re-projects a copy.
    CachedProjected {
        quad: [[f32; 4]; 4],
    },
    Staged {
        shapes: Vec<ClippedShape>,
        rotations: Vec<(f32, epaint::Pos2)>,
    },
}

/// Draw key used for projected-label cache entries: identical content with
/// the quad stripped, so camera motion alone still hits the cache.
fn strip_projected_quad(draw: &UiDraw) -> Option<UiDraw> {
    if let UiDraw::Label(label) = draw
        && label.projected_quad.is_some()
    {
        let mut stripped = label.clone();
        stripped.projected_quad = None;
        Some(UiDraw::Label(stripped))
    } else {
        None
    }
}

fn deep_clone_primitives(primitives: &[Arc<ClippedPrimitive>]) -> Vec<ClippedPrimitive> {
    primitives
        .iter()
        .map(|primitive| ClippedPrimitive {
            clip_rect: primitive.clip_rect,
            primitive: primitive.primitive.clone(),
        })
        .collect()
}

fn retain_nonempty_meshes(primitives: &mut Vec<ClippedPrimitive>) {
    primitives.retain(|primitive| match &primitive.primitive {
        Primitive::Mesh(mesh) => !mesh.vertices.is_empty() && !mesh.indices.is_empty(),
        Primitive::Callback(_) => false,
    });
}

pub(crate) struct EpaintUiPainter {
    fonts: Fonts,
    font_definitions: FontDefinitions,
    harfbuzz_atlas: HarfBuzzAtlas,
    shapes: Vec<ClippedShape>,
    shape_rotations: Vec<(f32, epaint::Pos2)>,
    primitives: Vec<Arc<ClippedPrimitive>>,
    // Previous generation of `primitives`, kept alive for one rebuild so fresh
    // tessellations cannot reuse a freed Arc address while the GPU's
    // pointer-identity mesh signature still references it (ABA guard).
    prev_primitives: Vec<Arc<ClippedPrimitive>>,
    node_cache: AHashMap<NodeID, CachedNode>,
    // Cached z-sorted draw order + the (node, z_index) signature it was built
    // from. Reused when the structure (id set / z-order) is unchanged, so a
    // content-only edit skips the re-sort.
    ordered_nodes: Vec<NodeID>,
    order_signature: Vec<(NodeID, i32)>,
    // Font-atlas size the node cache was tessellated against; any size change
    // (growth or epaint's fill-triggered recreation) invalidates every cached
    // primitive's glyph UVs.
    node_cache_atlas_size: [usize; 2],
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
        let font_definitions = default_ui_font_definitions();
        Self {
            fonts: Fonts::new(
                UI_FONT_ATLAS_SIZE,
                AlphaFromCoverage::default(),
                font_definitions.clone(),
            ),
            font_definitions,
            harfbuzz_atlas: HarfBuzzAtlas::new(),
            shapes: Vec::new(),
            shape_rotations: Vec::new(),
            primitives: Vec::new(),
            prev_primitives: Vec::new(),
            node_cache: AHashMap::new(),
            ordered_nodes: Vec::new(),
            order_signature: Vec::new(),
            node_cache_atlas_size: [0, 0],
            textures_delta: TexturesDelta::default(),
            last_viewport: [0.0, 0.0],
            paint_revision: u64::MAX,
        }
    }

    pub(crate) fn register_resource_font(
        &mut self,
        path: &str,
        lookup: Option<crate::StaticFontLookup>,
    ) {
        let family = resource_font_family(path);
        if self.font_definitions.families.contains_key(&family) {
            return;
        }
        append_default_family_fallbacks(
            &mut self.font_definitions,
            family.clone(),
            FontFamily::Proportional,
        );
        let hash = perro_ids::parse_hashed_source_uri(path)
            .unwrap_or_else(|| perro_ids::string_to_u64(path));
        let static_bytes = lookup
            .map(|lookup| lookup(hash))
            .filter(|bytes| !bytes.is_empty());
        let owned;
        let bytes = if let Some(bytes) = static_bytes {
            owned = perro_io::asset_io::decode_static_font(bytes).unwrap_or_default();
            owned.as_slice()
        } else {
            let Ok(bytes) = perro_io::asset_io::load_asset(path) else {
                self.fonts = Fonts::new(
                    UI_FONT_ATLAS_SIZE,
                    AlphaFromCoverage::default(),
                    self.font_definitions.clone(),
                );
                self.paint_revision = u64::MAX;
                return;
            };
            owned = bytes;
            &owned
        };
        let key = format!("perro-resource-{hash}");
        self.font_definitions
            .font_data
            .insert(key.clone(), Arc::new(FontData::from_owned(bytes.to_vec())));
        self.font_definitions
            .families
            .entry(family)
            .or_default()
            .insert(0, key);
        self.fonts = Fonts::new(
            UI_FONT_ATLAS_SIZE,
            AlphaFromCoverage::default(),
            self.font_definitions.clone(),
        );
        self.harfbuzz_atlas.invalidate_runs();
        self.paint_revision = u64::MAX;
    }

    /// Push a single node's shapes plus their per-shape rotation entries.
    fn push_node_shapes(&mut self, draw: &UiDraw, viewport: [f32; 2]) {
        let shape_start = self.shapes.len();
        match draw {
            UiDraw::Panel(panel) => push_panel_shape(panel, viewport, &mut self.shapes),
            UiDraw::ProgressBar(progress) => {
                push_progress_bar_shapes(progress, viewport, &mut self.shapes)
            }
            UiDraw::Shape(shape) => push_ui_shape(shape, viewport, &mut self.shapes),
            UiDraw::ColorWheel(wheel) => push_color_wheel_shape(wheel, viewport, &mut self.shapes),
            UiDraw::Button(button) => push_panel_shape(&button.panel, viewport, &mut self.shapes),
            UiDraw::Checkbox(checkbox) => {
                push_checkbox_shapes(checkbox, viewport, &mut self.shapes)
            }
            UiDraw::Image(image) => push_image_shape(image, viewport, &mut self.shapes),
            UiDraw::NineSlice(image) => push_nine_slice_shapes(image, viewport, &mut self.shapes),
            UiDraw::Label(label) => push_label_shape(
                label,
                viewport,
                &self.font_definitions,
                &mut self.harfbuzz_atlas,
                &mut self.fonts,
                &mut self.shapes,
            ),
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

    /// True when the cached sorted order is still valid: same node set and same
    /// per-node z_index (insertion order tiebreak is stable under id equality).
    fn order_matches(&self, nodes: &AHashMap<NodeID, UiDraw>) -> bool {
        if self.order_signature.len() != nodes.len() {
            return false;
        }
        self.order_signature.iter().all(|(node, z)| {
            nodes
                .get(node)
                .is_some_and(|draw| ui_rect(draw).z_index == *z)
        })
    }

    fn rebuild_primitives(
        &mut self,
        nodes: &AHashMap<NodeID, UiDraw>,
        revision: u64,
        viewport: [f32; 2],
    ) {
        self.fonts
            .begin_pass(UI_FONT_ATLAS_SIZE, AlphaFromCoverage::default());
        self.shapes.clear();
        self.shape_rotations.clear();
        // Hold the previous generation's Arcs alive through this rebuild: the
        // GPU mesh signature keys on Arc pointer identity, so freshly
        // tessellated primitives must not reuse a just-freed allocation (ABA)
        // while the prior signature is still the comparison baseline.
        std::mem::swap(&mut self.prev_primitives, &mut self.primitives);
        self.primitives.clear();

        // Only re-sort when the structure (id set / z-order) changed. A pure
        // content edit (e.g. color, text) leaves the order signature intact.
        if !self.order_matches(nodes) {
            self.ordered_nodes.clear();
            self.ordered_nodes.extend(nodes.keys().copied());
            self.ordered_nodes.sort_unstable_by(|a, b| {
                let za = nodes.get(a).map(|d| ui_rect(d).z_index).unwrap_or(0);
                let zb = nodes.get(b).map(|d| ui_rect(d).z_index).unwrap_or(0);
                za.cmp(&zb).then_with(|| a.as_u64().cmp(&b.as_u64()))
            });
            self.order_signature.clear();
            self.order_signature
                .extend(self.ordered_nodes.iter().map(|node| {
                    (
                        *node,
                        nodes.get(node).map(|d| ui_rect(d).z_index).unwrap_or(0),
                    )
                }));
        }

        // Reuse per-node cached primitives whose draw data + viewport are
        // unchanged; only re-tessellate mutated nodes.
        //
        // Two phases: stage every node's shapes first (text layout allocates
        // glyphs into the shared font atlas as it goes), then tessellate once
        // the atlas has settled. Tessellating per node mid-loop bakes glyph
        // UVs against the atlas size at that moment; a later node adding new
        // glyphs (e.g. CJK) grows the atlas and leaves earlier nodes sampling
        // wrong texels for a frame.
        let ordered_nodes = std::mem::take(&mut self.ordered_nodes);
        let atlas_size_before = self.fonts.font_image_size();
        // begin_pass may have recreated the atlas (fill > threshold), and past
        // rebuilds may have grown it: cached glyph UVs are only valid against
        // the exact size they were tessellated for.
        if atlas_size_before != self.node_cache_atlas_size {
            self.node_cache.clear();
        }
        let mut staged: Vec<(NodeID, NodeTess)> = Vec::with_capacity(ordered_nodes.len());
        for node in &ordered_nodes {
            let Some(draw) = nodes.get(node) else {
                continue;
            };
            if let Some(cached) = self.node_cache.get(node)
                && cached.viewport == viewport
            {
                if cached.draw == *draw {
                    staged.push((*node, NodeTess::Cached));
                    continue;
                }
                if let Some(stripped) = strip_projected_quad(draw)
                    && cached.draw == stripped
                    && let UiDraw::Label(label) = draw
                    && let Some(quad) = label.projected_quad
                {
                    staged.push((*node, NodeTess::CachedProjected { quad }));
                    continue;
                }
            }
            self.push_node_shapes(draw, viewport);
            staged.push((
                *node,
                NodeTess::Staged {
                    shapes: std::mem::take(&mut self.shapes),
                    rotations: std::mem::take(&mut self.shape_rotations),
                },
            ));
        }
        // Atlas resized during layout: cached primitives were tessellated
        // against the old size, so their UVs are stale. Re-stage everything.
        if self.fonts.font_image_size() != atlas_size_before {
            self.node_cache.clear();
            for (node, entry) in &mut staged {
                if matches!(entry, NodeTess::Staged { .. }) {
                    continue;
                }
                let Some(draw) = nodes.get(node) else {
                    continue;
                };
                self.push_node_shapes(draw, viewport);
                *entry = NodeTess::Staged {
                    shapes: std::mem::take(&mut self.shapes),
                    rotations: std::mem::take(&mut self.shape_rotations),
                };
            }
        }
        let mut tessellator = Tessellator::new(
            UI_RASTER_SCALE,
            TessellationOptions::default(),
            self.fonts.font_image_size(),
            self.fonts.texture_atlas().prepared_discs(),
        );
        for (node, entry) in staged {
            match entry {
                NodeTess::Cached => {
                    if let Some(cached) = self.node_cache.get(&node) {
                        self.primitives.extend(cached.primitives.iter().cloned());
                    }
                }
                NodeTess::CachedProjected { quad } => {
                    let Some(cached) = self.node_cache.get(&node) else {
                        continue;
                    };
                    let Some(UiDraw::Label(label)) = nodes.get(&node) else {
                        continue;
                    };
                    // Projection mutates vertices, so the frame gets a deep
                    // copy; the cached unprojected meshes stay pristine.
                    let mut frame = deep_clone_primitives(&cached.primitives);
                    project_label_primitives(&mut frame, label.rect, quad, viewport);
                    retain_nonempty_meshes(&mut frame);
                    self.primitives.extend(frame.into_iter().map(Arc::new));
                }
                NodeTess::Staged { shapes, rotations } => {
                    let mut tessellated = tessellator.tessellate_shapes(shapes);
                    rotate_primitives(&mut tessellated, &rotations);
                    let projected_label = match nodes.get(&node) {
                        Some(UiDraw::Label(label)) => {
                            label.projected_quad.map(|quad| (label.rect, quad))
                        }
                        _ => None,
                    };
                    if let Some((rect, quad)) = projected_label {
                        retain_nonempty_meshes(&mut tessellated);
                        // Cache the unprojected tessellation (quad stripped
                        // from the key) so camera motion re-projects instead
                        // of re-tessellating.
                        let unprojected: Vec<Arc<ClippedPrimitive>> =
                            tessellated.into_iter().map(Arc::new).collect();
                        let mut frame = deep_clone_primitives(&unprojected);
                        project_label_primitives(&mut frame, rect, quad, viewport);
                        retain_nonempty_meshes(&mut frame);
                        self.primitives.extend(frame.into_iter().map(Arc::new));
                        if let Some(draw) = nodes.get(&node)
                            && let Some(stripped) = strip_projected_quad(draw)
                        {
                            self.node_cache.insert(
                                node,
                                CachedNode {
                                    draw: stripped,
                                    viewport,
                                    primitives: unprojected,
                                },
                            );
                        }
                        continue;
                    }
                    retain_nonempty_meshes(&mut tessellated);
                    // Wrap each primitive in an Arc so the shared copy stored in
                    // the cache and the copy handed to the frame refer to the
                    // same heap mesh; downstream reuse is a refcount bump.
                    let node_primitives: Vec<Arc<ClippedPrimitive>> =
                        tessellated.into_iter().map(Arc::new).collect();
                    self.primitives.extend(node_primitives.iter().cloned());
                    if let Some(draw) = nodes.get(&node) {
                        self.node_cache.insert(
                            node,
                            CachedNode {
                                draw: draw.clone(),
                                viewport,
                                primitives: node_primitives,
                            },
                        );
                    }
                }
            }
        }
        self.ordered_nodes = ordered_nodes;
        self.node_cache_atlas_size = self.fonts.font_image_size();
        // Evict cache entries for nodes no longer present.
        if self.node_cache.len() > nodes.len() {
            self.node_cache.retain(|node, _| nodes.contains_key(node));
        }

        self.textures_delta.clear();
        if let Some(delta) = self.fonts.font_image_delta() {
            self.textures_delta
                .set
                .push((epaint::TextureId::default(), delta));
        }
        if let Some(delta) = self.harfbuzz_atlas.take_delta() {
            self.textures_delta
                .set
                .push((UI_HARFBUZZ_TEXTURE_ID, delta));
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

mod fonts;
use fonts::*;
mod shapes;
use shapes::*;
mod projection;
use projection::*;
mod geometry;
use geometry::*;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn nine_slice_maps_pixel_margins_into_full_uv_region() {
        let image = UiNineSliceDraw {
            rect: UiRectState {
                center: [100.0, 50.0],
                size: [200.0, 100.0],
                pivot: [0.5, 0.5],
                rotation_radians: 0.0,
                z_index: 0,
            },
            clip_rect: [0.0, 0.0, 800.0, 600.0],
            texture: perro_ids::TextureID::from_parts(1, 1),
            tint: perro_structs::Color::WHITE,
            uv_min: [0.0, 0.0],
            uv_max: [1.0, 1.0],
            margins: [10.0, 10.0, 10.0, 10.0],
            texture_size: [250, 125],
        };
        let mut shapes = Vec::new();

        push_nine_slice_shapes(&image, [800.0, 600.0], &mut shapes);

        let Shape::Mesh(mesh) = &shapes[0].shape else {
            panic!("expected nine-slice mesh");
        };
        assert_eq!(mesh.vertices.len(), 36);
        assert!(
            mesh.vertices
                .iter()
                .any(|vertex| vertex.uv == pos2(0.04, 0.08))
        );
        assert!(
            mesh.vertices
                .iter()
                .any(|vertex| vertex.uv == pos2(0.96, 0.92))
        );
        assert!(
            mesh.vertices
                .iter()
                .any(|vertex| vertex.uv == pos2(1.0, 1.0))
        );

        let mut tiled = image.clone();
        tiled.rect.size = [600.0, 300.0];
        let mut tiled_shapes = Vec::new();
        push_nine_slice_shapes(&tiled, [800.0, 600.0], &mut tiled_shapes);
        let Shape::Mesh(tiled_mesh) = &tiled_shapes[0].shape else {
            panic!("expected tiled nine-slice mesh");
        };
        assert_eq!(tiled_mesh.vertices.len(), 100);
    }

    #[test]
    fn cover_image_crops_to_own_bounds() {
        let image = UiImageDraw {
            rect: UiRectState {
                center: [0.0, 0.0],
                size: [200.0, 100.0],
                pivot: [0.5, 0.5],
                rotation_radians: 0.0,
                z_index: 0,
            },
            clip_rect: [0.0, 0.0, 800.0, 600.0],
            texture: perro_ids::TextureID::from_parts(1, 1),
            tint: perro_structs::Color::WHITE,
            uv_min: [0.0, 0.0],
            uv_max: [1.0, 1.0],
            scale_mode: UiImageScaleState::Cover,
            h_align: UiTextAlignState::Center,
            v_align: UiTextAlignState::Center,
            aspect_ratio: 0.5,
            corner_radii: UiCornerRadiiState::default(),
        };
        let mut shapes = Vec::new();

        push_image_shape(&image, [800.0, 600.0], &mut shapes);

        assert_eq!(shapes.len(), 1);
        assert_eq!(
            shapes[0].clip_rect,
            Rect::from_min_max(pos2(0.0, 0.0), pos2(800.0, 600.0))
        );
        let Shape::Mesh(mesh) = &shapes[0].shape else {
            panic!("expected image mesh");
        };
        assert!(mesh.vertices.iter().all(|vertex| {
            (300.0..=500.0).contains(&vertex.pos.x) && (250.0..=350.0).contains(&vertex.pos.y)
        }));
        let min_v = mesh
            .vertices
            .iter()
            .map(|vertex| vertex.uv.y)
            .fold(f32::INFINITY, f32::min);
        let max_v = mesh
            .vertices
            .iter()
            .map(|vertex| vertex.uv.y)
            .fold(f32::NEG_INFINITY, f32::max);
        assert!((min_v - 0.375).abs() < 1.0e-6);
        assert!((max_v - 0.625).abs() < 1.0e-6);
    }

    #[test]
    fn panel_corner_radius_is_size_ratio() {
        let panel = UiPanelDraw {
            rect: UiRectState {
                center: [0.0, 0.0],
                size: [100.0, 50.0],
                pivot: [0.5, 0.5],
                rotation_radians: 0.0,
                z_index: 0,
            },
            clip_rect: [0.0, 0.0, 800.0, 600.0],
            fill: perro_structs::Color::BLACK,
            fill_kind: UiFillKindState::Solid,
            gradient: UiLinearGradientState::none(),
            stroke: perro_structs::Color::TRANSPARENT,
            stroke_width: 0.0,
            corner_radii: UiCornerRadiiState {
                tl: 0.5,
                tr: 0.5,
                br: 0.5,
                bl: 0.5,
            },
            outer_shadow: UiDepthEffectState::none(),
            inner_shadow: UiDepthEffectState::none(),
            outer_highlight: UiDepthEffectState::none(),
            inner_highlight: UiDepthEffectState::none(),
        };
        let rect = Rect::from_center_size(pos2(0.0, 0.0), vec2(100.0, 50.0));
        assert_eq!(resolve_corner_radii(&panel, rect).tl, 12.5);
    }

    #[test]
    fn panel_corner_radius_clamps_to_full_round() {
        let panel = UiPanelDraw {
            rect: UiRectState {
                center: [0.0, 0.0],
                size: [100.0, 50.0],
                pivot: [0.5, 0.5],
                rotation_radians: 0.0,
                z_index: 0,
            },
            clip_rect: [0.0, 0.0, 800.0, 600.0],
            fill: perro_structs::Color::BLACK,
            fill_kind: UiFillKindState::Solid,
            gradient: UiLinearGradientState::none(),
            stroke: perro_structs::Color::TRANSPARENT,
            stroke_width: 0.0,
            corner_radii: UiCornerRadiiState {
                tl: 2.0,
                tr: 2.0,
                br: 2.0,
                bl: 2.0,
            },
            outer_shadow: UiDepthEffectState::none(),
            inner_shadow: UiDepthEffectState::none(),
            outer_highlight: UiDepthEffectState::none(),
            inner_highlight: UiDepthEffectState::none(),
        };
        let rect = Rect::from_center_size(pos2(0.0, 0.0), vec2(100.0, 50.0));
        assert_eq!(resolve_corner_radii(&panel, rect).tl, 25.0);
    }

    #[test]
    fn effect_size_is_rect_relative_multiplier() {
        let rect = Rect::from_min_size(pos2(0.0, 0.0), vec2(100.0, 50.0));
        let mut effect = UiDepthEffectState::none();

        effect.size = 1.0;
        assert_eq!(effect_size_expand(rect, effect), 0.0);

        effect.size = 2.0;
        assert_eq!(effect_size_expand(rect, effect), 25.0);

        effect.size = 0.5;
        assert_eq!(effect_size_expand(rect, effect), -12.5);
    }

    #[test]
    fn gradient_panel_pushes_mesh_shape() {
        let panel = UiPanelDraw {
            rect: UiRectState {
                center: [0.0, 0.0],
                size: [140.0, 60.0],
                pivot: [0.5, 0.5],
                rotation_radians: 0.0,
                z_index: 0,
            },
            clip_rect: [0.0, 0.0, 800.0, 600.0],
            fill: perro_structs::Color::WHITE,
            fill_kind: UiFillKindState::Linear,
            gradient: UiLinearGradientState {
                start_color: perro_structs::Color::WHITE,
                end_color: perro_structs::Color::BLACK,
                vector: [0.0, -1.0],
            },
            stroke: perro_structs::Color::TRANSPARENT,
            stroke_width: 0.0,
            corner_radii: UiCornerRadiiState {
                tl: 0.3,
                tr: 0.3,
                br: 0.3,
                bl: 0.3,
            },
            outer_shadow: UiDepthEffectState::none(),
            inner_shadow: UiDepthEffectState::none(),
            outer_highlight: UiDepthEffectState::none(),
            inner_highlight: UiDepthEffectState::none(),
        };
        let mut shapes = Vec::new();
        push_panel_shape(&panel, [800.0, 600.0], &mut shapes);
        assert!(
            shapes
                .iter()
                .any(|shape| matches!(shape.shape, Shape::Mesh(_)))
        );
    }

    #[test]
    fn label_text_clip_uses_parent_clip_not_own_rect() {
        let mut fonts = Fonts::new(
            UI_FONT_ATLAS_SIZE,
            AlphaFromCoverage::default(),
            default_ui_font_definitions(),
        );
        fonts.begin_pass(UI_FONT_ATLAS_SIZE, AlphaFromCoverage::default());
        let mut shapes = Vec::new();
        push_text_shape(
            TextShapeInput {
                rect: UiRectState {
                    center: [0.0, 0.0],
                    size: [80.0, 20.0],
                    pivot: [0.5, 0.5],
                    rotation_radians: 0.0,
                    z_index: 0,
                },
                viewport: [800.0, 600.0],
                clip_rect: Rect::from_min_max(pos2(0.0, 0.0), pos2(800.0, 600.0)),
                text: "alpha beta gamma delta epsilon",
                font_size: 24.0,
                font: &UiFont::Default,
                wrap_width: None,
                color: perro_structs::Color::WHITE,
                h_align: UiTextAlignState::Start,
                v_align: UiTextAlignState::Start,
                fit_content: false,
            },
            &mut fonts,
            &mut shapes,
        );

        assert_eq!(shapes.len(), 1);
        assert_eq!(
            shapes[0].clip_rect,
            Rect::from_min_max(pos2(0.0, 0.0), pos2(800.0, 600.0))
        );
    }

    #[test]
    fn label_text_h_align_sets_paragraph_align_and_anchor() {
        let mut fonts = Fonts::new(
            UI_FONT_ATLAS_SIZE,
            AlphaFromCoverage::default(),
            default_ui_font_definitions(),
        );
        fonts.begin_pass(UI_FONT_ATLAS_SIZE, AlphaFromCoverage::default());
        let rect = UiRectState {
            center: [0.0, 0.0],
            size: [100.0, 20.0],
            pivot: [0.5, 0.5],
            rotation_radians: 0.0,
            z_index: 0,
        };
        let cases = [
            (UiTextAlignState::Start, Align::LEFT, 350.0),
            (UiTextAlignState::Center, Align::Center, 400.0),
            (UiTextAlignState::End, Align::RIGHT, 450.0),
        ];

        for (h_align, expected_align, expected_x) in cases {
            let mut shapes = Vec::new();
            push_text_shape(
                TextShapeInput {
                    rect,
                    viewport: [800.0, 600.0],
                    clip_rect: Rect::from_min_max(pos2(0.0, 0.0), pos2(800.0, 600.0)),
                    text: "seed\nfood",
                    font_size: 16.0,
                    font: &UiFont::Default,
                    wrap_width: None,
                    color: perro_structs::Color::WHITE,
                    h_align,
                    v_align: UiTextAlignState::Start,
                    fit_content: false,
                },
                &mut fonts,
                &mut shapes,
            );

            let Shape::Text(text_shape) = &shapes[0].shape else {
                panic!("expected text shape");
            };
            assert_eq!(text_shape.galley.job.halign, expected_align);
            assert!((text_shape.pos.x - expected_x).abs() < 1.0e-3);
        }
    }

    #[test]
    fn harfbuzz_text_shape_uses_managed_font_texture() {
        let definitions = default_ui_font_definitions();
        let mut atlas = HarfBuzzAtlas::new();
        let mut shapes = Vec::new();

        let built = push_harfbuzz_text_shape(
            TextShapeInput {
                rect: UiRectState {
                    center: [0.0, 0.0],
                    size: [200.0, 60.0],
                    pivot: [0.5, 0.5],
                    rotation_radians: 0.0,
                    z_index: 0,
                },
                viewport: [800.0, 600.0],
                clip_rect: Rect::from_min_max(pos2(0.0, 0.0), pos2(800.0, 600.0)),
                text: "Perro",
                font_size: 24.0,
                font: &UiFont::Default,
                wrap_width: None,
                color: perro_structs::Color::WHITE,
                h_align: UiTextAlignState::Center,
                v_align: UiTextAlignState::Center,
                fit_content: false,
            },
            &definitions,
            &mut atlas,
            &mut shapes,
        );

        assert!(built);
        let Shape::Mesh(mesh) = &shapes[0].shape else {
            panic!("expected harfbuzz mesh");
        };
        assert_eq!(mesh.texture_id, UI_HARFBUZZ_TEXTURE_ID);
        assert!(!mesh.vertices.is_empty());
        assert!(atlas.take_delta().is_some());
    }

    #[test]
    fn harfbuzz_shapes_default_font_to_glyph_run() {
        let definitions = FontDefinitions::default();
        let font = font_sources_for_family(&definitions, FontFamily::Proportional)
            .into_iter()
            .next()
            .unwrap();

        let run = shape_text_with_harfbuzz(&font, "Perro").unwrap();

        assert!(!run.glyphs.is_empty());
        assert!(run.glyphs.iter().all(|glyph| glyph.glyph_id > 0));
    }

    #[test]
    fn font_definitions_keep_default_fonts() {
        let fonts = default_ui_font_definitions();

        let proportional = fonts.families.get(&FontFamily::Proportional).unwrap();
        let monospace = fonts.families.get(&FontFamily::Monospace).unwrap();

        assert!(proportional.iter().any(|name| name == "Ubuntu-Light"));
        assert!(monospace.iter().any(|name| name == "Hack"));
    }

    #[test]
    fn append_font_fallback_dedupes_family_entries() {
        let mut fonts = FontDefinitions::default();

        append_font_fallback(&mut fonts, FontFamily::Proportional, "perro-test-font");
        append_font_fallback(&mut fonts, FontFamily::Proportional, "perro-test-font");

        let count = fonts
            .families
            .get(&FontFamily::Proportional)
            .unwrap()
            .iter()
            .filter(|name| name.as_str() == "perro-test-font")
            .count();
        assert_eq!(count, 1);
    }

    #[test]
    fn text_font_family_picks_script_family() {
        assert_eq!(
            text_font_family("加入", FontFamily::Proportional),
            named_font_family(UI_CHINESE_FONT_FAMILY)
        );
        assert_eq!(
            text_font_family("スタート", FontFamily::Proportional),
            named_font_family(UI_JAPANESE_FONT_FAMILY)
        );
        assert_eq!(
            text_font_family("시작", FontFamily::Proportional),
            named_font_family(UI_KOREAN_FONT_FAMILY)
        );
        assert_eq!(
            text_font_family("Играть", FontFamily::Proportional),
            named_font_family(UI_CYRILLIC_FONT_FAMILY)
        );
    }

    #[test]
    fn text_font_family_keeps_latin_default() {
        assert_eq!(
            text_font_family("Play", FontFamily::Proportional),
            FontFamily::Proportional
        );
    }
}
