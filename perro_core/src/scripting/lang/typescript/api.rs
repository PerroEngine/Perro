use crate::api_modules::*;

// ---------------------------------------------------------------------
// Central router: maps *TypeScript syntax tokens* to engine semantic API calls
// ---------------------------------------------------------------------
pub struct TypeScriptAPI;

impl TypeScriptAPI {
    pub fn resolve(module: &str, func: &str) -> Option<ApiModule> {
        match module {
            TypeScriptJSON::NAME => TypeScriptJSON::resolve_method(func),
            TypeScriptTime::NAME => TypeScriptTime::resolve_method(func),
            TypeScriptOS::NAME => TypeScriptOS::resolve_method(func),
            TypeScriptConsole::NAME => TypeScriptConsole::resolve_method(func),
            TypeScriptScriptType::NAME => TypeScriptScriptType::resolve_method(func),
            TypeScriptSignal::NAME => TypeScriptSignal::resolve_method(func),
            TypeScriptArray::NAME => TypeScriptArray::resolve_method(func),
            TypeScriptMap::NAME => TypeScriptMap::resolve_method(func),
            TypeScriptInput::NAME => TypeScriptInput::resolve_method(func),
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
    pub const NAME: &'static str = "console";

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

pub struct TypeScriptScriptType;
impl TypeScriptScriptType {
    pub const NAME: &'static str = "ScriptType";

    pub fn resolve_method(method: &str) -> Option<ApiModule> {
        match method {
            "instantiate" => Some(ApiModule::ScriptType(ScriptTypeApi::Instantiate)),
            _ => None,
        }
    }
}

pub struct TypeScriptSignal;
impl TypeScriptSignal {
    pub const NAME: &'static str = "Signal";

    pub fn resolve_method(method: &str) -> Option<ApiModule> {
        match method {
            "new" | "create" => Some(ApiModule::Signal(SignalApi::New)),
            "connect" => Some(ApiModule::Signal(SignalApi::Connect)),
            "emit" => Some(ApiModule::Signal(SignalApi::Emit)),
            "emitDeferred" | "emit_deferred" => Some(ApiModule::Signal(SignalApi::EmitDeferred)),
            _ => None,
        }
    }
}

pub struct TypeScriptArray;
impl TypeScriptArray {
    pub const NAME: &'static str = "Array";

    pub fn resolve_method(method: &str) -> Option<ApiModule> {
        match method {
            "push" => Some(ApiModule::ArrayOp(ArrayApi::Push)),
            "pop" => Some(ApiModule::ArrayOp(ArrayApi::Pop)),
            "insert" => Some(ApiModule::ArrayOp(ArrayApi::Insert)),
            "remove" => Some(ApiModule::ArrayOp(ArrayApi::Remove)),
            "length" | "len" => Some(ApiModule::ArrayOp(ArrayApi::Len)),
            "new" | "create" => Some(ApiModule::ArrayOp(ArrayApi::New)),
            _ => None,
        }
    }
}

pub struct TypeScriptMap;
impl TypeScriptMap {
    pub const NAME: &'static str = "Map";

    pub fn resolve_method(method: &str) -> Option<ApiModule> {
        match method {
            "set" | "insert" => Some(ApiModule::MapOp(MapApi::Insert)),
            "delete" | "remove" => Some(ApiModule::MapOp(MapApi::Remove)),
            "get" => Some(ApiModule::MapOp(MapApi::Get)),
            "has" | "contains" => Some(ApiModule::MapOp(MapApi::Contains)),
            "size" | "len" => Some(ApiModule::MapOp(MapApi::Len)),
            "clear" => Some(ApiModule::MapOp(MapApi::Clear)),
            "new" | "create" => Some(ApiModule::MapOp(MapApi::New)),
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
