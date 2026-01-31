// ----------------------------------------------------------------
// Resource APIs - Types/resources that can be instantiated
// These are different from Module APIs (global functions)
// ----------------------------------------------------------------

use crate::resource_modules::*;

/// Central router for resource APIs
pub struct CSharpResourceAPI;

impl CSharpResourceAPI {
    pub fn resolve(module: &str, func: &str) -> Option<ResourceModule> {
        match module {
            CSharpSignal::NAME => CSharpSignal::resolve_method(func),
            CSharpTexture::NAME => CSharpTexture::resolve_method(func),
            CSharpMesh::NAME => CSharpMesh::resolve_method(func),
            CSharpArray::NAME => CSharpArray::resolve_method(func),
            CSharpMap::NAME => CSharpMap::resolve_method(func),
            CSharpShape::NAME => CSharpShape::resolve_method(func),
            CSharpQuaternion::NAME => CSharpQuaternion::resolve_method(func),
            _ => None,
        }
    }
}

/// Signal resource API - for creating and managing signals
pub struct CSharpSignal;
impl CSharpSignal {
    pub const NAME: &'static str = "Signal";

    pub fn resolve_method(method: &str) -> Option<ResourceModule> {
        match method {
            "New" | "Create" => Some(ResourceModule::Signal(SignalResource::New)),
            "Connect" => Some(ResourceModule::Signal(SignalResource::Connect)),
            "Emit" => Some(ResourceModule::Signal(SignalResource::Emit)),
            "EmitDeferred" => Some(ResourceModule::Signal(SignalResource::EmitDeferred)),
            _ => None,
        }
    }

    pub fn get_all_method_names() -> Vec<&'static str> {
        vec!["New", "Create", "Connect", "Emit", "EmitDeferred"]
    }
}

/// Texture resource API - for loading and managing textures
pub struct CSharpTexture;
impl CSharpTexture {
    pub const NAME: &'static str = "Texture";

    pub fn resolve_method(method: &str) -> Option<ResourceModule> {
        match method {
            "Load" => Some(ResourceModule::Texture(TextureResource::Load)),
            "Preload" => Some(ResourceModule::Texture(TextureResource::Preload)),
            "Remove" => Some(ResourceModule::Texture(TextureResource::Remove)),
            "CreateFromBytes" => Some(ResourceModule::Texture(TextureResource::CreateFromBytes)),
            "GetWidth" => Some(ResourceModule::Texture(TextureResource::GetWidth)),
            "GetHeight" => Some(ResourceModule::Texture(TextureResource::GetHeight)),
            "GetSize" => Some(ResourceModule::Texture(TextureResource::GetSize)),
            _ => None,
        }
    }

    pub fn get_all_method_names() -> Vec<&'static str> {
        vec![
            "Load",
            "Preload",
            "Remove",
            "CreateFromBytes",
            "GetWidth",
            "GetHeight",
            "GetSize",
        ]
    }
}

/// Mesh resource API - for loading and managing meshes
pub struct CSharpMesh;
impl CSharpMesh {
    pub const NAME: &'static str = "Mesh";

    pub fn resolve_method(method: &str) -> Option<ResourceModule> {
        match method {
            "Load" => Some(ResourceModule::Mesh(MeshResource::Load)),
            "Preload" => Some(ResourceModule::Mesh(MeshResource::Preload)),
            "Remove" => Some(ResourceModule::Mesh(MeshResource::Remove)),
            "Cube" => Some(ResourceModule::Mesh(MeshResource::Cube)),
            "Sphere" => Some(ResourceModule::Mesh(MeshResource::Sphere)),
            "Plane" => Some(ResourceModule::Mesh(MeshResource::Plane)),
            "Cylinder" => Some(ResourceModule::Mesh(MeshResource::Cylinder)),
            "Capsule" => Some(ResourceModule::Mesh(MeshResource::Capsule)),
            "Cone" => Some(ResourceModule::Mesh(MeshResource::Cone)),
            "SquarePyramid" | "SqPyramid" => {
                Some(ResourceModule::Mesh(MeshResource::SquarePyramid))
            }
            "TriangularPyramid" | "TriPyramid" => {
                Some(ResourceModule::Mesh(MeshResource::TriangularPyramid))
            }
            _ => None,
        }
    }

    pub fn get_all_method_names() -> Vec<&'static str> {
        vec![
            "Load",
            "Preload",
            "Remove",
            "Cube",
            "Sphere",
            "Plane",
            "Cylinder",
            "Capsule",
            "Cone",
            "SquarePyramid",
            "SqPyramid",
            "TriangularPyramid",
            "TriPyramid",
        ]
    }
}

/// Array resource API - for array operations
pub struct CSharpArray;
impl CSharpArray {
    pub const NAME: &'static str = "Array";

    pub fn resolve_method(method: &str) -> Option<ResourceModule> {
        match method {
            "Push" | "Add" => Some(ResourceModule::ArrayOp(ArrayResource::Push)),
            "Insert" => Some(ResourceModule::ArrayOp(ArrayResource::Insert)),
            "Remove" | "RemoveAt" => Some(ResourceModule::ArrayOp(ArrayResource::Remove)),
            "Pop" => Some(ResourceModule::ArrayOp(ArrayResource::Pop)),
            "Length" | "Count" => Some(ResourceModule::ArrayOp(ArrayResource::Len)),
            "New" | "Create" => Some(ResourceModule::ArrayOp(ArrayResource::New)),
            _ => None,
        }
    }

    pub fn get_all_method_names() -> Vec<&'static str> {
        vec![
            "Push", "Add", "Insert", "Remove", "RemoveAt", "Pop", "Length", "Count", "New",
            "Create",
        ]
    }
}

/// Map resource API - for map/dictionary operations
pub struct CSharpMap;
impl CSharpMap {
    pub const NAME: &'static str = "Map";

    pub fn resolve_method(method: &str) -> Option<ResourceModule> {
        match method {
            "Add" | "Insert" | "Set" => Some(ResourceModule::MapOp(MapResource::Insert)),
            "Remove" | "Delete" => Some(ResourceModule::MapOp(MapResource::Remove)),
            "Get" | "TryGetValue" => Some(ResourceModule::MapOp(MapResource::Get)),
            "ContainsKey" | "Contains" => Some(ResourceModule::MapOp(MapResource::Contains)),
            "Count" => Some(ResourceModule::MapOp(MapResource::Len)),
            "Clear" => Some(ResourceModule::MapOp(MapResource::Clear)),
            "New" | "Create" => Some(ResourceModule::MapOp(MapResource::New)),
            _ => None,
        }
    }

    pub fn get_all_method_names() -> Vec<&'static str> {
        vec![
            "Add",
            "Insert",
            "Set",
            "Remove",
            "Delete",
            "Get",
            "TryGetValue",
            "ContainsKey",
            "Contains",
            "Count",
            "Clear",
            "New",
            "Create",
        ]
    }
}

/// Shape resource API - for creating 2D shapes
pub struct CSharpShape;
impl CSharpShape {
    pub const NAME: &'static str = "Shape";

    pub fn resolve_method(method: &str) -> Option<ResourceModule> {
        match method {
            "Rectangle" => Some(ResourceModule::Shape(ShapeResource::Rectangle)),
            "Circle" => Some(ResourceModule::Shape(ShapeResource::Circle)),
            "Square" => Some(ResourceModule::Shape(ShapeResource::Square)),
            "Triangle" => Some(ResourceModule::Shape(ShapeResource::Triangle)),
            _ => None,
        }
    }

    pub fn get_all_method_names() -> Vec<&'static str> {
        vec!["Rectangle", "Circle", "Square", "Triangle"]
    }
}

/// Quaternion resource API - for quaternion math helpers
pub struct CSharpQuaternion;
impl CSharpQuaternion {
    pub const NAME: &'static str = "Quaternion";

    pub fn resolve_method(method: &str) -> Option<ResourceModule> {
        match method {
            "Identity" => Some(ResourceModule::QuaternionOp(QuaternionResource::Identity)),
            "FromEuler" => Some(ResourceModule::QuaternionOp(QuaternionResource::FromEuler)),
            "FromEulerXYZ" => Some(ResourceModule::QuaternionOp(
                QuaternionResource::FromEulerXYZ,
            )),
            "AsEuler" => Some(ResourceModule::QuaternionOp(QuaternionResource::AsEuler)),
            "RotateX" => Some(ResourceModule::QuaternionOp(QuaternionResource::RotateX)),
            "RotateY" => Some(ResourceModule::QuaternionOp(QuaternionResource::RotateY)),
            "RotateZ" => Some(ResourceModule::QuaternionOp(QuaternionResource::RotateZ)),
            "RotateEulerXYZ" => Some(ResourceModule::QuaternionOp(
                QuaternionResource::RotateEulerXYZ,
            )),
            _ => None,
        }
    }

    pub fn get_all_method_names() -> Vec<&'static str> {
        vec![
            "Identity",
            "FromEuler",
            "FromEulerXYZ",
            "AsEuler",
            "RotateX",
            "RotateY",
            "RotateZ",
            "RotateEulerXYZ",
        ]
    }
}
