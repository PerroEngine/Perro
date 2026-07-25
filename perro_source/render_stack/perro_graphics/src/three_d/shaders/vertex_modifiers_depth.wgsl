fn perro_depth_axis(axis: f32) -> vec3<f32> {
    if axis < 0.5 { return vec3<f32>(1.0, 0.0, 0.0); }
    if axis < 1.5 { return vec3<f32>(0.0, 1.0, 0.0); }
    return vec3<f32>(0.0, 0.0, 1.0);
}

fn perro_depth_axis_mask(position: vec3<f32>, axis: f32, start: f32, end: f32) -> f32 {
    let coord = dot(position, perro_depth_axis(axis));
    let span = end - start;
    if abs(span) < 0.000001 {
        return select(0.0, 1.0, coord >= start);
    }
    return clamp((coord - start) / span, 0.0, 1.0);
}

fn perro_depth_rotate(value: vec3<f32>, axis: vec3<f32>, angle: f32) -> vec3<f32> {
    let unit_axis = normalize(axis);
    let c = cos(angle);
    let s = sin(angle);
    return value * c
        + cross(unit_axis, value) * s
        + unit_axis * dot(unit_axis, value) * (1.0 - c);
}

fn perro_depth_transform_normal(
    row_0: vec3<f32>,
    row_1: vec3<f32>,
    row_2: vec3<f32>,
    normal: vec3<f32>,
) -> vec3<f32> {
    let cof_0 = cross(row_1, row_2);
    let cof_1 = cross(row_2, row_0);
    let cof_2 = cross(row_0, row_1);
    let det = dot(row_0, cof_0);
    if abs(det) <= 0.00000001 {
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

fn perro_depth_hash3(position: vec3<f32>, seed: f32) -> vec3<f32> {
    return fract(sin(vec3<f32>(
        dot(position, vec3<f32>(127.1, 311.7, 74.7)) + seed,
        dot(position, vec3<f32>(269.5, 183.3, 246.1)) + seed * 1.37,
        dot(position, vec3<f32>(113.5, 271.9, 124.6)) + seed * 2.11,
    )) * 43758.5453) * 2.0 - 1.0;
}

fn perro_depth_mask(position: vec3<f32>, record: array<vec4<f32>, 4>) -> f32 {
    if record[3].x < 0.5 { return 1.0; }
    return perro_depth_axis_mask(position, record[3].y, record[3].z, record[3].w);
}

struct PerroDepthVertex {
    position: vec3<f32>,
    normal: vec3<f32>,
}

fn perro_depth_apply_record(
    vertex_in: PerroDepthVertex,
    record: array<vec4<f32>, 4>,
) -> PerroDepthVertex {
    var vertex = vertex_in;
    let kind = u32(record[0].x + 0.5);
    let mask = perro_depth_mask(vertex.position, record);
    if kind == 0u {
        let direction = normalize(record[1].xyz);
        let phase = scene.time_params.x * record[2].x
            + dot(vertex.position, vec3<f32>(0.73, 0.0, 0.41)) * record[2].y;
        vertex.position += direction * sin(phase) * record[1].w * mask;
    } else if kind == 1u {
        let phase = dot(vertex.position, perro_depth_axis(record[0].y)) * record[2].y
            + scene.time_params.x * record[2].x
            + record[2].z;
        vertex.position += normalize(record[1].xyz) * sin(phase) * record[1].w * mask;
    } else if kind == 2u {
        let along = perro_depth_axis(record[0].y);
        let amount = perro_depth_axis_mask(vertex.position, record[0].y, record[1].y, record[1].z);
        let pivot = along * record[1].y;
        let bend_axis = perro_depth_axis(record[0].z);
        let angle = record[1].x * amount;
        vertex.position = pivot + perro_depth_rotate(vertex.position - pivot, bend_axis, angle);
        vertex.normal = perro_depth_rotate(vertex.normal, bend_axis, angle);
    } else if kind == 3u {
        let amount = perro_depth_axis_mask(vertex.position, record[0].y, record[1].y, record[1].z);
        let twist_axis = perro_depth_axis(record[0].y);
        let angle = record[1].x * amount;
        vertex.position = perro_depth_rotate(vertex.position, twist_axis, angle);
        vertex.normal = perro_depth_rotate(vertex.normal, twist_axis, angle);
    } else if kind == 4u {
        vertex.position += normalize(vertex.normal) * record[1].x * mask;
    } else if kind == 5u {
        let tick = floor(scene.time_params.x * max(record[1].z, 0.0));
        vertex.position += perro_depth_hash3(vertex.position * record[1].y, record[1].w + tick * 17.0)
            * record[1].x * mask;
    }
    return vertex;
}

fn perro_depth_apply_vertex_modifiers(
    position_in: vec3<f32>,
    normal_in: vec3<f32>,
    custom_range: vec2<u32>,
) -> vec4<f32> {
    var vertex = PerroDepthVertex(position_in, normal_in);
    if custom_range.x >= 2u {
        let header = custom_range.x - 2u;
        let value_offset = custom_params_meta[header];
        let count = min(custom_params_meta[header + 1u], 16u);
        for (var i = 0u; i < count; i = i + 1u) {
            let base = value_offset + i * 16u;
            let record = array<vec4<f32>, 4>(
                vec4<f32>(custom_params_values[base], custom_params_values[base + 1u], custom_params_values[base + 2u], custom_params_values[base + 3u]),
                vec4<f32>(custom_params_values[base + 4u], custom_params_values[base + 5u], custom_params_values[base + 6u], custom_params_values[base + 7u]),
                vec4<f32>(custom_params_values[base + 8u], custom_params_values[base + 9u], custom_params_values[base + 10u], custom_params_values[base + 11u]),
                vec4<f32>(custom_params_values[base + 12u], custom_params_values[base + 13u], custom_params_values[base + 14u], custom_params_values[base + 15u]),
            );
            vertex = perro_depth_apply_record(vertex, record);
        }
    }
    var clip = scene.view_proj * vec4<f32>(vertex.position, 1.0);
    if custom_range.x >= 2u {
        let header = custom_range.x - 2u;
        let value_offset = custom_params_meta[header];
        let count = min(custom_params_meta[header + 1u], 16u);
        for (var i = 0u; i < count; i = i + 1u) {
            let base = value_offset + i * 16u;
            if u32(custom_params_values[base] + 0.5) == 6u {
                let height = max(custom_params_values[base + 4u], 1.0);
                let aspect = max(scene.resolution.x / max(scene.resolution.y, 1.0), 0.000001);
                let grid = vec2<f32>(max(round(height * aspect), 1.0), height);
                let ndc = clip.xy / max(abs(clip.w), 0.000001);
                let snapped = (floor((ndc * 0.5 + 0.5) * grid) + vec2<f32>(0.5)) / grid;
                let snapped_ndc = snapped * 2.0 - 1.0;
                clip = vec4<f32>(
                    mix(ndc, snapped_ndc, clamp(custom_params_values[base + 5u], 0.0, 1.0)) * clip.w,
                    clip.zw,
                );
            }
        }
    }
    return clip;
}
