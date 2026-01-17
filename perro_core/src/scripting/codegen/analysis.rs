// Analysis functions for self usage, node access, etc.
use std::collections::HashMap;
use crate::api_modules::*;
use crate::ast::*;
use crate::scripting::ast::{Expr, Stmt, Type};
use super::utils::{is_node_type, string_to_node_type, rename_variable, type_is_node, type_becomes_id};

pub(crate) fn analyze_self_usage(script: &mut Script) {
    // Step 1: mark direct `self.node` usage
    let uses_self_flags: Vec<bool> = script
        .functions
        .iter()
        .map(|func| {
            func.body
                .iter()
                .any(|stmt| stmt_accesses_node(stmt, script))
        })
        .collect();

    // Step 1.5: Collect cloned child nodes for each function
    let cloned_nodes_per_func: Vec<Vec<String>> = script
        .functions
        .iter()
        .map(|func| {
            let mut cloned_nodes = Vec::new();
            let mut cloned_ui_elements = Vec::new();
            collect_cloned_node_vars(
                &func.body,
                &mut cloned_nodes,
                &mut cloned_ui_elements,
                script,
            );
            cloned_nodes
        })
        .collect();

    // Second pass: apply the flags and cloned child nodes
    for (func, (uses_self, cloned_nodes)) in script
        .functions
        .iter_mut()
        .zip(uses_self_flags.iter().zip(cloned_nodes_per_func.iter()))
    {
        func.uses_self = *uses_self;
        func.cloned_child_nodes = cloned_nodes.clone();
    }

    // Step 2: track which functions call which others
    let mut edges: HashMap<String, Vec<String>> = HashMap::new();
    for func in &script.functions {
        let callees = extract_called_functions(&func.body);
        edges.insert(func.name.clone(), callees);
    }

    // Step 3: recursively propagate self usage through the call graph
    let mut changed = true;
    while changed {
        changed = false;

        let snapshot: Vec<(String, bool)> = script
            .functions
            .iter()
            .map(|f| (f.name.clone(), f.uses_self))
            .collect();

        for func in &mut script.functions {
            if !func.uses_self {
                if let Some(callees) = edges.get(&func.name) {
                    if callees.iter().any(|callee_name| {
                        snapshot
                            .iter()
                            .any(|(name, uses_self)| name == callee_name && *uses_self)
                    }) {
                        func.uses_self = true;
                        changed = true;
                    }
                }
            }
        }
    }
}

fn expr_accesses_node(expr: &Expr, script: &Script) -> bool {
    match expr {
        Expr::SelfAccess => true,
        Expr::MemberAccess(base, field) => {
            if matches!(base.as_ref(), Expr::SelfAccess) {
                if field == "id" {
                    return true;
                }
                let is_script_member = script.variables.iter().any(|v| v.name == *field)
                    || script.functions.iter().any(|f| f.name == *field);
                if is_script_member {
                    return false;
                }
            }
            expr_accesses_node(base, script)
        }
        Expr::BinaryOp(left, _, right) => {
            expr_accesses_node(left, script) || expr_accesses_node(right, script)
        }
        Expr::Call(target, args) => {
            expr_accesses_node(target, script)
                || args.iter().any(|arg| expr_accesses_node(arg, script))
        }
        _ => false,
    }
}

fn stmt_accesses_node(stmt: &Stmt, script: &Script) -> bool {
    match stmt {
        Stmt::Expr(e) => expr_accesses_node(&e.expr, script),
        Stmt::VariableDecl(var) => var
            .value
            .as_ref()
            .map_or(false, |e| expr_accesses_node(&e.expr, script)),
        Stmt::Assign(_, e) | Stmt::AssignOp(_, _, e) => expr_accesses_node(&e.expr, script),
        Stmt::MemberAssign(lhs, rhs) | Stmt::MemberAssignOp(lhs, _, rhs) => {
            expr_accesses_node(&lhs.expr, script) || expr_accesses_node(&rhs.expr, script)
        }
        Stmt::ScriptAssign(_, _, expr) | Stmt::ScriptAssignOp(_, _, _, expr) => {
            expr_accesses_node(&expr.expr, script)
        }
        Stmt::IndexAssign(array, index, value)
        | Stmt::IndexAssignOp(array, index, _, value) => {
            expr_accesses_node(array, script)
                || expr_accesses_node(index, script)
                || expr_accesses_node(&value.expr, script)
        }
        Stmt::Pass => false,
        Stmt::If {
            condition,
            then_body,
            else_body,
        } => {
            expr_accesses_node(&condition.expr, script)
                || then_body.iter().any(|s| stmt_accesses_node(s, script))
                || else_body.as_ref().map_or(false, |body| {
                    body.iter().any(|s| stmt_accesses_node(s, script))
                })
        }
        Stmt::For { iterable, body, .. } => {
            expr_accesses_node(&iterable.expr, script)
                || body.iter().any(|s| stmt_accesses_node(s, script))
        }
        Stmt::ForTraditional {
            init,
            condition,
            increment,
            body,
        } => {
            (init
                .as_ref()
                .map_or(false, |s| stmt_accesses_node(s.as_ref(), script)))
                || (condition
                    .as_ref()
                    .map_or(false, |c| expr_accesses_node(&c.expr, script)))
                || (increment
                    .as_ref()
                    .map_or(false, |s| stmt_accesses_node(s.as_ref(), script)))
                || body.iter().any(|s| stmt_accesses_node(s, script))
        }
    }
}

pub(crate) fn collect_cloned_node_vars(
    stmts: &[Stmt],
    cloned_nodes: &mut Vec<String>,
    cloned_ui_elements: &mut Vec<(String, String, String)>,
    script: &Script,
) {
    fn expr_contains_get_node(expr: &Expr, _verbose: bool) -> bool {
        match expr {
            Expr::ApiCall(ApiModule::NodeSugar(NodeSugarApi::GetChildByName), _) => true,
            Expr::ApiCall(ApiModule::NodeSugar(NodeSugarApi::GetParent), _) => true,
            Expr::Cast(inner, target_type) => {
                let is_node_type_cast = match target_type {
                    Type::Custom(tn) => is_node_type(tn),
                    Type::Node(_) => true,
                    _ => false,
                };
                is_node_type_cast && expr_contains_get_node(inner, false)
            }
            _ => false,
        }
    }

    for stmt in stmts {
        match stmt {
            Stmt::VariableDecl(var) => {
                if let Some(value) = &var.value {
                    if expr_contains_get_node(&value.expr, false) {
                        cloned_nodes.push(var.name.clone());
                    }
                }
            }
            Stmt::If { then_body, else_body, .. } => {
                collect_cloned_node_vars(then_body, cloned_nodes, cloned_ui_elements, script);
                if let Some(else_body) = else_body {
                    collect_cloned_node_vars(else_body, cloned_nodes, cloned_ui_elements, script);
                }
            }
            Stmt::For { body, .. } | Stmt::ForTraditional { body, .. } => {
                collect_cloned_node_vars(body, cloned_nodes, cloned_ui_elements, script);
            }
            _ => {}
        }
    }
}

fn extract_called_functions(stmts: &[Stmt]) -> Vec<String> {
    let mut called = Vec::new();
    for stmt in stmts {
        match stmt {
            Stmt::Expr(e) => {
                if let Expr::Call(target, _) = &e.expr {
                    if let Expr::Ident(name) = target.as_ref() {
                        called.push(name.clone());
                    }
                }
            }
            Stmt::If { then_body, else_body, .. } => {
                called.extend(extract_called_functions(then_body));
                if let Some(else_body) = else_body {
                    called.extend(extract_called_functions(else_body));
                }
            }
            Stmt::For { body, .. } | Stmt::ForTraditional { body, .. } => {
                called.extend(extract_called_functions(body));
            }
            _ => {}
        }
    }
    called
}

/// Helper function to extract mutable API calls to temporary variables
/// Returns (temp_var_decl, temp_var_name) if extraction is needed, otherwise (String::new(), node_id)
pub(crate) fn extract_mutable_api_call(node_id: &str) -> (String, String) {
    // If node_id is already a temp variable (starts with __), don't extract it again
    if node_id.starts_with("__") && !node_id.contains("(") {
        return (String::new(), node_id.to_string());
    }
    
    // Check if node_id is an API call that requires mutable borrow (like api.get_parent)
    if node_id.starts_with("api.get_parent(") || node_id.starts_with("api.get_child_by_name(") {
        // Generate a unique UUID-based temporary variable name (no collisions)
        let full_uuid = uuid::Uuid::new_v4().to_string().replace('-', "");
        let unique_id = full_uuid[..12].to_string(); // First 12 hex chars (48 bits) from UUID without hyphens
        let temp_var = format!("__temp_api_{}", unique_id);
        
        let decl = format!("let {}: Uuid = {};", temp_var, node_id);
        (decl, temp_var)
    } else {
        (String::new(), node_id.to_string())
    }
}

/// Extract node information from a member access expression
/// Returns (node_id_expr, node_type_name, field_path, closure_var_name) if it's a node member access
/// field_path is the full path like "transform.position.x"
pub(crate) fn extract_node_member_info(
    expr: &Expr,
    script: &Script,
    current_func: Option<&Function>,
) -> Option<(String, String, String, String)> {
    fn extract_recursive(
        expr: &Expr,
        script: &Script,
        current_func: Option<&Function>,
        field_path: &mut Vec<String>,
    ) -> Option<(String, String, String, String)> {
        match expr {
            Expr::MemberAccess(base, field) => {
                field_path.push(field.clone());
                extract_recursive(base, script, current_func, field_path)
            }
            Expr::SelfAccess => {
                // self.transform.position.x
                let path: Vec<String> = field_path.iter().rev().cloned().collect();
                Some(("self.id".to_string(), script.node_type.clone(), path.join("."), "self_node".to_string()))
            }
            Expr::Ident(var_name) => {
                // Helper to find variable in nested blocks (for loops, if statements, etc.)
                fn find_variable_in_body<'a>(name: &str, body: &'a [crate::scripting::ast::Stmt]) -> Option<&'a crate::scripting::ast::Variable> {
                    use crate::scripting::ast::Stmt;
                    for stmt in body {
                        match stmt {
                            Stmt::VariableDecl(var) if var.name == name => {
                                return Some(var);
                            }
                            Stmt::If { then_body, else_body, .. } => {
                                if let Some(v) = find_variable_in_body(name, then_body) {
                                    return Some(v);
                                }
                                if let Some(else_body) = else_body {
                                    if let Some(v) = find_variable_in_body(name, else_body) {
                                        return Some(v);
                                    }
                                }
                            }
                            Stmt::For { body: for_body, .. } | Stmt::ForTraditional { body: for_body, .. } => {
                                if let Some(v) = find_variable_in_body(name, for_body) {
                                    return Some(v);
                                }
                            }
                            _ => {}
                        }
                    }
                    None
                }
                
                // Check if it's a node variable
                // First, check if the variable name ends with _id (renamed node variable)
                // If so, look up the original variable name
                let lookup_name = if var_name.ends_with("_id") {
                    &var_name[..var_name.len() - 3]
                } else {
                    var_name
                };
                
                let (var_type_ref, inferred_type_owned) = if let Some(func) = current_func {
                    // Strategy 1: Check in function locals first
                    let var_type_ref = func.locals.iter()
                        .find(|v| v.name == *lookup_name)
                        .and_then(|v| v.typ.as_ref())
                        .or_else(|| {
                            func.params.iter()
                                .find(|p| p.name == *lookup_name)
                                .map(|p| &p.typ)
                        });
                    
                    // Strategy 2: Check in nested blocks (for loops, if statements, etc.)
                    let var_type_ref = var_type_ref.or_else(|| {
                        find_variable_in_body(lookup_name, &func.body)
                            .and_then(|v| v.typ.as_ref())
                    });
                    
                    // Strategy 3: Fall back to script-level variables if not found in function
                    let var_type_ref = var_type_ref.or_else(|| {
                        script.get_variable_type(lookup_name)
                    });
                    
                    // Always try to infer from value expression, even if we have a type
                    // This handles cases where var b = new Sprite2D() creates a node but type might not be set
                    let inferred = func.locals.iter()
                        .find(|v| v.name == *lookup_name)
                        .and_then(|v| v.value.as_ref())
                        .and_then(|val| {
                            // First try infer_expr_type which should return Type::Node for StructNew with node types
                            let inferred = script.infer_expr_type(&val.expr, current_func);
                            if inferred.as_ref().map_or(false, |t| type_is_node(t)) {
                                return inferred;
                            }
                            // Fallback: directly check if it's a StructNew that creates a node
                            if let Expr::StructNew(ty_name, _) = &val.expr {
                                if let Some(node_type) = string_to_node_type(ty_name) {
                                    return Some(Type::Node(node_type));
                                }
                            }
                            inferred
                        })
                        .or_else(|| {
                            // Check in nested blocks for the variable value
                            find_variable_in_body(lookup_name, &func.body)
                                .and_then(|v| v.value.as_ref())
                                .and_then(|val| {
                                    let inferred = script.infer_expr_type(&val.expr, current_func);
                                    if inferred.as_ref().map_or(false, |t| type_is_node(t)) {
                                        return inferred;
                                    }
                                    // Fallback: directly check if it's a StructNew that creates a node
                                    if let Expr::StructNew(ty_name, _) = &val.expr {
                                        if let Some(node_type) = string_to_node_type(ty_name) {
                                            return Some(Type::Node(node_type));
                                        }
                                    }
                                    inferred
                                })
                        })
                        .or_else(|| {
                            // Also check if the value expression is a StructNew that creates a node
                            // (duplicate check for robustness)
                            func.locals.iter()
                                .find(|v| v.name == *lookup_name)
                                .and_then(|v| v.value.as_ref())
                                .and_then(|val| {
                                    if let Expr::StructNew(ty_name, _) = &val.expr {
                                        if let Some(node_type) = string_to_node_type(ty_name) {
                                            Some(Type::Node(node_type))
                                        } else {
                                            None
                                        }
                                    } else {
                                        None
                                    }
                                })
                        })
                        .or_else(|| {
                            // Check in nested blocks (duplicate check for robustness)
                            find_variable_in_body(lookup_name, &func.body)
                                .and_then(|v| v.value.as_ref())
                                .and_then(|val| {
                                    if let Expr::StructNew(ty_name, _) = &val.expr {
                                        if let Some(node_type) = string_to_node_type(ty_name) {
                                            Some(Type::Node(node_type))
                                        } else {
                                            None
                                        }
                                    } else {
                                        None
                                    }
                                })
                        })
                        // Fall back to script-level variable type if not found in function
                        .or_else(|| {
                            script.get_variable_type(lookup_name).cloned()
                        });
                    
                    (var_type_ref, inferred)
                } else {
                    (script.get_variable_type(lookup_name), None)
                };
                
                // Check if it's a node type
                let var_type = var_type_ref.or_else(|| inferred_type_owned.as_ref());
                if let Some(typ) = var_type {
                    if type_is_node(typ) || matches!(typ, Type::DynNode) {
                        // Use the original variable name for renaming (not the _id version)
                        let renamed = rename_variable(lookup_name, Some(typ));
                        let node_type_name = match typ {
                            Type::Node(nt) => format!("{:?}", nt),
                            Type::DynNode => "__DYN_NODE__".to_string(), // Special marker for DynNode
                            _ => return None,
                        };
                        let path: Vec<String> = field_path.iter().rev().cloned().collect();
                        // Use the original variable name as the closure parameter (like mutate_node does)
                        let closure_var = lookup_name.to_string();
                        Some((renamed, node_type_name, path.join("."), closure_var))
                    } else {
                        None
                    }
                } else {
                    None
                }
            }
            Expr::Cast(inner, target_type) => {
                // Handle casts to node types - extract from the inner expression
                match target_type {
                    Type::Node(node_type_enum) => {
                        // Cast to a specific node type - extract node_id from inner expression
                        // The inner might be GetParent which returns None, so we need to handle it specially
                        let (node_id, closure_var) = match inner.as_ref() {
                            Expr::ApiCall(ApiModule::NodeSugar(NodeSugarApi::GetParent), args) => {
                                // Extract the node ID argument
                                let arg_expr = if let Some(Expr::SelfAccess) = args.get(0) {
                                    "self.id".to_string()
                                } else if let Some(Expr::Ident(name)) = args.get(0) {
                                    let is_node_var = if let Some(func) = current_func {
                                        func.locals.iter()
                                            .find(|v| v.name == *name)
                                            .and_then(|v| v.typ.as_ref())
                                            .map(|t| type_becomes_id(t))
                                            .or_else(|| {
                                                func.params.iter()
                                                    .find(|p| p.name == *name)
                                                    .map(|p| type_becomes_id(&p.typ))
                                            })
                                            .unwrap_or(false)
                                    } else {
                                        script.get_variable_type(name)
                                            .map(|t| type_becomes_id(&t))
                                            .unwrap_or(false)
                                    };
                                    
                                    if is_node_var {
                                        format!("{}_id", name)
                                    } else {
                                        name.clone()
                                    }
                                } else {
                                    "self.id".to_string()
                                };
                                (format!("api.get_parent({})", arg_expr), "parent_node".to_string())
                            }
                            _ => {
                                // Try to extract from inner recursively
                                if let Some((node_id, _, _, closure_var)) = extract_recursive(inner, script, current_func, field_path) {
                                    (node_id, closure_var)
                                } else {
                                    return None;
                                }
                            }
                        };
                        let node_type_name = format!("{:?}", node_type_enum);
                        let path: Vec<String> = field_path.iter().rev().cloned().collect();
                        Some((node_id, node_type_name, path.join("."), closure_var))
                    }
                    Type::Custom(type_name) if is_node_type(&type_name) => {
                        // Cast to a node type by name - extract node_id from inner expression
                        let (node_id, closure_var) = match inner.as_ref() {
                            Expr::ApiCall(ApiModule::NodeSugar(NodeSugarApi::GetParent), args) => {
                                // Extract the node ID argument
                                let arg_expr = if let Some(Expr::SelfAccess) = args.get(0) {
                                    "self.id".to_string()
                                } else if let Some(Expr::Ident(name)) = args.get(0) {
                                    let is_node_var = if let Some(func) = current_func {
                                        func.locals.iter()
                                            .find(|v| v.name == *name)
                                            .and_then(|v| v.typ.as_ref())
                                            .map(|t| type_becomes_id(t))
                                            .or_else(|| {
                                                func.params.iter()
                                                    .find(|p| p.name == *name)
                                                    .map(|p| type_becomes_id(&p.typ))
                                            })
                                            .unwrap_or(false)
                                    } else {
                                        script.get_variable_type(name)
                                            .map(|t| type_becomes_id(&t))
                                            .unwrap_or(false)
                                    };
                                    
                                    if is_node_var {
                                        format!("{}_id", name)
                                    } else {
                                        name.clone()
                                    }
                                } else {
                                    "self.id".to_string()
                                };
                                (format!("api.get_parent({})", arg_expr), "parent_node".to_string())
                            }
                            _ => {
                                // Try to extract from inner recursively
                                if let Some((node_id, _, _, closure_var)) = extract_recursive(inner, script, current_func, field_path) {
                                    (node_id, closure_var)
                                } else {
                                    return None;
                                }
                            }
                        };
                        let path: Vec<String> = field_path.iter().rev().cloned().collect();
                        Some((node_id, type_name.clone(), path.join("."), closure_var))
                    }
                    _ => {
                        // Not a node type cast - continue extracting from inner
                        extract_recursive(inner, script, current_func, field_path)
                    }
                }
            }
            Expr::ApiCall(ApiModule::NodeSugar(NodeSugarApi::GetParent), args) => {
                // api.get_parent(node_id) returns Uuid - treat as node ID
                // Generate the full api.get_parent(...) expression as the node_id_expr
                // Extract the node ID argument similar to api_bindings.rs
                let arg_expr = if let Some(Expr::SelfAccess) = args.get(0) {
                    "self.id".to_string()
                } else if let Some(Expr::Ident(name)) = args.get(0) {
                    // Check if it's a type that becomes Uuid/Option<Uuid> (should have _id suffix)
                    let is_node_var = if let Some(func) = current_func {
                        func.locals.iter()
                            .find(|v| v.name == *name)
                            .and_then(|v| v.typ.as_ref())
                            .map(|t| type_becomes_id(t))
                            .or_else(|| {
                                func.params.iter()
                                    .find(|p| p.name == *name)
                                    .map(|p| type_becomes_id(&p.typ))
                            })
                            .unwrap_or(false)
                    } else {
                        script.get_variable_type(name)
                            .map(|t| type_becomes_id(&t))
                            .unwrap_or(false)
                    };
                    
                    if is_node_var {
                        // Node variables are stored as {name}_id
                        format!("{}_id", name)
                    } else {
                        // Uuid variable (like collision_id parameter) - use as-is
                        name.clone()
                    }
                } else {
                    // For complex expressions, fallback to self.id
                    // In practice, get_parent() is usually called with simple identifiers
                    "self.id".to_string()
                };
                
                // Generate the full api.get_parent(...) expression
                let _node_id_expr = format!("api.get_parent({})", arg_expr);
                
                // Cannot determine node type from get_parent() alone - return None to fail transpilation
                // The type must be specified via casting (e.g., get_parent(x) as Sprite2D) or variable type annotation
                None
            }
            _ => None,
        }
    }
    
    let mut field_path = Vec::new();
    if let Some((node_id, node_type, path, closure_var)) = extract_recursive(expr, script, current_func, &mut field_path) {
        // Check if the first field is a script member (for self access)
        if let Expr::MemberAccess(base, field) = expr {
            if matches!(base.as_ref(), Expr::SelfAccess) {
                let is_script_member = script.variables.iter().any(|v| v.name == *field)
                    || script.functions.iter().any(|f| f.name == *field);
                if is_script_member {
                    return None;
                }
            }
        }
        
        Some((node_id, node_type, path, closure_var))
    } else {
        None
    }
}

