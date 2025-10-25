#[derive(Debug, Clone)]
pub enum ApiModule {
    JSON(JSONApi),
    Time(TimeApi),
    OS(OSApi),
    Console(ConsoleApi),
    ScriptType(ScriptTypeApi),
    NodeSugar(NodeSugarApi),
}

#[derive(Debug, Clone)]
pub enum JSONApi {
    Parse,
    Stringify,
}

#[derive(Debug, Clone)]
pub enum TimeApi {
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
    Info
}

#[derive(Debug, Clone)]
pub enum ScriptTypeApi {
    Instantiate
}

#[derive(Debug, Clone)]
pub enum NodeSugarApi {
    GetVar,
    SetVar,
}
