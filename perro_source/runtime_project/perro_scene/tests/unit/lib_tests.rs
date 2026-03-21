use super::*;

#[test]
fn parse_basic_scene() {
    let src = r#"
    @root = main

    [main]
    name = "Root Node"
    [Node2D]
        position = (0, 0)
    [/Node2D]
    [/main]

    [player]
    parent = @root
    [Sprite2D]
        texture = "res://player.png"
    [/Sprite2D]
    [/player]
    "#;

    let scene = Parser::new(src).parse_scene();
    assert_eq!(scene.root.as_ref().map(|k| k.as_ref()), Some("main"));
    assert_eq!(scene.nodes.len(), 2);

    let main = scene
        .nodes
        .iter()
        .find(|n| n.key.as_ref() == "main")
        .expect("main node");
    assert_eq!(main.name.as_ref().map(|s| s.as_ref()), Some("Root Node"));

    let player = scene
        .nodes
        .iter()
        .find(|n| n.key.as_ref() == "player")
        .expect("player node");
    assert_eq!(player.parent.as_ref().map(|k| k.as_ref()), Some("main"));
    assert_eq!(player.data.ty.as_ref(), "Sprite2D");
}

#[test]
fn parse_object_literal() {
    let src = r#"
    @root = main
    @mat = { roughness: 1.0, metallic: 0.2 }

    [main]
    [MeshInstance3D]
        material = @mat
    [/MeshInstance3D]
    [/main]
    "#;

    let scene = Parser::new(src).parse_scene();
    let main = scene
        .nodes
        .iter()
        .find(|n| n.key.as_ref() == "main")
        .expect("main node");

    let material = main
        .data
        .fields
        .iter()
        .find(|(name, _)| name.as_ref() == "material")
        .expect("material field");

    match &material.1 {
        SceneValue::Object(entries) => {
            assert!(entries.iter().any(|(k, _)| k.as_ref() == "roughness"));
            assert!(entries.iter().any(|(k, _)| k.as_ref() == "metallic"));
        }
        _ => panic!("expected material object"),
    }
}

#[test]
fn scene_key_and_value_key_as_ref() {
    let key = SceneKey::from("player");
    let value_key = SceneValueKey::from("root");
    assert_eq!(key.as_ref(), "player");
    assert_eq!(value_key.as_ref(), "root");
}
