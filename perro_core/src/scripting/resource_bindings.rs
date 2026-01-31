// ===========================================================
// Resource API Bindings - Types/resources that can be instantiated
// These are different from Module API Bindings (global functions)
// ===========================================================

use crate::{
    api_bindings::{ModuleCodegen, ModuleTypes}, // Import traits from api_bindings
    ast::*,
    engine_structs::EngineStruct,
    prelude::string_to_u64,
    resource_modules::*, // Import resource API enums
    scripting::ast::{ContainerKind, NumberKind},
};

// ===========================================================
// Signal API Implementations
// ===========================================================

impl ModuleCodegen for SignalResource {
    fn to_rust_prepared(
        &self,
        args: &[Expr],
        args_strs: &[String],
        script: &Script,
        _needs_self: bool,
        current_func: Option<&Function>,
    ) -> String {
        fn prehash_if_literal(arg: &str) -> String {
            let trimmed = arg.trim();
            if trimmed.starts_with('"') && trimmed.ends_with('"') && trimmed.len() > 1 {
                let inner = &trimmed[1..trimmed.len() - 1];
                let id = string_to_u64(inner);
                return format!("{id}u64");
            }
            if trimmed.starts_with("String::from(") && trimmed.ends_with(')') {
                let inner_section = &trimmed["String::from(".len()..trimmed.len() - 1].trim();
                if inner_section.starts_with('"') && inner_section.ends_with('"') {
                    let inner = &inner_section[1..inner_section.len() - 1];
                    let id = string_to_u64(inner);
                    return format!("{id}u64");
                }
            }
            format!("string_to_u64(&{trimmed})")
        }

        fn strip_string_from(arg: &str) -> String {
            let trimmed = arg.trim();
            if trimmed.starts_with("String::from(") && trimmed.ends_with(')') {
                let inner_section = &trimmed["String::from(".len()..trimmed.len() - 1].trim();
                if inner_section.starts_with('"') && inner_section.ends_with('"') {
                    return inner_section[1..inner_section.len() - 1].to_string();
                }
            }
            if trimmed.starts_with('"') && trimmed.ends_with('"') {
                return trimmed[1..trimmed.len() - 1].to_string();
            }
            trimmed.to_string()
        }

        match self {
            SignalResource::New => {
                let signal = args_strs.get(0).cloned().unwrap_or_else(|| "\"\"".into());
                prehash_if_literal(&signal)
            }
            SignalResource::Connect | SignalResource::Emit | SignalResource::EmitDeferred => {
                // First arg is the signal. Pass through if already typed as signal/u64 or if it's
                // not a string literal (variable/expr is already u64). Only prehash string literals.
                let arg_expr = args.get(0).unwrap();
                let arg_type = script.infer_expr_type(arg_expr, current_func);
                let first = args_strs[0].trim();

                let signal = match arg_type {
                    Some(Type::Number(NumberKind::Unsigned(64))) | Some(Type::Signal) => {
                        args_strs[0].clone()
                    }
                    _ => {
                        if first.starts_with('"')
                            || (first.starts_with("String::from(") && first.ends_with(')'))
                        {
                            prehash_if_literal(&args_strs[0])
                        } else {
                            // Variable or expression — already a u64/signal, use as-is (same as Array/Map/Texture).
                            args_strs[0].clone()
                        }
                    }
                };

                match self {
                    SignalResource::Connect => {
                        // New format: Signal.connect(signal, function_reference)
                        // function_reference can be:
                        // - String literal (function name) -> use self.id
                        // - MemberAccess (bob.function) -> use bob_id (node variable is already bob_id)
                        // - Ident (function) -> use self.id
                        let func_expr = args.get(1).unwrap();
                        let (node_code, func_name) = match func_expr {
                            Expr::Literal(Literal::String(func_name)) => {
                                // String literal -> function on self
                                ("self".to_string(), func_name.clone())
                            }
                            Expr::MemberAccess(base, field) => {
                                // MemberAccess like bob.function -> node_code is already bob_id
                                let node_code =
                                    base.to_rust(false, script, None, current_func, None);
                                (node_code, field.clone())
                            }
                            Expr::Ident(func_name) => {
                                // Just an identifier -> function on self
                                ("self".to_string(), func_name.clone())
                            }
                            _ => {
                                // Fallback: treat as string literal
                                let func = strip_string_from(args_strs.get(1).unwrap());
                                ("self".to_string(), func)
                            }
                        };

                        let func_id = string_to_u64(&func_name);
                        // If node_code is "self", use self.id. Otherwise, node_code is already the node ID variable (bob_id or self.bob_id)
                        let node_id = if node_code == "self" {
                            "self.id".to_string()
                        } else {
                            node_code
                        };
                        format!("api.connect_signal_id({signal}, {}, {func_id}u64)", node_id)
                    }
                    SignalResource::Emit => {
                        if args_strs.len() > 1 {
                            let params: Vec<String> = args_strs[1..]
                                .iter()
                                .map(|a| format!("json!({a})"))
                                .collect();
                            // Use array literal - converted to slice automatically (zero-cost)
                            format!("api.emit_signal_id({signal}, &[{}])", params.join(", "))
                        } else {
                            // Empty array literal - zero allocation
                            format!("api.emit_signal_id({signal}, &[])")
                        }
                    }
                    SignalResource::EmitDeferred => {
                        if args_strs.len() > 1 {
                            let params: Vec<String> = args_strs[1..]
                                .iter()
                                .map(|a| format!("json!({a})"))
                                .collect();
                            // Use array literal - converted to slice automatically (zero-cost)
                            format!(
                                "api.emit_signal_id_deferred({signal}, &[{}])",
                                params.join(", ")
                            )
                        } else {
                            // Empty array literal - zero allocation
                            format!("api.emit_signal_id_deferred({signal}, &[])")
                        }
                    }
                    _ => unreachable!("SignalResource variant covered above"),
                }
            }
        }
    }
}

impl ModuleTypes for SignalResource {
    fn return_type(&self) -> Option<Type> {
        match self {
            SignalResource::New => Some(Type::Signal),
            _ => Some(Type::Void),
        }
    }

    fn param_types(&self) -> Option<Vec<Type>> {
        match self {
            SignalResource::New => Some(vec![Type::String]),
            SignalResource::Emit | SignalResource::EmitDeferred => {
                Some(vec![Type::Signal, Type::Object])
            }
            SignalResource::Connect => Some(vec![Type::Signal]),
        }
    }

    /// Script-side parameter names (what PUP users see)
    fn param_names(&self) -> Option<Vec<&'static str>> {
        match self {
            SignalResource::New => Some(vec!["name"]),
            SignalResource::Emit | SignalResource::EmitDeferred => Some(vec!["signal", "data"]),
            SignalResource::Connect => Some(vec!["signal"]),
        }
    }
}

// ===========================================================
// ArrayOp API Implementations
// ===========================================================

impl ModuleCodegen for ArrayResource {
    fn to_rust_prepared(
        &self,
        args: &[Expr],
        args_strs: &[String],
        script: &Script,
        needs_self: bool,
        current_func: Option<&Function>,
    ) -> String {
        match self {
            ArrayResource::Push => {
                // args[0] is the array expression, args[1] is the value to push
                let array_expr = &args[0];
                let value_expr = &args[1]; // The raw AST expression for the value

                // Infer the type of the array itself to get its inner type
                let array_type = script.infer_expr_type(array_expr, current_func);

                let inner_type =
                    if let Some(Type::Container(ContainerKind::Array, ref inner_types)) = array_type {
                        inner_types.get(0).cloned().unwrap_or(Type::Object)
                    } else {
                        Type::Object // Fallback if array type couldn't be inferred
                    };

                let mut value_code =
                    value_expr.to_rust(needs_self, script, Some(&inner_type), current_func, None);

                // If the value_code itself still indicates a JSON value AND the target is not Type::Object,
                // we should attempt to deserialize it.
                if value_code.starts_with("json!(") && inner_type != Type::Object {
                    // Strip the json! and try to deserialize if possible
                    let raw_json_content = value_code
                        .strip_prefix("json!(")
                        .and_then(|s| s.strip_suffix(")"))
                        .unwrap_or(&value_code);
                    value_code = format!(
                        "serde_json::from_value::<{}>({}).unwrap_or_default()",
                        inner_type.to_rust_type(),
                        raw_json_content
                    );
                } else if matches!(inner_type, Type::Object | Type::Any)
                    || matches!(inner_type, Type::Custom(ref n) if n == "Value")
                {
                    // For dynamic arrays (Vec<Value>), wrap the value in json!() so it becomes Value
                    if !value_code.starts_with("json!(") {
                        value_code = format!("json!({})", value_code);
                    }
                } else if array_type.is_none() && !value_code.starts_with("json!(") {
                    // Array type unknown (e.g. var arr = Array.new()) - assume Vec<Value> and wrap
                    value_code = format!("json!({})", value_code);
                } else if value_code.starts_with("json!(") {
                    // Already json! and target is typed - no change
                } else {
                    // Perform implicit cast if needed and not already handled
                    if let Some(actual_value_type) =
                        script.infer_expr_type(value_expr, current_func)
                    {
                        if actual_value_type.can_implicitly_convert_to(&inner_type)
                            && actual_value_type != inner_type
                        {
                            value_code = script.generate_implicit_cast_for_expr(
                                &value_code,
                                &actual_value_type,
                                &inner_type,
                            );
                        }
                    }
                    // Handle cloning if the type requires it (e.g., custom structs)
                    if inner_type.requires_clone()
                        && !value_code.contains(".clone()")
                        && !value_code.starts_with("String::from")
                    {
                        // Prevent double cloning if value_code already implies it (e.g., "new Player(...)")
                        let produces_owned_value = matches!(
                            value_expr,
                            Expr::StructNew(..) | Expr::Call(..) | Expr::ContainerLiteral(..)
                        );
                        if !produces_owned_value {
                            value_code = format!("{}.clone()", value_code);
                        }
                    }
                }

                // Final safety: primitives pushed to Vec<Value> must be wrapped
                let is_dynamic_element = matches!(inner_type, Type::Object | Type::Any)
                    || matches!(inner_type, Type::Custom(ref n) if n == "Value")
                    || array_type.is_none();
                if !value_code.starts_with("json!(")
                    && (value_code.ends_with("i32")
                        || value_code.ends_with("i64")
                        || value_code.ends_with("f32")
                        || value_code.ends_with("f64")
                        || value_code.starts_with("String::from(")
                        || (value_code.starts_with('"') && value_code.len() > 1))
                    && is_dynamic_element
                {
                    value_code = format!("json!({})", value_code);
                }

                format!("{}.push({})", args_strs[0], value_code)
            }
            ArrayResource::Pop => {
                format!("{}.pop()", args_strs[0])
            }
            ArrayResource::Len => {
                // Vec::len() returns usize; script type is u32, so convert
                format!("{}.len().try_into().unwrap()", args_strs[0])
            }
            ArrayResource::Insert => {
                let array_expr = &args[0];
                let value_expr = &args[2]; // value is the third arg for insert
                let array_type = script.infer_expr_type(array_expr, current_func);
                let inner_type =
                    if let Some(Type::Container(ContainerKind::Array, inner_types)) = array_type {
                        inner_types.get(0).cloned().unwrap_or(Type::Object)
                    } else {
                        Type::Object
                    };

                let mut value_code =
                    value_expr.to_rust(needs_self, script, Some(&inner_type), current_func, None);

                // Apply the same logic as Push for value_code conversion/cloning
                if value_code.starts_with("json!(") && inner_type != Type::Object {
                    let raw_json_content = value_code
                        .strip_prefix("json!(")
                        .and_then(|s| s.strip_suffix(")"))
                        .unwrap_or(&value_code);
                    value_code = format!(
                        "serde_json::from_value::<{}>({}).unwrap_or_default()",
                        inner_type.to_rust_type(),
                        raw_json_content
                    );
                } else if !value_code.starts_with("json!(") {
                    // Only do this if it's not already a json! and needs conversion
                    if let Some(actual_value_type) =
                        script.infer_expr_type(value_expr, current_func)
                    {
                        if actual_value_type.can_implicitly_convert_to(&inner_type)
                            && actual_value_type != inner_type
                        {
                            value_code = script.generate_implicit_cast_for_expr(
                                &value_code,
                                &actual_value_type,
                                &inner_type,
                            );
                        }
                    }
                    if inner_type.requires_clone()
                        && !value_code.contains(".clone()")
                        && !value_code.starts_with("String::from")
                    {
                        let produces_owned_value = matches!(
                            value_expr,
                            Expr::StructNew(..) | Expr::Call(..) | Expr::ContainerLiteral(..)
                        );
                        if !produces_owned_value {
                            value_code = format!("{}.clone()", value_code);
                        }
                    }
                }

                format!(
                    "{}.insert({} as usize, {})",
                    args_strs[0], args_strs[1], value_code
                )
            }
            ArrayResource::Remove => {
                format!("{}.remove({} as usize)", args_strs[0], args_strs[1])
            }
            ArrayResource::New => {
                format!("Vec::new()")
            }
        }
    }
}

impl ModuleTypes for ArrayResource {
    fn return_type(&self) -> Option<Type> {
        match self {
            ArrayResource::Push => Some(Type::Void),
            ArrayResource::Pop => Some(Type::Object),
            ArrayResource::Insert => Some(Type::Void),
            ArrayResource::Remove => Some(Type::Object),
            ArrayResource::Len => Some(Type::Number(NumberKind::Unsigned(32))),
            ArrayResource::New => Some(Type::Container(ContainerKind::Array, vec![Type::Object])),
        }
    }

    fn param_types(&self) -> Option<Vec<Type>> {
        use ContainerKind::*;
        use NumberKind::*;

        match self {
            ArrayResource::Push => Some(vec![
                Type::Container(Array, vec![Type::Object]),
                Type::Object, // any value; just "Value"
            ]),
            ArrayResource::Insert => Some(vec![
                Type::Container(Array, vec![Type::Object]),
                Type::Number(Unsigned(32)), // index
                Type::Object,               // value
            ]),
            ArrayResource::Remove => Some(vec![
                Type::Container(Array, vec![Type::Object]),
                Type::Number(Unsigned(32)), // index expected
            ]),
            ArrayResource::Len | ArrayResource::Pop | ArrayResource::New => None,
        }
    }

    /// Script-side parameter names (what PUP users see)
    fn param_names(&self) -> Option<Vec<&'static str>> {
        match self {
            ArrayResource::Push => Some(vec!["array", "value"]),
            ArrayResource::Insert => Some(vec!["array", "index", "value"]),
            ArrayResource::Remove => Some(vec!["array", "index"]),
            ArrayResource::Len | ArrayResource::Pop | ArrayResource::New => None,
        }
    }
}

// ===========================================================
// Map API Implementations
// ===========================================================

impl ModuleCodegen for MapResource {
    fn to_rust_prepared(
        &self,
        args: &[Expr],
        args_strs: &[String],
        script: &Script,
        needs_self: bool,
        current_func: Option<&Function>,
    ) -> String {
        match self {
            // args: [map, key (string), value]
            MapResource::Insert => {
                let key_type = script.infer_map_key_type(&args[0], current_func);
                let val_type = script.infer_map_value_type(&args[0], current_func);
                let key_code =
                    args[1].to_rust(needs_self, script, key_type.as_ref(), current_func, None);
                let mut val_code =
                    args[2].to_rust(needs_self, script, val_type.as_ref(), current_func, None);

                // For dynamic maps (HashMap<String, Value>), wrap the value in json!()
                let is_dynamic_val = matches!(val_type.as_ref(), Some(Type::Object | Type::Any))
                    || matches!(val_type.as_ref(), Some(Type::Custom(n)) if n == "Value")
                    || val_type.is_none();
                if is_dynamic_val && !val_code.starts_with("json!(") {
                    val_code = format!("json!({})", val_code);
                }
                // Final safety: primitive value for HashMap<String, Value> must be wrapped
                if !val_code.starts_with("json!(")
                    && (val_code.ends_with("i32")
                        || val_code.ends_with("i64")
                        || val_code.ends_with("f32")
                        || val_code.ends_with("f64")
                        || val_code.starts_with("String::from(")
                        || (val_code.starts_with('"') && val_code.len() > 1))
                    && is_dynamic_val
                {
                    val_code = format!("json!({})", val_code);
                }

                format!("{}.insert({}, {})", args_strs[0], key_code, val_code)
            }

            // args: [map, key]
            MapResource::Remove => {
                let key_type = script.infer_map_key_type(&args[0], current_func);
                let key_code =
                    args[1].to_rust(needs_self, script, key_type.as_ref(), current_func, None);
                let use_as_str = matches!(key_type.as_ref(), Some(Type::String))
                    && !key_code.contains("to_u8()")
                    && !key_code.contains("to_u16()")
                    && !key_code.contains("to_u32()")
                    && !key_code.contains("to_u64()")
                    && !key_code.contains("to_i8()")
                    && !key_code.contains("to_i16()")
                    && !key_code.contains("to_i32()")
                    && !key_code.contains("to_i64()");
                if use_as_str {
                    format!("{}.remove({}.as_str())", args_strs[0], key_code)
                } else {
                    format!("{}.remove(&{})", args_strs[0], key_code)
                }
            }

            // args: [map, key]
            MapResource::Get => {
                // 1. Infer key type from map
                let key_type = script.infer_map_key_type(&args[0], current_func);
                // 2. Render the key argument with the right type hint
                let key_code =
                    args[1].to_rust(needs_self, script, key_type.as_ref(), current_func, None);

                // Only use .as_str() for HashMap<String, V>; for other key types use &key
                let use_as_str = matches!(key_type.as_ref(), Some(Type::String))
                    && !key_code.contains("to_u8()")
                    && !key_code.contains("to_u16()")
                    && !key_code.contains("to_u32()")
                    && !key_code.contains("to_u64()")
                    && !key_code.contains("to_i8()")
                    && !key_code.contains("to_i16()")
                    && !key_code.contains("to_i32()")
                    && !key_code.contains("to_i64()");
                if use_as_str {
                    format!(
                        "{}.get({}.as_str()).cloned().unwrap_or_default()",
                        args_strs[0], key_code
                    )
                } else {
                    format!(
                        "{}.get(&{}).cloned().unwrap_or_default()",
                        args_strs[0], key_code
                    )
                }
            }

            // args: [map, key]
            MapResource::Contains => {
                let key_type = script.infer_map_key_type(&args[0], current_func);
                let key_code =
                    args[1].to_rust(needs_self, script, key_type.as_ref(), current_func, None);
                let use_as_str = matches!(key_type.as_ref(), Some(Type::String))
                    && !key_code.contains("to_u8()")
                    && !key_code.contains("to_u16()")
                    && !key_code.contains("to_u32()")
                    && !key_code.contains("to_u64()")
                    && !key_code.contains("to_i8()")
                    && !key_code.contains("to_i16()")
                    && !key_code.contains("to_i32()")
                    && !key_code.contains("to_i64()");
                if use_as_str {
                    format!("{}.contains_key({}.as_str())", args_strs[0], key_code)
                } else {
                    format!("{}.contains_key(&{})", args_strs[0], key_code)
                }
            }

            // args: [map]
            MapResource::Len => {
                // HashMap::len() returns usize; script type is u32, so convert
                format!("{}.len().try_into().unwrap()", args_strs[0])
            }

            // args: [map]
            MapResource::Clear => {
                format!("{}.clear()", args_strs[0])
            }

            // no args
            MapResource::New => "HashMap::new()".into(),
        }
    }
}

impl ModuleTypes for MapResource {
    fn return_type(&self) -> Option<Type> {
        match self {
            MapResource::Insert | MapResource::Clear => Some(Type::Void),
            MapResource::Remove | MapResource::Get => Some(Type::Object),
            MapResource::Contains => Some(Type::Bool),
            MapResource::Len => Some(Type::Number(NumberKind::Unsigned(32))),
            MapResource::New => Some(Type::Container(
                ContainerKind::Map,
                vec![Type::String, Type::Object],
            )),
        }
    }

    fn param_types(&self) -> Option<Vec<Type>> {
        match self {
            MapResource::Insert => Some(vec![
                Type::Container(ContainerKind::Map, vec![Type::String, Type::Object]),
                Type::String,
                Type::Object,
            ]),
            MapResource::Remove | MapResource::Get => Some(vec![
                Type::Container(ContainerKind::Map, vec![Type::String, Type::Object]),
                Type::String,
            ]),
            MapResource::Contains => Some(vec![
                Type::Container(ContainerKind::Map, vec![Type::String, Type::Object]),
                Type::String,
            ]),
            _ => None,
        }
    }

    /// Script-side parameter names (what PUP users see)
    fn param_names(&self) -> Option<Vec<&'static str>> {
        match self {
            MapResource::Insert => Some(vec!["map", "key", "value"]),
            MapResource::Remove | MapResource::Get => Some(vec!["map", "key"]),
            MapResource::Contains => Some(vec!["map", "key"]),
            _ => None,
        }
    }
}

// ===========================================================
// Texture API Implementations
// ===========================================================

impl ModuleCodegen for TextureResource {
    fn to_rust_prepared(
        &self,
        _args: &[Expr],
        args_strs: &[String],
        _script: &Script,
        _needs_self: bool,
        _current_func: Option<&Function>,
    ) -> String {
        match self {
            TextureResource::Load => {
                let arg = args_strs.get(0).cloned().unwrap_or_else(|| "\"\"".into());
                // Handle string parameters: literals stay as &str, variables need & prefix
                let arg_str = if arg.starts_with('"') && arg.ends_with('"') {
                    // String literal - use directly as &str
                    arg
                } else if arg.starts_with("String::from(") && arg.ends_with(')') {
                    // Extract string literal from String::from("...")
                    let inner = &arg["String::from(".len()..arg.len() - 1].trim();
                    if inner.starts_with('"') && inner.ends_with('"') {
                        // Use the string literal directly as &str
                        inner.to_string()
                    } else {
                        // Fallback: borrow the String
                        format!("&{}", arg)
                    }
                } else {
                    // Variable or complex expression - borrow it
                    format!("&{}", arg)
                };

                format!("api.Texture.load({})", arg_str)
            }
            TextureResource::Preload => {
                let arg = args_strs.get(0).cloned().unwrap_or_else(|| "\"\"".into());
                let arg_str = if arg.starts_with('"') && arg.ends_with('"') {
                    arg
                } else if arg.starts_with("String::from(") && arg.ends_with(')') {
                    let inner = &arg["String::from(".len()..arg.len() - 1].trim();
                    if inner.starts_with('"') && inner.ends_with('"') {
                        inner.to_string()
                    } else {
                        format!("&{}", arg)
                    }
                } else {
                    format!("&{}", arg)
                };
                format!("api.Texture.preload({})", arg_str)
            }
            TextureResource::Remove => {
                let arg = args_strs
                    .get(0)
                    .cloned()
                    .unwrap_or_else(|| "TextureID::nil()".into());
                format!("api.Texture.remove({})", arg)
            }
            TextureResource::CreateFromBytes => {
                let bytes = args_strs.get(0).cloned().unwrap_or_else(|| "vec![]".into());
                let width = args_strs.get(1).cloned().unwrap_or_else(|| "0".into());
                let height = args_strs.get(2).cloned().unwrap_or_else(|| "0".into());
                format!(
                    "api.Texture.create_from_bytes({}, {}, {})",
                    bytes, width, height
                )
            }
            TextureResource::GetWidth => {
                let arg = args_strs
                    .get(0)
                    .cloned()
                    .unwrap_or_else(|| "NodeID::nil()".into());
                format!("api.Texture.get_width({})", arg)
            }
            TextureResource::GetHeight => {
                let arg = args_strs
                    .get(0)
                    .cloned()
                    .unwrap_or_else(|| "NodeID::nil()".into());
                format!("api.Texture.get_height({})", arg)
            }
            TextureResource::GetSize => {
                let arg = args_strs
                    .get(0)
                    .cloned()
                    .unwrap_or_else(|| "NodeID::nil()".into());
                format!("api.Texture.get_size({})", arg)
            }
        }
    }
}

impl ModuleTypes for TextureResource {
    fn return_type(&self) -> Option<Type> {
        match self {
            TextureResource::Load | TextureResource::Preload => {
                Some(Type::EngineStruct(EngineStruct::Texture))
            }
            TextureResource::CreateFromBytes => Some(Type::EngineStruct(EngineStruct::Texture)),
            TextureResource::Remove => None, // void
            TextureResource::GetWidth | TextureResource::GetHeight => {
                Some(Type::Number(NumberKind::Unsigned(32)))
            }
            TextureResource::GetSize => Some(Type::EngineStruct(EngineStruct::Vector2)),
        }
    }

    fn param_types(&self) -> Option<Vec<Type>> {
        match self {
            TextureResource::Load | TextureResource::Preload => Some(vec![Type::String]),
            TextureResource::Remove => Some(vec![Type::EngineStruct(EngineStruct::Texture)]),
            TextureResource::CreateFromBytes => Some(vec![
                Type::Container(
                    ContainerKind::Array,
                    vec![Type::Number(NumberKind::Unsigned(8))],
                ),
                Type::Number(NumberKind::Unsigned(32)),
                Type::Number(NumberKind::Unsigned(32)),
            ]),
            TextureResource::GetWidth | TextureResource::GetHeight | TextureResource::GetSize => {
                Some(vec![Type::EngineStruct(EngineStruct::Texture)])
            }
        }
    }

    /// Script-side parameter names (what PUP users see)
    fn param_names(&self) -> Option<Vec<&'static str>> {
        match self {
            TextureResource::Load | TextureResource::Preload => Some(vec!["path"]),
            TextureResource::Remove => Some(vec!["texture"]),
            TextureResource::CreateFromBytes => Some(vec!["bytes", "width", "height"]),
            TextureResource::GetWidth | TextureResource::GetHeight | TextureResource::GetSize => {
                Some(vec!["texture"])
            }
        }
    }
}

// ===========================================================
// Shape2D API Implementations
// ===========================================================

impl ModuleCodegen for ShapeResource {
    fn to_rust_prepared(
        &self,
        _args: &[Expr],
        args_strs: &[String],
        _script: &Script,
        _needs_self: bool,
        _current_func: Option<&Function>,
    ) -> String {
        match self {
            ShapeResource::Rectangle => {
                let width = args_strs.get(0).cloned().unwrap_or_else(|| "0.0f32".into());
                let height = args_strs.get(1).cloned().unwrap_or_else(|| "0.0f32".into());
                // Ensure f32 suffix for float literals
                let width = if width.parse::<f32>().is_ok() && !width.contains('f') {
                    format!("{}f32", width)
                } else {
                    width
                };
                let height = if height.parse::<f32>().is_ok() && !height.contains('f') {
                    format!("{}f32", height)
                } else {
                    height
                };
                format!(
                    "Shape2D::Rectangle {{ width: {}, height: {} }}",
                    width, height
                )
            }
            ShapeResource::Circle => {
                let radius = args_strs.get(0).cloned().unwrap_or_else(|| "0.0f32".into());
                let radius = if radius.parse::<f32>().is_ok() && !radius.contains('f') {
                    format!("{}f32", radius)
                } else {
                    radius
                };
                format!("Shape2D::Circle {{ radius: {} }}", radius)
            }
            ShapeResource::Square => {
                let size = args_strs.get(0).cloned().unwrap_or_else(|| "0.0f32".into());
                let size = if size.parse::<f32>().is_ok() && !size.contains('f') {
                    format!("{}f32", size)
                } else {
                    size
                };
                format!("Shape2D::Square {{ size: {} }}", size)
            }
            ShapeResource::Triangle => {
                let base = args_strs.get(0).cloned().unwrap_or_else(|| "0.0f32".into());
                let height = args_strs.get(1).cloned().unwrap_or_else(|| "0.0f32".into());
                let base = if base.parse::<f32>().is_ok() && !base.contains('f') {
                    format!("{}f32", base)
                } else {
                    base
                };
                let height = if height.parse::<f32>().is_ok() && !height.contains('f') {
                    format!("{}f32", height)
                } else {
                    height
                };
                format!("Shape2D::Triangle {{ base: {}, height: {} }}", base, height)
            }
        }
    }
}

impl ModuleTypes for ShapeResource {
    fn return_type(&self) -> Option<Type> {
        // All Shape2D constructors return Shape2D enum
        Some(Type::EngineStruct(EngineStruct::Shape2D))
    }

    fn param_types(&self) -> Option<Vec<Type>> {
        use NumberKind::*;
        match self {
            ShapeResource::Rectangle => {
                Some(vec![Type::Number(Float(32)), Type::Number(Float(32))])
            }
            ShapeResource::Circle => Some(vec![Type::Number(Float(32))]),
            ShapeResource::Square => Some(vec![Type::Number(Float(32))]),
            ShapeResource::Triangle => Some(vec![Type::Number(Float(32)), Type::Number(Float(32))]),
        }
    }

    /// Script-side parameter names (what PUP users see)
    fn param_names(&self) -> Option<Vec<&'static str>> {
        match self {
            ShapeResource::Rectangle => Some(vec!["width", "height"]),
            ShapeResource::Circle => Some(vec!["radius"]),
            ShapeResource::Square => Some(vec!["size"]),
            ShapeResource::Triangle => Some(vec!["base", "height"]),
        }
    }
}

// ===========================================================
// Quaternion API Implementations
// ===========================================================

impl ModuleCodegen for QuaternionResource {
    fn to_rust_prepared(
        &self,
        args: &[Expr],
        args_strs: &[String],
        script: &Script,
        needs_self: bool,
        current_func: Option<&Function>,
    ) -> String {
        match self {
            QuaternionResource::Identity => "Quaternion::identity()".into(),
            QuaternionResource::FromEuler => {
                let e = args_strs.get(0).cloned().unwrap_or_else(|| "Vector3::ZERO".into());
                format!("Quaternion::from_euler_degrees({e}.x, {e}.y, {e}.z)")
            }
            QuaternionResource::FromEulerXYZ => {
                let pitch = args_strs.get(0).cloned().unwrap_or_else(|| "0.0".into());
                let yaw = args_strs.get(1).cloned().unwrap_or_else(|| "0.0".into());
                let roll = args_strs.get(2).cloned().unwrap_or_else(|| "0.0".into());
                format!("Quaternion::from_euler_degrees({pitch}, {yaw}, {roll})")
            }
            QuaternionResource::AsEuler => {
                // IMPORTANT: avoid implicit casts here.
                // If type inference is momentarily wrong, `args_strs[0]` might include an unwanted cast
                // (e.g. Vector3->Quaternion by interpreting x/y/z as degrees). Re-render the argument
                // directly from the AST with no expected type hint to preserve the real expression.
                let q_raw = args
                    .get(0)
                    .map(|e| e.to_rust(needs_self, script, None, current_func, None))
                    .filter(|s| !s.trim().is_empty())
                    .unwrap_or_else(|| {
                        args_strs
                            .get(0)
                            .cloned()
                            .unwrap_or_else(|| "Quaternion::identity()".into())
                    });

                // `Quaternion::as_euler()` returns a Vector3 in degrees.
                format!("({}).as_euler()", q_raw)
            }
            QuaternionResource::RotateX => {
                // For rotate helpers we can trust args_strs[0] (expected type is Quaternion).
                let q = args_strs
                    .get(0)
                    .cloned()
                    .unwrap_or_else(|| "Quaternion::identity()".into());
                let delta = args_strs.get(1).cloned().unwrap_or_else(|| "0.0".into());
                format!("({}).rotate_x({})", q, delta)
            }
            QuaternionResource::RotateY => {
                let q = args_strs
                    .get(0)
                    .cloned()
                    .unwrap_or_else(|| "Quaternion::identity()".into());
                let delta = args_strs.get(1).cloned().unwrap_or_else(|| "0.0".into());
                format!("({}).rotate_y({})", q, delta)
            }
            QuaternionResource::RotateZ => {
                let q = args_strs
                    .get(0)
                    .cloned()
                    .unwrap_or_else(|| "Quaternion::identity()".into());
                let delta = args_strs.get(1).cloned().unwrap_or_else(|| "0.0".into());
                format!("({}).rotate_z({})", q, delta)
            }
            QuaternionResource::RotateEulerXYZ => {
                let q = args_strs
                    .get(0)
                    .cloned()
                    .unwrap_or_else(|| "Quaternion::identity()".into());
                let dp = args_strs.get(1).cloned().unwrap_or_else(|| "0.0".into());
                let dy = args_strs.get(2).cloned().unwrap_or_else(|| "0.0".into());
                let dr = args_strs.get(3).cloned().unwrap_or_else(|| "0.0".into());
                format!("({}).rotate_euler_degrees({}, {}, {})", q, dp, dy, dr)
            }
        }
    }
}

impl ModuleTypes for QuaternionResource {
    fn return_type(&self) -> Option<Type> {
        match self {
            QuaternionResource::Identity
            | QuaternionResource::FromEuler
            | QuaternionResource::FromEulerXYZ
            | QuaternionResource::RotateX
            | QuaternionResource::RotateY
            | QuaternionResource::RotateZ
            | QuaternionResource::RotateEulerXYZ => {
                Some(Type::EngineStruct(EngineStruct::Quaternion))
            }
            QuaternionResource::AsEuler => Some(Type::EngineStruct(EngineStruct::Vector3)),
        }
    }

    fn param_types(&self) -> Option<Vec<Type>> {
        use NumberKind::*;
        match self {
            QuaternionResource::Identity => None,
            QuaternionResource::FromEuler => Some(vec![Type::EngineStruct(EngineStruct::Vector3)]),
            QuaternionResource::FromEulerXYZ => Some(vec![
                Type::Number(Float(32)),
                Type::Number(Float(32)),
                Type::Number(Float(32)),
            ]),
            QuaternionResource::AsEuler => Some(vec![Type::EngineStruct(EngineStruct::Quaternion)]),
            QuaternionResource::RotateX
            | QuaternionResource::RotateY
            | QuaternionResource::RotateZ => Some(vec![
                Type::EngineStruct(EngineStruct::Quaternion),
                Type::Number(Float(32)),
            ]),
            QuaternionResource::RotateEulerXYZ => Some(vec![
                Type::EngineStruct(EngineStruct::Quaternion),
                Type::Number(Float(32)),
                Type::Number(Float(32)),
                Type::Number(Float(32)),
            ]),
        }
    }

    fn param_names(&self) -> Option<Vec<&'static str>> {
        match self {
            QuaternionResource::Identity => None,
            QuaternionResource::FromEuler => Some(vec!["euler_deg"]),
            QuaternionResource::FromEulerXYZ => Some(vec!["pitch_deg", "yaw_deg", "roll_deg"]),
            QuaternionResource::AsEuler => Some(vec!["q"]),
            QuaternionResource::RotateX => Some(vec!["q", "delta_pitch_deg"]),
            QuaternionResource::RotateY => Some(vec!["q", "delta_yaw_deg"]),
            QuaternionResource::RotateZ => Some(vec!["q", "delta_roll_deg"]),
            QuaternionResource::RotateEulerXYZ => {
                Some(vec!["q", "delta_pitch_deg", "delta_yaw_deg", "delta_roll_deg"])
            }
        }
    }
}

// ===========================================================
// ResourceModule routing - similar to ApiModule
// ===========================================================

use crate::api_bindings::generate_rust_args;

impl ResourceModule {
    /// Primary entry point for code generation of a resource API call
    pub fn to_rust(
        &self,
        args: &[Expr],
        script: &Script,
        needs_self: bool,
        current_func: Option<&Function>,
    ) -> String {
        let expected_arg_types = self.param_types();
        let rust_args_strings = generate_rust_args(
            args,
            script,
            needs_self,
            current_func,
            expected_arg_types.as_ref(),
        );

        match self {
            ResourceModule::Signal(api) => {
                api.to_rust_prepared(args, &rust_args_strings, script, needs_self, current_func)
            }
            ResourceModule::Texture(api) => {
                api.to_rust_prepared(args, &rust_args_strings, script, needs_self, current_func)
            }
            ResourceModule::Mesh(api) => {
                api.to_rust_prepared(args, &rust_args_strings, script, needs_self, current_func)
            }
            ResourceModule::Shape(api) => {
                api.to_rust_prepared(args, &rust_args_strings, script, needs_self, current_func)
            }
            ResourceModule::ArrayOp(api) => {
                api.to_rust_prepared(args, &rust_args_strings, script, needs_self, current_func)
            }
            ResourceModule::MapOp(api) => {
                api.to_rust_prepared(args, &rust_args_strings, script, needs_self, current_func)
            }
            ResourceModule::QuaternionOp(api) => {
                api.to_rust_prepared(args, &rust_args_strings, script, needs_self, current_func)
            }
        }
    }

    pub fn return_type(&self) -> Option<Type> {
        match self {
            ResourceModule::Signal(api) => api.return_type(),
            ResourceModule::Texture(api) => api.return_type(),
            ResourceModule::Mesh(api) => api.return_type(),
            ResourceModule::Shape(api) => api.return_type(),
            ResourceModule::ArrayOp(api) => api.return_type(),
            ResourceModule::MapOp(api) => api.return_type(),
            ResourceModule::QuaternionOp(api) => api.return_type(),
        }
    }

    pub fn param_types(&self) -> Option<Vec<Type>> {
        match self {
            ResourceModule::Signal(api) => api.param_types(),
            ResourceModule::Texture(api) => api.param_types(),
            ResourceModule::Mesh(api) => api.param_types(),
            ResourceModule::Shape(api) => api.param_types(),
            ResourceModule::ArrayOp(api) => api.param_types(),
            ResourceModule::MapOp(api) => api.param_types(),
            ResourceModule::QuaternionOp(api) => api.param_types(),
        }
    }

    /// Get script-side parameter names (what PUP users see)
    pub fn param_names(&self) -> Option<Vec<&'static str>> {
        match self {
            ResourceModule::Signal(api) => api.param_names(),
            ResourceModule::Texture(api) => api.param_names(),
            ResourceModule::Mesh(api) => api.param_names(),
            ResourceModule::Shape(api) => api.param_names(),
            ResourceModule::ArrayOp(api) => api.param_names(),
            ResourceModule::MapOp(api) => api.param_names(),
            ResourceModule::QuaternionOp(api) => api.param_names(),
        }
    }
}

// ===========================================================
// Mesh API Implementations
// ===========================================================

impl ModuleCodegen for MeshResource {
    fn to_rust_prepared(
        &self,
        _args: &[Expr],
        args_strs: &[String],
        _script: &Script,
        _needs_self: bool,
        _current_func: Option<&Function>,
    ) -> String {
        match self {
            MeshResource::Load => {
                let arg = args_strs.get(0).cloned().unwrap_or_else(|| "\"\"".into());
                let arg_str = if arg.starts_with('"') && arg.ends_with('"') {
                    arg
                } else if arg.starts_with("String::from(") && arg.ends_with(')') {
                    let inner = &arg["String::from(".len()..arg.len() - 1].trim();
                    if inner.starts_with('"') && inner.ends_with('"') {
                        inner.to_string()
                    } else {
                        format!("&{}", arg)
                    }
                } else {
                    format!("&{}", arg)
                };
                format!("api.Mesh.load({})", arg_str)
            }
            MeshResource::Preload => {
                let arg = args_strs.get(0).cloned().unwrap_or_else(|| "\"\"".into());
                let arg_str = if arg.starts_with('"') && arg.ends_with('"') {
                    arg
                } else if arg.starts_with("String::from(") && arg.ends_with(')') {
                    let inner = &arg["String::from(".len()..arg.len() - 1].trim();
                    if inner.starts_with('"') && inner.ends_with('"') {
                        inner.to_string()
                    } else {
                        format!("&{}", arg)
                    }
                } else {
                    format!("&{}", arg)
                };
                format!("api.Mesh.preload({})", arg_str)
            }
            MeshResource::Remove => {
                let arg = args_strs
                    .get(0)
                    .cloned()
                    .unwrap_or_else(|| "MeshID::nil()".into());
                format!("api.Mesh.remove({})", arg)
            }
        }
    }
}

impl ModuleTypes for MeshResource {
    fn return_type(&self) -> Option<Type> {
        match self {
            MeshResource::Load | MeshResource::Preload => Some(Type::EngineStruct(EngineStruct::Mesh)),
            MeshResource::Remove => None,
        }
    }

    fn param_types(&self) -> Option<Vec<Type>> {
        match self {
            MeshResource::Load | MeshResource::Preload => Some(vec![Type::String]),
            MeshResource::Remove => Some(vec![Type::EngineStruct(EngineStruct::Mesh)]),
        }
    }

    fn param_names(&self) -> Option<Vec<&'static str>> {
        match self {
            MeshResource::Load | MeshResource::Preload => Some(vec!["path"]),
            MeshResource::Remove => Some(vec!["mesh"]),
        }
    }
}
