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
    Texture(TextureApi),

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
    GetParent, // For node.get_parent() - gets parent node ID
    AddChild, // For self.add_child(child) - adds a child node and sets parent relationship
    GetType, // For node.get_type() - gets the node's NodeType (takes Uuid as first param)
    GetParentType, // For node.get_parent_type() - gets the parent's NodeType (takes Uuid as first param)
}

#[derive(Debug, Clone)]
pub enum SignalApi {
    New,
    Connect,
    Emit,
    EmitDeferred,
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

#[derive(Debug, Clone)]
pub enum TextureApi {
    Load, // api.Texture.load(path: String) -> Uuid
    CreateFromBytes, // api.Texture.create_from_bytes(bytes: Array<u8>, width: u32, height: u32) -> Uuid
    GetWidth, // api.Texture.get_width(id: Uuid) -> u32
    GetHeight, // api.Texture.get_height(id: Uuid) -> u32
    GetSize, // api.Texture.get_size(id: Uuid) -> Vector2
    // Future: as_bytes, set_bytes, etc.
}
