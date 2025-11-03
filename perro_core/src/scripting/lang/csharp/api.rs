use crate::lang::api_modules::*;


// ---------------------------------------------------------------------
// Central router: maps *C# syntax tokens* to engine semantic API calls
// ---------------------------------------------------------------------
pub struct CSharpAPI;

impl CSharpAPI {
    pub fn resolve(module: &str, func: &str) -> Option<ApiModule> {
        match module {
            CSharpJSON::NAME => CSharpJSON::resolve_method(func),
            CSharpTime::NAME => CSharpTime::resolve_method(func),
            CSharpOS::NAME   => CSharpOS::resolve_method(func),
            CSharpConsole::NAME => CSharpConsole::resolve_method(func),
            _ => None,
        }
    }
}

// ---------------------------------------------------------------
// Standard C# API naming conventions
// ---------------------------------------------------------------

pub struct CSharpJSON;
impl CSharpJSON {
    // C# developers would typically recognize "JsonConvert"
    pub const NAME: &'static str = "JsonConvert";

    pub fn resolve_method(method: &str) -> Option<ApiModule> {
        match method {
            "DeserializeObject" => Some(ApiModule::JSON(JSONApi::Parse)),
            "SerializeObject"   => Some(ApiModule::JSON(JSONApi::Stringify)),
            _ => None,
        }
    }
}

pub struct CSharpTime;
impl CSharpTime {
    pub const NAME: &'static str = "Time";

    pub fn resolve_method(method: &str) -> Option<ApiModule> {
        match method {
            "GetDeltaTime" => Some(ApiModule::Time(TimeApi::DeltaTime)),
            "Sleep" => Some(ApiModule::Time(TimeApi::SleepMsec)),
            "Now"   => Some(ApiModule::Time(TimeApi::GetUnixMsec)),
            _ => None,
        }
    }
}

pub struct CSharpOS;
impl CSharpOS {
    // maybe wrapped under a utility class e.g., "Environment"
    pub const NAME: &'static str = "OS";

    pub fn resolve_method(method: &str) -> Option<ApiModule> {
        match method {
            "GetEnvironmentVariable" => Some(ApiModule::OS(OSApi::GetEnv)),
            "GetPlatform"              => Some(ApiModule::OS(OSApi::GetPlatformName)),
            _ => None,
        }
    }
}

pub struct CSharpConsole;
impl CSharpConsole {
    pub const NAME: &'static str = "Console";

    pub fn resolve_method(method: &str) -> Option<ApiModule> {
        match method {
            "WriteLine"      => Some(ApiModule::Console(ConsoleApi::Log)),
            "Warn"   => Some(ApiModule::Console(ConsoleApi::Warn)),
            "Error"     => Some(ApiModule::Console(ConsoleApi::Error)),
            "WriteInfo"      => Some(ApiModule::Console(ConsoleApi::Info)),
            _ => None,
        }
    }
}