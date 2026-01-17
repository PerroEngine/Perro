// ScriptObject boilerplate generation
use crate::ast::*;
use crate::scripting::ast::{ContainerKind, NumberKind, Type};
use crate::structs::engine_structs::EngineStruct as EngineStructKind;
use std::collections::HashMap;
use std::fmt::Write as _;
use crate::prelude::string_to_u64;
use super::utils::{rename_variable, rename_function, is_node_type};

pub fn implement_script_boilerplate(
    struct_name: &str,
    script_vars: &[Variable],
    functions: &[Function],
    _attributes_map: &HashMap<String, Vec<String>>,
) -> String {
    implement_script_boilerplate_internal(struct_name, script_vars, functions, _attributes_map, false)
}

pub fn implement_script_boilerplate_internal(
    struct_name: &str,
    script_vars: &[Variable],
    functions: &[Function],
    _attributes_map: &HashMap<String, Vec<String>>,
    is_rust_script: bool,
) -> String {
    let mut out = String::with_capacity(8192);
    let mut get_entries = String::with_capacity(512);
    let mut set_entries = String::with_capacity(512);
    let mut apply_entries = String::with_capacity(512);
    let mut dispatch_entries = String::with_capacity(4096);
    
    // Detect which lifecycle methods are implemented
    let has_init = functions.iter().any(|f| f.is_trait_method && f.name.to_lowercase() == "init");
    let has_update = functions.iter().any(|f| f.is_trait_method && f.name.to_lowercase() == "update");
    let has_fixed_update = functions.iter().any(|f| f.is_trait_method && f.name.to_lowercase() == "fixed_update");
    let has_draw = functions.iter().any(|f| f.is_trait_method && f.name.to_lowercase() == "draw");
    
    // Build the flags value
    let mut flags_value = 0u8;
    if has_init {
        flags_value |= 1; // ScriptFlags::HAS_INIT
    }
    if has_update {
        flags_value |= 2; // ScriptFlags::HAS_UPDATE
    }
    if has_fixed_update {
        flags_value |= 4; // ScriptFlags::HAS_FIXED_UPDATE
    }
    if has_draw {
        flags_value |= 8; // ScriptFlags::HAS_DRAW
    }

    //----------------------------------------------------
    // Generate VAR GET, SET, APPLY tables
    //----------------------------------------------------
    for var in script_vars {
        let name = &var.name;
        let renamed_name = rename_variable(name, var.typ.as_ref());
        let var_id = string_to_u64(name);
        let (accessor, conv) = var.json_access();

        // If public, generate GET and SET entries
        if var.is_public {

            // ------------------------------
            // Special casing for Containers (GET)
            // ------------------------------
            if let Some(Type::Container(kind, _elem_types)) = &var.typ {
                match kind {
                    ContainerKind::Array | ContainerKind::FixedArray(_) | ContainerKind::Map => {
                        writeln!(
                            get_entries,
                            "        {var_id}u64 => |script: &{struct_name}| -> Option<Value> {{
                                Some(serde_json::to_value(&script.{renamed_name}).unwrap_or_default())
                            }},"
                        )
                        .unwrap();
                    }
                }
            } else {
                writeln!(
                    get_entries,
                    "        {var_id}u64 => |script: &{struct_name}| -> Option<Value> {{
                        Some(json!(script.{renamed_name}))
                    }},"
                )
                .unwrap();
            }

            // ------------------------------
            // Special casing for Containers (SET)
            // ------------------------------
            if let Some(Type::Container(kind, elem_types)) = &var.typ {
                match kind {
                    ContainerKind::Array => {
                        let elem_ty = elem_types.get(0).unwrap_or(&Type::Object);
                        let elem_rs = elem_ty.to_rust_type();
                        let field_rust_type = var
                            .typ
                            .as_ref()
                            .map(|t| t.to_rust_type())
                            .unwrap_or_default();
                        // Check if field is Vec<Value> (for custom types or Object)
                        let is_value_vec = field_rust_type == "Vec<Value>"
                            || *elem_ty == Type::Object
                            || matches!(elem_ty, Type::Custom(_));
                        if *elem_ty != Type::Object {
                            if is_value_vec {
                                // Convert Vec<T> to Vec<Value>
                                writeln!(
                                    set_entries,
                                    "        {var_id}u64 => |script: &mut {struct_name}, val: Value| -> Option<()> {{
                                    if let Ok(vec_typed) = serde_json::from_value::<Vec<{elem_rs}>>(val) {{
                                        script.{renamed_name} = vec_typed.into_iter().map(|x| serde_json::to_value(x).unwrap_or_default()).collect();
                                        return Some(());
                                    }}
                                    None
                                }},"
                                ).unwrap();
                            } else {
                                writeln!(
                                    set_entries,
                                    "        {var_id}u64 => |script: &mut {struct_name}, val: Value| -> Option<()> {{
                                    if let Ok(vec_typed) = serde_json::from_value::<Vec<{elem_rs}>>(val) {{
                                        script.{renamed_name} = vec_typed;
                                        return Some(());
                                    }}
                                    None
                                }},"
                                ).unwrap();
                            }
                        } else {
                            writeln!(
                                set_entries,
                                "        {var_id}u64 => |script: &mut {struct_name}, val: Value| -> Option<()> {{
                                    if let Some(v) = val.as_array() {{
                                        script.{renamed_name} = v.clone();
                                        return Some(());
                                    }}
                                    None
                                }},"
                            ).unwrap();
                        }
                    }
                    ContainerKind::FixedArray(size) => {
                        let elem_ty = elem_types.get(0).unwrap_or(&Type::Object);
                        let elem_rs = elem_ty.to_rust_type();
                        if *elem_ty != Type::Object {
                            writeln!(
                                set_entries,
                                "        {var_id}u64 => |script: &mut {struct_name}, val: Value| -> Option<()> {{
                                    if let Ok(arr_typed) = serde_json::from_value::<[{elem_rs}; {size}]>(val) {{
                                        script.{renamed_name} = arr_typed;
                                        return Some(());
                                    }}
                                    None
                                }},"
                            ).unwrap();
                        } else {
                            writeln!(
                                set_entries,
                                "        {var_id}u64 => |script: &mut {struct_name}, val: Value| -> Option<()> {{
                                    if let Some(v) = val.as_array() {{
                                        let mut out: [{elem_rs}; {size}] = [Default::default(); {size}];
                                        for (i, el) in v.iter().enumerate().take({size}) {{
                                            out[i] = serde_json::from_value::<{elem_rs}>(el.clone()).unwrap_or_default();
                                        }}
                                        script.{renamed_name} = out;
                                        return Some(());
                                    }}
                                    None
                                }},"
                            ).unwrap();
                        }
                    }
                    ContainerKind::Map => {
                        let key_ty = elem_types.get(0).unwrap_or(&Type::String);
                        let val_ty = elem_types.get(1).unwrap_or(&Type::Object);
                        let key_rs = key_ty.to_rust_type();
                        let val_rs = val_ty.to_rust_type();

                        let field_rust_type = var
                            .typ
                            .as_ref()
                            .map(|t| t.to_rust_type())
                            .unwrap_or_default();
                        // Check if field is HashMap<String, Value> (for custom types or Object)
                        let is_value_map = field_rust_type == "HashMap<String, Value>"
                            || *val_ty == Type::Object
                            || matches!(val_ty, Type::Custom(_));
                        if *val_ty != Type::Object {
                            if is_value_map {
                                // Convert HashMap<K, T> to HashMap<String, Value>
                                writeln!(
                                    set_entries,
                                    "        {var_id}u64 => |script: &mut {struct_name}, val: Value| -> Option<()> {{
                                    if let Ok(map_typed) = serde_json::from_value::<HashMap<{key_rs}, {val_rs}>>(val) {{
                                        script.{renamed_name} = map_typed.into_iter().map(|(k, v)| (k, serde_json::to_value(v).unwrap_or_default())).collect();
                                        return Some(());
                                    }}
                                    None
                                }},"
                                ).unwrap();
                            } else {
                                writeln!(
                                    set_entries,
                                    "        {var_id}u64 => |script: &mut {struct_name}, val: Value| -> Option<()> {{
                                    if let Ok(map_typed) = serde_json::from_value::<HashMap<{key_rs}, {val_rs}>>(val) {{
                                        script.{renamed_name} = map_typed;
                                        return Some(());
                                    }}
                                    None
                                }},"
                                ).unwrap();
                            }
                        } else {
                            writeln!(
                                set_entries,
                                "        {var_id}u64 => |script: &mut {struct_name}, val: Value| -> Option<()> {{
                                    if let Some(v) = val.as_object() {{
                                        script.{renamed_name} = v.iter().map(|(k, v)| (k.clone(), v.clone())).collect();
                                        return Some(());
                                    }}
                                    None
                                }},"
                            ).unwrap();
                        }
                    }
                }
            } else {
                if accessor == "__CUSTOM__" {
                    let type_name = &conv;
                    writeln!(
                        set_entries,
                        "        {var_id}u64 => |script: &mut {struct_name}, val: Value| -> Option<()> {{
                            if let Ok(v) = serde_json::from_value::<{type_name}>(val) {{
                                script.{renamed_name} = v;
                                return Some(());
                            }}
                            None
                        }},"
                    ).unwrap();
                } else if accessor == "__NODE__" {
                    // Node variables are stored as Uuid, extract from JSON
                    // Nodes are serialized as UUID strings, so extract the string and parse it
                    writeln!(
                        set_entries,
                        "        {var_id}u64 => |script: &mut {struct_name}, val: Value| -> Option<()> {{
                            if let Some(vNodeType) = val.as_str().and_then(|s| uuid::Uuid::parse_str(s).ok()) {{
                                script.{renamed_name} = vNodeType;
                                return Some(());
                            }}
                            None
                        }},"
                    ).unwrap();
                } else {
                    writeln!(
                        set_entries,
                        "        {var_id}u64 => |script: &mut {struct_name}, val: Value| -> Option<()> {{
                            if let Some(v) = val.{accessor}() {{
                                script.{renamed_name} = v{conv};
                                return Some(());
                            }}
                            None
                        }},"
                    ).unwrap();
                }
            }
        }

        // If exposed, generate APPLY entries
        if var.is_exposed {

            // ------------------------------
            // Special casing for Containers (APPLY)
            // ------------------------------
            if let Some(Type::Container(kind, elem_types)) = &var.typ {
                match kind {
                    ContainerKind::Array => {
                        let elem_ty = elem_types.get(0).unwrap_or(&Type::Object);
                        let elem_rs = elem_ty.to_rust_type();
                        let field_rust_type = var
                            .typ
                            .as_ref()
                            .map(|t| t.to_rust_type())
                            .unwrap_or_default();
                        // Check if field is Vec<Value> (for custom types or Object)
                        let is_value_vec = field_rust_type == "Vec<Value>"
                            || *elem_ty == Type::Object
                            || matches!(elem_ty, Type::Custom(_));
                        if *elem_ty != Type::Object {
                            if is_value_vec {
                                // Convert Vec<T> to Vec<Value>
                                writeln!(
                                    apply_entries,
                                    "        {var_id}u64 => |script: &mut {struct_name}, val: &Value| {{
                                    if let Ok(vec_typed) = serde_json::from_value::<Vec<{elem_rs}>>(val.clone()) {{
                                        script.{renamed_name} = vec_typed.into_iter().map(|x| serde_json::to_value(x).unwrap_or_default()).collect();
                                    }}
                                }},"
                                ).unwrap();
                            } else {
                                writeln!(
                                    apply_entries,
                                    "        {var_id}u64 => |script: &mut {struct_name}, val: &Value| {{
                                    if let Ok(vec_typed) = serde_json::from_value::<Vec<{elem_rs}>>(val.clone()) {{
                                        script.{renamed_name} = vec_typed;
                                    }}
                                }},"
                                ).unwrap();
                            }
                        } else {
                            writeln!(
                                apply_entries,
                                "        {var_id}u64 => |script: &mut {struct_name}, val: &Value| {{
                                    if let Some(v) = val.as_array() {{
                                        script.{renamed_name} = v.clone();
                                    }}
                                }},"
                            )
                            .unwrap();
                        }
                    }
                    ContainerKind::FixedArray(size) => {
                        let elem_ty = elem_types.get(0).unwrap_or(&Type::Object);
                        let elem_rs = elem_ty.to_rust_type();
                        if *elem_ty != Type::Object {
                            writeln!(
                                apply_entries,
                                "        {var_id}u64 => |script: &mut {struct_name}, val: &Value| {{
                                    if let Ok(arr_typed) = serde_json::from_value::<[{elem_rs}; {size}]>(val.clone()) {{
                                        script.{renamed_name} = arr_typed;
                                    }}
                                }},"
                            ).unwrap();
                        } else {
                            writeln!(
                                apply_entries,
                                "        {var_id}u64 => |script: &mut {struct_name}, val: &Value| {{
                                    if let Some(v) = val.as_array() {{
                                        let mut out: [{elem_rs}; {size}] = [Default::default(); {size}];
                                        for (i, el) in v.iter().enumerate().take({size}) {{
                                            out[i] = serde_json::from_value::<{elem_rs}>(el.clone()).unwrap_or_default();
                                        }}
                                        script.{renamed_name} = out;
                                    }}
                                }},"
                            ).unwrap();
                        }
                    }
                    ContainerKind::Map => {
                        let key_ty = elem_types.get(0).unwrap_or(&Type::String);
                        let val_ty = elem_types.get(1).unwrap_or(&Type::Object);
                        let key_rs = key_ty.to_rust_type();
                        let val_rs = val_ty.to_rust_type();

                        let field_rust_type = var
                            .typ
                            .as_ref()
                            .map(|t| t.to_rust_type())
                            .unwrap_or_default();
                        // Check if field is HashMap<String, Value> (for custom types or Object)
                        let is_value_map = field_rust_type == "HashMap<String, Value>"
                            || *val_ty == Type::Object
                            || matches!(val_ty, Type::Custom(_));
                        if *val_ty != Type::Object {
                            if is_value_map {
                                // Convert HashMap<K, T> to HashMap<String, Value>
                                writeln!(
                                    apply_entries,
                                    "        {var_id}u64 => |script: &mut {struct_name}, val: &Value| {{
                                    if let Ok(map_typed) = serde_json::from_value::<HashMap<{key_rs}, {val_rs}>>(val.clone()) {{
                                        script.{renamed_name} = map_typed.into_iter().map(|(k, v)| (k, serde_json::to_value(v).unwrap_or_default())).collect();
                                    }}
                                }},"
                                ).unwrap();
                            } else {
                                writeln!(
                                    apply_entries,
                                    "        {var_id}u64 => |script: &mut {struct_name}, val: &Value| {{
                                    if let Ok(map_typed) = serde_json::from_value::<HashMap<{key_rs}, {val_rs}>>(val.clone()) {{
                                        script.{renamed_name} = map_typed;
                                    }}
                                }},"
                                ).unwrap();
                            }
                        } else {
                            writeln!(
                                apply_entries,
                                "        {var_id}u64 => |script: &mut {struct_name}, val: &Value| {{
                                    if let Some(v) = val.as_object() {{
                                        script.{renamed_name} = v.iter().map(|(k, v)| (k.clone(), v.clone())).collect();
                                    }}
                                }},"
                            ).unwrap();
                        }
                    }
                }
            } else {
                if accessor == "__CUSTOM__" {
                    let type_name = &conv;
                    writeln!(
                        apply_entries,
                        "        {var_id}u64 => |script: &mut {struct_name}, val: &Value| {{
                            if let Ok(v) = serde_json::from_value::<{type_name}>(val.clone()) {{
                                script.{renamed_name} = v;
                            }}
                        }},"
                    )
                    .unwrap();
                } else {
                    writeln!(
                        apply_entries,
                        "        {var_id}u64 => |script: &mut {struct_name}, val: &Value| {{
                            if let Some(v) = val.{accessor}() {{
                                script.{renamed_name} = v{conv};
                            }}
                        }},"
                    )
                    .unwrap();
                }
            }
        }
    }

    //----------------------------------------------------
    // FUNCTION DISPATCH TABLE GENERATION
    //----------------------------------------------------
    
    for func in functions {
        if func.is_trait_method {
            continue;
        }
        
        // Skip functions marked with @skip attribute - these are internal helpers
        if func.attributes.iter().any(|attr| attr.to_lowercase() == "skip") {
            continue;
        }

        let func_name = &func.name;
        let func_id = string_to_u64(func_name);
        // For Rust scripts, don't rename functions (they don't have __t_ prefix)
        let renamed_func_name = if is_rust_script {
            func_name.clone()
        } else {
            rename_function(func_name)
        };

        let mut param_parsing = String::new();
        let mut param_list = String::new();

        if !func.params.is_empty() {
            // Track actual parameter index (for JSON params array, skipping ScriptApi params)
            let mut actual_param_idx = 0;
            for param in func.params.iter() {
                // Skip ScriptApi parameters when parsing from JSON - they use the api parameter from the closure
                if matches!(param.typ, Type::Custom(ref tn) if tn == "ScriptApi") {
                    continue;
                }
                
                // Rename parameter: node types get _id suffix, others keep original name
                let param_name = if is_rust_script {
                    // For Rust scripts, use original parameter name
                    param.name.clone()
                } else {
                    rename_variable(&param.name, Some(&param.typ))
                };
                let parse_code = match &param.typ {
                    Type::String | Type::StrRef => format!(
                        "let {param_name} = params.get({actual_param_idx})
                            .and_then(|v| v.as_str())
                            .map(|s| s.to_string())
                            .unwrap_or_default();\n"
                    ),
                    Type::Number(NumberKind::Signed(w)) => format!(
                        "let {param_name} = params.get({actual_param_idx})
                            .and_then(|v| v.as_i64().or_else(|| v.as_f64().map(|f| f as i64)))
                            .unwrap_or_default() as i{w};\n"
                    ),
                    Type::Number(NumberKind::Unsigned(w)) => format!(
                        "let {param_name} = params.get({actual_param_idx})
                            .and_then(|v| v.as_u64().or_else(|| v.as_f64().map(|f| f as u64)))
                            .unwrap_or_default() as u{w};\n"
                    ),
                    Type::Number(NumberKind::Float(32)) => format!(
                        "let {param_name} = params.get({actual_param_idx})
                            .and_then(|v| v.as_f64().or_else(|| v.as_i64().map(|i| i as f64)))
                            .unwrap_or_default() as f32;\n"
                    ),
                    Type::Number(NumberKind::Float(64)) => format!(
                        "let {param_name} = params.get({actual_param_idx})
                            .and_then(|v| v.as_f64().or_else(|| v.as_i64().map(|i| i as f64)))
                            .unwrap_or_default();\n"
                    ),
                    Type::Bool => format!(
                        "let {param_name} = params.get({actual_param_idx})
                            .and_then(|v| v.as_bool())
                            .unwrap_or_default();\n"
                    ),
                    Type::Custom(tn) if tn == "Signal" => format!(
                        "let {param_name} = params.get({actual_param_idx})
                            .and_then(|v| v.as_u64().or_else(|| v.as_f64().map(|f| f as u64)))
                            .unwrap_or_default() as u64;\n"
                    ),
                    Type::Custom(tn) if is_node_type(tn) => {
                        // For node types, parse UUID from string (nodes are just UUIDs)
                        format!(
                            "let {param_name} = params.get({actual_param_idx})
                            .and_then(|v| v.as_str())
                            .and_then(|s| Uuid::parse_str(s).ok())
                            .unwrap_or_default();\n"
                        )
                    },
                    Type::Custom(tn) if tn == "&Path" || tn == "Path" || (tn.contains("Path") && !tn.starts_with('&')) => {
                        // Handle Path types - parse as string and convert to PathBuf
                        // For &Path parameters, we'll create a PathBuf and use a reference
                        format!(
                            "let __path_buf_{param_name} = params.get({actual_param_idx})
                            .and_then(|v| v.as_str())
                            .map(|s| std::path::PathBuf::from(s))
                            .unwrap_or_default();
let {param_name} = __path_buf_{param_name}.as_path();\n"
                        )
                    },
                    Type::Custom(tn) if tn == "&str" || tn == "str" => {
                        // Handle &str - parse as String (we'll borrow it when calling)
                        format!(
                            "let {param_name} = params.get({actual_param_idx})
                            .and_then(|v| v.as_str())
                            .map(|s| s.to_string())
                            .unwrap_or_default();\n"
                        )
                    },
                    Type::Custom(tn) if tn.starts_with("&[") && tn.ends_with(']') => {
                        // Handle slice types like &[String] - deserialize as Vec<T>, then take slice reference
                        let element_type = tn.strip_prefix("&[").and_then(|s| s.strip_suffix(']')).unwrap_or("");
                        // For types that might not have Default, we need to handle None case
                        // If deserialization fails, we can't proceed, so we'll return early
                        format!(
                            "let __vec_{param_name}_opt = params.get({actual_param_idx})
                            .and_then(|v| serde_json::from_value::<Vec<{element_type}>>(v.clone()).ok());
let __vec_{param_name} = match __vec_{param_name}_opt {{
    Some(val) => val,
    None => return, // Skip this function call if deserialization failed
}};
let {param_name} = __vec_{param_name}.as_slice();\n"
                        )
                    },
                    Type::Custom(tn) if tn.starts_with('&') && tn != "&str" && tn != "&Path" => {
                        // Handle reference types like &Manifest - deserialize owned type, then take reference
                        let mut owned_type = tn.strip_prefix('&').unwrap_or(tn).to_string();
                        // Strip 'mut' if present (e.g., "mut FurElement" -> "FurElement")
                        if owned_type.starts_with("mut ") {
                            owned_type = owned_type.strip_prefix("mut ").unwrap_or(&owned_type).to_string();
                        }
                        
                        // Check for internal types that don't implement Deserialize
                        // Handle both short names (FurElement) and full paths (perro_core::nodes::ui::fur_ast::FurElement)
                        if owned_type.contains("FurElement") || owned_type.contains("FurNode") {
                            // For internal types that can't be deserialized, we can't call this function from scripts
                            format!(
                                "return; // Cannot deserialize internal type {owned_type} - this method should not be called from scripts\n"
                            )
                        } else {
                            // For types that might not have Default, we need to handle None case
                            // If deserialization fails, we can't proceed, so we'll return early
                            format!(
                                "let __owned_{param_name}_opt = params.get({actual_param_idx})
                            .and_then(|v| serde_json::from_value::<{owned_type}>(v.clone()).ok());
let {param_name} = match __owned_{param_name}_opt {{
    Some(ref val) => val,
    None => return, // Skip this function call if deserialization failed
}};\n"
                            )
                        }
                    },
                    Type::Custom(tn) => {
                        // Strip 'mut' if present in type name (e.g., "mut FurElement" -> "FurElement")
                        let clean_type = if tn.starts_with("mut ") {
                            tn.strip_prefix("mut ").unwrap_or(tn)
                        } else {
                            tn
                        };
                        
                        // Check for internal types that don't implement Deserialize
                        // Handle both short names (FurElement) and full paths (perro_core::nodes::ui::fur_ast::FurElement)
                        if clean_type.contains("FurElement") || clean_type.contains("FurNode") {
                            // For internal types that can't be deserialized, we can't call this function from scripts
                            format!(
                                "return; // Cannot deserialize internal type {clean_type} - this method should not be called from scripts\n"
                            )
                        } else {
                            // For custom types without Default, we need to handle None case
                            // Try to deserialize, and if it fails, return early
                            format!(
                                "let {param_name}_opt = params.get({actual_param_idx})
                            .and_then(|v| serde_json::from_value::<{clean_type}>(v.clone()).ok());
let {param_name} = match {param_name}_opt {{
    Some(val) => val,
    None => return, // Skip this function call if deserialization failed
}};\n"
                            )
                        }
                    },
                    Type::Node(_) => {
                        // Handle Type::Node variant - nodes are just UUIDs
                        format!(
                            "let {param_name} = params.get({actual_param_idx})
                            .and_then(|v| v.as_str())
                            .and_then(|s| Uuid::parse_str(s).ok())
                            .unwrap_or_default();\n"
                        )
                    },
                    Type::EngineStruct(EngineStructKind::Texture) => {
                        // Handle Texture - textures are just UUIDs
                        format!(
                            "let {param_name} = params.get({actual_param_idx})
                            .and_then(|v| v.as_str())
                            .and_then(|s| Uuid::parse_str(s).ok())
                            .unwrap_or_default();\n"
                        )
                    },
                    Type::Uuid => {
                        // Handle Uuid - parse from string
                        format!(
                            "let {param_name} = params.get({actual_param_idx})
                            .and_then(|v| v.as_str())
                            .and_then(|s| Uuid::parse_str(s).ok())
                            .unwrap_or_default();\n"
                        )
                    },
                    Type::Option(boxed) if matches!(boxed.as_ref(), Type::Uuid) => {
                        // Handle Option<Uuid> - parse from string, return None if parsing fails
                        format!(
                            "let {param_name} = params.get({actual_param_idx})
                            .and_then(|v| v.as_str())
                            .and_then(|s| Uuid::parse_str(s).ok());\n"
                        )
                    },
                    _ => format!("let {param_name} = Default::default();\n"),
                };
                param_parsing.push_str(&parse_code);
                actual_param_idx += 1;
            }

            // Build param list, inserting 'api' where ScriptApi parameters should be
            // Preserve original parameter order
            let mut param_names: Vec<String> = Vec::new();
            for param in func.params.iter() {
                if matches!(param.typ, Type::Custom(ref tn) if tn == "ScriptApi") {
                    // Insert 'api' for ScriptApi parameters
                    // If original was &ScriptApi (not &mut), we need to use &*api
                    // But actually, &mut ScriptApi can be coerced to &ScriptApi, so just use api
                    param_names.push("api".to_string());
                } else {
                    // Use original parameter name for Rust scripts, renamed for others
                    let param_name = if is_rust_script {
                        param.name.clone()
                    } else {
                        rename_variable(&param.name, Some(&param.typ))
                    };
                    
                    // For Rust scripts, if the parameter type is a reference (&str, &Path, etc.),
                    // we need to add & prefix when calling the function
                    let param_expr = if is_rust_script {
                        match &param.typ {
                            Type::StrRef => {
                                // &str - we parsed as String, need to borrow it
                                format!("&{}", param_name)
                            },
                            Type::Custom(tn) if tn == "&Path" || tn == "Path" || (tn.contains("Path") && !tn.starts_with('&')) => {
                                // Path types - we created __path_buf_ variable and used .as_path()
                                // {param_name} is already a &Path reference, so use it directly
                                param_name.clone()
                            },
                            Type::Custom(tn) if tn.starts_with('&') => {
                                // Reference type like &Manifest - we created __owned_ variable
                                if tn == "&str" || tn == "str" {
                                    format!("&{}", param_name)
                                } else {
                                    // For other reference types, we created {param_name} as a reference from the match
                                    // So use it directly (it's already a reference)
                                    param_name.clone()
                                }
                            },
                            _ => param_name,
                        }
                    } else {
                        param_name
                    };
                    
                    param_names.push(param_expr);
                }
            }
            param_list = param_names.join(", ");
        }
        
        // For transpiled scripts (non-Rust), the api parameter is always added to the function signature
        // in function.rs, so we must always pass it in the call, even if it's not in func.params
        if !is_rust_script {
            if !param_list.is_empty() {
                param_list.push_str(", ");
            }
            param_list.push_str("api");
        }

        write!(
            dispatch_entries,
            "        {func_id}u64 => | script: &mut {struct_name}, params: &[Value], api: &mut ScriptApi<'_>| {{
{param_parsing}            script.{renamed_func_name}({param_list});
        }},\n"
        )
        .unwrap();
    }

    // MEMBER_TO_ATTRIBUTES_MAP and ATTRIBUTE_TO_MEMBERS_MAP are generated once at the top in to_rust(),
    // not here in the boilerplate to avoid duplicates

    //----------------------------------------------------
    // FINAL OUTPUT
    //----------------------------------------------------
    write!(
        out,
        r#"
impl ScriptObject for {struct_name} {{
    fn set_id(&mut self, id: Uuid) {{
        self.id = id;
    }}

    fn get_id(&self) -> Uuid {{
        self.id
    }}

    fn get_var(&self, var_id: u64) -> Option<Value> {{
        VAR_GET_TABLE.get(&var_id).and_then(|f| f(self))
    }}

    fn set_var(&mut self, var_id: u64, val: Value) -> Option<()> {{
        VAR_SET_TABLE.get(&var_id).and_then(|f| f(self, val))
    }}

    fn apply_exposed(&mut self, hashmap: &HashMap<u64, Value>) {{
        for (var_id, val) in hashmap.iter() {{
            if let Some(f) = VAR_APPLY_TABLE.get(var_id) {{
                f(self, val);
            }}
        }}
    }}

    fn call_function(
        &mut self,
        id: u64,
        api: &mut ScriptApi<'_>,
        params: &[Value],
    ) {{
        if let Some(f) = DISPATCH_TABLE.get(&id) {{
            f(self, params, api);
        }}
    }}

    // Attributes

    fn attributes_of(&self, member: &str) -> Vec<String> {{
        MEMBER_TO_ATTRIBUTES_MAP
            .get(member)
            .map(|attrs| attrs.iter().map(|s| s.to_string()).collect())
            .unwrap_or_default()
    }}

    fn members_with(&self, attribute: &str) -> Vec<String> {{
        ATTRIBUTE_TO_MEMBERS_MAP
            .get(attribute)
            .map(|members| members.iter().map(|s| s.to_string()).collect())
            .unwrap_or_default()
    }}

    fn has_attribute(&self, member: &str, attribute: &str) -> bool {{
        MEMBER_TO_ATTRIBUTES_MAP
            .get(member)
            .map(|attrs| attrs.iter().any(|a| *a == attribute))
            .unwrap_or(false)
    }}
    
    fn script_flags(&self) -> ScriptFlags {{
        ScriptFlags::new({flags_value})
    }}
}}

// =========================== Static PHF Dispatch Tables ===========================

static VAR_GET_TABLE: phf::Map<u64, fn(&{struct_name}) -> Option<Value>> =
    phf::phf_map! {{
{get_entries}
    }};

static VAR_SET_TABLE: phf::Map<u64, fn(&mut {struct_name}, Value) -> Option<()>> =
    phf::phf_map! {{
{set_entries}
    }};

static VAR_APPLY_TABLE: phf::Map<u64, fn(&mut {struct_name}, &Value)> =
    phf::phf_map! {{
{apply_entries}
    }};

static DISPATCH_TABLE: phf::Map<
    u64,
    fn(&mut {struct_name}, &[Value], &mut ScriptApi<'_>),
> = phf::phf_map! {{
{dispatch_entries}
    }};
"#,
        struct_name = struct_name,
        get_entries = get_entries,
        set_entries = set_entries,
        apply_entries = apply_entries,
        dispatch_entries = dispatch_entries,
        flags_value = flags_value,
    )
    .unwrap();

    out
}
