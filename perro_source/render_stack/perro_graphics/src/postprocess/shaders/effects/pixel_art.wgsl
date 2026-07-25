const PERRO_POST_BAYER_4X4: array<f32, 16> = array<f32, 16>(
    0.0, 8.0, 2.0, 10.0,
    12.0, 4.0, 14.0, 6.0,
    3.0, 11.0, 1.0, 9.0,
    15.0, 7.0, 13.0, 5.0,
);

fn pixel_art_sample(
    uv: vec2<f32>,
    virtual_height_in: f32,
    color_levels_in: f32,
    dither_strength: f32,
) -> vec4<f32> {
    let virtual_height = max(round(virtual_height_in), 1.0);
    let aspect = post.resolution.x / max(post.resolution.y, 1.0);
    let virtual_size = vec2<f32>(max(round(virtual_height * aspect), 1.0), virtual_height);
    let cell = clamp(
        floor(uv * virtual_size),
        vec2<f32>(0.0),
        virtual_size - vec2<f32>(1.0),
    );
    let sample_uv = (cell + vec2<f32>(0.5)) / virtual_size;
    let sampled = textureSampleLevel(input_tex, input_sampler, sample_uv, 0.0);
    let pixel = vec2<u32>(cell) & vec2<u32>(3u);
    let threshold =
        (PERRO_POST_BAYER_4X4[pixel.y * 4u + pixel.x] + 0.5) / 16.0 - 0.5;
    let levels = max(round(color_levels_in), 2.0);
    let steps = levels - 1.0;
    let rgb = round(
        clamp(
            sampled.rgb + vec3<f32>(threshold * dither_strength),
            vec3<f32>(0.0),
            vec3<f32>(1.0),
        ) * steps,
    ) / steps;
    return vec4<f32>(rgb, sampled.a);
}
