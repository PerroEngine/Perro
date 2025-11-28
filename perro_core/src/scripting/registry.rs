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
            let set_root_fn: libloading::Symbol<
                unsafe extern "C" fn(*const c_char, *const c_char),
            > = lib.get(b"perro_set_project_root\0")?;

            // Match to extract disk path and name
            if let ProjectRoot::Disk {
                root: path_buf,
                name,
            } = root
            {
                let path_c = CString::new(path_buf.to_string_lossy().as_ref())?;
                let name_c = CString::new(name.as_str())?;

                set_root_fn(path_c.as_ptr(), name_c.as_ptr());
            } else {
                anyhow::bail!("inject_project_root only supports ProjectRoot::Disk for now");
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
        eprintln!("🔍 Looking for symbol: '{}' (identifier: '{}')", symbol.trim_end_matches('\0'), short);
        let sym: libloading::Symbol<CreateFn> = unsafe {
            lib.get(symbol.as_bytes())
                .map_err(|e| anyhow::anyhow!("Failed to find symbol '{}' in DLL: {}", symbol.trim_end_matches('\0'), e))?
        };
        let fptr = *sym;
        self.ctors.insert(short.to_owned(), fptr);
        eprintln!("✅ Successfully loaded constructor for '{}'", short);
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
