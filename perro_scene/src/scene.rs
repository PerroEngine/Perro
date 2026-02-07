use std::collections::HashMap;

#[derive(Debug, Default)]
pub struct Scene {
    pub nodes: HashMap<SceneKey, SceneNode>,
    pub vars: HashMap<String, SceneValue>,
    pub root: Option<SceneKey>,
}

#[derive(Debug, Default)]
pub struct SceneNode {
    pub name: Option<String>,     // display name
    pub parent: Option<SceneKey>, // key reference
    pub script: Option<String>,   // res://path to script
    pub data: SceneNodeData,
}

#[derive(Debug, Default)]
pub struct SceneNodeData {
    pub ty: String, // Sprite2D, Node2D, etc
    pub fields: HashMap<String, SceneValue>,
    pub base: Option<Box<SceneNodeData>>,
}

#[derive(Clone, Debug, PartialEq, Eq, Hash, Default)]
pub struct SceneKey(pub String);

#[derive(Clone, Debug)]
pub enum SceneValue {
    Bool(bool),
    I32(i32),
    F32(f32),

    Vec2 { x: f32, y: f32 },
    Vec3 { x: f32, y: f32, z: f32 },
    Vec4 { x: f32, y: f32, z: f32, w: f32 },

    Str(String),
    SceneKey(SceneKey),
}
