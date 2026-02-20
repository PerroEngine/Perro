pub mod lexer;
pub mod parser;
pub mod runtime_scene;
pub mod static_scene;

pub use lexer::*;
pub use parser::*;
pub use runtime_scene::*;

// Re-export static scene types with different names to avoid confusion
pub use static_scene::{
    Scene as StaticScene, SceneKey as StaticSceneKey, SceneNodeDataEntry as StaticNodeData,
    SceneNodeEntry as StaticNodeEntry, SceneNodeType as StaticNodeType,
    SceneValue as StaticSceneValue,
};

#[cfg(test)]
mod tests {
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
        name = "Player"

        [Sprite2D]
            texture = "res://player.png"

            [Node2D]
                position = (10, 5)
                rotation = 12
            [/Node2D]
        [/Sprite2D]
        [/player]
    "#;

        let parser = Parser::new(src);
        let scene = parser.parse_scene();

        print!("{:#?}", scene);

        // root
        assert_eq!(scene.root.as_deref(), Some("main"));

        // node count
        assert_eq!(scene.nodes.len(), 2);

        // main node
        let main = scene.nodes.iter().find(|n| n.key == "main").unwrap();
        assert_eq!(main.name.as_deref(), Some("Root Node"));

        // player node
        let player = scene.nodes.iter().find(|n| n.key == "player").unwrap();
        assert_eq!(player.parent.as_deref(), Some("main"));

        // Sprite2D
        assert_eq!(player.data.ty, "Sprite2D");

        // texture field
        let texture = player
            .data
            .fields
            .iter()
            .find(|(k, _)| k == "texture")
            .unwrap();
        match &texture.1 {
            RuntimeValue::Str(s) => assert_eq!(s, "res://player.png"),
            _ => panic!("texture should be string"),
        }

        // Node2D base exists
        let base = player.data.base.as_ref().unwrap();
        assert_eq!(base.ty, "Node2D");

        // position field
        let position = base.fields.iter().find(|(k, _)| k == "position").unwrap();
        match &position.1 {
            RuntimeValue::Vec2 { x, y } => assert_eq!((*x, *y), (10.0, 5.0)),
            _ => panic!("position should be Vec2"),
        }
    }

    #[test]
    fn parse_scene_with_shared_variables() {
        let src = r#"
        @root = main
        @shared_texture = "res://shared.png"
        @spawn_pos = (10, 5)
        @p = player

        [main]
        name = "Root"

        [Node2D]
            position = (0, 0)
        [/Node2D]
        [/main]

        [player]
        parent = @root
        name = "Player"

        [Sprite2D]
            texture = @shared_texture

            [Node2D]
                position = @spawn_pos
            [/Node2D]
        [/Sprite2D]
        [/player]

        [enemy]
        parent = @p
        name = "Enemy"

        [Sprite2D]
            texture = @shared_texture
        [/Sprite2D]
        [/enemy]
        "#;

        let parser = Parser::new(src);
        let scene = parser.parse_scene();

        print!("{:#?}", scene);

        // root
        assert_eq!(scene.root.as_deref(), Some("main"));

        // node count
        assert_eq!(scene.nodes.len(), 3);

        let player = scene.nodes.iter().find(|n| n.key == "player").unwrap();
        let enemy = scene.nodes.iter().find(|n| n.key == "enemy").unwrap();

        // both nodes are Sprite2D
        assert_eq!(player.data.ty, "Sprite2D");
        assert_eq!(enemy.data.ty, "Sprite2D");

        // both reference the same resolved texture
        let player_texture = player
            .data
            .fields
            .iter()
            .find(|(k, _)| k == "texture")
            .unwrap();
        match &player_texture.1 {
            RuntimeValue::Str(s) => assert_eq!(s, "res://shared.png"),
            _ => panic!("player texture should resolve to string"),
        }

        let enemy_texture = enemy
            .data
            .fields
            .iter()
            .find(|(k, _)| k == "texture")
            .unwrap();
        match &enemy_texture.1 {
            RuntimeValue::Str(s) => assert_eq!(s, "res://shared.png"),
            _ => panic!("enemy texture should resolve to string"),
        }

        // base Node2D exists for player
        let base = player.data.base.as_ref().unwrap();
        assert_eq!(base.ty, "Node2D");

        // position resolved from @spawn_pos
        let position = base.fields.iter().find(|(k, _)| k == "position").unwrap();
        match &position.1 {
            RuntimeValue::Vec2 { x, y } => assert_eq!((*x, *y), (10.0, 5.0)),
            _ => panic!("position should resolve to Vec2"),
        }

        // enemy parent should be resolved to "player"
        assert_eq!(enemy.parent.as_deref(), Some("player"));
    }

    #[test]
    fn parse_scene_defaults_node_name_to_key() {
        let src = r#"
        @root = main

        [main]
        [Node2D]
            position = (0, 0)
        [/Node2D]
        [/main]

        [bob]
        parent = @root
        [Sprite2D]
            texture = "res://icon.png"
        [/Sprite2D]
        [/bob]
        "#;

        let scene = Parser::new(src).parse_scene();

        let main = scene.nodes.iter().find(|n| n.key == "main").unwrap();
        let bob = scene.nodes.iter().find(|n| n.key == "bob").unwrap();

        assert_eq!(main.name.as_deref(), Some("main"));
        assert_eq!(bob.name.as_deref(), Some("bob"));
    }

    #[test]
    fn parse_node3d_rotation_deg_as_quaternion() {
        let src = r#"
        @root = main

        [main]
        [Node3D]
            rotation_deg = (0, 180, 0)
        [/Node3D]
        [/main]
        "#;

        let scene = Parser::new(src).parse_scene();
        let main = scene.nodes.iter().find(|n| n.key == "main").unwrap();
        assert_eq!(main.data.ty, "Node3D");
        let rotation = main
            .data
            .fields
            .iter()
            .find(|(k, _)| k == "rotation")
            .unwrap();
        match &rotation.1 {
            RuntimeValue::Vec4 { y, w, .. } => {
                assert!((*y - 1.0).abs() < 1.0e-4);
                assert!(w.abs() < 1.0e-4);
            }
            _ => panic!("rotation should be quaternion vec4"),
        }
        assert!(
            main.data
                .fields
                .iter()
                .all(|(k, _)| k != "rotation_deg")
        );
    }

    #[test]
    fn parse_node3d_rotation_euler_radians_as_quaternion() {
        let src = r#"
        @root = main

        [main]
        [Node3D]
            rotation = (0, 3.1415927, 0)
        [/Node3D]
        [/main]
        "#;

        let scene = Parser::new(src).parse_scene();
        let main = scene.nodes.iter().find(|n| n.key == "main").unwrap();
        assert_eq!(main.data.ty, "Node3D");
        let rotation = main
            .data
            .fields
            .iter()
            .find(|(k, _)| k == "rotation")
            .unwrap();
        match &rotation.1 {
            RuntimeValue::Vec4 { y, w, .. } => {
                assert!((*y - 1.0).abs() < 1.0e-4);
                assert!(w.abs() < 1.0e-4);
            }
            _ => panic!("rotation should be quaternion vec4"),
        }
    }

    #[test]
    fn static_scene_equivalent_to_parsed() {
        // Define a static scene manually
        const MAIN_FIELDS: &[(&str, StaticSceneValue)] =
            &[("position", StaticSceneValue::Vec2 { x: 0.0, y: 0.0 })];

        const PLAYER_BASE_FIELDS: &[(&str, StaticSceneValue)] = &[
            ("position", StaticSceneValue::Vec2 { x: 10.0, y: 5.0 }),
            ("rotation", StaticSceneValue::F32(12.0)),
        ];

        const PLAYER_BASE_DATA: StaticNodeData = StaticNodeData {
            ty: StaticNodeType::Node2D,
            fields: PLAYER_BASE_FIELDS,
            base: None,
        };

        const PLAYER_FIELDS: &[(&str, StaticSceneValue)] =
            &[("texture", StaticSceneValue::Str("res://player.png"))];

        const STATIC_SCENE: StaticScene = StaticScene {
            nodes: &[
                StaticNodeEntry {
                    key: StaticSceneKey("main"),
                    name: Some("Root Node"),
                    children: &[StaticSceneKey("player")],
                    parent: None,
                    script: None,
                    data: StaticNodeData {
                        ty: StaticNodeType::Node2D,
                        fields: MAIN_FIELDS,
                        base: None,
                    },
                },
                StaticNodeEntry {
                    key: StaticSceneKey("player"),
                    name: Some("Player"),
                    children: &[],
                    parent: Some(StaticSceneKey("main")),
                    script: None,
                    data: StaticNodeData {
                        ty: StaticNodeType::Sprite2D,
                        fields: PLAYER_FIELDS,
                        base: Some(&PLAYER_BASE_DATA),
                    },
                },
            ],
            root: Some(StaticSceneKey("main")),
        };

        // Parse the same scene
        let src = r#"
        @root = main
        @texture = "res://player.png"

        [main]
        name = "Root Node"

        [Node2D]
            position = (0, 0)
        [/Node2D]
        [/main]

        [player]
        parent = @root
        name = "Player"

        [Sprite2D]
            texture = @texture

            [Node2D]
                position = (10, 5)
                rotation = 12
            [/Node2D]
        [/Sprite2D]
        [/player]
    "#;

        let parser = Parser::new(src);
        let runtime_scene = parser.parse_scene();

        println!("Static Scene: {:#?}", STATIC_SCENE);
        println!("Parsed Runtime Scene: {:#?}", runtime_scene);

        // Compare static and runtime scenes
        assert_eq!(STATIC_SCENE.nodes.len(), runtime_scene.nodes.len());
        assert_eq!(
            STATIC_SCENE.root.map(|k| k.0),
            runtime_scene.root.as_deref()
        );

        // Compare main node
        let static_main = &STATIC_SCENE.nodes[0];
        let runtime_main = runtime_scene
            .nodes
            .iter()
            .find(|n| n.key == "main")
            .unwrap();

        assert_eq!(static_main.key.0, runtime_main.key);
        assert_eq!(static_main.name, runtime_main.name.as_deref());
        assert_eq!(static_main.data.ty.as_str(), runtime_main.data.ty);
        assert_eq!(
            static_main.data.fields.len(),
            runtime_main.data.fields.len()
        );

        // Compare player node
        let static_player = &STATIC_SCENE.nodes[1];
        let runtime_player = runtime_scene
            .nodes
            .iter()
            .find(|n| n.key == "player")
            .unwrap();

        assert_eq!(static_player.key.0, runtime_player.key);
        assert_eq!(static_player.name, runtime_player.name.as_deref());
        assert_eq!(
            static_player.parent.map(|p| p.0),
            runtime_player.parent.as_deref()
        );
        assert_eq!(static_player.data.ty.as_str(), runtime_player.data.ty);

        // Compare texture field
        let static_texture = static_player
            .data
            .fields
            .iter()
            .find(|(k, _)| *k == "texture")
            .unwrap();
        let runtime_texture = runtime_player
            .data
            .fields
            .iter()
            .find(|(k, _)| k == "texture")
            .unwrap();

        match (static_texture.1, &runtime_texture.1) {
            (StaticSceneValue::Str(s), RuntimeValue::Str(r)) => assert_eq!(s, r.as_str()),
            _ => panic!("texture mismatch"),
        }

        // Compare base
        assert!(static_player.data.base.is_some());
        assert!(runtime_player.data.base.is_some());

        let static_base = static_player.data.base.as_ref().unwrap();
        let runtime_base = runtime_player.data.base.as_ref().unwrap();

        assert_eq!(static_base.ty.as_str(), runtime_base.ty);
        assert_eq!(static_base.fields.len(), runtime_base.fields.len());

        println!("✅ Static scene matches parsed runtime scene!");
    }
}
