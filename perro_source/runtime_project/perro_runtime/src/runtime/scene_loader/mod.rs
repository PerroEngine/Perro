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
mod prepare;

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
        let prepared = self.prepare_scene_with_project_styles(scene.as_ref(), &|import_path| {
            self.resolve_scene_by_path(import_path)
        })?;
        let merged = merge_prepared_scene(self, prepared)?;
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
                let prepared = self
                    .prepare_scene_with_project_styles(runtime_scene.as_ref(), &|import_path| {
                        self.resolve_scene_by_path(import_path)
                    })?;
                merge_prepared_scene(self, prepared)?
            }
            ProviderMode::Static => {
                if path.starts_with("dlc://") {
                    let runtime_scene = self.get_or_load_dynamic_scene_cached(path)?;
                    let prepared = self.prepare_scene_with_project_styles(
                        runtime_scene.as_ref(),
                        &|import_path| self.resolve_scene_by_path(import_path),
                    )?;
                    merge_prepared_scene(self, prepared)?
                } else if let Some(lookup) = static_lookup {
                    let scene = lookup(path_hash);
                    let prepared = self
                        .prepare_scene_with_project_styles(scene, &|import_path| {
                            self.resolve_scene_by_path(import_path)
                        })?;
                    merge_prepared_scene(self, prepared)?
                } else {
                    let runtime_scene = self.get_or_load_dynamic_scene_cached(path)?;
                    let prepared = self.prepare_scene_with_project_styles(
                        runtime_scene.as_ref(),
                        &|import_path| self.resolve_scene_by_path(import_path),
                    )?;
                    merge_prepared_scene(self, prepared)?
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

#[cfg(target_os = "windows")]
fn runtime_scripts_dylib_name() -> &'static str {
    "scripts.dll"
}

#[cfg(target_os = "linux")]
fn runtime_scripts_dylib_name() -> &'static str {
    "libscripts.so"
}

#[cfg(target_os = "macos")]
fn runtime_scripts_dylib_name() -> &'static str {
    "libscripts.dylib"
}

#[cfg(not(target_arch = "wasm32"))]
fn resolve_dev_dlc_scripts_dylib_path(project_root: &Path, dlc_name: &str) -> Option<PathBuf> {
    let staged = project_root
        .join(".perro")
        .join("dlc")
        .join(dlc_name)
        .join("scripts")
        .join(runtime_scripts_dylib_name());
    if staged.exists() {
        return Some(staged);
    }
    None
}

#[cfg(target_os = "windows")]
fn runtime_pack_dylib_name() -> &'static str {
    "pack.dll"
}

#[cfg(target_os = "linux")]
fn runtime_pack_dylib_name() -> &'static str {
    "libpack.so"
}

#[cfg(target_os = "macos")]
fn runtime_pack_dylib_name() -> &'static str {
    "libpack.dylib"
}

#[cfg(not(target_arch = "wasm32"))]
fn parse_manifest_string(manifest: &str, key: &str) -> Option<String> {
    for line in manifest.lines() {
        let trimmed = line.trim();
        if trimmed.starts_with('#') || !trimmed.starts_with(key) {
            continue;
        }
        let (_, rhs) = trimmed.split_once('=')?;
        let value = rhs.trim().trim_matches('"').to_string();
        if !value.is_empty() {
            return Some(value);
        }
    }
    None
}

#[cfg(not(target_arch = "wasm32"))]
fn extract_dlc_archive_file_to_cache(
    dlc_name: &str,
    virtual_path: &str,
    cache_root: &Path,
) -> Result<PathBuf, std::io::Error> {
    validate_asset_relative_path(virtual_path)?;
    let bytes = read_mounted_dlc_file(dlc_name, virtual_path)?;
    write_dlc_cache_file(cache_root, virtual_path, &bytes)
}

#[cfg(not(target_arch = "wasm32"))]
fn write_dlc_cache_file(
    cache_root: &Path,
    virtual_path: &str,
    bytes: &[u8],
) -> io::Result<PathBuf> {
    validate_asset_relative_path(virtual_path)?;
    let relative = Path::new(virtual_path);
    let parent = relative.parent().unwrap_or_else(|| Path::new(""));
    let secure_parent = ensure_secure_cache_dir(cache_root, parent)?;
    let canonical_root = cache_root.canonicalize()?;
    let mut target = cache_root.to_path_buf();
    for segment in virtual_path.split('/') {
        if !segment.is_empty() {
            target.push(segment);
        }
    }
    if target.parent() != Some(secure_parent.as_path()) {
        return Err(cache_permission_error(
            "dlc cache target escapes cache root",
        ));
    }
    reject_linked_cache_target(&target)?;

    let mut file = open_cache_target_no_follow(&target)?;
    let metadata = file.metadata()?;
    if is_link_or_reparse(&metadata) || !metadata.is_file() {
        return Err(cache_permission_error(
            "dlc cache target is link, reparse point, or non-file",
        ));
    }
    if !target.canonicalize()?.starts_with(&canonical_root) {
        return Err(cache_permission_error(
            "dlc cache target escapes cache root",
        ));
    }
    file.set_len(0)?;
    file.write_all(bytes)?;
    Ok(target)
}

#[cfg(not(target_arch = "wasm32"))]
fn ensure_secure_cache_dir(root: &Path, relative: &Path) -> io::Result<PathBuf> {
    if relative
        .components()
        .any(|component| !matches!(component, std::path::Component::Normal(_)))
        && !relative.as_os_str().is_empty()
    {
        return Err(cache_permission_error("invalid dlc cache path"));
    }

    let root_metadata = fs::symlink_metadata(root)?;
    if is_link_or_reparse(&root_metadata) || !root_metadata.is_dir() {
        return Err(cache_permission_error(
            "dlc cache root is link, reparse point, or non-directory",
        ));
    }
    let canonical_root = root.canonicalize()?;
    let mut current = root.to_path_buf();
    for component in relative.components() {
        let std::path::Component::Normal(component) = component else {
            return Err(cache_permission_error("invalid dlc cache path"));
        };
        current.push(component);
        match fs::symlink_metadata(&current) {
            Ok(metadata) => validate_cache_dir(&canonical_root, &current, &metadata)?,
            Err(err) if err.kind() == io::ErrorKind::NotFound => {
                match fs::create_dir(&current) {
                    Ok(()) => {}
                    Err(err) if err.kind() == io::ErrorKind::AlreadyExists => {}
                    Err(err) => return Err(err),
                }
                let metadata = fs::symlink_metadata(&current)?;
                validate_cache_dir(&canonical_root, &current, &metadata)?;
            }
            Err(err) => return Err(err),
        }
    }
    Ok(current)
}

#[cfg(not(target_arch = "wasm32"))]
fn validate_cache_dir(
    canonical_root: &Path,
    path: &Path,
    metadata: &fs::Metadata,
) -> io::Result<()> {
    if is_link_or_reparse(metadata) || !metadata.is_dir() {
        return Err(cache_permission_error(
            "dlc cache path contains link, reparse point, or non-directory",
        ));
    }
    if !path.canonicalize()?.starts_with(canonical_root) {
        return Err(cache_permission_error("dlc cache path escapes cache root"));
    }
    Ok(())
}

#[cfg(not(target_arch = "wasm32"))]
fn reject_linked_cache_target(path: &Path) -> io::Result<()> {
    match fs::symlink_metadata(path) {
        Ok(metadata) if is_link_or_reparse(&metadata) => Err(cache_permission_error(
            "dlc cache target is link or reparse point",
        )),
        Ok(metadata) if !metadata.is_file() => {
            Err(cache_permission_error("dlc cache target is not a file"))
        }
        Ok(_) => Ok(()),
        Err(err) if err.kind() == io::ErrorKind::NotFound => Ok(()),
        Err(err) => Err(err),
    }
}

#[cfg(not(target_arch = "wasm32"))]
fn open_cache_target_no_follow(path: &Path) -> io::Result<fs::File> {
    let mut options = fs::OpenOptions::new();
    options.write(true).create(true);
    #[cfg(any(target_os = "linux", target_os = "android"))]
    {
        use std::os::unix::fs::OpenOptionsExt;
        const O_NOFOLLOW: i32 = 0x2_0000;
        options.custom_flags(O_NOFOLLOW);
    }
    #[cfg(any(target_os = "macos", target_os = "ios"))]
    {
        use std::os::unix::fs::OpenOptionsExt;
        const O_NOFOLLOW: i32 = 0x100;
        options.custom_flags(O_NOFOLLOW);
    }
    #[cfg(windows)]
    {
        use std::os::windows::fs::OpenOptionsExt;
        const FILE_FLAG_OPEN_REPARSE_POINT: u32 = 0x20_0000;
        options.custom_flags(FILE_FLAG_OPEN_REPARSE_POINT);
    }
    options.open(path)
}

#[cfg(windows)]
fn is_link_or_reparse(metadata: &fs::Metadata) -> bool {
    use std::os::windows::fs::MetadataExt;

    const FILE_ATTRIBUTE_REPARSE_POINT: u32 = 0x400;
    metadata.file_attributes() & FILE_ATTRIBUTE_REPARSE_POINT != 0
}

#[cfg(all(not(target_arch = "wasm32"), not(windows)))]
fn is_link_or_reparse(metadata: &fs::Metadata) -> bool {
    metadata.file_type().is_symlink()
}

#[cfg(not(target_arch = "wasm32"))]
fn cache_permission_error(message: &'static str) -> io::Error {
    io::Error::new(io::ErrorKind::PermissionDenied, message)
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::rs_ctx::RuntimeResourceApi;
    use crate::runtime_project::RuntimeProject;
    use perro_nodes::{NodeType, SceneNode};
    use perro_project::LocalizationConfig;
    use perro_render_bridge::{RenderCommand, UiCommand};
    use perro_resource_api::sub_apis::{Locale, LocalizationAPI};
    use perro_scene::{Parser, Scene, SceneKey, SceneNodeData, SceneNodeEntry};
    use std::{
        borrow::Cow,
        fs,
        path::PathBuf,
        sync::atomic::{AtomicU64, Ordering},
    };

    static NEXT_CACHE_TEMP: AtomicU64 = AtomicU64::new(0);

    struct CacheTempDir(PathBuf);

    impl CacheTempDir {
        fn new(label: &str) -> Self {
            let id = NEXT_CACHE_TEMP.fetch_add(1, Ordering::Relaxed);
            let path = std::env::temp_dir().join(format!(
                "perro-runtime-cache-{label}-{}-{id}",
                std::process::id()
            ));
            let _ = fs::remove_dir_all(&path);
            fs::create_dir_all(&path).unwrap();
            Self(path)
        }
    }

    impl Drop for CacheTempDir {
        fn drop(&mut self) {
            let _ = fs::remove_dir_all(&self.0);
        }
    }

    const EMPTY_FIELDS: &[perro_scene::SceneObjectField] = &[];
    const EMPTY_KEYS: &[SceneKey] = &[];
    const EMPTY_TAGS: &[Cow<'static, str>] = &[];
    const HOST_KEY_NAMES: &[Cow<'static, str>] = &[Cow::Borrowed("wi")];
    const HOME_KEY_NAMES: &[Cow<'static, str>] = &[Cow::Borrowed("home")];
    const DOCS_KEY_NAMES: &[Cow<'static, str>] = &[Cow::Borrowed("docs"), Cow::Borrowed("copy")];
    const EMPTY_KEY_NAMES: &[Cow<'static, str>] = &[];
    const HOST_DATA: SceneNodeData =
        SceneNodeData::new(NodeType::Node, Cow::Borrowed(EMPTY_FIELDS), None);
    const HOST_NODES: &[SceneNodeEntry] = &[SceneNodeEntry {
        data: HOST_DATA,
        has_data_override: true,
        key: SceneKey(0),
        name: Some(Cow::Borrowed("wi")),
        tags: Cow::Borrowed(EMPTY_TAGS),
        children: Cow::Borrowed(EMPTY_KEYS),
        parent: None,
        script: None,
        clear_script: false,
        root_of: Some(Cow::Borrowed("dlc://test/scenes/main.scn")),
        script_vars: Cow::Borrowed(EMPTY_FIELDS),
    }];
    static HOST_SCENE: Scene = Scene {
        nodes: Cow::Borrowed(HOST_NODES),
        root: Some(SceneKey(0)),
        key_names: Cow::Borrowed(HOST_KEY_NAMES),
    };
    const HOME_DATA: SceneNodeData =
        SceneNodeData::new(NodeType::Node, Cow::Borrowed(EMPTY_FIELDS), None);
    const HOME_NODES: &[SceneNodeEntry] = &[SceneNodeEntry {
        data: HOME_DATA,
        has_data_override: true,
        key: SceneKey(0),
        name: Some(Cow::Borrowed("home")),
        tags: Cow::Borrowed(EMPTY_TAGS),
        children: Cow::Borrowed(EMPTY_KEYS),
        parent: None,
        script: None,
        clear_script: false,
        root_of: None,
        script_vars: Cow::Borrowed(EMPTY_FIELDS),
    }];
    static HOME_SCENE: Scene = Scene {
        nodes: Cow::Borrowed(HOME_NODES),
        root: Some(SceneKey(0)),
        key_names: Cow::Borrowed(HOME_KEY_NAMES),
    };
    const DOCS_CHILD_KEYS: &[SceneKey] = &[SceneKey(1)];
    const DOCS_ROOT_DATA: SceneNodeData =
        SceneNodeData::new(NodeType::Node, Cow::Borrowed(EMPTY_FIELDS), None);
    const DOCS_COPY_DATA: SceneNodeData =
        SceneNodeData::new(NodeType::Node, Cow::Borrowed(EMPTY_FIELDS), None);
    const DOCS_NODES: &[SceneNodeEntry] = &[
        SceneNodeEntry {
            data: DOCS_ROOT_DATA,
            has_data_override: true,
            key: SceneKey(0),
            name: Some(Cow::Borrowed("docs")),
            tags: Cow::Borrowed(EMPTY_TAGS),
            children: Cow::Borrowed(DOCS_CHILD_KEYS),
            parent: None,
            script: None,
            clear_script: false,
            root_of: None,
            script_vars: Cow::Borrowed(EMPTY_FIELDS),
        },
        SceneNodeEntry {
            data: DOCS_COPY_DATA,
            has_data_override: true,
            key: SceneKey(1),
            name: Some(Cow::Borrowed("copy")),
            tags: Cow::Borrowed(EMPTY_TAGS),
            children: Cow::Borrowed(EMPTY_KEYS),
            parent: Some(SceneKey(0)),
            script: None,
            clear_script: false,
            root_of: None,
            script_vars: Cow::Borrowed(EMPTY_FIELDS),
        },
    ];
    static DOCS_SCENE: Scene = Scene {
        nodes: Cow::Borrowed(DOCS_NODES),
        root: Some(SceneKey(0)),
        key_names: Cow::Borrowed(DOCS_KEY_NAMES),
    };
    const BAD_SCRIPT_KEY_NAMES: &[Cow<'static, str>] = &[Cow::Borrowed("bad")];
    const BAD_SCRIPT_NODES: &[SceneNodeEntry] = &[SceneNodeEntry {
        data: HOST_DATA,
        has_data_override: true,
        key: SceneKey(0),
        name: Some(Cow::Borrowed("bad")),
        tags: Cow::Borrowed(EMPTY_TAGS),
        children: Cow::Borrowed(EMPTY_KEYS),
        parent: None,
        script: Some(Cow::Borrowed("res://missing_script.rs")),
        clear_script: false,
        root_of: None,
        script_vars: Cow::Borrowed(EMPTY_FIELDS),
    }];
    static BAD_SCRIPT_SCENE: Scene = Scene {
        nodes: Cow::Borrowed(BAD_SCRIPT_NODES),
        root: Some(SceneKey(0)),
        key_names: Cow::Borrowed(BAD_SCRIPT_KEY_NAMES),
    };
    static EMPTY_SCENE: Scene = Scene {
        nodes: Cow::Borrowed(&[]),
        root: None,
        key_names: Cow::Borrowed(EMPTY_KEY_NAMES),
    };

    #[test]
    fn dlc_cache_write_stays_under_cache_root() {
        let temp = CacheTempDir::new("write");
        let cache = temp.0.join("cache");
        fs::create_dir(&cache).unwrap();

        let target = write_dlc_cache_file(&cache, "scripts/lib.bin", b"one").unwrap();
        write_dlc_cache_file(&cache, "scripts/lib.bin", b"two").unwrap();

        assert_eq!(target, cache.join("scripts/lib.bin"));
        assert_eq!(fs::read(target).unwrap(), b"two");
    }

    #[cfg(unix)]
    #[test]
    fn dlc_cache_rejects_linked_dir() {
        use std::os::unix::fs::symlink;

        let temp = CacheTempDir::new("linked-dir");
        let cache = temp.0.join("cache");
        let outside = temp.0.join("outside");
        fs::create_dir(&cache).unwrap();
        fs::create_dir(&outside).unwrap();
        symlink(&outside, cache.join("scripts")).unwrap();

        assert!(write_dlc_cache_file(&cache, "scripts/lib.bin", b"bad").is_err());
        assert!(!outside.join("lib.bin").exists());
    }

    #[cfg(unix)]
    #[test]
    fn dlc_cache_rejects_linked_target() {
        use std::os::unix::fs::symlink;

        let temp = CacheTempDir::new("linked-target");
        let cache = temp.0.join("cache");
        let outside = temp.0.join("outside.bin");
        fs::create_dir(&cache).unwrap();
        fs::write(&outside, b"safe").unwrap();
        symlink(&outside, cache.join("lib.bin")).unwrap();

        assert!(write_dlc_cache_file(&cache, "lib.bin", b"bad").is_err());
        assert_eq!(fs::read(outside).unwrap(), b"safe");
    }

    #[cfg(windows)]
    fn try_cache_symlink_dir(original: &Path, link: &Path) -> bool {
        match std::os::windows::fs::symlink_dir(original, link) {
            Ok(()) => true,
            Err(err)
                if err.kind() == io::ErrorKind::PermissionDenied
                    || err.raw_os_error() == Some(1314) =>
            {
                false
            }
            Err(err) => panic!("symlink create failed: {err}"),
        }
    }

    #[cfg(windows)]
    fn try_cache_symlink_file(original: &Path, link: &Path) -> bool {
        match std::os::windows::fs::symlink_file(original, link) {
            Ok(()) => true,
            Err(err)
                if err.kind() == io::ErrorKind::PermissionDenied
                    || err.raw_os_error() == Some(1314) =>
            {
                false
            }
            Err(err) => panic!("symlink create failed: {err}"),
        }
    }

    #[cfg(windows)]
    #[test]
    fn dlc_cache_rejects_linked_dir() {
        let temp = CacheTempDir::new("linked-dir");
        let cache = temp.0.join("cache");
        let outside = temp.0.join("outside");
        fs::create_dir(&cache).unwrap();
        fs::create_dir(&outside).unwrap();
        if !try_cache_symlink_dir(&outside, &cache.join("scripts")) {
            return;
        }

        assert!(write_dlc_cache_file(&cache, "scripts/lib.bin", b"bad").is_err());
        assert!(!outside.join("lib.bin").exists());
    }

    #[cfg(windows)]
    #[test]
    fn dlc_cache_rejects_linked_target() {
        let temp = CacheTempDir::new("linked-target");
        let cache = temp.0.join("cache");
        let outside = temp.0.join("outside.bin");
        fs::create_dir(&cache).unwrap();
        fs::write(&outside, b"safe").unwrap();
        if !try_cache_symlink_file(&outside, &cache.join("lib.bin")) {
            return;
        }

        assert!(write_dlc_cache_file(&cache, "lib.bin", b"bad").is_err());
        assert_eq!(fs::read(outside).unwrap(), b"safe");
    }

    fn test_lookup(path_hash: u64) -> &'static Scene {
        if path_hash == perro_ids::string_to_u64("res://boot.scn") {
            &HOST_SCENE
        } else if path_hash == 100 {
            &HOME_SCENE
        } else if path_hash == 200 {
            &DOCS_SCENE
        } else if path_hash == 300 {
            &BAD_SCRIPT_SCENE
        } else {
            &EMPTY_SCENE
        }
    }

    #[test]
    fn initial_route_scene_uses_match_or_root_fallback() {
        let mut project = RuntimeProject::new("Route Test", ".");
        project.routes = perro_project::ProjectRoutesConfig {
            routes: vec![
                perro_project::ProjectRoute {
                    href: "/".to_string(),
                    name: "home".to_string(),
                    scene: "100".to_string(),
                    title: None,
                    description: None,
                    keywords: Vec::new(),
                },
                perro_project::ProjectRoute {
                    href: "/docs".to_string(),
                    name: "docs".to_string(),
                    scene: "200".to_string(),
                    title: None,
                    description: None,
                    keywords: Vec::new(),
                },
            ],
        };
        let mut runtime = Runtime::new();
        runtime.project = Some(Arc::new(project));

        assert_eq!(
            runtime.initial_route_scene_for_href(Some("/docs?x=1#y")),
            Some(("/docs".to_string(), "200".to_string()))
        );
        assert_eq!(
            runtime.initial_route_scene_for_href(Some("/docs/index.html")),
            Some(("/docs".to_string(), "200".to_string()))
        );
        assert_eq!(
            runtime.initial_route_scene_for_href(Some("/missing")),
            Some(("/".to_string(), "100".to_string()))
        );
    }

    #[test]
    fn typed_preloaded_scene_load_reports_invalid_handle() {
        use perro_resource_api::LoadError;
        use perro_runtime_api::sub_apis::SceneAPI;

        let mut runtime = Runtime::new();
        let err = runtime
            .scene_load_preloaded_typed(PreloadedSceneID::from_u64(99))
            .unwrap_err();

        assert_eq!(
            err,
            LoadError::InvalidHandle {
                kind: "preloaded scene",
                id: 99
            }
        );
    }

    #[test]
    fn apply_route_change_swaps_scene_root() {
        let mut project = RuntimeProject::new("Route Test", ".");
        project.routes = perro_project::ProjectRoutesConfig {
            routes: vec![
                perro_project::ProjectRoute {
                    href: "/".to_string(),
                    name: "home".to_string(),
                    scene: "100".to_string(),
                    title: None,
                    description: None,
                    keywords: Vec::new(),
                },
                perro_project::ProjectRoute {
                    href: "/docs".to_string(),
                    name: "docs".to_string(),
                    scene: "200".to_string(),
                    title: None,
                    description: None,
                    keywords: Vec::new(),
                },
            ],
        };
        project.static_scene_lookup = Some(test_lookup);
        let mut runtime = Runtime::new();
        runtime.project = Some(Arc::new(project));
        runtime.provider_mode = ProviderMode::Static;

        runtime.active_route_root = Some(runtime.load_scene_at_runtime("100").expect("load home"));
        runtime.active_route_href = Some("/".to_string());

        runtime.apply_route_change("/docs").expect("route change");
        assert_eq!(runtime.active_route_href.as_deref(), Some("/docs"));
        assert!(
            runtime
                .nodes
                .iter()
                .any(|(_, node)| node.name.as_ref() == "docs")
        );
        assert!(
            runtime
                .nodes
                .iter()
                .any(|(_, node)| node.name.as_ref() == "copy")
        );
    }

    #[test]
    fn failed_route_change_keeps_current_scene_and_route() {
        let mut project = RuntimeProject::new("Route Test", ".");
        project.routes = perro_project::ProjectRoutesConfig {
            routes: vec![
                perro_project::ProjectRoute {
                    href: "/".to_string(),
                    name: "home".to_string(),
                    scene: "100".to_string(),
                    title: None,
                    description: None,
                    keywords: Vec::new(),
                },
                perro_project::ProjectRoute {
                    href: "/bad".to_string(),
                    name: "bad".to_string(),
                    scene: "300".to_string(),
                    title: None,
                    description: None,
                    keywords: Vec::new(),
                },
            ],
        };
        project.static_scene_lookup = Some(test_lookup);
        let mut runtime = Runtime::new();
        runtime.project = Some(Arc::new(project));
        runtime.provider_mode = ProviderMode::Static;

        let home = runtime.load_scene_at_runtime("100").expect("load home");
        runtime.active_route_root = Some(home);
        runtime.active_route_href = Some("/".to_string());
        let node_count = runtime.nodes.len();

        let err = runtime.apply_route_change("/bad").unwrap_err();
        assert!(
            err.contains("missing_script") || err.contains("script hash"),
            "{err}"
        );
        assert_eq!(runtime.active_route_href.as_deref(), Some("/"));
        assert_eq!(runtime.active_route_root, Some(home));
        assert!(runtime.nodes.get(home).is_some());
        assert_eq!(runtime.nodes.len(), node_count);
        assert!(
            !runtime
                .nodes
                .iter()
                .any(|(_, node)| node.name.as_ref() == "bad")
        );
    }

    #[test]
    fn merge_prevalidation_rejects_late_link_without_live_mutation() {
        let scene =
            Parser::new("$root = @root\n\n[root]\n[Node]\n[/Node]\n[/root]\n").parse_scene();
        let mut prepared =
            prepare_scene_with_loader_and_styles(&scene, &|_| unreachable!(), None).unwrap();
        prepared.nodes[0].camera_stream_target = Some(9_999);

        let mut runtime = Runtime::new();
        let mut sentinel = SceneNode::new(perro_nodes::SceneNodeData::Node);
        sentinel.name = Cow::Borrowed("sentinel");
        let sentinel = runtime.nodes.insert(sentinel);
        let node_count = runtime.nodes.len();
        let update_count = runtime.internal_updates.internal_update_nodes.len();

        let err = merge_prepared_scene(&mut runtime, prepared)
            .err()
            .expect("invalid link must fail");
        assert!(err.contains("camera stream target"), "{err}");
        assert_eq!(runtime.nodes.len(), node_count);
        assert_eq!(
            runtime.internal_updates.internal_update_nodes.len(),
            update_count
        );
        assert_eq!(
            runtime.nodes.get(sentinel).map(|node| node.name.as_ref()),
            Some("sentinel")
        );
    }

    #[test]
    fn merge_rejects_parent_cycle_before_live_mutation() {
        let scene = Parser::new(
            "[first]\n[Node]\n[/Node]\n[/first]\n[second]\n[Node]\n[/Node]\n[/second]\n",
        )
        .parse_scene();
        let mut prepared =
            prepare_scene_with_loader_and_styles(&scene, &|_| unreachable!(), None).unwrap();
        let first = prepared.nodes[0].key;
        let second = prepared.nodes[1].key;
        prepared.nodes[0].parent_key = Some(second);
        prepared.nodes[1].parent_key = Some(first);

        let mut runtime = Runtime::new();
        let err = merge_prepared_scene(&mut runtime, prepared)
            .err()
            .expect("parent cycle must fail");
        assert!(err.contains("parent cycle"), "{err}");
        assert!(runtime.nodes.is_empty());
    }

    #[test]
    fn merge_rejects_declared_root_with_parent_before_live_mutation() {
        let scene = Parser::new(
            "$root = @child\n\n[parent]\n[Node]\n[/Node]\n[/parent]\n[child]\nparent = parent\n[Node]\n[/Node]\n[/child]\n",
        )
        .parse_scene();
        let prepared =
            prepare_scene_with_loader_and_styles(&scene, &|_| unreachable!(), None).unwrap();

        let mut runtime = Runtime::new();
        let err = merge_prepared_scene(&mut runtime, prepared)
            .err()
            .expect("child root must fail");
        assert!(err.contains("must be a top-level node"), "{err}");
        assert!(runtime.nodes.is_empty());
    }

    #[test]
    fn loaded_scene_root_removes_hidden_owner_and_sibling_roots() {
        let scene = Parser::new(
            "$root = @primary\n\n[primary]\n[Node]\n[/Node]\n[/primary]\n[sibling]\n[Node]\n[/Node]\n[/sibling]\n",
        )
        .parse_scene();
        let mut runtime = Runtime::new();
        runtime.project = Some(Arc::new(RuntimeProject::new("Scene Test", ".")));

        let root = runtime
            .load_scene_doc_at_runtime(scene)
            .expect("load sibling scene");
        assert_eq!(runtime.nodes.len(), 3);
        assert_eq!(runtime.scene_ownership_roots.len(), 1);
        assert!(NodeAPI::remove_node(&mut runtime, root));
        assert!(runtime.nodes.is_empty());
        assert!(runtime.scene_ownership_roots.is_empty());
        assert!(runtime.nodes.named_ids("primary").is_empty());
        assert!(runtime.nodes.named_ids("sibling").is_empty());
    }

    #[test]
    fn scene_load_updates_tag_index_during_merge() {
        let scene = Parser::new(
            "$root = @root\n\n[root]\ntags = [\"scene_loaded\"]\n[Node]\n[/Node]\n[/root]\n",
        )
        .parse_scene();
        let prepared =
            prepare_scene_with_loader_and_styles(&scene, &|_| unreachable!(), None).unwrap();
        let mut runtime = Runtime::new();

        let merged = merge_prepared_scene(&mut runtime, prepared).unwrap();
        let tag = perro_ids::TagID::from_string("scene_loaded");

        assert!(
            runtime
                .nodes
                .tag_index()
                .get(&tag)
                .is_some_and(|nodes| nodes.contains(&merged.scene_root))
        );
    }

    #[test]
    fn runtime_scene_load_marks_ui_dirty_for_same_frame_extract() {
        let first_scene = Parser::new(
            r##"
            $root = @first

            [first]
            [Node]
            [/Node]
            [/first]

            [first_panel]
            parent = first
            [UiPanel]
                size_ratio = (0.25, 0.25)
            [/UiPanel]
            [/first_panel]
            "##,
        )
        .parse_scene();
        let second_scene = Parser::new(
            r##"
            $root = @second

            [second]
            [Node]
            [/Node]
            [/second]

            [loaded_panel]
            parent = second
            [UiPanel]
                size_ratio = (0.5, 0.5)
            [/UiPanel]
            [/loaded_panel]
            "##,
        )
        .parse_scene();

        let first = prepare_scene_with_loader_and_styles(&first_scene, &|_| unreachable!(), None)
            .expect("prepare first");
        let second = prepare_scene_with_loader_and_styles(&second_scene, &|_| unreachable!(), None)
            .expect("prepare second");
        let mut runtime = Runtime::new();
        runtime.set_viewport_size(800, 600);

        merge_prepared_scene(&mut runtime, first).expect("merge first");
        runtime.extract_render_ui_commands();
        runtime.drain_render_commands(&mut Vec::new());
        runtime.clear_dirty_flags();

        let merged = merge_prepared_scene(&mut runtime, second).expect("merge second");
        runtime.extract_render_2d_commands();
        runtime.extract_render_ui_commands();
        let mut commands = Vec::new();
        runtime.drain_render_commands(&mut commands);

        let loaded_panel = runtime
            .nodes
            .get(merged.scene_root)
            .and_then(|root| root.children_slice().first().copied())
            .expect("loaded panel exists");
        assert!(commands.iter().any(|cmd| matches!(
            cmd,
            RenderCommand::Ui(UiCommand::UpsertPanel { node, rect, .. })
                if *node == loaded_panel && rect.size == [400.0, 300.0]
        )));
    }

    #[test]
    fn static_boot_root_of_loads_dlc_scene_from_mount() {
        // load_boot_scene writes the process-global project root; serialize
        // with every other test that touches it.
        let _project_root_guard = crate::rs_ctx::PROJECT_ROOT_TEST_LOCK.lock().unwrap();
        let test_root = std::env::temp_dir().join(format!(
            "perro_runtime_static_dlc_scene_{}",
            std::process::id()
        ));
        let _ = fs::remove_dir_all(&test_root);
        let dlc_scene_dir = test_root.join("dlcs").join("test").join("scenes");
        fs::create_dir_all(&dlc_scene_dir).unwrap();
        fs::write(
            dlc_scene_dir.join("main.scn"),
            "$root = @main\n\n[main]\n[Node]\n[/Node]\n[/main]\n",
        )
        .unwrap();

        let mut project = RuntimeProject::new("Static Dlc Test", &test_root);
        project.config.main_scene = "res://boot.scn".to_string();
        project.config.main_scene_hash = Some(perro_ids::string_to_u64("res://boot.scn"));
        project.static_scene_lookup = Some(test_lookup);

        let mut runtime = Runtime::new();
        runtime.project = Some(Arc::new(project));
        runtime.provider_mode = ProviderMode::Static;

        let result = runtime.load_boot_scene();
        let _ = fs::remove_dir_all(&test_root);

        assert_eq!(result, Ok(()));
        assert_eq!(runtime.nodes.len(), 2);
    }

    #[test]
    fn scene_locale_text_binding_refreshes_on_locale_change() {
        fn static_lookup(locale: Locale, key_hash: u64) -> &'static str {
            if key_hash != perro_ids::string_to_u64("ui.center") {
                return "";
            }
            match locale {
                Locale::EN => "Center",
                Locale::ES => "Centro",
                _ => "",
            }
        }

        let scene = Parser::new(
            r#"
            $root = @label
            [label]
            [UiLabel]
                text = "%loc:\"ui.center\""
            [/UiLabel]
            [/label]

            [missing]
            [UiLabel]
                text = %loc: "ui.missing"
            [/UiLabel]
            [/missing]
            "#,
        )
        .parse_scene();
        let prepared = prepare::prepare_scene_with_loader(&scene, &|path| {
            Err(format!("unknown scene path `{path}`"))
        })
        .expect("prepare scene");
        let mut runtime = Runtime::new();
        runtime.resource_api = RuntimeResourceApi::new(
            None,
            None,
            None,
            None,
            None,
            Some(static_lookup),
            None,
            Some(LocalizationConfig {
                source_csv: "locale.csv".to_string(),
                key_column: "key".to_string(),
                default_locale: "en".to_string(),
            }),
        );
        merge::merge_prepared_scene(&mut runtime, prepared).expect("merge scene");

        let label_text = runtime
            .nodes
            .iter()
            .find_map(|(_, node)| match &node.data {
                perro_nodes::SceneNodeData::UiLabel(label) if node.name.as_ref() == "label" => {
                    Some(label.text.as_ref().to_string())
                }
                _ => None,
            })
            .expect("label text");
        assert_eq!(label_text, "Center");
        assert!(runtime.nodes.iter().any(|(_, node)| match &node.data {
            perro_nodes::SceneNodeData::UiLabel(label) => label.text.as_ref() == "ui.missing",
            _ => false,
        }));

        assert!(runtime.resource_api.localization_set_locale(Locale::ES));
        runtime.extract_render_ui_commands();
        let label_text = runtime
            .nodes
            .iter()
            .find_map(|(_, node)| match &node.data {
                perro_nodes::SceneNodeData::UiLabel(label) if node.name.as_ref() == "label" => {
                    Some(label.text.as_ref().to_string())
                }
                _ => None,
            })
            .expect("label text");
        assert_eq!(label_text, "Centro");
    }

    #[test]
    fn runtime_locale_text_binding_can_switch_key() {
        fn static_lookup(locale: Locale, key_hash: u64) -> &'static str {
            match (locale, key_hash) {
                (Locale::EN, hash) if hash == perro_ids::string_to_u64("ui.center") => "Center",
                (Locale::ES, hash) if hash == perro_ids::string_to_u64("ui.center") => "Centro",
                (Locale::EN, hash) if hash == perro_ids::string_to_u64("ui.alt") => "Alt",
                (Locale::ES, hash) if hash == perro_ids::string_to_u64("ui.alt") => "Otro",
                _ => "",
            }
        }

        let scene = Parser::new(
            r#"
            $root = @label
            [label]
            [UiLabel]
                text = %loc: "ui.center"
            [/UiLabel]
            [/label]
            "#,
        )
        .parse_scene();
        let prepared = prepare::prepare_scene_with_loader(&scene, &|path| {
            Err(format!("unknown scene path `{path}`"))
        })
        .expect("prepare scene");
        let mut runtime = Runtime::new();
        runtime.resource_api = RuntimeResourceApi::new(
            None,
            None,
            None,
            None,
            None,
            Some(static_lookup),
            None,
            Some(LocalizationConfig {
                source_csv: "locale.csv".to_string(),
                key_column: "key".to_string(),
                default_locale: "en".to_string(),
            }),
        );
        merge::merge_prepared_scene(&mut runtime, prepared).expect("merge scene");
        let label_id = runtime
            .nodes
            .iter()
            .find_map(|(id, node)| (node.name.as_ref() == "label").then_some(id))
            .expect("label id");

        assert!(runtime.bind_locale_text(label_id, "ui.alt"));
        assert_eq!(runtime.locale_text.bindings.len(), 1);
        assert!(
            runtime
                .nodes
                .get(label_id)
                .is_some_and(|node| match &node.data {
                    perro_nodes::SceneNodeData::UiLabel(label) => label.text.as_ref() == "Alt",
                    _ => false,
                })
        );

        assert!(runtime.resource_api.localization_set_locale(Locale::ES));
        runtime.extract_render_ui_commands();
        assert!(
            runtime
                .nodes
                .get(label_id)
                .is_some_and(|node| match &node.data {
                    perro_nodes::SceneNodeData::UiLabel(label) => label.text.as_ref() == "Otro",
                    _ => false,
                })
        );
    }

    #[test]
    fn runtime_locale_text_binding_supports_world_labels() {
        fn static_lookup(locale: Locale, key_hash: u64) -> &'static str {
            match (locale, key_hash) {
                (Locale::EN, hash) if hash == perro_ids::string_to_u64("ui.hp") => "HP",
                (Locale::ES, hash) if hash == perro_ids::string_to_u64("ui.hp") => "PV",
                (Locale::EN, hash) if hash == perro_ids::string_to_u64("ui.name") => "Name",
                (Locale::ES, hash) if hash == perro_ids::string_to_u64("ui.name") => "Nombre",
                _ => "",
            }
        }

        let scene = Parser::new(
            r#"
            [label_2d]
            [Label2D]
                text = %loc: "ui.hp"
            [/Label2D]
            [/label_2d]

            [label_3d]
            [Label3D]
                text = %loc: "ui.name"
            [/Label3D]
            [/label_3d]
            "#,
        )
        .parse_scene();
        let prepared = prepare::prepare_scene_with_loader(&scene, &|path| {
            Err(format!("unknown scene path `{path}`"))
        })
        .expect("prepare scene");
        let mut runtime = Runtime::new();
        runtime.resource_api = RuntimeResourceApi::new(
            None,
            None,
            None,
            None,
            None,
            Some(static_lookup),
            None,
            Some(LocalizationConfig {
                source_csv: "locale.csv".to_string(),
                key_column: "key".to_string(),
                default_locale: "en".to_string(),
            }),
        );
        merge::merge_prepared_scene(&mut runtime, prepared).expect("merge scene");

        let mut label_texts = runtime
            .nodes
            .iter()
            .filter_map(|(_, node)| match &node.data {
                perro_nodes::SceneNodeData::Label2D(label) => Some(label.text.as_ref().to_string()),
                perro_nodes::SceneNodeData::Label3D(label) => Some(label.text.as_ref().to_string()),
                _ => None,
            })
            .collect::<Vec<_>>();
        label_texts.sort();
        assert_eq!(label_texts, ["HP", "Name"]);

        assert!(runtime.resource_api.localization_set_locale(Locale::ES));
        runtime.extract_render_ui_commands();
        let mut label_texts = runtime
            .nodes
            .iter()
            .filter_map(|(_, node)| match &node.data {
                perro_nodes::SceneNodeData::Label2D(label) => Some(label.text.as_ref().to_string()),
                perro_nodes::SceneNodeData::Label3D(label) => Some(label.text.as_ref().to_string()),
                _ => None,
            })
            .collect::<Vec<_>>();
        label_texts.sort();
        assert_eq!(label_texts, ["Nombre", "PV"]);
    }
}
