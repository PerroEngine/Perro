use std::{collections::BTreeMap, path::PathBuf};

/// Script/provider loading mode used when constructing the runtime.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u8)]
pub enum ProviderMode {
    Dynamic,
    Static,
}

/// Immutable project boot data owned by the runtime.
#[derive(Debug, Clone)]
pub struct RuntimeProject {
    pub name: String,
    pub root: PathBuf,
    pub runtime_params: BTreeMap<String, String>,
}

impl RuntimeProject {
    pub fn new(name: impl Into<String>, root: impl Into<PathBuf>) -> Self {
        Self {
            name: name.into(),
            root: root.into(),
            runtime_params: BTreeMap::new(),
        }
    }

    pub fn with_param(mut self, key: impl Into<String>, value: impl Into<String>) -> Self {
        self.runtime_params.insert(key.into(), value.into());
        self
    }
}
