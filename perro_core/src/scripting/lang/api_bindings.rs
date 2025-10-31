use crate::lang::{ast::*, ast_modules::*};

/// ===========================================================
///  Shared API Traits — Codegen + Semantics
/// ===========================================================

/// Converts a generic API call into Rust source code output.
pub trait ApiCodegen {
    fn to_rust(
        &self,
        args: &[Expr],
        script: &Script,
        needs_self: bool,
        current_func: Option<&Function>,
    ) -> String;

/// Smart argument generator — automatically borrows values but
/// leaves string literals and `.as_str()` untouched.
/// Also prepends `self.` to struct field variables.
fn rust_args(
    &self,
    args: &[Expr],
    script: &Script,
    needs_self: bool,
    current_func: Option<&Function>,
) -> Vec<String> {
    args.iter()
        .map(|a| {
            let code = a.to_rust(needs_self, script, None, current_func);
            
            // Check if this is a simple identifier that's a struct field
            if let Expr::Ident(name) = a {
                if script.is_struct_field(name) {
                    return format!("self.{}", code);
                }
            }
            
            code
        })
        .collect()
}
}

/// Provides return‑type semantics for each API.
pub trait ApiSemantic {
    /// Returns what type this API call produces.
    fn return_type(&self) -> Option<Type>;
}

/// ===========================================================
///  Aggregator — routes both codegen + type semantics
/// ===========================================================

impl ApiModule {
    pub fn to_rust(
        &self,
        args: &[Expr],
        script: &Script,
        needs_self: bool,
        current_func: Option<&Function>,
    ) -> String {
        match self {
            ApiModule::JSON(api) => api.to_rust(args, script, needs_self, current_func),
            ApiModule::Time(api) => api.to_rust(args, script, needs_self, current_func),
            ApiModule::OS(api) => api.to_rust(args, script, needs_self, current_func),
            ApiModule::Console(api) => api.to_rust(args, script, needs_self, current_func),
            ApiModule::ScriptType(api) => api.to_rust(args, script, needs_self, current_func),
            ApiModule::NodeSugar(api) => api.to_rust(args, script, needs_self, current_func),
            ApiModule::Signal(api) => api.to_rust(args, script, needs_self, current_func)
        }
    }

    pub fn return_type(&self) -> Option<Type> {
        match self {
            ApiModule::JSON(api) => api.return_type(),
            ApiModule::Time(api) => api.return_type(),
            ApiModule::OS(api) => api.return_type(),
            ApiModule::Console(api) => api.return_type(),
            ApiModule::ScriptType(api) => api.return_type(),
            ApiModule::NodeSugar(api) => api.return_type(),
            ApiModule::Signal(api) => api.return_type()
        }
    }
}

/// ===========================================================
///  JSON API
/// ===========================================================

impl ApiCodegen for JSONApi {
    fn to_rust(
        &self,
        args: &[Expr],
        script: &Script,
        needs_self: bool,
        current_func: Option<&Function>,
    ) -> String {
        let args = self.rust_args(args, script, needs_self, current_func);
        match self {
            JSONApi::Parse => {
                let arg = args.get(0).cloned().unwrap_or_else(|| "\"\"".into());
                format!("api.JSON.parse(&{})", arg)
            }
            JSONApi::Stringify => {
                let arg = args.get(0).cloned().unwrap_or_else(|| "json!({})".into());
                format!("api.JSON.stringify(&{})", arg)
            }
        }
    }
}

impl ApiSemantic for JSONApi {
    fn return_type(&self) -> Option<Type> {
        match self {
            JSONApi::Parse => Some(Type::Custom("json".into())),
            JSONApi::Stringify => Some(Type::String),
        }
    }
}

/// ===========================================================
///  Time API
/// ===========================================================

impl ApiCodegen for TimeApi {
    fn to_rust(
        &self,
        args: &[Expr],
        script: &Script,
        needs_self: bool,
        current_func: Option<&Function>,
    ) -> String {
        let args = self.rust_args(args, script, needs_self, current_func);
        match self {
            TimeApi::DeltaTime => "api.Time.get_delta()".into(),
            TimeApi::GetUnixMsec => "api.Time.get_unix_time_msec()".into(),
            TimeApi::SleepMsec => {
                let arg = args.get(0).cloned().unwrap_or_else(|| "0".into());
                format!("api.Time.sleep_msec({})", arg)
            }
        }
    }
}

impl ApiSemantic for TimeApi {
    fn return_type(&self) -> Option<Type> {
        match self {
            TimeApi::DeltaTime => Some(Type::Number(NumberKind::Float(32))),
            TimeApi::GetUnixMsec => Some(Type::Number(NumberKind::Unsigned(64))),
            TimeApi::SleepMsec => Some(Type::Void),
        }
    }
}

/// ===========================================================
///  OS API
/// ===========================================================

impl ApiCodegen for OSApi {
    fn to_rust(
        &self,
        args: &[Expr],
        script: &Script,
        needs_self: bool,
        current_func: Option<&Function>,
    ) -> String {
        let args = self.rust_args(args, script, needs_self, current_func);
        match self {
            OSApi::GetPlatformName => "api.OS.get_platform_name()".into(),
            OSApi::GetEnv => {
                let arg = args.get(0).cloned().unwrap_or_else(|| "\"\"".into());
                format!("api.OS.getenv({})", arg)
            }
        }
    }
}

impl ApiSemantic for OSApi {
    fn return_type(&self) -> Option<Type> {
        match self {
            OSApi::GetPlatformName => Some(Type::String),
            OSApi::GetEnv => Some(Type::String),
        }
    }
}

/// ===========================================================
///  Console API
/// ===========================================================

impl ApiCodegen for ConsoleApi {
    fn to_rust(
        &self,
        args: &[Expr],
        script: &Script,
        needs_self: bool,
        current_func: Option<&Function>,
    ) -> String {
        let args = self.rust_args(args, script, needs_self, current_func);
        
        let joined = if args.len() <= 1 {
                args.get(0).cloned().unwrap_or("\"\"".into())
            } else {
                format!("format!(\"{}\", {})", 
                    (0..args.len()).map(|_i| "{}").collect::<Vec<_>>().join(" "), 
                    args.join(", ")
                )
            };

        match self {
            ConsoleApi::Log => format!("api.print(&{});", joined),
            ConsoleApi::Warn => format!("api.print_warn(&{});", joined),
            ConsoleApi::Error => format!("api.print_error(&{});", joined),
            ConsoleApi::Info => format!("api.print_info(&{});", joined),
        }
    }
}

impl ApiSemantic for ConsoleApi {
    fn return_type(&self) -> Option<Type> {
        Some(Type::Void)
    }
}

/// ===========================================================
///  ScriptType API
/// ===========================================================

impl ApiCodegen for ScriptTypeApi {
    fn to_rust(
        &self,
        args: &[Expr],
        script: &Script,
        needs_self: bool,
        current_func: Option<&Function>,
    ) -> String {
        let args = self.rust_args(args, script, needs_self, current_func);
        match self {
            ScriptTypeApi::Instantiate => {
                let arg = args.get(0).cloned().unwrap_or_else(|| "\"\"".into());
                format!("api.instantiate_script({})", arg)
            }
        }
    }
}

impl ApiSemantic for ScriptTypeApi {
    fn return_type(&self) -> Option<Type> {
        Some(Type::Script)
    }
}

/// ===========================================================
///  NodeSugar API
/// ===========================================================

impl ApiCodegen for NodeSugarApi {
    fn to_rust(
        &self,
        args: &[Expr],
        script: &Script,
        needs_self: bool,
        current_func: Option<&Function>,
    ) -> String {
        let args = self.rust_args(args, script, needs_self, current_func);
        match self {
            NodeSugarApi::GetVar => {
                let (node, name) = (args.get(0), args.get(1));
                format!(
                    "api.get_script_var(&{}.id, {})",
                    node.map(|s| s.as_str()).unwrap_or("self"),
                    name.map(|s| s.as_str()).unwrap_or("\"\"")
                )
            }
            NodeSugarApi::SetVar => {
                let (node, name, val) = (args.get(0), args.get(1), args.get(2));
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

impl ApiSemantic for NodeSugarApi {
    fn return_type(&self) -> Option<Type> {
        match self {
            NodeSugarApi::GetVar => Some(Type::Custom("Value".into())),
            NodeSugarApi::SetVar => Some(Type::Void),
        }
    }
}

impl ApiCodegen for SignalApi {
    fn to_rust(
        &self,
        args: &[Expr],
        script: &Script,
        needs_self: bool,
        current_func: Option<&Function>,
    ) -> String {
        let args = self.rust_args(args, script, needs_self, current_func);
        match self {
            SignalApi::New => {
                let arg = args.get(0).cloned().unwrap_or_else(|| "\"\"".into());
                // If it's already `String::from(`, leave it alone; otherwise wrap it
                if arg.starts_with("String::from(") || arg.starts_with('&') {
                    arg
                } else {
                    format!("String::from({})", arg)
                }
            }
           SignalApi::Connect => {
                let mut signal = args.get(0).cloned().unwrap_or_else(|| "\"\"".into());
                
                // Check if signal is a struct field variable
                if script.is_struct_field(&signal) {
                    signal = format!("self.{}", signal);
                }
                
                let mut node = args.get(1).cloned().unwrap_or_else(|| "self.node".into());
                if node == "self" { 
                    node = "self.node".into(); 
                }
                let func = args.get(2).cloned().unwrap_or_else(|| "\"\"".into());
                format!("api.connect_signal(&{}, {}.id, &{})", signal, node, func)
            }
           SignalApi::Emit => {
                let signal = args.get(0).cloned().unwrap_or_else(|| "\"\"".into());
                
                // If there are additional arguments, collect them into a Vec<Value>
                if args.len() > 1 {
                    let params: Vec<String> = args[1..]
                        .iter()
                        .map(|arg| format!("json!({})", arg))
                        .collect();
                    
                    format!(
                        "api.emit_signal(&{}, vec![{}])",
                        signal,
                        params.join(", ")
                    )
                } else {
                    // No parameters, pass empty vec
                    format!("api.emit_signal(&{}, vec![])", signal)
                }
            }
        }
    }
}

impl ApiSemantic for SignalApi {
        fn return_type(&self) -> Option<Type> {
        match self {
            SignalApi::New => Some(Type::Custom("Signal".into())),
            _ => Some(Type::Void),
        }
    }
}