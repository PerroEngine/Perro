use tower_lsp::lsp_types::*;
use perro_core::scripting::ast::{Script, Type};
use perro_core::structs::engine_registry::ENGINE_REGISTRY;
use perro_core::scripting::lang::pup::api::PupAPI;
use perro_core::scripting::lang::pup::resource_api::PupResourceAPI;
use perro_core::scripting::lang::pup::node_api::PUP_NODE_API;
use perro_core::node_registry::NodeType;
use perro_core::scripting::api_modules::*;
use crate::types::ParsedDocument;

/// Get completion items for a position in a PUP file
pub fn get_completions(
    document: &ParsedDocument,
    position: Position,
) -> Vec<CompletionItem> {
    match document {
        ParsedDocument::Pup { script, source, .. } => {
            get_pup_completions(script, source, position)
        }
        ParsedDocument::Fur { source, .. } => {
            get_fur_completions(source, position)
        }
    }
}

fn get_pup_completions(script: &Script, source: &str, position: Position) -> Vec<CompletionItem> {
    // Analyze context - what comes before the cursor
    let lines: Vec<&str> = source.lines().collect();
    if position.line as usize >= lines.len() {
        return Vec::new();
    }
    
    let current_line = lines[position.line as usize];
    let char_offset = position.character as usize;
    
    // Get text before cursor
    let line_text = if char_offset <= current_line.len() {
        &current_line[..char_offset]
    } else {
        current_line
    };
    
    // Get full current line for context
    let full_line = current_line;
    
    // Helper function to route completions based on identifier
    // MUST be defined before we use it in the dot detection logic
    let route_completions = |identifier: &str| -> Option<Vec<CompletionItem>> {
        if identifier.is_empty() {
            return None;
        }
        
        // 1. Check for "self" FIRST - ALWAYS return node completions
        // This must be an exact match to avoid false positives
        if identifier == "self" {
            return Some(get_self_completions(script));
        }
        
        // 2. Check for API modules (Console, Time, JSON, etc.)
        // This MUST come before node type checks to avoid matching "Console" as a node type
        if PupAPI::is_module_name(identifier) {
            // Safely get API module completions - catch any panics
            return Some(get_api_module_completions(identifier));
        }
        
        // 3. Check for resource APIs (Texture, Shape2D, Array, Map, Signal)
        // This MUST also come before node type checks
        if PupResourceAPI::is_resource_name(identifier) {
            return Some(get_api_module_completions(identifier));
        }
        
        // 4. Check if it's a node type name (Node2D, Sprite2D, etc.)
        // This comes AFTER API checks to avoid false matches
        if let Some(node_type) = get_node_type_from_string(identifier) {
            return Some(get_node_completions(node_type));
        }
        
        // 5. Check if it's a variable that's a node type
        if let Some(var) = script.variables.iter().find(|v| v.name == identifier) {
            if let Some(ref var_type) = var.typ {
                if let Some(node_type) = extract_node_type(var_type) {
                    return Some(get_node_completions(node_type));
                }
            }
        }
        
        None
    };
    
    // CRITICAL: Check for member access patterns FIRST (self., Console., Time., etc.)
    // This must happen before any other logic to prevent fallthrough to top-level
    
    // Simple, direct check: look for "IDENTIFIER." pattern anywhere in the line
    // We check multiple positions to catch all cases (dot before cursor, at cursor, after cursor)
    
    // Build a string that includes text before cursor + character at cursor (if it's a dot)
    let text_to_check = if char_offset < full_line.len() && full_line.chars().nth(char_offset) == Some('.') {
        // Dot is at cursor - include it in our check
        format!("{}.", line_text)
    } else {
        // No dot at cursor - just use line_text
        line_text.to_string()
    };
    
    // Check if there's a dot and extract identifier before it
    if let Some(dot_pos) = text_to_check.rfind('.') {
        let before_dot = &text_to_check[..dot_pos];
        let identifier = extract_identifier_before_dot(before_dot.trim_end());
        
        if !identifier.is_empty() {
            // Route completions based on identifier
            if let Some(completions) = route_completions(&identifier) {
                return completions;
            }
        }
        // Found a dot but couldn't resolve identifier - return empty (don't show top-level)
        return Vec::new();
    }
    
    // Also check if dot is right before cursor (edge case)
    if char_offset > 0 && char_offset <= full_line.len() && full_line.chars().nth(char_offset - 1) == Some('.') {
        let before_dot = if char_offset - 1 <= line_text.len() {
            &line_text[..char_offset - 1]
        } else {
            line_text
        };
        let identifier = extract_identifier_before_dot(before_dot.trim_end());
        if !identifier.is_empty() {
            if let Some(completions) = route_completions(&identifier) {
                return completions;
            }
        }
        // Found dot before cursor but couldn't resolve - return empty
        return Vec::new();
    }
    
    // Strategy 3: Check if line ends with "self" (user might be about to type the dot)
    let trimmed = line_text.trim_end();
    if trimmed.ends_with("self") {
        // Make sure it's actually "self" as a complete word
        if trimmed == "self" || trimmed.ends_with(" self") || trimmed.ends_with("\tself") {
            return get_self_completions(script);
        }
        // Check if it's the last word
        if let Some(last_word) = trimmed.split_whitespace().last() {
            if last_word == "self" {
                return get_self_completions(script);
            }
        }
    }
    
    
    // Default: return all top-level completions
    let mut items = Vec::new();

    // Add `self` as a top-level variable so users can discover it
    // (even when completion is manually triggered before typing the dot).
    items.push(create_completion_item(
        "self".to_string(),
        CompletionItemKind::VARIABLE,
        if script.node_type.is_empty() {
            Some("Type: Node".to_string())
        } else {
            Some(format!("Type: {}", script.node_type))
        },
    ));
    
    // Add script variables
    for var in &script.variables {
        items.push(create_completion_item(
            var.name.clone(),
            CompletionItemKind::VARIABLE,
            var.typ.as_ref().map(|t| format!("Type: {}", type_to_string(t))),
        ));
    }
    
    // Add script functions
    for func in &script.functions {
        if !func.is_lifecycle_method {
            let params: Vec<String> = func.params.iter()
                .map(|p| format!("{}: {}", p.name, type_to_string(&p.typ)))
                .collect();
            let signature = format!("{}({})", func.name, params.join(", "));
            
            items.push(create_completion_item(
                func.name.clone(),
                CompletionItemKind::FUNCTION,
                Some(signature),
            ));
        }
    }
    
    // Add Module APIs (global functions) - dynamically from PupAPI
    for module_name in PupAPI::get_all_module_names() {
        items.push(create_completion_item(
            module_name.to_string(),
            CompletionItemKind::MODULE,
            Some("Module API".to_string()),
        ));
    }
    
    // Add Resource APIs (types/resources that can be instantiated) - dynamically from PupResourceAPI
    for resource_name in PupResourceAPI::get_all_resource_names() {
        items.push(create_completion_item(
            resource_name.to_string(),
            CompletionItemKind::CLASS,
            Some("Resource API".to_string()),
        ));
    }
    
    items
}

/// Dynamically generate completions for an API module or resource by name
/// Uses the actual API definitions to get method names
fn get_api_module_completions(module_name: &str) -> Vec<CompletionItem> {
    let mut items = Vec::new();
    
    // Get method names dynamically from the API definitions
    let method_names: Vec<&str> = if PupAPI::is_module_name(module_name) {
        PupAPI::get_method_names_for_module(module_name)
    } else if PupResourceAPI::is_resource_name(module_name) {
        PupResourceAPI::get_method_names_for_resource(module_name)
    } else {
        // Not a known API - return empty (shouldn't happen if called correctly)
        return items;
    };
    
    // If no methods found, return empty (but still return, don't fall through)
    if method_names.is_empty() {
        return items;
    }
    
    for method_name in method_names {
        // Try module API first, then resource API
        let (return_type, param_types, param_names) = if let Some(api_module) = PupAPI::resolve(module_name, method_name) {
            // Safely get types - handle potential panics
            (
                api_module.return_type(),
                api_module.param_types(),
                api_module.param_names()
            )
        } else if let Some(resource_module) = PupResourceAPI::resolve(module_name, method_name) {
            (
                resource_module.return_type(),
                resource_module.param_types(),
                resource_module.param_names()
            )
        } else {
            // Method doesn't resolve - skip it but continue with other methods
            continue;
        };
        
        // Build signature string using core parameter names
        let params_str = if let Some(ref params) = param_types {
            params.iter()
                .enumerate()
                .map(|(i, typ)| {
                    let fixed_type = fix_type_for_completion(typ);
                    // Use core parameter names if available, otherwise fall back to inference
                    let param_name = param_names.as_ref()
                        .and_then(|names| names.get(i).copied())
                        .unwrap_or_else(|| get_fallback_param_name(method_name, i, typ));
                    format!("{}: {}", param_name, type_to_string(&fixed_type))
                })
                .collect::<Vec<_>>()
                .join(", ")
        } else {
            String::new()
        };
        
        let signature = if let Some(ref ret_typ) = return_type {
            let fixed_ret = fix_type_for_completion(ret_typ);
            if fixed_ret == Type::Void {
                format!("{}({})", method_name, params_str)
            } else {
                format!("{}({}) -> {}", method_name, params_str, type_to_string(&fixed_ret))
            }
        } else {
            format!("{}({})", method_name, params_str)
        };
        
        items.push(create_completion_item(
            method_name.to_string(),
            CompletionItemKind::METHOD,
            Some(signature),
        ));
    }
    
    items
}

fn get_fur_completions(_source: &str, _position: Position) -> Vec<CompletionItem> {
    Vec::new()
}

fn create_completion_item(
    label: String,
    kind: CompletionItemKind,
    detail: Option<String>,
) -> CompletionItem {
    CompletionItem {
        label,
        kind: Some(kind),
        detail,
        documentation: None,
        deprecated: None,
        preselect: None,
        sort_text: None,
        filter_text: None,
        insert_text: None,
        insert_text_format: None,
        insert_text_mode: None,
        text_edit: None,
        additional_text_edits: None,
        commit_characters: None,
        command: None,
        data: None,
        tags: None,
        label_details: None,
    }
}

/// Get completions for a specific node type (used for both self and node variables)
fn get_node_completions(node_type: NodeType) -> Vec<CompletionItem> {
    let mut items = Vec::new();
    
    // Get fields and methods from PUP_NODE_API (walks inheritance chain automatically)
    // This should return all fields/methods from the node type and all its base types
    let fields = PUP_NODE_API.get_fields(&node_type);
    let methods = PUP_NODE_API.get_methods(&node_type);
    
    // Debug: Log if we got no fields/methods (this shouldn't happen for registered types)
    // For now, we'll just continue - if both are empty, we'll return empty, which is fine
    
    // Add fields from the node API registry
    for field in fields {
        let display_type = fix_type_for_completion(&field.get_script_type());
        items.push(create_completion_item(
            field.script_name.to_string(),
            CompletionItemKind::FIELD,
            Some(format!("Type: {}", type_to_string(&display_type))),
        ));
    }
    
    // Add methods from the node API registry
    for method in methods {
        let param_types = method.get_param_types();
        let param_names = method.get_param_names();
        
        let params: Vec<String> = if let Some(ref param_types) = param_types {
            param_types.iter()
                .enumerate()
                .map(|(i, typ)| {
                    let fixed_type = fix_type_for_completion(typ);
                    let param_name = param_names.as_ref()
                        .and_then(|names| names.get(i).copied())
                        .unwrap_or_else(|| get_fallback_param_name(method.script_name, i, typ));
                    format!("{}: {}", param_name, type_to_string(&fixed_type))
                })
                .collect()
        } else {
            Vec::new()
        };
        
        let return_type = method.get_return_type()
            .map(|rt| fix_type_for_completion(&rt))
            .unwrap_or(Type::Void);
        
        let signature = if return_type == Type::Void {
            format!("{}({})", method.script_name, params.join(", "))
        } else {
            format!("{}({}) -> {}", method.script_name, params.join(", "), type_to_string(&return_type))
        };
        
        items.push(create_completion_item(
            method.script_name.to_string(),
            CompletionItemKind::METHOD,
            Some(signature),
        ));
    }
    
    items
}

fn get_self_completions(script: &Script) -> Vec<CompletionItem> {
    // Get the node type from the script
    // The script.node_type is set from the "extends NodeType" clause in the PUP file
    // e.g., "extends Sprite2D" -> script.node_type = "Sprite2D"
    
    let node_type_str = &script.node_type;
    
    if node_type_str.is_empty() {
        // No node type specified - return base Node completions
        return get_node_completions(NodeType::Node);
    }
    
    // Try multiple strategies to resolve the node type:
    // 1. Try matching using type_name() method (most reliable - this is what PUP uses)
    if let Some(node_type) = ENGINE_REGISTRY.node_defs.keys().find(|nt| {
        nt.type_name() == node_type_str
    }) {
        return get_node_completions(*node_type);
    }
    
    // 2. Try matching using Debug format (format!("{:?}", nt))
    if let Some(node_type) = ENGINE_REGISTRY.node_defs.keys().find(|nt| {
        format!("{:?}", nt) == *node_type_str
    }) {
        return get_node_completions(*node_type);
    }
    
    // 3. Try parsing directly using FromStr (uses Debug format)
    if let Ok(node_type) = node_type_str.parse::<NodeType>() {
        return get_node_completions(node_type);
    }
    
    // 4. Try case-insensitive matching
    if let Some(node_type) = ENGINE_REGISTRY.node_defs.keys().find(|nt| {
        nt.type_name().eq_ignore_ascii_case(node_type_str) ||
        format!("{:?}", nt).eq_ignore_ascii_case(node_type_str)
    }) {
        return get_node_completions(*node_type);
    }
    
    // If we still couldn't match, return base Node completions
    // This is better than returning empty, as it at least gives some completions
    get_node_completions(NodeType::Node)
}

/// Extract NodeType from a Type, handling Node, DynNode, and Option<Node>
fn extract_node_type(typ: &Type) -> Option<NodeType> {
    match typ {
        Type::Node(node_type) => Some(*node_type),
        Type::DynNode => Some(NodeType::Node), // DynNode can use base Node methods
        Type::Option(inner) => extract_node_type(inner),
        _ => None,
    }
}

/// Fix types for completion display - Node should be DynNode for arrays of nodes
fn fix_type_for_completion(typ: &Type) -> Type {
    match typ {
        Type::Container(kind, types) => {
            let fixed_types: Vec<Type> = types.iter()
                .map(|t| {
                    match t {
                        Type::Node(perro_core::node_registry::NodeType::Node) => Type::DynNode,
                        _ => fix_type_for_completion(t),
                    }
                })
                .collect();
            Type::Container(kind.clone(), fixed_types)
        }
        Type::Node(perro_core::node_registry::NodeType::Node) => Type::DynNode,
        _ => typ.clone(),
    }
}

/// Fallback parameter name inference when core doesn't provide names
/// This is only used as a last resort - prefer adding names to core API bindings
fn get_fallback_param_name(method_name: &str, param_index: usize, param_type: &Type) -> &'static str {
    match param_type {
        Type::String | Type::StrRef | Type::CowStr => {
            if method_name.contains("get") || method_name.contains("find") {
                "name"
            } else if method_name.contains("set") && param_index == 0 {
                "name"
            } else {
                "key"
            }
        }
        Type::Node(_) | Type::DynNode => {
            if method_name.contains("child") {
                "child"
            } else {
                "node"
            }
        }
        Type::Number(_) => {
            if method_name.contains("index") || method_name.contains("idx") {
                "index"
            } else if method_name.contains("count") || method_name.contains("size") {
                "count"
            } else {
                "value"
            }
        }
        Type::Bool => "value",
        Type::Any | Type::Object => "value",
        _ => "arg",
    }
}

/// Extract the last identifier before a dot
/// Handles cases like "Node2D.", "Texture.", "my_var.", "  self.", etc.
/// Uses a safe character-by-character approach to avoid UTF-8 slicing issues
fn extract_identifier_before_dot(before_dot: &str) -> String {
    // Trim whitespace from both ends first
    let trimmed = before_dot.trim();
    if trimmed.is_empty() {
        return String::new();
    }
    
    // Build the identifier by scanning backwards character by character
    // This is safer than string slicing with UTF-8
    let mut result = String::new();
    
    for ch in trimmed.chars().rev() {
        if ch.is_alphanumeric() || ch == '_' {
            result.push(ch);
        } else {
            // Non-identifier character - stop here (we've found the end of the identifier)
            break;
        }
    }
    
    // Reverse the result since we built it backwards
    result.chars().rev().collect()
}

fn get_node_type_from_string(name: &str) -> Option<NodeType> {
    // IMPORTANT: Don't match API module names or resource API names as node types
    // This prevents "Console", "Time", "Texture", etc. from being treated as node types
    if PupAPI::is_module_name(name) || PupResourceAPI::is_resource_name(name) {
        return None;
    }
    
    // Try multiple strategies to find the node type:
    // 1. Try matching using type_name() method (most reliable)
    if let Some(node_type) = ENGINE_REGISTRY.node_defs.keys().find(|nt| {
        nt.type_name() == name
    }) {
        return Some(*node_type);
    }
    
    // 2. Try matching using Debug format (format!("{:?}", nt))
    if let Some(node_type) = ENGINE_REGISTRY.node_defs.keys().find(|nt| {
        format!("{:?}", nt) == name
    }) {
        return Some(*node_type);
    }
    
    // 3. Try case-insensitive matching as a fallback
    ENGINE_REGISTRY.node_defs.keys().find(|nt| {
        nt.type_name().eq_ignore_ascii_case(name) ||
        format!("{:?}", nt).eq_ignore_ascii_case(name)
    }).cloned()
}

/// Convert a Type to a user-friendly string representation for LSP
/// Uses the PUP type representation since that's what the LSP is primarily for
pub fn type_to_string(typ: &Type) -> String {
    typ.to_pup_type()
}
