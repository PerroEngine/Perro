use crate::lang::{ast::*, ast_modules::*};


impl ApiModule {
    pub fn to_rust(&self, args: &[Expr], script: &Script, needs_self: bool) -> String {
        match self {
            ApiModule::JSON(api_fn) => api_fn.to_rust(args, script, needs_self),
            ApiModule::Time(api_fn) => api_fn.to_rust(args, script, needs_self),
            ApiModule::OS(api_fn) => api_fn.to_rust(args, script, needs_self),
            ApiModule::Console(api_fn) => api_fn.to_rust(args, script, needs_self),
        }
    }
}


impl JSONApi {
    pub fn to_rust(&self, args: &[Expr], script: &Script, needs_self: bool) -> String {
        match self {
            JSONApi::Parse => {
                // JSON.parse(string)
                // If first arg is a literal string, emit serde_json::from_str(...)
                let arg = args.get(0).map(|a| a.to_rust(needs_self, script, None)).unwrap_or_default();
                format!("api.JSON.parse({})", arg)
            }
            JSONApi::Stringify => {
                let arg = args.get(0).map(|a| a.to_rust(needs_self, script, None)).unwrap_or_default();
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