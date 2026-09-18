use super::*;

pub(super) fn screen_center(rect: UiRectState, viewport: [f32; 2]) -> epaint::Pos2 {
    pos2(
        viewport[0] * 0.5 + rect.center[0],
        viewport[1] * 0.5 - rect.center[1],
    )
}

pub(super) fn screen_pivot(rect: UiRectState, viewport: [f32; 2]) -> epaint::Pos2 {
    let center = screen_center(rect, viewport);
    pos2(
        center.x + (rect.pivot[0] - 0.5) * rect.size[0],
        center.y - (rect.pivot[1] - 0.5) * rect.size[1],
    )
}

pub(super) fn resolve_rect_corner_radius(rect: Rect, corner_radius: f32) -> f32 {
    let ratio = if corner_radius.is_finite() {
        corner_radius.clamp(0.0, 1.0)
    } else {
        1.0
    };
    (rect.width().min(rect.height()).max(0.0) * 0.5 * ratio).min(u8::MAX as f32)
}

#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub(super) struct ResolvedCornerRadii {
    pub(super) tl: f32,
    pub(super) tr: f32,
    pub(super) br: f32,
    pub(super) bl: f32,
}

pub(super) fn resolve_corner_radii(panel: &UiPanelDraw, rect: Rect) -> ResolvedCornerRadii {
    resolve_rect_corner_radii(rect, panel.corner_radii)
}

pub(super) fn resolve_rect_corner_radii(
    rect: Rect,
    corner_radii: UiCornerRadiiState,
) -> ResolvedCornerRadii {
    ResolvedCornerRadii {
        tl: resolve_rect_corner_radius(rect, corner_radii.tl),
        tr: resolve_rect_corner_radius(rect, corner_radii.tr),
        br: resolve_rect_corner_radius(rect, corner_radii.br),
        bl: resolve_rect_corner_radius(rect, corner_radii.bl),
    }
}

pub(super) fn has_any_radius(radii: ResolvedCornerRadii) -> bool {
    radii.tl > 0.0 || radii.tr > 0.0 || radii.br > 0.0 || radii.bl > 0.0
}

pub(super) fn radii_to_corner_radius(rect: Rect, radii: ResolvedCornerRadii) -> CornerRadius {
    let max_radius = rect.width().min(rect.height()).max(0.0) * 0.5;
    let clamp = |v: f32| v.clamp(0.0, max_radius).min(u8::MAX as f32) as u8;
    CornerRadius {
        nw: clamp(radii.tl),
        ne: clamp(radii.tr),
        se: clamp(radii.br),
        sw: clamp(radii.bl),
    }
}

pub(super) fn rounded_rect_segments(rect: Rect, radii: ResolvedCornerRadii) -> usize {
    let radius = radii
        .tl
        .max(radii.tr)
        .max(radii.br)
        .max(radii.bl)
        .min(rect.width().min(rect.height()) * 0.5);
    (radius * 0.45).ceil().clamp(6.0, 18.0) as usize
}

pub(super) fn rounded_rect_points(
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

/// Inner/outer rings around a rounded-rect edge for one output-pixel AA.
/// The UI target is fixed at 1x; half the ramp lands on each side of the
/// authored boundary. Custom `Shape::Mesh` inputs bypass epaint feathering,
/// so textured images and gradient panels use this explicitly.
pub(super) fn rounded_rect_feather_rings(
    rect: Rect,
    radii: ResolvedCornerRadii,
) -> Vec<(epaint::Pos2, epaint::Pos2)> {
    let mut points = rounded_rect_points(rect, radii, rounded_rect_segments(rect, radii));
    points.dedup_by(|a, b| (*a - *b).length_sq() <= 1.0e-8);
    if points.len() > 1 && (points[0] - points[points.len() - 1]).length_sq() <= 1.0e-8 {
        points.pop();
    }
    if points.len() < 3 {
        return Vec::new();
    }
    let edge_normal = |a: epaint::Pos2, b: epaint::Pos2| {
        let edge = b - a;
        let len = edge.length();
        if len > 1.0e-6 {
            vec2(edge.y / len, -edge.x / len)
        } else {
            vec2(0.0, 0.0)
        }
    };
    let half = 0.5;
    let count = points.len();
    points
        .iter()
        .enumerate()
        .map(|(idx, &point)| {
            let prev = points[(idx + count - 1) % count];
            let next = points[(idx + 1) % count];
            let prev_normal = edge_normal(prev, point);
            let next_normal = edge_normal(point, next);
            let sum = prev_normal + next_normal;
            let sum_len = sum.length();
            let outward = if sum_len > 1.0e-6 {
                sum / sum_len
            } else {
                let radial = point - rect.center();
                let radial_len = radial.length();
                if radial_len > 1.0e-6 {
                    radial / radial_len
                } else {
                    vec2(0.0, -1.0)
                }
            };
            // Preserve a half-pixel perpendicular offset at corners. Cap the
            // miter for degenerate/tiny geometry.
            let denom = outward.dot(next_normal).abs().max(0.25);
            let offset = outward * (half / denom).min(half * 2.0);
            (point - offset, point + offset)
        })
        .collect()
}

pub(super) fn add_feathered_rounded_rect(
    mesh: &mut Mesh,
    rect: Rect,
    radii: ResolvedCornerRadii,
    mut vertex: impl FnMut(epaint::Pos2, bool) -> Vertex,
) {
    let rings = rounded_rect_feather_rings(rect, radii);
    if rings.len() < 3 {
        return;
    }
    let base = mesh.vertices.len() as u32;
    mesh.vertices.push(vertex(rect.center(), true));
    for &(inner, _) in &rings {
        mesh.vertices.push(vertex(inner, true));
    }
    for &(_, outer) in &rings {
        mesh.vertices.push(vertex(outer, false));
    }
    let count = rings.len() as u32;
    let inner = base + 1;
    let outer = inner + count;
    for idx in 0..count {
        let next = (idx + 1) % count;
        mesh.indices
            .extend_from_slice(&[base, inner + idx, inner + next]);
        mesh.indices.extend_from_slice(&[
            inner + idx,
            outer + idx,
            outer + next,
            inner + idx,
            outer + next,
            inner + next,
        ]);
    }
}

pub(super) fn push_corner_points(
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

pub(super) fn add_rounded_rect_gradient(
    mesh: &mut Mesh,
    rect: Rect,
    radii: ResolvedCornerRadii,
    gradient: UiLinearGradientState,
) {
    add_feathered_rounded_rect(mesh, rect, radii, |pos, opaque| Vertex {
        pos,
        uv: pos2(0.0, 0.0),
        color: if opaque {
            gradient_color(gradient, rect, pos)
        } else {
            Color32::TRANSPARENT
        },
    });
}

pub(super) fn gradient_color(
    gradient: UiLinearGradientState,
    rect: Rect,
    pos: epaint::Pos2,
) -> Color32 {
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

pub(super) fn lerp_color(
    a: perro_structs::Color,
    b: perro_structs::Color,
    t: f32,
) -> perro_structs::Color {
    let [ar, ag, ab, aa] = a.to_rgba();
    let [br, bg, bb, ba] = b.to_rgba();
    perro_structs::Color::new(
        ar + (br - ar) * t,
        ag + (bg - ag) * t,
        ab + (bb - ab) * t,
        aa + (ba - aa) * t,
    )
}

pub(super) struct TextShapeInput<'a> {
    pub(super) rect: UiRectState,
    pub(super) viewport: [f32; 2],
    pub(super) clip_rect: Rect,
    pub(super) text: &'a str,
    pub(super) font_size: f32,
    pub(super) raster_font_size: Option<f32>,
    pub(super) font: &'a UiFont,
    pub(super) wrap_width: Option<f32>,
    pub(super) color: perro_structs::Color,
    pub(super) h_align: UiTextAlignState,
    pub(super) v_align: UiTextAlignState,
    pub(super) fit_content: bool,
}

pub(super) fn push_text_shape(
    input: TextShapeInput<'_>,
    fonts: &mut Fonts,
    out: &mut Vec<ClippedShape>,
) {
    let TextShapeInput {
        rect,
        viewport,
        clip_rect,
        text,
        mut font_size,
        raster_font_size,
        font,
        wrap_width,
        color,
        h_align,
        v_align,
        fit_content,
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
    let wrap_width = wrap_width
        .filter(|width| width.is_finite() && *width > 0.0)
        .unwrap_or(rect.size[0])
        .max(1.0);
    // Opt-in labels keep a stable raster size while their retained geometry
    // keeps the requested size. None preserves the existing epaint path.
    let raster_font_size = raster_font_size.filter(|size| size.is_finite() && *size > 0.0);
    let stable_raster = raster_font_size.is_some();
    let raster_font_size = raster_font_size.unwrap_or(font_size);
    let mut font_id = FontId::new(
        raster_font_size,
        selected_text_family(text, font, FontFamily::Proportional),
    );
    let paragraph_align = match h_align {
        UiTextAlignState::Start => Align::LEFT,
        UiTextAlignState::Center => Align::Center,
        UiTextAlignState::End => Align::RIGHT,
    };
    // Start at the authored width. This keeps ordinary one-line labels on
    // one galley cache key while the requested geometry scale animates.
    // Multi-line or overflowing labels relayout at the scaled width below.
    let mut layout_wrap_width = wrap_width;
    let layout = |fonts: &mut Fonts, font_id: FontId, wrap_width: f32| {
        let mut job = LayoutJob::simple(text.to_string(), font_id, color32(color), wrap_width);
        job.halign = paragraph_align;
        fonts.with_pixels_per_point(UI_RASTER_SCALE).layout_job(job)
    };
    let mut galley = layout(fonts, font_id.clone(), layout_wrap_width);
    let initial_geometry_scale = if stable_raster {
        font_size / raster_font_size
    } else {
        1.0
    };
    if stable_raster
        && (initial_geometry_scale - 1.0).abs() > f32::EPSILON
        && (galley.rows.len() > 1 || galley.size().x * initial_geometry_scale > wrap_width)
    {
        layout_wrap_width = wrap_width / initial_geometry_scale;
        galley = layout(fonts, font_id.clone(), layout_wrap_width);
    }
    // Keep whole words whole. Shrink a word that cannot fit instead of
    // splitting it across rows or letting it escape the label's world size.
    // A single row that fits proves no word overflowed, so the common case
    // (short label, no wrap) skips the per-word measuring entirely.
    if galley.rows.len() > 1 || galley.size().x > layout_wrap_width {
        let longest_word_width =
            measure_longest_word_width(text, fonts, font_id.clone(), color32(color));
        if longest_word_width > layout_wrap_width {
            font_size *= layout_wrap_width / longest_word_width;
            font_id.size = if stable_raster {
                raster_font_size
            } else {
                font_size.max(0.001)
            };
            layout_wrap_width = if stable_raster {
                wrap_width * raster_font_size / font_size
            } else {
                wrap_width
            };
            galley = layout(fonts, font_id.clone(), layout_wrap_width);
        }
    }
    let raster_scale = if stable_raster {
        font_size / raster_font_size
    } else {
        1.0
    };
    if fit_content && galley.size().y * raster_scale > rect.size[1] {
        font_size *= (rect.size[1] / (galley.size().y * raster_scale)).clamp(0.001, 1.0);
        font_id.size = if stable_raster {
            raster_font_size
        } else {
            font_size
        };
        layout_wrap_width = if stable_raster {
            wrap_width * raster_font_size / font_size
        } else {
            wrap_width
        };
        galley = layout(fonts, font_id.clone(), layout_wrap_width);
    }
    let geometry_scale = if stable_raster {
        let scale = font_size / raster_font_size;
        if scale.is_finite() && scale > 0.0 {
            scale
        } else {
            1.0
        }
    } else {
        1.0
    };
    let text_size = galley.size() * geometry_scale;
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
    let pos = pos2(x, y);
    let mut text_shape = epaint::TextShape::new(pos, galley, color32(color))
        .with_override_text_color(color32(color));
    if stable_raster && (geometry_scale - 1.0).abs() > f32::EPSILON {
        // Scale galley mesh around its draw anchor, so position / alignment
        // stay fixed while glyphs use one stable atlas size.
        text_shape.transform(TSTransform::new(
            pos.to_vec2() * (1.0 - geometry_scale),
            geometry_scale,
        ));
    }
    out.push(ClippedShape {
        clip_rect,
        shape: Shape::Text(text_shape),
    });
}

fn measure_longest_word_width(
    text: &str,
    fonts: &mut Fonts,
    font_id: FontId,
    color: Color32,
) -> f32 {
    let mut words = String::with_capacity(text.len());
    for word in text.split_whitespace() {
        if !words.is_empty() {
            words.push('\n');
        }
        words.push_str(word);
    }
    if words.is_empty() {
        return 0.0;
    }
    let job = LayoutJob::simple(words, font_id, color, f32::INFINITY);
    fonts
        .with_pixels_per_point(UI_RASTER_SCALE)
        .layout_job(job)
        .rows
        .iter()
        .map(|row| row.size.x)
        .fold(0.0_f32, f32::max)
}

pub(super) fn clip_rect_from_state(clip: [f32; 4], viewport: [f32; 2]) -> Rect {
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

pub(super) fn color32(color: perro_structs::Color) -> Color32 {
    let [r, g, b, a] = color.to_rgba_u8();
    Color32::from_rgba_unmultiplied(r, g, b, a)
}

pub(super) fn valid_rect(rect: UiRectState) -> bool {
    rect.center.iter().all(|v| v.is_finite())
        && rect.size.iter().all(|v| v.is_finite())
        && rect.size[0] > 0.0
        && rect.size[1] > 0.0
}

pub(super) fn valid_color(color: perro_structs::Color) -> bool {
    color.to_rgba().iter().all(|v| v.is_finite())
}

pub(super) fn valid_gradient(gradient: UiLinearGradientState) -> bool {
    valid_color(gradient.start_color)
        && valid_color(gradient.end_color)
        && gradient.vector.iter().all(|v| v.is_finite())
}

pub(super) fn valid_effect(effect: UiDepthEffectState) -> bool {
    valid_color(effect.color)
        && effect.color.a() > 0.0
        && effect.distance.is_finite()
        && effect.falloff.is_finite()
        && effect.vector.iter().all(|v| v.is_finite())
        && effect.size.is_finite()
        && (effect.distance > 0.0 || effect.falloff > 0.0 || effect.size > 0.0)
}

pub(super) fn effect_offset(effect: UiDepthEffectState) -> epaint::Vec2 {
    let len = (effect.vector[0] * effect.vector[0] + effect.vector[1] * effect.vector[1]).sqrt();
    if !len.is_finite() || len <= 0.0001 {
        return vec2(0.0, 0.0);
    }
    vec2(
        effect.vector[0] / len * effect.distance.max(0.0),
        -effect.vector[1] / len * effect.distance.max(0.0),
    )
}

pub(super) fn with_alpha(color: perro_structs::Color, alpha: f32) -> perro_structs::Color {
    let [r, g, b, _] = color.to_rgba();
    perro_structs::Color::new(r, g, b, alpha.clamp(0.0, 1.0))
}

pub(super) fn effect_size_expand(rect: Rect, effect: UiDepthEffectState) -> f32 {
    rect.width().min(rect.height()).max(0.0) * 0.5 * (effect.size.max(0.0) - 1.0)
}

pub(super) fn viewport_rect(viewport: [f32; 2]) -> Rect {
    Rect::from_min_size(pos2(0.0, 0.0), vec2(viewport[0], viewport[1]))
}

#[derive(Clone, Copy)]
pub(super) struct GalleyTextRow {
    pub(super) index: usize,
    pub(super) start: usize,
    pub(super) end: usize,
}

pub(super) fn galley_text_rows(text: &str, galley: &Galley) -> Vec<GalleyTextRow> {
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

pub(super) fn byte_col(text: &str, start: usize, end: usize) -> usize {
    let start = clamp_char_boundary(text, start);
    let end = clamp_char_boundary(text, end);
    text[start.min(end)..end].chars().count()
}

pub(super) fn advance_chars(text: &str, start: usize, count: usize) -> usize {
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

pub(super) fn clamp_char_boundary(text: &str, mut index: usize) -> usize {
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
    fn longest_word_measure_matches_individual_layouts() {
        let text = "short much-longer middle\nwide";
        let font_id = FontId::new(16.0, FontFamily::Proportional);
        let color = Color32::WHITE;

        let mut compact_fonts = Fonts::new(
            UI_FONT_ATLAS_SIZE,
            AlphaFromCoverage::default(),
            default_ui_font_definitions(),
        );
        compact_fonts.begin_pass(UI_FONT_ATLAS_SIZE, AlphaFromCoverage::default());
        let compact = measure_longest_word_width(text, &mut compact_fonts, font_id.clone(), color);

        let mut reference_fonts = Fonts::new(
            UI_FONT_ATLAS_SIZE,
            AlphaFromCoverage::default(),
            default_ui_font_definitions(),
        );
        reference_fonts.begin_pass(UI_FONT_ATLAS_SIZE, AlphaFromCoverage::default());
        let reference = text
            .split_whitespace()
            .map(|word| {
                reference_fonts
                    .with_pixels_per_point(UI_RASTER_SCALE)
                    .layout(word.to_owned(), font_id.clone(), color, f32::INFINITY)
                    .size()
                    .x
            })
            .fold(0.0_f32, f32::max);

        assert!((compact - reference).abs() < 1.0e-4);
    }
}
