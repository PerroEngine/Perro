use super::*;

pub(super) fn ui_rect(draw: &UiDraw) -> UiRectState {
    match draw {
        UiDraw::Panel(panel) => panel.rect,
        UiDraw::ProgressBar(progress) => progress.rect,
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

pub(super) fn push_ui_shape(shape: &UiShapeDraw, viewport: [f32; 2], out: &mut Vec<ClippedShape>) {
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

pub(super) fn push_color_wheel_shape(
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
        wheel.mode,
        wheel.selected,
        out,
    );
}

pub(super) fn push_checkbox_shapes(
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

pub(super) fn push_color_wheel(
    center: epaint::Pos2,
    radius: f32,
    clip_rect: Rect,
    mode: UiColorPickerMode,
    selected: perro_structs::Color,
    out: &mut Vec<ClippedShape>,
) {
    if matches!(mode, UiColorPickerMode::Swatches) {
        push_color_swatches(center, radius, selected, clip_rect, out);
        return;
    }
    let steps = if matches!(mode, UiColorPickerMode::BlockWheel) {
        12
    } else {
        96
    };
    let rings = if matches!(mode, UiColorPickerMode::BlockWheel) {
        4
    } else {
        12
    };
    let mut mesh = Mesh::default();
    for ring in 0..rings {
        let inner = ring as f32 / rings as f32;
        let outer = (ring + 1) as f32 / rings as f32;
        for idx in 0..steps {
            let h0 = idx as f32 / steps as f32;
            let h1 = (idx + 1) as f32 / steps as f32;
            let a0 = h0 * std::f32::consts::TAU;
            let a1 = h1 * std::f32::consts::TAU;
            let base = mesh.vertices.len() as u32;
            for (angle, saturation) in [(a0, inner), (a1, inner), (a1, outer), (a0, outer)] {
                let color = if matches!(mode, UiColorPickerMode::BlockWheel) {
                    hsv_color(
                        (idx as f32 + 0.5) / steps as f32,
                        (ring as f32 + 0.5) / rings as f32,
                        1.0,
                    )
                } else {
                    hsv_color(angle / std::f32::consts::TAU, saturation, 1.0)
                };
                mesh.vertices.push(Vertex {
                    pos: center + vec2(angle.cos(), -angle.sin()) * radius * saturation,
                    uv: pos2(0.0, 0.0),
                    color: color32(color),
                });
            }
            mesh.indices
                .extend_from_slice(&[base, base + 1, base + 2, base, base + 2, base + 3]);
        }
    }
    out.push(ClippedShape {
        clip_rect,
        shape: Shape::Mesh(mesh.into()),
    });
    out.push(ClippedShape {
        clip_rect,
        shape: Shape::Circle(CircleShape {
            center,
            radius,
            fill: Color32::TRANSPARENT,
            stroke: Stroke::new(1.0, Color32::from_white_alpha(90)),
        }),
    });
    let (hue, saturation, _) = rgb_to_hsv(selected);
    let angle = hue * std::f32::consts::TAU;
    let marker = center + vec2(angle.cos(), -angle.sin()) * radius * saturation;
    push_color_marker(marker, clip_rect, out);
}

pub(super) fn push_color_swatches(
    center: epaint::Pos2,
    radius: f32,
    selected: perro_structs::Color,
    clip_rect: Rect,
    out: &mut Vec<ClippedShape>,
) {
    let bounds = Rect::from_center_size(center, vec2(radius * 2.0, radius * 2.0));
    let swatches = perro_render_bridge::ui_color_picker_swatches();
    let gap = 4.0;
    let cell = vec2(
        (bounds.width() - gap * 5.0) / 6.0,
        (bounds.height() - gap * 3.0) / 4.0,
    );
    let mut best = (f32::INFINITY, bounds.center());
    for (idx, color) in swatches.into_iter().enumerate() {
        let col = (idx % 6) as f32;
        let row = (idx / 6) as f32;
        let min = bounds.min + vec2(col * (cell.x + gap), row * (cell.y + gap));
        let rect = Rect::from_min_size(min, cell);
        out.push(ClippedShape {
            clip_rect,
            shape: Shape::Rect(RectShape::new(
                rect,
                CornerRadius::same(4),
                color32(color),
                Stroke::new(1.0, Color32::from_black_alpha(80)),
                StrokeKind::Inside,
            )),
        });
        let distance = (color.r() - selected.r()).powi(2)
            + (color.g() - selected.g()).powi(2)
            + (color.b() - selected.b()).powi(2);
        if distance < best.0 {
            best = (distance, rect.center());
        }
    }
    push_color_marker(best.1, clip_rect, out);
}

pub(super) fn push_color_marker(pos: epaint::Pos2, clip_rect: Rect, out: &mut Vec<ClippedShape>) {
    out.push(ClippedShape {
        clip_rect,
        shape: Shape::Circle(CircleShape {
            center: pos,
            radius: 7.0,
            fill: Color32::TRANSPARENT,
            stroke: Stroke::new(3.0, Color32::from_black_alpha(170)),
        }),
    });
    out.push(ClippedShape {
        clip_rect,
        shape: Shape::Circle(CircleShape {
            center: pos,
            radius: 6.0,
            fill: Color32::TRANSPARENT,
            stroke: Stroke::new(2.0, Color32::WHITE),
        }),
    });
}

pub(super) fn rgb_to_hsv(color: perro_structs::Color) -> (f32, f32, f32) {
    let max = color.r().max(color.g()).max(color.b());
    let min = color.r().min(color.g()).min(color.b());
    let delta = max - min;
    let hue = if delta <= f32::EPSILON {
        0.0
    } else if max == color.r() {
        ((color.g() - color.b()) / delta).rem_euclid(6.0) / 6.0
    } else if max == color.g() {
        (((color.b() - color.r()) / delta) + 2.0) / 6.0
    } else {
        (((color.r() - color.g()) / delta) + 4.0) / 6.0
    };
    (
        hue,
        if max <= f32::EPSILON {
            0.0
        } else {
            delta / max
        },
        max,
    )
}

pub(super) fn hsv_color(h: f32, s: f32, v: f32) -> perro_structs::Color {
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

pub(super) fn push_nine_slice_shapes(
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
    let texture_size = [image.texture_size[0] as f32, image.texture_size[1] as f32];
    let pixel_region = image
        .uv_min
        .iter()
        .chain(image.uv_max.iter())
        .any(|v| *v > 1.0);
    let ([u0, v0], [u3, v3]) = if pixel_region && texture_size[0] > 0.0 && texture_size[1] > 0.0 {
        (
            [
                image.uv_min[0] / texture_size[0],
                image.uv_min[1] / texture_size[1],
            ],
            [
                image.uv_max[0] / texture_size[0],
                image.uv_max[1] / texture_size[1],
            ],
        )
    } else {
        (image.uv_min, image.uv_max)
    };
    let uw = (u3 - u0).max(0.0);
    let vh = (v3 - v0).max(0.0);
    if uw <= 0.0 || vh <= 0.0 {
        return;
    }
    let (ul, ur, vt, vb) = if texture_size[0] > 0.0 && texture_size[1] > 0.0 {
        let ul = (l / texture_size[0]).min(uw);
        let ur = (r / texture_size[0]).min((uw - ul).max(0.0));
        let vt = (t / texture_size[1]).min(vh);
        let vb = (b / texture_size[1]).min((vh - vt).max(0.0));
        (ul, ur, vt, vb)
    } else {
        // Keep the full image visible while texture data is still pending.
        (
            uw * l / outer.width(),
            uw * r / outer.width(),
            vh * t / outer.height(),
            vh * b / outer.height(),
        )
    };
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
            add_tiled_nine_slice_patch(
                &mut mesh,
                Rect::from_min_max(pos2(xs[x], ys[y]), pos2(xs[x + 1], ys[y + 1])),
                Rect::from_min_max(pos2(us[x], vs[y]), pos2(us[x + 1], vs[y + 1])),
                [
                    (us[x + 1] - us[x]) * texture_size[0],
                    (vs[y + 1] - vs[y]) * texture_size[1],
                ],
                x == 1,
                y == 1,
                color32(image.tint),
            );
        }
    }
    out.push(ClippedShape {
        clip_rect: clip_rect_from_state(image.clip_rect, viewport),
        shape: Shape::Mesh(mesh.into()),
    });
}

pub(super) fn add_tiled_nine_slice_patch(
    mesh: &mut Mesh,
    rect: Rect,
    uv: Rect,
    source_size: [f32; 2],
    tile_x: bool,
    tile_y: bool,
    color: Color32,
) {
    let x_count = if tile_x && source_size[0] > 0.0 {
        (rect.width() / source_size[0]).ceil().clamp(1.0, 256.0) as usize
    } else {
        1
    };
    let y_count = if tile_y && source_size[1] > 0.0 {
        (rect.height() / source_size[1]).ceil().clamp(1.0, 256.0) as usize
    } else {
        1
    };
    for y in 0..y_count {
        let top = rect.top() + y as f32 * source_size[1];
        let bottom = if y + 1 == y_count {
            rect.bottom()
        } else {
            (top + source_size[1]).min(rect.bottom())
        };
        let v_fraction = if tile_y {
            ((bottom - top) / source_size[1]).clamp(0.0, 1.0)
        } else {
            1.0
        };
        for x in 0..x_count {
            let left = rect.left() + x as f32 * source_size[0];
            let right = if x + 1 == x_count {
                rect.right()
            } else {
                (left + source_size[0]).min(rect.right())
            };
            let u_fraction = if tile_x {
                ((right - left) / source_size[0]).clamp(0.0, 1.0)
            } else {
                1.0
            };
            mesh.add_rect_with_uv(
                Rect::from_min_max(pos2(left, top), pos2(right, bottom)),
                Rect::from_min_max(
                    uv.min,
                    pos2(
                        uv.min.x + uv.width() * u_fraction,
                        uv.min.y + uv.height() * v_fraction,
                    ),
                ),
                color,
            );
        }
    }
}

pub(super) fn clamp_nine_margins(margins: [f32; 4], w: f32, h: f32) -> [f32; 4] {
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

pub(super) fn push_image_shape(
    image: &UiImageDraw,
    viewport: [f32; 2],
    out: &mut Vec<ClippedShape>,
) {
    if image.texture.is_nil() || !valid_rect(image.rect) || !valid_color(image.tint) {
        return;
    }
    let (min, max) = image.rect.screen_min_max(viewport);
    let outer = Rect::from_min_max(pos2(min[0], min[1]), pos2(max[0], max[1]));
    let resolved = resolve_image_rect(outer, image);
    if resolved.width() <= 0.0 || resolved.height() <= 0.0 {
        return;
    }
    let source_uv = Rect::from_min_max(
        pos2(image.uv_min[0], image.uv_min[1]),
        pos2(image.uv_max[0], image.uv_max[1]),
    );
    let (rect, uv) = if image.scale_mode == UiImageScaleState::Cover {
        (
            outer,
            Rect::from_min_max(
                rect_uv(resolved, source_uv, outer.min),
                rect_uv(resolved, source_uv, outer.max),
            ),
        )
    } else {
        (resolved, source_uv)
    };
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

pub(super) fn add_rounded_rect_with_uv(
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

    for pos in rounded_rect_points(rect, radii, rounded_rect_segments(rect, radii)) {
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

pub(super) fn rect_uv(rect: Rect, uv: Rect, pos: epaint::Pos2) -> epaint::Pos2 {
    let x = ((pos.x - rect.left()) / rect.width().max(0.0001)).clamp(0.0, 1.0);
    let y = ((pos.y - rect.top()) / rect.height().max(0.0001)).clamp(0.0, 1.0);
    pos2(uv.left() + uv.width() * x, uv.top() + uv.height() * y)
}

pub(super) fn resolve_image_rect(outer: Rect, image: &UiImageDraw) -> Rect {
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

pub(super) fn push_label_shape(
    label: &UiLabelDraw,
    viewport: [f32; 2],
    definitions: &FontDefinitions,
    harfbuzz_atlas: &mut HarfBuzzAtlas,
    fonts: &mut Fonts,
    out: &mut Vec<ClippedShape>,
) {
    push_panel_shape(
        &UiPanelDraw {
            rect: label.rect,
            clip_rect: label.clip_rect,
            fill: label.backdrop_color,
            fill_kind: UiFillKindState::Solid,
            gradient: UiLinearGradientState::none(),
            stroke: Color::TRANSPARENT,
            stroke_width: 0.0,
            corner_radii: label.corner_radii,
            outer_shadow: UiDepthEffectState::none(),
            inner_shadow: UiDepthEffectState::none(),
            outer_highlight: UiDepthEffectState::none(),
            inner_highlight: UiDepthEffectState::none(),
        },
        viewport,
        out,
    );
    let text_rect = label_text_rect(label.rect, label.padding);
    if needs_harfbuzz(label.text.as_ref())
        && push_harfbuzz_text_shape(
            TextShapeInput {
                rect: text_rect,
                viewport,
                clip_rect: clip_rect_from_state(label.clip_rect, viewport),
                text: label.text.as_ref(),
                font_size: label.font_size,
                font: &label.font,
                wrap_width: label.wrap_width,
                color: label.color,
                h_align: label.h_align,
                v_align: label.v_align,
                fit_content: label.fit_content,
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
            rect: text_rect,
            viewport,
            clip_rect: clip_rect_from_state(label.clip_rect, viewport),
            text: label.text.as_ref(),
            font_size: label.font_size,
            font: &label.font,
            wrap_width: label.wrap_width,
            color: label.color,
            h_align: label.h_align,
            v_align: label.v_align,
            fit_content: label.fit_content,
        },
        fonts,
        out,
    );
}

pub(super) fn label_text_rect(mut rect: UiRectState, padding: [f32; 4]) -> UiRectState {
    let [left, top, right, bottom] = padding.map(|value| value.max(0.0));
    let width = rect.size[0];
    let height = rect.size[1];
    rect.center[0] += (left - right) * width * 0.5;
    rect.center[1] += (bottom - top) * height * 0.5;
    rect.size[0] = (width * (1.0 - left - right)).max(0.001);
    rect.size[1] = (height * (1.0 - top - bottom)).max(0.001);
    rect
}

pub(super) fn push_harfbuzz_text_shape(
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
        mut font_size,
        font,
        wrap_width: _,
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
        return false;
    }
    let family = selected_text_family(text, font, FontFamily::Proportional);
    let Some((font, run)) = harfbuzz_atlas.shape_cached(definitions, family, text) else {
        return false;
    };
    let (min, max) = rect.screen_min_max(viewport);
    let mut line_width = run
        .glyphs
        .iter()
        .map(|glyph| glyph.x_advance * font_size)
        .sum::<f32>()
        .max(0.0);
    if fit_content {
        let scale = (rect.size[0] / line_width.max(0.001))
            .min(rect.size[1] / font_size.max(0.001))
            .min(1.0);
        font_size *= scale;
        line_width *= scale;
    }
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
    for glyph in run.glyphs.iter().copied() {
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

pub(super) fn push_panel_shape(
    panel: &UiPanelDraw,
    viewport: [f32; 2],
    out: &mut Vec<ClippedShape>,
) {
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

pub(super) fn push_progress_bar_shapes(
    progress: &UiProgressBarDraw,
    viewport: [f32; 2],
    out: &mut Vec<ClippedShape>,
) {
    let panel = |rect, fill, corner_radii| UiPanelDraw {
        rect,
        clip_rect: progress.clip_rect,
        fill,
        fill_kind: UiFillKindState::Solid,
        gradient: UiLinearGradientState::default(),
        stroke: Color::TRANSPARENT,
        stroke_width: 0.0,
        corner_radii,
        outer_shadow: UiDepthEffectState::default(),
        inner_shadow: UiDepthEffectState::default(),
        outer_highlight: UiDepthEffectState::default(),
        inner_highlight: UiDepthEffectState::default(),
    };
    push_panel_shape(
        &panel(
            progress.rect,
            progress.background_fill,
            progress.background_corner_radii,
        ),
        viewport,
        out,
    );
    let value = progress.value.clamp(0.0, 1.0);
    if value <= 0.0 {
        return;
    }
    let mut fill_rect = progress.rect;
    fill_rect.size[0] *= value;
    fill_rect.center[0] -= (progress.rect.size[0] - fill_rect.size[0]) * 0.5;
    push_panel_shape(
        &panel(fill_rect, progress.fill, progress.fill_corner_radii),
        viewport,
        out,
    );
}

pub(super) fn push_panel_fill_shape(
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

pub(super) fn push_panel_stroke_shape(
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

pub(super) fn push_gradient_panel_shape(
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

pub(super) fn push_outer_fill_effect(
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

pub(super) fn push_outer_stroke_effect(
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

pub(super) fn push_inner_fill_effect(
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

pub(super) fn push_inner_stroke_effect(
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

pub(super) fn push_text_edit_shapes(
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
        let family = selected_text_family(edit.text.as_ref(), &edit.font, FontFamily::Monospace);
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
        let family = selected_text_family(body, &edit.font, FontFamily::Monospace);
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

pub(super) fn text_edit_draw_pos(
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

pub(super) fn push_selection_shapes(
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

pub(super) fn push_caret_shape(
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
