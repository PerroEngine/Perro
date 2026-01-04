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

        let func_name = &func.name;
        let func_id = string_to_u64(func_name);
        let renamed_func_name = rename_function(func_name);

        let mut param_parsing = String::new();
        let mut param_list = String::new();

        if !func.params.is_empty() {
            for (i, param) in func.params.iter().enumerate() {
                // Rename parameter: node types get _id suffix, others keep original name
                let param_name = rename_variable(&param.name, Some(&param.typ));
                let parse_code = match &param.typ {
                    Type::String => format!(
                        "let {param_name} = params.get({i})
                            .and_then(|v| v.as_str())
                            .map(|s| s.to_string())
                            .unwrap_or_default();\n"
                    ),
                    Type::Number(NumberKind::Signed(w)) => format!(
                        "let {param_name} = params.get({i})
                            .and_then(|v| v.as_i64().or_else(|| v.as_f64().map(|f| f as i64)))
                            .unwrap_or_default() as i{w};\n"
                    ),
                    Type::Number(NumberKind::Unsigned(w)) => format!(
                        "let {param_name} = params.get({i})
                            .and_then(|v| v.as_u64().or_else(|| v.as_f64().map(|f| f as u64)))
                            .unwrap_or_default() as u{w};\n"
                    ),
                    Type::Number(NumberKind::Float(32)) => format!(
                        "let {param_name} = params.get({i})
                            .and_then(|v| v.as_f64().or_else(|| v.as_i64().map(|i| i as f64)))
                            .unwrap_or_default() as f32;\n"
                    ),
                    Type::Number(NumberKind::Float(64)) => format!(
                        "let {param_name} = params.get({i})
                            .and_then(|v| v.as_f64().or_else(|| v.as_i64().map(|i| i as f64)))
                            .unwrap_or_default();\n"
                    ),
                    Type::Bool => format!(
                        "let {param_name} = params.get({i})
                            .and_then(|v| v.as_bool())
                            .unwrap_or_default();\n"
                    ),
                    Type::Custom(tn) if tn == "Signal" => format!(
                        "let {param_name} = params.get({i})
                            .and_then(|v| v.as_u64().or_else(|| v.as_f64().map(|f| f as u64)))
                            .unwrap_or_default() as u64;\n"
                    ),
                    Type::Custom(tn) if is_node_type(tn) => {
                        // For node types, parse UUID from string (nodes are just UUIDs)
                        format!(
                            "let {param_name} = params.get({i})
                            .and_then(|v| v.as_str())
                            .and_then(|s| Uuid::parse_str(s).ok())
                            .unwrap_or_default();\n"
                        )
                    },
                    Type::Custom(tn) => format!(
                        "let {param_name} = params.get({i})
                            .and_then(|v| serde_json::from_value::<{tn}>(v.clone()).ok())
                            .unwrap_or_default();\n"
                    ),
                    Type::Node(_) => {
                        // Handle Type::Node variant - nodes are just UUIDs
                        format!(
                            "let {param_name} = params.get({i})
                            .and_then(|v| v.as_str())
                            .and_then(|s| Uuid::parse_str(s).ok())
                            .unwrap_or_default();\n"
                        )
                    },
                    Type::EngineStruct(EngineStructKind::Texture) => {
                        // Handle Texture - textures are just UUIDs
                        format!(
                            "let {param_name} = params.get({i})
                            .and_then(|v| v.as_str())
                            .and_then(|s| Uuid::parse_str(s).ok())
                            .unwrap_or_default();\n"
                        )
                    },
                    Type::Uuid => {
                        // Handle Uuid - parse from string
                        format!(
                            "let {param_name} = params.get({i})
                            .and_then(|v| v.as_str())
                            .and_then(|s| Uuid::parse_str(s).ok())
                            .unwrap_or_default();\n"
                        )
                    },
                    Type::Option(boxed) if matches!(boxed.as_ref(), Type::Uuid) => {
                        // Handle Option<Uuid> - parse from string, return None if parsing fails
                        format!(
                            "let {param_name} = params.get({i})
                            .and_then(|v| v.as_str())
                            .and_then(|s| Uuid::parse_str(s).ok());\n"
                        )
                    },
                    _ => format!("let {param_name} = Default::default();\n"),
                };
                param_parsing.push_str(&parse_code);
            }

            param_list = func
                .params
                .iter()
                .map(|p| rename_variable(&p.name, Some(&p.typ)))
                .collect::<Vec<_>>()
                .join(", ");
            param_list.push_str(", ");
        }

        write!(
            dispatch_entries,
            "        {func_id}u64 => | script: &mut {struct_name}, params: &[Value], api: &mut ScriptApi<'_>| {{
{param_parsing}            script.{renamed_func_name}({param_list}api);
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
