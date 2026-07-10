fn shade_material(in: FragmentInput) -> vec4<f32> {
    let color = unpack_rgba8(in.packed_color);
    let emissive = unpack_emissive_hdr(in.packed_emissive);
    let material = decode_material_params(in.packed_material_params);
    var n = normalize(in.normal_ws);
    if material.flat_shading {
        n = normalize(cross(dpdx(in.world_pos), dpdy(in.world_pos)));
        if material.mirrored_winding {
            n = -n;
        }
    }
    if material.double_sided && (in.is_front == material.mirrored_winding) {
        n = -n;
    }
    var base_sample = vec4<f32>(1.0, 1.0, 1.0, 1.0);
    if material.has_base_color_texture {
        base_sample = textureSample(material_base_color_tex, material_sampler, in.uv);
    }
    var alpha = clamp(color.a * base_sample.a, 0.0, 1.0);
    if material.alpha_mode == 1u && alpha < material.alpha_cutoff {
        discard;
    }
    if material.alpha_mode == 0u {
        alpha = 1.0;
    }
    alpha = apply_mesh_blend_alpha(in, material, alpha);
    if material.meshlet_debug_view {
        return vec4<f32>(color.rgb, 1.0);
    }

    let shaded = color.rgb * base_sample.rgb + emissive;
    return vec4<f32>(shaded, alpha);
}
