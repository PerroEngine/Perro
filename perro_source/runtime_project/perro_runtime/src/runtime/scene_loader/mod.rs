use crate::{Runtime, runtime_project::ProviderMode};
use perro_ids::NodeID;
use perro_ids::ScriptMemberID;
use perro_ids::parse_hashed_source_uri;
use perro_ids::string_to_u64;
use perro_io::{ProjectRoot, set_project_root};
use perro_runtime_context::sub_apis::PreloadedSceneID;
use perro_scene::Scene;
use perro_variant::Variant;
use std::sync::Arc;
#[cfg(feature = "profile")]
use std::time::{Duration, Instant};

mod merge;
mod prepare;

use merge::merge_prepared_scene;
use prepare::{load_runtime_scene_from_disk, prepare_scene_with_loader};

pub(crate) struct PendingScriptAttach {
    pub(crate) node_id: NodeID,
    pub(crate) script_path_hash: u64,
    pub(crate) scene_injected_vars: Vec<(ScriptMemberID, Variant)>,
}

#[cfg(feature = "profile")]
struct SceneLoadStats {
    mode_label: &'static str,
    source_load: Option<Duration>,
    parse: Option<Duration>,
    node_insert: Duration,
    total_excluding_debug_print: Duration,
}

impl Runtime {
    fn source_hash(path: &str) -> u64 {
        parse_hashed_source_uri(path).unwrap_or_else(|| string_to_u64(path))
    }

    fn resolve_scene_by_hash_and_path(
        &self,
        path_hash: u64,
        path: &str,
    ) -> Result<Arc<Scene>, String> {
        if let Some(id) = self.preloaded_scene_paths.get(&path_hash).copied() {
            if let Some(scene) = self.preloaded_scenes.get(&id) {
                return Ok(scene.clone());
            }
        }
        match self.provider_mode {
            ProviderMode::Dynamic => self.get_or_load_dynamic_scene_cached(path),
            ProviderMode::Static => {
                let static_lookup = self
                    .project()
                    .and_then(|project| project.static_scene_lookup);
                if let Some(lookup) = static_lookup {
                    Ok(Arc::new(lookup(path_hash).clone()))
                } else {
                    self.get_or_load_dynamic_scene_cached(path)
                }
            }
        }
    }

    fn get_or_load_dynamic_scene_cached(&self, path: &str) -> Result<Arc<Scene>, String> {
        if let Some(scene) = self.scene_cache.borrow().get(path).cloned() {
            return Ok(scene);
        }
        let (scene, _) = load_runtime_scene_from_disk(path)?;
        let scene = Arc::new(scene);
        self.scene_cache
            .borrow_mut()
            .insert(path.to_string(), scene.clone());
        Ok(scene)
    }

    fn resolve_scene_by_path(&self, path: &str) -> Result<Arc<Scene>, String> {
        self.resolve_scene_by_hash_and_path(Self::source_hash(path), path)
    }

    pub(crate) fn preload_scene_at_runtime(
        &mut self,
        path: &str,
    ) -> Result<PreloadedSceneID, String> {
        self.preload_scene_at_runtime_hashed(Self::source_hash(path), path)
    }

    pub(crate) fn preload_scene_at_runtime_hashed(
        &mut self,
        path_hash: u64,
        path: &str,
    ) -> Result<PreloadedSceneID, String> {
        if let Some(existing) = self.preloaded_scene_paths.get(&path_hash).copied() {
            return Ok(existing);
        }
        let scene = self.resolve_scene_by_hash_and_path(path_hash, path)?;
        let mut next = self.next_preloaded_scene_id;
        if next == 0 {
            next = 1;
        }
        let id = PreloadedSceneID::from_u64(next);
        self.next_preloaded_scene_id = next.saturating_add(1);
        self.preloaded_scenes.insert(id, scene);
        self.preloaded_scene_paths.insert(path_hash, id);
        self.preloaded_scene_reverse_paths
            .insert(id, path.to_string());
        Ok(id)
    }

    pub(crate) fn free_preloaded_scene_at_runtime(&mut self, id: PreloadedSceneID) -> bool {
        if id.is_nil() {
            return false;
        }
        let removed = self.preloaded_scenes.remove(&id).is_some();
        if let Some(path) = self.preloaded_scene_reverse_paths.remove(&id) {
            self.preloaded_scene_paths
                .remove(&Self::source_hash(path.as_str()));
            let _ = self.scene_cache.borrow_mut().remove(path.as_str());
        }
        removed
    }

    pub(crate) fn free_preloaded_scene_by_path_at_runtime(&mut self, path: &str) -> bool {
        self.free_preloaded_scene_by_path_at_runtime_hashed(Self::source_hash(path), path)
    }

    pub(crate) fn free_preloaded_scene_by_path_at_runtime_hashed(
        &mut self,
        path_hash: u64,
        path: &str,
    ) -> bool {
        let mut removed = false;
        if let Some(id) = self.preloaded_scene_paths.remove(&path_hash) {
            removed |= self.preloaded_scenes.remove(&id).is_some();
            self.preloaded_scene_reverse_paths.remove(&id);
        }
        removed |= self.scene_cache.borrow_mut().remove(path).is_some();
        removed
    }

    pub(crate) fn load_preloaded_scene_at_runtime(
        &mut self,
        id: PreloadedSceneID,
    ) -> Result<NodeID, String> {
        let scene = self
            .preloaded_scenes
            .get(&id)
            .cloned()
            .ok_or_else(|| format!("preloaded scene id `{}` is not valid", id.as_u64()))?;
        let prepared = prepare_scene_with_loader(scene.as_ref(), &|import_path| {
            self.resolve_scene_by_path(import_path)
        })?;
        let merged = merge_prepared_scene(self, prepared)?;
        self.rebuild_internal_node_schedules();
        self.rebuild_node_tag_index();
        self.attach_scene_scripts(merged.script_nodes)?;
        Ok(merged.scene_root)
    }

    pub(crate) fn load_scene_at_runtime(&mut self, path: &str) -> Result<NodeID, String> {
        self.load_scene_at_runtime_hashed(Self::source_hash(path), path)
    }

    pub(crate) fn load_scene_at_runtime_hashed(
        &mut self,
        path_hash: u64,
        path: &str,
    ) -> Result<NodeID, String> {
        let static_lookup = self
            .project()
            .and_then(|project| project.static_scene_lookup);
        let merged = match self.provider_mode {
            ProviderMode::Dynamic => {
                let runtime_scene = self.get_or_load_dynamic_scene_cached(path)?;
                let prepared = prepare_scene_with_loader(runtime_scene.as_ref(), &|import_path| {
                    self.resolve_scene_by_path(import_path)
                })?;
                merge_prepared_scene(self, prepared)?
            }
            ProviderMode::Static => {
                if let Some(lookup) = static_lookup {
                    let scene = lookup(path_hash);
                    let prepared = prepare_scene_with_loader(scene, &|import_path| {
                        self.resolve_scene_by_path(import_path)
                    })?;
                    merge_prepared_scene(self, prepared)?
                } else {
                    let runtime_scene = self.get_or_load_dynamic_scene_cached(path)?;
                    let prepared =
                        prepare_scene_with_loader(runtime_scene.as_ref(), &|import_path| {
                            self.resolve_scene_by_path(import_path)
                        })?;
                    merge_prepared_scene(self, prepared)?
                }
            }
        };

        self.rebuild_internal_node_schedules();
        self.rebuild_node_tag_index();
        self.attach_scene_scripts(merged.script_nodes)?;
        #[cfg(not(feature = "profile"))]
        let _ = path;
        Ok(merged.scene_root)
    }

    pub(crate) fn load_boot_scene(&mut self) -> Result<(), String> {
        #[cfg(feature = "profile")]
        let boot_start = Instant::now();
        let (
            project_root,
            project_name,
            main_scene_path,
            main_scene_hash,
            static_lookup,
            perro_assets_bytes,
        ) = {
            let project = self
                .project()
                .ok_or_else(|| "Runtime project is not set".to_string())?;
            (
                project.root.clone(),
                project.config.name.clone(),
                project.config.main_scene.clone(),
                project
                    .config
                    .main_scene_hash
                    .unwrap_or_else(|| string_to_u64(&project.config.main_scene)),
                project.static_scene_lookup,
                project.perro_assets_bytes,
            )
        };

        if self.provider_mode == ProviderMode::Static {
            if let Some(data) = perro_assets_bytes {
                set_project_root(ProjectRoot::PerroAssets {
                    data,
                    name: project_name,
                });
            } else {
                set_project_root(ProjectRoot::Disk {
                    root: project_root,
                    name: project_name,
                });
            }
        } else {
            set_project_root(ProjectRoot::Disk {
                root: project_root,
                name: project_name,
            });
        }
        self.resource_api.initialize_localization();

        let mut existing_script_ids = Vec::new();
        self.scripts.append_instance_ids(&mut existing_script_ids);
        for id in existing_script_ids {
            let _ = self.remove_script_instance(id);
        }

        self.nodes.clear();
        self.clear_physics();
        self.scripts = Default::default();
        self.script_runtime.pending_start_scripts.clear();
        self.script_runtime.pending_start_flags.clear();
        self.clear_internal_node_schedules();
        self.render_2d.traversal_ids.clear();
        self.render_2d.visible_now.clear();
        self.render_2d.prev_visible.clear();
        self.render_2d.retained_sprite_textures.clear();
        self.render_2d.texture_sources.clear();
        self.render_2d.removed_nodes.clear();
        self.render_3d.traversal_ids.clear();
        self.render_3d.visible_now.clear();
        self.render_3d.prev_visible.clear();
        self.render_3d.mesh_sources.clear();
        self.render_3d.material_surface_sources.clear();
        self.render_3d.material_surface_overrides.clear();
        self.render_3d.particle_path_cache.clear();
        self.render_3d.particle_path_cache_order.clear();
        self.render_3d.removed_nodes.clear();
        if self.provider_mode == ProviderMode::Dynamic {
            self.script_runtime.dynamic_script_registry.clear();
        }
        self.script_runtime.script_library = None;
        self.node_index.node_tag_index.clear();
        let mode_label;
        #[cfg(feature = "profile")]
        let mut source_load: Option<Duration> = None;
        #[cfg(feature = "profile")]
        let mut parse: Option<Duration> = None;
        #[cfg(feature = "profile")]
        let mut node_insert = Duration::ZERO;
        let merged;
        match self.provider_mode {
            ProviderMode::Dynamic => {
                mode_label = "dynamic";
                let (runtime_scene, load_stats) = load_runtime_scene_from_disk(&main_scene_path)?;
                #[cfg(feature = "profile")]
                {
                    source_load = Some(load_stats.source_load);
                    parse = Some(load_stats.parse);
                }
                let prepared = prepare_scene_with_loader(&runtime_scene, &|path| {
                    let (scene, _) = load_runtime_scene_from_disk(path)?;
                    Ok(Arc::new(scene))
                })?;
                #[cfg(feature = "profile")]
                let node_insert_start = Instant::now();
                merged = merge_prepared_scene(self, prepared)?;
                #[cfg(feature = "profile")]
                {
                    node_insert = node_insert_start.elapsed();
                }
                #[cfg(not(feature = "profile"))]
                {
                    let _ = (load_stats,);
                }
            }
            ProviderMode::Static => {
                if let Some(lookup) = static_lookup {
                    let scene = lookup(main_scene_hash);
                    mode_label = "static";
                    let prepared = prepare_scene_with_loader(scene, &|path| {
                        Ok(Arc::new(lookup(Self::source_hash(path)).clone()))
                    })?;
                    #[cfg(feature = "profile")]
                    let node_insert_start = Instant::now();
                    merged = merge_prepared_scene(self, prepared)?;
                    #[cfg(feature = "profile")]
                    {
                        node_insert = node_insert_start.elapsed();
                    }
                } else {
                    mode_label = "static_fallback_dynamic";
                    let (runtime_scene, load_stats) =
                        load_runtime_scene_from_disk(&main_scene_path)?;
                    #[cfg(feature = "profile")]
                    {
                        source_load = Some(load_stats.source_load);
                        parse = Some(load_stats.parse);
                    }
                    let prepared = prepare_scene_with_loader(&runtime_scene, &|path| {
                        let (scene, _) = load_runtime_scene_from_disk(path)?;
                        Ok(Arc::new(scene))
                    })?;
                    #[cfg(feature = "profile")]
                    let node_insert_start = Instant::now();
                    merged = merge_prepared_scene(self, prepared)?;
                    #[cfg(feature = "profile")]
                    {
                        node_insert = node_insert_start.elapsed();
                    }
                    #[cfg(not(feature = "profile"))]
                    {
                        let _ = (load_stats,);
                    }
                }
            }
        }
        self.rebuild_internal_node_schedules();
        self.rebuild_node_tag_index();
        self.attach_scene_scripts(merged.script_nodes)?;
        #[cfg(not(feature = "profile"))]
        {
            let _ = mode_label;
        }
        #[cfg(feature = "profile")]
        let stats = SceneLoadStats {
            mode_label,
            source_load,
            parse,
            node_insert,
            total_excluding_debug_print: boot_start.elapsed(),
        };
        #[cfg(feature = "profile")]
        println!(
            "[scene_load] mode={} path={} total_us={:.3} source_us={} parse_us={} insert_us={:.3}",
            stats.mode_label,
            main_scene_path,
            as_us(stats.total_excluding_debug_print),
            fmt_duration(stats.source_load),
            fmt_duration(stats.parse),
            as_us(stats.node_insert),
        );
        #[cfg(not(feature = "profile"))]
        let _ = main_scene_path;
        Ok(())
    }
}

#[cfg(feature = "profile")]
fn as_us(duration: Duration) -> f64 {
    duration.as_secs_f64() * 1_000_000.0
}

#[cfg(feature = "profile")]
fn fmt_duration(duration: Option<Duration>) -> String {
    duration
        .map(|value| format!("{:.3}", as_us(value)))
        .unwrap_or_else(|| "n/a".to_string())
}
