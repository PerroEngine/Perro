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
    // Build the full transform matrix
    let transform = mat4x4<f32>(
        instance.transform_0,
        instance.transform_1,
        instance.transform_2,
        instance.transform_3
    );

    // ✅ FIXED: Use the same pivot system as your working old shader
    // Convert pivot to match your layout system's expectations
    let pivot_centered = instance.pivot - vec2<f32>(0.5, 0.5);
    
    // Calculate pivot offset in pixels
    let pivot_offset = pivot_centered * instance.size;
    
    // Calculate vertex position in local space (quad vertices are -0.5 to 0.5)
    let local_vertex_pos = vertex.position * instance.size;
    
    // Apply pivot offset (subtract to move the rectangle relative to its origin)
    let adjusted_local_pos = local_vertex_pos - pivot_offset;

    // Transform to world space using full matrix multiplication
    let world_pos4 = transform * vec4<f32>(adjusted_local_pos, 0.0, 1.0);

    // Convert to NDC using precomputed scaling
    let ndc_pos = world_pos4.xy * camera.ndc_scale;
    
    // Depth from z_index
    let depth = f32(instance.z_index) * 0.001;

    // For SDF calculations, we need center-relative position (no pivot offset)
    let fragment_local_pos = vertex.position * instance.size;

    var out: VertexOutput;
    out.pos = vec4<f32>(ndc_pos, depth, 1.0);
    out.local_pos = fragment_local_pos; // Center-relative for SDF calculations
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

// SDF for rounded rectangle
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
    
    // OPTIMIZED: Pre-compute constants
    let edge_softness = 0.8;
    let edge_min = -edge_softness;
    let edge_max = edge_softness;
    
    // Calculate outer distance
    let outer_dist = sdf_rounded_box(p, in.size, in.corner_radius_xy, in.corner_radius_zw);
    
    // OPTIMIZED: Early exit with rough bounds check before expensive smoothstep
    // If we're way outside the bounds, discard immediately
    let half_size = in.size * 0.5;
    let max_radius = max(max(in.corner_radius_xy.x, in.corner_radius_xy.z), 
                         max(in.corner_radius_zw.x, in.corner_radius_zw.z));
    let rough_bounds = max(half_size.x, half_size.y) + max_radius + edge_softness;
    if (abs(p.x) > rough_bounds || abs(p.y) > rough_bounds) {
        discard;
    }
    
    // Anti-aliasing: smooth transition at edges
    let outer_alpha = 1.0 - smoothstep(edge_min, edge_max, outer_dist);
    
    // Early exit if completely outside
    if (outer_alpha <= 0.0) {
        discard;
    }
    
    var final_alpha = outer_alpha;
    
    // Handle border
    if (in.is_border == 1u) {
        let border_thickness = in.border_thickness;
        let border_thickness_2 = border_thickness * 2.0;
        let inner_size = in.size - vec2<f32>(border_thickness_2);
        
        // Make sure we don't have negative inner size
        if (inner_size.x <= 0.0 || inner_size.y <= 0.0) {
            return vec4<f32>(in.color.rgb, in.color.a * outer_alpha);
        }
        
        // OPTIMIZED: Pre-compute border radius reduction
        let border_reduction = vec4<f32>(border_thickness);
        let inner_corner_xy = max(in.corner_radius_xy - border_reduction, vec4<f32>(0.0));
        let inner_corner_zw = max(in.corner_radius_zw - border_reduction, vec4<f32>(0.0));
        
        // Calculate inner distance
        let inner_dist = sdf_rounded_box(p, inner_size, inner_corner_xy, inner_corner_zw);
        
        // Anti-aliased inner edge
        let inner_alpha = 1.0 - smoothstep(edge_min, edge_max, inner_dist);
        
        // For border: render where outer_alpha > 0 AND inner_alpha < 1
        final_alpha = outer_alpha * (1.0 - inner_alpha);
    }
    
    // Early exit if alpha too low
    if (final_alpha <= 0.0) {
        discard;
    }
    
    return vec4<f32>(in.color.rgb, in.color.a * final_alpha);
}