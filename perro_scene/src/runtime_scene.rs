// runtime_scene.rs - Runtime/parsed types with owned data

#[derive(Debug, Clone)]
pub struct RuntimeScene {
    pub nodes: Vec<RuntimeNodeEntry>,
    pub root: Option<String>,
}

#[derive(Debug, Clone)]
pub struct RuntimeNodeEntry {
    pub data: RuntimeNodeData,
    pub key: String,
    pub name: Option<String>,
    pub parent: Option<String>,
    pub script: Option<String>,
}

#[derive(Debug, Clone)]
pub struct RuntimeNodeData {
    pub ty: String,
    pub fields: Vec<(String, RuntimeValue)>,
    pub base: Option<Box<RuntimeNodeData>>,
}

#[derive(Clone, Debug)]
pub enum RuntimeValue {
    Bool(bool),
    I32(i32),
    F32(f32),
    Vec2 { x: f32, y: f32 },
    Vec3 { x: f32, y: f32, z: f32 },
    Vec4 { x: f32, y: f32, z: f32, w: f32 },
    Str(String),
    Key(String),
}
