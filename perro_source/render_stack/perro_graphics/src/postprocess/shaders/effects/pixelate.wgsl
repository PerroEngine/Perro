fn pixelate_load(pos: vec2<f32>, dims: vec2<u32>) -> vec4<f32> {
    let clamped = clamp(vec2<i32>(floor(pos)), vec2<i32>(0, 0), vec2<i32>(dims) - vec2<i32>(1, 1));
    return textureLoad(input_tex, clamped, 0);
}

fn pixelate_sample(uv: vec2<f32>, size: f32) -> vec4<f32> {
    let px = max(round(size), 1.0);
    let dims = textureDimensions(input_tex);
    let dims_f = vec2<f32>(dims);
    let center = dims_f * 0.5;
    let grid = (uv * dims_f - center) / px;
    let cell = floor(grid + vec2<f32>(0.5));
    let local = grid - cell;
    let step_dir = sign(local);
    let edge = smoothstep(vec2<f32>(0.38), vec2<f32>(0.5), abs(local));

    let base = cell * px + center;
    let x_pos = (cell + vec2<f32>(step_dir.x, 0.0)) * px + center;
    let y_pos = (cell + vec2<f32>(0.0, step_dir.y)) * px + center;
    let xy_pos = (cell + step_dir) * px + center;

    let c0 = pixelate_load(base, dims);
    let cx = pixelate_load(x_pos, dims);
    let cy = pixelate_load(y_pos, dims);
    let cxy = pixelate_load(xy_pos, dims);
    return mix(mix(c0, cx, edge.x), mix(cy, cxy, edge.x), edge.y);
}
