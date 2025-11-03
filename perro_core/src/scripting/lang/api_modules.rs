#[derive(Debug, Clone)]
pub enum ApiModule {
    JSON(JSONApi),
    Time(TimeApi),
    OS(OSApi),
    Console(ConsoleApi),
    ScriptType(ScriptTypeApi),
    NodeSugar(NodeSugarApi),
    Signal(SignalApi)
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



#[derive(Debug, Clone)]
pub enum SignalApi {
    New,
    Connect,
    Emit
}
