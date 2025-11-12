struct Camera {
    virtual_size: vec2<f32>,
    ndc_scale: vec2<f32>,
}

@group(0) @binding(0) var<uniform> camera: Camera;

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
    @location(9) corner_radius_xy: vec4<f32>,
    @location(10) corner_radius_zw: vec4<f32>,
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
    // ✅ Optimized: Pre-compute commonly used values
    let half_size = instance.size * 0.5;
    
    // ✅ Optimized: Combine pivot calculations
    let pivot_offset = (vec2<f32>(0.5) - instance.pivot) * instance.size;
    let adjusted_local_pos = vertex.position * instance.size + pivot_offset;

    // ✅ Optimized: Direct dot products instead of matrix construction
    let world_x = dot(instance.transform_0, vec4<f32>(adjusted_local_pos, 0.0, 1.0));
    let world_y = dot(instance.transform_1, vec4<f32>(adjusted_local_pos, 0.0, 1.0));
    
    // ✅ Optimized: Combine NDC conversion and depth
    let ndc_pos = vec2<f32>(world_x, world_y) * camera.ndc_scale;
    let depth = f32(instance.z_index) * 0.001;

    var out: VertexOutput;
    out.pos = vec4<f32>(ndc_pos, depth, 1.0);
    out.local_pos = vertex.position * instance.size; // For SDF calculations
    out.color = instance.color;
    out.size = instance.size;
    out.corner_radius_xy = instance.corner_radius_xy;
    out.corner_radius_zw = instance.corner_radius_zw;
    out.border_thickness = instance.border_thickness;
    out.is_border = instance.is_border;
    return out;
}

// ✅ Optimized: Simplified corner radius lookup
fn get_corner_radius(p: vec2<f32>, corner_radii: vec4<f32>, corner_radii2: vec4<f32>) -> f32 {
    let quadrant = select(
        select(corner_radii2.z, corner_radii2.x, p.x > 0.0), // bottom row
        select(corner_radii.x, corner_radii.z, p.x > 0.0),   // top row
        p.y >= 0.0
    );
    return quadrant;
}

fn sdf_rounded_box(p: vec2<f32>, size: vec2<f32>, corner_radii: vec4<f32>, corner_radii2: vec4<f32>) -> f32 {
    let half_size = size * 0.5;
    let radius = get_corner_radius(p, corner_radii, corner_radii2);
    let q = abs(p) - half_size + radius;
    return min(max(q.x, q.y), 0.0) + length(max(q, vec2<f32>(0.0))) - radius;
}

@fragment
fn fs_main(in: VertexOutput) -> @location(0) vec4<f32> {
    let p = in.local_pos;
    let half_size = in.size * 0.5;
    
    // ✅ Optimized: Quick AABB reject before expensive SDF
    if (abs(p.x) > half_size.x + 2.0 || abs(p.y) > half_size.y + 2.0) {
        discard;
    }
    
    let outer_dist = sdf_rounded_box(p, in.size, in.corner_radius_xy, in.corner_radius_zw);
    let edge_softness = 0.8;
    let outer_alpha = 1.0 - smoothstep(-edge_softness, edge_softness, outer_dist);
    
    if (outer_alpha <= 0.0) {
        discard;
    }
    
    var final_alpha = outer_alpha;
    
    if (in.is_border == 1u) {
        let border_thickness = in.border_thickness;
        let inner_size = in.size - vec2<f32>(border_thickness * 2.0);
        
        if (inner_size.x <= 0.0 || inner_size.y <= 0.0) {
            return vec4<f32>(in.color.rgb, in.color.a * outer_alpha);
        }
        
        let inner_corner_xy = max(in.corner_radius_xy - vec4<f32>(border_thickness), vec4<f32>(0.0));
        let inner_corner_zw = max(in.corner_radius_zw - vec4<f32>(border_thickness), vec4<f32>(0.0));
        
        let inner_dist = sdf_rounded_box(p, inner_size, inner_corner_xy, inner_corner_zw);
        let inner_alpha = 1.0 - smoothstep(-edge_softness, edge_softness, inner_dist);
        
        final_alpha = outer_alpha * (1.0 - inner_alpha);
    }
    
    if (final_alpha <= 0.0) {
        discard;
    }
    
    return vec4<f32>(in.color.rgb, in.color.a * final_alpha);
}