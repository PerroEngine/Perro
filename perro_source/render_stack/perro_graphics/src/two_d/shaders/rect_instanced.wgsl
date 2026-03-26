struct Camera2D {
    view: mat4x4<f32>,
    ndc_scale: vec2<f32>,
    pad: vec2<f32>,
}

@group(0) @binding(0)
var<uniform> camera: Camera2D;

struct VertexInput {
    @location(0) local_pos: vec2<f32>,
};

struct InstanceInput {
    @location(1) center: vec2<f32>,
    @location(2) size: vec2<f32>,
    @location(3) color: vec4<f32>,
    @location(4) z_index: i32,
    @location(5) shape_kind: u32,
    @location(6) thickness: f32,
    @location(7) filled: u32,
};

struct VertexOutput {
    @builtin(position) clip_pos: vec4<f32>,
    @location(0) color: vec4<f32>,
    @location(1) local_pos: vec2<f32>,
    @location(2) size: vec2<f32>,
    @location(3) shape_kind: u32,
    @location(4) thickness: f32,
    @location(5) filled: u32,
};

@vertex
fn vs_main(v: VertexInput, inst: InstanceInput) -> VertexOutput {
    let world_xy = inst.center + (v.local_pos * inst.size);
    let world = vec4<f32>(world_xy, 0.0, 1.0);
    let view = camera.view * world;
    let ndc_xy = view.xy * camera.ndc_scale;
    let depth = 1.0 - f32(inst.z_index) * 0.001;

    var out: VertexOutput;
    out.clip_pos = vec4<f32>(ndc_xy, depth, 1.0);
    out.color = inst.color;
    out.local_pos = v.local_pos;
    out.size = inst.size;
    out.shape_kind = inst.shape_kind;
    out.thickness = inst.thickness;
    out.filled = inst.filled;
    return out;
}

@fragment
fn fs_main(in: VertexOutput) -> @location(0) vec4<f32> {
    if in.shape_kind == 1u {
        let radius = 0.5 * min(in.size.x, in.size.y);
        let local = in.local_pos * in.size;
        let d = length(local);
        if in.filled == 1u {
            if d > radius {
                discard;
            }
        } else {
            let t = max(in.thickness, 0.0);
            if d > radius || d < max(radius - t, 0.0) {
                discard;
            }
        }
    } else if in.shape_kind == 2u {
        if in.filled == 0u {
            let half_size = 0.5 * in.size;
            let p = abs(in.local_pos * in.size);
            let t = max(in.thickness, 0.0);
            if p.x < max(half_size.x - t, 0.0) && p.y < max(half_size.y - t, 0.0) {
                discard;
            }
        }
    }
    return in.color;
}
