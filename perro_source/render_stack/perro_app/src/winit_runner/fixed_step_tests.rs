use super::{MAX_FIXED_STEPS_PER_FRAME, StartupSplashState, plan_fixed_steps};
use std::time::Instant;
#[cfg(not(target_arch = "wasm32"))]
use winit::dpi::PhysicalSize;

#[test]
fn fixed_step_plan_caps_large_delta() {
    let plan = plan_fixed_steps(1.0, 1.0 / 60.0, 0.0);
    assert_eq!(plan.steps, MAX_FIXED_STEPS_PER_FRAME);
    assert!(plan.dropped_catchup);
    assert!(plan.accumulator_after < 1.0 / 60.0);
}

#[test]
fn fixed_step_plan_keeps_substep_remainder() {
    let step = 1.0 / 60.0;
    let start = step * 0.5;
    let plan = plan_fixed_steps(step * 2.25, step, start);
    assert_eq!(plan.steps, 2);
    assert!(!plan.dropped_catchup);
    assert!((plan.accumulator_after - (step * 0.75)).abs() < 1e-6);
}

#[test]
fn fixed_step_plan_drops_full_catchup_but_keeps_fractional_progress() {
    let step = 1.0 / 60.0;
    let start = step * 0.25;
    let plan = plan_fixed_steps(step * 20.0, step, start);
    assert_eq!(plan.steps, MAX_FIXED_STEPS_PER_FRAME);
    assert!(plan.dropped_catchup);
    assert!(plan.accumulator_after < step);
}

#[test]
fn startup_splash_blocks_input_only_until_first_frame_capture() {
    let mut splash = StartupSplashState {
        active: true,
        source: None,
        source_hash: None,
        image_size: None,
        texture_size: None,
        texture_requested: false,
        texture_id: None,
        ready_streak: 0,
        shown_at: Instant::now(),
        fade_started_at: None,
        first_frame_inflight: Vec::new(),
        first_frame_captured: false,
    };

    assert!(splash.blocks_input());

    splash.first_frame_captured = true;

    assert!(!splash.blocks_input());
}

#[cfg(not(target_arch = "wasm32"))]
#[test]
fn fit_aspect_uses_monitor_fraction_box_without_distorting() {
    let fitted = super::fit_aspect(PhysicalSize::new(1920, 1080), 1920, 1080);
    assert_eq!(fitted, PhysicalSize::new(1920, 1080));

    let fitted = super::fit_aspect(PhysicalSize::new(1920, 1080), 2880, 1620);
    assert_eq!(fitted, PhysicalSize::new(2880, 1620));

    let fitted = super::fit_aspect(PhysicalSize::new(1920, 1080), 1440, 810);
    assert_eq!(fitted, PhysicalSize::new(1440, 810));

    let fitted = super::fit_aspect(PhysicalSize::new(1080, 1920), 1440, 810);
    assert_eq!(fitted, PhysicalSize::new(455, 810));
}
