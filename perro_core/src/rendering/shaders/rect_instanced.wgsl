struct Camera {
    virtual_size: vec2<f32>,
    ndc_scale: vec2<f32>,
    zoom: f32,
    _pad0: f32,
    _pad1: vec2<f32>,
    view: mat4x4<f32>,
};

@group(0) @binding(0)
var<uniform> camera: Camera;

struct VertexInput {
    @location(0) position: vec2<f32>,
    @location(1) uv: vec2<f32>,
}

struct InstanceInput {
    // Mat3 (column-major)
    @location(2) transform_0: vec3<f32>,
    @location(3) transform_1: vec3<f32>,
    @location(4) transform_2: vec3<f32>,

    @location(5) color: vec4<f32>,
    @location(6) size: vec2<f32>,
    @location(7) pivot: vec2<f32>,
    @location(8) corner_radius_xy: vec4<f32>,
    @location(9) corner_radius_zw: vec4<f32>,
    @location(10) border_thickness: f32,
    @location(11) is_border: u32,
    @location(12) z_index: i32,
}

struct VertexOutput {
    @builtin(position) pos: vec4<f32>,
    @location(0) local_pos: vec2<f32>,
    @location(1) color: vec4<f32>,
    @location(2) size: vec2<f32>,
    @location(3) corner_radius_xy: vec4<f32>,
    @location(4) corner_radius_zw: vec4<f32>,
    @location(5) border_thickness: f32,
    @location(6) is_border: u32,
}

// ─────────────────────────────────────────────
// Mat3 → Mat4 helper
// ─────────────────────────────────────────────
fn mat3_to_mat4(
    t0: vec3<f32>,
    t1: vec3<f32>,
    t2: vec3<f32>,
) -> mat4x4<f32> {
    return mat4x4<f32>(
        vec4<f32>(t0.xy, 0.0, t0.z),
        vec4<f32>(t1.xy, 0.0, t1.z),
        vec4<f32>(0.0, 0.0, 1.0, 0.0),
        vec4<f32>(t2.xy, 0.0, 1.0),
    );
}

@vertex
fn vs_main(vertex: VertexInput, instance: InstanceInput) -> VertexOutput {
    // Build transform from Mat3
    let transform = mat3_to_mat4(
        instance.transform_0,
        instance.transform_1,
        instance.transform_2,
    );

    // ✅ EXACT SAME pivot logic as original
    let pivot_centered = instance.pivot - vec2<f32>(0.5, 0.5);
    let pivot_offset = pivot_centered * instance.size;

    let local_vertex_pos = vertex.position * instance.size;
    let adjusted_local_pos = local_vertex_pos - pivot_offset;

    // Transform to world space
    var world_pos4 = transform * vec4<f32>(adjusted_local_pos, 0.0, 1.0);

    // Apply camera view
    world_pos4 = camera.view * world_pos4;

    // Apply zoom
    world_pos4 = vec4<f32>(
        world_pos4.xy * (1.0 + camera.zoom),
        world_pos4.z,
        world_pos4.w,
    );

    // Convert to NDC
    let ndc_pos = world_pos4.xy * camera.ndc_scale;

    // Depth from z_index
    let depth = 1.0 - f32(instance.z_index) * 0.001;

    let fragment_local_pos = vertex.position * instance.size;

    var out: VertexOutput;
    out.pos = vec4<f32>(ndc_pos, depth, 1.0);
    out.local_pos = fragment_local_pos;
    out.color = instance.color;
    out.size = instance.size;
    out.corner_radius_xy = instance.corner_radius_xy;
    out.corner_radius_zw = instance.corner_radius_zw;
    out.border_thickness = instance.border_thickness;
    out.is_border = instance.is_border;
    return out;
}

// ─────────────────────────────────────────────
// SDF helpers (UNCHANGED)
// ─────────────────────────────────────────────
fn get_corner_radius(
    p: vec2<f32>,
    corner_radii: vec4<f32>,
    corner_radii2: vec4<f32>,
) -> f32 {
    if (p.x <= 0.0 && p.y >= 0.0) {
        return corner_radii.x;
    } else if (p.x > 0.0 && p.y >= 0.0) {
        return corner_radii.z;
    } else if (p.x > 0.0 && p.y < 0.0) {
        return corner_radii2.x;
    } else {
        return corner_radii2.z;
    }
}

fn sdf_rounded_box(
    p: vec2<f32>,
    size: vec2<f32>,
    corner_radii: vec4<f32>,
    corner_radii2: vec4<f32>,
) -> f32 {
    let half_size = size * 0.5;
    let radius = get_corner_radius(p, corner_radii, corner_radii2);
    let q = abs(p) - half_size + radius;
    return min(max(q.x, q.y), 0.0)
        + length(max(q, vec2<f32>(0.0)))
        - radius;
}

@fragment
fn fs_main(in: VertexOutput) -> @location(0) vec4<f32> {
    let p = in.local_pos;

    let edge_softness = 0.8;
    let edge_min = -edge_softness;
    let edge_max = edge_softness;

    let outer_dist =
        sdf_rounded_box(p, in.size, in.corner_radius_xy, in.corner_radius_zw);

    let half_size = in.size * 0.5;
    let max_radius = max(
        max(in.corner_radius_xy.x, in.corner_radius_xy.z),
        max(in.corner_radius_zw.x, in.corner_radius_zw.z),
    );
    let rough_bounds =
        max(half_size.x, half_size.y) + max_radius + edge_softness;

    if (abs(p.x) > rough_bounds || abs(p.y) > rough_bounds) {
        discard;
    }

    let outer_alpha = 1.0 - smoothstep(edge_min, edge_max, outer_dist);

    if (outer_alpha <= 0.0) {
        discard;
    }

    var final_alpha = outer_alpha;

    if (in.is_border == 1u) {
        let border_thickness = in.border_thickness;
        let inner_size =
            in.size - vec2<f32>(border_thickness * 2.0);

        if (inner_size.x <= 0.0 || inner_size.y <= 0.0) {
            return vec4<f32>(in.color.rgb, in.color.a * outer_alpha);
        }

        let border_reduction = vec4<f32>(border_thickness);
        let inner_corner_xy =
            max(in.corner_radius_xy - border_reduction, vec4<f32>(0.0));
        let inner_corner_zw =
            max(in.corner_radius_zw - border_reduction, vec4<f32>(0.0));

        let inner_dist =
            sdf_rounded_box(p, inner_size, inner_corner_xy, inner_corner_zw);

        let inner_alpha =
            1.0 - smoothstep(edge_min, edge_max, inner_dist);

        final_alpha = outer_alpha * (1.0 - inner_alpha);
    }

    if (final_alpha <= 0.0) {
        discard;
    }

    return vec4<f32>(in.color.rgb, in.color.a * final_alpha);
}