// ----------------------------------------------------------------
// Central router used by the parser to map syntax → semantic call
// ----------------------------------------------------------------

use crate::{
    api_modules::*,
    ast::{ContainerKind, Type},
};

pub struct PupAPI;

impl PupAPI {
    pub fn resolve(module: &str, func: &str) -> Option<ApiModule> {
        match module {
            PupJSON::NAME => PupJSON::resolve_method(func),
            PupTime::NAME => PupTime::resolve_method(func),
            PupOS::NAME => PupOS::resolve_method(func),
            PupConsole::NAME => PupConsole::resolve_method(func),
            PupScriptType::NAME => PupScriptType::resolve_method(func),
            PupSignal::NAME => PupSignal::resolve_method(func),
            PupInput::NAME => PupInput::resolve_method(func),

            PupArray::NAME => PupArray::resolve_method(func),
            PupMap::NAME => PupMap::resolve_method(func),
            PupInput::NAME => PupInput::resolve_method(func),
            _ => PupNodeSugar::resolve_method(func),
        }
    }
}

pub fn normalize_type_name(type_: &Type) -> &str {
    match type_ {
        Type::Container(ContainerKind::Array, _) => "Array",
        Type::Container(ContainerKind::Map, _) => "Map",
        Type::Object => "Object",
        Type::Custom(s) => s.as_str(),
        _ => "",
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
}

pub struct PupScriptType;
impl PupScriptType {
    pub const NAME: &'static str = "Script";

    pub fn resolve_method(method: &str) -> Option<ApiModule> {
        match method {
            "new" => Some(ApiModule::ScriptType(ScriptTypeApi::Instantiate)),
            _ => None,
        }
    }
}

pub struct PupNodeSugar;
impl PupNodeSugar {
    pub const NAME: &'static str = "NodeSugar";

    pub fn resolve_method(method: &str) -> Option<ApiModule> {
        match method {
            "get_var" => Some(ApiModule::NodeSugar(NodeSugarApi::GetVar)),
            "set_var" => Some(ApiModule::NodeSugar(NodeSugarApi::SetVar)),
            "get_node" => Some(ApiModule::NodeSugar(NodeSugarApi::GetChildByName)),
            "get_parent" => Some(ApiModule::NodeSugar(NodeSugarApi::GetParent)),
            "add_child" => Some(ApiModule::NodeSugar(NodeSugarApi::AddChild)),
            _ => None,
        }
    }
}

pub struct PupSignal;
impl PupSignal {
    pub const NAME: &'static str = "Signal";

    pub fn resolve_method(method: &str) -> Option<ApiModule> {
        match method {
            "new" => Some(ApiModule::Signal(SignalApi::New)),
            "connect" => Some(ApiModule::Signal(SignalApi::Connect)),
            "emit" => Some(ApiModule::Signal(SignalApi::Emit)),
            _ => None,
        }
    }
}

pub struct PupArray;
impl PupArray {
    pub const NAME: &'static str = "Array";
    pub fn resolve_method(method: &str) -> Option<ApiModule> {
        match method {
            "push" | "append" => Some(ApiModule::ArrayOp(ArrayApi::Push)),
            "insert" => Some(ApiModule::ArrayOp(ArrayApi::Insert)),
            "remove" => Some(ApiModule::ArrayOp(ArrayApi::Remove)),
            "pop" => Some(ApiModule::ArrayOp(ArrayApi::Pop)),
            "len" | "size" => Some(ApiModule::ArrayOp(ArrayApi::Len)),

            "new" => Some(ApiModule::ArrayOp(ArrayApi::New)),
            // Add more mappings here!
            _ => None,
        }
    }
}

pub struct PupMap;
impl PupMap {
    pub const NAME: &'static str = "Map";

    pub fn resolve_method(method: &str) -> Option<ApiModule> {
        match method {
            "insert" => Some(ApiModule::MapOp(MapApi::Insert)),
            "remove" => Some(ApiModule::MapOp(MapApi::Remove)),
            "get" => Some(ApiModule::MapOp(MapApi::Get)),
            "contains" | "contains_key" => Some(ApiModule::MapOp(MapApi::Contains)),
            "len" | "size" => Some(ApiModule::MapOp(MapApi::Len)),
            "clear" => Some(ApiModule::MapOp(MapApi::Clear)),
            "new" => Some(ApiModule::MapOp(MapApi::New)),
            _ => None,
        }
    }
}

pub struct PupInput;
impl PupInput {
    pub const NAME: &'static str = "Input";

    pub fn resolve_method(method: &str) -> Option<ApiModule> {
        match method {
            // Actions
            "get_action" => Some(ApiModule::Input(InputApi::GetAction)),

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
}
