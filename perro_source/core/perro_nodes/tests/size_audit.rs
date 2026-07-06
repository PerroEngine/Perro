//! Size audit for scene node storage (SoA plan phase 0).
//! Run: cargo test -p perro_nodes --test size_audit -- --nocapture

use perro_nodes::*;
use std::mem::size_of;

macro_rules! sizes {
    ($($ty:ident),* $(,)?) => {{
        let mut v: Vec<(&'static str, usize)> = vec![
            $((stringify!($ty), size_of::<$ty>())),*
        ];
        v.sort_by(|a, b| b.1.cmp(&a.1));
        v
    }};
}

#[test]
fn scene_node_data_stride_stays_small() {
    // Heavy variants are boxed via the `Boxed` marker in define_scene_nodes!.
    // If this fails, a new/grown inline variant widened every arena slot —
    // box it (or shrink it) instead of raising the cap.
    assert!(
        size_of::<SceneNodeData>() <= 256,
        "SceneNodeData stride grew to {} B",
        size_of::<SceneNodeData>()
    );
}

#[test]
fn print_scene_node_sizes() {
    println!("SceneNode      = {} B", size_of::<SceneNode>());
    println!("SceneNodeData  = {} B", size_of::<SceneNodeData>());
    println!("---- payload sizes (desc) ----");
    let v = sizes![
        // 2d
        Node2D,
        Camera2D,
        CameraStream2D,
        Button2D,
        ImageButton2D,
        Sprite2D,
        NineSlice2D,
        AnimatedSprite2D,
        TileMap2D,
        ParticleEmitter2D,
        WaterBody2D,
        AmbientLight2D,
        RayLight2D,
        PointLight2D,
        SpotLight2D,
        Skeleton2D,
        BoneAttachment2D,
        IKTarget2D,
        PhysicsBoneChain2D,
        BoneCollider2D,
        CollisionShape2D,
        StaticBody2D,
        Area2D,
        RigidBody2D,
        CharacterBody2D,
        PhysicsForceEmitter2D,
        PinJoint2D,
        DistanceJoint2D,
        FixedJoint2D,
        AudioMask2D,
        AudioEffectZone2D,
        AudioPortal2D,
        // 3d
        Node3D,
        Camera3D,
        CameraStream3D,
        MeshInstance3D,
        MultiMeshInstance3D,
        ParticleEmitter3D,
        WaterBody3D,
        Decal3D,
        Sky3D,
        AmbientLight3D,
        RayLight3D,
        PointLight3D,
        SpotLight3D,
        Skeleton3D,
        BoneAttachment3D,
        IKTarget3D,
        PhysicsBoneChain3D,
        BoneCollider3D,
        CollisionShape3D,
        StaticBody3D,
        Area3D,
        RigidBody3D,
        CharacterBody3D,
        PhysicsForceEmitter3D,
        BallJoint3D,
        HingeJoint3D,
        FixedJoint3D,
        AudioMask3D,
        AudioEffectZone3D,
        AudioPortal3D,
        // ui
        UiNode,
        UiCameraStream,
        UiPanel,
        UiButton,
        UiDropdown,
        UiColorPicker,
        UiShape,
        UiCheckbox,
        UiImage,
        UiImageButton,
        UiNineSlice,
        UiAnimatedImage,
        UiLabel,
        UiTextBox,
        UiTextBlock,
        UiScrollContainer,
        UiLayout,
        UiHLayout,
        UiVLayout,
        UiGrid,
        UiTreeList,
        // resources
        AnimationPlayer,
        AnimationTree,
    ];
    for (name, size) in v {
        println!("{name:28} {size:>5} B");
    }
}
