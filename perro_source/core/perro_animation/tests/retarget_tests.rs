use perro_animation::{
    ANIMATION_TRANSFORM_MASK_POSITION, AnimationBoneSelector, AnimationKeyMode,
    AnimationRetargetMap, AnimationTrackValue, AnimationTranslationPolicy, parse_panim,
    parse_pretarget, parse_pretarget_profile, retarget_skeleton3d_clip,
    retarget_skeleton3d_clip_with_profile,
};
use perro_scene::NodeType;

#[test]
fn retarget_skeleton3d_clip_aliases_named_bones() {
    let clip = parse_panim(
        r#"
[Objects]
Rig = Skeleton3D
[/Objects]

[Frame0]
@Rig {
    bones["upper_arm_l"].rotation_deg = (0, 0, 0)
}
[/Frame0]

[Frame8]
@Rig {
    bones["upper_arm_l"].rotation_deg = (0, 45, 0)
}
[/Frame8]
"#,
    )
    .unwrap();
    let map = parse_pretarget(
        r#"
source_object = Rig
target_object = HeroRig
bone upper_arm_l => Arm.L
"#,
    )
    .unwrap();

    let (retargeted, report) = retarget_skeleton3d_clip(&clip, &map);

    assert_eq!(report.remapped_tracks, 1);
    assert_eq!(report.dropped_tracks, 0);
    assert!(
        retargeted
            .objects
            .iter()
            .any(|object| object.name == "HeroRig" && object.node_type == NodeType::Skeleton3D)
    );
    let track = retargeted
        .object_tracks
        .iter()
        .find(|track| track.object == "HeroRig")
        .expect("retargeted track");
    assert!(matches!(
        &track.bone_target.as_ref().unwrap().selector,
        AnimationBoneSelector::Name(name) if name.as_ref() == "Arm.L"
    ));
    assert!(matches!(
        track.keys[1].value,
        AnimationTrackValue::Transform3D(_)
    ));
}

#[test]
fn retarget_skeleton3d_clip_drops_unmapped_when_requested() {
    let clip = parse_panim(
        r#"
[Objects]
Rig = Skeleton3D
[/Objects]

[Frame0]
@Rig {
    bones["Spine"].position = (0, 1, 0)
    bones["Head"].position = (0, 2, 0)
}
[/Frame0]
"#,
    )
    .unwrap();
    let map = parse_pretarget(
        r#"
source = Rig
target = TargetRig
keep_unmapped = false
bone Spine => spine_01
"#,
    )
    .unwrap();

    let (retargeted, report) = retarget_skeleton3d_clip(&clip, &map);

    assert_eq!(report.remapped_tracks, 1);
    assert_eq!(report.dropped_tracks, 1);
    assert_eq!(retargeted.object_tracks.len(), 1);
    assert!(matches!(
        &retargeted.object_tracks[0].bone_target.as_ref().unwrap().selector,
        AnimationBoneSelector::Name(name) if name.as_ref() == "spine_01"
    ));
}

#[test]
fn retarget_skeleton3d_clip_leaves_non_source_tracks_unchanged() {
    let clip = parse_panim(
        r#"
[Objects]
Rig = Skeleton3D
Camera = Camera3D
[/Objects]

[Frame0]
@Rig {
    bones["Spine"].position = (0, 1, 0)
}
@Camera {
    position = (1, 2, 3)
}
[/Frame0]
"#,
    )
    .unwrap();
    let map = AnimationRetargetMap {
        source_object: "Rig".into(),
        target_object: "TargetRig".into(),
        keep_unmapped: true,
        bones: vec![].into(),
    };

    let (retargeted, report) = retarget_skeleton3d_clip(&clip, &map);

    assert_eq!(report.kept_unmapped_tracks, 1);
    assert!(
        retargeted
            .object_tracks
            .iter()
            .any(|track| track.object == "Camera")
    );
    assert!(
        retargeted
            .object_tracks
            .iter()
            .any(|track| track.object == "TargetRig")
    );
}

#[test]
fn parse_pretarget_rejects_missing_target() {
    let err = parse_pretarget("source = Rig").unwrap_err();
    assert!(err.contains("target_object"));
}

#[test]
fn retarget_keeps_key_modes() {
    let clip = parse_panim(
        r#"
[Objects]
Rig = Skeleton3D
[/Objects]

[Frame0?]
@Rig {
    bones["Spine"].position = (0, 1, 0)
}
[/Frame0]
"#,
    )
    .unwrap();
    let map = parse_pretarget("source=Rig\ntarget=TargetRig\nbone Spine=>spine_01\n").unwrap();

    let (retargeted, _) = retarget_skeleton3d_clip(&clip, &map);

    assert_eq!(
        retargeted.object_tracks[0].keys[0].mode,
        AnimationKeyMode::Open
    );
}

#[test]
fn retarget_profile_aligns_source_rest_to_target_rest() {
    let clip = parse_panim(
        r#"
[Objects]
Rig = Skeleton3D
[/Objects]

[Frame0]
@Rig {
    bones["arm"].position = (1, 2, 0)
    bones["arm"].rotation = (0, 0, 0, 1)
    bones["arm"].scale = (2, 4, 2)
}
[/Frame0]
"#,
    )
    .unwrap();
    let profile = parse_pretarget_profile(
        r#"
source = Rig
target = HeroRig
bone arm => Arm.L
source_rest arm = (1, 0, 0) | (0, 0, 0.70710677, 0.70710677) | (2, 4, 2)
target_rest Arm.L = (5, 6, 7) | (0, 0, 0.70710677, 0.70710677) | (3, 4, 5)
"#,
    )
    .unwrap();

    let (retargeted, report) = retarget_skeleton3d_clip_with_profile(&clip, &profile);

    assert_eq!(report.aligned_tracks, 1);
    let AnimationTrackValue::Transform3D(transform) = &retargeted.object_tracks[0].keys[0].value
    else {
        panic!("expected Transform3D");
    };
    assert!(transform.rotation.x.abs() < 1.0e-5);
    assert!(transform.rotation.y.abs() < 1.0e-5);
    assert!(transform.rotation.z.abs() < 1.0e-5);
    assert!((transform.rotation.w.abs() - 1.0).abs() < 1.0e-5);
    assert!((transform.position.x - 5.0).abs() < 1.0e-5);
    assert!((transform.position.y - 9.0).abs() < 1.0e-5);
    assert!((transform.position.z - 7.0).abs() < 1.0e-5);
    assert_eq!(transform.scale.to_array(), [3.0, 4.0, 5.0]);
}

#[test]
fn retarget_profile_root_only_drops_non_root_translation_track() {
    let clip = parse_panim(
        r#"
[Objects]
Rig = Skeleton3D
[/Objects]

[Frame0]
@Rig {
    bones["hips"].position = (1, 2, 3)
    bones["spine"].position = (4, 5, 6)
}
[/Frame0]
"#,
    )
    .unwrap();
    let profile = parse_pretarget_profile(
        r#"
source = Rig
target = HeroRig
translation = root_only
root_bone = hips
bone hips => Hips
bone spine => Spine
"#,
    )
    .unwrap();

    let (retargeted, report) = retarget_skeleton3d_clip_with_profile(&clip, &profile);

    assert_eq!(report.translation_dropped_tracks, 1);
    assert_eq!(retargeted.object_tracks.len(), 1);
    assert!(matches!(
        &retargeted.object_tracks[0].bone_target.as_ref().unwrap().selector,
        AnimationBoneSelector::Name(name) if name.as_ref() == "Hips"
    ));
    assert_eq!(
        retargeted.object_tracks[0].transform3d_mask,
        ANIMATION_TRANSFORM_MASK_POSITION
    );
}

#[test]
fn retarget_profile_root_only_keeps_rotation_on_mixed_track() {
    let clip = parse_panim(
        r#"
[Objects]
Rig = Skeleton3D
[/Objects]

[Frame0]
@Rig {
    bones["arm"].position = (4, 5, 6)
    bones["arm"].rotation = (0, 0, 0, 1)
}
[/Frame0]
"#,
    )
    .unwrap();
    let profile = parse_pretarget_profile(
        r#"
source = Rig
target = HeroRig
translation = root_only
root_bone = hips
bone arm => Arm.L
target_rest Arm.L = (1, 2, 3) | (0, 0, 0, 1)
"#,
    )
    .unwrap();

    let (retargeted, report) = retarget_skeleton3d_clip_with_profile(&clip, &profile);

    assert_eq!(report.translation_locked_tracks, 1);
    assert_eq!(retargeted.object_tracks[0].transform3d_mask, 2);
    let AnimationTrackValue::Transform3D(transform) = &retargeted.object_tracks[0].keys[0].value
    else {
        panic!("expected Transform3D");
    };
    assert_eq!(transform.position.to_array(), [1.0, 2.0, 3.0]);
}

#[test]
fn old_pretarget_format_keeps_legacy_translation_default() {
    let source = "source=Rig\ntarget=HeroRig\nkeep_unmapped=false\nbone arm=>Arm.L\n";
    let profile = parse_pretarget_profile(source).unwrap();

    assert_eq!(profile.translation_policy, AnimationTranslationPolicy::All);
    assert!(profile.source_rest.is_empty());
    assert!(profile.target_rest.is_empty());
    assert_eq!(parse_pretarget(source).unwrap(), profile.map);
}

#[test]
fn root_only_policy_needs_root_bone() {
    let err =
        parse_pretarget_profile("source=Rig\ntarget=HeroRig\ntranslation=root_only\n").unwrap_err();
    assert!(err.contains("root_bone"));
}
