use super::{RectInstanceGpu, Renderer2D};
use crate::resources::ResourceStore;
use perro_ids::{NodeID, TextureID};
use perro_render_bridge::{Rect2DCommand, Sprite2DCommand};

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
            z_index: 0,
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
            z_index: 1,
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
            z_index: 1,
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
        color: [1.0, 0.0, 0.0, 1.0],
        z_index: 1,
    };

    renderer.queue_rect(node, rect);
    let (_, _, first_plan) = renderer.prepare_frame(&resources);
    assert!(first_plan.full_reupload);
    assert_eq!(first_plan.draw_count, 1);

    renderer.queue_rect(
        node,
        Rect2DCommand {
            color: [0.0, 1.0, 0.0, 1.0],
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
            color: [0.0, 1.0, 0.0, 1.0],
            z_index: 1,
        }
    );
}
