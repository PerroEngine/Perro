use super::*;
use crate::RuntimeScriptApi;
use perro_ids::ScriptMemberID;
use perro_nodes::{Node3D, SceneNode, SceneNodeData, Sky3D, UiCameraStream, camera_3d::Camera3D};
use perro_render_bridge::{CameraStreamCommand, CameraStreamSourceState, RenderEvent};
use perro_resource_api::sub_apis::TextureAPI;
use perro_runtime_api::sub_apis::{NodeAPI, NodeSpec, SignalAPI};
use perro_scripting::{ScriptBehavior, ScriptContext, ScriptFlags, ScriptLifecycle};
use perro_structs::{Color, Quaternion, Transform3D, Vector3};
use perro_ui::{
    UiAnchor, UiAnimatedImage, UiAnimatedImageFrameSet, UiGrid, UiHLayout, UiLayoutSpacingMode,
    UiPanel, UiScrollContainer, UiShape, UiShapeKind, UiVLayout, UiVector2,
};
use std::any::Any;
use std::sync::{
    Arc,
    atomic::{AtomicUsize, Ordering},
};

fn rgba(r: f32, g: f32, b: f32, a: f32) -> [f32; 4] {
    Color::new(r, g, b, a).to_float_slice()
}

fn collect_resource_texture_request(
    runtime: &mut Runtime,
    texture: TextureID,
) -> perro_render_bridge::RenderRequestID {
    let mut commands = Vec::new();
    runtime.drain_render_commands(&mut commands);
    commands
        .into_iter()
        .find_map(|command| match command {
            RenderCommand::Resource(ResourceCommand::CreateTexture { request, id, .. })
                if id == texture =>
            {
                Some(request)
            }
            _ => None,
        })
        .expect("expected texture create request")
}

#[test]
fn ui_camera_stream_refreshes_when_source_camera_moves() {
    let mut runtime = Runtime::new();
    let camera = NodeAPI::create::<Camera3D>(&mut runtime);
    let stream = NodeAPI::create::<UiCameraStream>(&mut runtime);
    if let Some(node) = runtime.nodes.get_mut(stream)
        && let SceneNodeData::UiCameraStream(data) = &mut node.data
    {
        data.stream.camera = camera;
        data.stream.resolution = [320, 180].into();
    }

    runtime.extract_render_ui_commands();
    runtime.drain_render_commands(&mut Vec::new());

    if let Some(node) = runtime.nodes.get_mut(camera)
        && let SceneNodeData::Camera3D(data) = &mut node.data
    {
        data.transform = Transform3D::new(
            Vector3::new(4.0, 5.0, 6.0),
            Quaternion::IDENTITY,
            Vector3::ONE,
        );
    }
    runtime.mark_transform_dirty_recursive(camera);
    runtime.extract_render_ui_commands();
    let mut commands = Vec::new();
    runtime.drain_render_commands(&mut commands);

    assert!(commands.iter().any(|command| matches!(
        command,
        RenderCommand::CameraStream(CameraStreamCommand::Upsert { node, state })
            if *node == stream
                && matches!(&state.source, CameraStreamSourceState::ThreeD(camera) if camera.position == [4.0, 5.0, 6.0])
    )));
}

#[test]
fn ui_camera_stream_3d_captures_sky_from_source_camera() {
    let mut runtime = Runtime::new();
    let camera = NodeAPI::create::<Camera3D>(&mut runtime);
    let _sky = NodeAPI::create::<Sky3D>(&mut runtime);
    let stream = NodeAPI::create::<UiCameraStream>(&mut runtime);
    if let Some(node) = runtime.nodes.get_mut(stream)
        && let SceneNodeData::UiCameraStream(data) = &mut node.data
    {
        data.stream.camera = camera;
        data.stream.resolution = [320, 180].into();
    }

    runtime.extract_render_ui_commands();
    let mut commands = Vec::new();
    runtime.drain_render_commands(&mut commands);

    assert!(commands.iter().any(|command| matches!(
        command,
        RenderCommand::CameraStream(CameraStreamCommand::Upsert { node, state })
            if *node == stream
                && matches!(state.source, CameraStreamSourceState::ThreeD(_))
                && state.lighting_3d.sky.is_some()
    )));
}

#[test]
fn ui_camera_stream_emits_image_corner_radius() {
    let mut runtime = Runtime::new();
    runtime.set_viewport_size(800, 600);
    let camera = NodeAPI::create::<Camera3D>(&mut runtime);
    let stream = NodeAPI::create::<UiCameraStream>(&mut runtime);
    if let Some(node) = runtime.nodes.get_mut(stream)
        && let SceneNodeData::UiCameraStream(data) = &mut node.data
    {
        data.layout.size = UiVector2::pixels(320.0, 180.0);
        data.stream.camera = camera;
        data.stream.resolution = [320, 180].into();
        data.corner_radius = 0.25;
    }

    runtime.extract_render_ui_commands();
    let mut commands = Vec::new();
    runtime.drain_render_commands(&mut commands);

    assert!(commands.iter().any(|command| matches!(
        command,
        RenderCommand::Ui(UiCommand::UpsertImage { node, corner_radii, .. })
            if *node == stream && corner_radii.tl == 0.25 && corner_radii.tr == 0.25
    )));
}

#[test]
fn unchanged_ui_skips_redundant_upsert() {
    let mut runtime = Runtime::new();
    runtime.set_viewport_size(800, 600);
    let node = insert_panel(&mut runtime, [120.0, 40.0], Color::new(0.1, 0.2, 0.3, 1.0));

    runtime.extract_render_ui_commands();
    let mut commands = Vec::new();
    runtime.drain_render_commands(&mut commands);
    assert_eq!(commands.iter().filter(|cmd| matches!(cmd, RenderCommand::Ui(UiCommand::UpsertPanel { node: n, .. }) if *n == node)).count(), 1);

    runtime.clear_dirty_flags();
    runtime.extract_render_ui_commands();
    commands.clear();
    runtime.drain_render_commands(&mut commands);
    assert!(commands.is_empty());
}

#[test]
fn ui_animated_image_emits_current_frame_region() {
    let mut runtime = Runtime::new();
    runtime.set_viewport_size(800, 600);

    let mut image = UiAnimatedImage::new();
    image.texture = TextureID::from_parts(42, 0);
    image.layout.size = UiVector2::pixels(64.0, 64.0);
    image.current_frame = 1;
    image.animations.push(UiAnimatedImageFrameSet {
        name: Cow::Borrowed("default"),
        start: [0.0, 0.0],
        frame_size: [16.0, 16.0],
        frame_count: 4,
        columns: 2,
        fps: 12.0,
    });
    let node = insert_ui_node(&mut runtime, SceneNodeData::UiAnimatedImage(image));

    runtime.extract_render_ui_commands();
    let mut commands = Vec::new();
    runtime.drain_render_commands(&mut commands);

    assert!(commands.iter().any(|cmd| matches!(
        cmd,
        RenderCommand::Ui(UiCommand::UpsertImage { node: n, uv_min, uv_max, .. })
            if *n == node && *uv_min == [16.0, 0.0] && *uv_max == [32.0, 16.0]
    )));
}

#[test]
fn ui_image_button_emits_image_command_with_state_tint() {
    let mut runtime = Runtime::new();
    runtime.set_viewport_size(800, 600);

    let mut button = perro_ui::UiImageButton::new();
    button.texture = TextureID::from_parts(43, 0);
    button.layout.size = UiVector2::pixels(64.0, 64.0);
    button.tint = Color::new(0.1, 0.2, 0.3, 1.0);
    button.hover_tint = Color::new(0.4, 0.5, 0.6, 1.0);
    button.pressed_tint = Color::new(0.7, 0.8, 0.9, 1.0);
    button.scale_mode = perro_ui::UiImageScaleMode::Fit;
    let node = insert_ui_node(&mut runtime, SceneNodeData::UiImageButton(button));

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
        RenderCommand::Ui(UiCommand::UpsertImage { node: n, tint, scale_mode, .. })
            if *n == node
                && *tint == Color::new(0.4, 0.5, 0.6, 1.0)
                && *scale_mode == UiImageScaleState::Fit
    )));
}

#[test]
fn ui_image_uses_inherited_ui_modulate() {
    let mut runtime = Runtime::new();
    runtime.set_viewport_size(800, 600);

    let mut parent = UiPanel::new();
    parent.layout.size = UiVector2::pixels(120.0, 80.0);
    parent.modulate.children_modulate = Color::new(1.0, 0.5, 1.0, 1.0);
    parent.modulate.self_modulate = Color::RED;
    let parent = insert_ui_node(&mut runtime, SceneNodeData::UiPanel(parent));

    let mut image = perro_ui::UiImage::new();
    image.texture = TextureID::from_parts(44, 0);
    image.layout.size = UiVector2::pixels(32.0, 32.0);
    image.tint = Color::new(0.5, 1.0, 1.0, 1.0);
    let child = insert_ui_node(&mut runtime, SceneNodeData::UiImage(image));
    attach_child(&mut runtime, parent, child);

    runtime.extract_render_ui_commands();
    let mut commands = Vec::new();
    runtime.drain_render_commands(&mut commands);

    let expected = Runtime::color_modulate(
        Color::new(1.0, 0.5, 1.0, 1.0),
        Color::new(0.5, 1.0, 1.0, 1.0),
    );
    assert!(commands.iter().any(|cmd| matches!(
        cmd,
        RenderCommand::Ui(UiCommand::UpsertImage { node: n, tint, .. })
            if *n == child && *tint == expected
    )));
}

#[test]
fn ui_nine_slice_emits_nine_slice_command() {
    let mut runtime = Runtime::new();
    runtime.set_viewport_size(800, 600);

    let mut node_data = perro_ui::UiNineSlice::new();
    node_data.texture = TextureID::from_parts(64, 0);
    node_data.layout.size = UiVector2::pixels(120.0, 40.0);
    node_data.texture_region = Some([1.0, 2.0, 30.0, 20.0]);
    node_data.margins = [5.0, 6.0, 7.0, 8.0];
    let node = insert_ui_node(&mut runtime, SceneNodeData::UiNineSlice(node_data));

    runtime.extract_render_ui_commands();
    let mut commands = Vec::new();
    runtime.drain_render_commands(&mut commands);

    assert!(commands.iter().any(|cmd| matches!(
        cmd,
        RenderCommand::Ui(UiCommand::UpsertNineSlice {
            node: n,
            texture,
            uv_min,
            uv_max,
            margins,
            ..
        }) if *n == node
            && *texture == TextureID::from_parts(64, 0)
            && *uv_min == [1.0, 2.0]
            && *uv_max == [31.0, 22.0]
            && *margins == [5.0, 6.0, 7.0, 8.0]
    )));
}

#[test]
fn ui_image_keeps_retained_texture_while_replacement_texture_is_pending() {
    let mut runtime = Runtime::new();
    runtime.set_viewport_size(800, 600);

    let old_texture = TextureID::from_parts(61, 0);
    let mut image = perro_ui::UiImage::new();
    image.texture = old_texture;
    image.layout.size = UiVector2::pixels(64.0, 64.0);
    let node = insert_ui_node(&mut runtime, SceneNodeData::UiImage(image));

    runtime.extract_render_ui_commands();
    let mut commands = Vec::new();
    runtime.drain_render_commands(&mut commands);
    assert!(commands.iter().any(|cmd| matches!(
        cmd,
        RenderCommand::Ui(UiCommand::UpsertImage { node: n, texture, .. })
            if *n == node && *texture == old_texture
    )));

    let pending_texture = runtime
        .resource_api
        .load_texture("res://textures/ui_tool_version_b.png");
    let pending_request = collect_resource_texture_request(&mut runtime, pending_texture);
    if let Some(scene_node) = runtime.nodes.get_mut(node)
        && let SceneNodeData::UiImage(image) = &mut scene_node.data
    {
        image.texture = pending_texture;
    }
    runtime.mark_ui_dirty(node, Runtime::UI_DIRTY_COMMANDS);

    runtime.extract_render_ui_commands();
    commands.clear();
    runtime.drain_render_commands(&mut commands);
    assert!(!commands.iter().any(|cmd| matches!(
        cmd,
        RenderCommand::Ui(UiCommand::RemoveNode { node: n }) if *n == node
    )));
    assert!(!commands.iter().any(|cmd| matches!(
        cmd,
        RenderCommand::Ui(UiCommand::UpsertImage { node: n, .. }) if *n == node
    )));
    assert!(
        runtime
            .render_ui
            .retained_commands
            .get(&node)
            .is_some_and(|cmd| {
                matches!(cmd, UiCommand::UpsertImage { texture, .. } if *texture == old_texture)
            })
    );

    runtime.apply_render_event(RenderEvent::TextureCreated {
        request: pending_request,
        id: pending_texture,
    });
    runtime.mark_ui_dirty(node, Runtime::UI_DIRTY_COMMANDS);
    runtime.extract_render_ui_commands();
    commands.clear();
    runtime.drain_render_commands(&mut commands);
    assert!(commands.iter().any(|cmd| matches!(
        cmd,
        RenderCommand::Ui(UiCommand::UpsertImage { node: n, texture, .. })
            if *n == node && *texture == pending_texture
    )));
}

#[test]
fn viewport_resize_recomputes_percent_ui_rects() {
    let mut runtime = Runtime::new();
    runtime.set_viewport_size(800, 600);

    let mut panel = UiPanel::new();
    panel.layout.anchor = UiAnchor::TopRight;
    panel.layout.size = UiVector2::ratio(0.5, 0.25);
    panel.style.fill = Color::new(0.1, 0.2, 0.3, 1.0);
    let node = insert_ui_node(&mut runtime, SceneNodeData::UiPanel(panel));

    runtime.extract_render_ui_commands();
    runtime.drain_render_commands(&mut Vec::new());
    runtime.clear_dirty_flags();

    runtime.set_viewport_size(1200, 900);
    runtime.extract_render_ui_commands();
    let mut commands = Vec::new();
    runtime.drain_render_commands(&mut commands);

    assert!(commands.iter().any(|cmd| matches!(
        cmd,
        RenderCommand::Ui(UiCommand::UpsertPanel { node: n, rect, .. })
            if *n == node
                && rect.size == [600.0, 225.0]
                && rect.center == [300.0, 337.5]
    )));
}

#[test]
fn ui_panel_without_position_field_centers_in_parent() {
    let mut runtime = Runtime::new();
    runtime.set_viewport_size(800, 600);

    let node = insert_panel(&mut runtime, [100.0, 50.0], Color::new(0.1, 0.2, 0.3, 1.0));

    runtime.extract_render_ui_commands();

    let rect = runtime
        .render_ui
        .computed_rects
        .get(&node)
        .copied()
        .expect("computed rect");
    assert_eq!(rect.center, Vector2::ZERO);
    assert_eq!(rect.size, Vector2::new(100.0, 50.0));
}

#[test]
fn ui_bottom_anchor_places_rect_on_bottom_edge_without_position() {
    let mut runtime = Runtime::new();
    runtime.set_viewport_size(800, 600);

    let node = insert_panel(&mut runtime, [100.0, 50.0], Color::new(0.1, 0.2, 0.3, 1.0));
    if let Some(scene_node) = runtime.nodes.get_mut(node)
        && let SceneNodeData::UiPanel(panel) = &mut scene_node.data
    {
        panel.layout.anchor = UiAnchor::Bottom;
    }

    runtime.extract_render_ui_commands();

    let rect = runtime
        .render_ui
        .computed_rects
        .get(&node)
        .copied()
        .expect("computed rect");
    assert_eq!(rect.center, Vector2::new(0.0, -275.0));
    assert_eq!(rect.min().y, -300.0);
}

#[test]
fn ui_translation_ratio_moves_after_anchor_by_parent_size() {
    let mut runtime = Runtime::new();
    runtime.set_viewport_size(800, 600);

    let node = insert_panel(&mut runtime, [100.0, 80.0], Color::new(0.1, 0.2, 0.3, 1.0));
    if let Some(scene_node) = runtime.nodes.get_mut(node)
        && let SceneNodeData::UiPanel(panel) = &mut scene_node.data
    {
        panel.transform.translation = Vector2::new(0.25, -0.5);
    }

    runtime.extract_render_ui_commands();

    let rect = runtime
        .render_ui
        .computed_rects
        .get(&node)
        .copied()
        .expect("computed rect");
    assert_eq!(rect.center, Vector2::new(200.0, -300.0));
}

#[test]
fn ui_self_translation_ratio_moves_after_anchor_by_own_size() {
    let mut runtime = Runtime::new();
    runtime.set_viewport_size(800, 600);

    let node = insert_panel(&mut runtime, [100.0, 80.0], Color::new(0.1, 0.2, 0.3, 1.0));
    if let Some(scene_node) = runtime.nodes.get_mut(node)
        && let SceneNodeData::UiPanel(panel) = &mut scene_node.data
    {
        panel.transform.self_translation = Vector2::new(0.25, -0.5);
    }

    runtime.extract_render_ui_commands();

    let rect = runtime
        .render_ui
        .computed_rects
        .get(&node)
        .copied()
        .expect("computed rect");
    assert_eq!(rect.center, Vector2::new(25.0, -40.0));
}

#[test]
fn ui_bottom_anchor_keeps_edge_placed_while_pivot_moves_origin() {
    let mut runtime = Runtime::new();
    runtime.set_viewport_size(800, 600);

    let node = insert_panel(&mut runtime, [100.0, 100.0], Color::new(0.1, 0.2, 0.3, 1.0));
    if let Some(scene_node) = runtime.nodes.get_mut(node)
        && let SceneNodeData::UiPanel(panel) = &mut scene_node.data
    {
        panel.layout.anchor = UiAnchor::Bottom;
        panel.transform.pivot = UiVector2::ratio(0.5, 1.0);
    }

    runtime.extract_render_ui_commands();

    let rect = runtime
        .render_ui
        .computed_rects
        .get(&node)
        .copied()
        .expect("computed rect");
    assert_eq!(rect.center, Vector2::new(0.0, -250.0));
    assert_eq!(rect.min().y, -300.0);
    assert_eq!(rect.max().y, -200.0);

    let mut commands = Vec::new();
    runtime.drain_render_commands(&mut commands);
    assert!(commands.iter().any(|cmd| matches!(
        cmd,
        RenderCommand::Ui(UiCommand::UpsertPanel { node: n, rect, .. })
            if *n == node && rect.pivot == [0.5, 1.0]
    )));
}

#[test]
fn ui_pivot_changes_render_pivot_without_changing_anchor_layout() {
    let mut runtime = Runtime::new();
    runtime.set_viewport_size(800, 600);

    let centered = insert_panel(&mut runtime, [100.0, 50.0], Color::new(0.1, 0.2, 0.3, 1.0));
    if let Some(scene_node) = runtime.nodes.get_mut(centered)
        && let SceneNodeData::UiPanel(panel) = &mut scene_node.data
    {
        panel.layout.anchor = UiAnchor::Bottom;
        panel.transform.pivot = UiVector2::ratio(0.5, 0.5);
    }

    let top_pivot = insert_panel(&mut runtime, [100.0, 50.0], Color::new(0.1, 0.2, 0.3, 1.0));
    if let Some(scene_node) = runtime.nodes.get_mut(top_pivot)
        && let SceneNodeData::UiPanel(panel) = &mut scene_node.data
    {
        panel.layout.anchor = UiAnchor::Bottom;
        panel.transform.pivot = UiVector2::ratio(0.5, 1.0);
    }

    runtime.extract_render_ui_commands();

    let centered_rect = runtime
        .render_ui
        .computed_rects
        .get(&centered)
        .copied()
        .expect("centered rect");
    let top_pivot_rect = runtime
        .render_ui
        .computed_rects
        .get(&top_pivot)
        .copied()
        .expect("top pivot rect");
    assert_eq!(top_pivot_rect.center, centered_rect.center);

    let mut commands = Vec::new();
    runtime.drain_render_commands(&mut commands);
    assert!(commands.iter().any(|cmd| matches!(
        cmd,
        RenderCommand::Ui(UiCommand::UpsertPanel { node, rect, .. })
            if *node == top_pivot && rect.pivot == [0.5, 1.0]
    )));
}

#[test]
fn ui_center_and_right_anchor_translation_can_reach_same_parent_point() {
    let mut runtime = Runtime::new();
    runtime.set_viewport_size(800, 600);

    let center = insert_panel(&mut runtime, [200.0, 80.0], Color::new(0.1, 0.2, 0.3, 1.0));
    if let Some(scene_node) = runtime.nodes.get_mut(center)
        && let SceneNodeData::UiPanel(panel) = &mut scene_node.data
    {
        panel.layout.anchor = UiAnchor::Center;
        panel.transform.translation = Vector2::new(0.25, 0.0);
    }

    let right = insert_panel(&mut runtime, [200.0, 80.0], Color::new(0.1, 0.2, 0.3, 1.0));
    if let Some(scene_node) = runtime.nodes.get_mut(right)
        && let SceneNodeData::UiPanel(panel) = &mut scene_node.data
    {
        panel.layout.anchor = UiAnchor::Right;
        panel.transform.translation = Vector2::new(-0.125, 0.0);
    }

    runtime.extract_render_ui_commands();

    let center_rect = runtime
        .render_ui
        .computed_rects
        .get(&center)
        .copied()
        .expect("center rect");
    let right_rect = runtime
        .render_ui
        .computed_rects
        .get(&right)
        .copied()
        .expect("right rect");
    assert_eq!(center_rect.center, Vector2::new(200.0, 0.0));
    assert_eq!(right_rect.center, center_rect.center);
}

#[test]
fn ui_center_and_top_anchor_translation_can_reach_same_parent_point() {
    let mut runtime = Runtime::new();
    runtime.set_viewport_size(800, 600);

    let center = insert_panel(&mut runtime, [100.0, 150.0], Color::new(0.1, 0.2, 0.3, 1.0));
    if let Some(scene_node) = runtime.nodes.get_mut(center)
        && let SceneNodeData::UiPanel(panel) = &mut scene_node.data
    {
        panel.layout.anchor = UiAnchor::Center;
        panel.transform.translation = Vector2::new(0.0, 0.25);
    }

    let top = insert_panel(&mut runtime, [100.0, 150.0], Color::new(0.1, 0.2, 0.3, 1.0));
    if let Some(scene_node) = runtime.nodes.get_mut(top)
        && let SceneNodeData::UiPanel(panel) = &mut scene_node.data
    {
        panel.layout.anchor = UiAnchor::Top;
        panel.transform.translation = Vector2::new(0.0, -0.125);
    }

    runtime.extract_render_ui_commands();

    let center_rect = runtime
        .render_ui
        .computed_rects
        .get(&center)
        .copied()
        .expect("center rect");
    let top_rect = runtime
        .render_ui
        .computed_rects
        .get(&top)
        .copied()
        .expect("top rect");
    assert_eq!(center_rect.center, Vector2::new(0.0, 150.0));
    assert_eq!(top_rect.center, center_rect.center);
}

#[test]
fn ui_parent_scale_preserves_child_virtual_layout_size() {
    let mut runtime = Runtime::new();
    runtime.set_viewport_size(800, 600);

    let mut parent = UiPanel::new();
    parent.layout.size = UiVector2::pixels(200.0, 100.0);
    parent.transform.scale = Vector2::new(0.5, 0.5);
    let parent = insert_ui_node(&mut runtime, SceneNodeData::UiPanel(parent));

    let mut child = UiPanel::new();
    child.layout.size = UiVector2::ratio(1.0, 1.0);
    let child = insert_ui_node(&mut runtime, SceneNodeData::UiPanel(child));
    attach_child(&mut runtime, parent, child);

    runtime.extract_render_ui_commands();

    let parent_rect = runtime
        .render_ui
        .computed_rects
        .get(&parent)
        .expect("parent rect exists");
    let child_rect = runtime
        .render_ui
        .computed_rects
        .get(&child)
        .expect("child rect exists");

    assert_eq!(parent_rect.size, Vector2::new(100.0, 50.0));
    assert_eq!(child_rect.center, parent_rect.center);
    assert_eq!(child_rect.size, parent_rect.size);
}

#[test]
fn dirty_ui_node_emits_changed_upsert_only() {
    let mut runtime = Runtime::new();
    runtime.set_viewport_size(800, 600);
    let node = insert_panel(&mut runtime, [120.0, 40.0], Color::new(0.1, 0.2, 0.3, 1.0));

    runtime.extract_render_ui_commands();
    let mut commands = Vec::new();
    runtime.drain_render_commands(&mut commands);
    runtime.clear_dirty_flags();

    if let Some(scene_node) = runtime.nodes.get_mut(node)
        && let SceneNodeData::UiPanel(panel) = &mut scene_node.data
    {
        panel.style.fill = Color::new(0.8, 0.1, 0.1, 1.0);
    }
    runtime.mark_needs_rerender(node);
    runtime.extract_render_ui_commands();
    commands.clear();
    runtime.drain_render_commands(&mut commands);

    assert_eq!(commands.len(), 1);
    assert!(
        matches!(&commands[0], RenderCommand::Ui(UiCommand::UpsertPanel { node: n, fill, .. }) if *n == node && *fill == rgba(0.8, 0.1, 0.1, 1.0))
    );
}

#[test]
fn ui_reparent_marks_layout_dirty_without_resize() {
    let mut runtime = Runtime::new();
    runtime.set_viewport_size(800, 600);

    let mut parent_a = UiPanel::new();
    parent_a.layout.size = UiVector2::pixels(200.0, 200.0);
    parent_a.transform.translation.x = -0.125;
    let parent_a = insert_ui_node(&mut runtime, SceneNodeData::UiPanel(parent_a));

    let mut parent_b = UiPanel::new();
    parent_b.layout.size = UiVector2::pixels(200.0, 200.0);
    parent_b.transform.translation.x = 0.125;
    let parent_b = insert_ui_node(&mut runtime, SceneNodeData::UiPanel(parent_b));

    let child = insert_panel(&mut runtime, [40.0, 40.0], Color::new(0.1, 0.2, 0.3, 1.0));
    attach_child(&mut runtime, parent_a, child);

    runtime.extract_render_ui_commands();
    runtime.drain_render_commands(&mut Vec::new());
    runtime.clear_dirty_flags();

    assert!(runtime.reparent(parent_b, child));
    runtime.extract_render_ui_commands();
    let mut commands = Vec::new();
    runtime.drain_render_commands(&mut commands);

    assert!(commands.iter().any(|cmd| matches!(
        cmd,
        RenderCommand::Ui(UiCommand::UpsertPanel { node, rect, .. })
            if *node == child && rect.center == [100.0, 0.0]
    )));
}

#[test]
fn ui_descendant_reparented_via_non_ui_wrapper_recomputes_parent_space() {
    let mut runtime = Runtime::new();
    runtime.set_viewport_size(800, 600);

    let mut preview = UiPanel::new();
    preview.layout.size = UiVector2::ratio(1.0, 1.0);
    preview.transform.scale = Vector2::new(0.5, 0.5);
    let preview = insert_ui_node(&mut runtime, SceneNodeData::UiPanel(preview));

    let wrapper = runtime.create::<perro_nodes::Node2D>();
    let mut ui_root = UiPanel::new();
    ui_root.layout.size = UiVector2::ratio(1.0, 1.0);
    let ui_root = insert_ui_node(&mut runtime, SceneNodeData::UiPanel(ui_root));
    attach_child(&mut runtime, wrapper, ui_root);

    runtime.extract_render_ui_commands();
    runtime.drain_render_commands(&mut Vec::new());
    runtime.clear_dirty_flags();

    assert!(runtime.reparent(preview, wrapper));
    runtime.extract_render_ui_commands();
    let mut commands = Vec::new();
    runtime.drain_render_commands(&mut commands);

    assert!(commands.iter().any(|cmd| matches!(
        cmd,
        RenderCommand::Ui(UiCommand::UpsertPanel { node, rect, .. })
            if *node == ui_root && rect.size == [400.0, 300.0]
    )));
}

#[test]
fn ui_descendant_under_node3d_wrapper_resolves_against_closest_ui_parent() {
    let mut runtime = Runtime::new();
    runtime.set_viewport_size(800, 600);

    let mut root = UiPanel::new();
    root.layout.size = UiVector2::ratio(0.5, 0.5);
    let root = insert_ui_node(&mut runtime, SceneNodeData::UiPanel(root));

    let wrapper = runtime.create::<perro_nodes::Node3D>();
    let mut child = UiPanel::new();
    child.layout.size = UiVector2::ratio(1.0, 1.0);
    let child = insert_ui_node(&mut runtime, SceneNodeData::UiPanel(child));
    attach_child(&mut runtime, wrapper, child);
    attach_child(&mut runtime, root, wrapper);

    runtime.extract_render_ui_commands();

    let root_rect = runtime
        .render_ui
        .computed_rects
        .get(&root)
        .copied()
        .expect("root rect exists");
    let child_rect = runtime
        .render_ui
        .computed_rects
        .get(&child)
        .copied()
        .expect("child rect exists");
    assert_eq!(child_rect.size, root_rect.size);
    assert_eq!(child_rect.center, root_rect.center);
}

#[test]
fn ui_descendant_under_animation_player_wrapper_resolves_against_closest_ui_parent() {
    let mut runtime = Runtime::new();
    runtime.set_viewport_size(800, 600);

    let mut root = UiPanel::new();
    root.layout.size = UiVector2::ratio(0.5, 0.5);
    let root = insert_ui_node(&mut runtime, SceneNodeData::UiPanel(root));

    let wrapper = runtime.create::<perro_nodes::AnimationPlayer>();
    let mut child = UiPanel::new();
    child.layout.size = UiVector2::ratio(1.0, 1.0);
    let child = insert_ui_node(&mut runtime, SceneNodeData::UiPanel(child));
    attach_child(&mut runtime, wrapper, child);
    attach_child(&mut runtime, root, wrapper);

    runtime.extract_render_ui_commands();

    let root_rect = runtime
        .render_ui
        .computed_rects
        .get(&root)
        .copied()
        .expect("root rect exists");
    let child_rect = runtime
        .render_ui
        .computed_rects
        .get(&child)
        .copied()
        .expect("child rect exists");
    assert_eq!(child_rect.size, root_rect.size);
    assert_eq!(child_rect.center, root_rect.center);
}

#[test]
fn ui_auto_layout_includes_ui_descendants_through_non_ui_wrappers() {
    let mut runtime = Runtime::new();
    runtime.set_viewport_size(800, 600);

    let mut layout = UiHLayout::new();
    layout.layout.size = UiVector2::pixels(300.0, 120.0);
    layout.inner.h_spacing = 20.0 / 300.0;
    let layout = insert_ui_node(&mut runtime, SceneNodeData::UiHLayout(layout));

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
    let layout = insert_ui_node(&mut runtime, SceneNodeData::UiVLayout(layout));

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
    if let Some(scene_node) = runtime.nodes.get_mut(node)
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
    let label = insert_ui_node(&mut runtime, SceneNodeData::UiLabel(label));

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
    if let Some(scene_node) = runtime.nodes.get_mut(node)
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
    if let Some(scene_node) = runtime.nodes.get_mut(node)
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

    if let Some(scene_node) = runtime.nodes.get_mut(node)
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

    if let Some(scene_node) = runtime.nodes.get_mut(node)
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
    if let Some(scene_node) = runtime.nodes.get_mut(node)
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
    let node = insert_ui_node(&mut runtime, SceneNodeData::UiShape(shape));

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
    if let Some(scene_node) = runtime.nodes.get_mut(node)
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
    let node = insert_ui_node(&mut runtime, SceneNodeData::UiTextBox(text_box));

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
    let node = insert_ui_node(&mut runtime, SceneNodeData::UiTextBox(text_box));

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
    let node = insert_ui_node(&mut runtime, SceneNodeData::UiTextBox(text_box));

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

    if let Some(scene_node) = runtime.nodes.get_mut(hidden)
        && let SceneNodeData::UiButton(button) = &mut scene_node.data
    {
        button.visible = false;
    }
    if let Some(scene_node) = runtime.nodes.get_mut(disabled)
        && let SceneNodeData::UiButton(button) = &mut scene_node.data
    {
        button.disabled = true;
    }
    if let Some(scene_node) = runtime.nodes.get_mut(input_disabled)
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
    let clip_parent = insert_ui_node(&mut runtime, SceneNodeData::UiPanel(clip_parent));

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
    let clip_parent = insert_ui_node(&mut runtime, SceneNodeData::UiPanel(clip_parent));

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
    let clip_parent = insert_ui_node(&mut runtime, SceneNodeData::UiPanel(clip_parent));

    let mut button = perro_ui::UiImageButton::new();
    button.layout.size = UiVector2::pixels(120.0, 40.0);
    button.texture = TextureID::from_parts(99, 0);
    let button = insert_ui_node(&mut runtime, SceneNodeData::UiImageButton(button));
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
    let scroller = insert_ui_node(&mut runtime, SceneNodeData::UiScrollContainer(scroller));

    let mut list = UiVLayout::new();
    list.layout.size = UiVector2::pixels(220.0, 120.0);
    let list = insert_ui_node(&mut runtime, SceneNodeData::UiVLayout(list));
    attach_child(&mut runtime, scroller, list);

    let button = insert_button(&mut runtime, [140.0, 44.0]);
    attach_child(&mut runtime, list, button);

    let mut panel = UiPanel::new();
    panel.layout.size = UiVector2::pixels(180.0, 90.0);
    panel.layout.z_index = 20;
    let panel = insert_ui_node(&mut runtime, SceneNodeData::UiPanel(panel));

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
    if let Some(scene_node) = runtime.nodes.get_mut(button)
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
    if let Some(scene_node) = runtime.nodes.get_mut(right)
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

#[test]
fn ui_input_mask_filters_mouse_joycon_and_button_activation_sources() {
    let mut runtime = Runtime::new();
    runtime.set_viewport_size(800, 600);
    let button = insert_button_at(&mut runtime, [120.0, 40.0], 0.0, 0.0);
    if let Some(scene_node) = runtime.nodes.get_mut(button)
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
    let node = insert_ui_node(&mut runtime, SceneNodeData::UiButton(button));

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
    let node = insert_ui_node(&mut runtime, SceneNodeData::UiButton(button));

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
    let custom = insert_ui_node(&mut runtime, SceneNodeData::UiButton(button));
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
    let node = insert_ui_node(&mut runtime, SceneNodeData::UiImageButton(button));
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
    if let Some(scene_node) = runtime.nodes.get_mut(node)
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
    if let Some(scene_node) = runtime.nodes.get_mut(node)
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
        SceneNodeData::UiTextBox(perro_ui::UiTextBox::new()),
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
    let custom = insert_ui_node(&mut runtime, SceneNodeData::UiTextBlock(text_block));
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
    let parent = insert_ui_node(&mut runtime, SceneNodeData::UiPanel(parent_panel));

    let mut label = perro_ui::UiLabel::new().with_text("Scaled");
    label.layout.size = UiVector2::pixels(200.0, 40.0);
    label.font_size = 20.0;
    label.text_size_ratio = 0.0;
    let child = insert_ui_node(&mut runtime, SceneNodeData::UiLabel(label));
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
    let node = insert_ui_node(&mut runtime, SceneNodeData::UiLabel(label));

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
    let node = insert_ui_node(&mut runtime, SceneNodeData::UiTextBox(text_box));

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
    let parent = insert_ui_node(&mut runtime, SceneNodeData::UiPanel(parent_panel));

    let mut child_panel = UiPanel::new();
    child_panel.layout.size = UiVector2::pixels(200.0, 40.0);
    child_panel.style.set_corner_radius(0.4);
    child_panel.style.stroke_width = 2.0;
    let child = insert_ui_node(&mut runtime, SceneNodeData::UiPanel(child_panel));
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

    if let Some(scene_node) = runtime.nodes.get_mut(child)
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

    if let Some(scene_node) = runtime.nodes.get_mut(child)
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
    let parent_id = insert_ui_node(&mut runtime, SceneNodeData::UiPanel(parent));

    let mut child = UiPanel::new();
    child.layout.size = UiVector2::ratio(0.5, 0.5);
    child.layout.h_size = UiSizeMode::FitChildren;
    child.layout.v_size = UiSizeMode::FitChildren;
    child.layout.max_size_scale = Vector2::new(2.0, 2.0);
    let child_id = insert_ui_node(&mut runtime, SceneNodeData::UiPanel(child));
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
    let parent_id = insert_ui_node(&mut runtime, SceneNodeData::UiPanel(parent));

    let mut child = UiPanel::new();
    child.layout.size = UiVector2::ratio(0.5, 0.5);
    child.layout.h_size = UiSizeMode::FitChildren;
    child.layout.v_size = UiSizeMode::FitChildren;
    child.layout.max_size_scale = Vector2::new(2.0, 2.0);
    let child_id = insert_ui_node(&mut runtime, SceneNodeData::UiPanel(child));
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

    if let Some(scene_node) = runtime.nodes.get_mut(child_id)
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
    let top_id = insert_ui_node(&mut runtime, SceneNodeData::UiPanel(top));

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
    let panel_id = insert_ui_node(&mut runtime, SceneNodeData::UiPanel(panel));

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

    if let Some(scene_node) = runtime.nodes.get_mut(panel_id)
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
fn label_fill_mode_uses_parent_space_without_auto_layout_parent() {
    let mut runtime = Runtime::new();
    runtime.set_viewport_size(800, 600);

    let mut parent = UiPanel::new();
    parent.layout.size = UiVector2::ratio(1.0, 1.0);
    let parent_id = insert_ui_node(&mut runtime, SceneNodeData::UiPanel(parent));

    let mut label = perro_ui::UiLabel::new().with_text("HP");
    label.layout.h_size = UiSizeMode::Fill;
    label.layout.v_size = UiSizeMode::Fill;
    label.text_size_ratio = 1.0;
    let label_id = insert_ui_node(&mut runtime, SceneNodeData::UiLabel(label));
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
    let scroller_id = insert_ui_node(&mut runtime, SceneNodeData::UiScrollContainer(scroller));

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
    let scroller_id = insert_ui_node(&mut runtime, SceneNodeData::UiScrollContainer(scroller));

    let mut list = UiVLayout::new();
    list.layout.size = UiVector2::pixels(200.0, 300.0);
    let list_id = insert_ui_node(&mut runtime, SceneNodeData::UiVLayout(list));
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
    let scroller_id = insert_ui_node(&mut runtime, SceneNodeData::UiScrollContainer(scroller));

    let mut list = UiVLayout::new();
    list.layout.size = UiVector2::pixels(200.0, 300.0);
    let list_id = insert_ui_node(&mut runtime, SceneNodeData::UiVLayout(list));
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
    let scroller_id = insert_ui_node(&mut runtime, SceneNodeData::UiScrollContainer(scroller));

    let mut list = UiVLayout::new();
    list.layout.size = UiVector2::pixels(200.0, 300.0);
    let list_id = insert_ui_node(&mut runtime, SceneNodeData::UiVLayout(list));
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
    let scroller_id = insert_ui_node(&mut runtime, SceneNodeData::UiScrollContainer(scroller));

    let mut list = UiVLayout::new();
    list.layout.size = UiVector2::pixels(200.0, 300.0);
    let list_id = insert_ui_node(&mut runtime, SceneNodeData::UiVLayout(list));
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
fn scroll_container_reserves_default_gap_for_scrollbar() {
    let mut runtime = Runtime::new();
    runtime.set_viewport_size(800, 600);

    let mut scroller = UiScrollContainer::new();
    scroller.layout.size = UiVector2::pixels(200.0, 100.0);
    let scroller_id = insert_ui_node(&mut runtime, SceneNodeData::UiScrollContainer(scroller));

    let mut list = UiVLayout::new();
    list.layout.h_size = UiSizeMode::Fill;
    list.layout.size = UiVector2::pixels(0.0, 300.0);
    let list_id = insert_ui_node(&mut runtime, SceneNodeData::UiVLayout(list));
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
    let scroller_id = insert_ui_node(&mut runtime, SceneNodeData::UiScrollContainer(scroller));

    let mut list = UiVLayout::new();
    list.layout.h_size = UiSizeMode::Fill;
    list.layout.size = UiVector2::pixels(0.0, 300.0);
    let list_id = insert_ui_node(&mut runtime, SceneNodeData::UiVLayout(list));
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
    let scroller_id = insert_ui_node(&mut runtime, SceneNodeData::UiScrollContainer(scroller));

    let mut list = UiVLayout::new();
    list.layout.size = UiVector2::pixels(200.0, 300.0);
    let list_id = insert_ui_node(&mut runtime, SceneNodeData::UiVLayout(list));
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
    let scroller_id = insert_ui_node(&mut runtime, SceneNodeData::UiScrollContainer(scroller));

    let mut list = UiVLayout::new();
    list.layout.size = UiVector2::pixels(200.0, 300.0);
    let list_id = insert_ui_node(&mut runtime, SceneNodeData::UiVLayout(list));
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
    let scroller_id = insert_ui_node(&mut runtime, SceneNodeData::UiScrollContainer(scroller));

    let mut list = UiVLayout::new();
    list.layout.size = UiVector2::pixels(200.0, 300.0);
    let list_id = insert_ui_node(&mut runtime, SceneNodeData::UiVLayout(list));
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
    let scroller_id = insert_ui_node(&mut runtime, SceneNodeData::UiScrollContainer(scroller));

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
fn keyboard_scroll_targets_focused_scroll_container_ancestor() {
    let mut runtime = Runtime::new();
    runtime.set_viewport_size(800, 600);

    let mut scroller = UiScrollContainer::new();
    scroller.layout.size = UiVector2::pixels(200.0, 100.0);
    let scroller_id = insert_ui_node(&mut runtime, SceneNodeData::UiScrollContainer(scroller));

    let mut button = perro_ui::UiButton::new();
    button.layout.size = UiVector2::pixels(120.0, 40.0);
    let button_id = insert_ui_node(&mut runtime, SceneNodeData::UiButton(button));
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
    let scroller_id = insert_ui_node(&mut runtime, SceneNodeData::UiScrollContainer(scroller));

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
    let scroller_id = insert_ui_node(&mut runtime, SceneNodeData::UiScrollContainer(scroller));

    let mut text_block = perro_ui::UiTextBlock::new();
    text_block.inner.base.layout.size = UiVector2::pixels(220.0, 100.0);
    text_block.inner.text = Cow::Borrowed("line1\nline2\nline3\nline4\nline5\nline6");
    let text_id = insert_ui_node(&mut runtime, SceneNodeData::UiTextBlock(text_block));
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
    let label = insert_ui_node(&mut runtime, SceneNodeData::UiLabel(label));
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
    let label = insert_ui_node(&mut runtime, SceneNodeData::UiLabel(label));
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
    let label = insert_ui_node(&mut runtime, SceneNodeData::UiLabel(label));
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
    let parent = insert_ui_node(&mut runtime, SceneNodeData::UiPanel(parent));
    let button = insert_button(&mut runtime, [120.0, 40.0]);
    let mut label = perro_ui::UiLabel::new();
    label.layout.size = UiVector2::pixels(120.0, 30.0);
    label.text = "Play".into();
    let label = insert_ui_node(&mut runtime, SceneNodeData::UiLabel(label));
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
    let label = insert_ui_node(&mut runtime, SceneNodeData::UiLabel(label));
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
    let content = insert_ui_node(&mut runtime, SceneNodeData::UiVLayout(content));
    attach_child(&mut runtime, root, content);

    let mut grid = UiVLayout::new();
    grid.layout.size = UiVector2::ratio(1.0, 0.72);
    let grid = insert_ui_node(&mut runtime, SceneNodeData::UiVLayout(grid));
    attach_child(&mut runtime, content, grid);

    let row_top = insert_ui_node(&mut runtime, SceneNodeData::UiHLayout(UiHLayout::new()));
    let row_bottom = insert_ui_node(&mut runtime, SceneNodeData::UiHLayout(UiHLayout::new()));
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
        let label = insert_ui_node(&mut runtime, SceneNodeData::UiLabel(label));
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
    let tree_id = insert_ui_node(&mut runtime, SceneNodeData::UiTreeList(tree));

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

    if let Some(scene_node) = runtime.nodes.get_mut(tree_id)
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

    if let Some(scene_node) = runtime.nodes.get_mut(tree_id)
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
    let dropdown_id = insert_ui_node(&mut runtime, SceneNodeData::UiDropdown(dropdown));

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

fn insert_panel(runtime: &mut Runtime, size: [f32; 2], fill: Color) -> NodeID {
    let mut panel = UiPanel::new();
    panel.layout.size = UiVector2::pixels(size[0], size[1]);
    panel.style.fill = fill;
    insert_ui_node(runtime, SceneNodeData::UiPanel(panel))
}

fn insert_button(runtime: &mut Runtime, size: [f32; 2]) -> NodeID {
    let mut button = perro_ui::UiButton::new();
    button.layout.size = UiVector2::pixels(size[0], size[1]);
    button.style.fill = Color::new(0.1, 0.2, 0.3, 1.0);
    button.hover_style.fill = Color::new(0.2, 0.3, 0.4, 1.0);
    button.pressed_style.fill = Color::new(0.3, 0.4, 0.5, 1.0);
    insert_ui_node(runtime, SceneNodeData::UiButton(button))
}

fn insert_button_at(runtime: &mut Runtime, size: [f32; 2], x: f32, y: f32) -> NodeID {
    let mut button = perro_ui::UiButton::new();
    button.layout.size = UiVector2::pixels(size[0], size[1]);
    button.transform.position = UiVector2::pixels(x, y);
    button.style.fill = Color::new(0.1, 0.2, 0.3, 1.0);
    button.hover_style.fill = Color::new(0.2, 0.3, 0.4, 1.0);
    button.pressed_style.fill = Color::new(0.3, 0.4, 0.5, 1.0);
    insert_ui_node(runtime, SceneNodeData::UiButton(button))
}

fn insert_text_box_at(runtime: &mut Runtime, x: f32, y: f32) -> NodeID {
    let mut text_box = perro_ui::UiTextBox::new();
    text_box.inner.base.layout.size = UiVector2::pixels(140.0, 40.0);
    text_box.inner.base.transform.position = UiVector2::pixels(x, y);
    insert_ui_node(runtime, SceneNodeData::UiTextBox(text_box))
}

fn insert_text_block_at(runtime: &mut Runtime, x: f32, y: f32) -> NodeID {
    let mut text_block = perro_ui::UiTextBlock::new();
    text_block.inner.base.layout.size = UiVector2::pixels(140.0, 80.0);
    text_block.inner.base.transform.position = UiVector2::pixels(x, y);
    insert_ui_node(runtime, SceneNodeData::UiTextBlock(text_block))
}

fn tap_key_and_extract(runtime: &mut Runtime, key: KeyCode) {
    runtime.begin_input_frame();
    runtime.set_key_state(key, true);
    runtime.extract_render_ui_commands();
    runtime.set_key_state(key, false);
}

fn click_mouse_and_extract(runtime: &mut Runtime, x: f32, y: f32) {
    runtime.begin_input_frame();
    runtime.set_mouse_position(x, y);
    runtime.set_mouse_button_state(MouseButton::Left, true);
    runtime.extract_render_ui_commands();
    runtime.set_mouse_button_state(MouseButton::Left, false);
}

fn set_panel_visible(runtime: &mut Runtime, node: NodeID, visible: bool) {
    if let Some(scene_node) = runtime.nodes.get_mut(node)
        && let SceneNodeData::UiPanel(panel) = &mut scene_node.data
    {
        panel.visible = visible;
    }
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

fn insert_ui_node(runtime: &mut Runtime, data: SceneNodeData) -> NodeID {
    let node = runtime.nodes.insert(SceneNode::new(data));
    runtime
        .nodes
        .get_mut(node)
        .expect("inserted node exists")
        .id = node;
    runtime.mark_needs_rerender(node);
    node
}

fn attach_child(runtime: &mut Runtime, parent: NodeID, child: NodeID) {
    runtime
        .nodes
        .get_mut(parent)
        .expect("parent exists")
        .add_child(child);
    runtime.nodes.get_mut(child).expect("child exists").parent = parent;
    runtime.mark_needs_rerender(parent);
    runtime.mark_needs_rerender(child);
}

struct HideClickedButtonScript {
    calls: Arc<AtomicUsize>,
}

impl ScriptLifecycle<RuntimeScriptApi> for HideClickedButtonScript {}

impl ScriptBehavior<RuntimeScriptApi> for HideClickedButtonScript {
    fn script_flags(&self) -> ScriptFlags {
        ScriptFlags::new(ScriptFlags::NONE)
    }

    fn create_state(&self) -> Box<dyn Any> {
        Box::new(())
    }

    fn get_var(&self, _state: &dyn Any, _var: ScriptMemberID) -> Variant {
        Variant::Null
    }

    fn set_var(&self, _state: &mut dyn Any, _var: ScriptMemberID, _value: Variant) {}

    fn call_method(
        &self,
        _method: ScriptMemberID,
        ctx: &mut ScriptContext<'_, RuntimeScriptApi>,
        params: &[Variant],
    ) -> Variant {
        self.calls.fetch_add(1, Ordering::Relaxed);
        if let Some(button_id) = params.first().and_then(Variant::as_node) {
            let _ =
                ctx.run
                    .Nodes()
                    .with_node_mut::<perro_ui::UiButton, _, _>(button_id, |button| {
                        button.visible = false;
                    });
        }
        Variant::Null
    }
}
