use perro_api::prelude::*;
use std::borrow::Cow;
use std::path::{Path, PathBuf};
use std::process::Command;

type SelfNodeType = UiPanel;

const ACTIVE_PROJECT: &str = "user://perro_editor_active_project.txt";

#[State]
#[derive(Clone)]
struct EditorState {
    project_dir: String,
    main_scene: String,
    script_path: String,
    project_label: NodeID,
    scene_label: NodeID,
    viewport_status: NodeID,
    inspector_body: NodeID,
    status_label: NodeID,
    node_labels: Vec<NodeID>,
    live_root: NodeID,
}

lifecycle!({
    fn on_init(
        &self,
        _ctx: &mut RuntimeContext<'_, RT>,
        _res: &ResourceContext<'_, RS>,
        _ipt: &InputContext<'_, IP>,
        _self_id: NodeID,
    ) {
    }

    fn on_all_init(
        &self,
        ctx: &mut RuntimeContext<'_, RT>,
        res: &ResourceContext<'_, RS>,
        _ipt: &InputContext<'_, IP>,
        self_id: NodeID,
    ) {
        let top_bar = child(ctx, self_id, "top_bar");
        let workspace = child(ctx, self_id, "workspace");
        let scene_panel = child(ctx, workspace, "scene_panel");
        let scene_stack = child(ctx, scene_panel, "scene_stack");
        let viewport_panel = child(ctx, workspace, "viewport_panel");
        let inspector_panel = child(ctx, workspace, "inspector_panel");
        let inspector_stack = child(ctx, inspector_panel, "inspector_stack");
        let bottom_bar = child(ctx, self_id, "bottom_bar");

        let node_0 = child(ctx, scene_stack, "node_0");
        let node_1 = child(ctx, scene_stack, "node_1");
        let node_2 = child(ctx, scene_stack, "node_2");
        let node_3 = child(ctx, scene_stack, "node_3");
        let node_labels = vec![
            child(ctx, node_0, "node_0_label"),
            child(ctx, node_1, "node_1_label"),
            child(ctx, node_2, "node_2_label"),
            child(ctx, node_3, "node_3_label"),
        ];

        let project_label = child(ctx, top_bar, "project_label");
        let scene_label = child(ctx, top_bar, "scene_label");
        let viewport_status = child(ctx, viewport_panel, "viewport_status");
        let inspector_body = child(ctx, inspector_stack, "inspector_body");
        let status_label = child(ctx, bottom_bar, "status_label");

        let project_dir = FileMod::load_string(ACTIVE_PROJECT).unwrap_or_default();
        if !project_dir.trim().is_empty() {
            FileMod::set_project_root_disk(project_dir.trim(), "perro_editor_live_project");
        }
        let main_scene =
            read_main_scene(project_dir.trim()).unwrap_or_else(|| "res://main.scn".to_string());
        let mut script_path = String::new();
        let mut live_root = NodeID::default();
        let mut live_scene_path = String::new();

        set_label(ctx, project_label, project_dir.trim());
        set_label(ctx, scene_label, &format!("Scene: {main_scene}"));

        match scene_load_doc!(res, main_scene.clone()) {
            Ok(doc) => {
                let names = doc
                    .scene
                    .nodes
                    .iter()
                    .take(4)
                    .map(|node| {
                        let ty = node.data.ty.as_ref();
                        let name = node
                            .name
                            .as_ref()
                            .map(|n| n.as_ref())
                            .unwrap_or(node.key.as_ref());
                        format!("{name} : {ty}")
                    })
                    .collect::<Vec<_>>();
                write_node_labels(ctx, &node_labels, &names);
                script_path = doc
                    .scene
                    .nodes
                    .iter()
                    .find_map(|node| node.script.as_ref().map(|s| s.to_string()))
                    .unwrap_or_else(|| "res://scripts/script.rs".to_string());
                let summary = format!(
                    "Doc loaded\nnodes: {}\nroot: {}\nscript: {}",
                    doc.scene.nodes.len(),
                    doc.scene
                        .root
                        .as_ref()
                        .map(|r| r.as_ref())
                        .unwrap_or("none"),
                    script_path
                );
                set_text_block(ctx, viewport_status, &summary);
                set_text_block(ctx, inspector_body, &inspector_summary(&doc));
                live_scene_path = write_live_scene_doc(&doc).unwrap_or_default();
            }
            Err(err) => {
                set_text_block(ctx, viewport_status, &format!("Doc load fail\n{err}"));
                write_node_labels(ctx, &node_labels, &[]);
            }
        }

        if live_scene_path.is_empty() {
            set_label(ctx, status_label, "Live scene skip: doc missing");
        } else {
            match scene_load!(ctx, live_scene_path.clone()) {
                Ok(root) => {
                    live_root = root;
                    disable_physics(ctx, root);
                    set_label(
                        ctx,
                        status_label,
                        "Live edit scene loaded; scripts stripped; physics disabled",
                    );
                }
                Err(err) => set_label(ctx, status_label, &format!("Live scene load fail: {err}")),
            }
        }

        let _ = with_state_mut!(ctx, EditorState, self_id, |state| {
            state.project_dir = project_dir.trim().to_string();
            state.main_scene = main_scene;
            state.script_path = script_path;
            state.project_label = project_label;
            state.scene_label = scene_label;
            state.viewport_status = viewport_status;
            state.inspector_body = inspector_body;
            state.status_label = status_label;
            state.node_labels = node_labels;
            state.live_root = live_root;
        });

        signal_connect!(
            ctx,
            self_id,
            signal!("script_button_click"),
            func!("on_open_script")
        );
        signal_connect!(
            ctx,
            self_id,
            signal!("main_scene_button_click"),
            func!("on_open_main_scene")
        );
        signal_connect!(ctx, self_id, signal!("node_0_click"), func!("on_node_0"));
        signal_connect!(ctx, self_id, signal!("node_1_click"), func!("on_node_1"));
        signal_connect!(ctx, self_id, signal!("node_2_click"), func!("on_node_2"));
        signal_connect!(ctx, self_id, signal!("node_3_click"), func!("on_node_3"));
    }

    fn on_update(
        &self,
        _ctx: &mut RuntimeContext<'_, RT>,
        _res: &ResourceContext<'_, RS>,
        _ipt: &InputContext<'_, IP>,
        _self_id: NodeID,
    ) {
    }

    fn on_fixed_update(
        &self,
        _ctx: &mut RuntimeContext<'_, RT>,
        _res: &ResourceContext<'_, RS>,
        _ipt: &InputContext<'_, IP>,
        _self_id: NodeID,
    ) {
    }

    fn on_removal(
        &self,
        _ctx: &mut RuntimeContext<'_, RT>,
        _res: &ResourceContext<'_, RS>,
        _ipt: &InputContext<'_, IP>,
        _self_id: NodeID,
    ) {
    }
});

methods!({
    fn on_open_script(
        &self,
        ctx: &mut RuntimeContext<'_, RT>,
        _res: &ResourceContext<'_, RS>,
        _ipt: &InputContext<'_, IP>,
        self_id: NodeID,
        _button: NodeID,
    ) {
        let (project_dir, script_path, status_label) =
            with_state!(ctx, EditorState, self_id, |state| {
                (
                    state.project_dir.clone(),
                    state.script_path.clone(),
                    state.status_label,
                )
            });
        let path = res_path_to_disk(&project_dir, &script_path);
        match open_code(&path) {
            Ok(()) => set_label(ctx, status_label, "VS Code open script"),
            Err(err) => set_label(ctx, status_label, &format!("VS Code fail: {err}")),
        }
    }

    fn on_open_main_scene(
        &self,
        ctx: &mut RuntimeContext<'_, RT>,
        _res: &ResourceContext<'_, RS>,
        _ipt: &InputContext<'_, IP>,
        self_id: NodeID,
        _button: NodeID,
    ) {
        let (project_dir, main_scene, status_label) =
            with_state!(ctx, EditorState, self_id, |state| {
                (
                    state.project_dir.clone(),
                    state.main_scene.clone(),
                    state.status_label,
                )
            });
        let path = res_path_to_disk(&project_dir, &main_scene);
        match open_code(&path) {
            Ok(()) => set_label(ctx, status_label, "VS Code open scene"),
            Err(err) => set_label(ctx, status_label, &format!("VS Code fail: {err}")),
        }
    }

    fn on_node_0(
        &self,
        ctx: &mut RuntimeContext<'_, RT>,
        _res: &ResourceContext<'_, RS>,
        _ipt: &InputContext<'_, IP>,
        self_id: NodeID,
        _button: NodeID,
    ) {
        inspect_node(ctx, self_id, 0);
    }

    fn on_node_1(
        &self,
        ctx: &mut RuntimeContext<'_, RT>,
        _res: &ResourceContext<'_, RS>,
        _ipt: &InputContext<'_, IP>,
        self_id: NodeID,
        _button: NodeID,
    ) {
        inspect_node(ctx, self_id, 1);
    }

    fn on_node_2(
        &self,
        ctx: &mut RuntimeContext<'_, RT>,
        _res: &ResourceContext<'_, RS>,
        _ipt: &InputContext<'_, IP>,
        self_id: NodeID,
        _button: NodeID,
    ) {
        inspect_node(ctx, self_id, 2);
    }

    fn on_node_3(
        &self,
        ctx: &mut RuntimeContext<'_, RT>,
        _res: &ResourceContext<'_, RS>,
        _ipt: &InputContext<'_, IP>,
        self_id: NodeID,
        _button: NodeID,
    ) {
        inspect_node(ctx, self_id, 3);
    }
});

fn child<RT: RuntimeAPI + ?Sized>(
    ctx: &mut RuntimeContext<'_, RT>,
    parent: NodeID,
    name: &str,
) -> NodeID {
    if parent.is_nil() {
        return NodeID::default();
    }
    ctx.Nodes()
        .get_child_by_name(parent, name)
        .unwrap_or_default()
}

fn set_label<RT: RuntimeAPI + ?Sized>(ctx: &mut RuntimeContext<'_, RT>, id: NodeID, text: &str) {
    if id.is_nil() {
        return;
    }
    let _ = with_node_mut!(ctx, UiLabel, id, |label| {
        label.text = Cow::Owned(text.to_string());
    });
}

fn set_text_block<RT: RuntimeAPI + ?Sized>(
    ctx: &mut RuntimeContext<'_, RT>,
    id: NodeID,
    text: &str,
) {
    if id.is_nil() {
        return;
    }
    let _ = with_node_mut!(ctx, UiTextBlock, id, |block| {
        block.set_text(text.to_string());
    });
}

fn write_node_labels<RT: RuntimeAPI + ?Sized>(
    ctx: &mut RuntimeContext<'_, RT>,
    labels: &[NodeID],
    names: &[String],
) {
    for (index, label) in labels.iter().copied().enumerate() {
        let text = names.get(index).map(String::as_str).unwrap_or("-");
        set_label(ctx, label, text);
    }
}

fn read_main_scene(project_dir: &str) -> Option<String> {
    let text = std::fs::read_to_string(Path::new(project_dir).join("project.toml")).ok()?;
    for line in text.lines() {
        let trimmed = line.trim();
        if let Some(rest) = trimmed.strip_prefix("main_scene") {
            let (_, value) = rest.split_once('=')?;
            return Some(value.trim().trim_matches('"').to_string());
        }
    }
    None
}

fn inspector_summary(doc: &perro_scene::SceneDoc) -> String {
    let Some(root) = doc.scene.nodes.first() else {
        return "Empty scene".to_string();
    };
    let name = root
        .name
        .as_ref()
        .map(|n| n.as_ref())
        .unwrap_or(root.key.as_ref());
    let parent = root.parent.as_ref().map(|p| p.as_ref()).unwrap_or("none");
    let script = root.script.as_ref().map(|s| s.as_ref()).unwrap_or("none");
    format!(
        "selected: {name}\ntype: {}\nkey: {}\nparent: {parent}\nscript: {script}\nfields: {}",
        root.data.ty,
        root.key.as_ref(),
        root.data.fields.len()
    )
}

fn write_live_scene_doc(doc: &perro_scene::SceneDoc) -> Result<String, String> {
    let mut live_doc = doc.clone();
    for node in live_doc.scene.nodes.to_mut() {
        node.script = None;
        node.script_hash = None;
        node.clear_script = true;
    }
    live_doc.normalize_links();
    let path = std::env::temp_dir().join("perro_editor_live_scene.scn");
    std::fs::write(&path, live_doc.to_text()).map_err(|err| err.to_string())?;
    Ok(path.to_string_lossy().to_string())
}

fn inspect_node<RT: RuntimeAPI + ?Sized>(
    ctx: &mut RuntimeContext<'_, RT>,
    self_id: NodeID,
    index: usize,
) {
    let (project_dir, main_scene, inspector_body) =
        with_state!(ctx, EditorState, self_id, |state| {
            (
                state.project_dir.clone(),
                state.main_scene.clone(),
                state.inspector_body,
            )
        });
    let path = res_path_to_disk(&project_dir, &main_scene);
    let text = std::fs::read_to_string(path).unwrap_or_default();
    let lines = text
        .lines()
        .filter(|line| line.trim_start().starts_with('[') && !line.trim_start().starts_with("[/"))
        .skip(index)
        .take(1)
        .collect::<Vec<_>>();
    let detail = lines
        .first()
        .map(|line| format!("selected doc row\n{line}"))
        .unwrap_or_else(|| "No node at slot".to_string());
    set_text_block(ctx, inspector_body, &detail);
}

fn disable_physics<RT: RuntimeAPI + ?Sized>(ctx: &mut RuntimeContext<'_, RT>, root: NodeID) {
    let ids = query!(
        ctx,
        any(
            is[StaticBody2D, RigidBody2D, Area2D, StaticBody3D, RigidBody3D, Area3D]
        ),
        in_subtree(root)
    );
    for id in ids {
        let _ = with_node_mut!(ctx, StaticBody2D, id, |node| node.enabled = false);
        let _ = with_node_mut!(ctx, RigidBody2D, id, |node| node.enabled = false);
        let _ = with_node_mut!(ctx, Area2D, id, |node| node.enabled = false);
        let _ = with_node_mut!(ctx, StaticBody3D, id, |node| node.enabled = false);
        let _ = with_node_mut!(ctx, RigidBody3D, id, |node| node.enabled = false);
        let _ = with_node_mut!(ctx, Area3D, id, |node| node.enabled = false);
    }
}

fn res_path_to_disk(project_dir: &str, path: &str) -> PathBuf {
    if let Some(rel) = path.strip_prefix("res://") {
        Path::new(project_dir).join("res").join(rel)
    } else {
        Path::new(project_dir).join(path)
    }
}

fn open_code(path: &Path) -> Result<(), String> {
    Command::new("code")
        .arg(path)
        .spawn()
        .or_else(|_| Command::new("code.cmd").arg(path).spawn())
        .map(|_| ())
        .map_err(|err| err.to_string())
}
