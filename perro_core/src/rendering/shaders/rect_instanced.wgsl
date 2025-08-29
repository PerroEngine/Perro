struct Camera {
    virtual_size: vec2<f32>,
    window_size: vec2<f32>,
}

@group(0) @binding(0)
var<uniform> camera: Camera;

struct VertexInput {
    @location(0) position: vec2<f32>,
    @location(1) uv: vec2<f32>,
}

struct InstanceInput {
    @location(2) transform_0: vec4<f32>,
    @location(3) transform_1: vec4<f32>,
    @location(4) transform_2: vec4<f32>,
    @location(5) transform_3: vec4<f32>,
    @location(6) color: vec4<f32>,
    @location(7) size: vec2<f32>,
    @location(8) pivot: vec2<f32>,
    @location(9) corner_radius_0: vec4<f32>,
    @location(10) corner_radius_1: vec4<f32>,
    @location(11) corner_radius_2: vec4<f32>,
    @location(12) corner_radius_3: vec4<f32>,
    @location(13) border_thickness: f32,
    @location(14) is_border: u32,
    @location(15) z_index: i32,
}

struct VertexOutput {
    @builtin(position) pos: vec4<f32>,
    @location(0) local_pos: vec2<f32>,
    @location(1) color: vec4<f32>,
    @location(2) size: vec2<f32>,
    @location(3) corner_radius_0: vec4<f32>,
    @location(4) corner_radius_1: vec4<f32>,
    @location(5) corner_radius_2: vec4<f32>,
    @location(6) corner_radius_3: vec4<f32>,
    @location(7) border_thickness: f32,
    @location(8) is_border: u32,
}

@vertex
fn vs_main(vertex: VertexInput, instance: InstanceInput) -> VertexOutput {
    let transform = mat4x4<f32>(
        instance.transform_0,
        instance.transform_1,
        instance.transform_2,
        instance.transform_3,
    );

    // Apply pivot + transform (matching your old shader logic)
    let pivot_offset = (instance.pivot - vec2<f32>(0.5, 0.5)) * instance.size;
    let scaled = (vertex.position * instance.size) - pivot_offset;
    let world_pos = transform * vec4<f32>(scaled, 0.0, 1.0);

    // Aspect ratio correction (matching your old shader)
    let virtual_aspect = camera.virtual_size.x / camera.virtual_size.y;
    let window_aspect = camera.window_size.x / camera.window_size.y;

    var scale: vec2<f32>;
    if (window_aspect > virtual_aspect) {
        // Window is wider → fit height, pillarbox
        scale = vec2<f32>(virtual_aspect / window_aspect, 1.0);
    } else {
        // Window is taller → fit width, letterbox
        scale = vec2<f32>(1.0, window_aspect / virtual_aspect);
    }

    // Convert to NDC with aspect correction (matching your old shader)
    let ndc_x = ((world_pos.x / camera.virtual_size.x) * 2.0) * scale.x;
    let ndc_y = ((world_pos.y / camera.virtual_size.y) * 2.0) * scale.y;
    
    // Convert z_index to depth value (normalize for typical UI usage)
    let depth = f32(instance.z_index) * 0.001;

    var out: VertexOutput;
    out.pos = vec4<f32>(ndc_x, ndc_y, depth, world_pos.w);
    out.local_pos = vertex.position;
    out.color = instance.color;
    out.size = instance.size;
    out.corner_radius_0 = instance.corner_radius_0;
    out.corner_radius_1 = instance.corner_radius_1;
    out.corner_radius_2 = instance.corner_radius_2;
    out.corner_radius_3 = instance.corner_radius_3;
    out.border_thickness = instance.border_thickness;
    out.is_border = instance.is_border;
    return out;
}

@fragment
fn fs_main(in: VertexOutput) -> @location(0) vec4<f32> {
    let p = in.local_pos;

    // Pick correct corner radius (matching your old shader logic)
    var radius: vec2<f32>;
    if (p.x < 0.0 && p.y > 0.0) {
        radius = in.corner_radius_0.xy;
    } else if (p.x > 0.0 && p.y > 0.0) {
        radius = in.corner_radius_1.xy;
    } else if (p.x > 0.0 && p.y < 0.0) {
        radius = in.corner_radius_2.xy;
    } else {
        radius = in.corner_radius_3.xy;
    }

    let half_size = vec2<f32>(0.5, 0.5);

    // Outer rounded rect distance
    let q_outer = abs(p) - (half_size - radius);
    let dist_outer = length(max(q_outer, vec2<f32>(0.0))) - min(radius.x, radius.y);

    if (dist_outer > 0.0) {
        discard;
    }

    if (in.is_border == 1u) {
        let inner_half_size = half_size - vec2<f32>(
            in.border_thickness / in.size.x,
            in.border_thickness / in.size.y
        );
        let inner_radius = max(
            radius - vec2<f32>(
                in.border_thickness / in.size.x,
                in.border_thickness / in.size.y
            ),
            vec2<f32>(0.0)
        );
        let q_inner = abs(p) - (inner_half_size - inner_radius);
        let dist_inner = length(max(q_inner, vec2<f32>(0.0))) - min(inner_radius.x, inner_radius.y);

        if (dist_inner < 0.0) {
            discard;
        }
    }

    return in.color;
}