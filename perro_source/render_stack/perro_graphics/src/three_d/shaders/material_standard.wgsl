fn shade_material(in: FragmentInput) -> vec4<f32> {
    let color = unpack_rgba8(in.packed_color);
    let emissive = unpack_rgba8(in.packed_emissive).xyz;
    let pbr = decode_standard_pbr_params(in.packed_pbr_params_0, in.packed_pbr_params_1);
    let material = decode_material_params(in.packed_material_params);
    let base_sample = textureSample(material_base_color_tex, material_sampler, in.uv);
    let albedo = color.rgb * base_sample.rgb;
    var n = normalize(in.normal_ws);
    if material.flat_shading {
        n = normalize(cross(dpdx(in.world_pos), dpdy(in.world_pos)));
    }
    if material.double_sided && !in.is_front {
        n = -n;
    }
    let v = normalize(scene.camera_pos.xyz - in.world_pos);
    let roughness = clamp(pbr.x, 0.04, 1.0);
    let metallic = clamp(pbr.y, 0.0, 1.0);
    let ao = clamp(pbr.z, 0.0, 1.0);
    var alpha = clamp(color.a * base_sample.a, 0.0, 1.0);
    if material.alpha_mode == 1u && alpha < material.alpha_cutoff {
        discard;
    }
    if material.alpha_mode == 0u {
        alpha = 1.0;
    }
    if material.meshlet_debug_view {
        return vec4<f32>(color.rgb, 1.0);
    }

    var light_rgb = vec3<f32>(0.0);

    let ray_count = u32(scene.ambient_and_counts.x);
    for (var i = 0u; i < ray_count; i = i + 1u) {
        let ray = scene.ray_lights[i];
        let ray_dir = ray.direction.xyz;
        let l = -ray_dir * inverseSqrt(max(dot(ray_dir, ray_dir), 1.0e-8));
        var radiance = ray.color_intensity.xyz * ray.color_intensity.w;
        if i == 0u {
            radiance *= shadow_factor(in.world_pos, n, l);
        }
        light_rgb += brdf_pbr(albedo, n, v, l, roughness, metallic, radiance);
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
            let attenuation = 1.0 / max(dist_sq, 1.0);
            light_rgb += brdf_pbr(albedo, n, v, l, roughness, metallic, radiance * attenuation);
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
            let spot_dir = light.direction_outer_cos.xyz
                * inverseSqrt(max(dot(light.direction_outer_cos.xyz, light.direction_outer_cos.xyz), 1.0e-8));
            let cos_theta = dot(spot_dir, -l);
            let outer_cos = light.direction_outer_cos.w;
            let inner_cos = light.inner_cos_pad.x;
            let t = clamp((cos_theta - outer_cos) / max(inner_cos - outer_cos, 0.0001), 0.0, 1.0);
            let radiance = light.color_intensity.xyz * light.color_intensity.w * t;
            let attenuation = 1.0 / max(dist_sq, 1.0);
            light_rgb += brdf_pbr(albedo, n, v, l, roughness, metallic, radiance * attenuation);
        }
    }

    let f_ambient = fresnel_schlick_roughness(max(dot(n, v), 0.0), vec3<f32>(0.04), roughness);
    let k_s_ambient = f_ambient;
    let k_d_ambient = (vec3<f32>(1.0) - k_s_ambient) * (1.0 - metallic);
    let ambient_radiance = scene.ambient_color.xyz * scene.ambient_color.w * ao;
    let ambient_diffuse = k_d_ambient * albedo * ambient_radiance;
    let ambient_specular = k_s_ambient * ambient_radiance * (0.25 + 0.75 * (1.0 - roughness));

    let shaded = ambient_diffuse + ambient_specular + light_rgb + emissive;
    return vec4<f32>(shaded, alpha);
}
