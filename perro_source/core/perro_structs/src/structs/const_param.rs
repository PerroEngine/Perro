#[derive(Clone, Debug, PartialEq)]
pub enum ConstParamValue {
    F32(f32),
    I32(i32),
    Bool(bool),
    Vec2([f32; 2]),
    Vec3([f32; 3]),
    Vec4([f32; 4]),
}

impl Default for ConstParamValue {
    fn default() -> Self {
        Self::F32(0.0)
    }
}
