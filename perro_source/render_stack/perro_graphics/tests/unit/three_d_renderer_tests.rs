use super::{DenseMultiMeshDraw3D, Draw3DKind, Renderer3D};
use crate::resources::ResourceStore;
use perro_ids::{MaterialID, NodeID};
use perro_render_bridge::{
    CustomMaterial3D, DenseInstancePose3D, LODOptions3D, Material3D, MeshBlendOptions3D,
    MeshSurfaceBinding3D,
};
use perro_structs::Color;
use std::sync::Arc;

fn draw_surface(material: MaterialID) -> Arc<[MeshSurfaceBinding3D]> {
    Arc::from([MeshSurfaceBinding3D {
        material: Some(material),
        overrides: Arc::from([]),
        modulate: Color::WHITE,
    }])
}

fn queue_point(renderer: &mut Renderer3D, node: NodeID, x: f32) {
    renderer.queue_debug_point(node, [x, 0.0, 0.0], 1.0, [1.0; 4]);
}

fn seeded_points(count: u32) -> (Renderer3D, ResourceStore) {
    let resources = ResourceStore::new();
    let mut renderer = Renderer3D::new();
    // Reverse insertion ensures the sorted lookup differs from storage order.
    for id in (1..=count).rev() {
        queue_point(&mut renderer, NodeID::from_parts(id, 0), id as f32);
    }
    renderer.prepare_frame(&resources);
    assert_eq!(renderer.draw_cache_rebuilds, 0);
    assert_eq!(renderer.retained_draws_sorted().len(), count as usize);
    assert_eq!(renderer.draw_cache_rebuilds, 1);
    (renderer, resources)
}

#[test]
fn sparse_draw_updates_patch_only_changed_rows_without_rebuild() {
    let (mut renderer, resources) = seeded_points(100);
    let node = NodeID::from_parts(50, 0);
    let untouched = renderer.retained_draws_sorted()[0].instance_mats.clone();
    let revision = renderer.draw_revision();
    let bindings_revision = renderer.resource_binding_revision();
    let counts_revision = renderer.instance_count_revision();
    queue_point(&mut renderer, node, 999.0);
    let (_, stats, _) = renderer.prepare_frame(&resources);
    assert_eq!(stats.accepted_draws, 1);
    assert_eq!(renderer.draw_revision(), revision + 1);
    assert_eq!(renderer.resource_binding_revision(), bindings_revision);
    assert_eq!(renderer.instance_count_revision(), counts_revision);
    let rows = renderer.retained_draws_sorted();
    assert_eq!(rows[49].node, node);
    assert_eq!(rows[49].instance_mats[0][3][0], 999.0);
    assert!(Arc::ptr_eq(&rows[0].instance_mats, &untouched));
    assert_eq!(renderer.draw_cache_rebuilds, 1);
    queue_point(&mut renderer, node, 999.0);
    renderer.prepare_frame(&resources);
    assert_eq!(renderer.draw_revision(), revision + 1);
    assert_eq!(renderer.draw_cache_rebuilds, 1);
}

fn dense_draw(count: usize) -> DenseMultiMeshDraw3D {
    DenseMultiMeshDraw3D {
        node_model: glam::Mat4::IDENTITY.to_cols_array_2d(),
        instance_scale: 1.0,
        instances: (0..count)
            .map(|index| DenseInstancePose3D {
                position: [index as f32, 0.0, 0.0],
                scale: [1.0; 3],
                rotation: [0.0, 0.0, 0.0, 1.0],
                has_blend_shape_weight_override: false,
                blend_shape_weights: Arc::from([]),
            })
            .collect::<Vec<_>>()
            .into(),
    }
}

#[test]
fn binding_and_dense_count_revisions_follow_effective_retained_state() {
    let mut renderer = Renderer3D::new();
    let mut resources = ResourceStore::new();
    let mesh_a = resources.create_mesh("__mesh_a__", false);
    let mesh_b = resources.create_mesh("__mesh_b__", false);
    let material_a = resources.create_material(Material3D::default(), Some("__mat_a__"), false);
    let material_b = resources.create_material(Material3D::default(), Some("__mat_b__"), false);
    let missing_mesh = perro_ids::MeshID::from_parts(12_345, 0);
    let missing_material = MaterialID::from_parts(12_345, 0);
    let node = NodeID::from_parts(800, 0);
    let queue = |renderer: &mut Renderer3D, mesh, material, count| {
        renderer.queue_draw_multi_dense(
            node,
            mesh,
            draw_surface(material),
            dense_draw(count),
            Arc::from([]),
            None,
            LODOptions3D::default(),
            MeshBlendOptions3D::default(),
            true,
            true,
        );
    };

    queue(&mut renderer, mesh_a, material_a, 2);
    renderer.prepare_frame(&resources);
    let base_draw = renderer.draw_revision();
    let base_bindings = renderer.resource_binding_revision();
    let base_counts = renderer.instance_count_revision();

    queue(&mut renderer, mesh_a, material_b, 2);
    renderer.prepare_frame(&resources);
    assert_eq!(renderer.draw_revision(), base_draw + 1);
    assert_eq!(renderer.resource_binding_revision(), base_bindings + 1);
    assert_eq!(renderer.instance_count_revision(), base_counts);

    queue(&mut renderer, mesh_a, material_b, 3);
    renderer.prepare_frame(&resources);
    assert_eq!(renderer.resource_binding_revision(), base_bindings + 1);
    assert_eq!(renderer.instance_count_revision(), base_counts + 1);

    queue(&mut renderer, missing_mesh, missing_material, 4);
    let (_, stats, _) = renderer.prepare_frame(&resources);
    assert_eq!(stats.rejected_draws, 1);
    assert_eq!(renderer.resource_binding_revision(), base_bindings + 1);
    assert_eq!(renderer.instance_count_revision(), base_counts + 2);
    let retained = renderer.retained_draw(node).expect("retained draw");
    assert_eq!(retained.kind, Draw3DKind::Mesh(mesh_a));
    assert_eq!(retained.surfaces, draw_surface(material_b));
    assert_eq!(retained.retained_instance_count(), 4);

    queue(&mut renderer, mesh_b, missing_material, 4);
    renderer.prepare_frame(&resources);
    assert_eq!(renderer.resource_binding_revision(), base_bindings + 2);
    let retained = renderer.retained_draw(node).expect("retained draw");
    assert_eq!(retained.kind, Draw3DKind::Mesh(mesh_b));
    assert_eq!(retained.surfaces, draw_surface(material_b));

    queue(&mut renderer, missing_mesh, material_a, 4);
    renderer.prepare_frame(&resources);
    assert_eq!(renderer.resource_binding_revision(), base_bindings + 3);
    let retained = renderer.retained_draw(node).expect("retained draw");
    assert_eq!(retained.kind, Draw3DKind::Mesh(mesh_b));
    assert_eq!(retained.surfaces, draw_surface(material_a));
}

#[test]
fn missing_bindings_bump_only_when_requested_resources_become_retained() {
    let mut renderer = Renderer3D::new();
    let mut resources = ResourceStore::new();
    let mesh_a = resources.create_mesh("__ready_mesh__", false);
    let material_a = resources.create_material(Material3D::default(), Some("__ready_mat__"), false);
    let mesh_b = perro_ids::MeshID::from_parts(20_000, 0);
    let material_b = MaterialID::from_parts(20_000, 0);
    let node = NodeID::from_parts(801, 0);
    let queue = |renderer: &mut Renderer3D, mesh, material, x| {
        renderer.queue_draw(
            node,
            mesh,
            draw_surface(material),
            glam::Mat4::from_translation(glam::Vec3::X * x).to_cols_array_2d(),
            None,
            Arc::from([]),
            None,
            LODOptions3D::default(),
            MeshBlendOptions3D::default(),
            true,
            true,
        );
    };
    queue(&mut renderer, mesh_a, material_a, 0.0);
    renderer.prepare_frame(&resources);
    let binding_revision = renderer.resource_binding_revision();
    let count_revision = renderer.instance_count_revision();

    queue(&mut renderer, mesh_b, material_b, 1.0);
    renderer.prepare_frame(&resources);
    assert_eq!(renderer.resource_binding_revision(), binding_revision);
    assert_eq!(renderer.instance_count_revision(), count_revision);
    let retained = renderer.retained_draw(node).expect("retained draw");
    assert_eq!(retained.kind, Draw3DKind::Mesh(mesh_a));
    assert_eq!(retained.surfaces, draw_surface(material_a));

    assert_eq!(
        resources.create_mesh_with_id(mesh_b, "__became_ready_mesh__", false),
        mesh_b
    );
    assert_eq!(
        resources.create_material_with_id(
            material_b,
            Material3D::default(),
            Some("__became_ready_mat__"),
            false,
        ),
        material_b
    );
    assert_eq!(renderer.resource_binding_revision(), binding_revision);
    queue(&mut renderer, mesh_b, material_b, 1.0);
    renderer.prepare_frame(&resources);
    assert_eq!(renderer.resource_binding_revision(), binding_revision + 1);
    assert_eq!(renderer.instance_count_revision(), count_revision);
    let retained = renderer.retained_draw(node).expect("retained draw");
    assert_eq!(retained.kind, Draw3DKind::Mesh(mesh_b));
    assert_eq!(retained.surfaces, draw_surface(material_b));
}

#[test]
fn camera_texture_revision_waits_for_ready_texture() {
    let mut renderer = Renderer3D::new();
    let mut resources = ResourceStore::new();
    let texture_a = resources.create_texture("__ready_texture__", false);
    let texture_b = perro_ids::TextureID::from_parts(20_001, 0);
    let node = NodeID::from_parts(802, 0);
    renderer.queue_camera_stream_quad(
        node,
        texture_a,
        glam::Mat4::IDENTITY.to_cols_array_2d(),
        [1.0; 2],
        [1.0; 4],
    );
    renderer.prepare_frame(&resources);
    let binding_revision = renderer.resource_binding_revision();

    renderer.queue_camera_stream_quad(
        node,
        texture_b,
        glam::Mat4::IDENTITY.to_cols_array_2d(),
        [2.0; 2],
        [0.5; 4],
    );
    let (_, stats, _) = renderer.prepare_frame(&resources);
    assert_eq!(stats.rejected_draws, 1);
    assert_eq!(renderer.resource_binding_revision(), binding_revision);
    assert!(matches!(
        renderer.retained_draw(node).map(|draw| draw.kind),
        Some(Draw3DKind::CameraStreamQuad { texture, .. }) if texture == texture_a
    ));

    assert_eq!(
        resources.create_texture_with_id(texture_b, "__became_ready_texture__", false),
        texture_b
    );
    renderer.queue_camera_stream_quad(
        node,
        texture_b,
        glam::Mat4::IDENTITY.to_cols_array_2d(),
        [2.0; 2],
        [0.5; 4],
    );
    renderer.prepare_frame(&resources);
    assert_eq!(renderer.resource_binding_revision(), binding_revision + 1);
    assert!(matches!(
        renderer.retained_draw(node).map(|draw| draw.kind),
        Some(Draw3DKind::CameraStreamQuad { texture, .. }) if texture == texture_b
    ));
}

#[test]
fn bulk_removal_rebuilds_once_at_read_before_frame_prepare() {
    let (mut renderer, _) = seeded_points(100);
    let revision = renderer.draw_revision();
    let binding_revision = renderer.resource_binding_revision();
    let count_revision = renderer.instance_count_revision();
    for id in 1..=75 {
        renderer.remove_node(NodeID::from_parts(id, 0));
    }
    assert_eq!(renderer.non_draw_remove_hashes, 0);
    assert_eq!(renderer.draw_cache_rebuilds, 1);
    assert_eq!(renderer.draw_revision(), revision + 75);
    assert_eq!(renderer.resource_binding_revision(), binding_revision + 75);
    assert_eq!(renderer.instance_count_revision(), count_revision + 75);
    let nodes: Vec<_> = renderer
        .retained_draws_sorted()
        .iter()
        .map(|d| d.node)
        .collect();
    assert_eq!(
        nodes,
        (76..=100)
            .map(|id| NodeID::from_parts(id, 0))
            .collect::<Vec<_>>()
    );
    assert_eq!(renderer.draw_cache_rebuilds, 2);
    renderer.retained_draws_sorted();
    renderer.remove_node(NodeID::from_parts(1, 0));
    assert_eq!(renderer.non_draw_remove_hashes, 0);
    assert_eq!(renderer.draw_cache_rebuilds, 2);
    assert_eq!(renderer.draw_revision(), revision + 75);
}

#[test]
fn removal_still_clears_populated_non_draw_role_maps() {
    use perro_render_bridge::{AmbientLight3DState, RayLight3DState};

    let node = NodeID::from_parts(900, 4);
    let other = NodeID::from_parts(901, 2);
    let ambient = AmbientLight3DState {
        color: [1.0; 3],
        intensity: 1.0,
        cast_shadows: false,
    };
    let ray = RayLight3DState {
        direction: [0.0, -1.0, 0.0],
        color: [1.0; 3],
        intensity: 1.0,
        cast_shadows: false,
        shadow_strength: 1.0,
        shadow_depth_bias: 0.0,
        shadow_normal_bias: 0.0,
    };
    let mut renderer = Renderer3D::new();
    renderer.set_ambient_light(node, ambient);
    renderer.set_ambient_light(other, ambient);
    renderer.set_ray_light(node, ray);
    renderer.set_ray_light(other, ray);

    renderer.remove_node(node);

    assert_eq!(renderer.ambient_lights.get(&node), None);
    assert_eq!(renderer.ambient_lights.get(&other), Some(&ambient));
    assert_eq!(renderer.ray_lights.get(&node), None);
    assert_eq!(renderer.ray_lights.get(&other), Some(&ray));
    assert_eq!(renderer.non_draw_remove_hashes, 2);
    assert!(renderer.ray_lights_dirty);
}

#[test]
fn stale_sequential_cache_cannot_resurrect_removed_draw_with_unready_packet() {
    let (mut renderer, resources) = seeded_points(1);
    let node = NodeID::from_parts(1, 0);
    let mut unready = renderer.retained_draw(node).expect("retained draw");
    unready.kind = Draw3DKind::Mesh(perro_ids::MeshID::from_parts(12345, 0));
    renderer.remove_node(node);
    renderer.queued_draws.push(unready);
    let (_, stats, _) = renderer.prepare_frame(&resources);
    assert_eq!(stats.rejected_draws, 1);
    assert_eq!(renderer.retained_draw_count(), 0);
    assert!(renderer.retained_draws_sorted().is_empty());
}

#[test]
fn mixed_membership_and_duplicate_packets_keep_last_update_and_node_generation() {
    let (mut renderer, resources) = seeded_points(3);
    let old = NodeID::from_parts(2, 0);
    let new = NodeID::from_parts(2, 1);
    renderer.remove_node(old);
    queue_point(&mut renderer, new, 20.0);
    queue_point(&mut renderer, NodeID::from_parts(1, 0), 10.0);
    queue_point(&mut renderer, new, 21.0);
    renderer.prepare_frame(&resources);
    assert_eq!(renderer.draw_cache_rebuilds, 1);
    assert!(renderer.retained_draw(old).is_none());
    assert_eq!(
        renderer
            .retained_draw(new)
            .expect("retained draw")
            .instance_mats[0][3][0],
        21.0
    );
    let mut expected = vec![NodeID::from_parts(1, 0), new, NodeID::from_parts(3, 0)];
    expected.sort_unstable_by_key(|node| node.as_u64());
    assert_eq!(
        renderer
            .retained_draws_sorted()
            .iter()
            .map(|d| d.node)
            .collect::<Vec<_>>(),
        expected
    );
    assert_eq!(renderer.draw_cache_rebuilds, 2);
    queue_point(&mut renderer, new, 22.0);
    renderer.prepare_frame(&resources);
    let retained = renderer.retained_draw(new).expect("retained draw");
    assert_eq!(
        renderer
            .retained_draws_sorted()
            .iter()
            .find(|d| d.node == new)
            .expect("retained draw"),
        &retained
    );
    assert_eq!(renderer.draw_cache_rebuilds, 2);
}

#[test]
fn unready_sparse_and_sequential_updates_patch_transform_keep_bindings() {
    for count in [1, 3] {
        let (mut renderer, resources) = seeded_points(count);
        let node = NodeID::from_parts(1, 0);
        let mut unready = renderer.retained_draw(node).expect("retained draw");
        unready.kind = Draw3DKind::Mesh(perro_ids::MeshID::from_parts(12345, 0));
        unready.surfaces = draw_surface(MaterialID::from_parts(12345, 0));
        unready.instance_mats =
            Arc::from([glam::Mat4::from_translation(glam::Vec3::X * 42.0).to_cols_array_2d()]);
        renderer.queued_draws.push(unready.clone());
        let revision = renderer.draw_revision();
        let (_, stats, _) = renderer.prepare_frame(&resources);
        assert_eq!(stats.rejected_draws, 1);
        assert_eq!(renderer.draw_revision(), revision + 1);
        let updated = renderer
            .retained_draws_sorted()
            .iter()
            .find(|d| d.node == node)
            .expect("retained draw");
        assert_eq!(updated.kind, Draw3DKind::DebugPointCube);
        assert!(updated.surfaces.is_empty());
        assert_eq!(updated.instance_mats, unready.instance_mats);
        assert_eq!(renderer.draw_cache_rebuilds, 1);
        renderer.queued_draws.push(unready);
        renderer.prepare_frame(&resources);
        assert_eq!(renderer.draw_revision(), revision + 1);
    }
}

#[test]
fn sequential_updates_keep_cache_and_retained_store_in_sync() {
    let (mut renderer, resources) = seeded_points(3);
    for id in 1..=3 {
        queue_point(&mut renderer, NodeID::from_parts(id, 0), 100.0 + id as f32);
    }
    renderer.prepare_frame(&resources);
    let retained: Vec<_> = (1..=3)
        .map(|id| {
            renderer
                .retained_draw(NodeID::from_parts(id, 0))
                .expect("retained draw")
        })
        .collect();
    assert_eq!(renderer.retained_draws_sorted(), retained.as_slice());
    assert_eq!(renderer.draw_cache_rebuilds, 1);
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
        renderer
            .retained_draw(node)
            .expect("test setup/result must succeed")
            .kind,
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

    let retained = renderer
        .retained_draw(node)
        .expect("test setup/result must succeed");
    assert_eq!(
        retained.kind,
        Draw3DKind::CameraStreamQuad {
            texture,
            tint: [1.0, 0.8, 0.6, 0.5]
        }
    );
    assert_eq!(retained.instance_mats.len(), 1);
}

#[test]
fn retained_custom_material_requests_continuous_frames() {
    let mut renderer = Renderer3D::new();
    let mut resources = ResourceStore::new();
    let mesh = resources.create_mesh("__mesh__", false);
    let material = resources.create_material(
        Material3D::Custom(CustomMaterial3D::new("res://shaders/animated.wgsl")),
        Some("__custom_mat__"),
        false,
    );

    renderer.queue_draw(
        NodeID::from_parts(8, 0),
        mesh,
        draw_surface(material),
        glam::Mat4::IDENTITY.to_cols_array_2d(),
        None,
        Arc::from([]),
        None,
        LODOptions3D::default(),
        MeshBlendOptions3D::default(),
        true,
        true,
    );
    let _ = renderer.prepare_frame(&resources);

    assert!(renderer.has_retained_custom_material(&resources));

    // animated-gate: probe result decides; static shader => no continuous
    // redraw, animated shader => continuous.
    assert!(!renderer.any_retained_custom_material_where(&resources, |_| false));
    let mut seen_path = None;
    assert!(
        renderer.any_retained_custom_material_where(&resources, |path| {
            seen_path = Some(path.to_string());
            true
        })
    );
    assert_eq!(seen_path.as_deref(), Some("res://shaders/animated.wgsl"));
}
