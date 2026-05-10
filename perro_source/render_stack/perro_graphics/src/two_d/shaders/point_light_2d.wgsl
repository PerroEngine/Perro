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
    @location(1) position: vec2<f32>,
    @location(2) range: f32,
    @location(3) z_index: i32,
    @location(4) color: vec3<f32>,
    @location(5) intensity: f32,
    @location(6) direction: vec2<f32>,
    @location(7) inner_cos: f32,
    @location(8) outer_cos: f32,
    @location(9) kind: u32,
};

struct VertexOutput {
    @builtin(position) clip_pos: vec4<f32>,
    @location(0) local_pos: vec2<f32>,
    @location(1) color: vec3<f32>,
    @location(2) intensity: f32,
    @location(3) direction: vec2<f32>,
    @location(4) inner_cos: f32,
    @location(5) outer_cos: f32,
    @location(6) kind: u32,
};

@vertex
fn vs_main(v: VertexInput, inst: InstanceInput) -> VertexOutput {
    let depth = 1.0 - f32(inst.z_index) * 0.001;

    var out: VertexOutput;
    if inst.kind < 2u {
        out.clip_pos = vec4<f32>(v.local_pos * 2.0, depth, 1.0);
    } else {
        let world_xy = inst.position + (v.local_pos * inst.range * 2.0);
        let world = vec4<f32>(world_xy, 0.0, 1.0);
        let view = camera.view * world;
        let ndc_xy = view.xy * camera.ndc_scale;
        out.clip_pos = vec4<f32>(ndc_xy, depth, 1.0);
    }
    out.local_pos = v.local_pos;
    out.color = inst.color;
    out.intensity = inst.intensity;
    out.direction = inst.direction;
    out.inner_cos = inst.inner_cos;
    out.outer_cos = inst.outer_cos;
    out.kind = inst.kind;
    return out;
}

@fragment
fn fs_main(in: VertexOutput) -> @location(0) vec4<f32> {
    if in.kind < 2u {
        return vec4<f32>(in.color * in.intensity, 1.0);
    }

    let d = length(in.local_pos) * 2.0;
    if d > 1.0 {
        discard;
    }
    let falloff = (1.0 - d) * (1.0 - d);
    if in.kind == 3u {
        let to_px = normalize(in.local_pos);
        let c = dot(to_px, normalize(in.direction));
        if c < in.outer_cos {
            discard;
        }
        let cone = smoothstep(in.outer_cos, in.inner_cos, c);
        return vec4<f32>(in.color * in.intensity * falloff * cone, falloff * cone);
    }
    return vec4<f32>(in.color * in.intensity * falloff, falloff);
}
