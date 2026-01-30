use crate::api_modules::*;

// ---------------------------------------------------------------------
// Central router: maps *C# syntax tokens* to engine semantic API calls
// ---------------------------------------------------------------------
pub struct CSharpAPI;

impl CSharpAPI {
    pub fn resolve(module: &str, func: &str) -> Option<ApiModule> {
        match module {
            CSharpJSON::NAME => CSharpJSON::resolve_method(func),
            CSharpTime::NAME => CSharpTime::resolve_method(func),
            CSharpOS::NAME => CSharpOS::resolve_method(func),
            CSharpConsole::NAME => CSharpConsole::resolve_method(func),
            CSharpInput::NAME => CSharpInput::resolve_method(func),
            CSharpMath::NAME => CSharpMath::resolve_method(func),
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
            "SerializeObject" => Some(ApiModule::JSON(JSONApi::Stringify)),
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
            "Now" => Some(ApiModule::Time(TimeApi::GetUnixMsec)),
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
            "GetPlatform" => Some(ApiModule::OS(OSApi::GetPlatformName)),
            _ => None,
        }
    }
}

pub struct CSharpConsole;
impl CSharpConsole {
    pub const NAME: &'static str = "Console";

    pub fn resolve_method(method: &str) -> Option<ApiModule> {
        match method {
            "WriteLine" => Some(ApiModule::Console(ConsoleApi::Log)),
            "Warn" => Some(ApiModule::Console(ConsoleApi::Warn)),
            "Error" => Some(ApiModule::Console(ConsoleApi::Error)),
            "WriteInfo" => Some(ApiModule::Console(ConsoleApi::Info)),
            _ => None,
        }
    }
}

pub struct CSharpInput;
impl CSharpInput {
    pub const NAME: &'static str = "Input";

    pub fn resolve_method(method: &str) -> Option<ApiModule> {
        match method {
            // Actions
            "GetAction" => Some(ApiModule::Input(InputApi::GetAction)),

            // Controller
            "ControllerEnable" | "EnableController" => {
                Some(ApiModule::Input(InputApi::ControllerEnable))
            }

            // Keyboard
            "IsKeyPressed" | "GetKeyPressed" => Some(ApiModule::Input(InputApi::IsKeyPressed)),
            "GetTextInput" => Some(ApiModule::Input(InputApi::GetTextInput)),
            "ClearTextInput" => Some(ApiModule::Input(InputApi::ClearTextInput)),

            // Mouse
            "IsButtonPressed" | "IsMouseButtonPressed" => {
                Some(ApiModule::Input(InputApi::IsButtonPressed))
            }
            "GetMousePosition" | "GetMousePos" => {
                Some(ApiModule::Input(InputApi::GetMousePosition))
            }
            "GetMousePositionWorld" | "GetMousePosWorld" => {
                Some(ApiModule::Input(InputApi::GetMousePositionWorld))
            }
            "GetScrollDelta" | "GetScroll" => Some(ApiModule::Input(InputApi::GetScrollDelta)),
            "IsWheelUp" => Some(ApiModule::Input(InputApi::IsWheelUp)),
            "IsWheelDown" => Some(ApiModule::Input(InputApi::IsWheelDown)),
            "ScreenToWorld" => Some(ApiModule::Input(InputApi::ScreenToWorld)),
            _ => None,
        }
    }
}

pub struct CSharpMath;
impl CSharpMath {
    pub const NAME: &'static str = "Math";

    pub fn resolve_method(method: &str) -> Option<ApiModule> {
        match method {
            "Random" => Some(ApiModule::Math(MathApi::Random)),
            "RandomRange" => Some(ApiModule::Math(MathApi::RandomRange)),
            "RandomInt" => Some(ApiModule::Math(MathApi::RandomInt)),
            "Lerp" => Some(ApiModule::Math(MathApi::Lerp)),
            "LerpVec2" => Some(ApiModule::Math(MathApi::LerpVec2)),
            "LerpVec3" => Some(ApiModule::Math(MathApi::LerpVec3)),
            "Slerp" => Some(ApiModule::Math(MathApi::Slerp)),
            _ => None,
        }
    }

    pub fn get_all_method_names() -> Vec<&'static str> {
        vec![
            "Random",
            "RandomRange",
            "RandomInt",
            "Lerp",
            "LerpVec2",
            "LerpVec3",
            "Slerp",
        ]
    }
}
