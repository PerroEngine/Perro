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
    assert_eq!(player.data.type_name(), "Sprite2D");
}

#[test]
fn parse_ui_node_type_block() {
    let src = r#"
    [ui]
    [UiNode]
    [/UiNode]
    [/ui]
    "#;

    let scene = Parser::new(src).parse_scene();
    let ui = find_node(&scene, "ui");
    assert_eq!(ui.data.node_type, NodeType::UiNode);
    assert_eq!(ui.data.type_name(), "UiNode");
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
fn parse_script_vars_keep_custom_field_names() {
    let src = r#"
    [main]
    script = "res://main.rs"
    script_vars = { actors = { camera = @CameraNode } }
    [Node/]
    [/main]

    [CameraNode]
    [Node/]
    [/CameraNode]
    "#;

    let scene = Parser::new(src).parse_scene();
    let main = find_node(&scene, "main");

    let actors = main
        .script_vars
        .iter()
        .find(|(name, _)| matches!(name, SceneFieldName::Custom(name) if name.as_ref() == "actors"))
        .expect("actors");
    let SceneValue::Object(actor_fields) = &actors.1 else {
        panic!("actors object");
    };
    assert!(actor_fields.iter().any(
        |(name, _)| matches!(name, SceneFieldName::Custom(name) if name.as_ref() == "camera")
    ));
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
        max_character_id = 36,
    }
    [/relationship_manager]
    "#;

    let scene = Parser::new(src).parse_scene();
    let node = find_node(&scene, "relationship_manager");

    assert_eq!(node.data.type_name(), "Node");
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
    assert_eq!(node.data.type_name(), "Node2D");
    assert!(node.data.fields.is_empty());
    assert!(node.data.base_ref().is_none());
}

#[test]
fn parse_keeps_rotation_deg_field() {
    let src = r#"
    [node_2d]
    [Node2D]
        rotation_deg = 45
    [/Node2D]
    [/node_2d]

    [node_3d]
    [Node3D]
        rotation_deg = (10, 20, 30)
    [/Node3D]
    [/node_3d]
    "#;

    let scene = Parser::new(src).parse_scene();
    let node_2d = find_node(&scene, "node_2d");
    let node_3d = find_node(&scene, "node_3d");

    assert!(
        node_2d
            .data
            .fields
            .iter()
            .any(|(name, value)| name.as_ref() == "rotation_deg" && value.as_f32() == Some(45.0))
    );
    assert!(
        !node_2d
            .data
            .fields
            .iter()
            .any(|(name, _)| name.as_ref() == "rotation")
    );
    assert!(
        node_3d
            .data
            .fields
            .iter()
            .any(|(name, value)| name.as_ref() == "rotation_deg"
                && value.as_vec3() == Some((10.0, 20.0, 30.0)))
    );
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
    let text = doc.to_text_dedup();

    assert!(text.contains("    [Node2D/]"));
    assert!(!text.contains("[/Node2D]"));
    assert_eq!(Parser::new(&text).parse_scene().nodes.len(), 1);
}

#[test]
fn scene_doc_writes_node_data_indented_under_node() {
    let src = r#"
    [camera_stream_3d]
    parent = @PARENTKEY
    script = "res://path/to/script.rs"
    [CameraStream3D]
        camera = @CameraNode
        resolution = (512, 512)
        aspect_ratio = 0.0
        aspect_mode = "fit"
        enabled = true
        size = (1.0, 1.0)
        tint = (1, 1, 1, 1)
        post_processing = []
        [Node3D]
            position = (0, 0, 0)
            rotation = (0, 0, 0, 1)
            scale = (1, 1, 1)
            visible = true
        [/Node3D]
    [/CameraStream3D]
    [/camera_stream_3d]

    [PARENTKEY]
    [Node/]
    [/PARENTKEY]

    [CameraNode]
    [Node/]
    [/CameraNode]
    "#;

    let text = Parser::new(src).parse_scene_doc().to_text();

    assert!(text.contains(
        "[camera_stream_3d]\nparent = @PARENTKEY\nscript = \"res://path/to/script.rs\"\n    [CameraStream3D]\n        camera = @CameraNode"
    ));
    assert!(text.contains("        [Node3D/]"));
    assert!(text.contains("    [/CameraStream3D]\n[/camera_stream_3d]"));
    assert_eq!(Parser::new(&text).parse_scene().nodes.len(), 3);
}

#[test]
fn scene_doc_writes_arrays_and_objects_multiline() {
    let src = r#"
    [main]
    script_vars = { speed = 2.5, target = @CameraNode }
    [MeshInstance3D]
        materials = ["res://a.pmat", "res://b.pmat"]
        material = { type = "standard", roughness_factor = 0.7 }
    [/MeshInstance3D]
    [/main]

    [CameraNode]
    [Node/]
    [/CameraNode]
    "#;

    let text = Parser::new(src).parse_scene_doc().to_text();

    assert!(text.contains("script_vars = {\n    speed = 2.5,\n    target = @CameraNode\n}"));
    assert!(text.contains(
        "        materials = [\n            \"res://a.pmat\",\n            \"res://b.pmat\"\n        ]"
    ));
    assert!(text.contains(
        "        material = {\n            type = \"standard\",\n            roughness_factor = 0.7\n        }"
    ));
    assert_eq!(Parser::new(&text).parse_scene().nodes.len(), 2);
}

#[test]
fn parse_object_allows_line_separated_entries() {
    let scene = Parser::new(
        r#"
        [main]
        script_vars = {
            shape = {
                type = cube
                size = (140.0, 0.1, 140.0)
            }
        }
        [Node/]
        [/main]
        "#,
    )
    .parse_scene();
    let main = find_node(&scene, "main");
    assert_eq!(main.script_vars.len(), 1);
}

#[test]
fn scene_doc_formats_line_separated_object_entries() {
    let src = r#"
    [main]
    script_vars = {
        shape = {
            type = cube
            size = (140.0, 0.1, 140.0)
        }
    }
    [Node/]
    [/main]
    "#;

    let text = Parser::new(src).parse_scene_doc().to_text();

    assert!(text.contains("script_vars = { shape = { type = cube, size = (140.0, 0.1, 140.0) } }"));
    assert_eq!(Parser::new(&text).parse_scene().nodes.len(), 1);
}

#[test]
fn scene_doc_lenient_skips_orphan_close_and_duplicate_node() {
    let src = r#"
    [main]
    [Node/]
    [/main]

    [/MeshInstance3D]
    [/old_main]

    [main]
    [Node/]
    [/main]
    "#;

    let text = Parser::new(src).parse_scene_doc().to_text();

    assert!(!text.contains("[/MeshInstance3D]"));
    assert!(!text.contains("[/old_main]"));
    assert_eq!(text.matches("[main]").count(), 1);
    assert_eq!(Parser::new(&text).parse_scene().nodes.len(), 1);
}

#[test]
fn scene_doc_lenient_parse_adds_missing_commas_before_numeric_key() {
    let src = r#"
    [main]
    script_vars = {
        weights = { 0 = 1.0 1 = 2.0 }
    }
    [Node/]
    [/main]
    "#;

    let text = Parser::new(src).parse_scene_doc().to_text();

    assert!(text.contains("script_vars = { weights = { 0 = 1.0, 1 = 2.0 } }"));
    assert_eq!(Parser::new(&text).parse_scene().nodes.len(), 1);
}

#[test]
fn scene_doc_writes_single_item_arrays_and_objects_inline() {
    let src = r#"
    [main]
    script_vars = { material = { type = "standard", roughness_factor = 0.7 } }
    [MeshInstance3D]
        materials = ["res://a.pmat"]
        material = { type = "standard" }
    [/MeshInstance3D]
    [/main]
    "#;

    let text = Parser::new(src).parse_scene_doc().to_text();

    assert!(
        text.contains(
            "script_vars = { material = { type = \"standard\", roughness_factor = 0.7 } }"
        )
    );
    assert!(text.contains("materials = [\"res://a.pmat\"]"));
    assert!(text.contains("material = { type = \"standard\" }"));
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
fn scene_doc_omits_default_scene_fields() {
    let src = r#"
    [mesh]
    [MeshInstance3D]
        position = (0, 0, 0)
        scale = (1, 1, 1)
        render_layers = all
        surfaces = []
        cast_shadows = true
        receive_shadows = true
    [/MeshInstance3D]
    [/mesh]

    [body]
    [RigidBody3D]
        collision_layers = all
        collision_mask = none
        mass = 1.0
    [/RigidBody3D]
    [/body]
    "#;

    let text = Parser::new(src).parse_scene_doc().to_text();

    assert!(text.contains("    [MeshInstance3D/]"));
    assert!(text.contains("    [RigidBody3D/]"));
    assert!(!text.contains("render_layers"));
    assert!(!text.contains("surfaces"));
    assert!(!text.contains("collision_layers"));
    assert!(!text.contains("collision_mask"));
    assert!(!text.contains("mass"));
}

#[test]
fn scene_doc_does_not_duplicate_root_var() {
    let src = r#"
    $root = @main

    [main]
    [Node/]
    [/main]
    "#;

    let doc = Parser::new(src).parse_scene_doc();
    let text = doc.to_text_dedup();

    assert_eq!(text.matches("$root = @main").count(), 1);
    assert_eq!(Parser::new(&text).parse_scene().nodes.len(), 1);
}

#[test]
fn scene_doc_deduplicates_values_used_three_times() {
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

    [c]
    [MeshInstance3D]
        material = { roughness: 1.0, metallic: 0.2, color: (1, 1, 1, 1) }
    [/MeshInstance3D]
    [/c]
    "#;

    let doc = Parser::new(src).parse_scene_doc();
    let text = doc.to_text_dedup();
    assert!(text.contains("$var1 = {"));
    assert_eq!(text.matches("material = $var1").count(), 3);
    let reparsed = Parser::new(&text).parse_scene();
    assert_eq!(reparsed.nodes.len(), 3);
}

#[test]
fn scene_doc_does_not_deduplicate_values_used_twice() {
    let src = r#"
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
    assert!(!text.contains("$var1 ="));
    assert_eq!(text.matches("material = {").count(), 2);
    let reparsed = Parser::new(&text).parse_scene();
    assert_eq!(reparsed.nodes.len(), 2);
}

#[test]
fn scene_doc_dedup_is_opt_in() {
    let src = r#"
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

    [c]
    [MeshInstance3D]
        material = { roughness: 1.0, metallic: 0.2, color: (1, 1, 1, 1) }
    [/MeshInstance3D]
    [/c]
    "#;

    let doc = Parser::new(src).parse_scene_doc();
    let text = doc.to_text();
    assert!(!text.contains("$var1 ="));
    assert_eq!(text.matches("material = {").count(), 3);
}

#[test]
fn scene_doc_editor_add_node_preserves_script_data() {
    use std::borrow::Cow;

    let src = r#"
$root = @root

[root]
script = "res://scripts/root.rs"
script_vars = { target = @root, speed = 2.5 }
    [Node3D]
        position = (1, 2, 3)
    [/Node3D]
[/root]
"#;

    let mut doc = Parser::new(src).parse_scene_doc();
    let new_key = SceneKey::new(doc.scene.key_names.len() as u32);
    doc.scene
        .key_names
        .to_mut()
        .push(Cow::Borrowed("EditorNode3D_1"));
    doc.scene.nodes.to_mut().push(SceneNodeEntry {
        data: SceneNodeData::new(
            NodeType::Node3D,
            Cow::Owned(vec![(
                SceneFieldName::Position,
                SceneValue::Vec3 {
                    x: 0.0,
                    y: 0.0,
                    z: 0.0,
                },
            )]),
            None,
        ),
        has_data_override: true,
        key: new_key,
        name: None,
        tags: Cow::Owned(Vec::new()),
        children: Cow::Owned(Vec::new()),
        parent: doc.scene.root,
        script: None,
        clear_script: false,
        root_of: None,
        script_vars: Cow::Owned(Vec::new()),
    });
    doc.normalize_links();

    let text = doc.to_text();
    let reparsed = Parser::new(&text).parse_scene();
    let root = find_node(&reparsed, "root");
    let added = find_node(&reparsed, "EditorNode3D_1");

    assert_eq!(root.script.as_deref(), Some("res://scripts/root.rs"));
    assert!(
        root.script_vars
            .iter()
            .any(|(name, value)| name.as_ref() == "target" && value.as_key() == Some("root"))
    );
    assert_eq!(
        added.parent.and_then(|key| reparsed.key_name(key)),
        Some("root")
    );
}
