// Function code generation
use crate::ast::*;
use crate::scripting::ast::{Stmt, Type};
use std::fmt::Write as _;
use regex::Regex;
use super::utils::{rename_function, rename_variable, type_becomes_id};
use super::analysis::collect_cloned_node_vars;

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
                    // Check if it's a type that becomes Uuid or Option<Uuid>
                    if type_becomes_id(&p.typ) {
                        let renamed = rename_variable(&p.name, Some(&p.typ));
                        if matches!(&p.typ, Type::Option(boxed) if matches!(boxed.as_ref(), Type::Uuid)) {
                            format!("mut {}: Option<Uuid>", renamed)
                        } else {
                            format!("mut {}: Uuid", renamed)
                        }
                    } else {
                        match &p.typ {
                            Type::String => format!("mut {}: String", p.name),
                            Type::Custom(name) => format!("mut {}: {}", p.name, name),
                            _ => format!("mut {}: {}", p.name, p.typ.to_rust_type()),
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

        // Emit body
        for stmt in &self.body {
            out.push_str(&stmt.to_rust(needs_self, script, Some(self)));
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
