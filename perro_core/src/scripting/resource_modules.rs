// ----------------------------------------------------------------
// Resource API Enums - Types/resources that can be instantiated
// These are agnostic and don't necessarily have exact API equivalents
// They apply to Nodes and resources like Signal, Texture, Shape, Array, Map
// ----------------------------------------------------------------

/// Unified enum for all resource APIs
#[derive(Debug, Clone)]
pub enum ResourceModule {
    Signal(SignalResource),
    Texture(TextureResource),
    Mesh(MeshResource),
    Scene(SceneResource),
    Shape(ShapeResource),
    ArrayOp(ArrayResource),
    MapOp(MapResource),
    QuaternionOp(QuaternionResource),
}

#[derive(Debug, Clone)]
pub enum SignalResource {
    New,
    Connect,
    Emit,
    EmitDeferred,
}

#[derive(Debug, Clone)]
pub enum TextureResource {
    Load,
    Preload,
    Remove,
    CreateFromBytes,
    GetWidth,
    GetHeight,
    GetSize,
}

#[derive(Debug, Clone)]
pub enum MeshResource {
    Load,
    Preload,
    Remove,
    // Primitive factories (match built-in paths __cube__, __sphere__, etc.)
    Cube,
    Sphere,
    Plane,
    Cylinder,
    Capsule,
    Cone,
    SquarePyramid,
    TriangularPyramid,
}

#[derive(Debug, Clone)]
pub enum SceneResource {
    Load,
    Instantiate,
}

#[derive(Debug, Clone)]
pub enum ShapeResource {
    Rectangle,
    Circle,
    Square,
    Triangle,
}

#[derive(Debug, Clone)]
pub enum ArrayResource {
    Push,
    Pop,
    Insert,
    Remove,
    Len,

    New,
}

#[derive(Debug, Clone)]
pub enum MapResource {
    Insert,
    Remove,
    Get,
    Contains,
    Len,
    Clear,

    New,
}

#[derive(Debug, Clone)]
pub enum QuaternionResource {
    /// Quaternion.identity() -> Quaternion
    Identity,
    /// Quaternion.from_euler(euler_deg: Vector3) -> Quaternion
    FromEuler,
    /// Quaternion.from_euler_xyz(pitch_deg: f32, yaw_deg: f32, roll_deg: f32) -> Quaternion
    FromEulerXYZ,
    /// Quaternion.as_euler(q: Quaternion) -> Vector3 (degrees)
    AsEuler,
    /// Quaternion.rotate_x(q: Quaternion, delta_deg: f32) -> Quaternion
    RotateX,
    /// Quaternion.rotate_y(q: Quaternion, delta_deg: f32) -> Quaternion
    RotateY,
    /// Quaternion.rotate_z(q: Quaternion, delta_deg: f32) -> Quaternion
    RotateZ,
    /// Quaternion.rotate_euler(q: Quaternion, delta_pitch_deg: f32, delta_yaw_deg: f32, delta_roll_deg: f32) -> Quaternion
    RotateEulerXYZ,
}
