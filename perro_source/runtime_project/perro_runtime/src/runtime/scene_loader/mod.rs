use crate::{Runtime, runtime_project::ProviderMode};
use perro_ids::NodeID;
use perro_ids::ScriptMemberID;
use perro_ids::parse_hashed_source_uri;
use perro_ids::string_to_u64;
use perro_io::{
    ProjectRoot, clear_dlc_mounts, data_local_dir, mount_dlc_archive, mount_dlc_disk,
    read_mounted_dlc_file, set_project_root, is_reserved_dlc_name,
};
use perro_runtime_context::sub_apis::PreloadedSceneID;
use perro_scene::Scene;
use perro_variant::Variant;
use std::fs;
use std::path::PathBuf;
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
            && let Some(scene) = self.preloaded_scenes.get(&id) {
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
                if path.starts_with("dlc://") {
                    let runtime_scene = self.get_or_load_dynamic_scene_cached(path)?;
                    let prepared = prepare_scene_with_loader(runtime_scene.as_ref(), &|import_path| {
                        self.resolve_scene_by_path(import_path)
                    })?;
                    merge_prepared_scene(self, prepared)?
                } else if let Some(lookup) = static_lookup {
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
        self.reload_dlc_mounts()?;
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
        self.render_2d.retained_sprites.clear();
        self.render_2d.texture_sources.clear();
        self.render_2d.last_camera = None;
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
            self.script_runtime.base_scripts_loaded = false;
        }
        self.script_runtime.loaded_dlc_script_libs.clear();
        self.script_runtime.script_instance_dlc_mounts.clear();
        self.script_runtime.script_behavior_cache.clear();
        self.script_runtime.script_libraries.clear();
        self.node_index.node_tag_index.clear();
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
                    node_insert = Some(node_insert_start.elapsed());
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
                        node_insert = Some(node_insert_start.elapsed());
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
                        node_insert = Some(node_insert_start.elapsed());
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
            node_insert: node_insert.unwrap_or(Duration::ZERO),
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
                    format!("failed to read dlc entry in `{}`: {err}", dev_dlcs.display())
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
                    format!("failed to mount dev dlc `{}` from `{}`: {err}", name, path.display())
                })?;
                if let Some(script_dylib) = resolve_dev_dlc_scripts_dylib_path(&project_root, name) {
                    self.script_runtime
                        .mounted_dlc_script_libs
                        .insert(name.to_ascii_lowercase(), script_dylib);
                }
            }
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

                let manifest_bytes = read_mounted_dlc_file(stem, "manifest.toml").map_err(|err| {
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

                let extract_root = install_root.join(".runtime_cache").join(stem);
                fs::create_dir_all(&extract_root).map_err(|err| {
                    format!(
                        "failed to create dlc runtime cache dir `{}`: {err}",
                        extract_root.display()
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

                let pack_path =
                    extract_dlc_archive_file_to_cache(stem, &pack_rel, &extract_root).map_err(
                        |err| {
                            format!(
                                "failed to extract pack lib `{}` from dlc `{}`: {err}",
                                pack_rel, stem
                            )
                        },
                    )?;
                if let Ok(lib) = unsafe { libloading::Library::new(&pack_path) } {
                    self.script_runtime.script_libraries.push(lib);
                }
            }
        }

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

fn resolve_dev_dlc_scripts_dylib_path(project_root: &PathBuf, dlc_name: &str) -> Option<PathBuf> {
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

fn extract_dlc_archive_file_to_cache(
    dlc_name: &str,
    virtual_path: &str,
    cache_root: &PathBuf,
) -> Result<PathBuf, std::io::Error> {
    let bytes = read_mounted_dlc_file(dlc_name, virtual_path)?;
    let target = cache_root.join(virtual_path.replace('/', "\\"));
    if let Some(parent) = target.parent() {
        fs::create_dir_all(parent)?;
    }
    fs::write(&target, bytes)?;
    Ok(target)
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
