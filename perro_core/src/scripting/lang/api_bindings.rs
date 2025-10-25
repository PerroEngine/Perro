use crate::lang::{ast::*, ast_modules::*};


impl ApiModule {
    pub fn to_rust(&self, args: &[Expr], script: &Script, needs_self: bool) -> String {
        match self {
            ApiModule::JSON(api_fn) => api_fn.to_rust(args, script, needs_self),
            ApiModule::Time(api_fn) => api_fn.to_rust(args, script, needs_self),
            ApiModule::OS(api_fn) => api_fn.to_rust(args, script, needs_self),
            ApiModule::Console(api_fn) => api_fn.to_rust(args, script, needs_self),
            ApiModule::ScriptType(api_fn) => api_fn.to_rust(args, script, needs_self),
            ApiModule::NodeSugar(api_fn) => api_fn.to_rust(args, script, needs_self),
        }
    }
}


impl JSONApi {
    pub fn to_rust(&self, args: &[Expr], script: &Script, needs_self: bool) -> String {
        match self {
            JSONApi::Parse => {
                // JSON.parse(string)
                // If first arg is a Literal::String, we emit "&\"literal\"" instead of .to_string()
                let arg_expr = args.get(0);
                let arg_code = match arg_expr {
                    Some(Expr::Literal(Literal::String(s))) => format!("\"{}\"", s),
                    Some(expr) => expr.to_rust(needs_self, script, None),
                    None => "\"\".to_string()".to_string(),
                };
                format!("api.JSON.parse({})", arg_code)
            }

            JSONApi::Stringify => {
                let arg = args
                    .get(0)
                    .map(|a| a.to_rust(needs_self, script, None))
                    .unwrap_or_default();
                format!("api.JSON.stringify({})", arg)
            }
        }
    }
}

impl TimeApi {
    pub fn to_rust(&self, args: &[Expr], _script: &Script, _needs_self: bool) -> String {
        match self {
            TimeApi::GetUnixMsec => "api.Time.get_unix_time_msec()".to_string(),
            TimeApi::SleepMsec => {
                let arg = args.get(0).map(|a| a.to_rust(_needs_self, _script, None)).unwrap_or_default();
                format!("api.Time.sleep_msec({})", arg)
            }
        }
    }
}

impl OSApi {
    pub fn to_rust(&self, args: &[Expr], _script: &Script, _needs_self: bool) -> String {
        match self {
            OSApi::GetPlatformName => "api.OS.get_platform_name()".to_string(),
            OSApi::GetEnv => {
                let arg = args.get(0).map(|a| a.to_rust(_needs_self, _script, None)).unwrap_or_default();
                format!("api.OS.getenv({})", arg)
            }
        }
    }
}

impl ConsoleApi {
    pub fn to_rust(&self, args: &[Expr], script: &Script, needs_self: bool) -> String {
        // Convert all args to Rust string expressions first.
        let args_str = args
            .iter()
            .map(|a| a.to_rust(needs_self, script, None))
            .collect::<Vec<_>>()
            .join(", ");

        match self {
            ConsoleApi::Log => {
                if args.is_empty() {
                    "api.print(\"\");".to_string()
                } else {
                    format!("api.print({});", args_str)
                }
            }

            ConsoleApi::Warn => {
                if args.is_empty() {
                    "api.print_warn(\"\");".to_string()
                } else {
                    format!("api.print_warn({});", args_str)
                }
            }

            ConsoleApi::Error => {
                if args.is_empty() {
                    "api.print_error(\"\");".to_string()
                } else {
                    format!("api.print_error({});", args_str)
                }
            }

            ConsoleApi::Info => {
                if args.is_empty() {
                    "api.print_info(\"\");".to_string()
                } else {
                    format!("api.print_info({});", args_str)
                }
            }
        }
    }
}

impl ScriptTypeApi {
    pub fn to_rust(&self, args: &[Expr], _script: &Script, _needs_self: bool) -> String {
        match self {
            ScriptTypeApi::Instantiate => {
                let arg = args
                    .get(0)
                    .map(|a| a.to_rust(_needs_self, _script, None))
                    .unwrap_or_default();
                format!("api.instantiate_script({})", arg)
            }
        }
    }
}

impl NodeSugarApi {
    pub fn to_rust(&self, args: &[Expr], _script: &Script, _needs_self: bool) -> String {
        match self {
            NodeSugarApi::GetVar => {
                // args[0] = node expression
                // args[1] = variable name (string literal)
                let node_expr = args
                    .get(0)
                    .map(|a| a.to_rust(_needs_self, _script, None))
                    .unwrap_or_default();
                let var_name = args
                    .get(1)
                    .map(|a| a.to_rust(_needs_self, _script, None))
                    .unwrap_or_default();

                format!("api.get_script_var(&{}.id, {})", node_expr, var_name)
            }

            NodeSugarApi::SetVar => {
                // args[0] = node expression
                // args[1] = variable name (string literal)
                // args[2] = new value
                let node_expr = args
                    .get(0)
                    .map(|a| a.to_rust(_needs_self, _script, None))
                    .unwrap_or_default();
                let var_name = args
                    .get(1)
                    .map(|a| a.to_rust(_needs_self, _script, None))
                    .unwrap_or_default();
                let new_value = args
                    .get(2)
                    .map(|a| a.to_rust(_needs_self, _script, None))
                    .unwrap_or_default();

                format!("api.set_script_var(&{}.id, {}, {})", node_expr, var_name, new_value)
            }
        }
    }
}
