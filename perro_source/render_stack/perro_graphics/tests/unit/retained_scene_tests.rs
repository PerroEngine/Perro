//! Retained-scene fast path gate (`scene_fast_path_allowed`).
//!
//! The gate decides whether a frame may leave the whole scene chain (depth
//! prepass, culling, mesh pass, seam pass, 3D particles, water, 2D) out of the
//! encoder and present the retained scene texture instead. A missed fast path
//! costs one redundant encode; a wrong skip freezes the image, so every test
//! here pins a "must render fully" direction.
use super::*;

/// Named change to a signal set: label + the flip it applies.
type SignalFlip = (&'static str, fn(&mut SceneFastPathSignals));

/// Two identical static frames after a full render: nothing moved, no water,
/// no particles, no streams, no TAA, no post ping-pong.
fn static_frame() -> SceneFastPathSignals {
    SceneFastPathSignals {
        retained_scene_valid: true,
        retained_key_matches: true,
        ..SceneFastPathSignals::default()
    }
}

#[test]
fn second_identical_frame_takes_fast_path() {
    assert!(scene_fast_path_allowed(&static_frame()));
}

#[test]
fn first_frame_renders_fully() {
    // Nothing has been submitted yet: the retained texture is undefined.
    let signals = SceneFastPathSignals {
        retained_scene_valid: false,
        ..static_frame()
    };
    assert!(!scene_fast_path_allowed(&signals));
}

#[test]
fn key_mismatch_renders_fully() {
    // Resize / sample-count change / new post view generation / new clear
    // color all land here.
    let signals = SceneFastPathSignals {
        retained_key_matches: false,
        ..static_frame()
    };
    assert!(!scene_fast_path_allowed(&signals));
}

#[test]
fn camera_move_renders_fully() {
    // A camera change bumps the tracked 3D content compare, which also makes
    // the 3D prepare run.
    let content_only = SceneFastPathSignals {
        three_d_content_changed: true,
        ..static_frame()
    };
    let with_prepare = SceneFastPathSignals {
        did_prepare_3d: true,
        ..content_only
    };
    assert!(!scene_fast_path_allowed(&content_only));
    assert!(!scene_fast_path_allowed(&with_prepare));
}

#[test]
fn every_single_change_signal_forces_a_full_render() {
    // One flip per signal, from the all-clear static frame. Each must be
    // sufficient on its own to reject the fast path.
    let flips: [SignalFlip; 13] = [
        ("retained_scene_valid", |s| s.retained_scene_valid = false),
        ("retained_key_matches", |s| s.retained_key_matches = false),
        ("did_prepare_3d", |s| s.did_prepare_3d = true),
        ("three_d_content_changed", |s| {
            s.three_d_content_changed = true
        }),
        ("three_d_dirty", |s| s.three_d_dirty = true),
        ("did_prepare_2d", |s| s.did_prepare_2d = true),
        ("two_d_scene_changed", |s| s.two_d_scene_changed = true),
        ("taa_active", |s| s.taa_active = true),
        ("needs_water", |s| s.needs_water = true),
        ("needs_particles", |s| s.needs_particles = true),
        ("scene_continuous_updates", |s| {
            s.scene_continuous_updates = true
        }),
        ("streams_rendered", |s| s.streams_rendered = true),
        ("decals_texture_pending", |s| {
            s.decals_texture_pending = true
        }),
    ];
    for (name, flip) in flips {
        let mut signals = static_frame();
        flip(&mut signals);
        assert!(
            !scene_fast_path_allowed(&signals),
            "{name} must force a full scene render"
        );
    }
}

#[test]
fn single_post_stage_keeps_fast_path_two_stages_do_not() {
    // One stage reads the scene texture and writes the intermediate, leaving
    // the retained pixels intact. Two stages ping-pong back into the scene
    // texture and overwrite them.
    for stages in 0..=1 {
        let signals = SceneFastPathSignals {
            post_stage_count: stages,
            ..static_frame()
        };
        assert!(
            scene_fast_path_allowed(&signals),
            "{stages} post stage(s) must keep the fast path"
        );
    }
    for stages in 2..=3 {
        let signals = SceneFastPathSignals {
            post_stage_count: stages,
            ..static_frame()
        };
        assert!(
            !scene_fast_path_allowed(&signals),
            "{stages} post stages write the retained scene texture"
        );
    }
}

/// The headline case: a UI-only frame (FPS counter, HUD text) over a static
/// 3D scene. UI commands raise DIRTY_2D, but the UI composites onto the
/// swapchain after the scene texture, so the scene chain can still be skipped.
/// The UI pass itself always runs.
#[test]
fn ui_only_change_over_static_scene_takes_fast_path() {
    // `two_d_scene_changed` is `needs_2d_prepare && has_2d_content`; with no
    // 2D content a UI command leaves it false.
    assert!(scene_fast_path_allowed(&static_frame()));
    // ...but a real 2D scene edit in the same frame does block it.
    let with_2d_content = SceneFastPathSignals {
        two_d_scene_changed: true,
        ..static_frame()
    };
    assert!(!scene_fast_path_allowed(&with_2d_content));
}

#[test]
fn camera_image_save_renders_fully() {
    let signals = SceneFastPathSignals {
        camera_image_saves_pending: true,
        ..static_frame()
    };
    assert!(!scene_fast_path_allowed(&signals));
}

#[test]
fn taa_never_takes_fast_path_even_when_everything_else_is_static() {
    // TAA keeps converging over jittered frames, so a "static" frame still
    // changes the presented image.
    let signals = SceneFastPathSignals {
        taa_active: true,
        ..static_frame()
    };
    assert!(!scene_fast_path_allowed(&signals));
}

#[test]
fn retained_key_tracks_size_samples_generation_and_clear_color() {
    let base = RetainedSceneKey {
        render_width: 1280,
        render_height: 720,
        sample_count: 1,
        post_view_generation: 3,
        clear_color: [0.1, 0.2, 0.3, 1.0],
        depth_prepass_needed: false,
        blend_screen_active: false,
        post_stage_count: 0,
    };
    assert_eq!(base, base);
    let mutations = [
        RetainedSceneKey {
            render_width: 1281,
            ..base
        },
        RetainedSceneKey {
            render_height: 721,
            ..base
        },
        RetainedSceneKey {
            sample_count: 4,
            ..base
        },
        RetainedSceneKey {
            post_view_generation: 4,
            ..base
        },
        RetainedSceneKey {
            clear_color: [0.1, 0.2, 0.4, 1.0],
            ..base
        },
        // A depth-tested UI primitive appearing over an otherwise static scene
        // needs the depth prepass encoded once before it can test against it.
        RetainedSceneKey {
            depth_prepass_needed: true,
            ..base
        },
        RetainedSceneKey {
            blend_screen_active: true,
            ..base
        },
        RetainedSceneKey {
            post_stage_count: 1,
            ..base
        },
    ];
    for mutated in mutations {
        assert_ne!(base, mutated, "key must reject a changed scene target");
    }
}
