// ----------------------------------------------------------------
// Resource APIs - Types/resources that can be instantiated
// These are different from Module APIs (global functions)
// ----------------------------------------------------------------

use crate::resource_modules::*;

/// Central router for resource APIs
pub struct TypeScriptResourceAPI;

impl TypeScriptResourceAPI {
    pub fn resolve(module: &str, func: &str) -> Option<ResourceModule> {
        match module {
            TypeScriptSignal::NAME => TypeScriptSignal::resolve_method(func),
            TypeScriptTexture::NAME => TypeScriptTexture::resolve_method(func),
            TypeScriptMesh::NAME => TypeScriptMesh::resolve_method(func),
            TypeScriptScene::NAME => TypeScriptScene::resolve_method(func),
            TypeScriptArray::NAME => TypeScriptArray::resolve_method(func),
            TypeScriptMap::NAME => TypeScriptMap::resolve_method(func),
            TypeScriptShape::NAME => TypeScriptShape::resolve_method(func),
            TypeScriptQuaternion::NAME => TypeScriptQuaternion::resolve_method(func),
            _ => None,
        }
    }
}

/// Signal resource API - for creating and managing signals
pub struct TypeScriptSignal;
impl TypeScriptSignal {
    pub const NAME: &'static str = "Signal";

    pub fn resolve_method(method: &str) -> Option<ResourceModule> {
        match method {
            "new" | "create" => Some(ResourceModule::Signal(SignalResource::New)),
            "connect" => Some(ResourceModule::Signal(SignalResource::Connect)),
            "emit" => Some(ResourceModule::Signal(SignalResource::Emit)),
            "emitDeferred" | "emit_deferred" => {
                Some(ResourceModule::Signal(SignalResource::EmitDeferred))
            }
            _ => None,
        }
    }

    pub fn get_all_method_names() -> Vec<&'static str> {
        vec![
            "new",
            "create",
            "connect",
            "emit",
            "emitDeferred",
            "emit_deferred",
        ]
    }
}

/// Texture resource API - for loading and managing textures
pub struct TypeScriptTexture;
impl TypeScriptTexture {
    pub const NAME: &'static str = "Texture";

    pub fn resolve_method(method: &str) -> Option<ResourceModule> {
        match method {
            "load" => Some(ResourceModule::Texture(TextureResource::Load)),
            "preload" => Some(ResourceModule::Texture(TextureResource::Preload)),
            "remove" => Some(ResourceModule::Texture(TextureResource::Remove)),
            "createFromBytes" | "create_from_bytes" => {
                Some(ResourceModule::Texture(TextureResource::CreateFromBytes))
            }
            "getWidth" | "get_width" => Some(ResourceModule::Texture(TextureResource::GetWidth)),
            "getHeight" | "get_height" => Some(ResourceModule::Texture(TextureResource::GetHeight)),
            "getSize" | "get_size" => Some(ResourceModule::Texture(TextureResource::GetSize)),
            _ => None,
        }
    }

    pub fn get_all_method_names() -> Vec<&'static str> {
        vec![
            "load",
            "preload",
            "remove",
            "createFromBytes",
            "create_from_bytes",
            "getWidth",
            "get_width",
            "getHeight",
            "get_height",
            "getSize",
            "get_size",
        ]
    }
}

/// Mesh resource API - for loading and managing meshes
pub struct TypeScriptMesh;
impl TypeScriptMesh {
    pub const NAME: &'static str = "Mesh";

    pub fn resolve_method(method: &str) -> Option<ResourceModule> {
        match method {
            "load" => Some(ResourceModule::Mesh(MeshResource::Load)),
            "preload" => Some(ResourceModule::Mesh(MeshResource::Preload)),
            "remove" => Some(ResourceModule::Mesh(MeshResource::Remove)),
            "cube" => Some(ResourceModule::Mesh(MeshResource::Cube)),
            "sphere" => Some(ResourceModule::Mesh(MeshResource::Sphere)),
            "plane" => Some(ResourceModule::Mesh(MeshResource::Plane)),
            "cylinder" => Some(ResourceModule::Mesh(MeshResource::Cylinder)),
            "capsule" => Some(ResourceModule::Mesh(MeshResource::Capsule)),
            "cone" => Some(ResourceModule::Mesh(MeshResource::Cone)),
            "squarePyramid" | "square_pyramid" | "sq_pyramid" => {
                Some(ResourceModule::Mesh(MeshResource::SquarePyramid))
            }
            "triangularPyramid" | "triangular_pyramid" | "tri_pyramid" => {
                Some(ResourceModule::Mesh(MeshResource::TriangularPyramid))
            }
            _ => None,
        }
    }

    pub fn get_all_method_names() -> Vec<&'static str> {
        vec![
            "load",
            "preload",
            "remove",
            "cube",
            "sphere",
            "plane",
            "cylinder",
            "capsule",
            "cone",
            "squarePyramid",
            "square_pyramid",
            "sq_pyramid",
            "triangularPyramid",
            "triangular_pyramid",
            "tri_pyramid",
        ]
    }
}

/// Scene resource API - for loading and merging scenes
pub struct TypeScriptScene;
impl TypeScriptScene {
    pub const NAME: &'static str = "Scene";

    pub fn resolve_method(method: &str) -> Option<ResourceModule> {
        match method {
            "load" => Some(ResourceModule::Scene(SceneResource::Load)),
            "instantiate" => Some(ResourceModule::Scene(SceneResource::Instantiate)),
            _ => None,
        }
    }

    pub fn get_all_method_names() -> Vec<&'static str> {
        vec!["load", "instantiate"]
    }
}

/// Array resource API - for array operations
pub struct TypeScriptArray;
impl TypeScriptArray {
    pub const NAME: &'static str = "Array";

    pub fn resolve_method(method: &str) -> Option<ResourceModule> {
        match method {
            "push" => Some(ResourceModule::ArrayOp(ArrayResource::Push)),
            "insert" => Some(ResourceModule::ArrayOp(ArrayResource::Insert)),
            "remove" => Some(ResourceModule::ArrayOp(ArrayResource::Remove)),
            "pop" => Some(ResourceModule::ArrayOp(ArrayResource::Pop)),
            "length" | "len" | "size" => Some(ResourceModule::ArrayOp(ArrayResource::Len)),
            "new" | "create" => Some(ResourceModule::ArrayOp(ArrayResource::New)),
            _ => None,
        }
    }

    pub fn get_all_method_names() -> Vec<&'static str> {
        vec![
            "push", "insert", "remove", "pop", "length", "len", "size", "new", "create",
        ]
    }
}

/// Map resource API - for map/dictionary operations
pub struct TypeScriptMap;
impl TypeScriptMap {
    pub const NAME: &'static str = "Map";

    pub fn resolve_method(method: &str) -> Option<ResourceModule> {
        match method {
            "set" | "insert" => Some(ResourceModule::MapOp(MapResource::Insert)),
            "delete" | "remove" => Some(ResourceModule::MapOp(MapResource::Remove)),
            "get" => Some(ResourceModule::MapOp(MapResource::Get)),
            "has" | "contains" | "containsKey" => {
                Some(ResourceModule::MapOp(MapResource::Contains))
            }
            "size" | "len" => Some(ResourceModule::MapOp(MapResource::Len)),
            "clear" => Some(ResourceModule::MapOp(MapResource::Clear)),
            "new" | "create" => Some(ResourceModule::MapOp(MapResource::New)),
            _ => None,
        }
    }

    pub fn get_all_method_names() -> Vec<&'static str> {
        vec![
            "set",
            "insert",
            "delete",
            "remove",
            "get",
            "has",
            "contains",
            "containsKey",
            "size",
            "len",
            "clear",
            "new",
            "create",
        ]
    }
}

/// Shape resource API - for creating 2D shapes
pub struct TypeScriptShape;
impl TypeScriptShape {
    pub const NAME: &'static str = "Shape";

    pub fn resolve_method(method: &str) -> Option<ResourceModule> {
        match method {
            "rectangle" => Some(ResourceModule::Shape(ShapeResource::Rectangle)),
            "circle" => Some(ResourceModule::Shape(ShapeResource::Circle)),
            "square" => Some(ResourceModule::Shape(ShapeResource::Square)),
            "triangle" => Some(ResourceModule::Shape(ShapeResource::Triangle)),
            _ => None,
        }
    }

    pub fn get_all_method_names() -> Vec<&'static str> {
        vec!["rectangle", "circle", "square", "triangle"]
    }
}

/// Quaternion resource API - for quaternion math helpers
pub struct TypeScriptQuaternion;
impl TypeScriptQuaternion {
    pub const NAME: &'static str = "Quaternion";

    pub fn resolve_method(method: &str) -> Option<ResourceModule> {
        match method {
            "identity" => Some(ResourceModule::QuaternionOp(QuaternionResource::Identity)),
            "fromEuler" | "from_euler" => {
                Some(ResourceModule::QuaternionOp(QuaternionResource::FromEuler))
            }
            "fromEulerXYZ" | "from_euler_xyz" => Some(ResourceModule::QuaternionOp(
                QuaternionResource::FromEulerXYZ,
            )),
            "asEuler" | "as_euler" => {
                Some(ResourceModule::QuaternionOp(QuaternionResource::AsEuler))
            }
            "rotateX" | "rotate_x" => {
                Some(ResourceModule::QuaternionOp(QuaternionResource::RotateX))
            }
            "rotateY" | "rotate_y" => {
                Some(ResourceModule::QuaternionOp(QuaternionResource::RotateY))
            }
            "rotateZ" | "rotate_z" => {
                Some(ResourceModule::QuaternionOp(QuaternionResource::RotateZ))
            }
            "rotateEulerXYZ" | "rotate_euler_xyz" => Some(ResourceModule::QuaternionOp(
                QuaternionResource::RotateEulerXYZ,
            )),
            _ => None,
        }
    }

    pub fn get_all_method_names() -> Vec<&'static str> {
        vec![
            "identity",
            "fromEuler",
            "from_euler",
            "fromEulerXYZ",
            "from_euler_xyz",
            "asEuler",
            "as_euler",
            "rotateX",
            "rotate_x",
            "rotateY",
            "rotate_y",
            "rotateZ",
            "rotate_z",
            "rotateEulerXYZ",
            "rotate_euler_xyz",
        ]
    }
}
