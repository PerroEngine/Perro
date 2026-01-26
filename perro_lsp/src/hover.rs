use tower_lsp::lsp_types::*;
use perro_core::scripting::ast::{Script, Type, Function, Variable};
use crate::types::ParsedDocument;
use crate::completion::type_to_string;

/// Get hover information for a position
pub fn get_hover(
    document: &ParsedDocument,
    position: Position,
) -> Option<Hover> {
    match document {
        ParsedDocument::Pup { script, .. } => {
            get_pup_hover(script, position)
        }
        ParsedDocument::Fur { .. } => {
            // FUR hover support can be added here
            None
        }
    }
}

fn get_pup_hover(script: &Script, _position: Position) -> Option<Hover> {
    // This is a simplified version - in a real implementation,
    // you'd parse the position to find what identifier is being hovered
    
    // For now, return None - this would need position-based lookup
    // to find which variable/function/type is at the cursor position
    None
}

/// Get type information for a variable or expression
pub fn get_type_info(script: &Script, name: &str) -> Option<String> {
    // Check variables
    if let Some(var) = script.variables.iter().find(|v| v.name == name) {
        if let Some(ref typ) = var.typ {
            return Some(format_type(typ));
        }
    }
    
    // Check functions
    if let Some(func) = script.functions.iter().find(|f| f.name == name) {
        let params: Vec<String> = func.params.iter()
            .map(|p| format!("{}: {}", p.name, format_type(&p.typ)))
            .collect();
        let return_type = format_type(&func.return_type);
        return Some(format!("fn {}({}) -> {}", func.name, params.join(", "), return_type));
    }
    
    None
}

fn format_type(typ: &Type) -> String {
    // Use the same type formatting as completion for consistency
    type_to_string(typ)
}
