//! UI Bindings — codegen for UI element field read/mutate.
//!
//! Mirrors engine_bindings for nodes: given UI element type and field name, emits the
//! Rust code for api.read_ui_element / api.mutate_ui_element. Uses ui_registry as
//! the single source of truth for field definitions (rust type, read body, write template).

use crate::nodes::ui::ui_registry::UIElementType;
use crate::structs::script_ui_registry::UI_REGISTRY;

fn ui_element_type_variant_name(et: UIElementType) -> &'static str {
    match et {
        UIElementType::Text => "Text",
        UIElementType::Button => "Button",
        UIElementType::Panel => "Panel",
    }
}

/// Emit Rust for reading a typed UI element field: api.read_ui_element::<T>(ui_node_id, element_id, |e| ...).
/// No turbofish needed for R — Rust infers from closure return.
pub fn emit_read_typed(
    ui_node_id: &str,
    element_id: &str,
    element_type: UIElementType,
    field: &str,
) -> Option<String> {
    let def = UI_REGISTRY.get_field_def(element_type, field)?;
    Some(format!(
        "api.read_ui_element({}, {}, |e: &{}| {})",
        ui_node_id, element_id, def.rust_type_short, def.read_body
    ))
}

/// Emit Rust for reading a DynUIElement field: match on get_element_type, then read_ui_element per variant.
/// When only one element type has the field, emit a single typed call (same idea as dyn nodes).
pub fn emit_read_dyn(ui_node_id: &str, element_id_expr: &str, field: &str) -> Option<String> {
    let types_with_field = UI_REGISTRY.element_types_with_field(field);
    if types_with_field.is_empty() {
        return None;
    }
    // Single type with this field → emit typed read (no match)
    if types_with_field.len() == 1 {
        return emit_read_typed(ui_node_id, element_id_expr, types_with_field[0], field);
    }
    let mut arms = Vec::with_capacity(types_with_field.len() + 2);
    for et in types_with_field {
        if let Some(def) = UI_REGISTRY.get_field_def(*et, field) {
            let variant = ui_element_type_variant_name(*et);
            arms.push(format!(
                "UIElementType::{} => api.read_ui_element({}, {}, |e: &{}| {}),",
                variant, ui_node_id, element_id_expr, def.rust_type_short, def.read_body
            ));
        }
    }
    arms.push("UIElementType::Panel => Default::default(),".to_string());
    arms.push("_ => Default::default(),".to_string());
    Some(format!(
        "match api.get_element_type({}, {}) {{\n            {}\n        }}",
        ui_node_id,
        element_id_expr,
        arms.join("\n            ")
    ))
}

/// Emit Rust for mutating a typed UI element field.
pub fn emit_mutate_typed(
    ui_node_id: &str,
    element_id: &str,
    element_type: UIElementType,
    field: &str,
    rhs: &str,
) -> Option<String> {
    let def = UI_REGISTRY.get_field_def(element_type, field)?;
    let inner = crate::scripting::codegen::optimize_string_from_to_string(&def.write_template.replace("{}", rhs));
    Some(format!(
        "api.mutate_ui_element({}, {}, |e: &mut {}| {{\n            {}\n        }});",
        ui_node_id, element_id, def.rust_type_short, inner
    ))
}

/// Emit Rust for mutating a DynUIElement field: match on get_element_type, then mutate_ui_element per variant.
/// When only one element type has the field, emit a single typed call (same idea as dyn nodes).
pub fn emit_mutate_dyn(
    ui_node_id: &str,
    element_id_expr: &str,
    field: &str,
    rhs: &str,
) -> Option<String> {
    let types_with_field = UI_REGISTRY.element_types_with_field(field);
    if types_with_field.is_empty() {
        return None;
    }
    // Single type with this field → emit typed mutate (no match)
    if types_with_field.len() == 1 {
        return emit_mutate_typed(ui_node_id, element_id_expr, types_with_field[0], field, rhs);
    }
    let mut arms = Vec::with_capacity(types_with_field.len() + 2);
    for et in types_with_field {
        if let Some(def) = UI_REGISTRY.get_field_def(*et, field) {
            let variant = ui_element_type_variant_name(*et);
            let inner = crate::scripting::codegen::optimize_string_from_to_string(&def.write_template.replace("{}", rhs));
            arms.push(format!(
                "UIElementType::{} => api.mutate_ui_element({}, {}, |e: &mut {}| {{ {} }}),",
                variant, ui_node_id, element_id_expr, def.rust_type_short, inner
            ));
        }
    }
    arms.push("UIElementType::Panel => (),".to_string());
    arms.push("_ => (),".to_string());
    Some(format!(
        "match api.get_element_type({}, {}) {{\n            {}\n        }}",
        ui_node_id,
        element_id_expr,
        arms.join("\n            ")
    ))
}

/// Panic message when a field is not on a given element type (for codegen fallback).
pub fn panic_unknown_field(element_type: UIElementType, field: &str) -> String {
    format!(
        "panic!(\"UI element type {:?} does not have field '{}'\")",
        element_type, field
    )
}

/// Panic message for DynUIElement unknown field.
pub fn panic_unknown_dyn_field(field: &str) -> String {
    format!("panic!(\"DynUIElement does not have field '{}'\")", field)
}
