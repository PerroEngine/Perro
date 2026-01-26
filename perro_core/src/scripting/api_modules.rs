// ----------------------------------------------------------------
// Module API Enums - Global utility functions
// These have explicit Rust versions in api.rs (JsonApi, TimeApi, etc.)
// ----------------------------------------------------------------

#[derive(Debug, Clone)]
pub enum ApiModule {
    // Module APIs (global utility functions with explicit Rust versions in api.rs)
    JSON(JSONApi),
    Time(TimeApi),
    OS(OSApi),
    Console(ConsoleApi),
    Input(InputApi),
    Math(MathApi),
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
pub enum InputApi {
    // Actions
    GetAction,

    // Controller
    ControllerEnable,

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
pub enum MathApi {
    Random, // api.Math.random() -> f32
    RandomRange, // api.Math.random_range(min: f32, max: f32) -> f32
    RandomInt, // api.Math.random_int(min: i32, max: i32) -> i32
}

