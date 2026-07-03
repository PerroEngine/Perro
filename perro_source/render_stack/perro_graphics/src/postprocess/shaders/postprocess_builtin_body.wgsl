// Apply one cheap per-pixel color op given its type + two packed param vec4s.
// Only ops that fit in <=2 param vec4s are mergeable (color_grade is excluded).
fn apply_color_op(kind: u32, uv: vec2<f32>, color: vec4<f32>, p0: vec4<f32>, p1: vec4<f32>) -> vec4<f32> {
    if kind == 4u {
        return vignette_apply(uv, color, p0.x, p0.y, p0.z);
    }
    if kind == 6u {
        return color_filter_apply(color, p0.xyz, p0.w);
    }
    if kind == 7u {
        return reverse_filter_apply(color, p0.xyz, p0.w, p1.x);
    }
    if kind == 9u {
        return saturate_apply(color, p0.x);
    }
    if kind == 10u {
        return black_white_apply(color, p0.x);
    }
    return color;
}

fn post_process(uv: vec2<f32>, color: vec4<f32>, depth: f32) -> vec4<f32> {
    if post.effect_type == 15u {
        // Merged cheap color ops: apply param_count ops in one pass. Each op is
        // packed as 3 vec4 in custom_params: [type,_,_,_], p0, p1. params0.x is
        // the op base index (distinct per merged step to avoid buffer aliasing).
        var acc = color;
        let base_op = u32(post.params0.x);
        for (var i = 0u; i < post.param_count; i = i + 1u) {
            let base = (base_op + i) * 3u;
            let kind = u32(custom_params[base].x);
            acc = apply_color_op(kind, uv, acc, custom_params[base + 1u], custom_params[base + 2u]);
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
