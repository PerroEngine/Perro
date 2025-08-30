struct Camera {
    virtual_size: vec2<f32>,
    ndc_scale: vec2<f32>,  // Pre-computed scaling factors
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
    @location(9) corner_radius_xy: vec4<f32>,  // [top_left.xy, top_right.xy]
    @location(10) corner_radius_zw: vec4<f32>, // [bottom_right.xy, bottom_left.xy]
    @location(11) border_thickness: f32,
    @location(12) is_border: u32,
    @location(13) z_index: i32,
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

@vertex
fn vs_main(vertex: VertexInput, instance: InstanceInput) -> VertexOutput {
    let transform = mat4x4<f32>(
        instance.transform_0,
        instance.transform_1,
        instance.transform_2,
        instance.transform_3,
    );

    // Apply pivot + transform
    let pivot_offset = (instance.pivot - vec2<f32>(0.5, 0.5)) * instance.size;
    let scaled = (vertex.position * instance.size) - pivot_offset;
    let world_pos = transform * vec4<f32>(scaled, 0.0, 1.0);

    // Use pre-computed NDC scaling (no runtime aspect calculation!)
    let ndc_pos = world_pos.xy * camera.ndc_scale;
    
    // Convert z_index to depth value
    let depth = f32(instance.z_index) * 0.001;

    var out: VertexOutput;
    out.pos = vec4<f32>(ndc_pos, depth, world_pos.w);
    out.local_pos = vertex.position;
    out.color = instance.color;
    out.size = instance.size;
    out.corner_radius_xy = instance.corner_radius_xy;
    out.corner_radius_zw = instance.corner_radius_zw;
    out.border_thickness = instance.border_thickness;
    out.is_border = instance.is_border;
    return out;
}

@fragment
fn fs_main(in: VertexOutput) -> @location(0) vec4<f32> {
    let p = in.local_pos;

    // Branchless corner radius selection using step functions
    let corner_mask = vec4<f32>(
        step(0.0, -p.x) * step(0.0, p.y),   // top-left
        step(0.0, p.x) * step(0.0, p.y),    // top-right
        step(0.0, p.x) * step(0.0, -p.y),   // bottom-right
        step(0.0, -p.x) * step(0.0, -p.y)   // bottom-left
    );
    
    // Unpack corner radius from the two packed vec4s
    let radius = 
        corner_mask.x * in.corner_radius_xy.xy +      // top-left
        corner_mask.y * in.corner_radius_xy.zw +      // top-right
        corner_mask.z * in.corner_radius_zw.xy +      // bottom-right
        corner_mask.w * in.corner_radius_zw.zw;       // bottom-left

    let half_size = vec2<f32>(0.5, 0.5);

    // Outer rounded rect distance
    let q_outer = abs(p) - (half_size - radius);
    let dist_outer = length(max(q_outer, vec2<f32>(0.0))) - min(radius.x, radius.y);

    if (dist_outer > 0.0) {
        discard;
    }

    if (in.is_border == 1u) {
        let border_offset = vec2<f32>(
            in.border_thickness / in.size.x,
            in.border_thickness / in.size.y
        );
        let inner_half_size = half_size - border_offset;
        let inner_radius = max(radius - border_offset, vec2<f32>(0.0));
        
        let q_inner = abs(p) - (inner_half_size - inner_radius);
        let dist_inner = length(max(q_inner, vec2<f32>(0.0))) - min(inner_radius.x, inner_radius.y);

        if (dist_inner < 0.0) {
            discard;
        }
    }

    return in.color;
}