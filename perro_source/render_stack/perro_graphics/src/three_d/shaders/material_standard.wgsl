fn shade_material(in: FragmentInput) -> vec4<f32> {
    let color = unpack_rgba8(in.packed_color);
    let emissive = unpack_emissive_hdr(in.packed_emissive);
    let pbr = decode_standard_pbr_params(in.packed_pbr_params_0, in.packed_pbr_params_1);
    let material = decode_material_params(in.packed_material_params);
    var base_sample = vec4<f32>(1.0, 1.0, 1.0, 1.0);
    if material.has_base_color_texture {
        base_sample = textureSample(material_base_color_tex, material_sampler, in.uv);
    }
    let albedo = color.rgb * base_sample.rgb;
    let roughness = clamp(pbr.x, 0.04, 1.0);
    let metallic = clamp(pbr.y, 0.0, 1.0);
    let ao = clamp(pbr.z, 0.0, 1.0);
    if material.meshlet_debug_view {
        return vec4<f32>(color.rgb, 1.0);
    }
    return perro_lit_standard(in, vec4<f32>(albedo, color.a * base_sample.a), roughness, metallic, ao, emissive);
}
