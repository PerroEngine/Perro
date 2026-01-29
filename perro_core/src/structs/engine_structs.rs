#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum EngineStruct {
    // 2D structs
    Transform2D,
    Vector2,
    Rect,
    Color,
    Texture,
    Shape2D,
    // 3D structs
    Transform3D,
    Vector3,
    Quaternion,
}

impl EngineStruct {
    /// Convert a type name string to an EngineStruct variant
    pub fn from_string(type_name: &str) -> Option<Self> {
        match type_name {
            "Vector2" => Some(EngineStruct::Vector2),
            "Vector3" => Some(EngineStruct::Vector3),
            "Transform2D" => Some(EngineStruct::Transform2D),
            "Transform3D" => Some(EngineStruct::Transform3D),
            "Color" => Some(EngineStruct::Color),
            "Rect" => Some(EngineStruct::Rect),
            "Quaternion" => Some(EngineStruct::Quaternion),
            "Shape2D" => Some(EngineStruct::Shape2D),
            "Texture" => Some(EngineStruct::Texture),
            _ => None,
        }
    }

    /// Check if a type name is an engine struct
    pub fn is_engine_struct(type_name: &str) -> bool {
        Self::from_string(type_name).is_some()
    }
}
