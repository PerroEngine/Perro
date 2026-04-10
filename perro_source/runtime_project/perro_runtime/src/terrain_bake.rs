use crate::TerrainLayerRule;

pub fn terrain_layer_bake_upscale(
    layer_rules: &[TerrainLayerRule],
    sample_rate: Option<f32>,
) -> u32 {
    if layer_rules.is_empty() {
        return 1;
    }
    sample_rate
        .filter(|v| v.is_finite() && *v > 0.0)
        .unwrap_or(1.0)
        .round()
        .clamp(1.0, 12.0) as u32
}

pub fn build_smoothed_terrain_map(
    terrain_map: &image::RgbaImage,
    upscale: u32,
) -> Option<image::RgbaImage> {
    if upscale <= 1 {
        return None;
    }
    let w = terrain_map.width();
    let h = terrain_map.height();
    if w == 0 || h == 0 {
        return None;
    }
    let mut out = image::RgbaImage::new(w, h);
    for y in 0..h {
        for x in 0..w {
            let mut sum = [0u32; 4];
            let mut weight_sum = 0u32;
            for oy in -1i32..=1 {
                for ox in -1i32..=1 {
                    let sx = (x as i32 + ox).clamp(0, w as i32 - 1) as u32;
                    let sy = (y as i32 + oy).clamp(0, h as i32 - 1) as u32;
                    let wxy = match (ox.abs(), oy.abs()) {
                        (0, 0) => 4,
                        (1, 1) => 1,
                        _ => 2,
                    };
                    let px = terrain_map.get_pixel(sx, sy).0;
                    sum[0] += px[0] as u32 * wxy;
                    sum[1] += px[1] as u32 * wxy;
                    sum[2] += px[2] as u32 * wxy;
                    sum[3] += px[3] as u32 * wxy;
                    weight_sum += wxy;
                }
            }
            out.put_pixel(
                x,
                y,
                image::Rgba([
                    (sum[0] / weight_sum) as u8,
                    (sum[1] / weight_sum) as u8,
                    (sum[2] / weight_sum) as u8,
                    (sum[3] / weight_sum) as u8,
                ]),
            );
        }
    }
    Some(out)
}

pub fn build_layered_terrain_chunk_tile(
    terrain_map: &image::RgbaImage,
    smoothed_map: Option<&image::RgbaImage>,
    layer_textures: &[Option<image::RgbaImage>],
    layer_rules: &[TerrainLayerRule],
    terrain_bounds: (f32, f32, f32, f32),
    px0: u32,
    py0: u32,
    out_width: u32,
    out_height: u32,
    upscale: u32,
) -> image::RgbaImage {
    let allow_blending = layer_rules.iter().any(|rule| !rule.blend_with.is_empty());
    let (terrain_min_x, terrain_max_x, terrain_min_z, terrain_max_z) = terrain_bounds;
    let span_x = (terrain_max_x - terrain_min_x).max(1.0e-3);
    let span_z = (terrain_max_z - terrain_min_z).max(1.0e-3);
    let map_w = terrain_map.width().max(1);
    let map_h = terrain_map.height().max(1);
    let mut out = image::RgbaImage::new(out_width.max(1), out_height.max(1));
    let inv_scale = (upscale.max(1) as f32).recip();
    let smooth_mix = ((upscale.saturating_sub(1)) as f32 / 11.0).clamp(0.0, 1.0) * 0.65;
    let aa_samples = aa_samples_for_upscale(upscale);

    for y in 0..out_height {
        for x in 0..out_width {
            let sx_f = px0 as f32 + (x as f32 + 0.5) * inv_scale - 0.5;
            let sy_f = py0 as f32 + (y as f32 + 0.5) * inv_scale - 0.5;
            let mut acc = [0.0f32; 4];
            let mut weight_sum = 0.0f32;
            for (ox, oy, weight) in aa_samples {
                let sx = sx_f + ox * inv_scale;
                let sy = sy_f + oy * inv_scale;
                let src = sample_blended_map_color(terrain_map, smoothed_map, smooth_mix, sx, sy);
                let u = (sx + 0.5).clamp(0.0, map_w as f32 - 1.0) / map_w as f32;
                let v = (sy + 0.5).clamp(0.0, map_h as f32 - 1.0) / map_h as f32;
                let world_x = terrain_min_x + u * span_x;
                let world_z = terrain_min_z + v * span_z;
                let pixel = sample_terrain_layer_pixel(
                    src,
                    world_x,
                    world_z,
                    layer_rules,
                    layer_textures,
                    allow_blending,
                );
                acc[0] += pixel[0] as f32 * weight;
                acc[1] += pixel[1] as f32 * weight;
                acc[2] += pixel[2] as f32 * weight;
                acc[3] += pixel[3] as f32 * weight;
                weight_sum += weight;
            }
            let inv_w = weight_sum.max(1.0e-6).recip();
            out.put_pixel(
                x,
                y,
                image::Rgba([
                    (acc[0] * inv_w).round().clamp(0.0, 255.0) as u8,
                    (acc[1] * inv_w).round().clamp(0.0, 255.0) as u8,
                    (acc[2] * inv_w).round().clamp(0.0, 255.0) as u8,
                    (acc[3] * inv_w).round().clamp(0.0, 255.0) as u8,
                ]),
            );
        }
    }

    apply_adaptive_sharpen(&mut out, upscale);
    out
}

fn apply_adaptive_sharpen(image: &mut image::RgbaImage, upscale: u32) {
    let strength = match upscale {
        0 | 1 => 0.0,
        2..=3 => 0.10,
        4..=6 => 0.14,
        _ => 0.18,
    };
    if strength <= 0.0 || image.width() < 2 || image.height() < 2 {
        return;
    }
    let src = image.clone();
    let w = src.width();
    let h = src.height();
    for y in 0..h {
        for x in 0..w {
            let cx = src.get_pixel(x, y).0;
            let n = src.get_pixel(x, y.saturating_sub(1)).0;
            let s = src.get_pixel(x, (y + 1).min(h - 1)).0;
            let wv = src.get_pixel(x.saturating_sub(1), y).0;
            let e = src.get_pixel((x + 1).min(w - 1), y).0;
            let sharpen_chan = |i: usize| -> u8 {
                let center = cx[i] as f32;
                let around = (n[i] as f32 + s[i] as f32 + wv[i] as f32 + e[i] as f32) * 0.25;
                let v = center + (center - around) * strength;
                v.round().clamp(0.0, 255.0) as u8
            };
            image.put_pixel(
                x,
                y,
                image::Rgba([sharpen_chan(0), sharpen_chan(1), sharpen_chan(2), cx[3]]),
            );
        }
    }
}

fn sample_blended_map_color(
    terrain_map: &image::RgbaImage,
    smoothed_map: Option<&image::RgbaImage>,
    smooth_mix: f32,
    sx: f32,
    sy: f32,
) -> image::Rgba<u8> {
    let src = sample_map_color_bilinear(terrain_map, sx, sy);
    if let Some(smoothed) = smoothed_map
        && smooth_mix > 0.001
    {
        let smooth_src = sample_map_color_bilinear(smoothed, sx, sy);
        return mix_rgba(src, smooth_src, smooth_mix);
    }
    src
}

fn aa_samples_for_upscale(upscale: u32) -> &'static [(f32, f32, f32)] {
    const ONE: &[(f32, f32, f32)] = &[(0.0, 0.0, 1.0)];
    const TWO: &[(f32, f32, f32)] = &[(-0.2, -0.2, 0.5), (0.2, 0.2, 0.5)];
    const FOUR: &[(f32, f32, f32)] = &[
        (-0.25, -0.25, 0.25),
        (0.25, -0.25, 0.25),
        (-0.25, 0.25, 0.25),
        (0.25, 0.25, 0.25),
    ];
    if upscale >= 7 {
        FOUR
    } else if upscale >= 3 {
        TWO
    } else {
        ONE
    }
}

fn sample_terrain_layer_pixel(
    source_map_pixel: image::Rgba<u8>,
    world_x: f32,
    world_z: f32,
    layer_rules: &[TerrainLayerRule],
    layer_textures: &[Option<image::RgbaImage>],
    allow_blending: bool,
) -> image::Rgba<u8> {
    if layer_rules.is_empty() {
        return source_map_pixel;
    }

    if allow_blending
        && let Some((a_idx, b_idx, blend_t)) =
            classify_blend_pair_by_color(source_map_pixel, layer_rules)
    {
        let a = sample_layer_value(
            a_idx,
            source_map_pixel,
            world_x,
            world_z,
            layer_rules,
            layer_textures,
        );
        let b = sample_layer_value(
            b_idx,
            source_map_pixel,
            world_x,
            world_z,
            layer_rules,
            layer_textures,
        );
        return mix_rgba(a, b, blend_t);
    }

    if let Some(primary_idx) = layer_rules
        .iter()
        .enumerate()
        .find_map(|(i, rule)| terrain_layer_color_matches(source_map_pixel, rule).then_some(i))
    {
        return sample_layer_value(
            primary_idx,
            source_map_pixel,
            world_x,
            world_z,
            layer_rules,
            layer_textures,
        );
    }

    let nearest = nearest_layer_by_color(source_map_pixel, layer_rules).unwrap_or(0);
    sample_layer_value(
        nearest,
        source_map_pixel,
        world_x,
        world_z,
        layer_rules,
        layer_textures,
    )
}

fn sample_layer_value(
    idx: usize,
    source_map_pixel: image::Rgba<u8>,
    world_x: f32,
    world_z: f32,
    layer_rules: &[TerrainLayerRule],
    layer_textures: &[Option<image::RgbaImage>],
) -> image::Rgba<u8> {
    let Some(rule) = layer_rules.get(idx) else {
        return image::Rgba([0, 0, 0, 255]);
    };
    if let Some(texture) = layer_textures.get(idx).and_then(|v| v.as_ref())
        && texture.width() > 0
        && texture.height() > 0
    {
        let tile = rule.texture_tile_meters.max(0.001);
        let angle = rule.texture_rotation_degrees.to_radians();
        let (sin_a, cos_a) = angle.sin_cos();
        let tx = world_x / tile;
        let tz = world_z / tile;
        let rx = tx * cos_a - tz * sin_a;
        let rz = tx * sin_a + tz * cos_a;
        let fx = rx.rem_euclid(1.0);
        let fz = rz.rem_euclid(1.0);
        if rule.texture_hard_cut {
            return sample_texture_wrapped_nearest(texture, fx, fz);
        }
        return sample_texture_wrapped_bilinear(texture, fx, fz);
    }
    source_map_pixel
}

fn classify_blend_pair_by_color(
    pixel: image::Rgba<u8>,
    layer_rules: &[TerrainLayerRule],
) -> Option<(usize, usize, f32)> {
    if layer_rules.len() < 2 {
        return None;
    }
    let mut a_idx = usize::MAX;
    let mut b_idx = usize::MAX;
    let mut a_d = u32::MAX;
    let mut b_d = u32::MAX;
    for (idx, rule) in layer_rules.iter().enumerate() {
        let d = color_distance_sq(pixel, rule.color.r, rule.color.g, rule.color.b);
        if d < a_d {
            b_idx = a_idx;
            b_d = a_d;
            a_idx = idx;
            a_d = d;
        } else if d < b_d {
            b_idx = idx;
            b_d = d;
        }
    }
    if a_idx == usize::MAX || b_idx == usize::MAX {
        return None;
    }
    if !layers_can_blend(a_idx, b_idx, layer_rules) {
        return None;
    }
    let a = (a_d as f32).sqrt();
    let b = (b_d as f32).sqrt();
    const BLEND_MARGIN: f32 = 32.0;
    if b > BLEND_MARGIN {
        return None;
    }
    if (b - a) > BLEND_MARGIN * 0.6 {
        return None;
    }
    let denom = (a + b).max(1.0e-5);
    let t = (a / denom).clamp(0.0, 1.0);
    let t = t * t * (3.0 - 2.0 * t);
    Some((a_idx, b_idx, t))
}

fn nearest_layer_by_color(
    pixel: image::Rgba<u8>,
    layer_rules: &[TerrainLayerRule],
) -> Option<usize> {
    layer_rules
        .iter()
        .enumerate()
        .min_by_key(|(_, rule)| color_distance_sq(pixel, rule.color.r, rule.color.g, rule.color.b))
        .map(|(i, _)| i)
}

fn layers_can_blend(a_idx: usize, b_idx: usize, layer_rules: &[TerrainLayerRule]) -> bool {
    let Some(a) = layer_rules.get(a_idx) else {
        return false;
    };
    let Some(b) = layer_rules.get(b_idx) else {
        return false;
    };
    a.blend_with.contains(&b.index) || b.blend_with.contains(&a.index)
}

fn color_distance_sq(pixel: image::Rgba<u8>, r: u8, g: u8, b: u8) -> u32 {
    let dr = pixel[0] as i32 - r as i32;
    let dg = pixel[1] as i32 - g as i32;
    let db = pixel[2] as i32 - b as i32;
    (dr * dr + dg * dg + db * db) as u32
}

fn mix_rgba(a: image::Rgba<u8>, b: image::Rgba<u8>, t: f32) -> image::Rgba<u8> {
    let t = t.clamp(0.0, 1.0);
    let lerp = |x: u8, y: u8| -> u8 { (x as f32 + (y as f32 - x as f32) * t).round() as u8 };
    image::Rgba([
        lerp(a[0], b[0]),
        lerp(a[1], b[1]),
        lerp(a[2], b[2]),
        lerp(a[3], b[3]),
    ])
}

fn sample_map_color_bilinear(terrain_map: &image::RgbaImage, sx: f32, sy: f32) -> image::Rgba<u8> {
    let w = terrain_map.width().max(1);
    let h = terrain_map.height().max(1);
    let x = sx.clamp(0.0, w.saturating_sub(1) as f32);
    let y = sy.clamp(0.0, h.saturating_sub(1) as f32);
    let x0 = x.floor() as u32;
    let y0 = y.floor() as u32;
    let x1 = (x0 + 1).min(w.saturating_sub(1));
    let y1 = (y0 + 1).min(h.saturating_sub(1));
    let tx = x - x0 as f32;
    let ty = y - y0 as f32;

    let c00 = terrain_map.get_pixel(x0, y0).0;
    let c10 = terrain_map.get_pixel(x1, y0).0;
    let c01 = terrain_map.get_pixel(x0, y1).0;
    let c11 = terrain_map.get_pixel(x1, y1).0;
    let lerp = |a: f32, b: f32, t: f32| a + (b - a) * t;
    let chan = |i: usize| -> u8 {
        let a = lerp(c00[i] as f32, c10[i] as f32, tx);
        let b = lerp(c01[i] as f32, c11[i] as f32, tx);
        lerp(a, b, ty).round().clamp(0.0, 255.0) as u8
    };
    image::Rgba([chan(0), chan(1), chan(2), chan(3)])
}

fn sample_texture_wrapped_nearest(texture: &image::RgbaImage, u: f32, v: f32) -> image::Rgba<u8> {
    let w = texture.width().max(1);
    let h = texture.height().max(1);
    let x = (u.rem_euclid(1.0) * w as f32).floor() as u32 % w;
    let y = (v.rem_euclid(1.0) * h as f32).floor() as u32 % h;
    *texture.get_pixel(x, y)
}

fn sample_texture_wrapped_bilinear(texture: &image::RgbaImage, u: f32, v: f32) -> image::Rgba<u8> {
    let w = texture.width().max(1);
    let h = texture.height().max(1);
    let wf = w as f32;
    let hf = h as f32;

    let sx = u.rem_euclid(1.0) * wf - 0.5;
    let sy = v.rem_euclid(1.0) * hf - 0.5;
    let x0 = sx.floor();
    let y0 = sy.floor();
    let tx = sx - x0;
    let ty = sy - y0;

    let x0i = (x0 as i32).rem_euclid(w as i32) as u32;
    let y0i = (y0 as i32).rem_euclid(h as i32) as u32;
    let x1i = ((x0 as i32 + 1).rem_euclid(w as i32)) as u32;
    let y1i = ((y0 as i32 + 1).rem_euclid(h as i32)) as u32;

    let c00 = texture.get_pixel(x0i, y0i).0;
    let c10 = texture.get_pixel(x1i, y0i).0;
    let c01 = texture.get_pixel(x0i, y1i).0;
    let c11 = texture.get_pixel(x1i, y1i).0;

    let lerp = |a: f32, b: f32, t: f32| a + (b - a) * t;
    let chan = |i: usize| -> u8 {
        let a = lerp(c00[i] as f32, c10[i] as f32, tx);
        let b = lerp(c01[i] as f32, c11[i] as f32, tx);
        lerp(a, b, ty).round().clamp(0.0, 255.0) as u8
    };
    image::Rgba([chan(0), chan(1), chan(2), chan(3)])
}

fn terrain_layer_color_matches(pixel: image::Rgba<u8>, rule: &TerrainLayerRule) -> bool {
    let tol = rule.color_tolerance as i32;
    let dr = (pixel[0] as i32 - rule.color.r as i32).abs();
    let dg = (pixel[1] as i32 - rule.color.g as i32).abs();
    let db = (pixel[2] as i32 - rule.color.b as i32).abs();
    dr <= tol && dg <= tol && db <= tol
}
