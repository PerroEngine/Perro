use crate::{
    api_modules::*, ast::*, node_registry::NodeType, prelude::string_to_u64, scripting::ast::{ContainerKind, NumberKind}
};

// ===========================================================
// Shared API Traits — Codegen + Types
// ===========================================================

/// Provides type semantics for API calls (return types, parameter types).
pub trait ApiTypes {
    /// Returns the return type of the API call.
    fn return_type(&self) -> Option<Type>;

    /// Returns the expected argument types for the API call, in order.
    /// Default is `None` (no specific type expectations).
    fn param_types(&self) -> Option<Vec<Type>> {
        None // Default implementation, no specific param types
    }
}

/// Converts a generic API call into Rust source code output.
pub trait ApiCodegen {
    /// Generates the final Rust code string for a specific API call.
    /// The `args_strs` parameter contains the already-processed (and potentially casted)
    /// Rust code strings for each argument. This trait's implementors are purely
    /// responsible for constructing the final call string from these prepared arguments.
    fn to_rust_prepared(
        &self,
        args: &[Expr],        // <--- ADD THIS so you have access to the typed AST
        args_strs: &[String], // Keep: for easy code plug-in
        script: &Script,
        needs_self: bool,
        current_func: Option<&Function>,
    ) -> String;
}

// ===========================================================
// Aggregator — routes both codegen + type semantics
// ===========================================================

impl ApiModule {
    /// Primary entry point for code generation of an API call from its AST representation.
    /// This orchestrates the argument processing (including `self.` prefixing and type-aware casting)
    /// before delegating to the specific `ApiCodegen` implementation for final Rust string assembly.
    pub fn to_rust(
        &self,
        args: &[Expr], // Raw AST expressions for arguments
        script: &Script,
        needs_self: bool,
        current_func: Option<&Function>,
    ) -> String {
        // 1. Get the expected parameter types for *this specific* API call.
        // This call dispatches to the correct `ApiTypes` implementation (e.g., `ArrayApi`'s `param_types`).
        let expected_arg_types = self.param_types();

        // 2. Process the raw AST arguments into Rust code strings.
        // This `generate_rust_args` helper handles:
        //    - Converting `Expr` to basic Rust code string.
        //    - Applying `self.` prefixing for script fields.
        //    - Applying implicit type casts based on `expected_arg_types`.
        let rust_args_strings = generate_rust_args(
            args,
            script,
            needs_self,
            current_func,
            expected_arg_types.as_ref(),
        );

        // 3. Delegate to the specific `ApiCodegen` implementation to build the final Rust call string.
        // This `match self` ensures the correct `to_rust_prepared` method is called based on the `ApiModule` variant.
        match self {
            ApiModule::JSON(api) => {
                api.to_rust_prepared(args, &rust_args_strings, script, needs_self, current_func)
            }
            ApiModule::Time(api) => {
                api.to_rust_prepared(args, &rust_args_strings, script, needs_self, current_func)
            }
            ApiModule::OS(api) => {
                api.to_rust_prepared(args, &rust_args_strings, script, needs_self, current_func)
            }
            ApiModule::Console(api) => {
                api.to_rust_prepared(args, &rust_args_strings, script, needs_self, current_func)
            }
            ApiModule::ScriptType(api) => {
                api.to_rust_prepared(args, &rust_args_strings, script, needs_self, current_func)
            }
            ApiModule::NodeSugar(api) => {
                api.to_rust_prepared(args, &rust_args_strings, script, needs_self, current_func)
            }
            ApiModule::Signal(api) => {
                api.to_rust_prepared(args, &rust_args_strings, script, needs_self, current_func)
            }
            ApiModule::ArrayOp(api) => {
                api.to_rust_prepared(args, &rust_args_strings, script, needs_self, current_func)
            }
            ApiModule::MapOp(api) => {
                api.to_rust_prepared(args, &rust_args_strings, script, needs_self, current_func)
            }
        }
    }

    /// Dispatches the `return_type` call to the appropriate `ApiTypes` implementation for this module variant.
    pub fn return_type(&self) -> Option<Type> {
        match self {
            ApiModule::JSON(api) => api.return_type(),
            ApiModule::Time(api) => api.return_type(),
            ApiModule::OS(api) => api.return_type(),
            ApiModule::Console(api) => api.return_type(),
            ApiModule::ScriptType(api) => api.return_type(),
            ApiModule::NodeSugar(api) => api.return_type(),
            ApiModule::Signal(api) => api.return_type(),
            ApiModule::ArrayOp(api) => api.return_type(),
            ApiModule::MapOp(api) => api.return_type(),
        }
    }

    /// Dispatches the `param_types` call to the appropriate `ApiTypes` implementation for this module variant.
    pub fn param_types(&self) -> Option<Vec<Type>> {
        let result = match self {
            ApiModule::JSON(api) => api.param_types(),
            ApiModule::Time(api) => api.param_types(),
            ApiModule::OS(api) => api.param_types(),
            ApiModule::Console(api) => api.param_types(),
            ApiModule::ScriptType(api) => api.param_types(),
            ApiModule::NodeSugar(api) => api.param_types(),
            ApiModule::Signal(api) => api.param_types(),
            ApiModule::ArrayOp(api) => api.param_types(),
            ApiModule::MapOp(api) => api.param_types(),
        };
        // Add this line:
        result
    }
}

/// Helper function to process raw `Expr` arguments into formatted Rust code strings.
/// This includes converting the `Expr` to its basic Rust code, applying `self.` prefixing
/// for script fields, and handling implicit type casts based on `expected_arg_types`.
fn generate_rust_args(
    args: &[Expr],
    script: &Script,
    needs_self: bool,
    current_func: Option<&Function>,
    expected_arg_types: Option<&Vec<Type>>,
) -> Vec<String> {
    args.iter()
        .enumerate()
        .map(|(i, a)| {
            // 1. Convert the raw AST expression `a` into its basic Rust code string.
            //    This `code_raw` is the *uncasted*, *unprefixed* base string.
            let expected_ty_hint = expected_arg_types.and_then(|v| v.get(i));
            let mut code_raw = a.to_rust(needs_self, script, expected_ty_hint, current_func);

            // 2. Determine if a cast is needed and apply it to `code_raw`.
            //    This part happens first on the raw expression's representation.
            if let Some(expected_types) = expected_arg_types {
                if let Some(expect_ty) = expected_types.get(i) {
                    if let Some(actual_ty) = script.infer_expr_type(a, current_func) {
                        if actual_ty.can_implicitly_convert_to(expect_ty) && actual_ty != *expect_ty
                        {
                            code_raw = script.generate_implicit_cast_for_expr(
                                &code_raw, // Use code_raw here!
                                &actual_ty, expect_ty,
                            );
                        }
                    }
                }
            }

            // 3. Now, take the (potentially casted) `code_raw` and apply `self.` prefixing.
            //    This is the *last* step to construct the final argument string.
            let mut final_code = code_raw;
            if let Expr::Ident(name) = a {
                if script.is_struct_field(name) && !final_code.starts_with("self.") {
                    final_code = format!("self.{final_code}");
                }
            }
            final_code // Return the final, processed string
        })
        .collect()
}

// ===========================================================
// JSON API Implementations
// ===========================================================

impl ApiCodegen for JSONApi {
    fn to_rust_prepared(
        &self,
        args: &[Expr],
        args_strs: &[String],
        _script: &Script,                 // script not usually needed here
        _needs_self: bool,                // needs_self not usually needed here
        _current_func: Option<&Function>, // current_func not usually needed here
    ) -> String {
        match self {
            JSONApi::Parse => {
                let arg = args_strs.get(0).cloned().unwrap_or_else(|| "\"\"".into());
                format!("api.JSON.parse(&{})", arg)
            }
            JSONApi::Stringify => {
                let arg = args_strs
                    .get(0)
                    .cloned()
                    .unwrap_or_else(|| "json!({})".into());
                format!("api.JSON.stringify(&{})", arg)
            }
        }
    }
}

impl ApiTypes for JSONApi {
    fn return_type(&self) -> Option<Type> {
        match self {
            JSONApi::Parse => Some(Type::Object),
            JSONApi::Stringify => Some(Type::String),
        }
    }

    fn param_types(&self) -> Option<Vec<Type>> {
        match self {
            JSONApi::Parse => Some(vec![Type::String]),
            JSONApi::Stringify => Some(vec![Type::Object]),
        }
    }
}

// ===========================================================
// Time API Implementations
// ===========================================================

impl ApiCodegen for TimeApi {
    fn to_rust_prepared(
        &self,
        args: &[Expr],
        args_strs: &[String],
        _script: &Script,
        _needs_self: bool,
        _current_func: Option<&Function>,
    ) -> String {
        match self {
            TimeApi::DeltaTime => "api.Time.get_delta()".into(),
            TimeApi::GetUnixMsec => "api.Time.get_unix_time_msec()".into(),
            TimeApi::SleepMsec => {
                let arg = args_strs.get(0).cloned().unwrap_or_else(|| "0".into());
                format!("api.Time.sleep_msec({})", arg)
            }
        }
    }
}

impl ApiTypes for TimeApi {
    fn return_type(&self) -> Option<Type> {
        match self {
            TimeApi::DeltaTime => Some(Type::Number(NumberKind::Float(32))),
            TimeApi::GetUnixMsec => Some(Type::Number(NumberKind::Unsigned(64))),
            TimeApi::SleepMsec => Some(Type::Void),
        }
    }

    fn param_types(&self) -> Option<Vec<Type>> {
        match self {
            TimeApi::SleepMsec => Some(vec![Type::Number(NumberKind::Unsigned(64))]),
            _ => None,
        }
    }
    // No specific param_types for Time APIs needed by default, uses None.
}

// ===========================================================
// OS API Implementations
// ===========================================================

impl ApiCodegen for OSApi {
    fn to_rust_prepared(
        &self,
        args: &[Expr],
        args_strs: &[String],
        _script: &Script,
        _needs_self: bool,
        _current_func: Option<&Function>,
    ) -> String {
        match self {
            OSApi::GetPlatformName => "api.OS.get_platform_name()".into(),
            OSApi::GetEnv => {
                let arg = args_strs.get(0).cloned().unwrap_or_else(|| "\"\"".into());
                format!("api.OS.getenv({})", arg)
            }
        }
    }
}

impl ApiTypes for OSApi {
    fn return_type(&self) -> Option<Type> {
        match self {
            OSApi::GetPlatformName => Some(Type::String),
            OSApi::GetEnv => Some(Type::String),
        }
    }

    fn param_types(&self) -> Option<Vec<Type>> {
        match self {
            OSApi::GetEnv => Some(vec![Type::String]),
            _ => None,
        }
    }
    // No specific param_types for OS APIs needed by default, uses None.
}

// ===========================================================
// Console API Implementations
// ===========================================================

impl ApiCodegen for ConsoleApi {
    fn to_rust_prepared(
        &self,
        args: &[Expr],
        args_strs: &[String],
        script: &Script, // Keep script here for verbose check
        _needs_self: bool,
        _current_func: Option<&Function>,
    ) -> String {
        let joined = if args_strs.len() <= 1 {
            args_strs.get(0).cloned().unwrap_or("\"\"".into())
        } else {
            format!(
                "format!(\"{}\", {})",
                (0..args_strs.len())
                    .map(|_| "{}")
                    .collect::<Vec<_>>()
                    .join(" "),
                args_strs.join(", "),
            )
        };

        let line = match self {
            ConsoleApi::Log => format!("api.print(&{})", joined),
            ConsoleApi::Warn => format!("api.print_warn(&{})", joined),
            ConsoleApi::Error => format!("api.print_error(&{})", joined),
            ConsoleApi::Info => format!("api.print_info(&{})", joined),
        };

        if script.verbose {
            line
        } else {
            format!("// [stripped for release] {}", line)
        }
    }
}

impl ApiTypes for ConsoleApi {
    fn return_type(&self) -> Option<Type> {
        Some(Type::Void)
    }
    // No specific param_types for Console APIs needed by default, uses None.
}

// ===========================================================
// ScriptType API Implementations
// ===========================================================

impl ApiCodegen for ScriptTypeApi {
    fn to_rust_prepared(
        &self,
        args: &[Expr],
        args_strs: &[String],
        _script: &Script,
        _needs_self: bool,
        _current_func: Option<&Function>,
    ) -> String {
        match self {
            ScriptTypeApi::Instantiate => {
                let arg = args_strs.get(0).cloned().unwrap_or_else(|| "\"\"".into());
                format!("api.instantiate_script({})", arg)
            }
        }
    }
}

impl ApiTypes for ScriptTypeApi {
    fn return_type(&self) -> Option<Type> {
        Some(Type::Script)
    }
    // No specific param_types for ScriptType APIs needed by default, uses None.
}

// ===========================================================
// NodeSugar API Implementations
// ===========================================================

impl ApiCodegen for NodeSugarApi {
    fn to_rust_prepared(
        &self,
        args: &[Expr],
        args_strs: &[String],
        _script: &Script,
        _needs_self: bool,
        _current_func: Option<&Function>,
    ) -> String {
        match self {
            NodeSugarApi::GetVar => {
                let (node, name) = (args_strs.get(0), args_strs.get(1));
                format!(
                    "api.get_script_var(&{}.id, {})",
                    node.map(|s| s.as_str()).unwrap_or("self"),
                    name.map(|s| s.as_str()).unwrap_or("\"\"")
                )
            }
            NodeSugarApi::SetVar => {
                let (node, name, val) = (args_strs.get(0), args_strs.get(1), args_strs.get(2));
                format!(
                    "api.set_script_var(&{}.id, {}, {})",
                    node.map(|s| s.as_str()).unwrap_or("self"),
                    name.map(|s| s.as_str()).unwrap_or("\"\""),
                    val.map(|s| s.as_str()).unwrap_or("Value::Null")
                )
            }
        }
    }
}

impl ApiTypes for NodeSugarApi {
    fn return_type(&self) -> Option<Type> {
        match self {
            NodeSugarApi::GetVar => Some(Type::Custom("Value".into())),
            NodeSugarApi::SetVar => Some(Type::Void),
        }
    }
    // No specific param_types for NodeSugar APIs needed by default, uses None.
}

// ===========================================================
// Signal API Implementations
// ===========================================================

impl ApiCodegen for SignalApi {
    fn to_rust_prepared(
        &self,
        args: &[Expr],
        args_strs: &[String],
        script: &Script, // <-- Remove leading underscore
        _needs_self: bool,
        current_func: Option<&Function>, // <-- Remove leading underscore
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
            SignalApi::New => {
                let signal = args_strs.get(0).cloned().unwrap_or_else(|| "\"\"".into());
                prehash_if_literal(&signal)
            }
            SignalApi::Connect | SignalApi::Emit => {
                // -- Fix: Accept both u64 and Type::Custom("Signal") as passthrough variables
                let arg_expr = args.get(0).unwrap();
                let arg_type = script.infer_expr_type(arg_expr, current_func);

                let signal = match arg_type {
                    Some(Type::Number(NumberKind::Unsigned(64))) => args_strs[0].clone(),
                    Some(Type::Custom(ref s)) if s == "Signal" => args_strs[0].clone(),
                    _ => prehash_if_literal(&args_strs[0]),
                };

                match self {
                    SignalApi::Connect => {
                        let mut node = args_strs
                            .get(1)
                            .cloned()
                            .unwrap_or_else(|| "self.node".into());
                        if node == "self" {
                            node = "self.node".into()
                        }
                        let func = strip_string_from(args_strs.get(2).unwrap());
                        let func_id = string_to_u64(&func);
                        format!("api.connect_signal_id({signal}, {node}.id, {func_id}u64)")
                    }
                    SignalApi::Emit => {
                        if args_strs.len() > 1 {
                            let params: Vec<String> = args_strs[1..]
                                .iter()
                                .map(|a| format!("json!({a})"))
                                .collect();
                            format!(
                                "api.emit_signal_id({signal}, smallvec![{}])",
                                params.join(", ")
                            )
                        } else {
                            format!("api.emit_signal_id({signal}, smallvec![])")
                        }
                    }
                    _ => unreachable!(),
                }
            }
        }
    }
}

impl ApiTypes for SignalApi {
    fn return_type(&self) -> Option<Type> {
        match self {
            SignalApi::New => Some(Type::Custom("Signal".into())),
            _ => Some(Type::Void),
        }
    }

    fn param_types(&self) -> Option<Vec<Type>> {
        match self {
            SignalApi::New => Some(vec![Type::String]),
            SignalApi::Emit => Some(vec![Type::Custom("Signal".to_string()), Type::Object]),
            SignalApi::Connect => Some(vec![Type::Custom("Signal".to_string())]),
        }
    }
    // No specific param_types for Signal APIs needed by default, uses None.
}

// ===========================================================
// ArrayOp API Implementations
// ===========================================================

impl ApiCodegen for ArrayApi {
    fn to_rust_prepared(
        &self,
        args: &[Expr],
        args_strs: &[String],
        script: &Script,
        needs_self: bool,
        current_func: Option<&Function>,
    ) -> String {
        match self {
            ArrayApi::Push => {
                // args[0] is the array expression, args[1] is the value to push
                let array_expr = &args[0];
                let value_expr = &args[1]; // The raw AST expression for the value

                // Infer the type of the array itself to get its inner type
                let array_type = script.infer_expr_type(array_expr, current_func);

                let inner_type =
                    if let Some(Type::Container(ContainerKind::Array, inner_types)) = array_type {
                        inner_types.get(0).cloned().unwrap_or(Type::Object)
                    } else {
                        Type::Object // Fallback if array type couldn't be inferred
                    };

                let mut value_code =
                    value_expr.to_rust(needs_self, script, Some(&inner_type), current_func);

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
                } else if value_code.starts_with("json!(") && inner_type == Type::Object {
                    // If target is Type::Object, json! is fine, just use the string directly
                    // No change needed.
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

                format!("{}.push({})", args_strs[0], value_code)
            }
            ArrayApi::Pop => {
                format!("{}.pop()", args_strs[0])
            }
            ArrayApi::Len => {
                format!("{}.len()", args_strs[0])
            }
            ArrayApi::Insert => {
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
                    value_expr.to_rust(needs_self, script, Some(&inner_type), current_func);

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
            ArrayApi::Remove => {
                format!("{}.remove({} as usize)", args_strs[0], args_strs[1])
            }
            ArrayApi::New => {
                format!("Vec::new()")
            }
        }
    }
}

impl ApiTypes for ArrayApi {
    fn return_type(&self) -> Option<Type> {
        match self {
            ArrayApi::Push => Some(Type::Void),
            ArrayApi::Pop => Some(Type::Object),
            ArrayApi::Insert => Some(Type::Void),
            ArrayApi::Remove => Some(Type::Object),
            ArrayApi::Len => Some(Type::Number(NumberKind::Unsigned(32))),
            ArrayApi::New => Some(Type::Container(ContainerKind::Array, vec![Type::Object])),
        }
    }

    fn param_types(&self) -> Option<Vec<Type>> {
        use ContainerKind::*;
        use NumberKind::*;

        match self {
            ArrayApi::Push => Some(vec![
                Type::Container(Array, vec![Type::Object]),
                Type::Object, // any value; just “Value”
            ]),
            ArrayApi::Insert => Some(vec![
                Type::Container(Array, vec![Type::Object]),
                Type::Number(Unsigned(32)), // index
                Type::Object,               // value
            ]),
            ArrayApi::Remove => Some(vec![
                Type::Container(Array, vec![Type::Object]),
                Type::Number(Unsigned(32)), // index expected
            ]),
            ArrayApi::Len | ArrayApi::Pop | ArrayApi::New => None,
        }
    }
}

impl ApiCodegen for MapApi {
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
            MapApi::Insert => {
                let key_type = script.infer_map_key_type(&args[0], current_func);
                let val_type = script.infer_map_value_type(&args[0], current_func);
                let key_code = args[1].to_rust(needs_self, script, key_type.as_ref(), current_func);
                let val_code = args[2].to_rust(needs_self, script, val_type.as_ref(), current_func);
                format!("{}.insert({}, {})", args_strs[0], key_code, val_code)
            }

            // args: [map, key]
            MapApi::Remove => {
                let key_type = script.infer_map_key_type(&args[0], current_func);
                let key_code = args[1].to_rust(needs_self, script, key_type.as_ref(), current_func);
                if let Some(Type::String) = key_type.as_ref() {
                    format!("{}.remove({}.as_str())", args_strs[0], key_code)
                } else {
                    format!("{}.remove(&{})", args_strs[0], key_code)
                }
            }

            // args: [map, key]
            MapApi::Get => {
                // 1. Infer key type from map
                let key_type = script.infer_map_key_type(&args[0], current_func);
                // 2. Render the key argument with the right type hint
                let key_code = args[1].to_rust(needs_self, script, key_type.as_ref(), current_func);

                if let Some(Type::String) = key_type.as_ref() {
                    // for String keys, .as_str() may be appropriate
                    format!(
                        "{}.get({}.as_str()).cloned().unwrap_or_default()",
                        args_strs[0], key_code
                    )
                } else {
                    // for any other key type (i32, u64, f32, etc)
                    format!(
                        "{}.get(&{}).cloned().unwrap_or_default()",
                        args_strs[0], key_code
                    )
                }
            }

            // args: [map, key]
            MapApi::Contains => {
                let key_type = script.infer_map_key_type(&args[0], current_func);
                let key_code = args[1].to_rust(needs_self, script, key_type.as_ref(), current_func);
                if let Some(Type::String) = key_type.as_ref() {
                    format!("{}.contains_key({}.as_str())", args_strs[0], key_code)
                } else {
                    format!("{}.contains_key(&{})", args_strs[0], key_code)
                }
            }

            // args: [map]
            MapApi::Len => {
                format!("{}.len()", args_strs[0])
            }

            // args: [map]
            MapApi::Clear => {
                format!("{}.clear()", args_strs[0])
            }

            // no args
            MapApi::New => "HashMap::new()".into(),
        }
    }
}

impl ApiTypes for MapApi {
    fn return_type(&self) -> Option<Type> {
        match self {
            MapApi::Insert | MapApi::Clear => Some(Type::Void),
            MapApi::Remove | MapApi::Get => Some(Type::Object),
            MapApi::Contains => Some(Type::Bool),
            MapApi::Len => Some(Type::Number(NumberKind::Unsigned(32))),
            MapApi::New => Some(Type::Container(
                ContainerKind::Map,
                vec![Type::String, Type::Object],
            )),
        }
    }

    fn param_types(&self) -> Option<Vec<Type>> {
        match self {
            MapApi::Insert => Some(vec![
                Type::Container(ContainerKind::Map, vec![Type::String, Type::Object]),
                Type::String,
                Type::Object,
            ]),
            MapApi::Remove | MapApi::Get => Some(vec![
                Type::Container(ContainerKind::Map, vec![Type::String, Type::Object]),
                Type::String,
            ]),
            MapApi::Contains => Some(vec![
                Type::Container(ContainerKind::Map, vec![Type::String, Type::Object]),
                Type::String,
            ]),
            _ => None,
        }
    }
}
