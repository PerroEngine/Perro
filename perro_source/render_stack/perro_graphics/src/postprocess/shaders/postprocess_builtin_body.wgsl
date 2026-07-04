// Apply one per-pixel color op given its type + the vec4 index of its params
// in custom_params.
fn apply_color_op(kind: u32, uv: vec2<f32>, color: vec4<f32>, base: u32) -> vec4<f32> {
    if kind == 4u {
        let p0 = custom_params[base];
        return vignette_apply(uv, color, p0.x, p0.y, p0.z);
    }
    if kind == 6u {
        let p0 = custom_params[base];
        return color_filter_apply(color, p0.xyz, p0.w);
    }
    if kind == 7u {
        let p0 = custom_params[base];
        let p1 = custom_params[base + 1u];
        return reverse_filter_apply(color, p0.xyz, p0.w, p1.x);
    }
    if kind == 9u {
        return saturate_apply(color, custom_params[base].x);
    }
    if kind == 10u {
        return black_white_apply(color, custom_params[base].x);
    }
    if kind == 11u {
        return color_grade_apply_params(
            color,
            custom_params[base],
            custom_params[base + 1u],
            custom_params[base + 2u],
            custom_params[base + 3u],
            custom_params[base + 4u],
        );
    }
    return color;
}

fn post_process(uv: vec2<f32>, color: vec4<f32>, depth: f32) -> vec4<f32> {
    if post.effect_type == 15u {
        // Merged color ops: apply param_count ops in one pass. Each op sits in
        // custom_params as a header [type, param_vec4_count, _, _] followed by
        // that many param vec4s. params0.x is the vec4 base offset (distinct
        // per merged step to avoid buffer aliasing).
        var acc = color;
        var cursor = u32(post.params0.x);
        for (var i = 0u; i < post.param_count; i = i + 1u) {
            let header = custom_params[cursor];
            let kind = u32(header.x);
            acc = apply_color_op(kind, uv, acc, cursor + 1u);
            cursor = cursor + 1u + u32(header.y);
        }
        return acc;
    }
    if post.effect_type == 1u {
        let strength = post.params0.x;
        let axis = post.params0.y;
        return blur_axis_sample(uv, strength, axis);
    }
    if post.effect_type == 2u {
        let size = post.params0.x;
        return pixelate_sample(uv, size);
    }
    if post.effect_type == 3u {
        let waves = post.params0.x;
        let strength = post.params0.y;
        return warp_sample(uv, waves, strength);
    }
    if post.effect_type == 4u {
        let strength = post.params0.x;
        let radius = post.params0.y;
        let softness = post.params0.z;
        return vignette_apply(uv, color, strength, radius, softness);
    }
    if post.effect_type == 5u {
        let scanlines = post.params0.x;
        let curvature = post.params0.y;
        let chromatic = post.params0.z;
        let vignette = post.params0.w;
        return crt_sample(uv, scanlines, curvature, chromatic, vignette);
    }
    if post.effect_type == 6u {
        let tint = post.params0.xyz;
        let strength = post.params0.w;
        return color_filter_apply(color, tint, strength);
    }
    if post.effect_type == 7u {
        let target_color = post.params0.xyz;
        let strength = post.params0.w;
        let softness = post.params1.x;
        return reverse_filter_apply(color, target_color, strength, softness);
    }
    if post.effect_type == 8u {
        // Bloom composite (final pass of the downsample->blur->upsample chain).
        let strength = post.params0.x;
        return bloom_composite_sample(uv, strength);
    }
    if post.effect_type == 14u {
        // Bloom bright-pass (downsample + threshold into half-res target).
        let threshold = post.params0.y;
        return bloom_bright_sample(uv, threshold);
    }
    if post.effect_type == 9u {
        let amount = post.params0.x;
        return saturate_apply(color, amount);
    }
    if post.effect_type == 10u {
        let amount = post.params0.x;
        return black_white_apply(color, amount);
    }
    if post.effect_type == 11u {
        return color_grade_apply(color);
    }
    if post.effect_type == 12u {
        let strength = post.params0.x;
        let lut_size = post.params0.y;
        return lut_2d_apply(color, strength, lut_size);
    }
    if post.effect_type == 13u {
        let strength = post.params0.x;
        let lut_size = post.params0.y;
        return lut_3d_apply(color, strength, lut_size);
    }
    return color;
}
