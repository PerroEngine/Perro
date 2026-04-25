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

#[test]
fn parse_script_vars_object() {
    let src = r#"
    [main]
    script = "res://main.rs"
    script_vars = { "cam": Camera, speed: 2.5, enabled: true }
    [Node]
    [/Node]
    [/main]

    [Camera]
    [Node]
    [/Node]
    [/Camera]
    "#;

    let scene = Parser::new(src).parse_scene();
    let main = scene
        .nodes
        .iter()
        .find(|n| n.key.as_ref() == "main")
        .expect("main node");

    assert_eq!(main.script_vars.len(), 3);
    assert!(
        main.script_vars
            .iter()
            .any(|(name, _)| name.as_ref() == "cam")
    );
    assert!(
        main.script_vars
            .iter()
            .any(|(name, _)| name.as_ref() == "speed")
    );
    assert!(
        main.script_vars
            .iter()
            .any(|(name, _)| name.as_ref() == "enabled")
    );
}

#[test]
fn parse_root_of_header() {
    let src = r#"
    [main]
    root_of = "res://child.scn"
    [Node]
    [/Node]
    [/main]
    "#;

    let scene = Parser::new(src).parse_scene();
    let main = scene
        .nodes
        .iter()
        .find(|n| n.key.as_ref() == "main")
        .expect("main node");
    assert_eq!(main.root_of.as_deref(), Some("res://child.scn"));
}

#[test]
fn parse_script_clear_options() {
    let src = r#"
    [main]
    script = null
    [Node]
    [/Node]
    [/main]

    [child]
    clear_script = true
    [Node]
    [/Node]
    [/child]
    "#;

    let scene = Parser::new(src).parse_scene();
    let main = scene
        .nodes
        .iter()
        .find(|n| n.key.as_ref() == "main")
        .expect("main node");
    assert!(main.script.is_none());
    assert!(main.clear_script);

    let child = scene
        .nodes
        .iter()
        .find(|n| n.key.as_ref() == "child")
        .expect("child node");
    assert!(child.script.is_none());
    assert!(child.clear_script);
}

#[test]
fn parse_root_of_without_type_block() {
    let src = r#"
    @root = main
    [main]
    root_of = "res://base.scn"
    [/main]
    "#;

    let scene = Parser::new(src).parse_scene();
    let main = scene
        .nodes
        .iter()
        .find(|n| n.key.as_ref() == "main")
        .expect("main node");
    assert_eq!(main.root_of.as_deref(), Some("res://base.scn"));
    assert!(!main.has_data_override);
}

#[test]
fn parse_header_only_node_without_type_block_defaults_to_node() {
    let src = r#"
    @root = root
    [relationship_manager]
    parent = @root
    script = "res://scripts/relationship_manager.rs"
    script_vars = {
        max_character_id = 36
    }
    [/relationship_manager]
    "#;

    let scene = Parser::new(src).parse_scene();
    let node = scene
        .nodes
        .iter()
        .find(|n| n.key.as_ref() == "relationship_manager")
        .expect("relationship_manager node");

    assert_eq!(node.data.ty.as_ref(), "Node");
    assert!(!node.has_data_override);
    assert_eq!(node.parent.as_ref().map(|k| k.as_ref()), Some("root"));
    assert_eq!(
        node.script.as_ref().map(|s| s.as_ref()),
        Some("res://scripts/relationship_manager.rs")
    );
    assert!(
        node.script_vars
            .iter()
            .any(|(name, _)| name.as_ref() == "max_character_id")
    );
}
