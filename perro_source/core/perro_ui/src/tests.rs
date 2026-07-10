use super::*;

#[test]
fn percent_position_resolves_against_viewport() {
    let mut transform = UiTransform::new();
    transform.position = UiVector2::percent(50.0, 50.0);

    assert_eq!(
        transform.resolved_position(Vector2::new(1920.0, 1080.0)),
        Vector2::new(960.0, 540.0)
    );
}

#[test]
fn default_layout_origin_centers_in_parent() {
    let mut layout = UiLayoutData::new();
    let transform = UiTransform::new();
    layout.size = UiVector2::pixels(200.0, 100.0);

    assert_eq!(
        layout.resolved_origin(&transform, Vector2::new(800.0, 600.0)),
        Vector2::new(300.0, 250.0)
    );
}

#[test]
fn default_layout_aligns_children_to_center() {
    let transform = UiTransform::new();
    let layout = UiLayoutData::new();

    assert_eq!(layout.anchor, UiAnchor::Center);
    assert_eq!(transform.position, UiVector2::ratio(0.5, 0.5));
    assert_eq!(layout.h_align, UiHorizontalAlign::Center);
    assert_eq!(layout.v_align, UiVerticalAlign::Center);
}

#[test]
fn ui_node_defaults_to_no_child_clipping() {
    let base = UiNode::new();
    assert!(!base.clip_children);
}

#[test]
fn tree_list_flattens_open_roots_and_skips_closed_children() {
    let mut tree = UiTreeList::new();
    tree.items.push(UiTreeListItem::new("root"));
    tree.items.push(UiTreeListItem::new("child").child(0));
    tree.items.push(UiTreeListItem::new("leaf").child(1));

    let open = tree.visible_items();
    assert_eq!(
        open.iter().map(|item| item.index).collect::<Vec<_>>(),
        vec![0, 1, 2]
    );
    assert_eq!(
        open.iter().map(|item| item.depth).collect::<Vec<_>>(),
        vec![0, 1, 2]
    );

    tree.items[0].open = false;
    let closed = tree.visible_items();
    assert_eq!(closed.len(), 1);
    assert_eq!(closed[0].index, 0);
    assert!(closed[0].has_children);
}

#[test]
fn tree_list_has_children_hint_marks_childless_rows_expandable() {
    let mut tree = UiTreeList::new();
    tree.items.push(UiTreeListItem::new("plain_leaf"));
    let mut folder = UiTreeListItem::new("culled_folder");
    folder.open = false;
    folder.has_children_hint = true;
    tree.items.push(folder);

    let rows = tree.visible_items();
    assert_eq!(rows.len(), 2);
    assert!(!rows[0].has_children);
    assert!(rows[1].has_children);
}

#[test]
fn label_text_align_defaults_to_center() {
    let label = UiLabel::new();

    assert_eq!(label.h_align, UiTextAlign::Center);
    assert_eq!(label.v_align, UiTextAlign::Center);
}

#[test]
fn label_set_text_accepts_static_str_string_and_cow() {
    let mut label = UiLabel::new();

    label.set_text("static text");
    assert!(matches!(label.text, Cow::Borrowed("static text")));

    label.set_text(String::from("owned text"));
    assert!(matches!(label.text, Cow::Owned(ref text) if text == "owned text"));

    label.set_text(Cow::Borrowed("cow text"));
    assert!(matches!(label.text, Cow::Borrowed("cow text")));
}

#[test]
fn pixel_and_percent_units_can_mix() {
    let value = UiVector2::new(UiUnit::px(24.0), UiUnit::pct(25.0));

    assert_eq!(
        value.resolve(Vector2::new(800.0, 600.0)),
        Vector2::new(24.0, 150.0)
    );
}

#[test]
fn centered_position_percent_resolves_as_offset() {
    let value = UiVector2::percent(50.0, 25.0);

    assert_eq!(
        value.resolve_centered(Vector2::new(800.0, 600.0)),
        Vector2::new(0.0, -150.0)
    );
}

#[test]
fn ratio_units_match_percent_units() {
    let value = UiVector2::ratio(0.5, 0.25);

    assert_eq!(
        value.resolve(Vector2::new(800.0, 600.0)),
        Vector2::new(400.0, 150.0)
    );
    assert_eq!(
        value.resolve_centered(Vector2::new(800.0, 600.0)),
        Vector2::new(0.0, -150.0)
    );
}

#[test]
fn size_respects_min_and_max_size() {
    let mut layout = UiLayoutData::new();
    layout.size = UiVector2::ratio(0.5, 0.1);
    layout.min_size = Vector2::new(300.0, 80.0);
    layout.max_size = Vector2::new(1200.0, 90.0);

    assert_eq!(
        layout
            .clamp_size(layout.resolved_size(Vector2::new(3000.0, 1000.0)))
            .x,
        1200.0
    );
    assert!(
        (layout
            .clamp_size(layout.resolved_size(Vector2::new(3000.0, 1000.0)))
            .y
            - 90.0)
            .abs()
            < 1.0e-3
    );
    assert_eq!(
        layout
            .clamp_size(layout.resolved_size(Vector2::new(400.0, 400.0)))
            .x,
        300.0
    );
    assert!(
        (layout
            .clamp_size(layout.resolved_size(Vector2::new(400.0, 400.0)))
            .y
            - 80.0)
            .abs()
            < 1.0e-3
    );
}

#[test]
fn scale_applies_after_size_resolve() {
    let mut layout = UiLayoutData::new();
    let mut transform = UiTransform::new();
    layout.size = UiVector2::ratio(0.5, 0.1);
    layout.max_size = Vector2::new(1200.0, 90.0);
    transform.scale = Vector2::new(2.0, 0.5);

    let parent = ComputedUiRect::new(Vector2::ZERO, Vector2::new(3000.0, 1000.0));
    let rect = layout.compute_rect(&transform, parent);

    assert_eq!(rect.size.x, 3000.0);
    assert!((rect.size.y - 50.0).abs() < 1.0e-3);
}

#[test]
fn rect_inset_uses_top_bottom_edges() {
    let rect = ComputedUiRect::new(Vector2::ZERO, Vector2::new(100.0, 80.0));

    assert_eq!(
        rect.inset(UiRect::new(10.0, 20.0, 30.0, 5.0)),
        ComputedUiRect::new(Vector2::new(-10.0, -7.5), Vector2::new(60.0, 55.0))
    );
}

#[test]
fn rounded_contains_rejects_trimmed_corner() {
    let rect = ComputedUiRect::new(Vector2::ZERO, Vector2::new(120.0, 40.0));

    assert!(!rect.contains_rounded(Vector2::new(-59.0, 19.0), 1.0));
}

#[test]
fn rounded_contains_keeps_center_and_edge_band() {
    let rect = ComputedUiRect::new(Vector2::ZERO, Vector2::new(120.0, 40.0));

    assert!(rect.contains_rounded(Vector2::ZERO, 1.0));
    assert!(rect.contains_rounded(Vector2::new(-59.0, 0.0), 1.0));
}

#[test]
fn ui_button_defaults_to_no_web_action() {
    let button = UiButton::new();

    assert_eq!(button.cursor_icon, CursorIcon::Pointer);
    assert!(button.web.is_none());
}

#[test]
fn default_depth_effects_do_not_scale_past_widget_bounds() {
    let panel = UiStyle::panel();
    let button = UiStyle::button();

    assert_eq!(panel.outer_shadow.size, 1.0);
    assert_eq!(button.outer_shadow.size, 1.0);
    assert!(panel.outer_shadow.falloff > button.outer_shadow.falloff);
}

#[test]
fn color_picker_modes_accept_scene_names() {
    assert_eq!(
        UiColorPickerMode::parse("smooth_wheel"),
        Some(UiColorPickerMode::SmoothWheel)
    );
    assert_eq!(
        UiColorPickerMode::parse("blocky"),
        Some(UiColorPickerMode::BlockWheel)
    );
    assert_eq!(
        UiColorPickerMode::parse("swatches"),
        Some(UiColorPickerMode::Swatches)
    );
}

#[test]
fn ui_image_button_defaults_to_image_click_target() {
    let button = UiImageButton::new();

    assert!(button.texture.is_nil());
    assert_eq!(button.tint, Color::WHITE);
    assert_eq!(button.hover_tint, Color::WHITE);
    assert_eq!(button.pressed_tint, Color::WHITE);
    assert_eq!(button.scale_mode, UiImageScaleMode::Stretch);
    assert_eq!(button.cursor_icon, CursorIcon::Pointer);
    assert!(button.web.is_none());
    assert!(!button.disabled);
}

#[test]
fn ui_nine_slice_defaults_to_texture_panel_parts() {
    let node = UiNineSlice::new();

    assert!(node.texture.is_nil());
    assert_eq!(node.texture_region, None);
    assert_eq!(node.margins, [8.0, 8.0, 8.0, 8.0]);
    assert_eq!(node.tint, Color::WHITE);
}

#[test]
fn right_anchor_offsets_size_inward() {
    let mut layout = UiLayoutData::new();
    let mut transform = UiTransform::new();
    layout.anchor = UiAnchor::Right;
    transform.position = UiVector2::ZERO;
    layout.size = UiVector2::pixels(200.0, 100.0);
    transform.pivot = UiVector2::percent(50.0, 50.0);

    let parent = ComputedUiRect::new(Vector2::ZERO, Vector2::new(800.0, 600.0));
    let rect = layout.compute_rect(&transform, parent);

    assert_eq!(rect.center, Vector2::new(300.0, 0.0));
    assert_eq!(rect.max().x, 400.0);
}

#[test]
fn top_left_anchor_uses_center_origin_y_up() {
    let mut layout = UiLayoutData::new();
    let mut transform = UiTransform::new();
    layout.anchor = UiAnchor::TopLeft;
    transform.position = UiVector2::ZERO;
    layout.size = UiVector2::pixels(100.0, 50.0);
    transform.pivot = UiVector2::percent(50.0, 50.0);

    let parent = ComputedUiRect::new(Vector2::ZERO, Vector2::new(800.0, 600.0));
    let rect = layout.compute_rect(&transform, parent);

    assert_eq!(rect.center, Vector2::new(-350.0, 275.0));
    assert_eq!(rect.min(), Vector2::new(-400.0, 250.0));
    assert_eq!(rect.max(), Vector2::new(-300.0, 300.0));
}
