#[derive(Clone, Copy, Debug, PartialEq)]
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

// Ergonomic conversions so callers can pass a literal:
// `set_material_param!(res, mat, "glow", 0.7)` instead of naming the variant.
impl From<f32> for ConstParamValue {
    fn from(value: f32) -> Self {
        Self::F32(value)
    }
}

impl From<i32> for ConstParamValue {
    fn from(value: i32) -> Self {
        Self::I32(value)
    }
}

impl From<bool> for ConstParamValue {
    fn from(value: bool) -> Self {
        Self::Bool(value)
    }
}

impl From<[f32; 2]> for ConstParamValue {
    fn from(value: [f32; 2]) -> Self {
        Self::Vec2(value)
    }
}

impl From<[f32; 3]> for ConstParamValue {
    fn from(value: [f32; 3]) -> Self {
        Self::Vec3(value)
    }
}

impl From<[f32; 4]> for ConstParamValue {
    fn from(value: [f32; 4]) -> Self {
        Self::Vec4(value)
    }
}

#[cfg(test)]
mod const_param_from_tests {
    use super::*;

    #[test]
    fn literals_convert_to_the_matching_variant() {
        assert_eq!(ConstParamValue::from(0.7f32), ConstParamValue::F32(0.7));
        assert_eq!(ConstParamValue::from(3i32), ConstParamValue::I32(3));
        assert_eq!(ConstParamValue::from(true), ConstParamValue::Bool(true));
        assert_eq!(
            ConstParamValue::from([1.0f32, 2.0]),
            ConstParamValue::Vec2([1.0, 2.0])
        );
        assert_eq!(
            ConstParamValue::from([1.0f32, 2.0, 3.0]),
            ConstParamValue::Vec3([1.0, 2.0, 3.0])
        );
        assert_eq!(
            ConstParamValue::from([1.0f32, 2.0, 3.0, 4.0]),
            ConstParamValue::Vec4([1.0, 2.0, 3.0, 4.0])
        );
    }
}
