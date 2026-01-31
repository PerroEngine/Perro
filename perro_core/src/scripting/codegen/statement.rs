// Statement code generation
use super::analysis::{collect_cloned_node_vars, extract_node_member_info};
use super::utils::{TRANSPILED_IDENT, is_node_type, rename_variable, string_to_node_type};
use crate::ast::*;
use crate::node_registry::NodeType;
use crate::resource_modules::TextureResource;
use crate::scripting::ast::{ContainerKind, Expr, NumberKind, Op, Stmt, Type};
use crate::scripting::api_bindings::ModuleCodegen;
use crate::structs::engine_registry::ENGINE_REGISTRY;
use crate::structs::engine_structs::EngineStruct as EngineStructKind;

impl Stmt {
    #[allow(unused_assignments)] // temp_counter is read in next loop iteration
    pub fn to_rust(
        &self,
        needs_self: bool,
        script: &Script,
        current_func: Option<&Function>,
    ) -> String {
        match self {
            Stmt::Expr(expr) => {
                // ----------------------------------------------------------------
                // Auto-writeback sugar for "resource instance calls" used as statements
                //
                // Example (PUP):
                //   self.transform.rotation.rotate_x(2)
                //
                // Parser lowers instance-style resource calls to:
                //   Expr::ApiCall(CallModule::Resource(resource), [receiver, arg1, arg2, ...])
                //
                // When such a call is used as a *statement* (result ignored) and:
                // - the resource call returns the same type as the receiver, AND
                // - the receiver is an assignable node-field chain (self.transform.rotation, etc.), AND
                // - the other args don't require `api` or `self` access (so they can be captured into the mutate closure),
                //
                // then we treat it as an in-place update:
                //   api.mutate_node(self.id, |n| {
                //     let __tmp = n.transform.rotation;
                //     n.transform.rotation = <resource_call>(__tmp, ...);
                //   });
                //
                // This avoids hardcoding per-method behavior in expression codegen and keeps the sugar generic.
                // ----------------------------------------------------------------
                if let Expr::ApiCall(crate::call_modules::CallModule::Resource(resource), call_args) =
                    &expr.expr
                {
                    if let Some(receiver) = call_args.first() {
                        // Only apply when return type matches receiver type.
                        let recv_ty = script.infer_expr_type(receiver, current_func);
                        let ret_ty = resource.return_type();

                        if recv_ty.is_some()
                            && ret_ty.is_some()
                            && recv_ty == ret_ty
                            // Ensure other args are "closure-safe" (no api/self access).
                            && call_args
                                .iter()
                                .skip(1)
                                .all(|a| !a.contains_self() && !a.contains_api_call(script))
                        {
                            // Only apply when receiver is a node member chain we can assign into.
                            if let Some((node_id, node_type, field_path, closure_var)) =
                                extract_node_member_info(receiver, script, current_func)
                            {
                                // Skip DynNode for now (needs match-based mutation plumbing).
                                if node_type != "__DYN_NODE__" {
                                    // Resolve field names along the path for the concrete node type.
                                    if let Some(node_type_enum) = string_to_node_type(&node_type) {
                                        let fields: Vec<&str> = field_path.split('.').collect();
                                        if !fields.is_empty() {
                                            let resolved_fields: Vec<String> = fields
                                                .iter()
                                                .enumerate()
                                                .map(|(i, f)| {
                                                    if i == 0 {
                                                        ENGINE_REGISTRY
                                                            .resolve_field_name(&node_type_enum, f)
                                                    } else {
                                                        f.to_string()
                                                    }
                                                })
                                                .collect();
                                            let resolved_field_path = resolved_fields.join(".");

                                            // Only add self. prefix if node_id is a struct field (not a local).
                                            let node_id_with_self = if !node_id.starts_with("self.")
                                                && !node_id.starts_with("api.")
                                                && script.is_struct_field(&node_id)
                                            {
                                                format!("self.{}", node_id)
                                            } else {
                                                node_id.clone()
                                            };

                                            // Use the compiler-generated closure parameter name as-is.
                                            // User variables are always renamed with `__t_` so they cannot collide in Rust,
                                            // even if the user writes `var self_node`.
                                            let closure_param =
                                                closure_var.strip_prefix("self.").unwrap_or(&closure_var);

                                            // If receiver type is Copy, we can avoid the temp receiver variable.
                                            // For non-Copy types, the temp avoids borrow-checker issues on self-referential assignment.
                                            let receiver_is_copy = recv_ty
                                                .as_ref()
                                                .map(|t| t.is_copy_type())
                                                .unwrap_or(false);

                                            if receiver_is_copy {
                                                // For Copy receivers, emit A = A.method(...) (no temp receiver).
                                                // We generate the RHS via the resource module codegen but provide arg0 as a raw string
                                                // (`<closure_param>.<field_path>`) so it doesn't get renamed.
                                                use crate::api_bindings::generate_rust_args;

                                                let receiver_str =
                                                    format!("{}.{}", closure_param, resolved_field_path);

                                                let expected = resource.param_types();
                                                let expected_rest: Option<Vec<Type>> =
                                                    expected.as_ref().map(|v| v.iter().skip(1).cloned().collect());
                                                let rest_args: Vec<Expr> =
                                                    call_args.iter().skip(1).cloned().collect();
                                                let mut rest_strs = generate_rust_args(
                                                    &rest_args,
                                                    script,
                                                    needs_self,
                                                    current_func,
                                                    expected_rest.as_ref(),
                                                );

                                                let mut args_strs: Vec<String> =
                                                    Vec::with_capacity(1 + rest_strs.len());
                                                args_strs.push(receiver_str);
                                                args_strs.append(&mut rest_strs);

                                                // Reuse the same routing as ResourceModule::to_rust, but with our custom arg0 string.
                                                let rhs_code = match resource {
                                                    crate::resource_modules::ResourceModule::Signal(api) => api.to_rust_prepared(&rest_args, &args_strs, script, needs_self, current_func),
                                                    crate::resource_modules::ResourceModule::Texture(api) => api.to_rust_prepared(&rest_args, &args_strs, script, needs_self, current_func),
                                                    crate::resource_modules::ResourceModule::Mesh(api) => api.to_rust_prepared(&rest_args, &args_strs, script, needs_self, current_func),
                                                    crate::resource_modules::ResourceModule::Shape(api) => api.to_rust_prepared(&rest_args, &args_strs, script, needs_self, current_func),
                                                    crate::resource_modules::ResourceModule::ArrayOp(api) => api.to_rust_prepared(&rest_args, &args_strs, script, needs_self, current_func),
                                                    crate::resource_modules::ResourceModule::MapOp(api) => api.to_rust_prepared(&rest_args, &args_strs, script, needs_self, current_func),
                                                    crate::resource_modules::ResourceModule::QuaternionOp(api) => api.to_rust_prepared(&rest_args, &args_strs, script, needs_self, current_func),
                                                };

                                        return format!(
                                            "        api.mutate_node({}, |{}: &mut {}| {{ {}.{} = {}; }});\n",
                                            node_id_with_self,
                                            closure_param,
                                            node_type,
                                            closure_param,
                                            resolved_field_path,
                                            rhs_code
                                        );
                                    } else {
                                        // Build a new args list where arg0 is a temp local inside the closure.
                                        let mut new_args: Vec<Expr> =
                                            Vec::with_capacity(call_args.len());
                                        new_args.push(Expr::Ident("__t_recv_tmp".to_string()));
                                        new_args.extend(call_args.iter().skip(1).cloned());

                                        // Generate the resource call RHS using the temp receiver.
                                        let rhs_code = resource.to_rust(
                                            &new_args,
                                            script,
                                            /*needs_self*/ false,
                                            current_func,
                                        );

                                        return format!(
                                            "        api.mutate_node({}, |{}: &mut {}| {{ let __t_recv_tmp = {}.{}; {}.{} = {}; }});\n",
                                            node_id_with_self,
                                            closure_param,
                                            node_type,
                                            closure_param,
                                            resolved_field_path,
                                            closure_param,
                                            resolved_field_path,
                                            rhs_code
                                        );
                                    }
                                        }
                                    }
                                }
                            }
                        }
                    }
                }

                // SECOND PASS: Extract nested API calls to avoid borrow checker issues
                // This handles cases like api.call_function_id(api.get_parent(collision_id), ...)
                // Only extracts NESTED API calls, not top-level ones
                let mut extracted_api_calls = Vec::new();
                let mut temp_var_types: std::collections::HashMap<String, Type> =
                    std::collections::HashMap::new();

                // Use a deterministic counter for temp variable names to enable incremental compilation
                let mut temp_counter = 0usize;

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
                    temp_counter: &mut usize,
                ) -> Expr {
                    match expr {
                        // Node member access chains can lower into mutable API getters (e.g. global_transform).
                        // Hoist those into temps when nested, so we never end up with `api.*( ... api.get_*() ... )`
                        // which can trip Rust's evaluation/borrow order (receiver borrow happens before args).
                        Expr::MemberAccess(_base, _field) if !is_top_level => {
                            if let Some((node_id, node_type, field_path, _closure_var)) =
                                extract_node_member_info(expr, script, current_func)
                            {
                                if let Some(node_type_enum) = string_to_node_type(&node_type) {
                                    let first = field_path.split('.').next().unwrap_or("");
                                    if !first.is_empty() {
                                        use crate::scripting::lang::pup::node_api::PUP_NODE_API;
                                        if let Some(api_field) = PUP_NODE_API
                                            .get_fields(&node_type_enum)
                                            .iter()
                                            .find(|f| f.script_name == first)
                                        {
                                            if let Some(read_behavior) = ENGINE_REGISTRY
                                                .get_field_read_behavior(&api_field.rust_field)
                                            {
                                                // Deduplicate identical getter calls within the same statement:
                                                // use a stable hash-based temp name and only declare once.
                                                use std::collections::hash_map::DefaultHasher;
                                                use std::hash::{Hash, Hasher};

                                                let node_id_with_self = if !node_id.starts_with("self.")
                                                    && !node_id.starts_with("api.")
                                                    && script.is_struct_field(&node_id)
                                                {
                                                    format!("self.{}", node_id)
                                                } else {
                                                    node_id.clone()
                                                };

                                                let (getter_call, getter_type) = match read_behavior {
                                                    crate::structs::engine_registry::NodeFieldReadBehavior::GlobalTransform2D => (
                                                        format!(
                                                            "api.get_global_transform({}).unwrap_or_default()",
                                                            node_id_with_self
                                                        ),
                                                        Type::EngineStruct(EngineStructKind::Transform2D),
                                                    ),
                                                    crate::structs::engine_registry::NodeFieldReadBehavior::GlobalTransform3D => (
                                                        format!(
                                                            "api.get_global_transform_3d({}).unwrap_or_default()",
                                                            node_id_with_self
                                                        ),
                                                        Type::EngineStruct(EngineStructKind::Transform3D),
                                                    ),
                                                };

                                                let mut hasher = DefaultHasher::new();
                                                getter_call.hash(&mut hasher);
                                                let temp_var = format!("__temp_read_{}", hasher.finish());

                                                if !temp_var_types.contains_key(&temp_var) {
                                                    extracted.push((
                                                        format!(
                                                            "let {}: {} = {};",
                                                            temp_var,
                                                            getter_type.to_rust_type(),
                                                            getter_call
                                                        ),
                                                        temp_var.clone(),
                                                    ));
                                                    temp_var_types.insert(temp_var.clone(), getter_type);
                                                }

                                                // Rebuild the expression as `temp_var.<rest>`
                                                let rest = field_path
                                                    .strip_prefix(first)
                                                    .unwrap_or("")
                                                    .strip_prefix('.')
                                                    .unwrap_or("");
                                                let mut out = Expr::Ident(temp_var);
                                                if !rest.is_empty() {
                                                    for seg in rest.split('.') {
                                                        out = Expr::MemberAccess(
                                                            Box::new(out),
                                                            seg.to_string(),
                                                        );
                                                    }
                                                }
                                                return out;
                                            }
                                        }
                                    }
                                }
                            }

                            // Fall through to normal recursive behavior below (handled by MemberAccess arm).
                            let Expr::MemberAccess(base, field) = expr else { return expr.clone(); };
                            let new_base = extract_all_nested_api_calls(
                                base,
                                script,
                                current_func,
                                extracted,
                                temp_var_types,
                                needs_self,
                                false,
                                temp_counter,
                            );
                            return Expr::MemberAccess(Box::new(new_base), field.clone());
                        }
                        // Extract API calls (like api.get_parent, api.call_function_id, etc.)
                        Expr::ApiCall(api_module, api_args) => {
                            // First, recursively extract nested API calls from arguments
                            let new_args: Vec<Expr> = api_args
                                .iter()
                                .map(|arg| {
                                    extract_all_nested_api_calls(
                                        arg,
                                        script,
                                        current_func,
                                        extracted,
                                        temp_var_types,
                                        needs_self,
                                        false,
                                        temp_counter,
                                    )
                                })
                                .collect();

                            // Never extract Void-returning API calls (like Console.print_info)
                            let return_type = api_module.return_type();
                            if let Some(Type::Void) = return_type {
                                // Void-returning calls should never be extracted, just return with processed arguments
                                return Expr::ApiCall(api_module.clone(), new_args);
                            }

                            // Check if this API call returns an ID type (Uuid, NodeType, etc.) that should be extracted
                            let should_extract_top_level = is_top_level
                                && {
                                    if let Some(return_type) = return_type {
                                        matches!(return_type, Type::NodeType | Type::DynNode)
                                            || matches!(return_type, Type::Option(boxed) if matches!(boxed.as_ref(), Type::DynNode))
                                    } else {
                                        false
                                    }
                                };

                            // If this is a top-level API call that doesn't need extraction, just return it with processed arguments
                            if is_top_level && !should_extract_top_level {
                                return Expr::ApiCall(api_module.clone(), new_args);
                            }

                            // Generate deterministic temp variable name using occurrence counter
                            let current_index = *temp_counter;
                            *temp_counter += 1;
                            let temp_var = format!("temp_api_var_{}", current_index);

                            // Generate the API call code with extracted arguments
                            let mut api_call_str =
                                api_module.to_rust(&new_args, script, needs_self, current_func);

                            // Fix any incorrect renaming of "api" to "__t_api" or "t_id_api"
                            api_call_str = api_call_str
                                .replace("__t_api.", "api.")
                                .replace("t_id_api.", "api.");

                            // Infer the return type for the temp variable
                            let inferred_type = api_module.return_type();
                            let type_annotation = inferred_type
                                .as_ref()
                                .map(|t| {
                                    // Special case: Texture (EngineStruct) returns Option<TextureID>
                                    let rust_type = match t {
                                        Type::EngineStruct(EngineStructKind::Texture) => {
                                            "Option<TextureID>".to_string()
                                        }
                                        _ => t.to_rust_type(),
                                    };
                                    format!(": {}", rust_type)
                                })
                                .unwrap_or_default();

                            // Store the type for this temp variable
                            if let Some(ty) = inferred_type {
                                temp_var_types.insert(temp_var.clone(), ty);
                            }

                            extracted.push((
                                format!("let {}{} = {};", temp_var, type_annotation, api_call_str),
                                temp_var.clone(),
                            ));

                            // Return an identifier expression for the temp variable
                            Expr::Ident(temp_var)
                        }
                        Expr::Call(target, args) => {
                            // Recursively extract API calls from target and all arguments
                            let new_target = extract_all_nested_api_calls(
                                target,
                                script,
                                current_func,
                                extracted,
                                temp_var_types,
                                needs_self,
                                false,
                                temp_counter,
                            );
                            let new_args: Vec<Expr> = args
                                .iter()
                                .map(|arg| {
                                    extract_all_nested_api_calls(
                                        arg,
                                        script,
                                        current_func,
                                        extracted,
                                        temp_var_types,
                                        needs_self,
                                        false,
                                        temp_counter,
                                    )
                                })
                                .collect();
                            Expr::Call(Box::new(new_target), new_args)
                        }
                        Expr::MemberAccess(base, field) => {
                            // Recursively extract API calls from base
                            let new_base = extract_all_nested_api_calls(
                                base,
                                script,
                                current_func,
                                extracted,
                                temp_var_types,
                                needs_self,
                                false,
                                temp_counter,
                            );
                            Expr::MemberAccess(Box::new(new_base), field.clone())
                        }
                        Expr::BinaryOp(left, op, right) => {
                            let new_left = extract_all_nested_api_calls(
                                left,
                                script,
                                current_func,
                                extracted,
                                temp_var_types,
                                needs_self,
                                false,
                                temp_counter,
                            );
                            let new_right = extract_all_nested_api_calls(
                                right,
                                script,
                                current_func,
                                extracted,
                                temp_var_types,
                                needs_self,
                                false,
                                temp_counter,
                            );
                            Expr::BinaryOp(Box::new(new_left), op.clone(), Box::new(new_right))
                        }
                        Expr::Cast(inner, target_type) => {
                            let new_inner = extract_all_nested_api_calls(
                                inner,
                                script,
                                current_func,
                                extracted,
                                temp_var_types,
                                needs_self,
                                false,
                                temp_counter,
                            );
                            Expr::Cast(Box::new(new_inner), target_type.clone())
                        }
                        Expr::Index(array, index) => {
                            let new_array = extract_all_nested_api_calls(
                                array,
                                script,
                                current_func,
                                extracted,
                                temp_var_types,
                                needs_self,
                                false,
                                temp_counter,
                            );
                            let new_index = extract_all_nested_api_calls(
                                index,
                                script,
                                current_func,
                                extracted,
                                temp_var_types,
                                needs_self,
                                false,
                                temp_counter,
                            );
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
                    &mut temp_counter,
                );

                // Generate the expression string from the modified expression
                let expr_str = modified_expr.to_rust(needs_self, script, None, current_func, None);

                // Combine all temp declarations on the same line
                let combined_temp_decl = if !extracted_api_calls.is_empty() {
                    Some(
                        extracted_api_calls
                            .iter()
                            .map(|(decl, _): &(String, String)| decl.clone())
                            .collect::<Vec<_>>()
                            .join(" "),
                    )
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
                    let raw_expr = expr.expr.to_rust(
                        needs_self,
                        script,
                        var.typ.as_ref(),
                        current_func,
                        expr.span.as_ref(),
                    );

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
                    // Note: read_node/read_scene_node return Clone types; the .clone() (or Cow::Owned) inside the closure
                    // already produces an owned value, so we don't add an extra .clone()
                    let already_owned = raw_expr.contains(".unwrap_or_default()")
                        || raw_expr.contains(".unwrap()")
                        || raw_expr.contains("::from_str")
                        || raw_expr.contains("::from(")
                        || raw_expr.contains("::new(")
                        || raw_expr.contains("get_element_clone")
                        || raw_expr.contains("read_node(")
                        || raw_expr.contains("read_scene_node(");

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
                let (temp_stmt, final_expr_str) = if expr_str.contains("let __")
                    && (expr_str.contains("; api.") || expr_str.contains(";api."))
                {
                    // Extract the temporary variable declaration and the actual expression
                    // Look for any API call after the temp declaration
                    let semi_pos = expr_str.find("; api.").or_else(|| expr_str.find(";api."));
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

                // Add type annotation if variable has explicit type OR if we can infer from the expression.
                // Do not annotate when RHS is a DynNode match (match api.get_type(...)) so the compiler can infer Vector2 vs Vector3 etc.
                let inferred_type = if let Some(expr) = &var.value {
                    script.infer_expr_type(&expr.expr, current_func)
                } else {
                    None
                };

                // Helper to convert type to Rust type annotation
                // Special case: Texture (EngineStruct) becomes Option<TextureID> in Rust
                let type_to_rust_annotation = |typ: &Type| -> String {
                    match typ {
                        Type::EngineStruct(EngineStructKind::Texture) => {
                            "Option<TextureID>".to_string()
                        }
                        _ => typ.to_rust_type(),
                    }
                };

                let type_annotation = if let Some(typ) = &var.typ {
                    format!(": {}", type_to_rust_annotation(typ))
                } else if let Some(ref inferred) = inferred_type {
                    // DynNode match: omit type so compiler infers, unless we unified (Vector2->Vector3 or f32->Quaternion)
                    if final_expr_str.contains("match api.get_type(") {
                        if final_expr_str.contains("Vector3::new(") {
                            ": Vector3".to_string()
                        } else if final_expr_str.contains("Quaternion::from_rotation_2d(") {
                            ": Quaternion".to_string()
                        } else {
                            String::new()
                        }
                    } else {
                        format!(": {}", type_to_rust_annotation(inferred))
                    }
                } else {
                    String::new()
                };

                // When declared type (e.g. Vector2) differs from RHS type (e.g. Vector3), emit implicit conversion
                let rhs_emit = if let (Some(lhs_ty), Some(rhs_ty)) =
                    (var.typ.as_ref(), inferred_type.as_ref())
                {
                    if rhs_ty.can_implicitly_convert_to(lhs_ty) && rhs_ty != lhs_ty {
                        script.generate_implicit_cast_for_expr(&final_expr_str, rhs_ty, lhs_ty)
                    } else {
                        final_expr_str.clone()
                    }
                } else {
                    final_expr_str.clone()
                };

                // Use inferred type for renaming if var.typ is None
                let type_for_renaming = var.typ.as_ref().or(inferred_type.as_ref());
                let renamed_name = rename_variable(&var.name, type_for_renaming);

                // If we extracted a temporary statement, prepend it on the same line
                if let Some(ref temp_stmt) = temp_stmt {
                    if expr_str.is_empty() {
                        format!(
                            "        {} let mut {}{};\n",
                            temp_stmt.trim_end(),
                            renamed_name,
                            type_annotation
                        )
                    } else {
                        format!(
                            "        {} let mut {}{} = {};\n",
                            temp_stmt.trim_end(),
                            renamed_name,
                            type_annotation,
                            rhs_emit
                        )
                    }
                } else if expr_str.is_empty() {
                    format!("        let mut {}{};\n", renamed_name, type_annotation)
                } else {
                    format!(
                        "        let mut {}{} = {};\n",
                        renamed_name, type_annotation, rhs_emit
                    )
                }
            }
            Stmt::Assign(name, expr) => {
                // Check if this is a constant that can't be reassigned
                if let Some(var) = script.variables.iter().find(|v| v.name == *name) {
                    if var.is_const {
                        return format!("        // ERROR: Cannot assign to constant '{}'\n", name);
                    }
                }
                // Module scope: cannot assign to module-level constants
                if let Some(ref scope_vars) = script.module_scope_variables {
                    if scope_vars.iter().any(|v| v.name == *name) {
                        return format!(
                            "        // ERROR: Cannot assign to module constant '{}'\n",
                            name
                        );
                    }
                }
                // Also check in current function locals
                if let Some(func) = current_func {
                    if let Some(var) = func.locals.iter().find(|v| v.name == *name) {
                        if var.is_const {
                            return format!(
                                "        // ERROR: Cannot assign to constant '{}'\n",
                                name
                            );
                        }
                    }
                }

                let var_type = script.get_variable_type(name);
                let expr_type = script.infer_expr_type(&expr.expr, current_func);

                // Check if the expression returns a UUID that represents a node or texture
                // (e.g., get_parent(), get_child_by_name(), Texture.load(), casts to node types, etc.)
                // OR if it returns NodeType or DynNode (which are also node UUID types)
                let is_direct_node_call = matches!(
                    &expr.expr,
                    Expr::ApiCall(
                        crate::call_modules::CallModule::NodeMethod(
                            crate::structs::engine_registry::NodeMethodRef::GetParent
                        ),
                        _
                    ) | Expr::ApiCall(
                        crate::call_modules::CallModule::NodeMethod(
                            crate::structs::engine_registry::NodeMethodRef::GetChildByName
                        ),
                        _
                    )
                );

                let is_direct_texture_call = matches!(
                    &expr.expr,
                    Expr::ApiCall(
                        crate::call_modules::CallModule::Resource(
                            crate::resource_modules::ResourceModule::Texture(TextureResource::Load)
                        ),
                        _
                    ) | Expr::ApiCall(
                        crate::call_modules::CallModule::Resource(
                            crate::resource_modules::ResourceModule::Texture(
                                TextureResource::Preload
                            )
                        ),
                        _
                    ) | Expr::ApiCall(
                        crate::call_modules::CallModule::Resource(
                            crate::resource_modules::ResourceModule::Texture(
                                TextureResource::CreateFromBytes
                            )
                        ),
                        _
                    )
                );

                let is_node_cast = matches!(expr_type, Some(Type::DynNode))
                    && if let Expr::Cast(_, ref target_type) = expr.expr {
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
                let is_id_type = matches!(var_type, Some(Type::DynNode))
                    || matches!(expr_type.as_ref(), Some(Type::DynNode))
                    || matches!(expr_type.as_ref(), Some(Type::Option(boxed)) if matches!(boxed.as_ref(), Type::DynNode));

                let type_for_renaming = if is_id_uuid && is_id_type {
                    // For node calls returning Uuid, treat as node type for naming
                    // For texture calls returning Texture (EngineStruct), use the actual type
                    if is_direct_texture_call {
                        expr_type.as_ref().or(var_type)
                    } else {
                        Some(&Type::Node(NodeType::Node)) // Treat as node type for naming
                    }
                } else if is_node_type_return
                    || matches!(var_type, Some(Type::NodeType | Type::DynNode))
                {
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
                // Use a deterministic counter for temp variable names to enable incremental compilation
                let mut temp_counter = 0usize;

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
                                    let needs_extraction = matches!(return_type, Type::DynNode)
                                        || matches!(return_type, Type::Option(ref boxed) if matches!(boxed.as_ref(), Type::DynNode));

                                    if needs_extraction {
                                        has_nested = true;

                                        // Generate the inner call string - this should generate "api.get_parent(...)"
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

                                        // Generate deterministic temp variable name using occurrence counter
                                        let current_index = temp_counter;
                                        temp_counter += 1;
                                        let temp_var = format!("temp_api_var_{}", current_index);

                                        // Only add temp declaration if we haven't seen this temp var yet
                                        if !temp_decls.iter().any(|(var, _)| *var == temp_var) {
                                            let type_annotation = if matches!(
                                                return_type,
                                                Type::DynNode
                                            ) {
                                                ": NodeID"
                                            } else if matches!(return_type, Type::Option(ref boxed) if matches!(boxed.as_ref(), Type::DynNode))
                                            {
                                                ": Option<NodeID>"
                                            } else {
                                                ""
                                            };
                                            temp_decls.push((
                                                temp_var.clone(),
                                                format!(
                                                    "let {}{} = {};",
                                                    temp_var, type_annotation, inner_call_str
                                                ),
                                            ));
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
                            let all_temp_decls = temp_decls
                                .iter()
                                .map(|(_, decl)| decl.clone())
                                .collect::<Vec<_>>()
                                .join(" ");
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
                                    let needs_extraction = matches!(return_type, Type::DynNode)
                                        || matches!(return_type, Type::Option(ref boxed) if matches!(boxed.as_ref(), Type::DynNode));

                                    if needs_extraction {
                                        has_nested = true;

                                        // Generate the inner call string - this should generate "api.get_parent(...)"
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

                                        // Generate deterministic temp variable name using occurrence counter
                                        let current_index = temp_counter;
                                        temp_counter += 1;
                                        let temp_var = format!("temp_api_var_{}", current_index);

                                        // Only add temp declaration if we haven't seen this temp var yet
                                        if !temp_decls.iter().any(|(var, _)| *var == temp_var) {
                                            let type_annotation = if matches!(
                                                return_type,
                                                Type::DynNode
                                            ) {
                                                ": NodeID"
                                            } else if matches!(return_type, Type::Option(ref boxed) if matches!(boxed.as_ref(), Type::DynNode))
                                            {
                                                ": Option<NodeID>"
                                            } else {
                                                ""
                                            };
                                            temp_decls.push((
                                                temp_var.clone(),
                                                format!(
                                                    "let {}{} = {};",
                                                    temp_var, type_annotation, inner_call_str
                                                ),
                                            ));
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
                            let all_temp_decls = temp_decls
                                .iter()
                                .map(|(_, decl)| decl.clone())
                                .collect::<Vec<_>>()
                                .join(" ");
                            (Some(all_temp_decls), Some(new_expr))
                        } else {
                            (None, None)
                        }
                    }
                    _ => (None, None),
                };

                // Generate the expression string - use modified expression if we have one, otherwise use original
                let expr_str = if let Some(ref modified) = modified_expr {
                    modified.to_rust(needs_self, script, target_type.as_ref(), current_func, None)
                } else {
                    expr.expr.to_rust(
                        needs_self,
                        script,
                        target_type.as_ref(),
                        current_func,
                        expr.span.as_ref(),
                    )
                };

                // If we didn't catch it at AST level, try string-based detection as fallback
                let (temp_decl_opt, mut final_expr_str) = if temp_decl_opt.is_none() {
                    // Check if the expression string already contains an embedded temp declaration
                    // Pattern: "let __parent_id = api.get_parent(...); api.read_node(...)"
                    // or "let __parent_id: Uuid = api.get_parent(...); api.read_node(...)"
                    if expr_str.starts_with("let __")
                        && (expr_str.contains("; api.") || expr_str.contains(";api."))
                    {
                        // Extract the temp declaration and the actual expression
                        let semi_pos = expr_str.find("; api.").or_else(|| expr_str.find(";api."));
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
                    else if (expr_str.contains("api.get_parent(")
                        || expr_str.contains("api.get_child_by_name(")
                        || expr_str.contains("t_id_api.get_parent(")
                        || expr_str.contains("t_id_api.get_child_by_name("))
                        && (expr_str.matches("api.").count() > 1
                            || expr_str.matches("t_id_api.").count() > 0)
                    {
                        // Find the inner API call - check for both "api.get_parent(" and "t_id_api.get_parent("
                        let inner_start = expr_str
                            .find("api.get_parent(")
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
                            let fixed_inner_call = inner_call
                                .replace("__t_api.", "api.")
                                .replace("t_id_api.", "api.");
                            if inner_call.starts_with("temp_api_var_") && !inner_call.contains("(")
                            {
                                // It's already a temp variable, don't redeclare
                                (None, expr_str)
                            } else {
                                // Extract get_parent/get_child_by_name (and other nested API calls) to a temp to avoid borrow checker errors (e.g. api.read_node(api.get_parent(...), ...))
                                let current_index = temp_counter;
                                temp_counter = current_index + 1;
                                let temp_var = format!("temp_api_var_{}", current_index);

                                // Check if we're trying to assign temp_var to itself
                                if fixed_inner_call == temp_var {
                                    (None, expr_str)
                                } else if expr_str.contains(&format!("let {} =", temp_var)) {
                                    // Already declared earlier, just replace the inner call
                                    let final_expr = expr_str.replace(inner_call, &temp_var);
                                    (None, final_expr)
                                } else {
                                    // Determine type annotation based on the call
                                    let type_annotation = if fixed_inner_call.contains("get_parent")
                                        || fixed_inner_call.contains("get_child_by_name")
                                    {
                                        ": NodeID"
                                    } else {
                                        ""
                                    };
                                    let temp_decl = format!(
                                        "let {}{} = {};",
                                        temp_var, type_annotation, fixed_inner_call
                                    );
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
                // Check if this is a constant that can't be reassigned
                if let Some(var) = script.variables.iter().find(|v| v.name == *name) {
                    if var.is_const {
                        return format!("        // ERROR: Cannot assign to constant '{}'\n", name);
                    }
                }
                // Module scope: cannot assign to module-level constants
                if let Some(ref scope_vars) = script.module_scope_variables {
                    if scope_vars.iter().any(|v| v.name == *name) {
                        return format!(
                            "        // ERROR: Cannot assign to module constant '{}'\n",
                            name
                        );
                    }
                }
                // Also check in current function locals
                if let Some(func) = current_func {
                    if let Some(var) = func.locals.iter().find(|v| v.name == *name) {
                        if var.is_const {
                            return format!(
                                "        // ERROR: Cannot assign to constant '{}'\n",
                                name
                            );
                        }
                    }
                }

                let var_type = script.get_variable_type(name);
                let expr_type = script.infer_expr_type(&expr.expr, current_func);

                // Check if the expression returns a UUID that represents a node or texture
                let is_direct_node_call = matches!(
                    &expr.expr,
                    Expr::ApiCall(
                        crate::call_modules::CallModule::NodeMethod(
                            crate::structs::engine_registry::NodeMethodRef::GetParent
                        ),
                        _
                    ) | Expr::ApiCall(
                        crate::call_modules::CallModule::NodeMethod(
                            crate::structs::engine_registry::NodeMethodRef::GetChildByName
                        ),
                        _
                    )
                );

                let is_direct_texture_call = matches!(
                    &expr.expr,
                    Expr::ApiCall(
                        crate::call_modules::CallModule::Resource(
                            crate::resource_modules::ResourceModule::Texture(TextureResource::Load)
                        ),
                        _
                    ) | Expr::ApiCall(
                        crate::call_modules::CallModule::Resource(
                            crate::resource_modules::ResourceModule::Texture(
                                TextureResource::Preload
                            )
                        ),
                        _
                    ) | Expr::ApiCall(
                        crate::call_modules::CallModule::Resource(
                            crate::resource_modules::ResourceModule::Texture(
                                TextureResource::CreateFromBytes
                            )
                        ),
                        _
                    )
                );

                let is_node_cast = matches!(expr_type, Some(Type::DynNode))
                    && if let Expr::Cast(_, ref target_type) = expr.expr {
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
                let is_id_type = matches!(var_type, Some(Type::DynNode))
                    || matches!(expr_type.as_ref(), Some(Type::DynNode))
                    || matches!(expr_type.as_ref(), Some(Type::Option(boxed)) if matches!(boxed.as_ref(), Type::DynNode));

                let type_for_renaming = if is_id_uuid && is_id_type {
                    if is_direct_texture_call {
                        expr_type.as_ref().or(var_type)
                    } else {
                        Some(&Type::Node(NodeType::Node))
                    }
                } else if is_node_type_return
                    || matches!(var_type, Some(Type::NodeType | Type::DynNode))
                {
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
                let expr_str = expr.expr.to_rust(
                    needs_self,
                    script,
                    target_type.as_ref(),
                    current_func,
                    expr.span.as_ref(),
                );

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
                    // Clean closure_var (remove self. prefix) and ensure node_id has self. prefix only if it's a struct field
                    let clean_closure_var =
                        closure_var.strip_prefix("self.").unwrap_or(&closure_var);
                    // Only add self. prefix if node_id is actually a struct field, not a local variable
                    let node_id_with_self = if !node_id.starts_with("self.")
                        && !node_id.starts_with("api.")
                        && script.is_struct_field(&node_id)
                    {
                        format!("self.{}", node_id)
                    } else {
                        node_id.clone()
                    };
                    // Check if this is a DynNode (special marker)
                    if node_type == "__DYN_NODE__" {
                        // Build field_path_vec from field_path
                        let field_path_vec: Vec<&str> = field_path.split('.').collect();
                        // Check if the field is on the base Node type - if so, use mutate_scene_node
                        let first_field = field_path_vec.first().map(|s| *s).unwrap_or("");
                        let is_base_node_field = ENGINE_REGISTRY
                            .get_field_type_node(&NodeType::Node, first_field)
                            .is_some();

                        // If it's a single field on the base Node type, use mutate_scene_node
                        // This works for any node type, even if we don't know the specific type
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
                                // Generate RHS code
                                let lhs_type = script.infer_expr_type(&lhs_expr.expr, current_func);
                                let rhs_type = script.infer_expr_type(&rhs_expr.expr, current_func);

                                // Extract API calls from RHS (simplified - just handle top-level API calls)
                                let mut extracted_api_calls = Vec::new();
                                let mut temp_counter = 0usize;

                                let modified_rhs_expr =
                                    if let Expr::ApiCall(api_module, api_args) = &rhs_expr.expr {
                                        let current_index = temp_counter;
                                        temp_counter = current_index + 1;
                                        let temp_var = format!("__temp_api_{}", current_index);
                                        let mut api_call_str = api_module.to_rust(
                                            api_args,
                                            script,
                                            needs_self,
                                            current_func,
                                        );
                                        api_call_str = api_call_str
                                            .replace("__t_api.", "api.")
                                            .replace("t_id_api.", "api.");
                                        let inferred_type = api_module.return_type();
                                        let type_annotation = inferred_type
                                            .as_ref()
                                            .map(|t| {
                                                // Special case: Texture (EngineStruct) returns Option<TextureID>
                                                let rust_type = match t {
                                                    Type::EngineStruct(
                                                        EngineStructKind::Texture,
                                                    ) => "Option<TextureID>".to_string(),
                                                    _ => t.to_rust_type(),
                                                };
                                                format!(": {}", rust_type)
                                            })
                                            .unwrap_or_default();
                                        extracted_api_calls.push((
                                            format!(
                                                "let {}{} = {};",
                                                temp_var, type_annotation, api_call_str
                                            ),
                                            temp_var.clone(),
                                        ));
                                        Expr::Ident(temp_var)
                                    } else {
                                        rhs_expr.expr.clone()
                                    };

                                let rhs_code = modified_rhs_expr.to_rust(
                                    needs_self,
                                    script,
                                    lhs_type.as_ref(),
                                    current_func,
                                    rhs_expr.span.as_ref(),
                                );

                                let is_literal = matches!(rhs_expr.expr, Expr::Literal(_));
                                let final_rhs = if let Some(lhs_ty) = &lhs_type {
                                    if let Some(rhs_ty) = &rhs_type {
                                        if !is_literal
                                            && rhs_ty.can_implicitly_convert_to(lhs_ty)
                                            && rhs_ty != lhs_ty
                                        {
                                            script.generate_implicit_cast_for_expr(
                                                &rhs_code, rhs_ty, lhs_ty,
                                            )
                                        } else {
                                            rhs_code
                                        }
                                    } else {
                                        rhs_code
                                    }
                                } else {
                                    rhs_code
                                };

                                // Get the expected type for this setter by looking up the field type
                                // The setter parameter type should match the field type
                                let expected_setter_type = ENGINE_REGISTRY
                                    .get_field_type_node(&NodeType::Node, first_field);

                                // Use type conversion to convert RHS to the expected type
                                let rhs_for_setter =
                                    if let Some(expected_type) = expected_setter_type {
                                        if let Some(rhs_ty) = &rhs_type {
                                            if rhs_ty.can_implicitly_convert_to(&expected_type)
                                                && rhs_ty != &expected_type
                                            {
                                                script.generate_implicit_cast_for_expr(
                                                    &final_rhs,
                                                    rhs_ty,
                                                    &expected_type,
                                                )
                                            } else {
                                                final_rhs.clone()
                                            }
                                        } else {
                                            final_rhs.clone()
                                        }
                                    } else {
                                        final_rhs.clone()
                                    };

                                let temp_decl = if !extracted_api_calls.is_empty() {
                                    Some(
                                        extracted_api_calls
                                            .iter()
                                            .map(|(decl, _)| decl.clone())
                                            .collect::<Vec<_>>()
                                            .join(" "),
                                    )
                                } else {
                                    None
                                };

                                let temp_decl_str = temp_decl
                                    .as_ref()
                                    .map(|d| format!("        {}\n", d))
                                    .unwrap_or_default();
                                return format!(
                                    "{}        api.mutate_scene_node({}, |n| {{ n.{}({}); }});\n",
                                    temp_decl_str, node_id_with_self, setter, rhs_for_setter
                                );
                            }
                        }

                        // Find all node types that have this field path
                        let field_path_vec_string: Vec<String> =
                            field_path_vec.iter().map(|s| s.to_string()).collect();
                        let compatible_node_types =
                            ENGINE_REGISTRY.narrow_nodes_by_fields(&field_path_vec_string);

                        if compatible_node_types.is_empty() {
                            // No compatible node types found, fallback to error
                            format!(
                                "        // ERROR: No compatible node types found for field path: {}\n",
                                field_path
                            )
                        } else {
                            // Generate RHS code once
                            let lhs_type = script.infer_expr_type(&lhs_expr.expr, current_func);
                            let rhs_type = script.infer_expr_type(&rhs_expr.expr, current_func);

                            // Extract ALL API calls from RHS expression to avoid borrow checker issues
                            // API calls inside mutate_node closures need to be extracted before the closure
                            let mut extracted_api_calls = Vec::new();
                            let mut temp_var_types: std::collections::HashMap<String, Type> =
                                std::collections::HashMap::new();

                            // Use a deterministic counter for temp variable names to enable incremental compilation
                            let mut temp_counter = 0usize;

                            // Helper function to extract API calls from expressions
                            fn extract_api_calls_from_expr_helper(
                                expr: &Expr,
                                script: &Script,
                                current_func: Option<&Function>,
                                extracted: &mut Vec<(String, String)>,
                                temp_var_types: &mut std::collections::HashMap<String, Type>,
                                needs_self: bool,
                                expected_type: Option<&Type>,
                                temp_counter: &mut usize,
                            ) -> Expr {
                                match expr {
                                    // Extract API calls (like Math.random_range, Texture.load, etc.)
                                    Expr::ApiCall(api_module, api_args) => {
                                        // First, recursively extract nested API calls from arguments
                                        let new_args: Vec<Expr> = api_args
                                            .iter()
                                            .map(|arg| {
                                                extract_api_calls_from_expr_helper(
                                                    arg,
                                                    script,
                                                    current_func,
                                                    extracted,
                                                    temp_var_types,
                                                    needs_self,
                                                    None,
                                                    temp_counter,
                                                )
                                            })
                                            .collect();

                                        // Generate deterministic temp variable name using occurrence counter
                                        let current_index = *temp_counter;
                                        *temp_counter += 1;

                                        // Extract ALL API calls, not just ones returning Uuid
                                        // This prevents borrow checker issues when API calls are inside closures
                                        let temp_var = format!("temp_api_var_{}", current_index);

                                        // Generate the API call code with extracted arguments
                                        let mut api_call_str = api_module.to_rust(
                                            &new_args,
                                            script,
                                            needs_self,
                                            current_func,
                                        );

                                        // Fix any incorrect renaming of "api" to "__t_api" or "t_id_api"
                                        api_call_str = api_call_str
                                            .replace("__t_api.", "api.")
                                            .replace("t_id_api.", "api.");

                                        // Infer the return type for the temp variable
                                        let inferred_type = api_module.return_type();
                                        let type_annotation = inferred_type
                                            .as_ref()
                                            .map(|t| {
                                                // Special case: Texture (EngineStruct) returns Option<TextureID>
                                                let rust_type = match t {
                                                    Type::EngineStruct(
                                                        EngineStructKind::Texture,
                                                    ) => "Option<TextureID>".to_string(),
                                                    _ => t.to_rust_type(),
                                                };
                                                format!(": {}", rust_type)
                                            })
                                            .unwrap_or_default();

                                        // Store the type for this temp variable
                                        if let Some(ty) = inferred_type {
                                            temp_var_types.insert(temp_var.clone(), ty);
                                        }

                                        extracted.push((
                                            format!(
                                                "let {}{} = {};",
                                                temp_var, type_annotation, api_call_str
                                            ),
                                            temp_var.clone(),
                                        ));

                                        // Return an identifier expression for the temp variable
                                        Expr::Ident(temp_var)
                                    }
                                    Expr::MemberAccess(base, field) => {
                                        // First, recursively extract API calls from base
                                        let new_base = extract_api_calls_from_expr_helper(
                                            base,
                                            script,
                                            current_func,
                                            extracted,
                                            temp_var_types,
                                            needs_self,
                                            None,
                                            temp_counter,
                                        );

                                        // Check if this member access would generate a read_node call
                                        let test_expr = Expr::MemberAccess(
                                            Box::new(new_base.clone()),
                                            field.clone(),
                                        );
                                        if let Some((_node_id, _, _, _)) = extract_node_member_info(
                                            &test_expr,
                                            script,
                                            current_func,
                                        ) {
                                            // This is a node member access - extract it to a temp variable
                                            // Generate deterministic temp variable name using occurrence counter
                                            let current_index = *temp_counter;
                                            *temp_counter += 1;
                                            let temp_var = format!("__temp_read_{}", current_index);

                                            // Generate the read_node call
                                            let read_code = test_expr.to_rust(
                                                needs_self,
                                                script,
                                                expected_type,
                                                current_func,
                                                None,
                                            );

                                            // Infer the type for the temp variable
                                            let inferred_type =
                                                script.infer_expr_type(&test_expr, current_func);
                                            let type_annotation = inferred_type
                                                .as_ref()
                                                .map(|t| {
                                                    // Special case: Texture (EngineStruct) returns Option<TextureID>
                                                    let rust_type = match t {
                                                        Type::EngineStruct(
                                                            EngineStructKind::Texture,
                                                        ) => "Option<TextureID>".to_string(),
                                                        _ => t.to_rust_type(),
                                                    };
                                                    format!(": {}", rust_type)
                                                })
                                                .unwrap_or_default();

                                            // Store the type for this temp variable so we can check if it needs cloning
                                            if let Some(ty) = inferred_type {
                                                temp_var_types.insert(temp_var.clone(), ty);
                                            }

                                            extracted.push((
                                                format!(
                                                    "let {}{} = {};",
                                                    temp_var, type_annotation, read_code
                                                ),
                                                temp_var.clone(),
                                            ));

                                            // Return an identifier expression for the temp variable
                                            Expr::Ident(temp_var)
                                        } else {
                                            // Not a node member access, return the member access with processed base
                                            Expr::MemberAccess(Box::new(new_base), field.clone())
                                        }
                                    }
                                    Expr::BinaryOp(left, op, right) => {
                                        let new_left = extract_api_calls_from_expr_helper(
                                            left,
                                            script,
                                            current_func,
                                            extracted,
                                            temp_var_types,
                                            needs_self,
                                            None,
                                            temp_counter,
                                        );
                                        let new_right = extract_api_calls_from_expr_helper(
                                            right,
                                            script,
                                            current_func,
                                            extracted,
                                            temp_var_types,
                                            needs_self,
                                            None,
                                            temp_counter,
                                        );
                                        Expr::BinaryOp(
                                            Box::new(new_left),
                                            op.clone(),
                                            Box::new(new_right),
                                        )
                                    }
                                    Expr::Call(target, args) => {
                                        let new_target = extract_api_calls_from_expr_helper(
                                            target,
                                            script,
                                            current_func,
                                            extracted,
                                            temp_var_types,
                                            needs_self,
                                            None,
                                            temp_counter,
                                        );
                                        let new_args: Vec<Expr> = args
                                            .iter()
                                            .map(|arg| {
                                                extract_api_calls_from_expr_helper(
                                                    arg,
                                                    script,
                                                    current_func,
                                                    extracted,
                                                    temp_var_types,
                                                    needs_self,
                                                    None,
                                                    temp_counter,
                                                )
                                            })
                                            .collect();
                                        Expr::Call(Box::new(new_target), new_args)
                                    }
                                    Expr::Cast(inner, target_type) => {
                                        let new_inner = extract_api_calls_from_expr_helper(
                                            inner,
                                            script,
                                            current_func,
                                            extracted,
                                            temp_var_types,
                                            needs_self,
                                            None,
                                            temp_counter,
                                        );
                                        Expr::Cast(Box::new(new_inner), target_type.clone())
                                    }
                                    Expr::Index(array, index) => {
                                        let new_array = extract_api_calls_from_expr_helper(
                                            array,
                                            script,
                                            current_func,
                                            extracted,
                                            temp_var_types,
                                            needs_self,
                                            None,
                                            temp_counter,
                                        );
                                        let new_index = extract_api_calls_from_expr_helper(
                                            index,
                                            script,
                                            current_func,
                                            extracted,
                                            temp_var_types,
                                            needs_self,
                                            None,
                                            temp_counter,
                                        );
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
                                &mut temp_counter,
                            );

                            // Combine all temp declarations
                            let combined_temp_decl = if !extracted_api_calls.is_empty() {
                                Some(
                                    extracted_api_calls
                                        .iter()
                                        .map(|(decl, _): &(String, String)| decl.clone())
                                        .collect::<Vec<_>>()
                                        .join(" "),
                                )
                            } else {
                                None
                            };

                            // Generate code for the (possibly modified) RHS expression
                            let rhs_code = modified_rhs_expr.to_rust(
                                needs_self,
                                script,
                                lhs_type.as_ref(),
                                current_func,
                                rhs_expr.span.as_ref(),
                            );

                            let is_literal = matches!(rhs_expr.expr, Expr::Literal(_));

                            // Apply implicit conversion if needed (especially important for temp variables)
                            let final_rhs = if let Some(lhs_ty) = &lhs_type {
                                if let Some(rhs_ty) = &rhs_type {
                                    if !is_literal
                                        && rhs_ty.can_implicitly_convert_to(lhs_ty)
                                        && rhs_ty != lhs_ty
                                    {
                                        script.generate_implicit_cast_for_expr(
                                            &rhs_code, rhs_ty, lhs_ty,
                                        )
                                    } else {
                                        rhs_code
                                    }
                                } else {
                                    rhs_code
                                }
                            } else {
                                rhs_code
                            };

                            // Build field_path_vec from field_path
                            let field_path_vec: Vec<&str> = field_path.split('.').collect();
                            // Check if the field is on the base Node type - if so, use mutate_scene_node
                            let first_field = field_path_vec.first().map(|s| *s).unwrap_or("");
                            let is_base_node_field = ENGINE_REGISTRY
                                .get_field_type_node(&NodeType::Node, first_field)
                                .is_some();

                            // If it's a single field on the base Node type, use mutate_scene_node
                            // This works for any node type, even if we don't know the specific type
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
                                    // For set_name, convert string to String if needed
                                    let rhs_for_setter = if setter == "set_name" {
                                        // If final_rhs is a string literal, wrap it in String::from()
                                        // If it's already String::from() or a variable, use as-is
                                        if final_rhs.starts_with('"') && final_rhs.ends_with('"') {
                                            format!("String::from({})", final_rhs)
                                        } else {
                                            final_rhs.clone()
                                        }
                                    } else {
                                        final_rhs.clone()
                                    };

                                    let temp_decl = combined_temp_decl
                                        .as_ref()
                                        .map(|d| format!("        {}\n", d))
                                        .unwrap_or_default();
                                    format!(
                                        "{}        api.mutate_scene_node({}, |n| {{ n.{}({}); }});\n",
                                        temp_decl, node_id, setter, rhs_for_setter
                                    )
                                } else {
                                    // Field doesn't have a setter, fall back to match statement approach
                                    // If only one compatible node type, skip match and do direct mutation
                                    if compatible_node_types.len() == 1 {
                                        let node_type_name =
                                            format!("{:?}", compatible_node_types[0]);
                                        // Resolve field names in path (e.g., "texture" -> "texture_id")
                                        let resolved_path: Vec<String> = field_path_vec
                                            .iter()
                                            .map(|f| {
                                                ENGINE_REGISTRY.resolve_field_name(
                                                    &compatible_node_types[0],
                                                    f,
                                                )
                                            })
                                            .collect();
                                        let resolved_field_path = resolved_path.join(".");
                                        let temp_decl = combined_temp_decl
                                            .as_ref()
                                            .map(|d| format!("        {}\n", d))
                                            .unwrap_or_default();
                                        format!(
                                            "{}        api.mutate_node({}, |{}: &mut {}| {{ {}.{} = {}; }});\n",
                                            temp_decl,
                                            node_id_with_self,
                                            clean_closure_var,
                                            node_type_name,
                                            clean_closure_var,
                                            resolved_field_path,
                                            final_rhs
                                        )
                                    } else {
                                        let mut match_arms = Vec::new();
                                        for node_type_enum in &compatible_node_types {
                                            let node_type_name = format!("{:?}", node_type_enum);
                                            // Resolve field names in path for this node type
                                            let resolved_path: Vec<String> = field_path_vec
                                                .iter()
                                                .map(|f| {
                                                    ENGINE_REGISTRY
                                                        .resolve_field_name(node_type_enum, f)
                                                })
                                                .collect();
                                            let resolved_field_path = resolved_path.join(".");
                                            match_arms.push(format!(
                                                "            NodeType::{} => api.mutate_node({}, |{}: &mut {}| {{ {}.{} = {}; }}),",
                                                node_type_name, node_id_with_self, clean_closure_var, node_type_name, clean_closure_var, resolved_field_path, final_rhs
                                            ));
                                        }

                                        let temp_decl = combined_temp_decl
                                            .as_ref()
                                            .map(|d| format!("        {}\n", d))
                                            .unwrap_or_default();
                                        format!(
                                            "{}        match api.get_node_type({}) {{\n{}\n            _ => {{\n                let node_name = api.read_scene_node({}, |n| n.get_name().to_string());\n                let node_type = format!(\"{{:?}}\", api.get_node_type({}));\n                panic!(\"{{}} of type {{}} doesn't have field {{}}\", node_name, node_type, \"{}\");\n            }}\n        }}\n",
                                            temp_decl,
                                            node_id_with_self,
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
                                    let resolved_path: Vec<String> = field_path_vec
                                        .iter()
                                        .map(|f| {
                                            ENGINE_REGISTRY
                                                .resolve_field_name(&compatible_node_types[0], f)
                                        })
                                        .collect();
                                    let resolved_field_path = resolved_path.join(".");
                                    let temp_decl = combined_temp_decl
                                        .as_ref()
                                        .map(|d| format!("        {}\n", d))
                                        .unwrap_or_default();
                                    format!(
                                        "{}        api.mutate_node({}, |{}: &mut {}| {{ {}.{} = {}; }});\n",
                                        temp_decl,
                                        node_id_with_self,
                                        clean_closure_var,
                                        node_type_name,
                                        clean_closure_var,
                                        resolved_field_path,
                                        final_rhs
                                    )
                                } else {
                                    let mut match_arms = Vec::new();
                                    for node_type_enum in &compatible_node_types {
                                        let node_type_name = format!("{:?}", node_type_enum);
                                        // Resolve field names in path for this node type
                                        let resolved_path: Vec<String> = field_path_vec
                                            .iter()
                                            .map(|f| {
                                                ENGINE_REGISTRY
                                                    .resolve_field_name(node_type_enum, f)
                                            })
                                            .collect();
                                        let resolved_field_path = resolved_path.join(".");
                                        match_arms.push(format!(
                                            "            NodeType::{} => api.mutate_node({}, |{}: &mut {}| {{ {}.{} = {}; }}),",
                                            node_type_name, node_id_with_self, clean_closure_var, node_type_name, clean_closure_var, resolved_field_path, final_rhs
                                        ));
                                    }

                                    let temp_decl = combined_temp_decl
                                        .as_ref()
                                        .map(|d| format!("        {}\n", d))
                                        .unwrap_or_default();
                                    format!(
                                        "{}        match api.get_type({}) {{\n{}\n            _ => {{\n                let node_name = api.read_scene_node({}, |n| n.get_name().to_string());\n                let node_type = format!(\"{{:?}}\", api.get_type({}));\n                panic!(\"{{}} of type {{}} doesn't have field {{}}\", node_name, node_type, \"{}\");\n            }}\n        }}\n",
                                        temp_decl,
                                        node_id,
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
                        let mut temp_var_types: std::collections::HashMap<String, Type> =
                            std::collections::HashMap::new();

                        // Use a deterministic counter for temp variable names to enable incremental compilation
                        let mut temp_counter = 0usize;

                        fn extract_api_calls_from_expr(
                            expr: &Expr,
                            script: &Script,
                            current_func: Option<&Function>,
                            extracted: &mut Vec<(String, String)>,
                            temp_var_types: &mut std::collections::HashMap<String, Type>,
                            needs_self: bool,
                            expected_type: Option<&Type>,
                            temp_counter: &mut usize,
                        ) -> Expr {
                            match expr {
                                // Extract API calls (like Math.random_range, Texture.load, etc.)
                                Expr::ApiCall(api_module, api_args) => {
                                    // First, recursively extract nested API calls from arguments
                                    let new_args: Vec<Expr> = api_args
                                        .iter()
                                        .map(|arg| {
                                            extract_api_calls_from_expr(
                                                arg,
                                                script,
                                                current_func,
                                                extracted,
                                                temp_var_types,
                                                needs_self,
                                                None,
                                                temp_counter,
                                            )
                                        })
                                        .collect();

                                    // Generate deterministic temp variable name using occurrence counter
                                    let current_index = *temp_counter;
                                    *temp_counter += 1;

                                    // Extract ALL API calls, not just ones returning Uuid
                                    // This prevents borrow checker issues when API calls are inside closures
                                    let temp_var = format!("__temp_api_{}", current_index);

                                    // Generate the API call code with extracted arguments
                                    let mut api_call_str = api_module.to_rust(
                                        &new_args,
                                        script,
                                        needs_self,
                                        current_func,
                                    );

                                    // Fix any incorrect renaming of "api" to "__t_api" or "t_id_api"
                                    api_call_str = api_call_str
                                        .replace("__t_api.", "api.")
                                        .replace("t_id_api.", "api.");

                                    // Infer the return type for the temp variable
                                    let inferred_type = api_module.return_type();
                                    let type_annotation = inferred_type
                                        .as_ref()
                                        .map(|t| {
                                            // Special case: Texture (EngineStruct) returns Option<TextureID>
                                            let rust_type = match t {
                                                Type::EngineStruct(EngineStructKind::Texture) => {
                                                    "Option<TextureID>".to_string()
                                                }
                                                _ => t.to_rust_type(),
                                            };
                                            format!(": {}", rust_type)
                                        })
                                        .unwrap_or_default();

                                    // Store the type for this temp variable
                                    if let Some(ty) = inferred_type {
                                        temp_var_types.insert(temp_var.clone(), ty);
                                    }

                                    extracted.push((
                                        format!(
                                            "let {}{} = {};",
                                            temp_var, type_annotation, api_call_str
                                        ),
                                        temp_var.clone(),
                                    ));

                                    // Return an identifier expression for the temp variable
                                    Expr::Ident(temp_var)
                                }
                                Expr::MemberAccess(base, field) => {
                                    // First, recursively extract API calls from base
                                    let new_base = extract_api_calls_from_expr(
                                        base,
                                        script,
                                        current_func,
                                        extracted,
                                        temp_var_types,
                                        needs_self,
                                        None,
                                        temp_counter,
                                    );

                                    // Check if this member access would generate a read_node call
                                    let test_expr = Expr::MemberAccess(
                                        Box::new(new_base.clone()),
                                        field.clone(),
                                    );
                                    if let Some((_node_id, _, _, _)) =
                                        extract_node_member_info(&test_expr, script, current_func)
                                    {
                                        // This is a node member access - extract it to a temp variable
                                        // Generate deterministic temp variable name using occurrence counter
                                        let current_index = *temp_counter;
                                        *temp_counter += 1;
                                        let temp_var = format!("__temp_read_{}", current_index);

                                        // Generate the read_node call
                                        let read_code = test_expr.to_rust(
                                            needs_self,
                                            script,
                                            expected_type,
                                            current_func,
                                            None,
                                        );

                                        // Infer the type for the temp variable
                                        let inferred_type =
                                            script.infer_expr_type(&test_expr, current_func);
                                        let type_annotation = inferred_type
                                            .as_ref()
                                            .map(|t| {
                                                // Special case: Texture (EngineStruct) returns Option<TextureID>
                                                let rust_type = match t {
                                                    Type::EngineStruct(
                                                        EngineStructKind::Texture,
                                                    ) => "Option<TextureID>".to_string(),
                                                    _ => t.to_rust_type(),
                                                };
                                                format!(": {}", rust_type)
                                            })
                                            .unwrap_or_default();

                                        // Store the type for this temp variable so we can check if it needs cloning
                                        if let Some(ty) = inferred_type {
                                            temp_var_types.insert(temp_var.clone(), ty);
                                        }

                                        extracted.push((
                                            format!(
                                                "let {}{} = {};",
                                                temp_var, type_annotation, read_code
                                            ),
                                            temp_var.clone(),
                                        ));

                                        // Return an identifier expression for the temp variable
                                        Expr::Ident(temp_var)
                                    } else {
                                        // Not a node member access, return the member access with processed base
                                        Expr::MemberAccess(Box::new(new_base), field.clone())
                                    }
                                }
                                Expr::BinaryOp(left, op, right) => {
                                    let new_left = extract_api_calls_from_expr(
                                        left,
                                        script,
                                        current_func,
                                        extracted,
                                        temp_var_types,
                                        needs_self,
                                        None,
                                        temp_counter,
                                    );
                                    let new_right = extract_api_calls_from_expr(
                                        right,
                                        script,
                                        current_func,
                                        extracted,
                                        temp_var_types,
                                        needs_self,
                                        None,
                                        temp_counter,
                                    );
                                    Expr::BinaryOp(
                                        Box::new(new_left),
                                        op.clone(),
                                        Box::new(new_right),
                                    )
                                }
                                Expr::Call(target, args) => {
                                    let new_target = extract_api_calls_from_expr(
                                        target,
                                        script,
                                        current_func,
                                        extracted,
                                        temp_var_types,
                                        needs_self,
                                        None,
                                        temp_counter,
                                    );
                                    let new_args: Vec<Expr> = args
                                        .iter()
                                        .map(|arg| {
                                            extract_api_calls_from_expr(
                                                arg,
                                                script,
                                                current_func,
                                                extracted,
                                                temp_var_types,
                                                needs_self,
                                                None,
                                                temp_counter,
                                            )
                                        })
                                        .collect();
                                    Expr::Call(Box::new(new_target), new_args)
                                }
                                Expr::Cast(inner, target_type) => {
                                    let new_inner = extract_api_calls_from_expr(
                                        inner,
                                        script,
                                        current_func,
                                        extracted,
                                        temp_var_types,
                                        needs_self,
                                        None,
                                        temp_counter,
                                    );
                                    Expr::Cast(Box::new(new_inner), target_type.clone())
                                }
                                Expr::Index(array, index) => {
                                    let new_array = extract_api_calls_from_expr(
                                        array,
                                        script,
                                        current_func,
                                        extracted,
                                        temp_var_types,
                                        needs_self,
                                        None,
                                        temp_counter,
                                    );
                                    let new_index = extract_api_calls_from_expr(
                                        index,
                                        script,
                                        current_func,
                                        extracted,
                                        temp_var_types,
                                        needs_self,
                                        None,
                                        temp_counter,
                                    );
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
                            &mut temp_counter,
                        );

                        // Combine all temp declarations from extracted API calls
                        let combined_temp_decl = if !extracted_api_calls.is_empty() {
                            Some(
                                extracted_api_calls
                                    .iter()
                                    .map(|(decl, _): &(String, String)| decl.clone())
                                    .collect::<Vec<_>>()
                                    .join(" "),
                            )
                        } else {
                            None
                        };

                        // Generate code for the (possibly modified) RHS expression
                        // If API calls were extracted, the modified expression uses temp variables
                        let rhs_code = modified_rhs_expr.to_rust(
                            needs_self,
                            script,
                            lhs_type.as_ref(),
                            current_func,
                            rhs_expr.span.as_ref(),
                        );

                        // For literals, we already generated the code with the expected type,
                        // so skip implicit cast to avoid double conversion
                        let is_literal = matches!(rhs_expr.expr, Expr::Literal(_));

                        // Apply implicit conversion if needed (especially important for temp variables)
                        let final_rhs = if let Some(lhs_ty) = &lhs_type {
                            if let Some(rhs_ty) = &rhs_type {
                                // For literals, if they were generated with the correct expected type,
                                // they should already be correct. Only apply cast if types don't match
                                // and it's not a literal (literals handle their own type conversion)
                                if !is_literal
                                    && rhs_ty.can_implicitly_convert_to(lhs_ty)
                                    && rhs_ty != lhs_ty
                                {
                                    script
                                        .generate_implicit_cast_for_expr(&rhs_code, rhs_ty, lhs_ty)
                                } else if is_literal {
                                    // For literals, check if the generated code needs conversion
                                    // If lhs is Option<CowStr> but we got String::from, convert it
                                    if matches!(lhs_ty, Type::Option(inner) if matches!(inner.as_ref(), Type::CowStr))
                                        && rhs_code.contains("String::from(")
                                    {
                                        // Extract the literal from String::from("...") and convert to Some(Cow::Borrowed(...))
                                        let trimmed = rhs_code.trim();
                                        if trimmed.starts_with("String::from(")
                                            && trimmed.ends_with(')')
                                        {
                                            let inner_section = &trimmed
                                                ["String::from(".len()..trimmed.len() - 1]
                                                .trim();
                                            if inner_section.starts_with('"')
                                                && inner_section.ends_with('"')
                                            {
                                                format!("Some(Cow::Borrowed({}))", inner_section)
                                            } else {
                                                script.generate_implicit_cast_for_expr(
                                                    &rhs_code, rhs_ty, lhs_ty,
                                                )
                                            }
                                        } else {
                                            script.generate_implicit_cast_for_expr(
                                                &rhs_code, rhs_ty, lhs_ty,
                                            )
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

                        // Check if this is a base Node field - if so, use mutate_scene_node
                        // This handles cases where node_type is "Node" (from get_parent returning Type::Node(NodeType::Node))
                        let field_path_vec: Vec<&str> = field_path.split('.').collect();
                        let first_field = field_path_vec.first().map(|s| *s).unwrap_or("");
                        let is_base_node_field = ENGINE_REGISTRY
                            .get_field_type_node(&NodeType::Node, first_field)
                            .is_some();

                        // If it's a single field on the base Node type, use mutate_scene_node
                        if is_base_node_field && field_path_vec.len() == 1 {
                            // Map field names to their setter methods from BaseNode trait
                            let setter_method = match first_field {
                                "name" => Some("set_name"),
                                "id" => Some("set_id"),
                                "local_id" => Some("set_local_id"),
                                "parent" => Some("set_parent"),
                                "script_path" => Some("set_script_path"),
                                _ => None,
                            };

                            if let Some(setter) = setter_method {
                                // Get the expected type for this setter by looking up the field type
                                // The setter parameter type should match the field type
                                let expected_setter_type = ENGINE_REGISTRY
                                    .get_field_type_node(&NodeType::Node, first_field);

                                // Use type conversion to convert RHS to the expected type
                                let rhs_for_setter =
                                    if let Some(expected_type) = expected_setter_type {
                                        if let Some(rhs_ty) = &rhs_type {
                                            if rhs_ty.can_implicitly_convert_to(&expected_type)
                                                && rhs_ty != &expected_type
                                            {
                                                script.generate_implicit_cast_for_expr(
                                                    &final_rhs,
                                                    rhs_ty,
                                                    &expected_type,
                                                )
                                            } else {
                                                final_rhs.clone()
                                            }
                                        } else {
                                            final_rhs.clone()
                                        }
                                    } else {
                                        final_rhs.clone()
                                    };

                                let temp_decl = combined_temp_decl
                                    .as_ref()
                                    .map(|d| format!("        {}\n", d))
                                    .unwrap_or_default();
                                return format!(
                                    "{}        api.mutate_scene_node({}, |n| {{ n.{}({}); }});\n",
                                    temp_decl, node_id_with_self, setter, rhs_for_setter
                                );
                            }
                        }

                        // If node_type is "Node", we should use mutate_scene_node instead of mutate_node
                        // because we don't know the actual node type - it could be any node type
                        if node_type == "Node" {
                            // Check if this is a base Node field - if so, use mutate_scene_node with setter
                            let first_field = field_path_vec.first().map(|s| *s).unwrap_or("");
                            let is_base_node_field = ENGINE_REGISTRY
                                .get_field_type_node(&NodeType::Node, first_field)
                                .is_some();

                            if is_base_node_field && field_path_vec.len() == 1 {
                                // Map field names to their setter methods from BaseNode trait
                                let setter_method = match first_field {
                                    "name" => Some("set_name"),
                                    "id" => Some("set_id"),
                                    "local_id" => Some("set_local_id"),
                                    "parent" => Some("set_parent"),
                                    "script_path" => Some("set_script_path"),
                                    _ => None,
                                };

                                if let Some(setter) = setter_method {
                                    // Get the expected type for this setter by looking up the field type
                                    // The setter parameter type should match the field type
                                    let expected_setter_type = ENGINE_REGISTRY
                                        .get_field_type_node(&NodeType::Node, first_field);

                                    // Use type conversion to convert RHS to the expected type
                                    let rhs_for_setter =
                                        if let Some(expected_type) = expected_setter_type {
                                            if let Some(rhs_ty) = &rhs_type {
                                                if rhs_ty.can_implicitly_convert_to(&expected_type)
                                                    && rhs_ty != &expected_type
                                                {
                                                    script.generate_implicit_cast_for_expr(
                                                        &final_rhs,
                                                        rhs_ty,
                                                        &expected_type,
                                                    )
                                                } else {
                                                    final_rhs.clone()
                                                }
                                            } else {
                                                final_rhs.clone()
                                            }
                                        } else {
                                            final_rhs.clone()
                                        };

                                    let temp_decl = combined_temp_decl
                                        .as_ref()
                                        .map(|d| format!("        {}\n", d))
                                        .unwrap_or_default();
                                    return format!(
                                        "{}        api.mutate_scene_node({}, |n| {{ n.{}({}); }});\n",
                                        temp_decl, node_id_with_self, setter, rhs_for_setter
                                    );
                                }
                            }

                            // For non-base Node fields or when setter is not available,
                            // we still need to use mutate_scene_node but access the field directly
                            // However, this case should be rare since Node only has base fields
                            // For now, fall through to error or use mutate_scene_node with direct field access
                            let resolved_field_path = field_path.clone();
                            let temp_decl = combined_temp_decl
                                .as_ref()
                                .map(|d| format!("        {}\n", d))
                                .unwrap_or_default();
                            // Note: This path should rarely be hit since Node only has base fields
                            // If we need to support non-base fields on Node, we'd need to use a different approach
                            return format!(
                                "{}        // ERROR: Cannot mutate non-base field '{}' on Node type - use specific node type instead\n",
                                temp_decl, resolved_field_path
                            );
                        }

                        // Build field_path_vec from field_path for field resolution
                        let field_path_vec: Vec<&str> = field_path.split('.').collect();

                        // Resolve field names in path (e.g., "texture" -> "texture_id")
                        let (resolved_field_path, node_type_enum_opt) =
                            if let Some(node_type_enum) = string_to_node_type(&node_type) {
                                let resolved_path: Vec<String> = field_path_vec
                                    .iter()
                                    .map(|f| ENGINE_REGISTRY.resolve_field_name(&node_type_enum, f))
                                    .collect();
                                (resolved_path.join("."), Some(node_type_enum))
                            } else {
                                (field_path.clone(), None)
                            };

                        let temp_decl = combined_temp_decl
                            .as_ref()
                            .map(|d| format!("        {}\n", d))
                            .unwrap_or_default();

                        let output = if let Some(node_type_enum) = node_type_enum_opt {
                            let resolved_path: Vec<String> = field_path_vec
                                .iter()
                                .map(|f| ENGINE_REGISTRY.resolve_field_name(&node_type_enum, f))
                                .collect();
                            let resolved_field_path_full = resolved_path.join(".");
                            // Path-based behaviors for plain assignment (e.g. transform.rotation.z = rhs)
                            if let Some(behavior) = ENGINE_REGISTRY
                                .get_field_assign_behavior_path(&node_type_enum, &resolved_field_path_full)
                            {
                                if let Some(code) = behavior.emit_euler_axis_3d_block_expr(
                                    &node_id_with_self,
                                    &node_type,
                                    "set",
                                    &final_rhs,
                                ) {
                                    return format!("{}        {}\n", temp_decl, code);
                                }
                            }
                            let first_segment = resolved_path.first().map(String::as_str).unwrap_or("");
                            if let Some(behavior) = ENGINE_REGISTRY.get_field_assign_behavior(&node_type_enum, first_segment) {
                                let rest_path = resolved_path.get(1..).map(|v| v.join(".")).unwrap_or_default();
                                let mutate_expr = if rest_path.is_empty() {
                                    format!("__g = {}", final_rhs)
                                } else {
                                    format!("__g.{} = {}", rest_path, final_rhs)
                                };
                                format!("{}        {}\n", temp_decl, behavior.emit_get_set_block(&node_id_with_self, &mutate_expr, &node_type))
                        } else {
                            format!(
                                "{}        api.mutate_node({}, |{}: &mut {}| {{ {}.{} = {}; }});\n",
                                temp_decl,
                                node_id_with_self,
                                clean_closure_var,
                                node_type,
                                clean_closure_var,
                                resolved_field_path,
                                final_rhs
                            )
                        }
                        } else {
                            // When node_type is empty (e.g. TypeScript class without extends) or not a known engine type, use script's Rust struct name
                            let closure_type = if node_type.is_empty()
                                || string_to_node_type(&node_type).is_none()
                            {
                                script.rust_struct_name.as_deref().unwrap_or("Node")
                            } else {
                                node_type.as_str()
                            };
                            format!(
                                "{}        api.mutate_node({}, |{}: &mut {}| {{ {}.{} = {}; }});\n",
                                temp_decl,
                                node_id_with_self,
                                clean_closure_var,
                                closure_type,
                                clean_closure_var,
                                resolved_field_path,
                                final_rhs
                            )
                        };
                        output
                    }
                } else {
                    // Regular member assignment (not a node)
                    let lhs_code = lhs_expr.to_rust(needs_self, script, current_func);
                    // lhs_expr is TypedExpr, which already passes span through
                    let lhs_type = script.infer_expr_type(&lhs_expr.expr, current_func);
                    let rhs_type = script.infer_expr_type(&rhs_expr.expr, current_func);

                    let rhs_code = rhs_expr.expr.to_rust(
                        needs_self,
                        script,
                        lhs_type.as_ref(),
                        current_func,
                        rhs_expr.span.as_ref(),
                    );

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

                    let should_clone =
                        matches!(rhs_expr.expr, Expr::Ident(_) | Expr::MemberAccess(..))
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
                    // Clean closure_var (remove self. prefix) and ensure node_id has self. prefix only if it's a struct field
                    let clean_closure_var =
                        closure_var.strip_prefix("self.").unwrap_or(&closure_var);
                    // Only add self. prefix if node_id is actually a struct field, not a local variable
                    let node_id_with_self = if !node_id.starts_with("self.")
                        && !node_id.starts_with("api.")
                        && script.is_struct_field(&node_id)
                    {
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
                        let compatible_node_types =
                            ENGINE_REGISTRY.narrow_nodes_by_fields(&field_path_vec);

                        if compatible_node_types.is_empty() {
                            // No compatible node types found, fallback to error
                            format!(
                                "        // ERROR: No compatible node types found for field path: {}\n",
                                field_path
                            )
                        } else {
                            // Generate match arms for all compatible node types
                            let lhs_type = script.infer_expr_type(&lhs_expr.expr, current_func);

                            let rhs_code = rhs_expr.expr.to_rust(
                                needs_self,
                                script,
                                lhs_type.as_ref(),
                                current_func,
                                rhs_expr.span.as_ref(),
                            );

                            if matches!(op, Op::Add) && lhs_type == Some(Type::String) {
                                // If only one compatible node type, skip match and do direct mutation
                                if compatible_node_types.len() == 1 {
                                    let node_type_name = format!("{:?}", compatible_node_types[0]);
                                    // Resolve field names in path
                                    let resolved_path: Vec<String> = field_path_vec
                                        .iter()
                                        .map(|f| {
                                            ENGINE_REGISTRY
                                                .resolve_field_name(&compatible_node_types[0], f)
                                        })
                                        .collect();
                                    let resolved_field_path = resolved_path.join(".");
                                    return format!(
                                        "        api.mutate_node({}, |{}: &mut {}| {{ {}.{}.push_str({}.as_str()); }});\n",
                                        node_id_with_self,
                                        clean_closure_var,
                                        node_type_name,
                                        clean_closure_var,
                                        resolved_field_path,
                                        rhs_code
                                    );
                                } else {
                                    let mut match_arms = Vec::new();
                                    for node_type_enum in &compatible_node_types {
                                        let node_type_name = format!("{:?}", node_type_enum);
                                        // Resolve field names in path for this node type
                                        let resolved_path: Vec<String> = field_path_vec
                                            .iter()
                                            .map(|f| {
                                                ENGINE_REGISTRY
                                                    .resolve_field_name(node_type_enum, f)
                                            })
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
                                    if rhs_ty.can_implicitly_convert_to(lhs_ty) && rhs_ty != lhs_ty
                                    {
                                        script.generate_implicit_cast_for_expr(
                                            &rhs_code, rhs_ty, lhs_ty,
                                        )
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
                                let resolved_path: Vec<String> = field_path_vec
                                    .iter()
                                    .map(|f| {
                                        ENGINE_REGISTRY
                                            .resolve_field_name(&compatible_node_types[0], f)
                                    })
                                    .collect();
                                let resolved_field_path = resolved_path.join(".");
                                // Path-based behaviors (e.g. transform.rotation.z op= rhs)
                                if let Some(behavior) = ENGINE_REGISTRY
                                    .get_field_assign_behavior_path(&compatible_node_types[0], &resolved_field_path)
                                {
                                    let op_kind = match op {
                                        Op::Add => Some("add"),
                                        Op::Sub => Some("sub"),
                                        Op::Mul => Some("mul"),
                                        Op::Div => Some("div"),
                                        _ => None,
                                    };
                                    if let Some(op_kind) = op_kind {
                                        if let Some(code) = behavior.emit_euler_axis_3d_block_expr(
                                            &node_id_with_self,
                                            &node_type_name,
                                            op_kind,
                                            &final_rhs,
                                        ) {
                                            return format!("        {}\n", code);
                                        }
                                    }
                                }
                                let first_segment = resolved_path.first().map(String::as_str).unwrap_or("");
                                if let Some(behavior) = ENGINE_REGISTRY.get_field_assign_behavior(&compatible_node_types[0], first_segment) {
                                    let rest_path = resolved_path.get(1..).map(|v| v.join(".")).unwrap_or_default();
                                    let mutate_expr = if rest_path.is_empty() {
                                        format!("__g {}= {}", op.to_rust_assign(), final_rhs)
                                    } else {
                                        format!("__g.{} {}= {}", rest_path, op.to_rust_assign(), final_rhs)
                                    };
                                    format!("        {}\n", behavior.emit_get_set_block(&node_id_with_self, &mutate_expr, &node_type_name))
                                } else {
                                    format!(
                                        "        api.mutate_node({}, |{}: &mut {}| {{ {}.{} {}= {}; }});\n",
                                        node_id_with_self,
                                        clean_closure_var,
                                        node_type_name,
                                        clean_closure_var,
                                        resolved_field_path,
                                        op.to_rust_assign(),
                                        final_rhs
                                    )
                                }
                            } else {
                                let mut match_arms = Vec::new();
                                for node_type_enum in &compatible_node_types {
                                    let node_type_name = format!("{:?}", node_type_enum);
                                    let resolved_path: Vec<String> = field_path_vec
                                        .iter()
                                        .map(|f| ENGINE_REGISTRY.resolve_field_name(node_type_enum, f))
                                        .collect();
                                    let resolved_field_path = resolved_path.join(".");
                                    // Path-based behaviors first (rotation/euler lowering)
                                    let path_behavior = ENGINE_REGISTRY
                                        .get_field_assign_behavior_path(node_type_enum, &resolved_field_path);
                                    let first_segment = resolved_path.first().map(String::as_str).unwrap_or("");
                                    let arm_code = if let Some(behavior) = path_behavior {
                                        let op_kind = match op {
                                            Op::Add => Some("add"),
                                            Op::Sub => Some("sub"),
                                            Op::Mul => Some("mul"),
                                            Op::Div => Some("div"),
                                            _ => None,
                                        };
                                        if let Some(op_kind) = op_kind {
                                            if let Some(code) = behavior.emit_euler_axis_3d_block_expr(
                                                &node_id_with_self,
                                                &node_type_name,
                                                op_kind,
                                                &final_rhs,
                                            ) {
                                                code
                                            } else {
                                                format!(
                                                    "api.mutate_node({}, |{}: &mut {}| {{ {}.{} {}= {}; }})",
                                                    node_id_with_self,
                                                    clean_closure_var,
                                                    node_type_name,
                                                    clean_closure_var,
                                                    resolved_field_path,
                                                    op.to_rust_assign(),
                                                    final_rhs
                                                )
                                            }
                                        } else {
                                            format!(
                                                "api.mutate_node({}, |{}: &mut {}| {{ {}.{} {}= {}; }})",
                                                node_id_with_self,
                                                clean_closure_var,
                                                node_type_name,
                                                clean_closure_var,
                                                resolved_field_path,
                                                op.to_rust_assign(),
                                                final_rhs
                                            )
                                        }
                                    } else if let Some(behavior) = ENGINE_REGISTRY.get_field_assign_behavior(node_type_enum, first_segment) {
                                        let rest_path = resolved_path.get(1..).map(|v| v.join(".")).unwrap_or_default();
                                        let mutate_expr = if rest_path.is_empty() {
                                            format!("__g {}= {}", op.to_rust_assign(), final_rhs)
                                        } else {
                                            format!("__g.{} {}= {}", rest_path, op.to_rust_assign(), final_rhs)
                                        };
                                        behavior.emit_get_set_block(&node_id_with_self, &mutate_expr, &node_type_name)
                                    } else {
                                        format!(
                                            "api.mutate_node({}, |{}: &mut {}| {{ {}.{} {}= {}; }})",
                                            node_id_with_self, clean_closure_var, node_type_name, clean_closure_var, resolved_field_path, op.to_rust_assign(), final_rhs
                                        )
                                    };
                                    match_arms.push(format!(
                                        "            NodeType::{} => {},",
                                        node_type_name, arm_code
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
                        let mut temp_var_types: std::collections::HashMap<String, Type> =
                            std::collections::HashMap::new();

                        // Use a deterministic counter for temp variable names to enable incremental compilation
                        let mut temp_counter = 0usize;

                        fn extract_api_calls_from_expr(
                            expr: &Expr,
                            script: &Script,
                            current_func: Option<&Function>,
                            extracted: &mut Vec<(String, String)>,
                            temp_var_types: &mut std::collections::HashMap<String, Type>,
                            needs_self: bool,
                            expected_type: Option<&Type>,
                            temp_counter: &mut usize,
                        ) -> Expr {
                            match expr {
                                // Extract API calls (like Math.random_range, Texture.load, etc.)
                                Expr::ApiCall(api_module, api_args) => {
                                    // First, recursively extract nested API calls from arguments
                                    let new_args: Vec<Expr> = api_args
                                        .iter()
                                        .map(|arg| {
                                            extract_api_calls_from_expr(
                                                arg,
                                                script,
                                                current_func,
                                                extracted,
                                                temp_var_types,
                                                needs_self,
                                                None,
                                                temp_counter,
                                            )
                                        })
                                        .collect();

                                    // Generate deterministic temp variable name using occurrence counter
                                    let current_index = *temp_counter;
                                    *temp_counter += 1;

                                    // Extract ALL API calls, not just ones returning Uuid
                                    // This prevents borrow checker issues when API calls are inside closures
                                    let temp_var = format!("__temp_api_{}", current_index);

                                    // Generate the API call code with extracted arguments
                                    let mut api_call_str = api_module.to_rust(
                                        &new_args,
                                        script,
                                        needs_self,
                                        current_func,
                                    );

                                    // Fix any incorrect renaming of "api" to "__t_api" or "t_id_api"
                                    api_call_str = api_call_str
                                        .replace("__t_api.", "api.")
                                        .replace("t_id_api.", "api.");

                                    // Infer the return type for the temp variable
                                    let inferred_type = api_module.return_type();
                                    let type_annotation = inferred_type
                                        .as_ref()
                                        .map(|t| {
                                            // Special case: Texture (EngineStruct) returns Option<TextureID>
                                            let rust_type = match t {
                                                Type::EngineStruct(EngineStructKind::Texture) => {
                                                    "Option<TextureID>".to_string()
                                                }
                                                _ => t.to_rust_type(),
                                            };
                                            format!(": {}", rust_type)
                                        })
                                        .unwrap_or_default();

                                    // Store the type for this temp variable
                                    if let Some(ty) = inferred_type {
                                        temp_var_types.insert(temp_var.clone(), ty);
                                    }

                                    extracted.push((
                                        format!(
                                            "let {}{} = {};",
                                            temp_var, type_annotation, api_call_str
                                        ),
                                        temp_var.clone(),
                                    ));

                                    // Return an identifier expression for the temp variable
                                    Expr::Ident(temp_var)
                                }
                                Expr::MemberAccess(base, field) => {
                                    // First, recursively extract API calls from base
                                    let new_base = extract_api_calls_from_expr(
                                        base,
                                        script,
                                        current_func,
                                        extracted,
                                        temp_var_types,
                                        needs_self,
                                        None,
                                        temp_counter,
                                    );

                                    // Check if this member access would generate a read_node call
                                    let test_expr = Expr::MemberAccess(
                                        Box::new(new_base.clone()),
                                        field.clone(),
                                    );
                                    if let Some((_node_id, _, _, _)) =
                                        extract_node_member_info(&test_expr, script, current_func)
                                    {
                                        // This is a node member access - extract it to a temp variable
                                        // Generate deterministic temp variable name using occurrence counter
                                        let current_index = *temp_counter;
                                        *temp_counter += 1;
                                        let temp_var = format!("__temp_read_{}", current_index);

                                        // Generate the read_node call
                                        let read_code = test_expr.to_rust(
                                            needs_self,
                                            script,
                                            expected_type,
                                            current_func,
                                            None,
                                        );

                                        // Infer the type for the temp variable
                                        let inferred_type =
                                            script.infer_expr_type(&test_expr, current_func);
                                        let type_annotation = inferred_type
                                            .as_ref()
                                            .map(|t| {
                                                // Special case: Texture (EngineStruct) returns Option<TextureID>
                                                let rust_type = match t {
                                                    Type::EngineStruct(
                                                        EngineStructKind::Texture,
                                                    ) => "Option<TextureID>".to_string(),
                                                    _ => t.to_rust_type(),
                                                };
                                                format!(": {}", rust_type)
                                            })
                                            .unwrap_or_default();

                                        // Store the type for this temp variable so we can check if it needs cloning
                                        if let Some(ty) = inferred_type {
                                            temp_var_types.insert(temp_var.clone(), ty);
                                        }

                                        extracted.push((
                                            format!(
                                                "let {}{} = {};",
                                                temp_var, type_annotation, read_code
                                            ),
                                            temp_var.clone(),
                                        ));

                                        // Return an identifier expression for the temp variable
                                        Expr::Ident(temp_var)
                                    } else {
                                        // Not a node member access, return the member access with processed base
                                        Expr::MemberAccess(Box::new(new_base), field.clone())
                                    }
                                }
                                Expr::BinaryOp(left, op, right) => {
                                    let new_left = extract_api_calls_from_expr(
                                        left,
                                        script,
                                        current_func,
                                        extracted,
                                        temp_var_types,
                                        needs_self,
                                        None,
                                        temp_counter,
                                    );
                                    let new_right = extract_api_calls_from_expr(
                                        right,
                                        script,
                                        current_func,
                                        extracted,
                                        temp_var_types,
                                        needs_self,
                                        None,
                                        temp_counter,
                                    );
                                    Expr::BinaryOp(
                                        Box::new(new_left),
                                        op.clone(),
                                        Box::new(new_right),
                                    )
                                }
                                Expr::Call(target, args) => {
                                    let new_target = extract_api_calls_from_expr(
                                        target,
                                        script,
                                        current_func,
                                        extracted,
                                        temp_var_types,
                                        needs_self,
                                        None,
                                        temp_counter,
                                    );
                                    let new_args: Vec<Expr> = args
                                        .iter()
                                        .map(|arg| {
                                            extract_api_calls_from_expr(
                                                arg,
                                                script,
                                                current_func,
                                                extracted,
                                                temp_var_types,
                                                needs_self,
                                                None,
                                                temp_counter,
                                            )
                                        })
                                        .collect();
                                    Expr::Call(Box::new(new_target), new_args)
                                }
                                Expr::Cast(inner, target_type) => {
                                    let new_inner = extract_api_calls_from_expr(
                                        inner,
                                        script,
                                        current_func,
                                        extracted,
                                        temp_var_types,
                                        needs_self,
                                        None,
                                        temp_counter,
                                    );
                                    Expr::Cast(Box::new(new_inner), target_type.clone())
                                }
                                Expr::Index(array, index) => {
                                    let new_array = extract_api_calls_from_expr(
                                        array,
                                        script,
                                        current_func,
                                        extracted,
                                        temp_var_types,
                                        needs_self,
                                        None,
                                        temp_counter,
                                    );
                                    let new_index = extract_api_calls_from_expr(
                                        index,
                                        script,
                                        current_func,
                                        extracted,
                                        temp_var_types,
                                        needs_self,
                                        None,
                                        temp_counter,
                                    );
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
                            &mut temp_counter,
                        );

                        // Combine all temp declarations from extracted API calls
                        let combined_temp_decl = if !extracted_api_calls.is_empty() {
                            Some(
                                extracted_api_calls
                                    .iter()
                                    .map(|(decl, _): &(String, String)| decl.clone())
                                    .collect::<Vec<_>>()
                                    .join(" "),
                            )
                        } else {
                            None
                        };

                        // Generate code for the (possibly modified) RHS expression
                        // If API calls were extracted, the modified expression uses temp variables
                        let rhs_code = modified_rhs_expr.to_rust(
                            needs_self,
                            script,
                            lhs_type.as_ref(),
                            current_func,
                            rhs_expr.span.as_ref(),
                        );

                        // Resolve field names in path (e.g., "texture" -> "texture_id")
                        let resolved_field_path =
                            if let Some(node_type_enum) = string_to_node_type(&node_type) {
                                let field_path_vec: Vec<&str> = field_path.split('.').collect();
                                let resolved_path: Vec<String> = field_path_vec
                                    .iter()
                                    .map(|f| ENGINE_REGISTRY.resolve_field_name(&node_type_enum, f))
                                    .collect();
                                resolved_path.join(".")
                            } else {
                                field_path.clone()
                            };

                        if matches!(op, Op::Add) && lhs_type == Some(Type::String) {
                            let temp_decl = combined_temp_decl
                                .as_ref()
                                .map(|d| format!("        {}\n", d))
                                .unwrap_or_default();
                            return format!(
                                "{}        api.mutate_node({}, |{}: &mut {}| {{ {}.{}.push_str({}.as_str()); }});\n",
                                temp_decl,
                                node_id_with_self,
                                clean_closure_var,
                                node_type,
                                clean_closure_var,
                                resolved_field_path,
                                rhs_code
                            );
                        }

                        let final_rhs = if let Some(lhs_ty) = &lhs_type {
                            if let Some(rhs_ty) = &rhs_type {
                                if rhs_ty.can_implicitly_convert_to(lhs_ty) && rhs_ty != lhs_ty {
                                    script
                                        .generate_implicit_cast_for_expr(&rhs_code, rhs_ty, lhs_ty)
                                } else {
                                    rhs_code
                                }
                            } else {
                                rhs_code
                            }
                        } else {
                            rhs_code
                        };

                        let temp_decl = combined_temp_decl
                            .as_ref()
                            .map(|d| format!("        {}\n", d))
                            .unwrap_or_default();

                        // Known node type: use get/set API for global_transform etc. if registered
                        let field_path_vec: Vec<&str> = field_path.split('.').collect();
                        if let Some(node_type_enum) = string_to_node_type(&node_type) {
                            let resolved_path: Vec<String> = field_path_vec
                                .iter()
                                .map(|f| ENGINE_REGISTRY.resolve_field_name(&node_type_enum, f))
                                .collect();
                            let resolved_field_path = resolved_path.join(".");
                            // Path-based behaviors first (rotation/euler lowering)
                            if let Some(behavior) = ENGINE_REGISTRY
                                .get_field_assign_behavior_path(&node_type_enum, &resolved_field_path)
                            {
                                let op_kind = match op {
                                    Op::Add => Some("add"),
                                    Op::Sub => Some("sub"),
                                    Op::Mul => Some("mul"),
                                    Op::Div => Some("div"),
                                    _ => None,
                                };
                                if let Some(op_kind) = op_kind {
                                    if let Some(code) = behavior.emit_euler_axis_3d_block_expr(
                                        &node_id_with_self,
                                        &node_type,
                                        op_kind,
                                        &final_rhs,
                                    ) {
                                        return format!("{}        {}\n", temp_decl, code);
                                    }
                                }
                            }
                            let first_segment = resolved_path.first().map(String::as_str).unwrap_or("");
                            if let Some(behavior) = ENGINE_REGISTRY.get_field_assign_behavior(&node_type_enum, first_segment) {
                                let rest_path = resolved_path.get(1..).map(|v| v.join(".")).unwrap_or_default();
                                let mutate_expr = if rest_path.is_empty() {
                                    format!("__g {}= {}", op.to_rust_assign(), final_rhs)
                                } else {
                                    format!("__g.{} {}= {}", rest_path, op.to_rust_assign(), final_rhs)
                                };
                                return format!("{}        {}\n", temp_decl, behavior.emit_get_set_block(&node_id_with_self, &mutate_expr, &node_type));
                            }
                        }

                        format!(
                            "{}        api.mutate_node({}, |{}: &mut {}| {{ {}.{} {}= {}; }});\n",
                            temp_decl,
                            node_id_with_self,
                            clean_closure_var,
                            node_type,
                            clean_closure_var,
                            resolved_field_path,
                            op.to_rust_assign(),
                            final_rhs
                        )
                    }
                } else {
                    // Regular member assignment (not a node)
                    let lhs_code = lhs_expr.to_rust(needs_self, script, current_func);
                    // lhs_expr is TypedExpr, which already passes span through
                    let lhs_type = script.infer_expr_type(&lhs_expr.expr, current_func);

                    let rhs_code = rhs_expr.expr.to_rust(
                        needs_self,
                        script,
                        lhs_type.as_ref(),
                        current_func,
                        rhs_expr.span.as_ref(),
                    );

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

            Stmt::Return(expr) => {
                if let Some(expr) = expr {
                    let expr_code = expr
                        .expr
                        .to_rust(needs_self, script, None, current_func, None);
                    format!("return {};", expr_code)
                } else {
                    "return;".to_string()
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
                result.push_str(&format!(
                    "        for {} in {} {{\n",
                    loop_var_name, iter_str
                ));

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
                            if loop_node_vars.contains(name)
                                && !nodes_created_this_iter.contains(name)
                            {
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
                            if loop_node_vars.contains(name)
                                && !nodes_created_this_iter.contains(name)
                            {
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

                // If var is a global (Root, @global TestGlobal, etc.), use NodeID::from_u32 from global registry (Root=1, first global=2, ...).
                // Otherwise use the node variable's _id suffix.
                let mut node_id_expr = script
                    .global_name_to_node_id
                    .get(var)
                    .copied()
                    .map(|id| format!("NodeID::from_u32({})", id))
                    .unwrap_or_else(|| format!("{}_id", var));
                // If var is Option<NodeID> (e.g. from get_node), unwrap so get_script_var_id/set_script_var_id get NodeID.
                // Do NOT add .expect() for globals (NodeID::from_u32) or for function params typed as node (concrete NodeID).
                let ty = script.get_variable_type(var).or_else(|| {
                    current_func.and_then(|f| {
                        f.locals.iter().find(|v| v.name == *var).and_then(|v| v.typ.as_ref())
                    }).or_else(|| {
                        current_func.and_then(|f| {
                            f.params.iter().find(|p| p.name == *var).map(|p| &p.typ)
                        })
                    })
                });
                if let Some(ty) = ty {
                    let is_concrete_node_param = current_func.and_then(|f| {
                        f.params.iter().find(|p| p.name == *var).map(|p| matches!(p.typ, Type::Node(_) | Type::DynNode))
                    }).unwrap_or(false);
                    if !node_id_expr.starts_with("NodeID::from_u32(") && !is_concrete_node_param
                        && (matches!(ty, Type::Option(inner) if matches!(inner.as_ref(), Type::DynNode))
                            || matches!(ty, Type::Custom(name) if name == "UuidOption"))
                    {
                        node_id_expr = format!("{}.expect(\"Child node not found\")", node_id_expr);
                    }
                }

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
                    node_id_expr, var_id, val_expr
                )
            }

            Stmt::ScriptAssignOp(var, field, op, rhs) => {
                let rhs_str = rhs.to_rust(needs_self, script, current_func);
                // rhs is TypedExpr, which already passes span through

                // If var is a global, use NodeID::from_u32 from global registry; else use {}_id
                let mut node_id_expr = script
                    .global_name_to_node_id
                    .get(var)
                    .copied()
                    .map(|id| format!("NodeID::from_u32({})", id))
                    .unwrap_or_else(|| format!("{}_id", var));
                let ty = script.get_variable_type(var).or_else(|| {
                    current_func.and_then(|f| {
                        f.locals.iter().find(|v| v.name == *var).and_then(|v| v.typ.as_ref())
                    }).or_else(|| {
                        current_func.and_then(|f| {
                            f.params.iter().find(|p| p.name == *var).map(|p| &p.typ)
                        })
                    })
                });
                if let Some(ty) = ty {
                    let is_concrete_node_param = current_func.and_then(|f| {
                        f.params.iter().find(|p| p.name == *var).map(|p| matches!(p.typ, Type::Node(_) | Type::DynNode))
                    }).unwrap_or(false);
                    if !node_id_expr.starts_with("NodeID::from_u32(") && !is_concrete_node_param
                        && (matches!(ty, Type::Option(inner) if matches!(inner.as_ref(), Type::DynNode))
                            || matches!(ty, Type::Custom(name) if name == "UuidOption"))
                    {
                        node_id_expr = format!("{}.expect(\"Child node not found\")", node_id_expr);
                    }
                }

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
                    node_id_expr, var_id, rhs_str, op_rust, node_id_expr, var_id
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
                                matches!(t, Type::Object | Type::Any | Type::Custom(_))
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

                    let rhs_code = rhs_expr.expr.to_rust(
                        needs_self,
                        script,
                        Some(value_ty),
                        current_func,
                        None,
                    );

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
                    let rhs_code = rhs_expr.expr.to_rust(
                        needs_self,
                        script,
                        lhs_type.as_ref(),
                        current_func,
                        rhs_expr.span.as_ref(),
                    );

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
                        inner_types.get(0).map_or(true, |t| {
                            matches!(t, Type::Object | Type::Any | Type::Custom(_))
                        })
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
                    let rhs_code = rhs_expr.expr.to_rust(
                        needs_self,
                        script,
                        lhs_type.as_ref(),
                        current_func,
                        rhs_expr.span.as_ref(),
                    );

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
                            let inner_section =
                                &trimmed["String::from(".len()..trimmed.len() - 1].trim();
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
            // Node types -> DynNode (NodeID at runtime)
            (Node(_), DynNode) => {
                expr.to_string() // Already a NodeID, no conversion needed
            }
            // DynNode -> Node type (for type checking, just pass through)
            (DynNode, Node(_)) => {
                expr.to_string() // Already a NodeID, no conversion needed
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
            Stmt::Return(expr) => expr.as_ref().map_or(false, |e| e.contains_self()),
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
            Stmt::Return(expr) => expr.as_ref().map_or(false, |e| e.contains_api_call(script)),
        }
    }
}
