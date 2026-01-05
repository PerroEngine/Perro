// File I/O operations for code generation
use std::fs;
use std::path::Path;
use regex::Regex;
use crate::ast::*;
use crate::scripting::ast::Type;
use super::utils::to_pascal_case;
use super::boilerplate::implement_script_boilerplate_internal;

/// Strip println! and eprintln! statements from Rust code when not in verbose mode
fn strip_rust_prints(code: &str) -> String {
    // Match println! and eprintln! macro calls on a single line
    // Pattern matches: println!(...); or eprintln!(...);
    let print_re = Regex::new(r"(?m)^(\s*)((?:println|eprintln)!\([^;]*\);?)\s*$").unwrap();
    
    print_re.replace_all(code, |caps: &regex::Captures| {
        let indent = caps.get(1).map(|m| m.as_str()).unwrap_or("");
        let print_call = caps.get(2).map(|m| m.as_str()).unwrap_or("");
        format!("{}// [stripped for release] {}\n", indent, print_call)
    }).to_string()
}

pub fn write_to_crate(
    project_path: &Path,
    contents: &str,
    struct_name: &str,
) -> Result<(), String> {
    let base_path = project_path.join(".perro/scripts/src");
    let lower_name = struct_name.to_lowercase();
    let file_path = base_path.join(format!("{}.rs", lower_name));

    fs::create_dir_all(&base_path).map_err(|e| format!("Failed to create dir: {}", e))?;

    fs::write(&file_path, contents).map_err(|e| format!("Failed to write file: {}", e))?;

    let lib_rs_path = base_path.join("lib.rs");
    let mut current_content = fs::read_to_string(&lib_rs_path).unwrap_or_default();

    let mod_line = format!("pub mod {};", lower_name);
    if !current_content.contains(&mod_line) {
        current_content = current_content.replace(
            "// __PERRO_MODULES__",
            &format!("{}\n// __PERRO_MODULES__", mod_line),
        );
    }

    let import_line = format!("use {}::{}_create_script;", lower_name, lower_name);
    if !current_content.contains(&import_line) {
        current_content = current_content.replace(
            "// __PERRO_IMPORTS__",
            &format!("{}\n// __PERRO_IMPORTS__", import_line),
        );
    }

    // Check if this entry already exists in the phf_map!
    let existing_entry = format!("\"{}\" =>", lower_name);
    if !current_content.contains(&existing_entry) {
        let registry_line = format!(
            "    \"{}\" => {}_create_script as CreateFn,\n",
            lower_name, lower_name
        );
        current_content = current_content.replace(
            "    // __PERRO_REGISTRY__",
            &format!("{}    // __PERRO_REGISTRY__", registry_line),
        );
    }

    fs::write(&lib_rs_path, current_content)
        .map_err(|e| format!("Failed to update lib.rs: {}", e))?;

    Ok(())
}

fn extract_create_script_fn_name(contents: &str) -> Option<String> {
    for line in contents.lines() {
        if line.contains("pub extern \"C\" fn") && line.contains("_create_script") {
            if let Some(start) = line.find("fn ") {
                let after_fn = &line[start + 3..];
                if let Some(end) = after_fn.find('(') {
                    let fn_name = after_fn[..end].trim();
                    if fn_name.ends_with("_create_script") {
                        return Some(fn_name.to_string());
                    }
                }
            }
        }
    }
    None
}

pub fn derive_rust_perro_script(
    project_path: &Path,
    code: &str,
    struct_name: &str,
    verbose: bool,
) -> Result<(), String> {
    let marker_re = Regex::new(r"///\s*@PerroScript").unwrap();
    let marker_pos = match marker_re.find(code) {
        Some(m) => m.end(),
        None => return write_to_crate(project_path, code, struct_name),
    };

    let struct_after_marker_re = Regex::new(r"struct\s+(\w+)\s*\{([^}]*)\}").unwrap();
    let captures = struct_after_marker_re
        .captures(&code[marker_pos..])
        .ok_or_else(|| "Could not find struct after @PerroScript".to_string())?;

    let actual_struct_name_from_struct = captures[1].to_string();
    let struct_body = captures[2].to_string();

    let mut variables = Vec::new();
    let mut attributes_map = std::collections::HashMap::new();

    // Parse attributes from doc comments: ///@Expose, ///@OtherAttr, etc.
    // This regex matches: ///@AttributeName followed by a field or function
    let attr_re =
        Regex::new(r"///\s*@(\w+)[^\n]*\n\s*(?:pub\s+)?(\w+)(?:\s*:\s*[^,\n}]+)?[,}]?").unwrap();

    // First, collect all attributes for fields
    for cap in attr_re.captures_iter(&struct_body) {
        let attr_name = cap[1].to_string();
        let member_name = cap[2].to_string();

        // Skip if it's the node field
        if member_name == "node" {
            continue;
        }

        attributes_map
            .entry(member_name.clone())
            .or_insert_with(Vec::new)
            .push(attr_name);
    }

    // Parse exposed fields (///@expose)
    let expose_re =
        Regex::new(r"///\s*@expose[^\n]*\n\s*(?:pub\s+)?(\w+)\s*:\s*([^,]+),?").unwrap();
    for cap in expose_re.captures_iter(&struct_body) {
        let name = cap[1].to_string();
        let typ = cap[2].trim().to_string();
        let mut is_pub = false;
        if cap[0].contains("pub") {
            is_pub = true;
        }

        // Ensure Expose attribute is in the map
        attributes_map
            .entry(name.clone())
            .or_insert_with(Vec::new)
            .push("Expose".to_string());

        variables.push(Variable {
            name: name.clone(),
            typ: Some(Variable::parse_type(&typ)),
            value: None,
            is_exposed: true,
            is_public: is_pub,
            attributes: attributes_map.get(&name).cloned().unwrap_or_default(),
            span: None,
        });
    }

    // Parse public fields (pub field: Type)
    let pub_re = Regex::new(r"pub\s+(\w+)\s*:\s*([^,\n}]+)").unwrap();
    for cap in pub_re.captures_iter(&struct_body) {
        let name = cap[1].to_string();
        if name == "node" || variables.iter().any(|v| v.name == name) {
            continue;
        }
        let typ = cap[2].trim().to_string();
        variables.push(Variable {
            name: name.clone(),
            typ: Some(Variable::parse_type(&typ)),
            value: None,
            is_exposed: false,
            is_public: true,
            attributes: attributes_map.get(&name).cloned().unwrap_or_default(),
            span: None,
        });
    }

    let lower_name = struct_name.to_lowercase();

    // Extract struct name from @PerroScript struct definition (most accurate)
    // Fallback to extracting from "impl Script for ..." if struct not found
    let impl_script_re = Regex::new(r"impl\s+Script\s+for\s+(\w+)\s*\{").unwrap();
    let actual_struct_name = if !actual_struct_name_from_struct.is_empty() {
        actual_struct_name_from_struct
    } else if let Some(cap) = impl_script_re.captures(code) {
        cap[1].to_string()
    } else {
        to_pascal_case(struct_name)
    };

    // Extract function names from impl blocks
    let mut functions = Vec::new();

    // FIRST: Parse trait methods from impl Script for StructName { ... } block
    // Use a simpler approach: find the impl block start, then scan for trait methods
    let impl_script_marker = format!("impl Script for {}", actual_struct_name);
    if let Some(start_pos) = code.find(&impl_script_marker) {
        // Find the opening brace
        if let Some(brace_pos) = code[start_pos..].find('{') {
            let block_start = start_pos + brace_pos;
            
            // Search for trait methods after this point (they must be before the next impl block or EOF)
            let next_impl_pos = code[block_start..].find("impl ")
                .map(|p| block_start + p)
                .unwrap_or(code.len());
            
            let search_region = &code[block_start..next_impl_pos];
            
            // Find init, update, fixed_update, draw methods
            let fn_re = Regex::new(r"fn\s+(init|update|fixed_update|draw)\s*\(").unwrap();

            for fn_cap in fn_re.captures_iter(search_region) {
                let fn_name = fn_cap[1].to_string();

                functions.push(Function {
                    name: fn_name.clone(),
                    is_trait_method: true,  // Mark as trait method for flag detection
                    params: vec![],
                    return_type: Type::Void,
                    uses_self: false,
                    cloned_child_nodes: Vec::new(),
                    body: vec![],
                    locals: vec![],
                    attributes: vec![],
                    is_on_signal: false,
                    signal_name: None,
                    span: None,
                });
            }
        }
    }

    // SECOND: Find impl StructName { ... } blocks (non-trait methods)
    // Use actual_struct_name extracted from @PerroScript struct definition
    // Try both with and without "Script" suffix for backwards compatibility
    // Find the impl block start position
    let impl_pattern1 = format!("impl {}", format!("{}Script", actual_struct_name));
    let impl_pattern2 = format!("impl {}", actual_struct_name);
    
    let impl_start = code.find(&impl_pattern1)
        .or_else(|| code.find(&impl_pattern2));
    
    if let Some(start_pos) = impl_start {
        // Find the opening brace
        if let Some(brace_start) = code[start_pos..].find('{') {
            let brace_pos = start_pos + brace_start;
            
            // Find matching closing brace by counting braces
            let mut brace_count = 0;
            let mut impl_end = None;
            for (i, ch) in code[brace_pos..].char_indices() {
                match ch {
                    '{' => brace_count += 1,
                    '}' => {
                        brace_count -= 1;
                        if brace_count == 0 {
                            impl_end = Some(brace_pos + i + 1);
                            break;
                        }
                    }
                    _ => {}
                }
            }
            
            if let Some(end_pos) = impl_end {
                let impl_body = &code[brace_pos + 1..end_pos - 1]; // Exclude the braces

        // Parse attributes for functions: ///@AttributeName before pub fn or fn function_name
        let fn_attr_re = Regex::new(r"///\s*@(\w+)[^\n]*\n\s*(?:pub\s+)?fn\s+(\w+)").unwrap();
        for attr_cap in fn_attr_re.captures_iter(impl_body) {
            let attr_name = attr_cap[1].to_string();
            let fn_name = attr_cap[2].to_string();
            attributes_map
                .entry(fn_name.clone())
                .or_insert_with(Vec::new)
                .push(attr_name);
        }

        // Find all function definitions with their full signatures
        // Matches: pub fn or fn function_name(&mut self, param: Type, ...) -> ReturnType {
        let fn_re = Regex::new(r"(?:pub\s+)?fn\s+(\w+)\s*\(([^)]*)\)(?:\s*->\s*([^{]+))?").unwrap();

        for fn_cap in fn_re.captures_iter(impl_body) {
            let fn_name = fn_cap[1].to_string();
            let params_str = fn_cap.get(2).map_or("", |m| m.as_str());
            let return_str = fn_cap.get(3).map_or("", |m| m.as_str().trim());

            // Parse parameters
            let mut params = Vec::new();

            // Split by comma and parse each parameter
            for param in params_str.split(',') {
                let param = param.trim();
                if param.is_empty() || param == "&mut self" || param == "&self" {
                    continue;
                }

                // Remove 'mut ' prefix if present
                let param = param.strip_prefix("mut ").unwrap_or(param).trim();

                // Split by ':' to get name and type
                if let Some((name, typ_str)) = param.split_once(':') {
                    let name = name.trim().to_string();
                    let typ_str_trimmed = typ_str.trim();
                    
                    // Keep ScriptApi parameters in the list (we'll handle them specially in boilerplate)
                    // Mark them as Custom("ScriptApi") type so we can detect them
                    let typ = if typ_str_trimmed.contains("ScriptApi") {
                        Type::Custom("ScriptApi".to_string())
                    } else {
                        // For Rust scripts, preserve reference information in Custom types
                        // If it's a reference type like &Path or &Manifest, store it as "&Path" or "&Manifest"
                        // so we can add & prefix when calling the function
                        let parsed = Variable::parse_type(typ_str_trimmed);
                        match &parsed {
                            Type::Custom(tn) if typ_str_trimmed.starts_with('&') && !tn.starts_with('&') => {
                                // Preserve the & prefix for reference types
                                Type::Custom(format!("&{}", tn))
                            },
                            _ => parsed,
                        }
                    };

                    params.push(Param { name, typ, span: None });
                }
            }

            // Parse return type
            let return_type = if return_str.is_empty() {
                Type::Void
            } else {
                Variable::parse_type(return_str)
            };

            functions.push(Function {
                name: fn_name.clone(),
                is_trait_method: false,
                params,
                return_type,
                uses_self: false,
                span: None,
                cloned_child_nodes: Vec::new(), // Will be populated during analyze_self_usage
                body: vec![],
                locals: vec![],
                attributes: attributes_map.get(&fn_name).cloned().unwrap_or_default(),
                is_on_signal: false,
                signal_name: None,
            });
        }
            } else {
                // Couldn't find matching brace, skip this impl block
            }
        }
    }

    let final_contents = if let Some(actual_fn_name) = extract_create_script_fn_name(code) {
        let expected_fn_name = format!("{}_create_script", lower_name);
        code.replace(&actual_fn_name, &expected_fn_name)
    } else {
        code.to_string()
    };

    // Don't generate MEMBER_NAMES and ATTRIBUTES_MAP here - let the boilerplate generate them
    // to avoid duplicates. The boilerplate will add them after the struct definition.
    let injected_code = final_contents.clone();
    let marker_pos = marker_re
        .find(&final_contents)
        .map(|m| m.end())
        .unwrap_or(0);
    let _struct_pos = final_contents[marker_pos..]
        .find("struct ")
        .map(|p| marker_pos + p)
        .unwrap_or(0);

    // No need to inject/fix attributes field - we use MEMBER_TO_ATTRIBUTES_MAP directly in trait methods

    // For Rust scripts, remove any existing MEMBER_TO_ATTRIBUTES_MAP and ATTRIBUTE_TO_MEMBERS_MAP, then generate them once at the top
    // Match multiline from "static MEMBER_TO_ATTRIBUTES_MAP" to the closing "};"
    let member_to_attributes_map_re =
        Regex::new(r"(?s)static\s+MEMBER_TO_ATTRIBUTES_MAP\s*:.*?};").unwrap();
    // Also match old ATTRIBUTES_MAP name for backwards compatibility
    let attributes_map_re = Regex::new(r"(?s)static\s+ATTRIBUTES_MAP\s*:.*?};").unwrap();
    // Match multiline from "static ATTRIBUTE_TO_MEMBERS_MAP" to the closing "};"
    let attribute_to_members_map_re =
        Regex::new(r"(?s)static\s+ATTRIBUTE_TO_MEMBERS_MAP\s*:.*?};").unwrap();
    // Also remove any old MEMBER_NAMES if it exists
    let member_names_re = Regex::new(r"(?s)(pub\s+)?static\s+MEMBER_NAMES\s*:.*?];").unwrap();

    let mut cleaned_code = injected_code.clone();
    cleaned_code = member_names_re.replace_all(&cleaned_code, "").to_string();
    cleaned_code = member_to_attributes_map_re
        .replace_all(&cleaned_code, "")
        .to_string();
    cleaned_code = attributes_map_re.replace_all(&cleaned_code, "").to_string();
    cleaned_code = attribute_to_members_map_re
        .replace_all(&cleaned_code, "")
        .to_string();

    // Generate MEMBER_TO_ATTRIBUTES_MAP and ATTRIBUTE_TO_MEMBERS_MAP once at the top (before struct) - no need for separate MEMBER_NAMES
    // Build reverse index: attribute -> members
    let mut attribute_to_members: std::collections::HashMap<String, Vec<String>> =
        std::collections::HashMap::new();

    let mut attributes_map_code = String::new();
    attributes_map_code.push_str("static MEMBER_TO_ATTRIBUTES_MAP: Map<&'static str, &'static [&'static str]> = phf_map! {\n");
    for var in &variables {
        let attrs = attributes_map
            .get(&var.name)
            .cloned()
            .unwrap_or_else(|| var.attributes.clone());
        // Only store members that have attributes
        if !attrs.is_empty() {
            use std::fmt::Write as _;
            write!(attributes_map_code, "    \"{}\" => &[", var.name).unwrap();
            for (i, attr) in attrs.iter().enumerate() {
                if i > 0 {
                    attributes_map_code.push_str(", ");
                }
                write!(attributes_map_code, "\"{}\"", attr).unwrap();
                attribute_to_members
                    .entry(attr.clone())
                    .or_insert_with(Vec::new)
                    .push(var.name.clone());
            }
            attributes_map_code.push_str("],\n");
        }
    }
    for func in &functions {
        // Suffix function names with "()" to differentiate from variables
        let func_key = format!("{}()", func.name);
        let attrs = attributes_map
            .get(&func.name)
            .cloned()
            .unwrap_or_else(|| func.attributes.clone());
        // Only store members that have attributes
        if !attrs.is_empty() {
            use std::fmt::Write as _;
            write!(attributes_map_code, "    \"{}\" => &[", func_key).unwrap();
            for (i, attr) in attrs.iter().enumerate() {
                if i > 0 {
                    attributes_map_code.push_str(", ");
                }
                write!(attributes_map_code, "\"{}\"", attr).unwrap();
                attribute_to_members
                    .entry(attr.clone())
                    .or_insert_with(Vec::new)
                    .push(func_key.clone());
            }
            attributes_map_code.push_str("],\n");
        }
    }
    attributes_map_code.push_str("};\n\n");

    // Generate reverse index for O(1) attribute lookups
    attributes_map_code.push_str("static ATTRIBUTE_TO_MEMBERS_MAP: Map<&'static str, &'static [&'static str]> = phf_map! {\n");
    for (attr, members) in &attribute_to_members {
        use std::fmt::Write as _;
        write!(attributes_map_code, "    \"{}\" => &[", attr).unwrap();
        for (i, member) in members.iter().enumerate() {
            if i > 0 {
                attributes_map_code.push_str(", ");
            }
            write!(attributes_map_code, "\"{}\"", member).unwrap();
        }
        attributes_map_code.push_str("],\n");
    }
    attributes_map_code.push_str("};\n\n");

    // Find struct position in cleaned code
    let marker_pos = marker_re.find(&cleaned_code).map(|m| m.end()).unwrap_or(0);
    let struct_pos = cleaned_code[marker_pos..]
        .find("struct ")
        .map(|p| marker_pos + p)
        .unwrap_or(0);

    // Inject MEMBER_TO_ATTRIBUTES_MAP and ATTRIBUTE_TO_MEMBERS_MAP before the struct definition
    let mut final_code = cleaned_code;
    if struct_pos > 0 {
        final_code.insert_str(struct_pos, &attributes_map_code);
    }

    let boilerplate =
        implement_script_boilerplate_internal(&actual_struct_name, &variables, &functions, &attributes_map, true);
    let mut combined = format!("{}\n\n{}", final_code, boilerplate);

    // Strip println! and eprintln! statements when not in verbose mode
    if !verbose {
        combined = strip_rust_prints(&combined);
    }

    write_to_crate(project_path, &combined, struct_name)
}

