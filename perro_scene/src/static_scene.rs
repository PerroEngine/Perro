#[derive(Debug)]
pub struct Scene {
    pub nodes: &'static [SceneNodeEntry],
    pub root: Option<SceneKey>,
}

#[derive(Debug, Copy, Clone)]
pub struct SceneNodeEntry {
    pub key: SceneKey,
    pub name: Option<&'static str>,
    pub parent: Option<SceneKey>,
    pub script: Option<&'static str>,
    pub data: SceneNodeDataEntry,
}

#[derive(Debug, Copy, Clone)]
pub struct SceneNodeDataEntry {
    pub ty: &'static str,
    pub fields: &'static [(&'static str, SceneValue)],
    pub base: Option<&'static SceneNodeDataEntry>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct SceneKey(pub &'static str);

#[derive(Clone, Copy, Debug)]
pub enum SceneValue {
    Bool(bool),
    I32(i32),
    F32(f32),
    Vec2 { x: f32, y: f32 },
    Vec3 { x: f32, y: f32, z: f32 },
    Vec4 { x: f32, y: f32, z: f32, w: f32 },
    Str(&'static str),
    Key(SceneKey),
}

const EXAMPLE_SCENE: Scene = Scene {
    nodes: &[
        SceneNodeEntry {
            key: SceneKey("player"),
            name: Some("Player"),
            parent: None,
            script: Some("res://player.gd"),
            data: SceneNodeDataEntry {
                ty: "Sprite2D",
                fields: &[
                    ("position", SceneValue::Vec2 { x: 100.0, y: 50.0 }),
                    ("health", SceneValue::I32(100)),
                    ("speed", SceneValue::F32(200.0)),
                ],
                base: None,
            },
        },
        SceneNodeEntry {
            key: SceneKey("enemy"),
            name: Some("Enemy"),
            parent: Some(SceneKey("player")),
            script: Some("res://enemy.gd"),
            data: SceneNodeDataEntry {
                ty: "CharacterBody2D",
                fields: &[
                    ("position", SceneValue::Vec2 { x: 0.0, y: 0.0 }),
                    ("scale", SceneValue::Vec2 { x: 1.0, y: 1.0 }),
                ],
                base: None,
            },
        },
    ],
    root: Some(SceneKey("player")),
};
