use super::{RectInstanceGpu, Renderer2D};
use crate::resources::ResourceStore;
use perro_ids::{NodeID, TextureID};
use perro_render_bridge::{
    DrawShape2DCommand, Light2DState, PointLight2DState, Rect2DCommand, ShadowCaster2DShapeState,
    ShadowCaster2DState, Sprite2DCommand,
};
use perro_structs::{Color, DrawShape2D, Vector2};

#[test]
fn texture_upsert_requires_existing_resource() {
    let mut renderer = Renderer2D::new();
    let mut resources = ResourceStore::new();
    let node = NodeID::from_parts(2, 0);
    let missing = TextureID::from_parts(10, 0);
    renderer.queue_sprite(
        node,
        Sprite2DCommand {
            texture: missing,
            model: [[1.0, 0.0, 0.0], [0.0, 1.0, 0.0], [0.0, 0.0, 1.0]],
            tint: Color::WHITE,
            z_index: 0,
            ..Sprite2DCommand::default()
        },
    );
    let (_, stats, _) = renderer.prepare_frame(&resources);
    assert_eq!(stats.accepted_draws, 0);
    assert_eq!(stats.rejected_draws, 1);
    assert_eq!(renderer.retained_sprite(node), None);

    let loaded = resources.create_texture("__test__", false);
    renderer.queue_sprite(
        node,
        Sprite2DCommand {
            texture: loaded,
            model: [[1.0, 0.0, 2.0], [0.0, 1.0, 3.0], [0.0, 0.0, 1.0]],
            tint: Color::WHITE,
            z_index: 1,
            ..Sprite2DCommand::default()
        },
    );
    let (_, stats, _) = renderer.prepare_frame(&resources);
    assert_eq!(stats.accepted_draws, 1);
    assert_eq!(stats.rejected_draws, 0);
    assert_eq!(
        renderer.retained_sprite(node),
        Some(Sprite2DCommand {
            texture: loaded,
            model: [[1.0, 0.0, 2.0], [0.0, 1.0, 3.0], [0.0, 0.0, 1.0]],
            tint: Color::WHITE,
            z_index: 1,
            ..Sprite2DCommand::default()
        })
    );
    assert_eq!(renderer.retained_sprite_count(), 1);
}

#[test]
fn rect_upload_plan_tracks_incremental_updates() {
    let mut renderer = Renderer2D::new();
    let resources = ResourceStore::new();
    let node = NodeID::from_parts(5, 0);
    let rect = Rect2DCommand {
        center: [0.0, 0.0],
        size: [32.0, 32.0],
        color: Color::RED,
        z_index: 1,
    };

    renderer.queue_rect(node, rect);
    let (_, _, first_plan) = renderer.prepare_frame(&resources);
    assert!(first_plan.full_reupload);
    assert_eq!(first_plan.draw_count, 1);

    renderer.queue_rect(
        node,
        Rect2DCommand {
            color: Color::GREEN,
            ..rect
        },
    );
    let (_, _, second_plan) = renderer.prepare_frame(&resources);
    assert!(!second_plan.full_reupload);
    assert_eq!(second_plan.dirty_ranges, vec![0..1]);
    assert_eq!(
        renderer.retained_rects()[0],
        RectInstanceGpu {
            center: [0.0, 0.0],
            size: [32.0, 32.0],
            color: [0, 255, 0, 255],
            z_index: 1,
            // packed shape_kind = kind(0) * 2 + filled(1) = 1
            shape_kind: 1,
            thickness: 1.0,
        }
    );
}

#[test]
fn rect_upload_plan_keeps_10k_updates_incremental() {
    let mut renderer = Renderer2D::new();
    let resources = ResourceStore::new();
    let rect = Rect2DCommand {
        center: [0.0, 0.0],
        size: [8.0, 8.0],
        color: Color::RED,
        z_index: 1,
    };

    for i in 0..10_000u32 {
        renderer.queue_rect(NodeID::from_parts(i + 1, 0), rect);
    }
    let (_, _, first_plan) = renderer.prepare_frame(&resources);
    assert!(first_plan.full_reupload);
    assert_eq!(first_plan.draw_count, 10_000);

    renderer.queue_rect(
        NodeID::from_parts(5_000, 0),
        Rect2DCommand {
            color: Color::GREEN,
            ..rect
        },
    );
    let (_, _, second_plan) = renderer.prepare_frame(&resources);
    assert!(!second_plan.full_reupload);
    assert_eq!(second_plan.dirty_ranges.len(), 1);
    assert_eq!(second_plan.draw_count, 10_000);
}

#[test]
fn draw_shape_uses_normalized_screen_position_with_center_at_half() {
    let mut renderer = Renderer2D::new();
    let resources = ResourceStore::new();
    renderer.queue_shape(DrawShape2DCommand {
        shape: DrawShape2D::circle(12.0, Color::WHITE),
        position: [0.5, 0.5],
    });

    let _ = renderer.prepare_frame(&resources);
    let frame_shapes = renderer.frame_shapes();
    assert_eq!(frame_shapes.len(), 1);
    assert_eq!(frame_shapes[0].center, [0.0, 0.0]);
}

#[test]
fn draw_shape_uses_top_right_for_one_one() {
    let mut renderer = Renderer2D::new();
    let resources = ResourceStore::new();
    renderer.queue_shape(DrawShape2DCommand {
        shape: DrawShape2D::circle(12.0, Color::WHITE),
        position: [1.0, 1.0],
    });

    let _ = renderer.prepare_frame(&resources);
    let frame_shapes = renderer.frame_shapes();
    assert_eq!(frame_shapes.len(), 1);
    assert_eq!(frame_shapes[0].center, [960.0, 540.0]);
}

#[test]
fn draw_line_emits_line_shape_instance() {
    let mut renderer = Renderer2D::new();
    let resources = ResourceStore::new();
    renderer.queue_shape(DrawShape2DCommand {
        shape: DrawShape2D::line(Vector2::new(0.75, 0.5), Color::RED, 3.0),
        position: [0.25, 0.5],
    });

    let _ = renderer.prepare_frame(&resources);
    let frame_shapes = renderer.frame_shapes();
    assert_eq!(frame_shapes.len(), 1);
    // packed shape_kind = kind(3) * 2 + filled(1) = 7
    assert_eq!(frame_shapes[0].shape_kind, 7);
    assert_eq!(frame_shapes[0].thickness, 3.0);
}

#[test]
fn draw_sprite_is_transient_sprite() {
    let mut renderer = Renderer2D::new();
    let resources = ResourceStore::new();
    let texture = TextureID::from_parts(7, 0);
    renderer.queue_shape(DrawShape2DCommand {
        shape: DrawShape2D::sprite(texture, Vector2::new(32.0, 16.0), Color::WHITE),
        position: [0.5, 0.5],
    });

    let _ = renderer.prepare_frame(&resources);
    assert_eq!(renderer.retained_sprite_count(), 1);
    assert!(
        renderer
            .retained_sprites()
            .any(|sprite| sprite.texture == texture)
    );
}

#[test]
fn point_light_is_retained_and_removed_by_node() {
    let mut renderer = Renderer2D::new();
    let node = NodeID::from_parts(9, 0);
    let light = PointLight2DState {
        position: [12.0, -4.0],
        color: [1.0, 0.8, 0.4],
        intensity: 2.0,
        range: 128.0,
        z_index: 3,
        cast_shadows: true,
    };

    renderer.set_point_light(node, light);

    assert_eq!(renderer.light_count(), 1);
    assert!(
        renderer
            .lights()
            .any(|stored| stored == Light2DState::Point(light))
    );

    renderer.remove_node(node);

    assert_eq!(renderer.light_count(), 0);
}

#[test]
fn shadow_caster_is_retained_and_removed_by_node() {
    let mut renderer = Renderer2D::new();
    let node = NodeID::from_parts(10, 0);
    let caster = ShadowCaster2DState {
        center: [4.0, 8.0],
        half_extents: [16.0, 6.0],
        rotation_radians: 0.25,
        shape: ShadowCaster2DShapeState::Quad,
        z_index: 2,
    };

    renderer.upsert_shadow_caster(node, caster);

    assert!(renderer.shadow_casters().any(|stored| stored == caster));

    renderer.remove_node(node);

    assert_eq!(renderer.shadow_casters().count(), 0);
}

#[test]
fn retained_sprite_order_stays_stable_across_equal_upserts() {
    let mut renderer = Renderer2D::new();
    let mut resources = ResourceStore::new();
    let tex = resources.create_texture("__test__", false);

    for node_raw in [3u32, 7, 11] {
        renderer.queue_sprite(
            NodeID::from_parts(node_raw, 0),
            Sprite2DCommand {
                texture: tex,
                model: [
                    [1.0, 0.0, 0.0],
                    [0.0, 1.0, 0.0],
                    [node_raw as f32, 0.0, 1.0],
                ],
                tint: Color::WHITE,
                z_index: node_raw as i32,
                ..Sprite2DCommand::default()
            },
        );
    }
    let _ = renderer.prepare_frame(&resources);
    let first: Vec<_> = renderer
        .retained_sprites()
        .map(|sprite| sprite.z_index)
        .collect();

    for node_raw in [3u32, 7, 11] {
        renderer.queue_sprite(
            NodeID::from_parts(node_raw, 0),
            Sprite2DCommand {
                texture: tex,
                model: [
                    [1.0, 0.0, 0.0],
                    [0.0, 1.0, 0.0],
                    [node_raw as f32, 0.0, 1.0],
                ],
                tint: Color::WHITE,
                z_index: node_raw as i32,
                ..Sprite2DCommand::default()
            },
        );
    }
    let _ = renderer.prepare_frame(&resources);
    let second: Vec<_> = renderer
        .retained_sprites()
        .map(|sprite| sprite.z_index)
        .collect();

    assert_eq!(first, second);
    assert_eq!(second, vec![3, 7, 11]);
}
