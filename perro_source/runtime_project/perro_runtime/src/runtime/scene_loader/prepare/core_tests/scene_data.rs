mod scene_data {
    use super::*;

    #[test]
    fn scene_loader_parses_world_label_locale_text_markers() {
        let scene = Parser::new(
            r#"
            [label_2d]
            [Label2D]
                text = "%loc:\"ui.hp\""
            [/Label2D]
            [/label_2d]

            [label_3d]
            [Label3D]
                text = %loc: "ui.name"
                lock_orientation = true
                backface_cull = false
                visible_through_objects = true
                backdrop_color = (0.1, 0.2, 0.3, 1.0)
                corner_radii = (0.1, 0.2, 0.3, 0.4)
                padding = (0.05, 0.1, 0.05, 0.1)
            [/Label3D]
            [/label_3d]
            "#,
        )
        .parse_scene();

        let prepared =
            prepare_scene_with_loader(&scene, &|path| Err(format!("unknown scene path `{path}`")))
                .expect("prepare scene");

        let label_2d = prepared
            .nodes
            .iter()
            .find(|pending| pending.key_name == "label_2d")
            .expect("label2d node");
        assert_eq!(label_2d.locale_text_bindings.len(), 1);
        assert_eq!(label_2d.locale_text_bindings[0].key, "ui.hp");
        match &label_2d.node.data {
            SceneNodeData::Label2D(label) => assert_eq!(label.text.as_ref(), "ui.hp"),
            other => panic!("expected Label2D node, got {other:?}"),
        }

        let label_3d = prepared
            .nodes
            .iter()
            .find(|pending| pending.key_name == "label_3d")
            .expect("label3d node");
        assert_eq!(label_3d.locale_text_bindings.len(), 1);
        assert_eq!(label_3d.locale_text_bindings[0].key, "ui.name");
        match &label_3d.node.data {
            SceneNodeData::Label3D(label) => {
                assert_eq!(label.text.as_ref(), "ui.name");
                assert!(label.lock_orientation);
                assert!(!label.backface_cull);
                assert!(label.visible_through_objects);
                assert_eq!(label.backdrop_color, perro_structs::Color::new(0.1, 0.2, 0.3, 1.0));
                assert_eq!(label.corner_radii.tl, 0.1);
                assert_eq!(label.padding.top, 0.1);
            }
            other => panic!("expected Label3D node, got {other:?}"),
        }
    }

    #[test]
    fn scene_loader_escapes_locale_text_marker_prefix() {
        let scene = Parser::new(
            r#"
            $root = @label
            [label]
            [UiLabel]
                text = "%%loc:not_key"
            [/UiLabel]
            [/label]
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
        assert!(label.locale_text_bindings.is_empty());
        match &label.node.data {
            SceneNodeData::UiLabel(label) => assert_eq!(label.text.as_ref(), "%loc:not_key"),
            other => panic!("expected UiLabel node, got {other:?}"),
        }
    }

    #[test]
    fn scene_loader_builds_animated_sprite_2d_animations() {
        let scene = Parser::new(
            r#"
            $root = @hero
            [hero]
            [AnimatedSprite2D]
                texture = "res://hero.png"
                current_animation = "run"
                current_frame = 1
                fps_scale = 1.5
                animations = [
                    { name = "idle", start = (0, 0), frame_size = (32, 32), frame_count = 4, fps = 8 },
                    { name = "run", start = (0, 32), frame_size = (32, 32), frame_count = 6, fps = 12 }
                ]
            [/AnimatedSprite2D]
            [/hero]
            "#,
        )
        .parse_scene();

        let prepared =
            prepare_scene_with_loader(&scene, &|path| Err(format!("unknown scene path `{path}`")))
                .expect("prepare scene");

        let hero = prepared
            .nodes
            .iter()
            .find(|pending| pending.key_name == "hero")
            .expect("hero node");
        assert_eq!(hero.texture_source.as_deref(), Some("res://hero.png"));
        match &hero.node.data {
            SceneNodeData::AnimatedSprite2D(sprite) => {
                assert_eq!(sprite.current_animation.as_ref(), "run");
                assert_eq!(sprite.current_frame, 1);
                assert_eq!(sprite.fps_scale, 1.5);
                assert_eq!(sprite.animations.len(), 2);
                assert_eq!(sprite.animations[1].name.as_ref(), "run");
                assert_eq!(sprite.animations[1].frame_count, 6);
                assert_eq!(
                    sprite.current_texture_region(),
                    Some([32.0, 32.0, 32.0, 32.0])
                );
            }
            other => panic!("expected AnimatedSprite2D node, got {other:?}"),
        }
    }

    #[test]
    fn scene_loader_builds_joint_body_links() {
        let scene = Parser::new(
            r#"
            [AnchorBody]
                [RigidBody2D/]
            [/AnchorBody]

            [SwingBody]
                [RigidBody2D/]
            [/SwingBody]

            [Link]
                [FixedJoint2D]
                    body_a = @AnchorBody
                    body_b = @SwingBody
                    anchor_a = (0, 0)
                    anchor_b = (0, 1)
                [/FixedJoint2D]
            [/Link]
            "#,
        )
        .parse_scene();

        let prepared = prepare_scene_with_loader(&scene, &|_| unreachable!()).unwrap();
        let link = prepared
            .nodes
            .iter()
            .find(|pending| pending.key_name == "Link")
            .expect("joint node");

        assert_eq!(link.joint_body_links.len(), 2);
        assert!(link.joint_body_links.iter().any(|body| {
            body.field == PendingJointBodyField::BodyA
                && scene.key_name(SceneKey::new(body.target_key)) == Some("AnchorBody")
        }));
        assert!(link.joint_body_links.iter().any(|body| {
            body.field == PendingJointBodyField::BodyB
                && scene.key_name(SceneKey::new(body.target_key)) == Some("SwingBody")
        }));
    }

    #[test]
    fn scene_loader_builds_rigid_body_gravity_scale() {
        let scene = Parser::new(
            r#"
            [Body2D]
                [RigidBody2D]
                    gravity_scale = 0.5
                [/RigidBody2D]
            [/Body2D]

            [Body3D]
                [RigidBody3D]
                    gravity_scale = 0.25
                [/RigidBody3D]
            [/Body3D]
            "#,
        )
        .parse_scene();

        let prepared = prepare_scene_with_loader(&scene, &|_| unreachable!()).unwrap();
        let body_2d = prepared
            .nodes
            .iter()
            .find(|pending| pending.key_name == "Body2D")
            .expect("body2d node");
        let body_3d = prepared
            .nodes
            .iter()
            .find(|pending| pending.key_name == "Body3D")
            .expect("body3d node");

        match &body_2d.node.data {
            SceneNodeData::RigidBody2D(body) => assert_eq!(body.gravity_scale, 0.5),
            other => panic!("expected RigidBody2D node, got {other:?}"),
        }
        match &body_3d.node.data {
            SceneNodeData::RigidBody3D(body) => assert_eq!(body.gravity_scale, 0.25),
            other => panic!("expected RigidBody3D node, got {other:?}"),
        }
    }

    #[test]
    fn scene_loader_builds_audio_effect_zone_effect_fields() {
        let scene = Parser::new(
            r#"
            [zone2d]
            [AudioEffectZone2D]
                active = false
                audio_mask = [2, 4]
                bounce = true
                effects = [
                    {
                        reverb_send: 0.8,
                        echo: 0.4,
                        dampening: 0.2
                    },
                    {
                        reverb_send: 0.3,
                        echo: 0.1,
                        dampening: 0.7
                    }
                ]
            [/AudioEffectZone2D]
            [/zone2d]

            [zone3d]
            [AudioEffectZone3D]
                reverb = 0.6
                echo = 0.3
                low_pass = 0.5
                bounce = true
            [/AudioEffectZone3D]
            [/zone3d]
            "#,
        )
        .parse_scene();

        let prepared =
            prepare_scene_with_loader(&scene, &|_| Err("unexpected root_of".to_string()))
                .expect("prepare scene");
        let zone2d = prepared
            .nodes
            .iter()
            .find(|node| node.key_name == "zone2d")
            .expect("zone2d");
        let SceneNodeData::AudioEffectZone2D(zone2d) = &zone2d.node.data else {
            panic!("expected AudioEffectZone2D");
        };
        assert!(!zone2d.active);
        assert_eq!(zone2d.audio_mask.bits(), 0b1010);
        assert!(zone2d.bounce);
        assert_eq!(zone2d.effects.len(), 2);
        assert_eq!(zone2d.effects[0].reverb_send, 0.8);
        assert_eq!(zone2d.effects[0].echo, 0.4);
        assert_eq!(zone2d.effects[0].dampening, 0.2);
        assert_eq!(zone2d.effects[1].reverb_send, 0.3);
        assert_eq!(zone2d.effects[1].echo, 0.1);
        assert_eq!(zone2d.effects[1].dampening, 0.7);

        let zone3d = prepared
            .nodes
            .iter()
            .find(|node| node.key_name == "zone3d")
            .expect("zone3d");
        let SceneNodeData::AudioEffectZone3D(zone3d) = &zone3d.node.data else {
            panic!("expected AudioEffectZone3D");
        };
        assert!(zone3d.bounce);
        assert_eq!(zone3d.effects[0].reverb_send, 0.6);
        assert_eq!(zone3d.effects[0].echo, 0.3);
        assert_eq!(zone3d.effects[0].dampening, 0.5);
    }

    #[test]
    fn scene_loader_builds_audio_portal_link_fields() {
        let scene = Parser::new(
            r#"
            [mask2d]
            [AudioMask2D]
                enabled = false
            [/AudioMask2D]
            [/mask2d]

            [portal2d]
            [AudioPortal2D]
                enabled = false
                strength = 0.55
                targets = [8, 9, 10]
            [/AudioPortal2D]
            [/portal2d]

            [portal3d]
            [AudioPortal3D]
                strength = 0.75
                connections = [11, 12]
            [/AudioPortal3D]
            [/portal3d]
            "#,
        )
        .parse_scene();

        let prepared =
            prepare_scene_with_loader(&scene, &|_| Err("unexpected root_of".to_string()))
                .expect("prepare scene");
        let mask2d = prepared
            .nodes
            .iter()
            .find(|node| node.key_name == "mask2d")
            .expect("mask2d");
        let SceneNodeData::AudioMask2D(mask2d) = &mask2d.node.data else {
            panic!("expected AudioMask2D");
        };
        assert!(!mask2d.active);

        let portal2d = prepared
            .nodes
            .iter()
            .find(|node| node.key_name == "portal2d")
            .expect("portal2d");
        let SceneNodeData::AudioPortal2D(portal2d) = &portal2d.node.data else {
            panic!("expected AudioPortal2D");
        };
        assert!(!portal2d.active);
        assert_eq!(portal2d.strength, 0.55);
        assert_eq!(
            portal2d.targets,
            vec![
                NodeID::from_u32(8),
                NodeID::from_u32(9),
                NodeID::from_u32(10)
            ]
        );

        let portal3d = prepared
            .nodes
            .iter()
            .find(|node| node.key_name == "portal3d")
            .expect("portal3d");
        let SceneNodeData::AudioPortal3D(portal3d) = &portal3d.node.data else {
            panic!("expected AudioPortal3D");
        };
        assert_eq!(portal3d.strength, 0.75);
        assert_eq!(
            portal3d.targets,
            vec![NodeID::from_u32(11), NodeID::from_u32(12)]
        );
    }

    #[test]
    fn scene_loader_builds_chroma_keys_from_hex_and_tuple() {
        let scene = Parser::new(
            r##"
            [cam]
            [Camera3D]
                post_processing = [
                    { type = "chroma_key", color = "#00FF00", tolerance = 0.2, softness = 0.03 },
                    { type = "chroma_key", color = (1.0, 0.0, 1.0) }
                ]
            [/Camera3D]
            [/cam]
            "##,
        )
        .parse_scene();

        let prepared =
            prepare_scene_with_loader(&scene, &|_| Err("unexpected root_of".to_string()))
                .expect("prepare scene");
        let cam = prepared
            .nodes
            .iter()
            .find(|node| node.key_name == "cam")
            .expect("cam");
        let SceneNodeData::Camera3D(cam) = &cam.node.data else {
            panic!("expected Camera3D");
        };
        let effects = cam.post_processing.to_effects_vec();
        assert_eq!(effects.len(), 2);
        assert!(matches!(
            &effects[0],
            PostProcessEffect::ChromaKey { color, tolerance, softness }
                if *color == Color::GREEN && *tolerance == 0.2 && *softness == 0.03
        ));
        assert!(matches!(
            &effects[1],
            PostProcessEffect::ChromaKey { color, tolerance, softness }
                if *color == Color::MAGENTA && *tolerance == 0.1 && *softness == 0.05
        ));
    }

    #[test]
    fn scene_loader_builds_color_grade_and_luts() {
        let scene = Parser::new(
            r#"
            [cam]
            [Camera3D]
                active = true
                post_processing = [
                    {
                        type = "exposure",
                        auto_exposure = true,
                        exposure = -0.5,
                        min_exposure = -3.0,
                        max_exposure = 4.0,
                        speed_up = 5.0,
                        speed_down = 2.0,
                        target_luminance = 0.2
                    },
                    {
                        type = "color_grade",
                        exposure = 0.25,
                        contrast = 1.1,
                        brightness = 0.02,
                        saturation = 1.2,
                        gamma = 0.95,
                        temperature = 0.1,
                        tint = -0.05,
                        hue_shift = 0.2,
                        vibrance = 0.3,
                        lift = (0.01, 0.02, 0.03),
                        gain = (1.1, 1.0, 0.9),
                        offset = (-0.01, 0.0, 0.01)
                    },
                    { type = "lut2d", texture = "res://luts/film_32.png", lut_size = 32, strength = 0.75 },
                    { type = "lut3d", texture = "res://luts/print_32.png", lut_size = 16, strength = 1.0 }
                ]
            [/Camera3D]
            [/cam]
            "#,
        )
        .parse_scene();

        let prepared =
            prepare_scene_with_loader(&scene, &|_| Err("unexpected root_of".to_string()))
                .expect("prepare scene");
        let cam = prepared
            .nodes
            .iter()
            .find(|node| node.key_name == "cam")
            .expect("cam");
        let SceneNodeData::Camera3D(cam) = &cam.node.data else {
            panic!("expected Camera3D");
        };
        let effects = cam.post_processing.to_effects_vec();
        assert_eq!(effects.len(), 4);
        match &effects[0] {
            PostProcessEffect::Exposure {
                exposure,
                auto_exposure,
                min_exposure,
                max_exposure,
                speed_up,
                speed_down,
                target_luminance,
            } => {
                assert_eq!(*exposure, -0.5);
                assert!(*auto_exposure);
                assert_eq!((*min_exposure, *max_exposure), (-3.0, 4.0));
                assert_eq!((*speed_up, *speed_down), (5.0, 2.0));
                assert_eq!(*target_luminance, 0.2);
            }
            other => panic!("expected exposure, got {other:?}"),
        }
        match &effects[1] {
            PostProcessEffect::ColorGrade {
                exposure,
                gain,
                offset,
                ..
            } => {
                assert_eq!(*exposure, 0.25);
                assert_eq!(*gain, [1.1, 1.0, 0.9]);
                assert_eq!(*offset, [-0.01, 0.0, 0.01]);
            }
            other => panic!("expected color grade, got {other:?}"),
        }
        match &effects[2] {
            PostProcessEffect::Lut2D {
                texture_path,
                size,
                strength,
            } => {
                assert_eq!(texture_path.as_ref(), "res://luts/film_32.png");
                assert_eq!(*size, 32);
                assert_eq!(*strength, 0.75);
            }
            other => panic!("expected lut2d, got {other:?}"),
        }
        match &effects[3] {
            PostProcessEffect::Lut3D {
                texture_path,
                size,
                strength,
            } => {
                assert_eq!(texture_path.as_ref(), "res://luts/print_32.png");
                assert_eq!(*size, 16);
                assert_eq!(*strength, 1.0);
            }
            other => panic!("expected lut3d, got {other:?}"),
        }
    }

    #[test]
    fn scene_loader_builds_camera_audio_options() {
        let scene = Parser::new(
            r#"
            [cam2d]
            [Camera2D]
                audio_options = {
                    audio_mask = [1, 3],
                    effects = [
                        { reverb_send: 0.6, echo: 0.2, dampening: 0.4 }
                    ]
                }
            [/Camera2D]
            [/cam2d]

            [cam3d]
            [Camera3D]
                audio_mask = [2]
                reverb_send = 0.7
                echo = 0.1
                dampening = 0.5
            [/Camera3D]
            [/cam3d]
            "#,
        )
        .parse_scene();

        let prepared =
            prepare_scene_with_loader(&scene, &|_| Err("unexpected root_of".to_string()))
                .expect("prepare scene");
        let cam2d = prepared
            .nodes
            .iter()
            .find(|node| node.key_name == "cam2d")
            .expect("cam2d");
        let SceneNodeData::Camera2D(cam2d) = &cam2d.node.data else {
            panic!("expected Camera2D");
        };
        assert_eq!(cam2d.audio_options.audio_mask.bits(), 0b101);
        assert_eq!(cam2d.audio_options.effects.len(), 1);
        assert_eq!(cam2d.audio_options.effects[0].reverb_send, 0.6);
        assert_eq!(cam2d.audio_options.effects[0].echo, 0.2);
        assert_eq!(cam2d.audio_options.effects[0].dampening, 0.4);

        let cam3d = prepared
            .nodes
            .iter()
            .find(|node| node.key_name == "cam3d")
            .expect("cam3d");
        let SceneNodeData::Camera3D(cam3d) = &cam3d.node.data else {
            panic!("expected Camera3D");
        };
        assert_eq!(cam3d.audio_options.audio_mask.bits(), 0b10);
        assert_eq!(cam3d.audio_options.effects.len(), 1);
        assert_eq!(cam3d.audio_options.effects[0].reverb_send, 0.7);
        assert_eq!(cam3d.audio_options.effects[0].echo, 0.1);
        assert_eq!(cam3d.audio_options.effects[0].dampening, 0.5);
    }

    #[test]
    fn scene_loader_builds_ui_image_button_fields() {
        let scene = Parser::new(
            r##"
            $root = @icon
            [icon]
            [UiImageButton]
                texture = "res://ui/play.png"
                size_ratio = (0.08, 0.12)
                scale_mode = "fit"
                tint = "#11223344"
                hover_tint = "#55667788"
                pressed_tint = "#99AABBCC"
                texture_region = (1, 2, 16, 32)
                clicked_signals = ["play_clicked"]
                hover = { scale = (1.1, 1.1), tint = "#FFFFFFFF" }
                pressed = { scale = (0.9, 0.9), tint = "#CCCCCCFF" }
            [/UiImageButton]
            [/icon]
            "##,
        )
        .parse_scene();

        let prepared =
            prepare_scene_with_loader(&scene, &|path| Err(format!("unknown scene path `{path}`")))
                .expect("prepare scene");
        let icon = prepared
            .nodes
            .iter()
            .find(|pending| pending.key_name == "icon")
            .expect("icon node");
        assert_eq!(icon.texture_source.as_deref(), Some("res://ui/play.png"));
        match &icon.node.data {
            SceneNodeData::UiImageButton(button) => {
                assert_eq!(button.layout.size, perro_ui::UiVector2::ratio(0.08, 0.12));
                assert_eq!(button.scale_mode, perro_ui::UiImageScaleMode::Fit);
                assert_eq!(
                    button.tint,
                    Color::new(0.06666667, 0.13333334, 0.2, 0.26666668)
                );
                assert_eq!(button.hover_tint, Color::WHITE);
                assert_eq!(button.pressed_tint, Color::new(0.8, 0.8, 0.8, 1.0));
                assert_eq!(button.texture_region, Some([1.0, 2.0, 16.0, 32.0]));
                assert_eq!(
                    button.clicked_signals,
                    vec![SignalID::from_string("play_clicked")]
                );
                assert!(button.hover_base.is_some());
                assert!(button.pressed_base.is_some());
            }
            other => panic!("expected UiImageButton node, got {other:?}"),
        }
    }

    #[test]
    fn scene_loader_builds_ui_shape_fields() {
        let scene = Parser::new(
            r##"
            $root = @shape
            [shape]
            [UiShape]
                shape = "triangle"
                fill = "#336699FF"
                stroke = "#CCDDEEFF"
                stroke_width = 2.5
            [/UiShape]
            [/shape]
            "##,
        )
        .parse_scene();

        let prepared =
            prepare_scene_with_loader(&scene, &|path| Err(format!("unknown scene path `{path}`")))
                .expect("prepare scene");
        let shape = prepared
            .nodes
            .iter()
            .find(|pending| pending.key_name == "shape")
            .expect("shape node");
        match &shape.node.data {
            SceneNodeData::UiShape(shape) => {
                assert_eq!(shape.kind, perro_ui::UiShapeKind::Triangle);
                assert_eq!(shape.fill, Color::new(0.2, 0.4, 0.6, 1.0));
                assert_eq!(shape.stroke, Color::new(0.8, 0.8666667, 0.93333334, 1.0));
                assert_eq!(shape.stroke_width, 2.5);
            }
            other => panic!("expected UiShape node, got {other:?}"),
        }
    }

    #[test]
    fn scene_loader_builds_2d_button_fields() {
        let scene = Parser::new(
            r##"
            $root = @play
            [play]
            [Button2D]
                position = (12, 34)
                size = (96, 40)
                fill = "#112233FF"
                hover_fill = "#445566FF"
                pressed_fill = "#778899FF"
                disabled = true
                clicked_signals = ["play_clicked"]
            [/Button2D]
            [/play]

            [icon]
            [ImageButton2D]
                texture = "res://ui/icon.png"
                size = (24, 18)
                tint = "#FFFFFFFF"
                hover_tint = "#CCCCCCFF"
                pressed_tint = "#999999FF"
                texture_region = (1, 2, 8, 9)
                input_enabled = false
            [/ImageButton2D]
            [/icon]
            "##,
        )
        .parse_scene();

        let prepared =
            prepare_scene_with_loader(&scene, &|path| Err(format!("unknown scene path `{path}`")))
                .expect("prepare scene");
        let play = prepared
            .nodes
            .iter()
            .find(|pending| pending.key_name == "play")
            .expect("play node");
        match &play.node.data {
            SceneNodeData::Button2D(button) => {
                assert_eq!(button.transform.position, Vector2::new(12.0, 34.0));
                assert_eq!(button.size, Vector2::new(96.0, 40.0));
                assert_eq!(
                    button.style.fill,
                    Color::new(0.06666667, 0.13333334, 0.2, 1.0)
                );
                assert_eq!(
                    button.hover_style.fill,
                    Color::new(0.26666668, 0.33333334, 0.4, 1.0)
                );
                assert_eq!(
                    button.pressed_style.fill,
                    Color::new(0.46666667, 0.53333336, 0.6, 1.0)
                );
                assert_eq!(
                    button.clicked_signals,
                    vec![SignalID::from_string("play_clicked")]
                );
                assert!(!button.input_enabled);
            }
            other => panic!("expected Button2D node, got {other:?}"),
        }

        let icon = prepared
            .nodes
            .iter()
            .find(|pending| pending.key_name == "icon")
            .expect("icon node");
        assert_eq!(icon.texture_source.as_deref(), Some("res://ui/icon.png"));
        match &icon.node.data {
            SceneNodeData::ImageButton2D(button) => {
                assert_eq!(button.size, Vector2::new(24.0, 18.0));
                assert_eq!(button.hover_tint, Color::new(0.8, 0.8, 0.8, 1.0));
                assert_eq!(button.pressed_tint, Color::new(0.6, 0.6, 0.6, 1.0));
                assert_eq!(button.texture_region, Some([1.0, 2.0, 8.0, 9.0]));
                assert!(!button.input_enabled);
            }
            other => panic!("expected ImageButton2D node, got {other:?}"),
        }
    }

    #[test]
    fn scene_loader_builds_nine_slice_fields() {
        let scene = Parser::new(
            r##"
            $root = @panel
            [panel]
            [UiNineSlice]
                texture = "res://ui/panel.png"
                size_ratio = (0.3, 0.2)
                margins = (4, 5, 6, 7)
                texture_region = (1, 2, 30, 20)
                tint = "#FFFFFFFF"
            [/UiNineSlice]
            [/panel]

            [world]
            [NineSlice2D]
                texture = "res://ui/world_panel.png"
                size = (90, 30)
                margins = (3, 4, 5, 6)
                texture_region = (2, 4, 20, 10)
            [/NineSlice2D]
            [/world]
            "##,
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
        assert_eq!(panel.texture_source.as_deref(), Some("res://ui/panel.png"));
        match &panel.node.data {
            SceneNodeData::UiNineSlice(node) => {
                assert_eq!(node.layout.size, perro_ui::UiVector2::ratio(0.3, 0.2));
                assert_eq!(node.margins, [4.0, 5.0, 6.0, 7.0]);
                assert_eq!(node.texture_region, Some([1.0, 2.0, 30.0, 20.0]));
            }
            other => panic!("expected UiNineSlice node, got {other:?}"),
        }

        let world = prepared
            .nodes
            .iter()
            .find(|pending| pending.key_name == "world")
            .expect("world node");
        assert_eq!(
            world.texture_source.as_deref(),
            Some("res://ui/world_panel.png")
        );
        match &world.node.data {
            SceneNodeData::NineSlice2D(node) => {
                assert_eq!(node.size, Vector2::new(90.0, 30.0));
                assert_eq!(node.margins, [3.0, 4.0, 5.0, 6.0]);
                assert_eq!(node.texture_region, Some([2.0, 4.0, 20.0, 10.0]));
            }
            other => panic!("expected NineSlice2D node, got {other:?}"),
        }
    }

    #[test]
    fn scene_loader_accepts_layer_arrays_for_bitmasks() {
        let scene = Parser::new(
            r#"
            $root = @sprite
            [sprite]
            [Sprite2D]
                render_layers = [1, 3]
            [/Sprite2D]
            [/sprite]

            [body]
            [StaticBody2D]
                collision_layers = [2, 4]
                collision_mask = [1, 3]
            [/StaticBody2D]
            [/body]

            [camera]
            [Camera3D]
                render_mask = [2, 5]
            [/Camera3D]
            [/camera]

            [area]
            [Area3D]
                collision_layers = [5]
                collision_mask = [1, 2]
            [/Area3D]
            [/area]
            "#,
        )
        .parse_scene();

        let prepared =
            prepare_scene_with_loader(&scene, &|path| Err(format!("unknown scene path `{path}`")))
                .expect("prepare scene");

        let sprite = prepared
            .nodes
            .iter()
            .find(|pending| pending.key_name == "sprite")
            .expect("sprite node");
        match &sprite.node.data {
            SceneNodeData::Sprite2D(node) => assert_eq!(node.render_layers.bits(), 0b101),
            other => panic!("expected Sprite2D node, got {other:?}"),
        }

        let body = prepared
            .nodes
            .iter()
            .find(|pending| pending.key_name == "body")
            .expect("body node");
        match &body.node.data {
            SceneNodeData::StaticBody2D(node) => {
                assert_eq!(node.collision_layers.bits(), 0b1010);
                assert_eq!(node.collision_mask.bits(), 0b101);
            }
            other => panic!("expected StaticBody2D node, got {other:?}"),
        }

        let camera = prepared
            .nodes
            .iter()
            .find(|pending| pending.key_name == "camera")
            .expect("camera node");
        match &camera.node.data {
            SceneNodeData::Camera3D(node) => assert_eq!(node.render_mask.bits(), 0b10010),
            other => panic!("expected Camera3D node, got {other:?}"),
        }

        let area = prepared
            .nodes
            .iter()
            .find(|pending| pending.key_name == "area")
            .expect("area node");
        match &area.node.data {
            SceneNodeData::Area3D(node) => {
                assert_eq!(node.collision_layers.bits(), 0b10000);
                assert_eq!(node.collision_mask.bits(), 0b11);
            }
            other => panic!("expected Area3D node, got {other:?}"),
        }
    }

    #[test]
    fn scene_loader_accepts_bitmask_only_and_without_calls() {
        let scene = Parser::new(
            r#"
            $root = @sprite
            [sprite]
            [Sprite2D]
                render_layers = only(1, 3)
            [/Sprite2D]
            [/sprite]

            [camera]
            [Camera3D]
                render_mask = without(1)
            [/Camera3D]
            [/camera]

            [body]
            [StaticBody2D]
                collision_layers = without([1, 32])
                collision_mask = only([2, 4])
            [/StaticBody2D]
            [/body]
            "#,
        )
        .parse_scene();

        let prepared =
            prepare_scene_with_loader(&scene, &|path| Err(format!("unknown scene path `{path}`")))
                .expect("prepare scene");

        let sprite = prepared
            .nodes
            .iter()
            .find(|pending| pending.key_name == "sprite")
            .expect("sprite node");
        match &sprite.node.data {
            SceneNodeData::Sprite2D(node) => assert_eq!(node.render_layers.bits(), 0b101),
            other => panic!("expected Sprite2D node, got {other:?}"),
        }

        let camera = prepared
            .nodes
            .iter()
            .find(|pending| pending.key_name == "camera")
            .expect("camera node");
        match &camera.node.data {
            SceneNodeData::Camera3D(node) => {
                assert_eq!(node.render_mask.bits(), !0b1);
            }
            other => panic!("expected Camera3D node, got {other:?}"),
        }

        let body = prepared
            .nodes
            .iter()
            .find(|pending| pending.key_name == "body")
            .expect("body node");
        match &body.node.data {
            SceneNodeData::StaticBody2D(node) => {
                assert_eq!(node.collision_layers.bits(), !0b1 & !(1u32 << 31));
                assert_eq!(node.collision_mask.bits(), 0b1010);
            }
            other => panic!("expected StaticBody2D node, got {other:?}"),
        }
    }

    #[test]
    fn ui_viewport_scene_fields_parse_without_camera_ref() {
        let scene = Parser::new(
            r#"
            $root = @preview
            [preview]
            [UiViewport]
                resolution = (640, 360)
                view_position = (1, 2, 5)
                view_rotation = (0, 0, 0, 1)
                projection = "orthographic"
                orthographic_size = 4
                view_2d_position = (8, 9)
                view_2d_zoom = 2
                background = (0.1, 0.2, 0.3, 0.4)
                corner_radius = 0.2
                suspend_when_hidden = false
            [/UiViewport]
            [/preview]
            "#,
        )
        .parse_scene();

        let prepared =
            prepare_scene_with_loader(&scene, &|path| Err(format!("unknown scene path `{path}`")))
                .expect("prepare scene");
        let viewport = prepared
            .nodes
            .iter()
            .find(|pending| pending.key_name == "preview")
            .expect("viewport node");
        let SceneNodeData::UiViewport(viewport) = &viewport.node.data else {
            panic!("expected UiViewport node");
        };
        assert_eq!(viewport.resolution, UVector2::new(640, 360));
        assert_eq!(viewport.view_position, Vector3::new(1.0, 2.0, 5.0));
        assert_eq!(viewport.view_2d_position, Vector2::new(8.0, 9.0));
        assert_eq!(viewport.view_2d_zoom, 2.0);
        assert_eq!(viewport.background, Color::new(0.1, 0.2, 0.3, 0.4));
        assert_eq!(viewport.corner_radius, 0.2);
        assert!(!viewport.suspend_when_hidden);
        assert!(matches!(
            viewport.projection,
            CameraProjection::Orthographic { size, .. } if size == 4.0
        ));
    }

}
