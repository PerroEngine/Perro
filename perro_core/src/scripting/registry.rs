use crate::{
    SceneData,
    apply_fur::parse_fur_file,
    asset_io::ProjectRoot,
    fur_ast::{FurElement, FurNode},
    script::{CreateFn, ScriptProvider},
};
use libloading::Library;
use std::{collections::HashMap, ffi::CString, io, os::raw::c_char};

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

    pub fn inject_project_root(&self, root: &ProjectRoot) -> anyhow::Result<()> {
        let lib = self
            .lib
            .as_ref()
            .ok_or_else(|| anyhow::anyhow!("No DLL loaded"))?;

        unsafe {
            // Try to get the symbol - if it doesn't exist, that's okay (older DLLs might not have it)
            let set_root_fn_result: Result<libloading::Symbol<
                unsafe extern "C" fn(*const c_char, *const c_char),
            >, _> = lib.get(b"perro_set_project_root\0");
            
            match set_root_fn_result {
                Ok(set_root_fn) => {
                    // Match to extract disk path and name
                    if let ProjectRoot::Disk {
                        root: path_buf,
                        name,
                    } = root
                    {
                        let path_c = CString::new(path_buf.to_string_lossy().as_ref())
                            .map_err(|e| anyhow::anyhow!("Failed to create CString for path: {}", e))?;
                        let name_c = CString::new(name.as_str())
                            .map_err(|e| anyhow::anyhow!("Failed to create CString for name: {}", e))?;

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
                        anyhow::bail!("inject_project_root only supports ProjectRoot::Disk for now");
                    }
                }
                Err(e) => {
                    // Symbol doesn't exist - this is okay for older DLLs
                    eprintln!("⚠ Warning: perro_set_project_root symbol not found in DLL (this is okay for older builds): {}", e);
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
}
