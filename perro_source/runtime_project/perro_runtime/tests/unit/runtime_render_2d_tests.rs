use super::Runtime;
use perro_ids::TextureID;
use perro_nodes::{SceneNode, SceneNodeData, camera_2d::Camera2D, sprite_2d::Sprite2D};
use perro_render_bridge::{Command2D, RenderCommand, RenderEvent, ResourceCommand};

fn collect_commands(runtime: &mut Runtime) -> Vec<RenderCommand> {
    let mut out = Vec::new();
    runtime.drain_render_commands(&mut out);
    out
}

#[test]
fn sprite_requests_texture_once_until_created() {
    let mut runtime = Runtime::new();
    let expected_node = runtime
        .nodes
        .insert(SceneNode::new(SceneNodeData::Sprite2D(Sprite2D::new())));

    runtime.extract_render_2d_commands();
    let first = collect_commands(&mut runtime);
    assert_eq!(first.len(), 1);
    let request = match &first[0] {
        RenderCommand::Resource(ResourceCommand::CreateTexture {
            request,
            id,
            source,
            reserved,
        }) => {
            assert_eq!(source, "__default__");
            assert!(!reserved);
            assert!(id.is_nil());
            *request
        }
        _ => panic!("expected CreateTexture"),
    };

    runtime.extract_render_2d_commands();
    assert!(collect_commands(&mut runtime).is_empty());

    let texture = TextureID::from_parts(3, 1);
    runtime.apply_render_event(RenderEvent::TextureCreated {
        request,
        id: texture,
    });
    runtime.extract_render_2d_commands();
    let third = collect_commands(&mut runtime);
    assert_eq!(third.len(), 1);
    assert!(matches!(
        third[0],
        RenderCommand::TwoD(Command2D::UpsertSprite { node, sprite })
        if node == expected_node && sprite.texture == texture
    ));
}

#[test]
fn sprite_becoming_invisible_emits_remove_node() {
    let mut runtime = Runtime::new();
    let mut sprite = Sprite2D::new();
    sprite.texture = TextureID::from_parts(7, 0);
    let expected_node = runtime
        .nodes
        .insert(SceneNode::new(SceneNodeData::Sprite2D(sprite)));

    runtime.extract_render_2d_commands();
    let first = collect_commands(&mut runtime);
    assert_eq!(first.len(), 1);
    assert!(matches!(
        first[0],
        RenderCommand::TwoD(Command2D::UpsertSprite { node, .. }) if node == expected_node
    ));

    let node = runtime
        .nodes
        .get_mut(expected_node)
        .expect("sprite node must exist");
    if let SceneNodeData::Sprite2D(sprite) = &mut node.data {
        sprite.visible = false;
    }

    runtime.extract_render_2d_commands();
    let second = collect_commands(&mut runtime);
    assert_eq!(second.len(), 1);
    assert!(matches!(
        second[0],
        RenderCommand::TwoD(Command2D::RemoveNode { node }) if node == expected_node
    ));
}

#[test]
fn active_camera_2d_emits_set_camera_command() {
    let mut runtime = Runtime::new();
    let mut camera = Camera2D::new();
    camera.active = true;
    camera.zoom = 2.0;
    camera.transform.position.x = 128.0;
    camera.transform.position.y = -32.0;
    camera.transform.rotation = 0.5;
    runtime
        .nodes
        .insert(SceneNode::new(SceneNodeData::Camera2D(camera)));

    runtime.extract_render_2d_commands();
    let commands = collect_commands(&mut runtime);
    assert!(commands.iter().any(|command| matches!(
        command,
        RenderCommand::TwoD(Command2D::SetCamera { camera })
        if camera.position == [128.0, -32.0]
            && camera.rotation_radians == 0.5
            && camera.zoom == 2.0
    )));
}
