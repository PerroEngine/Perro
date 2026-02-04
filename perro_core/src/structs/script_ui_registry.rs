//! Script-facing UI Registry — single source of truth for UI element types and fields.
//!
//! Mirrors the role of engine_registry for nodes: defines what script APIs exist for UI
//! (UINode methods and per-element-type fields). Used by ui_bindings for codegen and by
//! lang-specific UI APIs (PUP, TypeScript, C#) for type inference and method resolution.
//!
//! Named `script_ui_registry` to avoid collision with `nodes::ui::ui_registry` (element types enum).

use crate::nodes::ui::ui_registry::UIElementType;
use crate::scripting::ast::Type;
use std::collections::HashMap;

/// Script type names that refer to a UI element (PUP/TS/C# may use "UIText" or "Text" etc.).
const UI_ELEMENT_SCRIPT_TYPE_NAMES: &[&str] = &[
    "UIText", "Text", "UIPanel", "Panel", "UIButton", "Button",
];

/// True if this script type is a UI element reference (UIText, UIButton, UIPanel or Type::UIElement).
/// Used by codegen: "as UIText" etc. are type narrows only; value is always Option<UIElementID>, so never emit a Rust cast.
pub fn is_ui_element_ref_type(ty: &Type) -> bool {
    matches!(ty, Type::UIElement(_))
        || matches!(ty, Type::Custom(name) if UI_ELEMENT_SCRIPT_TYPE_NAMES.contains(&name.as_str()))
}

/// Script-visible field on a UI element type (e.g. UIText.content).
#[derive(Debug, Clone)]
pub struct UIElementFieldDef {
    /// Script field name (e.g. "content")
    pub script_name: &'static str,
    /// Script type (e.g. Type::String)
    pub script_type: Type,
    /// Short Rust type for codegen (e.g. "ui_text::UIText") — used in read_ui_element::<T>
    pub rust_type_short: &'static str,
    /// Closure body for read: expression that yields the value (e.g. "e.props.content.clone()")
    pub read_body: &'static str,
    /// Closure body for write: template with "{}" for RHS (e.g. "e.props.content = {}.to_string();")
    pub write_template: &'static str,
}

/// Script-facing UI element type definition: which fields are exposed.
#[derive(Debug, Clone)]
pub struct UIElementTypeDef {
    pub element_type: UIElementType,
    pub fields: Vec<UIElementFieldDef>,
}

/// Registry of UI element types and their script-visible fields.
/// Single source of truth — add new element types or fields here; codegen and langs use this.
#[derive(Debug, Default)]
pub struct UIRegistry {
    /// UIElementType -> field definitions (script name, type, codegen fragments)
    by_type: HashMap<UIElementType, Vec<UIElementFieldDef>>,
    /// Script field name -> which element types have it (for DynUIElement member lookup)
    field_to_types: HashMap<&'static str, Vec<UIElementType>>,
}

impl UIRegistry {
    pub fn new() -> Self {
        let mut reg = Self::default();
        reg.register_all();
        reg
    }

    fn register(&mut self, def: UIElementTypeDef) {
        let et = def.element_type;
        for f in &def.fields {
            self.field_to_types
                .entry(f.script_name)
                .or_default()
                .push(et);
        }
        self.by_type.insert(et, def.fields);
    }

    fn register_all(&mut self) {
        use UIElementType::*;
        self.register(UIElementTypeDef {
            element_type: Text,
            fields: vec![UIElementFieldDef {
                script_name: "content",
                script_type: Type::String,
                rust_type_short: "ui_text::UIText",
                read_body: "e.props.content.clone()",
                write_template: "e.props.content = {}.to_string();",
            }],
        });
        self.register(UIElementTypeDef {
            element_type: Button,
            fields: vec![],
        });
        self.register(UIElementTypeDef {
            element_type: Panel,
            fields: vec![],
        });
    }

    /// Script type for a field on a typed UI element (e.g. UIText -> "content" -> String).
    pub fn get_field_type(&self, element_type: UIElementType, script_field: &str) -> Option<Type> {
        self.by_type
            .get(&element_type)
            .and_then(|fields| {
                fields
                    .iter()
                    .find(|f| f.script_name == script_field)
                    .map(|f| f.script_type.clone())
            })
    }

    /// Field definition for codegen (rust type, read body, write template).
    pub fn get_field_def(
        &self,
        element_type: UIElementType,
        script_field: &str,
    ) -> Option<&UIElementFieldDef> {
        self.by_type
            .get(&element_type)
            .and_then(|fields| fields.iter().find(|f| f.script_name == script_field))
    }

    /// For DynUIElement: which element types have this script field (e.g. "content" -> [Text]).
    pub fn element_types_with_field(&self, script_field: &str) -> &[UIElementType] {
        static EMPTY: Vec<UIElementType> = Vec::new();
        self.field_to_types
            .get(script_field)
            .map(|v| v.as_slice())
            .unwrap_or(&EMPTY)
    }

    /// All script-visible fields for an element type.
    pub fn fields_for_type(&self, element_type: UIElementType) -> &[UIElementFieldDef] {
        static EMPTY: Vec<UIElementFieldDef> = Vec::new();
        self.by_type
            .get(&element_type)
            .map(|v| v.as_slice())
            .unwrap_or(&EMPTY)
    }
}

/// Global script UI registry.
pub static UI_REGISTRY: once_cell::sync::Lazy<UIRegistry> =
    once_cell::sync::Lazy::new(|| UIRegistry::new());
