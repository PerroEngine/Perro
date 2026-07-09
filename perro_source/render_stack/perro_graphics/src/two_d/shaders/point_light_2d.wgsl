struct Camera2D {
    view: mat4x4<f32>,
    ndc_scale: vec2<f32>,
    pad: vec2<f32>,
}

@group(0) @binding(0)
var<uniform> camera: Camera2D;

struct ShadowCaster2D {
    center: vec2<f32>,
    axis_x: vec2<f32>,
    axis_y: vec2<f32>,
    half_extents: vec2<f32>,
    shape: u32,
    z_index: i32,
    pad: vec2<u32>,
}

@group(1) @binding(0)
var<storage, read> shadow_casters: array<ShadowCaster2D>;

struct VertexInput {
    @location(0) local_pos: vec2<f32>,
};

struct InstanceInput {
    @location(1) position: vec2<f32>,
    @location(2) range: f32,
    @location(3) @interpolate(flat) z_index: i32,
    @location(4) color: vec3<f32>,
    @location(5) intensity: f32,
    @location(6) direction: vec2<f32>,
    @location(7) inner_cos: f32,
    @location(8) outer_cos: f32,
    @location(9) @interpolate(flat) kind: u32,
    @location(10) @interpolate(flat) shadow_flags: u32,
};

struct VertexOutput {
    @builtin(position) clip_pos: vec4<f32>,
    @location(0) local_pos: vec2<f32>,
    @location(1) color: vec3<f32>,
    @location(2) intensity: f32,
    @location(3) direction: vec2<f32>,
    @location(4) inner_cos: f32,
    @location(5) outer_cos: f32,
    @location(6) @interpolate(flat) kind: u32,
    @location(7) @interpolate(flat) shadow_flags: u32,
    @location(8) world_pos: vec2<f32>,
    @location(9) light_pos: vec2<f32>,
};

@vertex
fn vs_main(v: VertexInput, inst: InstanceInput) -> VertexOutput {
    let depth = 1.0 - f32(inst.z_index) * 0.001;

    var out: VertexOutput;
    if inst.kind < 2u {
        out.clip_pos = vec4<f32>(v.local_pos * 2.0, depth, 1.0);
        out.world_pos = v.local_pos * vec2<f32>(1920.0, 1080.0);
    } else {
        let world_xy = inst.position + (v.local_pos * inst.range * 2.0);
        let world = vec4<f32>(world_xy, 0.0, 1.0);
        let view = camera.view * world;
        let ndc_xy = view.xy * camera.ndc_scale;
        out.clip_pos = vec4<f32>(ndc_xy, depth, 1.0);
        out.world_pos = world_xy;
    }
    out.light_pos = inst.position;
    out.local_pos = v.local_pos;
    out.color = inst.color;
    out.intensity = inst.intensity;
    out.direction = inst.direction;
    out.inner_cos = inst.inner_cos;
    out.outer_cos = inst.outer_cos;
    out.kind = inst.kind;
    out.shadow_flags = inst.shadow_flags;
    return out;
}

fn segment_axis_range(da: f32, db: f32, extent: f32, t_min_in: f32, t_max_in: f32) -> vec2<f32> {
    let delta = db - da;
    var t_min = t_min_in;
    var t_max = t_max_in;
    if abs(delta) < 0.00001 {
        if da < -extent || da > extent {
            return vec2<f32>(1.0, 0.0);
        }
        return vec2<f32>(t_min, t_max);
    }
    let inv_delta = 1.0 / delta;
    let t0 = (-extent - da) * inv_delta;
    let t1 = (extent - da) * inv_delta;
    t_min = max(t_min, min(t0, t1));
    t_max = min(t_max, max(t0, t1));
    return vec2<f32>(t_min, t_max);
}

fn segment_hits_box(a: vec2<f32>, b: vec2<f32>, caster: ShadowCaster2D) -> bool {
    let ra = a - caster.center;
    let rb = b - caster.center;
    let la = vec2<f32>(dot(ra, caster.axis_x), dot(ra, caster.axis_y));
    let lb = vec2<f32>(dot(rb, caster.axis_x), dot(rb, caster.axis_y));
    var range = segment_axis_range(la.x, lb.x, caster.half_extents.x, 0.0, 1.0);
    if range.x > range.y {
        return false;
    }
    range = segment_axis_range(la.y, lb.y, caster.half_extents.y, range.x, range.y);
    return range.x <= range.y;
}

fn segment_hits_circle(a: vec2<f32>, b: vec2<f32>, caster: ShadowCaster2D) -> bool {
    let d = b - a;
    let f = a - caster.center;
    let radius = max(caster.half_extents.x, caster.half_extents.y);
    let qa = dot(d, d);
    if qa < 0.00001 {
        return length(f) <= radius;
    }
    let qb = 2.0 * dot(f, d);
    let qc = dot(f, f) - radius * radius;
    let disc = qb * qb - 4.0 * qa * qc;
    if disc < 0.0 {
        return false;
    }
    let root = sqrt(disc);
    let inv = 0.5 / qa;
    let t0 = (-qb - root) * inv;
    let t1 = (-qb + root) * inv;
    return (t0 >= 0.0 && t0 <= 1.0) || (t1 >= 0.0 && t1 <= 1.0);
}

fn shadowed(in: VertexOutput) -> bool {
    if in.shadow_flags == 0u {
        return false;
    }
    var a = in.light_pos;
    let b = in.world_pos;
    if in.kind == 1u {
        a = b - normalize(in.direction) * 12000.0;
    }
    let count = min(arrayLength(&shadow_casters), 128u);
    for (var i = 0u; i < count; i = i + 1u) {
        let caster = shadow_casters[i];
        if caster.half_extents.x <= 0.0 || caster.half_extents.y <= 0.0 {
            continue;
        }
        if caster.shape == 1u {
            if segment_hits_circle(a, b, caster) {
                return true;
            }
        } else if segment_hits_box(a, b, caster) {
            return true;
        }
    }
    return false;
}

@fragment
fn fs_main(in: VertexOutput) -> @location(0) vec4<f32> {
    if in.kind < 2u {
        if shadowed(in) {
            return vec4<f32>(0.0, 0.0, 0.0, 1.0);
        }
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
        if shadowed(in) {
            discard;
        }
        let cone = smoothstep(in.outer_cos, in.inner_cos, c);
        return vec4<f32>(in.color * in.intensity * falloff * cone, falloff * cone);
    }
    if shadowed(in) {
        discard;
    }
    return vec4<f32>(in.color * in.intensity * falloff, falloff);
}
