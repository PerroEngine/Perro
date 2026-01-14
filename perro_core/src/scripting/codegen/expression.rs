// Expression code generation - Expr and TypedExpr
use crate::api_modules::*;
use crate::ast::*;
use crate::scripting::ast::{ContainerKind, Expr, Literal, NumberKind, Stmt, Type, TypedExpr};
use crate::structs::engine_registry::ENGINE_REGISTRY;
use crate::structs::engine_structs::EngineStruct as EngineStructKind;
use super::utils::{is_node_type, rename_variable, string_to_node_type, rename_function, rename_struct, type_is_node, get_node_type};
use super::analysis::{extract_node_member_info, extract_mutable_api_call};
use super::cache::SCRIPT_MEMBERS_CACHE;

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
                // For temp variables (__temp_*), we need to check their actual type
                // Since they're not in the script's variable list, infer_expr_type won't find them
                // But we know from context that most read_node results are Copy types (f32, i32, etc.)
                // So we'll try to infer, and if we can't, we'll check if it looks like a Copy type
                if name.starts_with("__temp_") {
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
                } else if name.starts_with("__t_") {
                    // Loop variables (transpiled identifiers) are typically i32 from ranges, which is Copy
                    // Even if type inference returns None, we know loop variables don't need cloning
                    false
                } else {
                    // Regular variable - use normal type inference
                    if let Some(ty) = script.infer_expr_type(expr, current_func) {
                        ty.requires_clone()
                    } else {
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
                
                // Helper function to find a variable in nested blocks (if, for, etc.)
                fn find_variable_in_body<'a>(name: &str, body: &'a [Stmt]) -> Option<&'a Variable> {
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
                
                let is_local = current_func
                    .map(|f| {
                        f.locals.iter().any(|v| v.name == *name)
                            || f.params.iter().any(|p| p.name == *name)
                            || find_variable_in_body(name, &f.body).is_some()
                    })
                    .unwrap_or(false);

                // Check against `script_vars` to see if it's a field
                let is_field = script.variables.iter().any(|v| v.name == *name);
                
                // Special case: temp variables (__temp_*) should NEVER be renamed if they're NOT user variables
                // If a user actually named a variable __temp_*, we need to rename it to avoid collisions
                if name.starts_with("__temp_") && !is_local && !is_field {
                    return name.to_string();
                }
                
                // Get variable type for renaming
                // If var.typ is None, infer from the variable's value expression
                // We need to handle inferred types separately since we can't return a ref to a temp
                let (var_type_ref, inferred_type_owned) = if is_local {
                    let var_type_ref = current_func.and_then(|f| {
                        f.locals.iter()
                            .find(|v| v.name == *name)
                            .and_then(|v| {
                                // First try explicit type
                                v.typ.as_ref()
                            })
                            .or_else(|| {
                                f.params.iter()
                                    .find(|p| p.name == *name)
                                    .map(|p| &p.typ)
                            })
                    });
                    
                    let inferred = if var_type_ref.is_none() {
                        // If no explicit type, infer from value expression
                        current_func.and_then(|f| {
                            f.locals.iter()
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
                        script.variables.iter()
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
                let (inferred_type_storage, api_return_type_storage): (Option<Type>, Option<Type>) = if is_local {
                    if let Some(func) = current_func {
                        // Try to find variable in top-level locals first
                        let local_opt = func.locals.iter().find(|v| v.name == *name)
                            .or_else(|| {
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
                                local.value.as_ref()
                                    .and_then(|val| script.infer_expr_type(&val.expr, current_func))
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
                        script.variables.iter()
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
                        let local_opt = func.locals.iter().find(|v| v.name == *name)
                            .or_else(|| {
                                // If not found, search nested blocks
                                find_variable_in_body(name, &func.body)
                            });
                        
                        if let Some(local) = local_opt {
                            // Prefer API return type if available (this is what was used during declaration)
                            let explicit_type = local.typ.as_ref();
                            
                            // Use API type first (if available), then explicit type, then inferred type
                            // This ensures we use the same type that was used during declaration
                            api_return_type_storage.as_ref()
                                .or_else(|| explicit_type)
                                .or_else(|| inferred_type_storage.as_ref())
                        } else {
                            // Not found in locals, try params
                            current_func.and_then(|f| {
                                f.params.iter()
                                    .find(|p| p.name == *name)
                                    .map(|p| &p.typ)
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

                // ✨ Add this: wrap in json! if going to Value/Object
                if let Some(Type::Object) = expected_type {
                    format!("json!({})", ident_code)
                } else {
                    ident_code
                }
            }
            Expr::Literal(lit) => {
                // New: check if the expected_type is Type::Object
                if let Some(Type::Object) = expected_type {
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
                        script.infer_literal_type(&Literal::Number(n.clone()), Some(&Type::Number(expected_num_kind.clone())))
                    } else if let Some(Type::Number(ref num_kind)) = right_type_first {
                        // Fallback: right operand is numeric - match its type
                        // eprintln!("[BINARY_OP] Context: right is numeric ({:?}), bypassing cache and inferring literal {} to match", right_type_first, n);
                        script.infer_literal_type(&Literal::Number(n.clone()), Some(&Type::Number(num_kind.clone())))
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
                        script.infer_literal_type(&Literal::Number(n.clone()), Some(&Type::Number(expected_num_kind.clone())))
                    } else if let Some(Type::Number(ref num_kind)) = left_type {
                        // Fallback: left operand is numeric - match its type
                        // eprintln!("[BINARY_OP] Context: left is numeric ({:?}), bypassing cache and inferring literal {} to match", left_type, n);
                        script.infer_literal_type(&Literal::Number(n.clone()), Some(&Type::Number(num_kind.clone())))
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

                let dominant_type = if let Some(expected) = expected_type.cloned() {
                    // eprintln!("[BINARY_OP] Using expected_type: {:?}", expected);
                    Some(expected)
                } else {
                    let promoted = match (&left_type, &right_type) {
                        (Some(l), Some(r)) => script.promote_types(l, r).or(Some(l.clone())),
                        (Some(l), None) => Some(l.clone()),
                        (None, Some(r)) => Some(r.clone()),
                        _ => None,
                    };
                    // eprintln!("[BINARY_OP] Promoted dominant_type: {:?}", promoted);
                    promoted
                };

                // Check if left/right are len() calls BEFORE generating code
                let left_is_len = matches!(
                    left.as_ref(),
                    Expr::ApiCall(ApiModule::ArrayOp(ArrayApi::Len), _)
                ) || matches!(left.as_ref(), Expr::MemberAccess(_, field) if field == "Length" || field == "length" || field == "len");
                let right_is_len = matches!(
                    right.as_ref(),
                    Expr::ApiCall(ApiModule::ArrayOp(ArrayApi::Len), _)
                ) || matches!(right.as_ref(), Expr::MemberAccess(_, field) if field == "Length" || field == "length" || field == "len");

                let left_raw =
                    left.to_rust(needs_self, script, dominant_type.as_ref(), current_func, None);
                let right_raw =
                    right.to_rust(needs_self, script, dominant_type.as_ref(), current_func, None);
                
                // eprintln!("[BINARY_OP] left_raw: {}", left_raw);
                // eprintln!("[BINARY_OP] right_raw: {}", right_raw);

                // Also check the generated code strings for .len() calls
                let left_is_len = left_is_len || left_raw.ends_with(".len()");
                let right_is_len = right_is_len || right_raw.ends_with(".len()");

                let mut l_str = left_raw.clone();
                let mut r_str = right_raw.clone();

                // If left is len() and right is u32/u64 or a literal that looks like u32, convert right to usize
                if left_is_len {
                    // Check the rendered string first (most reliable)
                    if right_raw.ends_with("u32") || right_raw.ends_with("u") {
                        r_str = format!("({} as usize)", r_str);
                    } else if matches!(right_type, Some(Type::Number(NumberKind::Unsigned(32))))
                    {
                        r_str = format!("({} as usize)", r_str);
                    } else if matches!(right_type, Some(Type::Number(NumberKind::Unsigned(64))))
                    {
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
                    } else if matches!(&left_type, Some(Type::Number(NumberKind::Unsigned(32))))
                    {
                        l_str = format!("({} as usize)", l_str);
                    } else if matches!(&left_type, Some(Type::Number(NumberKind::Unsigned(64))))
                    {
                        l_str = format!("({} as usize)", l_str);
                    } else if let Expr::Literal(Literal::Number(n)) = left.as_ref() {
                        if n.ends_with("u32") || n.ends_with("u") {
                            l_str = format!("({} as usize)", l_str);
                        }
                    }
                }

                // Apply normal type conversions
                // IMPORTANT: Special cases must come BEFORE the general case to ensure they match first
                let (left_str, right_str) = match (&left_type, &right_type, &dominant_type) {
                        // Special case: if left is float and right is integer (explicit cast for determinism)
                        (Some(Type::Number(NumberKind::Float(32))), Some(Type::Number(NumberKind::Signed(_) | NumberKind::Unsigned(_))), _) => {
                            // eprintln!("[BINARY_OP] MATCH: Float32 * Integer -> casting right to f32");
                            (l_str, format!("({} as f32)", r_str))
                        }
                        (Some(Type::Number(NumberKind::Float(64))), Some(Type::Number(NumberKind::Signed(_) | NumberKind::Unsigned(_))), _) => {
                            // eprintln!("[BINARY_OP] MATCH: Float64 * Integer -> casting right to f64");
                            (l_str, format!("({} as f64)", r_str))
                        }
                        // Special case: if left is integer and right is float (reverse case)
                        (Some(Type::Number(NumberKind::Signed(_) | NumberKind::Unsigned(_))), Some(Type::Number(NumberKind::Float(32))), _) => {
                            // eprintln!("[BINARY_OP] MATCH: Integer * Float32 -> casting left to f32");
                            (format!("({} as f32)", l_str), r_str)
                        }
                        (Some(Type::Number(NumberKind::Signed(_) | NumberKind::Unsigned(_))), Some(Type::Number(NumberKind::Float(64))), _) => {
                            // eprintln!("[BINARY_OP] MATCH: Integer * Float64 -> casting left to f64");
                            (format!("({} as f64)", l_str), r_str)
                        }
                        // General case: use implicit conversion logic
                        (Some(l), Some(r), Some(dom)) => {
                            // eprintln!("[BINARY_OP] MATCH: General case - l={:?}, r={:?}, dom={:?}", l, r, dom);
                            let l_cast = if l.can_implicitly_convert_to(dom) && l != dom {
                                let casted = script.generate_implicit_cast_for_expr(&l_str, l, dom);
                                // eprintln!("[BINARY_OP] Casting left: {} -> {}", l_str, casted);
                                casted
                            } else {
                                // eprintln!("[BINARY_OP] No cast needed for left: {} (type: {:?})", l_str, l);
                                l_str
                            };
                            let r_cast = if r.can_implicitly_convert_to(dom) && r != dom {
                                let casted = script.generate_implicit_cast_for_expr(&r_str, r, dom);
                                // eprintln!("[BINARY_OP] Casting right: {} -> {}", r_str, casted);
                                casted
                            } else {
                                // eprintln!("[BINARY_OP] No cast needed for right: {} (type: {:?})", r_str, r);
                                r_str
                            };
                            (l_cast, r_cast)
                        }
                        // Fallback: if left type is unknown but right is a float, cast left to float
                        (None, Some(Type::Number(NumberKind::Float(32))), _) => {
                            (format!("({} as f32)", l_str), r_str)
                        }
                        (None, Some(Type::Number(NumberKind::Float(64))), _) => {
                            (format!("({} as f64)", l_str), r_str)
                        }
                        // Fallback: if right type is unknown but left is a float, cast right to float
                        (Some(Type::Number(NumberKind::Float(32))), None, _) => {
                            (l_str, format!("({} as f32)", r_str))
                        }
                        (Some(Type::Number(NumberKind::Float(64))), None, _) => {
                            (l_str, format!("({} as f64)", r_str))
                        }
                        _ => {
                            // eprintln!("[BINARY_OP] MATCH: Fallback case - no casting");
                            (l_str, r_str)
                        }
                };
                
                // eprintln!("[BINARY_OP] After casting: left_str={}, right_str={}", left_str, right_str);
                
                // Apply cloning if needed for non-Copy types (BigInt, Decimal, String, etc.)
                let left_final = Expr::clone_if_needed(left_str.clone(), left, script, current_func);
                let right_final = Expr::clone_if_needed(right_str.clone(), right, script, current_func);
            
                // if left_final != left_str {
                //     eprintln!("[BINARY_OP] CLONE ADDED to left: {} -> {}", left_str, left_final);
                // }
                // if right_final != right_str {
                //     eprintln!("[BINARY_OP] CLONE ADDED to right: {} -> {}", right_str, right_final);
                // }
                
                // eprintln!("[BINARY_OP] FINAL: left_final={}, right_final={}", left_final, right_final);

                if matches!(op, Op::Add)
                    && (left_type == Some(Type::String) || right_type == Some(Type::String))
                {
                    return format!("format!(\"{{}}{{}}\", {}, {})", left_final, right_final);
                }

                // Handle null checks: body != null -> body.is_some(), body == null -> body.is_none()
                // Check if one side is the identifier "null" and the other is an Option type
                if matches!(op, Op::Ne | Op::Eq) {
                    let left_is_null = matches!(left.as_ref(), Expr::Ident(name) if name == "null");
                    let right_is_null =
                        matches!(right.as_ref(), Expr::Ident(name) if name == "null");

                    if left_is_null && !right_is_null {
                        // null != body -> body.is_none(), null == body -> body.is_none()
                        if matches!(op, Op::Ne) {
                            return format!("{}.is_some()", right_final);
                        } else {
                            return format!("{}.is_none()", right_final);
                        }
                    } else if right_is_null && !left_is_null {
                        // body != null -> body.is_some(), body == null -> body.is_none()
                        if matches!(op, Op::Ne) {
                            return format!("{}.is_some()", left_final);
                        } else {
                            return format!("{}.is_none()", left_final);
                        }
                    }
                }

                format!("({} {} {})", left_final, op.to_rust(), right_final)
            }
            Expr::MemberAccess(base, field) => {
                // Special case: accessing .id or .node_type on parent field
                // self.parent.id -> api.read_node(self.id, |n| n.parent.as_ref().map(|p| p.id).unwrap_or(Uuid::nil()))
                // self.parent.node_type -> api.read_node(self.id, |n| n.parent.as_ref().map(|p| p.node_type.clone()).unwrap())
                if let Expr::MemberAccess(parent_base, parent_field) = base.as_ref() {
                    if matches!(parent_base.as_ref(), Expr::SelfAccess) && parent_field == "parent" {
                        if field == "id" {
                            return format!("api.read_node(self.id, |self_node: &{}| self_node.parent.as_ref().map(|p| p.id).unwrap_or(Uuid::nil()))", script.node_type);
                        } else if field == "node_type" {
                            return format!("api.read_node(self.id, |self_node: &{}| self_node.parent.as_ref().map(|p| p.node_type.clone()).unwrap())", script.node_type);
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
                    extract_node_member_info(&Expr::MemberAccess(base.clone(), field.clone()), script, current_func) 
                {
                    // This is accessing node fields - use api.read_node
                    // Determine if we need to clone the result44
                    
                    if let Some(node_type_enum) = string_to_node_type(&node_type) {
                        let node_type_obj = Type::Node(node_type_enum);
                        
                        // Split the field path to check the final result type
                        let fields: Vec<&str> = field_path.split('.').collect();
                        
                        // Resolve field names in path (e.g., "texture" -> "texture_id")
                        let resolved_fields: Vec<String> = fields.iter()
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
                            if let Some(next_type) = script.get_member_type(&current_type, field_name) {
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
                        
                        let field_access = if should_unwrap {
                            format!("{}.{}.unwrap()", closure_var, resolved_field_path)
                        } else if needs_clone {
                            format!("{}.{}.clone()", closure_var, resolved_field_path)
                        } else {
                            format!("{}.{}", closure_var, resolved_field_path)
                        };
                        
                        // Extract mutable API calls to temporary variables to avoid borrow checker issues
                        let (temp_decl, actual_node_id) = extract_mutable_api_call(&node_id);
                        
                        // Use read_node with the determined node type (type must be known via cast or variable annotation)
                        if !temp_decl.is_empty() {
                            return format!("{}{}api.read_node({}, |{}: &{}| {})", temp_decl, if temp_decl.ends_with(';') { " " } else { "" }, actual_node_id, closure_var, node_type, field_access);
                        } else {
                            return format!("api.read_node({}, |{}: &{}| {})", node_id, closure_var, node_type, field_access);
                        }
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

                let base_type = script.infer_expr_type(base, current_func);

                match base_type {
                    Some(Type::Object) => {
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
                            let base_code = base.to_rust(needs_self, script, None, current_func, None);
                            format!("{}.len()", base_code)
                        } else {
                            // Vec or FixedArray (support access via integer index, not field name)
                            let base_code = base.to_rust(needs_self, script, None, current_func, None);
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
                            let is_node_id_var = base_code.ends_with("_id") || base_code == "self.id";
                            
                            // Check if base is an Option<Uuid> variable (from get_parent() or get_node())
                            // Check in current function's locals first, then script-level variables
                            let is_option_uuid = if let Some(current_func) = current_func {
                                current_func.locals.iter().any(|v| v.name == base_code && matches!(v.typ.as_ref(), Some(Type::Option(inner)) if matches!(inner.as_ref(), Type::Uuid)))
                            } else {
                                script.get_variable_type(&base_code).map_or(false, |t| matches!(t, Type::Option(inner) if matches!(inner.as_ref(), Type::Uuid)))
                            };
                            
                            if is_node_id_var || is_option_uuid {
                                // Use api.read_node to access node properties
                                // Check if the result type requires cloning
                                if let Some(node_type) = string_to_node_type(type_name.as_str()) {
                                    let base_node_type = Type::Node(node_type);
                                    let result_type = script.get_member_type(&base_node_type, field);
                                    let needs_clone = result_type.as_ref().map_or(false, |t| t.requires_clone());
                                    
                                    // Check if the result type is Option<T> - only unwrap if expected type is not Option
                                    let is_option = matches!(result_type.as_ref(), Some(Type::Option(_)));
                                    
                                    // Extract variable name from node_id (e.g., "c_id" -> "c", "par" -> "par")
                                    let param_name = if base_code.ends_with("_id") {
                                        &base_code[..base_code.len() - 3]
                                    } else {
                                        &base_code
                                    };
                                    
                                    // If base is Option<Uuid>, unwrap it before passing to read_node
                                    let node_id_expr = if is_option_uuid {
                                        format!("{}.unwrap()", base_code)
                                    } else {
                                        base_code.clone()
                                    };
                                    
                                    // Resolve field name (e.g., "texture" -> "texture_id")
                                    let resolved_field = ENGINE_REGISTRY.resolve_field_name(&node_type, field);
                                    
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
                                    
                                    let field_access = if should_unwrap {
                                        format!("{}.{}.unwrap()", param_name, resolved_field)
                                    } else if needs_clone {
                                        format!("{}.{}.clone()", param_name, resolved_field)
                                    } else {
                                        format!("{}.{}", param_name, resolved_field)
                                    };
                                    
                                    // Extract mutable API calls to temporary variables to avoid borrow checker issues
                                    let (temp_decl, actual_node_id) = extract_mutable_api_call(&node_id_expr);
                                    if !temp_decl.is_empty() {
                                        return format!("{}{}api.read_node({}, |{}: &{}| {})", temp_decl, if temp_decl.ends_with(';') { " " } else { "" }, actual_node_id, param_name, type_name, field_access);
                                    } else {
                                        return format!("api.read_node({}, |{}: &{}| {})", node_id_expr, param_name, type_name, field_access);
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
                            
                            // Look up the variable to see if it's a node type
                            // Try multiple lookup strategies to handle variables in different scopes (including loop-scoped)
                            let node_type_opt = if let Some(current_func) = current_func {
                                // Strategy 1: Check in function locals first
                                current_func.locals.iter()
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
                                            let inferred = script.infer_expr_type(&val.expr, Some(current_func));
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
                                                    let inferred = script.infer_expr_type(&val.expr, Some(current_func));
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
                                    })
                                    // Strategy 3: Check in params
                                    .or_else(|| {
                                        current_func.params.iter()
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
                                            if let Some(inferred_type) = script.infer_expr_type(base, Some(current_func)) {
                                                if type_is_node(&inferred_type) {
                                                    return get_node_type(&inferred_type).cloned();
                                                }
                                            }
                                        }
                                        None
                                    })
                            } else {
                                // Check script-level variables
                                script.get_variable_type(original_var_name)
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
                                            if let Some(inferred_type) = script.infer_expr_type(base, None) {
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
                                let needs_clone = result_type.as_ref().map_or(false, |t| t.requires_clone());
                                let is_option = matches!(result_type.as_ref(), Some(Type::Option(_)));
                                
                                let param_name = original_var_name;
                                let resolved_field = ENGINE_REGISTRY.resolve_field_name(&node_type, field);
                                
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
                                
                                let (temp_decl, actual_node_id) = extract_mutable_api_call(&base_code);
                                if !temp_decl.is_empty() {
                                    return format!("{}{}api.read_node({}, |{}: &{}| {})", temp_decl, if temp_decl.ends_with(';') { " " } else { "" }, actual_node_id, param_name, node_type_name, field_access);
                                } else {
                                    return format!("api.read_node({}, |{}: &{}| {})", base_code, param_name, node_type_name, field_access);
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
                            // Get the node type name from the base type
                            let node_type_name = format!("{:?}", node_type);
                            
                            // Use api.read_node and check if cloning is needed
                            let base_node_type = Type::Node(node_type.clone());
                            let result_type = script.get_member_type(&base_node_type, field);
                            let needs_clone = result_type.as_ref().map_or(false, |t| t.requires_clone());
                            
                            // Check if the result type is Option<T> - if so, unwrap inside the closure
                            let is_option = matches!(result_type.as_ref(), Some(Type::Option(_)));
                            
                            // Extract variable name from node_id (e.g., "c_id" -> "c", "self.id" -> "self_node")
                            let param_name = if base_code.ends_with("_id") {
                                &base_code[..base_code.len() - 3]
                            } else if base_code == "self.id" {
                                "self_node"
                            } else {
                                "n"
                            };
                            
                            // Resolve field name (e.g., "texture" -> "texture_id")
                            let resolved_field = ENGINE_REGISTRY.resolve_field_name(&node_type, field);
                            
                            let field_access = if is_option {
                                format!("{}.{}.unwrap()", param_name, resolved_field)
                            } else if needs_clone {
                                format!("{}.{}.clone()", param_name, resolved_field)
                            } else {
                                format!("{}.{}", param_name, resolved_field)
                            };
                            
                            // Extract mutable API calls to temporary variables to avoid borrow checker issues
                            let (temp_decl, actual_node_id) = extract_mutable_api_call(&base_code);
                            if !temp_decl.is_empty() {
                                format!("{}{}api.read_node({}, |{}: &{}| {})", temp_decl, if temp_decl.ends_with(';') { " " } else { "" }, actual_node_id, param_name, node_type_name, field_access)
                            } else {
                                format!("api.read_node({}, |{}: &{}| {})", base_code, param_name, node_type_name, field_access)
                            }
                        } else {
                            format!("{}.{}", base_code, field)
                        }
                    }
                    Some(Type::DynNode) => {
                        // DynNode: generate match arms for all node types that have this field
                        let base_code = base.to_rust(needs_self, script, None, current_func, None);
                        let is_node_id_var = base_code.ends_with("_id") || base_code == "self.id";
                        
                        if is_node_id_var {
                            // Build field path from the expression (e.g., node.transform.position.x)
                            let mut field_path = vec![field.clone()];
                            let mut current_expr = base.as_ref();
                            while let Expr::MemberAccess(inner_base, inner_field) = current_expr {
                                field_path.push(inner_field.clone());
                                current_expr = inner_base.as_ref();
                            }
                            field_path.reverse(); // Now field_path is [node_base, transform, position, x]
                            
                            // Extract just the field path (skip the base identifier)
                            // For nested access like node.transform.position.x, we want [transform, position, x]
                            let field_path_only: Vec<String> = if field_path.len() > 1 {
                                field_path[1..].to_vec()
                            } else {
                                field_path.clone()
                            };
                            
                            // Find all node types that have this field path
                            let compatible_node_types = ENGINE_REGISTRY.narrow_nodes_by_fields(&field_path_only);
                            
                            if compatible_node_types.is_empty() {
                                // No compatible node types found, fallback to error or default behavior
                                format!("{}.{}", base_code, field)
                            } else {
                                // Generate match arms for all compatible node types
                                let mut match_arms = Vec::new();
                                for node_type in &compatible_node_types {
                                    let node_type_name = format!("{:?}", node_type);
                                    let _base_node_type = Type::Node(*node_type);
                                    
                                    // Resolve the full field path to get the result type
                                    let result_type = ENGINE_REGISTRY.resolve_chain_from_node(node_type, &field_path_only);
                                    let needs_clone = result_type.as_ref().map_or(false, |t| t.requires_clone());
                                    let is_option = matches!(result_type.as_ref(), Some(Type::Option(_)));
                                    
                                    let param_name = "n";
                                    // Resolve field names in the path (e.g., "texture" -> "texture_id")
                                    let resolved_path: Vec<String> = field_path_only.iter()
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
                                    
                                    match_arms.push(format!(
                                        "NodeType::{} => api.read_node({}, |{}: &{}| {})",
                                        node_type_name, base_code, param_name, node_type_name, field_access
                                    ));
                                }
                                
                                // Generate match expression
                                format!(
                                    "match api.get_type({}) {{ {} _ => panic!(\"Node type not compatible with field access: {}\") }}",
                                    base_code,
                                    match_arms.join(", "),
                                    field_path_only.join(".")
                                )
                            }
                        } else {
                            format!("{}.{}", base_code, field)
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
            Expr::EnumAccess(variant) => {
                match variant {
                    BuiltInEnumVariant::NodeType(node_type) => {
                        format!("NodeType::{:?}", node_type)
                    }
                }
            }
            Expr::Call(target, args) => {
                // Check for chained calls where an ApiCall returning Uuid is followed by
                // a NodeSugar API method that accepts Uuid as its first parameter
                if let Expr::MemberAccess(base, method) = target.as_ref() {
                    // Try to resolve the method as a NodeSugar API
                    if let Some(outer_api) = crate::lang::pup::api::PupNodeSugar::resolve_method(method) {
                        // Check if this API's first parameter is Uuid
                        if let Some(param_types) = outer_api.param_types() {
                            if let Some(first_param_type) = param_types.get(0) {
                                if matches!(first_param_type, Type::Uuid) {
                                    // Check if base is an ApiCall that returns Uuid, or a MemberAccess that should be treated as one
                                    let (inner_call_str, temp_var_name) = if let Expr::ApiCall(api, args) = base.as_ref() {
                                        // Direct ApiCall
                                        if let Some(return_type) = api.return_type() {
                                            if matches!(return_type, Type::Uuid) {
                                                let mut inner_call_str = api.to_rust(args, script, needs_self, current_func);
                                                // Fix any incorrect renaming of "api" to "__t_api" or "t_id_api"
                                                // The "api" identifier should NEVER be renamed - it's always the API parameter
                                                inner_call_str = inner_call_str.replace("__t_api.", "api.").replace("t_id_api.", "api.");
                                                let temp_var = match api {
                                                    ApiModule::NodeSugar(NodeSugarApi::GetParent) => "__parent_id",
                                                    ApiModule::NodeSugar(NodeSugarApi::GetChildByName) => "__child_id",
                                                    _ => "__temp_id",
                                                };
                                                (Some(inner_call_str), Some(temp_var.to_string()))
                                            } else {
                                                (None, None)
                                            }
                                        } else {
                                            (None, None)
                                        }
                                    } else if let Expr::MemberAccess(inner_base, inner_method) = base.as_ref() {
                                        // Handle nested MemberAccess like collision.get_parent()
                                        // Check if this is a NodeSugar API call
                                        if let Some(api) = crate::lang::pup::api::PupNodeSugar::resolve_method(inner_method) {
                                            if let Some(return_type) = api.return_type() {
                                                if matches!(return_type, Type::Uuid) {
                                                    // Create args for the inner API call - the base becomes the first arg
                                                    let inner_api_args = vec![*inner_base.clone()];
                                                    let mut inner_call_str = api.to_rust(&inner_api_args, script, needs_self, current_func);
                                                    // Fix any incorrect renaming of "api" to "__t_api" or "t_id_api"
                                                    // The "api" identifier should NEVER be renamed - it's always the API parameter
                                                    inner_call_str = inner_call_str.replace("__t_api.", "api.").replace("t_id_api.", "api.");
                                                    let temp_var = match api {
                                                        ApiModule::NodeSugar(NodeSugarApi::GetParent) => "__parent_id",
                                                        ApiModule::NodeSugar(NodeSugarApi::GetChildByName) => "__child_id",
                                                        _ => "__temp_id",
                                                    };
                                                    (Some(inner_call_str), Some(temp_var.to_string()))
                                                } else {
                                                    (None, None)
                                                }
                                            } else {
                                                (None, None)
                                            }
                                        } else {
                                            (None, None)
                                        }
                                    } else {
                                        (None, None)
                                    };
                                    
                                    if let (Some(inner_call_str), Some(temp_var)) = (inner_call_str, temp_var_name) {
                                        // This is a chained call: inner_api() returns Uuid,
                                        // and outer_api() accepts Uuid as first param
                                        // Both APIs require mutable borrows (all NodeSugar APIs take &mut self),
                                        // so we MUST extract the inner call to a temporary variable
                                        // to avoid borrow checker errors
                                        
                                        let temp_decl = format!("let {}: Uuid = {};", temp_var, inner_call_str);
                                        
                                        // Create an Ident expression for the temp variable
                                        let temp_var_expr = Expr::Ident(temp_var.clone());
                                        let outer_args = vec![temp_var_expr];
                                        
                                        // Generate the outer call with the temp variable as argument
                                        let outer_call = outer_api.to_rust(&outer_args, script, needs_self, current_func);
                                        
                                        // Combine temp declaration with outer call
                                        return format!("{}{}{}", temp_decl, if temp_decl.ends_with(';') { " " } else { "" }, outer_call);
                                    }
                                }
                            }
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

                let is_engine_method = matches!(target.as_ref(), Expr::MemberAccess(_base, _method))
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
                        let code = arg.to_rust(needs_self, script, expected_type, current_func, None);

                        // Ask the script context to infer the argument type
                        let arg_type = script.infer_expr_type(arg, current_func);

                        match (arg, &arg_type) {
                            // ----------------------------------------------------------
                            // 1️⃣ Literal values — simple by-value semantics
                            // ----------------------------------------------------------
                            (Expr::Literal(Literal::String(_)), _)
                            | (Expr::Literal(Literal::Interpolated(_)), _) => {
                                // Strings use owned String, so clone
                                format!("{}.clone()", code)
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

                // Convert the target expression (e.g., func or self.method)
                let mut target_str = target.to_rust(needs_self, script, None, current_func, None);

                // If this is a local user-defined function, prefix with `self.`
                if is_local_function && !target_str.starts_with("self.") {
                    target_str = format!("self.{}", func_name.unwrap());
                }

                // ==============================================================
                // Finally, build the Rust call string
                // Handles API injection and empty arg lists
                // ==============================================================
                if is_engine_method {
                    // ✅ Engine methods: just pass normal args
                    if args_rust.is_empty() {
                        format!("{}()", target_str)
                    } else {
                        format!("{}({})", target_str, args_rust.join(", "))
                    }
                } else if is_local_function {
                    // Local script functions: add api
                    if args_rust.is_empty() {
                        format!("{}(api);", target_str)
                    } else {
                        format!("{}({}, api);", target_str, args_rust.join(", "))
                    }
                } else {
                    // Fallback: treat as external function with api
                    if args_rust.is_empty() {
                        format!("{}(api);", target_str)
                    } else {
                        format!("{}({}, api);", target_str, args_rust.join(", "))
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
                                let v_final = if *expected_val_type == Type::Object
                                    || matches!(expected_val_type, Type::Custom(_))
                                {
                                    // For dynamic maps or custom types, wrap in json!()
                                    if Expr::should_clone_expr(&raw_v, v_expr, script, current_func)
                                    {
                                        format!("json!({}.clone())", raw_v)
                                    } else {
                                        format!("json!({})", raw_v)
                                    }
                                } else {
                                    // For typed maps, just clone if needed
                                    if Expr::should_clone_expr(&raw_v, v_expr, script, current_func)
                                    {
                                        format!("{}.clone()", raw_v)
                                    } else {
                                        raw_v
                                    }
                                };

                                format!("({}, {})", k_final, v_final)
                            })
                            .collect();

                        // Determine the correct HashMap type based on expected types
                        let final_code = if *expected_val_type == Type::Object
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

                    if matches!(expected_type, Some(Type::Object)) {
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
                                let rendered =
                                    e.to_rust(needs_self, script, Some(elem_ty), current_func, None);

                                // If this is a custom type array or any[]/object[] array, wrap each element in json!()
                                let final_rendered = match elem_ty {
                                    Type::Custom(_) | Type::Object => {
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

                    if matches!(expected_type, Some(Type::Object)) {
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

                    if matches!(expected_type, Some(Type::Object)) {
                        format!("json!({})", code)
                    } else {
                        code
                    }
                }
            },
            Expr::StructNew(ty, args) => {
                use std::collections::HashMap;

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
                                    let field_index = field_name.strip_prefix("_").and_then(|s| s.parse::<usize>().ok());
                                    if let Some(idx) = field_index {
                                        if let Some(def) = ENGINE_REGISTRY.struct_defs.get(&engine_struct) {
                                            def.fields.get(idx).map(|f| f.typ.clone())
                                        } else {
                                            None
                                        }
                                    } else {
                                        None
                                    }
                                } else {
                                    // Named argument - get field type by name
                                    ENGINE_REGISTRY.get_field_type_struct(&engine_struct, field_name)
                                }
                            })
                            .collect();
                        
                        // Now generate code with expected types
                        let arg_codes: Vec<String> = args
                            .iter()
                            .zip(expected_types.iter())
                            .map(|((_, expr), expected_type_opt)| {
                                // Pass expected type to expression codegen
                                expr.to_rust(needs_self, script, expected_type_opt.as_ref(), current_func, None)
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

                // --- Group by base name (if parent) ---
                let mut base_fields: HashMap<&str, Vec<(&str, &Type, &Expr)>> = HashMap::new();
                let mut derived_fields: Vec<(&str, &Type, &Expr)> = Vec::new();

                for (fname, fty, parent, expr) in &field_exprs {
                    if let Some(base_name) = parent {
                        base_fields
                            .entry(base_name)
                            .or_default()
                            .push((*fname, *fty, *expr));
                    } else {
                        derived_fields.push((*fname, *fty, *expr));
                    }
                }

                // --- Recursive builder for nested base init ---
                fn build_base_init(
                    base_name: &str,
                    base_fields: &HashMap<&str, Vec<(&str, &Type, &Expr)>>,
                    script: &Script,
                    needs_self: bool,
                    current_func: Option<&Function>,
                ) -> String {
                    let base_struct = script
                        .structs
                        .iter()
                        .find(|s| s.name == base_name)
                        .expect("Base struct not found");

                    let renamed_base_name = rename_struct(base_name);
                    let mut parts = String::new();

                    // Handle deeper bases first
                    if let Some(ref inner) = base_struct.base {
                        let inner_code =
                            build_base_init(inner, base_fields, script, needs_self, current_func);
                        parts.push_str(&format!("base: {}, ", inner_code));
                    }

                    // Write base's own fields
                    if let Some(local_fields) = base_fields.get(base_name) {
                        for (fname, fty, expr) in local_fields {
                            let mut expr_code =
                                expr.to_rust(needs_self, script, Some(fty), current_func, None);
                            let expr_type = script.infer_expr_type(expr, current_func);
                            let should_clone =
                                matches!(expr, Expr::Ident(_) | Expr::MemberAccess(..))
                                    && expr_type.as_ref().map_or(false, |ty| ty.requires_clone());
                            if should_clone {
                                expr_code = format!("{}.clone()", expr_code);
                            }
                            parts.push_str(&format!("{}: {}, ", fname, expr_code));
                        }
                    }

                    format!("{}::new({})", renamed_base_name, parts.trim_end_matches(", "))
                }

                // --- Build final top-level struct ---
                let mut code = String::new();

                // 1️⃣ Base (if exists)
                if let Some(ref base_name) = struct_def.base {
                    let base_code =
                        build_base_init(base_name, &base_fields, script, needs_self, current_func);
                    code.push_str(&format!("base: {}, ", base_code));
                }

                // 2️⃣ Derived-only fields
                for (fname, fty, expr) in &derived_fields {
                    let mut expr_code = expr.to_rust(needs_self, script, Some(fty), current_func, None);
                    let expr_type = script.infer_expr_type(expr, current_func);
                    let should_clone = matches!(expr, Expr::Ident(_) | Expr::MemberAccess(..))
                        && expr_type.as_ref().map_or(false, |ty| ty.requires_clone());
                    if should_clone {
                        expr_code = format!("{}.clone()", expr_code);
                    }
                    code.push_str(&format!("{}: {}, ", fname, expr_code));
                }

                // Use renamed struct name for custom structs (not node types or engine structs)
                let struct_name = if is_node_type(ty) || EngineStructKind::from_string(ty).is_some() {
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
                        if matches!(first_param_type, Type::Uuid) {
                            if let Some(first_arg) = args.get(0) {
                                // Check if first_arg is a Cast containing an ApiCall, or a direct ApiCall
                                let inner_api_call = if let Expr::Cast(inner_expr, _) = first_arg {
                                    if let Expr::ApiCall(inner_api, inner_args) = inner_expr.as_ref() {
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
                                        if matches!(return_type, Type::Uuid | Type::DynNode) || 
                                           matches!(return_type, Type::Option(boxed) if matches!(boxed.as_ref(), Type::Uuid)) {
                                            // Both APIs require mutable borrows - extract inner call to temp variable
                                            let mut inner_call_str = inner_api.to_rust(inner_args, script, needs_self, current_func);
                                            
                                            // Fix any incorrect renaming of "api" to "__t_api" or "t_id_api"
                                            // The "api" identifier should NEVER be renamed - it's always the API parameter
                                            inner_call_str = inner_call_str.replace("__t_api.", "api.").replace("t_id_api.", "api.");
                                            
                                            // Generate temp variable name based on inner API
                                            let temp_var = match inner_api {
                                                ApiModule::NodeSugar(NodeSugarApi::GetParent) => "__parent_id".to_string(),
                                                ApiModule::NodeSugar(NodeSugarApi::GetChildByName) => "__child_id".to_string(),
                                                _ => "__temp_id".to_string(),
                                            };
                                            
                                            temp_decl_opt = Some(format!("let {}: Uuid = {};", temp_var, inner_call_str));
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
                            let expected_ty_hint = expected_param_types.as_ref().and_then(|v| v.get(i));

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
                let api_call_code = module.to_rust(&api_call_args, script, needs_self, current_func);
                
                // If we have a temp declaration, prepend it
                if let Some(temp_decl) = &temp_decl_opt {
                    return format!("{}{}{}", temp_decl, if temp_decl.ends_with(';') { " " } else { "" }, api_call_code);
                }

                // If we have an expected_type and the API returns Object, cast the result
                // This handles cases like: let x: number = map.get("key");
                // BUT: Only apply cast if the map is actually dynamic (returns Value)
                // For static maps (e.g., HashMap<String, BigInt>), the API already returns the correct type
                if let Some(expected_ty) = expected_type {
                    let api_return_type = module.return_type();
                    if let Some(Type::Object) = api_return_type.as_ref() {
                        // Check if this is MapApi::Get and if the map is actually dynamic
                        let should_cast = if let ApiModule::MapOp(MapApi::Get) = module {
                            // For MapApi::Get, check if the map's value type is Object (dynamic)
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

                        if should_cast && *expected_ty != Type::Object {
                            // Generate cast from Value to expected type
                            match expected_ty {
                                Type::Number(NumberKind::Float(64)) => {
                                    format!("{}.as_f64().unwrap_or_default()", api_call_code)
                                }
                                Type::Number(NumberKind::Float(32)) => {
                                    format!("{}.as_f64().unwrap_or_default() as f32", api_call_code)
                                }
                                Type::Number(NumberKind::BigInt) => {
                                    // Value can be a string representation of BigInt or a number
                                    format!(
                                        "{}.as_str().and_then(|s| s.parse::<BigInt>().ok()).unwrap_or_else(|| BigInt::from({}.as_i64().unwrap_or_default()))",
                                        api_call_code, api_call_code
                                    )
                                }
                                Type::Number(NumberKind::Decimal) => {
                                    // Decimal is stored as f64 in Value, convert using from_str_exact
                                    format!(
                                        "rust_decimal::Decimal::from_str_exact(&{}.as_f64().unwrap_or_default().to_string()).unwrap_or_default()",
                                        api_call_code
                                    )
                                }
                                Type::String => {
                                    format!(
                                        "{}.as_str().unwrap_or_default().to_string()",
                                        api_call_code
                                    )
                                }
                                Type::Bool => {
                                    format!("{}.as_bool().unwrap_or_default()", api_call_code)
                                }
                                Type::Custom(custom_type) => {
                                    // Strip 'mut' if present in type name
                                    let clean_type = if custom_type.starts_with("mut ") {
                                        custom_type.strip_prefix("mut ").unwrap_or(custom_type)
                                    } else {
                                        custom_type
                                    };
                                    format!(
                                        "serde_json::from_value::<{}>({}).unwrap_or_default()",
                                        clean_type, api_call_code
                                    )
                                }
                                _ => api_call_code,
                            }
                        } else {
                            api_call_code
                        }
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
                let end_code =
                    end.to_rust(needs_self, script, end_expected_type.as_ref(), current_func, None); // end is Expr, no span available
                format!("({}..{})", start_code, end_code)
            }
            Expr::Cast(inner, target_type) => {
                // Special case: if inner is SelfAccess, ALWAYS return self.id - never store it
                if matches!(inner.as_ref(), Expr::SelfAccess) {
                    return "self.id".to_string();
                }
                
                let inner_type = script.infer_expr_type(inner, current_func);
                // Don't pass target_type as expected_type - let the literal be its natural type, then cast

                // Special case: ui_node.get_element("name") as UIText
                // Convert get_element to get_element_clone with the target type
                if let Expr::Call(target, args) = inner.as_ref() {
                    if let Expr::MemberAccess(base, method) = target.as_ref() {
                        if method == "get_element" && args.len() == 1 {
                            // This is get_element call being cast - convert to get_element_clone
                            let base_code = base.to_rust(needs_self, script, None, current_func, None);
                            let arg_code = args[0].to_rust(needs_self, script, None, current_func, None);
                            if let Type::Custom(type_name) = target_type {
                                return format!(
                                    "{}.get_element_clone::<{}>({})",
                                    base_code, type_name, arg_code
                                );
                            }
                        }
                    }
                }

                let mut inner_code = inner.to_rust(needs_self, script, None, current_func, None);
                
                // Special case: if inner_code is "self" or contains t_id_self, fix it to self.id
                inner_code = if inner_code == "self" || inner_code.starts_with("t_id_self") {
                    "self.id".to_string()
                } else {
                    inner_code
                };
                
                // Fix any incorrect renaming of "api" to "__t_api" or "t_id_api"
                // The "api" identifier should NEVER be renamed - it's always the API parameter
                inner_code = inner_code.replace("__t_api.", "api.").replace("t_id_api.", "api.");

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
                        if let Some(captured_str) = inner_code.strip_prefix("String::from(\"")
                            .and_then(|s| s.strip_suffix("\")")) {
                            format!("Cow::Borrowed(\"{}\")", captured_str)
                        } else {
                            format!("{}.into()", inner_code)
                        }
                    }
                    // Option<String> -> Option<CowStr>
                    (Some(Type::Option(inner_from)), Type::Option(inner_to)) 
                        if matches!(inner_from.as_ref(), Type::String) && matches!(inner_to.as_ref(), Type::CowStr) => {
                        // Optimize Some(String::from("...")) to Some(Cow::Borrowed("..."))
                        if let Some(captured_str) = inner_code.strip_prefix("Some(String::from(\"")
                            .and_then(|s| s.strip_suffix("\"))")) {
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
                    (Some(Type::Node(_)), Type::Uuid) => {
                        // Special case: if inner_code is "self" or contains "self", ensure it's self.id
                        if inner_code == "self" || (inner_code.starts_with("self") && !inner_code.contains("self.id")) {
                            "self.id".to_string()
                        } else if inner_code == "self.id" || inner_code.ends_with(".id") {
                            // Already self.id or ends with .id - no cast needed, it's already Uuid
                            inner_code
                        } else {
                            inner_code // Already a Uuid, no conversion needed
                        }
                    }
                    // Uuid -> Node type (for type checking, just pass through)
                    (Some(Type::Uuid), Type::Node(_)) => {
                        inner_code // Already a Uuid, no conversion needed
                    }
                    // T -> Option<T> conversions (wrapping in Some)
                    (Some(from), Type::Option(inner)) if from == inner.as_ref() => {
                        format!("Some({})", inner_code)
                    }
                    // UuidOption (Option<Uuid>) -> Uuid
                    // This is for get_child_by_name() which returns Option<Uuid>
                    (Some(Type::Custom(from_name)), Type::Uuid)
                        if from_name == "UuidOption" =>
                    {
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
                    // JSON Value (ContainerKind::Object) → Anything
                    // ==========================================================
                    (Some(Type::Object), target) => {
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
                                    ApiModule::NodeSugar(NodeSugarApi::GetChildByName),
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
                                    // Strip 'mut' if present in type name
                                    let clean_name = if name.starts_with("mut ") {
                                        name.strip_prefix("mut ").unwrap_or(name)
                                    } else {
                                        name
                                    };
                                    format!(
                                        "serde_json::from_value::<{}>({}.clone()).unwrap_or_default()",
                                        clean_name, inner_code
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

                    // UIElement (from get_element) to specific UI element type
                    // Pattern: ui_node.get_element("bob") as UIText
                    (Some(Type::Custom(from_name)), Type::Custom(to_name))
                        if from_name == "UIElement" =>
                    {
                        // Check if this is a get_element call being cast
                        // Convert to get_element_clone call
                        if inner_code.contains(".get_element(") {
                            // Replace .get_element( with .get_element_clone::<Type>(
                            let new_code = inner_code.replace(
                                ".get_element(",
                                &format!(".get_element_clone::<{}>(", to_name),
                            );
                            format!("{}", new_code)
                        } else {
                            // Fallback for other UIElement casts
                            format!("{}.clone()", inner_code)
                        }
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
                            // Strip 'mut' if present in type name
                            let clean_to_name = if to_name.starts_with("mut ") {
                                to_name.strip_prefix("mut ").unwrap_or(to_name)
                            } else {
                                to_name
                            };
                            format!(
                                "serde_json::from_value::<{}>(serde_json::to_value(&{}).unwrap_or_default()).unwrap_or_default()",
                                clean_to_name, cloned_code
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
                        format!(
                            "serde_json::from_value::<{}>(serde_json::to_value(&{}).unwrap_or_default()).unwrap_or_default()",
                            to_name, cloned_code
                        )
                    }

                    _ => {
                        // For non-primitive types, try .into() instead of as cast
                        // This handles String -> CowStr and other conversions
                        if matches!(target_type, Type::CowStr | Type::String | Type::Custom(_)) {
                            format!("{}.into()", inner_code)
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

                match base_type {
                    // ----------------------------------------------------------
                    // ✅ Typed HashMap<K,V>
                    // ----------------------------------------------------------
                    Some(Type::Container(ContainerKind::Map, ref inner_types)) => {
                        let key_ty = inner_types.get(0).unwrap_or(&Type::String);
                        // No need to re-infer key_code, already done above with correct type
                        let final_key_code = if *key_ty == Type::String {
                            // For String keys, convert the key to string if it's not already
                            let key_type = script.infer_expr_type(key, current_func);
                            if matches!(key_type, Some(Type::Number(_)) | Some(Type::Bool)) {
                                format!("{}.to_string().as_str()", key_code)
                            } else {
                                format!("{}.as_str()", key_code)
                            }
                        } else {
                            format!("&{}", key_code)
                        };
                        format!(
                            "{}.get({}).cloned().unwrap_or_default()",
                            base_code, final_key_code
                        )
                    }

                    // ----------------------------------------------------------
                    // ✅ Dynamic JSON object (serde_json::Value)
                    // ----------------------------------------------------------
                    Some(Type::Object) => {
                        // Produces a `Value`, good for later .as_* casts
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
                    // Invalid or unsupported index base
                    // ----------------------------------------------------------
                    Some(Type::Custom(_)) => "/* invalid index on struct */".to_string(),
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

