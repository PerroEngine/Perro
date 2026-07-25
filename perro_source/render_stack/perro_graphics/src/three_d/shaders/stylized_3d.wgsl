fn perro_lambert(normal: vec3<f32>, light: vec3<f32>) -> f32 {
    return max(dot(normal, light), 0.0);
}

fn perro_toon_quantize_light(light_rgb: vec3<f32>, band_count_in: f32) -> vec3<f32> {
    let band_count = max(round(band_count_in), 1.0);
    let luma = dot(light_rgb, vec3<f32>(0.2126, 0.7152, 0.0722));
    let step = 1.0 / band_count;
    let quantized_luma = floor(luma / step) * step;
    if luma > 0.0001 {
        return light_rgb * (quantized_luma / luma);
    }
    return light_rgb;
}

fn perro_unlit(
    in: FragmentInput,
    base_color: vec4<f32>,
    emissive: vec3<f32>,
) -> vec4<f32> {
    let material = perro_decode_material_params(in.packed_material_params);
    let alpha = perro_material_alpha(in, base_color.a);
    if material.meshlet_debug_view {
        return vec4<f32>(base_color.rgb, 1.0);
    }
    return vec4<f32>(base_color.rgb + emissive, alpha);
}

fn perro_toon(
    in: FragmentInput,
    base_color: vec4<f32>,
    band_count: f32,
    rim_strength_in: f32,
    rim_width_in: f32,
    emissive: vec3<f32>,
) -> vec4<f32> {
    let material = perro_decode_material_params(in.packed_material_params);
    var albedo = base_color.rgb;
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
    let mesh_fade = perro_mesh_blend_fade(in, material);
    n = perro_apply_mesh_normal_blend(material, n, in.world_pos, mesh_fade);
    var decal_emissive = vec3<f32>(0.0);
    if scene_decals.count.x > 0u {
        let decal_surface = perro_apply_decals(in.world_pos, albedo, n);
        albedo = decal_surface.albedo;
        n = decal_surface.normal;
        decal_emissive = decal_surface.emissive;
    }
    let v = normalize(scene.camera_pos.xyz - in.world_pos);
    let alpha = perro_material_alpha_with_fade(in, base_color.a, mesh_fade);
    if material.meshlet_debug_view {
        return vec4<f32>(albedo, 1.0);
    }

    var light_rgb = vec3<f32>(0.0);
    let hemi = clamp(n.y * 0.5 + 0.5, 0.0, 1.0);
    let ambient =
        mix(scene.ground_color.xyz, scene.ambient_color.xyz * scene.ambient_color.w, hemi);
    light_rgb += ambient;
    if (material.material_flags & 0x80u) != 0u {
        let bleed = perro_decode_local_bleed(in.packed_pbr_params_1);
        let wrap = clamp(dot(n, bleed.dir) * 0.5 + 0.5, 0.0, 1.0);
        light_rgb += bleed.color * bleed.strength * 0.4 * (0.35 + 0.65 * wrap);
    }

    let ray_count = u32(scene.ambient_and_counts.x);
    for (var i = 0u; i < ray_count; i = i + 1u) {
        let ray = scene.ray_lights[i];
        let ray_dir = ray.direction.xyz;
        let l = -ray_dir * inverseSqrt(max(dot(ray_dir, ray_dir), 1.0e-8));
        var radiance = ray.color_intensity.xyz * ray.color_intensity.w;
        if i == 0u && material.receive_shadows {
            radiance *= perro_shadow_factor(in.world_pos, n, l);
        }
        light_rgb += radiance * perro_lambert(n, l);
    }

    let point_count = u32(scene.ambient_and_counts.y);
    for (var i = 0u; i < point_count; i = i + 1u) {
        let light = scene.point_lights[i];
        let to_light = light.position_range.xyz - in.world_pos;
        let dist_sq = dot(to_light, to_light);
        let range_sq = light.position_range.w * light.position_range.w;
        if dist_sq <= range_sq {
            let inv_dist = inverseSqrt(max(dist_sq, 1.0e-8));
            let l = to_light * inv_dist;
            let radiance = light.color_intensity.xyz * light.color_intensity.w;
            let attenuation = perro_range_attenuation(dist_sq, range_sq);
            var shadow_vis = 1.0;
            if material.receive_shadows {
                shadow_vis = perro_point_shadow_factor(in.world_pos, n, i, to_light);
            }
            light_rgb +=
                radiance * attenuation * shadow_vis * perro_lambert(n, l);
        }
    }

    let spot_count = u32(scene.ambient_and_counts.z);
    for (var i = 0u; i < spot_count; i = i + 1u) {
        let light = scene.spot_lights[i];
        let to_light = light.position_range.xyz - in.world_pos;
        let dist_sq = dot(to_light, to_light);
        let range_sq = light.position_range.w * light.position_range.w;
        if dist_sq <= range_sq {
            let inv_dist = inverseSqrt(max(dist_sq, 1.0e-8));
            let l = to_light * inv_dist;
            let spot_dir = light.direction_outer_cos.xyz;
            let cos_theta = dot(spot_dir, -l);
            let outer_cos = light.direction_outer_cos.w;
            let inner_cos = light.inner_cos_pad.x;
            let cone =
                clamp((cos_theta - outer_cos) / max(inner_cos - outer_cos, 0.0001), 0.0, 1.0);
            let radiance = light.color_intensity.xyz * light.color_intensity.w * cone;
            let attenuation = perro_range_attenuation(dist_sq, range_sq);
            var shadow_vis = 1.0;
            if material.receive_shadows {
                shadow_vis = perro_spot_shadow_factor(in.world_pos, n, i);
            }
            light_rgb +=
                radiance * attenuation * shadow_vis * perro_lambert(n, l);
        }
    }

    light_rgb = perro_toon_quantize_light(light_rgb, band_count);
    let rim_strength = max(rim_strength_in, 0.0);
    let rim_width = max(rim_width_in, 0.0);
    let rim_power = 2.0 + rim_width * 4.0;
    let rim = pow(1.0 - max(dot(n, v), 0.0), rim_power) * rim_strength;
    let shaded = albedo * light_rgb + emissive + decal_emissive + rim;
    return vec4<f32>(shaded, alpha);
}

// Legacy alias. Use perro_toon in new shaders.
fn perro_lit_toon(
    in: FragmentInput,
    base_color: vec4<f32>,
    band_count: f32,
    rim_strength: f32,
    rim_width: f32,
    emissive: vec3<f32>,
) -> vec4<f32> {
    return perro_toon(
        in,
        base_color,
        band_count,
        rim_strength,
        rim_width,
        emissive,
    );
}

fn perro_hand_drawn(
    in: FragmentInput,
    base_color: vec4<f32>,
    band_count: f32,
    hatch_scale: f32,
    grain_strength: f32,
    emissive: vec3<f32>,
) -> vec4<f32> {
    var shaded = perro_toon(in, base_color, band_count, 0.18, 0.12, emissive);
    let luma = dot(shaded.rgb, vec3<f32>(0.2126, 0.7152, 0.0722));
    let ink = perro_crosshatch(in.uv, 1.0 - clamp(luma, 0.0, 1.0), hatch_scale, 0.785398, 0.08);
    let grain = perro_paper_grain(in.uv, hatch_scale * 5.333333, grain_strength);
    shaded = vec4<f32>(
        mix(shaded.rgb + vec3<f32>(grain), vec3<f32>(0.035), ink * 0.8),
        shaded.a,
    );
    return shaded;
}

fn perro_pixel_surface(
    in: FragmentInput,
    base_color: vec4<f32>,
    color_levels: f32,
    dither_strength: f32,
    emissive: vec3<f32>,
) -> vec4<f32> {
    var shaded = perro_toon(in, base_color, 4.0, 0.0, 0.0, emissive);
    shaded = vec4<f32>(
        perro_posterize(
            perro_bayer_dither(shaded.rgb, in.frag_pos.xy, dither_strength),
            color_levels,
        ),
        shaded.a,
    );
    return shaded;
}
