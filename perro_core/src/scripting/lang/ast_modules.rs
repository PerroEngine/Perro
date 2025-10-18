#[derive(Debug, Clone)]
pub enum ApiModule {
    JSON(JSONApi),
    Time(TimeApi),
    OS(OSApi),
    Console(ConsoleApi),
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