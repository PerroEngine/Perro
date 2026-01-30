// ----------------------------------------------------------------
// Module APIs - Global utility functions
// These are different from Resource APIs (types/resources that can be instantiated)
// ----------------------------------------------------------------

use crate::api_modules::*;

pub struct PupAPI;

impl PupAPI {
    pub fn resolve(module: &str, func: &str) -> Option<ApiModule> {
        match module {
            PupJSON::NAME => PupJSON::resolve_method(func),
            PupTime::NAME => PupTime::resolve_method(func),
            PupOS::NAME => PupOS::resolve_method(func),
            PupConsole::NAME => PupConsole::resolve_method(func),
            PupInput::NAME => PupInput::resolve_method(func),
            PupMath::NAME => PupMath::resolve_method(func),
            _ => None,
        }
    }

    /// Get all available module API names
    pub fn get_all_module_names() -> Vec<&'static str> {
        vec![
            PupJSON::NAME,
            PupTime::NAME,
            PupOS::NAME,
            PupConsole::NAME,
            PupInput::NAME,
            PupMath::NAME,
        ]
    }

    /// Check if a name is a valid module API name
    pub fn is_module_name(name: &str) -> bool {
        Self::get_all_module_names().contains(&name)
    }

    /// Get all method names for a given module name
    pub fn get_method_names_for_module(module_name: &str) -> Vec<&'static str> {
        match module_name {
            PupJSON::NAME => PupJSON::get_all_method_names(),
            PupTime::NAME => PupTime::get_all_method_names(),
            PupOS::NAME => PupOS::get_all_method_names(),
            PupConsole::NAME => PupConsole::get_all_method_names(),
            PupInput::NAME => PupInput::get_all_method_names(),
            PupMath::NAME => PupMath::get_all_method_names(),
            _ => Vec::new(),
        }
    }
}

pub struct PupJSON;
impl PupJSON {
    pub const NAME: &'static str = "JSON";

    pub fn resolve_method(method: &str) -> Option<ApiModule> {
        match method {
            "parse" => Some(ApiModule::JSON(JSONApi::Parse)),
            "stringify" => Some(ApiModule::JSON(JSONApi::Stringify)),
            _ => None,
        }
    }

    /// Returns all method names available for this API module
    pub fn get_all_method_names() -> Vec<&'static str> {
        vec!["parse", "stringify"]
    }
}

pub struct PupTime;
impl PupTime {
    pub const NAME: &'static str = "Time";

    pub fn resolve_method(method: &str) -> Option<ApiModule> {
        match method {
            "get_delta" => Some(ApiModule::Time(TimeApi::DeltaTime)),
            "sleep_msec" => Some(ApiModule::Time(TimeApi::SleepMsec)),
            "get_unix_time_msec" => Some(ApiModule::Time(TimeApi::GetUnixMsec)),
            _ => None,
        }
    }

    /// Returns all method names available for this API module
    pub fn get_all_method_names() -> Vec<&'static str> {
        vec!["get_delta", "sleep_msec", "get_unix_time_msec"]
    }
}

pub struct PupOS;
impl PupOS {
    pub const NAME: &'static str = "OS";

    pub fn resolve_method(method: &str) -> Option<ApiModule> {
        match method {
            "get_env" => Some(ApiModule::OS(OSApi::GetEnv)),
            "get_platform_name" => Some(ApiModule::OS(OSApi::GetPlatformName)),
            _ => None,
        }
    }

    pub fn get_all_method_names() -> Vec<&'static str> {
        vec!["get_env", "get_platform_name"]
    }
}

pub struct PupConsole;
impl PupConsole {
    pub const NAME: &'static str = "Console";

    pub fn resolve_method(method: &str) -> Option<ApiModule> {
        match method {
            "print" | "log" => Some(ApiModule::Console(ConsoleApi::Log)),
            "warn" | "print_warn" => Some(ApiModule::Console(ConsoleApi::Warn)),
            "error" | "print_error" => Some(ApiModule::Console(ConsoleApi::Error)),
            "info" | "print_info" => Some(ApiModule::Console(ConsoleApi::Info)),
            _ => None,
        }
    }

    pub fn get_all_method_names() -> Vec<&'static str> {
        vec!["print", "log", "warn", "error", "info"]
    }
}

pub struct PupInput;
impl PupInput {
    pub const NAME: &'static str = "Input";

    pub fn resolve_method(method: &str) -> Option<ApiModule> {
        match method {
            // Actions
            "get_action" => Some(ApiModule::Input(InputApi::GetAction)),

            // Controller
            "controller_enable" | "enable_controller" => {
                Some(ApiModule::Input(InputApi::ControllerEnable))
            }

            // Keyboard
            "is_key_pressed" | "get_key_pressed" => Some(ApiModule::Input(InputApi::IsKeyPressed)),
            "get_text_input" => Some(ApiModule::Input(InputApi::GetTextInput)),
            "clear_text_input" => Some(ApiModule::Input(InputApi::ClearTextInput)),

            // Mouse
            "is_button_pressed" | "is_mouse_button_pressed" => {
                Some(ApiModule::Input(InputApi::IsButtonPressed))
            }
            "get_mouse_position" | "get_mouse_pos" => {
                Some(ApiModule::Input(InputApi::GetMousePosition))
            }
            "get_mouse_position_world" | "get_mouse_pos_world" => {
                Some(ApiModule::Input(InputApi::GetMousePositionWorld))
            }
            "get_scroll_delta" | "get_scroll" => Some(ApiModule::Input(InputApi::GetScrollDelta)),
            "is_wheel_up" => Some(ApiModule::Input(InputApi::IsWheelUp)),
            "is_wheel_down" => Some(ApiModule::Input(InputApi::IsWheelDown)),
            "screen_to_world" => Some(ApiModule::Input(InputApi::ScreenToWorld)),
            _ => None,
        }
    }

    pub fn get_all_method_names() -> Vec<&'static str> {
        vec![
            "get_action",
            "controller_enable",
            "enable_controller",
            "is_key_pressed",
            "get_key_pressed",
            "get_text_input",
            "clear_text_input",
            "is_button_pressed",
            "is_mouse_button_pressed",
            "get_mouse_position",
            "get_mouse_pos",
            "get_mouse_position_world",
            "get_mouse_pos_world",
            "get_scroll_delta",
            "get_scroll",
            "is_wheel_up",
            "is_wheel_down",
            "screen_to_world",
        ]
    }
}

pub struct PupMath;
impl PupMath {
    pub const NAME: &'static str = "Math";

    pub fn resolve_method(method: &str) -> Option<ApiModule> {
        match method {
            "random" => Some(ApiModule::Math(MathApi::Random)),
            "random_range" => Some(ApiModule::Math(MathApi::RandomRange)),
            "random_int" => Some(ApiModule::Math(MathApi::RandomInt)),
            "lerp" => Some(ApiModule::Math(MathApi::Lerp)),
            "lerp_vec2" => Some(ApiModule::Math(MathApi::LerpVec2)),
            "lerp_vec3" => Some(ApiModule::Math(MathApi::LerpVec3)),
            "slerp" => Some(ApiModule::Math(MathApi::Slerp)),
            _ => None,
        }
    }

    pub fn get_all_method_names() -> Vec<&'static str> {
        vec![
            "random",
            "random_range",
            "random_int",
            "lerp",
            "lerp_vec2",
            "lerp_vec3",
            "slerp",
        ]
    }
}

/// Normalize a Type to a string name that can be used to resolve APIs
/// Returns an empty string if the type doesn't map to any API module
pub fn normalize_type_name(typ: &crate::ast::Type) -> String {
    use crate::ast::Type;
    use crate::structs::engine_structs::EngineStruct;
    match typ {
        Type::Signal => "Signal".to_string(),
        Type::Container(crate::ast::ContainerKind::Array, _) => "Array".to_string(),
        Type::Container(crate::ast::ContainerKind::Map, _) => "Map".to_string(),
        Type::EngineStruct(es) => match es {
            EngineStruct::Texture => "Texture".to_string(),
            EngineStruct::Shape2D => "Shape2D".to_string(),
            EngineStruct::Quaternion => "Quaternion".to_string(),
            _ => String::new(),
        },
        Type::Custom(name) => {
            // Check if it matches any module API names
            match name.as_str() {
                "JSON" | "json" => "JSON".to_string(),
                "Time" | "time" => "Time".to_string(),
                "OS" | "os" => "OS".to_string(),
                "Console" | "console" => "Console".to_string(),
                "Input" | "input" => "Input".to_string(),
                "Math" | "math" => "Math".to_string(),
                _ => String::new(),
            }
        }
        _ => String::new(),
    }
}
