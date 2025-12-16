#[derive(Debug, Clone)]
pub enum ApiModule {
    JSON(JSONApi),
    Time(TimeApi),
    OS(OSApi),
    Console(ConsoleApi),
    ScriptType(ScriptTypeApi),
    NodeSugar(NodeSugarApi),
    Signal(SignalApi),
    Input(InputApi),

    ArrayOp(ArrayApi),
    MapOp(MapApi),
}

#[derive(Debug, Clone)]
pub enum JSONApi {
    Parse,
    Stringify,
}

#[derive(Debug, Clone)]
pub enum TimeApi {
    DeltaTime,
    GetUnixMsec,
    SleepMsec,
}

#[derive(Debug, Clone)]
pub enum OSApi {
    GetPlatformName,
    GetEnv,
}

#[derive(Debug, Clone)]
pub enum ConsoleApi {
    Log,
    Warn,
    Error,
    Info,
}

#[derive(Debug, Clone)]
pub enum ScriptTypeApi {
    Instantiate,
}

#[derive(Debug, Clone)]
pub enum NodeSugarApi {
    GetVar,
    SetVar,
    GetChildByName, // For self.get_node("name") - finds child by name and returns ID
}

#[derive(Debug, Clone)]
pub enum SignalApi {
    New,
    Connect,
    Emit,
}

#[derive(Debug, Clone)]
pub enum ArrayApi {
    Push,
    Pop,
    Insert,
    Remove,
    Len,

    New,
}

#[derive(Debug, Clone)]
pub enum MapApi {
    Insert,
    Remove,
    Get,
    Contains,
    Len,
    Clear,

    New,
}

#[derive(Debug, Clone)]
pub enum InputApi {
    // Actions
    GetAction,

    // Keyboard
    IsKeyPressed,
    GetTextInput,
    ClearTextInput,

    // Mouse
    IsButtonPressed,
    GetMousePosition,
    GetMousePositionWorld,
    GetScrollDelta,
    IsWheelUp,
    IsWheelDown,
    ScreenToWorld,
}
