fn lambert(n: vec3<f32>, l: vec3<f32>) -> f32 {
    return max(dot(n, l), 0.0);
}

fn shade_material(in: FragmentInput) -> vec4<f32> {
    let color = unpack_rgba8(in.packed_color);
    let emissive = perro_unpack_emissive_hdr(in.packed_emissive);
    let toon = decode_toon_params(in.packed_pbr_params_0, in.packed_pbr_params_1);
    let material = perro_decode_material_params(in.packed_material_params);
    var albedo = color.rgb;
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
    var alpha = clamp(color.a, 0.0, 1.0);
    if material.alpha_mode == 1u && alpha < material.alpha_cutoff {
        discard;
    }
    if material.alpha_mode == 0u {
        alpha = 1.0;
    }
    alpha *= mesh_fade;
    if material.meshlet_debug_view {
        return vec4<f32>(color.rgb, 1.0);
    }

    var light_rgb = vec3<f32>(0.0);
    // Hemisphere ambient: sky radiance from above, ground bounce from below.
    let hemi = clamp(n.y * 0.5 + 0.5, 0.0, 1.0);
    let ambient =
        mix(scene.ground_color.xyz, scene.ambient_color.xyz * scene.ambient_color.w, hemi);
    light_rgb += ambient;
    // Local color bleed folded into the banded light term.
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
        light_rgb += radiance * lambert(n, l);
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
            // if-branch, not select: select evaluates the PCF arm unconditionally.
            var shadow_vis = 1.0;
            if material.receive_shadows {
                shadow_vis = perro_point_shadow_factor(in.world_pos, n, i, to_light);
            }
            light_rgb += radiance * attenuation * shadow_vis * lambert(n, l);
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
            let t = clamp((cos_theta - outer_cos) / max(inner_cos - outer_cos, 0.0001), 0.0, 1.0);
            let radiance = light.color_intensity.xyz * light.color_intensity.w * t;
            let attenuation = perro_range_attenuation(dist_sq, range_sq);
            // if-branch, not select: select evaluates the PCF arm unconditionally.
            var shadow_vis = 1.0;
            if material.receive_shadows {
                shadow_vis = perro_spot_shadow_factor(in.world_pos, n, i);
            }
            light_rgb += radiance * attenuation * shadow_vis * lambert(n, l);
        }
    }

    let band_count = max(1.0, toon.x);
    // Quantize on luma, not vector length: length skews colored light into
    // wrong bands and the old clamp collapsed bright light into the top band.
    let luma = dot(light_rgb, vec3<f32>(0.2126, 0.7152, 0.0722));
    let step = 1.0 / band_count;
    let quant = floor(luma / step) * step;
    if luma > 0.0001 {
        light_rgb *= quant / luma;
    }

    let rim_strength = max(toon.y, 0.0);
    let outline_width = max(toon.z, 0.0);
    let rim_power = 2.0 + outline_width * 4.0;
    let rim = pow(1.0 - max(dot(n, v), 0.0), rim_power) * rim_strength;

    let shaded = albedo * light_rgb + emissive + decal_emissive + rim;
    return vec4<f32>(shaded, alpha);
}
