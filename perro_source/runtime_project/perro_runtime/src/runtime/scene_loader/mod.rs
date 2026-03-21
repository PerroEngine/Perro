use crate::{Runtime, runtime_project::ProviderMode};
#[cfg(feature = "profile")]
use perro_ids::NodeID;
use perro_io::{ProjectRoot, set_project_root};
#[cfg(feature = "profile")]
use std::collections::HashMap;
#[cfg(feature = "profile")]
use std::time::{Duration, Instant};

mod merge;
mod prepare;

use merge::merge_prepared_scene;
use prepare::{load_runtime_scene_from_disk, prepare_runtime_scene, prepare_static_scene};

#[cfg(feature = "profile")]
struct SceneLoadStats {
    mode_label: &'static str,
    source_load: Option<Duration>,
    parse: Option<Duration>,
    node_insert: Duration,
    total_excluding_debug_print: Duration,
}

impl Runtime {
    pub(crate) fn load_boot_scene(&mut self) -> Result<(), String> {
        #[cfg(feature = "profile")]
        let boot_start = Instant::now();
        let (project_root, project_name, main_scene_path, static_lookup, perro_assets_bytes) = {
            let project = self
                .project()
                .ok_or_else(|| "Runtime project is not set".to_string())?;
            (
                project.root.clone(),
                project.config.name.clone(),
                project.config.main_scene.clone(),
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
        self.render_3d.material_sources.clear();
        self.render_3d.material_overrides.clear();
        self.render_3d.terrain_material = perro_ids::MaterialID::nil();
        self.render_3d.terrain_chunk_meshes.clear();
        self.render_3d.particle_path_cache.clear();
        self.render_3d.removed_nodes.clear();
        self.terrain_store
            .lock()
            .expect("terrain store mutex poisoned")
            .clear();
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
        let script_nodes;
        match self.provider_mode {
            ProviderMode::Dynamic => {
                mode_label = "dynamic";
                let (runtime_scene, load_stats) = load_runtime_scene_from_disk(&main_scene_path)?;
                #[cfg(feature = "profile")]
                {
                    source_load = Some(load_stats.source_load);
                    parse = Some(load_stats.parse);
                }
                let prepared = prepare_runtime_scene(runtime_scene)?;
                #[cfg(feature = "profile")]
                let node_insert_start = Instant::now();
                script_nodes = merge_prepared_scene(self, prepared)?;
                #[cfg(feature = "profile")]
                {
                    node_insert = node_insert_start.elapsed();
                }
                #[cfg(not(feature = "profile"))]
                {
                    let _ = (load_stats,);
                }
            }
            ProviderMode::Static => match static_lookup.and_then(|lookup| lookup(&main_scene_path))
            {
                Some(scene) => {
                    mode_label = "static";
                    let prepared = prepare_static_scene(scene)?;
                    #[cfg(feature = "profile")]
                    let node_insert_start = Instant::now();
                    script_nodes = merge_prepared_scene(self, prepared)?;
                    #[cfg(feature = "profile")]
                    let node_insert = node_insert_start.elapsed();
                }
                None => {
                    mode_label = "static_fallback_dynamic";
                    let (runtime_scene, load_stats) =
                        load_runtime_scene_from_disk(&main_scene_path)?;
                    #[cfg(feature = "profile")]
                    {
                        source_load = Some(load_stats.source_load);
                        parse = Some(load_stats.parse);
                    }
                    let prepared = prepare_runtime_scene(runtime_scene)?;
                    #[cfg(feature = "profile")]
                    let node_insert_start = Instant::now();
                    script_nodes = merge_prepared_scene(self, prepared)?;
                    #[cfg(feature = "profile")]
                    {
                        node_insert = node_insert_start.elapsed();
                    }
                    #[cfg(not(feature = "profile"))]
                    {
                        let _ = (load_stats,);
                    }
                }
            },
        }
        #[cfg(feature = "profile")]
        let script_paths_by_node: HashMap<NodeID, String> = script_nodes
            .iter()
            .map(|(id, script_path)| (*id, script_path.clone()))
            .collect();
        self.rebuild_internal_node_schedules();
        self.rebuild_node_tag_index();
        self.attach_scene_scripts(script_nodes)?;
        #[cfg(not(feature = "profile"))]
        {
            let _ = mode_label;
        }
        #[cfg(feature = "profile")]
        {
            let stats = SceneLoadStats {
                mode_label,
                source_load,
                parse,
                node_insert,
                total_excluding_debug_print: boot_start.elapsed(),
            };
            debug_print_scene_load(self, &main_scene_path, stats, &script_paths_by_node);
        }
        Ok(())
    }
}

#[cfg(feature = "profile")]
fn debug_print_scene_load(
    runtime: &Runtime,
    path: &str,
    stats: SceneLoadStats,
    script_paths_by_node: &HashMap<NodeID, String>,
) {
    println!(
        "[scene_load] mode={} path={} total_us={:.3} source_us={} parse_us={} insert_us={:.3}",
        stats.mode_label,
        path,
        as_us(stats.total_excluding_debug_print),
        fmt_duration(stats.source_load),
        fmt_duration(stats.parse),
        as_us(stats.node_insert),
    );
    print_scene_tree(runtime, NodeID::ROOT, "", 0, script_paths_by_node);
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

#[cfg(feature = "profile")]
fn print_scene_tree(
    runtime: &Runtime,
    node: NodeID,
    indent: &str,
    depth: usize,
    script_paths_by_node: &HashMap<NodeID, String>,
) {
    let Some(node_ref) = runtime.nodes.get(node) else {
        return;
    };
    let color = depth_color(depth);
    let script_suffix = script_paths_by_node
        .get(&node)
        .map(|script_path| format!(" {}script={}{}", ANSI_ORANGE, script_path, color))
        .unwrap_or_default();
    println!(
        "{}{}- [{}] {} ({}){}{}",
        color,
        indent,
        node,
        node_ref.name.as_ref(),
        node_ref.node_type(),
        script_suffix,
        ANSI_RESET,
    );
    let child_indent = format!("{indent}  ");
    for child in node_ref.children_slice() {
        print_scene_tree(
            runtime,
            *child,
            &child_indent,
            depth + 1,
            script_paths_by_node,
        );
    }
}

#[cfg(feature = "profile")]
const ANSI_RESET: &str = "\x1b[0m";
#[cfg(feature = "profile")]
const ANSI_WHITE: &str = "\x1b[97m";
#[cfg(feature = "profile")]
const ANSI_ORANGE: &str = "\x1b[38;5;208m";

#[cfg(feature = "profile")]
fn depth_color(depth: usize) -> &'static str {
    if depth == 0 { ANSI_WHITE } else { "" }
}

