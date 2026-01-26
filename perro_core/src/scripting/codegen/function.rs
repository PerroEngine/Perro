// Function code generation
use crate::ast::*;
use crate::scripting::ast::{Stmt, Type, Expr};
use crate::node_registry::NodeType;
use crate::structs::engine_registry::ENGINE_REGISTRY;
use std::fmt::Write as _;
use regex::Regex;
use super::utils::{rename_function, rename_variable, type_becomes_id};
use super::analysis::{collect_cloned_node_vars, extract_node_member_info};

/// Post-process generated Rust code to batch consecutive api.mutate_node calls on the same node
fn batch_consecutive_mutations(code: &str) -> String {
    // Pattern to match: api.mutate_node(node_id, |closure_var: &mut NodeType| { body });
    let re = Regex::new(r"(?m)^\s*api\.mutate_node\(([^,]+),\s*\|([^:]+):\s*&mut\s+([^|]+)\|\s*\{\s*([^}]+)\s*\}\);?\s*$").unwrap();
    
    let lines: Vec<&str> = code.lines().collect();
    let mut result = String::with_capacity(code.len());
    let mut i = 0;
    
    while i < lines.len() {
        let line = lines[i];
        
        // Try to match a mutate_node call
        if let Some(caps) = re.captures(line) {
            let node_id = caps.get(1).unwrap().as_str();
            let closure_var = caps.get(2).unwrap().as_str();
            let node_type = caps.get(3).unwrap().as_str();
            let first_body = caps.get(4).unwrap().as_str();
            
            // Collect all consecutive mutations on the same node
            let mut bodies = vec![first_body.trim()];
            let mut j = i + 1;
            
            while j < lines.len() {
                if let Some(next_caps) = re.captures(lines[j]) {
                    let next_node_id = next_caps.get(1).unwrap().as_str();
                    let next_closure_var = next_caps.get(2).unwrap().as_str();
                    let next_node_type = next_caps.get(3).unwrap().as_str();
                    
                    // Same node, same closure var, same type - batch it
                    if next_node_id == node_id && next_closure_var == closure_var && next_node_type == node_type {
                        let next_body = next_caps.get(4).unwrap().as_str();
                        bodies.push(next_body.trim());
                        j += 1;
                    } else {
                        break;
                    }
                } else {
                    break;
                }
            }
            
            // Generate batched mutation
            if bodies.len() > 1 {
                // Multiple mutations - batch them
                let indent = line.chars().take_while(|c| c.is_whitespace()).collect::<String>();
                result.push_str(&format!("{}api.mutate_node({}, |{}: &mut {}| {{\n", indent, node_id, closure_var, node_type));
                for body in &bodies {
                    // Remove trailing semicolon from body if present (we'll add it back)
                    let body_trimmed = body.trim_end_matches(';').trim();
                    result.push_str(&format!("{}    {};\n", indent, body_trimmed));
                }
                result.push_str(&format!("{}}});\n", indent));
            } else {
                // Single mutation - keep as is
                result.push_str(line);
                result.push('\n');
            }
            
            i = j;
        } else {
            // Not a mutate_node call, keep as is
            result.push_str(line);
            result.push('\n');
            i += 1;
        }
    }
    
    result
}

/// Generate batched mutation code for consecutive mutations on the same dynamic node
/// This finds the intersection of compatible node types and generates a single match statement
fn generate_batched_dynamic_mutation(
    mutations: &[(crate::scripting::ast::TypedExpr, &Stmt)],
    node_id: String,
    needs_self: bool,
    script: &Script,
    current_func: Option<&Function>,
) -> String {
    
    // Extract all field paths from mutations
    let mut field_paths = Vec::new();
    let mut mutation_data = Vec::new();
    
    for (lhs_expr, stmt) in mutations {
        // Build field path
        let mut field_path_vec = vec![];
        let mut current_expr = &lhs_expr.expr;
        while let Expr::MemberAccess(inner_base, inner_field) = current_expr {
            field_path_vec.push(inner_field.clone());
            current_expr = inner_base.as_ref();
        }
        field_path_vec.reverse();
        
        field_paths.push(field_path_vec.clone());
        
        // Get RHS expression
        if let Stmt::MemberAssign(_, rhs_expr) = stmt {
            mutation_data.push((field_path_vec, rhs_expr.clone()));
        }
    }
    
    // Determine node_id_with_self
    let node_id_with_self = if !node_id.starts_with("self.") && !node_id.starts_with("api.") && script.is_struct_field(&node_id) {
        format!("self.{}", node_id)
    } else {
        node_id.clone()
    };
    
    // Check if any field is on the base Node type - if so, use mutate_scene_node for those
    // But only if ALL fields are base Node fields (single field paths)
    let all_base_node_fields = field_paths.iter().all(|path| {
        path.len() == 1 && ENGINE_REGISTRY.get_field_type_node(&NodeType::Node, &path[0]).is_some()
    });
    
    if all_base_node_fields && field_paths.len() == 1 {
        // Single base Node field - use mutate_scene_node
        let first_field = &field_paths[0][0];
        let setter_method = match first_field.as_str() {
            "name" => Some("set_name"),
            "id" => Some("set_id"),
            "local_id" => Some("set_local_id"),
            "parent" => Some("set_parent"),
            "script_path" => Some("set_script_path"),
            _ => None,
        };
        
        if let Some(setter) = setter_method {
            // Generate batched body with all mutations
            let mut batched_body = Vec::new();
            for (_, rhs_expr) in &mutation_data {
                // Get the expected type for this setter by looking up the field type
                // The setter parameter type should match the field type
                let expected_setter_type = ENGINE_REGISTRY.get_field_type_node(&NodeType::Node, first_field);
                
                // Generate RHS code with the expected type as a hint
                let rhs_code = rhs_expr.expr.to_rust(needs_self, script, expected_setter_type.as_ref(), current_func, rhs_expr.span.as_ref());
                
                // Infer the RHS type and use type conversion if needed
                let rhs_for_setter = if let Some(expected_type) = expected_setter_type {
                    if let Some(rhs_ty) = script.infer_expr_type(&rhs_expr.expr, current_func) {
                        if rhs_ty.can_implicitly_convert_to(&expected_type) && rhs_ty != expected_type {
                            script.generate_implicit_cast_for_expr(&rhs_code, &rhs_ty, &expected_type)
                        } else {
                            rhs_code
                        }
                    } else {
                        rhs_code
                    }
                } else {
                    rhs_code
                };
                
                batched_body.push(format!("n.{}({});", setter, rhs_for_setter));
            }
            
            return format!(
                "        api.mutate_scene_node({}, |n| {{\n            {}\n        }});\n",
                node_id_with_self,
                batched_body.join("\n            ")
            );
        }
    }
    
    // Find intersection of compatible node types for all field paths
    let compatible_node_types = ENGINE_REGISTRY.intersect_nodes_by_fields(&field_paths);
    
    if compatible_node_types.is_empty() {
        // No compatible types - generate individual mutations (fallback)
        let mut result = String::new();
        for (lhs_expr, stmt) in mutations {
            result.push_str(&stmt.to_rust(needs_self, script, current_func));
        }
        return result;
    }
    
    // Determine node_id_with_self (need to do this before the early return above)
    let node_id_with_self = if !node_id.starts_with("self.") && !node_id.starts_with("api.") && script.is_struct_field(&node_id) {
        format!("self.{}", node_id)
    } else {
        node_id.clone()
    };
    
    // Extract closure var name from first mutation
    let clean_closure_var = if let Some((_, _, _, closure_var)) = extract_node_member_info(&mutations[0].0.expr, script, current_func) {
        closure_var.strip_prefix("self.").unwrap_or(&closure_var).to_string()
    } else {
        "node".to_string()
    };
    
    // If only one compatible type, use it directly (no match needed)
    if compatible_node_types.len() == 1 {
        let node_type_name = format!("{:?}", compatible_node_types[0]);
        
        // Generate RHS code for all mutations and extract API calls
        let mut all_extracted_api_calls = Vec::new();
        let mut temp_var_types: std::collections::HashMap<String, Type> = std::collections::HashMap::new();
        let mut temp_counter = 0usize;
        
        // Helper to extract API calls (simplified version)
        fn extract_api_calls_simple(expr: &Expr, script: &Script, current_func: Option<&Function>,
                                   extracted: &mut Vec<(String, String)>,
                                   temp_var_types: &mut std::collections::HashMap<String, Type>,
                                   temp_counter: &mut usize) -> Expr {
            match expr {
                Expr::ApiCall(api_module, api_args) => {
                    let current_index = *temp_counter;
                    *temp_counter += 1;
                    let temp_var = format!("__temp_api_{}", current_index);
                    let api_call_str = api_module.to_rust(api_args, script, false, current_func);
                    let inferred_type = api_module.return_type();
                    let type_annotation = inferred_type.as_ref().map(|t| format!(": {}", t.to_rust_type())).unwrap_or_default();
                    if let Some(ty) = inferred_type {
                        temp_var_types.insert(temp_var.clone(), ty);
                    }
                    extracted.push((format!("let {}{} = {};", temp_var, type_annotation, api_call_str), temp_var.clone()));
                    Expr::Ident(temp_var)
                }
                _ => expr.clone(),
            }
        }
        
        let mut batched_body = Vec::new();
        for (field_path_vec, rhs_expr) in &mutation_data {
            let modified_rhs = extract_api_calls_simple(&rhs_expr.expr, script, current_func, &mut all_extracted_api_calls, &mut temp_var_types, &mut temp_counter);
            let rhs_code = modified_rhs.to_rust(needs_self, script, None, current_func, rhs_expr.span.as_ref());
            
            // Resolve field path
            let resolved_path: Vec<String> = field_path_vec.iter()
                .map(|f| ENGINE_REGISTRY.resolve_field_name(&compatible_node_types[0], f))
                .collect();
            let resolved_field_path = resolved_path.join(".");
            
            batched_body.push(format!("{}.{} = {};", clean_closure_var, resolved_field_path, rhs_code));
        }
        
        let temp_decl = if !all_extracted_api_calls.is_empty() {
            Some(all_extracted_api_calls.iter().map(|(decl, _)| decl.clone()).collect::<Vec<_>>().join(" "))
        } else {
            None
        };
        
        let temp_decl_str = temp_decl.as_ref().map(|d| format!("        {}\n", d)).unwrap_or_default();
        format!(
            "{}        api.mutate_node({}, |{}: &mut {}| {{\n            {}\n        }});\n",
            temp_decl_str,
            node_id_with_self,
            clean_closure_var,
            node_type_name,
            batched_body.join("\n            ")
        )
    } else {
        // Multiple compatible types - generate match statement
        let mut match_arms = Vec::new();
        
        for node_type_enum in &compatible_node_types {
            let node_type_name = format!("{:?}", node_type_enum);
            
            // Generate batched body for this node type
            let mut batched_body = Vec::new();
            for (field_path_vec, rhs_expr) in &mutation_data {
                let rhs_code = rhs_expr.expr.to_rust(needs_self, script, None, current_func, rhs_expr.span.as_ref());
                
                // Resolve field path for this node type
                let resolved_path: Vec<String> = field_path_vec.iter()
                    .map(|f| ENGINE_REGISTRY.resolve_field_name(node_type_enum, f))
                    .collect();
                let resolved_field_path = resolved_path.join(".");
                
                batched_body.push(format!("{}.{} = {};", clean_closure_var, resolved_field_path, rhs_code));
            }
            
            match_arms.push(format!(
                "            NodeType::{} => api.mutate_node({}, |{}: &mut {}| {{\n                {}\n            }}),",
                node_type_name,
                node_id_with_self,
                clean_closure_var,
                node_type_name,
                batched_body.join("\n                ")
            ));
        }
        
        format!(
            "        match api.get_node_type({}) {{\n{}\n            _ => {{\n                let node_name = api.read_scene_node({}, |n| n.get_name().to_string());\n                let node_type = format!(\"{{:?}}\", api.get_node_type({}));\n                panic!(\"{{}} of type {{}} doesn't have all required fields\", node_name, node_type);\n            }}\n        }}\n",
            node_id_with_self,
            match_arms.join("\n"),
            node_id_with_self,
            node_id_with_self
        )
    }
}

impl Function {
    pub fn to_rust_method(&self, _node_type: &str, script: &Script) -> String {
        let mut out = String::with_capacity(512);

        // Generate method signature using owned parameters
        let mut param_list = String::from("&mut self");

        if !self.params.is_empty() {
            let joined = self
                .params
                .iter()
                .map(|p| {
                    // Always rename parameters with the transpiled ident prefix
                    let renamed = rename_variable(&p.name, Some(&p.typ));
                    
                    // Check if it's a type that becomes NodeID/TextureID/etc or Option<NodeID>/Option<TextureID>
                    if type_becomes_id(&p.typ) {
                        if matches!(&p.typ, Type::Option(boxed) if matches!(boxed.as_ref(), Type::Uid32)) {
                            format!("mut {}: Option<NodeID>", renamed)
                        } else if matches!(&p.typ, Type::Node(_) | Type::DynNode) {
                            format!("mut {}: NodeID", renamed)
                        } else if matches!(&p.typ, Type::EngineStruct(es) if matches!(es, crate::structs::engine_structs::EngineStruct::Texture)) {
                            format!("mut {}: TextureID", renamed)
                        } else {
                            format!("mut {}: NodeID", renamed)
                        }
                    } else {
                        match &p.typ {
                            Type::String => format!("mut {}: String", renamed),
                            Type::Custom(name) => format!("mut {}: {}", renamed, name),
                            _ => format!("mut {}: {}", renamed, p.typ.to_rust_type()),
                        }
                    }
                })
                .collect::<Vec<_>>()
                .join(", ");

            write!(param_list, ", {}", joined).unwrap();
        }

        param_list.push_str(", api: &mut ScriptApi<'_>");

        let renamed_func_name = rename_function(&self.name);
        writeln!(out, "    fn {}({}) {{", renamed_func_name, param_list).unwrap();

        let needs_self = self.uses_self;

        // Use cloned child nodes that were already collected during analysis
        let _cloned_node_vars = &self.cloned_child_nodes;

        // Collect cloned UI elements
        let mut cloned_ui_elements: Vec<(String, String, String)> = Vec::new();
        collect_cloned_node_vars(&self.body, &mut Vec::new(), &mut cloned_ui_elements, script);

        // Emit body with batching for consecutive dynamic node mutations
        let mut i = 0;
        while i < self.body.len() {
            let stmt = &self.body[i];
            
            // Check if this is a mutation on a dynamic node
            if let Stmt::MemberAssign(lhs_expr, _) = stmt {
                if let Some((node_id, node_type, _, _)) = extract_node_member_info(&lhs_expr.expr, script, Some(self)) {
                    if node_type == "__DYN_NODE__" {
                        // Collect consecutive mutations on the same dynamic node
                        let mut mutations = vec![(lhs_expr.clone(), &self.body[i])];
                        let mut j = i + 1;
                        
                        while j < self.body.len() {
                            if let Stmt::MemberAssign(next_lhs, _) = &self.body[j] {
                                if let Some((next_node_id, next_node_type, _, _)) = extract_node_member_info(&next_lhs.expr, script, Some(self)) {
                                    if next_node_type == "__DYN_NODE__" && next_node_id == node_id {
                                        mutations.push((next_lhs.clone(), &self.body[j]));
                                        j += 1;
                                        continue;
                                    }
                                }
                            }
                            break;
                        }
                        
                        if mutations.len() > 1 {
                            // Batch multiple mutations
                            out.push_str(&generate_batched_dynamic_mutation(&mutations, node_id, needs_self, script, Some(self)));
                            i = j;
                            continue;
                        }
                    }
                }
            }
            
            // Not a batched mutation, generate normally
            out.push_str(&stmt.to_rust(needs_self, script, Some(self)));
            i += 1;
        }

        // Merge cloned UI elements back into their UINodes
        if !cloned_ui_elements.is_empty() {
            out.push_str("\n        // Merge cloned UI elements back\n");
            use std::collections::HashMap;
            let mut by_ui_node: HashMap<String, Vec<(String, String)>> = HashMap::new();
            for (ui_node_var, element_name, element_var) in &cloned_ui_elements {
                by_ui_node
                    .entry(ui_node_var.clone())
                    .or_insert_with(Vec::new)
                    .push((element_name.clone(), element_var.clone()));
            }
            for (ui_node_var, elements) in by_ui_node {
                let merge_pairs: Vec<String> = elements
                    .iter()
                    .map(|(name, var)| {
                        format!(
                            "(\"{}\".to_string(), crate::ui_element::UIElement::Text({}.clone()))",
                            name, var
                        )
                    })
                    .collect();
                out.push_str(&format!(
                    "        {}.merge_elements(vec![{}]);\n",
                    ui_node_var,
                    merge_pairs.join(", ")
                ));
            }
        }

        out.push_str("    }\n\n");
        
        // Post-process to batch consecutive mutations on the same node
        batch_consecutive_mutations(&out)
    }

    // For trait-style API methods
    pub fn to_rust_trait_method(&self, _node_type: &str, script: &Script) -> String {
        let mut out = String::with_capacity(512);
        writeln!(
            out,
            "    fn {}(&mut self, api: &mut ScriptApi<'_>) {{",
            self.name.to_lowercase()
        )
        .unwrap();

        let needs_self = self.uses_self;

        // Emit body
        for stmt in &self.body {
            out.push_str(&stmt.to_rust(needs_self, script, Some(self)));
        }

        out.push_str("    }\n\n");
        
        // Post-process to batch consecutive mutations on the same node
        batch_consecutive_mutations(&out)
    }
}
