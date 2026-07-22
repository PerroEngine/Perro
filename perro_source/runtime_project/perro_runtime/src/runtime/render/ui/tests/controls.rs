mod controls {
    use super::*;

    #[test]
    fn ui_auto_layout_includes_ui_descendants_through_non_ui_wrappers() {
        let mut runtime = Runtime::new();
        runtime.set_viewport_size(800, 600);

        let mut layout = UiHLayout::new();
        layout.layout.size = UiVector2::pixels(300.0, 120.0);
        layout.inner.h_spacing = 20.0 / 300.0;
        let layout = insert_ui_node(&mut runtime, layout.into());

        let left = insert_panel(&mut runtime, [80.0, 80.0], Color::new(1.0, 0.0, 0.0, 1.0));
        attach_child(&mut runtime, layout, left);

        let wrapper = runtime.create::<perro_nodes::Node3D>();
        let right = insert_panel(&mut runtime, [80.0, 80.0], Color::new(0.0, 1.0, 0.0, 1.0));
        attach_child(&mut runtime, wrapper, right);
        attach_child(&mut runtime, layout, wrapper);

        runtime.extract_render_ui_commands();

        let left_rect = runtime
            .render_ui
            .computed_rects
            .get(&left)
            .copied()
            .expect("left rect exists");
        let right_rect = runtime
            .render_ui
            .computed_rects
            .get(&right)
            .copied()
            .expect("right rect exists");
        let delta = right_rect.center.x - left_rect.center.x;
        assert!((delta.abs() - 100.0).abs() < 0.001);
    }

    #[test]
    fn ui_child_added_to_retained_layout_renders_without_resize() {
        let mut runtime = Runtime::new();
        runtime.set_viewport_size(800, 600);

        let mut layout = UiVLayout::new();
        layout.layout.size = UiVector2::pixels(300.0, 200.0);
        let layout = insert_ui_node(&mut runtime, layout.into());

        runtime.extract_render_ui_commands();
        runtime.drain_render_commands(&mut Vec::new());
        runtime.clear_dirty_flags();

        let child = runtime.create::<UiPanel>();
        assert!(runtime.reparent(layout, child));
        let _ = runtime.with_node_mut::<UiPanel, _, _>(child, |panel| {
            panel.layout.size = UiVector2::pixels(80.0, 40.0);
            panel.style.fill = Color::new(0.7, 0.2, 0.1, 1.0);
        });

        runtime.extract_render_ui_commands();
        let mut commands = Vec::new();
        runtime.drain_render_commands(&mut commands);

        assert!(commands.iter().any(|cmd| matches!(
            cmd,
            RenderCommand::Ui(UiCommand::UpsertPanel { node, rect, fill, .. })
                if *node == child && rect.size == [80.0, 40.0] && *fill == rgba(0.7, 0.2, 0.1, 1.0)
        )));
    }

    #[test]
    fn runtime_created_ui_child_under_hidden_parent_renders_when_shown() {
        let mut runtime = Runtime::new();
        runtime.set_viewport_size(800, 600);

        let parent = runtime.create::<UiPanel>();
        let _ = runtime.with_node_mut::<UiPanel, _, _>(parent, |panel| {
            panel.layout.size = UiVector2::pixels(260.0, 120.0);
            panel.visible = false;
        });

        let ids = runtime.create_nodes(&[NodeSpec::new(UiPanel::new())], parent);
        let child = ids[0];
        let _ = runtime.with_node_mut::<UiPanel, _, _>(child, |panel| {
            panel.layout.size = UiVector2::pixels(80.0, 40.0);
            panel.style.fill = Color::new(0.7, 0.2, 0.1, 1.0);
        });

        runtime.extract_render_ui_commands();
        let mut commands = Vec::new();
        runtime.drain_render_commands(&mut commands);
        assert!(!commands.iter().any(|cmd| matches!(
            cmd,
            RenderCommand::Ui(UiCommand::UpsertPanel { node, .. }) if *node == child
        )));
        runtime.clear_dirty_flags();

        let _ = runtime.with_node_mut::<UiPanel, _, _>(parent, |panel| {
            panel.visible = true;
        });
        runtime.extract_render_ui_commands();
        commands.clear();
        runtime.drain_render_commands(&mut commands);

        assert!(commands.iter().any(|cmd| matches!(
            cmd,
            RenderCommand::Ui(UiCommand::UpsertPanel { node, rect, fill, .. })
                if *node == child && rect.size == [80.0, 40.0] && *fill == rgba(0.7, 0.2, 0.1, 1.0)
        )));
    }

    #[test]
    fn runtime_created_root_ui_extracts_after_direct_setup_before_first_frame() {
        let mut runtime = Runtime::new();
        runtime.set_viewport_size(800, 600);

        let node = runtime.create::<UiPanel>();
        if let Some(mut scene_node) = runtime.nodes.get_mut(node)
            && let SceneNodeData::UiPanel(panel) = &mut scene_node.data
        {
            panel.layout.size = UiVector2::pixels(90.0, 45.0);
            panel.style.fill = Color::new(0.3, 0.6, 0.9, 1.0);
        }

        runtime.extract_render_ui_commands();
        let mut commands = Vec::new();
        runtime.drain_render_commands(&mut commands);

        assert!(commands.iter().any(|cmd| matches!(
            cmd,
            RenderCommand::Ui(UiCommand::UpsertPanel { node: n, rect, fill, .. })
                if *n == node && rect.size == [90.0, 45.0] && *fill == rgba(0.3, 0.6, 0.9, 1.0)
        )));
    }

    #[test]
    fn same_z_ui_child_draws_above_newer_parent_after_reparent() {
        let mut runtime = Runtime::new();
        runtime.set_viewport_size(800, 600);

        let mut label = perro_ui::UiLabel::new();
        label.layout.size = UiVector2::pixels(90.0, 45.0);
        label.text = "7".into();
        let label = insert_ui_node(&mut runtime, SceneNodeData::UiLabel(Box::new(label)));

        let panel = insert_panel(&mut runtime, [120.0, 60.0], Color::new(0.2, 0.4, 0.7, 1.0));
        attach_child(&mut runtime, panel, label);

        runtime.extract_render_ui_commands();
        let mut commands = Vec::new();
        runtime.drain_render_commands(&mut commands);

        let parent_z = commands
            .iter()
            .find_map(|cmd| match cmd {
                RenderCommand::Ui(UiCommand::UpsertPanel { node, rect, .. }) if *node == panel => {
                    Some(rect.z_index)
                }
                _ => None,
            })
            .expect("parent panel command");
        let child_z = commands
            .iter()
            .find_map(|cmd| match cmd {
                RenderCommand::Ui(UiCommand::UpsertLabel { node, rect, .. }) if *node == label => {
                    Some(rect.z_index)
                }
                _ => None,
            })
            .expect("child label command");

        assert!(child_z > parent_z);
    }

    #[test]
    fn saturated_z_ui_viewport_draws_above_parent_panel() {
        let mut runtime = Runtime::new();
        runtime.set_viewport_size(800, 600);

        let mut viewport = UiViewport::default();
        viewport.layout.size = UiVector2::pixels(90.0, 45.0);
        let viewport = insert_ui_node(&mut runtime, SceneNodeData::UiViewport(Box::new(viewport)));

        let mut panel = UiPanel::new();
        panel.layout.size = UiVector2::pixels(120.0, 60.0);
        panel.layout.z_index = i32::MAX;
        let panel = insert_ui_node(&mut runtime, SceneNodeData::UiPanel(Box::new(panel)));
        attach_child(&mut runtime, panel, viewport);

        runtime.extract_render_ui_commands();
        let mut commands = Vec::new();
        runtime.drain_render_commands(&mut commands);

        let parent_z = commands.iter().find_map(|cmd| match cmd {
            RenderCommand::Ui(UiCommand::UpsertPanel { node, rect, .. }) if *node == panel => {
                Some(rect.z_index)
            }
            _ => None,
        });
        let viewport_z = commands.iter().find_map(|cmd| match cmd {
            RenderCommand::Ui(UiCommand::UpsertImage { node, rect, .. }) if *node == viewport => {
                Some(rect.z_index)
            }
            _ => None,
        });

        assert!(viewport_z.expect("viewport z") > parent_z.expect("parent z"));
    }

    #[test]
    fn button_uses_hover_and_pressed_styles_from_mouse_state() {
        let mut runtime = Runtime::new();
        runtime.set_viewport_size(800, 600);
        let node = insert_button(&mut runtime, [120.0, 40.0]);

        runtime.extract_render_ui_commands();
        let mut commands = Vec::new();
        runtime.drain_render_commands(&mut commands);
        assert!(commands.iter().any(|cmd| matches!(
            cmd,
            RenderCommand::Ui(UiCommand::UpsertButton { node: n, fill, .. })
                if *n == node && *fill == rgba(0.1, 0.2, 0.3, 1.0)
        )));
        runtime.clear_dirty_flags();

        runtime.set_mouse_position(400.0, 300.0);
        runtime.extract_render_ui_commands();
        commands.clear();
        runtime.drain_render_commands(&mut commands);
        assert!(commands.iter().any(|cmd| matches!(
            cmd,
            RenderCommand::Ui(UiCommand::UpsertButton { node: n, fill, .. })
                if *n == node && *fill == rgba(0.2, 0.3, 0.4, 1.0)
        )));

        runtime.clear_dirty_flags();
        runtime.set_mouse_button_state(MouseButton::Left, true);
        runtime.extract_render_ui_commands();
        commands.clear();
        runtime.drain_render_commands(&mut commands);
        assert!(commands.iter().any(|cmd| matches!(
            cmd,
            RenderCommand::Ui(UiCommand::UpsertButton { node: n, fill, .. })
                if *n == node && *fill == rgba(0.3, 0.4, 0.5, 1.0)
        )));
    }

    #[test]
    fn button_held_before_hover_does_not_press_or_click() {
        let mut runtime = Runtime::new();
        runtime.set_viewport_size(800, 600);
        let node = insert_button(&mut runtime, [120.0, 40.0]);
        runtime.nodes.get_mut(node).expect("button").name = Cow::Borrowed("play");

        runtime.extract_render_ui_commands();
        runtime.drain_render_commands(&mut Vec::new());
        runtime.clear_dirty_flags();

        runtime.begin_input_frame();
        runtime.set_mouse_position(20.0, 20.0);
        runtime.set_mouse_button_state(MouseButton::Left, true);
        runtime.extract_render_ui_commands();

        runtime.begin_input_frame();
        runtime.set_mouse_position(400.0, 300.0);
        runtime.set_mouse_button_state(MouseButton::Left, true);
        runtime.extract_render_ui_commands();
        assert_eq!(
            runtime.render_ui.button_states.get(&node).copied(),
            Some(UiButtonVisualState::Hover)
        );

        runtime.begin_input_frame();
        runtime.set_mouse_button_state(MouseButton::Left, false);
        runtime.extract_render_ui_commands();
        assert_eq!(
            runtime.render_ui.button_states.get(&node).copied(),
            Some(UiButtonVisualState::Hover)
        );
        assert!(
            !runtime
                .signal_runtime
                .queued_ui_signals
                .iter()
                .any(
                    |(signal, _)| *signal == SignalID::from_string("play_pressed")
                        || *signal == SignalID::from_string("play_released")
                        || *signal == SignalID::from_string("play_clicked")
                )
        );
    }

    #[test]
    fn disabled_button_ignores_hover_and_pressed_mouse_state() {
        let mut runtime = Runtime::new();
        runtime.set_viewport_size(800, 600);
        let node = insert_button(&mut runtime, [120.0, 40.0]);
        if let Some(mut scene_node) = runtime.nodes.get_mut(node)
            && let SceneNodeData::UiButton(button) = &mut scene_node.data
        {
            button.disabled = true;
        }

        runtime.extract_render_ui_commands();
        runtime.drain_render_commands(&mut Vec::new());
        runtime.clear_dirty_flags();

        runtime.set_mouse_position(400.0, 300.0);
        runtime.extract_render_ui_commands();
        let mut commands = Vec::new();
        runtime.drain_render_commands(&mut commands);
        assert!(!commands.iter().any(|cmd| matches!(
            cmd,
            RenderCommand::Ui(UiCommand::UpsertButton { node: n, fill, .. })
                if *n == node && *fill == rgba(0.2, 0.3, 0.4, 1.0)
        )));
        assert_eq!(runtime.take_cursor_icon_request(), None);

        runtime.clear_dirty_flags();
        runtime.set_mouse_button_state(MouseButton::Left, true);
        runtime.extract_render_ui_commands();
        commands.clear();
        runtime.drain_render_commands(&mut commands);
        assert!(!commands.iter().any(|cmd| matches!(
            cmd,
            RenderCommand::Ui(UiCommand::UpsertButton { node: n, fill, .. })
                if *n == node && *fill == rgba(0.3, 0.4, 0.5, 1.0)
        )));
        assert_eq!(runtime.take_cursor_icon_request(), None);
    }

    #[test]
    fn input_disabled_button_ignores_hover_and_pressed_mouse_state() {
        let mut runtime = Runtime::new();
        runtime.set_viewport_size(800, 600);
        let node = insert_button(&mut runtime, [120.0, 40.0]);
        if let Some(mut scene_node) = runtime.nodes.get_mut(node)
            && let SceneNodeData::UiButton(button) = &mut scene_node.data
        {
            button.input_enabled = false;
        }

        runtime.extract_render_ui_commands();
        runtime.drain_render_commands(&mut Vec::new());
        runtime.clear_dirty_flags();

        runtime.set_mouse_position(400.0, 300.0);
        runtime.extract_render_ui_commands();
        let mut commands = Vec::new();
        runtime.drain_render_commands(&mut commands);
        assert!(!commands.iter().any(|cmd| matches!(
            cmd,
            RenderCommand::Ui(UiCommand::UpsertButton { node: n, fill, .. })
                if *n == node && *fill == rgba(0.2, 0.3, 0.4, 1.0)
        )));
        assert_eq!(runtime.take_cursor_icon_request(), None);

        runtime.clear_dirty_flags();
        runtime.set_mouse_button_state(MouseButton::Left, true);
        runtime.extract_render_ui_commands();
        commands.clear();
        runtime.drain_render_commands(&mut commands);
        assert!(!commands.iter().any(|cmd| matches!(
            cmd,
            RenderCommand::Ui(UiCommand::UpsertButton { node: n, fill, .. })
                if *n == node && *fill == rgba(0.3, 0.4, 0.5, 1.0)
        )));
        assert_eq!(runtime.take_cursor_icon_request(), None);
    }

    #[test]
    fn disabling_hovered_button_restores_neutral_visual_state() {
        let mut runtime = Runtime::new();
        runtime.set_viewport_size(800, 600);
        let node = insert_button(&mut runtime, [120.0, 40.0]);

        runtime.extract_render_ui_commands();
        runtime.drain_render_commands(&mut Vec::new());
        runtime.clear_dirty_flags();

        runtime.set_mouse_position(400.0, 300.0);
        runtime.set_mouse_button_state(MouseButton::Left, true);
        runtime.extract_render_ui_commands();
        runtime.drain_render_commands(&mut Vec::new());
        runtime.clear_dirty_flags();

        if let Some(mut scene_node) = runtime.nodes.get_mut(node)
            && let SceneNodeData::UiButton(button) = &mut scene_node.data
        {
            button.disabled = true;
        }
        runtime.mark_ui_dirty(node, Runtime::UI_DIRTY_COMMANDS);
        runtime.extract_render_ui_commands();

        let mut commands = Vec::new();
        runtime.drain_render_commands(&mut commands);
        assert!(commands.iter().any(|cmd| matches!(
            cmd,
            RenderCommand::Ui(UiCommand::UpsertButton {
                node: n,
                fill,
                disabled,
                ..
            }) if *n == node && *fill == rgba(0.1, 0.2, 0.3, 1.0) && *disabled
        )));
        assert_eq!(
            runtime.render_ui.button_states.get(&node).copied(),
            Some(UiButtonVisualState::Neutral)
        );
    }

    #[test]
    fn input_disabling_hovered_button_restores_neutral_visual_state() {
        let mut runtime = Runtime::new();
        runtime.set_viewport_size(800, 600);
        let node = insert_button(&mut runtime, [120.0, 40.0]);

        runtime.extract_render_ui_commands();
        runtime.drain_render_commands(&mut Vec::new());
        runtime.clear_dirty_flags();

        runtime.set_mouse_position(400.0, 300.0);
        runtime.set_mouse_button_state(MouseButton::Left, true);
        runtime.extract_render_ui_commands();
        runtime.drain_render_commands(&mut Vec::new());
        runtime.clear_dirty_flags();

        if let Some(mut scene_node) = runtime.nodes.get_mut(node)
            && let SceneNodeData::UiButton(button) = &mut scene_node.data
        {
            button.input_enabled = false;
        }
        runtime.mark_ui_dirty(node, Runtime::UI_DIRTY_COMMANDS);
        runtime.extract_render_ui_commands();

        let mut commands = Vec::new();
        runtime.drain_render_commands(&mut commands);
        assert!(commands.iter().any(|cmd| matches!(
            cmd,
            RenderCommand::Ui(UiCommand::UpsertButton {
                node: n,
                fill,
                disabled,
                ..
            }) if *n == node && *fill == rgba(0.1, 0.2, 0.3, 1.0) && !*disabled
        )));
        assert_eq!(
            runtime.render_ui.button_states.get(&node).copied(),
            Some(UiButtonVisualState::Neutral)
        );
    }

    #[test]
    fn button_hover_requests_cursor_icon_and_unhover_restores_default() {
        let mut runtime = Runtime::new();
        runtime.set_viewport_size(800, 600);
        let node = insert_button(&mut runtime, [120.0, 40.0]);
        if let Some(mut scene_node) = runtime.nodes.get_mut(node)
            && let SceneNodeData::UiButton(button) = &mut scene_node.data
        {
            button.cursor_icon = perro_ui::CursorIcon::Grab;
        }

        runtime.extract_render_ui_commands();
        let _ = runtime.take_cursor_icon_request();
        runtime.clear_dirty_flags();

        runtime.set_mouse_position(400.0, 300.0);
        runtime.extract_render_ui_commands();
        assert_eq!(
            runtime.take_cursor_icon_request(),
            Some(perro_ui::CursorIcon::Grab)
        );
        runtime.clear_dirty_flags();

        runtime.set_mouse_position(0.0, 0.0);
        runtime.extract_render_ui_commands();
        assert_eq!(
            runtime.take_cursor_icon_request(),
            Some(perro_ui::CursorIcon::Default)
        );
    }

    #[test]
    fn ui_shape_emits_triangle_draw_command() {
        let mut runtime = Runtime::new();
        runtime.set_viewport_size(800, 600);
        let mut shape = UiShape::new();
        shape.layout.size = UiVector2::pixels(18.0, 18.0);
        shape.kind = UiShapeKind::Triangle;
        shape.fill = Color::new(0.2, 0.3, 0.4, 1.0);
        shape.stroke = Color::new(0.9, 0.2, 0.1, 1.0);
        shape.stroke_width = 2.0;
        let node = insert_ui_node(&mut runtime, shape.into());

        runtime.extract_render_ui_commands();
        let mut commands = Vec::new();
        runtime.drain_render_commands(&mut commands);

        assert!(commands.iter().any(|cmd| matches!(
            cmd,
            RenderCommand::Ui(UiCommand::UpsertShape {
                node: n,
                kind,
                fill,
                stroke,
                stroke_width,
                ..
            }) if *n == node
                && *kind == UiShapeKind::Triangle
                && *fill == rgba(0.2, 0.3, 0.4, 1.0)
                && *stroke == rgba(0.9, 0.2, 0.1, 1.0)
                && *stroke_width == 2.0
        )));
    }

    #[test]
    fn button_hover_respects_rounded_visible_shape() {
        let mut runtime = Runtime::new();
        runtime.set_viewport_size(800, 600);
        let node = insert_button(&mut runtime, [120.0, 40.0]);
        if let Some(mut scene_node) = runtime.nodes.get_mut(node)
            && let SceneNodeData::UiButton(button) = &mut scene_node.data
        {
            button.style.set_corner_radius(1.0);
            button.hover_style.set_corner_radius(1.0);
        }

        runtime.extract_render_ui_commands();
        runtime.drain_render_commands(&mut Vec::new());
        runtime.clear_dirty_flags();

        runtime.set_mouse_position(341.0, 281.0);
        runtime.extract_render_ui_commands();
        let mut commands = Vec::new();
        runtime.drain_render_commands(&mut commands);
        assert!(!commands.iter().any(|cmd| matches!(
            cmd,
            RenderCommand::Ui(UiCommand::UpsertButton { node: n, fill, .. })
                if *n == node && *fill == rgba(0.2, 0.3, 0.4, 1.0)
        )));
        assert_eq!(runtime.take_cursor_icon_request(), None);

        runtime.clear_dirty_flags();
        runtime.set_mouse_position(400.0, 300.0);
        runtime.extract_render_ui_commands();
        commands.clear();
        runtime.drain_render_commands(&mut commands);
        assert!(commands.iter().any(|cmd| matches!(
            cmd,
            RenderCommand::Ui(UiCommand::UpsertButton { node: n, fill, .. })
                if *n == node && *fill == rgba(0.2, 0.3, 0.4, 1.0)
        )));
    }

    #[test]
    fn text_box_focus_accepts_committed_text_and_backspace() {
        let mut runtime = Runtime::new();
        runtime.set_viewport_size(800, 600);
        let mut text_box = perro_ui::UiTextBox::new();
        text_box.inner.base.layout.size = UiVector2::pixels(200.0, 40.0);
        let node = insert_ui_node(&mut runtime, SceneNodeData::UiTextBox(Box::new(text_box)));

        runtime.extract_render_ui_commands();
        runtime.drain_render_commands(&mut Vec::new());
        runtime.clear_dirty_flags();

        runtime.set_mouse_position(400.0, 300.0);
        runtime.set_mouse_button_state(MouseButton::Left, true);
        runtime.extract_render_ui_commands();
        runtime.clear_dirty_flags();
        runtime.set_mouse_button_state(MouseButton::Left, false);
        runtime.begin_input_frame();

        runtime.push_text_input("abc");
        runtime.extract_render_ui_commands();
        let text = runtime.nodes.get(node).and_then(|scene_node| {
            if let SceneNodeData::UiTextBox(text_box) = &scene_node.data {
                Some(text_box.inner.text.as_ref())
            } else {
                None
            }
        });
        assert_eq!(text, Some("abc"));

        runtime.clear_dirty_flags();
        runtime.begin_input_frame();
        runtime.set_key_state(KeyCode::Backspace, true);
        runtime.extract_render_ui_commands();
        let text = runtime.nodes.get(node).and_then(|scene_node| {
            if let SceneNodeData::UiTextBox(text_box) = &scene_node.data {
                Some(text_box.inner.text.as_ref())
            } else {
                None
            }
        });
        assert_eq!(text, Some("ab"));
    }

    #[test]
    fn text_box_input_type_filters_text() {
        let mut edit = perro_ui::UiTextEdit::new(false);
        edit.input_type = perro_ui::UiTextInputType::SignedFloat;
        assert!(insert_text_input(&mut edit, "a-1.2.3b"));
        assert_eq!(edit.text.as_ref(), "-1.23");

        edit.set_text("");
        edit.input_type = perro_ui::UiTextInputType::UnsignedFloat;
        assert!(insert_text_input(&mut edit, "-1.5x"));
        assert_eq!(edit.text.as_ref(), "1.5");

        edit.set_text("");
        edit.input_type = perro_ui::UiTextInputType::SignedInteger;
        assert!(insert_text_input(&mut edit, "x-12.7"));
        assert_eq!(edit.text.as_ref(), "-127");

        edit.set_text("");
        edit.input_type = perro_ui::UiTextInputType::UnsignedInteger;
        assert!(insert_text_input(&mut edit, "-12.7"));
        assert_eq!(edit.text.as_ref(), "127");

        edit.set_text("");
        edit.input_type = perro_ui::UiTextInputType::Letters;
        assert!(insert_text_input(&mut edit, "abc123_D"));
        assert_eq!(edit.text.as_ref(), "abcD");
    }

    #[test]
    fn text_box_ctrl_shortcut_does_not_insert_key_text() {
        let mut runtime = Runtime::new();
        runtime.set_viewport_size(800, 600);
        let mut text_box = perro_ui::UiTextBox::new();
        text_box.inner.base.layout.size = UiVector2::pixels(200.0, 40.0);
        let node = insert_ui_node(&mut runtime, SceneNodeData::UiTextBox(Box::new(text_box)));

        runtime.extract_render_ui_commands();
        runtime.drain_render_commands(&mut Vec::new());
        runtime.clear_dirty_flags();

        runtime.set_mouse_position(400.0, 300.0);
        runtime.set_mouse_button_state(MouseButton::Left, true);
        runtime.extract_render_ui_commands();
        runtime.clear_dirty_flags();
        runtime.set_mouse_button_state(MouseButton::Left, false);
        runtime.begin_input_frame();

        runtime.set_key_state(KeyCode::ControlLeft, true);
        runtime.set_key_state(KeyCode::KeyA, true);
        runtime.push_text_input("a");
        runtime.extract_render_ui_commands();

        let text = runtime.nodes.get(node).and_then(|scene_node| {
            if let SceneNodeData::UiTextBox(text_box) = &scene_node.data {
                Some(text_box.inner.text.as_ref())
            } else {
                None
            }
        });
        assert_eq!(text, Some(""));
    }

    #[test]
    fn held_backspace_repeats_in_text_box() {
        let mut runtime = Runtime::new();
        runtime.set_viewport_size(800, 600);
        let mut text_box = perro_ui::UiTextBox::new();
        text_box.inner.base.layout.size = UiVector2::pixels(200.0, 40.0);
        text_box.inner.text = Cow::Borrowed("abcd");
        text_box.inner.caret = 4;
        text_box.inner.anchor = 4;
        let node = insert_ui_node(&mut runtime, SceneNodeData::UiTextBox(Box::new(text_box)));

        runtime.extract_render_ui_commands();
        runtime.drain_render_commands(&mut Vec::new());
        runtime.clear_dirty_flags();

        runtime.set_mouse_position(400.0, 300.0);
        runtime.set_mouse_button_state(MouseButton::Left, true);
        runtime.extract_render_ui_commands();
        runtime.clear_dirty_flags();
        runtime.set_mouse_button_state(MouseButton::Left, false);
        runtime.begin_input_frame();

        runtime.set_key_state(KeyCode::Backspace, true);
        runtime.extract_render_ui_commands();
        runtime.clear_dirty_flags();
        runtime.begin_input_frame();
        runtime.update(0.36);
        runtime.extract_render_ui_commands();

        let text = runtime.nodes.get(node).and_then(|scene_node| {
            if let SceneNodeData::UiTextBox(text_box) = &scene_node.data {
                Some(text_box.inner.text.as_ref())
            } else {
                None
            }
        });
        assert_eq!(text, Some("ab"));
    }

    #[test]
    fn tab_cycles_focus_by_visual_order() {
        let mut runtime = Runtime::new();
        runtime.set_viewport_size(800, 600);
        let top = insert_button_at(&mut runtime, [120.0, 40.0], 0.0, 120.0);
        let middle = insert_text_box_at(&mut runtime, 0.0, 0.0);
        let bottom = insert_text_block_at(&mut runtime, 0.0, -120.0);

        runtime.extract_render_ui_commands();
        runtime.drain_render_commands(&mut Vec::new());
        runtime.clear_dirty_flags();

        tap_key_and_extract(&mut runtime, KeyCode::Tab);
        assert_eq!(runtime.render_ui.focused_ui_node, Some(top));

        tap_key_and_extract(&mut runtime, KeyCode::Tab);
        assert_eq!(runtime.render_ui.focused_ui_node, Some(middle));

        tap_key_and_extract(&mut runtime, KeyCode::Tab);
        assert_eq!(runtime.render_ui.focused_ui_node, Some(bottom));

        tap_key_and_extract(&mut runtime, KeyCode::Tab);
        assert_eq!(runtime.render_ui.focused_ui_node, Some(top));
    }

    #[test]
    fn shift_tab_cycles_focus_reverse() {
        let mut runtime = Runtime::new();
        runtime.set_viewport_size(800, 600);
        let top = insert_button_at(&mut runtime, [120.0, 40.0], 0.0, 120.0);
        let middle = insert_text_box_at(&mut runtime, 0.0, 0.0);
        let bottom = insert_text_block_at(&mut runtime, 0.0, -120.0);

        runtime.extract_render_ui_commands();
        runtime.drain_render_commands(&mut Vec::new());
        runtime.clear_dirty_flags();

        runtime.set_key_state(KeyCode::ShiftLeft, true);
        tap_key_and_extract(&mut runtime, KeyCode::Tab);
        assert_eq!(runtime.render_ui.focused_ui_node, Some(bottom));

        tap_key_and_extract(&mut runtime, KeyCode::Tab);
        assert_eq!(runtime.render_ui.focused_ui_node, Some(middle));

        tap_key_and_extract(&mut runtime, KeyCode::Tab);
        assert_eq!(runtime.render_ui.focused_ui_node, Some(top));
    }

    #[test]
    fn focus_nav_skips_hidden_disabled_and_input_disabled_controls() {
        let mut runtime = Runtime::new();
        runtime.set_viewport_size(800, 600);
        let hidden = insert_button_at(&mut runtime, [120.0, 40.0], 0.0, 180.0);
        let disabled = insert_button_at(&mut runtime, [120.0, 40.0], 0.0, 60.0);
        let input_disabled = insert_text_box_at(&mut runtime, 0.0, -60.0);
        let active = insert_button_at(&mut runtime, [120.0, 40.0], 0.0, -180.0);

        if let Some(mut scene_node) = runtime.nodes.get_mut(hidden)
            && let SceneNodeData::UiButton(button) = &mut scene_node.data
        {
            button.visible = false;
        }
        if let Some(mut scene_node) = runtime.nodes.get_mut(disabled)
            && let SceneNodeData::UiButton(button) = &mut scene_node.data
        {
            button.disabled = true;
        }
        if let Some(mut scene_node) = runtime.nodes.get_mut(input_disabled)
            && let SceneNodeData::UiTextBox(text_box) = &mut scene_node.data
        {
            text_box.inner.base.input_enabled = false;
        }

        runtime.extract_render_ui_commands();
        runtime.drain_render_commands(&mut Vec::new());
        runtime.clear_dirty_flags();

        tap_key_and_extract(&mut runtime, KeyCode::Tab);
        assert_eq!(runtime.render_ui.focused_ui_node, Some(active));
    }

    #[test]
    fn mouse_click_moves_focus_between_text_button_and_empty_space() {
        let mut runtime = Runtime::new();
        runtime.set_viewport_size(800, 600);
        let text = insert_text_box_at(&mut runtime, -160.0, 0.0);
        let button = insert_button_at(&mut runtime, [120.0, 40.0], 160.0, 0.0);

        runtime.extract_render_ui_commands();
        runtime.drain_render_commands(&mut Vec::new());
        runtime.clear_dirty_flags();

        click_mouse_and_extract(&mut runtime, 240.0, 300.0);
        assert_eq!(runtime.render_ui.focused_ui_node, Some(text));
        assert_eq!(runtime.render_ui.focused_text_edit, Some(text));

        click_mouse_and_extract(&mut runtime, 560.0, 300.0);
        assert_eq!(runtime.render_ui.focused_ui_node, Some(button));
        assert_eq!(runtime.render_ui.focused_text_edit, None);

        click_mouse_and_extract(&mut runtime, 20.0, 20.0);
        assert_eq!(runtime.render_ui.focused_ui_node, None);
    }

    #[test]
    fn clipped_text_edit_hit_area_does_not_block_visible_text_edit() {
        let mut runtime = Runtime::new();
        runtime.set_viewport_size(800, 600);
        let low = insert_text_box_at(&mut runtime, 0.0, 0.0);

        let mut clip_parent = perro_ui::UiPanel::new();
        clip_parent.layout.size = UiVector2::pixels(20.0, 20.0);
        clip_parent.layout.z_index = 10;
        clip_parent.clip_children = true;
        let clip_parent =
            insert_ui_node(&mut runtime, SceneNodeData::UiPanel(Box::new(clip_parent)));

        let high = insert_text_box_at(&mut runtime, 0.0, 0.0);
        attach_child(&mut runtime, clip_parent, high);

        runtime.extract_render_ui_commands();
        runtime.drain_render_commands(&mut Vec::new());
        runtime.clear_dirty_flags();

        click_mouse_and_extract(&mut runtime, 450.0, 300.0);

        assert_eq!(runtime.render_ui.focused_ui_node, Some(low));
        assert_eq!(runtime.render_ui.focused_text_edit, Some(low));
    }

    #[test]
    fn clipped_button_hit_area_does_not_block_visible_button() {
        let mut runtime = Runtime::new();
        runtime.set_viewport_size(800, 600);
        let low = insert_button(&mut runtime, [120.0, 40.0]);

        let mut clip_parent = perro_ui::UiPanel::new();
        clip_parent.layout.size = UiVector2::pixels(20.0, 20.0);
        clip_parent.layout.z_index = 10;
        clip_parent.clip_children = true;
        let clip_parent =
            insert_ui_node(&mut runtime, SceneNodeData::UiPanel(Box::new(clip_parent)));

        let high = insert_button(&mut runtime, [120.0, 40.0]);
        attach_child(&mut runtime, clip_parent, high);

        runtime.extract_render_ui_commands();
        runtime.drain_render_commands(&mut Vec::new());
        runtime.clear_dirty_flags();

        click_mouse_and_extract(&mut runtime, 450.0, 300.0);

        assert_eq!(runtime.render_ui.focused_ui_node, Some(low));
        assert_eq!(
            runtime.render_ui.button_states.get(&low).copied(),
            Some(UiButtonVisualState::Pressed)
        );
        assert_ne!(
            runtime.render_ui.button_states.get(&high).copied(),
            Some(UiButtonVisualState::Pressed)
        );
    }

    #[test]
    fn clipped_image_button_hit_area_does_not_hover() {
        let mut runtime = Runtime::new();
        runtime.set_viewport_size(800, 600);

        let mut clip_parent = perro_ui::UiPanel::new();
        clip_parent.layout.size = UiVector2::pixels(20.0, 20.0);
        clip_parent.clip_children = true;
        let clip_parent =
            insert_ui_node(&mut runtime, SceneNodeData::UiPanel(Box::new(clip_parent)));

        let mut button = perro_ui::UiImageButton::new();
        button.layout.size = UiVector2::pixels(120.0, 40.0);
        button.texture = TextureID::from_parts(99, 0);
        let button = insert_ui_node(&mut runtime, SceneNodeData::UiImageButton(Box::new(button)));
        attach_child(&mut runtime, clip_parent, button);

        runtime.extract_render_ui_commands();
        runtime.drain_render_commands(&mut Vec::new());
        runtime.clear_dirty_flags();

        runtime.begin_input_frame();
        runtime.set_mouse_position(450.0, 300.0);
        runtime.extract_render_ui_commands();

        assert_ne!(
            runtime.render_ui.button_states.get(&button).copied(),
            Some(UiButtonVisualState::Hover)
        );
    }

    #[test]
    fn panel_over_scroll_vlayout_button_blocks_click() {
        let mut runtime = Runtime::new();
        runtime.set_viewport_size(800, 600);

        let mut scroller = UiScrollContainer::new();
        scroller.layout.size = UiVector2::pixels(220.0, 120.0);
        let scroller = insert_ui_node(
            &mut runtime,
            SceneNodeData::UiScrollContainer(Box::new(scroller)),
        );

        let mut list = UiVLayout::new();
        list.layout.size = UiVector2::pixels(220.0, 120.0);
        let list = insert_ui_node(&mut runtime, list.into());
        attach_child(&mut runtime, scroller, list);

        let button = insert_button(&mut runtime, [140.0, 44.0]);
        attach_child(&mut runtime, list, button);

        let mut panel = UiPanel::new();
        panel.layout.size = UiVector2::pixels(180.0, 90.0);
        panel.layout.z_index = 20;
        let panel = insert_ui_node(&mut runtime, SceneNodeData::UiPanel(Box::new(panel)));

        runtime.extract_render_ui_commands();
        runtime.drain_render_commands(&mut Vec::new());
        runtime.clear_dirty_flags();

        click_mouse_and_extract(&mut runtime, 400.0, 300.0);

        assert_eq!(runtime.render_ui.focused_ui_node, None);
        assert_ne!(
            runtime.render_ui.button_states.get(&button).copied(),
            Some(UiButtonVisualState::Pressed)
        );
        assert!(runtime.render_ui.computed_rects[&panel].contains(Vector2::ZERO));
    }

    #[test]
    fn gamepad_dpad_picks_nearest_directional_focus() {
        let mut runtime = Runtime::new();
        runtime.set_viewport_size(800, 600);
        let current = insert_button_at(&mut runtime, [120.0, 40.0], 0.0, 0.0);
        let right = insert_button_at(&mut runtime, [120.0, 40.0], 160.0, 0.0);
        let up = insert_button_at(&mut runtime, [120.0, 40.0], 0.0, 160.0);

        runtime.extract_render_ui_commands();
        runtime.drain_render_commands(&mut Vec::new());
        runtime.clear_dirty_flags();

        click_mouse_and_extract(&mut runtime, 400.0, 300.0);
        assert_eq!(runtime.render_ui.focused_ui_node, Some(current));

        runtime.set_gamepad_button_state(0, GamepadButton::DpadRight, true);
        runtime.extract_render_ui_commands();
        assert_eq!(runtime.render_ui.focused_ui_node, Some(right));

        runtime.set_gamepad_button_state(0, GamepadButton::DpadRight, false);
        runtime.begin_input_frame();
        runtime.set_gamepad_button_state(0, GamepadButton::DpadUp, true);
        runtime.extract_render_ui_commands();
        assert_eq!(runtime.render_ui.focused_ui_node, Some(up));
    }

    #[test]
    fn gamepad_stick_nav_repeats_after_delay() {
        let mut runtime = Runtime::new();
        runtime.set_viewport_size(800, 600);
        let current = insert_button_at(&mut runtime, [120.0, 40.0], -160.0, 0.0);
        let mid = insert_button_at(&mut runtime, [120.0, 40.0], 0.0, 0.0);
        let right = insert_button_at(&mut runtime, [120.0, 40.0], 160.0, 0.0);

        runtime.extract_render_ui_commands();
        runtime.drain_render_commands(&mut Vec::new());
        runtime.clear_dirty_flags();

        click_mouse_and_extract(&mut runtime, 240.0, 300.0);
        assert_eq!(runtime.render_ui.focused_ui_node, Some(current));

        runtime.set_gamepad_axis(0, GamepadAxis::LeftStickX, 1.0);
        runtime.extract_render_ui_commands();
        assert_eq!(runtime.render_ui.focused_ui_node, Some(mid));

        runtime.begin_input_frame();
        runtime.update(0.36);
        runtime.extract_render_ui_commands();
        assert_eq!(runtime.render_ui.focused_ui_node, Some(right));
    }

    #[test]
    fn joycon_stick_drives_directional_focus() {
        let mut runtime = Runtime::new();
        runtime.set_viewport_size(800, 600);
        let current = insert_button_at(&mut runtime, [120.0, 40.0], 0.0, 0.0);
        let up = insert_button_at(&mut runtime, [120.0, 40.0], 0.0, 160.0);

        runtime.extract_render_ui_commands();
        runtime.drain_render_commands(&mut Vec::new());
        runtime.clear_dirty_flags();

        click_mouse_and_extract(&mut runtime, 400.0, 300.0);
        assert_eq!(runtime.render_ui.focused_ui_node, Some(current));

        runtime.set_joycon_stick(0, 0.0, 1.0);
        runtime.extract_render_ui_commands();
        assert_eq!(runtime.render_ui.focused_ui_node, Some(up));
    }

    #[test]
    fn ui_input_mask_filters_player_nav_sources() {
        let mut runtime = Runtime::new();
        runtime.set_viewport_size(800, 600);
        runtime.bind_player(0, PlayerBinding::Gamepad { index: 0 });
        runtime.bind_player(1, PlayerBinding::Gamepad { index: 1 });
        let button = insert_button_at(&mut runtime, [120.0, 40.0], 0.0, 0.0);
        if let Some(mut scene_node) = runtime.nodes.get_mut(button)
            && let SceneNodeData::UiButton(button) = &mut scene_node.data
        {
            button.input_mask.allow_players.push(0);
        }

        runtime.extract_render_ui_commands();
        runtime.drain_render_commands(&mut Vec::new());
        runtime.clear_dirty_flags();

        runtime.set_gamepad_button_state(1, GamepadButton::DpadRight, true);
        runtime.extract_render_ui_commands();
        assert_eq!(runtime.render_ui.focused_ui_node, None);

        runtime.set_gamepad_button_state(1, GamepadButton::DpadRight, false);
        runtime.begin_input_frame();
        runtime.set_gamepad_button_state(0, GamepadButton::DpadRight, true);
        runtime.extract_render_ui_commands();
        assert_eq!(runtime.render_ui.focused_ui_node, Some(button));
    }

    #[test]
    fn ui_input_mask_filters_device_directional_targets() {
        let mut runtime = Runtime::new();
        runtime.set_viewport_size(800, 600);
        let left = insert_button_at(&mut runtime, [120.0, 40.0], -160.0, 0.0);
        let right = insert_button_at(&mut runtime, [120.0, 40.0], 160.0, 0.0);
        if let Some(mut scene_node) = runtime.nodes.get_mut(right)
            && let SceneNodeData::UiButton(button) = &mut scene_node.data
        {
            button.input_mask.deny_gamepads.push(1);
        }

        runtime.extract_render_ui_commands();
        runtime.drain_render_commands(&mut Vec::new());
        runtime.clear_dirty_flags();

        click_mouse_and_extract(&mut runtime, 240.0, 300.0);
        assert_eq!(runtime.render_ui.focused_ui_node, Some(left));

        runtime.set_gamepad_button_state(1, GamepadButton::DpadRight, true);
        runtime.extract_render_ui_commands();
        assert_eq!(runtime.render_ui.focused_ui_node, Some(left));

        runtime.set_gamepad_button_state(1, GamepadButton::DpadRight, false);
        runtime.begin_input_frame();
        runtime.set_gamepad_button_state(0, GamepadButton::DpadRight, true);
        runtime.extract_render_ui_commands();
        assert_eq!(runtime.render_ui.focused_ui_node, Some(right));
    }
}
