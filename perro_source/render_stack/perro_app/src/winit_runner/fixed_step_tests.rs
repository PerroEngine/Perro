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

/// The opening window is a standard rung of the canvas, not "whatever fraction
/// of this monitor happens to fit". A 2560x1440 display (0.75 -> 1920x1080)
/// and a 3840x2160 one (0.75 -> 2880x1620) both open at 1920x1080; the old
/// aspect-fit gave the 4K display 2880x1620, a size nothing is authored for.
#[cfg(not(target_arch = "wasm32"))]
#[test]
fn standard_window_never_exceeds_the_canvas() {
    let canvas = PhysicalSize::new(1920, 1080);
    for (max_w, max_h) in [(1920, 1080), (2880, 1620), (5760, 3240)] {
        assert_eq!(
            super::standard_window_size(canvas, max_w, max_h),
            PhysicalSize::new(1920, 1080),
            "budget {max_w}x{max_h}"
        );
    }
}

/// Too small for 1080p steps down the ladder to the next familiar size rather
/// than inventing one.
#[cfg(not(target_arch = "wasm32"))]
#[test]
fn standard_window_steps_down_the_ladder() {
    let canvas = PhysicalSize::new(1920, 1080);
    assert_eq!(
        super::standard_window_size(canvas, 1900, 1000),
        PhysicalSize::new(1600, 900)
    );
    assert_eq!(
        super::standard_window_size(canvas, 1500, 800),
        PhysicalSize::new(1280, 720)
    );
    assert_eq!(
        super::standard_window_size(canvas, 1000, 600),
        PhysicalSize::new(960, 540)
    );
}

/// Portrait canvases walk the same rungs on their own long axis.
#[cfg(not(target_arch = "wasm32"))]
#[test]
fn standard_window_handles_portrait_canvas() {
    let canvas = PhysicalSize::new(1080, 1920);
    assert_eq!(
        super::standard_window_size(canvas, 1440, 2160),
        PhysicalSize::new(1080, 1920)
    );
    assert_eq!(
        super::standard_window_size(canvas, 1000, 1700),
        PhysicalSize::new(900, 1600)
    );
}

/// A display too small for even the last rung still gets a correctly shaped
/// window instead of one that overflows the screen.
#[cfg(not(target_arch = "wasm32"))]
#[test]
fn standard_window_falls_back_to_aspect_fit_when_nothing_fits() {
    let fitted = super::standard_window_size(PhysicalSize::new(1920, 1080), 500, 200);
    assert!(fitted.width <= 500 && fitted.height <= 200);
    let ratio = fitted.width as f32 / fitted.height as f32;
    assert!((ratio - 16.0 / 9.0).abs() < 0.05, "{fitted:?}");
}
