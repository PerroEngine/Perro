// ----------------------------------------------------------------
// Resource APIs - Types/resources that can be instantiated
// These are different from Module APIs (global functions)
// ----------------------------------------------------------------

use crate::resource_modules::*;

/// Central router for resource APIs
pub struct PupResourceAPI;

impl PupResourceAPI {
    pub fn resolve(module: &str, func: &str) -> Option<ResourceModule> {
        match module {
            PupSignal::NAME => PupSignal::resolve_method(func),
            PupTexture::NAME => PupTexture::resolve_method(func),
            PupMesh::NAME => PupMesh::resolve_method(func),
            PupArray::NAME => PupArray::resolve_method(func),
            PupMap::NAME => PupMap::resolve_method(func),
            PupShape2D::NAME => PupShape2D::resolve_method(func),
            PupQuaternion::NAME => PupQuaternion::resolve_method(func),
            _ => None,
        }
    }

    /// Get all available resource API names
    pub fn get_all_resource_names() -> Vec<&'static str> {
        vec![
            PupSignal::NAME,
            PupTexture::NAME,
            PupMesh::NAME,
            PupArray::NAME,
            PupMap::NAME,
            PupShape2D::NAME,
            PupQuaternion::NAME,
        ]
    }

    /// Check if a name is a valid resource API name
    pub fn is_resource_name(name: &str) -> bool {
        Self::get_all_resource_names().contains(&name)
    }

    /// Get all method names for a given resource name
    pub fn get_method_names_for_resource(resource_name: &str) -> Vec<&'static str> {
        match resource_name {
            PupSignal::NAME => PupSignal::get_all_method_names(),
            PupTexture::NAME => PupTexture::get_all_method_names(),
            PupMesh::NAME => PupMesh::get_all_method_names(),
            PupArray::NAME => PupArray::get_all_method_names(),
            PupMap::NAME => PupMap::get_all_method_names(),
            PupShape2D::NAME => PupShape2D::get_all_method_names(),
            PupQuaternion::NAME => PupQuaternion::get_all_method_names(),
            _ => Vec::new(),
        }
    }
}

/// Signal resource API - for creating and managing signals
pub struct PupSignal;
impl PupSignal {
    pub const NAME: &'static str = "Signal";

    pub fn resolve_method(method: &str) -> Option<ResourceModule> {
        match method {
            "new" => Some(ResourceModule::Signal(SignalResource::New)),
            "connect" => Some(ResourceModule::Signal(SignalResource::Connect)),
            "emit" => Some(ResourceModule::Signal(SignalResource::Emit)),
            "emit_deferred" => Some(ResourceModule::Signal(SignalResource::EmitDeferred)),
            _ => None,
        }
    }

    pub fn get_all_method_names() -> Vec<&'static str> {
        vec!["new", "connect", "emit", "emit_deferred"]
    }
}

/// Texture resource API - for loading and managing textures
pub struct PupTexture;
impl PupTexture {
    pub const NAME: &'static str = "Texture";

    pub fn resolve_method(method: &str) -> Option<ResourceModule> {
        match method {
            "load" => Some(ResourceModule::Texture(TextureResource::Load)),
            "preload" => Some(ResourceModule::Texture(TextureResource::Preload)),
            "remove" => Some(ResourceModule::Texture(TextureResource::Remove)),
            "create_from_bytes" => Some(ResourceModule::Texture(TextureResource::CreateFromBytes)),
            "get_width" => Some(ResourceModule::Texture(TextureResource::GetWidth)),
            "get_height" => Some(ResourceModule::Texture(TextureResource::GetHeight)),
            "get_size" => Some(ResourceModule::Texture(TextureResource::GetSize)),
            _ => None,
        }
    }

    pub fn get_all_method_names() -> Vec<&'static str> {
        vec![
            "load",
            "preload",
            "remove",
            "create_from_bytes",
            "get_width",
            "get_height",
            "get_size",
        ]
    }
}

/// Mesh resource API - for loading/managing meshes
pub struct PupMesh;
impl PupMesh {
    pub const NAME: &'static str = "Mesh";

    pub fn resolve_method(method: &str) -> Option<ResourceModule> {
        match method {
            "load" => Some(ResourceModule::Mesh(MeshResource::Load)),
            "preload" => Some(ResourceModule::Mesh(MeshResource::Preload)),
            "remove" => Some(ResourceModule::Mesh(MeshResource::Remove)),
            _ => None,
        }
    }

    pub fn get_all_method_names() -> Vec<&'static str> {
        vec!["load", "preload", "remove"]
    }
}

/// Array resource API - for array operations
pub struct PupArray;
impl PupArray {
    pub const NAME: &'static str = "Array";

    pub fn resolve_method(method: &str) -> Option<ResourceModule> {
        match method {
            "push" | "append" => Some(ResourceModule::ArrayOp(ArrayResource::Push)),
            "insert" => Some(ResourceModule::ArrayOp(ArrayResource::Insert)),
            "remove" => Some(ResourceModule::ArrayOp(ArrayResource::Remove)),
            "pop" => Some(ResourceModule::ArrayOp(ArrayResource::Pop)),
            "len" | "size" => Some(ResourceModule::ArrayOp(ArrayResource::Len)),
            "new" => Some(ResourceModule::ArrayOp(ArrayResource::New)),
            _ => None,
        }
    }

    pub fn get_all_method_names() -> Vec<&'static str> {
        vec![
            "push", "append", "insert", "remove", "pop", "len", "size", "new",
        ]
    }
}

/// Map resource API - for map/dictionary operations
pub struct PupMap;
impl PupMap {
    pub const NAME: &'static str = "Map";

    pub fn resolve_method(method: &str) -> Option<ResourceModule> {
        match method {
            "insert" => Some(ResourceModule::MapOp(MapResource::Insert)),
            "remove" => Some(ResourceModule::MapOp(MapResource::Remove)),
            "get" => Some(ResourceModule::MapOp(MapResource::Get)),
            "contains" | "contains_key" => Some(ResourceModule::MapOp(MapResource::Contains)),
            "len" | "size" => Some(ResourceModule::MapOp(MapResource::Len)),
            "clear" => Some(ResourceModule::MapOp(MapResource::Clear)),
            "new" => Some(ResourceModule::MapOp(MapResource::New)),
            _ => None,
        }
    }

    pub fn get_all_method_names() -> Vec<&'static str> {
        vec![
            "insert",
            "remove",
            "get",
            "contains",
            "contains_key",
            "len",
            "size",
            "clear",
            "new",
        ]
    }
}

/// Shape2D resource API - for creating 2D shapes
pub struct PupShape2D;
impl PupShape2D {
    pub const NAME: &'static str = "Shape2D";

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
pub struct PupQuaternion;
impl PupQuaternion {
    pub const NAME: &'static str = "Quaternion";

    pub fn resolve_method(method: &str) -> Option<ResourceModule> {
        match method {
            "identity" => Some(ResourceModule::QuaternionOp(QuaternionResource::Identity)),
            "from_euler" => Some(ResourceModule::QuaternionOp(QuaternionResource::FromEuler)),
            "from_euler_xyz" => Some(ResourceModule::QuaternionOp(
                QuaternionResource::FromEulerXYZ,
            )),
            "as_euler" => Some(ResourceModule::QuaternionOp(QuaternionResource::AsEuler)),
            "rotate_x" => Some(ResourceModule::QuaternionOp(QuaternionResource::RotateX)),
            "rotate_y" => Some(ResourceModule::QuaternionOp(QuaternionResource::RotateY)),
            "rotate_z" => Some(ResourceModule::QuaternionOp(QuaternionResource::RotateZ)),
            "rotate_euler_xyz" => Some(ResourceModule::QuaternionOp(
                QuaternionResource::RotateEulerXYZ,
            )),
            _ => None,
        }
    }

    pub fn get_all_method_names() -> Vec<&'static str> {
        vec![
            "identity",
            "from_euler",
            "from_euler_xyz",
            "as_euler",
            "rotate_x",
            "rotate_y",
            "rotate_z",
            "rotate_euler_xyz",
        ]
    }
}
