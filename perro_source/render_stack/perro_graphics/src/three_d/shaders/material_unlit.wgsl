fn shade_material(in: FragmentInput) -> vec4<f32> {
    let color = unpack_rgba8(in.packed_color);
    let emissive = perro_unpack_emissive_hdr(in.packed_emissive);
    let material = perro_decode_material_params(in.packed_material_params);
    var base_sample = vec4<f32>(1.0);
    if material.has_base_color_texture {
        base_sample = textureSample(material_base_color_tex, material_sampler, in.uv);
    }
    return perro_unlit(in, color * base_sample, emissive);
}
