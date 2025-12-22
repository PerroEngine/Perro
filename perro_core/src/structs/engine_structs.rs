#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum EngineStruct {
    // 2D structs
    Transform2D,
    Vector2,
    Rect,
    Color,
    ImageTexture,
    ShapeType2D,
    // 3D structs
    Transform3D,
    Vector3,
    Quaternion,
}
