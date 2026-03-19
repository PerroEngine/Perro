fn lambert(n: vec3<f32>, l: vec3<f32>) -> f32 {
    return max(dot(n, l), 0.0);
}

fn shade_material(in: FragmentInput) -> vec4<f32> {
    let albedo = in.color.rgb;
    let double_sided = in.material_params.z > 0.5;
    var n = normalize(in.normal_ws);
    if double_sided && !in.is_front {
        n = -n;
    }
    let v = normalize(scene.camera_pos.xyz - in.world_pos);
    let alpha_mode = u32(in.material_params.x + 0.5);
    let alpha_cutoff = clamp(in.material_params.y, 0.0, 1.0);
    let meshlet_debug_view = in.material_params.w > 0.5;
    var alpha = clamp(in.color.a, 0.0, 1.0);
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
    let ambient = scene.ambient_color.xyz * scene.ambient_color.w;
    light_rgb += ambient;

    if scene.ambient_and_counts.w > 0.5 {
        let dir = normalize(scene.ray_light.direction.xyz);
        let l = -dir;
        let radiance = scene.ray_light.color_intensity.xyz * scene.ray_light.color_intensity.w;
        light_rgb += radiance * lambert(n, l);
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
            light_rgb += radiance * attenuation * lambert(n, l);
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
            light_rgb += radiance * attenuation * lambert(n, l);
        }
    }

    let band_count = max(1.0, in.pbr_params.x);
    let intensity = clamp(length(light_rgb), 0.0, 1.0);
    let step = 1.0 / band_count;
    let quant = floor(intensity / step) * step;
    if intensity > 0.0001 {
        light_rgb *= quant / intensity;
    }

    let rim_strength = max(in.pbr_params.y, 0.0);
    let outline_width = max(in.pbr_params.z, 0.0);
    let rim_power = 2.0 + outline_width * 4.0;
    let rim = pow(1.0 - max(dot(n, v), 0.0), rim_power) * rim_strength;

    let color = albedo * light_rgb + in.emissive_factor + rim;
    return vec4<f32>(color, alpha);
}
