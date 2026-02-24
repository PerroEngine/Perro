pub type SceneNodeType = perro_core::NodeType;

#[derive(Debug)]
pub struct Scene {
    pub nodes: &'static [SceneNodeEntry],
    pub root: Option<SceneKey>,
}

#[derive(Debug, Copy, Clone)]
pub struct SceneNodeEntry {
    pub data: SceneNodeDataEntry,
    pub key: SceneKey,
    pub name: Option<&'static str>,
    pub children: &'static [SceneKey],
    pub parent: Option<SceneKey>,
    pub script: Option<&'static str>,
}

#[derive(Debug, Copy, Clone)]
pub struct SceneNodeDataEntry {
    pub ty: SceneNodeType,
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
    Object(&'static [(&'static str, SceneValue)]),
}
