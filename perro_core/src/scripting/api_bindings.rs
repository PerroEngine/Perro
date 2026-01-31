use super::codegen::is_node_type;
use crate::{
    api_modules::*, // Import module API enums and ApiModule
    ast::*,
    scripting::ast::NumberKind,
};

// ===========================================================
// Shared API Traits — Codegen + Types
// ===========================================================

/// Provides type semantics for API calls (return types, parameter types).
pub trait ModuleTypes {
    /// Returns the return type of the API call.
    fn return_type(&self) -> Option<Type>;

    /// Returns the expected argument types for the API call, in order.
    /// Default is `None` (no specific type expectations).
    fn param_types(&self) -> Option<Vec<Type>> {
        None // Default implementation, no specific param types
    }

    /// Returns friendly parameter names for the API call, in order.
    /// Should match the length of param_types() if both are Some.
    /// Default is `None` (no specific parameter names).
    fn param_names(&self) -> Option<Vec<&'static str>> {
        None // Default implementation, no specific param names
    }
}

/// Converts a generic API call into Rust source code output.
pub trait ModuleCodegen {
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
    /// before delegating to the specific `ModuleCodegen` implementation for final Rust string assembly.
    pub fn to_rust(
        &self,
        args: &[Expr], // Raw AST expressions for arguments
        script: &Script,
        needs_self: bool,
        current_func: Option<&Function>,
    ) -> String {
        // 1. Get the expected parameter types for *this specific* API call.
        // This call dispatches to the correct `ModuleTypes` implementation (e.g., `ArrayResource`'s `param_types`).
        let expected_arg_types = self.param_types();

        // 2. Process the raw AST arguments into Rust code strings.
        // This `generate_rust_args` helper handles:
        //    - Converting `Expr` to basic Rust code string.
        //    - Applying `self.` prefixing for script fields.
        //    - Applying implicit type casts based on `expected_arg_types`.
        let mut rust_args_strings = generate_rust_args(
            args,
            script,
            needs_self,
            current_func,
            expected_arg_types.as_ref(),
        );

        // DEBUG: Check if rust_args_strings is wrong
        if !args.is_empty()
            && (rust_args_strings.is_empty()
                || rust_args_strings.iter().any(|s| s.trim().is_empty()))
        {
            eprintln!(
                "[DEBUG ApiModule::to_rust] args.len()={}, rust_args_strings.len()={}, rust_args_strings={:?}, args={:?}",
                args.len(),
                rust_args_strings.len(),
                rust_args_strings,
                args
            );
        }

        // If generate_rust_args returned empty strings, regenerate directly from args
        // This handles cases where temp variables aren't properly converted
        if rust_args_strings.iter().any(|s| s.trim().is_empty()) && !args.is_empty() {
            rust_args_strings = args
                .iter()
                .enumerate()
                .map(|(i, a)| {
                    let expected_ty_hint =
                        expected_arg_types.as_ref().and_then(|v| v.get(i).cloned());
                    let code = a.to_rust(
                        needs_self,
                        script,
                        expected_ty_hint.as_ref(),
                        current_func,
                        None,
                    );
                    if code.trim().is_empty() {
                        eprintln!(
                            "[DEBUG ApiModule::to_rust] to_rust returned empty for arg[{}]: {:?}",
                            i, a
                        );
                    }
                    code
                })
                .collect();
        }

        // 3. Delegate to the specific `ModuleCodegen` implementation to build the final Rust call string.
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
            ApiModule::Input(api) => {
                api.to_rust_prepared(args, &rust_args_strings, script, needs_self, current_func)
            }
            ApiModule::Math(api) => {
                api.to_rust_prepared(args, &rust_args_strings, script, needs_self, current_func)
            }
        }
    }

    /// Dispatches the `return_type` call to the appropriate `ModuleTypes` implementation for this module variant.
    pub fn return_type(&self) -> Option<Type> {
        match self {
            ApiModule::JSON(api) => api.return_type(),
            ApiModule::Time(api) => api.return_type(),
            ApiModule::OS(api) => api.return_type(),
            ApiModule::Console(api) => api.return_type(),
            ApiModule::Input(api) => api.return_type(),
            ApiModule::Math(api) => api.return_type(),
        }
    }

    /// Dispatches the `param_types` call to the appropriate `ModuleTypes` implementation for this module variant.
    pub fn param_types(&self) -> Option<Vec<Type>> {
        let result = match self {
            ApiModule::JSON(api) => api.param_types(),
            ApiModule::Time(api) => api.param_types(),
            ApiModule::OS(api) => api.param_types(),
            ApiModule::Console(api) => api.param_types(),
            ApiModule::Input(api) => api.param_types(),
            ApiModule::Math(api) => api.param_types(),
        };
        // Add this line:
        result
    }

    /// Dispatches the `param_names` call to the appropriate `ModuleTypes` implementation for this module variant.
    /// Returns script-side parameter names (what PUP users see), not internal Rust parameter names.
    pub fn param_names(&self) -> Option<Vec<&'static str>> {
        match self {
            ApiModule::JSON(api) => api.param_names(),
            ApiModule::Time(api) => api.param_names(),
            ApiModule::OS(api) => api.param_names(),
            ApiModule::Console(api) => api.param_names(),
            ApiModule::Input(api) => api.param_names(),
            ApiModule::Math(api) => api.param_names(),
        }
    }
}

/// True when an Ident used as a NodeID argument is already a concrete NodeID (no .expect() needed).
/// Callback params (c: CollisionShape2D), globals (NodeID::from_u32), and self.id are always concrete.
fn is_concrete_node_id_arg(
    _script: &Script,
    current_func: Option<&Function>,
    name: &str,
    code_raw: &str,
    actual_ty: Option<&Type>,
) -> bool {
    // Globals: code is NodeID::from_u32(...) — always concrete
    if code_raw.starts_with("NodeID::from_u32(") {
        return true;
    }
    // Function param typed as node (e.g. on Deadzone_AreaExited(c: CollisionShape2D)) — engine always passes concrete NodeID
    if let Some(f) = current_func {
        if let Some(p) = f.params.iter().find(|p| p.name == name) {
            if matches!(p.typ, Type::Node(_) | Type::DynNode) {
                return true;
            }
        }
    }
    // Inferred/declared type is concrete Node/DynNode (not Option<NodeID> or UuidOption)
    if let Some(ty) = actual_ty {
        if matches!(ty, Type::Node(_) | Type::DynNode) {
            return true;
        }
    }
    false
}

/// Helper function to process raw `Expr` arguments into formatted Rust code strings.
/// This includes converting the `Expr` to its basic Rust code, applying `self.` prefixing
/// for script fields, and handling implicit type casts based on `expected_arg_types`.
pub(crate) fn generate_rust_args(
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
            let mut code_raw = a.to_rust(needs_self, script, expected_ty_hint, current_func, None);

            // 2. Determine if a cast is needed and apply it to `code_raw`.
            //    This part happens first on the raw expression's representation.
            if let Some(expected_types) = expected_arg_types {
                if let Some(expect_ty) = expected_types.get(i) {
                    let actual_ty = script.infer_expr_type(a, current_func);
                    if let Some(ref actual_ty) = actual_ty {
                        if actual_ty.can_implicitly_convert_to(expect_ty) && actual_ty != expect_ty {
                            code_raw = script.generate_implicit_cast_for_expr(
                                &code_raw,
                                actual_ty,
                                expect_ty,
                            );
                        }
                    } else if i == 0
                        && matches!(expect_ty, Type::DynNode)
                        && matches!(a, Expr::Ident(name) if !is_concrete_node_id_arg(script, current_func, name, &code_raw, None))
                        && !code_raw.is_empty()
                        && code_raw != "self.id"
                        && code_raw != "self"
                        && !code_raw.contains(".expect(")
                    {
                        // Fallback: first arg expects NodeID; unwrap Option<NodeID> so get_script_var_id/call_function_id get NodeID.
                        // Covers both unknown actual_ty and Option(DynNode)/UuidOption when implicit cast wasn't applied.
                        code_raw = format!("{}.expect(\"Child node not found\")", code_raw);
                    }
                }
            }
            // Only add .expect() when the argument is actually Option<NodeID>/UuidOption (e.g. get_node result).
            // Do NOT add for concrete NodeIDs: callback params (c: CollisionShape2D), globals (NodeID::from_u32), self.id.
            if let Some(expected_types) = expected_arg_types {
                if let Some(expect_ty) = expected_types.get(i) {
                    let actual_ty = script.infer_expr_type(a, current_func);
                    if i == 0
                        && matches!(expect_ty, Type::DynNode)
                        && matches!(a, Expr::Ident(name) if !is_concrete_node_id_arg(script, current_func, name, &code_raw, actual_ty.as_ref()))
                        && !code_raw.is_empty()
                        && code_raw != "self.id"
                        && code_raw != "self"
                        && !code_raw.contains(".expect(")
                    {
                        code_raw = format!("{}.expect(\"Child node not found\")", code_raw);
                    }
                }
            }

            // 3. Now, take the (potentially casted) `code_raw` and apply `self.` prefixing.
            //    This is the *last* step to construct the final argument string.
            let mut final_code = code_raw;

            // If code_raw is empty, try to generate it directly from the expression
            // This handles cases where temp variables aren't found in the script's variable list
            if final_code.trim().is_empty() {
                if let Expr::Ident(name) = a {
                    // For temp variables, just return the name as-is
                    if name.starts_with("temp_api_var_") || name.starts_with("__temp_api_") {
                        return name.to_string();
                    }
                }
                // Last resort: try to_rust again without expected type hint
                final_code = a.to_rust(needs_self, script, None, current_func, None);
            }

            if let Expr::Ident(name) = a {
                // Special case: temp variables (temp_api_var_*) should NEVER get .clone() or be renamed
                // They're already in the correct format from to_rust
                if name.starts_with("temp_api_var_") || name.starts_with("__temp_api_") {
                    // Return as-is, don't add self. prefix or any modifications
                    return final_code;
                }
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

impl ModuleCodegen for JSONApi {
    fn to_rust_prepared(
        &self,
        _args: &[Expr],
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

impl ModuleTypes for JSONApi {
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

    /// Script-side parameter names (what PUP users see)
    fn param_names(&self) -> Option<Vec<&'static str>> {
        match self {
            JSONApi::Parse => Some(vec!["json_string"]),
            JSONApi::Stringify => Some(vec!["object"]),
        }
    }
}

// ===========================================================
// Time API Implementations
// ===========================================================

impl ModuleCodegen for TimeApi {
    fn to_rust_prepared(
        &self,
        _args: &[Expr],
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

impl ModuleTypes for TimeApi {
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

    /// Script-side parameter names (what PUP users see)
    fn param_names(&self) -> Option<Vec<&'static str>> {
        match self {
            TimeApi::SleepMsec => Some(vec!["milliseconds"]),
            _ => None,
        }
    }
}

// ===========================================================
// OS API Implementations
// ===========================================================

impl ModuleCodegen for OSApi {
    fn to_rust_prepared(
        &self,
        _args: &[Expr],
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

impl ModuleTypes for OSApi {
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

    /// Script-side parameter names (what PUP users see)
    fn param_names(&self) -> Option<Vec<&'static str>> {
        match self {
            OSApi::GetEnv => Some(vec!["name"]),
            _ => None,
        }
    }
}

// ===========================================================
// Console API Implementations
// ===========================================================

impl ModuleCodegen for ConsoleApi {
    fn to_rust_prepared(
        &self,
        args: &[Expr],
        args_strs: &[String],
        script: &Script, // Keep script here for verbose check
        _needs_self: bool,
        current_func: Option<&Function>,
    ) -> String {
        // Ensure args_strs is valid: same length as args, no empty strings.
        // Regenerate from args when needed (e.g. after temp variable extraction).
        let mut args_strs = args_strs.to_vec();
        let needs_regenerate = args_strs.is_empty() && !args.is_empty()
            || args_strs.len() != args.len()
            || args_strs.iter().any(|s| s.trim().is_empty());
        if needs_regenerate {
            args_strs = generate_rust_args(args, script, _needs_self, current_func, None);
            if args_strs.is_empty() && !args.is_empty() {
                args_strs = args
                    .iter()
                    .map(|a| a.to_rust(_needs_self, script, None, current_func, None))
                    .collect::<Vec<_>>();
            }
        }

        // Final check: if args_strs is still empty or all empty strings, regenerate from args
        let args_strs = if args_strs.is_empty() || args_strs.iter().all(|s| s.trim().is_empty()) {
            if !args.is_empty() {
                // Last resort: convert args directly
                args.iter()
                    .map(|a| {
                        let code = a.to_rust(_needs_self, script, None, current_func, None);
                        // Remove .clone() from temp_api_var_* if it was added
                        if code.starts_with("temp_api_var_") && code.ends_with(".clone()") {
                            code.strip_suffix(".clone()").unwrap_or(&code).to_string()
                        } else {
                            code
                        }
                    })
                    .collect::<Vec<_>>()
            } else {
                args_strs
            }
        } else {
            args_strs
        };

        // CRITICAL: If args_strs still contains empty strings but args is not empty, force regenerate
        // This handles cases where generate_rust_args returned empty strings incorrectly
        let args_strs = if (!args.is_empty() && args_strs.iter().any(|s| s.trim().is_empty()))
            || args_strs.len() != args.len()
        {
            // Force regenerate from args using to_rust directly
            args.iter()
                .map(|a| {
                    let code = a.to_rust(_needs_self, script, None, current_func, None);
                    // Remove .clone() from temp_api_var_* if it was added
                    if code.starts_with("temp_api_var_") && code.ends_with(".clone()") {
                        code.strip_suffix(".clone()").unwrap_or(&code).to_string()
                    } else {
                        code
                    }
                })
                .collect::<Vec<_>>()
        } else {
            args_strs
        };

        // ONE MORE CHECK: Right before using args_strs, verify it's not empty or contains empty strings
        // This is the last chance to fix it before it causes api.print_info("") to be generated
        let args_strs = if (!args.is_empty()
            && (args_strs.is_empty() || args_strs.iter().any(|s| s.trim().is_empty())))
            || args_strs.len() != args.len()
        {
            // Force regenerate from args - this should NEVER fail
            args.iter()
                .map(|a| {
                    let code = a.to_rust(_needs_self, script, None, current_func, None);
                    if code.trim().is_empty() {
                        // If to_rust returned empty, try with expected type hint
                        let expected_ty = self.param_types().and_then(|v| v.get(0).cloned());
                        a.to_rust(
                            _needs_self,
                            script,
                            expected_ty.as_ref(),
                            current_func,
                            None,
                        )
                    } else {
                        // Remove .clone() from temp_api_var_* if it was added
                        if code.starts_with("temp_api_var_") && code.ends_with(".clone()") {
                            code.strip_suffix(".clone()").unwrap_or(&code).to_string()
                        } else {
                            code
                        }
                    }
                })
                .collect::<Vec<_>>()
        } else {
            args_strs
        };

        // ABSOLUTE LAST CHECK: If args_strs is still wrong, force regenerate from args
        // This should NEVER happen, but if it does, we need to fix it
        let args_strs = if (!args.is_empty()
            && (args_strs.is_empty() || args_strs.iter().any(|s| s.trim().is_empty())))
            || args_strs.len() != args.len()
        {
            eprintln!(
                "[WARNING] args_strs is wrong in ConsoleApi::to_rust_prepared! args.len()={}, args_strs.len()={}, args_strs={:?}",
                args.len(),
                args_strs.len(),
                args_strs
            );
            // Force regenerate - this should work
            args.iter()
                .map(|a| {
                    let code = a.to_rust(_needs_self, script, None, current_func, None);
                    if code.trim().is_empty() {
                        eprintln!("[WARNING] to_rust returned empty string for arg: {:?}", a);
                        // Try with expected type
                        let expected_ty = self.param_types().and_then(|v| v.get(0).cloned());
                        a.to_rust(
                            _needs_self,
                            script,
                            expected_ty.as_ref(),
                            current_func,
                            None,
                        )
                    } else {
                        code
                    }
                })
                .collect::<Vec<_>>()
        } else {
            args_strs
        };

        let joined = if args_strs.len() <= 1 {
            // If args_strs is still empty or contains empty string, but args is not empty, use the first arg directly
            if (args_strs.is_empty() || args_strs[0].trim().is_empty()) && !args.is_empty() {
                let code = args[0].to_rust(_needs_self, script, None, current_func, None);
                if code.trim().is_empty() {
                    eprintln!(
                        "[ERROR] to_rust returned empty string for first arg: {:?}",
                        args[0]
                    );
                    // Try with expected type hint
                    let expected_ty = self.param_types().and_then(|v| v.get(0).cloned());
                    let code_with_hint = args[0].to_rust(
                        _needs_self,
                        script,
                        expected_ty.as_ref(),
                        current_func,
                        None,
                    );
                    if code_with_hint.trim().is_empty() {
                        format!("{:?}", args[0]) // Fallback to debug format
                    } else {
                        code_with_hint
                    }
                } else {
                    code
                }
            } else {
                args_strs.get(0).cloned().unwrap_or("\"\"".into())
            }
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

        // Helper function to find a variable in nested blocks (if, for, etc.)
        fn find_variable_in_body<'a>(
            name: &str,
            body: &'a [crate::scripting::ast::Stmt],
        ) -> Option<&'a crate::scripting::ast::Variable> {
            use crate::scripting::ast::Stmt;
            for stmt in body {
                match stmt {
                    Stmt::VariableDecl(var) if var.name == name => {
                        return Some(var);
                    }
                    Stmt::If {
                        then_body,
                        else_body,
                        ..
                    } => {
                        if let Some(v) = find_variable_in_body(name, then_body) {
                            return Some(v);
                        }
                        if let Some(else_body) = else_body {
                            if let Some(v) = find_variable_in_body(name, else_body) {
                                return Some(v);
                            }
                        }
                    }
                    Stmt::For { body: for_body, .. }
                    | Stmt::ForTraditional { body: for_body, .. } => {
                        if let Some(v) = find_variable_in_body(name, for_body) {
                            return Some(v);
                        }
                    }
                    _ => {}
                }
            }
            None
        }

        // Helper function to check if a variable represents a node
        fn check_if_node_var(
            var_name: &str,
            script: &Script,
            current_func: Option<&Function>,
        ) -> bool {
            if let Some(func) = current_func {
                // Check function local (including nested blocks)
                let local_opt = func
                    .locals
                    .iter()
                    .find(|v| v.name == *var_name)
                    .or_else(|| find_variable_in_body(var_name, &func.body));

                if let Some(local) = local_opt {
                    let declared_is_node = local
                        .typ
                        .as_ref()
                        .map(|t| matches!(t, Type::Node(_) | Type::DynNode))
                        .unwrap_or(false);

                    if declared_is_node {
                        return true;
                    }

                    if let Some(val) = &local.value {
                        let inferred_type = script.infer_expr_type(&val.expr, current_func);
                        let inferred_is_node = inferred_type
                            .as_ref()
                            .map(|t| matches!(t, Type::Node(_) | Type::DynNode))
                            .unwrap_or(false);

                        if inferred_is_node {
                            return true;
                        }

                        // Check if assigned from node expression
                        match &val.expr {
                            Expr::StructNew(ty, _) => return is_node_type(ty),
                            Expr::ApiCall(
                                crate::call_modules::CallModule::NodeMethod(
                                    crate::structs::engine_registry::NodeMethodRef::GetParent,
                                ),
                                _,
                            )
                            | Expr::ApiCall(
                                crate::call_modules::CallModule::NodeMethod(
                                    crate::structs::engine_registry::NodeMethodRef::GetChildByName,
                                ),
                                _,
                            )
                            | Expr::Cast(_, Type::Node(_)) => return true,
                            Expr::Cast(_, Type::Custom(name)) => return is_node_type(name),
                            _ => {}
                        }
                    }
                }

                // Check script-level variable
                if let Some(script_var) = script.variables.iter().find(|v| v.name == *var_name) {
                    let declared_is_node = script_var
                        .typ
                        .as_ref()
                        .map(|t| matches!(t, Type::Node(_) | Type::DynNode))
                        .unwrap_or(false);

                    if declared_is_node {
                        return true;
                    }

                    if let Some(val) = &script_var.value {
                        let inferred_type = script.infer_expr_type(&val.expr, current_func);
                        let inferred_is_node = inferred_type
                            .as_ref()
                            .map(|t| matches!(t, Type::Node(_) | Type::DynNode))
                            .unwrap_or(false);

                        if inferred_is_node {
                            return true;
                        }

                        match &val.expr {
                            Expr::StructNew(ty, _) => return is_node_type(ty),
                            Expr::Cast(_, Type::Node(_)) => return true,
                            Expr::Cast(_, Type::Custom(name)) => return is_node_type(name),
                            _ => {}
                        }
                    }
                }
            } else {
                // Check script-level variable
                if let Some(script_var) = script.variables.iter().find(|v| v.name == *var_name) {
                    let declared_is_node = script_var
                        .typ
                        .as_ref()
                        .map(|t| matches!(t, Type::Node(_) | Type::DynNode))
                        .unwrap_or(false);

                    if declared_is_node {
                        return true;
                    }

                    if let Some(val) = &script_var.value {
                        let inferred_type = script.infer_expr_type(&val.expr, current_func);
                        let inferred_is_node = inferred_type
                            .as_ref()
                            .map(|t| matches!(t, Type::Node(_) | Type::DynNode))
                            .unwrap_or(false);

                        if inferred_is_node {
                            return true;
                        }

                        match &val.expr {
                            Expr::StructNew(ty, _) => return is_node_type(ty),
                            Expr::Cast(_, Type::Node(_)) => return true,
                            Expr::Cast(_, Type::Custom(name)) => return is_node_type(name),
                            _ => {}
                        }
                    }
                }
            }
            false
        }

        // When do we print node type (get_type) vs the value?
        // Only when we KNOW the argument is a node: self (SelfAccess) or a variable typed Node/DynNode (e.g. c_par).
        // c_par::test_var and c_par::[b] are expressions that resolve to a value that ISN'T a node (script var);
        // we only use the node's id to fetch that value, so we print the result (temp_api_var), never get_type.
        let format_str = if args.len() == 1 {
            let arg_expr = args.get(0);

            if let Some(arg_expr) = arg_expr {
                if matches!(arg_expr, Expr::Literal(_)) {
                    joined
                } else {
                    // Only get_type for bare node refs: SelfAccess or Ident of a variable we KNOW is Node/DynNode.
                    // MemberAccess (c_par::test_var) and Index (c_par::[b]) resolve to a non-node value → print value.
                    let is_bare_node_ref = match arg_expr {
                        Expr::SelfAccess => true,
                        Expr::Ident(var_name) => {
                            let var_typ = if let Some(func) = current_func {
                                func.locals
                                    .iter()
                                    .find(|v| v.name == *var_name)
                                    .and_then(|v| v.typ.as_ref())
                                    .or_else(|| {
                                        find_variable_in_body(var_name, &func.body)
                                            .and_then(|v| v.typ.as_ref())
                                    })
                            } else {
                                script
                                    .variables
                                    .iter()
                                    .find(|v| v.name == *var_name)
                                    .and_then(|v| v.typ.as_ref())
                            };
                            match var_typ {
                                Some(t) if matches!(t, Type::Node(_) | Type::DynNode) => true,
                                _ => check_if_node_var(var_name, script, current_func),
                            }
                        }
                        _ => false, // MemberAccess, Index, Call, etc. → expression is not a node, print value
                    };
                    let node_id_expr = args_strs
                        .get(0)
                        .cloned()
                        .unwrap_or_else(|| "NodeID::nil()".to_string());

                    if is_bare_node_ref {
                        // This is a node - convert to print its type instead of UUID
                        // api.get_type() requires &mut self, so we must extract it to a temp variable
                        // Return a special marker that codegen will detect and extract
                        format!("__EXTRACT_NODE_TYPE__({})", node_id_expr)
                    } else {
                        // Not a node - use normal logic
                        // Optimize: remove unnecessary clones and String::from() wrappers
                        let optimized = {
                            let trimmed = joined.trim();
                            // Remove .clone() from the end if present
                            let without_clone = if trimmed.ends_with(".clone()") {
                                &trimmed[..trimmed.len() - ".clone()".len()]
                            } else {
                                trimmed
                            };

                            // Remove String::from() wrapper for string literals
                            if without_clone.starts_with("String::from(")
                                && without_clone.ends_with(')')
                            {
                                let inner = &without_clone
                                    ["String::from(".len()..without_clone.len() - 1]
                                    .trim();
                                if inner.starts_with('"') && inner.ends_with('"') {
                                    // Use the string literal directly (optimization: avoid String::from allocation)
                                    inner.to_string()
                                } else {
                                    without_clone.to_string()
                                }
                            } else if without_clone.starts_with('"') && without_clone.ends_with('"')
                            {
                                // It's already a string literal, use it directly
                                without_clone.to_string()
                            } else {
                                // For variables or format!() expressions, use as-is (they already produce Display types)
                                without_clone.to_string()
                            }
                        };
                        optimized
                    }
                }
            } else {
                joined
            }
        } else {
            // Multiple arguments - check each one individually for nodes
            // Process each argument to see if it's a node and wrap with __EXTRACT_NODE_TYPE__
            // Also optimize arguments to remove unnecessary clones
            // Ensure args_strs has the same length as args - if not, regenerate it
            let args_strs = if args_strs.len() != args.len() {
                // Regenerate args_strs to match args length
                generate_rust_args(args, script, false, current_func, None)
            } else {
                args_strs
            };

            let processed_args: Vec<String> = args
                .iter()
                .zip(args_strs.iter())
                .map(|(arg_expr, arg_str)| {
                    // Optimize the argument string to remove unnecessary clones
                    let optimized_arg = {
                        // Remove .clone() from the end if present
                        let trimmed = arg_str.trim();
                        if trimmed.ends_with(".clone()") {
                            &trimmed[..trimmed.len() - ".clone()".len()]
                        } else {
                            trimmed
                        }
                    };

                    // Remove String::from() wrapper for string literals
                    let optimized_arg = if optimized_arg.starts_with("String::from(")
                        && optimized_arg.ends_with(')')
                    {
                        let inner =
                            &optimized_arg["String::from(".len()..optimized_arg.len() - 1].trim();
                        if inner.starts_with('"') && inner.ends_with('"') {
                            // It's a string literal, use it directly
                            inner.to_string()
                        } else {
                            optimized_arg.to_string()
                        }
                    } else {
                        optimized_arg.to_string()
                    };

                    if matches!(arg_expr, Expr::Literal(_)) {
                        optimized_arg
                    } else {
                        // Only get_type for bare node refs: SelfAccess or Ident we KNOW is Node/DynNode.
                        let is_bare_node_ref = match arg_expr {
                            Expr::SelfAccess => true,
                            Expr::Ident(var_name) => {
                                let var_typ = if let Some(func) = current_func {
                                    func.locals
                                        .iter()
                                        .find(|v| v.name == *var_name)
                                        .and_then(|v| v.typ.as_ref())
                                        .or_else(|| {
                                            find_variable_in_body(var_name, &func.body)
                                                .and_then(|v| v.typ.as_ref())
                                        })
                                } else {
                                    script
                                        .variables
                                        .iter()
                                        .find(|v| v.name == *var_name)
                                        .and_then(|v| v.typ.as_ref())
                                };
                                match var_typ {
                                    Some(t) if matches!(t, Type::Node(_) | Type::DynNode) => true,
                                    _ => check_if_node_var(var_name, script, current_func),
                                }
                            }
                            _ => false,
                        };

                        if is_bare_node_ref {
                            format!("__EXTRACT_NODE_TYPE__({})", optimized_arg)
                        } else {
                            optimized_arg
                        }
                    }
                })
                .collect();

            // Build format string with processed arguments
            // Ensure we have arguments - if processed_args is empty, something went wrong
            if processed_args.is_empty() {
                // Fallback: use original args_strs if processed_args is empty
                if args_strs.is_empty() && !args.is_empty() {
                    // Last resort: generate from args directly
                    let direct_args: Vec<String> = args
                        .iter()
                        .map(|a| a.to_rust(false, script, None, current_func, None))
                        .collect();
                    if !direct_args.is_empty() {
                        format!(
                            "format!(\"{}\", {})",
                            (0..direct_args.len())
                                .map(|_| "{}")
                                .collect::<Vec<_>>()
                                .join(" "),
                            direct_args.join(", "),
                        )
                    } else {
                        // Truly no arguments - use empty string
                        "\"\"".into()
                    }
                } else if !args_strs.is_empty() {
                    format!(
                        "format!(\"{}\", {})",
                        (0..args_strs.len())
                            .map(|_| "{}")
                            .collect::<Vec<_>>()
                            .join(" "),
                        args_strs.join(", "),
                    )
                } else {
                    // No arguments at all
                    "\"\"".into()
                }
            } else {
                format!(
                    "format!(\"{}\", {})",
                    (0..processed_args.len())
                        .map(|_| "{}")
                        .collect::<Vec<_>>()
                        .join(" "),
                    processed_args.join(", "),
                )
            }
        };

        // Assign format_str to arg for further processing
        let arg = format_str;

        // Check if arg contains the extraction marker for node type
        // This can be either a direct marker or inside a format!() string
        let (final_arg, temp_decl) = if arg.starts_with("__EXTRACT_NODE_TYPE__(") {
            // Single argument case - direct marker
            let node_id = arg
                .strip_prefix("__EXTRACT_NODE_TYPE__(")
                .and_then(|s: &str| s.strip_suffix(")"))
                .unwrap_or("NodeID::nil()");
            let temp_var = "node_type";
            let decl = format!("let {} = api.get_type({});", temp_var, node_id);
            (format!("format!(\"{{:?}}\", {})", temp_var), Some(decl))
        } else if arg.contains("__EXTRACT_NODE_TYPE__(") {
            // Multiple arguments case - marker inside format!() string
            // Extract all __EXTRACT_NODE_TYPE__(...) markers from the string
            let mut temp_decls = Vec::new();
            let mut counter = 0;
            let mut result = String::new();
            let mut last_pos = 0;
            let arg_chars: Vec<char> = arg.chars().collect();

            // Find and replace all markers
            let mut i = 0;
            while i < arg_chars.len() {
                // Check if we found the start of a marker
                if i + "__EXTRACT_NODE_TYPE__(".len() <= arg_chars.len() {
                    let marker_start: String = arg_chars[i..i + "__EXTRACT_NODE_TYPE__(".len()]
                        .iter()
                        .collect();
                    if marker_start == "__EXTRACT_NODE_TYPE__(" {
                        // Found a marker - find the matching closing paren
                        let mut paren_count = 1;
                        let mut j = i + "__EXTRACT_NODE_TYPE__(".len();
                        let mut node_id_end = j;

                        while j < arg_chars.len() && paren_count > 0 {
                            if arg_chars[j] == '(' {
                                paren_count += 1;
                            } else if arg_chars[j] == ')' {
                                paren_count -= 1;
                                if paren_count == 0 {
                                    node_id_end = j;
                                }
                            }
                            j += 1;
                        }

                        // Extract node_id
                        let node_id: String = arg_chars
                            [i + "__EXTRACT_NODE_TYPE__(".len()..node_id_end]
                            .iter()
                            .collect();
                        let temp_var = format!("node_type_{}", counter);
                        counter += 1;

                        // Create the temp declaration
                        temp_decls.push(format!("let {} = api.get_type({});", temp_var, node_id));

                        // Add replacement
                        result.push_str(&arg[last_pos..i]);
                        result.push_str(&format!("format!(\"{{:?}}\", {})", temp_var));

                        // Skip past the marker
                        i = node_id_end + 1;
                        last_pos = i;
                        continue;
                    }
                }
                i += 1;
            }

            // Add remaining string
            result.push_str(&arg[last_pos..]);

            if !temp_decls.is_empty() {
                let decl = temp_decls.join(" ");
                (result, Some(decl))
            } else {
                (arg, None)
            }
        } else {
            (arg, None)
        };

        let line = match self {
            ConsoleApi::Log => format!("api.print({})", final_arg),
            ConsoleApi::Warn => format!("api.print_warn({})", final_arg),
            ConsoleApi::Error => format!("api.print_error({})", final_arg),
            ConsoleApi::Info => format!("api.print_info({})", final_arg),
        };

        // If we have a temp declaration, prepend it
        let final_line = if let Some(decl) = temp_decl {
            format!("{} {}", decl, line)
        } else {
            line
        };

        if script.verbose {
            final_line
        } else {
            format!("// [stripped for release] {}", final_line)
        }
    }
}

impl ModuleTypes for ConsoleApi {
    fn return_type(&self) -> Option<Type> {
        Some(Type::Void)
    }

    // Console methods take variadic arguments, so we don't specify param types
    fn param_types(&self) -> Option<Vec<Type>> {
        None // Console methods accept any number of arguments
    }

    /// Script-side parameter names (what PUP users see)
    /// Console methods take variadic arguments, so we don't specify names
    fn param_names(&self) -> Option<Vec<&'static str>> {
        None // Console methods accept any number of arguments
    }
}

// ===========================================================
// Input API Implementations
// ===========================================================

impl ModuleCodegen for InputApi {
    fn to_rust_prepared(
        &self,
        _args: &[Expr],
        args_strs: &[String],
        _script: &Script,
        _needs_self: bool,
        _current_func: Option<&Function>,
    ) -> String {
        match self {
            InputApi::GetAction => {
                let arg = args_strs.get(0).cloned().unwrap_or_else(|| "\"\"".into());
                format!("api.Input.get_action({})", arg)
            }
            InputApi::ControllerEnable => "api.Input.Controller.enable()".into(),
            InputApi::IsKeyPressed => {
                let arg = args_strs.get(0).cloned().unwrap_or_else(|| "\"\"".into());
                format!("api.Input.Keyboard.is_key_pressed({})", arg)
            }
            InputApi::GetTextInput => "api.Input.Keyboard.get_text_input()".into(),
            InputApi::ClearTextInput => "api.Input.Keyboard.clear_text_input()".into(),
            InputApi::IsButtonPressed => {
                let arg = args_strs.get(0).cloned().unwrap_or_else(|| "\"\"".into());
                format!("api.Input.Mouse.is_button_pressed({})", arg)
            }
            InputApi::GetMousePosition => "api.Input.Mouse.get_position()".into(),
            InputApi::GetMousePositionWorld => "api.Input.Mouse.get_position_world()".into(),
            InputApi::GetScrollDelta => "api.Input.Mouse.get_scroll_delta()".into(),
            InputApi::IsWheelUp => "api.Input.Mouse.is_wheel_up()".into(),
            InputApi::IsWheelDown => "api.Input.Mouse.is_wheel_down()".into(),
            InputApi::ScreenToWorld => {
                let camera_pos = args_strs
                    .get(0)
                    .cloned()
                    .unwrap_or_else(|| "Vector2::ZERO".into());
                let camera_rotation = args_strs.get(1).cloned().unwrap_or_else(|| "0.0".into());
                let camera_zoom = args_strs.get(2).cloned().unwrap_or_else(|| "1.0".into());
                let virtual_width = args_strs.get(3).cloned().unwrap_or_else(|| "1920.0".into());
                let virtual_height = args_strs.get(4).cloned().unwrap_or_else(|| "1080.0".into());
                let window_width = args_strs.get(5).cloned().unwrap_or_else(|| "1920.0".into());
                let window_height = args_strs.get(6).cloned().unwrap_or_else(|| "1080.0".into());
                format!(
                    "api.Input.Mouse.screen_to_world({}, {}, {}, {}, {}, {}, {})",
                    camera_pos,
                    camera_rotation,
                    camera_zoom,
                    virtual_width,
                    virtual_height,
                    window_width,
                    window_height
                )
            }
        }
    }
}

impl ModuleTypes for InputApi {
    fn return_type(&self) -> Option<Type> {
        use NumberKind::*;
        match self {
            InputApi::GetAction
            | InputApi::IsKeyPressed
            | InputApi::IsButtonPressed
            | InputApi::IsWheelUp
            | InputApi::IsWheelDown => Some(Type::Bool),
            InputApi::ControllerEnable => Some(Type::Bool),
            InputApi::GetTextInput => Some(Type::String),
            InputApi::GetMousePosition
            | InputApi::GetMousePositionWorld
            | InputApi::ScreenToWorld => Some(Type::EngineStruct(
                crate::engine_structs::EngineStruct::Vector2,
            )),
            InputApi::GetScrollDelta => Some(Type::Number(Float(32))),
            InputApi::ClearTextInput => Some(Type::Void),
        }
    }

    fn param_types(&self) -> Option<Vec<Type>> {
        use NumberKind::*;
        match self {
            InputApi::GetAction | InputApi::IsKeyPressed | InputApi::IsButtonPressed => {
                Some(vec![Type::String])
            }
            InputApi::ScreenToWorld => Some(vec![
                Type::EngineStruct(crate::engine_structs::EngineStruct::Vector2),
                Type::Number(Float(32)),
                Type::Number(Float(32)),
                Type::Number(Float(32)),
                Type::Number(Float(32)),
                Type::Number(Float(32)),
                Type::Number(Float(32)),
            ]),
            _ => None,
        }
    }

    /// Script-side parameter names (what PUP users see)
    fn param_names(&self) -> Option<Vec<&'static str>> {
        match self {
            InputApi::GetAction => Some(vec!["action_name"]),
            InputApi::IsKeyPressed => Some(vec!["key"]),
            InputApi::IsButtonPressed => Some(vec!["button"]),
            InputApi::ScreenToWorld => Some(vec![
                "screen_pos",
                "camera_rotation",
                "camera_zoom",
                "virtual_width",
                "virtual_height",
                "window_width",
                "window_height",
            ]),
            _ => None,
        }
    }
}

// ===========================================================
// Math API Implementations
// ===========================================================

impl ModuleCodegen for MathApi {
    fn to_rust_prepared(
        &self,
        _args: &[Expr],
        args_strs: &[String],
        _script: &Script,
        _needs_self: bool,
        _current_func: Option<&Function>,
    ) -> String {
        match self {
            MathApi::Random => "api.Math.random()".into(),
            MathApi::RandomRange => {
                let min = args_strs.get(0).cloned().unwrap_or_else(|| "0.0".into());
                let max = args_strs.get(1).cloned().unwrap_or_else(|| "1.0".into());
                format!("api.Math.random_range({}, {})", min, max)
            }
            MathApi::RandomInt => {
                let min = args_strs.get(0).cloned().unwrap_or_else(|| "0".into());
                let max = args_strs.get(1).cloned().unwrap_or_else(|| "1".into());
                format!("api.Math.random_int({}, {})", min, max)
            }
            MathApi::Lerp => {
                let a = args_strs.get(0).cloned().unwrap_or_else(|| "0.0".into());
                let b = args_strs.get(1).cloned().unwrap_or_else(|| "1.0".into());
                let t = args_strs.get(2).cloned().unwrap_or_else(|| "0.0".into());
                format!("api.Math.lerp({}, {}, {})", a, b, t)
            }
            MathApi::LerpVec2 => {
                let a = args_strs
                    .get(0)
                    .cloned()
                    .unwrap_or_else(|| "crate::Vector2::ZERO".into());
                let b = args_strs
                    .get(1)
                    .cloned()
                    .unwrap_or_else(|| "crate::Vector2::ONE".into());
                let t = args_strs.get(2).cloned().unwrap_or_else(|| "0.0".into());
                format!("api.Math.lerp_vec2({}, {}, {})", a, b, t)
            }
            MathApi::LerpVec3 => {
                let a = args_strs
                    .get(0)
                    .cloned()
                    .unwrap_or_else(|| "crate::Vector3::ZERO".into());
                let b = args_strs
                    .get(1)
                    .cloned()
                    .unwrap_or_else(|| "crate::Vector3::ONE".into());
                let t = args_strs.get(2).cloned().unwrap_or_else(|| "0.0".into());
                format!("api.Math.lerp_vec3({}, {}, {})", a, b, t)
            }
            MathApi::Slerp => {
                let a = args_strs
                    .get(0)
                    .cloned()
                    .unwrap_or_else(|| "crate::Quaternion::identity()".into());
                let b = args_strs
                    .get(1)
                    .cloned()
                    .unwrap_or_else(|| "crate::Quaternion::identity()".into());
                let t = args_strs.get(2).cloned().unwrap_or_else(|| "0.0".into());
                format!("api.Math.slerp({}, {}, {})", a, b, t)
            }
        }
    }
}

impl ModuleTypes for MathApi {
    fn return_type(&self) -> Option<Type> {
        use NumberKind::*;
        match self {
            MathApi::Random | MathApi::RandomRange => Some(Type::Number(Float(32))),
            MathApi::RandomInt => Some(Type::Number(Signed(32))),
            MathApi::Lerp => Some(Type::Number(Float(32))),
            MathApi::LerpVec2 => Some(Type::EngineStruct(
                crate::structs::engine_structs::EngineStruct::Vector2,
            )),
            MathApi::LerpVec3 => Some(Type::EngineStruct(
                crate::structs::engine_structs::EngineStruct::Vector3,
            )),
            MathApi::Slerp => Some(Type::EngineStruct(
                crate::structs::engine_structs::EngineStruct::Quaternion,
            )),
        }
    }

    fn param_types(&self) -> Option<Vec<Type>> {
        use NumberKind::*;
        match self {
            MathApi::Random => None,
            MathApi::RandomRange => Some(vec![Type::Number(Float(32)), Type::Number(Float(32))]),
            MathApi::RandomInt => Some(vec![Type::Number(Signed(32)), Type::Number(Signed(32))]),
            MathApi::Lerp => Some(vec![
                Type::Number(Float(32)),
                Type::Number(Float(32)),
                Type::Number(Float(32)),
            ]),
            MathApi::LerpVec2 => Some(vec![
                Type::EngineStruct(crate::structs::engine_structs::EngineStruct::Vector2),
                Type::EngineStruct(crate::structs::engine_structs::EngineStruct::Vector2),
                Type::Number(Float(32)),
            ]),
            MathApi::LerpVec3 => Some(vec![
                Type::EngineStruct(crate::structs::engine_structs::EngineStruct::Vector3),
                Type::EngineStruct(crate::structs::engine_structs::EngineStruct::Vector3),
                Type::Number(Float(32)),
            ]),
            MathApi::Slerp => Some(vec![
                Type::EngineStruct(crate::structs::engine_structs::EngineStruct::Quaternion),
                Type::EngineStruct(crate::structs::engine_structs::EngineStruct::Quaternion),
                Type::Number(Float(32)),
            ]),
        }
    }

    /// Script-side parameter names (what PUP users see)
    fn param_names(&self) -> Option<Vec<&'static str>> {
        match self {
            MathApi::Random => None,
            MathApi::RandomRange => Some(vec!["min", "max"]),
            MathApi::RandomInt => Some(vec!["min", "max"]),
            MathApi::Lerp | MathApi::LerpVec2 | MathApi::LerpVec3 | MathApi::Slerp => {
                Some(vec!["a", "b", "t"])
            }
        }
    }
}
