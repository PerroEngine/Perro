use perro_core::script::ScriptProvider;
use perro_core::script::CreateFn;
use std::collections::HashMap;
use scripts::get_script_registry;

pub struct StaticScriptProvider {
    ctors: HashMap<String, CreateFn>,
}

impl StaticScriptProvider {
    pub fn new() -> Self {
        Self { ctors: get_script_registry() }
    }
}

impl ScriptProvider for StaticScriptProvider {
    fn load_ctor(&mut self, short: &str) -> anyhow::Result<CreateFn> {
        self.ctors
            .get(short)
            .copied()
            .ok_or_else(|| anyhow::anyhow!("No static ctor for {short}"))
    }
}