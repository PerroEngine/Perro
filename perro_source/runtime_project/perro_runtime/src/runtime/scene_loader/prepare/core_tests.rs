#[cfg(test)]
mod tests {
    use super::*;
    use perro_nodes::SceneNodeData;
    use perro_scene::Parser;

    #[test]
    fn root_of_merges_root_defaults_overrides_and_children() {
        let host = Parser::new(
            r#"
            @root = host
            [host]
            root_of = "res://base.scn"
            script_vars = {
                keep: 5,
                remove_me: __unset__,
                nested: { b: 20, c: 30 },
                added: true
            }
            [Node2D]
                rotation = 3.0
            [/Node2D]
            [/host]

            [local_child]
            parent = host
            [Node]
            [/Node]
            [/local_child]
            "#,
        )
        .parse_scene();

        let base = Parser::new(
            r#"
            @root = base_root
            [base_root]
            script = "res://base_script.rs"
            script_vars = {
                keep: 1,
                remove_me: 2,
                nested: { a: 10, b: 11 },
                old_only: 9
            }
            [Node2D]
                position = (1, 2)
                rotation = 1.0
            [/Node2D]
            [/base_root]

            [base_child]
            parent = base_root
            [Node]
            [/Node]
            [/base_child]
            "#,
        )
        .parse_scene();

        let prepared = prepare_scene_with_loader(&host, &|path| match path {
            "res://base.scn" => Ok(std::sync::Arc::new(base.clone())),
            _ => Err(format!("unknown scene path `{path}`")),
        })
        .expect("prepare scene");

        let host_script = prepared
            .scripts
            .iter()
            .find(|pending| pending.node_key_name == "host")
            .expect("host script");
        assert_eq!(
            host_script.script_path_hash,
            string_to_u64("res://base_script.rs")
        );

        let mut vars = BTreeMap::new();
        for (name, value) in &host_script.scene_injected_vars {
            vars.insert(name.as_str(), value);
        }
        assert!(vars.contains_key("keep"));
        assert!(vars.contains_key("added"));
        assert!(vars.contains_key("nested"));
        assert!(vars.contains_key("old_only"));
        assert!(!vars.contains_key("remove_me"));

        match vars.get("nested").expect("nested var") {
            SceneValue::Object(fields) => {
                assert!(fields.iter().any(|(k, _)| k.as_ref() == "a"));
                assert!(fields.iter().any(|(k, _)| k.as_ref() == "b"));
                assert!(fields.iter().any(|(k, _)| k.as_ref() == "c"));
            }
            other => panic!("expected nested object, got {other:?}"),
        }

        let host_node = prepared
            .nodes
            .iter()
            .find(|pending| pending.key_name == "host")
            .expect("host node");
        match &host_node.node.data {
            SceneNodeData::Node2D(node_2d) => {
                assert_eq!(node_2d.position.x, 1.0);
                assert_eq!(node_2d.position.y, 2.0);
                assert_eq!(node_2d.rotation, 3.0);
            }
            other => panic!("expected Node2D host node, got {other:?}"),
        }

        assert!(prepared.nodes.iter().any(|pending| pending.key_name == "base_child"));
        assert!(prepared.nodes.iter().any(|pending| pending.key_name == "local_child"));
    }

    #[test]
    fn root_of_script_clear_prevents_inherited_script() {
        let host = Parser::new(
            r#"
            @root = host
            [host]
            root_of = "res://base.scn"
            script = null
            [Node]
            [/Node]
            [/host]
            "#,
        )
        .parse_scene();

        let base = Parser::new(
            r#"
            @root = base_root
            [base_root]
            script = "res://base_script.rs"
            [Node]
            [/Node]
            [/base_root]
            "#,
        )
        .parse_scene();

        let prepared = prepare_scene_with_loader(&host, &|path| match path {
            "res://base.scn" => Ok(std::sync::Arc::new(base.clone())),
            _ => Err(format!("unknown scene path `{path}`")),
        })
        .expect("prepare scene");

        assert!(!prepared.scripts.iter().any(|pending| pending.node_key_name == "host"));
    }

    #[test]
    fn root_of_without_host_type_block_inherits_template_root_data() {
        let host = Parser::new(
            r#"
            @root = host
            [host]
            root_of = "res://base.scn"
            [/host]
            "#,
        )
        .parse_scene();

        let base = Parser::new(
            r#"
            @root = base_root
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
    fn scene_loader_button_state_style_inherits_base_fields() {
        let scene = Parser::new(
            r##"
            @root = button
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

        let prepared = prepare_scene_with_loader(&scene, &|path| {
            Err(format!("unknown scene path `{path}`"))
        })
        .expect("prepare scene");

        let node = prepared
            .nodes
            .iter()
            .find(|pending| pending.key_name == "button")
            .expect("button node");
        match &node.node.data {
            SceneNodeData::UiButton(button) => {
                assert_eq!(button.style.corner_radius, 1.0);
                assert_eq!(button.style.shadow.color, Color::from_hex("#00000066").unwrap());
                assert_eq!(button.style.shadow.distance, 8.0);
                assert_eq!(button.style.shadow.falloff, 12.0);
                assert_eq!(button.style.shadow.vector, Vector2::new(1.0, -1.0));
                assert_eq!(button.style.shadow.size, 1.5);
                assert_eq!(button.style.highlight.color, Color::from_hex("#FFFFFF55").unwrap());
                assert_eq!(button.style.highlight.distance, 2.0);
                assert_eq!(button.style.highlight.falloff, 4.0);
                assert_eq!(button.style.highlight.vector, Vector2::new(-1.0, 1.0));
                assert_eq!(button.style.highlight.size, 1.0);
                assert_eq!(button.hover_style.fill, Color::from_hex("#202830").unwrap());
                assert_eq!(button.hover_style.stroke, button.style.stroke);
                assert_eq!(button.hover_style.corner_radius, 1.0);
                assert_eq!(button.pressed_style.fill, Color::from_hex("#303840").unwrap());
                assert_eq!(button.pressed_style.stroke, button.style.stroke);
                assert_eq!(button.pressed_style.corner_radius, 1.0);
            }
            other => panic!("expected UiButton node, got {other:?}"),
        }
    }

    #[test]
    fn scene_loader_builds_ui_nodes_from_scene_blocks() {
        let scene = Parser::new(
            r##"
            @root = menu
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
                padding = (1, 2, 3, 4)
                style = { fill = "#101820" stroke = "#A0A8B0" radius = 0.3 }
                hover_fill = "#202830"
                cursor_icon = "grab"
                pressed_fill = "#303840"
                hover_signals = ["ui_hover"]
                pressed_signals = ["ui_down", "ui_press_any"]
                click_signals = ["ui_click"]
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
                v_spacing = 12
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
            [UiPanel]
            [/UiPanel]
            [/defaults]

            [entry]
            parent = menu
            [UiTextBox]
                hover_signals = ["entry_hover"]
                hover_exit_signals = ["entry_unhover"]
                focused_signals = ["entry_focus"]
                unfocused_signals = ["entry_unfocus"]
                text_changed_signals = ["entry_text"]
            [/UiTextBox]
            [/entry]
            "##,
        )
        .parse_scene();

        let prepared = prepare_scene_with_loader(&scene, &|path| {
            Err(format!("unknown scene path `{path}`"))
        })
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
                assert_eq!(button.style.corner_radius, 0.3);
                assert_eq!(button.style.fill, Color::from_hex("#101820").unwrap());
                assert_eq!(button.style.stroke, Color::from_hex("#A0A8B0").unwrap());
                assert_eq!(button.hover_style.fill, Color::from_hex("#405060").unwrap());
                assert_eq!(
                    button.hover_style.stroke,
                    Color::from_hex("#C0D0E0").unwrap()
                );
                assert_eq!(button.hover_style.corner_radius, 0.4);
                assert_eq!(button.cursor_icon, perro_ui::CursorIcon::Grab);
                assert_eq!(
                    button.pressed_style.fill,
                    Color::from_hex("#182028").unwrap()
                );
                assert_eq!(button.hover_signals, vec![perro_ids::SignalID::from_string("ui_hover")]);
                assert_eq!(
                    button.pressed_signals,
                    vec![
                        perro_ids::SignalID::from_string("ui_down"),
                        perro_ids::SignalID::from_string("ui_press_any"),
                    ]
                );
                assert_eq!(button.click_signals, vec![perro_ids::SignalID::from_string("ui_click")]);
                let hover = button.hover_base.as_ref().expect("hover base");
                assert_eq!(hover.layout.size, perro_ui::UiVector2::ratio(0.65, 0.08666667));
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
                    perro_ui::UiRect::new(1.0, 2.0, 3.0, 4.0)
                );
                match button.transform.position.x {
                    perro_ui::UiUnit::Percent(v) => assert_eq!(v, 50.0),
                    other => panic!("expected percent x, got {other:?}"),
                }
                match button.transform.position.y {
                    perro_ui::UiUnit::Percent(v) => assert_eq!(v, 25.0),
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
                assert_eq!(grid.v_spacing, 12.0);
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
                assert_eq!(panel.transform.position, perro_ui::UiVector2::ratio(0.5, 0.5));
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
            }
            other => panic!("expected UiTextBox entry node, got {other:?}"),
        }
    }

    #[test]
    fn scene_loader_builds_ik_target_3d_fields_and_skeleton_link() {
        let scene = Parser::new(
            r#"
            @root = Rig
            [Rig]
            [Skeleton3D]
                skeleton = "res://rig.pskel"
            [/Skeleton3D]
            [/Rig]

            [HandTarget]
            [IKTarget3D]
                skeleton = "Rig"
                bone = 5
                chain_length = 3
                iterations = 12
                tolerance = 0.05
                weight = 0.75
                match_rotation = false
            [/IKTarget3D]
            [/HandTarget]
            "#,
        )
        .parse_scene();

        let prepared = prepare_scene_with_loader(&scene, &|path| {
            Err(format!("unknown scene path `{path}`"))
        })
        .expect("prepare scene");

        let target = prepared
            .nodes
            .iter()
            .find(|pending| pending.key_name == "HandTarget")
            .expect("ik target node");
        match &target.node.data {
            SceneNodeData::IKTarget3D(ik) => {
                assert_eq!(ik.bone_index, 5);
                assert_eq!(ik.chain_length, 3);
                assert_eq!(ik.iterations, 12);
                assert_eq!(ik.tolerance, 0.05);
                assert_eq!(ik.weight, 0.75);
                assert!(!ik.match_rotation);
            }
            other => panic!("expected IKTarget3D node, got {other:?}"),
        }
        assert!(target.ik_target_skeleton_target.is_some());
    }

}
