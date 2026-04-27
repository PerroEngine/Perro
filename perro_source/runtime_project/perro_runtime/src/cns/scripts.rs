use crate::{Runtime, runtime_project::ProviderMode};
use perro_ids::ScriptMemberID;
use perro_input::InputContext;
use perro_io::set_dlc_self_context;
use perro_resource_context::ResourceContext;
use perro_runtime_context::RuntimeContext;
use perro_scripting::{ScriptBehavior, ScriptConstructor};
use perro_variant::Variant;
use std::{
    fs,
    path::{Path, PathBuf},
    sync::Arc,
};

impl Runtime {
    pub(crate) fn attach_scene_scripts(
        &mut self,
        script_nodes: Vec<crate::runtime::PendingScriptAttach>,
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

        if self.provider_mode() == ProviderMode::Dynamic
            || !self.script_runtime.mounted_dlc_script_libs.is_empty()
        {
            self.ensure_dynamic_script_registry_loaded(&project_root, &project_name)?;
        }

        for pending in script_nodes {
            self.attach_script_instance(
                pending.node_id,
                pending.script_path_hash,
                pending.script_mount.as_deref(),
                &pending.scene_injected_vars,
            )?;
        }

        Ok(())
    }

    pub(crate) fn attach_script_instance(
        &mut self,
        node: perro_ids::NodeID,
        script_path_hash: u64,
        script_mount: Option<&str>,
        scene_injected_vars: &[(ScriptMemberID, Variant)],
    ) -> Result<(), String> {
        if node.is_nil() || self.nodes.get(node).is_none() {
            return Err(format!(
                "node `{node}` not found for script hash `{script_path_hash}`"
            ));
        }

        let ctor = *self
            .script_runtime
            .dynamic_script_registry
            .get(&script_path_hash)
            .ok_or_else(|| {
                format!(
                    "script hash `{script_path_hash}` is not present in dynamic script registry"
                )
            })?;
        let behavior: Arc<
            dyn ScriptBehavior<Self, crate::RuntimeResourceApi, perro_input::InputSnapshot>,
        > = if let Some(cached) = self
            .script_runtime
            .script_behavior_cache
            .get(&script_path_hash)
        {
            Arc::clone(cached)
        } else {
            let raw = ctor();
            if raw.is_null() {
                return Err(format!(
                    "script constructor returned null for hash `{script_path_hash}`"
                ));
            }

            let behavior: Box<
                dyn ScriptBehavior<Self, crate::RuntimeResourceApi, perro_input::InputSnapshot>,
            > = unsafe { Box::from_raw(raw) };
            let behavior: Arc<
                dyn ScriptBehavior<Self, crate::RuntimeResourceApi, perro_input::InputSnapshot>,
            > = behavior.into();
            self.script_runtime
                .script_behavior_cache
                .insert(script_path_hash, Arc::clone(&behavior));
            behavior
        };
        let state = behavior.create_state();
        let flags = behavior.script_flags();
        if self.scripts.get_instance(node).is_some() {
            self.remove_script_instance(node);
        }
        if let Some(mount) = script_mount {
            self.script_runtime
                .script_instance_dlc_mounts
                .insert(node, mount.to_ascii_lowercase());
        } else {
            self.script_runtime.script_instance_dlc_mounts.remove(&node);
        }
        self.scripts.insert(node, Arc::clone(&behavior), state);
        let _ = self.scripts.with_instance_mut(node, |instance| {
            instance
                .behavior
                .apply_scene_injected_vars(instance.state.as_mut(), scene_injected_vars);
        });

        if flags.has_init() {
            let resource_api = self.resource_api.clone();
            let res: ResourceContext<'_, crate::RuntimeResourceApi> =
                ResourceContext::new(resource_api.as_ref());
            let input_ptr = std::ptr::addr_of!(self.input);
            // SAFETY: During callback dispatch, input is treated as immutable runtime state.
            // Engine invariant: only window/event ingestion mutates input, outside script callback execution.
            let ipt: InputContext<'_, perro_input::InputSnapshot> =
                unsafe { InputContext::new(&*input_ptr) };
            let mount = self
                .script_runtime
                .script_instance_dlc_mounts
                .get(&node)
                .cloned();
            set_dlc_self_context(mount.as_deref());
            let mut ctx = RuntimeContext::new(self);
            behavior.on_init(&mut ctx, &res, &ipt, node);
            set_dlc_self_context(None);
        }
        if flags.has_all_init() {
            self.queue_start_script(node);
        }

        Ok(())
    }

    pub(crate) fn ensure_dynamic_script_registry_loaded(
        &mut self,
        project_root: &Path,
        project_name: &str,
    ) -> Result<(), String> {
        if self.provider_mode() == ProviderMode::Dynamic && !self.script_runtime.base_scripts_loaded
        {
            let dylib_path = resolve_scripts_dylib_path(project_root)?;
            self.load_script_registry_library(&dylib_path, project_root, project_name)?;
            self.script_runtime.base_scripts_loaded = true;
        }

        let mounted = self
            .script_runtime
            .mounted_dlc_script_libs
            .clone()
            .into_iter()
            .collect::<Vec<_>>();
        for (dlc_key, dylib_path) in mounted {
            if self.script_runtime.loaded_dlc_script_libs.contains(&dlc_key) {
                continue;
            }
            self.load_script_registry_library(&dylib_path, project_root, project_name)?;
            self.script_runtime.loaded_dlc_script_libs.insert(dlc_key);
        }

        Ok(())
    }

    fn load_script_registry_library(
        &mut self,
        dylib_path: &Path,
        project_root: &Path,
        project_name: &str,
    ) -> Result<(), String> {
        let library = unsafe {
            libloading::Library::new(dylib_path).map_err(|err| {
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
                *mut u64,
                *mut ScriptConstructor<
                    Runtime,
                    crate::RuntimeResourceApi,
                    perro_input::InputSnapshot,
                >,
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
                let mut path_hash = 0u64;
                let mut ctor = std::mem::MaybeUninit::<
                    ScriptConstructor<
                        Runtime,
                        crate::RuntimeResourceApi,
                        perro_input::InputSnapshot,
                    >,
                >::uninit();
                let ok = registry_get(i, &mut path_hash, ctor.as_mut_ptr());
                if !ok {
                    return Err(format!("scripts registry entry {i} could not be read"));
                }
                self.script_runtime
                    .dynamic_script_registry
                    .insert(path_hash, ctor.assume_init());
            }
        }

        self.script_runtime.script_libraries.push(library);
        Ok(())
    }
}

fn resolve_scripts_dylib_path(project_root: &Path) -> Result<PathBuf, String> {
    let profiles = ["debug", "release"];
    let mut scanned = Vec::<String>::new();
    let mut candidates = Vec::<(std::time::SystemTime, PathBuf)>::new();
    for profile in profiles {
        let profile_dir = project_root.join("target").join(profile);
        let file_name = scripts_dylib_name();
        let primary = profile_dir.join(file_name);
        scanned.push(primary.display().to_string());
        if primary.exists()
            && let Ok(meta) = fs::metadata(&primary)
                && let Ok(modified) = meta.modified()
            {
                candidates.push((modified, primary.clone()));
            }

        let deps_dir = profile_dir.join("deps");
        scanned.push(deps_dir.display().to_string());
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
                if name.starts_with(prefix) && name.ends_with(suffix)
                    && let Ok(meta) = fs::metadata(&path)
                        && let Ok(modified) = meta.modified()
                    {
                        candidates.push((modified, path));
                    }
            }
        }
    }

    if let Some((_, path)) = candidates.into_iter().max_by_key(|(modified, _)| *modified) {
        return Ok(path);
    }

    Err(format!(
        "scripts dylib not found. searched: {}",
        scanned.join(", ")
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
