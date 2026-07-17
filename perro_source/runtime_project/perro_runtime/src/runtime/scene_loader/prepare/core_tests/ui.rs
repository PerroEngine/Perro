mod ui {
    use super::*;

    #[test]
    fn scene_loader_ignores_ui_position_ratio_and_uses_translation_ratios() {
        let scene = Parser::new(
            r#"
            $root = @panel
            [panel]
            [UiPanel]
                position_ratio = (0.5, 0.98)
                position_percent = (20, 80)
                translation_ratio = (0.25, -0.5)
                self_translation_ratio = (1.0, 0.0)
            [/UiPanel]
            [/panel]
            "#,
        )
        .parse_scene();

        let prepared =
            prepare_scene_with_loader(&scene, &|path| Err(format!("unknown scene path `{path}`")))
                .expect("prepare scene");
        let panel = prepared
            .nodes
            .iter()
            .find(|pending| pending.key_name == "panel")
            .expect("panel node");

        match &panel.node.data {
            SceneNodeData::UiPanel(panel) => {
                assert_eq!(
                    panel.transform.position,
                    perro_ui::UiVector2::ratio(0.5, 0.5)
                );
                assert_eq!(panel.transform.translation, Vector2::new(0.25, -0.5));
                assert_eq!(panel.transform.self_translation, Vector2::new(1.0, 0.0));
            }
            other => panic!("expected UiPanel node, got {other:?}"),
        }
    }

    #[test]
    fn scene_loader_builds_ik_target_3d_fields_and_skeleton_link() {
        let scene = Parser::new(
            r#"
            $root = @Rig
            [Rig]
            [Skeleton3D]
                skeleton = "res://rig.pskel"
            [/Skeleton3D]
            [/Rig]

            [HandTarget]
            [IKTarget3D]
                skeleton = @Rig
                bone = 5
                chain_length = 3
                iterations = 1000000
                tolerance = 0.05
                weight = 0.75
                match_rotation = false
            [/IKTarget3D]
            [/HandTarget]

            [HandTarget2D]
            [IKTarget2D]
                skeleton = @Rig
                iterations = 1000000
            [/IKTarget2D]
            [/HandTarget2D]
            "#,
        )
        .parse_scene();

        let prepared =
            prepare_scene_with_loader(&scene, &|path| Err(format!("unknown scene path `{path}`")))
                .expect("prepare scene");

        let target = prepared
            .nodes
            .iter()
            .find(|pending| pending.key_name == "HandTarget")
            .expect("ik target node");
        match &target.node.data {
            SceneNodeData::IKTarget3D(ik) => {
                assert_eq!(ik.params.bone_index, 5);
                assert_eq!(ik.params.chain_length, 3);
                assert_eq!(
                    ik.params.iterations,
                    perro_structs::MAX_SKELETAL_SOLVER_ITERATIONS
                );
                assert_eq!(ik.params.tolerance, 0.05);
                assert_eq!(ik.params.weight, 0.75);
                assert!(!ik.params.match_rotation);
            }
            other => panic!("expected IKTarget3D node, got {other:?}"),
        }
        assert!(target.ik_target_skeleton_target.is_some());

        let target_2d = prepared
            .nodes
            .iter()
            .find(|pending| pending.key_name == "HandTarget2D")
            .expect("2d ik target node");
        match &target_2d.node.data {
            SceneNodeData::IKTarget2D(ik) => assert_eq!(
                ik.params.iterations,
                perro_structs::MAX_SKELETAL_SOLVER_ITERATIONS
            ),
            other => panic!("expected IKTarget2D node, got {other:?}"),
        }
    }

    #[test]
    fn scene_loader_parses_physics_bone_chain_iters_alias() {
        let scene = Parser::new(
            r#"
            $root = @Rig
            [Rig]
            [Skeleton3D]
                skeleton = "res://rig.pskel"
            [/Skeleton3D]
            [/Rig]

            [Tail2D]
            [PhysicsBoneChain2D]
                skeleton = @Rig
                bone = 4
                iters = 1000000
            [/PhysicsBoneChain2D]
            [/Tail2D]

            [Tail3D]
            [PhysicsBoneChain3D]
                skeleton = @Rig
                bone = 5
                iters = 1000000
            [/PhysicsBoneChain3D]
            [/Tail3D]
            "#,
        )
        .parse_scene();

        let prepared =
            prepare_scene_with_loader(&scene, &|path| Err(format!("unknown scene path `{path}`")))
                .expect("prepare scene");

        let tail_2d = prepared
            .nodes
            .iter()
            .find(|pending| pending.key_name == "Tail2D")
            .expect("2d physics chain");
        match &tail_2d.node.data {
            SceneNodeData::PhysicsBoneChain2D(chain) => assert_eq!(
                chain.iterations,
                perro_structs::MAX_SKELETAL_SOLVER_ITERATIONS
            ),
            other => panic!("expected PhysicsBoneChain2D node, got {other:?}"),
        }

        let tail_3d = prepared
            .nodes
            .iter()
            .find(|pending| pending.key_name == "Tail3D")
            .expect("3d physics chain");
        match &tail_3d.node.data {
            SceneNodeData::PhysicsBoneChain3D(chain) => assert_eq!(
                chain.iterations,
                perro_structs::MAX_SKELETAL_SOLVER_ITERATIONS
            ),
            other => panic!("expected PhysicsBoneChain3D node, got {other:?}"),
        }
    }

    #[test]
    fn scene_loader_rejects_bone_2d_node() {
        let err = std::panic::catch_unwind(|| {
            Parser::new(
                r#"
            $root = @Rig2D
            [Rig2D]
            [Skeleton2D]
                position = (10, 20)
            [/Skeleton2D]
            [/Rig2D]

            [UpperArm]
            parent = @Rig2D
            [Bone2D]
                position = (4, 5)
                rotation = 0.25
                scale = (1, 1)
                rest = { position = (4, 5), rotation = 0.25, scale = (1, 1) }
                pose = { position = (6, 7), rotation = 0.5, scale = (1, 1) }
            [/Bone2D]
            [/UpperArm]
            "#,
            )
            .parse_scene()
        })
        .expect_err("expected bone2d scene node rejection");
        let msg = err
            .downcast_ref::<String>()
            .map(String::as_str)
            .or_else(|| err.downcast_ref::<&str>().copied())
            .unwrap_or("");
        assert!(msg.contains("unsupported scene node type `Bone2D`"));
    }

    #[test]
    fn scene_loader_builds_skeleton_2d_mirror_nodes() {
        let scene = Parser::new(
            r#"
            $root = @Rig2D
            [Rig2D]
            [Skeleton2D]
                position = (10, 20)
                skeleton = "res://rig.pskel2d"
            [/Skeleton2D]
            [/Rig2D]

            [Hand]
            parent = @Rig2D
            [BoneAttachment2D]
                skeleton = @Rig2D
                bone = 1
            [/BoneAttachment2D]
            [/Hand]
            "#,
        )
        .parse_scene();

        let prepared =
            prepare_scene_with_loader(&scene, &|path| Err(format!("unknown scene path `{path}`")))
                .expect("prepare scene");

        let rig = prepared
            .nodes
            .iter()
            .find(|pending| pending.key_name == "Rig2D")
            .expect("rig node");
        assert!(matches!(rig.node.data, SceneNodeData::Skeleton2D(_)));
        assert_eq!(rig.skeleton_source.as_deref(), Some("res://rig.pskel2d"));

        let hand = prepared
            .nodes
            .iter()
            .find(|pending| pending.key_name == "Hand")
            .expect("hand node");
        match &hand.node.data {
            SceneNodeData::BoneAttachment2D(node) => {
                assert_eq!(node.bone_index, 1);
                assert!(hand.bone_attachment_skeleton_target.is_some());
            }
            other => panic!("expected BoneAttachment2D node, got {other:?}"),
        }
    }

    #[test]
    fn scene_loader_extracts_bone_pose_overrides() {
        let scene = Parser::new(
            r#"
            $root = @Rig
            [Rig]
            [Skeleton3D]
                skeleton = "res://rig.gltf:skeleton[0]"
                bones = {
                    Spine = { position = (0, 1.5, 0), rotation = (0, 0, 0, 1), scale = (1, 1, 1) },
                    Head = { rotation_deg = (0, 90, 0) },
                    Empty = { }
                }
            [/Skeleton3D]
            [/Rig]

            [Rig2D]
            parent = @Rig
            [Skeleton2D]
                skeleton = "res://rig.pskel2d"
                bones = {
                    Arm = { position = (4, 5), rotation_deg = 90 }
                }
            [/Skeleton2D]
            [/Rig2D]
            "#,
        )
        .parse_scene();

        let prepared =
            prepare_scene_with_loader(&scene, &|path| Err(format!("unknown scene path `{path}`")))
                .expect("prepare scene");

        let rig = prepared
            .nodes
            .iter()
            .find(|pending| pending.key_name == "Rig")
            .expect("rig node");
        assert_eq!(rig.bone_pose_overrides.len(), 2, "empty override dropped");
        let spine = &rig.bone_pose_overrides[0];
        assert_eq!(spine.bone, "Spine");
        assert_eq!(spine.position_3d, Some(Vector3::new(0.0, 1.5, 0.0)));
        assert_eq!(spine.rotation_3d, Some(Quaternion::new(0.0, 0.0, 0.0, 1.0)));
        assert_eq!(spine.scale_3d, Some(Vector3::new(1.0, 1.0, 1.0)));
        let head = &rig.bone_pose_overrides[1];
        assert_eq!(head.bone, "Head");
        assert!(head.position_3d.is_none());
        let quat = head.rotation_3d.expect("rotation_deg converts to quat");
        let expected = Quaternion::from_euler_xyz(0.0, 90f32.to_radians(), 0.0);
        assert!((quat.x - expected.x).abs() < 1.0e-5);
        assert!((quat.w - expected.w).abs() < 1.0e-5);

        let rig_2d = prepared
            .nodes
            .iter()
            .find(|pending| pending.key_name == "Rig2D")
            .expect("rig2d node");
        assert_eq!(rig_2d.bone_pose_overrides.len(), 1);
        let arm = &rig_2d.bone_pose_overrides[0];
        assert_eq!(arm.bone, "Arm");
        assert_eq!(arm.position_2d, Some(Vector2::new(4.0, 5.0)));
        let rot = arm.rotation_2d.expect("deg converts to radians");
        assert!((rot - 90f32.to_radians()).abs() < 1.0e-5);
    }

    #[test]
    fn bone_pose_overrides_apply_to_pose_only() {
        let mut skeleton = Skeleton3D::default();
        skeleton.bones = vec![
            perro_nodes::skeleton_3d::Bone3D {
                name: std::borrow::Cow::Borrowed("Spine"),
                ..perro_nodes::skeleton_3d::Bone3D::new()
            },
            perro_nodes::skeleton_3d::Bone3D {
                name: std::borrow::Cow::Borrowed("Head"),
                ..perro_nodes::skeleton_3d::Bone3D::new()
            },
        ];
        let overrides = vec![
            PendingBonePoseOverride {
                bone: "Spine".to_string(),
                position_3d: Some(Vector3::new(1.0, 2.0, 3.0)),
                ..PendingBonePoseOverride::default()
            },
            PendingBonePoseOverride {
                bone: "Missing".to_string(),
                position_3d: Some(Vector3::new(9.0, 9.0, 9.0)),
                ..PendingBonePoseOverride::default()
            },
        ];

        apply_bone_pose_overrides_3d(&mut skeleton, &overrides);

        assert_eq!(skeleton.bones[0].pose.position, Vector3::new(1.0, 2.0, 3.0));
        // Rest pose stays untouched; only the live pose overrides.
        assert_eq!(skeleton.bones[0].rest.position, Vector3::ZERO);
        assert_eq!(skeleton.bones[1].pose.position, Vector3::ZERO);
    }

    #[test]
    fn scene_loader_rejects_quoted_skeleton_node_refs() {
        let scene = Parser::new(
            r#"
            $root = @Rig
            [Rig]
            [Skeleton3D]
                skeleton = "res://rig.pskel"
            [/Skeleton3D]
            [/Rig]

            [Mesh]
            [MeshInstance3D]
                skeleton = "Rig"
            [/MeshInstance3D]
            [/Mesh]
            "#,
        )
        .parse_scene();

        let err = match prepare_scene_with_loader(&scene, &|path| {
            Err(format!("unknown scene path `{path}`"))
        }) {
            Ok(_) => panic!("expected quoted skeleton node ref rejection"),
            Err(err) => err,
        };
        assert!(err.contains("MeshInstance3D.skeleton must be a node ref like @SkeletonNode"));
    }

    #[test]
    fn scene_loader_parses_multimesh_instance_grid() {
        let scene = Parser::new(
            r#"
            $root = @Batch
            [Batch]
            [MultiMeshInstance3D]
                instance_grid = { counts=(4, 3, 2) spacing=(2, 1.5, 3) origin=(-3, 0.5, -4) scale=(1.5, 0.75, 2.0) height_wave=0.0 rotation_step_deg=(0, 10, 0) }
            [/MultiMeshInstance3D]
            [/Batch]
            "#,
        )
        .parse_scene();

        let prepared =
            prepare_scene_with_loader(&scene, &|path| Err(format!("unknown scene path `{path}`")))
                .expect("prepare scene");

        let batch = prepared
            .nodes
            .iter()
            .find(|pending| pending.key_name == "Batch")
            .expect("batch node");
        match &batch.node.data {
            SceneNodeData::MultiMeshInstance3D(mesh) => {
                assert_eq!(mesh.instances.len(), 24);
                assert_eq!(
                    mesh.instances[0].transform.position,
                    Vector3::new(-3.0, 0.5, -4.0)
                );
                assert_eq!(
                    mesh.instances[23].transform.position,
                    Vector3::new(3.0, 3.5, -1.0)
                );
                assert_eq!(
                    mesh.instances[0].transform.scale,
                    Vector3::new(1.5, 0.75, 2.0)
                );
            }
            other => panic!("expected MultiMeshInstance3D node, got {other:?}"),
        }
    }

    #[test]
    fn scene_loader_parses_mesh_lod_options() {
        let scene = Parser::new(
            r#"
            $root = @Mesh
            [Mesh]
            [MeshInstance3D]
                min_lod = 1
                max_lod = 3
            [/MeshInstance3D]
            [/Mesh]

            [Batch]
            [MultiMeshInstance3D]
                lod_min = 2
                lod_max = 4
            [/MultiMeshInstance3D]
            [/Batch]
            "#,
        )
        .parse_scene();

        let prepared =
            prepare_scene_with_loader(&scene, &|path| Err(format!("unknown scene path `{path}`")))
                .expect("prepare scene");

        let mesh = prepared
            .nodes
            .iter()
            .find(|pending| pending.key_name == "Mesh")
            .expect("mesh node");
        match &mesh.node.data {
            SceneNodeData::MeshInstance3D(mesh) => {
                assert_eq!(mesh.lod.min_lod, 1);
                assert_eq!(mesh.lod.max_lod, 3);
            }
            other => panic!("expected MeshInstance3D node, got {other:?}"),
        }

        let batch = prepared
            .nodes
            .iter()
            .find(|pending| pending.key_name == "Batch")
            .expect("batch node");
        match &batch.node.data {
            SceneNodeData::MultiMeshInstance3D(mesh) => {
                assert_eq!(mesh.lod.min_lod, 2);
                assert_eq!(mesh.lod.max_lod, 4);
            }
            other => panic!("expected MultiMeshInstance3D node, got {other:?}"),
        }
    }

    #[test]
    fn scene_loader_parses_blend_shape_weights_and_aliases() {
        let scene = Parser::new(
            r#"
            $root = @Mesh
            [Mesh]
            [MeshInstance3D]
                shape_key_weights = [-1.0, 0.5, 2.0]
            [/MeshInstance3D]
            [/Mesh]

            [Batch]
            [MultiMeshInstance3D]
                morph_weights = [0.2, 1.5]
                instances = [
                    { position=[1.0, 0.0, 0.0] blend_shape_weights=[0.7, 0.4] },
                    { position=[2.0, 0.0, 0.0] }
                ]
            [/MultiMeshInstance3D]
            [/Batch]
            "#,
        )
        .parse_scene();

        let prepared =
            prepare_scene_with_loader(&scene, &|path| Err(format!("unknown scene path `{path}`")))
                .expect("prepare scene");

        let mesh = prepared
            .nodes
            .iter()
            .find(|pending| pending.key_name == "Mesh")
            .expect("mesh node");
        match &mesh.node.data {
            SceneNodeData::MeshInstance3D(mesh) => {
                assert_eq!(mesh.blend_shape_weights, vec![0.0, 0.5, 1.0]);
            }
            other => panic!("expected MeshInstance3D node, got {other:?}"),
        }

        let batch = prepared
            .nodes
            .iter()
            .find(|pending| pending.key_name == "Batch")
            .expect("batch node");
        match &batch.node.data {
            SceneNodeData::MultiMeshInstance3D(mesh) => {
                assert_eq!(mesh.blend_shape_weights, vec![0.2, 1.0]);
                assert_eq!(
                    mesh.instances[0].blend_shape_weights.as_deref(),
                    Some([0.7, 0.4].as_slice())
                );
                assert!(mesh.instances[1].blend_shape_weights.is_none());
            }
            other => panic!("expected MultiMeshInstance3D node, got {other:?}"),
        }
    }

    #[test]
    fn scene_loader_parses_mesh_blend_options() {
        let scene = Parser::new(
            r#"
            $root = @Mesh
            [Mesh]
            [MeshInstance3D]
                cast_shadows = false
                receive_shadows = false
                blend = { enabled=true screen_blending=false normal_blending=true blend_layers=[2, 4] blend_mask=[1, 3] distance=0.5 min_distance=0.05 noise=0.25 noise_scale=6.0 }
            [/MeshInstance3D]
            [/Mesh]

            [Batch]
            [MultiMeshInstance3D]
                cast_shadows = false
                receive_shadows = false
                blend_enabled = true
                blend_screen = false
                blend_normals = true
                blend_layers = [5]
                blend_mask = none
                blend_distance = 0.25
            [/MultiMeshInstance3D]
            [/Batch]
            "#,
        )
        .parse_scene();

        let prepared =
            prepare_scene_with_loader(&scene, &|path| Err(format!("unknown scene path `{path}`")))
                .expect("prepare scene");

        let mesh = prepared
            .nodes
            .iter()
            .find(|pending| pending.key_name == "Mesh")
            .expect("mesh node");
        match &mesh.node.data {
            SceneNodeData::MeshInstance3D(mesh) => {
                assert!(!mesh.cast_shadows);
                assert!(!mesh.receive_shadows);
                assert!(mesh.blend.enabled);
                assert!(!mesh.blend.screen_blending);
                assert!(mesh.blend.normal_blending);
                assert_eq!(mesh.blend.blend_layers, BitMask::with([2, 4]));
                assert_eq!(mesh.blend.blend_mask, BitMask::with([1, 3]));
                assert_eq!(mesh.blend.distance, 0.5);
                assert_eq!(mesh.blend.min_distance, 0.05);
                assert_eq!(mesh.blend.noise_factor, 0.25);
                assert_eq!(mesh.blend.noise_scale, 6.0);
            }
            other => panic!("expected MeshInstance3D node, got {other:?}"),
        }

        let batch = prepared
            .nodes
            .iter()
            .find(|pending| pending.key_name == "Batch")
            .expect("batch node");
        match &batch.node.data {
            SceneNodeData::MultiMeshInstance3D(mesh) => {
                assert!(!mesh.cast_shadows);
                assert!(!mesh.receive_shadows);
                assert!(mesh.blend.enabled);
                assert!(!mesh.blend.screen_blending);
                assert!(mesh.blend.normal_blending);
                assert_eq!(mesh.blend.blend_layers, BitMask::with([5]));
                assert_eq!(mesh.blend.blend_mask, BitMask::NONE);
                assert_eq!(mesh.blend.distance, 0.25);
            }
            other => panic!("expected MultiMeshInstance3D node, got {other:?}"),
        }
    }

    #[test]
    fn scene_loader_defaults_mesh_normal_blending_false() {
        let scene = Parser::new(
            r#"
            $root = @Mesh
            [Mesh]
            [MeshInstance3D]
            [/MeshInstance3D]
            [/Mesh]
            "#,
        )
        .parse_scene();

        let prepared =
            prepare_scene_with_loader(&scene, &|path| Err(format!("unknown scene path `{path}`")))
                .expect("prepare scene");

        let mesh = prepared
            .nodes
            .iter()
            .find(|pending| pending.key_name == "Mesh")
            .expect("mesh node");
        match &mesh.node.data {
            SceneNodeData::MeshInstance3D(mesh) => {
                assert!(!mesh.blend.normal_blending);
                assert!(mesh.blend.screen_blending);
            }
            other => panic!("expected MeshInstance3D node, got {other:?}"),
        }
    }

    #[test]
    fn scene_loader_parses_locale_text_markers() {
        let scene = Parser::new(
            r#"
            $root = @label
            [label]
            [UiLabel]
                text = "%loc:\"ui.center\""
            [/UiLabel]
            [/label]

            [box]
            [UiTextBox]
                text = %loc: "ui.entry"
                placeholder = "%loc:\"ui.placeholder\""
            [/UiTextBox]
            [/box]
            "#,
        )
        .parse_scene();

        let prepared =
            prepare_scene_with_loader(&scene, &|path| Err(format!("unknown scene path `{path}`")))
                .expect("prepare scene");

        let label = prepared
            .nodes
            .iter()
            .find(|pending| pending.key_name == "label")
            .expect("label node");
        assert_eq!(label.locale_text_bindings.len(), 1);
        assert_eq!(label.locale_text_bindings[0].key, "ui.center");
        assert_eq!(
            label.locale_text_bindings[0].field,
            crate::runtime::state::LocaleTextField::LabelText
        );
        match &label.node.data {
            SceneNodeData::UiLabel(label) => assert_eq!(label.text.as_ref(), "ui.center"),
            other => panic!("expected UiLabel node, got {other:?}"),
        }

        let text_box = prepared
            .nodes
            .iter()
            .find(|pending| pending.key_name == "box")
            .expect("box node");
        assert_eq!(text_box.locale_text_bindings.len(), 2);
        assert!(
            text_box
                .locale_text_bindings
                .iter()
                .any(|binding| binding.key == "ui.entry"
                    && binding.field == crate::runtime::state::LocaleTextField::TextEditText)
        );
        assert!(
            text_box
                .locale_text_bindings
                .iter()
                .any(|binding| binding.key == "ui.placeholder"
                    && binding.field
                        == crate::runtime::state::LocaleTextField::TextEditPlaceholder)
        );
    }

}
