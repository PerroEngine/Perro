fn shade_material(in: FragmentInput) -> vec4<f32> {
    let base_sample = textureSample(material_base_color_tex, material_sampler, in.uv);
    let albedo = in.color.rgb * base_sample.rgb;
    let double_sided = in.material_params.z > 0.5;
    let material_flags = u32(in.material_params.w + 0.5);
    let meshlet_debug_view = (material_flags & 1u) != 0u;
    let flat_shading = (material_flags & 2u) != 0u;
    var n = normalize(in.normal_ws);
    if flat_shading {
        n = normalize(cross(dpdx(in.world_pos), dpdy(in.world_pos)));
    }
    if double_sided && !in.is_front {
        n = -n;
    }
    let v = normalize(scene.camera_pos.xyz - in.world_pos);
    let roughness = clamp(in.pbr_params.x, 0.04, 1.0);
    let metallic = clamp(in.pbr_params.y, 0.0, 1.0);
    let ao = clamp(in.pbr_params.z, 0.0, 1.0);
    let alpha_mode = u32(in.material_params.x + 0.5);
    let alpha_cutoff = clamp(in.material_params.y, 0.0, 1.0);
    var alpha = clamp(in.color.a * base_sample.a, 0.0, 1.0);
    if alpha_mode == 1u && alpha < alpha_cutoff {
        discard;
    }
    if alpha_mode == 0u {
        alpha = 1.0;
    }
    if meshlet_debug_view {
        return vec4<f32>(in.color.rgb, 1.0);
    }

    var light_rgb = vec3<f32>(0.0);

    let ray_count = u32(scene.ambient_and_counts.x);
    for (var i = 0u; i < ray_count; i = i + 1u) {
        let ray = scene.ray_lights[i];
        let dir = normalize(ray.direction.xyz);
        let l = -dir;
        let radiance = ray.color_intensity.xyz * ray.color_intensity.w;
        light_rgb += brdf_pbr(albedo, n, v, l, roughness, metallic, radiance);
    }

    let point_count = u32(scene.ambient_and_counts.y);
    for (var i = 0u; i < point_count; i = i + 1u) {
        let light = scene.point_lights[i];
        let to_light = light.position_range.xyz - in.world_pos;
        let dist = length(to_light);
        if dist <= light.position_range.w {
            let l = to_light / max(dist, 0.0001);
            let radiance = light.color_intensity.xyz * light.color_intensity.w;
            let attenuation = 1.0 / max(dist * dist, 1.0);
            light_rgb += brdf_pbr(albedo, n, v, l, roughness, metallic, radiance * attenuation);
        }
    }

    let spot_count = u32(scene.ambient_and_counts.z);
    for (var i = 0u; i < spot_count; i = i + 1u) {
        let light = scene.spot_lights[i];
        let to_light = light.position_range.xyz - in.world_pos;
        let dist = length(to_light);
        if dist <= light.position_range.w {
            let l = to_light / max(dist, 0.0001);
            let cos_theta = dot(normalize(light.direction_outer_cos.xyz), -l);
            let outer_cos = light.direction_outer_cos.w;
            let inner_cos = light.inner_cos_pad.x;
            let t = clamp((cos_theta - outer_cos) / max(inner_cos - outer_cos, 0.0001), 0.0, 1.0);
            let radiance = light.color_intensity.xyz * light.color_intensity.w * t;
            let attenuation = 1.0 / max(dist * dist, 1.0);
            light_rgb += brdf_pbr(albedo, n, v, l, roughness, metallic, radiance * attenuation);
        }
    }

    let f_ambient = fresnel_schlick_roughness(max(dot(n, v), 0.0), vec3<f32>(0.04), roughness);
    let k_s_ambient = f_ambient;
    let k_d_ambient = (vec3<f32>(1.0) - k_s_ambient) * (1.0 - metallic);
    let ambient_radiance = scene.ambient_color.xyz * scene.ambient_color.w * ao;
    let ambient_diffuse = k_d_ambient * albedo * ambient_radiance;
    let ambient_specular = k_s_ambient * ambient_radiance * (0.25 + 0.75 * (1.0 - roughness));

    let color = ambient_diffuse + ambient_specular + light_rgb + in.emissive_factor;
    return vec4<f32>(color, alpha);
}
