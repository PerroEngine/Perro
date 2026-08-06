mod assets {
    use super::*;

    #[test]
    fn root_of_without_host_type_block_inherits_template_root_data() {
        let host = Parser::new(
            r#"
            $root = @host
            [host]
            root_of = "res://base.scn"
            [/host]
            "#,
        )
        .parse_scene();

        let base = Parser::new(
            r#"
            $root = @base_root
            [base_root]
            [Node2D]
                position = (7, 8)
                rotation = 1.25
            [/Node2D]
            [/base_root]
            "#,
        )
        .parse_scene();

        let prepared = prepare_scene_with_loader(&host, &|path| match path {
            "res://base.scn" => Ok(std::sync::Arc::new(base.clone())),
            _ => Err(format!("unknown scene path `{path}`")),
        })
        .expect("prepare scene");

        let host_node = prepared
            .nodes
            .iter()
            .find(|pending| pending.key_name == "host")
            .expect("host node");
        match &host_node.node.data {
            SceneNodeData::Node2D(node_2d) => {
                assert_eq!(node_2d.position.x, 7.0);
                assert_eq!(node_2d.position.y, 8.0);
                assert_eq!(node_2d.rotation, 1.25);
            }
            other => panic!("expected inherited Node2D host node, got {other:?}"),
        }
    }

    #[test]
    fn scene_loader_accepts_rotation_deg_for_spatial_nodes() {
        let scene = Parser::new(
            r#"
            $root = @scene_root
            [scene_root]
            [Node2D]
                rotation_deg = 90
            [/Node2D]
            [/scene_root]

            [camera]
            [Camera3D]
                rotation_deg = (0, 90, 0)
            [/Camera3D]
            [/camera]
            "#,
        )
        .parse_scene();

        let prepared =
            prepare_scene_with_loader(&scene, &|path| Err(format!("unknown scene path `{path}`")))
                .expect("prepare scene");

        let root = prepared
            .nodes
            .iter()
            .find(|pending| pending.key_name == "scene_root")
            .expect("root node");
        match &root.node.data {
            SceneNodeData::Node2D(node) => {
                assert!((node.rotation - std::f32::consts::FRAC_PI_2).abs() < 1e-5);
            }
            other => panic!("expected Node2D root node, got {other:?}"),
        }

        let camera = prepared
            .nodes
            .iter()
            .find(|pending| pending.key_name == "camera")
            .expect("camera node");
        match &camera.node.data {
            SceneNodeData::Camera3D(node) => {
                assert!(
                    (node.transform.rotation.y.abs() - std::f32::consts::FRAC_1_SQRT_2).abs()
                        < 1e-5
                );
            }
            other => panic!("expected Camera3D node, got {other:?}"),
        }
    }

    #[test]
    fn scene_loader_button_state_style_inherits_base_fields() {
        let scene = Parser::new(
            r##"
            $root = @button
            [button]
            [UiButton]
                style = {
                    fill = "#101820"
                    stroke = "#A0A8B0"
                    radius = 1.0
                    shadow = { color = "#00000066" distance = 8 falloff = 12 vector = (1, -1) size = 1.5 }
                    highlight = { color = "#FFFFFF55" distance = 2 falloff = 4 vector = (-1, 1) size = 1.0 }
                }
                hover_fill = "#202830"
                pressed = {
                    style = { fill = "#303840" }
                }
            [/UiButton]
            [/button]
            "##,
        )
        .parse_scene();

        let prepared =
            prepare_scene_with_loader(&scene, &|path| Err(format!("unknown scene path `{path}`")))
                .expect("prepare scene");

        let node = prepared
            .nodes
            .iter()
            .find(|pending| pending.key_name == "button")
            .expect("button node");
        match &node.node.data {
            SceneNodeData::UiButton(button) => {
                assert_eq!(button.style.corner_radius(), 1.0);
                assert_eq!(
                    button.style.outer_shadow.color,
                    Color::from_hex("#00000066").expect("test or bench setup must succeed")
                );
                assert_eq!(button.style.outer_shadow.distance, 8.0);
                assert_eq!(button.style.outer_shadow.falloff, 12.0);
                assert_eq!(button.style.outer_shadow.vector, Vector2::new(1.0, -1.0));
                assert_eq!(button.style.outer_shadow.size, 1.5);
                assert_eq!(
                    button.style.inner_highlight.color,
                    Color::from_hex("#FFFFFF55").expect("test or bench setup must succeed")
                );
                assert_eq!(button.style.inner_highlight.distance, 2.0);
                assert_eq!(button.style.inner_highlight.falloff, 4.0);
                assert_eq!(button.style.inner_highlight.vector, Vector2::new(-1.0, 1.0));
                assert_eq!(button.style.inner_highlight.size, 1.0);
                assert_eq!(button.hover_style.fill, Color::from_hex("#202830").expect("test or bench setup must succeed"));
                assert_eq!(button.hover_style.stroke, button.style.stroke);
                assert_eq!(button.hover_style.corner_radius(), 1.0);
                assert_eq!(
                    button.pressed_style.fill,
                    Color::from_hex("#303840").expect("test or bench setup must succeed")
                );
                assert_eq!(button.pressed_style.stroke, button.style.stroke);
                assert_eq!(button.pressed_style.corner_radius(), 1.0);
            }
            other => panic!("expected UiButton node, got {other:?}"),
        }
    }

    #[test]
    fn scene_loader_ui_input_masks_apply_to_button_and_text_edit() {
        let scene = Parser::new(
            r#"
            $root = @button
            [button]
            [UiButton]
                input_only_players = [0, 2]
                input_block_gamepads = [1]
                input_allow_kbm = true
            [/UiButton]
            [/button]

            [field]
            [UiTextBox]
                input_only_joycons = [3]
                input_block_players = [4]
                input_deny_kbm = true
                h_align = "center"
                v_align = "end"
            [/UiTextBox]
            [/field]
            "#,
        )
        .parse_scene();

        let prepared =
            prepare_scene_with_loader(&scene, &|path| Err(format!("unknown scene path `{path}`")))
                .expect("prepare scene");

        let button = prepared
            .nodes
            .iter()
            .find(|pending| pending.key_name == "button")
            .expect("button node");
        match &button.node.data {
            SceneNodeData::UiButton(button) => {
                assert_eq!(button.input_mask.allow_players, vec![0, 2]);
                assert_eq!(button.input_mask.deny_gamepads, vec![1]);
                assert!(button.input_mask.allow_kbm);
            }
            other => panic!("expected UiButton node, got {other:?}"),
        }

        let field = prepared
            .nodes
            .iter()
            .find(|pending| pending.key_name == "field")
            .expect("field node");
        match &field.node.data {
            SceneNodeData::UiTextBox(text_box) => {
                assert_eq!(text_box.inner.input_mask.allow_joycons, vec![3]);
                assert_eq!(text_box.inner.input_mask.deny_players, vec![4]);
                assert!(text_box.inner.input_mask.deny_kbm);
                assert_eq!(text_box.inner.h_align, perro_ui::UiTextAlign::Center);
                assert_eq!(text_box.inner.v_align, perro_ui::UiTextAlign::End);
            }
            other => panic!("expected UiTextBox node, got {other:?}"),
        }
    }

    #[test]
    fn scene_loader_button_web_href_parses() {
        let scene = Parser::new(
            r#"
            $root = @button
            [button]
            [UiButton]
                web = { href = "docs/" }
            [/UiButton]
            [/button]
            "#,
        )
        .parse_scene();

        let prepared =
            prepare_scene_with_loader(&scene, &|path| Err(format!("unknown scene path `{path}`")))
                .expect("prepare scene");

        let button = prepared
            .nodes
            .iter()
            .find(|pending| pending.key_name == "button")
            .expect("button node");
        match &button.node.data {
            SceneNodeData::UiButton(button) => {
                let web = button.web.as_ref().expect("web config");
                assert_eq!(web.href.as_ref(), "/docs");
            }
            other => panic!("expected UiButton node, got {other:?}"),
        }
    }

    #[test]
    fn scene_loader_ui_style_resource_applies_base_state_and_focus() {
        static BASE: perro_ui::UiStyle = perro_ui::UiStyle {
            fill: Color::new(0.10, 0.20, 0.30, 1.0),
            stroke: Color::new(0.40, 0.50, 0.60, 1.0),
            stroke_width: 2.0,
            corner_radii: perro_ui::UiCornerRadii::all(0.4),
            ..perro_ui::UiStyle::panel()
        };
        static HOVER: perro_ui::UiStyle = perro_ui::UiStyle {
            fill: Color::new(0.20, 0.30, 0.40, 1.0),
            stroke: Color::new(0.50, 0.60, 0.70, 1.0),
            stroke_width: 3.0,
            corner_radii: perro_ui::UiCornerRadii::all(0.5),
            ..perro_ui::UiStyle::panel()
        };
        static PRESSED: perro_ui::UiStyle = perro_ui::UiStyle {
            fill: Color::new(0.05, 0.10, 0.15, 1.0),
            stroke: Color::new(0.30, 0.40, 0.50, 1.0),
            stroke_width: 4.0,
            corner_radii: perro_ui::UiCornerRadii::all(0.6),
            ..perro_ui::UiStyle::panel()
        };
        static FOCUS: perro_ui::UiStyle = perro_ui::UiStyle {
            fill: Color::new(0.70, 0.80, 0.90, 1.0),
            stroke: Color::new(0.10, 0.20, 0.30, 1.0),
            stroke_width: 5.0,
            corner_radii: perro_ui::UiCornerRadii::all(0.7),
            ..perro_ui::UiStyle::panel()
        };
        static EMPTY: perro_ui::UiStyle = perro_ui::UiStyle::panel();

        fn lookup(path_hash: u64) -> &'static perro_ui::UiStyle {
            match path_hash {
                hash if hash == perro_ids::string_to_u64("res://ui/base.uistyle") => &BASE,
                hash if hash == perro_ids::string_to_u64("res://ui/hover.uistyle") => &HOVER,
                hash if hash == perro_ids::string_to_u64("res://ui/pressed.uistyle") => &PRESSED,
                hash if hash == perro_ids::string_to_u64("res://ui/focus.uistyle") => &FOCUS,
                _ => &EMPTY,
            }
        }

        let scene = Parser::new(
            r#"
            $root = @button
            [button]
            [UiButton]
                style = "res://ui/base.uistyle"
                hover = { style = "res://ui/hover.uistyle" }
                pressed = { style = "res://ui/pressed.uistyle" }
            [/UiButton]
            [/button]

            [input]
            parent = button
            [UiTextBox]
                style = "res://ui/base.uistyle"
                focused_style = "res://ui/focus.uistyle"
            [/UiTextBox]
            [/input]
            "#,
        )
        .parse_scene();

        let prepared = prepare_scene_with_loader_and_styles(
            &scene,
            &|path| Err(format!("unknown scene path `{path}`")),
            Some(lookup),
        )
        .expect("prepare scene");

        let button = prepared
            .nodes
            .iter()
            .find(|pending| pending.key_name == "button")
            .expect("button node");
        match &button.node.data {
            SceneNodeData::UiButton(button) => {
                assert_eq!(button.style.fill, BASE.fill);
                assert_eq!(button.hover_style.fill, HOVER.fill);
                assert_eq!(button.pressed_style.fill, PRESSED.fill);
            }
            other => panic!("expected UiButton node, got {other:?}"),
        }

        let input = prepared
            .nodes
            .iter()
            .find(|pending| pending.key_name == "input")
            .expect("input node");
        match &input.node.data {
            SceneNodeData::UiTextBox(text_box) => {
                assert_eq!(text_box.style.fill, BASE.fill);
                assert_eq!(text_box.focused_style.fill, FOCUS.fill);
            }
            other => panic!("expected UiTextBox node, got {other:?}"),
        }
    }

    #[test]
    fn scene_loader_ui_style_parses_gradient_and_split_depth_fields() {
        let scene = Parser::new(
            r##"
            $root = @panel
            [panel]
            [UiPanel]
                style = {
                    fill_kind = "linear"
                    gradient = { start_color = "#445566" end_color = "#112233" vector = (0, -1) }
                    corner_radii = (0.1, 0.2, 0.3, 0.4)
                    outer_shadow = { color = "#00000088" distance = 6 falloff = 9 vector = (1, -1) size = 1.2 }
                    inner_shadow = { color = "#00000044" distance = 2 falloff = 4 vector = (0, -1) size = 1.0 }
                    outer_highlight = { color = "#FFFFFF22" distance = 1 falloff = 3 vector = (-1, 1) size = 1.0 }
                    inner_highlight = { color = "#FFFFFF33" distance = 1 falloff = 2 vector = (-1, 1) size = 1.0 }
                }
            [/UiPanel]
            [/panel]
            "##,
        )
        .parse_scene();

        let prepared =
            prepare_scene_with_loader(&scene, &|path| Err(format!("unknown scene path `{path}`")))
                .expect("prepare scene");
        let node = prepared
            .nodes
            .iter()
            .find(|pending| pending.key_name == "panel")
            .expect("panel node");
        match &node.node.data {
            SceneNodeData::UiPanel(panel) => {
                assert_eq!(panel.style.fill_kind, perro_ui::UiFillKind::Linear);
                assert_eq!(
                    panel.style.gradient.start_color,
                    Color::from_hex("#445566").expect("test or bench setup must succeed")
                );
                assert_eq!(panel.style.corner_radii.tl, 0.1);
                assert_eq!(panel.style.corner_radii.tr, 0.2);
                assert_eq!(panel.style.corner_radii.br, 0.3);
                assert_eq!(panel.style.corner_radii.bl, 0.4);
                assert_eq!(panel.style.outer_shadow.distance, 6.0);
                assert_eq!(panel.style.inner_shadow.distance, 2.0);
                assert_eq!(panel.style.outer_highlight.falloff, 3.0);
                assert_eq!(panel.style.inner_highlight.falloff, 2.0);
            }
            other => panic!("expected UiPanel node, got {other:?}"),
        }
    }

    #[test]
    fn ui_nodes_ignore_absolute_pixel_size() {
        let scene = Parser::new(
            r#"
            [button]
            [UiButton]
                size = (160, 48)
                size_px = (170, 50)
                pixel_size = (180, 52)
                min_size = (120, 40)
                max_size = (240, 96)
                min_width = 110
                min_height = 32
                max_width = 220
                max_height = 80
            [/UiButton]
            [/button]
            "#,
        )
        .parse_scene();

        let prepared =
            prepare_scene_with_loader(&scene, &|path| Err(format!("unknown scene path `{path}`")))
                .expect("scene prepare");
        let button = prepared
            .nodes
            .iter()
            .find(|pending| matches!(pending.node.data, SceneNodeData::UiButton(_)))
            .expect("button node");
        match &button.node.data {
            SceneNodeData::UiButton(button) => {
                assert_eq!(button.layout.size, perro_ui::UiVector2::ZERO);
                assert_eq!(button.layout.min_size, Vector2::ZERO);
                assert_eq!(button.layout.max_size, perro_ui::UiLayoutData::NO_MAX_SIZE);
            }
            other => panic!("expected UiButton node, got {other:?}"),
        }
    }

    #[test]
    fn scene_loader_builds_ui_nodes_from_scene_blocks() {
        let scene = Parser::new(
            r##"
            $root = @menu
            [menu]
            [UiButton]
                visible = false
                input_enabled = false
                mouse_filter = "pass"
                clip_children = true
                anchor = "tr"
                position_ratio = (0.5, 0.25)
                size_ratio = (0.5, 0.1)
                scale = (2, 0.5)
                rotation = 0.25
                h_size = "fill"
                v_size = "fit_children"
                pivot_ratio = (0, 0)
                padding = (0.1, 0.2, 0.3, 0.4)
                style = { fill = "#101820" stroke = "#A0A8B0" radius = 0.3 }
                hover_fill = "#202830"
                cursor_icon = "grab"
                pressed_fill = "#303840"
                hover_signals = ["ui_hover"]
                pressed_signals = ["ui_down", "ui_press_any"]
                clicked_signals = ["ui_click"]
                hover = {
                    size_ratio = (0.65, 0.08666667)
                    scale = (1.1, 1.2)
                    rotation = 0.5
                    style = { fill = "#405060" stroke = "#C0D0E0" radius = 0.4 }
                }
                pressed = {
                    size_ratio = (0.55, 0.07)
                    scale = (0.9, 0.8)
                    rotation = -0.25
                    style = { fill = "#182028" stroke = "#8090A0" radius = 0.2 }
                }
                radius = "full"
                disabled = true
            [/UiButton]
            [/menu]

            [items]
            parent = menu
            [UiGrid]
                columns = 3
                h_spacing = 8
                v_spacing = "fill"
            [/UiGrid]
            [/items]

            [generic]
            parent = menu
            [UiLayout]
                mode = "grid"
                columns = 2
                spacing = 4
            [/UiLayout]
            [/generic]

            [forced_h]
            parent = menu
            [UiHLayout]
                mode = "v"
                spacing = "fill"
            [/UiHLayout]
            [/forced_h]

            [forced_v]
            parent = menu
            [UiVLayout]
                mode = "grid"
            [/UiVLayout]
            [/forced_v]

            [defaults]
            parent = menu
            [UiPanel/]
            [/defaults]

            [entry]
            parent = menu
            [UiTextBox]
                hover_signals = ["entry_hover"]
                hover_exit_signals = ["entry_unhover"]
                focused_signals = ["entry_focus"]
                unfocused_signals = ["entry_unfocus"]
                text_changed_signals = ["entry_text"]
                input_type = "f32"
            [/UiTextBox]
            [/entry]

            [scroller]
            parent = menu
            [UiScrollContainer]
                scroll = (12, 34)
                scroll_dir = "horizontal"
                scrollbar_side = "left"
                scrollbar_padding = 9
            [/UiScrollContainer]
            [/scroller]
            "##,
        )
        .parse_scene();

        let prepared =
            prepare_scene_with_loader(&scene, &|path| Err(format!("unknown scene path `{path}`")))
                .expect("prepare scene");

        let menu = prepared
            .nodes
            .iter()
            .find(|pending| pending.key_name == "menu")
            .expect("menu node");
        match &menu.node.data {
            SceneNodeData::UiButton(button) => {
                assert!(!button.visible);
                assert!(!button.input_enabled);
                assert_eq!(button.mouse_filter, UiMouseFilter::Pass);
                assert!(button.clip_children);
                assert_eq!(button.layout.anchor, perro_ui::UiAnchor::TopRight);
                assert!(button.disabled);
                assert_eq!(button.style.corner_radius(), 0.3);
                assert_eq!(button.style.fill, Color::from_hex("#101820").expect("test or bench setup must succeed"));
                assert_eq!(button.style.stroke, Color::from_hex("#A0A8B0").expect("test or bench setup must succeed"));
                assert_eq!(button.hover_style.fill, Color::from_hex("#405060").expect("test or bench setup must succeed"));
                assert_eq!(
                    button.hover_style.stroke,
                    Color::from_hex("#C0D0E0").expect("test or bench setup must succeed")
                );
                assert_eq!(button.hover_style.corner_radius(), 0.4);
                assert_eq!(button.cursor_icon, perro_ui::CursorIcon::Grab);
                assert_eq!(
                    button.pressed_style.fill,
                    Color::from_hex("#182028").expect("test or bench setup must succeed")
                );
                assert_eq!(
                    button.hover_signals,
                    vec![perro_ids::SignalID::from_string("ui_hover")]
                );
                assert_eq!(
                    button.pressed_signals,
                    vec![
                        perro_ids::SignalID::from_string("ui_down"),
                        perro_ids::SignalID::from_string("ui_press_any"),
                    ]
                );
                assert_eq!(
                    button.clicked_signals,
                    vec![perro_ids::SignalID::from_string("ui_click")]
                );
                let hover = button.hover_base.as_ref().expect("hover base");
                assert_eq!(
                    hover.layout.size,
                    perro_ui::UiVector2::ratio(0.65, 0.08666667)
                );
                assert_eq!(hover.transform.scale, Vector2::new(1.1, 1.2));
                assert_eq!(hover.transform.rotation, 0.5);
                assert!(button.hover_size_override);
                let pressed = button.pressed_base.as_ref().expect("pressed base");
                assert_eq!(pressed.layout.size, perro_ui::UiVector2::ratio(0.55, 0.07));
                assert_eq!(pressed.transform.scale, Vector2::new(0.9, 0.8));
                assert_eq!(pressed.transform.rotation, -0.25);
                assert!(button.pressed_size_override);
                assert_eq!(button.transform.scale, Vector2::new(2.0, 0.5));
                assert_eq!(button.transform.rotation, 0.25);
                assert_eq!(button.layout.h_size, perro_ui::UiSizeMode::Fill);
                assert_eq!(button.layout.v_size, perro_ui::UiSizeMode::FitChildren);
                assert_eq!(
                    button.layout.padding,
                    perro_ui::UiRect::new(0.1, 0.2, 0.3, 0.4)
                );
                match button.transform.position.x {
                    perro_ui::UiUnit::Percent(v) => assert_eq!(v, 50.0),
                    other => panic!("expected percent x, got {other:?}"),
                }
                match button.transform.position.y {
                    perro_ui::UiUnit::Percent(v) => assert_eq!(v, 50.0),
                    other => panic!("expected percent y, got {other:?}"),
                }
                match button.transform.pivot.x {
                    perro_ui::UiUnit::Percent(v) => assert_eq!(v, 0.0),
                    other => panic!("expected percent pivot x, got {other:?}"),
                }
            }
            other => panic!("expected UiButton menu node, got {other:?}"),
        }

        let items = prepared
            .nodes
            .iter()
            .find(|pending| pending.key_name == "items")
            .expect("items node");
        match &items.node.data {
            SceneNodeData::UiGrid(grid) => {
                assert_eq!(grid.columns, 3);
                assert_eq!(grid.h_spacing, 8.0);
                assert_eq!(grid.v_spacing_mode, perro_ui::UiLayoutSpacingMode::Fill);
            }
            other => panic!("expected UiGrid items node, got {other:?}"),
        }

        let generic = prepared
            .nodes
            .iter()
            .find(|pending| pending.key_name == "generic")
            .expect("generic node");
        match &generic.node.data {
            SceneNodeData::UiLayout(layout) => {
                assert_eq!(layout.inner.mode, perro_ui::UiLayoutMode::Grid);
                assert_eq!(layout.inner.columns, 2);
                assert_eq!(layout.inner.spacing, 4.0);
            }
            other => panic!("expected UiLayout generic node, got {other:?}"),
        }

        let forced_h = prepared
            .nodes
            .iter()
            .find(|pending| pending.key_name == "forced_h")
            .expect("forced_h node");
        match &forced_h.node.data {
            SceneNodeData::UiHLayout(layout) => {
                assert_eq!(layout.mode(), perro_ui::UiLayoutMode::H);
                assert_eq!(
                    layout.inner.spacing_mode,
                    perro_ui::UiLayoutSpacingMode::Fill
                );
            }
            other => panic!("expected UiHLayout forced_h node, got {other:?}"),
        }

        let forced_v = prepared
            .nodes
            .iter()
            .find(|pending| pending.key_name == "forced_v")
            .expect("forced_v node");
        match &forced_v.node.data {
            SceneNodeData::UiVLayout(layout) => {
                assert_eq!(layout.mode(), perro_ui::UiLayoutMode::V);
            }
            other => panic!("expected UiVLayout forced_v node, got {other:?}"),
        }

        let defaults = prepared
            .nodes
            .iter()
            .find(|pending| pending.key_name == "defaults")
            .expect("defaults node");
        match &defaults.node.data {
            SceneNodeData::UiPanel(panel) => {
                assert_eq!(panel.layout.anchor, perro_ui::UiAnchor::Center);
                assert_eq!(
                    panel.transform.position,
                    perro_ui::UiVector2::ratio(0.5, 0.5)
                );
                assert_eq!(panel.layout.h_align, perro_ui::UiHorizontalAlign::Center);
                assert_eq!(panel.layout.v_align, perro_ui::UiVerticalAlign::Center);
            }
            other => panic!("expected UiPanel defaults node, got {other:?}"),
        }

        let entry = prepared
            .nodes
            .iter()
            .find(|pending| pending.key_name == "entry")
            .expect("entry node");
        match &entry.node.data {
            SceneNodeData::UiTextBox(text_box) => {
                assert_eq!(
                    text_box.inner.hover_signals,
                    vec![perro_ids::SignalID::from_string("entry_hover")]
                );
                assert_eq!(
                    text_box.inner.hover_exit_signals,
                    vec![perro_ids::SignalID::from_string("entry_unhover")]
                );
                assert_eq!(
                    text_box.inner.focused_signals,
                    vec![perro_ids::SignalID::from_string("entry_focus")]
                );
                assert_eq!(
                    text_box.inner.unfocused_signals,
                    vec![perro_ids::SignalID::from_string("entry_unfocus")]
                );
                assert_eq!(
                    text_box.inner.text_changed_signals,
                    vec![perro_ids::SignalID::from_string("entry_text")]
                );
                assert_eq!(
                    text_box.inner.input_type,
                    perro_ui::UiTextInputType::SignedFloat
                );
            }
            other => panic!("expected UiTextBox entry node, got {other:?}"),
        }

        let scroller = prepared
            .nodes
            .iter()
            .find(|pending| pending.key_name == "scroller")
            .expect("scroller node");
        match &scroller.node.data {
            SceneNodeData::UiScrollContainer(scroller) => {
                assert!(scroller.clip_children);
                assert_eq!(scroller.scroll, Vector2::new(12.0, 34.0));
                assert_eq!(scroller.scroll_dir, perro_ui::UiScrollDirection::Horizontal);
                assert_eq!(scroller.scroll_bar_side, perro_ui::UiScrollBarSide::Left);
                assert_eq!(scroller.scroll_bar_padding, 9.0);
            }
            other => panic!("expected UiScrollContainer scroller node, got {other:?}"),
        }
    }

}
