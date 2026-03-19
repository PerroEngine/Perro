// Custom post-process effect: extreme channel swap + glitch + swirl + scanlines
// params[0].x = split strength (pixels)
// params[0].y = glitch strength (0-1)
// params[0].z = scanline strength (0-1)
// params[0].w = swirl strength (0-1)

fn hash21(p: vec2<f32>) -> f32 {
    let h = dot(p, vec2<f32>(127.1, 311.7));
    return fract(sin(h) * 43758.5453123);
}

fn post_process(uv: vec2<f32>, color: vec4<f32>, depth: f32) -> vec4<f32> {
    let split = custom_params[0].x;
    let glitch = clamp(custom_params[0].y, 0.0, 1.0);
    let scan = clamp(custom_params[0].z, 0.0, 1.0);
    let swirl = clamp(custom_params[0].w, 0.0, 1.0);

    // Horizontal glitch bands
    let band = floor(uv.y * 120.0);
    let n = hash21(vec2<f32>(band, band + 1.0));
    let jitter = (n - 0.5) * glitch * 0.08;

    // Swirl around center
    let centered = uv - vec2<f32>(0.5, 0.5);
    let r = length(centered);
    let ang = atan2(centered.y, centered.x) + swirl * (1.0 - r) * 10.0;
    let swirl_uv = vec2<f32>(cos(ang), sin(ang)) * r + vec2<f32>(0.5, 0.5);

    let off = split * post.inv_resolution;
    let uv_r = swirl_uv + vec2<f32>(off.x + jitter, 0.0);
    let uv_g = swirl_uv + vec2<f32>(jitter * 0.5, 0.0);
    let uv_b = swirl_uv - vec2<f32>(off.x - jitter, 0.0);

    let rch = textureSample(input_tex, input_sampler, uv_r).r;
    let gch = textureSample(input_tex, input_sampler, uv_g).g;
    let bch = textureSample(input_tex, input_sampler, uv_b).b;

    // Hard channel swap and boost
    let swapped = vec3<f32>(gch, bch, rch) * 1.35;

    // Scanlines
    let scanline = 0.5 + 0.5 * sin(uv.y * post.resolution.y * 3.14159);
    let scan_mix = 1.0 - scan * 0.85 * (1.0 - scanline);

    let out_rgb = clamp(swapped * scan_mix, vec3<f32>(0.0), vec3<f32>(2.0));
    return vec4<f32>(out_rgb, color.a);
}