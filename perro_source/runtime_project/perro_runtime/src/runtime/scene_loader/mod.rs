use crate::{Runtime, runtime_project::ProviderMode};
use perro_ids::NodeID;
use perro_ids::ScriptMemberID;
use perro_ids::parse_hashed_source_uri;
use perro_ids::string_to_u64;
use perro_io::{ProjectRoot, clear_dlc_mounts, try_set_project_root};
#[cfg(not(target_arch = "wasm32"))]
use perro_io::{
    data_local_dir, is_reserved_dlc_name, mount_dlc_archive, mount_dlc_disk, read_mounted_dlc_file,
    register_dlc_static_binary_lookup, validate_asset_relative_path,
};
use perro_runtime_api::sub_apis::NodeAPI;
use perro_runtime_api::sub_apis::PreloadedSceneID;
use perro_scene::Scene;
use perro_variant::Variant;
#[cfg(not(target_arch = "wasm32"))]
use std::fs;
#[cfg(not(target_arch = "wasm32"))]
use std::io::{self, Write};
#[cfg(not(target_arch = "wasm32"))]
use std::path::{Path, PathBuf};
use std::sync::Arc;
#[cfg(feature = "profile")]
use std::time::{Duration, Instant};

mod merge;
pub(crate) mod prepare;

use merge::merge_prepared_scene;
use prepare::{load_runtime_scene_from_disk, prepare_scene_with_loader_and_styles};

#[cfg(feature = "bench")]
pub fn bench_prepare_scene(scene: &Scene) -> Result<(usize, usize), String> {
    let prepared = prepare_scene_with_loader_and_styles(
        scene,
        &|path| Err(format!("bench scene import unsupported: {path}")),
        None,
    )?;
    Ok((prepared.nodes.len(), prepared.scripts.len()))
}

#[cfg(feature = "bench")]
pub struct BenchPreparedScene(prepare::PreparedScene);

#[cfg(feature = "bench")]
pub struct BenchSceneSpawner(Runtime);

#[cfg(feature = "bench")]
impl BenchSceneSpawner {
    pub fn new() -> Self {
        Self(Runtime::new())
    }

    pub fn spawn(&mut self, scene: &BenchPreparedScene) -> Result<usize, String> {
        let _ = merge_prepared_scene(&mut self.0, scene.0.clone())?;
        Ok(self.0.nodes.len())
    }

    pub fn spawn_uncompiled(&mut self, scene: &Scene) -> Result<usize, String> {
        let prepared = prepare_scene_with_loader_and_styles(
            scene,
            &|path| Err(format!("bench scene import unsupported: {path}")),
            None,
        )?;
        let _ = merge_prepared_scene(&mut self.0, prepared)?;
        Ok(self.0.nodes.len())
    }
}

#[cfg(feature = "bench")]
impl Default for BenchSceneSpawner {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(feature = "bench")]
pub fn bench_compile_scene(scene: &Scene) -> Result<BenchPreparedScene, String> {
    prepare_scene_with_loader_and_styles(
        scene,
        &|path| Err(format!("bench scene import unsupported: {path}")),
        None,
    )
    .map(BenchPreparedScene)
}

#[cfg(feature = "bench")]
pub fn bench_merge_compiled_scene(scene: &BenchPreparedScene) -> Result<usize, String> {
    let mut runtime = Runtime::new();
    let _ = merge_prepared_scene(&mut runtime, scene.0.clone())?;
    Ok(runtime.nodes.len())
}

#[cfg(feature = "bench")]
pub fn bench_prepare_and_merge_scene(scene: &Scene) -> Result<usize, String> {
    let prepared = prepare_scene_with_loader_and_styles(
        scene,
        &|path| Err(format!("bench scene import unsupported: {path}")),
        None,
    )?;
    let mut runtime = Runtime::new();
    let _ = merge_prepared_scene(&mut runtime, prepared)?;
    Ok(runtime.nodes.len())
}

#[cfg(feature = "bench")]
pub fn bench_prepare_merge_extract_scene(scene: &Scene) -> Result<(usize, usize), String> {
    let prepared = prepare_scene_with_loader_and_styles(
        scene,
        &|path| Err(format!("bench scene import unsupported: {path}")),
        None,
    )?;
    let mut runtime = Runtime::new();
    let _ = merge_prepared_scene(&mut runtime, prepared)?;
    let mut commands = Vec::new();
    runtime.extract_render_snapshot_commands(&mut commands);
    Ok((runtime.nodes.len(), commands.len()))
}

pub(crate) struct PendingScriptAttach {
    pub(crate) node_id: NodeID,
    pub(crate) script_path_hash: u64,
    pub(crate) script_mount: Option<String>,
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
        if let Some(id) = self.preloaded_scene_paths.get(&path_hash).copied()
            && let Some(scene) = self.preloaded_scenes.get(&id)
        {
            return Ok(scene.clone());
        }
        match self.provider_mode {
            ProviderMode::Dynamic => self.get_or_load_dynamic_scene_cached(path),
            ProviderMode::Static => {
                if path.starts_with("dlc://") {
                    return self.get_or_load_dynamic_scene_cached(path);
                }
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

    fn route_scene_path(&self, href: &str) -> Option<String> {
        let href = perro_project::normalize_route_href(href);
        self.project()
            .and_then(|project| project.routes.scene_for_href(&href))
            .map(str::to_string)
    }

    fn initial_route_scene_for_href(&self, browser_href: Option<&str>) -> Option<(String, String)> {
        let href = perro_project::normalize_route_href(browser_href?);
        if let Some(scene) = self.route_scene_path(&href) {
            return Some((href, scene));
        }
        self.route_scene_path("/")
            .map(|scene| ("/".to_string(), scene))
    }

    fn initial_route_scene(&self) -> Option<(String, String)> {
        self.initial_route_scene_for_href(perro_web::current_href().as_deref())
    }

    pub(crate) fn process_pending_web_route_change(&mut self) {
        let Some(next_href) = perro_web::take_pending_route_change() else {
            return;
        };
        let _ = self.apply_route_change(&next_href);
    }

    fn apply_route_change(&mut self, next_href: &str) -> Result<(), String> {
        let next_href = perro_project::normalize_route_href(next_href);
        if self.active_route_href.as_deref() == Some(next_href.as_str()) {
            return Ok(());
        }
        let Some(scene_path) = self.route_scene_path(&next_href) else {
            return Err(format!("route `{next_href}` not found"));
        };
        let root = self.load_scene_at_runtime(&scene_path)?;
        if let Some(old_root) = self.active_route_root {
            let _ = NodeAPI::remove_node(self, old_root);
        }
        self.active_route_href = Some(next_href);
        self.active_route_root = Some(root);
        Ok(())
    }

    fn finish_scene_merge(
        &mut self,
        merged: merge::MergePreparedSceneResult,
    ) -> Result<NodeID, String> {
        let scene_root = merged.scene_root;
        let ownership_root = merged.ownership_root;
        if !merged.script_nodes.is_empty()
            && let Err(err) = self.attach_scene_scripts(merged.script_nodes)
        {
            let _ = NodeAPI::remove_node(self, ownership_root);
            return Err(err);
        }
        self.scene_ownership_roots
            .insert(scene_root, ownership_root);
        Ok(scene_root)
    }

    fn prepare_scene_with_project_styles(
        &self,
        scene: &Scene,
        load_scene: &dyn Fn(&str) -> Result<Arc<Scene>, String>,
    ) -> Result<prepare::PreparedScene, String> {
        let static_ui_style_lookup = self
            .project()
            .and_then(|project| project.static_ui_style_lookup);
        prepare_scene_with_loader_and_styles(scene, load_scene, static_ui_style_lookup)
    }

    fn get_or_prepare_scene_cached(
        &self,
        path: &str,
        scene: &Scene,
    ) -> Result<Arc<prepare::PreparedScene>, String> {
        if let Some(prepared) = self.prepared_scene_cache.borrow().get(path).cloned() {
            return Ok(prepared);
        }
        let prepared = Arc::new(
            self.prepare_scene_with_project_styles(scene, &|import_path| {
                self.resolve_scene_by_path(import_path)
            })?,
        );
        self.prepared_scene_cache
            .borrow_mut()
            .insert(path.to_string(), prepared.clone());
        Ok(prepared)
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
        let prepared = self.get_or_prepare_scene_cached(path, scene.as_ref())?;
        self.preloaded_scenes.insert(id, scene);
        self.preloaded_prepared_scenes.insert(id, prepared);
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
        self.preloaded_prepared_scenes.remove(&id);
        if let Some(path) = self.preloaded_scene_reverse_paths.remove(&id) {
            self.preloaded_scene_paths
                .remove(&Self::source_hash(path.as_str()));
            let _ = self.scene_cache.borrow_mut().remove(path.as_str());
            let _ = self.prepared_scene_cache.borrow_mut().remove(path.as_str());
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
            self.preloaded_prepared_scenes.remove(&id);
            self.preloaded_scene_reverse_paths.remove(&id);
        }
        removed |= self.scene_cache.borrow_mut().remove(path).is_some();
        self.prepared_scene_cache.borrow_mut().remove(path);
        removed
    }

    pub(crate) fn load_preloaded_scene_at_runtime(
        &mut self,
        id: PreloadedSceneID,
    ) -> Result<NodeID, String> {
        let prepared = self
            .preloaded_prepared_scenes
            .get(&id)
            .cloned()
            .ok_or_else(|| format!("preloaded scene id `{}` is not valid", id.as_u64()))?;
        let merged = merge_prepared_scene(self, prepared.as_ref().clone())?;
        self.finish_scene_merge(merged)
    }

    pub(crate) fn load_scene_at_runtime(&mut self, path: &str) -> Result<NodeID, String> {
        self.load_scene_at_runtime_hashed(Self::source_hash(path), path)
    }

    pub(crate) fn load_scene_doc_at_runtime(&mut self, scene: Scene) -> Result<NodeID, String> {
        let prepared = self.prepare_scene_with_project_styles(&scene, &|import_path| {
            self.resolve_scene_by_path(import_path)
        })?;
        let merged = merge_prepared_scene(self, prepared)?;
        self.finish_scene_merge(merged)
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
                let prepared = self.get_or_prepare_scene_cached(path, runtime_scene.as_ref())?;
                merge_prepared_scene(self, prepared.as_ref().clone())?
            }
            ProviderMode::Static => {
                if path.starts_with("dlc://") {
                    let runtime_scene = self.get_or_load_dynamic_scene_cached(path)?;
                    let prepared =
                        self.get_or_prepare_scene_cached(path, runtime_scene.as_ref())?;
                    merge_prepared_scene(self, prepared.as_ref().clone())?
                } else if let Some(lookup) = static_lookup {
                    let scene = lookup(path_hash);
                    let prepared = self.get_or_prepare_scene_cached(path, scene)?;
                    merge_prepared_scene(self, prepared.as_ref().clone())?
                } else {
                    let runtime_scene = self.get_or_load_dynamic_scene_cached(path)?;
                    let prepared =
                        self.get_or_prepare_scene_cached(path, runtime_scene.as_ref())?;
                    merge_prepared_scene(self, prepared.as_ref().clone())?
                }
            }
        };

        let scene_root = self.finish_scene_merge(merged)?;
        #[cfg(not(feature = "profile"))]
        let _ = path;
        Ok(scene_root)
    }

    pub(crate) fn load_boot_scene(&mut self) -> Result<(), String> {
        #[cfg(feature = "profile")]
        let boot_start = Instant::now();
        let (
            project_root,
            project_name,
            boot_scene_path,
            boot_scene_hash,
            boot_route_href,
            static_lookup,
            static_resource_lookups,
            perro_assets_bytes,
        ) = {
            let project = self
                .project()
                .ok_or_else(|| "Runtime project is not set".to_string())?;
            let mut route_href = None;
            let mut scene_path = project.config.main_scene.clone();
            if let Some((href, route_scene)) = self.initial_route_scene() {
                route_href = Some(href);
                scene_path = route_scene;
            }
            (
                project.root.clone(),
                project.config.name.clone(),
                scene_path.clone(),
                parse_hashed_source_uri(&scene_path).unwrap_or_else(|| {
                    if scene_path == project.config.main_scene {
                        project
                            .config
                            .main_scene_hash
                            .unwrap_or_else(|| string_to_u64(&project.config.main_scene))
                    } else {
                        string_to_u64(&scene_path)
                    }
                }),
                route_href,
                project.static_scene_lookup,
                project.static_resource_lookups,
                project.perro_assets_bytes,
            )
        };

        if self.provider_mode == ProviderMode::Static {
            if let Some(data) = perro_assets_bytes {
                try_set_project_root(ProjectRoot::PerroAssets {
                    data,
                    name: project_name,
                    static_resource_lookups,
                })
                .map_err(|err| format!("failed to set project asset root: {err}"))?;
            } else {
                try_set_project_root(ProjectRoot::Disk {
                    root: project_root,
                    name: project_name,
                })
                .map_err(|err| format!("failed to set project asset root: {err}"))?;
            }
        } else {
            try_set_project_root(ProjectRoot::Disk {
                root: project_root,
                name: project_name,
            })
            .map_err(|err| format!("failed to set project asset root: {err}"))?;
        }
        self.reload_dlc_mounts()?;
        self.resource_api.initialize_localization();

        let mut existing_script_ids = Vec::new();
        self.scripts.append_instance_ids(&mut existing_script_ids);
        for id in existing_script_ids {
            let _ = self.remove_script_instance(id);
        }

        self.nodes.clear();
        self.scene_ownership_roots.clear();
        self.clear_physics();
        self.force_water_impacts_2d.clear();
        self.force_water_impacts_3d.clear();
        self.water_entry_states_3d.clear();
        self.pending_force_emitters_2d.clear();
        self.pending_force_emitters_3d.clear();
        self.scripts = Default::default();
        self.script_runtime.pending_start_scripts.clear();
        self.script_runtime.pending_start_flags.clear();
        self.clear_internal_node_schedules();
        self.render_2d.traversal_ids.clear();
        self.render_2d.traversal_child_scratch.clear();
        self.render_2d.visible_now.clear();
        self.render_2d.prev_visible.clear();
        self.render_2d.retained_sprites.clear();
        self.render_2d.tileset_cache.clear();
        self.render_2d.texture_sources.clear();
        self.render_2d.last_camera = None;
        self.render_2d.removed_nodes.clear();
        self.render_3d.traversal_ids.clear();
        self.render_3d.traversal_child_scratch.clear();
        self.render_3d.visible_now.clear();
        self.render_3d.prev_visible.clear();
        self.render_3d.mesh_sources.clear();
        self.render_3d.clear_skeleton_mesh_index();
        self.render_3d.material_surface_sources.clear();
        self.render_3d.material_surface_overrides.clear();
        self.render_3d.particle_path_cache.clear();
        self.render_3d.particle_path_cache_order.clear();
        self.render_3d.removed_nodes.clear();
        self.render_ui.traversal_ids.clear();
        self.render_ui.traversal_seen.clear();
        self.render_ui.command_ids.clear();
        self.render_ui.command_seen.clear();
        self.render_ui.visible_now.clear();
        self.render_ui.prev_visible.clear();
        self.render_ui.computed_rects.clear();
        self.render_ui.size_clamp_baselines.borrow_mut().clear();
        self.render_ui.computed_scales.clear();
        self.render_ui.auto_layout_computed.clear();
        self.render_ui.retained_commands.clear();
        self.render_ui.retained_rects.clear();
        self.render_ui.button_states.clear();
        self.render_ui.focused_ui_node = None;
        self.render_ui.nav_pressed_button = None;
        self.render_ui.ui_nav_repeat_dir = None;
        self.render_ui.ui_nav_repeat_timer = 0.0;
        self.render_ui.focused_text_edit = None;
        self.render_ui.hovered_text_edit = None;
        self.render_ui.pressed_text_edit = None;
        self.render_ui.pressed_ui_button = None;
        self.render_ui.last_ui_pointer = None;
        self.render_ui.cursor_icon = perro_ui::CursorIcon::Default;
        self.render_ui.cursor_icon_2d = perro_ui::CursorIcon::Default;
        self.render_ui.cursor_icon_ui = perro_ui::CursorIcon::Default;
        self.render_ui.cursor_icon_script = perro_ui::CursorIcon::Default;
        self.render_ui.removed_nodes.clear();
        self.locale_text.bindings.clear();
        self.locale_text.last_epoch = self.resource_api.localization_epoch();
        if self.provider_mode == ProviderMode::Dynamic {
            self.script_runtime.dynamic_script_registry.clear();
            self.script_runtime.base_scripts_loaded = false;
        }
        self.script_runtime.loaded_dlc_script_libs.clear();
        self.script_runtime.script_instance_dlc_mounts.clear();
        self.script_runtime.script_behavior_cache.clear();
        self.script_runtime.script_libraries.clear();
        self.active_route_href = None;
        self.active_route_root = None;
        let mode_label;
        #[cfg(feature = "profile")]
        let mut source_load: Option<Duration> = None;
        #[cfg(feature = "profile")]
        let mut parse: Option<Duration> = None;
        #[cfg(feature = "profile")]
        let node_insert: Option<Duration>;
        let merged;
        match self.provider_mode {
            ProviderMode::Dynamic => {
                mode_label = "dynamic";
                let (runtime_scene, load_stats) = load_runtime_scene_from_disk(&boot_scene_path)?;
                #[cfg(feature = "profile")]
                {
                    source_load = Some(load_stats.source_load);
                    parse = Some(load_stats.parse);
                }
                let prepared = self.prepare_scene_with_project_styles(&runtime_scene, &|path| {
                    let (scene, _) = load_runtime_scene_from_disk(path)?;
                    Ok(Arc::new(scene))
                })?;
                #[cfg(feature = "profile")]
                let node_insert_start = Instant::now();
                merged = merge_prepared_scene(self, prepared)?;
                #[cfg(feature = "profile")]
                {
                    node_insert = Some(node_insert_start.elapsed());
                }
                #[cfg(not(feature = "profile"))]
                {
                    let _ = (load_stats,);
                }
            }
            ProviderMode::Static => {
                if let Some(lookup) = static_lookup {
                    let scene = lookup(boot_scene_hash);
                    mode_label = "static";
                    let prepared = self.prepare_scene_with_project_styles(scene, &|path| {
                        self.resolve_scene_by_path(path)
                    })?;
                    #[cfg(feature = "profile")]
                    let node_insert_start = Instant::now();
                    merged = merge_prepared_scene(self, prepared)?;
                    #[cfg(feature = "profile")]
                    {
                        node_insert = Some(node_insert_start.elapsed());
                    }
                } else {
                    mode_label = "static_fallback_dynamic";
                    let (runtime_scene, load_stats) =
                        load_runtime_scene_from_disk(&boot_scene_path)?;
                    #[cfg(feature = "profile")]
                    {
                        source_load = Some(load_stats.source_load);
                        parse = Some(load_stats.parse);
                    }
                    let prepared =
                        self.prepare_scene_with_project_styles(&runtime_scene, &|path| {
                            let (scene, _) = load_runtime_scene_from_disk(path)?;
                            Ok(Arc::new(scene))
                        })?;
                    #[cfg(feature = "profile")]
                    let node_insert_start = Instant::now();
                    merged = merge_prepared_scene(self, prepared)?;
                    #[cfg(feature = "profile")]
                    {
                        node_insert = Some(node_insert_start.elapsed());
                    }
                    #[cfg(not(feature = "profile"))]
                    {
                        let _ = (load_stats,);
                    }
                }
            }
        }
        let scene_root = self.finish_scene_merge(merged)?;
        self.active_route_href = boot_route_href;
        self.active_route_root = Some(scene_root);
        #[cfg(not(feature = "profile"))]
        {
            let _ = mode_label;
        }
        #[cfg(feature = "profile")]
        let stats = SceneLoadStats {
            mode_label,
            source_load,
            parse,
            node_insert: node_insert.unwrap_or(Duration::ZERO),
            total_excluding_debug_print: boot_start.elapsed(),
        };
        #[cfg(feature = "profile")]
        println!(
            "[scene_load] mode={} path={} total_us={:.3} source_us={} parse_us={} insert_us={:.3}",
            stats.mode_label,
            boot_scene_path,
            as_us(stats.total_excluding_debug_print),
            fmt_duration(stats.source_load),
            fmt_duration(stats.parse),
            as_us(stats.node_insert),
        );
        #[cfg(not(feature = "profile"))]
        let _ = boot_scene_path;
        Ok(())
    }

    #[cfg(not(target_arch = "wasm32"))]
    fn reload_dlc_mounts(&mut self) -> Result<(), String> {
        clear_dlc_mounts();
        self.script_runtime.mounted_dlc_script_libs.clear();

        let Some(project) = self.project() else {
            return Ok(());
        };
        let project_root = project.root.clone();
        let project_name = project.config.name.clone();

        let dev_dlcs = project_root.join("dlcs");
        if dev_dlcs.exists() {
            let entries = fs::read_dir(&dev_dlcs)
                .map_err(|err| format!("failed to scan dlc dir `{}`: {err}", dev_dlcs.display()))?;
            for entry in entries {
                let entry = entry.map_err(|err| {
                    format!(
                        "failed to read dlc entry in `{}`: {err}",
                        dev_dlcs.display()
                    )
                })?;
                let path = entry.path();
                if !path.is_dir() {
                    continue;
                }
                let Some(name) = path.file_name().and_then(|v| v.to_str()) else {
                    continue;
                };
                if is_reserved_dlc_name(name) {
                    eprintln!(
                        "warning: skipping dev dlc mount with reserved name `self` at {}",
                        path.display()
                    );
                    continue;
                }
                mount_dlc_disk(name, &path).map_err(|err| {
                    format!(
                        "failed to mount dev dlc `{}` from `{}`: {err}",
                        name,
                        path.display()
                    )
                })?;
                if let Some(script_dylib) = resolve_dev_dlc_scripts_dylib_path(&project_root, name)
                {
                    self.script_runtime
                        .mounted_dlc_script_libs
                        .insert(name.to_ascii_lowercase(), script_dylib);
                }
            }
        }

        // Dynamic provider should always resolve dlc:// from source disk mounts.
        // Do not mount installed .dlc archives in this mode.
        if self.provider_mode == ProviderMode::Dynamic {
            return Ok(());
        }

        let install_root = data_local_dir()
            .unwrap_or_else(std::env::temp_dir)
            .join(project_name)
            .join("dlc");
        if install_root.exists() {
            let entries = fs::read_dir(&install_root).map_err(|err| {
                format!(
                    "failed to scan installed dlc dir `{}`: {err}",
                    install_root.display()
                )
            })?;
            for entry in entries {
                let entry = entry.map_err(|err| {
                    format!(
                        "failed to read installed dlc entry in `{}`: {err}",
                        install_root.display()
                    )
                })?;
                let path = entry.path();
                if !path.is_file() {
                    continue;
                }
                if path.extension().and_then(|v| v.to_str()) != Some("dlc") {
                    continue;
                }
                let Some(stem) = path.file_stem().and_then(|v| v.to_str()) else {
                    continue;
                };
                if is_reserved_dlc_name(stem) {
                    eprintln!(
                        "warning: skipping installed dlc archive with reserved name `self` at {}",
                        path.display()
                    );
                    continue;
                }
                mount_dlc_archive(stem, &path).map_err(|err| {
                    format!(
                        "failed to mount installed dlc `{}` archive `{}`: {err}",
                        stem,
                        path.display()
                    )
                })?;

                let manifest_bytes =
                    read_mounted_dlc_file(stem, "manifest.toml").map_err(|err| {
                        format!(
                            "failed to read manifest.toml from dlc `{}` (`{}`): {err}",
                            stem,
                            path.display()
                        )
                    })?;
                let manifest_text = String::from_utf8(manifest_bytes).map_err(|err| {
                    format!(
                        "manifest.toml in dlc `{}` is not valid UTF-8 (`{}`): {err}",
                        stem,
                        path.display()
                    )
                })?;
                let script_rel = parse_manifest_string(&manifest_text, "script_lib")
                    .unwrap_or_else(|| format!("scripts/{}", runtime_scripts_dylib_name()));
                let pack_rel = parse_manifest_string(&manifest_text, "pack_lib")
                    .unwrap_or_else(|| format!("pack/{}", runtime_pack_dylib_name()));

                let extract_root =
                    ensure_secure_cache_dir(&install_root, &Path::new(".runtime_cache").join(stem))
                        .map_err(|err| {
                            format!(
                                "failed to create dlc runtime cache dir `{}`: {err}",
                                install_root.join(".runtime_cache").join(stem).display()
                            )
                        })?;

                let script_path =
                    extract_dlc_archive_file_to_cache(stem, &script_rel, &extract_root).map_err(
                        |err| {
                            format!(
                                "failed to extract script lib `{}` from dlc `{}`: {err}",
                                script_rel, stem
                            )
                        },
                    )?;
                self.script_runtime
                    .mounted_dlc_script_libs
                    .insert(stem.to_ascii_lowercase(), script_path);

                let pack_path = extract_dlc_archive_file_to_cache(stem, &pack_rel, &extract_root)
                    .map_err(|err| {
                    format!(
                        "failed to extract pack lib `{}` from dlc `{}`: {err}",
                        pack_rel, stem
                    )
                })?;
                // SAFETY: Pack library comes from extracted Perro DLC build output.
                // Library is stored after symbol registration so static lookup code stays loaded.
                if let Ok(lib) = unsafe { libloading::Library::new(&pack_path) } {
                    // SAFETY: Symbol ABI is generated by Perro compiler. Copied function
                    // pointer remains valid while the library is held in script_libraries.
                    unsafe {
                        type Lookup = unsafe extern "C" fn(u64, *mut *const u8, *mut usize) -> bool;
                        if let Ok(symbol) = lib.get::<Lookup>(b"perro_dlc_pack_lookup") {
                            // SAFETY: Compiler-generated callback returns immutable static
                            // bytes and the pack library remains loaded for runtime lifetime.
                            register_dlc_static_binary_lookup(stem, *symbol);
                        }
                    }
                    self.script_runtime.script_libraries.push(lib);
                }
            }
        }

        Ok(())
    }

    #[cfg(target_arch = "wasm32")]
    fn reload_dlc_mounts(&mut self) -> Result<(), String> {
        clear_dlc_mounts();
        self.script_runtime.mounted_dlc_script_libs.clear();
        self.script_runtime.loaded_dlc_script_libs.clear();
        Ok(())
    }
}

mod dlc_cache;
#[cfg(any(not(target_arch = "wasm32"), feature = "profile"))]
use dlc_cache::*;

#[cfg(test)]
#[path = "tests/mod.rs"]
mod tests;
