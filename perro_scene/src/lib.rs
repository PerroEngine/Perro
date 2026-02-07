pub mod lexer;
pub mod parser;
pub mod scene;

pub use lexer::*;
pub use parser::*;
pub use scene::*;

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
        assert_eq!(scene.root, Some(SceneKey("main".to_string())));

        //root var count
        assert_eq!(scene.vars.len(), 1);

        // node count
        assert_eq!(scene.nodes.len(), 2);

        // main node
        let main = scene.nodes.get(&SceneKey("main".to_string())).unwrap();
        assert_eq!(main.name.as_deref(), Some("Root Node"));

        // player node
        let player = scene.nodes.get(&SceneKey("player".to_string())).unwrap();
        assert_eq!(player.parent, Some(SceneKey("main".to_string())));

        // Sprite2D
        assert_eq!(player.data.ty, "Sprite2D");

        // texture field
        match player.data.fields.get("texture").unwrap() {
            SceneValue::Str(s) => assert_eq!(s, "res://player.png"),
            _ => panic!("texture should be string"),
        }

        // Node2D base exists
        let base = player.data.base.as_ref().unwrap();
        assert_eq!(base.ty, "Node2D");

        // position field
        match base.fields.get("position").unwrap() {
            SceneValue::Vec2 { x, y } => assert_eq!((*x, *y), (10.0, 5.0)),
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
        assert_eq!(scene.root, Some(SceneKey("main".to_string())));

        // variables
        assert_eq!(scene.vars.len(), 4);

        match scene.vars.get("shared_texture").unwrap() {
            SceneValue::Str(s) => assert_eq!(s, "res://shared.png"),
            _ => panic!("shared_texture should be string"),
        }

        match scene.vars.get("spawn_pos").unwrap() {
            SceneValue::Vec2 { x, y } => assert_eq!((*x, *y), (10.0, 5.0)),
            _ => panic!("spawn_pos should be Vec2"),
        }

        // node count
        assert_eq!(scene.nodes.len(), 3);

        let player = scene.nodes.get(&SceneKey("player".to_string())).unwrap();

        let enemy = scene.nodes.get(&SceneKey("enemy".to_string())).unwrap();

        // both nodes are Sprite2D
        assert_eq!(player.data.ty, "Sprite2D");
        assert_eq!(enemy.data.ty, "Sprite2D");

        // both reference the same resolved texture
        match player.data.fields.get("texture").unwrap() {
            SceneValue::Str(s) => assert_eq!(s, "res://shared.png"),
            _ => panic!("player texture should resolve to string"),
        }

        match enemy.data.fields.get("texture").unwrap() {
            SceneValue::Str(s) => assert_eq!(s, "res://shared.png"),
            _ => panic!("enemy texture should resolve to string"),
        }

        // base Node2D exists for player
        let base = player.data.base.as_ref().unwrap();
        assert_eq!(base.ty, "Node2D");

        // position resolved from @spawn_pos
        match base.fields.get("position").unwrap() {
            SceneValue::Vec2 { x, y } => assert_eq!((*x, *y), (10.0, 5.0)),
            _ => panic!("position should resolve to Vec2"),
        }
    }
}
