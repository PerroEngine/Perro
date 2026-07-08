use perro_animation::{
    AnimationBoneSelector, AnimationKeyMode, AnimationRetargetMap, AnimationTrackValue,
    parse_panim, parse_pretarget, retarget_skeleton3d_clip,
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
