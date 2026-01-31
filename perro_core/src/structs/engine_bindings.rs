// ===========================================================
// Engine Bindings - Codegen for node methods registered in engine_registry
// These handle methods that need special codegen (like NodeSugar methods)
// Methods that are real Rust methods will be called directly, but built-in
// methods (like get_parent) need special handling here
// ===========================================================

use crate::{ast::*, prelude::string_to_u64, structs::engine_registry::NodeMethodRef};

/// Trait for generating Rust code from NodeMethodRef
/// Similar to ModuleCodegen but specifically for engine methods
pub trait EngineMethodCodegen {
    /// Generates the final Rust code string for a method call
    fn to_rust_prepared(
        &self,
        args: &[Expr],
        args_strs: &[String],
        script: &Script,
        needs_self: bool,
        current_func: Option<&Function>,
    ) -> String;
}

impl EngineMethodCodegen for NodeMethodRef {
    fn to_rust_prepared(
        &self,
        args: &[Expr],
        args_strs: &[String],
        _script: &Script,
        _needs_self: bool,
        _current_func: Option<&Function>,
    ) -> String {
        match self {
            NodeMethodRef::GetVar => {
                let node_id = args_strs
                    .get(0)
                    .cloned()
                    .unwrap_or_else(|| "self.id".to_string());

                // Check if the variable name is a static literal string or a dynamic expression
                if let Some(Expr::Literal(crate::ast::Literal::String(var_name))) = args.get(1) {
                    // Static variable name - use _id method with precomputed hash
                    let var_id = string_to_u64(var_name);
                    let node_id_clean = node_id.strip_prefix("&").unwrap_or(&node_id);
                    format!("api.get_script_var_id({}, {}u64)", node_id_clean, var_id)
                } else {
                    // Dynamic variable name - use string method
                    let mut name_str = args_strs
                        .get(1)
                        .cloned()
                        .unwrap_or_else(|| "\"\"".to_string());
                    if !name_str.starts_with('"')
                        && !name_str.starts_with('&')
                        && !name_str.contains(".as_str()")
                    {
                        name_str = format!("{}.as_str()", name_str);
                    }
                    let node_id_clean = node_id.strip_prefix("&").unwrap_or(&node_id);
                    format!("api.get_script_var({}, {})", node_id_clean, name_str)
                }
            }

            NodeMethodRef::SetVar => {
                let node_id = args_strs
                    .get(0)
                    .cloned()
                    .unwrap_or_else(|| "self.id".to_string());
                let value = args_strs
                    .get(2)
                    .cloned()
                    .unwrap_or_else(|| "json!({})".to_string());

                if let Some(Expr::Literal(crate::ast::Literal::String(var_name))) = args.get(1) {
                    let var_id = string_to_u64(var_name);
                    let node_id_clean = node_id.strip_prefix("&").unwrap_or(&node_id);
                    format!(
                        "api.set_script_var_id({}, {}u64, json!({}))",
                        node_id_clean, var_id, value
                    )
                } else {
                    // Dynamic variable name - treat like dynamic variable, use .as_str()
                    let mut name_str = args_strs
                        .get(1)
                        .cloned()
                        .unwrap_or_else(|| "\"\"".to_string());
                    // Remove json!() wrapper if present (from expression codegen when expected type is Object/Any)
                    // Handle both "json!(expr)" and "(json!(expr) as Value)" patterns
                    if name_str.contains("json!(") {
                        // Find the start of json!( and extract the inner expression
                        if let Some(json_start) = name_str.find("json!(") {
                            let inner_start = json_start + 6; // Skip "json!("
                            // Find matching closing paren for json!(...)
                            let mut paren_count = 1;
                            let mut json_end = inner_start;
                            for (i, ch) in name_str[inner_start..].char_indices() {
                                if ch == '(' {
                                    paren_count += 1;
                                } else if ch == ')' {
                                    paren_count -= 1;
                                    if paren_count == 0 {
                                        json_end = inner_start + i;
                                        break;
                                    }
                                }
                            }
                            // Extract the inner expression
                            name_str = name_str[inner_start..json_end].to_string();
                            // If there's a trailing " as Value)" or ".as_str()", remove it
                            if name_str.ends_with(" as Value") {
                                name_str = name_str
                                    .strip_suffix(" as Value")
                                    .unwrap_or(&name_str)
                                    .to_string();
                            }
                        }
                    }
                    // Remove any trailing ".as_str()" that might have been added incorrectly
                    if name_str.ends_with(".as_str()") {
                        name_str = name_str
                            .strip_suffix(".as_str()")
                            .unwrap_or(&name_str)
                            .to_string();
                    }
                    // Add .as_str() if not already present and not a string literal
                    if !name_str.starts_with('"')
                        && !name_str.starts_with('&')
                        && !name_str.contains(".as_str()")
                    {
                        name_str = format!("{}.as_str()", name_str);
                    }
                    let node_id_clean = node_id.strip_prefix("&").unwrap_or(&node_id);
                    format!(
                        "api.set_script_var({}, {}, json!({}))",
                        node_id_clean, name_str, value
                    )
                }
            }

            NodeMethodRef::GetChildByName => {
                let node_id = args_strs
                    .get(0)
                    .cloned()
                    .unwrap_or_else(|| "self.id".to_string());
                let child_name = args_strs
                    .get(1)
                    .cloned()
                    .unwrap_or_else(|| "\"\"".to_string());
                let node_id_clean = node_id.strip_prefix("&").unwrap_or(&node_id);
                let child_name_clean = if child_name.starts_with('"') {
                    child_name
                } else if child_name.contains(".as_str()") {
                    child_name
                } else {
                    format!("{}.as_str()", child_name)
                };
                format!(
                    "api.get_child_by_name({}, {})",
                    node_id_clean, child_name_clean
                )
            }

            NodeMethodRef::GetParent => {
                let node_id = args_strs
                    .get(0)
                    .cloned()
                    .unwrap_or_else(|| "self.id".to_string());
                let node_id_clean = node_id.strip_prefix("&").unwrap_or(&node_id);
                format!("api.get_parent({})", node_id_clean)
            }

            NodeMethodRef::AddChild => {
                let parent_id = args_strs
                    .get(0)
                    .cloned()
                    .unwrap_or_else(|| "self.id".to_string());
                let child_id = args_strs
                    .get(1)
                    .cloned()
                    .unwrap_or_else(|| "NodeID::nil()".to_string());
                let parent_id_clean = parent_id.strip_prefix("&").unwrap_or(&parent_id);
                let child_id_clean = child_id.strip_prefix("&").unwrap_or(&child_id);
                format!("api.reparent({}, {})", parent_id_clean, child_id_clean)
            }

            NodeMethodRef::ClearChildren => {
                let node_id = args_strs
                    .get(0)
                    .cloned()
                    .unwrap_or_else(|| "self.id".to_string());
                let node_id_clean = node_id.strip_prefix("&").unwrap_or(&node_id);
                format!("api.clear_children({})", node_id_clean)
            }

            NodeMethodRef::GetType => {
                let node_id = args_strs
                    .get(0)
                    .cloned()
                    .unwrap_or_else(|| "self.id".to_string());
                let node_id_clean = node_id.strip_prefix("&").unwrap_or(&node_id);
                format!("api.get_type({})", node_id_clean)
            }

            NodeMethodRef::GetParentType => {
                let node_id = args_strs
                    .get(0)
                    .cloned()
                    .unwrap_or_else(|| "self.id".to_string());
                let node_id_clean = node_id.strip_prefix("&").unwrap_or(&node_id);
                format!("api.get_parent_type({})", node_id_clean)
            }

            NodeMethodRef::Remove => {
                let node_id = args_strs
                    .get(0)
                    .cloned()
                    .unwrap_or_else(|| "self.id".to_string());
                let node_id_clean = node_id.strip_prefix("&").unwrap_or(&node_id);
                format!("api.remove_node({})", node_id_clean)
            }

            NodeMethodRef::CallFunction => {
                let node_id = args_strs
                    .get(0)
                    .cloned()
                    .unwrap_or_else(|| "self.id".to_string());
                let node_id_clean = node_id.strip_prefix("&").unwrap_or(&node_id);

                if let Some(Expr::Literal(crate::ast::Literal::String(func_name))) = args.get(1) {
                    let func_id = string_to_u64(func_name);
                    let params: Vec<String> = args_strs
                        .iter()
                        .skip(2)
                        .map(|param_str| {
                            if param_str.starts_with("json!(") || param_str.contains("Value") {
                                param_str.clone()
                            } else {
                                format!("json!({})", param_str)
                            }
                        })
                        .collect();

                    if params.is_empty() {
                        format!(
                            "api.call_function_id({}, {}u64, &[])",
                            node_id_clean, func_id
                        )
                    } else {
                        format!(
                            "api.call_function_id({}, {}u64, &[{}])",
                            node_id_clean,
                            func_id,
                            params.join(", ")
                        )
                    }
                } else {
                    // Dynamic function name - treat like dynamic variable, use .as_str()
                    let mut func_name_str = args_strs
                        .get(1)
                        .cloned()
                        .unwrap_or_else(|| "\"\"".to_string());
                    // Remove json!() wrapper if present (from expression codegen when expected type is Object/Any)
                    // Handle both "json!(expr)" and "(json!(expr) as Value)" patterns
                    if func_name_str.contains("json!(") {
                        // Find the start of json!( and extract the inner expression
                        if let Some(json_start) = func_name_str.find("json!(") {
                            let inner_start = json_start + 6; // Skip "json!("
                            // Find matching closing paren for json!(...)
                            let mut paren_count = 1;
                            let mut json_end = inner_start;
                            for (i, ch) in func_name_str[inner_start..].char_indices() {
                                if ch == '(' {
                                    paren_count += 1;
                                } else if ch == ')' {
                                    paren_count -= 1;
                                    if paren_count == 0 {
                                        json_end = inner_start + i;
                                        break;
                                    }
                                }
                            }
                            // Extract the inner expression
                            func_name_str = func_name_str[inner_start..json_end].to_string();
                            // If there's a trailing " as Value)" or ".as_str()", remove it
                            if func_name_str.ends_with(" as Value") {
                                func_name_str = func_name_str
                                    .strip_suffix(" as Value")
                                    .unwrap_or(&func_name_str)
                                    .to_string();
                            }
                        }
                    }
                    // Remove any trailing ".as_str()" that might have been added incorrectly
                    if func_name_str.ends_with(".as_str()") {
                        func_name_str = func_name_str
                            .strip_suffix(".as_str()")
                            .unwrap_or(&func_name_str)
                            .to_string();
                    }
                    // Add .as_str() if not already present and not a string literal
                    if !func_name_str.starts_with('"')
                        && !func_name_str.starts_with('&')
                        && !func_name_str.contains(".as_str()")
                    {
                        func_name_str = format!("{}.as_str()", func_name_str);
                    }
                    let params: Vec<String> = args_strs
                        .iter()
                        .skip(2)
                        .map(|param_str| {
                            if param_str.starts_with("json!(") || param_str.contains("Value") {
                                param_str.clone()
                            } else {
                                format!("json!({})", param_str)
                            }
                        })
                        .collect();

                    if params.is_empty() {
                        format!(
                            "api.call_function({}, {}, &[])",
                            node_id_clean, func_name_str
                        )
                    } else {
                        format!(
                            "api.call_function({}, {}, &[{}])",
                            node_id_clean,
                            func_name_str,
                            params.join(", ")
                        )
                    }
                }
            }

            NodeMethodRef::CallDeferred => {
                let node_id = args_strs
                    .get(0)
                    .cloned()
                    .unwrap_or_else(|| "self.id".to_string());
                let node_id_clean = node_id.strip_prefix("&").unwrap_or(&node_id);

                if let Some(Expr::Literal(crate::ast::Literal::String(func_name))) = args.get(1) {
                    let func_id = string_to_u64(func_name);
                    let params: Vec<String> = args_strs
                        .iter()
                        .skip(2)
                        .map(|param_str| {
                            if param_str.starts_with("json!(") || param_str.contains("Value") {
                                param_str.clone()
                            } else {
                                format!("json!({})", param_str)
                            }
                        })
                        .collect();

                    if params.is_empty() {
                        format!(
                            "api.call_function_id_deferred({}, {}u64, &[])",
                            node_id_clean, func_id
                        )
                    } else {
                        format!(
                            "api.call_function_id_deferred({}, {}u64, &[{}])",
                            node_id_clean,
                            func_id,
                            params.join(", ")
                        )
                    }
                } else {
                    // Dynamic function name - use call_function_deferred(node_id, name, &[])
                    let mut func_name_str = args_strs
                        .get(1)
                        .cloned()
                        .unwrap_or_else(|| "\"\"".to_string());
                    if func_name_str.contains("json!(") {
                        if let Some(json_start) = func_name_str.find("json!(") {
                            let inner_start = json_start + 6;
                            let mut paren_count = 1;
                            let mut json_end = inner_start;
                            for (i, ch) in func_name_str[inner_start..].char_indices() {
                                if ch == '(' {
                                    paren_count += 1;
                                } else if ch == ')' {
                                    paren_count -= 1;
                                    if paren_count == 0 {
                                        json_end = inner_start + i;
                                        break;
                                    }
                                }
                            }
                            func_name_str = func_name_str[inner_start..json_end].to_string();
                            if func_name_str.ends_with(" as Value") {
                                func_name_str = func_name_str
                                    .strip_suffix(" as Value")
                                    .unwrap_or(&func_name_str)
                                    .to_string();
                            }
                        }
                    }
                    if func_name_str.ends_with(".as_str()") {
                        func_name_str = func_name_str
                            .strip_suffix(".as_str()")
                            .unwrap_or(&func_name_str)
                            .to_string();
                    }
                    if !func_name_str.starts_with('"')
                        && !func_name_str.starts_with('&')
                        && !func_name_str.contains(".as_str()")
                    {
                        func_name_str = format!("{}.as_str()", func_name_str);
                    }
                    let params: Vec<String> = args_strs
                        .iter()
                        .skip(2)
                        .map(|param_str| {
                            if param_str.starts_with("json!(") || param_str.contains("Value") {
                                param_str.clone()
                            } else {
                                format!("json!({})", param_str)
                            }
                        })
                        .collect();

                    if params.is_empty() {
                        format!(
                            "api.call_function_deferred({}, {}, &[])",
                            node_id_clean, func_name_str
                        )
                    } else {
                        format!(
                            "api.call_function_deferred({}, {}, &[{}])",
                            node_id_clean,
                            func_name_str,
                            params.join(", ")
                        )
                    }
                }
            }
        }
    }
}
