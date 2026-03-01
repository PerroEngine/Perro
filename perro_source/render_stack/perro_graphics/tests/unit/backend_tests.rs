use super::PerroGraphics;
use crate::backend::GraphicsBackend;
use perro_ids::{MaterialID, MeshID, NodeID, TextureID};
use perro_render_bridge::{
    Camera3DState, CameraProjectionState, Command2D, Command3D, Material3D, RenderBridge,
    RenderCommand, ResourceCommand, Sprite2DCommand,
};
use crate::three_d::renderer::Draw3DKind;

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
            z_index: 2,
        },
    }));
    graphics.draw_frame();

    assert_eq!(
        graphics.renderer_2d.retained_sprite(node),
        Some(Sprite2DCommand {
            texture: created,
            model: [[1.0, 0.0, 10.0], [0.0, 1.0, 5.0], [0.0, 0.0, 1.0]],
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

    graphics.submit(RenderCommand::ThreeD(Command3D::Draw {
        mesh: created_meshes[0],
        material: created_materials[0],
        node: node_a,
        model: model_a,
    }));
    graphics.submit(RenderCommand::ThreeD(Command3D::Draw {
        mesh: created_meshes[1],
        material: created_materials[1],
        node: node_b,
        model: model_b,
    }));
    graphics.draw_frame();

    assert_eq!(graphics.renderer_3d.retained_draw_count(), 2);
    assert_eq!(
        graphics.renderer_3d.retained_draw(node_a),
        Some(crate::three_d::renderer::Draw3DInstance {
            node: node_a,
            kind: Draw3DKind::Mesh(created_meshes[0]),
            material: Some(created_materials[0]),
            model: model_a,
        })
    );
    assert_eq!(
        graphics.renderer_3d.retained_draw(node_b),
        Some(crate::three_d::renderer::Draw3DInstance {
            node: node_b,
            kind: Draw3DKind::Mesh(created_meshes[1]),
            material: Some(created_materials[1]),
            model: model_b,
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
    graphics.submit(RenderCommand::ThreeD(Command3D::Draw {
        mesh: mesh_id,
        material: material_id,
        node,
        model: first_model,
    }));
    graphics.draw_frame();
    assert_eq!(
        graphics.renderer_3d.retained_draw(node),
        Some(crate::three_d::renderer::Draw3DInstance {
            node,
            kind: Draw3DKind::Mesh(mesh_id),
            material: Some(material_id),
            model: first_model,
        })
    );

    let missing_mesh = MeshID::from_parts(999_999, 0);
    let second_model = [
        [1.0, 0.0, 0.0, 2.0],
        [0.0, 1.0, 0.0, 0.0],
        [0.0, 0.0, 1.0, 0.0],
        [0.0, 0.0, 0.0, 1.0],
    ];
    graphics.submit(RenderCommand::ThreeD(Command3D::Draw {
        mesh: missing_mesh,
        material: material_id,
        node,
        model: second_model,
    }));
    graphics.draw_frame();

    assert_eq!(
        graphics.renderer_3d.retained_draw(node),
        Some(crate::three_d::renderer::Draw3DInstance {
            node,
            kind: Draw3DKind::Mesh(mesh_id),
            material: Some(material_id),
            model: second_model,
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
    graphics.submit(RenderCommand::ThreeD(Command3D::Draw {
        mesh: mesh_id,
        material: material_id,
        node,
        model: first_model,
    }));
    graphics.draw_frame();

    let missing_material = MaterialID::from_parts(999_998, 0);
    let second_model = [
        [1.0, 0.0, 0.0, 1.5],
        [0.0, 1.0, 0.0, 0.0],
        [0.0, 0.0, 1.0, 0.0],
        [0.0, 0.0, 0.0, 1.0],
    ];
    graphics.submit(RenderCommand::ThreeD(Command3D::Draw {
        mesh: mesh_id,
        material: missing_material,
        node,
        model: second_model,
    }));
    graphics.draw_frame();

    assert_eq!(
        graphics.renderer_3d.retained_draw(node),
        Some(crate::three_d::renderer::Draw3DInstance {
            node,
            kind: Draw3DKind::Mesh(mesh_id),
            material: Some(material_id),
            model: second_model,
        })
    );
}

#[test]
fn set_camera_3d_updates_retained_camera_state() {
    let mut graphics = PerroGraphics::new();
    graphics.submit(RenderCommand::ThreeD(Command3D::SetCamera {
        camera: Camera3DState {
            position: [1.0, 2.0, 3.0],
            rotation: [0.0, 0.5, 0.0, 0.8660254],
            projection: CameraProjectionState::Perspective {
                fov_y_degrees: 48.0,
                near: 0.2,
                far: 900.0,
            },
        },
    }));
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
            z_index: 7,
        },
    }));
    graphics.draw_frame();

    assert_eq!(
        graphics.renderer_2d.retained_sprite(node),
        Some(Sprite2DCommand {
            texture,
            model: second_model,
            z_index: 7,
        })
    );
}
