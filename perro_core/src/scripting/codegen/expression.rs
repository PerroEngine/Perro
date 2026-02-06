// Expression code generation - Expr and TypedExpr
use super::analysis::{extract_mutable_api_call, extract_node_member_info};
use super::cache::SCRIPT_MEMBERS_CACHE;
use super::utils::{
    get_node_type, is_node_type, is_ui_element_type, rename_function, rename_struct,
    rename_variable, string_to_node_type, type_is_node,
};
use crate::ast::*;
use crate::resource_modules::{ArrayResource, MapResource};
use crate::scripting::ast::{ContainerKind, Expr, Literal, NumberKind, Stmt, Type, TypedExpr};
use crate::structs::engine_registry::ENGINE_REGISTRY;
use crate::structs::script_ui_registry::is_ui_element_ref_type;
use crate::structs::engine_structs::EngineStruct as EngineStructKind;

/// If `key` is a string literal or identifier, return the field name for struct field access.
fn index_field_name(key: &Expr) -> Option<&str> {
    match key {
        Expr::Literal(Literal::String(s)) => Some(s.as_str()),
        Expr::Ident(s) => Some(s.as_str()),
        _ => None,
    }
}

/// Wrap a Value-returning expression (e.g. api.call_function_id(...)) to extract the expected type.
fn value_to_expected_rust(expr: &str, ty: &Type) -> String {
    use crate::scripting::ast::NumberKind;
    match ty {
        Type::Object | Type::Any => expr.to_string(),
        Type::Bool => format!("({}.as_bool().unwrap_or(false))", expr),
        Type::String => format!("({}.as_str().unwrap_or(\"\").to_string())", expr),
        Type::Number(NumberKind::Float(32)) => format!("({}.as_f64().unwrap_or(0.0) as f32)", expr),
        Type::Number(NumberKind::Float(64)) => format!("({}.as_f64().unwrap_or(0.0))", expr),
        Type::Number(NumberKind::Signed(w)) => match w {
            8 => format!("({}.as_i64().unwrap_or(0) as i8)", expr),
            16 => format!("({}.as_i64().unwrap_or(0) as i16)", expr),
            32 => format!("({}.as_i64().unwrap_or(0) as i32)", expr),
            64 | 128 => format!("({}.as_i64().unwrap_or(0))", expr),
            _ => format!("({}.as_i64().unwrap_or(0) as i32)", expr),
        },
        Type::Number(NumberKind::Unsigned(w)) => match w {
            8 => format!("({}.as_u64().unwrap_or(0) as u8)", expr),
            16 => format!("({}.as_u64().unwrap_or(0) as u16)", expr),
            32 => format!("({}.as_u64().unwrap_or(0) as u32)", expr),
            64 | 128 => format!("({}.as_u64().unwrap_or(0))", expr),
            _ => format!("({}.as_u64().unwrap_or(0) as u32)", expr),
        },
        _ => expr.to_string(),
    }
}

/// Closure body for read_scene_node when reading a base Node field (name, id, parent, etc.).
/// Uses BaseNode trait on SceneNode (get_name(), get_id(), etc.) — not read_node with &Node.
fn scene_node_base_field_read(field: &str, result_type: Option<&Type>) -> String {
    use crate::scripting::ast::Type;
    match field {
        "name" => {
            if matches!(result_type, Some(Type::CowStr)) {
                "Cow::Owned(n.get_name().to_string())".into()
            } else {
                "n.get_name().to_string()".into()
            }
        }
        "id" => "n.get_id()".into(),
        "parent" => "n.get_parent().map(|p| p.id).unwrap_or(NodeID::nil())".into(),
        "node_type" => "n.get_type()".into(),
        _ => {
            // Fallback: assume script field name matches BaseNode method (e.g. get_script_path)
            let getter = format!("get_{}", field);
            format!("n.{}()", getter)
        }
    }
}

impl TypedExpr {
    pub fn to_rust(
        &self,
        needs_self: bool,
        script: &Script,
        current_func: Option<&Function>,
    ) -> String {
        let type_hint = self.inferred_type.as_ref();
        let source_span = self.span.as_ref();
        self.expr
            .to_rust(needs_self, script, type_hint, current_func, source_span)
    }

    pub fn contains_self(&self) -> bool {
        match &self.expr {
            Expr::Range(start, end) => start.contains_self() || end.contains_self(),
            _ => self.expr.contains_self(),
        }
    }

    pub fn contains_api_call(&self, script: &Script) -> bool {
        match &self.expr {
            Expr::Range(start, end) => {
                start.contains_api_call(script) || end.contains_api_call(script)
            }
            _ => self.expr.contains_api_call(script),
        }
    }
}

impl Expr {
    fn clone_if_needed(
        expr_code: String,
        expr: &Expr,
        script: &Script,
        current_func: Option<&Function>,
    ) -> String {
        if Expr::should_clone_expr(&expr_code, expr, script, current_func) {
            format!("{}.clone()", expr_code)
        } else {
            expr_code
        }
    }

    fn should_clone_expr(
        expr_code: &str,
        expr: &Expr,
        script: &Script,
        current_func: Option<&Function>,
    ) -> bool {
        // Don't clone cast expressions - they're already Copy types or the cast handles ownership
        if expr_code.starts_with("(") && expr_code.contains(" as ") {
            return false;
        }

        if expr_code.starts_with("json!(")
            || expr_code.starts_with("HashMap::from(")
            || expr_code.starts_with("vec![")
            || expr_code.contains("serde_json::from_value::<")
            || expr_code.contains(".parse::<")
            || expr_code.contains(".unwrap()")  // unwrap() produces owned value
            || expr_code.contains('{')
        // struct literal produces an owned value
        {
            return false;
        }

        match expr {
            Expr::Ident(name) => {
                // For temp variables (__temp_* or temp_api_var_*), we need to check their actual type
                // Since they're not in the script's variable list, infer_expr_type won't find them
                // But we know from context that most read_node results are Copy types (f32, i32, etc.)
                // So we'll try to infer, and if we can't, we'll check if it looks like a Copy type
                if name.starts_with("__temp_") || name.starts_with("temp_api_var_") {
                    // Try to infer type first
                    if let Some(ty) = script.infer_expr_type(expr, current_func) {
                        // We have the type - check if it requires cloning
                        ty.requires_clone()
                    } else {
                        // Can't infer type for temp variable
                        // Most temp variables from read_node are Copy types (f32, i32, Vector2, etc.)
                        // So we assume it doesn't need cloning unless we can prove otherwise
                        // This is safe because Copy types don't need cloning
                        false
                    }
                } else {
                    // Regular variable (including __t_ prefixed variables) - use normal type inference
                    // The __t_ prefix is used for ALL transpiled identifiers, not just loop variables
                    // So we need to check the actual type to determine if cloning is needed
                    if let Some(ty) = script.infer_expr_type(expr, current_func) {
                        ty.requires_clone()
                    } else {
                        // If we can't infer the type, check if it's likely a loop variable
                        // Loop variables from ranges are typically i32 (Copy), but other __t_ variables
                        // might be non-Copy types, so we default to false (no clone) only if we can't determine
                        false
                    }
                }
            }
            Expr::MemberAccess(..) => {
                if let Some(ty) = script.infer_expr_type(expr, current_func) {
                    ty.requires_clone()
                } else {
                    false
                }
            }
            _ => false,
        }
    }

    pub fn to_rust(
        &self,
        needs_self: bool,
        script: &Script,
        expected_type: Option<&Type>,
        current_func: Option<&Function>,
        _source_span: Option<&crate::scripting::source_span::SourceSpan>, // Source location for error reporting
    ) -> String {
        match self {
            Expr::Ident(name) => {
                // Special case: "self" ALWAYS becomes self.id - never rename it
                if name == "self" {
                    return "self.id".to_string();
                }

                // Special case: "api" should NEVER be renamed - it's always the API parameter
                if name == "api" {
                    return "api".to_string();
                }

                // Global (Root, @global TestGlobal): use global registry NodeID (Root=1, first global=2, etc.)
                if let Some(&node_id) = script.global_name_to_node_id.get(name) {
                    return format!("NodeID::from_u32({})", node_id);
                }

                // Helper function to find a variable in nested blocks (if, for, etc.)
                fn find_variable_in_body<'a>(name: &str, body: &'a [Stmt]) -> Option<&'a Variable> {
                    use crate::scripting::ast::Stmt;
                    for stmt in body {
                        match stmt {
                            Stmt::VariableDecl(var) if var.name == name => {
                                return Some(var);
                            }
                            Stmt::If {
                                then_body,
                                else_body,
                                ..
                            } => {
                                if let Some(v) = find_variable_in_body(name, then_body) {
                                    return Some(v);
                                }
                                if let Some(else_body) = else_body {
                                    if let Some(v) = find_variable_in_body(name, else_body) {
                                        return Some(v);
                                    }
                                }
                            }
                            Stmt::For { body: for_body, .. }
                            | Stmt::ForTraditional { body: for_body, .. } => {
                                if let Some(v) = find_variable_in_body(name, for_body) {
                                    return Some(v);
                                }
                            }
                            _ => {}
                        }
                    }
                    None
                }

                let is_local = current_func
                    .map(|f| {
                        f.locals.iter().any(|v| v.name == *name)
                            || f.params.iter().any(|p| p.name == *name)
                            || find_variable_in_body(name, &f.body).is_some()
                            // Also check if this is a renamed variable (e.g., n_id from n)
                            || (name.ends_with("_id") && {
                                let original_name = &name[..name.len() - 3];
                                f.locals.iter().any(|v| v.name == original_name)
                                    || f.params.iter().any(|p| p.name == original_name)
                                    || find_variable_in_body(original_name, &f.body).is_some()
                            })
                    })
                    .unwrap_or(false);

                // Check against `script_vars` to see if it's a field
                let is_field = script.variables.iter().any(|v| v.name == *name);

                // Special case: temp variables (__temp_* or temp_api_var_*) should NEVER be renamed if they're NOT user variables
                // If a user actually named a variable __temp_* or temp_api_var_*, we need to rename it to avoid collisions
                if (name.starts_with("__temp_") || name.starts_with("temp_api_var_"))
                    && !is_local
                    && !is_field
                {
                    return name.to_string();
                }

                // Module scope: when generating module function bodies, module constants use transpiled ident (no self.)
                if let Some(ref scope_vars) = script.module_scope_variables {
                    if let Some(module_var) = scope_vars.iter().find(|v| v.name == *name) {
                        let renamed = rename_variable(name, module_var.typ.as_ref());
                        return renamed;
                    }
                }

                // Get variable type for renaming
                // If var.typ is None, infer from the variable's value expression
                // We need to handle inferred types separately since we can't return a ref to a temp
                let (var_type_ref, inferred_type_owned) = if is_local {
                    let var_type_ref = current_func.and_then(|f| {
                        f.locals
                            .iter()
                            .find(|v| v.name == *name)
                            .and_then(|v| {
                                // First try explicit type
                                v.typ.as_ref()
                            })
                            .or_else(|| f.params.iter().find(|p| p.name == *name).map(|p| &p.typ))
                    });

                    let inferred = if var_type_ref.is_none() {
                        // If no explicit type, infer from value expression
                        current_func.and_then(|f| {
                            f.locals
                                .iter()
                                .find(|v| v.name == *name)
                                .and_then(|v| v.value.as_ref())
                                .and_then(|val| script.infer_expr_type(&val.expr, current_func))
                        })
                    } else {
                        None
                    };

                    (var_type_ref, inferred)
                } else if is_field {
                    let var_type_ref = script.get_variable_type(name);

                    let inferred = if var_type_ref.is_none() {
                        // If no explicit type, infer from value expression
                        script
                            .variables
                            .iter()
                            .find(|v| v.name == *name)
                            .and_then(|v| v.value.as_ref())
                            .and_then(|val| script.infer_expr_type(&val.expr, current_func))
                    } else {
                        None
                    };

                    (var_type_ref, inferred)
                } else {
                    (None, None)
                };

                // Use the EXACT same type determination logic as variable declaration
                // This ensures that when a variable is referenced, it uses the same renamed name
                // as when it was declared (e.g., if declared as tex_id, use tex_id when referenced)

                // First, compute the inferred type and API return type if needed
                // Store them in variables that live long enough
                // IMPORTANT: For API calls, the API return type is the most reliable source
                let (inferred_type_storage, api_return_type_storage): (Option<Type>, Option<Type>) =
                    if is_local {
                        if let Some(func) = current_func {
                            // Try to find variable in top-level locals first
                            let local_opt =
                                func.locals.iter().find(|v| v.name == *name).or_else(|| {
                                    // If not found, search nested blocks
                                    find_variable_in_body(name, &func.body)
                                });

                            if let Some(local) = local_opt {
                                // Get API return type FIRST if value is an API call (most reliable)
                                let api_type = if let Some(val) = &local.value {
                                    if let Expr::ApiCall(api_module, _) = &val.expr {
                                        api_module.return_type()
                                    } else {
                                        None
                                    }
                                } else {
                                    None
                                };

                                // Infer from value expression if no explicit type and not an API call
                                let explicit_type = local.typ.as_ref();
                                let inferred = if explicit_type.is_none() && api_type.is_none() {
                                    local.value.as_ref().and_then(|val| {
                                        script.infer_expr_type(&val.expr, current_func)
                                    })
                                } else {
                                    None
                                };

                                (inferred, api_type)
                            } else {
                                (None, None)
                            }
                        } else {
                            (None, None)
                        }
                    } else if is_field {
                        let explicit_type = script.get_variable_type(name);
                        let inferred = if explicit_type.is_none() {
                            script
                                .variables
                                .iter()
                                .find(|v| v.name == *name)
                                .and_then(|v| v.value.as_ref())
                                .and_then(|val| script.infer_expr_type(&val.expr, current_func))
                        } else {
                            None
                        };
                        (inferred, None)
                    } else {
                        (None, None)
                    };

                // Now determine the type using the same logic as Stmt::VariableDecl
                // IMPORTANT: For variables assigned from API calls, prefer API return type
                // This ensures consistency with how they were declared
                let type_for_renaming = if is_local {
                    // For local variables, use the same logic as Stmt::VariableDecl:
                    // 1. If value is an API call, use API return type (most reliable)
                    // 2. Try explicit type (var.typ)
                    // 3. If not available, infer from value expression
                    if let Some(func) = current_func {
                        // Try to find variable in top-level locals first, then nested blocks
                        let local_opt =
                            func.locals.iter().find(|v| v.name == *name).or_else(|| {
                                // If not found, search nested blocks
                                find_variable_in_body(name, &func.body)
                            });

                        if let Some(local) = local_opt {
                            // Prefer API return type if available (this is what was used during declaration)
                            let explicit_type = local.typ.as_ref();

                            // Use API type first (if available), then explicit type, then inferred type
                            // This ensures we use the same type that was used during declaration
                            api_return_type_storage
                                .as_ref()
                                .or_else(|| explicit_type)
                                .or_else(|| inferred_type_storage.as_ref())
                        } else {
                            // Not found in locals, try params
                            current_func.and_then(|f| {
                                f.params.iter().find(|p| p.name == *name).map(|p| &p.typ)
                            })
                        }
                    } else {
                        var_type_ref
                    }
                } else if is_field {
                    // For script-level variables, use the same logic
                    let explicit_type = script.get_variable_type(name);
                    explicit_type.or_else(|| inferred_type_storage.as_ref())
                } else {
                    var_type_ref.or_else(|| inferred_type_owned.as_ref())
                };

                // Rename variable with t_id_ prefix or _id suffix
                // Use the same type determination logic as declaration to ensure consistency
                let renamed_name = rename_variable(name, type_for_renaming);

                let ident_code = if !is_local && is_field && !name.starts_with("self.") {
                    format!("self.{}", renamed_name)
                } else {
                    renamed_name
                };

                // Script-level or local variable of non-Copy type (String, Container, Custom): add .clone() when read
                // so return/assign don't move out of the variable.
                // Do NOT add .clone() for ID types (Node, DynNode, UIElement, DynUIElement) — they are Copy-like IDs.
                let needs_clone = type_for_renaming.map_or(false, |t| {
                    !matches!(t, Type::Node(_) | Type::DynNode | Type::UIElement(_) | Type::DynUIElement)
                        && (matches!(t, Type::String | Type::CowStr)
                            || matches!(t, Type::Container(_, _))
                            || matches!(t, Type::Custom(_)))
                });
                let ident_code = if needs_clone {
                    format!("{}.clone()", ident_code)
                } else {
                    ident_code
                };

                // ✨ Wrap in json! if going to Value/Object/Any
                if matches!(expected_type, Some(Type::Object | Type::Any)) {
                    format!("json!({})", ident_code)
                } else {
                    ident_code
                }
            }
            Expr::Literal(lit) => {
                // New: check if the expected_type is Type::Object or Type::Any
                if matches!(expected_type, Some(Type::Object | Type::Any)) {
                    format!("json!({})", lit.to_rust(None))
                } else if let Some(expected) = expected_type {
                    // Pass expected type to literal generation
                    lit.to_rust(Some(expected))
                } else {
                    // No expected type, infer it
                    let inferred_type = script.infer_literal_type(lit, None);
                    lit.to_rust(inferred_type.as_ref())
                }
            }
            Expr::BinaryOp(left, op, right) => {
                // For binary operations, infer types with context from the other operand
                // This helps when one operand is a literal and the other is a typed variable (like loop variable)
                // CRITICAL: Infer right first to get its type, then use that to help infer left
                let right_type_first = script.infer_expr_type(right, current_func);

                // Priority 1: Use expected_type from parent context (e.g., Vector2::new expects f32)
                // Priority 2: Use the other operand's type if it's numeric
                // This ensures literals match what the API/struct expects, not just the operand type
                let left_type = if let Expr::Literal(Literal::Number(n)) = left.as_ref() {
                    // First check if we have an expected_type from parent context (most important)
                    if let Some(Type::Number(expected_num_kind)) = expected_type {
                        // eprintln!("[BINARY_OP] Context: expected_type is numeric ({:?}), bypassing cache and inferring literal {} to match", expected_type, n);
                        script.infer_literal_type(
                            &Literal::Number(n.clone()),
                            Some(&Type::Number(expected_num_kind.clone())),
                        )
                    } else if let Some(Type::Number(ref num_kind)) = right_type_first {
                        // Fallback: right operand is numeric - match its type
                        // eprintln!("[BINARY_OP] Context: right is numeric ({:?}), bypassing cache and inferring literal {} to match", right_type_first, n);
                        script.infer_literal_type(
                            &Literal::Number(n.clone()),
                            Some(&Type::Number(num_kind.clone())),
                        )
                    } else {
                        // No numeric context, use normal inference (may hit cache)
                        script.infer_expr_type(left, current_func)
                    }
                } else {
                    script.infer_expr_type(left, current_func)
                };

                // Similarly for right operand
                let right_type = if let Expr::Literal(Literal::Number(n)) = right.as_ref() {
                    // First check if we have an expected_type from parent context (most important)
                    if let Some(Type::Number(expected_num_kind)) = expected_type {
                        // eprintln!("[BINARY_OP] Context: expected_type is numeric ({:?}), bypassing cache and inferring literal {} to match", expected_type, n);
                        script.infer_literal_type(
                            &Literal::Number(n.clone()),
                            Some(&Type::Number(expected_num_kind.clone())),
                        )
                    } else if let Some(Type::Number(ref num_kind)) = left_type {
                        // Fallback: left operand is numeric - match its type
                        // eprintln!("[BINARY_OP] Context: left is numeric ({:?}), bypassing cache and inferring literal {} to match", left_type, n);
                        script.infer_literal_type(
                            &Literal::Number(n.clone()),
                            Some(&Type::Number(num_kind.clone())),
                        )
                    } else {
                        right_type_first
                    }
                } else {
                    right_type_first
                };

                // DEBUG: Track type inference
                // let left_expr_str = format!("{:?}", left);
                // let right_expr_str = format!("{:?}", right);
                // eprintln!("[BINARY_OP] Left: {} -> {:?}", left_expr_str, left_type);
                // eprintln!("[BINARY_OP] Right: {} -> {:?}", right_expr_str, right_type);

                // Loop variables are now always inferred as i32 in infer_expr_type (deterministic)
                // The promotion/casting logic below will handle converting to f32 when used with floats

                // Logical ops (And, Or): result is always bool; force dominant_type so we don't promote to f32.
                let is_logical_op = matches!(op, Op::And); // Or not yet in Op enum
                let dominant_type = if is_logical_op {
                    Some(Type::Bool)
                } else if let Some(expected) = expected_type.cloned() {
                    // When expected_type is Value but op is numeric (e.g. s + delta for set_score arg),
                    // use promoted numeric type for operands so we generate (s_extracted + delta) then wrap in json!().
                    let is_numeric_op_type = matches!(op, Op::Add | Op::Sub | Op::Mul | Op::Div);
                    if matches!(expected, Type::Object | Type::Any) && is_numeric_op_type {
                        // Use promoted type for operands so we don't wrap each operand in json!()
                        match (&left_type, &right_type) {
                            (Some(l), Some(r)) => script
                                .promote_types(l, r)
                                .filter(|t| !matches!(t, Type::Object | Type::Any))
                                .or_else(|| {
                                    if !matches!(l, Type::Object | Type::Any) {
                                        Some(l.clone())
                                    } else if !matches!(r, Type::Object | Type::Any) {
                                        Some(r.clone())
                                    } else {
                                        None
                                    }
                                }),
                            (Some(l), None) if !matches!(l, Type::Object | Type::Any) => {
                                Some(l.clone())
                            }
                            (None, Some(r)) if !matches!(r, Type::Object | Type::Any) => {
                                Some(r.clone())
                            }
                            _ => Some(expected),
                        }
                    } else {
                        Some(expected)
                    }
                } else {
                    let promoted = match (&left_type, &right_type) {
                        (Some(l), Some(r)) => script.promote_types(l, r).or(Some(l.clone())),
                        (Some(l), None) => Some(l.clone()),
                        (None, Some(r)) => Some(r.clone()),
                        _ => None,
                    };
                    promoted
                };

                // Check if left/right are len() calls BEFORE generating code
                let left_is_len = matches!(
                    left.as_ref(),
                    Expr::ApiCall(
                        crate::call_modules::CallModule::Resource(
                            crate::resource_modules::ResourceModule::ArrayOp(ArrayResource::Len)
                        ),
                        _
                    )
                ) || matches!(left.as_ref(), Expr::MemberAccess(_, field) if field == "Length" || field == "length" || field == "len");
                let right_is_len = matches!(
                    right.as_ref(),
                    Expr::ApiCall(
                        crate::call_modules::CallModule::Resource(
                            crate::resource_modules::ResourceModule::ArrayOp(ArrayResource::Len)
                        ),
                        _
                    )
                ) || matches!(right.as_ref(), Expr::MemberAccess(_, field) if field == "Length" || field == "length" || field == "len");

                let left_raw = left.to_rust(
                    needs_self,
                    script,
                    dominant_type.as_ref(),
                    current_func,
                    None,
                );
                let right_raw = right.to_rust(
                    needs_self,
                    script,
                    dominant_type.as_ref(),
                    current_func,
                    None,
                );

                // eprintln!("[BINARY_OP] left_raw: {}", left_raw);
                // eprintln!("[BINARY_OP] right_raw: {}", right_raw);

                // Also check the generated code strings for .len() calls
                let left_is_len = left_is_len || left_raw.ends_with(".len()");
                let right_is_len = right_is_len || right_raw.ends_with(".len()");

                // Strip .clone() from Copy types before applying casts
                // This prevents unnecessary cloning of i32, f32, etc.
                // We'll check the types and strip .clone() for Copy types
                // IMPORTANT: Always strip .clone() from simple identifiers (variables like __t_i, __t_delta)
                // as they are always Copy types (i32, f32, etc.) and cloning is never needed
                let strip_clone_if_copy = |s: &str, ty: Option<&Type>| -> String {
                    if s.ends_with(".clone()") {
                        // If we know the type and it's Copy, strip .clone()
                        if let Some(t) = ty {
                            if t.is_copy_type() {
                                return s[..s.len() - 7].to_string();
                            }
                        }
                        // Always strip .clone() from simple identifiers (variables like __t_i, __t_delta)
                        // These are always Copy types (i32, f32, etc.) and cloning is never needed
                        let base = &s[..s.len() - 7];
                        if !base.contains('(')
                            && !base.contains('.')
                            && !base.contains('[')
                            && !base.contains(' ')
                            && !base.contains('{')
                        {
                            // Simple identifier - always Copy type (i32, f32, etc.) - strip .clone()
                            return base.to_string();
                        }
                    }
                    s.to_string()
                };

                let mut l_str = strip_clone_if_copy(&left_raw, left_type.as_ref());
                let mut r_str = strip_clone_if_copy(&right_raw, right_type.as_ref());

                // If left is len() and right is u32/u64 or a literal that looks like u32, convert right to usize
                if left_is_len {
                    // Check the rendered string first (most reliable)
                    if right_raw.ends_with("u32") || right_raw.ends_with("u") {
                        r_str = format!("({} as usize)", r_str);
                    } else if matches!(right_type, Some(Type::Number(NumberKind::Unsigned(32)))) {
                        r_str = format!("({} as usize)", r_str);
                    } else if matches!(right_type, Some(Type::Number(NumberKind::Unsigned(64)))) {
                        r_str = format!("({} as usize)", r_str);
                    } else if let Expr::Literal(Literal::Number(n)) = right.as_ref() {
                        // Check if it's a u32 literal (ends with u32 or is just a number that should be usize)
                        if n.ends_with("u32") || n.ends_with("u") {
                            r_str = format!("({} as usize)", r_str);
                        }
                    }
                }
                // If right is len() and left is u32/u64 or a literal, convert left to usize
                if right_is_len {
                    // Check the rendered string first (most reliable)
                    if left_raw.ends_with("u32") || left_raw.ends_with("u") {
                        l_str = format!("({} as usize)", l_str);
                    } else if matches!(&left_type, Some(Type::Number(NumberKind::Unsigned(32)))) {
                        l_str = format!("({} as usize)", l_str);
                    } else if matches!(&left_type, Some(Type::Number(NumberKind::Unsigned(64)))) {
                        l_str = format!("({} as usize)", l_str);
                    } else if let Expr::Literal(Literal::Number(n)) = left.as_ref() {
                        if n.ends_with("u32") || n.ends_with("u") {
                            l_str = format!("({} as usize)", l_str);
                        }
                    }
                }

                // Apply normal type conversions
                // IMPORTANT: Special cases must come BEFORE the general case to ensure they match first
                // CRITICAL: Integer-float mixing must ALWAYS cast for determinism - check this FIRST
                // Check for integer-float mixing by examining both types AND generated code strings
                // This ensures we catch cases even if type inference is incomplete
                // RECURSIVE: Also check for loop variable patterns (__t_*) in code strings as they are always i32
                let is_left_int_by_type = matches!(
                    &left_type,
                    Some(Type::Number(
                        NumberKind::Signed(_) | NumberKind::Unsigned(_)
                    ))
                );
                let is_right_int_by_type = matches!(
                    &right_type,
                    Some(Type::Number(
                        NumberKind::Signed(_) | NumberKind::Unsigned(_)
                    ))
                );
                // Check if left/right are loop variables (always i32) by checking code strings
                // Loop variables have pattern __t_* and are always integers
                let is_left_loop_var = left_raw.starts_with("__t_")
                    && !left_raw.contains("f32")
                    && !left_raw.contains("f64");
                let is_right_loop_var = right_raw.starts_with("__t_")
                    && !right_raw.contains("f32")
                    && !right_raw.contains("f64");
                // Also check if it's a simple identifier that looks like an integer (not containing f32/f64)
                let is_left_int_like = is_left_loop_var
                    || (is_left_int_by_type
                        && !left_raw.contains("f32")
                        && !left_raw.contains("f64")
                        && !left_raw.contains(" as f"));
                let is_right_int_like = is_right_loop_var
                    || (is_right_int_by_type
                        && !right_raw.contains("f32")
                        && !right_raw.contains("f64")
                        && !right_raw.contains(" as f"));
                // Use the more aggressive check: type OR pattern-based detection
                let is_left_int = is_left_int_by_type || is_left_loop_var;
                let is_right_int = is_right_int_by_type || is_right_loop_var;
                let is_left_float = matches!(&left_type, Some(Type::Number(NumberKind::Float(_))));
                let is_right_float =
                    matches!(&right_type, Some(Type::Number(NumberKind::Float(_))));

                // Check original expressions for float literals (e.g., "100f32", "5.0f32")
                let right_is_float_literal_expr = matches!(right.as_ref(), Expr::Literal(Literal::Number(n)) if n.contains("f32") || n.contains("f64"));
                let left_is_float_literal_expr = matches!(left.as_ref(), Expr::Literal(Literal::Number(n)) if n.contains("f32") || n.contains("f64"));

                // Also check generated code strings as fallback (in case literal was already processed)
                let right_is_float_literal = right_is_float_literal_expr
                    || right_raw.contains("f32")
                    || right_raw.contains("f64");
                let left_is_float_literal = left_is_float_literal_expr
                    || left_raw.contains("f32")
                    || left_raw.contains("f64");

                // Determine float width from types or literals
                let float_width = if is_right_float {
                    if let Some(Type::Number(NumberKind::Float(w))) = right_type {
                        Some(w)
                    } else {
                        Some(32)
                    }
                } else if is_left_float {
                    if let Some(Type::Number(NumberKind::Float(w))) = left_type {
                        Some(w)
                    } else {
                        Some(32)
                    }
                } else if right_is_float_literal {
                    Some(if right_raw.contains("f64") { 64 } else { 32 })
                } else if left_is_float_literal {
                    Some(if left_raw.contains("f64") { 64 } else { 32 })
                } else {
                    None
                };

                // CRITICAL: Check for string concatenation FIRST and handle it immediately
                // String concatenation uses + operator - we need to convert numbers to strings
                // Check both types and raw code strings (for string literals and format! calls)
                let is_string_concat = matches!(op, Op::Add)
                    && (left_type == Some(Type::String)
                        || right_type == Some(Type::String)
                        || left_raw.starts_with('"')
                        || right_raw.starts_with('"')
                        || left_raw.contains("String::from")
                        || right_raw.contains("String::from")
                        || left_raw.contains("format!")
                        || right_raw.contains("format!"));

                // If this is string concatenation, handle it immediately and return
                // This prevents any numeric type conversions from interfering
                if is_string_concat {
                    // For string concatenation, format! will handle Display for numbers
                    // No need to cast integers to floats - format! handles all Display types
                    return format!("format!(\"{{}}{{}}\", {}, {})", l_str, r_str);
                }

                // Value (serde_json::Value) with Number: extract Value to the known numeric type
                // for numeric ops and comparisons (Value == 100, Value > 3.0) so we don't generate "Value as f32".
                let is_numeric_or_comparison_op = matches!(
                    op,
                    Op::Add
                        | Op::Sub
                        | Op::Mul
                        | Op::Div
                        | Op::Lt
                        | Op::Gt
                        | Op::Le
                        | Op::Ge
                        | Op::Eq
                        | Op::Ne
                );
                let left_is_value = left_type == Some(Type::Any) || left_type == Some(Type::Object);
                let right_is_value =
                    right_type == Some(Type::Any) || right_type == Some(Type::Object);
                let right_is_number = right_type
                    .as_ref()
                    .map_or(false, |t| matches!(t, Type::Number(_)));
                let left_is_number = left_type
                    .as_ref()
                    .map_or(false, |t| matches!(t, Type::Number(_)));

                fn value_to_numeric_extract(expr: &str, ty: &Type) -> String {
                    let expr_clean = if expr.ends_with(".clone()") {
                        &expr[..expr.len() - 7]
                    } else {
                        expr
                    };
                    match ty {
                        Type::Number(NumberKind::Float(32)) => {
                            format!("({}.as_f64().unwrap_or(0.0) as f32)", expr_clean)
                        }
                        Type::Number(NumberKind::Float(64)) => {
                            format!("({}.as_f64().unwrap_or(0.0))", expr_clean)
                        }
                        Type::Number(NumberKind::Signed(w)) => match w {
                            8 => format!("({}.as_i64().unwrap_or(0) as i8)", expr_clean),
                            16 => format!("({}.as_i64().unwrap_or(0) as i16)", expr_clean),
                            32 => format!("({}.as_i64().unwrap_or(0) as i32)", expr_clean),
                            64 | 128 => format!("({}.as_i64().unwrap_or(0))", expr_clean),
                            _ => format!("({}.as_i64().unwrap_or(0) as i32)", expr_clean),
                        },
                        Type::Number(NumberKind::Unsigned(w)) => match w {
                            8 => format!("({}.as_u64().unwrap_or(0) as u8)", expr_clean),
                            16 => format!("({}.as_u64().unwrap_or(0) as u16)", expr_clean),
                            32 => format!("({}.as_u64().unwrap_or(0) as u32)", expr_clean),
                            64 | 128 => format!("({}.as_u64().unwrap_or(0))", expr_clean),
                            _ => format!("({}.as_u64().unwrap_or(0) as u32)", expr_clean),
                        },
                        _ => expr.to_string(),
                    }
                }

                // For comparison/logical ops with expected_type Bool, keep dominant_type = Bool so we never
                // cast the result to f32 (bool as f32 is invalid).
                let is_comparison_or_logical = matches!(
                    op,
                    Op::Eq | Op::Ne | Op::Lt | Op::Gt | Op::Le | Op::Ge | Op::And
                );
                // Logical ops (And, Or): operands are always bool; never cast them to f32/f64 or we get (bool as f32).
                let is_logical_binary = matches!(op, Op::And);
                let (l_str, r_str, dominant_type) = if is_logical_binary {
                    // Use sub-expressions as-is; do not apply any numeric/float casting to operands of &&.
                    (l_str.clone(), r_str.clone(), dominant_type)
                } else if is_numeric_or_comparison_op
                    && ((left_is_value && right_is_number) || (right_is_value && left_is_number))
                {
                    let (new_l, new_r, dom) = if left_is_value && right_is_number {
                        let num_ty = right_type.as_ref().unwrap();
                        let extract = value_to_numeric_extract(&l_str, num_ty);
                        (extract, r_str.clone(), Some(num_ty.clone()))
                    } else {
                        let num_ty = left_type.as_ref().unwrap();
                        let extract = value_to_numeric_extract(&r_str, num_ty);
                        (l_str.clone(), extract, Some(num_ty.clone()))
                    };
                    // Preserve Bool so later "Final cast" and Float(32) operand casting don't apply to comparison result
                    let dom = if is_comparison_or_logical && expected_type == Some(&Type::Bool) {
                        Some(Type::Bool)
                    } else {
                        dom
                    };
                    (new_l, new_r, dom)
                } else {
                    (l_str.clone(), r_str.clone(), dominant_type)
                };

                // CRITICAL: If we detect integer * float mixing, ALWAYS cast the integer to the float type
                // This must happen BEFORE any other type conversion logic
                // Logical ops (And/Or): operands are bool; never cast them.
                let (left_str, right_str) = if is_logical_binary {
                    (l_str.clone(), r_str.clone())
                } else if is_left_int && (is_right_float || right_is_float_literal) {
                    // Integer * Float -> cast integer to float
                    let float_w = float_width.unwrap_or(32);
                    let l_str_clean = if l_str.ends_with(".clone()") {
                        &l_str[..l_str.len() - 7]
                    } else {
                        &l_str
                    };
                    let cast_type = if float_w == 64 { "f64" } else { "f32" };
                    (format!("({} as {})", l_str_clean, cast_type), r_str)
                } else if is_right_int && (is_left_float || left_is_float_literal) {
                    // Float * Integer -> cast integer to float
                    let float_w = float_width.unwrap_or(32);
                    let r_str_clean = if r_str.ends_with(".clone()") {
                        &r_str[..r_str.len() - 7]
                    } else {
                        &r_str
                    };
                    let cast_type = if float_w == 64 { "f64" } else { "f32" };
                    (l_str, format!("({} as {})", r_str_clean, cast_type))
                } else if !is_comparison_or_logical
                    && matches!(dominant_type, Some(Type::Number(NumberKind::Float(32))))
                {
                    // Expected type is f32 - ensure integers are cast to f32 (use aggressive detection)
                    // Skip for comparison/logical so we don't cast (bool) result to f32
                    let l_str_final = if is_left_int || is_left_int_like {
                        let l_str_clean = if l_str.ends_with(".clone()") {
                            &l_str[..l_str.len() - 7]
                        } else {
                            &l_str
                        };
                        format!("({} as f32)", l_str_clean)
                    } else {
                        l_str.clone()
                    };
                    let r_str_final = if is_right_int || is_right_int_like {
                        let r_str_clean = if r_str.ends_with(".clone()") {
                            &r_str[..r_str.len() - 7]
                        } else {
                            &r_str
                        };
                        format!("({} as f32)", r_str_clean)
                    } else {
                        r_str.clone()
                    };
                    (l_str_final, r_str_final)
                } else if !is_comparison_or_logical
                    && matches!(dominant_type, Some(Type::Number(NumberKind::Float(64))))
                {
                    // Expected type is f64 - ensure integers are cast to f64 (use aggressive detection)
                    let l_str_final = if is_left_int || is_left_int_like {
                        let l_str_clean = if l_str.ends_with(".clone()") {
                            &l_str[..l_str.len() - 7]
                        } else {
                            &l_str
                        };
                        format!("({} as f64)", l_str_clean)
                    } else {
                        l_str.clone()
                    };
                    let r_str_final = if is_right_int || is_right_int_like {
                        let r_str_clean = if r_str.ends_with(".clone()") {
                            &r_str[..r_str.len() - 7]
                        } else {
                            &r_str
                        };
                        format!("({} as f64)", r_str_clean)
                    } else {
                        r_str.clone()
                    };
                    (l_str_final, r_str_final)
                } else if (is_left_int || is_left_int_like)
                    && (is_right_float || right_is_float_literal)
                {
                    // Final catch-all: Integer * Float -> cast integer to float (even if dominant_type isn't float)
                    let float_w = float_width.unwrap_or(32);
                    let l_str_clean = if l_str.ends_with(".clone()") {
                        &l_str[..l_str.len() - 7]
                    } else {
                        &l_str
                    };
                    let cast_type = if float_w == 64 { "f64" } else { "f32" };
                    (format!("({} as {})", l_str_clean, cast_type), r_str)
                } else if (is_right_int || is_right_int_like)
                    && (is_left_float || left_is_float_literal)
                {
                    // Final catch-all: Float * Integer -> cast integer to float (even if dominant_type isn't float)
                    let float_w = float_width.unwrap_or(32);
                    let r_str_clean = if r_str.ends_with(".clone()") {
                        &r_str[..r_str.len() - 7]
                    } else {
                        &r_str
                    };
                    let cast_type = if float_w == 64 { "f64" } else { "f32" };
                    (l_str, format!("({} as {})", r_str_clean, cast_type))
                } else {
                    // Normal type conversion logic
                    // Also check patterns as fallback for cases where type inference might have failed
                    match (&left_type, &right_type) {
                        // Special case: if left is float and right is integer (explicit cast for determinism)
                        // Also check patterns: loop variables (__t_*) are always integers
                        (Some(Type::Number(NumberKind::Float(32))), _)
                            if is_right_int || is_right_loop_var =>
                        {
                            // eprintln!("[BINARY_OP] MATCH: Float32 * Integer -> casting right to f32");
                            // Strip .clone() if present before applying cast (Copy types don't need cloning)
                            let r_str_clean = if r_str.ends_with(".clone()") {
                                &r_str[..r_str.len() - 7]
                            } else {
                                &r_str
                            };
                            (l_str, format!("({} as f32)", r_str_clean))
                        }
                        (Some(Type::Number(NumberKind::Float(64))), _)
                            if is_right_int || is_right_loop_var =>
                        {
                            // eprintln!("[BINARY_OP] MATCH: Float64 * Integer -> casting right to f64");
                            // Strip .clone() if present before applying cast (Copy types don't need cloning)
                            let r_str_clean = if r_str.ends_with(".clone()") {
                                &r_str[..r_str.len() - 7]
                            } else {
                                &r_str
                            };
                            (l_str, format!("({} as f64)", r_str_clean))
                        }
                        // Special case: if left is integer and right is float (reverse case) - MOST COMMON CASE
                        // Also check patterns: loop variables (__t_*) are always integers
                        (_, Some(Type::Number(NumberKind::Float(32))))
                            if is_left_int || is_left_loop_var =>
                        {
                            // eprintln!("[BINARY_OP] MATCH: Integer * Float32 -> casting left to f32");
                            // Strip .clone() if present before applying cast (Copy types don't need cloning)
                            let l_str_clean = if l_str.ends_with(".clone()") {
                                &l_str[..l_str.len() - 7]
                            } else {
                                &l_str
                            };
                            (format!("({} as f32)", l_str_clean), r_str)
                        }
                        (_, Some(Type::Number(NumberKind::Float(64))))
                            if is_left_int || is_left_loop_var =>
                        {
                            // eprintln!("[BINARY_OP] MATCH: Integer * Float64 -> casting left to f64");
                            // Strip .clone() if present before applying cast (Copy types don't need cloning)
                            let l_str_clean = if l_str.ends_with(".clone()") {
                                &l_str[..l_str.len() - 7]
                            } else {
                                &l_str
                            };
                            (format!("({} as f64)", l_str_clean), r_str)
                        }
                        // Fallback: check for integer-float mixing by pattern even if types don't match
                        // BUT: Skip for string concatenation
                        _ if !is_string_concat
                            && (is_left_int || is_left_loop_var)
                            && (is_right_float || right_is_float_literal) =>
                        {
                            // Integer * Float -> cast integer to float
                            let float_w = float_width.unwrap_or(32);
                            let l_str_clean = if l_str.ends_with(".clone()") {
                                &l_str[..l_str.len() - 7]
                            } else {
                                &l_str
                            };
                            let cast_type = if float_w == 64 { "f64" } else { "f32" };
                            (format!("({} as {})", l_str_clean, cast_type), r_str)
                        }
                        _ if !is_string_concat
                            && (is_right_int || is_right_loop_var)
                            && (is_left_float || left_is_float_literal) =>
                        {
                            // Float * Integer -> cast integer to float
                            let float_w = float_width.unwrap_or(32);
                            let r_str_clean = if r_str.ends_with(".clone()") {
                                &r_str[..r_str.len() - 7]
                            } else {
                                &r_str
                            };
                            let cast_type = if float_w == 64 { "f64" } else { "f32" };
                            (l_str, format!("({} as {})", r_str_clean, cast_type))
                        }
                        // Fall through to general case if not integer-float mix
                        _ => {
                            // Use the original match with dominant_type for other cases
                            match (&left_type, &right_type, &dominant_type) {
                                // General case: use implicit conversion logic
                                (Some(l), Some(r), Some(dom)) => {
                                    // eprintln!("[BINARY_OP] MATCH: General case - l={:?}, r={:?}, dom={:?}", l, r, dom);
                                    // Special handling: if mixing integer and float, always cast to float for determinism
                                    let l_cast = match (l, r) {
                                        (
                                            Type::Number(
                                                NumberKind::Signed(_) | NumberKind::Unsigned(_),
                                            ),
                                            Type::Number(NumberKind::Float(32)),
                                        ) => {
                                            // Integer * Float32 -> cast integer to f32
                                            let l_str_clean = if l_str.ends_with(".clone()") {
                                                &l_str[..l_str.len() - 7]
                                            } else {
                                                &l_str
                                            };
                                            format!("({} as f32)", l_str_clean)
                                        }
                                        (
                                            Type::Number(
                                                NumberKind::Signed(_) | NumberKind::Unsigned(_),
                                            ),
                                            Type::Number(NumberKind::Float(64)),
                                        ) => {
                                            // Integer * Float64 -> cast integer to f64
                                            let l_str_clean = if l_str.ends_with(".clone()") {
                                                &l_str[..l_str.len() - 7]
                                            } else {
                                                &l_str
                                            };
                                            format!("({} as f64)", l_str_clean)
                                        }
                                        _ => {
                                            // Normal case: use implicit conversion
                                            if l.can_implicitly_convert_to(dom) && l != dom {
                                                // Strip .clone() before casting (casts handle conversion, Copy types don't need cloning)
                                                let l_str_clean = if l_str.ends_with(".clone()") {
                                                    &l_str[..l_str.len() - 7]
                                                } else {
                                                    &l_str
                                                };
                                                script.generate_implicit_cast_for_expr(
                                                    l_str_clean,
                                                    l,
                                                    dom,
                                                )
                                            } else {
                                                l_str
                                            }
                                        }
                                    };
                                    let r_cast = match (l, r) {
                                        (
                                            Type::Number(NumberKind::Float(32)),
                                            Type::Number(
                                                NumberKind::Signed(_) | NumberKind::Unsigned(_),
                                            ),
                                        ) => {
                                            // Float32 * Integer -> cast integer to f32
                                            let r_str_clean = if r_str.ends_with(".clone()") {
                                                &r_str[..r_str.len() - 7]
                                            } else {
                                                &r_str
                                            };
                                            format!("({} as f32)", r_str_clean)
                                        }
                                        (
                                            Type::Number(NumberKind::Float(64)),
                                            Type::Number(
                                                NumberKind::Signed(_) | NumberKind::Unsigned(_),
                                            ),
                                        ) => {
                                            // Float64 * Integer -> cast integer to f64
                                            let r_str_clean = if r_str.ends_with(".clone()") {
                                                &r_str[..r_str.len() - 7]
                                            } else {
                                                &r_str
                                            };
                                            format!("({} as f64)", r_str_clean)
                                        }
                                        _ => {
                                            // Normal case: use implicit conversion
                                            if r.can_implicitly_convert_to(dom) && r != dom {
                                                // Strip .clone() before casting (casts handle conversion, Copy types don't need cloning)
                                                let r_str_clean = if r_str.ends_with(".clone()") {
                                                    &r_str[..r_str.len() - 7]
                                                } else {
                                                    &r_str
                                                };
                                                script.generate_implicit_cast_for_expr(
                                                    r_str_clean,
                                                    r,
                                                    dom,
                                                )
                                            } else {
                                                r_str
                                            }
                                        }
                                    };
                                    (l_cast, r_cast)
                                }
                                // Fallback: if left type is unknown but right is a float, cast left to float
                                (None, Some(Type::Number(NumberKind::Float(32))), _) => {
                                    // Strip .clone() if present before applying cast (Copy types don't need cloning)
                                    let l_str_clean = if l_str.ends_with(".clone()") {
                                        &l_str[..l_str.len() - 7]
                                    } else {
                                        &l_str
                                    };
                                    (format!("({} as f32)", l_str_clean), r_str)
                                }
                                (None, Some(Type::Number(NumberKind::Float(64))), _) => {
                                    // Strip .clone() if present before applying cast (Copy types don't need cloning)
                                    let l_str_clean = if l_str.ends_with(".clone()") {
                                        &l_str[..l_str.len() - 7]
                                    } else {
                                        &l_str
                                    };
                                    (format!("({} as f64)", l_str_clean), r_str)
                                }
                                // Fallback: if right type is unknown but left is a float, cast right to float
                                (Some(Type::Number(NumberKind::Float(32))), None, _) => {
                                    // Strip .clone() if present before applying cast (Copy types don't need cloning)
                                    let r_str_clean = if r_str.ends_with(".clone()") {
                                        &r_str[..r_str.len() - 7]
                                    } else {
                                        &r_str
                                    };
                                    (l_str, format!("({} as f32)", r_str_clean))
                                }
                                (Some(Type::Number(NumberKind::Float(64))), None, _) => {
                                    // Strip .clone() if present before applying cast (Copy types don't need cloning)
                                    let r_str_clean = if r_str.ends_with(".clone()") {
                                        &r_str[..r_str.len() - 7]
                                    } else {
                                        &r_str
                                    };
                                    (l_str, format!("({} as f64)", r_str_clean))
                                }
                                _ => {
                                    // eprintln!("[BINARY_OP] MATCH: Fallback case - no casting");
                                    (l_str, r_str)
                                }
                            }
                        }
                    }
                };

                // eprintln!("[BINARY_OP] After casting: left_str={}, right_str={}", left_str, right_str);

                // Apply cloning if needed for non-Copy types (BigInt, Decimal, String, etc.)
                // BUT: Don't clone if we've already applied a cast (cast expressions are Copy)
                // Also don't clone if the type is Copy
                // IMPORTANT: Never clone simple identifiers (variables like __t_i, __t_delta) as they're always Copy
                let is_simple_identifier = |s: &str| -> bool {
                    !s.contains('(')
                        && !s.contains('.')
                        && !s.contains('[')
                        && !s.contains(' ')
                        && !s.contains('{')
                        && !s.contains(" as ")
                };

                let left_final = if left_str.contains(" as ")
                    || left_type.as_ref().map_or(false, |t| t.is_copy_type())
                    || is_simple_identifier(&left_str)
                {
                    // Already casted, Copy type, or simple identifier - no clone needed
                    left_str
                } else {
                    Expr::clone_if_needed(left_str.clone(), left, script, current_func)
                };
                let right_final = if right_str.contains(" as ")
                    || right_type.as_ref().map_or(false, |t| t.is_copy_type())
                    || is_simple_identifier(&right_str)
                {
                    // Already casted, Copy type, or simple identifier - no clone needed
                    right_str
                } else {
                    Expr::clone_if_needed(right_str.clone(), right, script, current_func)
                };

                // if left_final != left_str {
                //     eprintln!("[BINARY_OP] CLONE ADDED to left: {} -> {}", left_str, left_final);
                // }
                // if right_final != right_str {
                //     eprintln!("[BINARY_OP] CLONE ADDED to right: {} -> {}", right_str, right_final);
                // }

                // eprintln!("[BINARY_OP] FINAL: left_final={}, right_final={}", left_final, right_final);

                // Fallback string concatenation check (should rarely be needed since we handle it early)
                // This catches cases where type inference might have failed earlier
                if matches!(op, Op::Add)
                    && (left_type == Some(Type::String)
                        || right_type == Some(Type::String)
                        || left_final.contains("format!")
                        || right_final.contains("format!")
                        || left_final.starts_with('"')
                        || right_final.starts_with('"'))
                {
                    // format! handles Display for all types including numbers
                    return format!("format!(\"{{}}{{}}\", {}, {})", left_final, right_final);
                }

                // Handle null checks: direct ID types (NodeID, UIElementID) use .is_nil(); Option types use .is_some()/.is_none().
                if matches!(op, Op::Ne | Op::Eq) {
                    let left_is_null = matches!(left.as_ref(), Expr::Literal(Literal::Null))
                        || matches!(left.as_ref(), Expr::Ident(name) if name == "null");
                    let right_is_null = matches!(right.as_ref(), Expr::Literal(Literal::Null))
                        || matches!(right.as_ref(), Expr::Ident(name) if name == "null");

                    // Only bare ID types (NodeID) use .is_nil(). Option types (DynNode Option<NodeID>, DynUIElement/UIElement Option<UIElementID>) use .is_some()/.is_none().
                    let use_nil_check = |ty: &Option<Type>| {
                        ty.as_ref().map_or(false, |t| match t {
                            Type::DynNode => true, // NodeID (not Option in Rust for some paths)
                            Type::DynUIElement | Type::UIElement(_) | Type::Option(_) => false,
                            _ => false,
                        })
                    };

                    if left_is_null && !right_is_null {
                        let ty = script.infer_expr_type(right, current_func);
                        if use_nil_check(&ty) {
                            if matches!(op, Op::Ne) {
                                return format!("!{}.is_nil()", right_final);
                            } else {
                                return format!("{}.is_nil()", right_final);
                            }
                        }
                        // Option type
                        if matches!(op, Op::Ne) {
                            return format!("{}.is_some()", right_final);
                        } else {
                            return format!("{}.is_none()", right_final);
                        }
                    } else if right_is_null && !left_is_null {
                        let ty = script.infer_expr_type(left, current_func);
                        if use_nil_check(&ty) {
                            if matches!(op, Op::Ne) {
                                return format!("!{}.is_nil()", left_final);
                            } else {
                                return format!("{}.is_nil()", left_final);
                            }
                        }
                        if matches!(op, Op::Ne) {
                            return format!("{}.is_some()", left_final);
                        } else {
                            return format!("{}.is_none()", left_final);
                        }
                    }
                }

                // Final cast: only for numeric ops (Add/Sub/Mul/Div). Never cast logical/comparison result to f32.
                let result_expr = format!("({} {} {})", left_final, op.to_rust(), right_final);
                let is_numeric_result_op = matches!(op, Op::Add | Op::Sub | Op::Mul | Op::Div);
                // Comparison and logical ops produce bool; never wrap result in (result as f32).
                if is_comparison_or_logical {
                    return result_expr;
                }
                if is_numeric_result_op
                    && matches!(dominant_type, Some(Type::Number(NumberKind::Float(32))))
                {
                    // Check if both operands are integers
                    let both_integers = matches!(
                        &left_type,
                        Some(Type::Number(
                            NumberKind::Signed(_) | NumberKind::Unsigned(_)
                        ))
                    ) && matches!(
                        &right_type,
                        Some(Type::Number(
                            NumberKind::Signed(_) | NumberKind::Unsigned(_)
                        ))
                    );
                    if both_integers {
                        // Cast the entire result to f32 for determinism
                        return format!("({} as f32)", result_expr);
                    }
                } else if is_numeric_result_op
                    && matches!(dominant_type, Some(Type::Number(NumberKind::Float(64))))
                {
                    // Check if both operands are integers
                    let both_integers = matches!(
                        &left_type,
                        Some(Type::Number(
                            NumberKind::Signed(_) | NumberKind::Unsigned(_)
                        ))
                    ) && matches!(
                        &right_type,
                        Some(Type::Number(
                            NumberKind::Signed(_) | NumberKind::Unsigned(_)
                        ))
                    );
                    if both_integers {
                        // Cast the entire result to f64 for determinism
                        return format!("({} as f64)", result_expr);
                    }
                }

                result_expr
            }
            Expr::MemberAccess(base, field) => {
                // Special case: chained API calls like api.get_parent(...).get_type()
                // Convert to api.get_parent_type(...)
                if let Expr::ApiCall(
                    crate::call_modules::CallModule::NodeMethod(
                        crate::structs::engine_registry::NodeMethodRef::GetParent,
                    ),
                    parent_args,
                ) = base.as_ref()
                {
                    if field == "get_type" {
                        // Extract the node ID argument from get_parent
                        let node_id_expr = if let Some(Expr::SelfAccess) = parent_args.get(0) {
                            "self.id".to_string()
                        } else if let Some(Expr::Ident(name)) = parent_args.get(0) {
                            // Check if it's a type that becomes Uuid/Option<Uuid> (should have _id suffix)
                            let is_node_var = if let Some(func) = current_func {
                                func.locals
                                    .iter()
                                    .find(|v| v.name == *name)
                                    .and_then(|v| v.typ.as_ref())
                                    .map(|t| super::utils::type_becomes_id(t))
                                    .or_else(|| {
                                        func.params
                                            .iter()
                                            .find(|p| p.name == *name)
                                            .map(|p| super::utils::type_becomes_id(&p.typ))
                                    })
                                    .unwrap_or(false)
                            } else {
                                script
                                    .get_variable_type(name)
                                    .map(|t| super::utils::type_becomes_id(&t))
                                    .unwrap_or(false)
                            };

                            if is_node_var {
                                format!("{}_id", name)
                            } else {
                                name.clone()
                            }
                        } else {
                            // For complex expressions, generate the expression string
                            // The statement-level extraction will handle temp variables if needed
                            parent_args[0].to_rust(needs_self, script, None, current_func, None)
                        };

                        return format!("api.get_parent_type({})", node_id_expr);
                    }
                }

                // Special case: accessing .id or .node_type on parent field
                // self.parent.id -> api.read_node(self.id, |n| n.parent.as_ref().map(|p| p.id).unwrap_or(NodeID::nil()))
                // self.parent.node_type -> api.read_node(self.id, |n| n.parent.as_ref().map(|p| p.node_type.clone()).unwrap())
                if let Expr::MemberAccess(parent_base, parent_field) = base.as_ref() {
                    if matches!(parent_base.as_ref(), Expr::SelfAccess) && parent_field == "parent"
                    {
                        if field == "id" {
                            return format!(
                                "api.read_node(self.id, |self_node: &{}| self_node.parent.as_ref().map(|p| p.id).unwrap_or(NodeID::nil()))",
                                script.node_type
                            );
                        } else if field == "node_type" {
                            return format!(
                                "api.read_node(self.id, |self_node: &{}| self_node.parent.as_ref().map(|p| p.node_type.clone()).unwrap())",
                                script.node_type
                            );
                        }
                    }
                }

                // Special case: accessing .id on a node just returns the ID directly
                // self.id -> self.id (already a Uuid on the script)
                // nodeVar.id -> nodeVar_id (node variables are stored as UUIDs)
                if field == "id" {
                    if matches!(base.as_ref(), Expr::SelfAccess) {
                        return "self.id".to_string();
                    } else if let Expr::Ident(var_name) = base.as_ref() {
                        // Check if this is a node variable
                        let var_type = script.infer_expr_type(base, current_func);
                        let is_node = match &var_type {
                            Some(Type::Node(_)) => true,
                            Some(Type::Custom(type_name)) => is_node_type(type_name),
                            _ => false,
                        };

                        if is_node {
                            // Node variable - the variable itself is already the ID
                            return rename_variable(var_name, var_type.as_ref());
                        }
                    }
                }

                // Check if this is a node member access chain (like self.transform.position)
                // If so, wrap the entire chain in api.read_node
                if let Some((node_id, node_type, field_path, closure_var)) =
                    extract_node_member_info(
                        &Expr::MemberAccess(base.clone(), field.clone()),
                        script,
                        current_func,
                    )
                {
                    // Node or DynNode + field on base Node (engine registry) -> read_scene_node only, no match
                    let fields: Vec<&str> = field_path.split('.').collect();
                    if fields.len() == 1 {
                        let first_field = fields[0];
                        if ENGINE_REGISTRY
                            .get_field_type_node(&crate::node_registry::NodeType::Node, first_field)
                            .is_some()
                        {
                            let result_type = script.get_member_type(
                                &Type::Node(crate::node_registry::NodeType::Node),
                                first_field,
                            );
                            let scene_field_access =
                                scene_node_base_field_read(first_field, result_type.as_ref());
                            let (temp_decl, actual_node_id) = extract_mutable_api_call(&node_id);
                            if !temp_decl.is_empty() {
                                return format!(
                                    "{}{}api.read_scene_node({}, |n| {})",
                                    temp_decl,
                                    if temp_decl.ends_with(';') { " " } else { "" },
                                    actual_node_id,
                                    scene_field_access
                                );
                            }
                            return format!(
                                "api.read_scene_node({}, |n| {})",
                                node_id, scene_field_access
                            );
                        }
                    }

                    // This is accessing node fields - use api.read_node (or match for DynNode)
                    if let Some(node_type_enum) = string_to_node_type(&node_type) {
                        let node_type_obj = Type::Node(node_type_enum);

                        // Split the field path to check the final result type
                        let fields: Vec<&str> = field_path.split('.').collect();

                        // Resolve field names in path (e.g., "texture" -> "texture_id")
                        let resolved_fields: Vec<String> = fields
                            .iter()
                            .enumerate()
                            .map(|(i, f)| {
                                // For the first field, resolve against the node type
                                // For subsequent fields, we'd need to resolve against the intermediate type
                                // For now, just resolve the first field against the node type
                                if i == 0 {
                                    ENGINE_REGISTRY.resolve_field_name(&node_type_enum, f)
                                } else {
                                    f.to_string() // TODO: Resolve nested fields properly
                                }
                            })
                            .collect();
                        let resolved_field_path = resolved_fields.join(".");

                        // Walk through the field chain to get the final type (using original field names for type checking)
                        let mut current_type = node_type_obj.clone();
                        for field_name in &fields {
                            if let Some(next_type) =
                                script.get_member_type(&current_type, field_name)
                            {
                                current_type = next_type;
                            }
                        }

                        let needs_clone = current_type.requires_clone();
                        let is_option = matches!(current_type, Type::Option(_));

                        // Only unwrap if the expected type is explicitly NOT an Option
                        // If both the field and expected type are Option, keep it as Option
                        // If expected type is None, keep as Option (don't unwrap by default)
                        let should_unwrap = if is_option {
                            match expected_type {
                                Some(Type::Option(expected_inner)) => {
                                    // Check if the inner types match
                                    match &current_type {
                                        Type::Option(actual_inner) => {
                                            // Only unwrap if inner types don't match
                                            // If they match, keep as Option
                                            actual_inner.as_ref() != expected_inner.as_ref()
                                        }
                                        _ => false, // Keep as Option if we can't determine
                                    }
                                }
                                Some(_) => {
                                    // Expected type is explicitly not an Option, so unwrap
                                    true
                                }
                                None => {
                                    // No expected type hint - keep as Option (don't unwrap by default)
                                    // This is safer and allows the caller to handle Option as needed
                                    false
                                }
                            }
                        } else {
                            false
                        };

                        // Extract mutable API calls to temporary variables to avoid borrow checker issues
                        let (temp_decl, actual_node_id) = extract_mutable_api_call(&node_id);

                        // Helper function to find a variable in nested blocks (if, for, etc.)
                        fn find_variable_in_body<'a>(
                            name: &str,
                            body: &'a [crate::scripting::ast::Stmt],
                        ) -> Option<&'a crate::scripting::ast::Variable> {
                            use crate::scripting::ast::Stmt;
                            for stmt in body {
                                match stmt {
                                    Stmt::VariableDecl(var) if var.name == name => {
                                        return Some(var);
                                    }
                                    Stmt::If {
                                        then_body,
                                        else_body,
                                        ..
                                    } => {
                                        if let Some(v) = find_variable_in_body(name, then_body) {
                                            return Some(v);
                                        }
                                        if let Some(else_body) = else_body {
                                            if let Some(v) = find_variable_in_body(name, else_body)
                                            {
                                                return Some(v);
                                            }
                                        }
                                    }
                                    Stmt::For { body: for_body, .. }
                                    | Stmt::ForTraditional { body: for_body, .. } => {
                                        if let Some(v) = find_variable_in_body(name, for_body) {
                                            return Some(v);
                                        }
                                    }
                                    _ => {}
                                }
                            }
                            None
                        }

                        // Check if node_id is a local variable (check original name if it ends with _id)
                        let is_local_node_id = if node_id.ends_with("_id") {
                            let original_name = &node_id[..node_id.len() - 3];
                            current_func
                                .map(|f| {
                                    f.locals.iter().any(|v| v.name == original_name)
                                        || f.params.iter().any(|p| p.name == original_name)
                                        || find_variable_in_body(original_name, &f.body).is_some()
                                })
                                .unwrap_or(false)
                        } else {
                            false
                        };

                        // Ensure node_id has self. prefix if it's a script variable (not self.id, api.get_parent, or a local variable)
                        let node_id_with_self = if !node_id.starts_with("self.")
                            && !node_id.starts_with("api.")
                            && !is_local_node_id
                        {
                            format!("self.{}", node_id)
                        } else {
                            node_id.clone()
                        };

                        // (Base Node field case already handled at top of extract_node_member_info block.)
                        // Base Node (NodeType::Node): always use read_scene_node, never read_node (globals and var b: Node = ...)
                        if node_type_enum == crate::node_registry::NodeType::Node {
                            let result_type =
                                script.get_member_type(&Type::Node(node_type_enum), fields[0]);
                            let scene_field_access =
                                scene_node_base_field_read(fields[0], result_type.as_ref());
                            if !temp_decl.is_empty() {
                                return format!(
                                    "{}{}api.read_scene_node({}, |n| {})",
                                    temp_decl,
                                    if temp_decl.ends_with(';') { " " } else { "" },
                                    actual_node_id,
                                    scene_field_access
                                );
                            }
                            return format!(
                                "api.read_scene_node({}, |n| {})",
                                node_id_with_self, scene_field_access
                            );
                        }
                        // Ensure closure_var doesn't have "self." prefix (it's a new variable in the closure)
                        let clean_closure_var =
                            closure_var.strip_prefix("self.").unwrap_or(&closure_var);

                        // Centralized read behavior (engine registry).
                        // Example: global_transform is a computed value so reads use the getter.
                        if let Some(first_script_field) = fields.first().copied() {
                            use crate::scripting::lang::pup::node_api::PUP_NODE_API;
                            if let Some(api_field) = PUP_NODE_API
                                .get_fields(&node_type_enum)
                                .iter()
                                .find(|f| f.script_name == first_script_field)
                            {
                                if let Some(read_behavior) =
                                    ENGINE_REGISTRY.get_field_read_behavior(&api_field.rust_field)
                                {
                                    let get_call = match read_behavior {
                                        crate::structs::engine_registry::NodeFieldReadBehavior::GlobalTransform2D => format!(
                                            "api.get_global_transform({}).unwrap_or_default()",
                                            node_id_with_self
                                        ),
                                        crate::structs::engine_registry::NodeFieldReadBehavior::GlobalTransform3D => format!(
                                            "api.get_global_transform_3d({}).unwrap_or_default()",
                                            node_id_with_self
                                        ),
                                    };

                                    // If the access was exactly `global_transform`, return the getter call.
                                    // Otherwise, access the remaining resolved path.
                                    if resolved_fields.len() <= 1 {
                                        return get_call;
                                    }
                                    let rest = resolved_fields[1..].join(".");
                                    return format!("({}).{}", get_call, rest);
                                }
                            }
                        }

                        // Use clean_closure_var for field access (not the original closure_var which might have self.)
                        let field_access = if should_unwrap {
                            format!("{}.{}.unwrap()", clean_closure_var, resolved_field_path)
                        } else if needs_clone {
                            format!("{}.{}.clone()", clean_closure_var, resolved_field_path)
                        } else {
                            format!("{}.{}", clean_closure_var, resolved_field_path)
                        };

                        // Use read_node with the determined node type (Node2D, Sprite2D, etc. — not base Node)
                        // Wrap in parentheses to allow chaining (e.g., (api.read_node(...)).position.y)
                        return format!(
                            "(api.read_node({}, |{}: &{}| {}))",
                            node_id_with_self, clean_closure_var, node_type, field_access
                        );
                    } else if node_type == "__DYN_NODE__" {
                        // DynNode with full path from extract_node_member_info (e.g. c_par.transform.position -> one match, each arm returns full path value)
                        let field_path_only: Vec<String> =
                            field_path.split('.').map(|s| s.to_string()).collect();
                        let (temp_decl, actual_node_id) = extract_mutable_api_call(&node_id);
                        let base_code = if temp_decl.is_empty() {
                            node_id.clone()
                        } else {
                            actual_node_id.clone()
                        };
                        let compatible_node_types =
                            ENGINE_REGISTRY.narrow_nodes_by_fields(&field_path_only);
                        if compatible_node_types.is_empty() {
                            return format!("{}.{}", node_id, field_path.replace('.', "."));
                        }
                        // Check if we need to unify Vector2/Vector3 (match arms return different types)
                        let mut has_vector2 = false;
                        let mut has_vector3 = false;
                        let mut has_f32_rotation = false;
                        let mut has_quaternion = false;
                        for nt in &compatible_node_types {
                            let rt = ENGINE_REGISTRY.resolve_chain_from_node(nt, &field_path_only);
                            if matches!(
                                rt.as_ref(),
                                Some(Type::EngineStruct(EngineStructKind::Vector2))
                            ) {
                                has_vector2 = true;
                            }
                            if matches!(
                                rt.as_ref(),
                                Some(Type::EngineStruct(EngineStructKind::Vector3))
                            ) {
                                has_vector3 = true;
                            }
                            if matches!(rt.as_ref(), Some(Type::Number(NumberKind::Float(32)))) {
                                has_f32_rotation = true;
                            }
                            if matches!(
                                rt.as_ref(),
                                Some(Type::EngineStruct(EngineStructKind::Quaternion))
                            ) {
                                has_quaternion = true;
                            }
                        }
                        let needs_unify_vector = has_vector2 && has_vector3;
                        let needs_unify_rotation = has_f32_rotation && has_quaternion;
                        let mut match_arms = Vec::new();
                        for nt in &compatible_node_types {
                            let node_type_name = format!("{:?}", nt);
                            let result_type =
                                ENGINE_REGISTRY.resolve_chain_from_node(nt, &field_path_only);
                            let needs_clone =
                                result_type.as_ref().map_or(false, |t| t.requires_clone());
                            let is_option = matches!(result_type.as_ref(), Some(Type::Option(_)));
                            let resolved_path: Vec<String> = field_path_only
                                .iter()
                                .map(|f| ENGINE_REGISTRY.resolve_field_name(nt, f))
                                .collect();
                            let field_access_str = resolved_path.join(".");
                            let field_access = if is_option {
                                format!("n.{}.unwrap()", field_access_str)
                            } else if needs_clone {
                                format!("n.{}.clone()", field_access_str)
                            } else {
                                format!("n.{}", field_access_str)
                            };
                            let arm_value = if needs_unify_vector {
                                match result_type.as_ref() {
                                    Some(Type::EngineStruct(EngineStructKind::Vector2)) => {
                                        format!(
                                            "Vector3::new({}.x, {}.y, 0.0)",
                                            field_access, field_access
                                        )
                                    }
                                    _ => field_access,
                                }
                            } else if needs_unify_rotation {
                                match result_type.as_ref() {
                                    Some(Type::Number(NumberKind::Float(32))) => {
                                        format!("Quaternion::from_rotation_2d({})", field_access)
                                    }
                                    _ => field_access,
                                }
                            } else {
                                field_access
                            };
                            match_arms.push(format!(
                                "NodeType::{} => api.read_node({}, |n: &{}| {})",
                                node_type_name, base_code, node_type_name, arm_value
                            ));
                        }
                        let match_expr = format!(
                            "match api.get_type({}) {{\n            {},\n            _ => panic!(\"Node type not compatible with field access: {}\") }}",
                            base_code,
                            match_arms.join(",\n            "),
                            field_path_only.join(".")
                        );
                        if !temp_decl.is_empty() {
                            return format!(
                                "{}{}{}",
                                temp_decl,
                                if temp_decl.ends_with(';') { " " } else { "" },
                                match_expr
                            );
                        }
                        return match_expr;
                    }
                }

                // Special case: if base is SelfAccess and field is a script variable,
                // generate self.field instead of self.node.field
                if matches!(base.as_ref(), Expr::SelfAccess) {
                    // Use cached HashSet for O(1) lookup instead of O(n) iteration
                    let script_ptr = script as *const Script as usize;
                    let is_script_member = SCRIPT_MEMBERS_CACHE.with(|cache| {
                        let mut cache_ref = cache.borrow_mut();

                        // Check if cache is valid for this script
                        let needs_rebuild = match cache_ref.as_ref() {
                            Some((cached_ptr, _)) => *cached_ptr != script_ptr,
                            None => true,
                        };

                        if needs_rebuild {
                            // Build HashSet with all script member names
                            let mut set = std::collections::HashSet::new();
                            for var in &script.variables {
                                set.insert(var.name.clone());
                            }
                            for func in &script.functions {
                                set.insert(func.name.clone());
                            }
                            *cache_ref = Some((script_ptr, set));
                        }

                        // Now we know cache exists and is valid
                        cache_ref.as_ref().unwrap().1.contains(field)
                    });

                    if is_script_member {
                        // This is a script field/method, access directly on self
                        // Need to use the renamed variable/function name
                        // Check if it's a variable or function
                        if let Some(var) = script.variables.iter().find(|v| v.name == *field) {
                            // It's a variable, use renamed variable name
                            let renamed_name = rename_variable(&var.name, var.typ.as_ref());
                            let needs_clone = var.typ.as_ref().map_or(false, |t| {
                                matches!(t, Type::String | Type::CowStr)
                                    || matches!(t, Type::Container(_, _))
                                    || matches!(t, Type::Custom(_))
                            });
                            if needs_clone {
                                return format!("self.{}.clone()", renamed_name);
                            }
                            return format!("self.{}", renamed_name);
                        } else if script.functions.iter().any(|f| f.name == *field) {
                            // It's a function, use renamed function name
                            let renamed_name = rename_function(field);
                            return format!("self.{}", renamed_name);
                        } else {
                            // Fallback (shouldn't happen if is_script_member is true)
                            return format!("self.{}", field);
                        }
                    }
                    // Otherwise, it's a node field, use self.base.field
                }

                // Global access (Root, @global TestGlobal): same as node variable access — use global registry for NodeID (Root=1, first global=2, etc.).
                // Base.to_rust() already returns NodeID::from_u32(id) for Ident(global_name). Route through engine_bindings (GetVar / read_scene_node) like Root::b.
                if let Expr::Ident(name) = base.as_ref() {
                    if script.global_name_to_node_id.contains_key(name) {
                        let is_node_method = ENGINE_REGISTRY
                            .method_ref_map
                            .get(&(crate::node_registry::NodeType::Node, field.clone()))
                            .is_some()
                            || ENGINE_REGISTRY.node_defs.keys().any(|nt| {
                                ENGINE_REGISTRY
                                    .method_ref_map
                                    .get(&(*nt, field.clone()))
                                    .is_some()
                            });
                        if !is_node_method {
                            // Node/DynNode + field on base Node (engine registry) -> read_scene_node
                            if ENGINE_REGISTRY
                                .get_field_type_node(&crate::node_registry::NodeType::Node, field)
                                .is_some()
                            {
                                let result_type = script.get_member_type(
                                    &Type::Node(crate::node_registry::NodeType::Node),
                                    field,
                                );
                                let field_access =
                                    scene_node_base_field_read(field, result_type.as_ref());
                                let node_id_expr =
                                    base.to_rust(needs_self, script, None, current_func, None);
                                return format!(
                                    "api.read_scene_node({}, |n| {})",
                                    node_id_expr, field_access
                                );
                            }
                            // Script variable on global: use GetVar binding (same as Root::b / node.get_var("b"))
                            use crate::api_bindings::generate_rust_args;
                            use crate::structs::engine_bindings::EngineMethodCodegen;
                            use crate::structs::engine_registry::NodeMethodRef;
                            let get_var_args: Vec<Expr> = vec![
                                (**base).clone(),
                                Expr::Literal(Literal::String(field.clone())),
                            ];
                            let expected = NodeMethodRef::GetVar.param_types().map(|p| {
                                std::iter::once(Type::DynNode)
                                    .chain(p.into_iter())
                                    .collect::<Vec<_>>()
                            });
                            let rust_args = generate_rust_args(
                                &get_var_args,
                                script,
                                needs_self,
                                current_func,
                                expected.as_ref(),
                            );
                            return NodeMethodRef::GetVar.to_rust_prepared(
                                &get_var_args,
                                &rust_args,
                                script,
                                needs_self,
                                current_func,
                            );
                        }
                    }
                }

                // Check if this is module access (e.g., Utils.function())
                // Module access: base is an Ident that's not a script member, not a node, not an API module
                if let Expr::Ident(mod_name) = base.as_ref() {
                    // Check if it's not a script variable/function
                    let is_not_script_member = script.variables.iter().all(|v| v.name != *mod_name)
                        && script.functions.iter().all(|f| f.name != *mod_name);

                    // Check if it's not a node type (would be in type_env if it's a variable)
                    let is_not_node = if let Some(f) = current_func {
                        // Check locals first (Variable has typ: Option<Type>)
                        if let Some(v) = f.locals.iter().find(|v| v.name == *mod_name) {
                            v.typ
                                .as_ref()
                                .map_or(true, |t| !matches!(t, Type::Node(_) | Type::DynNode))
                        } else if let Some(p) = f.params.iter().find(|p| p.name == *mod_name) {
                            // Check params (Param has typ: Type, not Option)
                            !matches!(p.typ, Type::Node(_) | Type::DynNode)
                        } else {
                            true
                        }
                    } else {
                        true
                    };

                    // Check if it's not an API module (Time, Console, etc.)
                    use crate::lang::pup::api::PupAPI;
                    let is_not_api_module = PupAPI::resolve(mod_name, field).is_none();

                    // Check if it's actually a known module name
                    let is_known_module = script.module_names.contains(mod_name);

                    // Only treat as module access if it's a known module AND passes all other checks
                    if is_known_module && is_not_script_member && is_not_node && is_not_api_module {
                        // Look up which file identifier contains this module
                        if let Some(identifier) = script.module_name_to_identifier.get(mod_name) {
                            // Use transpiled ident for module constants and functions (same as definition)
                            let field_rust = if let Some(module_vars) =
                                script.module_variables.get(mod_name)
                            {
                                if let Some(var) = module_vars.iter().find(|v| v.name == *field) {
                                    rename_variable(field, var.typ.as_ref())
                                } else if let Some(module_funcs) =
                                    script.module_functions.get(mod_name)
                                {
                                    if module_funcs.iter().any(|f| f.name == *field) {
                                        rename_function(field)
                                    } else {
                                        field.clone()
                                    }
                                } else {
                                    field.clone()
                                }
                            } else if let Some(module_funcs) = script.module_functions.get(mod_name)
                            {
                                if module_funcs.iter().any(|f| f.name == *field) {
                                    rename_function(field)
                                } else {
                                    field.clone()
                                }
                            } else {
                                field.clone()
                            };
                            // Generate crate::identifier::ModuleName::__t_field
                            return format!("crate::{}::{}::{}", identifier, mod_name, field_rust);
                        } else {
                            // Fallback (shouldn't happen if is_known_module is true)
                            return format!("crate::{}::{}", mod_name, field);
                        }
                    }
                }

                let base_type = script.infer_expr_type(base, current_func);

                match base_type {
                    Some(Type::Object | Type::Any) => {
                        // dynamic object (serde_json::Value)
                        let base_code = base.to_rust(needs_self, script, None, current_func, None);
                        format!("{}[\"{}\"].clone()", base_code, field)
                    }
                    Some(Type::Container(ContainerKind::Map, _)) => {
                        let base_code = base.to_rust(needs_self, script, None, current_func, None);
                        format!("{}[\"{}\"].clone()", base_code, field)
                    }
                    Some(Type::Container(ContainerKind::Array, _))
                    | Some(Type::Container(ContainerKind::FixedArray(_), _)) => {
                        // Special case: .Length or .length on arrays should convert to .len()
                        if field == "Length" || field == "length" || field == "len" {
                            let base_code =
                                base.to_rust(needs_self, script, None, current_func, None);
                            format!("{}.len()", base_code)
                        } else {
                            // Vec or FixedArray (support access via integer index, not field name)
                            let base_code =
                                base.to_rust(needs_self, script, None, current_func, None);
                            format!(
                                "/* Cannot perform field access '{}' on array or fixed array */ {}",
                                field, base_code
                            )
                        }
                    }
                    Some(Type::EngineStruct(_engine_struct)) => {
                        // Engine struct: regular .field access
                        // The base should already be generated correctly (either from read_node or direct access)
                        let base_code = base.to_rust(needs_self, script, None, current_func, None);
                        format!("{}.{}", base_code, field)
                    }
                    Some(Type::Custom(type_name)) => {
                        // typed struct: regular .field access
                        let base_code = base.to_rust(needs_self, script, None, current_func, None);

                        // Check if this is a node type and the base is a node ID variable (UUID or Option<Uuid>)
                        if is_node_type(&type_name) {
                            // Check if base_code is a node ID variable (ends with _id or is self.id)
                            // OR if it's an Option<Uuid> variable (from get_child_by_name() or get_node())
                            // Node variables are renamed to {name}_id, and self.id is the script's node ID
                            let is_node_id_var =
                                base_code.ends_with("_id") || base_code == "self.id";

                            // Check if base is an Option<NodeID> variable (from get_parent() or get_node())
                            // Check in current function's locals first, then script-level variables
                            let is_option_id = if let Some(current_func) = current_func {
                                current_func.locals.iter().any(|v| v.name == base_code && matches!(v.typ.as_ref(), Some(Type::Option(inner)) if matches!(inner.as_ref(), Type::DynNode)))
                            } else {
                                script.get_variable_type(&base_code).map_or(false, |t| matches!(t, Type::Option(inner) if matches!(inner.as_ref(), Type::DynNode)))
                            };

                            if is_node_id_var || is_option_id {
                                // Type::Custom is for concrete node type names (Sprite2D, Node2D, etc.), not base Node.
                                // Base Node is Type::Node(NodeType::Node) or Type::DynNode — handled in those arms.
                                if let Some(node_type) = string_to_node_type(type_name.as_str()) {
                                    let node_id_expr = if is_option_id {
                                        format!("{}.unwrap()", base_code)
                                    } else {
                                        base_code.clone()
                                    };
                                    let (temp_decl, actual_node_id) =
                                        extract_mutable_api_call(&node_id_expr);
                                    let temp_prefix = if temp_decl.is_empty() {
                                        ""
                                    } else {
                                        if temp_decl.ends_with(';') { " " } else { "" }
                                    };
                                    let base_node_type = Type::Node(node_type);
                                    let result_type =
                                        script.get_member_type(&base_node_type, field);
                                    let needs_clone =
                                        result_type.as_ref().map_or(false, |t| t.requires_clone());

                                    // Check if the result type is Option<T> - only unwrap if expected type is not Option
                                    let is_option =
                                        matches!(result_type.as_ref(), Some(Type::Option(_)));

                                    // Extract variable name from node_id (e.g., "c_id" -> "c", "par" -> "par")
                                    let param_name = if base_code.ends_with("_id") {
                                        &base_code[..base_code.len() - 3]
                                    } else {
                                        &base_code
                                    };

                                    // Resolve field name (e.g., "texture" -> "texture_id")
                                    let resolved_field =
                                        ENGINE_REGISTRY.resolve_field_name(&node_type, field);

                                    // Only unwrap if the expected type is explicitly NOT an Option
                                    // If both the field and expected type are Option, keep it as Option
                                    // If expected type is None, keep as Option (don't unwrap by default)
                                    let should_unwrap = if is_option {
                                        match expected_type {
                                            Some(Type::Option(expected_inner)) => {
                                                // Check if the inner types match
                                                match result_type.as_ref() {
                                                    Some(Type::Option(actual_inner)) => {
                                                        // Only unwrap if inner types don't match
                                                        // If they match, keep as Option
                                                        actual_inner.as_ref()
                                                            != expected_inner.as_ref()
                                                    }
                                                    _ => false, // Keep as Option if we can't determine
                                                }
                                            }
                                            Some(_) => {
                                                // Expected type is explicitly not an Option, so unwrap
                                                true
                                            }
                                            None => {
                                                // No expected type hint - keep as Option (don't unwrap by default)
                                                // This is safer and allows the caller to handle Option as needed
                                                false
                                            }
                                        }
                                    } else {
                                        false
                                    };

                                    let field_access = if should_unwrap {
                                        format!("{}.{}.unwrap()", param_name, resolved_field)
                                    } else if needs_clone {
                                        format!("{}.{}.clone()", param_name, resolved_field)
                                    } else {
                                        format!("{}.{}", param_name, resolved_field)
                                    };

                                    if !temp_decl.is_empty() {
                                        return format!(
                                            "{}{}api.read_node({}, |{}: &{}| {})",
                                            temp_decl,
                                            temp_prefix,
                                            actual_node_id,
                                            param_name,
                                            type_name,
                                            field_access
                                        );
                                    } else {
                                        return format!(
                                            "api.read_node({}, |{}: &{}| {})",
                                            node_id_expr, param_name, type_name, field_access
                                        );
                                    }
                                }
                            }
                        }

                        // Also check if base_code is a UUID variable that represents a node (ends with _id)
                        // This handles cases where var b = new Sprite2D() creates b_id: Uuid
                        // We need to look up the original variable name to determine the node type
                        if base_code.ends_with("_id") && base_code != "self.id" {
                            // Extract original variable name (e.g., "b_id" -> "b")
                            let original_var_name = &base_code[..base_code.len() - 3];

                            // Helper to find variable in nested blocks (for loops, if statements, etc.)
                            fn find_variable_in_body<'a>(
                                name: &str,
                                body: &'a [crate::scripting::ast::Stmt],
                            ) -> Option<&'a crate::scripting::ast::Variable>
                            {
                                use crate::scripting::ast::Stmt;
                                for stmt in body {
                                    match stmt {
                                        Stmt::VariableDecl(var) if var.name == name => {
                                            return Some(var);
                                        }
                                        Stmt::If {
                                            then_body,
                                            else_body,
                                            ..
                                        } => {
                                            if let Some(v) = find_variable_in_body(name, then_body)
                                            {
                                                return Some(v);
                                            }
                                            if let Some(else_body) = else_body {
                                                if let Some(v) =
                                                    find_variable_in_body(name, else_body)
                                                {
                                                    return Some(v);
                                                }
                                            }
                                        }
                                        Stmt::For { body: for_body, .. }
                                        | Stmt::ForTraditional { body: for_body, .. } => {
                                            if let Some(v) = find_variable_in_body(name, for_body) {
                                                return Some(v);
                                            }
                                        }
                                        _ => {}
                                    }
                                }
                                None
                            }

                            // Look up the variable to see if it's a node type
                            // Try multiple lookup strategies to handle variables in different scopes (including loop-scoped)
                            let node_type_opt = if let Some(current_func) = current_func {
                                // Strategy 1: Check in function locals first
                                current_func
                                    .locals
                                    .iter()
                                    .find(|v| v.name == *original_var_name)
                                    .and_then(|v| {
                                        // Check declared type
                                        if let Some(ref typ) = v.typ {
                                            if type_is_node(typ) {
                                                return get_node_type(typ).cloned();
                                            }
                                        }
                                        // Check inferred type from value expression
                                        v.value.as_ref().and_then(|val| {
                                            let inferred = script
                                                .infer_expr_type(&val.expr, Some(current_func));
                                            if let Some(ref inferred_typ) = inferred {
                                                if type_is_node(inferred_typ) {
                                                    return get_node_type(inferred_typ).cloned();
                                                }
                                            }
                                            // Check if value is StructNew creating a node
                                            if let Expr::StructNew(ty_name, _) = &val.expr {
                                                return string_to_node_type(ty_name);
                                            }
                                            None
                                        })
                                    })
                                    // Strategy 2: Check in nested blocks (for loops, if statements, etc.)
                                    .or_else(|| {
                                        find_variable_in_body(original_var_name, &current_func.body)
                                            .and_then(|v| {
                                                // Check declared type
                                                if let Some(ref typ) = v.typ {
                                                    if type_is_node(typ) {
                                                        return get_node_type(typ).cloned();
                                                    }
                                                }
                                                // Check inferred type from value expression
                                                v.value.as_ref().and_then(|val| {
                                                    let inferred = script.infer_expr_type(
                                                        &val.expr,
                                                        Some(current_func),
                                                    );
                                                    if let Some(ref inferred_typ) = inferred {
                                                        if type_is_node(inferred_typ) {
                                                            return get_node_type(inferred_typ)
                                                                .cloned();
                                                        }
                                                    }
                                                    // Check if value is StructNew creating a node
                                                    if let Expr::StructNew(ty_name, _) = &val.expr {
                                                        return string_to_node_type(ty_name);
                                                    }
                                                    None
                                                })
                                            })
                                    })
                                    // Strategy 3: Check in params
                                    .or_else(|| {
                                        current_func
                                            .params
                                            .iter()
                                            .find(|p| p.name == *original_var_name)
                                            .and_then(|p| {
                                                if type_is_node(&p.typ) {
                                                    get_node_type(&p.typ).cloned()
                                                } else {
                                                    None
                                                }
                                            })
                                    })
                                    // Strategy 4: Try to infer type directly from the base expression
                                    // This works even if variable isn't found in locals or nested blocks
                                    .or_else(|| {
                                        if let Expr::Ident(_) = base.as_ref() {
                                            // Try to infer the type of the identifier directly
                                            if let Some(inferred_type) =
                                                script.infer_expr_type(base, Some(current_func))
                                            {
                                                if type_is_node(&inferred_type) {
                                                    return get_node_type(&inferred_type).cloned();
                                                }
                                            }
                                        }
                                        None
                                    })
                            } else {
                                // Check script-level variables
                                script
                                    .get_variable_type(original_var_name)
                                    .and_then(|typ| {
                                        if type_is_node(&typ) {
                                            get_node_type(&typ).cloned()
                                        } else {
                                            None
                                        }
                                    })
                                    // Fallback: try to infer from the base expression
                                    .or_else(|| {
                                        if let Expr::Ident(_) = base.as_ref() {
                                            if let Some(inferred_type) =
                                                script.infer_expr_type(base, None)
                                            {
                                                if type_is_node(&inferred_type) {
                                                    return get_node_type(&inferred_type).cloned();
                                                }
                                            }
                                        }
                                        None
                                    })
                            };

                            if let Some(node_type) = node_type_opt {
                                // This is a node UUID variable - use api.read_node
                                let node_type_name = format!("{:?}", node_type);
                                let base_node_type = Type::Node(node_type);
                                let result_type = script.get_member_type(&base_node_type, field);
                                let needs_clone =
                                    result_type.as_ref().map_or(false, |t| t.requires_clone());
                                let is_option =
                                    matches!(result_type.as_ref(), Some(Type::Option(_)));

                                let param_name = original_var_name;
                                let resolved_field =
                                    ENGINE_REGISTRY.resolve_field_name(&node_type, field);

                                let should_unwrap = if is_option {
                                    match expected_type {
                                        Some(Type::Option(expected_inner)) => {
                                            match result_type.as_ref() {
                                                Some(Type::Option(actual_inner)) => {
                                                    actual_inner.as_ref() != expected_inner.as_ref()
                                                }
                                                _ => false,
                                            }
                                        }
                                        Some(_) => true,
                                        None => false,
                                    }
                                } else {
                                    false
                                };

                                let field_access = if should_unwrap {
                                    format!("{}.{}.unwrap()", param_name, resolved_field)
                                } else if needs_clone {
                                    format!("{}.{}.clone()", param_name, resolved_field)
                                } else {
                                    format!("{}.{}", param_name, resolved_field)
                                };

                                let (temp_decl, actual_node_id) =
                                    extract_mutable_api_call(&base_code);
                                if !temp_decl.is_empty() {
                                    return format!(
                                        "{}{}api.read_node({}, |{}: &{}| {})",
                                        temp_decl,
                                        if temp_decl.ends_with(';') { " " } else { "" },
                                        actual_node_id,
                                        param_name,
                                        node_type_name,
                                        field_access
                                    );
                                } else {
                                    return format!(
                                        "api.read_node({}, |{}: &{}| {})",
                                        base_code, param_name, node_type_name, field_access
                                    );
                                }
                            }
                        }

                        // Special handling for UINode.get_element - this will be handled in Call expression
                        // when it's cast to a specific type like UIText
                        format!("{}.{}", base_code, field)
                    }
                    Some(Type::Node(node_type)) => {
                        // Node type: check if base is a node ID variable
                        let base_code = base.to_rust(needs_self, script, None, current_func, None);
                        let is_node_id_var = base_code.ends_with("_id") || base_code == "self.id";

                        if is_node_id_var {
                            // Base Node type: use read_scene_node only for base Node fields (name, id, parent, node_type).
                            // Match table is only for DynNode; for Node-typed variables we only support base Node fields here.
                            if node_type == crate::node_registry::NodeType::Node {
                                let is_base_node_field = ENGINE_REGISTRY
                                    .get_field_type_node(
                                        &crate::node_registry::NodeType::Node,
                                        field,
                                    )
                                    .is_some();
                                if is_base_node_field {
                                    let base_node_type = Type::Node(node_type.clone());
                                    let result_type =
                                        script.get_member_type(&base_node_type, field);
                                    let field_access =
                                        scene_node_base_field_read(field, result_type.as_ref());
                                    let (temp_decl, actual_node_id) =
                                        extract_mutable_api_call(&base_code);
                                    if !temp_decl.is_empty() {
                                        return format!(
                                            "{}{}api.read_scene_node({}, |n| {})",
                                            temp_decl,
                                            if temp_decl.ends_with(';') { " " } else { "" },
                                            actual_node_id,
                                            field_access
                                        );
                                    } else {
                                        return format!(
                                            "api.read_scene_node({}, |n| {})",
                                            base_code, field_access
                                        );
                                    }
                                }
                                // Node type does not have this field (e.g. transform, texture). Cast to Node2D/Node3D/etc. or use a DynNode variable.
                                return format!(
                                    "{{ panic!(\"Node does not have field '{}'; cast to a concrete type (e.g. Node2D, Sprite2D) or use a DynNode variable\") }}",
                                    field
                                );
                            }
                            // Concrete node type (Node2D, Sprite2D, etc.): use read_node with that type
                            let node_type_name = format!("{:?}", node_type);
                            let base_node_type = Type::Node(node_type.clone());
                            let result_type = script.get_member_type(&base_node_type, field);
                            let needs_clone =
                                result_type.as_ref().map_or(false, |t| t.requires_clone());
                            let is_option = matches!(result_type.as_ref(), Some(Type::Option(_)));
                            let param_name = if base_code.ends_with("_id") {
                                &base_code[..base_code.len() - 3]
                            } else if base_code == "self.id" {
                                "self_node"
                            } else {
                                "n"
                            };
                            let resolved_field =
                                ENGINE_REGISTRY.resolve_field_name(&node_type, field);
                            let field_access = if is_option {
                                format!("{}.{}.unwrap()", param_name, resolved_field)
                            } else if needs_clone {
                                format!("{}.{}.clone()", param_name, resolved_field)
                            } else {
                                format!("{}.{}", param_name, resolved_field)
                            };
                            let (temp_decl, actual_node_id) = extract_mutable_api_call(&base_code);
                            if !temp_decl.is_empty() {
                                format!(
                                    "{}{}api.read_node({}, |{}: &{}| {})",
                                    temp_decl,
                                    if temp_decl.ends_with(';') { " " } else { "" },
                                    actual_node_id,
                                    param_name,
                                    node_type_name,
                                    field_access
                                )
                            } else {
                                format!(
                                    "api.read_node({}, |{}: &{}| {})",
                                    base_code, param_name, node_type_name, field_access
                                )
                            }
                        } else {
                            format!("{}.{}", base_code, field)
                        }
                    }
                    Some(Type::DynNode) => {
                        // DynNode: when the field is on base Node only, use read_scene_node instead of match on every type.
                        let base_code = base.to_rust(needs_self, script, None, current_func, None);
                        let is_node_id_var = base_code.ends_with("_id") || base_code == "self.id";

                        if is_node_id_var {
                            // Single base Node field (name, id, parent, etc.): use read_scene_node, no match
                            let is_base_node_field = ENGINE_REGISTRY
                                .get_field_type_node(&crate::node_registry::NodeType::Node, field)
                                .is_some();
                            if is_base_node_field {
                                let result_type = script.get_member_type(
                                    &Type::Node(crate::node_registry::NodeType::Node),
                                    field,
                                );
                                let field_access =
                                    scene_node_base_field_read(field, result_type.as_ref());
                                let (temp_decl, actual_node_id) =
                                    extract_mutable_api_call(&base_code);
                                if !temp_decl.is_empty() {
                                    return format!(
                                        "{}{}api.read_scene_node({}, |n| {})",
                                        temp_decl,
                                        if temp_decl.ends_with(';') { " " } else { "" },
                                        actual_node_id,
                                        field_access
                                    );
                                } else {
                                    return format!(
                                        "api.read_scene_node({}, |n| {})",
                                        base_code, field_access
                                    );
                                }
                            }
                            // Build full field path for nested access (e.g. node.transform.position.x)
                            // Use full path so narrow_nodes_by_fields correctly narrows: transform.position -> Node2D/Node3D and descendants; transform.position.z -> Node3D and descendants; texture -> Sprite2D and descendants
                            let mut field_path = vec![field.clone()];
                            let mut current_expr = base.as_ref();
                            while let Expr::MemberAccess(inner_base, inner_field) = current_expr {
                                field_path.push(inner_field.clone());
                                current_expr = inner_base.as_ref();
                            }
                            field_path.reverse();
                            let field_path_only: Vec<String> = field_path.clone();

                            // Find all node types that have this field path — generate match arms
                            let compatible_node_types =
                                ENGINE_REGISTRY.narrow_nodes_by_fields(&field_path_only);

                            if compatible_node_types.is_empty() {
                                // No compatible node types found, fallback to error or default behavior
                                format!("{}.{}", base_code, field)
                            } else {
                                let mut has_vector2 = false;
                                let mut has_vector3 = false;
                                let mut has_f32_rotation = false;
                                let mut has_quaternion = false;
                                for nt in &compatible_node_types {
                                    let rt = ENGINE_REGISTRY
                                        .resolve_chain_from_node(nt, &field_path_only);
                                    if matches!(
                                        rt.as_ref(),
                                        Some(Type::EngineStruct(EngineStructKind::Vector2))
                                    ) {
                                        has_vector2 = true;
                                    }
                                    if matches!(
                                        rt.as_ref(),
                                        Some(Type::EngineStruct(EngineStructKind::Vector3))
                                    ) {
                                        has_vector3 = true;
                                    }
                                    if matches!(
                                        rt.as_ref(),
                                        Some(Type::Number(NumberKind::Float(32)))
                                    ) {
                                        has_f32_rotation = true;
                                    }
                                    if matches!(
                                        rt.as_ref(),
                                        Some(Type::EngineStruct(EngineStructKind::Quaternion))
                                    ) {
                                        has_quaternion = true;
                                    }
                                }
                                let needs_unify_vector = has_vector2 && has_vector3;
                                let needs_unify_rotation = has_f32_rotation && has_quaternion;
                                let mut match_arms = Vec::new();
                                for node_type in &compatible_node_types {
                                    let node_type_name = format!("{:?}", node_type);
                                    let result_type = ENGINE_REGISTRY
                                        .resolve_chain_from_node(node_type, &field_path_only);
                                    let needs_clone =
                                        result_type.as_ref().map_or(false, |t| t.requires_clone());
                                    let is_option =
                                        matches!(result_type.as_ref(), Some(Type::Option(_)));
                                    let param_name = "n";
                                    let resolved_path: Vec<String> = field_path_only
                                        .iter()
                                        .map(|f| ENGINE_REGISTRY.resolve_field_name(node_type, f))
                                        .collect();
                                    let field_access_str = resolved_path.join(".");
                                    let field_access = if is_option {
                                        format!("{}.{}.unwrap()", param_name, field_access_str)
                                    } else if needs_clone {
                                        format!("{}.{}.clone()", param_name, field_access_str)
                                    } else {
                                        format!("{}.{}", param_name, field_access_str)
                                    };
                                    let arm_value = if needs_unify_vector {
                                        match result_type.as_ref() {
                                            Some(Type::EngineStruct(EngineStructKind::Vector2)) => {
                                                format!(
                                                    "Vector3::new({}.x, {}.y, 0.0)",
                                                    field_access, field_access
                                                )
                                            }
                                            _ => field_access,
                                        }
                                    } else if needs_unify_rotation {
                                        match result_type.as_ref() {
                                            Some(Type::Number(NumberKind::Float(32))) => {
                                                format!(
                                                    "Quaternion::from_rotation_2d({})",
                                                    field_access
                                                )
                                            }
                                            _ => field_access,
                                        }
                                    } else {
                                        field_access
                                    };
                                    match_arms.push(format!(
                                        "NodeType::{} => api.read_node({}, |{}: &{}| {})",
                                        node_type_name,
                                        base_code,
                                        param_name,
                                        node_type_name,
                                        arm_value
                                    ));
                                }
                                // One arm per line, comma before _ => so Rust parses correctly
                                format!(
                                    "match api.get_type({}) {{\n            {},\n            _ => panic!(\"Node type not compatible with field access: {}\") }}",
                                    base_code,
                                    match_arms.join(",\n            "),
                                    field_path_only.join(".")
                                )
                            }
                        } else {
                            format!("{}.{}", base_code, field)
                        }
                    }
                    Some(Type::UIElement(et)) => {
                        let base_code = base.to_rust(needs_self, script, None, current_func, None);
                        let ui_node_id = "self.id";
                        let element_id_arg = format!("{}.unwrap_or(UIElementID::nil())", base_code);
                        match crate::structs::ui_bindings::emit_read_typed(
                            ui_node_id, &element_id_arg, et, field,
                        ) {
                            Some(code) => code,
                            None => format!(
                                "{{ {} }}",
                                crate::structs::ui_bindings::panic_unknown_field(et, field)
                            ),
                        }
                    }
                    Some(Type::DynUIElement) => {
                        let base_code = base.to_rust(needs_self, script, None, current_func, None);
                        let ui_node_id = "self.id";
                        let element_id_arg = format!("{}.unwrap_or(UIElementID::nil())", base_code);
                        match crate::structs::ui_bindings::emit_read_dyn(
                            ui_node_id, &element_id_arg, field,
                        ) {
                            Some(code) => code,
                            None => format!(
                                "{{ {} }}",
                                crate::structs::ui_bindings::panic_unknown_dyn_field(field)
                            ),
                        }
                    }
                    _ => {
                        // fallback, assume normal member access
                        let base_code = base.to_rust(needs_self, script, None, current_func, None);
                        format!("{}.{}", base_code, field)
                    }
                }
            }
            Expr::SelfAccess => {
                // self ALWAYS becomes self.id - never store it as a variable
                // This ensures self is never renamed to t_id_self
                "self.id".to_string()
            }
            Expr::BaseAccess => {
                // BaseAccess is deprecated - use self directly
                "self".to_string()
            }
            Expr::EnumAccess(variant) => match variant {
                BuiltInEnumVariant::NodeType(node_type) => {
                    format!("NodeType::{:?}", node_type)
                }
            },
            Expr::Call(target, args) => {
                // GetVar/SetVar/Call are special node methods — handle all language names
                // (get_var/getVar/GetVar, set_var/setVar/SetVar, call/Call) so TS/C# work.
                if let Expr::MemberAccess(base, method) = target.as_ref() {
                    let is_get_var =
                        method == "get_var" || method == "getVar" || method == "GetVar";
                    let is_set_var =
                        method == "set_var" || method == "setVar" || method == "SetVar";
                    let is_call = method == "call" || method == "Call";

                    if is_get_var && args.len() == 1 {
                        use crate::api_bindings::generate_rust_args;
                        use crate::structs::engine_bindings::EngineMethodCodegen;
                        use crate::structs::engine_registry::NodeMethodRef;
                        let get_var_args: Vec<Expr> = vec![(**base).clone(), args[0].clone()];
                        // Receiver (node id) is first arg; param_types() omits it, so prepend DynNode so Option<NodeID> gets unwrapped
                        let expected = NodeMethodRef::GetVar.param_types().map(|p| {
                            std::iter::once(Type::DynNode)
                                .chain(p.into_iter())
                                .collect::<Vec<_>>()
                        });
                        let rust_args = generate_rust_args(
                            &get_var_args,
                            script,
                            needs_self,
                            current_func,
                            expected.as_ref(),
                        );
                        let code = NodeMethodRef::GetVar.to_rust_prepared(
                            &get_var_args,
                            &rust_args,
                            script,
                            needs_self,
                            current_func,
                        );
                        return code;
                    }
                    if is_set_var && args.len() == 2 {
                        use crate::api_bindings::generate_rust_args;
                        use crate::structs::engine_bindings::EngineMethodCodegen;
                        use crate::structs::engine_registry::NodeMethodRef;
                        let set_var_args: Vec<Expr> =
                            vec![(**base).clone(), args[0].clone(), args[1].clone()];
                        let expected = NodeMethodRef::SetVar.param_types().map(|p| {
                            std::iter::once(Type::DynNode)
                                .chain(p.into_iter())
                                .collect::<Vec<_>>()
                        });
                        let rust_args = generate_rust_args(
                            &set_var_args,
                            script,
                            needs_self,
                            current_func,
                            expected.as_ref(),
                        );
                        let code = NodeMethodRef::SetVar.to_rust_prepared(
                            &set_var_args,
                            &rust_args,
                            script,
                            needs_self,
                            current_func,
                        );
                        return code;
                    }
                    if is_call && !args.is_empty() {
                        use crate::api_bindings::generate_rust_args;
                        use crate::structs::engine_bindings::EngineMethodCodegen;
                        use crate::structs::engine_registry::NodeMethodRef;
                        let mut call_args: Vec<Expr> = vec![(**base).clone()];
                        call_args.extend(args.iter().cloned());
                        let expected = NodeMethodRef::CallFunction.param_types().map(|p| {
                            std::iter::once(Type::DynNode)
                                .chain(p.into_iter())
                                .collect::<Vec<_>>()
                        });
                        let rust_args = generate_rust_args(
                            &call_args,
                            script,
                            needs_self,
                            current_func,
                            expected.as_ref(),
                        );
                        let code = NodeMethodRef::CallFunction.to_rust_prepared(
                            &call_args,
                            &rust_args,
                            script,
                            needs_self,
                            current_func,
                        );
                        // call_function_id returns Value; wrap in extraction when expected type is concrete
                        if let Some(ty) = expected_type {
                            let wrapped = value_to_expected_rust(&code, ty);
                            return wrapped;
                        }
                        return code;
                    }
                }

                // Special case: chained API calls like api.get_parent(...).get_type()
                // Convert to api.get_parent_type(...) - this should NOT be treated as a call
                if let Expr::MemberAccess(base, field) = target.as_ref() {
                    if let Expr::ApiCall(
                        crate::call_modules::CallModule::NodeMethod(
                            crate::structs::engine_registry::NodeMethodRef::GetParent,
                        ),
                        parent_args,
                    ) = base.as_ref()
                    {
                        if field == "get_type" && args.is_empty() {
                            // Extract the node ID argument from get_parent
                            let node_id_expr = if let Some(Expr::SelfAccess) = parent_args.get(0) {
                                "self.id".to_string()
                            } else if let Some(Expr::Ident(name)) = parent_args.get(0) {
                                // Check if it's a type that becomes Uuid/Option<Uuid> (should have _id suffix)
                                let is_node_var = if let Some(func) = current_func {
                                    func.locals
                                        .iter()
                                        .find(|v| v.name == *name)
                                        .and_then(|v| v.typ.as_ref())
                                        .map(|t| super::utils::type_becomes_id(t))
                                        .or_else(|| {
                                            func.params
                                                .iter()
                                                .find(|p| p.name == *name)
                                                .map(|p| super::utils::type_becomes_id(&p.typ))
                                        })
                                        .unwrap_or(false)
                                } else {
                                    script
                                        .get_variable_type(name)
                                        .map(|t| super::utils::type_becomes_id(&t))
                                        .unwrap_or(false)
                                };

                                if is_node_var {
                                    format!("{}_id", name)
                                } else {
                                    name.clone()
                                }
                            } else {
                                // For complex expressions, generate the expression string
                                // The statement-level extraction will handle temp variables if needed
                                parent_args[0].to_rust(needs_self, script, None, current_func, None)
                            };

                            // Return directly without adding () - this is not a function call
                            return format!("api.get_parent_type({})", node_id_expr);
                        }
                    }
                }

                // Check for chained calls where an ApiCall returning Uuid is followed by
                // a NodeSugar API method that accepts Uuid as its first parameter
                if let Expr::MemberAccess(base, method) = target.as_ref() {
                    // Try to resolve the method as a node method by looking it up in the engine registry
                    // Try NodeType::Node first since most methods are registered there
                    let method_ref_opt = ENGINE_REGISTRY
                        .method_ref_map
                        .get(&(crate::node_registry::NodeType::Node, method.clone()))
                        .or_else(|| {
                            // Try other node types if not found on Node
                            ENGINE_REGISTRY.node_defs.keys().find_map(|node_type| {
                                ENGINE_REGISTRY
                                    .method_ref_map
                                    .get(&(*node_type, method.clone()))
                            })
                        })
                        .copied();

                    if let Some(method_ref) = method_ref_opt {
                        // One path for all node method calls: base is a node reference (NodeType / DynNode).
                        // Get node_id from base: global (Root, @global) = known NodeID; or ApiCall/MemberAccess that returns NodeID.
                        use crate::api_bindings::generate_rust_args;
                        use crate::structs::engine_bindings::EngineMethodCodegen;
                        // (node_id_expr_or_literal, temp_var if needed). For globals we pass literal in id slot, no temp.
                        let (inner_call_str, temp_var_name) = if let Expr::Ident(name) =
                            base.as_ref()
                        {
                            // Global (Root, @global): same path as c_par_id — only override is id slot = NodeID::from_u32(id), no temp
                            script
                                .global_name_to_node_id
                                .get(name)
                                .copied()
                                .map(|node_id| {
                                    (
                                        format!("NodeID::from_u32({})", node_id),
                                        None as Option<String>,
                                    )
                                })
                                .map(|(s, t)| (Some(s), t))
                                .unwrap_or((None, None))
                        } else if let Expr::ApiCall(api, api_args) = base.as_ref() {
                            if let Some(return_type) = api.return_type() {
                                if matches!(return_type, Type::DynNode) {
                                    let mut inner_call_str =
                                        api.to_rust(api_args, script, needs_self, current_func);
                                    inner_call_str = inner_call_str
                                        .replace("__t_api.", "api.")
                                        .replace("t_id_api.", "api.");
                                    use std::collections::hash_map::DefaultHasher;
                                    use std::hash::{Hash, Hasher};
                                    let mut hasher = DefaultHasher::new();
                                    inner_call_str.hash(&mut hasher);
                                    let temp_var = format!("__temp_api_{}", hasher.finish());
                                    (Some(inner_call_str), Some(temp_var))
                                } else {
                                    (None, None)
                                }
                            } else {
                                (None, None)
                            }
                        } else if let Expr::MemberAccess(inner_base, inner_method) = base.as_ref() {
                            let inner_method_ref_opt = ENGINE_REGISTRY
                                .method_ref_map
                                .get(&(crate::node_registry::NodeType::Node, inner_method.clone()))
                                .or_else(|| {
                                    ENGINE_REGISTRY.node_defs.keys().find_map(|nt| {
                                        ENGINE_REGISTRY
                                            .method_ref_map
                                            .get(&(*nt, inner_method.clone()))
                                    })
                                })
                                .copied();
                            if let Some(inner_method_ref) = inner_method_ref_opt {
                                if inner_method_ref
                                    .return_type()
                                    .map_or(false, |t| matches!(t, Type::DynNode))
                                {
                                    let inner_api_args = vec![*inner_base.clone()];
                                    let rust_args_strings = generate_rust_args(
                                        &inner_api_args,
                                        script,
                                        needs_self,
                                        current_func,
                                        inner_method_ref.param_types().as_ref(),
                                    );
                                    let mut inner_call_str = inner_method_ref.to_rust_prepared(
                                        &inner_api_args,
                                        &rust_args_strings,
                                        script,
                                        needs_self,
                                        current_func,
                                    );
                                    inner_call_str = inner_call_str
                                        .replace("__t_api.", "api.")
                                        .replace("t_id_api.", "api.");
                                    use std::collections::hash_map::DefaultHasher;
                                    use std::hash::{Hash, Hasher};
                                    let mut hasher = DefaultHasher::new();
                                    inner_call_str.hash(&mut hasher);
                                    let temp_var = format!("__temp_api_{}", hasher.finish());
                                    (Some(inner_call_str), Some(temp_var))
                                } else {
                                    (None, None)
                                }
                            } else {
                                (None, None)
                            }
                        } else {
                            (None, None)
                        };
                        if let Some(inner_call_str) = inner_call_str {
                            if let Some(temp_var) = temp_var_name {
                                // Base was ApiCall or MemberAccess returning NodeID — need temp, same as c_par_id from a call
                                let temp_decl =
                                    format!("let {}: NodeID = {};", temp_var, inner_call_str);
                                let temp_var_expr = Expr::Ident(temp_var.clone());
                                let outer_args: Vec<Expr> = std::iter::once(temp_var_expr)
                                    .chain(args.iter().cloned())
                                    .collect();
                                let rust_args_strings = generate_rust_args(
                                    &outer_args,
                                    script,
                                    needs_self,
                                    current_func,
                                    method_ref.param_types().as_ref(),
                                );
                                let call_code = method_ref.to_rust_prepared(
                                    &outer_args,
                                    &rust_args_strings,
                                    script,
                                    needs_self,
                                    current_func,
                                );
                                return format!(
                                    "{}{} {}",
                                    temp_decl,
                                    if temp_decl.ends_with(';') { " " } else { "" },
                                    call_code
                                );
                            }
                            // Global: put literal in id slot only — same path as c_par.get_node(...), no temp
                            let args_rust = generate_rust_args(
                                args,
                                script,
                                needs_self,
                                current_func,
                                method_ref.param_types().as_ref(),
                            );
                            let rust_args_strings: Vec<String> =
                                std::iter::once(inner_call_str).chain(args_rust).collect();
                            let outer_args: Vec<Expr> = std::iter::once(Expr::SelfAccess)
                                .chain(args.iter().cloned())
                                .collect();
                            let call_code = method_ref.to_rust_prepared(
                                &outer_args,
                                &rust_args_strings,
                                script,
                                needs_self,
                                current_func,
                            );
                            return call_code;
                        } else {
                            // Base is Ident (e.g. c_id) but not a global — pass base as first arg with DynNode expected so Option<NodeID> gets unwrapped
                            use crate::api_bindings::generate_rust_args;
                            use crate::structs::engine_bindings::EngineMethodCodegen;
                            let outer_args: Vec<Expr> = std::iter::once((**base).clone())
                                .chain(args.iter().cloned())
                                .collect();
                            let expected: Option<Vec<Type>> = method_ref.param_types().map(|p| {
                                std::iter::once(Type::DynNode)
                                    .chain(p.into_iter())
                                    .collect::<Vec<_>>()
                            });
                            let rust_args_strings = generate_rust_args(
                                &outer_args,
                                script,
                                needs_self,
                                current_func,
                                expected.as_ref(),
                            );
                            return method_ref.to_rust_prepared(
                                &outer_args,
                                &rust_args_strings,
                                script,
                                needs_self,
                                current_func,
                            );
                        }
                    }
                }

                // ==============================================================
                // Extract the target function name, if possible
                // ==============================================================
                let func_name = Self::get_target_name(target);

                // Determine whether this is a local method on the current script
                let is_local_function = func_name
                    .as_ref()
                    .map(|name| script.functions.iter().any(|f| f.name == *name))
                    .unwrap_or(false);

                // ✅ Check if this is a lifecycle method - lifecycle methods cannot be called
                if let Some(name) = &func_name {
                    if let Some(func) = script.functions.iter().find(|f| f.name == *name) {
                        if func.is_lifecycle_method {
                            return format!(
                                "compile_error!(\"Cannot call lifecycle method '{}' - lifecycle methods (defined with 'on {}()') are not callable\");",
                                name, name
                            );
                        }
                    }
                }

                let is_engine_method =
                    matches!(target.as_ref(), Expr::MemberAccess(_base, _method))
                        && !is_local_function;

                // ✅ NEW: Look up the function to get parameter types
                let func_params = if let Some(name) = &func_name {
                    script
                        .functions
                        .iter()
                        .find(|f| f.name == *name)
                        .map(|f| &f.params)
                } else {
                    None
                };

                // ==============================================================
                // Convert each argument expression into Rust source code
                // with proper ownership semantics and type-aware cloning
                // ==============================================================
                let args_rust: Vec<String> = args
                    .iter()
                    .enumerate()
                    .map(|(i, arg)| {
                        // ✅ Get the expected type for this parameter position
                        let expected_type =
                            func_params.and_then(|params| params.get(i)).map(|p| &p.typ);

                        // Generate code for argument with expected type hint
                        let code =
                            arg.to_rust(needs_self, script, expected_type, current_func, None);

                        // Ask the script context to infer the argument type
                        let arg_type = script.infer_expr_type(arg, current_func);

                        match (arg, &arg_type) {
                            // ----------------------------------------------------------
                            // 1️⃣ Literal values — simple by-value semantics
                            // ----------------------------------------------------------
                            (Expr::Literal(Literal::String(_)), _)
                            | (Expr::Literal(Literal::Interpolated(_)), _) => {
                                // String-ish literals already evaluate to a fresh owned/borrowed value
                                // (e.g. `String::from("...")`, `format!(...)`, or `"..."` for &str),
                                // so cloning here is redundant and can double-allocate.
                                code
                            }
                            (Expr::Literal(_), _) => {
                                // Numeric or bool literals — pass directly
                                code
                            }

                            // ----------------------------------------------------------
                            // 2️⃣ Identifiers & member accesses
                            // ----------------------------------------------------------
                            (Expr::Ident(_) | Expr::MemberAccess(..), Some(Type::String))
                            | (Expr::Ident(_) | Expr::MemberAccess(..), Some(Type::Custom(_)))
                            | (Expr::Ident(_) | Expr::MemberAccess(..), Some(Type::Signal)) => {
                                // Owned strings and structs cloned
                                format!("{}.clone()", code)
                            }
                            (Expr::Ident(_) | Expr::MemberAccess(..), _) => {
                                // Primitives & known copies — pass directly
                                code
                            }

                            // ----------------------------------------------------------
                            // 3️⃣ Computed expressions — ops, casts, nested calls, etc.
                            // ----------------------------------------------------------
                            (
                                Expr::BinaryOp(..) | Expr::Call(..) | Expr::Cast(..),
                                Some(Type::String),
                            )
                            | (
                                Expr::BinaryOp(..) | Expr::Call(..) | Expr::Cast(..),
                                Some(Type::Custom(_)),
                            )
                            | (
                                Expr::BinaryOp(..) | Expr::Call(..) | Expr::Cast(..),
                                Some(Type::Signal),
                            ) => {
                                // Complex expressions producing owned objects → clone
                                format!("({}).clone()", code)
                            }
                            (Expr::BinaryOp(..) | Expr::Call(..) | Expr::Cast(..), _) => {
                                // Pure primitives / temporaries
                                format!("({})", code)
                            }

                            // ----------------------------------------------------------
                            // 4️⃣ Fallback / unknown type (inference unresolved)
                            // ----------------------------------------------------------
                            _ => {
                                // Safe fallback — assume Clone is implemented
                                format!("{}.clone()", code)
                            }
                        }
                    })
                    .collect();

                // Check if this is an API module call FIRST, before processing arguments
                // API module calls should NOT have (api) appended - they're already complete
                let (is_api_module_call, api_module_opt) =
                    if let Expr::MemberAccess(base, method) = target.as_ref() {
                        if let Expr::MemberAccess(inner_base, inner_method) = base.as_ref() {
                            // Check if inner_base is "api" and inner_method is an API module name
                            if let Expr::Ident(api_mod) = inner_base.as_ref() {
                                if api_mod == "api" {
                                    // Check if this resolves to an API module
                                    use crate::scripting::lang::pup::api::PupAPI;
                                    if let Some(api) = PupAPI::resolve(inner_method, method) {
                                        (true, Some(api))
                                    } else {
                                        (false, None)
                                    }
                                } else {
                                    (false, None)
                                }
                            } else {
                                (false, None)
                            }
                        } else if let Expr::Ident(mod_name) = base.as_ref() {
                            // Direct module access like Time.get_delta (without api. prefix)
                            use crate::scripting::lang::pup::api::PupAPI;
                            if let Some(api) = PupAPI::resolve(mod_name, method) {
                                (true, Some(api))
                            } else {
                                (false, None)
                            }
                        } else {
                            (false, None)
                        }
                    } else {
                        (false, None)
                    };

                // If this is an API module call, generate it directly without processing arguments again
                if is_api_module_call {
                    if let Some(api_module) = api_module_opt {
                        // Args are already Vec<Expr>, just clone them
                        let api_args: Vec<Expr> = args.iter().cloned().collect();
                        // Generate the API call using the module's codegen
                        return api_module.to_rust(&api_args, script, needs_self, current_func);
                    }
                }

                // Convert the target expression (e.g., func or self.method)
                // This is needed for non-API calls (script functions, engine methods, etc.)
                let mut target_str = target.to_rust(needs_self, script, None, current_func, None);

                // If this is a local user-defined function, prefix with `self.`
                // BUT: don't override if it's already a module call (starts with crate::)
                if is_local_function && !target_str.starts_with("crate::") {
                    // Script-local functions are emitted as methods on the script struct with renamed names.
                    // Always call the renamed version to match the generated impl (e.g. __t_apply_step).
                    let name = func_name.unwrap();
                    let renamed = rename_function(&name);
                    target_str = format!("self.{}", renamed);
                }

                // ==============================================================
                // Finally, build the Rust call string
                // Handles API injection and empty arg lists
                // ==============================================================
                // Note: API module calls are already handled above and returned early

                // Check if this is a module call (starts with crate::)
                // IMPORTANT: Check this BEFORE is_engine_method since module calls
                // are also MemberAccess expressions and would be misclassified
                let is_module_call = target_str.starts_with("crate::");

                if is_module_call {
                    // Module functions: add api as last parameter
                    if args_rust.is_empty() {
                        format!("{}(api)", target_str)
                    } else {
                        format!("{}({}, api)", target_str, args_rust.join(", "))
                    }
                } else if is_engine_method {
                    // ✅ Engine methods: just pass normal args
                    if args_rust.is_empty() {
                        format!("{}()", target_str)
                    } else {
                        format!("{}({})", target_str, args_rust.join(", "))
                    }
                } else if is_local_function {
                    // Local script functions: add api (ONLY for functions in script.functions)
                    if args_rust.is_empty() {
                        format!("{}(api);", target_str)
                    } else {
                        format!("{}({}, api);", target_str, args_rust.join(", "))
                    }
                } else {
                    // Unknown function - don't append api, just call it normally
                    // This handles external functions, engine methods that weren't detected, etc.
                    if args_rust.is_empty() {
                        format!("{}()", target_str)
                    } else {
                        format!("{}({})", target_str, args_rust.join(", "))
                    }
                }
            }
            Expr::ContainerLiteral(_, data) => match data {
                // ===============================================================
                // MAP LITERAL: { "key": value, other_key: expr }
                // ===============================================================
                ContainerLiteralData::Map(pairs) => {
                    let code = if pairs.is_empty() {
                        "HashMap::new()".to_string()
                    } else {
                        // Expected key/value types (from context if known)
                        let (expected_key_type, expected_val_type) = match expected_type {
                            Some(Type::Container(ContainerKind::Map, types))
                                if types.len() == 2 =>
                            {
                                (&types[0], &types[1])
                            }
                            _ => (&Type::String, &Type::Object),
                        };

                        let entries: Vec<_> = pairs
                            .iter()
                            .map(|(k_expr, v_expr)| {
                                let raw_k = k_expr.to_rust(
                                    needs_self,
                                    script,
                                    Some(expected_key_type),
                                    current_func,
                                    None, // k_expr is Expr, no span available
                                );
                                let raw_v = v_expr.to_rust(
                                    needs_self,
                                    script,
                                    Some(expected_val_type),
                                    current_func,
                                    None, // v_expr is Expr, no span available
                                );

                                // For dynamic maps (String keys), convert numeric keys to strings
                                let k_final = if *expected_key_type == Type::String {
                                    let k_type = script.infer_expr_type(k_expr, current_func);
                                    match k_type {
                                        Some(Type::Number(_)) | Some(Type::Bool) => {
                                            format!("{}.to_string()", raw_k)
                                        }
                                        _ => {
                                            if Expr::should_clone_expr(
                                                &raw_k,
                                                k_expr,
                                                script,
                                                current_func,
                                            ) {
                                                format!("{}.clone()", raw_k)
                                            } else {
                                                raw_k
                                            }
                                        }
                                    }
                                } else {
                                    if Expr::should_clone_expr(&raw_k, k_expr, script, current_func)
                                    {
                                        format!("{}.clone()", raw_k)
                                    } else {
                                        raw_k
                                    }
                                };

                                // Wrap value in json!() if this is a dynamic map (Value type) or custom type
                                let v_final =
                                    if matches!(expected_val_type, Type::Object | Type::Any)
                                        || matches!(expected_val_type, Type::Custom(_))
                                    {
                                        // For dynamic maps or custom types, wrap in json!()
                                        if Expr::should_clone_expr(
                                            &raw_v,
                                            v_expr,
                                            script,
                                            current_func,
                                        ) {
                                            format!("json!({}.clone())", raw_v)
                                        } else {
                                            format!("json!({})", raw_v)
                                        }
                                    } else {
                                        // For typed maps, just clone if needed
                                        if Expr::should_clone_expr(
                                            &raw_v,
                                            v_expr,
                                            script,
                                            current_func,
                                        ) {
                                            format!("{}.clone()", raw_v)
                                        } else {
                                            raw_v
                                        }
                                    };

                                format!("({}, {})", k_final, v_final)
                            })
                            .collect();

                        // Determine the correct HashMap type based on expected types
                        let final_code = if matches!(expected_val_type, Type::Object | Type::Any)
                            || matches!(expected_val_type, Type::Custom(_))
                        {
                            // Dynamic map: HashMap<String, Value>
                            format!("HashMap::<String, Value>::from([{}])", entries.join(", "))
                        } else {
                            // Typed map: HashMap<K, V>
                            let key_rust = expected_key_type.to_rust_type();
                            let val_rust = expected_val_type.to_rust_type();
                            format!(
                                "HashMap::<{}, {}>::from([{}])",
                                key_rust,
                                val_rust,
                                entries.join(", ")
                            )
                        };
                        final_code
                    };

                    if matches!(expected_type, Some(Type::Object | Type::Any)) {
                        format!("json!({})", code)
                    } else {
                        code
                    }
                }

                // ===============================================================
                // ARRAY LITERAL: [expr1, expr2, expr3]
                // ===============================================================
                ContainerLiteralData::Array(elems) => {
                    let code = if elems.is_empty() {
                        "Vec::new()".to_string()
                    } else {
                        let elem_ty = match expected_type {
                            Some(Type::Container(ContainerKind::Array, types))
                                if !types.is_empty() =>
                            {
                                &types[0]
                            }
                            _ => &Type::Object,
                        };

                        let elements: Vec<_> = elems
                            .iter()
                            .map(|e| {
                                let rendered = e.to_rust(
                                    needs_self,
                                    script,
                                    Some(elem_ty),
                                    current_func,
                                    None,
                                );

                                // If this is a custom type array or any[]/object[] array, wrap each element in json!()
                                let final_rendered = match elem_ty {
                                    Type::Custom(_) | Type::Object | Type::Any => {
                                        // Custom types and any[]/object[] arrays need to be serialized to Value
                                        format!("json!({})", rendered)
                                    }
                                    _ => {
                                        if Expr::should_clone_expr(
                                            &rendered,
                                            e,
                                            script,
                                            current_func,
                                        ) {
                                            format!("{}.clone()", rendered)
                                        } else {
                                            rendered
                                        }
                                    }
                                };
                                final_rendered
                            })
                            .collect();

                        format!("vec![{}]", elements.join(", "))
                    };

                    if matches!(expected_type, Some(Type::Object | Type::Any)) {
                        format!("json!({})", code)
                    } else {
                        code
                    }
                }

                // ===============================================================
                // FIXED ARRAY LITERAL: [a, b, c] with explicit constant size
                // ===============================================================
                ContainerLiteralData::FixedArray(size, elems) => {
                    // Extract element type from expected_type if it's a Container
                    let elem_ty = match expected_type {
                        Some(Type::Container(ContainerKind::Array, types))
                        | Some(Type::Container(ContainerKind::FixedArray(_), types))
                            if !types.is_empty() =>
                        {
                            &types[0]
                        }
                        _ => &Type::Object,
                    };

                    let mut body: Vec<_> = elems
                        .iter()
                        .map(|e| {
                            // Pass element type to to_rust so literals get correct suffix (e.g., f64 for number[])
                            let rendered =
                                e.to_rust(needs_self, script, Some(elem_ty), current_func, None);

                            // If this is a custom type or any[]/object[] array, wrap in json!()
                            let final_rendered = match elem_ty {
                                Type::Custom(_) | Type::Object => {
                                    format!("json!({})", rendered)
                                }
                                _ => match e {
                                    Expr::Ident(_) | Expr::MemberAccess(..) => {
                                        let ty = script.infer_expr_type(e, current_func);
                                        if ty.as_ref().map_or(false, |t| t.requires_clone()) {
                                            format!("{}.clone()", rendered)
                                        } else {
                                            rendered
                                        }
                                    }
                                    _ => rendered,
                                },
                            };
                            final_rendered
                        })
                        .collect();

                    while body.len() < *size {
                        body.push("Default::default()".into());
                    }
                    if body.len() > *size {
                        body.truncate(*size);
                    }

                    // Check if expected type is Array (Vec<T>) - if so, convert FixedArray to vec![]
                    let should_convert_to_vec =
                        if let Some(Type::Container(ContainerKind::Array, _)) = expected_type {
                            true
                        } else {
                            false
                        };

                    let code = if should_convert_to_vec {
                        // Convert FixedArray to Vec for Array variable types
                        format!("vec![{}]", body.join(", "))
                    } else {
                        // Keep as fixed array [T; N]
                        format!("[{}]", body.join(", "))
                    };

                    if matches!(expected_type, Some(Type::Object | Type::Any)) {
                        format!("json!({})", code)
                    } else {
                        code
                    }
                }
            },
            Expr::StructNew(ty, args) => {
                // Special case: For node types with no arguments, use api.create_node::<Type>()
                // This returns a Uuid, not a node instance
                if args.is_empty() && is_node_type(ty) {
                    return format!("api.create_node::<{}>()", ty);
                }

                // Special case: For engine structs, use their constructor functions
                if let Some(engine_struct) = EngineStructKind::from_string(ty) {
                    // Engine structs like Vector2, Transform2D, etc. use ::new() constructors
                    if args.is_empty() {
                        // No args: use default constructor or ::default()
                        return format!("{}::default()", ty);
                    } else {
                        // With args: use ::new() constructor
                        // Get the expected types for each argument from the engine struct field types
                        // Collect expected types first to avoid temporary value issues
                        let expected_types: Vec<Option<Type>> = args
                            .iter()
                            .map(|(field_name, _)| {
                                if field_name.starts_with('_') {
                                    // Positional argument - get field type by index
                                    let field_index = field_name
                                        .strip_prefix("_")
                                        .and_then(|s| s.parse::<usize>().ok());
                                    if let Some(idx) = field_index {
                                        if let Some(def) =
                                            ENGINE_REGISTRY.struct_defs.get(&engine_struct)
                                        {
                                            def.fields.get(idx).map(|f| f.typ.clone())
                                        } else {
                                            None
                                        }
                                    } else {
                                        None
                                    }
                                } else {
                                    // Named argument - get field type by name
                                    ENGINE_REGISTRY
                                        .get_field_type_struct(&engine_struct, field_name)
                                }
                            })
                            .collect();

                        // Now generate code with expected types
                        let arg_codes: Vec<String> = args
                            .iter()
                            .zip(expected_types.iter())
                            .map(|((_, expr), expected_type_opt)| {
                                // Pass expected type to expression codegen
                                expr.to_rust(
                                    needs_self,
                                    script,
                                    expected_type_opt.as_ref(),
                                    current_func,
                                    None,
                                )
                            })
                            .collect();
                        return format!("{}::new({})", ty, arg_codes.join(", "));
                    }
                }

                // --- Flatten structure hierarchy correctly ---
                fn gather_flat_fields<'a>(
                    s: &'a StructDef,
                    script: &'a Script,
                    out: &mut Vec<(&'a str, &'a Type, Option<&'a str>)>,
                ) {
                    if let Some(ref base) = s.base {
                        if let Some(basedef) = script.structs.iter().find(|b| &b.name == base) {
                            gather_flat_fields_with_parent(
                                basedef,
                                script,
                                out,
                                Some(base.as_str()),
                            );
                        }
                    }

                    // Derived-level fields: no parent
                    for f in &s.fields {
                        out.push((f.name.as_str(), &f.typ, None));
                    }
                }

                fn gather_flat_fields_with_parent<'a>(
                    s: &'a StructDef,
                    script: &'a Script,
                    out: &mut Vec<(&'a str, &'a Type, Option<&'a str>)>,
                    parent_name: Option<&'a str>,
                ) {
                    // Include base of the base, recursively
                    if let Some(ref base) = s.base {
                        if let Some(basedef) = script.structs.iter().find(|b| &b.name == base) {
                            gather_flat_fields_with_parent(
                                basedef,
                                script,
                                out,
                                Some(base.as_str()),
                            );
                        }
                    }

                    // Tag each field in this struct with its owning base
                    for f in &s.fields {
                        out.push((f.name.as_str(), &f.typ, parent_name));
                    }
                }

                // --- Get struct info ---
                let struct_def = script
                    .structs
                    .iter()
                    .find(|s| s.name == *ty)
                    .unwrap_or_else(|| {
                        panic!(
                            "Struct not found: '{}'. Available structs: {:?}",
                            ty,
                            script.structs.iter().map(|s| &s.name).collect::<Vec<_>>()
                        )
                    });

                let mut flat_fields = Vec::new();
                gather_flat_fields(struct_def, script, &mut flat_fields);

                // Map arguments in order to flattened field list
                // ----------------------------------------------------------
                // Map each parsed (field_name, expr) to its real definition
                // ----------------------------------------------------------
                let mut field_exprs: Vec<(&str, &Type, Option<&str>, &Expr)> = Vec::new();

                for (field_name, expr) in args {
                    // look for a matching field by name anywhere in the flattened struct hierarchy
                    if let Some((fname, fty, parent)) = flat_fields
                        .iter()
                        .find(|(fname, _, _)| *fname == field_name.as_str())
                    {
                        // found: record exact type & base
                        field_exprs.push((*fname, *fty, *parent, expr));
                    } else {
                        // unknown field; keep it but use Type::Object as a fallback
                        field_exprs.push((field_name.as_str(), &Type::Object, None, expr));
                    }
                }

                // --- Build flat struct literal: all fields in flattened order ---
                let mut code = String::new();
                for (fname, _fty, _parent) in &flat_fields {
                    if let Some((_fname, fty, _, expr)) =
                        field_exprs.iter().find(|(n, _, _, _)| *n == *fname)
                    {
                        let mut expr_code =
                            expr.to_rust(needs_self, script, Some(fty), current_func, None);
                        let expr_type = script.infer_expr_type(expr, current_func);
                        let should_clone = matches!(expr, Expr::Ident(_) | Expr::MemberAccess(..))
                            && expr_type.as_ref().map_or(false, |ty| ty.requires_clone());
                        if should_clone {
                            expr_code = format!("{}.clone()", expr_code);
                        }
                        code.push_str(&format!("{}: {}, ", fname, expr_code));
                    }
                }

                // Use renamed struct name for custom structs (not node types or engine structs)
                let struct_name = if is_node_type(ty) || EngineStructKind::from_string(ty).is_some()
                {
                    ty.to_string()
                } else {
                    rename_struct(ty)
                };
                // Use struct literal syntax with renamed struct name
                format!("{} {{ {}..Default::default() }}", struct_name, code)
            }
            Expr::ApiCall(module, args) => {
                // Get expected param types (if defined for this API)
                let expected_param_types = module.param_types();

                // Check if first argument is an ApiCall that returns Uuid and this API takes Uuid as first param
                // If so, extract the inner call to a temp variable to avoid borrow checker errors
                let mut temp_decl_opt: Option<String> = None;
                let mut temp_var_opt: Option<String> = None;

                if let Some(param_types) = &expected_param_types {
                    if let Some(first_param_type) = param_types.get(0) {
                        if matches!(first_param_type, Type::DynNode) {
                            if let Some(first_arg) = args.get(0) {
                                // Check if first_arg is a Cast containing an ApiCall, or a direct ApiCall
                                let inner_api_call = if let Expr::Cast(inner_expr, _) = first_arg {
                                    if let Expr::ApiCall(inner_api, inner_args) =
                                        inner_expr.as_ref()
                                    {
                                        Some((inner_api, inner_args))
                                    } else {
                                        None
                                    }
                                } else if let Expr::ApiCall(inner_api, inner_args) = first_arg {
                                    Some((inner_api, inner_args))
                                } else {
                                    None
                                };

                                if let Some((inner_api, inner_args)) = inner_api_call {
                                    if let Some(return_type) = inner_api.return_type() {
                                        if matches!(return_type, Type::DynNode)
                                            || matches!(return_type, Type::Option(boxed) if matches!(boxed.as_ref(), Type::DynNode))
                                        {
                                            // Both APIs require mutable borrows - extract inner call to temp variable
                                            let mut inner_call_str = inner_api.to_rust(
                                                inner_args,
                                                script,
                                                needs_self,
                                                current_func,
                                            );

                                            // Fix any incorrect renaming of "api" to "__t_api" or "t_id_api"
                                            // The "api" identifier should NEVER be renamed - it's always the API parameter
                                            inner_call_str = inner_call_str
                                                .replace("__t_api.", "api.")
                                                .replace("t_id_api.", "api.");

                                            // Generate deterministic temp variable name using hash of the API call
                                            use std::collections::hash_map::DefaultHasher;
                                            use std::hash::{Hash, Hasher};
                                            let mut hasher = DefaultHasher::new();
                                            inner_call_str.hash(&mut hasher);
                                            let hash = hasher.finish();
                                            let temp_var = format!("__temp_api_{}", hash);

                                            temp_decl_opt = Some(format!(
                                                "let {}: NodeID = {};",
                                                temp_var, inner_call_str
                                            ));
                                            temp_var_opt = Some(temp_var);
                                        }
                                    }
                                }
                            }
                        }
                    }
                }

                // Generate argument code with expected type hints applied **now**
                let mut arg_strs: Vec<String> = args
                    .iter()
                    .enumerate()
                    .map(|(i, arg)| {
                        // If this is the first arg and we extracted it to a temp variable, use the temp var
                        if i == 0 && temp_var_opt.is_some() {
                            temp_var_opt.as_ref().unwrap().clone()
                        } else {
                            // Determine expected type for this argument
                            let expected_ty_hint =
                                expected_param_types.as_ref().and_then(|v| v.get(i));

                            // Ask expression to render itself, with the hint
                            // Note: We don't have source span for individual args, pass None
                            // Get source span from arg if it's a TypedExpr (we don't have that context here)
                            arg.to_rust(needs_self, script, expected_ty_hint, current_func, None)
                        }
                    })
                    .collect();

                // Re‑enforce if API declares argument types and conversion is still needed
                if let Some(expected) = &expected_param_types {
                    for (i, expected_ty) in expected.iter().enumerate() {
                        if let Some(arg_expr) = args.get(i) {
                            // 1. Infer arg type (contextually refined now)
                            let actual_ty = script.infer_expr_type(arg_expr, current_func);

                            // 2. If convertible and different ⇒ implicit cast
                            if let Some(actual_ty) = &actual_ty {
                                if actual_ty.can_implicitly_convert_to(expected_ty)
                                    && actual_ty != expected_ty
                                {
                                    arg_strs[i] = script.generate_implicit_cast_for_expr(
                                        &arg_strs[i],
                                        actual_ty,
                                        expected_ty,
                                    );
                                }
                            }
                        }
                    }
                }

                // Generate the API call code
                // If we extracted the first arg to a temp variable, create a new args list with the temp var
                let api_call_args = if temp_var_opt.is_some() {
                    // Replace first arg with Ident expression for temp variable
                    let mut new_args = args.clone();
                    if let Some(temp_var) = &temp_var_opt {
                        new_args[0] = Expr::Ident(temp_var.clone());
                    }
                    new_args
                } else {
                    args.clone()
                };
                // Generate API call
                let api_call_code =
                    module.to_rust(&api_call_args, script, needs_self, current_func);

                // If we have a temp declaration, prepend it
                if let Some(temp_decl) = &temp_decl_opt {
                    return format!(
                        "{}{}{}",
                        temp_decl,
                        if temp_decl.ends_with(';') { " " } else { "" },
                        api_call_code
                    );
                }

                // If we have an expected_type and the API returns Object/Any (Value), cast the result
                // This handles: let x: int = Root::get_value(); and map.get("key")
                if let Some(expected_ty) = expected_type {
                    let api_return_type = module.return_type();
                    if matches!(api_return_type.as_ref(), Some(Type::Object | Type::Any)) {
                        // Check if this is MapResource::Get and if the map is actually dynamic
                        let should_cast = if let crate::call_modules::CallModule::Resource(
                            crate::resource_modules::ResourceModule::MapOp(MapResource::Get),
                        ) = module
                        {
                            // For MapResource::Get, check if the map's value type is Object (dynamic)
                            // If it's not Object, then it's a static map and we shouldn't cast
                            if let Some(map_expr) = args.get(0) {
                                let map_value_type =
                                    script.infer_map_value_type(map_expr, current_func);
                                map_value_type.as_ref() == Some(&Type::Object)
                            } else {
                                true // Fallback: assume dynamic if we can't infer
                            }
                        } else {
                            true // For other APIs, apply cast if they return Object
                        };

                        if should_cast && !matches!(expected_ty, Type::Object | Type::Any) {
                            return value_to_expected_rust(&api_call_code, expected_ty);
                        }
                        api_call_code
                    } else {
                        api_call_code
                    }
                } else {
                    api_call_code
                }
            }
            Expr::Range(start, end) => {
                // For ranges, ensure integer literals are typed as integers, not floats
                // Rust ranges require types that implement Step, which f32 doesn't
                // Check if start/end are number literals - if so, prefer i32 for ranges
                let start_inferred = script.infer_expr_type(start, current_func);
                let start_expected_type = match &**start {
                    Expr::Literal(Literal::Number(_)) => {
                        // For number literals in ranges, default to i32 unless already typed as integer
                        start_inferred
                            .map(|t| match t {
                                Type::Number(NumberKind::Float(_)) => {
                                    Type::Number(NumberKind::Signed(32))
                                }
                                other => other,
                            })
                            .or(Some(Type::Number(NumberKind::Signed(32))))
                    }
                    _ => start_inferred,
                };
                let end_inferred = script.infer_expr_type(end, current_func);
                let end_expected_type = match &**end {
                    Expr::Literal(Literal::Number(_)) => end_inferred
                        .map(|t| match t {
                            Type::Number(NumberKind::Float(_)) => {
                                Type::Number(NumberKind::Signed(32))
                            }
                            other => other,
                        })
                        .or(Some(Type::Number(NumberKind::Signed(32)))),
                    _ => end_inferred,
                };

                let start_code = start.to_rust(
                    needs_self,
                    script,
                    start_expected_type.as_ref(),
                    current_func,
                    None, // start is Expr, no span available
                );
                let end_code = end.to_rust(
                    needs_self,
                    script,
                    end_expected_type.as_ref(),
                    current_func,
                    None,
                ); // end is Expr, no span available
                format!("({}..{})", start_code, end_code)
            }
            Expr::Cast(inner, target_type) => {
                // Special case: if inner is SelfAccess, ALWAYS return self.id - never store it
                if matches!(inner.as_ref(), Expr::SelfAccess) {
                    return "self.id".to_string();
                }

                let inner_type = script.infer_expr_type(inner, current_func);
                // Don't pass target_type as expected_type - let the literal be its natural type, then cast

                let mut inner_code = inner.to_rust(needs_self, script, None, current_func, None);

                // Special case: if inner_code is "self" or contains t_id_self, fix it to self.id
                inner_code = if inner_code == "self" || inner_code.starts_with("t_id_self") {
                    "self.id".to_string()
                } else {
                    inner_code
                };

                // Fix any incorrect renaming of "api" to "__t_api" or "t_id_api"
                // The "api" identifier should NEVER be renamed - it's always the API parameter
                inner_code = inner_code
                    .replace("__t_api.", "api.")
                    .replace("t_id_api.", "api.");

                match (&inner_type, target_type) {
                    // String → Numeric Type Conversions
                    (Some(Type::String), Type::Number(NumberKind::Signed(w))) => match w {
                        8 => format!("{}.parse::<i8>().unwrap_or_default()", inner_code),
                        16 => format!("{}.parse::<i16>().unwrap_or_default()", inner_code),
                        32 => format!("{}.parse::<i32>().unwrap_or_default()", inner_code),
                        64 => format!("{}.parse::<i64>().unwrap_or_default()", inner_code),
                        128 => format!("{}.parse::<i128>().unwrap_or_default()", inner_code),
                        _ => format!("{}.parse::<i32>().unwrap_or_default()", inner_code),
                    },

                    (Some(Type::String), Type::Number(NumberKind::Unsigned(w))) => match w {
                        8 => format!("{}.parse::<u8>().unwrap_or_default()", inner_code),
                        16 => format!("{}.parse::<u16>().unwrap_or_default()", inner_code),
                        32 => format!("{}.parse::<u32>().unwrap_or_default()", inner_code),
                        64 => format!("{}.parse::<u64>().unwrap_or_default()", inner_code),
                        128 => format!("{}.parse::<u128>().unwrap_or_default()", inner_code),
                        _ => format!("{}.parse::<u32>().unwrap_or_default()", inner_code),
                    },

                    (Some(Type::String), Type::Number(NumberKind::Float(w))) => match w {
                        32 => format!("{}.parse::<f32>().unwrap_or_default()", inner_code),
                        64 => format!("{}.parse::<f64>().unwrap_or_default()", inner_code),
                        _ => format!("{}.parse::<f32>().unwrap_or_default()", inner_code),
                    },

                    (Some(Type::String), Type::Number(NumberKind::Decimal)) => format!(
                        "Decimal::from_str({}.as_ref()).unwrap_or_default()",
                        inner_code
                    ),

                    (Some(Type::String), Type::Number(NumberKind::BigInt)) => format!(
                        "BigInt::from_str({}.as_ref()).unwrap_or_default()",
                        inner_code
                    ),

                    (Some(Type::String), Type::Bool) => {
                        format!("{}.parse::<bool>().unwrap_or_default()", inner_code)
                    }

                    // Numeric/Bool → String Conversions
                    (Some(Type::Number(_)), Type::String) | (Some(Type::Bool), Type::String) => {
                        format!("{}.to_string()", inner_code)
                    }

                    // String type conversions
                    // String -> CowStr (owned string to Cow)
                    (Some(Type::String), Type::CowStr) => {
                        // Optimize String::from("...") to Cow::Borrowed("...")
                        if let Some(captured_str) = inner_code
                            .strip_prefix("String::from(\"")
                            .and_then(|s| s.strip_suffix("\")"))
                        {
                            format!("Cow::Borrowed(\"{}\")", captured_str)
                        } else {
                            format!("{}.into()", inner_code)
                        }
                    }
                    // Option<String> -> Option<CowStr>
                    (Some(Type::Option(inner_from)), Type::Option(inner_to))
                        if matches!(inner_from.as_ref(), Type::String)
                            && matches!(inner_to.as_ref(), Type::CowStr) =>
                    {
                        // Optimize Some(String::from("...")) to Some(Cow::Borrowed("..."))
                        if let Some(captured_str) = inner_code
                            .strip_prefix("Some(String::from(\"")
                            .and_then(|s| s.strip_suffix("\"))"))
                        {
                            format!("Some(Cow::Borrowed(\"{}\"))", captured_str)
                        } else {
                            format!("{}.map(|s| s.into())", inner_code)
                        }
                    }
                    // StrRef -> CowStr (borrowed string to Cow)
                    (Some(Type::StrRef), Type::CowStr) => {
                        format!("{}.into()", inner_code)
                    }
                    // CowStr -> String (Cow to owned String)
                    (Some(Type::CowStr), Type::String) => {
                        format!("{}.into_owned()", inner_code)
                    }
                    // CowStr -> StrRef (Cow to &str - only if Borrowed)
                    (Some(Type::CowStr), Type::StrRef) => {
                        format!("{}.as_ref()", inner_code)
                    }
                    // Node types -> Uuid (nodes are Uuid IDs)
                    (Some(Type::Node(_)), Type::DynNode) => {
                        // Special case: if inner_code is "self" or contains "self", ensure it's self.id
                        if inner_code == "self"
                            || (inner_code.starts_with("self") && !inner_code.contains("self.id"))
                        {
                            "self.id".to_string()
                        } else if inner_code == "self.id" || inner_code.ends_with(".id") {
                            // Already self.id or ends with .id - no cast needed, it's already Uuid
                            inner_code
                        } else {
                            inner_code // Already a Uuid, no conversion needed
                        }
                    }
                    // Uuid -> Node type (for type checking, just pass through)
                    (Some(Type::DynNode), Type::Node(_)) => {
                        inner_code // Already a Uuid, no conversion needed
                    }
                    // T -> Option<T> conversions (wrapping in Some)
                    (Some(from), Type::Option(inner)) if from == inner.as_ref() => {
                        format!("Some({})", inner_code)
                    }
                    // UuidOption (Option<Uuid>) -> Uuid
                    // This is for get_child_by_name() which returns Option<Uuid>
                    (Some(Type::Custom(from_name)), Type::DynNode) if from_name == "UuidOption" => {
                        // Unwrap the Option<Uuid>
                        format!("{}.unwrap()", inner_code)
                    }

                    // BigInt → Signed Integer
                    (
                        Some(Type::Number(NumberKind::BigInt)),
                        Type::Number(NumberKind::Signed(w)),
                    ) => match w {
                        8 => format!("{}.to_i8().unwrap_or_default()", inner_code),
                        16 => format!("{}.to_i16().unwrap_or_default()", inner_code),
                        32 => format!("{}.to_i32().unwrap_or_default()", inner_code),
                        64 => format!("{}.to_i64().unwrap_or_default()", inner_code),
                        128 => format!("{}.to_i128().unwrap_or_default()", inner_code),
                        _ => format!("({}.to_i64().unwrap_or_default() as i{})", inner_code, w),
                    },

                    // BigInt → Unsigned Integer
                    (
                        Some(Type::Number(NumberKind::BigInt)),
                        Type::Number(NumberKind::Unsigned(w)),
                    ) => match w {
                        8 => format!("{}.to_u8().unwrap_or_default()", inner_code),
                        16 => format!("{}.to_u16().unwrap_or_default()", inner_code),
                        32 => format!("{}.to_u32().unwrap_or_default()", inner_code),
                        64 => format!("{}.to_u64().unwrap_or_default()", inner_code),
                        128 => format!("{}.to_u128().unwrap_or_default()", inner_code),
                        _ => format!("({}.to_u64().unwrap_or_default() as u{})", inner_code, w),
                    },

                    // BigInt ↔ Float
                    (
                        Some(Type::Number(NumberKind::BigInt)),
                        Type::Number(NumberKind::Float(32)),
                    ) => format!("{}.to_f32().unwrap_or_default()", inner_code),
                    (
                        Some(Type::Number(NumberKind::BigInt)),
                        Type::Number(NumberKind::Float(64)),
                    ) => format!("{}.to_f64().unwrap_or_default()", inner_code),
                    (
                        Some(Type::Number(NumberKind::Float(w))),
                        Type::Number(NumberKind::BigInt),
                    ) => match w {
                        32 => format!("BigInt::from({} as i32)", inner_code),
                        64 => format!("BigInt::from({} as i64)", inner_code),
                        _ => format!("BigInt::from({} as i64)", inner_code),
                    },

                    // Decimal → Integer
                    (
                        Some(Type::Number(NumberKind::Decimal)),
                        Type::Number(NumberKind::Signed(w)),
                    ) => match w {
                        8 => format!("{}.to_i8().unwrap_or_default()", inner_code),
                        16 => format!("{}.to_i16().unwrap_or_default()", inner_code),
                        32 => format!("{}.to_i32().unwrap_or_default()", inner_code),
                        64 => format!("{}.to_i64().unwrap_or_default()", inner_code),
                        128 => format!("({}.to_i64().unwrap_or_default() as i{})", inner_code, w),
                        _ => format!("({}.to_i64().unwrap_or_default() as i{})", inner_code, w),
                    },
                    (
                        Some(Type::Number(NumberKind::Decimal)),
                        Type::Number(NumberKind::Unsigned(w)),
                    ) => match w {
                        8 => format!("{}.to_u8().unwrap_or_default()", inner_code),
                        16 => format!("{}.to_u16().unwrap_or_default()", inner_code),
                        32 => format!("{}.to_u32().unwrap_or_default()", inner_code),
                        64 => format!("{}.to_u64().unwrap_or_default()", inner_code),
                        128 => format!("({}.to_u64().unwrap_or_default() as u{})", inner_code, w),
                        _ => format!("({}.to_u64().unwrap_or_default() as u{})", inner_code, w),
                    },

                    // Decimal → Float
                    (
                        Some(Type::Number(NumberKind::Decimal)),
                        Type::Number(NumberKind::Float(32)),
                    ) => format!("{}.to_f32().unwrap_or_default()", inner_code),
                    (
                        Some(Type::Number(NumberKind::Decimal)),
                        Type::Number(NumberKind::Float(64)),
                    ) => format!("{}.to_f64().unwrap_or_default()", inner_code),

                    // Integer/Float → Decimal
                    (
                        Some(Type::Number(NumberKind::Signed(_) | NumberKind::Unsigned(_))),
                        Type::Number(NumberKind::Decimal),
                    ) => format!("Decimal::from({})", inner_code),

                    (
                        Some(Type::Number(NumberKind::Float(32))),
                        Type::Number(NumberKind::Decimal),
                    ) => format!(
                        "rust_decimal::prelude::FromPrimitive::from_f32({}).unwrap_or_default()",
                        inner_code
                    ),
                    (
                        Some(Type::Number(NumberKind::Float(64))),
                        Type::Number(NumberKind::Decimal),
                    ) => format!(
                        "rust_decimal::prelude::FromPrimitive::from_f64({}).unwrap_or_default()",
                        inner_code
                    ),

                    // Decimal ↔ BigInt
                    (Some(Type::Number(NumberKind::Decimal)), Type::Number(NumberKind::BigInt)) => {
                        format!("BigInt::from({}.to_i64().unwrap_or_default())", inner_code)
                    }
                    (Some(Type::Number(NumberKind::BigInt)), Type::Number(NumberKind::Decimal)) => {
                        format!("Decimal::from({}.to_i64().unwrap_or_default())", inner_code)
                    }

                    // Standard Numeric Casts
                    (
                        Some(Type::Number(NumberKind::Signed(_))),
                        Type::Number(NumberKind::Signed(to_w)),
                    ) => format!("({} as i{})", inner_code, to_w),
                    (
                        Some(Type::Number(NumberKind::Signed(_))),
                        Type::Number(NumberKind::Unsigned(to_w)),
                    ) => format!("({} as u{})", inner_code, to_w),
                    (
                        Some(Type::Number(NumberKind::Unsigned(_))),
                        Type::Number(NumberKind::Unsigned(to_w)),
                    ) => format!("({} as u{})", inner_code, to_w),
                    (
                        Some(Type::Number(NumberKind::Unsigned(_))),
                        Type::Number(NumberKind::Signed(to_w)),
                    ) => format!("({} as i{})", inner_code, to_w),

                    (
                        Some(Type::Number(NumberKind::Signed(_) | NumberKind::Unsigned(_))),
                        Type::Number(NumberKind::Float(32)),
                    ) => format!("({} as f32)", inner_code),
                    (
                        Some(Type::Number(NumberKind::Signed(_) | NumberKind::Unsigned(_))),
                        Type::Number(NumberKind::Float(64)),
                    ) => format!("({} as f64)", inner_code),

                    (
                        Some(Type::Number(NumberKind::Float(_))),
                        Type::Number(NumberKind::Signed(w)),
                    ) => format!("({}.round() as i{})", inner_code, w),
                    (
                        Some(Type::Number(NumberKind::Float(_))),
                        Type::Number(NumberKind::Unsigned(w)),
                    ) => format!("({}.round() as u{})", inner_code, w),

                    (
                        Some(Type::Number(NumberKind::Float(32))),
                        Type::Number(NumberKind::Float(64)),
                    ) => format!("({} as f64)", inner_code),
                    (
                        Some(Type::Number(NumberKind::Float(64))),
                        Type::Number(NumberKind::Float(32)),
                    ) => format!("({} as f32)", inner_code),

                    (
                        Some(Type::Number(NumberKind::Signed(w))),
                        Type::Number(NumberKind::BigInt),
                    ) => match w {
                        32 => format!("BigInt::from({} as i32)", inner_code),
                        64 => format!("BigInt::from({} as i64)", inner_code),
                        _ => format!("BigInt::from({} as i64)", inner_code),
                    },
                    (
                        Some(Type::Number(NumberKind::Unsigned(w))),
                        Type::Number(NumberKind::BigInt),
                    ) => match w {
                        32 => format!("BigInt::from({} as u32)", inner_code),
                        64 => format!("BigInt::from({} as u64)", inner_code),
                        _ => format!("BigInt::from({} as u64)", inner_code),
                    },

                    // ==========================================================
                    // Bool → Number (for arithmetic operations)
                    // ==========================================================
                    (Some(Type::Bool), Type::Number(NumberKind::Float(32))) => {
                        format!("({} as u8 as f32)", inner_code)
                    }
                    (Some(Type::Bool), Type::Number(NumberKind::Float(64))) => {
                        format!("({} as u8 as f64)", inner_code)
                    }
                    (Some(Type::Bool), Type::Number(NumberKind::Signed(w))) => {
                        format!("({} as i{})", inner_code, w)
                    }
                    (Some(Type::Bool), Type::Number(NumberKind::Unsigned(w))) => {
                        format!("({} as u{})", inner_code, w)
                    }

                    // ==========================================================
                    // JSON Value (Object/Any) → Anything
                    // ==========================================================
                    (Some(inner_ty), target) if matches!(inner_ty, Type::Object | Type::Any) => {
                        use NumberKind::*;
                        match target {
                            Type::Number(Signed(w)) => {
                                format!("{}.as_i64().unwrap_or_default() as i{}", inner_code, w)
                            }

                            Type::Number(Unsigned(w)) => {
                                format!("{}.as_u64().unwrap_or_default() as u{}", inner_code, w)
                            }

                            Type::Number(Float(w)) => match w {
                                32 => format!("{}.as_f64().unwrap_or_default() as f32", inner_code),
                                64 => format!("{}.as_f64().unwrap_or_default()", inner_code),
                                _ => format!("{}.as_f64().unwrap_or_default() as f64", inner_code),
                            },

                            Type::String => {
                                format!("{}.as_str().unwrap_or_default().to_string()", inner_code)
                            }

                            Type::Bool => format!("{}.as_bool().unwrap_or_default()", inner_code),

                            Type::Number(NumberKind::BigInt) => {
                                // Value to BigInt: try as string first (JSON serializes BigInt as string)
                                format!(
                                    "{}.as_str().map(|s| s.parse::<BigInt>().unwrap_or_default()).unwrap_or_else(|| BigInt::from({}.as_i64().unwrap_or_default()))",
                                    inner_code, inner_code
                                )
                            }

                            Type::Number(NumberKind::Decimal) => {
                                // Value to Decimal: try as string first, then use FromPrimitive for f64
                                format!(
                                    "{}.as_str().map(|s| Decimal::from_str(s).unwrap_or_default()).unwrap_or_else(|| rust_decimal::prelude::FromPrimitive::from_f64({}.as_f64().unwrap_or_default()).unwrap_or_default())",
                                    inner_code, inner_code
                                )
                            }

                            Type::Custom(name) => {
                                // Check if this is a cast from get_child_by_name (Option<Uuid>) to a node type
                                // Pattern: self.get_node("name") as Sprite2D
                                // Note: get_parent() now returns Node directly, so it doesn't need special handling here
                                if let Expr::ApiCall(
                                    crate::call_modules::CallModule::NodeMethod(crate::structs::engine_registry::NodeMethodRef::GetChildByName),
                                    _,
                                ) = inner.as_ref()
                                {
                                // get_child_by_name returns Option<Uuid>, casting to node type just unwraps the Option
                                // Property access will use read_node/mutate_node under the hood
                                // Unwrap the Option - panic if child not found (user expects this behavior)
                                format!(
                                    "{}.unwrap_or_else(|| panic!(\"Child node not found\"))",
                                    inner_code
                                )
                                } else {
                                    // Strip 'mut' if present; use transpiled struct name (__t_Foo) for Rust
                                    let clean_name = if name.starts_with("mut ") {
                                        name.strip_prefix("mut ").unwrap_or(name)
                                    } else {
                                        name
                                    };
                                    let rust_struct = rename_struct(clean_name);
                                    format!(
                                        "serde_json::from_value::<{}>({}.clone()).unwrap_or_default()",
                                        rust_struct, inner_code
                                    )
                                }
                            }

                            Type::Container(ContainerKind::Array, inner) => format!(
                                "serde_json::from_value::<Vec<{}>>({}).unwrap_or_default()",
                                inner
                                    .get(0)
                                    .map_or("Value".to_string(), |t| t.to_rust_type()),
                                inner_code
                            ),

                            Type::Container(ContainerKind::Map, inner) => format!(
                                "serde_json::from_value::<HashMap<{}, {}>>({}).unwrap_or_default()",
                                inner
                                    .get(0)
                                    .map_or("String".to_string(), |k| k.to_rust_type()),
                                inner
                                    .get(1)
                                    .map_or("Value".to_string(), |v| v.to_rust_type()),
                                inner_code
                            ),

                            _ => format!("{}.clone()", inner_code),
                        }
                    }

                    // Option<Uuid> (from get_child_by_name) to Custom type (node type)
                    // Pattern: self.get_node("name") as Sprite2D
                    // Note: get_parent() now returns Uuid directly, not Option<Uuid>
                    (Some(Type::Custom(from_name)), Type::Custom(to_name))
                        if from_name == "UuidOption" =>
                    {
                        // get_child_by_name returns Option<Uuid>, cast to node type
                        // Keep it as Option<Uuid> - property access will unwrap and use read_node/mutate_node
                        // The variable will be stored as Option<Uuid> and unwrapped when accessing properties
                        inner_code
                    }

                    // UIElement (from get_element) to specific UI element type — cast is same ID (no clone)
                    (_, Type::UIElement(_)) => inner_code.clone(),
                    (Some(Type::Custom(_)), Type::Custom(to_name)) if is_ui_element_type(to_name) => {
                        // Cast to UIText/UIButton/UIPanel: same ID
                        inner_code.clone()
                    }

                    // Option<NodeID> to NodeID (explicit cast means non-optional)
                    (Some(Type::Option(inner)), Type::Node(_))
                        if matches!(inner.as_ref(), Type::DynNode | Type::Node(_)) =>
                    {
                        format!("{}.expect(\"Node not found\")", inner_code)
                    }
                    // NodeID to specific node type (e.g., get_parent() as Sprite2D)
                    (Some(Type::Option(inner)), Type::Custom(to_name))
                        if is_node_type(to_name)
                            && matches!(inner.as_ref(), Type::DynNode | Type::Node(_)) =>
                    {
                        // Option<NodeID> to specific node type: unwrap (explicit cast means non-optional)
                        format!("{}.unwrap()", inner_code)
                    }
                    (Some(Type::DynNode), Type::Custom(to_name)) if is_node_type(to_name) => {
                        // Cast from NodeID (from get_parent() or other methods) to specific node type
                        // Casting to a node type just returns the UUID - property access will use read_node/mutate_node
                        // The inner_code is already a NodeID (Uuid), so just return it as-is
                        inner_code.clone()
                    }

                    // Node to specific node type (e.g., Node as Sprite2D)
                    (Some(Type::Node(_)), Type::Custom(to_name)) if is_node_type(to_name) => {
                        // Cast from base Node to specific node type
                        // Casting to a node type just returns the UUID - property access will use read_node/mutate_node
                        // If inner_code is "self", use self.id. Otherwise, it's already a node ID variable (bob_id)
                        if inner_code == "self" {
                            "self.id".to_string()
                        } else {
                            inner_code.clone()
                        }
                    }

                    // Any cast to UI element type (incl. Type::Custom("UIText")): value stays Option<UIElementID>, no Rust cast.
                    (_, ref tt) if is_ui_element_ref_type(tt) => inner_code.clone(),

                    // Custom type to Custom type (struct casts)
                    (Some(Type::Custom(from_name)), Type::Custom(to_name)) => {
                        if from_name == to_name {
                            inner_code
                        } else {
                            // Use serde_json conversion for struct casts - clone if it's a MemberAccess to avoid move
                            let cloned_code = if inner_code.contains("self.")
                                && !inner_code.contains(".clone()")
                            {
                                format!("{}.clone()", inner_code)
                            } else {
                                inner_code
                            };
                            // Strip 'mut' if present in type name; use transpiled struct name (__t_Foo)
                            let clean_to_name = if to_name.starts_with("mut ") {
                                to_name.strip_prefix("mut ").unwrap_or(to_name)
                            } else {
                                to_name
                            };
                            let rust_to_name = rename_struct(clean_to_name);
                            format!(
                                "serde_json::from_value::<{}>(serde_json::to_value(&{}).unwrap_or_default()).unwrap_or_default()",
                                rust_to_name, cloned_code
                            )
                        }
                    }

                    // Custom type to Custom type (from any type)
                    (_, Type::Custom(to_name)) => {
                        // Clone if it's a MemberAccess to avoid move
                        let cloned_code =
                            if inner_code.contains("self.") && !inner_code.contains(".clone()") {
                                format!("{}.clone()", inner_code)
                            } else {
                                inner_code
                            };
                        let rust_to_name = rename_struct(to_name);
                        format!(
                            "serde_json::from_value::<{}>(serde_json::to_value(&{}).unwrap_or_default()).unwrap_or_default()",
                            rust_to_name, cloned_code
                        )
                    }

                    // When inner type is None or Object/Any, Value -> BigInt/Decimal must use extraction (not "as")
                    // Do NOT wrap in outer ( ); when used in format!("{} {}", a, b) extra parens can cause
                    // mismatched delimiter (unclosed ( or } confusion) in generated code.
                    (inner_opt, Type::Number(NumberKind::BigInt))
                        if matches!(inner_opt, None | Some(Type::Object) | Some(Type::Any)) =>
                    {
                        format!(
                            "{}.as_str().map(|s| s.parse::<BigInt>().unwrap_or_default()).unwrap_or_else(|| BigInt::from({}.as_i64().unwrap_or_default()))",
                            inner_code, inner_code
                        )
                    }
                    (Some(Type::Custom(name)), Type::Number(NumberKind::BigInt))
                        if name == "Value" =>
                    {
                        format!(
                            "{}.as_str().map(|s| s.parse::<BigInt>().unwrap_or_default()).unwrap_or_else(|| BigInt::from({}.as_i64().unwrap_or_default()))",
                            inner_code, inner_code
                        )
                    }
                    (inner_opt, Type::Number(NumberKind::Decimal))
                        if matches!(inner_opt, None | Some(Type::Object) | Some(Type::Any)) =>
                    {
                        format!(
                            "{}.as_str().map(|s| Decimal::from_str(s).unwrap_or_default()).unwrap_or_else(|| rust_decimal::prelude::FromPrimitive::from_f64({}.as_f64().unwrap_or_default()).unwrap_or_default())",
                            inner_code, inner_code
                        )
                    }
                    (Some(Type::Custom(name)), Type::Number(NumberKind::Decimal))
                        if name == "Value" =>
                    {
                        format!(
                            "{}.as_str().map(|s| Decimal::from_str(s).unwrap_or_default()).unwrap_or_else(|| rust_decimal::prelude::FromPrimitive::from_f64({}.as_f64().unwrap_or_default()).unwrap_or_default())",
                            inner_code, inner_code
                        )
                    }
                    // Value (None/Object/Any) -> signed integer (e.g. BigInt::from(value as i32))
                    (inner_opt, Type::Number(NumberKind::Signed(w)))
                        if matches!(inner_opt, None | Some(Type::Object) | Some(Type::Any)) =>
                    {
                        format!("{}.as_i64().unwrap_or_default() as i{}", inner_code, w)
                    }
                    (Some(Type::Custom(name)), Type::Number(NumberKind::Signed(w)))
                        if name == "Value" =>
                    {
                        format!("{}.as_i64().unwrap_or_default() as i{}", inner_code, w)
                    }
                    // None -> unsigned integer (e.g. script var typed_big_int not inferred): use .to_u64() so BigInt works
                    (None, Type::Number(NumberKind::Unsigned(w))) => match w {
                        8 => format!("{}.to_u8().unwrap_or_default()", inner_code),
                        16 => format!("{}.to_u16().unwrap_or_default()", inner_code),
                        32 => format!("{}.to_u32().unwrap_or_default()", inner_code),
                        64 => format!("{}.to_u64().unwrap_or_default()", inner_code),
                        128 => format!("{}.to_u128().unwrap_or_default()", inner_code),
                        _ => format!("({}.to_u64().unwrap_or_default() as u{})", inner_code, w),
                    },
                    // Value (Object/Any) -> unsigned integer
                    (inner_opt, Type::Number(NumberKind::Unsigned(w)))
                        if matches!(inner_opt, Some(Type::Object) | Some(Type::Any)) =>
                    {
                        format!("{}.as_u64().unwrap_or_default() as u{}", inner_code, w)
                    }
                    (Some(Type::Custom(name)), Type::Number(NumberKind::Unsigned(w)))
                        if name == "Value" =>
                    {
                        format!("{}.as_u64().unwrap_or_default() as u{}", inner_code, w)
                    }

                    _ => {
                        // For non-primitive types, try .into() instead of as cast
                        if matches!(target_type, Type::CowStr | Type::String | Type::Custom(_)) {
                            format!("{}.into()", inner_code)
                        } else if matches!(target_type, Type::Number(NumberKind::BigInt)) {
                            // Value-like source (inferred or custom) -> BigInt: use extraction, not "as"
                            format!(
                                "{}.as_str().map(|s| s.parse::<BigInt>().unwrap_or_default()).unwrap_or_else(|| BigInt::from({}.as_i64().unwrap_or_default()))",
                                inner_code, inner_code
                            )
                        } else if matches!(target_type, Type::Number(NumberKind::Decimal)) {
                            // Value-like source -> Decimal: use extraction, not "as"
                            format!(
                                "{}.as_str().map(|s| Decimal::from_str(s).unwrap_or_default()).unwrap_or_else(|| rust_decimal::prelude::FromPrimitive::from_f64({}.as_f64().unwrap_or_default()).unwrap_or_default())",
                                inner_code, inner_code
                            )
                        } else if let Type::Number(NumberKind::Unsigned(w)) = target_type {
                            // Fallback when source type wasn't inferred (e.g. script var): use .to_u*() so BigInt works (non-primitive "as" would error)
                            match w {
                                8 => format!("{}.to_u8().unwrap_or_default()", inner_code),
                                16 => format!("{}.to_u16().unwrap_or_default()", inner_code),
                                32 => format!("{}.to_u32().unwrap_or_default()", inner_code),
                                64 => format!("{}.to_u64().unwrap_or_default()", inner_code),
                                128 => format!("{}.to_u128().unwrap_or_default()", inner_code),
                                _ => format!(
                                    "({}.to_u64().unwrap_or_default() as u{})",
                                    inner_code, w
                                ),
                            }
                        } else if is_ui_element_ref_type(target_type) {
                            // UI element type narrow: value stays Option<UIElementID>; no Rust cast.
                            inner_code
                        } else {
                            eprintln!(
                                "Warning: Unhandled cast from {:?} to {:?}",
                                inner_type, target_type
                            );
                            format!("({} as {})", inner_code, target_type.to_rust_type())
                        }
                    }
                }
            }
            Expr::Index(base, key) => {
                let base_type = script.infer_expr_type(base, current_func);
                let base_code = base.to_rust(needs_self, script, None, current_func, None);
                // Key type inference for Map access should be specific, otherwise it defaults to String
                let key_code =
                    if let Some(Type::Container(ContainerKind::Map, inner_types)) = &base_type {
                        let key_ty = inner_types.get(0).unwrap_or(&Type::String);
                        key.to_rust(needs_self, script, Some(key_ty), current_func, None)
                    } else {
                        // For arrays or objects, assume string key for now (or other default)
                        key.to_rust(needs_self, script, Some(&Type::String), current_func, None)
                    };

                // Deterministic struct field access: when key is a string/identifier and base is an
                // identifier, resolve type from inference OR declared type (script vars + locals/params).
                // Always emit .field for Custom types so we never emit ["field"] for structs.
                let key_is_field_like = matches!(
                    key.as_ref(),
                    Expr::Literal(Literal::String(_)) | Expr::Ident(_)
                );
                let effective_base_type = if key_is_field_like {
                    base_type.clone().or_else(|| {
                        if let Expr::Ident(var_name) = base.as_ref() {
                            script.get_declared_variable_type(var_name, current_func)
                        } else {
                            None
                        }
                    })
                } else {
                    base_type.clone()
                };
                if key_is_field_like {
                    if let (Some(Type::Custom(_)), Some(field_name)) =
                        (&effective_base_type, index_field_name(key))
                    {
                        return format!("{}.{}", base_code, field_name);
                    }
                }

                match base_type {
                    // ----------------------------------------------------------
                    // ✅ Typed HashMap<K,V>
                    // ----------------------------------------------------------
                    Some(Type::Container(ContainerKind::Map, ref inner_types)) => {
                        let key_ty = inner_types.get(0).unwrap_or(&Type::String);
                        let key_expr_type = script.infer_expr_type(key, current_func);
                        // When key type is not String, ensure key_code is converted (e.g. BigInt -> u8 for Map<u8, V>)
                        let key_code_converted = if *key_ty != Type::String {
                            if let Some(ref kt) = key_expr_type {
                                if *kt != *key_ty && kt.can_implicitly_convert_to(key_ty) {
                                    script.generate_implicit_cast_for_expr(&key_code, kt, key_ty)
                                } else {
                                    key_code.clone()
                                }
                            } else {
                                key_code.clone()
                            }
                        } else {
                            key_code.clone()
                        };
                        let final_key_code = if *key_ty == Type::String {
                            if matches!(key_expr_type, Some(Type::Number(_)) | Some(Type::Bool)) {
                                format!("{}.to_string().as_str()", key_code_converted)
                            } else {
                                format!("{}.as_str()", key_code_converted)
                            }
                        } else {
                            format!("&{}", key_code_converted)
                        };
                        format!(
                            "{}.get({}).cloned().unwrap_or_default()",
                            base_code, final_key_code
                        )
                    }

                    // ----------------------------------------------------------
                    // ✅ Dynamic JSON object (serde_json::Value)
                    // When base is an Ident, check declared type: if Custom (struct), use field access.
                    // ----------------------------------------------------------
                    Some(Type::Object) => {
                        if let (Expr::Ident(var_name), Some(field_name)) =
                            (base.as_ref(), index_field_name(key))
                        {
                            if script
                                .get_declared_variable_type(var_name, current_func)
                                .as_ref()
                                .map_or(false, |t| matches!(t, Type::Custom(_)))
                            {
                                return format!("{}.{}", base_code, field_name);
                            }
                        }
                        format!("{}[{}].clone()", base_code, key_code)
                    }

                    // ----------------------------------------------------------
                    // ✅ Arrays: differentiate typed Vec<T> vs. Vec<Value>
                    // ----------------------------------------------------------
                    Some(Type::Container(ContainerKind::Array, ref inner_types)) => {
                        let index_code = key.to_rust(
                            needs_self,
                            script,
                            Some(&Type::Number(NumberKind::Unsigned(32))),
                            current_func,
                            None,
                        );

                        // Check if this is a custom type array (polymorphic - stored as Vec<Value>)
                        if let Some(inner_type) = inner_types.get(0) {
                            match inner_type {
                                Type::Custom(_) => {
                                    // Custom type arrays are stored as Vec<Value>, auto-cast on access
                                    let rust_type = inner_type.to_rust_type();
                                    format!(
                                        "serde_json::from_value::<{}>({}.get({} as usize).cloned().unwrap_or_default()).unwrap_or_default()",
                                        rust_type, base_code, index_code
                                    )
                                }
                                _ => {
                                    // Primitive types - direct access
                                    format!(
                                        "{}.get({} as usize).cloned().unwrap_or_default()",
                                        base_code, index_code
                                    )
                                }
                            }
                        } else {
                            // No inner type specified - treat as Vec<Value>
                            format!(
                                "{}.get({} as usize).cloned().unwrap_or_default()",
                                base_code, index_code
                            )
                        }
                    }

                    // ----------------------------------------------------------
                    // ✅ Fixed-size array: [T; N]
                    // ----------------------------------------------------------
                    Some(Type::Container(ContainerKind::FixedArray(_), _)) => {
                        // inner_types not needed for codegen here
                        let index_code = key.to_rust(
                            needs_self,
                            script,
                            Some(&Type::Number(NumberKind::Unsigned(32))),
                            current_func,
                            None,
                        );
                        // Result from .get() is cloned, so it's a T or Value, handled by infer_expr_type
                        format!(
                            "{}.get({} as usize).cloned().unwrap_or_default()",
                            base_code, index_code
                        )
                    }

                    // ----------------------------------------------------------
                    // Custom type (struct): key is field name -> .field_name; else numeric index.
                    // ----------------------------------------------------------
                    Some(Type::Custom(_)) => {
                        if let Some(field_name) = index_field_name(key) {
                            format!("{}.{}", base_code, field_name)
                        } else {
                            let index_code = key.to_rust(
                                needs_self,
                                script,
                                Some(&Type::Number(NumberKind::Unsigned(32))),
                                current_func,
                                None,
                            );
                            format!(
                                "{}.get({} as usize).cloned().unwrap_or_default()",
                                base_code, index_code
                            )
                        }
                    }
                    // Unsupported: not Map/Array/FixedArray/Object/Custom (e.g. unknown type).
                    _ => "/* unsupported index expression */".to_string(),
                }
            }
            Expr::ObjectLiteral(items) => {
                let pairs: Vec<_> = items
                    .iter()
                    .map(|(k, v)| {
                        format!(
                            "\"{}\": {}",
                            k.as_deref().unwrap_or(""),
                            v.to_rust(needs_self, script, None, current_func, None)
                        )
                    })
                    .collect();
                format!("json!({{ {} }})", pairs.join(", "))
            }
        }
    }

    pub fn contains_self(&self) -> bool {
        match self {
            Expr::SelfAccess => true,
            Expr::MemberAccess(base, _) => base.contains_self(),
            Expr::BinaryOp(left, _, right) => left.contains_self() || right.contains_self(),
            Expr::Call(target, args) => {
                target.contains_self() || args.iter().any(|arg| arg.contains_self())
            }
            _ => false,
        }
    }

    pub fn contains_api_call(&self, script: &Script) -> bool {
        match self {
            Expr::ApiCall(..) => true,
            Expr::MemberAccess(base, _) => base.contains_api_call(script),
            Expr::BinaryOp(l, _, r) => l.contains_api_call(script) || r.contains_api_call(script),
            Expr::Call(target, args) => {
                target.contains_api_call(script) || args.iter().any(|a| a.contains_api_call(script))
            }
            Expr::ContainerLiteral(_, data) => match data {
                ContainerLiteralData::Array(elements) => {
                    elements.iter().any(|e| e.contains_api_call(script))
                }
                ContainerLiteralData::Map(pairs) => pairs
                    .iter()
                    .any(|(k, v)| k.contains_api_call(script) || v.contains_api_call(script)),
                ContainerLiteralData::FixedArray(_, elements) => {
                    elements.iter().any(|e| e.contains_api_call(script))
                }
            },
            _ => false,
        }
    }

    fn get_target_name(expr: &Expr) -> Option<&str> {
        match expr {
            Expr::Ident(n) => Some(n.as_str()),
            Expr::MemberAccess(_, n) => Some(n.as_str()),
            _ => None,
        }
    }
}
