use crate::api_modules::*;

// ---------------------------------------------------------------------
// Central router: maps *TypeScript syntax tokens* to engine semantic API calls
// ---------------------------------------------------------------------
pub struct TypeScriptAPI;

impl TypeScriptAPI {
    pub fn resolve(module: &str, func: &str) -> Option<ApiModule> {
        // Match case-insensitively for console so "console".log works
        let module_key = if module.eq_ignore_ascii_case("console") {
            TypeScriptConsole::NAME
        } else {
            module
        };
        match module_key {
            TypeScriptJSON::NAME => TypeScriptJSON::resolve_method(func),
            TypeScriptTime::NAME => TypeScriptTime::resolve_method(func),
            TypeScriptOS::NAME => TypeScriptOS::resolve_method(func),
            TypeScriptConsole::NAME => TypeScriptConsole::resolve_method(func),
            // ScriptType instantiation is handled through node methods, not as an API module
            // TypeScriptScriptType::NAME => TypeScriptScriptType::resolve_method(func),
            TypeScriptInput::NAME => TypeScriptInput::resolve_method(func),
            TypeScriptMath::NAME => TypeScriptMath::resolve_method(func),
            _ => None,
        }
    }
}

// ---------------------------------------------------------------
// TypeScript API naming conventions
// ---------------------------------------------------------------

pub struct TypeScriptJSON;
impl TypeScriptJSON {
    pub const NAME: &'static str = "JSON";

    pub fn resolve_method(method: &str) -> Option<ApiModule> {
        match method {
            "parse" => Some(ApiModule::JSON(JSONApi::Parse)),
            "stringify" => Some(ApiModule::JSON(JSONApi::Stringify)),
            _ => None,
        }
    }
}

pub struct TypeScriptTime;
impl TypeScriptTime {
    pub const NAME: &'static str = "Time";

    pub fn resolve_method(method: &str) -> Option<ApiModule> {
        match method {
            "getDeltaTime" | "getDelta" => Some(ApiModule::Time(TimeApi::DeltaTime)),
            "sleep" | "sleepMsec" => Some(ApiModule::Time(TimeApi::SleepMsec)),
            "now" | "getUnixMsec" => Some(ApiModule::Time(TimeApi::GetUnixMsec)),
            _ => None,
        }
    }
}

pub struct TypeScriptOS;
impl TypeScriptOS {
    pub const NAME: &'static str = "OS";

    pub fn resolve_method(method: &str) -> Option<ApiModule> {
        match method {
            "getEnv" | "getEnvironmentVariable" => Some(ApiModule::OS(OSApi::GetEnv)),
            "getPlatform" | "getPlatformName" => Some(ApiModule::OS(OSApi::GetPlatformName)),
            _ => None,
        }
    }
}

pub struct TypeScriptConsole;
impl TypeScriptConsole {
    pub const NAME: &'static str = "Console";

    pub fn resolve_method(method: &str) -> Option<ApiModule> {
        match method {
            "log" => Some(ApiModule::Console(ConsoleApi::Log)),
            "warn" => Some(ApiModule::Console(ConsoleApi::Warn)),
            "error" => Some(ApiModule::Console(ConsoleApi::Error)),
            "info" => Some(ApiModule::Console(ConsoleApi::Info)),
            _ => None,
        }
    }
}


pub struct TypeScriptInput;
impl TypeScriptInput {
    pub const NAME: &'static str = "Input";

    pub fn resolve_method(method: &str) -> Option<ApiModule> {
        match method {
            // Actions
            "getAction" | "get_action" => Some(ApiModule::Input(InputApi::GetAction)),

            // Controller
            "controllerEnable" | "controller_enable" | "enableController" | "enable_controller" => {
                Some(ApiModule::Input(InputApi::ControllerEnable))
            }

            // Keyboard
            "isKeyPressed" | "is_key_pressed" | "getKeyPressed" => {
                Some(ApiModule::Input(InputApi::IsKeyPressed))
            }
            "getTextInput" | "get_text_input" => Some(ApiModule::Input(InputApi::GetTextInput)),
            "clearTextInput" | "clear_text_input" => {
                Some(ApiModule::Input(InputApi::ClearTextInput))
            }

            // Mouse
            "isButtonPressed" | "is_button_pressed" | "isMouseButtonPressed" => {
                Some(ApiModule::Input(InputApi::IsButtonPressed))
            }
            "getMousePosition" | "get_mouse_position" | "getMousePos" => {
                Some(ApiModule::Input(InputApi::GetMousePosition))
            }
            "getMousePositionWorld" | "get_mouse_position_world" | "getMousePosWorld" => {
                Some(ApiModule::Input(InputApi::GetMousePositionWorld))
            }
            "getScrollDelta" | "get_scroll_delta" | "getScroll" => {
                Some(ApiModule::Input(InputApi::GetScrollDelta))
            }
            "isWheelUp" | "is_wheel_up" => Some(ApiModule::Input(InputApi::IsWheelUp)),
            "isWheelDown" | "is_wheel_down" => Some(ApiModule::Input(InputApi::IsWheelDown)),
            "screenToWorld" | "screen_to_world" => Some(ApiModule::Input(InputApi::ScreenToWorld)),
            _ => None,
        }
    }
}

pub struct TypeScriptMath;
impl TypeScriptMath {
    pub const NAME: &'static str = "Math";

    pub fn resolve_method(method: &str) -> Option<ApiModule> {
        match method {
            "random" => Some(ApiModule::Math(MathApi::Random)),
            "randomRange" | "random_range" => Some(ApiModule::Math(MathApi::RandomRange)),
            "randomInt" | "random_int" => Some(ApiModule::Math(MathApi::RandomInt)),
            "lerp" => Some(ApiModule::Math(MathApi::Lerp)),
            "lerpVec2" | "lerp_vec2" => Some(ApiModule::Math(MathApi::LerpVec2)),
            "lerpVec3" | "lerp_vec3" => Some(ApiModule::Math(MathApi::LerpVec3)),
            "slerp" => Some(ApiModule::Math(MathApi::Slerp)),
            _ => None,
        }
    }

    pub fn get_all_method_names() -> Vec<&'static str> {
        vec![
            "random",
            "randomRange",
            "random_range",
            "randomInt",
            "random_int",
            "lerp",
            "lerpVec2",
            "lerp_vec2",
            "lerpVec3",
            "lerp_vec3",
            "slerp",
        ]
    }
}
