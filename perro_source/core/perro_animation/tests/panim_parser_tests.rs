use perro_animation::{
    AnimationBoneSelector, AnimationEase, AnimationInterpolation, AnimationParam,
    AnimationTrackValue, parse_panim,
};
use perro_scene::{MeshInstance3DField, Node2DField, Node3DField, NodeField, Sprite2DField};

#[test]
fn parses_sparse_keyframes_and_events() {
    let src = r#"
[Animation]
name = "AttackA"
fps = 30
[/Animation]

[Objects]
@Player = MeshInstance3D
[/Objects]

[Frame0]
@Player {
    position = (0,0,0)
    rotation = (0,0,0,1)
    scale = (1,1,1)
    visible = true
}
[/Frame0]

[Frame25]
@Player {
    call_method = { name="slash", params=[1.0] }
}
[/Frame25]
"#;

    let clip = parse_panim(src).expect("expected valid panim");
    assert_eq!(clip.name.as_ref(), "AttackA");
    assert_eq!(clip.fps, 30.0);
    assert_eq!(clip.total_frames, 26);
    assert_eq!(clip.objects.len(), 1);
    assert_eq!(clip.object_tracks.len(), 2);
    assert_eq!(clip.frame_events.len(), 1);
    assert_eq!(clip.frame_events[0].frame, 25);
}

#[test]
fn parses_node3d_rotation_from_euler_vec3() {
    let src = r#"
[Animation]
name = "EulerRotation"
fps = 30
[/Animation]

[Objects]
@Hero = Node3D
[/Objects]

[Frame0]
@Hero {
    rotation = (0, 0, 0)
}
[/Frame0]

[Frame10]
@Hero {
    rotation = (0, 1.5707964, 0)
}
[/Frame10]
"#;

    let clip = parse_panim(src).expect("expected valid panim");
    let track = clip
        .object_tracks
        .iter()
        .find(|t| matches!(t.field, NodeField::Node3D(Node3DField::Position)))
        .expect("node3d transform track");

    assert_eq!(track.keys.len(), 2);
    let AnimationTrackValue::Transform3D(t0) = track.keys[0].value else {
        panic!("expected transform3d key at frame 0");
    };
    let AnimationTrackValue::Transform3D(t1) = track.keys[1].value else {
        panic!("expected transform3d key at frame 10");
    };

    // Identity at frame 0.
    assert!((t0.rotation.x - 0.0).abs() < 1e-5);
    assert!((t0.rotation.y - 0.0).abs() < 1e-5);
    assert!((t0.rotation.z - 0.0).abs() < 1e-5);
    assert!((t0.rotation.w - 1.0).abs() < 1e-5);

    // Non-identity quaternion should be produced for a non-zero Euler key.
    let norm = (t1.rotation.x * t1.rotation.x
        + t1.rotation.y * t1.rotation.y
        + t1.rotation.z * t1.rotation.z
        + t1.rotation.w * t1.rotation.w)
        .sqrt();
    assert!((norm - 1.0).abs() < 1e-4);
    assert!(
        (t1.rotation.x - 0.0).abs() > 1e-5
            || (t1.rotation.y - 0.0).abs() > 1e-5
            || (t1.rotation.z - 0.0).abs() > 1e-5
            || (t1.rotation.w - 1.0).abs() > 1e-5
    );
}

#[test]
fn parses_asset_field_tracks_with_vars() {
    let src = r#"
@mesh = "res://meshes/hero.glb:mesh[0]"
@mat = "res://materials/hero.mat"
@tex = "res://textures/hero.png"

[Animation]
name = "SwapAssets"
fps = 24
[/Animation]

[Objects]
@HeroMesh = MeshInstance3D
@HeroSprite = Sprite2D
[/Objects]

[Frame0]
@HeroMesh {
    mesh = @mesh
    material = @mat
}
@HeroSprite {
    texture = @tex
}
[/Frame0]
"#;

    let clip = parse_panim(src).expect("expected valid panim");
    assert_eq!(clip.object_tracks.len(), 3);

    let mut mesh_track = None;
    let mut material_track = None;
    let mut texture_track = None;
    for track in clip.object_tracks.iter() {
        match track.field {
            NodeField::MeshInstance3D(MeshInstance3DField::Mesh) => mesh_track = Some(track),
            NodeField::MeshInstance3D(MeshInstance3DField::Material) => material_track = Some(track),
            NodeField::Sprite2D(Sprite2DField::Texture) => texture_track = Some(track),
            _ => {}
        }
    }

    let mesh_track = mesh_track.expect("mesh track");
    let material_track = material_track.expect("material track");
    let texture_track = texture_track.expect("texture track");

    assert!(matches!(
        mesh_track.keys[0].value,
        AnimationTrackValue::AssetPath(_)
    ));
    assert!(matches!(
        material_track.keys[0].value,
        AnimationTrackValue::AssetPath(_)
    ));
    assert!(matches!(
        texture_track.keys[0].value,
        AnimationTrackValue::AssetPath(_)
    ));
}

#[test]
fn parses_persistent_track_controls() {
    let src = r#"
[Animation]
name = "InterpModes"
fps = 30
default_interp = "interpolate"
default_ease = "ease_in_out"
[/Animation]

[Objects]
@Hero = Node3D
[/Objects]

[Frame0]
@Hero {
    position.interp = "interpolate"
    position.ease = "ease_in"
    position = (0,0,0)
}
[/Frame0]

[Frame10]
@Hero {
    position.ease = "ease_out"
    position = (10,0,0)
}
[/Frame10]
"#;

    let clip = parse_panim(src).expect("expected valid panim");
    let track = clip
        .object_tracks
        .iter()
        .find(|t| matches!(t.field, NodeField::Node3D(Node3DField::Position)))
        .expect("position track");
    assert_eq!(track.keys.len(), 2);
    assert_eq!(track.keys[0].interpolation, AnimationInterpolation::Linear);
    assert_eq!(track.keys[0].ease, AnimationEase::EaseIn);
    assert_eq!(track.keys[1].interpolation, AnimationInterpolation::Linear);
    assert_eq!(track.keys[1].ease, AnimationEase::EaseOut);
}

#[test]
fn parses_node2d_z_index_track() {
    let src = r#"
[Animation]
name = "LayerSwap"
fps = 30
[/Animation]

[Objects]
@Hud = Node2D
[/Objects]

[Frame0]
@Hud {
    z_index = 1
}
[/Frame0]

[Frame5]
@Hud {
    z_index = 4
}
[/Frame5]
"#;

    let clip = parse_panim(src).expect("expected valid panim");
    let track = clip
        .object_tracks
        .iter()
        .find(|t| matches!(t.field, NodeField::Node2D(Node2DField::ZIndex)))
        .expect("z_index track");
    assert_eq!(track.keys.len(), 2);
    assert!(matches!(track.keys[0].value, AnimationTrackValue::I32(1)));
    assert!(matches!(track.keys[1].value, AnimationTrackValue::I32(4)));
}

#[test]
fn parses_every_interp_and_ease_combo_on_defaults() {
    let interp_cases = [
        ("step", AnimationInterpolation::Step),
        ("interpolate", AnimationInterpolation::Linear),
        ("linear", AnimationInterpolation::Linear),
        ("lerp", AnimationInterpolation::Linear),
        ("slerp", AnimationInterpolation::Linear),
    ];
    let ease_cases = [
        ("linear", AnimationEase::Linear),
        ("ease_in", AnimationEase::EaseIn),
        ("ease_out", AnimationEase::EaseOut),
        ("ease_in_out", AnimationEase::EaseInOut),
        ("easein", AnimationEase::EaseIn),
        ("easeout", AnimationEase::EaseOut),
        ("easeinout", AnimationEase::EaseInOut),
        ("in", AnimationEase::EaseIn),
        ("out", AnimationEase::EaseOut),
    ];

    for (interp_token, interp_expected) in interp_cases {
        for (ease_token, ease_expected) in ease_cases {
            let src = format!(
                r#"
[Animation]
name = "Combo"
fps = 30
default_interp = "{interp_token}"
default_ease = "{ease_token}"
[/Animation]

[Objects]
@Hero = Node3D
[/Objects]

[Frame0]
@Hero {{
    position = (0,0,0)
}}
[/Frame0]

[Frame10]
@Hero {{
    position = (10,0,0)
}}
[/Frame10]
"#
            );

            let clip = parse_panim(&src).unwrap_or_else(|e| {
                panic!(
                    "failed parsing combo interp={} ease={}: {}",
                    interp_token, ease_token, e
                )
            });
            let track = clip
                .object_tracks
                .iter()
                .find(|t| matches!(t.field, NodeField::Node3D(Node3DField::Position)))
                .expect("position track");
            assert_eq!(track.keys.len(), 2);
            assert_eq!(
                track.keys[0].interpolation, interp_expected,
                "interp token {}",
                interp_token
            );
            assert_eq!(track.keys[1].interpolation, interp_expected);
            assert_eq!(track.keys[0].ease, ease_expected, "ease token {}", ease_token);
            assert_eq!(track.keys[1].ease, ease_expected);
        }
    }
}

#[test]
fn track_controls_persist_until_reset() {
    let src = r#"
[Animation]
name = "Persist"
fps = 30
[/Animation]

[Objects]
@Hero = Node3D
[/Objects]

[Frame0]
@Hero {
    position.interp = "step"
    position.ease = "ease_in"
    position = (0,0,0)
}
[/Frame0]

[Frame5]
@Hero {
    position = (5,0,0)
}
[/Frame5]

[Frame8]
@Hero {
    position.interp = "interpolate"
    position.ease = "ease_out"
    position = (8,0,0)
}
[/Frame8]
"#;

    let clip = parse_panim(src).expect("expected valid panim");
    let track = clip
        .object_tracks
        .iter()
        .find(|t| matches!(t.field, NodeField::Node3D(Node3DField::Position)))
        .expect("position track");

    assert_eq!(track.keys.len(), 3);
    assert_eq!(track.keys[0].interpolation, AnimationInterpolation::Step);
    assert_eq!(track.keys[1].interpolation, AnimationInterpolation::Step);
    assert_eq!(track.keys[2].interpolation, AnimationInterpolation::Linear);
    assert_eq!(track.keys[0].ease, AnimationEase::EaseIn);
    assert_eq!(track.keys[1].ease, AnimationEase::EaseIn);
    assert_eq!(track.keys[2].ease, AnimationEase::EaseOut);
}

#[test]
fn track_control_only_affects_following_keys_in_same_frame() {
    let src = r#"
[Animation]
name = "Order"
fps = 30
[/Animation]

[Objects]
@Hero = Node3D
[/Objects]

[Frame0]
@Hero {
    position = (0,0,0)
    position.interp = "step"
    position.ease = "ease_in"
}
[/Frame0]

[Frame10]
@Hero {
    position = (10,0,0)
}
[/Frame10]
"#;

    let clip = parse_panim(src).expect("expected valid panim");
    let track = clip
        .object_tracks
        .iter()
        .find(|t| matches!(t.field, NodeField::Node3D(Node3DField::Position)))
        .expect("position track");

    assert_eq!(track.keys.len(), 2);
    assert_eq!(track.keys[0].interpolation, AnimationInterpolation::Linear);
    assert_eq!(track.keys[0].ease, AnimationEase::Linear);
    assert_eq!(track.keys[1].interpolation, AnimationInterpolation::Step);
    assert_eq!(track.keys[1].ease, AnimationEase::EaseIn);
}

#[test]
fn rejects_unknown_interp_and_ease_tokens() {
    let bad_interp = r#"
[Animation]
name = "BadInterp"
fps = 30
[/Animation]

[Objects]
@Hero = Node3D
[/Objects]

[Frame0]
@Hero {
    position.interp = "cubic"
    position = (0,0,0)
}
[/Frame0]
"#;
    let err = parse_panim(bad_interp).expect_err("expected parse failure");
    assert!(err.contains("unknown interpolation"));

    let bad_ease = r#"
[Animation]
name = "BadEase"
fps = 30
[/Animation]

[Objects]
@Hero = Node3D
[/Objects]

[Frame0]
@Hero {
    position.ease = "bounce"
    position = (0,0,0)
}
[/Frame0]
"#;
    let err = parse_panim(bad_ease).expect_err("expected parse failure");
    assert!(err.contains("unknown ease"));
}

#[test]
fn parses_object_node_and_field_param_references() {
    let src = r#"
[Animation]
name = "Refs"
fps = 30
[/Animation]

[Objects]
@Hero = Node3D
@Target = Node3D
[/Objects]

[Frame0]
@Hero {
    position = (1,2,3)
}
@Target {
    position = (4,5,6)
    call_method = { name="aim_at", params=[@Hero, @Hero.position] }
    set_var = { name="cached_target", value=@Hero }
}
[/Frame0]
"#;

    let clip = parse_panim(src).expect("expected valid panim");
    assert_eq!(clip.frame_events.len(), 2);

    let call_event = clip
        .frame_events
        .iter()
        .find(|e| matches!(e.event, perro_animation::AnimationEvent::CallMethod { .. }))
        .expect("call_method event");
    if let perro_animation::AnimationEvent::CallMethod { params, .. } = &call_event.event {
        assert_eq!(params.len(), 2);
        assert!(matches!(
            &params[0],
            AnimationParam::ObjectNode(object) if object.as_ref() == "Hero"
        ));
        assert!(matches!(
            &params[1],
            AnimationParam::ObjectField { object, field }
                if object.as_ref() == "Hero" && field.as_ref() == "position"
        ));
    } else {
        panic!("expected call_method");
    }

    let set_event = clip
        .frame_events
        .iter()
        .find(|e| matches!(e.event, perro_animation::AnimationEvent::SetVar { .. }))
        .expect("set_var event");
    if let perro_animation::AnimationEvent::SetVar { value, .. } = &set_event.event {
        assert!(matches!(
            value,
            AnimationParam::ObjectNode(object) if object.as_ref() == "Hero"
        ));
    } else {
        panic!("expected set_var");
    }
}

#[test]
fn rejects_invalid_reference_tokens() {
    let src = r#"
[Animation]
name = "BadRef"
fps = 30
[/Animation]

[Objects]
@Hero = Node3D
[/Objects]

[Frame0]
@Hero {
    call_method = { name="broken", params=[@1Hero.position] }
}
[/Frame0]
"#;

    let err = parse_panim(src).expect_err("expected parse failure");
    assert!(err.contains("invalid object reference"));
}

#[test]
fn parses_skeleton_bone_tracks_by_index_and_name() {
    let src = r#"
[Animation]
name = "BoneClip"
fps = 30
[/Animation]

[Objects]
@Rig = Skeleton3D
[/Objects]

[Frame0]
@Rig {
    bones[0].position = (1,2,3)
    bone["Spine"].rotation = (0,0,0,1)
}
[/Frame0]
"#;

    let clip = parse_panim(src).expect("expected valid panim");
    assert_eq!(clip.object_tracks.len(), 2);

    let index_track = clip
        .object_tracks
        .iter()
        .find(|t| {
            matches!(
                &t.bone_target,
                Some(target) if matches!(target.selector, AnimationBoneSelector::Index(0))
            )
        })
        .expect("index bone track");
    assert!(matches!(
        index_track.keys[0].value,
        AnimationTrackValue::Transform3D(_)
    ));

    let name_track = clip
        .object_tracks
        .iter()
        .find(|t| {
            matches!(
                &t.bone_target,
                Some(target)
                    if matches!(
                        &target.selector,
                        AnimationBoneSelector::Name(name) if name.as_ref() == "Spine"
                    )
            )
        })
        .expect("named bone track");
    assert!(matches!(
        name_track.keys[0].value,
        AnimationTrackValue::Transform3D(_)
    ));
}

#[test]
fn skeleton_bone_track_controls_persist() {
    let src = r#"
[Animation]
name = "BoneControls"
fps = 30
[/Animation]

[Objects]
@Rig = Skeleton3D
[/Objects]

[Frame0]
@Rig {
    bones[0].position.interp = "step"
    bones[0].position.ease = "ease_in"
    bones[0].position = (0,0,0)
}
[/Frame0]

[Frame10]
@Rig {
    bones[0].position = (5,0,0)
}
[/Frame10]
"#;

    let clip = parse_panim(src).expect("expected valid panim");
    let track = clip
        .object_tracks
        .iter()
        .find(|t| {
            matches!(
                &t.bone_target,
                Some(target) if matches!(target.selector, AnimationBoneSelector::Index(0))
            )
        })
        .expect("bone track");

    assert_eq!(track.keys.len(), 2);
    assert_eq!(track.keys[0].interpolation, AnimationInterpolation::Step);
    assert_eq!(track.keys[1].interpolation, AnimationInterpolation::Step);
    assert_eq!(track.keys[0].ease, AnimationEase::EaseIn);
    assert_eq!(track.keys[1].ease, AnimationEase::EaseIn);
}
