fn post_process(uv: vec2<f32>, color: vec4<f32>, depth: f32) -> vec4<f32> {
    if post.effect_type == 1u {
        let strength = post.params0.x;
        return blur_sample(uv, strength);
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
        let strength = post.params0.x;
        let threshold = post.params0.y;
        let radius = post.params0.z;
        return bloom_sample(uv, strength, threshold, radius);
    }
    if post.effect_type == 9u {
        let amount = post.params0.x;
        return saturate_apply(color, amount);
    }
    if post.effect_type == 10u {
        let amount = post.params0.x;
        return black_white_apply(color, amount);
    }
    return color;
}
