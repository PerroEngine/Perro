fn shade_material(in: FragmentInput) -> vec4<f32> {
    let color = unpack_rgba8(in.packed_color);
    let emissive = unpack_emissive_hdr(in.packed_emissive);
    let pbr = decode_standard_pbr_params(in.packed_pbr_params_0, in.packed_pbr_params_1);
    let material = decode_material_params(in.packed_material_params);
    var base_sample = vec4<f32>(1.0, 1.0, 1.0, 1.0);
    if material.has_base_color_texture {
        base_sample = textureSample(material_base_color_tex, material_sampler, in.uv);
    }
    var albedo = color.rgb * base_sample.rgb;
    // Chromatic modulate (0x100): re-apply the hue bias against the texture
    // sample so saturated texels don't collapse under an opposing modulate.
    // CPU already biased the factor, so with no texture (sample = white)
    // this mix is an exact no-op. Constant mirrors MODULATE_TINT_BIAS.
    if (material.material_flags & 0x100u) != 0u {
        let sat = max(max(color.r, color.g), color.b) - min(min(color.r, color.g), color.b);
        let k = 0.2 * clamp(sat, 0.0, 1.0);
        let tex_luma = dot(base_sample.rgb, vec3<f32>(0.2126, 0.7152, 0.0722));
        albedo = mix(albedo, color.rgb * tex_luma, k);
    }
    let roughness = clamp(pbr.x, 0.04, 1.0);
    let metallic = clamp(pbr.y, 0.0, 1.0);
    let ao = clamp(pbr.z, 0.0, 1.0);
    if material.meshlet_debug_view {
        return vec4<f32>(color.rgb, 1.0);
    }
    return perro_lit_standard(in, vec4<f32>(albedo, color.a * base_sample.a), roughness, metallic, ao, emissive);
}
