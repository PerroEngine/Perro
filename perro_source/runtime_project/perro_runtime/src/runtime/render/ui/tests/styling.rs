mod styling {
    use super::*;

    #[test]
    fn label_fill_mode_uses_parent_space_without_auto_layout_parent() {
        let mut runtime = Runtime::new();
        runtime.set_viewport_size(800, 600);

        let mut parent = UiPanel::new();
        parent.layout.size = UiVector2::ratio(1.0, 1.0);
        let parent_id = insert_ui_node(&mut runtime, SceneNodeData::UiPanel(Box::new(parent)));

        let mut label = perro_ui::UiLabel::new().with_text("HP");
        label.layout.h_size = UiSizeMode::Fill;
        label.layout.v_size = UiSizeMode::Fill;
        label.text_size_ratio = 1.0;
        let label_id = insert_ui_node(&mut runtime, SceneNodeData::UiLabel(Box::new(label)));
        attach_child(&mut runtime, parent_id, label_id);

        runtime.extract_render_ui_commands();

        let parent_rect = runtime
            .render_ui
            .computed_rects
            .get(&parent_id)
            .copied()
            .expect("parent rect");
        let label_rect = runtime
            .render_ui
            .computed_rects
            .get(&label_id)
            .copied()
            .expect("label rect");

        assert_eq!(label_rect.center, parent_rect.center);
        assert_eq!(label_rect.size, parent_rect.size);
    }

    #[test]
    fn scroll_container_offsets_children_and_clips_to_view() {
        let mut runtime = Runtime::new();
        runtime.set_viewport_size(800, 600);

        let mut scroller = UiScrollContainer::new();
        scroller.layout.size = UiVector2::pixels(200.0, 100.0);
        scroller.scroll = Vector2::new(30.0, 40.0);
        let scroller_id = insert_ui_node(
            &mut runtime,
            SceneNodeData::UiScrollContainer(Box::new(scroller)),
        );

        let child = insert_panel(&mut runtime, [50.0, 30.0], Color::new(0.1, 0.2, 0.3, 1.0));
        attach_child(&mut runtime, scroller_id, child);

        runtime.extract_render_ui_commands();

        let child_rect = runtime
            .render_ui
            .computed_rects
            .get(&child)
            .copied()
            .expect("child rect");
        assert_eq!(child_rect.center, Vector2::new(-6.0, 35.0));

        let scroll = runtime
            .nodes
            .get(scroller_id)
            .and_then(|node| match &node.data {
                SceneNodeData::UiScrollContainer(scroller) => Some(scroller.scroll),
                _ => None,
            })
            .expect("scroller node");
        assert_eq!(scroll, Vector2::ZERO);

        let mut commands = Vec::new();
        runtime.drain_render_commands(&mut commands);
        let clip = commands
            .iter()
            .find_map(|cmd| match cmd {
                RenderCommand::Ui(UiCommand::UpsertPanel {
                    node, clip_rect, ..
                }) if *node == child => Some(*clip_rect),
                _ => None,
            })
            .expect("child panel command");
        for (actual, expected) in clip.iter().zip([300.0, 250.0, 500.0, 350.0]) {
            assert!((actual - expected).abs() < 1.0e-5);
        }
    }

    #[test]
    fn scroll_container_places_large_child_at_top() {
        let mut runtime = Runtime::new();
        runtime.set_viewport_size(800, 600);

        let mut scroller = UiScrollContainer::new();
        scroller.layout.size = UiVector2::pixels(200.0, 100.0);
        let scroller_id = insert_ui_node(
            &mut runtime,
            SceneNodeData::UiScrollContainer(Box::new(scroller)),
        );

        let mut list = UiVLayout::new();
        list.layout.size = UiVector2::pixels(200.0, 300.0);
        let list_id = insert_ui_node(&mut runtime, list.into());
        attach_child(&mut runtime, scroller_id, list_id);

        runtime.extract_render_ui_commands();

        let scroller_rect = runtime
            .render_ui
            .computed_rects
            .get(&scroller_id)
            .copied()
            .expect("scroller rect");
        let list_rect = runtime
            .render_ui
            .computed_rects
            .get(&list_id)
            .copied()
            .expect("list rect");
        assert_eq!(list_rect.max().y, scroller_rect.max().y);

        runtime.clear_dirty_flags();
        tap_key_and_extract(&mut runtime, KeyCode::End);

        let scroll = runtime
            .nodes
            .get(scroller_id)
            .and_then(|node| match &node.data {
                SceneNodeData::UiScrollContainer(scroller) => Some(scroller.scroll.y),
                _ => None,
            })
            .expect("scroller node");
        assert_eq!(scroll, 200.0);
    }

    #[test]
    fn scroll_container_scroll_to_snaps_to_normalized_part() {
        let mut runtime = Runtime::new();
        runtime.set_viewport_size(800, 600);

        let mut scroller = UiScrollContainer::new();
        scroller.layout.size = UiVector2::pixels(200.0, 100.0);
        let scroller_id = insert_ui_node(
            &mut runtime,
            SceneNodeData::UiScrollContainer(Box::new(scroller)),
        );

        let mut list = UiVLayout::new();
        list.layout.size = UiVector2::pixels(200.0, 300.0);
        let list_id = insert_ui_node(&mut runtime, list.into());
        attach_child(&mut runtime, scroller_id, list_id);

        runtime.extract_render_ui_commands();
        runtime.clear_dirty_flags();

        let _ = runtime.with_node_mut::<UiScrollContainer, _, _>(scroller_id, |node| {
            node.scroll_to(0.5, 0.0);
        });
        runtime.extract_render_ui_commands();

        let scroll = runtime
            .nodes
            .get(scroller_id)
            .and_then(|node| match &node.data {
                SceneNodeData::UiScrollContainer(scroller) => Some(scroller.scroll.y),
                _ => None,
            })
            .expect("scroller node");
        assert_eq!(scroll, 100.0);
    }

    #[test]
    fn scroll_container_scroll_to_animates_to_normalized_part() {
        let mut runtime = Runtime::new();
        runtime.set_viewport_size(800, 600);

        let mut scroller = UiScrollContainer::new();
        scroller.layout.size = UiVector2::pixels(200.0, 100.0);
        let scroller_id = insert_ui_node(
            &mut runtime,
            SceneNodeData::UiScrollContainer(Box::new(scroller)),
        );

        let mut list = UiVLayout::new();
        list.layout.size = UiVector2::pixels(200.0, 300.0);
        let list_id = insert_ui_node(&mut runtime, list.into());
        attach_child(&mut runtime, scroller_id, list_id);

        runtime.extract_render_ui_commands();
        runtime.clear_dirty_flags();

        let _ = runtime.with_node_mut::<UiScrollContainer, _, _>(scroller_id, |node| {
            node.scroll_to(1.0, 1.0);
        });
        runtime.update(0.5);
        runtime.extract_render_ui_commands();

        let mid = runtime
            .nodes
            .get(scroller_id)
            .and_then(|node| match &node.data {
                SceneNodeData::UiScrollContainer(scroller) => Some(scroller.scroll.y),
                _ => None,
            })
            .expect("scroller node");
        assert_eq!(mid, 100.0);

        runtime.clear_dirty_flags();
        runtime.update(0.5);
        runtime.extract_render_ui_commands();

        let (end, active) = runtime
            .nodes
            .get(scroller_id)
            .and_then(|node| match &node.data {
                SceneNodeData::UiScrollContainer(scroller) => {
                    Some((scroller.scroll.y, scroller.scroll_animation.is_some()))
                }
                _ => None,
            })
            .expect("scroller node");
        assert_eq!(end, 200.0);
        assert!(!active);
    }

    #[test]
    fn scroll_container_emits_right_scrollbar_thumb() {
        let mut runtime = Runtime::new();
        runtime.set_viewport_size(800, 600);

        let mut scroller = UiScrollContainer::new();
        scroller.layout.size = UiVector2::pixels(200.0, 100.0);
        let scroller_id = insert_ui_node(
            &mut runtime,
            SceneNodeData::UiScrollContainer(Box::new(scroller)),
        );

        let mut list = UiVLayout::new();
        list.layout.size = UiVector2::pixels(200.0, 300.0);
        let list_id = insert_ui_node(&mut runtime, list.into());
        attach_child(&mut runtime, scroller_id, list_id);

        runtime.extract_render_ui_commands();

        let mut commands = Vec::new();
        runtime.drain_render_commands(&mut commands);
        let rect = commands
            .iter()
            .find_map(|cmd| match cmd {
                RenderCommand::Ui(UiCommand::UpsertShape { node, rect, .. })
                    if *node == scroller_id =>
                {
                    Some(rect)
                }
                _ => None,
            })
            .expect("scrollbar command");
        assert_eq!(rect.center[0], 97.0);
        assert!((rect.center[1] - 33.333332).abs() < 1.0e-4);
        assert_eq!(rect.size[0], 6.0);
        assert!((rect.size[1] - 33.333332).abs() < 1.0e-4);
    }

    #[test]
    fn short_scroll_content_disables_scroll_and_removes_scrollbar() {
        let mut runtime = Runtime::new();
        runtime.set_viewport_size(800, 600);

        let mut scroller = UiScrollContainer::new();
        scroller.layout.size = UiVector2::pixels(200.0, 100.0);
        let scroller_id = insert_ui_node(
            &mut runtime,
            SceneNodeData::UiScrollContainer(Box::new(scroller)),
        );

        let mut list = UiVLayout::new();
        list.layout.size = UiVector2::pixels(200.0, 300.0);
        let list_id = insert_ui_node(&mut runtime, list.into());
        attach_child(&mut runtime, scroller_id, list_id);

        runtime.extract_render_ui_commands();
        runtime.drain_render_commands(&mut Vec::new());
        runtime.clear_dirty_flags();

        let _ = runtime.with_node_mut::<UiScrollContainer, _, _>(scroller_id, |node| {
            node.scroll = Vector2::new(0.0, 200.0);
        });
        let _ = runtime.with_node_mut::<UiVLayout, _, _>(list_id, |node| {
            node.layout.size = UiVector2::pixels(200.0, 80.0);
        });
        runtime.extract_render_ui_commands();

        let scroll = runtime
            .nodes
            .get(scroller_id)
            .and_then(|node| match &node.data {
                SceneNodeData::UiScrollContainer(scroller) => Some(scroller.scroll),
                _ => None,
            })
            .expect("scroller node");
        assert_eq!(scroll, Vector2::ZERO);

        let mut commands = Vec::new();
        runtime.drain_render_commands(&mut commands);
        assert!(commands.iter().any(|command| matches!(
            command,
            RenderCommand::Ui(UiCommand::RemoveNode { node }) if *node == scroller_id
        )));

        runtime.clear_dirty_flags();
        runtime.begin_input_frame();
        runtime.set_mouse_position(400.0, 300.0);
        runtime.add_mouse_wheel(0.0, -1.0);
        runtime.extract_render_ui_commands();

        let scroll = runtime
            .nodes
            .get(scroller_id)
            .and_then(|node| match &node.data {
                SceneNodeData::UiScrollContainer(scroller) => Some(scroller.scroll),
                _ => None,
            })
            .expect("scroller node");
        assert_eq!(scroll, Vector2::ZERO);
    }

    #[test]
    fn scroll_container_reserves_default_gap_for_scrollbar() {
        let mut runtime = Runtime::new();
        runtime.set_viewport_size(800, 600);

        let mut scroller = UiScrollContainer::new();
        scroller.layout.size = UiVector2::pixels(200.0, 100.0);
        let scroller_id = insert_ui_node(
            &mut runtime,
            SceneNodeData::UiScrollContainer(Box::new(scroller)),
        );

        let mut list = UiVLayout::new();
        list.layout.h_size = UiSizeMode::Fill;
        list.layout.size = UiVector2::pixels(0.0, 300.0);
        let list_id = insert_ui_node(&mut runtime, list.into());
        attach_child(&mut runtime, scroller_id, list_id);

        runtime.extract_render_ui_commands();

        let list_rect = runtime
            .render_ui
            .computed_rects
            .get(&list_id)
            .copied()
            .expect("list rect");
        assert_eq!(list_rect.size.x, 188.0);
        assert_eq!(list_rect.max().x, 88.0);
    }

    #[test]
    fn scroll_container_uses_custom_scrollbar_gap() {
        let mut runtime = Runtime::new();
        runtime.set_viewport_size(800, 600);

        let mut scroller = UiScrollContainer::new();
        scroller.layout.size = UiVector2::pixels(200.0, 100.0);
        scroller.scroll_bar_padding = 18.0;
        let scroller_id = insert_ui_node(
            &mut runtime,
            SceneNodeData::UiScrollContainer(Box::new(scroller)),
        );

        let mut list = UiVLayout::new();
        list.layout.h_size = UiSizeMode::Fill;
        list.layout.size = UiVector2::pixels(0.0, 300.0);
        let list_id = insert_ui_node(&mut runtime, list.into());
        attach_child(&mut runtime, scroller_id, list_id);

        runtime.extract_render_ui_commands();

        let list_rect = runtime
            .render_ui
            .computed_rects
            .get(&list_id)
            .copied()
            .expect("list rect");
        assert_eq!(list_rect.size.x, 176.0);
        assert_eq!(list_rect.max().x, 76.0);
    }

    #[test]
    fn scroll_container_thumb_drag_updates_scroll() {
        let mut runtime = Runtime::new();
        runtime.set_viewport_size(800, 600);

        let mut scroller = UiScrollContainer::new();
        scroller.layout.size = UiVector2::pixels(200.0, 100.0);
        let scroller_id = insert_ui_node(
            &mut runtime,
            SceneNodeData::UiScrollContainer(Box::new(scroller)),
        );

        let mut list = UiVLayout::new();
        list.layout.size = UiVector2::pixels(200.0, 300.0);
        let list_id = insert_ui_node(&mut runtime, list.into());
        attach_child(&mut runtime, scroller_id, list_id);

        runtime.extract_render_ui_commands();
        runtime.drain_render_commands(&mut Vec::new());
        runtime.clear_dirty_flags();

        runtime.begin_input_frame();
        runtime.set_mouse_position(497.0, 266.66666);
        runtime.set_mouse_button_state(MouseButton::Left, true);
        runtime.extract_render_ui_commands();

        runtime.begin_input_frame();
        runtime.set_mouse_position(497.0, 300.0);
        runtime.set_mouse_button_state(MouseButton::Left, true);
        runtime.extract_render_ui_commands();

        let scroll = runtime
            .nodes
            .get(scroller_id)
            .and_then(|node| match &node.data {
                SceneNodeData::UiScrollContainer(scroller) => Some(scroller.scroll.y),
                _ => None,
            })
            .expect("scroller node");
        assert!((scroll - 100.0).abs() < 1.0e-4);
    }

    #[test]
    fn scroll_container_track_click_updates_scroll() {
        let mut runtime = Runtime::new();
        runtime.set_viewport_size(800, 600);

        let mut scroller = UiScrollContainer::new();
        scroller.layout.size = UiVector2::pixels(200.0, 100.0);
        let scroller_id = insert_ui_node(
            &mut runtime,
            SceneNodeData::UiScrollContainer(Box::new(scroller)),
        );

        let mut list = UiVLayout::new();
        list.layout.size = UiVector2::pixels(200.0, 300.0);
        let list_id = insert_ui_node(&mut runtime, list.into());
        attach_child(&mut runtime, scroller_id, list_id);

        runtime.extract_render_ui_commands();
        runtime.drain_render_commands(&mut Vec::new());
        runtime.clear_dirty_flags();

        runtime.begin_input_frame();
        runtime.set_mouse_position(497.0, 345.0);
        runtime.set_mouse_button_state(MouseButton::Left, true);
        runtime.extract_render_ui_commands();

        let scroll = runtime
            .nodes
            .get(scroller_id)
            .and_then(|node| match &node.data {
                SceneNodeData::UiScrollContainer(scroller) => Some(scroller.scroll.y),
                _ => None,
            })
            .expect("scroller node");
        assert_eq!(scroll, 200.0);
    }

    #[test]
    fn scroll_container_scrollbar_requests_pointer_cursor() {
        let mut runtime = Runtime::new();
        runtime.set_viewport_size(800, 600);

        let mut scroller = UiScrollContainer::new();
        scroller.layout.size = UiVector2::pixels(200.0, 100.0);
        let scroller_id = insert_ui_node(
            &mut runtime,
            SceneNodeData::UiScrollContainer(Box::new(scroller)),
        );

        let mut list = UiVLayout::new();
        list.layout.size = UiVector2::pixels(200.0, 300.0);
        let list_id = insert_ui_node(&mut runtime, list.into());
        attach_child(&mut runtime, scroller_id, list_id);

        runtime.extract_render_ui_commands();
        runtime.drain_render_commands(&mut Vec::new());
        runtime.clear_dirty_flags();
        let _ = runtime.take_cursor_icon_request();

        runtime.set_mouse_position(497.0, 300.0);
        runtime.extract_render_ui_commands();

        assert_eq!(
            runtime.take_cursor_icon_request(),
            Some(perro_ui::CursorIcon::Pointer)
        );
    }

    #[test]
    fn wheel_scroll_updates_hovered_scroll_container() {
        let mut runtime = Runtime::new();
        runtime.set_viewport_size(800, 600);

        let mut scroller = UiScrollContainer::new();
        scroller.layout.size = UiVector2::pixels(200.0, 100.0);
        let scroller_id = insert_ui_node(
            &mut runtime,
            SceneNodeData::UiScrollContainer(Box::new(scroller)),
        );

        let child = insert_panel(&mut runtime, [200.0, 300.0], Color::new(0.1, 0.2, 0.3, 1.0));
        attach_child(&mut runtime, scroller_id, child);

        runtime.extract_render_ui_commands();
        runtime.clear_dirty_flags();

        runtime.begin_input_frame();
        runtime.set_mouse_position(400.0, 300.0);
        runtime.add_mouse_wheel(0.0, -1.0);
        runtime.extract_render_ui_commands();

        let scroll = runtime
            .nodes
            .get(scroller_id)
            .and_then(|node| match &node.data {
                SceneNodeData::UiScrollContainer(scroller) => Some(scroller.scroll.y),
                _ => None,
            })
            .expect("scroller node");
        assert!((scroll - 12.0).abs() < 1.0e-5);
    }

    #[test]
    fn wheel_scroll_skips_scroll_container_without_overflow() {
        let mut runtime = Runtime::new();
        runtime.set_viewport_size(800, 600);

        let mut outer = UiScrollContainer::new();
        outer.layout.size = UiVector2::pixels(240.0, 120.0);
        let outer_id = insert_ui_node(
            &mut runtime,
            SceneNodeData::UiScrollContainer(Box::new(outer)),
        );

        let mut inner = UiScrollContainer::new();
        inner.layout.size = UiVector2::pixels(220.0, 100.0);
        let inner_id = insert_ui_node(
            &mut runtime,
            SceneNodeData::UiScrollContainer(Box::new(inner)),
        );
        attach_child(&mut runtime, outer_id, inner_id);

        let inner_child = insert_panel(&mut runtime, [200.0, 80.0], Color::new(0.1, 0.2, 0.3, 1.0));
        attach_child(&mut runtime, inner_id, inner_child);

        let outer_child =
            insert_panel(&mut runtime, [220.0, 300.0], Color::new(0.2, 0.3, 0.4, 1.0));
        attach_child(&mut runtime, outer_id, outer_child);

        runtime.extract_render_ui_commands();
        runtime.clear_dirty_flags();

        runtime.begin_input_frame();
        runtime.set_mouse_position(400.0, 300.0);
        runtime.add_mouse_wheel(0.0, -1.0);
        runtime.extract_render_ui_commands();

        let inner_scroll = runtime
            .nodes
            .get(inner_id)
            .and_then(|node| match &node.data {
                SceneNodeData::UiScrollContainer(scroller) => Some(scroller.scroll.y),
                _ => None,
            })
            .expect("inner scroller node");
        let outer_scroll = runtime
            .nodes
            .get(outer_id)
            .and_then(|node| match &node.data {
                SceneNodeData::UiScrollContainer(scroller) => Some(scroller.scroll.y),
                _ => None,
            })
            .expect("outer scroller node");

        assert_eq!(inner_scroll, 0.0);
        assert!((outer_scroll - 14.4).abs() < 1.0e-5);
    }

    #[test]
    fn keyboard_scroll_targets_focused_scroll_container_ancestor() {
        let mut runtime = Runtime::new();
        runtime.set_viewport_size(800, 600);

        let mut scroller = UiScrollContainer::new();
        scroller.layout.size = UiVector2::pixels(200.0, 100.0);
        let scroller_id = insert_ui_node(
            &mut runtime,
            SceneNodeData::UiScrollContainer(Box::new(scroller)),
        );

        let mut button = perro_ui::UiButton::new();
        button.layout.size = UiVector2::pixels(120.0, 40.0);
        let button_id = insert_ui_node(&mut runtime, SceneNodeData::UiButton(Box::new(button)));
        attach_child(&mut runtime, scroller_id, button_id);

        let child = insert_panel(&mut runtime, [200.0, 300.0], Color::new(0.1, 0.2, 0.3, 1.0));
        attach_child(&mut runtime, scroller_id, child);

        runtime.extract_render_ui_commands();
        runtime.clear_dirty_flags();

        click_mouse_and_extract(&mut runtime, 400.0, 300.0);
        runtime.clear_dirty_flags();
        tap_key_and_extract(&mut runtime, KeyCode::End);

        let max_scroll = runtime
            .nodes
            .get(scroller_id)
            .and_then(|node| match &node.data {
                SceneNodeData::UiScrollContainer(scroller) => Some(scroller.scroll.y),
                _ => None,
            })
            .expect("scroller node");
        assert!((max_scroll - 200.0).abs() < 1.0e-5);

        runtime.clear_dirty_flags();
        tap_key_and_extract(&mut runtime, KeyCode::Home);
        let reset_scroll = runtime
            .nodes
            .get(scroller_id)
            .and_then(|node| match &node.data {
                SceneNodeData::UiScrollContainer(scroller) => Some(scroller.scroll.y),
                _ => None,
            })
            .expect("scroller node");
        assert_eq!(reset_scroll, 0.0);
    }

    #[test]
    fn keyboard_scroll_falls_back_to_sole_root_scroll_container() {
        let mut runtime = Runtime::new();
        runtime.set_viewport_size(800, 600);

        let mut scroller = UiScrollContainer::new();
        scroller.layout.size = UiVector2::pixels(200.0, 100.0);
        let scroller_id = insert_ui_node(
            &mut runtime,
            SceneNodeData::UiScrollContainer(Box::new(scroller)),
        );

        let child = insert_panel(&mut runtime, [200.0, 300.0], Color::new(0.1, 0.2, 0.3, 1.0));
        attach_child(&mut runtime, scroller_id, child);

        runtime.extract_render_ui_commands();
        runtime.clear_dirty_flags();

        tap_key_and_extract(&mut runtime, KeyCode::PageDown);

        let scroll = runtime
            .nodes
            .get(scroller_id)
            .and_then(|node| match &node.data {
                SceneNodeData::UiScrollContainer(scroller) => Some(scroller.scroll.y),
                _ => None,
            })
            .expect("scroller node");
        assert!((scroll - 90.0).abs() < 1.0e-5);
    }

    #[test]
    fn multiline_text_edit_wheel_takes_precedence_over_parent_scroll_container() {
        let mut runtime = Runtime::new();
        runtime.set_viewport_size(800, 600);

        let mut scroller = UiScrollContainer::new();
        scroller.layout.size = UiVector2::pixels(240.0, 120.0);
        let scroller_id = insert_ui_node(
            &mut runtime,
            SceneNodeData::UiScrollContainer(Box::new(scroller)),
        );

        let mut text_block = perro_ui::UiTextBlock::new();
        text_block.inner.base.layout.size = UiVector2::pixels(220.0, 100.0);
        text_block.inner.text = Cow::Borrowed("line1\nline2\nline3\nline4\nline5\nline6");
        let text_id = insert_ui_node(
            &mut runtime,
            SceneNodeData::UiTextBlock(Box::new(text_block)),
        );
        attach_child(&mut runtime, scroller_id, text_id);

        let filler = insert_panel(&mut runtime, [220.0, 260.0], Color::new(0.1, 0.2, 0.3, 1.0));
        attach_child(&mut runtime, scroller_id, filler);

        runtime.extract_render_ui_commands();
        runtime.clear_dirty_flags();

        click_mouse_and_extract(&mut runtime, 400.0, 300.0);
        runtime.clear_dirty_flags();

        runtime.begin_input_frame();
        runtime.set_mouse_position(400.0, 300.0);
        runtime.add_mouse_wheel(0.0, -1.0);
        runtime.extract_render_ui_commands();

        let parent_scroll = runtime
            .nodes
            .get(scroller_id)
            .and_then(|node| match &node.data {
                SceneNodeData::UiScrollContainer(scroller) => Some(scroller.scroll.y),
                _ => None,
            })
            .expect("scroller node");
        assert_eq!(parent_scroll, 0.0);

        let text_scroll = runtime
            .nodes
            .get(text_id)
            .and_then(|node| match &node.data {
                SceneNodeData::UiTextBlock(text_block) => Some(text_block.inner.v_scroll),
                _ => None,
            })
            .expect("text block node");
        assert!(text_scroll > 0.0);
    }

    #[test]
    fn parent_visibility_toggle_restores_button_hover_without_resize() {
        let mut runtime = Runtime::new();
        runtime.set_viewport_size(800, 600);

        let parent = insert_panel(&mut runtime, [220.0, 120.0], Color::new(0.2, 0.2, 0.2, 1.0));
        let button = insert_button(&mut runtime, [120.0, 40.0]);
        attach_child(&mut runtime, parent, button);

        runtime.extract_render_ui_commands();
        runtime.drain_render_commands(&mut Vec::new());
        runtime.clear_dirty_flags();

        let _ = runtime.with_node_mut::<UiPanel, _, _>(parent, |panel| {
            panel.visible = false;
        });
        runtime.extract_render_ui_commands();
        runtime.drain_render_commands(&mut Vec::new());
        runtime.clear_dirty_flags();

        let _ = runtime.with_base_node_mut::<perro_ui::UiNode, _, _>(parent, |panel| {
            panel.visible = true;
        });
        runtime.set_mouse_position(400.0, 300.0);
        runtime.extract_render_ui_commands();

        let mut commands = Vec::new();
        runtime.drain_render_commands(&mut commands);
        assert!(commands.iter().any(|cmd| matches!(
            cmd,
            RenderCommand::Ui(UiCommand::UpsertButton { node: n, fill, .. })
                if *n == button && *fill == rgba(0.2, 0.3, 0.4, 1.0)
        )));
    }

    #[test]
    fn hidden_parent_clears_button_hover_state() {
        let mut runtime = Runtime::new();
        runtime.set_viewport_size(800, 600);

        let parent = insert_panel(&mut runtime, [220.0, 120.0], Color::new(0.2, 0.2, 0.2, 1.0));
        let button = insert_button(&mut runtime, [120.0, 40.0]);
        attach_child(&mut runtime, parent, button);

        runtime.extract_render_ui_commands();
        runtime.drain_render_commands(&mut Vec::new());
        runtime.clear_dirty_flags();

        runtime.set_mouse_position(400.0, 300.0);
        runtime.extract_render_ui_commands();
        runtime.drain_render_commands(&mut Vec::new());
        assert_eq!(
            runtime.render_ui.button_states.get(&button).copied(),
            Some(UiButtonVisualState::Hover)
        );
        runtime.clear_dirty_flags();

        let _ = runtime.with_node_mut::<UiPanel, _, _>(parent, |panel| {
            panel.visible = false;
        });
        runtime.extract_render_ui_commands();
        runtime.drain_render_commands(&mut Vec::new());

        assert_ne!(
            runtime.render_ui.button_states.get(&button).copied(),
            Some(UiButtonVisualState::Hover)
        );
    }

    #[test]
    fn input_change_rechecks_retained_label_visibility() {
        let mut runtime = Runtime::new();
        runtime.set_viewport_size(800, 600);

        let parent = insert_panel(&mut runtime, [260.0, 120.0], Color::new(0.2, 0.2, 0.2, 1.0));
        let button = insert_button(&mut runtime, [120.0, 40.0]);
        let mut label = perro_ui::UiLabel::new();
        label.layout.size = UiVector2::pixels(120.0, 30.0);
        label.text = "Play".into();
        let label = insert_ui_node(&mut runtime, SceneNodeData::UiLabel(Box::new(label)));
        attach_child(&mut runtime, parent, button);
        attach_child(&mut runtime, parent, label);

        runtime.extract_render_ui_commands();
        runtime.drain_render_commands(&mut Vec::new());
        runtime.clear_dirty_flags();

        set_panel_visible(&mut runtime, parent, false);
        runtime.set_mouse_position(400.0, 300.0);
        runtime.extract_render_ui_commands();

        let mut commands = Vec::new();
        runtime.drain_render_commands(&mut commands);
        assert!(commands.iter().any(|cmd| matches!(
            cmd,
            RenderCommand::Ui(UiCommand::RemoveNode { node }) if *node == label
        )));
        assert!(commands.iter().any(|cmd| matches!(
            cmd,
            RenderCommand::Ui(UiCommand::RemoveNode { node }) if *node == button
        )));
    }

    #[test]
    fn non_ui_parent_visibility_change_removes_retained_ui_descendants() {
        let mut runtime = Runtime::new();
        runtime.set_viewport_size(800, 600);

        let root = runtime.create::<Node3D>();
        let button = insert_button(&mut runtime, [120.0, 40.0]);
        let mut label = perro_ui::UiLabel::new();
        label.layout.size = UiVector2::pixels(120.0, 30.0);
        label.text = "New".into();
        let label = insert_ui_node(&mut runtime, SceneNodeData::UiLabel(Box::new(label)));
        attach_child(&mut runtime, root, button);
        attach_child(&mut runtime, button, label);

        runtime.extract_render_ui_commands();
        runtime.drain_render_commands(&mut Vec::new());
        runtime.clear_dirty_flags();

        let _ = runtime.with_node_mut::<Node3D, _, _>(root, |node| {
            node.visible = false;
        });
        runtime.extract_render_ui_commands();

        let mut commands = Vec::new();
        runtime.drain_render_commands(&mut commands);
        assert!(commands.iter().any(|cmd| matches!(
            cmd,
            RenderCommand::Ui(UiCommand::RemoveNode { node }) if *node == button
        )));
        assert!(commands.iter().any(|cmd| matches!(
            cmd,
            RenderCommand::Ui(UiCommand::RemoveNode { node }) if *node == label
        )));
        assert!(!runtime.render_ui.prev_visible.contains(&button));
        assert!(!runtime.render_ui.prev_visible.contains(&label));
    }

    #[test]
    fn parent_visibility_toggle_restores_all_ui_descendants_without_resize() {
        let mut runtime = Runtime::new();
        runtime.set_viewport_size(800, 600);

        let parent = insert_panel(&mut runtime, [260.0, 120.0], Color::new(0.2, 0.2, 0.2, 1.0));
        let button = insert_button(&mut runtime, [120.0, 40.0]);
        let mut label = perro_ui::UiLabel::new();
        label.layout.size = UiVector2::pixels(120.0, 30.0);
        label.text = "Play".into();
        let label = insert_ui_node(&mut runtime, SceneNodeData::UiLabel(Box::new(label)));
        attach_child(&mut runtime, parent, button);
        attach_child(&mut runtime, parent, label);

        runtime.extract_render_ui_commands();
        runtime.drain_render_commands(&mut Vec::new());
        runtime.clear_dirty_flags();

        let _ = runtime.with_node_mut::<UiPanel, _, _>(parent, |panel| {
            panel.visible = false;
        });
        runtime.extract_render_ui_commands();
        runtime.drain_render_commands(&mut Vec::new());
        runtime.clear_dirty_flags();

        let _ = runtime.with_node_mut::<UiPanel, _, _>(parent, |panel| {
            panel.visible = true;
        });
        runtime.extract_render_ui_commands();

        let mut commands = Vec::new();
        runtime.drain_render_commands(&mut commands);
        assert!(commands.iter().any(|cmd| matches!(
            cmd,
            RenderCommand::Ui(UiCommand::UpsertButton { node: n, .. }) if *n == button
        )));
        assert!(commands.iter().any(|cmd| matches!(
            cmd,
            RenderCommand::Ui(UiCommand::UpsertLabel { node: n, .. }) if *n == label
        )));
    }

    #[test]
    fn initially_hidden_parent_show_extracts_button_label_descendant() {
        let mut runtime = Runtime::new();
        runtime.set_viewport_size(800, 600);

        let mut parent = UiPanel::new();
        parent.layout.size = UiVector2::pixels(260.0, 120.0);
        parent.visible = false;
        let parent = insert_ui_node(&mut runtime, SceneNodeData::UiPanel(Box::new(parent)));
        let button = insert_button(&mut runtime, [120.0, 40.0]);
        let mut label = perro_ui::UiLabel::new();
        label.layout.size = UiVector2::pixels(120.0, 30.0);
        label.text = "Play".into();
        let label = insert_ui_node(&mut runtime, SceneNodeData::UiLabel(Box::new(label)));
        attach_child(&mut runtime, parent, button);
        attach_child(&mut runtime, button, label);

        runtime.extract_render_ui_commands();
        runtime.drain_render_commands(&mut Vec::new());
        runtime.clear_dirty_flags();

        let _ = runtime.with_node_mut::<UiPanel, _, _>(parent, |panel| {
            panel.visible = true;
        });
        runtime.extract_render_ui_commands();

        let mut commands = Vec::new();
        runtime.drain_render_commands(&mut commands);
        assert!(commands.iter().any(|cmd| matches!(
            cmd,
            RenderCommand::Ui(UiCommand::UpsertButton { node: n, .. }) if *n == button
        )));
        assert!(commands.iter().any(|cmd| matches!(
            cmd,
            RenderCommand::Ui(UiCommand::UpsertLabel { node: n, .. }) if *n == label
        )));
    }

    #[test]
    fn force_rerender_marks_ui_subtree_after_raw_visibility_change() {
        let mut runtime = Runtime::new();
        runtime.set_viewport_size(800, 600);

        let parent = insert_panel(&mut runtime, [260.0, 120.0], Color::new(0.2, 0.2, 0.2, 1.0));
        let button = insert_button(&mut runtime, [120.0, 40.0]);
        let mut label = perro_ui::UiLabel::new();
        label.layout.size = UiVector2::pixels(120.0, 30.0);
        label.text = "Play".into();
        let label = insert_ui_node(&mut runtime, SceneNodeData::UiLabel(Box::new(label)));
        attach_child(&mut runtime, parent, button);
        attach_child(&mut runtime, button, label);

        runtime.extract_render_ui_commands();
        runtime.drain_render_commands(&mut Vec::new());
        runtime.clear_dirty_flags();

        set_panel_visible(&mut runtime, parent, false);
        runtime.force_rerender(parent);
        runtime.extract_render_ui_commands();
        runtime.drain_render_commands(&mut Vec::new());
        runtime.clear_dirty_flags();

        set_panel_visible(&mut runtime, parent, true);
        runtime.force_rerender(parent);
        runtime.extract_render_ui_commands();

        let mut commands = Vec::new();
        runtime.drain_render_commands(&mut commands);
        assert!(commands.iter().any(|cmd| matches!(
            cmd,
            RenderCommand::Ui(UiCommand::UpsertButton { node: n, .. }) if *n == button
        )));
        assert!(commands.iter().any(|cmd| matches!(
            cmd,
            RenderCommand::Ui(UiCommand::UpsertLabel { node: n, .. }) if *n == label
        )));
    }

    #[test]
    fn initially_hidden_ui_subtree_inserts_after_force_visible() {
        let mut runtime = Runtime::new();
        runtime.set_viewport_size(800, 600);

        let parent = insert_panel(&mut runtime, [260.0, 120.0], Color::new(0.2, 0.2, 0.2, 1.0));
        let button = insert_button(&mut runtime, [120.0, 40.0]);
        attach_child(&mut runtime, parent, button);
        set_panel_visible(&mut runtime, parent, false);
        runtime.force_rerender(parent);

        runtime.extract_render_ui_commands();
        runtime.drain_render_commands(&mut Vec::new());
        runtime.clear_dirty_flags();

        set_panel_visible(&mut runtime, parent, true);
        runtime.force_rerender(parent);
        runtime.extract_render_ui_commands();

        let mut commands = Vec::new();
        runtime.drain_render_commands(&mut commands);
        assert!(commands.iter().any(|cmd| matches!(
            cmd,
            RenderCommand::Ui(UiCommand::UpsertPanel { node: n, .. }) if *n == parent
        )));
        assert!(commands.iter().any(|cmd| matches!(
            cmd,
            RenderCommand::Ui(UiCommand::UpsertButton { node: n, .. }) if *n == button
        )));
    }

    #[test]
    fn zero_size_visible_panel_upserts_after_expand() {
        let mut runtime = Runtime::new();
        runtime.set_viewport_size(800, 600);

        let parent = insert_panel(&mut runtime, [0.0, 0.0], Color::new(0.0, 0.0, 0.0, 0.0));
        runtime.extract_render_ui_commands();
        runtime.drain_render_commands(&mut Vec::new());
        runtime.clear_dirty_flags();

        let _ = runtime.with_node_mut::<UiPanel, _, _>(parent, |panel| {
            panel.layout.size = UiVector2::ratio(0.5, 0.1);
            panel.style.fill = Color::new(0.341, 0.780, 0.851, 1.0);
            panel.style.stroke = Color::new(1.0, 1.0, 1.0, 1.0);
            panel.style.stroke_width = 2.0;
        });
        runtime.force_rerender(parent);
        runtime.extract_render_ui_commands();

        let mut commands = Vec::new();
        runtime.drain_render_commands(&mut commands);
        assert!(commands.iter().any(|cmd| matches!(
            cmd,
            RenderCommand::Ui(UiCommand::UpsertPanel { node: n, rect, fill, .. })
                if *n == parent && rect.size[0] > 300.0 && rect.size[1] > 50.0 && *fill == rgba(0.341, 0.780, 0.851, 1.0)
        )));
    }

    #[test]
    fn menu_like_nested_layout_restores_all_buttons_and_labels_after_show() {
        let mut runtime = Runtime::new();
        runtime.set_viewport_size(800, 600);

        let mut root = perro_ui::UiNode::new();
        root.layout.size = UiVector2::ratio(1.0, 1.0);
        let root = insert_ui_node(&mut runtime, SceneNodeData::UiNode(root));

        let mut content = UiVLayout::new();
        content.layout.size = UiVector2::ratio(0.92, 0.92);
        let content = insert_ui_node(&mut runtime, content.into());
        attach_child(&mut runtime, root, content);

        let mut grid = UiVLayout::new();
        grid.layout.size = UiVector2::ratio(1.0, 0.72);
        let grid = insert_ui_node(&mut runtime, grid.into());
        attach_child(&mut runtime, content, grid);

        let row_top = insert_ui_node(&mut runtime, UiHLayout::new().into());
        let row_bottom = insert_ui_node(&mut runtime, UiHLayout::new().into());
        attach_child(&mut runtime, grid, row_top);
        attach_child(&mut runtime, grid, row_bottom);

        let mut buttons = Vec::new();
        let mut labels = Vec::new();
        for row in [row_top, row_top, row_bottom, row_bottom] {
            let button = insert_button(&mut runtime, [120.0, 40.0]);
            attach_child(&mut runtime, row, button);
            buttons.push(button);

            let mut label = perro_ui::UiLabel::new();
            label.layout.size = UiVector2::ratio(1.0, 1.0);
            label.text = "Sport".into();
            let label = insert_ui_node(&mut runtime, SceneNodeData::UiLabel(Box::new(label)));
            attach_child(&mut runtime, button, label);
            labels.push(label);
        }

        runtime.extract_render_ui_commands();
        runtime.drain_render_commands(&mut Vec::new());
        runtime.clear_dirty_flags();

        let _ = runtime.with_node_mut::<perro_ui::UiNode, _, _>(root, |ui| {
            ui.visible = false;
        });
        runtime.extract_render_ui_commands();
        runtime.drain_render_commands(&mut Vec::new());
        runtime.clear_dirty_flags();

        let _ = runtime.with_node_mut::<perro_ui::UiNode, _, _>(root, |ui| {
            ui.visible = true;
        });
        runtime.extract_render_ui_commands();

        let mut commands = Vec::new();
        runtime.drain_render_commands(&mut commands);
        for button in buttons {
            assert!(commands.iter().any(|cmd| matches!(
                cmd,
                RenderCommand::Ui(UiCommand::UpsertButton { node: n, .. }) if *n == button
            )));
        }
        for label in labels {
            assert!(commands.iter().any(|cmd| matches!(
                cmd,
                RenderCommand::Ui(UiCommand::UpsertLabel { node: n, .. }) if *n == label
            )));
        }
    }

    #[test]
    fn tree_list_rows_expand_down_from_top_and_hide_closed_children() {
        let mut runtime = Runtime::new();
        runtime.set_viewport_size(400, 300);

        let mut tree = perro_ui::UiTreeList::new();
        tree.layout.anchor = perro_ui::UiAnchor::Top;
        tree.layout.size = UiVector2::pixels(200.0, 160.0);
        tree.row_height = 20.0;
        tree.items.push(perro_ui::UiTreeListItem::new("ROOT"));
        tree.items
            .push(perro_ui::UiTreeListItem::new("child").child(0));
        tree.items
            .push(perro_ui::UiTreeListItem::new("leaf").child(1));
        let tree_id = insert_ui_node(&mut runtime, SceneNodeData::UiTreeList(Box::new(tree)));

        runtime.extract_render_ui_commands();
        let rows = runtime
            .nodes
            .get(tree_id)
            .and_then(|node| match &node.data {
                SceneNodeData::UiTreeList(tree) => Some(tree.internal_rows.clone()),
                _ => None,
            })
            .expect("tree rows");
        let toggles = runtime
            .nodes
            .get(tree_id)
            .and_then(|node| match &node.data {
                SceneNodeData::UiTreeList(tree) => Some(tree.internal_toggles.clone()),
                _ => None,
            })
            .expect("tree toggles");
        let y0 = runtime.render_ui.computed_rects[&rows[0]].center.y;
        let y1 = runtime.render_ui.computed_rects[&rows[1]].center.y;
        let y2 = runtime.render_ui.computed_rects[&rows[2]].center.y;
        assert!(y0 > y1);
        assert!(y1 > y2);
        assert!(matches!(
            runtime.nodes.get(toggles[0]).map(|node| &node.data),
            Some(SceneNodeData::UiShape(shape))
                if shape.base.visible
                    && (shape.base.transform.rotation - std::f32::consts::FRAC_PI_2).abs() < 1.0e-6
        ));

        if let Some(mut scene_node) = runtime.nodes.get_mut(tree_id)
            && let SceneNodeData::UiTreeList(tree) = &mut scene_node.data
        {
            tree.items[0].open = false;
        }
        runtime.sync_tree_list_internal_nodes(tree_id);
        let row_count = runtime
            .nodes
            .get(tree_id)
            .and_then(|node| match &node.data {
                SceneNodeData::UiTreeList(tree) => Some(tree.visible_items().len()),
                _ => None,
            })
            .expect("tree row count");
        assert_eq!(row_count, 1);
        assert!(matches!(
            runtime.nodes.get(toggles[0]).map(|node| &node.data),
            Some(SceneNodeData::UiShape(shape))
                if shape.base.visible && shape.base.transform.rotation.abs() < 1.0e-6
        ));
        assert!(rows.iter().copied().skip(1).all(|id| {
            runtime
                .nodes
                .get(id)
                .is_some_and(|node| ui_root_from_data(&node.data).is_some_and(|ui| !ui.visible))
        }));

        if let Some(mut scene_node) = runtime.nodes.get_mut(tree_id)
            && let SceneNodeData::UiTreeList(tree) = &mut scene_node.data
        {
            tree.items[0].open = true;
        }
        runtime.sync_tree_list_internal_nodes(tree_id);
        let reopened_rows = runtime
            .nodes
            .get(tree_id)
            .and_then(|node| match &node.data {
                SceneNodeData::UiTreeList(tree) => Some(tree.internal_rows.clone()),
                _ => None,
            })
            .expect("reopened tree rows");
        assert_eq!(&reopened_rows[..3], &rows[..3]);
        assert!(rows.iter().copied().take(3).all(|id| {
            runtime
                .nodes
                .get(id)
                .is_some_and(|node| ui_root_from_data(&node.data).is_some_and(|ui| ui.visible))
        }));
    }

    #[test]
    fn dropdown_options_match_button_width() {
        let mut runtime = Runtime::new();
        runtime.set_viewport_size(400, 300);

        let mut dropdown = perro_ui::UiDropdown::new();
        dropdown.layout.size = UiVector2::pixels(180.0, 32.0);
        dropdown.open = true;
        dropdown.options.push(perro_ui::UiDropdownOption::new(
            "One",
            perro_variant::Variant::from(1_i32),
        ));
        dropdown.options.push(perro_ui::UiDropdownOption::new(
            "Two",
            perro_variant::Variant::from(2_i32),
        ));
        let dropdown_id =
            insert_ui_node(&mut runtime, SceneNodeData::UiDropdown(Box::new(dropdown)));

        runtime.extract_render_ui_commands();
        let (button_width, option_id) = runtime
            .nodes
            .get(dropdown_id)
            .and_then(|node| match &node.data {
                SceneNodeData::UiDropdown(dropdown) => Some((
                    runtime.render_ui.computed_rects[&dropdown_id].size.x,
                    dropdown.internal_option_buttons[0],
                )),
                _ => None,
            })
            .expect("dropdown option");
        let option_width = runtime.render_ui.computed_rects[&option_id].size.x;

        assert_eq!(button_width, 180.0);
        assert_eq!(option_width, button_width);
    }

    #[test]
    fn ui_nine_slice_button_emits_resized_nine_slice_with_hover_tint() {
        let mut runtime = Runtime::new();
        runtime.set_viewport_size(800, 600);

        let mut button = perro_ui::UiNineSliceButton::new();
        button.texture = TextureID::from_parts(65, 0);
        button.layout.size = UiVector2::pixels(140.0, 52.0);
        button.margins = [6.0, 7.0, 8.0, 9.0];
        button.hover_tint = Color::new(0.3, 0.5, 0.7, 1.0);
        let node = insert_ui_node(
            &mut runtime,
            SceneNodeData::UiNineSliceButton(Box::new(button)),
        );

        runtime.extract_render_ui_commands();
        runtime.drain_render_commands(&mut Vec::new());
        runtime.clear_dirty_flags();
        runtime.begin_input_frame();
        runtime.set_mouse_position(400.0, 300.0);
        runtime.extract_render_ui_commands();
        let mut commands = Vec::new();
        runtime.drain_render_commands(&mut commands);

        assert!(commands.iter().any(|cmd| matches!(
            cmd,
            RenderCommand::Ui(UiCommand::UpsertNineSlice {
                node: n,
                rect,
                tint,
                margins,
                ..
            }) if *n == node
                && rect.size == [140.0, 52.0]
                && *tint == Color::new(0.3, 0.5, 0.7, 1.0)
                && *margins == [6.0, 7.0, 8.0, 9.0]
        )));
    }

    #[test]
    fn dropdown_popup_uses_custom_size_direction_and_style() {
        let mut runtime = Runtime::new();
        runtime.set_viewport_size(400, 300);

        let mut dropdown = perro_ui::UiDropdown::new();
        dropdown.layout.size = UiVector2::pixels(180.0, 32.0);
        dropdown.open = true;
        dropdown.popup_size = [240.0, 80.0];
        dropdown.popup_direction = perro_ui::UiDropdownDirection::Up;
        dropdown.popup_style.fill = Color::new(0.3, 0.2, 0.1, 1.0);
        dropdown.options.push(perro_ui::UiDropdownOption::new(
            "One",
            perro_variant::Variant::from(1_i32),
        ));
        let dropdown_id =
            insert_ui_node(&mut runtime, SceneNodeData::UiDropdown(Box::new(dropdown)));

        runtime.extract_render_ui_commands();
        let popup_id = runtime
            .nodes
            .get(dropdown_id)
            .and_then(|node| match &node.data {
                SceneNodeData::UiDropdown(dropdown) => Some(dropdown.internal_popup_panel),
                _ => None,
            })
            .expect("dropdown popup");
        let popup = runtime.render_ui.computed_rects[&popup_id];
        let dropdown_rect = runtime.render_ui.computed_rects[&dropdown_id];

        assert_eq!(runtime.nodes.get(popup_id).expect("test or bench setup must succeed").parent, dropdown_id);
        assert_eq!(popup.size, Vector2::new(240.0, 80.0));
        assert_eq!(popup.max().y, dropdown_rect.min().y);
        assert!(matches!(
            &runtime.nodes.get(popup_id).expect("test or bench setup must succeed").data,
            SceneNodeData::UiPanel(panel) if panel.style.fill == Color::new(0.3, 0.2, 0.1, 1.0)
        ));
    }

    #[test]
    fn dropdown_popup_directions_touch_control_edge() {
        for direction in [
            perro_ui::UiDropdownDirection::Down,
            perro_ui::UiDropdownDirection::Up,
            perro_ui::UiDropdownDirection::Left,
            perro_ui::UiDropdownDirection::Right,
        ] {
            let mut runtime = Runtime::new();
            runtime.set_viewport_size(400, 300);

            let mut dropdown = perro_ui::UiDropdown::new();
            dropdown.layout.size = UiVector2::pixels(180.0, 32.0);
            dropdown.open = true;
            dropdown.popup_size = [240.0, 80.0];
            dropdown.popup_direction = direction;
            dropdown.options.push(perro_ui::UiDropdownOption::new(
                "One",
                perro_variant::Variant::from(1_i32),
            ));
            let dropdown_id =
                insert_ui_node(&mut runtime, SceneNodeData::UiDropdown(Box::new(dropdown)));

            runtime.extract_render_ui_commands();
            let popup_id = runtime
                .nodes
                .get(dropdown_id)
                .and_then(|node| match &node.data {
                    SceneNodeData::UiDropdown(dropdown) => Some(dropdown.internal_popup_panel),
                    _ => None,
                })
                .expect("dropdown popup");
            let popup = runtime.render_ui.computed_rects[&popup_id];
            let control = runtime.render_ui.computed_rects[&dropdown_id];

            match direction {
                perro_ui::UiDropdownDirection::Down => assert_eq!(popup.min().y, control.max().y),
                perro_ui::UiDropdownDirection::Up => assert_eq!(popup.max().y, control.min().y),
                perro_ui::UiDropdownDirection::Left => assert_eq!(popup.max().x, control.min().x),
                perro_ui::UiDropdownDirection::Right => assert_eq!(popup.min().x, control.max().x),
            }
        }
    }

    #[test]
    fn dropdown_extend_animation_grows_popup() {
        let mut runtime = Runtime::new();
        runtime.set_viewport_size(400, 300);
        runtime.time.delta = 0.1;

        let mut dropdown = perro_ui::UiDropdown::new();
        dropdown.layout.size = UiVector2::pixels(180.0, 32.0);
        dropdown.open = true;
        dropdown.open_animation = perro_ui::UiDropdownOpenAnimation::Extend;
        dropdown.open_animation_duration = 0.2;
        dropdown.popup_size = [180.0, 80.0];
        dropdown.options.push(perro_ui::UiDropdownOption::new(
            "One",
            perro_variant::Variant::from(1_i32),
        ));
        let dropdown_id =
            insert_ui_node(&mut runtime, SceneNodeData::UiDropdown(Box::new(dropdown)));

        runtime.extract_render_ui_commands();
        let popup_id = runtime
            .nodes
            .get(dropdown_id)
            .and_then(|node| match &node.data {
                SceneNodeData::UiDropdown(dropdown) => Some(dropdown.internal_popup_panel),
                _ => None,
            })
            .expect("dropdown popup");
        assert_eq!(runtime.render_ui.computed_rects[&popup_id].size.y, 40.0);

        runtime.extract_render_ui_commands();
        assert_eq!(runtime.render_ui.computed_rects[&popup_id].size.y, 80.0);
    }

    #[test]
    fn dropdown_option_churn_reuses_internal_nodes() {
        let mut runtime = Runtime::new();
        runtime.set_viewport_size(400, 300);

        let mut dropdown = perro_ui::UiDropdown::new();
        dropdown.layout.size = UiVector2::pixels(180.0, 32.0);
        dropdown.open = true;
        for idx in 0..4 {
            dropdown.options.push(perro_ui::UiDropdownOption::new(
                format!("Option {idx}"),
                perro_variant::Variant::from(idx),
            ));
        }
        let dropdown_id =
            insert_ui_node(&mut runtime, SceneNodeData::UiDropdown(Box::new(dropdown)));

        runtime.extract_render_ui_commands();
        let (buttons, labels) = runtime
            .nodes
            .get(dropdown_id)
            .and_then(|node| match &node.data {
                SceneNodeData::UiDropdown(dropdown) => Some((
                    dropdown.internal_option_buttons.clone(),
                    dropdown.internal_option_labels.clone(),
                )),
                _ => None,
            })
            .expect("dropdown internals");
        let arena_len = runtime.nodes.len();
        let slot_count = runtime.nodes.slot_count();
        let update_schedule_len = runtime.internal_updates.internal_update_nodes.len();
        let fixed_schedule_len = runtime.internal_updates.internal_fixed_update_nodes.len();
        let dropdown_children = runtime.nodes.get(dropdown_id).expect("test or bench setup must succeed").children.len();

        for cycle in 0..16 {
            if let Some(mut node) = runtime.nodes.get_mut(dropdown_id)
                && let SceneNodeData::UiDropdown(dropdown) = &mut node.data
            {
                dropdown.options.truncate(1);
                dropdown.options[0].label = format!("Small {cycle}").into();
                dropdown.open = true;
            }
            runtime.sync_dropdown_internal_nodes(dropdown_id);

            assert!(buttons.iter().copied().skip(1).all(|id| {
                matches!(
                    runtime.nodes.get(id).map(|node| &node.data),
                    Some(SceneNodeData::UiButton(button)) if !button.base.visible
                )
            }));
            assert!(labels.iter().copied().skip(1).all(|id| {
                matches!(
                    runtime.nodes.get(id).map(|node| &node.data),
                    Some(SceneNodeData::UiLabel(label)) if !label.base.visible
                )
            }));
            assert!(matches!(
                runtime.nodes.get(labels[0]).map(|node| &node.data),
                Some(SceneNodeData::UiLabel(label)) if label.text == format!("Small {cycle}")
            ));

            if let Some(mut node) = runtime.nodes.get_mut(dropdown_id)
                && let SceneNodeData::UiDropdown(dropdown) = &mut node.data
            {
                dropdown.options.clear();
                for idx in 0..4 {
                    dropdown.options.push(perro_ui::UiDropdownOption::new(
                        format!("Cycle {cycle} option {idx}"),
                        perro_variant::Variant::from(idx),
                    ));
                }
            }
            runtime.sync_dropdown_internal_nodes(dropdown_id);

            let (current_buttons, current_labels) = runtime
                .nodes
                .get(dropdown_id)
                .and_then(|node| match &node.data {
                    SceneNodeData::UiDropdown(dropdown) => Some((
                        &dropdown.internal_option_buttons,
                        &dropdown.internal_option_labels,
                    )),
                    _ => None,
                })
                .expect("dropdown internals");
            assert_eq!(current_buttons, &buttons);
            assert_eq!(current_labels, &labels);
            assert!(buttons.iter().all(|id| {
                matches!(
                    runtime.nodes.get(*id).map(|node| &node.data),
                    Some(SceneNodeData::UiButton(button)) if button.base.visible
                )
            }));
            for (idx, label_id) in labels.iter().copied().enumerate() {
                assert!(matches!(
                    runtime.nodes.get(label_id).map(|node| &node.data),
                    Some(SceneNodeData::UiLabel(label))
                        if label.base.visible && label.text == format!("Cycle {cycle} option {idx}")
                ));
            }

            assert_eq!(runtime.nodes.len(), arena_len);
            assert_eq!(runtime.nodes.slot_count(), slot_count);
            assert_eq!(
                runtime.internal_updates.internal_update_nodes.len(),
                update_schedule_len
            );
            assert_eq!(
                runtime.internal_updates.internal_fixed_update_nodes.len(),
                fixed_schedule_len
            );
            assert_eq!(
                runtime.nodes.get(dropdown_id).expect("test or bench setup must succeed").children.len(),
                dropdown_children
            );
            assert_eq!(
                runtime
                    .nodes
                    .named_ids("__perro_dropdown_option_label")
                    .len(),
                4
            );
            for idx in 0..4 {
                assert_eq!(
                    runtime
                        .nodes
                        .named_ids(&format!("__perro_dropdown_option_{idx}"))
                        .len(),
                    1
                );
                assert_eq!(
                    runtime.nodes.get(buttons[idx]).expect("test or bench setup must succeed").children,
                    [labels[idx]]
                );
            }
        }
    }

    #[test]
    fn dropdown_click_open_renders_options_after_frame_dirty_clear() {
        let mut runtime = Runtime::new();
        runtime.set_viewport_size(800, 600);

        let mut dropdown = perro_ui::UiDropdown::new();
        dropdown.layout.size = UiVector2::pixels(180.0, 32.0);
        dropdown.options.push(perro_ui::UiDropdownOption::new(
            "One",
            perro_variant::Variant::from(1_i32),
        ));
        let dropdown_id =
            insert_ui_node(&mut runtime, SceneNodeData::UiDropdown(Box::new(dropdown)));

        runtime.extract_render_ui_commands();
        runtime.drain_render_commands(&mut Vec::new());
        runtime.clear_dirty_flags();

        // Press over the dropdown, then release: the click opens the dropdown
        // during the post-layout input phase of extraction.
        runtime.set_mouse_position(400.0, 300.0);
        runtime.set_mouse_button_state(MouseButton::Left, true);
        runtime.extract_render_ui_commands();
        runtime.drain_render_commands(&mut Vec::new());
        runtime.clear_dirty_flags();

        runtime.set_mouse_button_state(MouseButton::Left, false);
        runtime.begin_input_frame();
        runtime.extract_render_ui_commands();
        runtime.drain_render_commands(&mut Vec::new());
        runtime.clear_dirty_flags();

        let (open, option_id) = runtime
            .nodes
            .get(dropdown_id)
            .and_then(|node| match &node.data {
                SceneNodeData::UiDropdown(dropdown) => Some((
                    dropdown.open,
                    dropdown.internal_option_buttons.first().copied(),
                )),
                _ => None,
            })
            .expect("dropdown node");
        assert!(open, "click opens dropdown");
        let option_id = option_id.expect("dropdown option button exists");

        // Next frame has no new input; the deferred dirty marks must force a
        // relayout so the option renders without waiting for other UI work.
        runtime.begin_input_frame();
        runtime.extract_render_ui_commands();
        let mut commands = Vec::new();
        runtime.drain_render_commands(&mut commands);
        assert!(
            commands.iter().any(|command| matches!(
                command,
                RenderCommand::Ui(UiCommand::UpsertButton { node, .. }) if *node == option_id
            )),
            "open dropdown option renders on the following frame"
        );
    }

    #[test]
    fn snap_computed_ui_rect_rounds_screen_space_rect() {
        let rect = ComputedUiRect::new(Vector2::new(-19.3, 198.2), Vector2::new(136.6, 42.2));

        let snapped = snap_computed_ui_rect(rect, Vector2::new(800.0, 600.0), 1.0);
        let min = snapped.min();
        let screen_min = Vector2::new(400.0 + min.x, 300.0 - snapped.max().y);

        assert_eq!(screen_min, Vector2::new(312.0, 81.0));
        assert_eq!(snapped.size, Vector2::new(137.0, 42.0));
    }

    #[test]
    fn snap_to_physical_pixels_respects_scale_factor() {
        assert_eq!(snap_to_physical_pixels(10.25, 2.0), 10.5);
        assert_eq!(snap_to_physical_pixels(10.2, 2.0), 10.0);
    }
}
