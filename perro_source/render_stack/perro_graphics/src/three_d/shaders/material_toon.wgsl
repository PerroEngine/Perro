fn shade_material(in: FragmentInput) -> vec4<f32> {
    let color = unpack_rgba8(in.packed_color);
    let emissive = perro_unpack_emissive_hdr(in.packed_emissive);
    let material = perro_decode_material_params(in.packed_material_params);
    let style_tag = perro_unpack_byte(in.packed_pbr_params_0, 0u);
    var sample_uv = in.uv;
    if style_tag == 255u {
        let pixel_count = max(f32(perro_unpack_byte(in.packed_pbr_params_0, 8u)), 1.0);
        sample_uv = perro_pixel_uv(in.uv, vec2<f32>(pixel_count));
    }
    var base_sample = vec4<f32>(1.0);
    if /*__PERRO_STD_BASE_TEXTURE__*/ material.has_base_color_texture {
        base_sample = textureSample(material_base_color_tex, material_sampler, sample_uv);
    }
    let base_color = color * base_sample;
    if style_tag == 0u {
        let band_count = max(f32(perro_unpack_byte(in.packed_pbr_params_0, 8u)), 1.0);
        let hatch_scale = perro_unpack_unorm8(in.packed_pbr_params_0, 16u) * 128.0;
        let grain_strength = perro_unpack_unorm8(in.packed_pbr_params_0, 24u);
        return perro_hand_drawn(
            in,
            base_color,
            band_count,
            hatch_scale,
            grain_strength,
            emissive,
        );
    }
    if style_tag == 255u {
        let color_levels = max(f32(perro_unpack_byte(in.packed_pbr_params_0, 16u)), 2.0);
        let dither_strength = perro_unpack_unorm8(in.packed_pbr_params_0, 24u);
        return perro_pixel_surface(
            in,
            base_color,
            color_levels,
            dither_strength,
            emissive,
        );
    }
    let toon = decode_toon_params(in.packed_pbr_params_0, in.packed_pbr_params_1);
    return perro_toon(
        in,
        base_color,
        toon.x,
        toon.y,
        toon.z,
        emissive,
    );
}
