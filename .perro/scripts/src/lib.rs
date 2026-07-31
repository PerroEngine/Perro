use perro_runtime::RuntimeScriptApi;
use perro_api::scripting::ScriptConstructor;

pub static SCRIPT_REGISTRY: &[(u64, ScriptConstructor<RuntimeScriptApi>)] = &[];

#[cfg(feature = "dynamic-scripts")]
#[unsafe(no_mangle)]
pub extern "C" fn perro_scripts_init() {}
