use crate::{Runtime, runtime_project::ProviderMode};
use perro_context::RuntimeContext;
use perro_scripting::{ScriptBehavior, ScriptConstructor};
use std::{
    fs,
    path::{Path, PathBuf},
    sync::Arc,
};

impl Runtime {
    pub(super) fn attach_scene_scripts(
        &mut self,
        script_nodes: Vec<(perro_ids::NodeID, String)>,
    ) -> Result<(), String> {
        let project_root = self
            .project()
            .ok_or_else(|| "Runtime project is not set".to_string())?
            .root
            .clone();
        let project_name = self
            .project()
            .ok_or_else(|| "Runtime project is not set".to_string())?
            .config
            .name
            .clone();

        match self.provider_mode {
            ProviderMode::Dynamic => {
                self.ensure_dynamic_script_registry_loaded(&project_root, &project_name)?
            }
            ProviderMode::Static => {
                if self.dynamic_script_registry.is_empty() {
                    return Ok(());
                }
            }
        }

        for (id, script_path) in script_nodes {
            let ctor = *self
                .dynamic_script_registry
                .get(&script_path)
                .ok_or_else(|| {
                    format!("script `{script_path}` is not present in the dynamic script registry")
                })?;
            let raw = ctor();
            if raw.is_null() {
                return Err(format!(
                    "script constructor returned null for `{script_path}`"
                ));
            }

            let behavior: Box<dyn ScriptBehavior<Self>> = unsafe { Box::from_raw(raw) };
            let behavior: Arc<dyn ScriptBehavior<Self>> = behavior.into();
            let state = behavior.create_state();
            let flags = behavior.script_flags();
            self.scripts.insert(id, Arc::clone(&behavior), state);

            if flags.has_init() {
                let mut ctx = RuntimeContext::new(self);
                behavior.init(&mut ctx, id);
            }
        }

        Ok(())
    }

    fn ensure_dynamic_script_registry_loaded(
        &mut self,
        project_root: &Path,
        project_name: &str,
    ) -> Result<(), String> {
        if !self.dynamic_script_registry.is_empty() {
            return Ok(());
        }

        let dylib_path = resolve_scripts_dylib_path()?;
        let library = unsafe {
            libloading::Library::new(&dylib_path).map_err(|err| {
                format!(
                    "failed to load scripts dylib `{}`: {err}",
                    dylib_path.display()
                )
            })?
        };

        unsafe {
            type InitFn = unsafe extern "C" fn();
            type SetProjectRootFn =
                unsafe extern "C" fn(*const u8, usize, *const u8, usize) -> bool;
            type RegistryLenFn = unsafe extern "C" fn() -> usize;
            type RegistryGetFn = unsafe extern "C" fn(
                usize,
                *mut *const u8,
                *mut usize,
                *mut ScriptConstructor<Runtime>,
            ) -> bool;

            if let Ok(init) = library.get::<InitFn>(b"perro_scripts_init") {
                init();
            }
            if let Ok(set_project_root) =
                library.get::<SetProjectRootFn>(b"perro_scripts_set_project_root")
            {
                let root = project_root.to_string_lossy();
                let ok = set_project_root(
                    root.as_bytes().as_ptr(),
                    root.len(),
                    project_name.as_bytes().as_ptr(),
                    project_name.len(),
                );
                if !ok {
                    return Err(
                        "scripts dylib rejected project root injection via `perro_scripts_set_project_root`"
                            .to_string(),
                    );
                }
            }

            let registry_len = *library
                .get::<RegistryLenFn>(b"perro_script_registry_len")
                .map_err(|err| {
                    format!("missing `perro_script_registry_len` in scripts dylib: {err}")
                })?;
            let registry_get = *library
                .get::<RegistryGetFn>(b"perro_script_registry_get")
                .map_err(|err| {
                    format!("missing `perro_script_registry_get` in scripts dylib: {err}")
                })?;

            for i in 0..registry_len() {
                let mut ptr: *const u8 = std::ptr::null();
                let mut len = 0usize;
                let mut ctor = std::mem::MaybeUninit::<ScriptConstructor<Runtime>>::uninit();
                let ok = registry_get(i, &mut ptr, &mut len, ctor.as_mut_ptr());
                if !ok {
                    return Err(format!("scripts registry entry {i} could not be read"));
                }
                if ptr.is_null() || len == 0 {
                    return Err(format!("scripts registry entry {i} has an invalid path"));
                }
                let bytes = std::slice::from_raw_parts(ptr, len);
                let path = std::str::from_utf8(bytes)
                    .map_err(|err| format!("scripts registry entry {i} path is not UTF-8: {err}"))?
                    .to_string();
                self.dynamic_script_registry
                    .insert(path, ctor.assume_init());
            }
        }

        self.script_library = Some(library);
        Ok(())
    }
}

fn resolve_scripts_dylib_path() -> Result<PathBuf, String> {
    let workspace_root = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("..")
        .canonicalize()
        .unwrap_or_else(|_| PathBuf::from(env!("CARGO_MANIFEST_DIR")).join(".."));
    let release_dir = workspace_root.join("target").join("release");
    let file_name = scripts_dylib_name();
    let primary = release_dir.join(file_name);
    if primary.exists() {
        return Ok(primary);
    }

    let deps_dir = release_dir.join("deps");
    if deps_dir.exists() {
        let prefix = scripts_dylib_prefix();
        let suffix = scripts_dylib_suffix();
        let entries = fs::read_dir(&deps_dir).map_err(|err| {
            format!(
                "failed to scan `{}` for scripts dylib: {err}",
                deps_dir.display()
            )
        })?;
        for entry in entries.flatten() {
            let path = entry.path();
            let Some(name) = path.file_name().and_then(|v| v.to_str()) else {
                continue;
            };
            if name.starts_with(prefix) && name.ends_with(suffix) {
                return Ok(path);
            }
        }
    }

    Err(format!(
        "scripts dylib not found at `{}` (or `{}`)",
        primary.display(),
        deps_dir.display()
    ))
}

#[cfg(target_os = "windows")]
fn scripts_dylib_name() -> &'static str {
    "scripts.dll"
}

#[cfg(target_os = "linux")]
fn scripts_dylib_name() -> &'static str {
    "libscripts.so"
}

#[cfg(target_os = "macos")]
fn scripts_dylib_name() -> &'static str {
    "libscripts.dylib"
}

#[cfg(target_os = "windows")]
fn scripts_dylib_prefix() -> &'static str {
    "scripts-"
}

#[cfg(target_os = "linux")]
fn scripts_dylib_prefix() -> &'static str {
    "libscripts-"
}

#[cfg(target_os = "macos")]
fn scripts_dylib_prefix() -> &'static str {
    "libscripts-"
}

#[cfg(target_os = "windows")]
fn scripts_dylib_suffix() -> &'static str {
    ".dll"
}

#[cfg(target_os = "linux")]
fn scripts_dylib_suffix() -> &'static str {
    ".so"
}

#[cfg(target_os = "macos")]
fn scripts_dylib_suffix() -> &'static str {
    ".dylib"
}
