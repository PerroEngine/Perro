use crate::{Runtime, runtime_project::ProviderMode};
use perro_ids::NodeID;
use perro_io::{ProjectRoot, set_project_root};
use std::time::{Duration, Instant};

mod merge;
mod prepare;
mod scripts;

use merge::merge_prepared_scene;
use prepare::{load_runtime_scene_from_disk, prepare_runtime_scene, prepare_static_scene};

struct SceneLoadStats {
    mode_label: &'static str,
    source_load: Option<Duration>,
    parse: Option<Duration>,
    node_insert: Duration,
    total_excluding_debug_print: Duration,
}

impl Runtime {
    pub(crate) fn load_boot_scene(&mut self) -> Result<(), String> {
        let boot_start = Instant::now();
        let (project_root, project_name, main_scene_path, static_lookup, brk_bytes) = {
            let project = self
                .project()
                .ok_or_else(|| "Runtime project is not set".to_string())?;
            (
                project.root.clone(),
                project.config.name.clone(),
                project.config.main_scene.clone(),
                project.static_scene_lookup,
                project.brk_bytes,
            )
        };

        if self.provider_mode == ProviderMode::Static {
            if let Some(data) = brk_bytes {
                set_project_root(ProjectRoot::Brk {
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

        self.nodes.clear();
        self.scripts = Default::default();
        self.render_2d.traversal_ids.clear();
        self.render_2d.visible_now.clear();
        self.render_2d.prev_visible.clear();
        self.render_2d.retained_sprite_textures.clear();
        self.render_2d.texture_sources.clear();
        self.render_2d.removed_nodes.clear();
        self.render_3d.traversal_ids.clear();
        self.render_3d.mesh_sources.clear();
        self.render_3d.material_sources.clear();
        self.render_3d.material_overrides.clear();
        if self.provider_mode == ProviderMode::Dynamic {
            self.dynamic_script_registry.clear();
        }
        self.script_library = None;
        let mode_label;
        let mut source_load = None;
        let mut parse = None;
        let node_insert;
        let script_nodes;
        match self.provider_mode {
            ProviderMode::Dynamic => {
                mode_label = "dynamic";
                let (runtime_scene, load_stats) = load_runtime_scene_from_disk(&main_scene_path)?;
                source_load = Some(load_stats.source_load);
                parse = Some(load_stats.parse);
                let prepared = prepare_runtime_scene(runtime_scene)?;
                let node_insert_start = Instant::now();
                script_nodes = merge_prepared_scene(self, prepared)?;
                node_insert = node_insert_start.elapsed();
            }
            ProviderMode::Static => match static_lookup.and_then(|lookup| lookup(&main_scene_path)) {
                Some(scene) => {
                    mode_label = "static";
                    let prepared = prepare_static_scene(scene)?;
                    let node_insert_start = Instant::now();
                    script_nodes = merge_prepared_scene(self, prepared)?;
                    node_insert = node_insert_start.elapsed();
                }
                None => {
                    mode_label = "static_fallback_dynamic";
                    let (runtime_scene, load_stats) = load_runtime_scene_from_disk(&main_scene_path)?;
                    source_load = Some(load_stats.source_load);
                    parse = Some(load_stats.parse);
                    let prepared = prepare_runtime_scene(runtime_scene)?;
                    let node_insert_start = Instant::now();
                    script_nodes = merge_prepared_scene(self, prepared)?;
                    node_insert = node_insert_start.elapsed();
                }
            },
        }
        self.attach_scene_scripts(script_nodes)?;
        let stats = SceneLoadStats {
            mode_label,
            source_load,
            parse,
            node_insert,
            total_excluding_debug_print: boot_start.elapsed(),
        };
        debug_print_scene_load(self, &main_scene_path, stats);
        Ok(())
    }
}

fn debug_print_scene_load(runtime: &Runtime, path: &str, stats: SceneLoadStats) {
    println!(
        "[scene_load] mode={} path={} total_us={:.3} source_us={} parse_us={} insert_us={:.3}",
        stats.mode_label,
        path,
        as_us(stats.total_excluding_debug_print),
        fmt_duration(stats.source_load),
        fmt_duration(stats.parse),
        as_us(stats.node_insert),
    );
    print_scene_tree(runtime, NodeID::ROOT, "", 0);
}

fn as_us(duration: Duration) -> f64 {
    duration.as_secs_f64() * 1_000_000.0
}

fn fmt_duration(duration: Option<Duration>) -> String {
    duration
        .map(|value| format!("{:.3}", as_us(value)))
        .unwrap_or_else(|| "n/a".to_string())
}

fn print_scene_tree(runtime: &Runtime, node_id: NodeID, indent: &str, depth: usize) {
    let Some(node) = runtime.nodes.get(node_id) else {
        return;
    };
    let color = depth_color(depth);
    println!(
        "{}{}- [{}] {} ({}){}",
        color,
        indent,
        node_id,
        node.name.as_ref(),
        node.node_type(),
        ANSI_RESET,
    );
    let child_indent = format!("{indent}  ");
    for child_id in node.children_slice() {
        print_scene_tree(runtime, *child_id, &child_indent, depth + 1);
    }
}

const ANSI_RESET: &str = "\x1b[0m";
const ANSI_WHITE: &str = "\x1b[97m";

fn depth_color(depth: usize) -> &'static str {
    if depth == 0 { ANSI_WHITE } else { "" }
}
