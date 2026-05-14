use super::*;

fn find_node<'a>(scene: &'a Scene, key: &str) -> &'a SceneNodeEntry {
    scene
        .nodes
        .iter()
        .find(|node| scene.key_name(node.key) == Some(key))
        .expect("node")
}

#[test]
fn parse_basic_scene() {
    let src = r#"
    $root = @main

    [main]
    name = "Root Node"
    [Node2D]
        position = (0, 0)
    [/Node2D]
    [/main]

    [player]
    parent = @main
    [Sprite2D]
        texture = "res://player.png"
    [/Sprite2D]
    [/player]
    "#;

    let scene = Parser::new(src).parse_scene();
    assert_eq!(scene.root.and_then(|k| scene.key_name(k)), Some("main"));
    assert_eq!(scene.nodes.len(), 2);

    let main = find_node(&scene, "main");
    assert_eq!(main.name.as_ref().map(|s| s.as_ref()), Some("Root Node"));

    let player = find_node(&scene, "player");
    assert_eq!(player.parent.and_then(|k| scene.key_name(k)), Some("main"));
    assert_eq!(player.data.ty.as_ref(), "Sprite2D");
}

#[test]
fn parse_object_literal() {
    let src = r#"
    $root = @main
    $mat = { roughness: 1.0, metallic: 0.2 }

    [main]
    [MeshInstance3D]
        material = $mat
    [/MeshInstance3D]
    [/main]
    "#;

    let scene = Parser::new(src).parse_scene();
    let main = find_node(&scene, "main");

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
    let key = SceneKey::new(7);
    let value_key = SceneValueKey::from("root");
    assert_eq!(key.as_u32(), 7);
    assert_eq!(value_key.as_ref(), "root");
}

#[test]
fn parse_script_vars_object() {
    let src = r#"
    [main]
    script = "res://main.rs"
    script_vars = { "cam": Camera, speed: 2.5, enabled: true }
    [Node/]
    [/main]

    [Camera]
    [Node/]
    [/Camera]
    "#;

    let scene = Parser::new(src).parse_scene();
    let main = find_node(&scene, "main");

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
fn node_refs_use_at_before_bare_scene_key() {
    let src = r#"
    $root = @scene_root

    [scene_root]
    [Node/]
    [/scene_root]

    [Child]
    parent = $root
    script_vars = { target = @scene_root }
    [Node/]
    [/Child]
    "#;

    let scene = Parser::new(src).parse_scene();
    let child = find_node(&scene, "Child");

    assert_eq!(
        scene.root.and_then(|k| scene.key_name(k)),
        Some("scene_root")
    );
    assert_eq!(
        child.parent.and_then(|k| scene.key_name(k)),
        Some("scene_root")
    );
    assert!(
        child
            .script_vars
            .iter()
            .any(|(name, value)| name.as_ref() == "target" && value.as_key() == Some("scene_root"))
    );
}

#[test]
fn scene_keys_can_start_with_at_and_refs_escape_at() {
    let src = r#"
    $root = @@@Root

    [@@Root]
    [Node/]
    [/@@Root]

    [Child]
    parent = @@@Root
    script_vars = { target = @@@Root }
    [Node/]
    [/Child]
    "#;

    let scene = Parser::new(src).parse_scene();
    let child = find_node(&scene, "Child");

    assert_eq!(scene.root.and_then(|k| scene.key_name(k)), Some("@@Root"));
    assert_eq!(child.parent.and_then(|k| scene.key_name(k)), Some("@@Root"));
    assert!(
        child
            .script_vars
            .iter()
            .any(|(name, value)| name.as_ref() == "target" && value.as_key() == Some("@@Root"))
    );

    let text = Parser::new(src).parse_scene_doc().to_text();
    assert!(text.contains("$root = @@@Root"));
    assert!(text.contains("[@@Root]"));
    assert!(text.contains("parent = @@@Root"));
    assert!(text.contains("target = @@@Root"));

    let reparsed = Parser::new(&text).parse_scene();
    let child = find_node(&reparsed, "Child");
    assert_eq!(
        reparsed.root.and_then(|k| reparsed.key_name(k)),
        Some("@@Root")
    );
    assert_eq!(
        child.parent.and_then(|k| reparsed.key_name(k)),
        Some("@@Root")
    );
}

#[test]
fn parse_root_of_header() {
    let src = r#"
    [main]
    root_of = "res://child.scn"
    [Node/]
    [/main]
    "#;

    let scene = Parser::new(src).parse_scene();
    let main = find_node(&scene, "main");
    assert_eq!(main.root_of.as_deref(), Some("res://child.scn"));
}

#[test]
fn parse_script_clear_options() {
    let src = r#"
    [main]
    script = null
    [Node/]
    [/main]

    [child]
    clear_script = true
    [Node/]
    [/child]
    "#;

    let scene = Parser::new(src).parse_scene();
    let main = find_node(&scene, "main");
    assert!(main.script.is_none());
    assert!(main.clear_script);

    let child = find_node(&scene, "child");
    assert!(child.script.is_none());
    assert!(child.clear_script);
}

#[test]
fn parse_root_of_without_type_block() {
    let src = r#"
    $root = @main
    [main]
    root_of = "res://base.scn"
    [/main]
    "#;

    let scene = Parser::new(src).parse_scene();
    let main = find_node(&scene, "main");
    assert_eq!(main.root_of.as_deref(), Some("res://base.scn"));
    assert!(!main.has_data_override);
}

#[test]
fn parse_header_only_node_without_type_block_defaults_to_node() {
    let src = r#"
    $root = @scene_root
    [scene_root]
    [Node/]
    [/scene_root]

    [relationship_manager]
    parent = $root
    script = "res://scripts/relationship_manager.rs"
    script_vars = {
        max_character_id = 36
    }
    [/relationship_manager]
    "#;

    let scene = Parser::new(src).parse_scene();
    let node = find_node(&scene, "relationship_manager");

    assert_eq!(node.data.ty.as_ref(), "Node");
    assert!(!node.has_data_override);
    assert_eq!(
        node.parent.and_then(|k| scene.key_name(k)),
        Some("scene_root")
    );
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

#[test]
fn parse_self_closing_type_block() {
    let src = r#"
    $root = @scene_root
    [scene_root]
    [Node2D/]
    [/scene_root]
    "#;

    let scene = Parser::new(src).parse_scene();
    let node = find_node(&scene, "scene_root");

    assert!(node.has_data_override);
    assert_eq!(node.data.ty.as_ref(), "Node2D");
    assert!(node.data.fields.is_empty());
    assert!(node.data.base_ref().is_none());
}

#[test]
fn scene_doc_writes_empty_type_block_self_closing() {
    let src = r#"
    $root = @scene_root
    [scene_root]
    [Node2D/]
    [/scene_root]
    "#;

    let doc = Parser::new(src).parse_scene_doc();
    let text = doc.to_text();

    assert!(text.contains("[Node2D/]"));
    assert!(!text.contains("[/Node2D]"));
    assert_eq!(Parser::new(&text).parse_scene().nodes.len(), 1);
}

#[test]
fn scene_doc_writes_valid_scene_and_syncs_children() {
    let src = r#"
    $root = @scene_root
    $shared = { color: (1, 0, 0, 1), roughness: 0.5 }

    [scene_root]
    [Node/]
    [/scene_root]

    [child]
    parent = $root
    [MeshInstance3D]
        material = $shared
    [/MeshInstance3D]
    [/child]
    "#;

    let mut doc = Parser::new(src).parse_scene_doc();
    doc.normalize_links();
    let root = find_node(&doc.scene, "scene_root");
    assert_eq!(root.children.len(), 1);
    assert_eq!(doc.scene.key_name(root.children[0]), Some("child"));

    let text = doc.to_text();
    let reparsed = Parser::new(&text).parse_scene();
    assert_eq!(
        reparsed.root.and_then(|key| reparsed.key_name(key)),
        Some("scene_root")
    );
    assert!(
        reparsed
            .nodes
            .iter()
            .any(|node| reparsed.key_name(node.key) == Some("child"))
    );
}

#[test]
fn scene_doc_deduplicates_repeated_values() {
    let src = r#"
    $root = @a

    [a]
    [MeshInstance3D]
        material = { roughness: 1.0, metallic: 0.2, color: (1, 1, 1, 1) }
    [/MeshInstance3D]
    [/a]

    [b]
    [MeshInstance3D]
        material = { roughness: 1.0, metallic: 0.2, color: (1, 1, 1, 1) }
    [/MeshInstance3D]
    [/b]
    "#;

    let doc = Parser::new(src).parse_scene_doc();
    let text = doc.to_text();
    assert!(
        text.contains("$var1 = { roughness: 1.0, metallic: 0.2, color: (1.0, 1.0, 1.0, 1.0) }")
    );
    assert_eq!(text.matches("material = $var1").count(), 2);
    let reparsed = Parser::new(&text).parse_scene();
    assert_eq!(reparsed.nodes.len(), 2);
}
