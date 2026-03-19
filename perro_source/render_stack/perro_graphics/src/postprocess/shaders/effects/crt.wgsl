fn crt_distort(uv: vec2<f32>, curvature: f32) -> vec2<f32> {
    let c = max(curvature, 0.0);
    if c <= 0.0001 {
        return uv;
    }
    let p = uv * 2.0 - 1.0;
    let r2 = dot(p, p);
    let distorted = p + p * r2 * c;
    return distorted * 0.5 + 0.5;
}

fn crt_sample(
    uv: vec2<f32>,
    scanlines: f32,
    curvature: f32,
    chromatic: f32,
    vignette: f32,
) -> vec4<f32> {
    let duv = crt_distort(uv, curvature);
    if duv.x < 0.0 || duv.x > 1.0 || duv.y < 0.0 || duv.y > 1.0 {
        return vec4<f32>(0.0, 0.0, 0.0, 1.0);
    }
    let chroma_px = chromatic * post.inv_resolution.x;
    let r = textureSample(input_tex, input_sampler, duv + vec2<f32>(chroma_px, 0.0)).r;
    let g = textureSample(input_tex, input_sampler, duv).g;
    let b = textureSample(input_tex, input_sampler, duv - vec2<f32>(chroma_px, 0.0)).b;
    var color = vec3<f32>(r, g, b);

    let scan_strength = clamp(scanlines, 0.0, 1.0);
    if scan_strength > 0.001 {
        let scan = sin(duv.y * post.resolution.y * 3.14159265);
        let scan_factor = 1.0 - scan_strength * (0.5 + 0.5 * scan);
        color *= scan_factor;
    }

    let vig = clamp(vignette, 0.0, 1.0);
    if vig > 0.001 {
        let dist = distance(duv, vec2<f32>(0.5, 0.5));
        let t = smoothstep(0.35, 0.9, dist);
        color *= mix(1.0, 1.0 - vig, t);
    }

    return vec4<f32>(color, 1.0);
}
