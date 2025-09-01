// Fixed WGSL shader using proper SDF technique for rounded rectangles

struct Camera {
    virtual_size: vec2<f32>,
    ndc_scale: vec2<f32>,
}

@group(0) @binding(0) var<uniform> camera: Camera;

struct VertexInput {
    @location(0) position: vec2<f32>, // quad local position: e.g. (-0.5..0.5)
    @location(1) uv: vec2<f32>,       // if you need uv (unused here)
}

// Instance inputs (match your Rust RectInstance fields)
struct InstanceInput {
    @location(2) transform_0: vec4<f32>,
    @location(3) transform_1: vec4<f32>,
    @location(4) transform_2: vec4<f32>,
    @location(5) transform_3: vec4<f32>,
    @location(6) color: vec4<f32>,
    @location(7) size: vec2<f32>,
    @location(8) pivot: vec2<f32>,
    @location(9) corner_radius_xy: vec4<f32>, // [tl.x, tl.y, tr.x, tr.y]
    @location(10) corner_radius_zw: vec4<f32>,// [br.x, br.y, bl.x, bl.y]
    @location(11) border_thickness: f32,
    @location(12) is_border: u32,
    @location(13) z_index: i32,
}

struct VertexOutput {
    @builtin(position) pos: vec4<f32>,
    @location(0) local_pos: vec2<f32>,       // in pixels, relative to rect center
    @location(1) color: vec4<f32>,
    @location(2) size: vec2<f32>,
    @location(3) corner_radius_xy: vec4<f32>,
    @location(4) corner_radius_zw: vec4<f32>,
    @location(5) border_thickness: f32,
    @location(6) is_border: u32,
}

@vertex
fn vs_main(vertex: VertexInput, instance: InstanceInput) -> VertexOutput {
    // Build transform mat4
    let transform = mat4x4<f32>(
        instance.transform_0,
        instance.transform_1,
        instance.transform_2,
        instance.transform_3
    );

    // pivot offset in pixels
    let pivot_offset = (instance.pivot - vec2<f32>(0.5, 0.5)) * instance.size;

    // local position in pixels relative to center
    let local_scaled = (vertex.position * instance.size) - pivot_offset;

    // world_pos from full transform
    let world_pos4 = transform * vec4<f32>(local_scaled, 0.0, 1.0);

    // convert to NDC using precomputed ndc_scale
    let ndc_pos = world_pos4.xy * camera.ndc_scale;

    // depth from z_index
    let depth = f32(instance.z_index) * 0.001;

    var out: VertexOutput;
    out.pos = vec4<f32>(ndc_pos, depth, 1.0);
    out.local_pos = local_scaled; // in pixels relative to rect center
    out.color = instance.color;
    out.size = instance.size;
    out.corner_radius_xy = instance.corner_radius_xy;
    out.corner_radius_zw = instance.corner_radius_zw;
    out.border_thickness = instance.border_thickness;
    out.is_border = instance.is_border;
    return out;
}

// Helper function to get corner radius for current fragment
fn get_corner_radius(p: vec2<f32>, corner_radii: vec4<f32>, corner_radii2: vec4<f32>) -> f32 {
    // Determine which quadrant we're in and return appropriate radius
    // corner_radii = [tl.x, tl.y, tr.x, tr.y] (but we assume circular, so use .x values)
    // corner_radii2 = [br.x, br.y, bl.x, bl.y]
    
    if (p.x <= 0.0 && p.y >= 0.0) {
        return corner_radii.x; // Top-left
    } else if (p.x > 0.0 && p.y >= 0.0) {
        return corner_radii.z; // Top-right  
    } else if (p.x > 0.0 && p.y < 0.0) {
        return corner_radii2.x; // Bottom-right
    } else {
        return corner_radii2.z; // Bottom-left
    }
}

// SDF for rounded rectangle - standard implementation
fn sdf_rounded_box(p: vec2<f32>, size: vec2<f32>, corner_radii: vec4<f32>, corner_radii2: vec4<f32>) -> f32 {
    let half_size = size * 0.5;
    
    // Get radius for current corner
    let radius = get_corner_radius(p, corner_radii, corner_radii2);
    
    // Standard rounded box SDF calculation
    let q = abs(p) - half_size + radius;
    return min(max(q.x, q.y), 0.0) + length(max(q, vec2<f32>(0.0))) - radius;
}

@fragment
fn fs_main(in: VertexOutput) -> @location(0) vec4<f32> {
    let p = in.local_pos;
    
    // Calculate outer distance
    let outer_dist = sdf_rounded_box(p, in.size, in.corner_radius_xy, in.corner_radius_zw);
    
    // Anti-aliasing: smooth transition at edges (typically 1-2 pixels)
    let edge_softness = 0.8;
    let outer_alpha = 1.0 - smoothstep(-edge_softness, edge_softness, outer_dist);
    
    // Early exit if completely outside
    if (outer_alpha <= 0.0) {
        discard;
    }
    
    var final_alpha = outer_alpha;
    
    // Handle border
    if (in.is_border == 1u) {
        let border_thickness = in.border_thickness;
        let inner_size = in.size - vec2<f32>(border_thickness * 2.0);
        
        // Make sure we don't have negative inner size
        if (inner_size.x <= 0.0 || inner_size.y <= 0.0) {
            return vec4<f32>(in.color.rgb, in.color.a * outer_alpha);
        }
        
        // Calculate inner corner radii (reduce by border thickness)
        let inner_corner_xy = max(in.corner_radius_xy - vec4<f32>(border_thickness), vec4<f32>(0.0));
        let inner_corner_zw = max(in.corner_radius_zw - vec4<f32>(border_thickness), vec4<f32>(0.0));
        
        // Calculate inner distance
        let inner_dist = sdf_rounded_box(p, inner_size, inner_corner_xy, inner_corner_zw);
        
        // Anti-aliased inner edge - creates smooth border
        let inner_alpha = 1.0 - smoothstep(-edge_softness, edge_softness, inner_dist);
        
        // For border: we want to render where outer_alpha > 0 AND inner_alpha < 1
        // This creates the "donut" effect
        final_alpha = outer_alpha * (1.0 - inner_alpha);
    }
    
    // Early exit if alpha too low
    if (final_alpha <= 0.0) {
        discard;
    }
    
    return vec4<f32>(in.color.rgb, in.color.a * final_alpha);
}