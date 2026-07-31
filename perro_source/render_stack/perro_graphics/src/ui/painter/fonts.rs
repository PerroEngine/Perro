use super::*;

#[derive(Clone, Debug)]
pub(super) struct UiFontSource {
    pub(super) key: Arc<str>,
    pub(super) data: Arc<FontData>,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub(super) struct HarfBuzzGlyph {
    pub(super) glyph_id: u32,
    pub(super) cluster: u32,
    pub(super) x_advance: f32,
    pub(super) y_advance: f32,
    pub(super) x_offset: f32,
    pub(super) y_offset: f32,
}

#[derive(Clone, Debug, PartialEq)]
pub(super) struct HarfBuzzGlyphRun {
    pub(super) glyphs: Vec<HarfBuzzGlyph>,
}

#[derive(Clone, Copy, Debug)]
pub(super) struct HarfBuzzGlyphAlloc {
    pub(super) offset: epaint::Vec2,
    pub(super) size: epaint::Vec2,
    pub(super) uv_min: epaint::Pos2,
    pub(super) uv_max: epaint::Pos2,
}

/// Cached placement in atlas pixels. UVs stay out of the cache on purpose:
/// the atlas grows by doubling its height, which reinterprets every UV baked
/// against an older size. Pixels survive growth; UVs get derived per lookup.
#[derive(Clone, Copy, Debug)]
pub(super) struct HarfBuzzGlyphPixels {
    offset: epaint::Vec2,
    size: epaint::Vec2,
    pos: (usize, usize),
    extent: (usize, usize),
}

#[derive(Clone, Debug, Hash, PartialEq, Eq)]
pub(super) struct HarfBuzzGlyphKey {
    font_key: Arc<str>,
    glyph_id: u32,
    font_size_bits: u32,
}

/// Shaped runs are size-independent (advances are per-em); cache them by
/// family + text so unchanged labels skip re-parsing rustybuzz faces and
/// re-shaping on every UI rebuild. Misses (no font shapes the text) cache
/// too, so unshapeable labels don't retry the whole fallback list per frame.
pub(super) const HARFBUZZ_RUN_CACHE_LIMIT: usize = 1024;

/// Outer (per-family) run cache bound. Script fallback plus the selectable
/// system fonts give ~20 reachable families; keep the few in active use and
/// drop the coldest instead of holding `families * 1024` strings forever.
pub(super) const HARFBUZZ_FAMILY_CACHE_LIMIT: usize = 16;

/// Rebuild the atlas once it is this full, same threshold epaint's `Fonts`
/// uses. Past it `TextureAtlas` wraps its cursor and overwrites live glyphs.
pub(super) const HARFBUZZ_ATLAS_REBUILD_FILL: f32 = 0.8;

/// Raster sizes below this snap to a half-point grid, above it to whole
/// points. Animated / fit_content sizes otherwise mint one bitmap per pixel
/// of size change.
pub(super) const HARFBUZZ_RASTER_GRID_LIMIT: f32 = 24.0;

pub(super) type HarfBuzzShapedRun = Option<(UiFontSource, Arc<HarfBuzzGlyphRun>)>;

pub(super) struct HarfBuzzAtlas {
    /// Built on the first shaped glyph: pure-Latin apps never reach
    /// rustybuzz, and an eager atlas costs them the full CPU image, a
    /// same-size clone through the first delta, and a GPU texture.
    atlas: Option<TextureAtlas>,
    glyphs: AHashMap<HarfBuzzGlyphKey, HarfBuzzGlyphPixels>,
    runs: AHashMap<FontFamily, AHashMap<String, HarfBuzzShapedRun>>,
    /// Least-recently-used first; caps `runs`.
    family_order: Vec<FontFamily>,
    /// Bumped on every rebuild so callers holding baked UVs can tell that
    /// their glyph pixels moved.
    generation: u64,
}

impl HarfBuzzAtlas {
    pub(super) fn new() -> Self {
        Self {
            atlas: None,
            glyphs: AHashMap::new(),
            runs: AHashMap::new(),
            family_order: Vec::new(),
            generation: 0,
        }
    }

    /// Identity of the current glyph placement. Any change (rebuild or growth)
    /// invalidates UVs already tessellated into cached meshes.
    pub(super) fn epoch(&self) -> (u64, [usize; 2]) {
        (self.generation, self.size())
    }

    pub(super) fn size(&self) -> [usize; 2] {
        self.atlas
            .as_ref()
            .map(|atlas| atlas.size())
            .unwrap_or([0, 0])
    }

    /// Mirror of `Fonts::begin_pass`: recreate the atlas once it is nearly
    /// full instead of letting `TextureAtlas` wrap its cursor over live
    /// glyphs, which leaves the cached UVs pointing at other glyphs' pixels
    /// for good. Dropping the glyph map here is also what bounds it — the
    /// live glyphs re-raster on the next pass, the dead ones don't.
    pub(super) fn begin_pass(&mut self) {
        let full = self
            .atlas
            .as_ref()
            .is_some_and(|atlas| atlas.fill_ratio() > HARFBUZZ_ATLAS_REBUILD_FILL);
        if !full {
            return;
        }
        self.atlas = None;
        self.glyphs.clear();
        self.invalidate_runs();
        self.generation = self.generation.wrapping_add(1);
    }

    pub(super) fn take_delta(&mut self) -> Option<epaint::ImageDelta> {
        self.atlas.as_mut()?.take_delta()
    }

    /// Font set changed (resource font registered): cached runs may resolve
    /// to a different face now.
    pub(super) fn invalidate_runs(&mut self) {
        self.runs.clear();
        self.family_order.clear();
    }

    pub(super) fn shape_cached(
        &mut self,
        definitions: &FontDefinitions,
        family: FontFamily,
        text: &str,
    ) -> HarfBuzzShapedRun {
        self.touch_family(&family);
        let by_text = self.runs.entry(family.clone()).or_default();
        if let Some(cached) = by_text.get(text) {
            return cached.clone();
        }
        let shaped = shape_text_with_font_fallbacks(definitions, family, text)
            .map(|(font, run)| (font, Arc::new(run)));
        if by_text.len() >= HARFBUZZ_RUN_CACHE_LIMIT {
            by_text.clear();
        }
        by_text.insert(text.to_string(), shaped.clone());
        shaped
    }

    /// Move `family` to the hot end of the LRU, evicting the coldest family's
    /// whole run map when a new family pushes past the cap.
    fn touch_family(&mut self, family: &FontFamily) {
        if let Some(index) = self.family_order.iter().position(|held| held == family) {
            let held = self.family_order.remove(index);
            self.family_order.push(held);
            return;
        }
        if self.family_order.len() >= HARFBUZZ_FAMILY_CACHE_LIMIT {
            let evicted = self.family_order.remove(0);
            self.runs.remove(&evicted);
        }
        self.family_order.push(family.clone());
    }

    pub(super) fn glyph(
        &mut self,
        font: &UiFontSource,
        glyph_id: u32,
        font_size: f32,
    ) -> Option<HarfBuzzGlyphAlloc> {
        let raster_size = quantize_raster_size(font_size);
        let key = HarfBuzzGlyphKey {
            font_key: font.key.clone(),
            glyph_id,
            font_size_bits: raster_size.to_bits(),
        };
        let pixels = match self.glyphs.get(&key).copied() {
            Some(pixels) => pixels,
            None => {
                let atlas = self.atlas.get_or_insert_with(|| {
                    TextureAtlas::new(
                        [UI_HARFBUZZ_ATLAS_SIZE, UI_HARFBUZZ_ATLAS_INITIAL_HEIGHT],
                        AlphaFromCoverage::default(),
                    )
                });
                let pixels = raster_harfbuzz_glyph(font, glyph_id, raster_size, atlas)?;
                self.glyphs.insert(key, pixels);
                pixels
            }
        };
        // Quad geometry stays in the true size: only the bitmap resolution
        // quantizes, so a glyph mid-animation samples a slightly coarser or
        // finer bitmap but lands on the exact same pixels on screen.
        let scale = font_size / raster_size;
        Some(HarfBuzzGlyphAlloc {
            offset: pixels.offset * scale,
            size: pixels.size * scale,
            uv_min: self.uv(pixels.pos),
            uv_max: self.uv((
                pixels.pos.0 + pixels.extent.0,
                pixels.pos.1 + pixels.extent.1,
            )),
        })
    }

    fn uv(&self, pos: (usize, usize)) -> epaint::Pos2 {
        let size = self.size();
        if size[0] == 0 || size[1] == 0 {
            return pos2(0.0, 0.0);
        }
        pos2(pos.0 as f32 / size[0] as f32, pos.1 as f32 / size[1] as f32)
    }

    #[cfg(test)]
    pub(super) fn glyph_count(&self) -> usize {
        self.glyphs.len()
    }

    #[cfg(test)]
    pub(super) fn family_count(&self) -> usize {
        self.runs.len()
    }

    /// Swap in a filled stand-in atlas. Reaching the threshold on the real
    /// 4096-wide atlas would materialize a 64 MiB image just to trip a check.
    #[cfg(test)]
    pub(super) fn fill_atlas_for_test(&mut self) {
        let mut atlas = TextureAtlas::new([1024, 32], AlphaFromCoverage::default());
        while atlas.fill_ratio() <= HARFBUZZ_ATLAS_REBUILD_FILL {
            atlas.allocate((512, 64));
        }
        self.atlas = Some(atlas);
    }
}

/// Snap a raster size onto a coarse grid so a size sweep (animation,
/// `fit_content` deriving size from rect dimensions) reuses bitmaps instead of
/// rasterizing and allocating a fresh atlas entry per pixel of change. Same
/// trade epaint makes when it rounds glyph scale to whole pixels.
pub(super) fn quantize_raster_size(font_size: f32) -> f32 {
    if !font_size.is_finite() || font_size <= 0.0 {
        return 1.0;
    }
    if font_size < HARFBUZZ_RASTER_GRID_LIMIT {
        ((font_size * 2.0).round() / 2.0).max(0.5)
    } else {
        font_size.round()
    }
}

pub(super) fn default_ui_font_definitions() -> FontDefinitions {
    let mut definitions = FontDefinitions::default();
    // One system scan shared by the script-fallback and selectable passes;
    // scanning font directories twice doubled startup cost for nothing.
    let mut db = fontdb::Database::new();
    db.load_system_fonts();
    append_system_font_fallbacks(&mut definitions, &mut db);
    append_selectable_system_fonts(&mut definitions, &mut db);
    definitions
}

pub(super) fn append_selectable_system_fonts(
    definitions: &mut FontDefinitions,
    db: &mut fontdb::Database,
) {
    let serif_family = ui_font_family(
        &UiFont::System(UiSystemFont::Serif),
        FontFamily::Proportional,
    );
    append_default_family_fallbacks(definitions, serif_family.clone(), FontFamily::Proportional);
    let serif_query = fontdb::Query {
        families: &[fontdb::Family::Serif],
        ..Default::default()
    };
    if let Some(id) = db.query(&serif_query)
        && let Some(data) = shared_font_data(db, id)
    {
        definitions
            .font_data
            .insert("perro-select-serif".into(), Arc::new(data));
        definitions
            .families
            .entry(serif_family)
            .or_default()
            .insert(0, "perro-select-serif".into());
    }
    for font in [
        UiSystemFont::Arial,
        UiSystemFont::Calibri,
        UiSystemFont::Cambria,
        UiSystemFont::Consolas,
        UiSystemFont::CourierNew,
        UiSystemFont::Georgia,
        UiSystemFont::Helvetica,
        UiSystemFont::SegoeUi,
        UiSystemFont::TimesNewRoman,
        UiSystemFont::Verdana,
    ] {
        let family = ui_font_family(&UiFont::System(font), FontFamily::Proportional);
        append_default_family_fallbacks(definitions, family.clone(), FontFamily::Proportional);
        if let Some(name) = font.family_name() {
            let query = fontdb::Query {
                families: &[fontdb::Family::Name(name)],
                ..Default::default()
            };
            let Some(id) = db.query(&query) else {
                continue;
            };
            let Some(index) = db.face(id).map(|face| face.index) else {
                continue;
            };
            // Same key scheme as the script-fallback pass so a face both
            // passes want (Segoe UI, Arial, ...) registers once.
            let key = format!("{UI_SYSTEM_FONT_PREFIX}-{name}-{index}");
            if !definitions.font_data.contains_key(&key) {
                let Some(data) = shared_font_data(db, id) else {
                    continue;
                };
                definitions.font_data.insert(key.clone(), Arc::new(data));
            }
            definitions
                .families
                .entry(family)
                .or_default()
                .insert(0, key);
        }
    }
}

pub(super) fn ui_font_family(font: &UiFont, default: FontFamily) -> FontFamily {
    match font {
        UiFont::Default => default,
        UiFont::Resource(path) => resource_font_family(path),
        UiFont::System(UiSystemFont::SansSerif) => FontFamily::Proportional,
        UiFont::System(UiSystemFont::Monospace) => FontFamily::Monospace,
        UiFont::System(font) => system_font_family(*font),
    }
}

/// Cached per-variant families: this runs per text shape per rebuild, and
/// building `FontFamily::Name` fresh costs a format! + Arc allocation.
pub(super) fn system_font_family(font: UiSystemFont) -> FontFamily {
    use std::sync::OnceLock;
    static FAMILIES: OnceLock<AHashMap<UiSystemFont, FontFamily>> = OnceLock::new();
    let families = FAMILIES.get_or_init(|| {
        let mut map = AHashMap::new();
        map.insert(
            UiSystemFont::Serif,
            FontFamily::Name(Arc::from("perro-system-serif")),
        );
        for font in [
            UiSystemFont::Arial,
            UiSystemFont::Calibri,
            UiSystemFont::Cambria,
            UiSystemFont::Consolas,
            UiSystemFont::CourierNew,
            UiSystemFont::Georgia,
            UiSystemFont::Helvetica,
            UiSystemFont::SegoeUi,
            UiSystemFont::TimesNewRoman,
            UiSystemFont::Verdana,
        ] {
            map.insert(
                font,
                FontFamily::Name(Arc::from(format!("perro-select-{font:?}"))),
            );
        }
        map
    });
    families
        .get(&font)
        .cloned()
        .unwrap_or(FontFamily::Proportional)
}

pub(super) fn resource_font_family(path: &str) -> FontFamily {
    FontFamily::Name(Arc::from(format!(
        "perro-resource-family-{}",
        perro_ids::string_to_u64(path)
    )))
}

pub(super) fn selected_text_family(text: &str, font: &UiFont, default: FontFamily) -> FontFamily {
    if matches!(font, UiFont::Default) {
        text_font_family(text, default)
    } else {
        ui_font_family(font, default)
    }
}

pub(super) fn append_system_font_fallbacks(
    definitions: &mut FontDefinitions,
    db: &mut fontdb::Database,
) {
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
            append_system_font_family(definitions, db, name, font_family.clone());
        }
    }

    for (target_family, _) in &script_families {
        for (source_family, _) in &script_families {
            append_family_fallbacks(definitions, target_family.clone(), source_family.clone());
        }
    }
}

pub(super) fn named_font_family(name: &'static str) -> FontFamily {
    FontFamily::Name(Arc::from(name))
}

pub(super) fn append_system_font_family(
    definitions: &mut FontDefinitions,
    db: &mut fontdb::Database,
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
    let Some(index) = db.face(id).map(|face| face.index) else {
        return;
    };
    let font_key = format!("{UI_SYSTEM_FONT_PREFIX}-{family_name}-{index}");
    if !definitions.font_data.contains_key(&font_key) {
        let Some(font_data) = shared_font_data(db, id) else {
            return;
        };
        definitions
            .font_data
            .insert(font_key.clone(), Arc::new(font_data));
    }
    append_font_fallback(definitions, target_family, &font_key);
}

/// Memory-map a system face instead of copying the file onto the heap.
/// System fonts (CJK families especially) total tens of MB; mapping defers
/// all I/O to on-demand page faults and keeps the bytes file-backed and
/// evictable instead of resident for the app's lifetime.
pub(super) fn shared_font_data(db: &mut fontdb::Database, id: fontdb::ID) -> Option<FontData> {
    // SAFETY: the mapping is leaked below and never unmapped, so the slice
    // stays valid for the process lifetime. A font file changing on disk
    // mid-run could corrupt the mapping; every desktop text stack accepts
    // this same trade for system fonts.
    let (data, index) = unsafe { db.make_shared_face_data(id) }?;
    let slice: &[u8] = (*data).as_ref();
    let slice: &'static [u8] = unsafe { std::slice::from_raw_parts(slice.as_ptr(), slice.len()) };
    // Fonts register once and live until exit; leaking the Arc is the bound.
    std::mem::forget(data);
    Some(FontData {
        font: std::borrow::Cow::Borrowed(slice),
        index,
        tweak: Default::default(),
    })
}

pub(super) fn append_default_family_fallbacks(
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

pub(super) fn append_family_fallbacks(
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

pub(super) fn append_font_fallback(
    definitions: &mut FontDefinitions,
    family: FontFamily,
    font_key: &str,
) {
    let list = definitions.families.entry(family).or_default();
    if !list.iter().any(|name| name == font_key) {
        list.push(font_key.to_owned());
    }
}

pub(super) fn text_font_family(text: &str, default_family: FontFamily) -> FontFamily {
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

pub(super) fn is_han(ch: char) -> bool {
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

pub(super) fn is_kana(ch: char) -> bool {
    matches!(ch as u32, 0x3040..=0x30FF | 0x31F0..=0x31FF)
}

pub(super) fn is_hangul(ch: char) -> bool {
    matches!(ch as u32, 0x1100..=0x11FF | 0x3130..=0x318F | 0xAC00..=0xD7AF)
}

pub(super) fn is_cyrillic(ch: char) -> bool {
    matches!(ch as u32, 0x0400..=0x052F | 0x2DE0..=0x2DFF | 0xA640..=0xA69F)
}

pub(super) fn is_arabic(ch: char) -> bool {
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

pub(super) fn is_hebrew(ch: char) -> bool {
    matches!(ch as u32, 0x0590..=0x05FF | 0xFB1D..=0xFB4F)
}

pub(super) fn is_indic(ch: char) -> bool {
    matches!(ch as u32, 0x0900..=0x0DFF)
}

pub(super) fn is_thai(ch: char) -> bool {
    matches!(ch as u32, 0x0E00..=0x0E7F)
}

pub(super) fn is_se_asian(ch: char) -> bool {
    matches!(
        ch as u32,
        0x0E80..=0x0EFF | 0x0F00..=0x0FFF | 0x1000..=0x109F | 0x1780..=0x17FF
    )
}

pub(super) fn needs_harfbuzz(text: &str) -> bool {
    text.chars()
        .any(|ch| is_arabic(ch) || is_hebrew(ch) || is_indic(ch) || is_thai(ch) || is_se_asian(ch))
}

pub(super) fn font_sources_for_family(
    definitions: &FontDefinitions,
    family: FontFamily,
) -> Vec<UiFontSource> {
    definitions
        .families
        .get(&family)
        .into_iter()
        .flatten()
        .filter_map(|key| {
            definitions.font_data.get(key).map(|data| UiFontSource {
                key: Arc::from(key.as_str()),
                data: data.clone(),
            })
        })
        .collect()
}

pub(super) fn shape_text_with_harfbuzz(
    font: &UiFontSource,
    text: &str,
) -> Option<HarfBuzzGlyphRun> {
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

pub(super) fn shape_text_with_font_fallbacks(
    definitions: &FontDefinitions,
    family: FontFamily,
    text: &str,
) -> Option<(UiFontSource, HarfBuzzGlyphRun)> {
    font_sources_for_family(definitions, family)
        .into_iter()
        .find_map(|font| shape_text_with_harfbuzz(&font, text).map(|run| (font, run)))
}

pub(super) fn raster_harfbuzz_glyph(
    font: &UiFontSource,
    glyph_id: u32,
    font_size: f32,
    atlas: &mut TextureAtlas,
) -> Option<HarfBuzzGlyphPixels> {
    let font_ref = ab_glyph::FontRef::try_from_slice(font.data.font.as_ref()).ok()?;
    let glyph_id = ab_glyph::GlyphId(glyph_id.min(u16::MAX as u32) as u16);
    let glyph =
        glyph_id.with_scale_and_position(font_size * UI_RASTER_SCALE, ab_glyph::point(0.0, 0.0));
    let outlined = font_ref.outline_glyph(glyph)?;
    let bounds = outlined.px_bounds();
    let width = bounds.width().ceil().max(0.0) as usize;
    let height = bounds.height().ceil().max(0.0) as usize;
    if width == 0 || height == 0 {
        return Some(HarfBuzzGlyphPixels {
            offset: vec2(0.0, 0.0),
            size: vec2(0.0, 0.0),
            pos: (0, 0),
            extent: (0, 0),
        });
    }
    let (glyph_pos, image) = atlas.allocate((width, height));
    outlined.draw(|x, y, coverage| {
        if coverage > 0.0 {
            image[(glyph_pos.0 + x as usize, glyph_pos.1 + y as usize)] =
                Color32::from_white_alpha((coverage * 255.0).round().clamp(0.0, 255.0) as u8);
        }
    });
    Some(HarfBuzzGlyphPixels {
        offset: vec2(bounds.min.x, bounds.min.y) / UI_RASTER_SCALE,
        size: vec2(width as f32, height as f32) / UI_RASTER_SCALE,
        pos: glyph_pos,
        extent: (width, height),
    })
}

pub(super) fn font_texture_size(fonts: &Fonts) -> [u32; 2] {
    let size = fonts.font_image_size();
    [
        size[0].min(u32::MAX as usize) as u32,
        size[1].min(u32::MAX as usize) as u32,
    ]
}
