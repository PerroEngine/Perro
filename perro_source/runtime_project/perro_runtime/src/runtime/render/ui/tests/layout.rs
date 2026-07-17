mod layout {
    use super::*;

    #[test]
    fn ui_input_mask_filters_mouse_joycon_and_button_activation_sources() {
        let mut runtime = Runtime::new();
        runtime.set_viewport_size(800, 600);
        let button = insert_button_at(&mut runtime, [120.0, 40.0], 0.0, 0.0);
        if let Some(mut scene_node) = runtime.nodes.get_mut(button)
            && let SceneNodeData::UiButton(button) = &mut scene_node.data
        {
            button.input_mask.allow_joycons.push(1);
        }

        runtime.extract_render_ui_commands();
        runtime.drain_render_commands(&mut Vec::new());
        runtime.clear_dirty_flags();

        click_mouse_and_extract(&mut runtime, 400.0, 300.0);
        assert_eq!(runtime.render_ui.focused_ui_node, None);

        runtime.set_joycon_stick(0, 1.0, 0.0);
        runtime.extract_render_ui_commands();
        assert_eq!(runtime.render_ui.focused_ui_node, None);

        runtime.set_joycon_stick(0, 0.0, 0.0);
        runtime.begin_input_frame();
        runtime.extract_render_ui_commands();
        runtime.begin_input_frame();
        runtime.set_joycon_stick(1, 1.0, 0.0);
        runtime.extract_render_ui_commands();
        assert_eq!(runtime.render_ui.focused_ui_node, Some(button));

        runtime.begin_input_frame();
        runtime.set_joycon_button_state(0, JoyConButton::Right, true);
        runtime.extract_render_ui_commands();
        assert_ne!(
            runtime.render_ui.button_states.get(&button).copied(),
            Some(UiButtonVisualState::Pressed)
        );

        runtime.set_joycon_button_state(0, JoyConButton::Right, false);
        runtime.begin_input_frame();
        runtime.set_joycon_button_state(1, JoyConButton::Right, true);
        runtime.extract_render_ui_commands();
        assert_eq!(
            runtime.render_ui.button_states.get(&button).copied(),
            Some(UiButtonVisualState::Pressed)
        );
    }

    #[test]
    fn focused_button_activates_from_keyboard_gamepad_and_joycon() {
        let mut runtime = Runtime::new();
        runtime.set_viewport_size(800, 600);
        let button = insert_button_at(&mut runtime, [120.0, 40.0], 0.0, 0.0);

        runtime.extract_render_ui_commands();
        runtime.drain_render_commands(&mut Vec::new());
        runtime.clear_dirty_flags();

        tap_key_and_extract(&mut runtime, KeyCode::Tab);
        assert_eq!(runtime.render_ui.focused_ui_node, Some(button));

        runtime.set_key_state(KeyCode::Space, true);
        runtime.extract_render_ui_commands();
        assert_eq!(
            runtime.render_ui.button_states.get(&button).copied(),
            Some(UiButtonVisualState::Pressed)
        );
        runtime.set_key_state(KeyCode::Space, false);
        runtime.extract_render_ui_commands();
        assert_eq!(
            runtime.render_ui.button_states.get(&button).copied(),
            Some(UiButtonVisualState::Hover)
        );

        runtime.begin_input_frame();
        runtime.set_gamepad_button_state(0, GamepadButton::Bottom, true);
        runtime.extract_render_ui_commands();
        assert_eq!(
            runtime.render_ui.button_states.get(&button).copied(),
            Some(UiButtonVisualState::Pressed)
        );
        runtime.set_gamepad_button_state(0, GamepadButton::Bottom, false);
        runtime.extract_render_ui_commands();
        assert_eq!(
            runtime.render_ui.button_states.get(&button).copied(),
            Some(UiButtonVisualState::Hover)
        );

        runtime.begin_input_frame();
        runtime.set_joycon_button_state(0, JoyConButton::Right, true);
        runtime.extract_render_ui_commands();
        assert_eq!(
            runtime.render_ui.button_states.get(&button).copied(),
            Some(UiButtonVisualState::Pressed)
        );
        runtime.set_joycon_button_state(0, JoyConButton::Right, false);
        runtime.extract_render_ui_commands();
        assert_eq!(
            runtime.render_ui.button_states.get(&button).copied(),
            Some(UiButtonVisualState::Hover)
        );
    }

    #[test]
    fn button_state_base_overrides_rect_transform() {
        let mut runtime = Runtime::new();
        runtime.set_viewport_size(800, 600);

        let mut button = perro_ui::UiButton::new();
        button.layout.size = UiVector2::pixels(120.0, 40.0);
        let mut hover_base = button.base.clone();
        hover_base.layout.size = UiVector2::pixels(150.0, 48.0);
        hover_base.transform.translation = Vector2::new(6.0, -3.0);
        hover_base.transform.rotation = 0.25;
        button.hover_base = Some(hover_base);
        button.hover_size_override = true;
        let node = insert_ui_node(&mut runtime, SceneNodeData::UiButton(Box::new(button)));

        runtime.extract_render_ui_commands();
        let mut commands = Vec::new();
        runtime.drain_render_commands(&mut commands);
        runtime.clear_dirty_flags();

        runtime.set_mouse_position(400.0, 300.0);
        runtime.extract_render_ui_commands();
        commands.clear();
        runtime.drain_render_commands(&mut commands);

        assert!(commands.iter().any(|cmd| matches!(
            cmd,
            RenderCommand::Ui(UiCommand::UpsertButton { node: n, rect, .. })
                if *n == node
                    && rect.center == [6.0, -3.0]
                    && rect.size == [150.0, 48.0]
                    && rect.rotation_radians == 0.25
        )));
    }

    #[test]
    fn button_hover_style_without_size_override_keeps_base_size() {
        let mut runtime = Runtime::new();
        runtime.set_viewport_size(800, 600);
        let mut button = perro_ui::UiButton::new();
        button.layout.size = UiVector2::pixels(120.0, 40.0);
        button.hover_style.fill = Color::new(0.3, 0.4, 0.5, 1.0);
        let mut hover_base = button.base.clone();
        hover_base.transform.translation = Vector2::new(8.0, 0.0);
        button.hover_base = Some(hover_base);
        let node = insert_ui_node(&mut runtime, SceneNodeData::UiButton(Box::new(button)));

        runtime.extract_render_ui_commands();
        runtime.drain_render_commands(&mut Vec::new());
        runtime.clear_dirty_flags();
        runtime.set_mouse_position(400.0, 300.0);
        runtime.extract_render_ui_commands();
        let mut commands = Vec::new();
        runtime.drain_render_commands(&mut commands);

        assert!(commands.iter().any(|cmd| matches!(
            cmd,
            RenderCommand::Ui(UiCommand::UpsertButton { node: id, rect, fill, .. })
                if *id == node
                    && rect.size == [120.0, 40.0]
                    && *fill == rgba(0.3, 0.4, 0.5, 1.0)
        )));
    }

    #[test]
    fn button_event_signals_include_named_and_custom_signals() {
        let mut runtime = Runtime::new();
        let named = insert_button(&mut runtime, [120.0, 40.0]);
        runtime.nodes.get_mut(named).expect("named button").name = Cow::Borrowed("play");
        assert_eq!(
            runtime.button_event_signals(named, "click"),
            vec![SignalID::from_string("play_clicked")]
        );

        let mut button = perro_ui::UiButton::new();
        button
            .pressed_signals
            .push(SignalID::from_string("custom_a"));
        button
            .pressed_signals
            .push(SignalID::from_string("custom_b"));
        let custom = insert_ui_node(&mut runtime, SceneNodeData::UiButton(Box::new(button)));
        runtime.nodes.get_mut(custom).expect("custom button").name = Cow::Borrowed("fire");
        assert_eq!(
            runtime.button_event_signals(custom, "pressed"),
            vec![
                SignalID::from_string("fire_pressed"),
                SignalID::from_string("custom_a"),
                SignalID::from_string("custom_b"),
            ]
        );
    }

    #[test]
    fn image_button_event_signals_include_named_and_custom_signals() {
        let mut runtime = Runtime::new();
        let mut button = perro_ui::UiImageButton::new();
        button
            .clicked_signals
            .push(SignalID::from_string("custom_click"));
        let node = insert_ui_node(&mut runtime, SceneNodeData::UiImageButton(Box::new(button)));
        runtime.nodes.get_mut(node).expect("image button").name = Cow::Borrowed("icon");

        assert_eq!(
            runtime.button_event_signals(node, "click"),
            vec![
                SignalID::from_string("icon_clicked"),
                SignalID::from_string("custom_click"),
            ]
        );
    }

    #[test]
    fn disabled_button_event_signals_empty() {
        let mut runtime = Runtime::new();
        let node = insert_button(&mut runtime, [120.0, 40.0]);
        if let Some(mut scene_node) = runtime.nodes.get_mut(node)
            && let SceneNodeData::UiButton(button) = &mut scene_node.data
        {
            button.disabled = true;
            button
                .hover_signals
                .push(SignalID::from_string("custom_hover"));
            button
                .pressed_signals
                .push(SignalID::from_string("custom_press"));
            button
                .clicked_signals
                .push(SignalID::from_string("custom_click"));
        }
        runtime.nodes.get_mut(node).expect("named button").name = Cow::Borrowed("play");

        assert!(runtime.button_event_signals(node, "hover_enter").is_empty());
        assert!(runtime.button_event_signals(node, "pressed").is_empty());
        assert!(runtime.button_event_signals(node, "click").is_empty());
    }

    #[test]
    fn input_disabled_button_event_signals_empty() {
        let mut runtime = Runtime::new();
        let node = insert_button(&mut runtime, [120.0, 40.0]);
        if let Some(mut scene_node) = runtime.nodes.get_mut(node)
            && let SceneNodeData::UiButton(button) = &mut scene_node.data
        {
            button.input_enabled = false;
            button
                .hover_signals
                .push(SignalID::from_string("custom_hover"));
            button
                .pressed_signals
                .push(SignalID::from_string("custom_press"));
            button
                .clicked_signals
                .push(SignalID::from_string("custom_click"));
        }
        runtime.nodes.get_mut(node).expect("named button").name = Cow::Borrowed("play");

        assert!(runtime.button_event_signals(node, "hover_enter").is_empty());
        assert!(runtime.button_event_signals(node, "pressed").is_empty());
        assert!(runtime.button_event_signals(node, "click").is_empty());
    }

    #[test]
    fn button_click_signal_defers_script_mutation_until_after_ui_extraction() {
        let mut runtime = Runtime::new();
        runtime.set_viewport_size(800, 600);
        let button = insert_button_at(&mut runtime, [120.0, 40.0], 0.0, 0.0);
        runtime.nodes.get_mut(button).expect("button").name = Cow::Borrowed("play");

        let calls = Arc::new(AtomicUsize::new(0));
        let script_id = runtime.create::<Node3D>();
        runtime.scripts.insert(
            script_id,
            Arc::new(HideClickedButtonScript {
                calls: Arc::clone(&calls),
            }),
            Box::new(()),
        );
        assert!(runtime.signal_connect(
            script_id,
            SignalID::from_string("play_clicked"),
            ScriptMemberID::from_string("on_click"),
            &[]
        ));

        runtime.extract_render_ui_commands();
        runtime.drain_render_commands(&mut Vec::new());
        runtime.clear_dirty_flags();

        runtime.begin_input_frame();
        runtime.set_mouse_position(400.0, 300.0);
        runtime.set_mouse_button_state(MouseButton::Left, true);
        runtime.extract_render_ui_commands();
        runtime.drain_render_commands(&mut Vec::new());
        runtime.clear_dirty_flags();

        runtime.begin_input_frame();
        runtime.set_mouse_button_state(MouseButton::Left, false);
        runtime.extract_render_ui_commands();

        assert_eq!(calls.load(Ordering::Relaxed), 0);

        runtime.drain_render_commands(&mut Vec::new());
        runtime.clear_dirty_flags();
        runtime.update(0.016);

        assert_eq!(calls.load(Ordering::Relaxed), 1);
        assert!(
            runtime
                .nodes
                .get(button)
                .and_then(|node| match &node.data {
                    SceneNodeData::UiButton(button) => Some(button.visible),
                    _ => None,
                })
                .is_some_and(|visible| !visible)
        );

        let mut commands = Vec::new();
        runtime.drain_render_commands(&mut commands);
        assert!(commands.is_empty());

        runtime.extract_render_ui_commands();
        commands.clear();
        runtime.drain_render_commands(&mut commands);
        assert!(commands.iter().any(|cmd| matches!(
            cmd,
            RenderCommand::Ui(UiCommand::RemoveNode { node }) if *node == button
        )));
    }

    #[test]
    fn text_edit_event_signals_include_named_and_custom_signals() {
        let mut runtime = Runtime::new();
        let named = insert_ui_node(
            &mut runtime,
            SceneNodeData::UiTextBox(Box::new(perro_ui::UiTextBox::new())),
        );
        runtime.nodes.get_mut(named).expect("named text box").name = Cow::Borrowed("name");
        assert_eq!(
            runtime.text_edit_event_signals(named, "focused"),
            vec![SignalID::from_string("name_focused")]
        );
        assert_eq!(
            runtime.text_edit_event_signals(named, "text_changed"),
            vec![SignalID::from_string("name_text_changed")]
        );

        let mut text_block = perro_ui::UiTextBlock::new();
        text_block
            .inner
            .hover_signals
            .push(SignalID::from_string("custom_hover"));
        text_block
            .inner
            .text_changed_signals
            .push(SignalID::from_string("custom_text"));
        let custom = insert_ui_node(
            &mut runtime,
            SceneNodeData::UiTextBlock(Box::new(text_block)),
        );
        runtime
            .nodes
            .get_mut(custom)
            .expect("custom text block")
            .name = Cow::Borrowed("bio");
        assert_eq!(
            runtime.text_edit_event_signals(custom, "hovered"),
            vec![
                SignalID::from_string("bio_hovered"),
                SignalID::from_string("custom_hover"),
            ]
        );
        assert_eq!(
            runtime.text_edit_event_signals(custom, "text_changed"),
            vec![
                SignalID::from_string("bio_text_changed"),
                SignalID::from_string("custom_text"),
            ]
        );
    }

    #[test]
    fn default_hlayout_centers_child_group() {
        let mut runtime = Runtime::new();
        runtime.set_viewport_size(800, 600);

        let mut layout = UiHLayout::new();
        layout.layout.size = UiVector2::pixels(300.0, 100.0);
        let parent = insert_ui_node(&mut runtime, SceneNodeData::UiHLayout(layout));
        let child = insert_panel(&mut runtime, [60.0, 40.0], Color::new(0.1, 0.2, 0.3, 1.0));
        attach_child(&mut runtime, parent, child);

        runtime.extract_render_ui_commands();

        let child_rect = runtime
            .render_ui
            .computed_rects
            .get(&child)
            .expect("child rect exists");
        assert_eq!(child_rect.center, Vector2::ZERO);
    }

    #[test]
    fn layout_padding_uses_parent_rect_ratio() {
        let mut runtime = Runtime::new();
        runtime.set_viewport_size(800, 600);

        let mut layout = UiHLayout::new();
        layout.layout.size = UiVector2::pixels(300.0, 100.0);
        layout.layout.padding = perro_ui::UiRect::new(0.1, 0.0, 0.0, 0.0);
        let parent = insert_ui_node(&mut runtime, SceneNodeData::UiHLayout(layout));
        let child = insert_panel(&mut runtime, [60.0, 40.0], Color::new(0.1, 0.2, 0.3, 1.0));
        attach_child(&mut runtime, parent, child);

        runtime.extract_render_ui_commands();

        let child_rect = runtime
            .render_ui
            .computed_rects
            .get(&child)
            .expect("child rect exists");
        assert_eq!(child_rect.center, Vector2::new(15.0, 0.0));
    }

    #[test]
    fn hlayout_ignores_invisible_child_space() {
        let mut runtime = Runtime::new();
        runtime.set_viewport_size(800, 600);

        let mut layout = UiHLayout::new();
        layout.layout.size = UiVector2::pixels(300.0, 100.0);
        layout.inner.spacing = 10.0 / 300.0;
        let parent = insert_ui_node(&mut runtime, SceneNodeData::UiHLayout(layout));
        let first = insert_panel(&mut runtime, [60.0, 40.0], Color::new(0.1, 0.2, 0.3, 1.0));
        let middle = insert_panel(&mut runtime, [60.0, 40.0], Color::new(0.2, 0.3, 0.4, 1.0));
        let last = insert_panel(&mut runtime, [60.0, 40.0], Color::new(0.3, 0.4, 0.5, 1.0));
        attach_child(&mut runtime, parent, first);
        attach_child(&mut runtime, parent, middle);
        attach_child(&mut runtime, parent, last);

        set_panel_visible(&mut runtime, middle, false);
        runtime.mark_ui_dirty(
            middle,
            Runtime::UI_DIRTY_LAYOUT_SELF
                | Runtime::UI_DIRTY_LAYOUT_PARENT
                | Runtime::UI_DIRTY_COMMANDS,
        );
        runtime.extract_render_ui_commands();

        let first_rect = runtime
            .render_ui
            .computed_rects
            .get(&first)
            .expect("first rect exists");
        let last_rect = runtime
            .render_ui
            .computed_rects
            .get(&last)
            .expect("last rect exists");
        assert_eq!(first_rect.center.x, -35.0);
        assert_eq!(last_rect.center.x, 35.0);
    }

    #[test]
    fn hlayout_fill_spacing_spreads_children_to_edges() {
        let mut runtime = Runtime::new();
        runtime.set_viewport_size(800, 600);

        let mut layout = UiHLayout::new();
        layout.layout.size = UiVector2::pixels(300.0, 100.0);
        layout.inner.spacing_mode = UiLayoutSpacingMode::Fill;
        let parent = insert_ui_node(&mut runtime, SceneNodeData::UiHLayout(layout));
        let first = insert_panel(&mut runtime, [60.0, 40.0], Color::new(0.1, 0.2, 0.3, 1.0));
        let middle = insert_panel(&mut runtime, [60.0, 40.0], Color::new(0.2, 0.3, 0.4, 1.0));
        let last = insert_panel(&mut runtime, [60.0, 40.0], Color::new(0.3, 0.4, 0.5, 1.0));
        attach_child(&mut runtime, parent, first);
        attach_child(&mut runtime, parent, middle);
        attach_child(&mut runtime, parent, last);

        runtime.extract_render_ui_commands();

        let first_rect = runtime.render_ui.computed_rects.get(&first).expect("first");
        let middle_rect = runtime
            .render_ui
            .computed_rects
            .get(&middle)
            .expect("middle");
        let last_rect = runtime.render_ui.computed_rects.get(&last).expect("last");
        assert_eq!(first_rect.center.x, -120.0);
        assert_eq!(middle_rect.center.x, 0.0);
        assert_eq!(last_rect.center.x, 120.0);
    }

    #[test]
    fn vlayout_preserves_child_order() {
        let mut runtime = Runtime::new();
        runtime.set_viewport_size(800, 600);

        let mut layout = UiVLayout::new();
        layout.layout.size = UiVector2::pixels(200.0, 180.0);
        layout.inner.spacing = 10.0 / 180.0;
        let parent = insert_ui_node(&mut runtime, SceneNodeData::UiVLayout(layout));
        let first = insert_panel(&mut runtime, [100.0, 40.0], Color::new(0.1, 0.2, 0.3, 1.0));
        let second = insert_panel(&mut runtime, [100.0, 40.0], Color::new(0.2, 0.3, 0.4, 1.0));
        attach_child(&mut runtime, parent, first);
        attach_child(&mut runtime, parent, second);

        runtime.extract_render_ui_commands();

        let first_rect = runtime
            .render_ui
            .computed_rects
            .get(&first)
            .expect("first rect exists");
        let second_rect = runtime
            .render_ui
            .computed_rects
            .get(&second)
            .expect("second rect exists");
        assert!(first_rect.center.y > second_rect.center.y);
    }

    #[test]
    fn vlayout_fill_spacing_spreads_children_to_edges() {
        let mut runtime = Runtime::new();
        runtime.set_viewport_size(800, 600);

        let mut layout = UiVLayout::new();
        layout.layout.size = UiVector2::pixels(200.0, 180.0);
        layout.inner.spacing_mode = UiLayoutSpacingMode::Fill;
        let parent = insert_ui_node(&mut runtime, SceneNodeData::UiVLayout(layout));
        let first = insert_panel(&mut runtime, [100.0, 40.0], Color::new(0.1, 0.2, 0.3, 1.0));
        let second = insert_panel(&mut runtime, [100.0, 40.0], Color::new(0.2, 0.3, 0.4, 1.0));
        attach_child(&mut runtime, parent, first);
        attach_child(&mut runtime, parent, second);

        runtime.extract_render_ui_commands();

        let first_rect = runtime.render_ui.computed_rects.get(&first).expect("first");
        let second_rect = runtime
            .render_ui
            .computed_rects
            .get(&second)
            .expect("second");
        assert_eq!(first_rect.center.y, 70.0);
        assert_eq!(second_rect.center.y, -70.0);
    }

    #[test]
    fn default_grid_centers_rows_in_parent() {
        let mut runtime = Runtime::new();
        runtime.set_viewport_size(800, 600);

        let mut grid = UiGrid::new();
        grid.layout.size = UiVector2::pixels(300.0, 200.0);
        let parent = insert_ui_node(&mut runtime, SceneNodeData::UiGrid(grid));
        let child = insert_panel(&mut runtime, [60.0, 40.0], Color::new(0.1, 0.2, 0.3, 1.0));
        attach_child(&mut runtime, parent, child);

        runtime.extract_render_ui_commands();

        let child_rect = runtime
            .render_ui
            .computed_rects
            .get(&child)
            .expect("child rect exists");
        assert_eq!(child_rect.center, Vector2::ZERO);
    }

    #[test]
    fn grid_columns_auto_wrap_into_centered_rows() {
        let mut runtime = Runtime::new();
        runtime.set_viewport_size(800, 600);

        let mut grid = UiGrid::new();
        grid.layout.size = UiVector2::pixels(300.0, 200.0);
        grid.columns = 3;
        grid.h_spacing = 10.0 / 300.0;
        grid.v_spacing = 10.0 / 200.0;
        let parent = insert_ui_node(&mut runtime, SceneNodeData::UiGrid(grid));

        let mut children = Vec::new();
        for _ in 0..6 {
            let child = insert_panel(&mut runtime, [60.0, 40.0], Color::new(0.1, 0.2, 0.3, 1.0));
            attach_child(&mut runtime, parent, child);
            children.push(child);
        }

        runtime.extract_render_ui_commands();

        let first = runtime
            .render_ui
            .computed_rects
            .get(&children[0])
            .expect("first rect exists");
        let fourth = runtime
            .render_ui
            .computed_rects
            .get(&children[3])
            .expect("fourth rect exists");
        assert_eq!(first.center, Vector2::new(-70.0, 25.0));
        assert_eq!(fourth.center, Vector2::new(-70.0, -25.0));
    }

    #[test]
    fn grid_fill_spacing_spreads_columns_to_edges() {
        let mut runtime = Runtime::new();
        runtime.set_viewport_size(800, 600);

        let mut grid = UiGrid::new();
        grid.layout.size = UiVector2::pixels(300.0, 200.0);
        grid.columns = 3;
        grid.h_spacing_mode = UiLayoutSpacingMode::Fill;
        let parent = insert_ui_node(&mut runtime, SceneNodeData::UiGrid(grid));

        let first = insert_panel(&mut runtime, [60.0, 40.0], Color::new(0.1, 0.2, 0.3, 1.0));
        let middle = insert_panel(&mut runtime, [60.0, 40.0], Color::new(0.2, 0.3, 0.4, 1.0));
        let last = insert_panel(&mut runtime, [60.0, 40.0], Color::new(0.3, 0.4, 0.5, 1.0));
        attach_child(&mut runtime, parent, first);
        attach_child(&mut runtime, parent, middle);
        attach_child(&mut runtime, parent, last);

        runtime.extract_render_ui_commands();

        let first_rect = runtime.render_ui.computed_rects.get(&first).expect("first");
        let middle_rect = runtime
            .render_ui
            .computed_rects
            .get(&middle)
            .expect("middle");
        let last_rect = runtime.render_ui.computed_rects.get(&last).expect("last");
        assert_eq!(first_rect.center.x, -120.0);
        assert_eq!(middle_rect.center.x, 0.0);
        assert_eq!(last_rect.center.x, 120.0);
    }

    #[test]
    fn grid_uses_uniform_cells_for_even_column_spacing() {
        let mut runtime = Runtime::new();
        runtime.set_viewport_size(800, 600);

        let mut grid = UiGrid::new();
        grid.layout.size = UiVector2::pixels(400.0, 200.0);
        grid.columns = 3;
        grid.h_spacing = 10.0 / 400.0;
        let parent = insert_ui_node(&mut runtime, SceneNodeData::UiGrid(grid));

        let first = insert_panel(&mut runtime, [80.0, 40.0], Color::new(0.1, 0.2, 0.3, 1.0));
        let middle = insert_panel(&mut runtime, [60.0, 40.0], Color::new(0.1, 0.2, 0.3, 1.0));
        let last = insert_panel(&mut runtime, [60.0, 40.0], Color::new(0.1, 0.2, 0.3, 1.0));
        attach_child(&mut runtime, parent, first);
        attach_child(&mut runtime, parent, middle);
        attach_child(&mut runtime, parent, last);

        runtime.extract_render_ui_commands();

        let first = runtime
            .render_ui
            .computed_rects
            .get(&first)
            .expect("first rect exists");
        let middle = runtime
            .render_ui
            .computed_rects
            .get(&middle)
            .expect("middle rect exists");
        let last = runtime
            .render_ui
            .computed_rects
            .get(&last)
            .expect("last rect exists");
        assert_eq!(
            middle.center.x - first.center.x,
            last.center.x - middle.center.x
        );
    }

    #[test]
    fn grid_ignores_invisible_child_index() {
        let mut runtime = Runtime::new();
        runtime.set_viewport_size(800, 600);

        let mut grid = UiGrid::new();
        grid.layout.size = UiVector2::pixels(300.0, 200.0);
        grid.columns = 3;
        grid.h_spacing = 10.0 / 300.0;
        grid.v_spacing = 10.0 / 200.0;
        let parent = insert_ui_node(&mut runtime, SceneNodeData::UiGrid(grid));

        let first = insert_panel(&mut runtime, [60.0, 40.0], Color::new(0.1, 0.2, 0.3, 1.0));
        let hidden = insert_panel(&mut runtime, [60.0, 40.0], Color::new(0.2, 0.3, 0.4, 1.0));
        let third = insert_panel(&mut runtime, [60.0, 40.0], Color::new(0.3, 0.4, 0.5, 1.0));
        let fourth = insert_panel(&mut runtime, [60.0, 40.0], Color::new(0.4, 0.5, 0.6, 1.0));
        attach_child(&mut runtime, parent, first);
        attach_child(&mut runtime, parent, hidden);
        attach_child(&mut runtime, parent, third);
        attach_child(&mut runtime, parent, fourth);

        set_panel_visible(&mut runtime, hidden, false);
        runtime.mark_ui_dirty(
            hidden,
            Runtime::UI_DIRTY_LAYOUT_SELF
                | Runtime::UI_DIRTY_LAYOUT_PARENT
                | Runtime::UI_DIRTY_COMMANDS,
        );
        runtime.extract_render_ui_commands();

        let first_rect = runtime
            .render_ui
            .computed_rects
            .get(&first)
            .expect("first rect exists");
        let third_rect = runtime
            .render_ui
            .computed_rects
            .get(&third)
            .expect("third rect exists");
        let fourth_rect = runtime
            .render_ui
            .computed_rects
            .get(&fourth)
            .expect("fourth rect exists");
        assert_eq!(first_rect.center, Vector2::new(-70.0, 0.0));
        assert_eq!(third_rect.center, Vector2::ZERO);
        assert_eq!(fourth_rect.center, Vector2::new(70.0, 0.0));
    }

    #[test]
    fn parent_ui_scale_scales_child_label_font() {
        let mut runtime = Runtime::new();
        runtime.set_viewport_size(800, 600);

        let mut parent_panel = UiPanel::new();
        parent_panel.layout.size = UiVector2::pixels(400.0, 200.0);
        parent_panel.transform.scale = Vector2::new(0.5, 0.5);
        let parent = insert_ui_node(&mut runtime, SceneNodeData::UiPanel(Box::new(parent_panel)));

        let mut label = perro_ui::UiLabel::new().with_text("Scaled");
        label.layout.size = UiVector2::pixels(200.0, 40.0);
        label.font_size = 20.0;
        label.text_size_ratio = 0.0;
        let child = insert_ui_node(&mut runtime, SceneNodeData::UiLabel(Box::new(label)));
        attach_child(&mut runtime, parent, child);

        runtime.extract_render_ui_commands();
        let mut commands = Vec::new();
        runtime.drain_render_commands(&mut commands);

        assert!(commands.iter().any(|cmd| matches!(
            cmd,
            RenderCommand::Ui(UiCommand::UpsertLabel { node, rect, font_size, .. })
                if *node == child && rect.size == [100.0, 20.0] && *font_size == 10.0
        )));
    }

    #[test]
    fn label_relative_font_size_scales_with_virtual_resolution() {
        let mut runtime = Runtime::new();
        runtime.project = Some(std::sync::Arc::new(
            crate::runtime_project::RuntimeProject::new("Test", "."),
        ));
        runtime.set_viewport_size(960, 540);

        let mut label = perro_ui::UiLabel::new().with_text("Scaled");
        label.layout.size = perro_ui::UiVector2::pixels(200.0, 40.0);
        label.font_size = 20.0;
        label.text_size_ratio = 0.0;
        label.font_sizing.relative_to_virtual = true;
        let node = insert_ui_node(&mut runtime, SceneNodeData::UiLabel(Box::new(label)));

        runtime.extract_render_ui_commands();
        let mut commands = Vec::new();
        runtime.drain_render_commands(&mut commands);

        assert!(commands.iter().any(|cmd| matches!(
            cmd,
            RenderCommand::Ui(UiCommand::UpsertLabel { node: id, font_size, .. })
                if *id == node && (*font_size - 10.0).abs() < 1.0e-3
        )));
    }

    #[test]
    fn text_box_relative_font_size_uses_min_and_max_scale_clamp() {
        let mut runtime = Runtime::new();
        runtime.project = Some(std::sync::Arc::new(
            crate::runtime_project::RuntimeProject::new("Test", "."),
        ));
        runtime.set_viewport_size(3840, 2160);

        let mut text_box = perro_ui::UiTextBox::new();
        text_box.inner.font_size = 20.0;
        text_box.inner.text_size_ratio = 0.0;
        text_box.inner.font_sizing.relative_to_virtual = true;
        text_box.inner.font_sizing.min_scale = 0.5;
        text_box.inner.font_sizing.max_scale = 1.5;
        let node = insert_ui_node(&mut runtime, SceneNodeData::UiTextBox(Box::new(text_box)));

        runtime.extract_render_ui_commands();
        let mut commands = Vec::new();
        runtime.drain_render_commands(&mut commands);

        assert!(commands.iter().any(|cmd| matches!(
            cmd,
            RenderCommand::Ui(UiCommand::UpsertTextEdit { node: id, font_size, .. })
                if *id == node && (*font_size - 30.0).abs() < 1.0e-3
        )));
    }

    #[test]
    fn parent_ui_scale_keeps_child_panel_radius_ratio() {
        let mut runtime = Runtime::new();
        runtime.set_viewport_size(800, 600);

        let mut parent_panel = UiPanel::new();
        parent_panel.layout.size = UiVector2::pixels(400.0, 200.0);
        parent_panel.transform.scale = Vector2::new(0.5, 0.5);
        let parent = insert_ui_node(&mut runtime, SceneNodeData::UiPanel(Box::new(parent_panel)));

        let mut child_panel = UiPanel::new();
        child_panel.layout.size = UiVector2::pixels(200.0, 40.0);
        child_panel.style.set_corner_radius(0.4);
        child_panel.style.stroke_width = 2.0;
        let child = insert_ui_node(&mut runtime, SceneNodeData::UiPanel(Box::new(child_panel)));
        attach_child(&mut runtime, parent, child);

        runtime.extract_render_ui_commands();
        let mut commands = Vec::new();
        runtime.drain_render_commands(&mut commands);

        assert!(commands.iter().any(|cmd| matches!(
            cmd,
            RenderCommand::Ui(UiCommand::UpsertPanel {
                node,
                rect,
                corner_radii,
                stroke_width,
                ..
            }) if *node == child
                && rect.size == [100.0, 20.0]
                && corner_radii.tl == 0.4
                && *stroke_width == 1.0
        )));
    }

    #[test]
    fn ui_transform_dirty_updates_only_changed_branch() {
        let mut runtime = Runtime::new();
        runtime.set_viewport_size(800, 600);

        let mut layout = UiHLayout::new();
        layout.layout.size = UiVector2::pixels(300.0, 100.0);
        let parent = insert_ui_node(&mut runtime, SceneNodeData::UiHLayout(layout));
        let child = insert_panel(&mut runtime, [60.0, 40.0], Color::new(0.1, 0.2, 0.3, 1.0));
        let sibling = insert_panel(&mut runtime, [60.0, 40.0], Color::new(0.2, 0.3, 0.4, 1.0));
        attach_child(&mut runtime, parent, child);
        attach_child(&mut runtime, parent, sibling);

        runtime.extract_render_ui_commands();
        let mut commands = Vec::new();
        runtime.drain_render_commands(&mut commands);
        runtime.clear_dirty_flags();

        if let Some(mut scene_node) = runtime.nodes.get_mut(child)
            && let SceneNodeData::UiPanel(panel) = &mut scene_node.data
        {
            panel.transform.translation.x = 24.0;
        }
        runtime.mark_ui_dirty(
            child,
            Runtime::UI_DIRTY_TRANSFORM | Runtime::UI_DIRTY_COMMANDS,
        );
        let timing = runtime.extract_render_ui_commands_timed();
        commands.clear();
        runtime.drain_render_commands(&mut commands);

        assert_eq!(timing.affected_nodes, 1);
        assert_eq!(timing.command_nodes, 1);
        assert_eq!(
                commands
                    .iter()
                    .filter(|cmd| matches!(cmd, RenderCommand::Ui(UiCommand::UpsertPanel { node, .. }) if *node == child))
                    .count(),
                1
            );
        assert!(
                !commands
                    .iter()
                    .any(|cmd| matches!(cmd, RenderCommand::Ui(UiCommand::UpsertPanel { node, .. }) if *node == sibling))
            );
    }

    #[test]
    fn ui_layout_parent_dirty_updates_auto_layout_siblings() {
        let mut runtime = Runtime::new();
        runtime.set_viewport_size(800, 600);

        let mut layout = UiHLayout::new();
        layout.layout.size = UiVector2::pixels(300.0, 100.0);
        let parent = insert_ui_node(&mut runtime, SceneNodeData::UiHLayout(layout));
        let child = insert_panel(&mut runtime, [60.0, 40.0], Color::new(0.1, 0.2, 0.3, 1.0));
        let sibling = insert_panel(&mut runtime, [60.0, 40.0], Color::new(0.2, 0.3, 0.4, 1.0));
        attach_child(&mut runtime, parent, child);
        attach_child(&mut runtime, parent, sibling);

        runtime.extract_render_ui_commands();
        runtime.clear_dirty_flags();

        if let Some(mut scene_node) = runtime.nodes.get_mut(child)
            && let SceneNodeData::UiPanel(panel) = &mut scene_node.data
        {
            panel.layout.size = UiVector2::pixels(90.0, 40.0);
        }
        runtime.mark_ui_dirty(
            child,
            Runtime::UI_DIRTY_LAYOUT_SELF
                | Runtime::UI_DIRTY_LAYOUT_PARENT
                | Runtime::UI_DIRTY_COMMANDS,
        );
        let timing = runtime.extract_render_ui_commands_timed();

        assert_eq!(timing.affected_nodes, 2);
        assert_eq!(timing.command_nodes, 2);
    }

    #[test]
    fn child_fit_size_uses_spawn_baseline_for_relative_max_scale() {
        let mut runtime = Runtime::new();
        runtime.set_viewport_size(1920, 1080);

        let mut parent = UiPanel::new();
        parent.layout.size = UiVector2::ratio(0.5, 0.5);
        let parent_id = insert_ui_node(&mut runtime, SceneNodeData::UiPanel(Box::new(parent)));

        let mut child = UiPanel::new();
        child.layout.size = UiVector2::ratio(0.5, 0.5);
        child.layout.h_size = UiSizeMode::FitChildren;
        child.layout.v_size = UiSizeMode::FitChildren;
        child.layout.max_size_scale = Vector2::new(2.0, 2.0);
        let child_id = insert_ui_node(&mut runtime, SceneNodeData::UiPanel(Box::new(child)));
        attach_child(&mut runtime, parent_id, child_id);

        let oversized = insert_panel(
            &mut runtime,
            [1400.0, 900.0],
            Color::new(0.2, 0.3, 0.4, 1.0),
        );
        attach_child(&mut runtime, child_id, oversized);

        runtime.extract_render_ui_commands();
        let rect = runtime
            .render_ui
            .computed_rects
            .get(&child_id)
            .copied()
            .expect("child rect");

        assert_eq!(rect.size, Vector2::new(1400.0, 900.0));
    }

    #[test]
    fn relative_min_max_scale_rebases_when_size_definition_changes() {
        let mut runtime = Runtime::new();
        runtime.set_viewport_size(1920, 1080);

        let mut parent = UiPanel::new();
        parent.layout.size = UiVector2::ratio(0.5, 0.5);
        let parent_id = insert_ui_node(&mut runtime, SceneNodeData::UiPanel(Box::new(parent)));

        let mut child = UiPanel::new();
        child.layout.size = UiVector2::ratio(0.5, 0.5);
        child.layout.h_size = UiSizeMode::FitChildren;
        child.layout.v_size = UiSizeMode::FitChildren;
        child.layout.max_size_scale = Vector2::new(2.0, 2.0);
        let child_id = insert_ui_node(&mut runtime, SceneNodeData::UiPanel(Box::new(child)));
        attach_child(&mut runtime, parent_id, child_id);
        let oversized = insert_panel(
            &mut runtime,
            [1400.0, 900.0],
            Color::new(0.2, 0.3, 0.4, 1.0),
        );
        attach_child(&mut runtime, child_id, oversized);

        runtime.extract_render_ui_commands();
        let before = runtime
            .render_ui
            .computed_rects
            .get(&child_id)
            .copied()
            .expect("before rect");
        assert_eq!(before.size, Vector2::new(1400.0, 900.0));

        if let Some(mut scene_node) = runtime.nodes.get_mut(child_id)
            && let SceneNodeData::UiPanel(panel) = &mut scene_node.data
        {
            panel.layout.size = UiVector2::ratio(0.75, 0.75);
        }
        runtime.mark_ui_dirty(
            child_id,
            Runtime::UI_DIRTY_LAYOUT_SELF
                | Runtime::UI_DIRTY_LAYOUT_PARENT
                | Runtime::UI_DIRTY_COMMANDS,
        );
        runtime.extract_render_ui_commands();
        let after = runtime
            .render_ui
            .computed_rects
            .get(&child_id)
            .copied()
            .expect("after rect");

        assert_eq!(after.size, Vector2::new(1400.0, 900.0));
    }

    #[test]
    fn min_size_ratio_one_locks_spawn_floor() {
        let mut runtime = Runtime::new();
        runtime.project = Some(std::sync::Arc::new(
            crate::runtime_project::RuntimeProject::new("Test", "."),
        ));
        runtime.set_viewport_size(1280, 720);

        let mut top = UiPanel::new();
        top.layout.anchor = perro_ui::UiAnchor::Top;
        top.layout.size = UiVector2::ratio(0.96, 0.09);
        top.layout.min_size_scale = Vector2::new(1.0, 1.0);
        let top_id = insert_ui_node(&mut runtime, SceneNodeData::UiPanel(Box::new(top)));

        runtime.extract_render_ui_commands();
        let rect = runtime
            .render_ui
            .computed_rects
            .get(&top_id)
            .copied()
            .expect("top rect");

        assert_eq!(rect.size, Vector2::new(1229.0, 65.0));
    }

    #[test]
    fn min_size_ratio_rebases_when_size_ratio_changes() {
        let mut runtime = Runtime::new();
        runtime.set_viewport_size(1000, 1000);

        let mut panel = UiPanel::new();
        panel.layout.size = UiVector2::ratio(1.0, 1.0);
        panel.layout.min_size_scale = Vector2::new(0.5, 0.5);
        let panel_id = insert_ui_node(&mut runtime, SceneNodeData::UiPanel(Box::new(panel)));

        runtime.extract_render_ui_commands();
        runtime.set_viewport_size(400, 400);
        runtime.extract_render_ui_commands();
        let clamped_before = runtime
            .render_ui
            .computed_rects
            .get(&panel_id)
            .copied()
            .expect("panel rect before ratio change");
        assert_eq!(clamped_before.size, Vector2::new(500.0, 500.0));

        if let Some(mut scene_node) = runtime.nodes.get_mut(panel_id)
            && let SceneNodeData::UiPanel(panel) = &mut scene_node.data
        {
            panel.layout.size = UiVector2::ratio(0.5, 0.5);
        }
        runtime.mark_ui_dirty(
            panel_id,
            Runtime::UI_DIRTY_LAYOUT_SELF
                | Runtime::UI_DIRTY_LAYOUT_PARENT
                | Runtime::UI_DIRTY_COMMANDS,
        );
        runtime.extract_render_ui_commands();
        let after_ratio_change = runtime
            .render_ui
            .computed_rects
            .get(&panel_id)
            .copied()
            .expect("panel rect after ratio change");
        assert_eq!(after_ratio_change.size, Vector2::new(200.0, 200.0));
    }

    #[test]
    fn size_ratio_clamp_rebases_after_zero_size_first_pass() {
        let mut runtime = Runtime::new();

        let mut panel = UiPanel::new();
        panel.layout.size = UiVector2::ratio(0.5, 0.5);
        panel.layout.min_size_scale = Vector2::new(0.8, 0.8);
        panel.layout.max_size_scale = Vector2::new(1.2, 1.2);
        let panel_id = insert_ui_node(&mut runtime, SceneNodeData::UiPanel(Box::new(panel)));

        runtime.extract_render_ui_commands();
        assert_eq!(
            runtime
                .render_ui
                .computed_rects
                .get(&panel_id)
                .unwrap()
                .size,
            Vector2::new(0.5, 0.5)
        );

        runtime.set_viewport_size(1000, 800);
        runtime.extract_render_ui_commands();
        assert_eq!(
            runtime
                .render_ui
                .computed_rects
                .get(&panel_id)
                .unwrap()
                .size,
            Vector2::new(500.0, 400.0)
        );
    }

}
