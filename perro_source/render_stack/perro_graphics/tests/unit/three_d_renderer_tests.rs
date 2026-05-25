use super::{Draw3DKind, Renderer3D};
use crate::resources::ResourceStore;
use perro_ids::{MaterialID, NodeID};
use perro_render_bridge::{LODOptions3D, Material3D, MeshBlendOptions3D, MeshSurfaceBinding3D};
use perro_structs::Color;
use std::sync::Arc;

fn draw_surface(material: MaterialID) -> Arc<[MeshSurfaceBinding3D]> {
    Arc::from([MeshSurfaceBinding3D {
        material: Some(material),
        overrides: Arc::from([]),
        modulate: Color::WHITE,
    }])
}

#[test]
fn repeated_equal_draw_upsert_keep_revision_stable() {
    let mut renderer = Renderer3D::new();
    let mut resources = ResourceStore::new();
    let mesh = resources.create_mesh("__mesh__", false);
    let material = resources.create_material(Material3D::default(), Some("__mat__"), false);
    let node = NodeID::from_parts(5, 0);

    renderer.queue_draw(
        node,
        mesh,
        draw_surface(material),
        [
            [1.0, 0.0, 0.0, 2.0],
            [0.0, 1.0, 0.0, 0.0],
            [0.0, 0.0, 1.0, 3.0],
            [0.0, 0.0, 0.0, 1.0],
        ],
        None,
        Arc::from([]),
        None,
        LODOptions3D::default(),
        MeshBlendOptions3D::default(),
        true,
        true,
    );
    let _ = renderer.prepare_frame(&resources);
    let first_revision = renderer.draw_revision();

    renderer.queue_draw(
        node,
        mesh,
        draw_surface(material),
        [
            [1.0, 0.0, 0.0, 2.0],
            [0.0, 1.0, 0.0, 0.0],
            [0.0, 0.0, 1.0, 3.0],
            [0.0, 0.0, 0.0, 1.0],
        ],
        None,
        Arc::from([]),
        None,
        LODOptions3D::default(),
        MeshBlendOptions3D::default(),
        true,
        true,
    );
    let _ = renderer.prepare_frame(&resources);

    assert_eq!(renderer.draw_revision(), first_revision);
    assert_eq!(renderer.retained_draw_count(), 1);
    assert_eq!(
        renderer.retained_draw(node).unwrap().kind,
        Draw3DKind::Mesh(mesh)
    );
}

#[test]
fn repeated_equal_draw_upsert_keep_sorted_node_order() {
    let mut renderer = Renderer3D::new();
    let mut resources = ResourceStore::new();
    let mesh = resources.create_mesh("__mesh__", false);
    let material = resources.create_material(Material3D::default(), Some("__mat__"), false);

    for node_raw in [9u32, 12, 20] {
        renderer.queue_draw(
            NodeID::from_parts(node_raw, 0),
            mesh,
            draw_surface(material),
            [
                [1.0, 0.0, 0.0, node_raw as f32],
                [0.0, 1.0, 0.0, 0.0],
                [0.0, 0.0, 1.0, 0.0],
                [0.0, 0.0, 0.0, 1.0],
            ],
            None,
            Arc::from([]),
            None,
            LODOptions3D::default(),
            MeshBlendOptions3D::default(),
            true,
            true,
        );
    }
    let _ = renderer.prepare_frame(&resources);

    for node_raw in [9u32, 12, 20] {
        renderer.queue_draw(
            NodeID::from_parts(node_raw, 0),
            mesh,
            draw_surface(material),
            [
                [1.0, 0.0, 0.0, node_raw as f32],
                [0.0, 1.0, 0.0, 0.0],
                [0.0, 0.0, 1.0, 0.0],
                [0.0, 0.0, 0.0, 1.0],
            ],
            None,
            Arc::from([]),
            None,
            LODOptions3D::default(),
            MeshBlendOptions3D::default(),
            true,
            true,
        );
    }
    let _ = renderer.prepare_frame(&resources);

    let nodes: Vec<_> = renderer
        .retained_draws_sorted()
        .iter()
        .map(|draw| draw.node)
        .collect();
    assert_eq!(
        nodes,
        vec![
            NodeID::from_parts(9, 0),
            NodeID::from_parts(12, 0),
            NodeID::from_parts(20, 0),
        ]
    );
}

#[test]
fn camera_stream_quad_retains_texture_draw() {
    let mut renderer = Renderer3D::new();
    let mut resources = ResourceStore::new();
    let texture = resources.create_texture("__camera_stream__:7", true);
    let node = NodeID::from_parts(7, 0);

    renderer.queue_camera_stream_quad(
        node,
        texture,
        glam::Mat4::IDENTITY.to_cols_array_2d(),
        [2.0, 1.0],
        [1.0, 0.8, 0.6, 0.5],
    );
    let _ = renderer.prepare_frame(&resources);

    let retained = renderer.retained_draw(node).unwrap();
    assert_eq!(
        retained.kind,
        Draw3DKind::CameraStreamQuad {
            texture,
            tint: [1.0, 0.8, 0.6, 0.5]
        }
    );
    assert_eq!(retained.instance_mats.len(), 1);
}
