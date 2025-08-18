struct VertexOut {
    @builtin(position) pos: vec4<f32>,
    @location(0) local_pos: vec2<f32>,
};

struct RectUniform {
    transform: mat4x4<f32>,
    color: vec4<f32>,
    size: vec2<f32>,
    pivot: vec2<f32>,
    corner_radius: array<vec4<f32>, 4>,
    border_thickness: f32,
    is_border: u32,
    _pad: vec2<f32>,
};

// u_camera.x = virtual width, u_camera.y = virtual height
@group(1) @binding(0)
var<uniform> u_camera: vec4<f32>;

@group(0) @binding(0)
var<uniform> u_rect: RectUniform;

@vertex
fn vs_main(
    @location(0) position: vec2<f32>,
    @location(1) _uv: vec2<f32>
) -> VertexOut {
    var out: VertexOut;

    let pivot_offset = (u_rect.pivot - vec2<f32>(0.5, 0.5)) * u_rect.size;
    let scaled = (position * u_rect.size) - pivot_offset;
    let world_pos = u_rect.transform * vec4(scaled, 0.0, 1.0);

    // Convert virtual pixels → NDC
    // u_camera.x = virtual width, u_camera.y = virtual height
    let ndc_x = (world_pos.x / u_camera.x) * 2.0;
    let ndc_y = (world_pos.y / u_camera.y) * 2.0;

    out.pos = vec4(ndc_x, ndc_y, world_pos.z, world_pos.w);
    out.local_pos = position;
    return out;
}

@fragment
fn fs_main(in: VertexOut) -> @location(0) vec4<f32> {
    let p = in.local_pos;

    // Pick correct corner radius
    var radius: vec2<f32>;
    if (p.x < 0.0 && p.y > 0.0) {
        radius = u_rect.corner_radius[0].xy;
    } else if (p.x > 0.0 && p.y > 0.0) {
        radius = u_rect.corner_radius[1].xy;
    } else if (p.x > 0.0 && p.y < 0.0) {
        radius = u_rect.corner_radius[2].xy;
    } else {
        radius = u_rect.corner_radius[3].xy;
    }

    let half_size = vec2<f32>(0.5, 0.5);

    // Outer rounded rect distance
    let q_outer = abs(p) - (half_size - radius);
    let dist_outer = length(max(q_outer, vec2<f32>(0.0))) - min(radius.x, radius.y);

    // Discard outside outer shape
    if (dist_outer > 0.0) {
        discard;
    }

    if (u_rect.is_border == 1u) {
        // Inner rounded rect for border
        let inner_half_size = half_size - vec2<f32>(u_rect.border_thickness / u_rect.size.x,
                                                    u_rect.border_thickness / u_rect.size.y);
        let inner_radius = max(radius - vec2<f32>(u_rect.border_thickness / u_rect.size.x,
                                                  u_rect.border_thickness / u_rect.size.y), vec2<f32>(0.0));
        let q_inner = abs(p) - (inner_half_size - inner_radius);
        let dist_inner = length(max(q_inner, vec2<f32>(0.0))) - min(inner_radius.x, inner_radius.y);

        // Discard inside inner shape
        if (dist_inner < 0.0) {
            discard;
        }
    }

    return u_rect.color;
}