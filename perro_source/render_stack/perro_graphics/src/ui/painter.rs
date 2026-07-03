use super::renderer::{
    UiCheckboxDraw, UiColorWheelDraw, UiDraw, UiImageDraw, UiLabelDraw, UiNineSliceDraw,
    UiPanelDraw, UiShapeDraw, UiTextEditDraw,
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
    UiCornerRadiiState, UiDepthEffectState, UiFillKindState, UiImageScaleState,
    UiLinearGradientState, UiRectState, UiShapeKind, UiTextAlignState,
};
use std::sync::Arc;

const UI_RASTER_SCALE: f32 = 2.0;
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

/// Per-node cached tessellation output. Reused across rebuilds when the node's
/// own draw data + viewport are unchanged, so only mutated nodes re-tessellate.
/// Text nodes (Label/TextEdit) are never cached because they depend on the
/// shared font atlas pass, which resets each rebuild.
struct CachedNode {
    draw: UiDraw,
    viewport: [f32; 2],
    primitives: Vec<ClippedPrimitive>,
}

/// Per-node staging entry for the two-phase rebuild: shapes are staged for
/// every node first, then tessellated together once the font atlas is final.
enum NodeTess {
    Cached,
    Staged {
        shapes: Vec<ClippedShape>,
        rotations: Vec<(f32, epaint::Pos2)>,
        is_text: bool,
    },
}

pub(crate) struct EpaintUiPainter {
    fonts: Fonts,
    font_definitions: FontDefinitions,
    harfbuzz_atlas: HarfBuzzAtlas,
    shapes: Vec<ClippedShape>,
    shape_rotations: Vec<(f32, epaint::Pos2)>,
    primitives: Vec<ClippedPrimitive>,
    node_cache: AHashMap<NodeID, CachedNode>,
    // Cached z-sorted draw order + the (node, z_index) signature it was built
    // from. Reused when the structure (id set / z-order) is unchanged, so a
    // content-only edit skips the re-sort.
    ordered_nodes: Vec<NodeID>,
    order_signature: Vec<(NodeID, i32)>,
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
            node_cache: AHashMap::new(),
            ordered_nodes: Vec::new(),
            order_signature: Vec::new(),
            textures_delta: TexturesDelta::default(),
            last_viewport: [0.0, 0.0],
            paint_revision: u64::MAX,
        }
    }

    /// Push a single node's shapes plus their per-shape rotation entries.
    /// Returns true if the node's tessellation touches the shared font atlas
    /// (text), which forbids caching it across rebuilds.
    fn push_node_shapes(&mut self, draw: &UiDraw, viewport: [f32; 2]) -> bool {
        let shape_start = self.shapes.len();
        let is_text = matches!(draw, UiDraw::Label(_) | UiDraw::TextEdit(_));
        match draw {
            UiDraw::Panel(panel) => push_panel_shape(panel, viewport, &mut self.shapes),
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
        is_text
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
        // unchanged; only re-tessellate mutated (or text) nodes.
        //
        // Two phases: stage every node's shapes first (text layout allocates
        // glyphs into the shared font atlas as it goes), then tessellate once
        // the atlas has settled. Tessellating per node mid-loop bakes glyph
        // UVs against the atlas size at that moment; a later node adding new
        // glyphs (e.g. CJK) grows the atlas and leaves earlier nodes sampling
        // wrong texels for a frame.
        let ordered_nodes = std::mem::take(&mut self.ordered_nodes);
        let atlas_size_before = self.fonts.font_image_size();
        let mut staged: Vec<(NodeID, NodeTess)> = Vec::with_capacity(ordered_nodes.len());
        for node in &ordered_nodes {
            let Some(draw) = nodes.get(node) else {
                continue;
            };
            let cache_hit = self
                .node_cache
                .get(node)
                .is_some_and(|cached| cached.viewport == viewport && &cached.draw == draw);
            if cache_hit {
                staged.push((*node, NodeTess::Cached));
                continue;
            }
            let is_text = self.push_node_shapes(draw, viewport);
            staged.push((
                *node,
                NodeTess::Staged {
                    shapes: std::mem::take(&mut self.shapes),
                    rotations: std::mem::take(&mut self.shape_rotations),
                    is_text,
                },
            ));
        }
        // Atlas resized during layout: cached primitives were tessellated
        // against the old size, so their UVs are stale. Re-stage everything.
        if self.fonts.font_image_size() != atlas_size_before {
            self.node_cache.clear();
            for (node, entry) in &mut staged {
                if !matches!(entry, NodeTess::Cached) {
                    continue;
                }
                let Some(draw) = nodes.get(node) else {
                    continue;
                };
                let is_text = self.push_node_shapes(draw, viewport);
                *entry = NodeTess::Staged {
                    shapes: std::mem::take(&mut self.shapes),
                    rotations: std::mem::take(&mut self.shape_rotations),
                    is_text,
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
                NodeTess::Staged {
                    shapes,
                    rotations,
                    is_text,
                } => {
                    let mut node_primitives = tessellator.tessellate_shapes(shapes);
                    rotate_primitives(&mut node_primitives, &rotations);
                    node_primitives.retain(|primitive| match &primitive.primitive {
                        Primitive::Mesh(mesh) => {
                            !mesh.vertices.is_empty() && !mesh.indices.is_empty()
                        }
                        Primitive::Callback(_) => false,
                    });
                    self.primitives.extend(node_primitives.iter().cloned());
                    if is_text {
                        // Text depends on the shared atlas pass; do not cache it.
                        self.node_cache.remove(&node);
                    } else if let Some(draw) = nodes.get(&node) {
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

#[derive(Clone, Debug)]
struct UiFontSource {
    key: String,
    data: Arc<FontData>,
}

#[derive(Clone, Copy, Debug, PartialEq)]
struct HarfBuzzGlyph {
    glyph_id: u32,
    cluster: u32,
    x_advance: f32,
    y_advance: f32,
    x_offset: f32,
    y_offset: f32,
}

#[derive(Clone, Debug, PartialEq)]
struct HarfBuzzGlyphRun {
    glyphs: Vec<HarfBuzzGlyph>,
}

#[derive(Clone, Copy, Debug)]
struct HarfBuzzGlyphAlloc {
    offset: epaint::Vec2,
    size: epaint::Vec2,
    uv_min: epaint::Pos2,
    uv_max: epaint::Pos2,
}

#[derive(Clone, Debug, Hash, PartialEq, Eq)]
struct HarfBuzzGlyphKey {
    font_key: String,
    glyph_id: u32,
    font_size_bits: u32,
}

struct HarfBuzzAtlas {
    atlas: TextureAtlas,
    glyphs: AHashMap<HarfBuzzGlyphKey, HarfBuzzGlyphAlloc>,
}

impl HarfBuzzAtlas {
    fn new() -> Self {
        Self {
            atlas: TextureAtlas::new(
                [UI_HARFBUZZ_ATLAS_SIZE, UI_HARFBUZZ_ATLAS_SIZE],
                AlphaFromCoverage::default(),
            ),
            glyphs: AHashMap::new(),
        }
    }

    fn take_delta(&mut self) -> Option<epaint::ImageDelta> {
        self.atlas.take_delta()
    }

    fn glyph(
        &mut self,
        font: &UiFontSource,
        glyph_id: u32,
        font_size: f32,
    ) -> Option<HarfBuzzGlyphAlloc> {
        let key = HarfBuzzGlyphKey {
            font_key: font.key.clone(),
            glyph_id,
            font_size_bits: font_size.to_bits(),
        };
        if let Some(alloc) = self.glyphs.get(&key).copied() {
            return Some(alloc);
        }
        let alloc = raster_harfbuzz_glyph(font, glyph_id, font_size, &mut self.atlas)?;
        self.glyphs.insert(key, alloc);
        Some(alloc)
    }
}

fn default_ui_font_definitions() -> FontDefinitions {
    let mut definitions = FontDefinitions::default();
    append_system_font_fallbacks(&mut definitions);
    definitions
}

fn append_system_font_fallbacks(definitions: &mut FontDefinitions) {
    let mut db = fontdb::Database::new();
    db.load_system_fonts();

    let script_families = [
        (
            named_font_family(UI_CYRILLIC_FONT_FAMILY),
            UI_CYRILLIC_FONT_FAMILIES,
        ),
        (
            named_font_family(UI_ARABIC_FONT_FAMILY),
            UI_ARABIC_FONT_FAMILIES,
        ),
        (
            named_font_family(UI_HEBREW_FONT_FAMILY),
            UI_HEBREW_FONT_FAMILIES,
        ),
        (
            named_font_family(UI_INDIC_FONT_FAMILY),
            UI_INDIC_FONT_FAMILIES,
        ),
        (
            named_font_family(UI_THAI_FONT_FAMILY),
            UI_THAI_FONT_FAMILIES,
        ),
        (
            named_font_family(UI_SE_ASIAN_FONT_FAMILY),
            UI_SE_ASIAN_FONT_FAMILIES,
        ),
        (
            named_font_family(UI_JAPANESE_FONT_FAMILY),
            UI_JAPANESE_FONT_FAMILIES,
        ),
        (
            named_font_family(UI_CHINESE_FONT_FAMILY),
            UI_CHINESE_FONT_FAMILIES,
        ),
        (
            named_font_family(UI_KOREAN_FONT_FAMILY),
            UI_KOREAN_FONT_FAMILIES,
        ),
    ];

    // Default (Latin) fonts go first in every script family so shared Latin
    // glyphs in mixed text render from the same font as pure-Latin labels,
    // with consistent metrics. Script fonts only pick up the glyphs the
    // defaults lack (CJK, Arabic, ...).
    for (target_family, _) in &script_families {
        append_default_family_fallbacks(
            definitions,
            target_family.clone(),
            FontFamily::Proportional,
        );
    }

    for (font_family, source_families) in &script_families {
        for &name in *source_families {
            append_system_font_family(definitions, &db, name, font_family.clone());
        }
    }

    for (target_family, _) in &script_families {
        for (source_family, _) in &script_families {
            append_family_fallbacks(definitions, target_family.clone(), source_family.clone());
        }
    }
}

fn named_font_family(name: &'static str) -> FontFamily {
    FontFamily::Name(Arc::from(name))
}

fn append_system_font_family(
    definitions: &mut FontDefinitions,
    db: &fontdb::Database,
    family_name: &str,
    target_family: FontFamily,
) {
    let query = fontdb::Query {
        families: &[fontdb::Family::Name(family_name)],
        ..fontdb::Query::default()
    };
    let Some(id) = db.query(&query) else {
        return;
    };
    let Some((source, index)) = db.face_source(id) else {
        return;
    };
    let Some(font_data) = font_data_from_source(source, index) else {
        return;
    };
    let font_key = format!("{UI_SYSTEM_FONT_PREFIX}-{family_name}-{index}");
    if !definitions.font_data.contains_key(&font_key) {
        definitions
            .font_data
            .insert(font_key.clone(), Arc::new(font_data));
    }
    append_font_fallback(definitions, target_family, &font_key);
}

fn font_data_from_source(source: fontdb::Source, index: u32) -> Option<FontData> {
    let font = match source {
        fontdb::Source::Binary(data) => data.as_ref().as_ref().to_vec(),
        fontdb::Source::File(path) => std::fs::read(path).ok()?,
        fontdb::Source::SharedFile(_, data) => data.as_ref().as_ref().to_vec(),
    };
    Some(FontData {
        font: std::borrow::Cow::Owned(font),
        index,
        tweak: Default::default(),
    })
}

fn append_default_family_fallbacks(
    definitions: &mut FontDefinitions,
    target_family: FontFamily,
    default_family: FontFamily,
) {
    let defaults = definitions
        .families
        .get(&default_family)
        .cloned()
        .unwrap_or_default();
    let list = definitions.families.entry(target_family).or_default();
    for font_key in defaults {
        if !list.iter().any(|name| name == &font_key) {
            list.push(font_key);
        }
    }
}

fn append_family_fallbacks(
    definitions: &mut FontDefinitions,
    target_family: FontFamily,
    source_family: FontFamily,
) {
    let source = definitions
        .families
        .get(&source_family)
        .cloned()
        .unwrap_or_default();
    let target = definitions.families.entry(target_family).or_default();
    for font_key in source {
        if !target.iter().any(|name| name == &font_key) {
            target.push(font_key);
        }
    }
}

fn append_font_fallback(definitions: &mut FontDefinitions, family: FontFamily, font_key: &str) {
    let list = definitions.families.entry(family).or_default();
    if !list.iter().any(|name| name == font_key) {
        list.push(font_key.to_owned());
    }
}

fn text_font_family(text: &str, default_family: FontFamily) -> FontFamily {
    let mut has_han = false;
    let mut has_kana = false;
    let mut has_hangul = false;
    let mut has_cyrillic = false;
    let mut has_arabic = false;
    let mut has_hebrew = false;
    let mut has_indic = false;
    let mut has_thai = false;
    let mut has_se_asian = false;

    for ch in text.chars() {
        has_han |= is_han(ch);
        has_kana |= is_kana(ch);
        has_hangul |= is_hangul(ch);
        has_cyrillic |= is_cyrillic(ch);
        has_arabic |= is_arabic(ch);
        has_hebrew |= is_hebrew(ch);
        has_indic |= is_indic(ch);
        has_thai |= is_thai(ch);
        has_se_asian |= is_se_asian(ch);
    }

    if has_hangul {
        named_font_family(UI_KOREAN_FONT_FAMILY)
    } else if has_kana {
        named_font_family(UI_JAPANESE_FONT_FAMILY)
    } else if has_han {
        named_font_family(UI_CHINESE_FONT_FAMILY)
    } else if has_cyrillic {
        named_font_family(UI_CYRILLIC_FONT_FAMILY)
    } else if has_arabic {
        named_font_family(UI_ARABIC_FONT_FAMILY)
    } else if has_hebrew {
        named_font_family(UI_HEBREW_FONT_FAMILY)
    } else if has_indic {
        named_font_family(UI_INDIC_FONT_FAMILY)
    } else if has_thai {
        named_font_family(UI_THAI_FONT_FAMILY)
    } else if has_se_asian {
        named_font_family(UI_SE_ASIAN_FONT_FAMILY)
    } else {
        default_family
    }
}

fn is_han(ch: char) -> bool {
    matches!(
        ch as u32,
        0x3400..=0x4DBF
            | 0x4E00..=0x9FFF
            | 0xF900..=0xFAFF
            | 0x20000..=0x2A6DF
            | 0x2A700..=0x2B73F
            | 0x2B740..=0x2B81F
            | 0x2B820..=0x2CEAF
            | 0x2CEB0..=0x2EBEF
            | 0x30000..=0x3134F
    )
}

fn is_kana(ch: char) -> bool {
    matches!(ch as u32, 0x3040..=0x30FF | 0x31F0..=0x31FF)
}

fn is_hangul(ch: char) -> bool {
    matches!(ch as u32, 0x1100..=0x11FF | 0x3130..=0x318F | 0xAC00..=0xD7AF)
}

fn is_cyrillic(ch: char) -> bool {
    matches!(ch as u32, 0x0400..=0x052F | 0x2DE0..=0x2DFF | 0xA640..=0xA69F)
}

fn is_arabic(ch: char) -> bool {
    matches!(
        ch as u32,
        0x0600..=0x06FF
            | 0x0750..=0x077F
            | 0x0870..=0x089F
            | 0x08A0..=0x08FF
            | 0xFB50..=0xFDFF
            | 0xFE70..=0xFEFF
    )
}

fn is_hebrew(ch: char) -> bool {
    matches!(ch as u32, 0x0590..=0x05FF | 0xFB1D..=0xFB4F)
}

fn is_indic(ch: char) -> bool {
    matches!(ch as u32, 0x0900..=0x0DFF)
}

fn is_thai(ch: char) -> bool {
    matches!(ch as u32, 0x0E00..=0x0E7F)
}

fn is_se_asian(ch: char) -> bool {
    matches!(
        ch as u32,
        0x0E80..=0x0EFF | 0x0F00..=0x0FFF | 0x1000..=0x109F | 0x1780..=0x17FF
    )
}

fn needs_harfbuzz(text: &str) -> bool {
    text.chars()
        .any(|ch| is_arabic(ch) || is_hebrew(ch) || is_indic(ch) || is_thai(ch) || is_se_asian(ch))
}

fn font_sources_for_family(definitions: &FontDefinitions, family: FontFamily) -> Vec<UiFontSource> {
    definitions
        .families
        .get(&family)
        .into_iter()
        .flatten()
        .filter_map(|key| {
            definitions.font_data.get(key).map(|data| UiFontSource {
                key: key.clone(),
                data: data.clone(),
            })
        })
        .collect()
}

fn shape_text_with_harfbuzz(font: &UiFontSource, text: &str) -> Option<HarfBuzzGlyphRun> {
    let face = rustybuzz::Face::from_slice(font.data.font.as_ref(), font.data.index)?;
    let units_per_em = face.units_per_em().max(1) as f32;
    let mut buffer = rustybuzz::UnicodeBuffer::new();
    buffer.push_str(text);
    let glyph_buffer = rustybuzz::shape(&face, &[], buffer);
    let glyphs = glyph_buffer
        .glyph_infos()
        .iter()
        .zip(glyph_buffer.glyph_positions())
        .map(|(info, pos)| HarfBuzzGlyph {
            glyph_id: info.glyph_id,
            cluster: info.cluster,
            x_advance: pos.x_advance as f32 / units_per_em,
            y_advance: pos.y_advance as f32 / units_per_em,
            x_offset: pos.x_offset as f32 / units_per_em,
            y_offset: pos.y_offset as f32 / units_per_em,
        })
        .collect::<Vec<_>>();
    if glyphs.is_empty() || glyphs.iter().any(|glyph| glyph.glyph_id == 0) {
        return None;
    }
    Some(HarfBuzzGlyphRun { glyphs })
}

fn shape_text_with_font_fallbacks(
    definitions: &FontDefinitions,
    family: FontFamily,
    text: &str,
) -> Option<(UiFontSource, HarfBuzzGlyphRun)> {
    font_sources_for_family(definitions, family)
        .into_iter()
        .find_map(|font| shape_text_with_harfbuzz(&font, text).map(|run| (font, run)))
}

fn raster_harfbuzz_glyph(
    font: &UiFontSource,
    glyph_id: u32,
    font_size: f32,
    atlas: &mut TextureAtlas,
) -> Option<HarfBuzzGlyphAlloc> {
    let font_ref = ab_glyph::FontRef::try_from_slice(font.data.font.as_ref()).ok()?;
    let glyph_id = ab_glyph::GlyphId(glyph_id.min(u16::MAX as u32) as u16);
    let glyph =
        glyph_id.with_scale_and_position(font_size * UI_RASTER_SCALE, ab_glyph::point(0.0, 0.0));
    let outlined = font_ref.outline_glyph(glyph)?;
    let bounds = outlined.px_bounds();
    let width = bounds.width().ceil().max(0.0) as usize;
    let height = bounds.height().ceil().max(0.0) as usize;
    if width == 0 || height == 0 {
        return Some(HarfBuzzGlyphAlloc {
            offset: vec2(0.0, 0.0),
            size: vec2(0.0, 0.0),
            uv_min: pos2(0.0, 0.0),
            uv_max: pos2(0.0, 0.0),
        });
    }
    let (glyph_pos, image) = atlas.allocate((width, height));
    outlined.draw(|x, y, coverage| {
        if coverage > 0.0 {
            image[(glyph_pos.0 + x as usize, glyph_pos.1 + y as usize)] =
                Color32::from_white_alpha((coverage * 255.0).round().clamp(0.0, 255.0) as u8);
        }
    });
    let atlas_size = atlas.size();
    Some(HarfBuzzGlyphAlloc {
        offset: vec2(bounds.min.x, bounds.min.y) / UI_RASTER_SCALE,
        size: vec2(width as f32, height as f32) / UI_RASTER_SCALE,
        uv_min: pos2(
            glyph_pos.0 as f32 / atlas_size[0] as f32,
            glyph_pos.1 as f32 / atlas_size[1] as f32,
        ),
        uv_max: pos2(
            (glyph_pos.0 + width) as f32 / atlas_size[0] as f32,
            (glyph_pos.1 + height) as f32 / atlas_size[1] as f32,
        ),
    })
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
        UiDraw::Shape(shape) => shape.rect,
        UiDraw::ColorWheel(wheel) => wheel.rect,
        UiDraw::Button(button) => button.panel.rect,
        UiDraw::Checkbox(checkbox) => checkbox.panel.rect,
        UiDraw::Image(image) => image.rect,
        UiDraw::NineSlice(image) => image.rect,
        UiDraw::Label(label) => label.rect,
        UiDraw::TextEdit(edit) => edit.panel.rect,
    }
}

fn push_ui_shape(shape: &UiShapeDraw, viewport: [f32; 2], out: &mut Vec<ClippedShape>) {
    if !valid_rect(shape.rect) {
        return;
    }
    let (min, max) = shape.rect.screen_min_max(viewport);
    let rect = Rect::from_min_max(pos2(min[0], min[1]), pos2(max[0], max[1]));
    let clip_rect = clip_rect_from_state(shape.clip_rect, viewport);
    let fill = color32(shape.fill);
    let stroke = Stroke::new(shape.stroke_width.max(0.0), color32(shape.stroke));
    let draw_shape = match shape.kind {
        UiShapeKind::Rect => Shape::Rect(RectShape::new(
            rect,
            CornerRadius::ZERO,
            fill,
            stroke,
            StrokeKind::Inside,
        )),
        UiShapeKind::Circle => Shape::Circle(CircleShape {
            center: rect.center(),
            radius: rect.width().min(rect.height()) * 0.5,
            fill,
            stroke,
        }),
        UiShapeKind::Triangle => Shape::convex_polygon(
            vec![
                rect.left_top(),
                rect.left_bottom(),
                pos2(rect.right(), rect.center().y),
            ],
            fill,
            stroke,
        ),
    };
    out.push(ClippedShape {
        clip_rect,
        shape: draw_shape,
    });
}

fn push_color_wheel_shape(
    wheel: &UiColorWheelDraw,
    viewport: [f32; 2],
    out: &mut Vec<ClippedShape>,
) {
    if !valid_rect(wheel.rect) {
        return;
    }
    let (min, max) = wheel.rect.screen_min_max(viewport);
    let rect = Rect::from_min_max(pos2(min[0], min[1]), pos2(max[0], max[1]));
    let clip_rect = clip_rect_from_state(wheel.clip_rect, viewport);
    push_color_wheel(
        rect.center(),
        rect.width().min(rect.height()) * 0.5,
        clip_rect,
        out,
    );
}

fn push_checkbox_shapes(
    checkbox: &UiCheckboxDraw,
    viewport: [f32; 2],
    out: &mut Vec<ClippedShape>,
) {
    push_panel_shape(&checkbox.panel, viewport, out);
    if !checkbox.checked || !valid_color(checkbox.dot_fill) {
        return;
    }
    let (min, max) = checkbox.panel.rect.screen_min_max(viewport);
    let rect = Rect::from_min_max(pos2(min[0], min[1]), pos2(max[0], max[1]));
    let clip_rect = clip_rect_from_state(checkbox.panel.clip_rect, viewport);
    let radius = rect.width().min(rect.height()) * 0.24;
    out.push(ClippedShape {
        clip_rect,
        shape: Shape::circle_filled(rect.center(), radius.max(1.0), color32(checkbox.dot_fill)),
    });
}

fn push_color_wheel(
    center: epaint::Pos2,
    radius: f32,
    clip_rect: Rect,
    out: &mut Vec<ClippedShape>,
) {
    let steps = 48;
    for idx in 0..steps {
        let a0 = idx as f32 / steps as f32 * std::f32::consts::TAU;
        let a1 = (idx + 1) as f32 / steps as f32 * std::f32::consts::TAU;
        let p0 = center + vec2(a0.cos() * radius, a0.sin() * radius);
        let p1 = center + vec2(a1.cos() * radius, a1.sin() * radius);
        let color = hsv_color(idx as f32 / steps as f32, 1.0, 1.0);
        out.push(ClippedShape {
            clip_rect,
            shape: Shape::convex_polygon(vec![center, p0, p1], color32(color), Stroke::NONE),
        });
    }
    out.push(ClippedShape {
        clip_rect,
        shape: Shape::Circle(CircleShape {
            center,
            radius: radius * 0.42,
            fill: Color32::from_white_alpha(210),
            stroke: Stroke::new(1.0, Color32::from_black_alpha(80)),
        }),
    });
}

fn hsv_color(h: f32, s: f32, v: f32) -> perro_structs::Color {
    let h = h.rem_euclid(1.0) * 6.0;
    let i = h.floor();
    let f = h - i;
    let p = v * (1.0 - s);
    let q = v * (1.0 - f * s);
    let t = v * (1.0 - (1.0 - f) * s);
    let (r, g, b) = match i as i32 {
        0 => (v, t, p),
        1 => (q, v, p),
        2 => (p, v, t),
        3 => (p, q, v),
        4 => (t, p, v),
        _ => (v, p, q),
    };
    perro_structs::Color::new(r, g, b, 1.0)
}

fn push_nine_slice_shapes(
    image: &UiNineSliceDraw,
    viewport: [f32; 2],
    out: &mut Vec<ClippedShape>,
) {
    if image.texture.is_nil() || !valid_rect(image.rect) || !valid_color(image.tint) {
        return;
    }
    let (min, max) = image.rect.screen_min_max(viewport);
    let outer = Rect::from_min_max(pos2(min[0], min[1]), pos2(max[0], max[1]));
    if outer.width() <= 0.0 || outer.height() <= 0.0 {
        return;
    }
    let [l, t, r, b] = clamp_nine_margins(image.margins, outer.width(), outer.height());
    let [u0, v0] = image.uv_min;
    let [u3, v3] = image.uv_max;
    let uw = (u3 - u0).max(0.0);
    let vh = (v3 - v0).max(0.0);
    if uw <= 0.0 || vh <= 0.0 {
        return;
    }
    let ul = l.min(uw);
    let ur = r.min((uw - ul).max(0.0));
    let vt = t.min(vh);
    let vb = b.min((vh - vt).max(0.0));
    let xs = [
        outer.left(),
        outer.left() + l,
        outer.right() - r,
        outer.right(),
    ];
    let ys = [
        outer.top(),
        outer.top() + t,
        outer.bottom() - b,
        outer.bottom(),
    ];
    let us = [u0, u0 + ul, u3 - ur, u3];
    let vs = [v0, v0 + vt, v3 - vb, v3];
    let mut mesh = Mesh::with_texture(TextureId::User(image.texture.as_u64()));
    for y in 0..3 {
        for x in 0..3 {
            if xs[x + 1] <= xs[x] || ys[y + 1] <= ys[y] || us[x + 1] <= us[x] || vs[y + 1] <= vs[y]
            {
                continue;
            }
            mesh.add_rect_with_uv(
                Rect::from_min_max(pos2(xs[x], ys[y]), pos2(xs[x + 1], ys[y + 1])),
                Rect::from_min_max(pos2(us[x], vs[y]), pos2(us[x + 1], vs[y + 1])),
                color32(image.tint),
            );
        }
    }
    out.push(ClippedShape {
        clip_rect: clip_rect_from_state(image.clip_rect, viewport),
        shape: Shape::Mesh(mesh.into()),
    });
}

fn clamp_nine_margins(margins: [f32; 4], w: f32, h: f32) -> [f32; 4] {
    let mut l = margins[0].max(0.0);
    let mut t = margins[1].max(0.0);
    let mut r = margins[2].max(0.0);
    let mut b = margins[3].max(0.0);
    let sx = (w / (l + r).max(w)).min(1.0);
    let sy = (h / (t + b).max(h)).min(1.0);
    l *= sx;
    r *= sx;
    t *= sy;
    b *= sy;
    [l, t, r, b]
}

fn push_image_shape(image: &UiImageDraw, viewport: [f32; 2], out: &mut Vec<ClippedShape>) {
    if image.texture.is_nil() || !valid_rect(image.rect) || !valid_color(image.tint) {
        return;
    }
    let (min, max) = image.rect.screen_min_max(viewport);
    let outer = Rect::from_min_max(pos2(min[0], min[1]), pos2(max[0], max[1]));
    let rect = resolve_image_rect(outer, image);
    if rect.width() <= 0.0 || rect.height() <= 0.0 {
        return;
    }
    let uv = Rect::from_min_max(
        pos2(image.uv_min[0], image.uv_min[1]),
        pos2(image.uv_max[0], image.uv_max[1]),
    );
    let mut mesh = Mesh::with_texture(TextureId::User(image.texture.as_u64()));
    let radii = resolve_rect_corner_radii(rect, image.corner_radii);
    if has_any_radius(radii) {
        add_rounded_rect_with_uv(&mut mesh, rect, uv, radii, color32(image.tint));
    } else {
        mesh.add_rect_with_uv(rect, uv, color32(image.tint));
    }
    out.push(ClippedShape {
        clip_rect: clip_rect_from_state(image.clip_rect, viewport),
        shape: Shape::Mesh(mesh.into()),
    });
}

fn add_rounded_rect_with_uv(
    mesh: &mut Mesh,
    rect: Rect,
    uv: Rect,
    radii: ResolvedCornerRadii,
    color: Color32,
) {
    if !has_any_radius(radii) {
        mesh.add_rect_with_uv(rect, uv, color);
        return;
    }

    let base = mesh.vertices.len() as u32;
    let center = rect.center();
    mesh.vertices.push(Vertex {
        pos: center,
        uv: rect_uv(rect, uv, center),
        color,
    });

    for pos in rounded_rect_points(rect, radii, 6) {
        mesh.vertices.push(Vertex {
            pos,
            uv: rect_uv(rect, uv, pos),
            color,
        });
    }

    let point_count = mesh.vertices.len() as u32 - base - 1;
    for idx in 0..point_count {
        mesh.indices.extend_from_slice(&[
            base,
            base + idx + 1,
            base + ((idx + 1) % point_count) + 1,
        ]);
    }
}

fn rect_uv(rect: Rect, uv: Rect, pos: epaint::Pos2) -> epaint::Pos2 {
    let x = ((pos.x - rect.left()) / rect.width().max(0.0001)).clamp(0.0, 1.0);
    let y = ((pos.y - rect.top()) / rect.height().max(0.0001)).clamp(0.0, 1.0);
    pos2(uv.left() + uv.width() * x, uv.top() + uv.height() * y)
}

fn resolve_image_rect(outer: Rect, image: &UiImageDraw) -> Rect {
    if image.scale_mode == UiImageScaleState::Stretch {
        return outer;
    }
    let aspect = image.aspect_ratio;
    if !aspect.is_finite() || aspect <= 0.0 || outer.width() <= 0.0 || outer.height() <= 0.0 {
        return outer;
    }
    let outer_aspect = outer.width() / outer.height();
    let scale_by_width = match image.scale_mode {
        UiImageScaleState::Stretch => return outer,
        UiImageScaleState::Fit => outer_aspect <= aspect,
        UiImageScaleState::Cover => outer_aspect > aspect,
    };
    let size = if scale_by_width {
        vec2(outer.width(), outer.width() / aspect)
    } else {
        vec2(outer.height() * aspect, outer.height())
    };
    let x = match image.h_align {
        UiTextAlignState::Start => outer.left(),
        UiTextAlignState::Center => outer.left() + (outer.width() - size.x) * 0.5,
        UiTextAlignState::End => outer.right() - size.x,
    };
    let y = match image.v_align {
        UiTextAlignState::Start => outer.top(),
        UiTextAlignState::Center => outer.top() + (outer.height() - size.y) * 0.5,
        UiTextAlignState::End => outer.bottom() - size.y,
    };
    Rect::from_min_size(pos2(x, y), size)
}

fn push_label_shape(
    label: &UiLabelDraw,
    viewport: [f32; 2],
    definitions: &FontDefinitions,
    harfbuzz_atlas: &mut HarfBuzzAtlas,
    fonts: &mut Fonts,
    out: &mut Vec<ClippedShape>,
) {
    if needs_harfbuzz(label.text.as_ref())
        && push_harfbuzz_text_shape(
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
            definitions,
            harfbuzz_atlas,
            out,
        )
    {
        return;
    }
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

fn push_harfbuzz_text_shape(
    input: TextShapeInput<'_>,
    definitions: &FontDefinitions,
    harfbuzz_atlas: &mut HarfBuzzAtlas,
    out: &mut Vec<ClippedShape>,
) -> bool {
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
        return false;
    }
    let family = text_font_family(text, FontFamily::Proportional);
    let Some((font, run)) = shape_text_with_font_fallbacks(definitions, family, text) else {
        return false;
    };
    let (min, max) = rect.screen_min_max(viewport);
    let line_width = run
        .glyphs
        .iter()
        .map(|glyph| glyph.x_advance * font_size)
        .sum::<f32>()
        .max(0.0);
    let line_height = font_size;
    let mut cursor = match h_align {
        UiTextAlignState::Start => min[0],
        UiTextAlignState::Center => min[0] + (rect.size[0] - line_width).max(0.0) * 0.5,
        UiTextAlignState::End => max[0] - line_width,
    };
    let baseline = match v_align {
        UiTextAlignState::Start => min[1] + line_height,
        UiTextAlignState::Center => {
            min[1] + (rect.size[1] - line_height).max(0.0) * 0.5 + line_height
        }
        UiTextAlignState::End => max[1],
    };
    let mut mesh = Mesh::with_texture(UI_HARFBUZZ_TEXTURE_ID);
    let color = color32(color);
    for glyph in run.glyphs {
        let Some(alloc) = harfbuzz_atlas.glyph(&font, glyph.glyph_id, font_size) else {
            cursor += glyph.x_advance * font_size;
            continue;
        };
        if alloc.size.x > 0.0 && alloc.size.y > 0.0 {
            let x = cursor + glyph.x_offset * font_size + alloc.offset.x;
            let y = baseline - glyph.y_offset * font_size + alloc.offset.y;
            let rect = Rect::from_min_size(pos2(x, y), alloc.size);
            let base = mesh.vertices.len() as u32;
            mesh.vertices.extend_from_slice(&[
                Vertex {
                    pos: rect.left_top(),
                    uv: alloc.uv_min,
                    color,
                },
                Vertex {
                    pos: rect.right_top(),
                    uv: pos2(alloc.uv_max.x, alloc.uv_min.y),
                    color,
                },
                Vertex {
                    pos: rect.right_bottom(),
                    uv: alloc.uv_max,
                    color,
                },
                Vertex {
                    pos: rect.left_bottom(),
                    uv: pos2(alloc.uv_min.x, alloc.uv_max.y),
                    color,
                },
            ]);
            mesh.indices
                .extend_from_slice(&[base, base + 1, base + 2, base, base + 2, base + 3]);
        }
        cursor += glyph.x_advance * font_size;
    }
    if mesh.vertices.is_empty() || mesh.indices.is_empty() {
        return false;
    }
    out.push(ClippedShape {
        clip_rect,
        shape: Shape::Mesh(Arc::new(mesh)),
    });
    true
}

fn push_panel_shape(panel: &UiPanelDraw, viewport: [f32; 2], out: &mut Vec<ClippedShape>) {
    if !valid_rect(panel.rect) || !valid_color(panel.stroke) || !valid_gradient(panel.gradient) {
        return;
    }

    let (min, max) = panel.rect.screen_min_max(viewport);
    let rect = Rect::from_min_max(pos2(min[0], min[1]), pos2(max[0], max[1]));
    let clip_rect = clip_rect_from_state(panel.clip_rect, viewport);
    let radii = resolve_corner_radii(panel, rect);
    push_outer_fill_effect(panel.outer_shadow, rect, radii, clip_rect, out);
    push_outer_stroke_effect(panel.outer_highlight, rect, radii, clip_rect, out);
    push_panel_fill_shape(panel, rect, radii, clip_rect, out);
    push_panel_stroke_shape(panel, rect, radii, clip_rect, out);
    push_inner_fill_effect(panel.inner_shadow, rect, radii, clip_rect, out);
    push_inner_stroke_effect(panel.inner_highlight, rect, radii, clip_rect, out);
}

fn push_panel_fill_shape(
    panel: &UiPanelDraw,
    rect: Rect,
    radii: ResolvedCornerRadii,
    clip_rect: Rect,
    out: &mut Vec<ClippedShape>,
) {
    match panel.fill_kind {
        UiFillKindState::Solid => {
            if !valid_color(panel.fill) {
                return;
            }
            out.push(ClippedShape {
                clip_rect,
                shape: Shape::Rect(RectShape::filled(
                    rect,
                    radii_to_corner_radius(rect, radii),
                    color32(panel.fill),
                )),
            });
        }
        UiFillKindState::Linear => {
            push_gradient_panel_shape(panel, rect, radii, clip_rect, out);
        }
    }
}

fn push_panel_stroke_shape(
    panel: &UiPanelDraw,
    rect: Rect,
    radii: ResolvedCornerRadii,
    clip_rect: Rect,
    out: &mut Vec<ClippedShape>,
) {
    if !valid_color(panel.stroke) || panel.stroke_width <= 0.0 {
        return;
    }
    out.push(ClippedShape {
        clip_rect,
        shape: Shape::Rect(RectShape::new(
            rect,
            radii_to_corner_radius(rect, radii),
            Color32::TRANSPARENT,
            Stroke::new(panel.stroke_width.max(0.0), color32(panel.stroke)),
            StrokeKind::Inside,
        )),
    });
}

fn push_gradient_panel_shape(
    panel: &UiPanelDraw,
    rect: Rect,
    radii: ResolvedCornerRadii,
    clip_rect: Rect,
    out: &mut Vec<ClippedShape>,
) {
    let mut mesh = Mesh::default();
    add_rounded_rect_gradient(&mut mesh, rect, radii, panel.gradient);
    if mesh.vertices.is_empty() || mesh.indices.is_empty() {
        return;
    }
    out.push(ClippedShape {
        clip_rect,
        shape: Shape::Mesh(mesh.into()),
    });
}

fn push_outer_fill_effect(
    effect: UiDepthEffectState,
    rect: Rect,
    radii: ResolvedCornerRadii,
    clip_rect: Rect,
    out: &mut Vec<ClippedShape>,
) {
    if !valid_effect(effect) {
        return;
    }
    let offset = effect_offset(effect);
    let steps = effect.falloff.max(0.0).ceil().clamp(1.0, 24.0) as usize;
    for step in (0..steps).rev() {
        let t = (step + 1) as f32 / steps as f32;
        let expand = effect_size_expand(rect, effect) + effect.falloff.max(0.0) * t;
        let alpha = effect.color.a() * (1.0 - t * 0.82);
        let color = with_alpha(effect.color, alpha);
        if !valid_color(color) || alpha <= 0.0 {
            continue;
        }
        let rect = rect.translate(offset).expand(expand);
        if rect.width() <= 0.0 || rect.height() <= 0.0 {
            continue;
        }
        out.push(ClippedShape {
            clip_rect,
            shape: Shape::Rect(RectShape::filled(
                rect,
                radii_to_corner_radius(rect, radii),
                color32(color),
            )),
        });
    }
}

fn push_outer_stroke_effect(
    effect: UiDepthEffectState,
    rect: Rect,
    radii: ResolvedCornerRadii,
    clip_rect: Rect,
    out: &mut Vec<ClippedShape>,
) {
    if !valid_effect(effect) {
        return;
    }
    let offset = effect_offset(effect);
    let steps = effect.falloff.max(1.0).ceil().clamp(1.0, 24.0) as usize;
    let size_expand = effect_size_expand(rect, effect);
    let stroke_base = (rect.width().min(rect.height()).max(1.0) * 0.035).max(1.0);
    for step in 0..steps {
        let t = step as f32 / steps as f32;
        let expand = effect.distance.max(0.0) + effect.falloff.max(0.0) * t;
        let stroke_width = (stroke_base * (1.0 - t * 0.65)).max(0.5);
        let alpha = effect.color.a() * (1.0 - t);
        let color = with_alpha(effect.color, alpha);
        if !valid_color(color) || alpha <= 0.0 {
            continue;
        }
        let rect = rect.translate(-offset).expand(size_expand + expand);
        if rect.width() <= 0.0 || rect.height() <= 0.0 {
            continue;
        }
        out.push(ClippedShape {
            clip_rect,
            shape: Shape::Rect(RectShape::new(
                rect,
                radii_to_corner_radius(rect, radii),
                Color32::TRANSPARENT,
                Stroke::new(stroke_width, color32(color)),
                StrokeKind::Inside,
            )),
        });
    }
}

fn push_inner_fill_effect(
    effect: UiDepthEffectState,
    rect: Rect,
    radii: ResolvedCornerRadii,
    clip_rect: Rect,
    out: &mut Vec<ClippedShape>,
) {
    if !valid_effect(effect) {
        return;
    }
    let inner_clip = clip_rect.intersect(rect);
    let offset = effect_offset(effect);
    let steps = effect.falloff.max(1.0).ceil().clamp(1.0, 24.0) as usize;
    for step in 0..steps {
        let t = (step + 1) as f32 / steps as f32;
        let expand = effect_size_expand(rect, effect) + effect.falloff.max(0.0) * (1.0 - t);
        let shrink = effect.distance.max(0.0) * t * 0.6;
        let alpha = effect.color.a() * (1.0 - t * 0.78);
        let color = with_alpha(effect.color, alpha);
        if !valid_color(color) || alpha <= 0.0 {
            continue;
        }
        let rect = rect.translate(offset).expand(expand).shrink(shrink);
        if rect.width() <= 0.0 || rect.height() <= 0.0 {
            continue;
        }
        out.push(ClippedShape {
            clip_rect: inner_clip,
            shape: Shape::Rect(RectShape::filled(
                rect,
                radii_to_corner_radius(rect, radii),
                color32(color),
            )),
        });
    }
}

fn push_inner_stroke_effect(
    effect: UiDepthEffectState,
    rect: Rect,
    radii: ResolvedCornerRadii,
    clip_rect: Rect,
    out: &mut Vec<ClippedShape>,
) {
    if !valid_effect(effect) {
        return;
    }
    let inner_clip = clip_rect.intersect(rect);
    let offset = effect_offset(effect);
    let steps = effect.falloff.max(1.0).ceil().clamp(1.0, 24.0) as usize;
    let size_expand = effect_size_expand(rect, effect);
    let stroke_base = (rect.width().min(rect.height()).max(1.0) * 0.035).max(1.0);
    for step in 0..steps {
        let t = step as f32 / steps as f32;
        let inset = effect.distance.max(0.0) + effect.falloff.max(0.0) * t;
        let stroke_width = (stroke_base * (1.0 - t * 0.65)).max(0.5);
        let alpha = effect.color.a() * (1.0 - t);
        let color = with_alpha(effect.color, alpha);
        if !valid_color(color) || alpha <= 0.0 {
            continue;
        }
        let rect = rect.translate(-offset).expand(size_expand).shrink(inset);
        if rect.width() <= 0.0 || rect.height() <= 0.0 {
            break;
        }
        out.push(ClippedShape {
            clip_rect: inner_clip,
            shape: Shape::Rect(RectShape::new(
                rect,
                radii_to_corner_radius(rect, radii),
                Color32::TRANSPARENT,
                Stroke::new(stroke_width, color32(color)),
                StrokeKind::Inside,
            )),
        });
    }
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
        let family = text_font_family(edit.text.as_ref(), FontFamily::Monospace);
        Some(fonts.with_pixels_per_point(UI_RASTER_SCALE).layout(
            edit.text.to_string(),
            FontId::new(edit.font_size, family),
            color32(edit.color),
            wrap_width,
        ))
    } else {
        None
    };
    if edit.focused
        && let Some(galley) = edit_galley.as_deref()
    {
        let draw_pos = text_edit_draw_pos(edit, content_min, content_max, galley);
        push_selection_shapes(edit, galley, clip_rect, draw_pos, out);
    }
    if !body.is_empty() && valid_color(color) {
        let family = text_font_family(body, FontFamily::Monospace);
        let galley = fonts.with_pixels_per_point(UI_RASTER_SCALE).layout(
            body.to_string(),
            FontId::new(edit.font_size, family),
            color32(color),
            wrap_width,
        );
        let draw_pos = text_edit_draw_pos(edit, content_min, content_max, &galley);
        out.push(ClippedShape {
            clip_rect,
            shape: Shape::galley_with_override_text_color(draw_pos, galley, color32(color)),
        });
    }

    if !edit.focused {
        return;
    }
    if let Some(galley) = edit_galley.as_deref() {
        let draw_pos = text_edit_draw_pos(edit, content_min, content_max, galley);
        push_caret_shape(edit, galley, clip_rect, draw_pos, out);
    }
}

fn text_edit_draw_pos(
    edit: &UiTextEditDraw,
    content_min: epaint::Pos2,
    content_max: epaint::Pos2,
    galley: &Galley,
) -> epaint::Pos2 {
    let content_size = content_max - content_min;
    let x_offset = match edit.h_align {
        UiTextAlignState::Start => 0.0,
        UiTextAlignState::Center => (content_size.x - galley.size().x).max(0.0) * 0.5,
        UiTextAlignState::End => (content_size.x - galley.size().x).max(0.0),
    };
    let y_offset = match edit.v_align {
        UiTextAlignState::Start => 0.0,
        UiTextAlignState::Center => {
            if edit.multiline {
                0.0
            } else {
                (content_size.y - galley.size().y).max(0.0) * 0.5
            }
        }
        UiTextAlignState::End => {
            if edit.multiline {
                0.0
            } else {
                (content_size.y - galley.size().y).max(0.0)
            }
        }
    };
    pos2(
        content_min.x + x_offset - edit.scroll[0],
        content_min.y + y_offset - edit.scroll[1],
    )
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

fn resolve_rect_corner_radius(rect: Rect, corner_radius: f32) -> f32 {
    let ratio = if corner_radius.is_finite() {
        corner_radius.clamp(0.0, 1.0)
    } else {
        1.0
    };
    (rect.width().min(rect.height()).max(0.0) * 0.5 * ratio).min(u8::MAX as f32)
}

#[derive(Clone, Copy, Debug, Default, PartialEq)]
struct ResolvedCornerRadii {
    tl: f32,
    tr: f32,
    br: f32,
    bl: f32,
}

fn resolve_corner_radii(panel: &UiPanelDraw, rect: Rect) -> ResolvedCornerRadii {
    resolve_rect_corner_radii(rect, panel.corner_radii)
}

fn resolve_rect_corner_radii(rect: Rect, corner_radii: UiCornerRadiiState) -> ResolvedCornerRadii {
    ResolvedCornerRadii {
        tl: resolve_rect_corner_radius(rect, corner_radii.tl),
        tr: resolve_rect_corner_radius(rect, corner_radii.tr),
        br: resolve_rect_corner_radius(rect, corner_radii.br),
        bl: resolve_rect_corner_radius(rect, corner_radii.bl),
    }
}

fn has_any_radius(radii: ResolvedCornerRadii) -> bool {
    radii.tl > 0.0 || radii.tr > 0.0 || radii.br > 0.0 || radii.bl > 0.0
}

fn radii_to_corner_radius(rect: Rect, radii: ResolvedCornerRadii) -> CornerRadius {
    let max_radius = rect.width().min(rect.height()).max(0.0) * 0.5;
    let clamp = |v: f32| v.clamp(0.0, max_radius).min(u8::MAX as f32) as u8;
    CornerRadius {
        nw: clamp(radii.tl),
        ne: clamp(radii.tr),
        se: clamp(radii.br),
        sw: clamp(radii.bl),
    }
}

fn rounded_rect_points(
    rect: Rect,
    radii: ResolvedCornerRadii,
    segments: usize,
) -> Vec<epaint::Pos2> {
    let mut out = Vec::new();
    push_corner_points(
        &mut out,
        pos2(rect.right() - radii.tr, rect.top() + radii.tr),
        radii.tr,
        -90.0,
        0.0,
        segments,
        pos2(rect.right(), rect.top()),
    );
    push_corner_points(
        &mut out,
        pos2(rect.right() - radii.br, rect.bottom() - radii.br),
        radii.br,
        0.0,
        90.0,
        segments,
        pos2(rect.right(), rect.bottom()),
    );
    push_corner_points(
        &mut out,
        pos2(rect.left() + radii.bl, rect.bottom() - radii.bl),
        radii.bl,
        90.0,
        180.0,
        segments,
        pos2(rect.left(), rect.bottom()),
    );
    push_corner_points(
        &mut out,
        pos2(rect.left() + radii.tl, rect.top() + radii.tl),
        radii.tl,
        180.0,
        270.0,
        segments,
        pos2(rect.left(), rect.top()),
    );
    out
}

fn push_corner_points(
    out: &mut Vec<epaint::Pos2>,
    center: epaint::Pos2,
    radius: f32,
    start_deg: f32,
    end_deg: f32,
    segments: usize,
    fallback: epaint::Pos2,
) {
    if radius <= 0.0 {
        out.push(fallback);
        return;
    }
    for step in 0..=segments {
        let t = step as f32 / segments as f32;
        let angle = (start_deg + (end_deg - start_deg) * t).to_radians();
        out.push(pos2(
            center.x + angle.cos() * radius,
            center.y + angle.sin() * radius,
        ));
    }
}

fn add_rounded_rect_gradient(
    mesh: &mut Mesh,
    rect: Rect,
    radii: ResolvedCornerRadii,
    gradient: UiLinearGradientState,
) {
    let points = rounded_rect_points(rect, radii, 6);
    if points.len() < 3 {
        return;
    }
    let base = mesh.vertices.len() as u32;
    let center = rect.center();
    mesh.vertices.push(Vertex {
        pos: center,
        uv: pos2(0.0, 0.0),
        color: gradient_color(gradient, rect, center),
    });
    for pos in points {
        mesh.vertices.push(Vertex {
            pos,
            uv: pos2(0.0, 0.0),
            color: gradient_color(gradient, rect, pos),
        });
    }
    let point_count = mesh.vertices.len() as u32 - base - 1;
    for idx in 0..point_count {
        mesh.indices.extend_from_slice(&[
            base,
            base + idx + 1,
            base + ((idx + 1) % point_count) + 1,
        ]);
    }
}

fn gradient_color(gradient: UiLinearGradientState, rect: Rect, pos: epaint::Pos2) -> Color32 {
    let dir = vec2(gradient.vector[0], -gradient.vector[1]);
    let len = dir.length();
    let dir = if len <= 0.0001 || !len.is_finite() {
        vec2(0.0, -1.0)
    } else {
        dir / len
    };
    let rel = pos - rect.center();
    let extent = [
        vec2(rect.left() - rect.center().x, rect.top() - rect.center().y),
        vec2(rect.right() - rect.center().x, rect.top() - rect.center().y),
        vec2(
            rect.right() - rect.center().x,
            rect.bottom() - rect.center().y,
        ),
        vec2(
            rect.left() - rect.center().x,
            rect.bottom() - rect.center().y,
        ),
    ]
    .into_iter()
    .map(|v| v.dot(dir).abs())
    .fold(0.0_f32, f32::max)
    .max(1.0);
    let t = ((rel.dot(dir) / extent) * 0.5 + 0.5).clamp(0.0, 1.0);
    color32(lerp_color(gradient.start_color, gradient.end_color, t))
}

fn lerp_color(a: perro_structs::Color, b: perro_structs::Color, t: f32) -> perro_structs::Color {
    let [ar, ag, ab, aa] = a.to_rgba();
    let [br, bg, bb, ba] = b.to_rgba();
    perro_structs::Color::new(
        ar + (br - ar) * t,
        ag + (bg - ag) * t,
        ab + (bb - ab) * t,
        aa + (ba - aa) * t,
    )
}

struct TextShapeInput<'a> {
    rect: UiRectState,
    viewport: [f32; 2],
    clip_rect: Rect,
    text: &'a str,
    font_size: f32,
    color: perro_structs::Color,
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
    let mut job = LayoutJob::simple(
        text.to_string(),
        FontId::new(font_size, text_font_family(text, FontFamily::Proportional)),
        color32(color),
        rect.size[0].max(1.0),
    );
    job.halign = match h_align {
        UiTextAlignState::Start => Align::LEFT,
        UiTextAlignState::Center => Align::Center,
        UiTextAlignState::End => Align::RIGHT,
    };
    let galley = fonts.with_pixels_per_point(UI_RASTER_SCALE).layout_job(job);
    let text_size = galley.size();
    let x = match h_align {
        UiTextAlignState::Start => min[0],
        UiTextAlignState::Center => min[0] + rect.size[0] * 0.5,
        UiTextAlignState::End => max[0],
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

fn color32(color: perro_structs::Color) -> Color32 {
    let [r, g, b, a] = color.to_rgba_u8();
    Color32::from_rgba_unmultiplied(r, g, b, a)
}

fn valid_rect(rect: UiRectState) -> bool {
    rect.center.iter().all(|v| v.is_finite())
        && rect.size.iter().all(|v| v.is_finite())
        && rect.size[0] > 0.0
        && rect.size[1] > 0.0
}

fn valid_color(color: perro_structs::Color) -> bool {
    color.to_rgba().iter().all(|v| v.is_finite())
}

fn valid_gradient(gradient: UiLinearGradientState) -> bool {
    valid_color(gradient.start_color)
        && valid_color(gradient.end_color)
        && gradient.vector.iter().all(|v| v.is_finite())
}

fn valid_effect(effect: UiDepthEffectState) -> bool {
    valid_color(effect.color)
        && effect.color.a() > 0.0
        && effect.distance.is_finite()
        && effect.falloff.is_finite()
        && effect.vector.iter().all(|v| v.is_finite())
        && effect.size.is_finite()
        && (effect.distance > 0.0 || effect.falloff > 0.0 || effect.size > 0.0)
}

fn effect_offset(effect: UiDepthEffectState) -> epaint::Vec2 {
    let len = (effect.vector[0] * effect.vector[0] + effect.vector[1] * effect.vector[1]).sqrt();
    if !len.is_finite() || len <= 0.0001 {
        return vec2(0.0, 0.0);
    }
    vec2(
        effect.vector[0] / len * effect.distance.max(0.0),
        -effect.vector[1] / len * effect.distance.max(0.0),
    )
}

fn with_alpha(color: perro_structs::Color, alpha: f32) -> perro_structs::Color {
    let [r, g, b, _] = color.to_rgba();
    perro_structs::Color::new(r, g, b, alpha.clamp(0.0, 1.0))
}

fn effect_size_expand(rect: Rect, effect: UiDepthEffectState) -> f32 {
    rect.width().min(rect.height()).max(0.0) * 0.5 * (effect.size.max(0.0) - 1.0)
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

#[cfg(test)]
mod tests {
    use super::*;

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
                color: perro_structs::Color::WHITE,
                h_align: UiTextAlignState::Start,
                v_align: UiTextAlignState::Start,
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
                    color: perro_structs::Color::WHITE,
                    h_align,
                    v_align: UiTextAlignState::Start,
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
                color: perro_structs::Color::WHITE,
                h_align: UiTextAlignState::Center,
                v_align: UiTextAlignState::Center,
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
