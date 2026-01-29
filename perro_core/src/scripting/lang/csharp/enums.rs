// ----------------------------------------------------------------
// Built-in Enum Resolver for C# Language
// Maps syntax strings to AST enum types
// ----------------------------------------------------------------

use crate::ast::BuiltInEnumVariant;
use crate::structs::engine_registry::ENGINE_REGISTRY;

/// Resolves enum access syntax to the actual enum variant
/// Returns None if the enum or variant doesn't exist
///
/// Enum names should be PascalCase (C# convention)
/// e.g., NodeType.Sprite2D
pub fn resolve_enum_access(enum_name: &str, variant_name: &str) -> Option<BuiltInEnumVariant> {
    // Normalize enum name to handle PascalCase
    let normalized_enum_name = match enum_name {
        "NodeType" | "NODE_TYPE" => "NODE_TYPE",
        _ => enum_name,
    };

    match normalized_enum_name {
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
