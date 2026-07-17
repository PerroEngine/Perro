// Shared 3D shader library: fns used verbatim by BOTH the rigid/skinned
// prelude pipeline and the multimesh pipeline. Concatenated ahead of each
// at module build (WGSL module-scope decls are order-independent), so a fn
// lives here only if its body is byte-identical for both paths. Anything
// divergent (perro_unpack_unorm8, perro_apply_blend_shapes, the ggx/lit family) stays
// in the owning file.

fn custom_image_sample_at(index: u32, uv: vec2<f32>) -> vec4<f32> {
    if index == 0u {
        return textureSample(custom_image_tex_0, material_sampler, uv);
    }
    if index == 1u {
        return textureSample(custom_image_tex_1, material_sampler, uv);
    }
    if index == 2u {
        return textureSample(custom_image_tex_2, material_sampler, uv);
    }
    if index == 3u {
        return textureSample(custom_image_tex_3, material_sampler, uv);
    }
    if index == 4u {
        return textureSample(custom_image_tex_4, material_sampler, uv);
    }
    if index == 5u {
        return textureSample(custom_image_tex_5, material_sampler, uv);
    }
    if index == 6u {
        return textureSample(custom_image_tex_6, material_sampler, uv);
    }
    return textureSample(custom_image_tex_7, material_sampler, uv);
}

fn perro_decal_srgb_to_linear(c: vec3<f32>) -> vec3<f32> {
    return pow(max(c, vec3<f32>(0.0)), vec3<f32>(2.2));
}

fn perro_apply_decals(world_pos: vec3<f32>, albedo_in: vec3<f32>, normal_in: vec3<f32>) -> DecalSurface {
    var out: DecalSurface;
    out.albedo = albedo_in;
    out.normal = normal_in;
    out.emissive = vec3<f32>(0.0);
    // Derivatives before any branch (uniform control flow); per-decal uv
    // gradients are linear transforms of these, keeping textureSampleGrad
    // valid inside the non-uniform loop body.
    let dpx = dpdx(world_pos);
    let dpy = dpdy(world_pos);
    let count = scene_decals.count.x;
    for (var i = 0u; i < count; i = i + 1u) {
        let d = scene_decals.decals[i];
        let p = vec4<f32>(world_pos, 1.0);
        let local = vec3<f32>(dot(d.inv_row_0, p), dot(d.inv_row_1, p), dot(d.inv_row_2, p));
        if any(abs(local) > vec3<f32>(0.5)) {
            continue;
        }
        // Rows of the inverse are the decal axes scaled by 1/size; the decal
        // projects along its -Z, so +Z is the receiving direction.
        let axis = normalize(d.inv_row_2.xyz);
        let facing = dot(normal_in, axis);
        let fade_t = d.params0.w;
        let angle_fade = clamp((facing - fade_t) / max(1.0 - fade_t, 0.001), 0.0, 1.0);
        var opacity = d.tint.a * angle_fade;
        if d.params1.y > 0.0 {
            let dist = distance(scene.camera_pos.xyz, world_pos);
            opacity *= clamp(1.0 - (dist - d.params1.y) * d.params1.z, 0.0, 1.0);
        }
        if opacity <= 0.001 {
            continue;
        }
        let uv = vec2<f32>(local.x + 0.5, 0.5 - local.y);
        let g0 = vec2<f32>(dot(d.inv_row_0.xyz, dpx), -dot(d.inv_row_1.xyz, dpx));
        let g1 = vec2<f32>(dot(d.inv_row_0.xyz, dpy), -dot(d.inv_row_1.xyz, dpy));
        var albedo_alpha = opacity;
        if d.params0.x >= 0.0 {
            let s = textureSampleGrad(decal_textures, decal_sampler, uv, i32(d.params0.x), g0, g1);
            albedo_alpha = s.a * opacity;
            out.albedo = mix(out.albedo, perro_decal_srgb_to_linear(s.rgb) * d.tint.rgb, albedo_alpha * d.params1.x);
        } else {
            out.albedo = mix(out.albedo, d.tint.rgb, opacity * d.params1.x);
        }
        if d.params0.y >= 0.0 && d.emission.w > 0.0 {
            let ns = textureSampleGrad(decal_textures, decal_sampler, uv, i32(d.params0.y), g0, g1);
            let nt = ns.xyz * 2.0 - vec3<f32>(1.0);
            let t_axis = normalize(d.inv_row_0.xyz);
            let b_axis = normalize(d.inv_row_1.xyz);
            // The v axis is flipped in uv space, so the bitangent negates.
            let mapped = normalize(t_axis * nt.x - b_axis * nt.y + out.normal * max(nt.z, 0.35));
            out.normal = normalize(mix(out.normal, mapped, albedo_alpha * min(d.emission.w, 1.0)));
        }
        if d.params0.z >= 0.0 {
            let es = textureSampleGrad(decal_textures, decal_sampler, uv, i32(d.params0.z), g0, g1);
            out.emissive += perro_decal_srgb_to_linear(es.rgb) * d.emission.rgb * es.a * opacity;
        }
    }
    return out;
}

fn perro_unpack_byte(packed: u32, shift: u32) -> u32 {
    return (packed >> shift) & 0xffu;
}

fn perro_decode_mesh_blend_params(packed: u32) -> vec4<f32> {
    return vec4<f32>(
        perro_unpack_unorm8(packed, 0u) * 16.0,
        perro_unpack_unorm8(packed, 8u) * 16.0,
        perro_unpack_unorm8(packed, 16u),
        perro_unpack_unorm8(packed, 24u) * 64.0,
    );
}

fn perro_mesh_blend_hash(p: vec2<f32>) -> f32 {
    return fract(sin(dot(p, vec2<f32>(12.9898, 78.233))) * 43758.5453);
}

fn perro_mesh_blend_noise(p: vec2<f32>) -> f32 {
    let cell = floor(p);
    let local = fract(p);
    let curve = local * local * (3.0 - 2.0 * local);
    let a = perro_mesh_blend_hash(cell);
    let b = perro_mesh_blend_hash(cell + vec2<f32>(1.0, 0.0));
    let c = perro_mesh_blend_hash(cell + vec2<f32>(0.0, 1.0));
    let d = perro_mesh_blend_hash(cell + vec2<f32>(1.0, 1.0));
    return mix(mix(a, b, curve.x), mix(c, d, curve.x), curve.y);
}

fn perro_mesh_blend_world_from_depth(coord: vec2<i32>, dims_u: vec2<u32>, depth: f32) -> vec3<f32> {
    let uv = (vec2<f32>(coord) + vec2<f32>(0.5)) / vec2<f32>(dims_u);
    let ndc_xy = vec2<f32>(uv.x * 2.0 - 1.0, 1.0 - uv.y * 2.0);
    let ndc = vec4<f32>(ndc_xy, depth, 1.0);
    let world_h = scene.inv_view_proj * ndc;
    return world_h.xyz / max(abs(world_h.w), 1.0e-5);
}

fn perro_transform_normal_ws(
    row_0: vec3<f32>,
    row_1: vec3<f32>,
    row_2: vec3<f32>,
    normal: vec3<f32>,
) -> vec3<f32> {
    let cof_0 = cross(row_1, row_2);
    let cof_1 = cross(row_2, row_0);
    let cof_2 = cross(row_0, row_1);
    let det = dot(row_0, cof_0);
    if abs(det) <= 1.0e-8 {
        return normalize(vec3<f32>(
            dot(row_0, normal),
            dot(row_1, normal),
            dot(row_2, normal),
        ));
    }
    let det_sign = select(-1.0, 1.0, det >= 0.0);
    return normalize(vec3<f32>(
        dot(cof_0, normal),
        dot(cof_1, normal),
        dot(cof_2, normal),
    ) * det_sign);
}

fn custom_f_param(in: FragmentInput, index: u32) -> vec4<f32> {
    if index >= in.custom_range.y {
        return vec4<f32>(0.0);
    }
    let packed_meta = custom_params_meta[in.custom_range.x + index];
    let kind = packed_meta & 0x3u;
    let value_offset = packed_meta >> 2u;
    if kind == 0u {
        return vec4<f32>(custom_params_values[value_offset], 0.0, 0.0, 0.0);
    }
    if kind == 1u {
        return vec4<f32>(
            custom_params_values[value_offset],
            custom_params_values[value_offset + 1u],
            0.0,
            0.0,
        );
    }
    if kind == 2u {
        return vec4<f32>(
            custom_params_values[value_offset],
            custom_params_values[value_offset + 1u],
            custom_params_values[value_offset + 2u],
            0.0,
        );
    }
    return vec4<f32>(
        custom_params_values[value_offset],
        custom_params_values[value_offset + 1u],
        custom_params_values[value_offset + 2u],
        custom_params_values[value_offset + 3u],
    );
}

fn custom_v_param(out: VertexOutput, index: u32) -> vec4<f32> {
    if index >= out.custom_range.y {
        return vec4<f32>(0.0);
    }
    let packed_meta = custom_params_meta[out.custom_range.x + index];
    let kind = packed_meta & 0x3u;
    let value_offset = packed_meta >> 2u;
    if kind == 0u {
        return vec4<f32>(custom_params_values[value_offset], 0.0, 0.0, 0.0);
    }
    if kind == 1u {
        return vec4<f32>(
            custom_params_values[value_offset],
            custom_params_values[value_offset + 1u],
            0.0,
            0.0,
        );
    }
    if kind == 2u {
        return vec4<f32>(
            custom_params_values[value_offset],
            custom_params_values[value_offset + 1u],
            custom_params_values[value_offset + 2u],
            0.0,
        );
    }
    return vec4<f32>(
        custom_params_values[value_offset],
        custom_params_values[value_offset + 1u],
        custom_params_values[value_offset + 2u],
        custom_params_values[value_offset + 3u],
    );
}

fn custom_param(in: FragmentInput, index: u32) -> vec4<f32> {
    return custom_f_param(in, index);
}

fn custom_param_vertex(out: VertexOutput, index: u32) -> vec4<f32> {
    return custom_v_param(out, index);
}

fn custom_image_sample(in: FragmentInput, index: u32, uv: vec2<f32>) -> vec4<f32> {
    return custom_image_sample_at(index, uv);
}

fn perro_decode_local_bleed(packed: u32) -> LocalBleed {
    let color = vec3<f32>(
        f32(packed & 0x1fu) / 31.0,
        f32((packed >> 5u) & 0x1fu) / 31.0,
        f32((packed >> 10u) & 0x1fu) / 31.0,
    );
    let strength = f32((packed >> 15u) & 0x1fu) / 31.0;
    let ox = f32((packed >> 20u) & 0x3fu) / 63.0 * 2.0 - 1.0;
    let oy = f32((packed >> 26u) & 0x3fu) / 63.0 * 2.0 - 1.0;
    return LocalBleed(color, strength, perro_oct_decode_dir(ox, oy));
}

fn perro_oct_decode_dir(x: f32, y: f32) -> vec3<f32> {
    var v = vec3<f32>(x, y, 1.0 - abs(x) - abs(y));
    if v.z < 0.0 {
        let old_x = v.x;
        v.x = (1.0 - abs(v.y)) * select(-1.0, 1.0, old_x >= 0.0);
        v.y = (1.0 - abs(old_x)) * select(-1.0, 1.0, v.y >= 0.0);
    }
    return normalize(v);
}

fn perro_environment_brdf_texel(x: u32, y: u32) -> vec2<f32> {
    let word = 137208u + (y * 128u + x) * 2u;
    return vec2<f32>(
        bitcast<f32>(environment_data[word]),
        bitcast<f32>(environment_data[word + 1u]),
    );
}

fn perro_environment_cube_coord(dir: vec3<f32>) -> EnvironmentCubeCoord {
    let absolute = abs(dir);
    var face = 0u;
    var uv = vec2<f32>(0.0);
    if absolute.x >= absolute.y && absolute.x >= absolute.z {
        let inv_axis = 1.0 / max(absolute.x, 1.0e-8);
        if dir.x >= 0.0 {
            face = 0u;
            uv = vec2<f32>(-dir.z, -dir.y) * inv_axis;
        } else {
            face = 1u;
            uv = vec2<f32>(dir.z, -dir.y) * inv_axis;
        }
    } else if absolute.y >= absolute.z {
        let inv_axis = 1.0 / max(absolute.y, 1.0e-8);
        if dir.y >= 0.0 {
            face = 2u;
            uv = vec2<f32>(dir.x, dir.z) * inv_axis;
        } else {
            face = 3u;
            uv = vec2<f32>(dir.x, -dir.z) * inv_axis;
        }
    } else {
        let inv_axis = 1.0 / max(absolute.z, 1.0e-8);
        if dir.z >= 0.0 {
            face = 4u;
            uv = vec2<f32>(dir.x, -dir.y) * inv_axis;
        } else {
            face = 5u;
            uv = vec2<f32>(-dir.x, -dir.y) * inv_axis;
        }
    }
    return EnvironmentCubeCoord(face, uv * 0.5 + vec2<f32>(0.5));
}

fn perro_environment_cube_texel(
    base_word: u32,
    size: u32,
    face: u32,
    xy: vec2<u32>,
) -> vec3<f32> {
    let texel = face * size * size + xy.y * size + xy.x;
    let word = base_word + texel * 4u;
    return vec3<f32>(
        bitcast<f32>(environment_data[word]),
        bitcast<f32>(environment_data[word + 1u]),
        bitcast<f32>(environment_data[word + 2u]),
    );
}

fn perro_environment_specular_level(mip: u32) -> vec2<u32> {
    var base_word = 6144u;
    var size = 64u;
    var level = 0u;
    loop {
        if level >= mip {
            break;
        }
        base_word += size * size * 24u;
        size = max(size >> 1u, 1u);
        level += 1u;
    }
    return vec2<u32>(base_word, size);
}

fn perro_rotate_environment_direction(dir: vec3<f32>) -> vec3<f32> {
    let rotation_sin = scene.ibl_params.z;
    let rotation_cos = scene.ibl_params.w;
    return vec3<f32>(
        rotation_cos * dir.x + rotation_sin * dir.z,
        dir.y,
        -rotation_sin * dir.x + rotation_cos * dir.z,
    );
}

fn perro_sample_environment_brdf(uv: vec2<f32>) -> vec2<f32> {
    let position = clamp(uv * 128.0 - vec2<f32>(0.5), vec2<f32>(0.0), vec2<f32>(127.0));
    let low = vec2<u32>(floor(position));
    let high = min(low + vec2<u32>(1u), vec2<u32>(127u));
    let blend = fract(position);
    let top = mix(
        perro_environment_brdf_texel(low.x, low.y),
        perro_environment_brdf_texel(high.x, low.y),
        blend.x,
    );
    let bottom = mix(
        perro_environment_brdf_texel(low.x, high.y),
        perro_environment_brdf_texel(high.x, high.y),
        blend.x,
    );
    return mix(top, bottom, blend.y);
}

fn perro_sample_environment_cube_level(base_word: u32, size: u32, dir: vec3<f32>) -> vec3<f32> {
    let coord = perro_environment_cube_coord(dir);
    let edge = f32(size - 1u);
    let position = clamp(coord.uv * f32(size) - vec2<f32>(0.5), vec2<f32>(0.0), vec2<f32>(edge));
    let low = vec2<u32>(floor(position));
    let high = min(low + vec2<u32>(1u), vec2<u32>(size - 1u));
    let blend = fract(position);
    let top = mix(
        perro_environment_cube_texel(base_word, size, coord.face, low),
        perro_environment_cube_texel(base_word, size, coord.face, vec2<u32>(high.x, low.y)),
        blend.x,
    );
    let bottom = mix(
        perro_environment_cube_texel(base_word, size, coord.face, vec2<u32>(low.x, high.y)),
        perro_environment_cube_texel(base_word, size, coord.face, high),
        blend.x,
    );
    return mix(top, bottom, blend.y);
}

fn perro_sample_environment_irradiance(dir: vec3<f32>) -> vec3<f32> {
    return perro_sample_environment_cube_level(0u, 16u, dir);
}

fn perro_sample_environment_specular(dir: vec3<f32>, lod_in: f32) -> vec3<f32> {
    let lod = clamp(lod_in, 0.0, 6.0);
    let low_mip = u32(floor(lod));
    let high_mip = min(low_mip + 1u, 6u);
    let low_level = perro_environment_specular_level(low_mip);
    let high_level = perro_environment_specular_level(high_mip);
    return mix(
        perro_sample_environment_cube_level(low_level.x, low_level.y, dir),
        perro_sample_environment_cube_level(high_level.x, high_level.y, dir),
        fract(lod),
    );
}

fn perro_time() -> f32 { return scene.time_params.x; }

fn perro_delta_time() -> f32 { return scene.time_params.y; }

fn perro_frame_index() -> f32 { return scene.time_params.z; }

fn perro_time_phase() -> f32 { return scene.time_params.w; }

fn perro_resolution() -> vec2<f32> { return scene.resolution.xy; }

fn perro_inv_resolution() -> vec2<f32> { return scene.resolution.zw; }
