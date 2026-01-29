// ----------------------------------------------------------------
// Built-in Enum Resolver for Pup Language
// Maps syntax strings to AST enum types
// ----------------------------------------------------------------

use crate::ast::BuiltInEnumVariant;
use crate::structs::engine_registry::ENGINE_REGISTRY;

/// Resolves enum access syntax to the actual enum variant
/// Returns None if the enum or variant doesn't exist
///
/// Enum names must be SCREAMING_SNAKE_CASE (all caps with underscores)
/// e.g., NODE_TYPE.Sprite2D
pub fn resolve_enum_access(enum_name: &str, variant_name: &str) -> Option<BuiltInEnumVariant> {
    match enum_name {
        "NODE_TYPE" => {
            // Try to find the NodeType variant
            ENGINE_REGISTRY
                .node_defs
                .keys()
                .find(|nt| format!("{:?}", nt) == variant_name)
                .copied()
                .map(BuiltInEnumVariant::NodeType)
        }
        _ => None,
    }
}
