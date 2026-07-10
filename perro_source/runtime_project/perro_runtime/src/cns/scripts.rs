use crate::{Runtime, runtime_project::ProviderMode};
use perro_ids::ScriptMemberID;
use perro_input_api::InputWindow;
use perro_resource_api::ResourceWindow;
use perro_runtime_api::RuntimeWindow;
#[cfg(any(target_os = "windows", target_os = "linux", target_os = "macos"))]
use perro_scripting::{
    DynamicScriptConstructor, SCRIPT_ABI_V2_MAGIC, SCRIPT_ABI_V2_VERSION, ScriptAbiDescriptor,
    ScriptAbiDescriptorHeader,
};
use perro_scripting::{ScriptBehavior, ScriptContext};
use perro_variant::Variant;
#[cfg(any(target_os = "windows", target_os = "linux", target_os = "macos"))]
use std::{env, fs, path::PathBuf};
use std::{path::Path, sync::Arc};

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
            let crate::runtime::PendingScriptAttach {
                node_id,
                script_path_hash,
                script_mount,
                scene_injected_vars,
            } = pending;
            self.attach_script_instance(
                node_id,
                script_path_hash,
                script_mount.as_deref(),
                scene_injected_vars,
            )?;
        }

        Ok(())
    }

    pub(crate) fn attach_script_instance(
        &mut self,
        node: perro_ids::NodeID,
        script_path_hash: u64,
        script_mount: Option<&str>,
        scene_injected_vars: Vec<(ScriptMemberID, Variant)>,
    ) -> Result<(), String> {
        if node.is_nil() || self.nodes.get(node).is_none() {
            return Err(format!(
                "node `{node}` not found for script hash `{script_path_hash}`"
            ));
        }

        let ctor = self
            .script_runtime
            .resolve_script_constructor(script_path_hash)
            .ok_or_else(|| {
                format!("script hash `{script_path_hash}` is not present in script registry")
            })?;
        let behavior: Arc<dyn ScriptBehavior<crate::runtime::RuntimeScriptApi>> =
            if let Some(cached) = self
                .script_runtime
                .script_behavior_cache
                .get(&script_path_hash)
            {
                Arc::clone(cached)
            } else {
                let raw = ctor.call();
                if raw.is_null() {
                    return Err(format!(
                        "script constructor returned null for hash `{script_path_hash}`"
                    ));
                }

                // SAFETY: Script constructors transfer ownership of a Box allocated
                // by generated script glue and return null on failure.
                let behavior: Box<dyn ScriptBehavior<crate::runtime::RuntimeScriptApi>> =
                    unsafe { Box::from_raw(raw) };
                let behavior: Arc<dyn ScriptBehavior<crate::runtime::RuntimeScriptApi>> =
                    behavior.into();
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
            let Some(instance_index) = self.scripts.instance_index_for_id(node) else {
                return Ok(());
            };
            let resource_api = self.resource_api.clone();
            let res: ResourceWindow<'_, crate::RuntimeResourceApi> =
                ResourceWindow::new(resource_api.as_ref());
            let input_ptr = std::ptr::addr_of!(self.input);
            // SAFETY: During callback dispatch, input is treated as immutable runtime state.
            // Engine invariant: only window/event ingestion mutates input, outside script callback execution.
            let ipt: InputWindow<'_, perro_input_api::InputSnapshot> =
                unsafe { InputWindow::new(&*input_ptr) };
            let _dlc_self_context = self.push_script_dlc_self_context(node);
            self.push_active_script_with_context(
                instance_index,
                node,
                self.script_callback_context(),
            );
            let mut run = RuntimeWindow::new(self);
            let mut sctx = ScriptContext {
                run: &mut run,
                res: &res,
                ipt: &ipt,
                id: node,
            };
            behavior.on_init(&mut sctx);
            self.pop_active_script(instance_index, node);
        }
        if flags.has_all_init() {
            self.queue_start_script(node);
        }

        Ok(())
    }

    #[cfg(any(target_os = "windows", target_os = "linux", target_os = "macos"))]
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
            if self
                .script_runtime
                .loaded_dlc_script_libs
                .contains(&dlc_key)
            {
                continue;
            }
            self.load_script_registry_library(&dylib_path, project_root, project_name)?;
            self.script_runtime.loaded_dlc_script_libs.insert(dlc_key);
        }

        Ok(())
    }

    #[cfg(not(any(target_os = "windows", target_os = "linux", target_os = "macos")))]
    pub(crate) fn ensure_dynamic_script_registry_loaded(
        &mut self,
        _project_root: &Path,
        _project_name: &str,
    ) -> Result<(), String> {
        Err("dynamic scripts are not supported on this target".to_string())
    }

    #[cfg(any(target_os = "windows", target_os = "linux", target_os = "macos"))]
    fn load_script_registry_library(
        &mut self,
        dylib_path: &Path,
        project_root: &Path,
        project_name: &str,
    ) -> Result<(), String> {
        // SAFETY: Path comes from Perro build output. Loaded Library is stored in
        // script_runtime so symbols and code outlive registered constructors.
        let library = unsafe {
            libloading::Library::new(dylib_path).map_err(|err| {
                format!(
                    "failed to load scripts dylib `{}`: {err}",
                    dylib_path.display()
                )
            })?
        };

        // SAFETY: The first call uses only the fixed v2 C descriptor prefix. All
        // Rust ABI symbols are read and called only after that descriptor matches.
        // Calls happen while the Library is kept alive by script_runtime.
        unsafe {
            type AbiDescriptorFn = unsafe extern "C" fn() -> *const ScriptAbiDescriptorHeader;
            type InitFn = unsafe extern "C" fn();
            type SetProjectRootFn =
                unsafe extern "C" fn(*const u8, usize, *const u8, usize) -> bool;
            type RegistryLenFn = unsafe extern "C" fn() -> usize;
            type RegistryGetFn = unsafe extern "C" fn(
                usize,
                *mut u64,
                *mut DynamicScriptConstructor<crate::runtime::RuntimeScriptApi>,
            ) -> bool;

            let abi_descriptor = *library
                .get::<AbiDescriptorFn>(b"perro_script_abi_descriptor_v2")
                .map_err(|err| {
                    format!(
                        "scripts dylib has no compatible ABI v2 descriptor; rebuild scripts with this engine: {err}"
                    )
                })?;
            let descriptor_ptr = abi_descriptor();
            if descriptor_ptr.is_null() {
                return Err("scripts dylib returned a null ABI v2 descriptor".to_string());
            }
            let header = descriptor_ptr.read();
            validate_script_abi_header(&header)?;
            let descriptor = descriptor_ptr.cast::<ScriptAbiDescriptor>().read();
            validate_script_abi_descriptor(&descriptor, crate::SCRIPT_ABI_BUILD_FINGERPRINT)?;

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

            let registry_len = registry_len();
            let mut entries = Vec::with_capacity(registry_len);
            for i in 0..registry_len {
                let mut path_hash = 0u64;
                let mut ctor = std::mem::MaybeUninit::<
                    DynamicScriptConstructor<crate::runtime::RuntimeScriptApi>,
                >::uninit();
                let ok = registry_get(i, &mut path_hash, ctor.as_mut_ptr());
                if !ok {
                    return Err(format!("scripts registry entry {i} could not be read"));
                }
                entries.push((path_hash, ctor.assume_init()));
            }
            for (path_hash, ctor) in entries {
                self.script_runtime
                    .dynamic_script_registry
                    .insert(path_hash, ctor);
            }
        }

        self.script_runtime.script_libraries.push(library);
        Ok(())
    }
}

#[cfg(any(target_os = "windows", target_os = "linux", target_os = "macos"))]
fn validate_script_abi_header(header: &ScriptAbiDescriptorHeader) -> Result<(), String> {
    if header.magic != SCRIPT_ABI_V2_MAGIC {
        return Err("scripts dylib ABI descriptor has invalid magic".to_string());
    }
    if header.abi_version != SCRIPT_ABI_V2_VERSION {
        return Err(format!(
            "scripts dylib ABI version mismatch: expected {}, found {}",
            SCRIPT_ABI_V2_VERSION, header.abi_version
        ));
    }
    let required_size = std::mem::size_of::<ScriptAbiDescriptor>();
    if (header.descriptor_size as usize) < required_size {
        return Err(format!(
            "scripts dylib ABI descriptor is too small: expected at least {required_size} bytes, found {}",
            header.descriptor_size
        ));
    }
    Ok(())
}

#[cfg(any(target_os = "windows", target_os = "linux", target_os = "macos"))]
fn validate_script_abi_descriptor(
    descriptor: &ScriptAbiDescriptor,
    expected_fingerprint: u64,
) -> Result<(), String> {
    validate_script_abi_header(&descriptor.header)?;
    if descriptor.build_fingerprint != expected_fingerprint {
        return Err(format!(
            "scripts dylib build fingerprint mismatch: expected {expected_fingerprint:016x}, found {:016x}; rebuild scripts with this engine",
            descriptor.build_fingerprint
        ));
    }
    Ok(())
}

#[cfg(all(
    test,
    any(target_os = "windows", target_os = "linux", target_os = "macos")
))]
mod script_abi_tests {
    use super::*;

    #[test]
    fn accepts_matching_v2_descriptor() {
        let descriptor = ScriptAbiDescriptor::v2(42);
        assert_eq!(validate_script_abi_descriptor(&descriptor, 42), Ok(()));
    }

    #[test]
    fn rejects_wrong_magic_before_fingerprint() {
        let mut descriptor = ScriptAbiDescriptor::v2(42);
        descriptor.header.magic = *b"NOTPERRO";
        let err = validate_script_abi_descriptor(&descriptor, 42).unwrap_err();
        assert!(err.contains("invalid magic"));
    }

    #[test]
    fn rejects_short_descriptor_before_full_read() {
        let mut descriptor = ScriptAbiDescriptor::v2(42);
        descriptor.header.descriptor_size = (std::mem::size_of::<ScriptAbiDescriptor>() - 1) as u32;
        let err = validate_script_abi_header(&descriptor.header).unwrap_err();
        assert!(err.contains("too small"));
    }

    #[test]
    fn rejects_mismatched_build_fingerprint() {
        let descriptor = ScriptAbiDescriptor::v2(42);
        let err = validate_script_abi_descriptor(&descriptor, 7).unwrap_err();
        assert!(err.contains("build fingerprint mismatch"));
        assert!(err.contains("rebuild scripts"));
    }
}

#[cfg(any(target_os = "windows", target_os = "linux", target_os = "macos"))]
fn resolve_scripts_dylib_path(project_root: &Path) -> Result<PathBuf, String> {
    if let Some(path) = env::var_os("PERRO_SCRIPTS_DYLIB_PATH") {
        let path = PathBuf::from(path);
        if path.is_file() {
            return Ok(path);
        }
        return Err(format!(
            "PERRO_SCRIPTS_DYLIB_PATH does not point to a scripts dylib: {}",
            path.display()
        ));
    }

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
                if name.starts_with(prefix)
                    && name.ends_with(suffix)
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
