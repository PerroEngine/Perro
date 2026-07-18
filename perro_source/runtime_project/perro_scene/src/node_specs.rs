use perro_nodes::NodeType;
use std::marker::PhantomData;

use crate::{
    NodeField, SceneNodeField, SceneObjectField, default_scene_field_value,
    resolve_scene_node_field, scene_default_fields, scene_node_asset_fields, scene_node_field,
    scene_node_fields,
};

/// Scene-authored view of one runtime node type.
///
/// Keep this layer free of runtime resource IDs. Asset fields describe source
/// references; the runtime scene loader resolves those references later.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct SceneNodeSpec {
    node_type: NodeType,
}

impl SceneNodeSpec {
    pub const fn new(node_type: NodeType) -> Self {
        Self { node_type }
    }

    pub const fn node_type(self) -> NodeType {
        self.node_type
    }

    pub fn fields(self) -> Vec<SceneNodeField> {
        scene_node_fields(self.node_type)
    }

    pub fn field(self, name: &str) -> Option<SceneNodeField> {
        scene_node_field(self.node_type, name)
    }

    pub fn default_fields(self) -> Vec<SceneObjectField> {
        scene_default_fields(self.node_type)
    }

    pub fn asset_fields(self) -> Vec<SceneNodeField> {
        scene_node_asset_fields(self.node_type)
    }

    pub fn resolve(self, name: &crate::SceneFieldName) -> Option<NodeField> {
        resolve_scene_node_field(self.node_type.name(), name)
    }

    pub fn default_value(self, name: &crate::SceneFieldName) -> Option<crate::SceneValue> {
        default_scene_field_value(self.node_type, name)
    }
}

pub const fn scene_node_spec(node_type: NodeType) -> SceneNodeSpec {
    SceneNodeSpec::new(node_type)
}

/// Typed authored-field key shared by schema and runtime decode.
pub struct SceneFieldKey<T> {
    pub name: &'static str,
    ty: fn() -> crate::NodeFieldType,
    pub aliases: &'static [&'static str],
    marker: PhantomData<fn() -> T>,
}

impl<T> Copy for SceneFieldKey<T> {}

impl<T> Clone for SceneFieldKey<T> {
    fn clone(&self) -> Self {
        *self
    }
}

impl<T> SceneFieldKey<T> {
    pub const fn new(
        name: &'static str,
        ty: fn() -> crate::NodeFieldType,
        aliases: &'static [&'static str],
    ) -> Self {
        Self {
            name,
            ty,
            aliases,
            marker: PhantomData,
        }
    }

    pub fn matches(self, name: &str) -> bool {
        self.name == name || self.aliases.contains(&name)
    }

    pub fn schema(self, section: &'static str) -> crate::SceneNodeField {
        crate::SceneNodeField::new(section, self.name, (self.ty)()).with_aliases(self.aliases)
    }
}

/// Define one reusable field concept group.
/// Names, aliases, scene types, and Rust decode types live in one row.
#[macro_export]
macro_rules! scene_field_group {
    (
        $vis:vis mod $module:ident($section:literal) {
            $(
                $key:ident : $rust_ty:ty = $name:literal => $scene_ty:expr
                $(, aliases [$($alias:literal),* $(,)?])? ;
            )*
        }
    ) => {
        $vis mod $module {
            $(
                pub const $key: $crate::SceneFieldKey<$rust_ty> =
                    $crate::SceneFieldKey::new(
                        $name,
                        || $scene_ty,
                        &[$($($alias),*)?],
                    );
            )*

            pub fn push_schema(fields: &mut Vec<$crate::SceneNodeField>) {
                $(fields.push($key.schema($section));)*
            }
        }
    };
}

crate::scene_field_group! {
    pub mod audio_mask_fields("Audio") {
        ACTIVE: bool = "active" => crate::NodeFieldType::Bool, aliases ["enabled"];
    }
}

crate::scene_field_group! {
    pub mod audio_effect_zone_fields("Audio") {
        ACTIVE: bool = "active" => crate::NodeFieldType::Bool, aliases ["enabled"];
        AUDIO_MASK: perro_structs::BitMask = "audio_mask" => crate::NodeFieldType::BitMask
            , aliases ["audio_masks", "mask", "masks"];
        BOUNCE: bool = "bounce" => crate::NodeFieldType::Bool;
        REVERB: f32 = "reverb" => crate::NodeFieldType::F32
            , aliases ["reverb_send", "reverbSend"];
        ECHO: f32 = "echo" => crate::NodeFieldType::F32;
        DAMPENING: f32 = "dampening" => crate::NodeFieldType::F32
            , aliases ["damping", "low_pass", "lowPass"];
        EFFECTS: crate::SceneValue = "effects" =>
            crate::NodeFieldType::array(crate::NodeFieldType::String)
            , aliases ["effect", "effect_chain", "effectChain"];
    }
}

crate::scene_field_group! {
    pub mod audio_portal_fields("Audio") {
        ACTIVE: bool = "active" => crate::NodeFieldType::Bool, aliases ["enabled"];
        STRENGTH: f32 = "strength" => crate::NodeFieldType::F32;
        TARGETS: crate::SceneValue = "targets" =>
            crate::NodeFieldType::array(crate::NodeFieldType::NodeRef(crate::NodeRefHint::any()))
            , aliases ["connections", "connected"];
    }
}

/// Map compact, Rust-shaped spec types to editor/parser schema types.
/// `Option<T>` changes authored presence, not the value schema itself.
#[doc(hidden)]
#[macro_export]
macro_rules! __scene_node_field_type {
    (bool) => {
        $crate::NodeFieldType::Bool
    };
    (i32) => {
        $crate::NodeFieldType::I32
    };
    (u32) => {
        $crate::NodeFieldType::U32
    };
    (f32) => {
        $crate::NodeFieldType::F32
    };
    (Vec2) => {
        $crate::NodeFieldType::Vec2
    };
    (Vec3) => {
        $crate::NodeFieldType::Vec3
    };
    (Vec4) => {
        $crate::NodeFieldType::Vec4
    };
    (IVec2) => {
        $crate::NodeFieldType::IVec2
    };
    (IVec3) => {
        $crate::NodeFieldType::IVec3
    };
    (IVec4) => {
        $crate::NodeFieldType::IVec4
    };
    (UVec2) => {
        $crate::NodeFieldType::UVec2
    };
    (UVec3) => {
        $crate::NodeFieldType::UVec3
    };
    (UVec4) => {
        $crate::NodeFieldType::UVec4
    };
    (Quat) => {
        $crate::NodeFieldType::Quat
    };
    (Color) => {
        $crate::NodeFieldType::Color
    };
    (String) => {
        $crate::NodeFieldType::String
    };
    (BitMask) => {
        $crate::NodeFieldType::BitMask
    };
    (Unknown) => {
        $crate::NodeFieldType::Unknown
    };
    (Asset($kind:ident)) => {
        $crate::NodeFieldType::Asset($crate::SceneAssetKind::$kind)
    };
    (Option<$inner:ident>) => {
        $crate::__scene_node_field_type!($inner)
    };
    (Option<Asset($kind:ident)>) => {
        $crate::__scene_node_field_type!(Asset($kind))
    };
    (Vec<$inner:ident>) => {
        $crate::NodeFieldType::array($crate::__scene_node_field_type!($inner))
    };
    (Vec<Asset($kind:ident)>) => {
        $crate::NodeFieldType::array($crate::__scene_node_field_type!(Asset($kind)))
    };
}

/// Add fields to a scene-node spec.
///
/// ```ignore
/// scene_node_fields!(fields, "Image", {
///     texture: Asset(Texture);
///     texture_region: Option<Vec4>;
///     flip_x: bool [default(SceneValue::Bool(false))];
///     source: String [aliases["src"]];
/// });
/// ```
#[macro_export]
macro_rules! scene_node_fields {
    ($out:expr, $section:expr, { $($rows:tt)* }) => {
        $crate::scene_node_fields!(@rows $out, $section, $($rows)*);
    };

    (@rows $out:expr, $section:expr,) => {};
    (@rows $out:expr, $section:expr) => {};

    (@rows $out:expr, $section:expr,
        $name:ident : Asset($kind:ident)
        $([$attr:ident $value:tt])*
        ; $($rest:tt)*
    ) => {
        $crate::scene_node_fields!(
            @push $out, $section, stringify!($name),
            $crate::__scene_node_field_type!(Asset($kind)),
            $([$attr $value])*
        );
        $crate::scene_node_fields!(@rows $out, $section, $($rest)*);
    };

    (@rows $out:expr, $section:expr,
        $name:ident : Option<Asset($kind:ident)>
        $([$attr:ident $value:tt])*
        ; $($rest:tt)*
    ) => {
        $crate::scene_node_fields!(
            @push $out, $section, stringify!($name),
            $crate::__scene_node_field_type!(Option<Asset($kind)>),
            $([$attr $value])*
        );
        $crate::scene_node_fields!(@rows $out, $section, $($rest)*);
    };

    (@rows $out:expr, $section:expr,
        $name:ident : Vec<Asset($kind:ident)>
        $([$attr:ident $value:tt])*
        ; $($rest:tt)*
    ) => {
        $crate::scene_node_fields!(
            @push $out, $section, stringify!($name),
            $crate::__scene_node_field_type!(Vec<Asset($kind)>),
            $([$attr $value])*
        );
        $crate::scene_node_fields!(@rows $out, $section, $($rest)*);
    };

    (@rows $out:expr, $section:expr,
        $name:ident : Option<$kind:ident>
        $([$attr:ident $value:tt])*
        ; $($rest:tt)*
    ) => {
        $crate::scene_node_fields!(
            @push $out, $section, stringify!($name),
            $crate::__scene_node_field_type!(Option<$kind>),
            $([$attr $value])*
        );
        $crate::scene_node_fields!(@rows $out, $section, $($rest)*);
    };

    (@rows $out:expr, $section:expr,
        $name:ident : Vec<$kind:ident>
        $([$attr:ident $value:tt])*
        ; $($rest:tt)*
    ) => {
        $crate::scene_node_fields!(
            @push $out, $section, stringify!($name),
            $crate::__scene_node_field_type!(Vec<$kind>),
            $([$attr $value])*
        );
        $crate::scene_node_fields!(@rows $out, $section, $($rest)*);
    };

    (@rows $out:expr, $section:expr,
        $name:ident : $kind:ident
        $([$attr:ident $value:tt])*
        ; $($rest:tt)*
    ) => {
        $crate::scene_node_fields!(
            @push $out, $section, stringify!($name),
            $crate::__scene_node_field_type!($kind),
            $([$attr $value])*
        );
        $crate::scene_node_fields!(@rows $out, $section, $($rest)*);
    };

    (@push $out:expr, $section:expr, $name:expr, $ty:expr, $($attrs:tt)*) => {{
        #[allow(unused_mut)]
        let mut field = $crate::SceneNodeField::new($section, $name, $ty);
        $crate::scene_node_fields!(@attrs field, $($attrs)*);
        $out.push(field);
    }};

    (@attrs $field:ident,) => {};
    (@attrs $field:ident) => {};
    (@attrs $field:ident, [default ($value:expr)] $($rest:tt)*) => {
        $field.default = Some($value);
        $crate::scene_node_fields!(@attrs $field, $($rest)*);
    };
    (@attrs $field:ident, [aliases [$($alias:literal),* $(,)?]] $($rest:tt)*) => {
        $field.aliases = &[$($alias),*];
        $crate::scene_node_fields!(@attrs $field, $($rest)*);
    };
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{NodeFieldType, SceneAssetKind, SceneValue};

    #[test]
    fn rust_shaped_field_spec_builds_schema() {
        let mut fields = Vec::new();
        crate::scene_node_fields!(fields, "Image", {
            texture: Asset(Texture);
            texture_region: Option<Vec4>;
            flip_x: bool [default(SceneValue::Bool(false))] [aliases["mirror_x"]];
        });

        assert_eq!(fields.len(), 3);
        assert!(matches!(
            fields[0].ty,
            NodeFieldType::Asset(SceneAssetKind::Texture)
        ));
        assert!(matches!(fields[1].ty, NodeFieldType::Vec4));
        assert_eq!(fields[2].default, Some(SceneValue::Bool(false)));
        assert_eq!(fields[2].aliases, &["mirror_x"]);
    }

    #[test]
    fn all_node_types_expose_specs() {
        for &node_type in NodeType::ALL {
            let spec = scene_node_spec(node_type);
            assert_eq!(spec.node_type(), node_type);
            let _ = spec.fields();
        }
    }

    #[test]
    fn typed_field_group_owns_aliases_and_schema_type() {
        assert!(audio_portal_fields::ACTIVE.matches("enabled"));
        assert!(audio_portal_fields::TARGETS.matches("connections"));
        let field = audio_portal_fields::STRENGTH.schema("Audio");
        assert_eq!(field.name, "strength");
        assert!(matches!(field.ty, NodeFieldType::F32));
    }
}
