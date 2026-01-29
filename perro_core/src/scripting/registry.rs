use crate::{
    SceneData,
    apply_fur::parse_fur_file,
    asset_io::ProjectRoot,
    fur_ast::{FurElement, FurNode},
    script::{CreateFn, ScriptProvider},
};
use libloading::Library;
use std::sync::RwLock;
use std::{collections::HashMap, ffi::CString, io, os::raw::c_char};

/// Wraps (ptr, len) from the DLL so we can store it in RwLock (raw pointers are !Send + !Sync).
/// The pointer points at the DLL's static GLOBAL_ORDER; the DLL is kept alive by DllScriptProvider.
#[derive(Clone, Copy)]
struct GlobalSlicePtr(*const u8, usize);
unsafe impl Send for GlobalSlicePtr {}
unsafe impl Sync for GlobalSlicePtr {}

/// Dynamic DLL-based provider (default for game projects)
pub struct DllScriptProvider {
    lib: Option<Library>,
    ctors: HashMap<String, CreateFn>,
    /// Cached (ptr, len) from DLL's get_global_registry_order() for get_global_registry_order().
    cached_global_slice: RwLock<Option<GlobalSlicePtr>>,
    /// Cached (ptr, len) from DLL's get_global_registry_names() for node display names.
    cached_global_names_slice: RwLock<Option<GlobalSlicePtr>>,
}

impl DllScriptProvider {
    pub fn new(lib: Option<Library>) -> Self {
        Self {
            lib,
            ctors: HashMap::new(),
            cached_global_slice: RwLock::new(None),
            cached_global_names_slice: RwLock::new(None),
        }
    }

    /// Call DLL's perro_get_global_registry_slice and return (ptr to first &str, length).
    fn fetch_global_registry_slice(&self) -> GlobalSlicePtr {
        let lib = match self.lib.as_ref() {
            Some(l) => l,
            None => return GlobalSlicePtr(std::ptr::null(), 0),
        };
        type Fn = extern "C" fn(*mut *const u8, *mut usize);
        let sym: libloading::Symbol<Fn> =
            match unsafe { lib.get(b"perro_get_global_registry_slice\0") } {
                Ok(s) => s,
                Err(_) => return GlobalSlicePtr(std::ptr::null(), 0),
            };
        let mut ptr: *const u8 = std::ptr::null();
        let mut len: usize = 0;
        sym(&mut ptr, &mut len);
        GlobalSlicePtr(ptr, len)
    }

    /// Call DLL's perro_get_global_registry_names_slice for display names.
    fn fetch_global_registry_names_slice(&self) -> GlobalSlicePtr {
        let lib = match self.lib.as_ref() {
            Some(l) => l,
            None => return GlobalSlicePtr(std::ptr::null(), 0),
        };
        type Fn = extern "C" fn(*mut *const u8, *mut usize);
        let sym: libloading::Symbol<Fn> =
            match unsafe { lib.get(b"perro_get_global_registry_names_slice\0") } {
                Ok(s) => s,
                Err(_) => return GlobalSlicePtr(std::ptr::null(), 0),
            };
        let mut ptr: *const u8 = std::ptr::null();
        let mut len: usize = 0;
        sym(&mut ptr, &mut len);
        GlobalSlicePtr(ptr, len)
    }

    pub fn inject_project_root(&self, root: &ProjectRoot) -> anyhow::Result<()> {
        let lib = self
            .lib
            .as_ref()
            .ok_or_else(|| anyhow::anyhow!("No DLL loaded"))?;

        unsafe {
            // Try to get the symbol - if it doesn't exist, that's okay (older DLLs might not have it)
            let set_root_fn_result: Result<
                libloading::Symbol<unsafe extern "C" fn(*const c_char, *const c_char)>,
                _,
            > = lib.get(b"perro_set_project_root\0");

            match set_root_fn_result {
                Ok(set_root_fn) => {
                    // Match to extract disk path and name
                    if let ProjectRoot::Disk {
                        root: path_buf,
                        name,
                    } = root
                    {
                        let path_c =
                            CString::new(path_buf.to_string_lossy().as_ref()).map_err(|e| {
                                anyhow::anyhow!("Failed to create CString for path: {}", e)
                            })?;
                        let name_c = CString::new(name.as_str()).map_err(|e| {
                            anyhow::anyhow!("Failed to create CString for name: {}", e)
                        })?;

                        // Ensure the CStrings stay alive during the function call
                        // by keeping them in scope
                        let path_ptr = path_c.as_ptr();
                        let name_ptr = name_c.as_ptr();

                        // Call the function
                        // NOTE: On Windows, this can cause STATUS_ACCESS_VIOLATION if the DLL
                        // was built against a different version of perro_core, because DLLs
                        // have separate static variable instances. The caller should skip this
                        // on Windows if experiencing issues.
                        set_root_fn(path_ptr, name_ptr);
                    } else {
                        anyhow::bail!(
                            "inject_project_root only supports ProjectRoot::Disk for now"
                        );
                    }
                }
                Err(e) => {
                    // Symbol doesn't exist - this is okay for older DLLs
                    eprintln!(
                        "⚠ Warning: perro_set_project_root symbol not found in DLL (this is okay for older builds): {}",
                        e
                    );
                }
            }
        }

        Ok(())
    }
}

impl ScriptProvider for DllScriptProvider {
    fn load_ctor(&mut self, short: &str) -> anyhow::Result<CreateFn> {
        if let Some(&f) = self.ctors.get(short) {
            return Ok(f);
        }

        let lib = self
            .lib
            .as_ref()
            .ok_or_else(|| anyhow::anyhow!("No DLL loaded"))?;
        let symbol = format!("{short}_create_script\0");
        let sym: libloading::Symbol<CreateFn> = unsafe {
            lib.get(symbol.as_bytes()).map_err(|e| {
                anyhow::anyhow!(
                    "Failed to find symbol '{}' in DLL: {}",
                    symbol.trim_end_matches('\0'),
                    e
                )
            })?
        };
        let fptr = *sym;
        self.ctors.insert(short.to_owned(), fptr);
        Ok(fptr)
    }

    fn load_scene_data(&self, path: &str) -> io::Result<SceneData> {
        // DLL / editor mode always loads from disk
        SceneData::load(path)
    }

    fn load_fur_data(&self, path: &str) -> io::Result<Vec<FurElement>> {
        match parse_fur_file(path) {
            Ok(ast) => {
                let fur_elements: Vec<FurElement> = ast
                    .into_iter()
                    .filter_map(|f| match f {
                        FurNode::Element(el) => Some(el),
                        _ => None,
                    })
                    .collect();

                Ok(fur_elements)
            }
            Err(e) => Err(io::Error::new(io::ErrorKind::Other, e)),
        }
    }

    fn get_global_registry_order(&self) -> &[&str] {
        if self.cached_global_slice.read().unwrap().is_none() {
            *self.cached_global_slice.write().unwrap() = Some(self.fetch_global_registry_slice());
        }
        let GlobalSlicePtr(ptr, len) = self
            .cached_global_slice
            .read()
            .unwrap()
            .as_ref()
            .copied()
            .unwrap_or(GlobalSlicePtr(std::ptr::null(), 0));
        if ptr.is_null() || len == 0 {
            return &[];
        }
        unsafe { std::slice::from_raw_parts(ptr as *const &str, len) }
    }

    fn get_global_registry_names(&self) -> &[&str] {
        if self.cached_global_names_slice.read().unwrap().is_none() {
            *self.cached_global_names_slice.write().unwrap() =
                Some(self.fetch_global_registry_names_slice());
        }
        let GlobalSlicePtr(ptr, len) = self
            .cached_global_names_slice
            .read()
            .unwrap()
            .as_ref()
            .copied()
            .unwrap_or(GlobalSlicePtr(std::ptr::null(), 0));
        if ptr.is_null() || len == 0 {
            return &[];
        }
        unsafe { std::slice::from_raw_parts(ptr as *const &str, len) }
    }
}
