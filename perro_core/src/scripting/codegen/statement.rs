// Statement code generation
use crate::api_modules::*;
use crate::ast::*;
use crate::scripting::ast::{ContainerKind, Expr, NumberKind, Op, Stmt, Type};
use crate::node_registry::NodeType;
use crate::structs::engine_registry::ENGINE_REGISTRY;
use crate::structs::engine_structs::EngineStruct as EngineStructKind;
use super::utils::{is_node_type, rename_variable, string_to_node_type, TRANSPILED_IDENT};
use super::analysis::{collect_cloned_node_vars, extract_node_member_info};

impl Stmt {
    pub fn to_rust(
        &self,
        needs_self: bool,
        script: &Script,
        current_func: Option<&Function>,
    ) -> String {
        match self {
            Stmt::Expr(expr) => {
                // SECOND PASS: Extract nested API calls to avoid borrow checker issues
                // This handles cases like api.call_function_id(api.get_parent(collision_id), ...)
                // Only extracts NESTED API calls, not top-level ones
                let mut extracted_api_calls = Vec::new();
                let mut temp_var_types: std::collections::HashMap<String, Type> = std::collections::HashMap::new();
                
                // Generate a unique ID for this code generation session using UUID (no hyphens)
                let full_uuid = uuid::Uuid::new_v4().to_string().replace('-', "");
                let session_id = full_uuid[..12].to_string(); // First 12 hex chars (48 bits) from UUID without hyphens
                
                // Helper function to recursively extract nested API calls from an expression
                // is_top_level: true when processing the root expression, false when nested
                fn extract_all_nested_api_calls(
                    expr: &Expr,
                    script: &Script,
                    current_func: Option<&Function>,
                    extracted: &mut Vec<(String, String)>,
                    temp_var_types: &mut std::collections::HashMap<String, Type>,
                    needs_self: bool,
                    is_top_level: bool,
                    session_id: &str,
                ) -> Expr {
                    match expr {
                        // Extract API calls (like api.get_parent, api.call_function_id, etc.)
                        Expr::ApiCall(api_module, api_args) => {
                            // First, recursively extract nested API calls from arguments
                            let new_args: Vec<Expr> = api_args.iter()
                                .map(|arg| extract_all_nested_api_calls(arg, script, current_func, extracted, temp_var_types, needs_self, false, session_id))
                                .collect();
                            
                            // Check if this API call returns an ID type (Uuid, NodeType, etc.) that should be extracted
                            let should_extract_top_level = is_top_level && {
                                if let Some(return_type) = api_module.return_type() {
                                    matches!(return_type, Type::Uuid | Type::NodeType | Type::DynNode) ||
                                    matches!(return_type, Type::Option(boxed) if matches!(boxed.as_ref(), Type::Uuid))
                                } else {
                                    false
                                }
                            };
                            
                            // If this is a top-level API call that doesn't need extraction, just return it with processed arguments
                            if is_top_level && !should_extract_top_level {
                                return Expr::ApiCall(api_module.clone(), new_args);
                            }
                            
                            // Generate a unique UUID for this temp variable (very low collision chance)
                            let full_uuid = uuid::Uuid::new_v4().to_string().replace('-', "");
                            let unique_id = full_uuid[..12].to_string(); // First 12 hex chars (48 bits) from UUID without hyphens
                            
                            // Generate temp variable name using UUID for guaranteed uniqueness
                            let temp_var = format!("__temp_api_{}", unique_id);
                            
                            // Generate the API call code with extracted arguments
                            let mut api_call_str = api_module.to_rust(&new_args, script, needs_self, current_func);
                            
                            // Fix any incorrect renaming of "api" to "__t_api" or "t_id_api"
                            api_call_str = api_call_str.replace("__t_api.", "api.").replace("t_id_api.", "api.");
                            
                            // Infer the return type for the temp variable
                            let inferred_type = api_module.return_type();
                            let type_annotation = inferred_type
                                .as_ref()
                                .map(|t| format!(": {}", t.to_rust_type()))
                                .unwrap_or_default();
                            
                            // Store the type for this temp variable
                            if let Some(ty) = inferred_type {
                                temp_var_types.insert(temp_var.clone(), ty);
                            }
                            
                            extracted.push((format!("let {}{} = {};", temp_var, type_annotation, api_call_str), temp_var.clone()));
                            
                            // Return an identifier expression for the temp variable
                            Expr::Ident(temp_var)
                        }
                        Expr::Call(target, args) => {
                            // Recursively extract API calls from target and all arguments
                            let new_target = extract_all_nested_api_calls(target, script, current_func, extracted, temp_var_types, needs_self, false, session_id);
                            let new_args: Vec<Expr> = args.iter()
                                .map(|arg| extract_all_nested_api_calls(arg, script, current_func, extracted, temp_var_types, needs_self, false, session_id))
                                .collect();
                            Expr::Call(Box::new(new_target), new_args)
                        }
                        Expr::MemberAccess(base, field) => {
                            // Recursively extract API calls from base
                            let new_base = extract_all_nested_api_calls(base, script, current_func, extracted, temp_var_types, needs_self, false, session_id);
                            Expr::MemberAccess(Box::new(new_base), field.clone())
                        }
                        Expr::BinaryOp(left, op, right) => {
                            let new_left = extract_all_nested_api_calls(left, script, current_func, extracted, temp_var_types, needs_self, false, session_id);
                            let new_right = extract_all_nested_api_calls(right, script, current_func, extracted, temp_var_types, needs_self, false, session_id);
                            Expr::BinaryOp(Box::new(new_left), op.clone(), Box::new(new_right))
                        }
                        Expr::Cast(inner, target_type) => {
                            let new_inner = extract_all_nested_api_calls(inner, script, current_func, extracted, temp_var_types, needs_self, false, session_id);
                            Expr::Cast(Box::new(new_inner), target_type.clone())
                        }
                        Expr::Index(array, index) => {
                            let new_array = extract_all_nested_api_calls(array, script, current_func, extracted, temp_var_types, needs_self, false, session_id);
                            let new_index = extract_all_nested_api_calls(index, script, current_func, extracted, temp_var_types, needs_self, false, session_id);
                            Expr::Index(Box::new(new_array), Box::new(new_index))
                        }
                        _ => expr.clone(),
                    }
                }
                
                // Extract nested API calls from the expression (pass true for is_top_level)
                let modified_expr = extract_all_nested_api_calls(
                    &expr.expr,
                    script,
                    current_func,
                    &mut extracted_api_calls,
                    &mut temp_var_types,
                    needs_self,
                    true, // This is the top-level expression
                    &session_id,
                );
                
                // Generate the expression string from the modified expression
                let expr_str = modified_expr.to_rust(needs_self, script, None, current_func, None);
                
                // Combine all temp declarations on the same line
                let combined_temp_decl = if !extracted_api_calls.is_empty() {
                    Some(extracted_api_calls.iter().map(|(decl, _): &(String, String)| decl.clone()).collect::<Vec<_>>().join(" "))
                } else {
                    None
                };
                
                // Format the final statement with temp declarations on the same line
                if let Some(ref temp_decl) = combined_temp_decl {
                    if expr_str.trim().is_empty() {
                        format!("        {};\n", temp_decl)
                    } else if expr_str.trim_end().ends_with(';') {
                        format!("        {} {}\n", temp_decl, expr_str.trim())
                    } else {
                        format!("        {} {};\n", temp_decl, expr_str)
                    }
                } else {
                    // No temp declarations, use original formatting
                    if expr_str.trim().is_empty() {
                        String::new()
                    } else if expr_str.trim_end().ends_with(';') {
                        format!("        {}\n", expr_str)
                    } else {
                        format!("        {};\n", expr_str)
                    }
                }
            }

            Stmt::VariableDecl(var) => {
                let expr_str = if let Some(expr) = &var.value {
                    // Pass the variable's type as expected type so map/array literals know what to generate
                    let raw_expr =
                        expr.expr
                            .to_rust(needs_self, script, var.typ.as_ref(), current_func, expr.span.as_ref());

                    // Check if we need to clone based on both the expression type and variable type
                    let var_type_is_custom = var
                        .typ
                        .as_ref()
                        .map_or(false, |t| matches!(t, Type::Custom(_)));
                    let var_type_requires_clone =
                        var.typ.as_ref().map_or(false, |t| t.requires_clone());
                    let expr_type = script.infer_expr_type(&expr.expr, current_func);

                    // Check if the expression is an Ident that refers to a struct field (which will get self. prefix)
                    // OR if it's a Cast with an Ident inside that is a struct field
                    let is_struct_field_access = match &expr.expr {
                        Expr::Ident(name) => script.is_struct_field(name),
                        Expr::Cast(inner, _) => {
                            if let Expr::Ident(name) = inner.as_ref() {
                                script.is_struct_field(name)
                            } else {
                                false
                            }
                        }
                        Expr::MemberAccess(..) => true, // MemberAccess always needs checking
                        _ => false,
                    };

                    let needs_clone = if is_struct_field_access {
                        // Always clone struct field access when assigning to a custom type (to avoid move errors)
                        var_type_is_custom
                            || var_type_requires_clone
                            || expr_type.as_ref().map_or(false, |ty| ty.requires_clone())
                    } else if matches!(expr.expr, Expr::Ident(_)) {
                        // Clone if the expression type requires it, or if assigning to a custom type
                        expr_type.as_ref().map_or(var_type_requires_clone, |ty| {
                            ty.requires_clone() || var_type_requires_clone
                        })
                    } else {
                        false
                    };

                    // Also check if the generated code contains self. but doesn't have .clone() yet
                    // This handles cases where casts might be optimized away but we still need to clone
                    let needs_clone_fallback = if !needs_clone && var_type_is_custom {
                        raw_expr.contains("self.") && !raw_expr.contains(".clone()")
                    } else {
                        false
                    };

                    // Don't clone if the expression already produces an owned value (e.g., from unwrap_or_default, from_str, etc.)
                    // This is important for generic functions like FromPrimitive::from_f32 where cloning breaks type inference
                    // Also don't clone if the expression already produces an owned value
                    // Note: read_node now returns Clone types, and for non-Copy types the .clone() inside the closure
                    // already produces an owned value, so read_node calls don't need an extra .clone()
                    let already_owned = raw_expr.contains(".unwrap_or_default()")
                        || raw_expr.contains(".unwrap()")
                        || raw_expr.contains("::from_str")
                        || raw_expr.contains("::from(")
                        || raw_expr.contains("::new(")
                        || raw_expr.contains("get_element_clone")
                        || raw_expr.contains("read_node(");

                    if (needs_clone || needs_clone_fallback) && !already_owned {
                        format!("{}.clone()", raw_expr)
                    } else {
                        raw_expr
                    }
                } else if var.typ.is_some() {
                    var.default_value()
                } else {
                    String::new()
                };

                // Check if the expression contains a temporary variable extraction for mutable API calls
                // Pattern: "let __parent_id = api.get_parent(...); api.read_node(...)" or "api.get_type(...)" etc.
                let (temp_stmt, final_expr_str) = if expr_str.contains("let __") && (expr_str.contains("; api.") || expr_str.contains(";api.")) {
                    // Extract the temporary variable declaration and the actual expression
                    // Look for any API call after the temp declaration
                    let semi_pos = expr_str.find("; api.")
                        .or_else(|| expr_str.find(";api."));
                    if let Some(pos) = semi_pos {
                        let temp_decl = expr_str[..pos + 1].trim_start().to_string();
                        // Skip "; " or ";"
                        let actual_expr = if expr_str.as_bytes().get(pos + 1) == Some(&b' ') {
                            &expr_str[pos + 2..]
                        } else {
                            &expr_str[pos + 1..]
                        };
                        (Some(temp_decl), actual_expr.to_string())
                    } else {
                        (None, expr_str.clone())
                    }
                } else {
                    (None, expr_str.clone())
                };

                // Add type annotation if variable has explicit type OR if we can infer from the expression
                let inferred_type = if let Some(expr) = &var.value {
                    script.infer_expr_type(&expr.expr, current_func)
                } else {
                    None
                };
                
                // Helper to convert type to Rust type annotation
                // Special case: Texture (EngineStruct) becomes Option<Uuid> in Rust
                let type_to_rust_annotation = |typ: &Type| -> String {
                    match typ {
                        Type::EngineStruct(EngineStructKind::Texture) => "Option<Uuid>".to_string(),
                        _ => typ.to_rust_type(),
                    }
                };
                
                let type_annotation = if let Some(typ) = &var.typ {
                    format!(": {}", type_to_rust_annotation(typ))
                } else if let Some(ref inferred) = inferred_type {
                    format!(": {}", type_to_rust_annotation(inferred))
                } else {
                    String::new()
                };

                // Use inferred type for renaming if var.typ is None
                let type_for_renaming = var.typ.as_ref().or(inferred_type.as_ref());
                let renamed_name = rename_variable(&var.name, type_for_renaming);
                
                // If we extracted a temporary statement, prepend it on the same line
                if let Some(ref temp_stmt) = temp_stmt {
                    if expr_str.is_empty() {
                        format!("        {} let mut {}{};\n", temp_stmt.trim_end(), renamed_name, type_annotation)
                    } else {
                        format!(
                            "        {} let mut {}{} = {};\n",
                            temp_stmt.trim_end(), renamed_name, type_annotation, final_expr_str
                        )
                    }
                } else if expr_str.is_empty() {
                    format!("        let mut {}{};\n", renamed_name, type_annotation)
                } else {
                    format!(
                        "        let mut {}{} = {};\n",
                        renamed_name, type_annotation, final_expr_str
                    )
                }
            }
            Stmt::Assign(name, expr) => {
                let var_type = script.get_variable_type(name);
                let expr_type = script.infer_expr_type(&expr.expr, current_func);
                
                // Check if the expression returns a UUID that represents a node or texture
                // (e.g., get_parent(), get_child_by_name(), Texture.load(), casts to node types, etc.)
                // OR if it returns NodeType or DynNode (which are also node UUID types)
                let is_direct_node_call = matches!(&expr.expr, 
                    Expr::ApiCall(ApiModule::NodeSugar(NodeSugarApi::GetParent), _) |
                    Expr::ApiCall(ApiModule::NodeSugar(NodeSugarApi::GetChildByName), _)
                );
                
                let is_direct_texture_call = matches!(&expr.expr,
                    Expr::ApiCall(ApiModule::Texture(TextureApi::Load), _) |
                    Expr::ApiCall(ApiModule::Texture(TextureApi::CreateFromBytes), _)
                );
                
                let is_node_cast = matches!(expr_type, Some(Type::Uuid)) && 
                    if let Expr::Cast(_, ref target_type) = expr.expr {
                        match target_type {
                            Type::Node(_) => true,
                            Type::Custom(tn) => is_node_type(tn),
                            _ => false,
                        }
                    } else {
                        false
                    };
                
                let is_id_uuid = is_direct_node_call || is_direct_texture_call || is_node_cast;
                
                // Check if the return type is NodeType or DynNode (from get_type(), etc.)
                let is_node_type_return = matches!(expr_type, Some(Type::NodeType | Type::DynNode));
                
                // If it's a UUID/Option<Uuid> representing a node/texture, or returns NodeType/DynNode, use _id suffix naming
                // This follows the same pattern as nodes: check both var_type and expr_type for Uuid/Option<Uuid>
                let is_id_type = matches!(var_type, Some(Type::Uuid)) 
                    || matches!(expr_type.as_ref(), Some(Type::Uuid)) 
                    || matches!(expr_type.as_ref(), Some(Type::Option(boxed)) if matches!(boxed.as_ref(), Type::Uuid));
                
                let type_for_renaming = if is_id_uuid && is_id_type {
                    // For node calls returning Uuid, treat as node type for naming
                    // For texture calls returning Texture (EngineStruct), use the actual type
                    if is_direct_texture_call {
                        expr_type.as_ref().or(var_type)
                    } else {
                        Some(&Type::Node(NodeType::Node)) // Treat as node type for naming
                    }
                } else if is_node_type_return || matches!(var_type, Some(Type::NodeType | Type::DynNode)) {
                    // Use the actual type (NodeType or DynNode) for naming
                    expr_type.as_ref().or(var_type)
                } else {
                    var_type
                };
                
                let renamed_name = rename_variable(name, type_for_renaming);
                let target = if script.is_struct_field(name) && !name.starts_with("self.") {
                    format!("self.{}", renamed_name)
                } else {
                    renamed_name
                };

                let target_type = self.get_target_type(name, script, current_func);

                // FIRST: Check for nested API calls at AST level BEFORE generating the string
                // This ensures we extract temp variables correctly and api is never renamed
                // Generate a unique ID for this code generation session using UUID (no hyphens)
                let full_uuid = uuid::Uuid::new_v4().to_string().replace('-', "");
                let session_id = full_uuid[..12].to_string(); // First 12 hex chars (48 bits) from UUID without hyphens
                
                let (temp_decl_opt, modified_expr) = match &expr.expr {
                    Expr::ApiCall(outer_api, outer_args) => {
                        // Check if any argument is itself an API call that returns Uuid (or wrapped in a Cast)
                        let mut temp_decls = Vec::new();
                        let mut new_args = Vec::new();
                        let mut has_nested = false;
                        
                        for arg in outer_args.iter() {
                            // Check if arg is a Cast containing an ApiCall
                            let inner_api_call = if let Expr::Cast(inner_expr, _) = arg {
                                if let Expr::ApiCall(inner_api, inner_args) = inner_expr.as_ref() {
                                    Some((inner_api, inner_args))
                                } else {
                                    None
                                }
                            } else if let Expr::ApiCall(inner_api, inner_args) = arg {
                                Some((inner_api, inner_args))
                            } else {
                                None
                            };
                            
                            if let Some((inner_api, inner_args)) = inner_api_call {
                                if let Some(return_type) = inner_api.return_type() {
                                    // Check if it returns Uuid, DynNode, or Option<Uuid> (all need extraction)
                                    let needs_extraction = matches!(return_type, Type::Uuid | Type::DynNode) || 
                                        matches!(return_type, Type::Option(ref boxed) if matches!(boxed.as_ref(), Type::Uuid));
                                    
                                    if needs_extraction {
                                        has_nested = true;
                                        
                                        // Generate the inner call string - this should generate "api.get_parent(...)"
                                        let mut inner_call_str = inner_api.to_rust(inner_args, script, needs_self, current_func);
                                        
                                        // Fix any incorrect renaming of "api" to "__t_api" or "t_id_api"
                                        // The "api" identifier should NEVER be renamed - it's always the API parameter
                                        inner_call_str = inner_call_str.replace("__t_api.", "api.").replace("t_id_api.", "api.");
                                        
                                        // Generate a unique UUID for this temp variable (very low collision chance)
                                        let unique_id = uuid::Uuid::new_v4().simple().to_string().replace('-', "").chars().take(12).collect::<String>(); // First 12 hex chars (48 bits) from UUID without hyphens
                                        let temp_var = format!("__temp_api_{}", unique_id);
                                        
                                        // Only add temp declaration if we haven't seen this temp var yet
                                        if !temp_decls.iter().any(|(var, _)| *var == temp_var) {
                                            let type_annotation = if matches!(return_type, Type::Uuid) {
                                                ": Uuid"
                                            } else if matches!(return_type, Type::Option(ref boxed) if matches!(boxed.as_ref(), Type::Uuid)) {
                                                ": Option<Uuid>"
                                            } else {
                                                ""
                                            };
                                            temp_decls.push((temp_var.clone(), format!("let {}{} = {};", temp_var, type_annotation, inner_call_str)));
                                        }
                                        
                                        // Replace the nested call with a temp variable identifier
                                        // If the original was a Cast, we don't need the cast anymore since we're extracting to a temp var
                                        // The temp var is already a Uuid, so we can use it directly
                                        new_args.push(Expr::Ident(temp_var));
                                    } else {
                                        new_args.push(arg.clone());
                                    }
                                } else {
                                    new_args.push(arg.clone());
                                }
                            } else {
                                new_args.push(arg.clone());
                            }
                        }
                        
                        if has_nested && !temp_decls.is_empty() {
                            // Create a new expression with temp variables replaced
                            let new_expr = Expr::ApiCall(outer_api.clone(), new_args);
                            // Join temp declarations with spaces to put them on the same line
                            let all_temp_decls = temp_decls.iter().map(|(_, decl)| decl.clone()).collect::<Vec<_>>().join(" ");
                            (Some(all_temp_decls), Some(new_expr))
                        } else {
                            (None, None)
                        }
                    }
                    Expr::Call(target, call_args) => {
                        // Handle calls like api.read_node(api.get_parent(...), ...)
                        // Check if any argument is an API call that returns Uuid
                        let mut temp_decls = Vec::new();
                        let mut new_call_args = Vec::new();
                        let mut has_nested = false;
                        
                        for arg in call_args.iter() {
                            if let Expr::ApiCall(inner_api, inner_args) = arg {
                                if let Some(return_type) = inner_api.return_type() {
                                    // Check if it returns Uuid, DynNode, or Option<Uuid> (all need extraction)
                                    let needs_extraction = matches!(return_type, Type::Uuid | Type::DynNode) || 
                                        matches!(return_type, Type::Option(ref boxed) if matches!(boxed.as_ref(), Type::Uuid));
                                    
                                    if needs_extraction {
                                        has_nested = true;
                                        
                                        // Generate the inner call string - this should generate "api.get_parent(...)"
                                        let mut inner_call_str = inner_api.to_rust(inner_args, script, needs_self, current_func);
                                        
                                        // Fix any incorrect renaming of "api" to "__t_api" or "t_id_api"
                                        // The "api" identifier should NEVER be renamed - it's always the API parameter
                                        inner_call_str = inner_call_str.replace("__t_api.", "api.").replace("t_id_api.", "api.");
                                        
                                        // Generate a unique UUID for this temp variable (very low collision chance)
                                        let unique_id = uuid::Uuid::new_v4().simple().to_string().replace('-', "").chars().take(12).collect::<String>(); // First 12 hex chars (48 bits) from UUID without hyphens
                                        let temp_var = format!("__temp_api_{}", unique_id);
                                        
                                        // Only add temp declaration if we haven't seen this temp var yet
                                        if !temp_decls.iter().any(|(var, _)| *var == temp_var) {
                                            let type_annotation = if matches!(return_type, Type::Uuid) {
                                                ": Uuid"
                                            } else if matches!(return_type, Type::Option(ref boxed) if matches!(boxed.as_ref(), Type::Uuid)) {
                                                ": Option<Uuid>"
                                            } else {
                                                ""
                                            };
                                            temp_decls.push((temp_var.clone(), format!("let {}{} = {};", temp_var, type_annotation, inner_call_str)));
                                        }
                                        
                                        // Replace the nested call with a temp variable identifier
                                        new_call_args.push(Expr::Ident(temp_var));
                                    } else {
                                        new_call_args.push(arg.clone());
                                    }
                                } else {
                                    new_call_args.push(arg.clone());
                                }
                            } else {
                                new_call_args.push(arg.clone());
                            }
                        }
                        
                        if has_nested && !temp_decls.is_empty() {
                            // Create a new expression with temp variables replaced
                            let new_expr = Expr::Call(target.clone(), new_call_args);
                            // Join temp declarations with spaces to put them on the same line
                            let all_temp_decls = temp_decls.iter().map(|(_, decl)| decl.clone()).collect::<Vec<_>>().join(" ");
                            (Some(all_temp_decls), Some(new_expr))
                        } else {
                            (None, None)
                        }
                    }
                    _ => (None, None)
                };
                
                // Generate the expression string - use modified expression if we have one, otherwise use original
                let expr_str = if let Some(ref modified) = modified_expr {
                    modified.to_rust(needs_self, script, target_type.as_ref(), current_func, None)
                } else {
                    expr.expr.to_rust(needs_self, script, target_type.as_ref(), current_func, expr.span.as_ref())
                };
                
                // If we didn't catch it at AST level, try string-based detection as fallback
                let (temp_decl_opt, mut final_expr_str) = if temp_decl_opt.is_none() {
                    // Check if the expression string already contains an embedded temp declaration
                    // Pattern: "let __parent_id = api.get_parent(...); api.read_node(...)"
                    // or "let __parent_id: Uuid = api.get_parent(...); api.read_node(...)"
                    if expr_str.starts_with("let __") && (expr_str.contains("; api.") || expr_str.contains(";api.")) {
                        // Extract the temp declaration and the actual expression
                        let semi_pos = expr_str.find("; api.")
                            .or_else(|| expr_str.find(";api."));
                        if let Some(pos) = semi_pos {
                            // Extract temp declaration without leading spaces
                            let temp_decl = expr_str[..pos + 1].trim_start().to_string();
                            // Skip "; " or ";"
                            let actual_expr = if expr_str.as_bytes().get(pos + 1) == Some(&b' ') {
                                &expr_str[pos + 2..]
                            } else {
                                &expr_str[pos + 1..]
                            };
                            (Some(temp_decl), actual_expr.to_string())
                        } else {
                            (None, expr_str)
                        }
                    }
                    // For non-API-call expressions, use string-based detection
                    // Check for both "api.get_parent(" and "t_id_api.get_parent(" (in case api was renamed)
                    else if (expr_str.contains("api.get_parent(") || expr_str.contains("api.get_child_by_name(") || 
                                     expr_str.contains("t_id_api.get_parent(") || expr_str.contains("t_id_api.get_child_by_name(")) &&
                                    (expr_str.matches("api.").count() > 1 || expr_str.matches("t_id_api.").count() > 0) {
                        // Find the inner API call - check for both "api.get_parent(" and "t_id_api.get_parent("
                        let inner_start = expr_str.find("api.get_parent(")
                            .or_else(|| expr_str.find("api.get_child_by_name("))
                            .or_else(|| expr_str.find("t_id_api.get_parent("))
                            .or_else(|| expr_str.find("t_id_api.get_child_by_name("));
                        
                        if let Some(start) = inner_start {
                            // Find the matching closing parenthesis for the inner call
                            let mut depth = 0;
                            let mut end = start;
                            for (i, ch) in expr_str[start..].char_indices() {
                                if ch == '(' {
                                    depth += 1;
                                } else if ch == ')' {
                                    depth -= 1;
                                    if depth == 0 {
                                        end = start + i + 1;
                                        break;
                                    }
                                }
                            }
                            
                            let inner_call = &expr_str[start..end];
                            // Check if this inner call is already a temp variable (avoid redeclaration)
                            if inner_call.starts_with("__temp_api_") && !inner_call.contains("(") {
                                // It's already a temp variable, don't redeclare
                                (None, expr_str)
                            } else {
                                // Generate a unique UUID for this temp variable (guaranteed no collisions)
                                let unique_id = uuid::Uuid::new_v4().simple().to_string()[..12].to_string(); // First 12 hex chars (48 bits) from UUID without hyphens
                                let temp_var = format!("__temp_api_{}", unique_id);
                                
                                // Fix the inner call - replace any incorrect renaming of "api" back to "api"
                                // The "api" identifier should NEVER be renamed - it's always the API parameter
                                let fixed_inner_call = inner_call.replace("__t_api.", "api.").replace("t_id_api.", "api.");
                                
                                // Check if we're trying to assign temp_var to itself
                                if fixed_inner_call == temp_var {
                                    (None, expr_str)
                                } else if expr_str.contains(&format!("let {} =", temp_var)) {
                                    // Already declared earlier, just replace the inner call
                                    let final_expr = expr_str.replace(inner_call, &temp_var);
                                    (None, final_expr)
                                } else {
                                    // Determine type annotation based on the call
                                    let type_annotation = if fixed_inner_call.contains("get_parent") || fixed_inner_call.contains("get_child_by_name") {
                                        ": Uuid"
                                    } else {
                                        ""
                                    };
                                    let temp_decl = format!("let {}{} = {};", temp_var, type_annotation, fixed_inner_call);
                                    let final_expr = expr_str.replace(inner_call, &temp_var);
                                    (Some(temp_decl), final_expr)
                                }
                            }
                        } else {
                            (None, expr_str)
                        }
                    } else {
                        (None, expr_str)
                    }
                } else {
                    (temp_decl_opt, expr_str)
                };

                // Clone if:
                // 1. Expression type requires clone (BigInt, Decimal, String, etc.)
                // 2. OR if it's a MemberAccess and target type is a custom type (to avoid move errors)
                let should_clone = if matches!(expr.expr, Expr::Ident(_) | Expr::MemberAccess(..)) {
                    let expr_requires_clone =
                        expr_type.as_ref().map_or(false, |ty| ty.requires_clone());
                    let target_is_custom = target_type
                        .as_ref()
                        .map_or(false, |t| matches!(t, Type::Custom(_)));
                    let is_member_access = matches!(expr.expr, Expr::MemberAccess(..));
                    expr_requires_clone || (is_member_access && target_is_custom)
                } else {
                    false
                };

                if should_clone {
                    final_expr_str = format!("{}.clone()", final_expr_str);
                }

                let final_expr = if let Some(target_type) = &target_type {
                    if let Some(expr_type) = &expr_type {
                        if expr_type.can_implicitly_convert_to(target_type)
                            && expr_type != target_type
                        {
                            script.generate_implicit_cast_for_expr(
                                &final_expr_str,
                                expr_type,
                                target_type,
                            )
                        } else {
                            final_expr_str
                        }
                    } else {
                        final_expr_str
                    }
                } else {
                    final_expr_str
                };

                // If we have a temp declaration, prepend it before the assignment on the same line
                if let Some(temp_decl) = temp_decl_opt {
                    format!("        {} {} = {};\n", temp_decl, target, final_expr)
                } else {
                    format!("        {} = {};\n", target, final_expr)
                }
            }

            Stmt::AssignOp(name, op, expr) => {
                let var_type = script.get_variable_type(name);
                let expr_type = script.infer_expr_type(&expr.expr, current_func);
                
                // Check if the expression returns a UUID that represents a node or texture
                let is_direct_node_call = matches!(&expr.expr, 
                    Expr::ApiCall(ApiModule::NodeSugar(NodeSugarApi::GetParent), _) |
                    Expr::ApiCall(ApiModule::NodeSugar(NodeSugarApi::GetChildByName), _)
                );
                
                let is_direct_texture_call = matches!(&expr.expr,
                    Expr::ApiCall(ApiModule::Texture(TextureApi::Load), _) |
                    Expr::ApiCall(ApiModule::Texture(TextureApi::CreateFromBytes), _)
                );
                
                let is_node_cast = matches!(expr_type, Some(Type::Uuid)) && 
                    if let Expr::Cast(_, ref target_type) = expr.expr {
                        match target_type {
                            Type::Node(_) => true,
                            Type::Custom(tn) => is_node_type(tn),
                            _ => false,
                        }
                    } else {
                        false
                    };
                
                let is_id_uuid = is_direct_node_call || is_direct_texture_call || is_node_cast;
                
                // Check if the return type is NodeType or DynNode
                let is_node_type_return = matches!(expr_type, Some(Type::NodeType | Type::DynNode));
                
                // Determine type for renaming (same logic as Assign)
                let is_id_type = matches!(var_type, Some(Type::Uuid)) 
                    || matches!(expr_type.as_ref(), Some(Type::Uuid)) 
                    || matches!(expr_type.as_ref(), Some(Type::Option(boxed)) if matches!(boxed.as_ref(), Type::Uuid));
                
                let type_for_renaming = if is_id_uuid && is_id_type {
                    if is_direct_texture_call {
                        expr_type.as_ref().or(var_type)
                    } else {
                        Some(&Type::Node(NodeType::Node))
                    }
                } else if is_node_type_return || matches!(var_type, Some(Type::NodeType | Type::DynNode)) {
                    expr_type.as_ref().or(var_type)
                } else {
                    var_type
                };
                
                let renamed_name = rename_variable(name, type_for_renaming);
                let target = if script.is_struct_field(name) && !name.starts_with("self.") {
                    format!("self.{}", renamed_name)
                } else {
                    renamed_name
                };

                let target_type = self.get_target_type(name, script, current_func);
                let expr_str =
                    expr.expr
                        .to_rust(needs_self, script, target_type.as_ref(), current_func, expr.span.as_ref());

                if matches!(op, Op::Add) && target_type == Some(Type::String) {
                    return format!("        {target}.push_str({expr_str}.as_str());\n");
                }

                if let Some(target_type) = &target_type {
                    let expr_type = script.infer_expr_type(&expr.expr, current_func);
                    if let Some(expr_type) = expr_type {
                        let cast_expr = if expr_type.can_implicitly_convert_to(target_type)
                            && &expr_type != target_type
                        {
                            Self::generate_implicit_cast(&expr_str, &expr_type, target_type)
                        } else {
                            expr_str
                        };
                        // For Decimal AddAssign, ensure the expression is clearly typed as owned Decimal
                        let final_expr = if *target_type == Type::Number(NumberKind::Decimal)
                            && matches!(op, Op::Add)
                        {
                            // Use a block with explicit type to help compiler choose AddAssign<Decimal> impl
                            format!("{{ let tmp: Decimal = {}; tmp }}", cast_expr)
                        } else {
                            cast_expr
                        };
                        format!(
                            "        {} {}= {};\n",
                            target,
                            op.to_rust_assign(),
                            final_expr
                        )
                    } else {
                        format!(
                            "        {} {}= {};\n",
                            target,
                            op.to_rust_assign(),
                            expr_str
                        )
                    }
                } else {
                    format!(
                        "        {} {}= {};\n",
                        target,
                        op.to_rust_assign(),
                        expr_str
                    )
                }
            }

            Stmt::MemberAssign(lhs_expr, rhs_expr) => {
                // Check if this is a node member assignment (like self.transform.position.x = value)
                if let Some((node_id, node_type, field_path, closure_var)) = 
                    extract_node_member_info(&lhs_expr.expr, script, current_func) 
                {
                    // Clean closure_var (remove self. prefix) and ensure node_id has self. prefix
                    let clean_closure_var = closure_var.strip_prefix("self.").unwrap_or(&closure_var);
                    let node_id_with_self = if !node_id.starts_with("self.") && !node_id.starts_with("api.") {
                        format!("self.{}", node_id)
                    } else {
                        node_id.clone()
                    };
                    // Check if this is a DynNode (special marker)
                    if node_type == "__DYN_NODE__" {
                        // Build field path from the expression
                        let mut field_path_vec = vec![];
                        let mut current_expr = &lhs_expr.expr;
                        while let Expr::MemberAccess(inner_base, inner_field) = current_expr {
                            field_path_vec.push(inner_field.clone());
                            current_expr = inner_base.as_ref();
                        }
                        field_path_vec.reverse();
                        
                        // Find all node types that have this field path
                        let compatible_node_types = ENGINE_REGISTRY.narrow_nodes_by_fields(&field_path_vec);
                        
                        if compatible_node_types.is_empty() {
                            // No compatible node types found, fallback to error
                            format!("        // ERROR: No compatible node types found for field path: {}\n", field_path)
                        } else {
                            // Generate RHS code once
                            let lhs_type = script.infer_expr_type(&lhs_expr.expr, current_func);
                            let rhs_type = script.infer_expr_type(&rhs_expr.expr, current_func);
                            
                            // Extract ALL API calls from RHS expression to avoid borrow checker issues
                            // API calls inside mutate_node closures need to be extracted before the closure
                            let mut extracted_api_calls = Vec::new();
                            let mut temp_var_types: std::collections::HashMap<String, Type> = std::collections::HashMap::new();
                            
                            // Generate a unique ID for this code generation session using UUID
                            let session_id = uuid::Uuid::new_v4().simple().to_string()[..12].to_string(); // First 12 hex chars (48 bits) from UUID without hyphens
                            
                            // Helper function to extract API calls from expressions
                            fn extract_api_calls_from_expr_helper(expr: &Expr, script: &Script, current_func: Option<&Function>, 
                                                       extracted: &mut Vec<(String, String)>,
                                                       temp_var_types: &mut std::collections::HashMap<String, Type>,
                                                       needs_self: bool, expected_type: Option<&Type>,
                                                       session_id: &str) -> Expr {
                                match expr {
                                    // Extract API calls (like Math.random_range, Texture.load, etc.)
                                    Expr::ApiCall(api_module, api_args) => {
                                        // First, recursively extract nested API calls from arguments
                                        let new_args: Vec<Expr> = api_args.iter()
                                            .map(|arg| extract_api_calls_from_expr_helper(arg, script, current_func, extracted, temp_var_types, needs_self, None, session_id))
                                            .collect();
                                        
                                        // Generate a unique UUID for this temp variable (very low collision chance)
                                        let unique_id = uuid::Uuid::new_v4().simple().to_string().replace('-', "").chars().take(12).collect::<String>(); // First 12 hex chars (48 bits) from UUID without hyphens
                                        
                                        // Extract ALL API calls, not just ones returning Uuid
                                        // This prevents borrow checker issues when API calls are inside closures
                                        let temp_var = format!("__temp_api_{}", unique_id);
                                        
                                        // Generate the API call code with extracted arguments
                                        let mut api_call_str = api_module.to_rust(&new_args, script, needs_self, current_func);
                                        
                                        // Fix any incorrect renaming of "api" to "__t_api" or "t_id_api"
                                        api_call_str = api_call_str.replace("__t_api.", "api.").replace("t_id_api.", "api.");
                                        
                                        // Infer the return type for the temp variable
                                        let inferred_type = api_module.return_type();
                                        let type_annotation = inferred_type
                                            .as_ref()
                                            .map(|t| format!(": {}", t.to_rust_type()))
                                            .unwrap_or_default();
                                        
                                        // Store the type for this temp variable
                                        if let Some(ty) = inferred_type {
                                            temp_var_types.insert(temp_var.clone(), ty);
                                        }
                                        
                                        extracted.push((format!("let {}{} = {};", temp_var, type_annotation, api_call_str), temp_var.clone()));
                                        
                                        // Return an identifier expression for the temp variable
                                        Expr::Ident(temp_var)
                                    }
                                    Expr::MemberAccess(base, field) => {
                                        // First, recursively extract API calls from base
                                        let new_base = extract_api_calls_from_expr_helper(base, script, current_func, extracted, temp_var_types, needs_self, None, session_id);
                                        
                                        // Check if this member access would generate a read_node call
                                        let test_expr = Expr::MemberAccess(Box::new(new_base.clone()), field.clone());
                                        if let Some((_node_id, _, _, _)) = extract_node_member_info(&test_expr, script, current_func) {
                                            // This is a node member access - extract it to a temp variable
                                            // Generate a unique UUID for this temp variable (very low collision chance)
                                            let full_uuid = uuid::Uuid::new_v4().to_string().replace('-', "");
                                            let unique_id = full_uuid[..12].to_string(); // First 12 hex chars (48 bits) from UUID without hyphens
                                            let temp_var = format!("__temp_read_{}", unique_id);
                                            
                                            // Generate the read_node call
                                            let read_code = test_expr.to_rust(needs_self, script, expected_type, current_func, None);
                                            
                                            // Infer the type for the temp variable
                                            let inferred_type = script.infer_expr_type(&test_expr, current_func);
                                            let type_annotation = inferred_type
                                                .as_ref()
                                                .map(|t| format!(": {}", t.to_rust_type()))
                                                .unwrap_or_default();
                                            
                                            // Store the type for this temp variable so we can check if it needs cloning
                                            if let Some(ty) = inferred_type {
                                                temp_var_types.insert(temp_var.clone(), ty);
                                            }
                                            
                                            extracted.push((format!("let {}{} = {};", temp_var, type_annotation, read_code), temp_var.clone()));
                                            
                                            // Return an identifier expression for the temp variable
                                            Expr::Ident(temp_var)
                                        } else {
                                            // Not a node member access, return the member access with processed base
                                            Expr::MemberAccess(Box::new(new_base), field.clone())
                                        }
                                    }
                                    Expr::BinaryOp(left, op, right) => {
                                        let new_left = extract_api_calls_from_expr_helper(left, script, current_func, extracted, temp_var_types, needs_self, None, session_id);
                                        let new_right = extract_api_calls_from_expr_helper(right, script, current_func, extracted, temp_var_types, needs_self, None, session_id);
                                        Expr::BinaryOp(Box::new(new_left), op.clone(), Box::new(new_right))
                                    }
                                    Expr::Call(target, args) => {
                                        let new_target = extract_api_calls_from_expr_helper(target, script, current_func, extracted, temp_var_types, needs_self, None, session_id);
                                        let new_args: Vec<Expr> = args.iter()
                                            .map(|arg| extract_api_calls_from_expr_helper(arg, script, current_func, extracted, temp_var_types, needs_self, None, session_id))
                                            .collect();
                                        Expr::Call(Box::new(new_target), new_args)
                                    }
                                    Expr::Cast(inner, target_type) => {
                                        let new_inner = extract_api_calls_from_expr_helper(inner, script, current_func, extracted, temp_var_types, needs_self, None, session_id);
                                        Expr::Cast(Box::new(new_inner), target_type.clone())
                                    }
                                    Expr::Index(array, index) => {
                                        let new_array = extract_api_calls_from_expr_helper(array, script, current_func, extracted, temp_var_types, needs_self, None, session_id);
                                        let new_index = extract_api_calls_from_expr_helper(index, script, current_func, extracted, temp_var_types, needs_self, None, session_id);
                                        Expr::Index(Box::new(new_array), Box::new(new_index))
                                    }
                                    _ => expr.clone(),
                                }
                            }
                            
                            let modified_rhs_expr = extract_api_calls_from_expr_helper(
                                &rhs_expr.expr, 
                                script, 
                                current_func, 
                                &mut extracted_api_calls, 
                                &mut temp_var_types,
                                needs_self,
                                lhs_type.as_ref(),
                                &session_id,
                            );
                            
                            // Combine all temp declarations
                            let combined_temp_decl = if !extracted_api_calls.is_empty() {
                                Some(extracted_api_calls.iter().map(|(decl, _): &(String, String)| decl.clone()).collect::<Vec<_>>().join(" "))
                            } else {
                                None
                            };
                            
                            // Generate code for the (possibly modified) RHS expression
                            let rhs_code = modified_rhs_expr.to_rust(needs_self, script, lhs_type.as_ref(), current_func, rhs_expr.span.as_ref());
                            
                            let is_literal = matches!(rhs_expr.expr, Expr::Literal(_));
                            
                            // Apply implicit conversion if needed (especially important for temp variables)
                            let final_rhs = if let Some(lhs_ty) = &lhs_type {
                                if let Some(rhs_ty) = &rhs_type {
                                    if !is_literal && rhs_ty.can_implicitly_convert_to(lhs_ty) && rhs_ty != lhs_ty {
                                        script.generate_implicit_cast_for_expr(&rhs_code, rhs_ty, lhs_ty)
                                    } else {
                                        rhs_code
                                    }
                                } else {
                                    rhs_code
                                }
                            } else {
                                rhs_code
                            };
                            
                            // Check if the field is on the base Node type - if so, use mutate_scene_node
                            let first_field = field_path_vec.first().map(|s| s.as_str()).unwrap_or("");
                            let is_base_node_field = ENGINE_REGISTRY.get_field_type_node(&NodeType::Node, first_field).is_some();
                            
                            // If it's a single field on the base Node type, use mutate_scene_node
                            if is_base_node_field && field_path_vec.len() == 1 {
                                // Map field names to their setter methods from BaseNode trait
                                let setter_method = match first_field {
                                    "name" => Some("set_name"),
                                    "id" => Some("set_id"),
                                    "local_id" => Some("set_local_id"),
                                    "parent" => Some("set_parent"),
                                    "script_path" => Some("set_script_path"),
                                    // is_root_of doesn't have a setter, fall through to match statement
                                    _ => None,
                                };
                                
                                if let Some(setter) = setter_method {
                                    let temp_decl = combined_temp_decl.as_ref().map(|d| format!("        {}\n", d)).unwrap_or_default();
                                    format!(
                                        "{}        api.mutate_scene_node({}, |n| {{ n.{}({}); }});\n",
                                        temp_decl, node_id, setter, final_rhs
                                    )
                                } else {
                                    // Field doesn't have a setter, fall back to match statement approach
                                    // If only one compatible node type, skip match and do direct mutation
                                    if compatible_node_types.len() == 1 {
                                        let node_type_name = format!("{:?}", compatible_node_types[0]);
                                        // Resolve field names in path (e.g., "texture" -> "texture_id")
                                        let resolved_path: Vec<String> = field_path_vec.iter()
                                            .map(|f| ENGINE_REGISTRY.resolve_field_name(&compatible_node_types[0], f))
                                            .collect();
                                        let resolved_field_path = resolved_path.join(".");
                                        let temp_decl = combined_temp_decl.as_ref().map(|d| format!("        {}\n", d)).unwrap_or_default();
                                        format!(
                                            "{}        api.mutate_node({}, |{}: &mut {}| {{ {}.{} = {}; }});\n",
                                            temp_decl, node_id_with_self, clean_closure_var, node_type_name, clean_closure_var, resolved_field_path, final_rhs
                                        )
                                    } else {
                                        let mut match_arms = Vec::new();
                                        for node_type_enum in &compatible_node_types {
                                            let node_type_name = format!("{:?}", node_type_enum);
                                            // Resolve field names in path for this node type
                                            let resolved_path: Vec<String> = field_path_vec.iter()
                                                .map(|f| ENGINE_REGISTRY.resolve_field_name(node_type_enum, f))
                                                .collect();
                                            let resolved_field_path = resolved_path.join(".");
                                            match_arms.push(format!(
                                                "            NodeType::{} => api.mutate_node({}, |{}: &mut {}| {{ {}.{} = {}; }}),",
                                                node_type_name, node_id_with_self, clean_closure_var, node_type_name, clean_closure_var, resolved_field_path, final_rhs
                                            ));
                                        }
                                        
                                        let temp_decl = combined_temp_decl.as_ref().map(|d| format!("        {}\n", d)).unwrap_or_default();
                                        format!(
                                            "{}        match api.get_node_type({}) {{\n{}\n            _ => {{\n                let node_name = api.read_scene_node({}, |n| n.get_name().to_string());\n                let node_type = format!(\"{{:?}}\", api.get_node_type({}));\n                panic!(\"{{}} of type {{}} doesn't have field {{}}\", node_name, node_type, \"{}\");\n            }}\n        }}\n",
                                            temp_decl, node_id_with_self,
                                            match_arms.join("\n"),
                                            node_id_with_self,
                                            node_id_with_self,
                                            field_path
                                        )
                                    }
                                }
                            } else {
                                // Generate match arms for all compatible node types
                                // If only one compatible node type, skip match and do direct mutation
                                if compatible_node_types.len() == 1 {
                                    let node_type_name = format!("{:?}", compatible_node_types[0]);
                                    // Resolve field names in path
                                    let resolved_path: Vec<String> = field_path_vec.iter()
                                        .map(|f| ENGINE_REGISTRY.resolve_field_name(&compatible_node_types[0], f))
                                        .collect();
                                    let resolved_field_path = resolved_path.join(".");
                                    let temp_decl = combined_temp_decl.as_ref().map(|d| format!("        {}\n", d)).unwrap_or_default();
                                    format!(
                                        "{}        api.mutate_node({}, |{}: &mut {}| {{ {}.{} = {}; }});\n",
                                        temp_decl, node_id_with_self, clean_closure_var, node_type_name, clean_closure_var, resolved_field_path, final_rhs
                                    )
                                } else {
                                    let mut match_arms = Vec::new();
                                    for node_type_enum in &compatible_node_types {
                                        let node_type_name = format!("{:?}", node_type_enum);
                                        // Resolve field names in path for this node type
                                        let resolved_path: Vec<String> = field_path_vec.iter()
                                            .map(|f| ENGINE_REGISTRY.resolve_field_name(node_type_enum, f))
                                            .collect();
                                        let resolved_field_path = resolved_path.join(".");
                                        match_arms.push(format!(
                                            "            NodeType::{} => api.mutate_node({}, |{}: &mut {}| {{ {}.{} = {}; }}),",
                                            node_type_name, node_id_with_self, clean_closure_var, node_type_name, clean_closure_var, resolved_field_path, final_rhs
                                        ));
                                    }
                                    
                                    let temp_decl = combined_temp_decl.as_ref().map(|d| format!("        {}\n", d)).unwrap_or_default();
                                    format!(
                                        "{}        match api.get_type({}) {{\n{}\n            _ => {{\n                let node_name = api.read_scene_node({}, |n| n.get_name().to_string());\n                let node_type = format!(\"{{:?}}\", api.get_type({}));\n                panic!(\"{{}} of type {{}} doesn't have field {{}}\", node_name, node_type, \"{}\");\n            }}\n        }}\n",
                                        temp_decl, node_id,
                                        match_arms.join("\n"),
                                        node_id,
                                        node_id,
                                        field_path
                                    )
                                }
                            }
                        }
                    } else {
                        // This is a node member assignment - use mutate_node
                        let lhs_type = script.infer_expr_type(&lhs_expr.expr, current_func);
                        let rhs_type = script.infer_expr_type(&rhs_expr.expr, current_func);
                        
                        // Extract ALL API calls and read_node calls from RHS expression to avoid borrow checker issues
                        // API calls inside mutate_node closures need to be extracted before the closure
                        let mut extracted_api_calls = Vec::new();
                        let mut temp_counter = 0;
                        let mut temp_var_types: std::collections::HashMap<String, Type> = std::collections::HashMap::new();
                        
                        // Generate a unique ID for this code generation session using UUID
                        let session_id = uuid::Uuid::new_v4().simple().to_string()[..12].to_string(); // First 12 hex chars (48 bits) from UUID without hyphens
                        
                        fn extract_api_calls_from_expr(expr: &Expr, script: &Script, current_func: Option<&Function>, 
                                                       extracted: &mut Vec<(String, String)>,
                                                       temp_var_types: &mut std::collections::HashMap<String, Type>,
                                                       needs_self: bool, expected_type: Option<&Type>,
                                                       session_id: &str) -> Expr {
                            match expr {
                                // Extract API calls (like Math.random_range, Texture.load, etc.)
                                Expr::ApiCall(api_module, api_args) => {
                                    // First, recursively extract nested API calls from arguments
                                    let new_args: Vec<Expr> = api_args.iter()
                                        .map(|arg| extract_api_calls_from_expr(arg, script, current_func, extracted, temp_var_types, needs_self, None, session_id))
                                        .collect();
                                    
                                    // Generate a unique UUID for this temp variable (guaranteed no collisions)
                                    let unique_id = uuid::Uuid::new_v4().simple().to_string()[..12].to_string(); // First 12 hex chars (48 bits) from UUID without hyphens
                                    
                                    // Extract ALL API calls, not just ones returning Uuid
                                    // This prevents borrow checker issues when API calls are inside closures
                                    let temp_var = format!("__temp_api_{}", unique_id);
                                    
                                    // Generate the API call code with extracted arguments
                                    let mut api_call_str = api_module.to_rust(&new_args, script, needs_self, current_func);
                                    
                                    // Fix any incorrect renaming of "api" to "__t_api" or "t_id_api"
                                    api_call_str = api_call_str.replace("__t_api.", "api.").replace("t_id_api.", "api.");
                                    
                                    // Infer the return type for the temp variable
                                    let inferred_type = api_module.return_type();
                                    let type_annotation = inferred_type
                                        .as_ref()
                                        .map(|t| format!(": {}", t.to_rust_type()))
                                        .unwrap_or_default();
                                    
                                    // Store the type for this temp variable
                                    if let Some(ty) = inferred_type {
                                        temp_var_types.insert(temp_var.clone(), ty);
                                    }
                                    
                                    extracted.push((format!("let {}{} = {};", temp_var, type_annotation, api_call_str), temp_var.clone()));
                                    
                                    // Return an identifier expression for the temp variable
                                    Expr::Ident(temp_var)
                                }
                                Expr::MemberAccess(base, field) => {
                                    // First, recursively extract API calls from base
                                    let new_base = extract_api_calls_from_expr(base, script, current_func, extracted, temp_var_types, needs_self, None, session_id);
                                    
                                    // Check if this member access would generate a read_node call
                                    let test_expr = Expr::MemberAccess(Box::new(new_base.clone()), field.clone());
                                    if let Some((_node_id, _, _, _)) = extract_node_member_info(&test_expr, script, current_func) {
                                        // This is a node member access - extract it to a temp variable
                                        // Generate a unique UUID for this temp variable (very low collision chance)
                                        let unique_id = uuid::Uuid::new_v4().simple().to_string().replace('-', "").chars().take(12).collect::<String>(); // First 12 hex chars (48 bits) from UUID without hyphens
                                        let temp_var = format!("__temp_read_{}", unique_id);
                                        
                                        // Generate the read_node call
                                        let read_code = test_expr.to_rust(needs_self, script, expected_type, current_func, None);
                                        
                                        // Infer the type for the temp variable
                                        let inferred_type = script.infer_expr_type(&test_expr, current_func);
                                        let type_annotation = inferred_type
                                            .as_ref()
                                            .map(|t| format!(": {}", t.to_rust_type()))
                                            .unwrap_or_default();
                                        
                                        // Store the type for this temp variable so we can check if it needs cloning
                                        if let Some(ty) = inferred_type {
                                            temp_var_types.insert(temp_var.clone(), ty);
                                        }
                                        
                                        extracted.push((format!("let {}{} = {};", temp_var, type_annotation, read_code), temp_var.clone()));
                                        
                                        // Return an identifier expression for the temp variable
                                        Expr::Ident(temp_var)
                                    } else {
                                        // Not a node member access, return the member access with processed base
                                        Expr::MemberAccess(Box::new(new_base), field.clone())
                                    }
                                }
                                Expr::BinaryOp(left, op, right) => {
                                    let new_left = extract_api_calls_from_expr(left, script, current_func, extracted, temp_var_types, needs_self, None, session_id);
                                    let new_right = extract_api_calls_from_expr(right, script, current_func, extracted, temp_var_types, needs_self, None, session_id);
                                    Expr::BinaryOp(Box::new(new_left), op.clone(), Box::new(new_right))
                                }
                                Expr::Call(target, args) => {
                                    let new_target = extract_api_calls_from_expr(target, script, current_func, extracted, temp_var_types, needs_self, None, session_id);
                                    let new_args: Vec<Expr> = args.iter()
                                        .map(|arg| extract_api_calls_from_expr(arg, script, current_func, extracted, temp_var_types, needs_self, None, session_id))
                                        .collect();
                                    Expr::Call(Box::new(new_target), new_args)
                                }
                                Expr::Cast(inner, target_type) => {
                                    let new_inner = extract_api_calls_from_expr(inner, script, current_func, extracted, temp_var_types, needs_self, None, session_id);
                                    Expr::Cast(Box::new(new_inner), target_type.clone())
                                }
                                Expr::Index(array, index) => {
                                    let new_array = extract_api_calls_from_expr(array, script, current_func, extracted, temp_var_types, needs_self, None, session_id);
                                    let new_index = extract_api_calls_from_expr(index, script, current_func, extracted, temp_var_types, needs_self, None, session_id);
                                    Expr::Index(Box::new(new_array), Box::new(new_index))
                                }
                                _ => expr.clone(),
                            }
                        }
                        
                        // Extract API calls and read_node calls from RHS expression
                        let modified_rhs_expr = extract_api_calls_from_expr(
                            &rhs_expr.expr, 
                            script, 
                            current_func, 
                            &mut extracted_api_calls, 
                            &mut temp_var_types,
                            needs_self,
                            lhs_type.as_ref(),
                            &session_id,
                        );
                        
                        // Combine all temp declarations from extracted API calls
                        let combined_temp_decl = if !extracted_api_calls.is_empty() {
                            Some(extracted_api_calls.iter().map(|(decl, _): &(String, String)| decl.clone()).collect::<Vec<_>>().join(" "))
                        } else {
                            None
                        };
                        
                        // Generate code for the (possibly modified) RHS expression
                        // If API calls were extracted, the modified expression uses temp variables
                        let rhs_code = modified_rhs_expr.to_rust(needs_self, script, lhs_type.as_ref(), current_func, rhs_expr.span.as_ref());
                        
                        // For literals, we already generated the code with the expected type,
                        // so skip implicit cast to avoid double conversion
                        let is_literal = matches!(rhs_expr.expr, Expr::Literal(_));
                        
                        // Apply implicit conversion if needed (especially important for temp variables)
                        let final_rhs = if let Some(lhs_ty) = &lhs_type {
                            if let Some(rhs_ty) = &rhs_type {
                                // For literals, if they were generated with the correct expected type,
                                // they should already be correct. Only apply cast if types don't match
                                // and it's not a literal (literals handle their own type conversion)
                                if !is_literal && rhs_ty.can_implicitly_convert_to(lhs_ty) && rhs_ty != lhs_ty {
                                    script.generate_implicit_cast_for_expr(&rhs_code, rhs_ty, lhs_ty)
                                } else if is_literal {
                                    // For literals, check if the generated code needs conversion
                                    // If lhs is Option<CowStr> but we got String::from, convert it
                                    if matches!(lhs_ty, Type::Option(inner) if matches!(inner.as_ref(), Type::CowStr))
                                        && rhs_code.contains("String::from(") {
                                        // Extract the literal from String::from("...") and convert to Some(Cow::Borrowed(...))
                                        let trimmed = rhs_code.trim();
                                        if trimmed.starts_with("String::from(") && trimmed.ends_with(')') {
                                            let inner_section = &trimmed["String::from(".len()..trimmed.len() - 1].trim();
                                            if inner_section.starts_with('"') && inner_section.ends_with('"') {
                                                format!("Some(Cow::Borrowed({}))", inner_section)
                                            } else {
                                                script.generate_implicit_cast_for_expr(&rhs_code, rhs_ty, lhs_ty)
                                            }
                                        } else {
                                            script.generate_implicit_cast_for_expr(&rhs_code, rhs_ty, lhs_ty)
                                        }
                                    } else {
                                        rhs_code
                                    }
                                } else {
                                    rhs_code
                                }
                            } else {
                                rhs_code
                            }
                        } else {
                            rhs_code
                        };
                        
                        // Resolve field names in path (e.g., "texture" -> "texture_id")
                        let resolved_field_path = if let Some(node_type_enum) = string_to_node_type(&node_type) {
                            let field_path_vec: Vec<&str> = field_path.split('.').collect();
                            let resolved_path: Vec<String> = field_path_vec.iter()
                                .map(|f| ENGINE_REGISTRY.resolve_field_name(&node_type_enum, f))
                                .collect();
                            resolved_path.join(".")
                        } else {
                            field_path.clone()
                        };
                        
                        let temp_decl = combined_temp_decl.as_ref().map(|d| format!("        {}\n", d)).unwrap_or_default();
                        format!(
                            "{}        api.mutate_node({}, |{}: &mut {}| {{ {}.{} = {}; }});\n",
                            temp_decl, node_id_with_self, clean_closure_var, node_type, clean_closure_var, resolved_field_path, final_rhs
                        )
                    }
                } else {
                    // Regular member assignment (not a node)
                    let lhs_code = lhs_expr.to_rust(needs_self, script, current_func);
                    // lhs_expr is TypedExpr, which already passes span through
                    let lhs_type = script.infer_expr_type(&lhs_expr.expr, current_func);
                    let rhs_type = script.infer_expr_type(&rhs_expr.expr, current_func);

                    let rhs_code =
                        rhs_expr
                            .expr
                            .to_rust(needs_self, script, lhs_type.as_ref(), current_func, rhs_expr.span.as_ref());

                    let final_rhs = if let Some(lhs_ty) = &lhs_type {
                        if let Some(rhs_ty) = &rhs_type {
                            if rhs_ty.can_implicitly_convert_to(lhs_ty) && rhs_ty != lhs_ty {
                                script.generate_implicit_cast_for_expr(&rhs_code, rhs_ty, lhs_ty)
                            } else {
                                rhs_code
                            }
                        } else {
                            rhs_code
                        }
                    } else {
                        rhs_code
                    };

                    let should_clone = matches!(rhs_expr.expr, Expr::Ident(_) | Expr::MemberAccess(..))
                        && rhs_type.as_ref().map_or(false, |ty| ty.requires_clone());

                    if should_clone {
                        format!("        {lhs_code} = {}.clone();\n", final_rhs)
                    } else {
                        format!("        {lhs_code} = {final_rhs};\n")
                    }
                }
            }

            Stmt::MemberAssignOp(lhs_expr, op, rhs_expr) => {
                // Check if this is a node member assignment (like self.transform.position.x += value)
                if let Some((node_id, node_type, field_path, closure_var)) = 
                    extract_node_member_info(&lhs_expr.expr, script, current_func) 
                {
                    // Clean closure_var (remove self. prefix) and ensure node_id has self. prefix
                    let clean_closure_var = closure_var.strip_prefix("self.").unwrap_or(&closure_var);
                    let node_id_with_self = if !node_id.starts_with("self.") && !node_id.starts_with("api.") {
                        format!("self.{}", node_id)
                    } else {
                        node_id.clone()
                    };
                    // Check if this is a DynNode (special marker)
                    if node_type == "__DYN_NODE__" {
                        // Build field path from the expression
                        let mut field_path_vec = vec![];
                        let mut current_expr = &lhs_expr.expr;
                        while let Expr::MemberAccess(inner_base, inner_field) = current_expr {
                            field_path_vec.push(inner_field.clone());
                            current_expr = inner_base.as_ref();
                        }
                        field_path_vec.reverse();
                        
                        // Find all node types that have this field path
                        let compatible_node_types = ENGINE_REGISTRY.narrow_nodes_by_fields(&field_path_vec);
                        
                        if compatible_node_types.is_empty() {
                            // No compatible node types found, fallback to error
                            format!("        // ERROR: No compatible node types found for field path: {}\n", field_path)
                        } else {
                            // Generate match arms for all compatible node types
                            let lhs_type = script.infer_expr_type(&lhs_expr.expr, current_func);
                            
                            let rhs_code =
                                rhs_expr
                                    .expr
                                    .to_rust(needs_self, script, lhs_type.as_ref(), current_func, rhs_expr.span.as_ref());
                            
                            if matches!(op, Op::Add) && lhs_type == Some(Type::String) {
                                // If only one compatible node type, skip match and do direct mutation
                                if compatible_node_types.len() == 1 {
                                    let node_type_name = format!("{:?}", compatible_node_types[0]);
                                    // Resolve field names in path
                                    let resolved_path: Vec<String> = field_path_vec.iter()
                                        .map(|f| ENGINE_REGISTRY.resolve_field_name(&compatible_node_types[0], f))
                                        .collect();
                                    let resolved_field_path = resolved_path.join(".");
                                    return format!(
                                        "        api.mutate_node({}, |{}: &mut {}| {{ {}.{}.push_str({}.as_str()); }});\n",
                                        node_id_with_self, clean_closure_var, node_type_name, clean_closure_var, resolved_field_path, rhs_code
                                    );
                                } else {
                                    let mut match_arms = Vec::new();
                                    for node_type_enum in &compatible_node_types {
                                        let node_type_name = format!("{:?}", node_type_enum);
                                        // Resolve field names in path for this node type
                                        let resolved_path: Vec<String> = field_path_vec.iter()
                                            .map(|f| ENGINE_REGISTRY.resolve_field_name(node_type_enum, f))
                                            .collect();
                                        let resolved_field_path = resolved_path.join(".");
                                        match_arms.push(format!(
                                            "            NodeType::{} => api.mutate_node({}, |{}: &mut {}| {{ {}.{}.push_str({}.as_str()); }}),",
                                            node_type_name, node_id_with_self, clean_closure_var, node_type_name, clean_closure_var, resolved_field_path, rhs_code
                                        ));
                                    }
                                    return format!(
                                        "        match api.get_node_type({}) {{\n{}\n            _ => {{\n                let node_name = api.read_scene_node({}, |n| n.get_name().to_string());\n                let node_type = format!(\"{{:?}}\", api.get_node_type({}));\n                panic!(\"{{}} of type {{}} doesn't have field {{}}\", node_name, node_type, \"{}\");\n            }}\n        }}\n",
                                        node_id_with_self,
                                        match_arms.join("\n"),
                                        node_id_with_self,
                                        node_id_with_self,
                                        field_path
                                    );
                                }
                            }
                            
                            let final_rhs = if let Some(lhs_ty) = &lhs_type {
                                let rhs_ty = script.infer_expr_type(&rhs_expr.expr, current_func);
                                if let Some(rhs_ty) = &rhs_ty {
                                    if rhs_ty.can_implicitly_convert_to(lhs_ty) && rhs_ty != lhs_ty {
                                        script.generate_implicit_cast_for_expr(&rhs_code, rhs_ty, lhs_ty)
                                    } else {
                                        rhs_code
                                    }
                                } else {
                                    rhs_code
                                }
                            } else {
                                rhs_code
                            };
                            
                            // If only one compatible node type, skip match and do direct mutation
                            if compatible_node_types.len() == 1 {
                                let node_type_name = format!("{:?}", compatible_node_types[0]);
                                // Resolve field names in path
                                let resolved_path: Vec<String> = field_path_vec.iter()
                                    .map(|f| ENGINE_REGISTRY.resolve_field_name(&compatible_node_types[0], f))
                                    .collect();
                                let resolved_field_path = resolved_path.join(".");
                                format!(
                                    "        api.mutate_node({}, |{}: &mut {}| {{ {}.{} {}= {}; }});\n",
                                    node_id_with_self, clean_closure_var, node_type_name, clean_closure_var, resolved_field_path, op.to_rust_assign(), final_rhs
                                )
                            } else {
                                let mut match_arms = Vec::new();
                                for node_type_enum in &compatible_node_types {
                                    let node_type_name = format!("{:?}", node_type_enum);
                                    // Resolve field names in path for this node type
                                    let resolved_path: Vec<String> = field_path_vec.iter()
                                        .map(|f| ENGINE_REGISTRY.resolve_field_name(node_type_enum, f))
                                        .collect();
                                    let resolved_field_path = resolved_path.join(".");
                                    match_arms.push(format!(
                                        "            NodeType::{} => api.mutate_node({}, |{}: &mut {}| {{ {}.{} {}= {}; }}),",
                                        node_type_name, node_id_with_self, clean_closure_var, node_type_name, clean_closure_var, resolved_field_path, op.to_rust_assign(), final_rhs
                                    ));
                                }
                                
                                format!(
                                    "        match api.get_node_type({}) {{\n{}\n            _ => {{\n                let node_name = api.read_scene_node({}, |n| n.get_name().to_string());\n                let node_type = format!(\"{{:?}}\", api.get_node_type({}));\n                panic!(\"{{}} of type {{}} doesn't have field {{}}\", node_name, node_type, \"{}\");\n            }}\n        }}\n",
                                    node_id_with_self,
                                    match_arms.join("\n"),
                                    node_id_with_self,
                                    node_id_with_self,
                                    field_path
                                )
                            }
                        }
                    } else {
                        // This is a node member assignment - use mutate_node
                        let lhs_type = script.infer_expr_type(&lhs_expr.expr, current_func);
                        let rhs_type = script.infer_expr_type(&rhs_expr.expr, current_func);
                        
                        // Extract ALL API calls and read_node calls from RHS expression to avoid borrow checker issues
                        // API calls inside mutate_node closures need to be extracted before the closure
                        let mut extracted_api_calls = Vec::new();
                        let mut temp_var_types: std::collections::HashMap<String, Type> = std::collections::HashMap::new();
                        
                        // Generate a unique ID for this code generation session using UUID
                        let session_id = uuid::Uuid::new_v4().simple().to_string()[..12].to_string(); // First 12 hex chars (48 bits) from UUID without hyphens
                        
                        fn extract_api_calls_from_expr(expr: &Expr, script: &Script, current_func: Option<&Function>, 
                                                       extracted: &mut Vec<(String, String)>,
                                                       temp_var_types: &mut std::collections::HashMap<String, Type>,
                                                       needs_self: bool, expected_type: Option<&Type>,
                                                       session_id: &str) -> Expr {
                            match expr {
                                // Extract API calls (like Math.random_range, Texture.load, etc.)
                                Expr::ApiCall(api_module, api_args) => {
                                    // First, recursively extract nested API calls from arguments
                                    let new_args: Vec<Expr> = api_args.iter()
                                        .map(|arg| extract_api_calls_from_expr(arg, script, current_func, extracted, temp_var_types, needs_self, None, session_id))
                                        .collect();
                                    
                                    // Generate a unique UUID for this temp variable (guaranteed no collisions)
                                    let unique_id = uuid::Uuid::new_v4().simple().to_string()[..12].to_string(); // First 12 hex chars (48 bits) from UUID without hyphens
                                    
                                    // Extract ALL API calls, not just ones returning Uuid
                                    // This prevents borrow checker issues when API calls are inside closures
                                    let temp_var = format!("__temp_api_{}", unique_id);
                                    
                                    // Generate the API call code with extracted arguments
                                    let mut api_call_str = api_module.to_rust(&new_args, script, needs_self, current_func);
                                    
                                    // Fix any incorrect renaming of "api" to "__t_api" or "t_id_api"
                                    api_call_str = api_call_str.replace("__t_api.", "api.").replace("t_id_api.", "api.");
                                    
                                    // Infer the return type for the temp variable
                                    let inferred_type = api_module.return_type();
                                    let type_annotation = inferred_type
                                        .as_ref()
                                        .map(|t| format!(": {}", t.to_rust_type()))
                                        .unwrap_or_default();
                                    
                                    // Store the type for this temp variable
                                    if let Some(ty) = inferred_type {
                                        temp_var_types.insert(temp_var.clone(), ty);
                                    }
                                    
                                    extracted.push((format!("let {}{} = {};", temp_var, type_annotation, api_call_str), temp_var.clone()));
                                    
                                    // Return an identifier expression for the temp variable
                                    Expr::Ident(temp_var)
                                }
                                Expr::MemberAccess(base, field) => {
                                    // First, recursively extract API calls from base
                                    let new_base = extract_api_calls_from_expr(base, script, current_func, extracted, temp_var_types, needs_self, None, session_id);
                                    
                                    // Check if this member access would generate a read_node call
                                    let test_expr = Expr::MemberAccess(Box::new(new_base.clone()), field.clone());
                                    if let Some((_node_id, _, _, _)) = extract_node_member_info(&test_expr, script, current_func) {
                                        // This is a node member access - extract it to a temp variable
                                        // Generate a unique UUID for this temp variable (very low collision chance)
                                        let unique_id = uuid::Uuid::new_v4().simple().to_string().replace('-', "").chars().take(12).collect::<String>(); // First 12 hex chars (48 bits) from UUID without hyphens
                                        let temp_var = format!("__temp_read_{}", unique_id);
                                        
                                        // Generate the read_node call
                                        let read_code = test_expr.to_rust(needs_self, script, expected_type, current_func, None);
                                        
                                        // Infer the type for the temp variable
                                        let inferred_type = script.infer_expr_type(&test_expr, current_func);
                                        let type_annotation = inferred_type
                                            .as_ref()
                                            .map(|t| format!(": {}", t.to_rust_type()))
                                            .unwrap_or_default();
                                        
                                        // Store the type for this temp variable so we can check if it needs cloning
                                        if let Some(ty) = inferred_type {
                                            temp_var_types.insert(temp_var.clone(), ty);
                                        }
                                        
                                        extracted.push((format!("let {}{} = {};", temp_var, type_annotation, read_code), temp_var.clone()));
                                        
                                        // Return an identifier expression for the temp variable
                                        Expr::Ident(temp_var)
                                    } else {
                                        // Not a node member access, return the member access with processed base
                                        Expr::MemberAccess(Box::new(new_base), field.clone())
                                    }
                                }
                                Expr::BinaryOp(left, op, right) => {
                                    let new_left = extract_api_calls_from_expr(left, script, current_func, extracted, temp_var_types, needs_self, None, session_id);
                                    let new_right = extract_api_calls_from_expr(right, script, current_func, extracted, temp_var_types, needs_self, None, session_id);
                                    Expr::BinaryOp(Box::new(new_left), op.clone(), Box::new(new_right))
                                }
                                Expr::Call(target, args) => {
                                    let new_target = extract_api_calls_from_expr(target, script, current_func, extracted, temp_var_types, needs_self, None, session_id);
                                    let new_args: Vec<Expr> = args.iter()
                                        .map(|arg| extract_api_calls_from_expr(arg, script, current_func, extracted, temp_var_types, needs_self, None, session_id))
                                        .collect();
                                    Expr::Call(Box::new(new_target), new_args)
                                }
                                Expr::Cast(inner, target_type) => {
                                    let new_inner = extract_api_calls_from_expr(inner, script, current_func, extracted, temp_var_types, needs_self, None, session_id);
                                    Expr::Cast(Box::new(new_inner), target_type.clone())
                                }
                                Expr::Index(array, index) => {
                                    let new_array = extract_api_calls_from_expr(array, script, current_func, extracted, temp_var_types, needs_self, None, session_id);
                                    let new_index = extract_api_calls_from_expr(index, script, current_func, extracted, temp_var_types, needs_self, None, session_id);
                                    Expr::Index(Box::new(new_array), Box::new(new_index))
                                }
                                _ => expr.clone(),
                            }
                        }
                        
                        // Extract API calls and read_node calls from RHS expression
                        let modified_rhs_expr = extract_api_calls_from_expr(
                            &rhs_expr.expr, 
                            script, 
                            current_func, 
                            &mut extracted_api_calls, 
                            &mut temp_var_types,
                            needs_self,
                            lhs_type.as_ref(),
                            &session_id,
                        );
                        
                        // Combine all temp declarations from extracted API calls
                        let combined_temp_decl = if !extracted_api_calls.is_empty() {
                            Some(extracted_api_calls.iter().map(|(decl, _): &(String, String)| decl.clone()).collect::<Vec<_>>().join(" "))
                        } else {
                            None
                        };
                        
                        // Generate code for the (possibly modified) RHS expression
                        // If API calls were extracted, the modified expression uses temp variables
                        let rhs_code = modified_rhs_expr.to_rust(needs_self, script, lhs_type.as_ref(), current_func, rhs_expr.span.as_ref());
                        
                        // Resolve field names in path (e.g., "texture" -> "texture_id")
                        let resolved_field_path = if let Some(node_type_enum) = string_to_node_type(&node_type) {
                            let field_path_vec: Vec<&str> = field_path.split('.').collect();
                            let resolved_path: Vec<String> = field_path_vec.iter()
                                .map(|f| ENGINE_REGISTRY.resolve_field_name(&node_type_enum, f))
                                .collect();
                            resolved_path.join(".")
                        } else {
                            field_path.clone()
                        };
                        
                        if matches!(op, Op::Add) && lhs_type == Some(Type::String) {
                            let temp_decl = combined_temp_decl.as_ref().map(|d| format!("        {}\n", d)).unwrap_or_default();
                            return format!(
                                "{}        api.mutate_node({}, |{}: &mut {}| {{ {}.{}.push_str({}.as_str()); }});\n",
                                temp_decl, node_id_with_self, clean_closure_var, node_type, clean_closure_var, resolved_field_path, rhs_code
                            );
                        }
                        
                        let final_rhs = if let Some(lhs_ty) = &lhs_type {
                            if let Some(rhs_ty) = &rhs_type {
                                if rhs_ty.can_implicitly_convert_to(lhs_ty) && rhs_ty != lhs_ty {
                                    script.generate_implicit_cast_for_expr(&rhs_code, rhs_ty, lhs_ty)
                                } else {
                                    rhs_code
                                }
                            } else {
                                rhs_code
                            }
                        } else {
                            rhs_code
                        };
                        
                        let temp_decl = combined_temp_decl.as_ref().map(|d| format!("        {}\n", d)).unwrap_or_default();
                        format!(
                            "{}        api.mutate_node({}, |{}: &mut {}| {{ {}.{} {}= {}; }});\n",
                            temp_decl, node_id_with_self, clean_closure_var, node_type, clean_closure_var, resolved_field_path, op.to_rust_assign(), final_rhs
                        )
                    }
                } else {
                    // Regular member assignment (not a node)
                    let lhs_code = lhs_expr.to_rust(needs_self, script, current_func);
                    // lhs_expr is TypedExpr, which already passes span through
                    let lhs_type = script.infer_expr_type(&lhs_expr.expr, current_func);

                    let rhs_code =
                        rhs_expr
                            .expr
                            .to_rust(needs_self, script, lhs_type.as_ref(), current_func, rhs_expr.span.as_ref());

                    if matches!(op, Op::Add) && lhs_type == Some(Type::String) {
                        return format!("        {lhs_code}.push_str({rhs_code}.as_str());\n");
                    }

                    let final_rhs = if let Some(lhs_ty) = &lhs_type {
                        let rhs_ty = script.infer_expr_type(&rhs_expr.expr, current_func);
                        if let Some(rhs_ty) = rhs_ty {
                            if rhs_ty.can_implicitly_convert_to(lhs_ty) && rhs_ty != *lhs_ty {
                                script.generate_implicit_cast_for_expr(&rhs_code, &rhs_ty, lhs_ty)
                            } else {
                                rhs_code
                            }
                        } else {
                            rhs_code
                        }
                    } else {
                        rhs_code
                    };

                    format!(
                        "        {lhs_code} {}= {};\n",
                        op.to_rust_assign(),
                        final_rhs
                    )
                }
            }

            Stmt::Pass => String::new(),

            Stmt::If {
                condition,
                then_body,
                else_body,
            } => {
                let cond_str = condition.to_rust(needs_self, script, current_func);
                // condition is TypedExpr, which already passes span through
                let mut result = format!("        if {} {{\n", cond_str);

                for stmt in then_body {
                    let stmt_str = stmt.to_rust(needs_self, script, current_func);
                    // stmt is Stmt, which handles spans internally
                    // Add extra indentation for statements inside the block
                    let indented = stmt_str
                        .lines()
                        .map(|line| {
                            if line.trim().is_empty() {
                                String::new()
                            } else {
                                format!("    {}", line)
                            }
                        })
                        .collect::<Vec<_>>()
                        .join("\n");
                    result.push_str(&indented);
                    if !indented.ends_with('\n') {
                        result.push('\n');
                    }
                }

                result.push_str("        }");

                if let Some(else_body) = else_body {
                    result.push_str(" else {\n");
                    for stmt in else_body {
                        let stmt_str = stmt.to_rust(needs_self, script, current_func);
                    // stmt is Stmt, which handles spans internally
                        // Add extra indentation for statements inside the block
                        let indented = stmt_str
                            .lines()
                            .map(|line| {
                                if line.trim().is_empty() {
                                    String::new()
                                } else {
                                    format!("    {}", line)
                                }
                            })
                            .collect::<Vec<_>>()
                            .join("\n");
                        result.push_str(&indented);
                        if !indented.ends_with('\n') {
                            result.push('\n');
                        }
                    }
                    result.push_str("        }\n");
                } else {
                    result.push_str("\n");
                }

                result
            }

            Stmt::For {
                var_name,
                iterable,
                body,
            } => {
                // Check if loop body creates any nodes that need to be merged
                let mut loop_node_vars = Vec::new();
                let mut loop_ui_elements = Vec::new();
                collect_cloned_node_vars(body, &mut loop_node_vars, &mut loop_ui_elements, script);
                
                let iter_str = iterable.to_rust(needs_self, script, current_func);
                // iterable is TypedExpr, which already passes span through
                let mut result = String::new();
                
                // Use TRANSPILED_IDENT prefix for loop variable (e.g., i -> __t_i)
                let loop_var_name = format!("{}{}", TRANSPILED_IDENT, var_name);
                result.push_str(&format!("        for {} in {} {{\n", loop_var_name, iter_str));

                // Track which nodes are created/modified in this iteration
                let mut nodes_created_this_iter = Vec::new();
                
                for stmt in body {
                    let stmt_str = stmt.to_rust(needs_self, script, current_func);
                    // stmt is Stmt, which handles spans internally
                    // Add extra indentation for statements inside the block
                    let indented = stmt_str
                        .lines()
                        .map(|line| {
                            if line.trim().is_empty() {
                                String::new()
                            } else {
                                format!("    {}", line)
                            }
                        })
                        .collect::<Vec<_>>()
                        .join("\n");
                    result.push_str(&indented);
                    if !indented.ends_with('\n') {
                        result.push('\n');
                    }
                    
                    // Track nodes created in this iteration (but don't push yet - wait until after modifications)
                    match stmt {
                        Stmt::VariableDecl(var) => {
                            if loop_node_vars.contains(&var.name) {
                                nodes_created_this_iter.push(var.name.clone());
                            }
                        }
                        Stmt::Assign(name, _) => {
                            if loop_node_vars.contains(name) && !nodes_created_this_iter.contains(name) {
                                nodes_created_this_iter.push(name.clone());
                            }
                        }
                        _ => {}
                    }
                }
                
                // No longer need to track nodes for merging - we use mutate_node for assignments

                result.push_str("        }\n");
                result
            }

            Stmt::ForTraditional {
                init,
                condition,
                increment,
                body,
            } => {
                // Check if loop body creates any nodes that need to be merged
                let mut loop_node_vars = Vec::new();
                let mut loop_ui_elements = Vec::new();
                collect_cloned_node_vars(body, &mut loop_node_vars, &mut loop_ui_elements, script);
                
                let mut result = String::new();

                // Init - declare variable before the loop if it's a VariableDecl
                if let Some(init_stmt) = init {
                    match init_stmt.as_ref() {
                        Stmt::VariableDecl(var) => {
                            // Default to f32 if type is not inferred (common for loop counters)
                            let var_type = if var.typ.is_none() {
                                "f32".to_string()
                            } else {
                                var.rust_type()
                            };
                            let init_val = if var.value.is_none() {
                                "0.0".to_string()
                            } else {
                                var.rust_initialization(script, current_func)
                            };
                            result.push_str(&format!(
                                "        let mut {}: {} = {};\n",
                                var.name, var_type, init_val
                            ));
                        }
                        Stmt::Assign(name, expr) => {
                            let expr_str = expr.to_rust(needs_self, script, current_func);
                // expr is TypedExpr, which already passes span through
                            result.push_str(&format!("        let mut {} = {};\n", name, expr_str));
                        }
                        _ => {
                            // For other init statements, just generate the code
                            let init_code = init_stmt.to_rust(needs_self, script, current_func);
                            // init_stmt is Stmt, which handles spans internally
                            result.push_str(&format!(
                                "        {}\n",
                                init_code.trim().trim_end_matches(';')
                            ));
                        }
                    }
                }

                // Convert to while loop since Rust doesn't support C-style for loops
                result.push_str("        while ");

                // Condition
                if let Some(cond) = condition {
                    let cond_str = cond.to_rust(needs_self, script, current_func);
                    result.push_str(&cond_str);
                } else {
                    result.push_str("true");
                }
                result.push_str(" {\n");

                // Track which nodes are created/modified in this iteration
                let mut nodes_created_this_iter = Vec::new();

                // Body
                for stmt in body {
                    let stmt_str = stmt.to_rust(needs_self, script, current_func);
                    // stmt is Stmt, which handles spans internally
                    // Add extra indentation for statements inside the block
                    let indented = stmt_str
                        .lines()
                        .map(|line| {
                            if line.trim().is_empty() {
                                String::new()
                            } else {
                                format!("            {}", line)
                            }
                        })
                        .collect::<Vec<_>>()
                        .join("\n");
                    result.push_str(&indented);
                    if !indented.ends_with('\n') {
                        result.push('\n');
                    }
                    
                    // Track nodes created in this iteration (but don't push yet - wait until after modifications)
                    match stmt {
                        Stmt::VariableDecl(var) => {
                            if loop_node_vars.contains(&var.name) {
                                nodes_created_this_iter.push(var.name.clone());
                            }
                        }
                        Stmt::Assign(name, _) => {
                            if loop_node_vars.contains(name) && !nodes_created_this_iter.contains(name) {
                                nodes_created_this_iter.push(name.clone());
                            }
                        }
                        _ => {}
                    }
                }
                
                // Push all nodes created/modified in this iteration AFTER all statements (so modifications are captured)
                if !loop_node_vars.is_empty() && !nodes_created_this_iter.is_empty() {
                    for _node_var in &nodes_created_this_iter {
                        // No longer need to track nodes for merging - we use mutate_node for assignments
                    }
                }

                // Increment at the end of the loop body
                if let Some(incr_stmt) = increment {
                    match incr_stmt.as_ref() {
                        Stmt::AssignOp(name, op, expr) => {
                            // Rust doesn't have ++ or --, so use += and -=
                            let op_str = match op {
                                Op::Add => "+=",
                                Op::Sub => "-=",
                                Op::Mul => "*=",
                                Op::Div => "/=",
                                Op::Lt | Op::Gt | Op::Le | Op::Ge | Op::Eq | Op::Ne => {
                                    unreachable!(
                                        "Comparison operators cannot be used in assignment operations"
                                    )
                                }
                            };
                            let expr_str = expr.to_rust(needs_self, script, current_func);
                // expr is TypedExpr, which already passes span through
                            result.push_str(&format!(
                                "            {} {} {};\n",
                                name, op_str, expr_str
                            ));
                        }
                        Stmt::Assign(name, expr) => {
                            let expr_str = expr.to_rust(needs_self, script, current_func);
                // expr is TypedExpr, which already passes span through
                            result.push_str(&format!("            {} = {};\n", name, expr_str));
                        }
                        _ => {
                            let incr_code = incr_stmt.to_rust(needs_self, script, current_func);
                            // incr_stmt is Stmt, which handles spans internally
                            result.push_str(&format!(
                                "            {}\n",
                                incr_code.trim().trim_end_matches(';')
                            ));
                        }
                    }
                }

                result.push_str("        }\n");
                result
            }

            Stmt::ScriptAssign(var, field, rhs) => {
                let rhs_str = rhs.to_rust(needs_self, script, current_func);
                // rhs is TypedExpr, which already passes span through
                
                // Get the node ID variable name (should already have _id suffix from parser)
                let node_id_var = format!("{}_id", var);
                
                // Precompute the variable ID hash at compile time
                use crate::prelude::string_to_u64;
                let var_id = string_to_u64(field);
                
                // Convert rhs to Value using json!
                let val_expr = if rhs_str.starts_with("json!(") || rhs_str.contains("Value::") {
                    rhs_str
                } else {
                    format!("json!({})", rhs_str)
                };

                format!(
                    "        api.set_script_var_id({}, {}u64, {});\n",
                    node_id_var, var_id, val_expr
                )
            }

            Stmt::ScriptAssignOp(var, field, op, rhs) => {
                let rhs_str = rhs.to_rust(needs_self, script, current_func);
                // rhs is TypedExpr, which already passes span through
                
                // Get the node ID variable name (should already have _id suffix from parser)
                let node_id_var = format!("{}_id", var);
                
                // Precompute the variable ID hash at compile time
                use crate::prelude::string_to_u64;
                let var_id = string_to_u64(field);
                
                // For assign-op, we need to get the current value, apply the operation, then set it
                // This requires a get, compute, set pattern
                let op_rust = match op {
                    Op::Add => "+",
                    Op::Sub => "-",
                    Op::Mul => "*",
                    Op::Div => "/",
                    Op::Lt | Op::Gt | Op::Le | Op::Ge | Op::Eq | Op::Ne => {
                        unreachable!("Comparison operators cannot be used in assignment operations")
                    }
                };
                
                // Convert rhs to a number for the operation
                // We'll need to extract the numeric value from the Value
                // For now, assume it's a simple numeric expression
                format!(
                    "        {{\n            let __current_val = api.get_script_var_id({}, {}u64);\n            let __rhs_val = json!({});\n            let __new_val = json!((__current_val.as_f64().unwrap_or(0.0) {} __rhs_val.as_f64().unwrap_or(0.0)));\n            api.set_script_var_id({}, {}u64, __new_val);\n        }}\n",
                    node_id_var, var_id, rhs_str, op_rust, node_id_var, var_id
                )
            }

            Stmt::IndexAssign(array_expr, index_expr, rhs_expr) => {
                let lhs_type = script.infer_expr_type(&array_expr, current_func);
                let rhs_type = script.infer_expr_type(&rhs_expr.expr, current_func);
                let base_code = array_expr.to_rust(needs_self, script, None, current_func, None);

                // Check if this is a map (HashMap) vs array (Vec)
                let is_map = matches!(lhs_type, Some(Type::Container(ContainerKind::Map, _)));

                let (index_code, is_dynamic_array) = if is_map {
                    // For maps, use string key handling
                    // For assignment, we need String (not &str) for .insert()
                    let key_ty =
                        if let Some(Type::Container(ContainerKind::Map, inner_types)) = &lhs_type {
                            inner_types.get(0).unwrap_or(&Type::String)
                        } else {
                            &Type::String
                        };
                    let key_code_raw =
                        index_expr.to_rust(needs_self, script, Some(key_ty), current_func, None);
                    let key_type = script.infer_expr_type(index_expr, current_func);
                    let final_key_code = if *key_ty == Type::String {
                        // For String keys, convert the key to string if it's not already
                        // For assignment, we need String (not &str), so don't add .as_str()
                        if matches!(key_type, Some(Type::Number(_)) | Some(Type::Bool)) {
                            format!("{}.to_string()", key_code_raw)
                        } else if key_code_raw.starts_with("String::from") {
                            key_code_raw
                        } else {
                            format!("{}.to_string()", key_code_raw)
                        }
                    } else {
                        // For non-string keys, use reference
                        format!("&{}", key_code_raw)
                    };
                    (final_key_code, false)
                } else {
                    // For arrays, ensure index is usize
                    let index_code_raw = index_expr.to_rust(
                        needs_self,
                        script,
                        Some(&Type::Number(NumberKind::Unsigned(32))),
                        current_func,
                        None, // index_expr is Expr, no span available
                    );
                    let index_code = format!("{} as usize", index_code_raw);

                    // Check if this is a dynamic array (Vec<Value>) that needs special handling
                    let is_dynamic_array =
                        if let Some(Type::Container(ContainerKind::Array, inner_types)) = &lhs_type
                        {
                            inner_types.get(0).map_or(true, |t| {
                                *t == Type::Object || matches!(t, Type::Custom(_))
                            })
                        } else {
                            false
                        };
                    (index_code, is_dynamic_array)
                };

                if is_map {
                    // Handle map assignment
                    let inner_types =
                        if let Some(Type::Container(ContainerKind::Map, inner_types)) = &lhs_type {
                            inner_types
                        } else {
                            &vec![]
                        };
                    let value_ty = inner_types.get(1).unwrap_or(&Type::Object);
                    let is_dynamic_map = value_ty == &Type::Object;

                    let rhs_code =
                        rhs_expr
                            .expr
                            .to_rust(needs_self, script, Some(value_ty), current_func, None);

                    // Insert implicit conversion if needed
                    let final_rhs = if let Some(rhs_ty) = &rhs_type {
                        if rhs_ty.can_implicitly_convert_to(value_ty) && rhs_ty != value_ty {
                            script.generate_implicit_cast_for_expr(&rhs_code, rhs_ty, value_ty)
                        } else {
                            rhs_code
                        }
                    } else {
                        rhs_code
                    };

                    // For dynamic maps, wrap the value in json!()
                    let final_rhs_wrapped = if is_dynamic_map {
                        // Check if it's already wrapped in json!() or is a Value
                        if final_rhs.starts_with("json!") || final_rhs.contains("Value") {
                            final_rhs
                        } else {
                            format!("json!({})", final_rhs)
                        }
                    } else {
                        final_rhs
                    };

                    // Maps use .insert() for assignment
                    // index_code is already a String for string keys, or a reference for other key types
                    format!(
                        "        {}.insert({}, {});\n",
                        base_code, index_code, final_rhs_wrapped
                    )
                } else {
                    // Handle array assignment
                    let rhs_code =
                        rhs_expr
                            .expr
                            .to_rust(needs_self, script, lhs_type.as_ref(), current_func, rhs_expr.span.as_ref());

                    // Insert implicit conversion if needed, matching your member assign arm
                    let final_rhs = if let Some(lhs_ty) = &lhs_type {
                        if let Some(rhs_ty) = &rhs_type {
                            if rhs_ty.can_implicitly_convert_to(lhs_ty) && rhs_ty != lhs_ty {
                                script.generate_implicit_cast_for_expr(&rhs_code, rhs_ty, lhs_ty)
                            } else {
                                rhs_code
                            }
                        } else {
                            rhs_code
                        }
                    } else {
                        rhs_code
                    };

                    // For dynamic arrays, wrap the value in json!()
                    let final_rhs_wrapped = if is_dynamic_array {
                        // Check if it's already wrapped in json!() or is a Value
                        if final_rhs.starts_with("json!") || final_rhs.contains("Value") {
                            final_rhs
                        } else {
                            format!("json!({})", final_rhs)
                        }
                    } else {
                        final_rhs
                    };

                    // Check if index expression references the same array being indexed
                    // This causes a borrow checker error: cannot borrow as immutable and mutable
                    let index_refs_array = index_code.contains(&base_code);

                    // If the index references the array, extract it to a temporary variable first
                    if index_refs_array {
                        // Generate a temporary variable name based on the array name
                        // Extract the variable name from base_code (e.g., "self.array" -> "array")
                        let temp_index_var = if base_code.starts_with("self.") {
                            let var_name = base_code.strip_prefix("self.").unwrap_or(&base_code);
                            format!("__{}_idx", var_name.replace(".", "_"))
                        } else {
                            format!("__{}_idx", base_code.replace(".", "_"))
                        };
                        let bounds_check = if is_dynamic_array {
                            format!(
                                "        if {}.len() <= {} {{\n            {}.resize({} + 1, json!(null));\n        }}\n",
                                base_code, temp_index_var, base_code, temp_index_var
                            )
                        } else {
                            String::new()
                        };
                        format!(
                            "        let {} = {};\n{}{}[{}] = {};\n",
                            temp_index_var,
                            index_code,
                            bounds_check,
                            base_code,
                            temp_index_var,
                            final_rhs_wrapped
                        )
                    } else {
                        // Insert `.clone()` if needed, matching your member assign arm
                        let should_clone = !is_dynamic_array
                            && matches!(rhs_expr.expr, Expr::Ident(_) | Expr::MemberAccess(..))
                            && rhs_type.as_ref().map_or(false, |ty| ty.requires_clone());

                        if is_dynamic_array {
                            // For dynamic arrays, extract index and check bounds
                            let temp_index_var = format!(
                                "__idx_{}",
                                base_code.replace(".", "_").replace("self", "")
                            );
                            format!(
                                "        let {} = {};\n        if {}.len() <= {} {{\n            {}.resize({} + 1, json!(null));\n        }}\n        {}[{}] = {};\n",
                                temp_index_var,
                                index_code,
                                base_code,
                                temp_index_var,
                                base_code,
                                temp_index_var,
                                base_code,
                                temp_index_var,
                                final_rhs_wrapped
                            )
                        } else if should_clone {
                            format!(
                                "        {}[{}] = {}.clone();\n",
                                base_code, index_code, final_rhs_wrapped
                            )
                        } else {
                            format!(
                                "        {}[{}] = {};\n",
                                base_code, index_code, final_rhs_wrapped
                            )
                        }
                    }
                }
            }

            Stmt::IndexAssignOp(array_expr, index_expr, op, rhs_expr) => {
                let array_code = array_expr.to_rust(needs_self, script, None, current_func, None);
                // Ensure index is usize for array indexing
                let index_code_raw = index_expr.to_rust(
                    needs_self,
                    script,
                    Some(&Type::Number(NumberKind::Unsigned(32))),
                    current_func,
                    None, // index_expr is Expr, no span available
                );
                let index_code = format!("{} as usize", index_code_raw);

                let lhs_type = script.infer_expr_type(&array_expr, current_func);
                let rhs_type = script.infer_expr_type(&rhs_expr.expr, current_func);

                // Check if this is a dynamic array (Vec<Value>) that needs special handling
                let is_dynamic_array =
                    if let Some(Type::Container(ContainerKind::Array, inner_types)) = &lhs_type {
                        inner_types
                            .get(0)
                            .map_or(true, |t| *t == Type::Object || matches!(t, Type::Custom(_)))
                    } else {
                        false
                    };

                if is_dynamic_array {
                    // For dynamic arrays stored as Vec<Value>, operations need explicit casting
                    // This is a limitation - the user should cast the element first
                    format!(
                        "        // TODO: Dynamic array compound assignment - cast element to type first, do operation, then assign back as json!()\n"
                    )
                } else {
                    let rhs_code =
                        rhs_expr
                            .expr
                            .to_rust(needs_self, script, lhs_type.as_ref(), current_func, rhs_expr.span.as_ref());

                    // Special case: string += something becomes push_str.
                    if matches!(op, Op::Add) && lhs_type == Some(Type::String) {
                        return format!(
                            "        {}[{}].push_str({}.as_str());\n",
                            array_code, index_code, rhs_code
                        );
                    }

                    // Insert implicit cast if needed
                    let final_rhs = if let Some(lhs_ty) = &lhs_type {
                        if let Some(rhs_ty) = &rhs_type {
                            if rhs_ty.can_implicitly_convert_to(lhs_ty) && rhs_ty != lhs_ty {
                                script.generate_implicit_cast_for_expr(&rhs_code, rhs_ty, lhs_ty)
                            } else {
                                rhs_code
                            }
                        } else {
                            rhs_code
                        }
                    } else {
                        rhs_code
                    };

                    format!(
                        "        {}[{}] {}= {};\n",
                        array_code,
                        index_code,
                        op.to_rust_assign(),
                        final_rhs
                    )
                }
            }
        }
    }

    fn generate_implicit_cast(expr: &str, from_type: &Type, to_type: &Type) -> String {
        use NumberKind::*;
        use Type::*;

        if from_type == to_type {
            return expr.to_string();
        }

        match (from_type, to_type) {
            (Number(Float(32)), Number(Float(64))) => format!("({} as f64)", expr),
            (Number(Float(64)), Number(Float(32))) => format!("({} as f32)", expr),
            (Number(Signed(_) | Unsigned(_)), Number(Float(64))) => format!("({} as f64)", expr),
            (Number(Signed(_) | Unsigned(_)), Number(Float(32))) => format!("({} as f32)", expr),
            (Number(Signed(_)), Number(Signed(to_w))) => format!("({} as i{})", expr, to_w),
            (Number(Signed(_)), Number(Unsigned(to_w))) => format!("({} as u{})", expr, to_w),
            (Number(Unsigned(_)), Number(Unsigned(to_w))) => format!("({} as u{})", expr, to_w),
            (Number(Unsigned(_)), Number(NumberKind::BigInt)) => format!("BigInt::from({})", expr), // Added: Unsigned to BigInt
            (Number(Unsigned(_)), Number(Signed(to_w))) => format!("({} as i{})", expr, to_w),
            (Number(BigInt), Number(Signed(w))) => match w {
                32 => format!("{}.to_i32().unwrap_or_default()", expr),
                64 => format!("{}.to_i64().unwrap_or_default()", expr),
                _ => format!("({}.to_i64().unwrap_or_default() as i{})", expr, w),
            },
            (Number(Signed(_)), Number(BigInt)) => format!("BigInt::from({})", expr),
            (Number(Decimal), Number(Signed(w))) => match w {
                32 => format!("{}.to_i32().unwrap_or_default()", expr),
                64 => format!("{}.to_i64().unwrap_or_default()", expr),
                _ => format!("({}.to_i64().unwrap_or_default() as i{})", expr, w),
            },
            (Number(Signed(_) | Unsigned(_)), Number(Decimal)) => {
                format!("Decimal::from({})", expr)
            }

            // String type conversions
            (String, CowStr) => {
                format!("{}.into()", expr)
            }
            (StrRef, CowStr) => {
                format!("{}.into()", expr)
            }
            (CowStr, String) => {
                format!("{}.into_owned()", expr)
            }
            (CowStr, StrRef) => {
                format!("{}.as_ref()", expr)
            }
            // String -> StrRef conversion
            (String, StrRef) => {
                format!("{}.as_str()", expr)
            }
            // StrRef -> String conversion
            (StrRef, String) => {
                format!("{}.to_string()", expr)
            }
            // String/StrRef/CowStr -> Option<CowStr> conversions
            (String, Option(inner)) if matches!(inner.as_ref(), CowStr) => {
                // Check if expr is a string literal (direct or wrapped in String::from)
                let trimmed = expr.trim();
                if trimmed.starts_with('"') && trimmed.ends_with('"') {
                    // Direct string literal: "..." -> Some(Cow::Borrowed("..."))
                    format!("Some(Cow::Borrowed({}))", expr)
                } else if trimmed.starts_with("String::from(") && trimmed.ends_with(')') {
                    // String::from("...") -> extract literal and use Cow::Borrowed
                    let inner_section = &trimmed["String::from(".len()..trimmed.len() - 1].trim();
                    if inner_section.starts_with('"') && inner_section.ends_with('"') {
                        format!("Some(Cow::Borrowed({}))", inner_section)
                    } else {
                        format!("Some({}.into())", expr)
                    }
                } else {
                    // Variable or other expression: use .into()
                    format!("Some({}.into())", expr)
                }
            }
            (StrRef, Option(inner)) if matches!(inner.as_ref(), CowStr) => {
                // StrRef is already &'static str, so for literals use Cow::Borrowed
                let trimmed = expr.trim();
                if trimmed.starts_with('"') && trimmed.ends_with('"') {
                    format!("Some(Cow::Borrowed({}))", expr)
                } else {
                    format!("Some({}.into())", expr)
                }
            }
            (CowStr, Option(inner)) if matches!(inner.as_ref(), CowStr) => {
                format!("Some({})", expr)
            }
            // Option unwrapping: Option<T> -> T (when assigning to non-Option field)
            (Option(inner_from), to) if inner_from.as_ref() == to => {
                format!("{}.unwrap_or_default()", expr)
            }
            // Wrapping: T -> Option<T> (when assigning T to Option<T> field)
            (from, Option(inner_to)) if from == inner_to.as_ref() => {
                format!("Some({})", expr)
            }
            // Option conversion: Option<From> -> Option<To>
            (Option(inner_from), Option(inner_to)) => {
                // Convert the inner type first
                let inner_expr = format!("{}", expr);
                let inner_from_ty = inner_from.as_ref();
                let inner_to_ty = inner_to.as_ref();
                
                // Handle the inner conversion
                let converted_inner = match (inner_from_ty, inner_to_ty) {
                    (String, CowStr) => {
                        // Check if expr is a string literal
                        let trimmed = inner_expr.trim();
                        if trimmed.starts_with('"') && trimmed.ends_with('"') {
                            format!("Cow::Borrowed({})", inner_expr)
                        } else if trimmed.starts_with("String::from(") && trimmed.ends_with(')') {
                            let inner_section = &trimmed["String::from(".len()..trimmed.len() - 1].trim();
                            if inner_section.starts_with('"') && inner_section.ends_with('"') {
                                format!("Cow::Borrowed({})", inner_section)
                            } else {
                                format!("{}.into()", inner_expr)
                            }
                        } else {
                            format!("{}.into()", inner_expr)
                        }
                    }
                    (StrRef, CowStr) => {
                        let trimmed = inner_expr.trim();
                        if trimmed.starts_with('"') && trimmed.ends_with('"') {
                            format!("Cow::Borrowed({})", inner_expr)
                        } else {
                            format!("{}.into()", inner_expr)
                        }
                    }
                    (CowStr, CowStr) => inner_expr,
                    (CowStr, String) => format!("{}.into_owned()", inner_expr),
                    (CowStr, StrRef) => format!("{}.as_ref()", inner_expr),
                    _ if inner_from_ty == inner_to_ty => inner_expr,
                    _ => {
                        // For other conversions, recursively call generate_implicit_cast
                        // Note: This is a standalone function, so we call it directly
                        Self::generate_implicit_cast(&inner_expr, inner_from_ty, inner_to_ty)
                    }
                };
                
                // Wrap in Some() if not already wrapped
                if converted_inner.starts_with("Some(") {
                    converted_inner
                } else {
                    format!("Some({})", converted_inner)
                }
            }
            // Node types -> Uuid (nodes are Uuid IDs)
            (Node(_), Uuid) => {
                expr.to_string() // Already a Uuid, no conversion needed
            }
            // Uuid -> Node type (for type checking, just pass through)
            (Uuid, Node(_)) => {
                expr.to_string() // Already a Uuid, no conversion needed
            }

            _ => {
                eprintln!(
                    "Warning: Unhandled cast from {:?} to {:?}",
                    from_type, to_type
                );
                expr.to_string()
            }
        }
    }

    fn get_target_type(
        &self,
        name: &str,
        script: &Script,
        current_func: Option<&Function>,
    ) -> Option<Type> {
        if let Some(func) = current_func {
            if let Some(local) = func.locals.iter().find(|v| v.name == name) {
                // First try explicit type
                if let Some(typ) = &local.typ {
                    return Some(typ.clone());
                }
                // If no explicit type, try to infer from value
                if let Some(value) = &local.value {
                    return script.infer_expr_type(&value.expr, current_func);
                }
            }
            if let Some(param) = func.params.iter().find(|p| p.name == name) {
                return Some(param.typ.clone());
            }
        }

        if let Some((base, field)) = name.split_once('.') {
            if let Some(base_ty) = script.get_variable_type(base) {
                if let Some(field_ty) = script.get_member_type(base_ty, field) {
                    return Some(field_ty);
                }
            }
        }

        script.get_variable_type(name).cloned()
    }

    #[allow(dead_code)]
    fn contains_self(&self) -> bool {
        match self {
            Stmt::Expr(e) => e.contains_self(),
            Stmt::VariableDecl(var) => var.value.as_ref().map_or(false, |e| e.contains_self()),
            Stmt::Assign(_, e) | Stmt::AssignOp(_, _, e) => e.contains_self(),
            Stmt::MemberAssign(lhs, rhs) | Stmt::MemberAssignOp(lhs, _, rhs) => {
                lhs.contains_self() || rhs.contains_self()
            }
            Stmt::ScriptAssign(_, _, expr) | Stmt::ScriptAssignOp(_, _, _, expr) => {
                expr.contains_self()
            }
            Stmt::IndexAssign(array, index, value)
            | Stmt::IndexAssignOp(array, index, _, value) => {
                array.contains_self() || index.contains_self() || value.contains_self()
            }
            Stmt::Pass => false,
            Stmt::If {
                condition,
                then_body,
                else_body,
            } => {
                condition.contains_self()
                    || then_body.iter().any(|s| s.contains_self())
                    || else_body
                        .as_ref()
                        .map_or(false, |body| body.iter().any(|s| s.contains_self()))
            }
            Stmt::For { iterable, body, .. } => {
                iterable.contains_self() || body.iter().any(|s| s.contains_self())
            }
            Stmt::ForTraditional {
                init,
                condition,
                increment,
                body,
            } => {
                (init.as_ref().map_or(false, |s| s.as_ref().contains_self()))
                    || (condition.as_ref().map_or(false, |c| c.contains_self()))
                    || (increment
                        .as_ref()
                        .map_or(false, |s| s.as_ref().contains_self()))
                    || body.iter().any(|s| s.contains_self())
            }
        }
    }

    pub fn contains_api_call(&self, script: &Script) -> bool {
        match self {
            Stmt::Expr(e) => e.contains_api_call(script),
            Stmt::VariableDecl(v) => v
                .value
                .as_ref()
                .map_or(false, |e| e.contains_api_call(script)),
            Stmt::Assign(_, e) | Stmt::AssignOp(_, _, e) => e.contains_api_call(script),
            Stmt::MemberAssign(a, b) | Stmt::MemberAssignOp(a, _, b) => {
                a.contains_api_call(script) || b.contains_api_call(script)
            }
            Stmt::IndexAssign(array, index, value)
            | Stmt::IndexAssignOp(array, index, _, value) => {
                array.contains_api_call(script)
                    || index.contains_api_call(script)
                    || value.contains_api_call(script)
            }
            Stmt::ScriptAssign(_, _, e) | Stmt::ScriptAssignOp(_, _, _, e) => {
                e.contains_api_call(script)
            }
            Stmt::Pass => false,
            Stmt::If {
                condition,
                then_body,
                else_body,
            } => {
                condition.contains_api_call(script)
                    || then_body.iter().any(|s| s.contains_api_call(script))
                    || else_body.as_ref().map_or(false, |body| {
                        body.iter().any(|s| s.contains_api_call(script))
                    })
            }
            Stmt::For { iterable, body, .. } => {
                iterable.contains_api_call(script)
                    || body.iter().any(|s| s.contains_api_call(script))
            }
            Stmt::ForTraditional {
                init,
                condition,
                increment,
                body,
            } => {
                (init
                    .as_ref()
                    .map_or(false, |s| s.as_ref().contains_api_call(script)))
                    || (condition
                        .as_ref()
                        .map_or(false, |c| c.contains_api_call(script)))
                    || (increment
                        .as_ref()
                        .map_or(false, |s| s.as_ref().contains_api_call(script)))
                    || body.iter().any(|s| s.contains_api_call(script))
            }
        }
    }
}
