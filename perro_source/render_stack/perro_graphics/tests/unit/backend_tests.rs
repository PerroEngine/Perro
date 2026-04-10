use super::PerroGraphics;
use crate::backend::GraphicsBackend;
use crate::three_d::renderer::Draw3DKind;
use perro_ids::{MaterialID, MeshID, NodeID, TextureID};
use perro_render_bridge::{
    Camera3DState, CameraProjectionState, Command2D, Command3D, Material3D, MeshSurfaceBinding3D,
    PostProcessingCommand, RenderBridge, RenderCommand, ResourceCommand, Sprite2DCommand,
    VisualAccessibilityCommand,
};
use perro_structs::{ColorBlindFilter, PostProcessEffect, PostProcessSet};
use std::sync::Arc;

fn surfaces_for(material: MaterialID) -> Arc<[MeshSurfaceBinding3D]> {
    Arc::from([MeshSurfaceBinding3D {
        material: Some(material),
        overrides: Arc::from([]),
        modulate: [1.0, 1.0, 1.0, 1.0],
    }])
}

#[test]
fn sprite_texture_upsert_is_accepted_after_texture_creation() {
    let mut graphics = PerroGraphics::new();
    let request = perro_render_bridge::RenderRequestID::new(99);
    let node = NodeID::from_parts(1, 0);

    graphics.submit(RenderCommand::Resource(ResourceCommand::CreateTexture {
        request,
        id: TextureID::nil(),
        source: "__default__".to_string(),
        reserved: false,
    }));
    graphics.draw_frame();

    let mut events = Vec::new();
    graphics.drain_events(&mut events);
    let created = events
        .into_iter()
        .find_map(|event| match event {
            perro_render_bridge::RenderEvent::TextureCreated { id, .. } => Some(id),
            _ => None,
        })
        .expect("texture creation event should exist");

    graphics.submit(RenderCommand::TwoD(Command2D::UpsertSprite {
        node,
        sprite: Sprite2DCommand {
            texture: created,
            model: [[1.0, 0.0, 10.0], [0.0, 1.0, 5.0], [0.0, 0.0, 1.0]],
            tint: [1.0, 1.0, 1.0, 1.0],
            z_index: 2,
        },
    }));
    graphics.draw_frame();

    assert_eq!(
        graphics.renderer_2d.retained_sprite(node),
        Some(Sprite2DCommand {
            texture: created,
            model: [[1.0, 0.0, 10.0], [0.0, 1.0, 5.0], [0.0, 0.0, 1.0]],
            tint: [1.0, 1.0, 1.0, 1.0],
            z_index: 2,
        })
    );
}

#[test]
fn draw_3d_updates_retained_state_per_node() {
    let mut graphics = PerroGraphics::new();
    let node_a = NodeID::from_parts(10, 0);
    let node_b = NodeID::from_parts(11, 0);

    graphics.submit(RenderCommand::Resource(ResourceCommand::CreateMesh {
        request: perro_render_bridge::RenderRequestID::new(1001),
        id: MeshID::nil(),
        source: "__cube__".to_string(),
        reserved: false,
    }));
    graphics.submit(RenderCommand::Resource(ResourceCommand::CreateMaterial {
        request: perro_render_bridge::RenderRequestID::new(1002),
        id: MaterialID::nil(),
        material: Material3D::default(),
        source: None,
        reserved: false,
    }));
    graphics.submit(RenderCommand::Resource(ResourceCommand::CreateMesh {
        request: perro_render_bridge::RenderRequestID::new(1003),
        id: MeshID::nil(),
        source: "__sphere__".to_string(),
        reserved: false,
    }));
    graphics.submit(RenderCommand::Resource(ResourceCommand::CreateMaterial {
        request: perro_render_bridge::RenderRequestID::new(1004),
        id: MaterialID::nil(),
        material: Material3D::default(),
        source: None,
        reserved: false,
    }));
    graphics.draw_frame();

    let mut events = Vec::new();
    graphics.drain_events(&mut events);
    let mut created_meshes = Vec::new();
    let mut created_materials = Vec::new();
    for event in events {
        match event {
            perro_render_bridge::RenderEvent::MeshCreated { id, .. } => created_meshes.push(id),
            perro_render_bridge::RenderEvent::MaterialCreated { id, .. } => {
                created_materials.push(id)
            }
            _ => {}
        }
    }
    assert_eq!(created_meshes.len(), 2);
    assert_eq!(created_materials.len(), 2);

    let model_a = [
        [1.0, 0.0, 0.0, 2.0],
        [0.0, 1.0, 0.0, 0.0],
        [0.0, 0.0, 1.0, 0.0],
        [0.0, 0.0, 0.0, 1.0],
    ];
    let model_b = [
        [1.0, 0.0, 0.0, 0.0],
        [0.0, 1.0, 0.0, 3.0],
        [0.0, 0.0, 1.0, 0.0],
        [0.0, 0.0, 0.0, 1.0],
    ];

    graphics.submit(RenderCommand::ThreeD(Box::new(Command3D::Draw {
        mesh: created_meshes[0],
        surfaces: surfaces_for(created_materials[0]),
        node: node_a,
        model: model_a,
        skeleton: None,
    })));
    graphics.submit(RenderCommand::ThreeD(Box::new(Command3D::Draw {
        mesh: created_meshes[1],
        surfaces: surfaces_for(created_materials[1]),
        node: node_b,
        model: model_b,
        skeleton: None,
    })));
    graphics.draw_frame();

    assert_eq!(graphics.renderer_3d.retained_draw_count(), 2);
    assert_eq!(
        graphics.renderer_3d.retained_draw(node_a),
        Some(crate::three_d::renderer::Draw3DInstance {
            node: node_a,
            kind: Draw3DKind::Mesh(created_meshes[0]),
            surfaces: surfaces_for(created_materials[0]),
            model: model_a,
            skeleton: None,
        })
    );
    assert_eq!(
        graphics.renderer_3d.retained_draw(node_b),
        Some(crate::three_d::renderer::Draw3DInstance {
            node: node_b,
            kind: Draw3DKind::Mesh(created_meshes[1]),
            surfaces: surfaces_for(created_materials[1]),
            model: model_b,
            skeleton: None,
        })
    );
}

#[test]
fn rejected_3d_draw_keeps_previous_retained_binding() {
    let mut graphics = PerroGraphics::new();
    let node = NodeID::from_parts(20, 0);

    graphics.submit(RenderCommand::Resource(ResourceCommand::CreateMesh {
        request: perro_render_bridge::RenderRequestID::new(2001),
        id: MeshID::nil(),
        source: "__cube__".to_string(),
        reserved: false,
    }));
    graphics.submit(RenderCommand::Resource(ResourceCommand::CreateMaterial {
        request: perro_render_bridge::RenderRequestID::new(2002),
        id: MaterialID::nil(),
        material: Material3D::default(),
        source: None,
        reserved: false,
    }));
    graphics.draw_frame();

    let mut events = Vec::new();
    graphics.drain_events(&mut events);
    let mut mesh_id = MeshID::nil();
    let mut material_id = MaterialID::nil();
    for event in events {
        match event {
            perro_render_bridge::RenderEvent::MeshCreated { id, .. } => mesh_id = id,
            perro_render_bridge::RenderEvent::MaterialCreated { id, .. } => material_id = id,
            _ => {}
        }
    }
    assert!(!mesh_id.is_nil());
    assert!(!material_id.is_nil());

    let first_model = [
        [1.0, 0.0, 0.0, 1.0],
        [0.0, 1.0, 0.0, 0.0],
        [0.0, 0.0, 1.0, 0.0],
        [0.0, 0.0, 0.0, 1.0],
    ];
    graphics.submit(RenderCommand::ThreeD(Box::new(Command3D::Draw {
        mesh: mesh_id,
        surfaces: surfaces_for(material_id),
        node,
        model: first_model,
        skeleton: None,
    })));
    graphics.draw_frame();
    assert_eq!(
        graphics.renderer_3d.retained_draw(node),
        Some(crate::three_d::renderer::Draw3DInstance {
            node,
            kind: Draw3DKind::Mesh(mesh_id),
            surfaces: surfaces_for(material_id),
            model: first_model,
            skeleton: None,
        })
    );

    let missing_mesh = MeshID::from_parts(999_999, 0);
    let second_model = [
        [1.0, 0.0, 0.0, 2.0],
        [0.0, 1.0, 0.0, 0.0],
        [0.0, 0.0, 1.0, 0.0],
        [0.0, 0.0, 0.0, 1.0],
    ];
    graphics.submit(RenderCommand::ThreeD(Box::new(Command3D::Draw {
        mesh: missing_mesh,
        surfaces: surfaces_for(material_id),
        node,
        model: second_model,
        skeleton: None,
    })));
    graphics.draw_frame();

    assert_eq!(
        graphics.renderer_3d.retained_draw(node),
        Some(crate::three_d::renderer::Draw3DInstance {
            node,
            kind: Draw3DKind::Mesh(mesh_id),
            surfaces: surfaces_for(material_id),
            model: second_model,
            skeleton: None,
        })
    );
}

#[test]
fn rejected_3d_material_swap_keeps_previous_material_binding() {
    let mut graphics = PerroGraphics::new();
    let node = NodeID::from_parts(21, 0);

    graphics.submit(RenderCommand::Resource(ResourceCommand::CreateMesh {
        request: perro_render_bridge::RenderRequestID::new(2101),
        id: MeshID::nil(),
        source: "__cube__".to_string(),
        reserved: false,
    }));
    graphics.submit(RenderCommand::Resource(ResourceCommand::CreateMaterial {
        request: perro_render_bridge::RenderRequestID::new(2102),
        id: MaterialID::nil(),
        material: Material3D::default(),
        source: None,
        reserved: false,
    }));
    graphics.draw_frame();

    let mut events = Vec::new();
    graphics.drain_events(&mut events);
    let mut mesh_id = MeshID::nil();
    let mut material_id = MaterialID::nil();
    for event in events {
        match event {
            perro_render_bridge::RenderEvent::MeshCreated { id, .. } => mesh_id = id,
            perro_render_bridge::RenderEvent::MaterialCreated { id, .. } => material_id = id,
            _ => {}
        }
    }
    assert!(!mesh_id.is_nil());
    assert!(!material_id.is_nil());

    let first_model = [
        [1.0, 0.0, 0.0, 0.5],
        [0.0, 1.0, 0.0, 0.0],
        [0.0, 0.0, 1.0, 0.0],
        [0.0, 0.0, 0.0, 1.0],
    ];
    graphics.submit(RenderCommand::ThreeD(Box::new(Command3D::Draw {
        mesh: mesh_id,
        surfaces: surfaces_for(material_id),
        node,
        model: first_model,
        skeleton: None,
    })));
    graphics.draw_frame();

    let missing_material = MaterialID::from_parts(999_998, 0);
    let second_model = [
        [1.0, 0.0, 0.0, 1.5],
        [0.0, 1.0, 0.0, 0.0],
        [0.0, 0.0, 1.0, 0.0],
        [0.0, 0.0, 0.0, 1.0],
    ];
    graphics.submit(RenderCommand::ThreeD(Box::new(Command3D::Draw {
        mesh: mesh_id,
        surfaces: surfaces_for(missing_material),
        node,
        model: second_model,
        skeleton: None,
    })));
    graphics.draw_frame();

    assert_eq!(
        graphics.renderer_3d.retained_draw(node),
        Some(crate::three_d::renderer::Draw3DInstance {
            node,
            kind: Draw3DKind::Mesh(mesh_id),
            surfaces: surfaces_for(material_id),
            model: second_model,
            skeleton: None,
        })
    );
}

#[test]
fn set_camera_3d_updates_retained_camera_state() {
    let mut graphics = PerroGraphics::new();
    graphics.submit(RenderCommand::ThreeD(Box::new(Command3D::SetCamera {
        camera: Camera3DState {
            position: [1.0, 2.0, 3.0],
            rotation: [0.0, 0.5, 0.0, 0.8660254],
            projection: CameraProjectionState::Perspective {
                fov_y_degrees: 48.0,
                near: 0.2,
                far: 900.0,
            },
            post_processing: Arc::from([]),
        },
    })));
    graphics.draw_frame();

    assert_eq!(
        graphics.renderer_3d.camera(),
        Camera3DState {
            position: [1.0, 2.0, 3.0],
            rotation: [0.0, 0.5, 0.0, 0.8660254],
            projection: CameraProjectionState::Perspective {
                fov_y_degrees: 48.0,
                near: 0.2,
                far: 900.0,
            },
            post_processing: Arc::from([]),
        }
    );
}

#[test]
fn rejected_sprite_texture_does_not_update_retained_binding() {
    let mut graphics = PerroGraphics::new();
    let node = NodeID::from_parts(2, 0);
    let missing = TextureID::from_parts(999, 0);

    graphics.submit(RenderCommand::TwoD(Command2D::UpsertSprite {
        node,
        sprite: Sprite2DCommand {
            texture: missing,
            model: [[1.0, 0.0, 0.0], [0.0, 1.0, 0.0], [0.0, 0.0, 1.0]],
            tint: [1.0, 1.0, 1.0, 1.0],
            z_index: 0,
        },
    }));
    graphics.draw_frame();

    assert_eq!(graphics.renderer_2d.retained_sprite(node), None);
}

#[test]
fn rejected_sprite_texture_swap_keeps_previous_texture_binding() {
    let mut graphics = PerroGraphics::new();
    let request = perro_render_bridge::RenderRequestID::new(3001);
    let node = NodeID::from_parts(3, 0);

    graphics.submit(RenderCommand::Resource(ResourceCommand::CreateTexture {
        request,
        id: TextureID::nil(),
        source: "__default__".to_string(),
        reserved: false,
    }));
    graphics.draw_frame();

    let mut events = Vec::new();
    graphics.drain_events(&mut events);
    let texture = events
        .into_iter()
        .find_map(|event| match event {
            perro_render_bridge::RenderEvent::TextureCreated { id, .. } => Some(id),
            _ => None,
        })
        .expect("texture creation event should exist");

    let first_model = [[1.0, 0.0, 2.0], [0.0, 1.0, 3.0], [0.0, 0.0, 1.0]];
    graphics.submit(RenderCommand::TwoD(Command2D::UpsertSprite {
        node,
        sprite: Sprite2DCommand {
            texture,
            model: first_model,
            tint: [1.0, 1.0, 1.0, 1.0],
            z_index: 1,
        },
    }));
    graphics.draw_frame();

    let missing_texture = TextureID::from_parts(999_997, 0);
    let second_model = [[1.0, 0.0, 9.0], [0.0, 1.0, 4.0], [0.0, 0.0, 1.0]];
    graphics.submit(RenderCommand::TwoD(Command2D::UpsertSprite {
        node,
        sprite: Sprite2DCommand {
            texture: missing_texture,
            model: second_model,
            tint: [1.0, 1.0, 1.0, 1.0],
            z_index: 7,
        },
    }));
    graphics.draw_frame();

    assert_eq!(
        graphics.renderer_2d.retained_sprite(node),
        Some(Sprite2DCommand {
            texture,
            model: second_model,
            tint: [1.0, 1.0, 1.0, 1.0],
            z_index: 7,
        })
    );
}

#[test]
fn accessibility_command_updates_global_accessibility_state() {
    let mut graphics = PerroGraphics::new();
    graphics.submit(RenderCommand::VisualAccessibility(
        VisualAccessibilityCommand::EnableColorBlind {
            mode: ColorBlindFilter::Deuteran,
            strength: 0.75,
        },
    ));
    graphics.draw_frame();

    let filter = graphics
        .accessibility
        .color_blind
        .expect("color blind filter should be enabled");
    assert_eq!(filter.filter, ColorBlindFilter::Deuteran);
    assert_eq!(filter.strength, 0.75);

    graphics.submit(RenderCommand::VisualAccessibility(
        VisualAccessibilityCommand::DisableColorBlind,
    ));
    graphics.draw_frame();
    assert_eq!(graphics.accessibility.color_blind, None);
}

#[test]
fn post_processing_commands_update_global_post_processing_state() {
    let mut graphics = PerroGraphics::new();
    graphics.submit(RenderCommand::PostProcessing(
        PostProcessingCommand::AddGlobalNamed {
            name: "crt".into(),
            effect: PostProcessEffect::Crt {
                scanline_strength: 0.25,
                curvature: 0.1,
                chromatic: 0.5,
                vignette: 0.2,
            },
        },
    ));
    graphics.submit(RenderCommand::PostProcessing(
        PostProcessingCommand::AddGlobalUnnamed(PostProcessEffect::Bloom {
            strength: 0.7,
            threshold: 0.8,
            radius: 1.2,
        }),
    ));
    graphics.draw_frame();

    assert_eq!(graphics.global_post_processing.len(), 2);
    assert!(matches!(
        graphics.global_post_processing.get("crt"),
        Some(PostProcessEffect::Crt { .. })
    ));

    graphics.submit(RenderCommand::PostProcessing(
        PostProcessingCommand::RemoveGlobalByName("crt".into()),
    ));
    graphics.submit(RenderCommand::PostProcessing(
        PostProcessingCommand::RemoveGlobalByIndex(0),
    ));
    graphics.draw_frame();
    assert!(graphics.global_post_processing.is_empty());

    let set = PostProcessSet::from_effects(vec![PostProcessEffect::Blur { strength: 2.0 }]);
    graphics.submit(RenderCommand::PostProcessing(
        PostProcessingCommand::SetGlobal(set),
    ));
    graphics.draw_frame();
    assert_eq!(graphics.global_post_processing.len(), 1);

    graphics.submit(RenderCommand::PostProcessing(
        PostProcessingCommand::ClearGlobal,
    ));
    graphics.draw_frame();
    assert!(graphics.global_post_processing.is_empty());
}
