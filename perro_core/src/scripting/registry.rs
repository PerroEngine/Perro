use std::collections::HashMap;
use libloading::Library;
use crate::script::CreateFn;

/// Trait for anything that can provide script constructors
pub trait ScriptProvider {
    fn load_ctor(&mut self, short: &str) -> anyhow::Result<CreateFn>;
}

/// Dynamic DLL-based provider (default for game projects)
pub struct DllScriptProvider {
    lib: Option<Library>,
    ctors: HashMap<String, CreateFn>,
}

impl DllScriptProvider {
    pub fn new(lib: Option<Library>) -> Self {
        Self {
            lib,
            ctors: HashMap::new(),
        }
    }
}

impl ScriptProvider for DllScriptProvider {
    fn load_ctor(&mut self, short: &str) -> anyhow::Result<CreateFn> {
        if let Some(&f) = self.ctors.get(short) {
            return Ok(f);
        }

        let lib = self.lib.as_ref().ok_or_else(|| anyhow::anyhow!("No DLL loaded"))?;
        let symbol = format!("{short}_create_script\0");
        let sym: libloading::Symbol<CreateFn> = unsafe { lib.get(symbol.as_bytes())? };
        let fptr = *sym;
        self.ctors.insert(short.to_owned(), fptr);
        Ok(fptr)
    }
}