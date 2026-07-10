use super::*;

#[test]
fn compile_and_eval_works() {
    let p = compile_expression("sin(t*pi*2.0) * params[0]").expect("compile");
    let mut stack = Vec::new();
    let v = p
        .eval(0.25, 1.0, &[2.0], &mut stack)
        .expect("eval should succeed");
    assert!(v.is_finite());
    assert!((v - 2.0).abs() < 1.0e-3);
}

#[test]
fn wgsl_emit_works() {
    let p = compile_expression("clamp(t,0.0,1.0)").expect("compile");
    let e = p.emit_wgsl_expr().expect("emit");
    assert!(e.contains("min(max("));
}

#[test]
fn clamp_orders_reversed_bounds_for_cpu_and_wgsl() {
    let p = compile_expression("clamp(t,1.0,0.0)").expect("compile");
    let mut stack = Vec::new();

    assert_eq!(p.eval(0.25, 1.0, &[], &mut stack), Some(0.25));
    assert_eq!(p.eval(-1.0, 1.0, &[], &mut stack), Some(0.0));
    assert_eq!(p.eval(2.0, 1.0, &[], &mut stack), Some(1.0));

    let wgsl = p.emit_wgsl_expr().expect("emit");
    assert!(wgsl.contains("min(1.0, 0.0)"));
    assert!(wgsl.contains("max(1.0, 0.0)"));
}

#[test]
fn hash_function_works() {
    let p = compile_expression("hash(id + params[0])").expect("compile");
    let mut stack = Vec::new();
    let input = ParticleEvalInput {
        t: 0.5,
        life: 0.5,
        lifetime: 1.0,
        spawn_time: 0.0,
        emitter_time: 0.0,
        speed: 1.0,
        particle_id: 42.0,
        dir: [0.0, 1.0, 0.0],
        vel: [0.0, 1.0, 0.0],
        rand: [0.1, 0.2, 0.3],
        seed: 42.0,
        ring_u: 0.0,
        index01: 0.0,
        emitter_pos: [0.0, 0.0, 0.0],
        prev_pos: [0.0, 0.0, 0.0],
        params: &[2.0],
    };
    let v = p
        .eval_particle(&input, &mut stack)
        .expect("eval should succeed");
    assert!(v.is_finite());
    assert!((0.0..1.0).contains(&v));
}

#[test]
fn prev_position_inputs_work() {
    let p = compile_expression("prev_x + prev_y * 2.0 + prev_z * 3.0").expect("compile");
    let mut stack = Vec::new();
    let input = ParticleEvalInput {
        t: 0.5,
        life: 0.5,
        lifetime: 1.0,
        spawn_time: 0.0,
        emitter_time: 0.0,
        speed: 1.0,
        particle_id: 1.0,
        dir: [0.0, 1.0, 0.0],
        vel: [0.0, 1.0, 0.0],
        rand: [0.1, 0.2, 0.3],
        seed: 1.0,
        ring_u: 0.0,
        index01: 0.0,
        emitter_pos: [0.0, 0.0, 0.0],
        prev_pos: [1.0, 2.0, 3.0],
        params: &[],
    };
    let v = p
        .eval_particle(&input, &mut stack)
        .expect("eval should succeed");
    assert!((v - 14.0).abs() < 1.0e-6);
}

#[test]
fn tau_constant_works() {
    let p = compile_expression("tau").expect("compile");
    let mut stack = Vec::new();
    let v = p
        .eval(0.0, 0.0, &[], &mut stack)
        .expect("eval should succeed");
    assert!((v - std::f32::consts::TAU).abs() < 1.0e-6);
}
