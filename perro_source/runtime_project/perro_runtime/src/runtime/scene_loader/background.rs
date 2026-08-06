//! Off-thread scene load + prepare.
//!
//! The blocking part of `scene_preload` was never the IO: it is parse plus
//! `prepare` (import resolution, node materialization, style baking), and it ran
//! on the game thread. Every input the work needs is a plain `Copy` value or a
//! `fn` pointer, and `Scene` / `PreparedScene` are `Send + Sync`, so the whole
//! stage moves to a worker and the game thread only installs the finished
//! `Arc`s.
//!
//! The `Runtime` itself stays on the game thread — it is `Rc`/`RefCell`-bound
//! and never crosses. That is why the worker carries its own resolve context
//! and its own import cache instead of borrowing the runtime's.

use super::{
    ProviderMode, Scene, load_runtime_scene_from_disk, prepare,
    prepare_scene_with_loader_and_styles,
};
use crate::runtime_project::{StaticSceneLookup, StaticUiStyleLookup};
use perro_ids::{parse_hashed_source_uri, string_to_u64};
use perro_runtime_api::sub_apis::PreloadedSceneID;
use std::cell::RefCell;
use std::collections::HashMap;
use std::sync::Arc;

/// Everything a worker needs to resolve and prepare a scene without the
/// `Runtime`. All fields are `Copy` + `Send`: two enums and two fn pointers.
#[derive(Clone, Copy)]
pub(crate) struct BackgroundSceneContext {
    pub(crate) provider_mode: ProviderMode,
    pub(crate) static_scene_lookup: Option<StaticSceneLookup>,
    pub(crate) static_ui_style_lookup: Option<StaticUiStyleLookup>,
}

/// A finished background preload, handed back to the game thread.
pub(crate) struct BackgroundPreloadResult {
    pub(crate) id: PreloadedSceneID,
    pub(crate) path_hash: u64,
    pub(crate) path: String,
    pub(crate) prepared: Result<(Arc<Scene>, Arc<prepare::PreparedScene>), String>,
}

impl BackgroundSceneContext {
    /// Worker-side mirror of `Runtime::resolve_scene_by_hash_and_path`, minus
    /// the preloaded-scene lookup (that map lives on the game thread and is
    /// only a cache, so missing it costs a re-resolve, never correctness).
    fn resolve(&self, path: &str) -> Result<Arc<Scene>, String> {
        let path_hash = parse_hashed_source_uri(path).unwrap_or_else(|| string_to_u64(path));
        match self.provider_mode {
            ProviderMode::Dynamic => {
                load_runtime_scene_from_disk(path).map(|(scene, _)| Arc::new(scene))
            }
            ProviderMode::Static => {
                // DLC scenes are not in the static table; they parse from the
                // mounted archive like dynamic ones.
                match self
                    .static_scene_lookup
                    .filter(|_| !path.starts_with("dlc://"))
                {
                    Some(lookup) => Ok(Arc::new(lookup(path_hash).clone())),
                    None => load_runtime_scene_from_disk(path).map(|(scene, _)| Arc::new(scene)),
                }
            }
        }
    }

    /// Load + prepare `path` end to end. Import resolves share a per-job cache,
    /// so a scene that includes the same sub-scene twice parses it once.
    pub(crate) fn load_and_prepare(
        &self,
        path: &str,
    ) -> Result<(Arc<Scene>, Arc<prepare::PreparedScene>), String> {
        let imports = RefCell::new(HashMap::<String, Arc<Scene>>::new());
        let scene = self.resolve(path)?;
        let load_scene = |import_path: &str| -> Result<Arc<Scene>, String> {
            if let Some(cached) = imports.borrow().get(import_path) {
                return Ok(cached.clone());
            }
            let resolved = self.resolve(import_path)?;
            imports
                .borrow_mut()
                .insert(import_path.to_string(), resolved.clone());
            Ok(resolved)
        };
        let prepared = prepare_scene_with_loader_and_styles(
            scene.as_ref(),
            &load_scene,
            self.static_ui_style_lookup,
        )?;
        Ok((scene, Arc::new(prepared)))
    }
}
