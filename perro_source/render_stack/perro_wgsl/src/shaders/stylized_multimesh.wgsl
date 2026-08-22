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
    let alpha_mode = in.packed_material_params & 0x3u;
    var material_alpha = base_color.a;
    if alpha_mode == 0u {
        material_alpha = 1.0;
    }
    if alpha_mode == 1u {
        let cutoff = perro_unpack_unorm8(in.packed_material_params, 16u);
        if material_alpha < cutoff {
            discard;
        }
    }
    let alpha = perro_mesh_blend_alpha(in.frag_pos, in.world_pos, in.packed_blend_params)
        * material_alpha;
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
    let flags = (in.packed_material_params >> 3u) & 0x1fffu;
    let mirrored_winding = (flags & 0x20u) != 0u;
    var n = normalize(in.normal_ws);
    if (flags & 0x2u) != 0u {
        n = normalize(cross(perro_d_world_ddx, perro_d_world_ddy));
        if mirrored_winding {
            n = -n;
        }
    }
    let double_sided = ((in.packed_material_params >> 2u) & 0x1u) != 0u;
    if double_sided && (in.is_front == mirrored_winding) {
        n = -n;
    }
    var albedo = base_color.rgb;
    var decal_emissive = vec3<f32>(0.0);
    if scene_decals.count.x > 0u {
        let decal_surface = perro_apply_decals(in.world_pos, albedo, n);
        albedo = decal_surface.albedo;
        n = decal_surface.normal;
        decal_emissive = decal_surface.emissive;
    }
    let v = normalize(scene.camera_pos.xyz - in.world_pos);
    let hemi = clamp(n.y * 0.5 + 0.5, 0.0, 1.0);
    var light_rgb =
        mix(scene.ground_color.xyz, scene.ambient_color.xyz * scene.ambient_color.w, hemi);
    let bleed = perro_decode_local_bleed(in.packed_bleed);
    let bleed_wrap = clamp(dot(n, bleed.dir) * 0.5 + 0.5, 0.0, 1.0);
    light_rgb += bleed.color * bleed.strength * 0.4 * (0.35 + 0.65 * bleed_wrap);

    let ray_count = u32(scene.ambient_and_counts.x);
    for (var i = 0u; i < ray_count; i = i + 1u) {
        let ray = scene.ray_lights[i];
        let ray_dir = ray.direction.xyz;
        let light = -ray_dir * inverseSqrt(max(dot(ray_dir, ray_dir), 1.0e-8));
        let radiance = ray.color_intensity.xyz * ray.color_intensity.w;
        light_rgb += radiance * perro_lambert(n, light);
    }
    light_rgb = perro_toon_quantize_light(light_rgb, band_count);

    let rim_power = 2.0 + max(rim_width_in, 0.0) * 4.0;
    let rim =
        pow(1.0 - max(dot(n, v), 0.0), rim_power) * max(rim_strength_in, 0.0);
    let alpha_mode = in.packed_material_params & 0x3u;
    var material_alpha = base_color.a;
    if alpha_mode == 0u {
        material_alpha = 1.0;
    }
    if alpha_mode == 1u {
        let cutoff = perro_unpack_unorm8(in.packed_material_params, 16u);
        if material_alpha < cutoff {
            discard;
        }
    }
    let alpha = perro_mesh_blend_alpha(in.frag_pos, in.world_pos, in.packed_blend_params)
        * material_alpha;
    return vec4<f32>(
        albedo * light_rgb + emissive + decal_emissive + rim,
        alpha,
    );
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
