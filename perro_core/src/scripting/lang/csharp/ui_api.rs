//! C# UI API — script-facing UI element types and fields for C#.
//!
//! Mirrors node_api for nodes: defines what UI element types and fields scripts can use.
//! Uses the central UI registry as the single source of truth; this module provides
//! the C#-facing view and any naming conventions.

use crate::nodes::ui::ui_registry::UIElementType;
use crate::scripting::ast::Type;
use crate::structs::script_ui_registry::UI_REGISTRY;

/// C# script name for a UI element type (e.g. "UIText", "Text").
pub fn csharp_element_type_names(et: UIElementType) -> &'static [&'static str] {
    match et {
        UIElementType::Text => &["UIText", "Text"],
        UIElementType::Button => &["UIButton", "Button"],
        UIElementType::Panel => &["UIPanel", "Panel"],
    }
}

/// Resolve script type name to UIElementType (e.g. "UIText" -> Text).
pub fn resolve_element_type(name: &str) -> Option<UIElementType> {
    match name {
        "UIText" | "Text" => Some(UIElementType::Text),
        "UIButton" | "Button" => Some(UIElementType::Button),
        "UIPanel" | "Panel" => Some(UIElementType::Panel),
        _ => None,
    }
}

/// Get script type for a field on a typed UI element (delegates to UI_REGISTRY).
pub fn get_field_type(element_type: UIElementType, script_field: &str) -> Option<Type> {
    UI_REGISTRY.get_field_type(element_type, script_field)
}

/// Get script type for a field on DynUIElement (any element type that has this field).
pub fn get_dyn_field_type(script_field: &str) -> Option<Type> {
    let types = UI_REGISTRY.element_types_with_field(script_field);
    types.first().and_then(|&et| UI_REGISTRY.get_field_type(et, script_field))
}
