use perro_api::prelude::*;
use std::borrow::Cow;
use std::path::{Path, PathBuf};

type SelfNodeType = UiPanel;

const ACTIVE_PROJECT: &str = "user://perro_editor_active_project.txt";
const RECENT_PROJECTS: &str = "user://perro_editor_recent_projects.txt";

#[State]
#[derive(Clone)]
struct ProjectManagerState {
    status_label: NodeID,
    project_name_input: NodeID,
    project_root_input: NodeID,
    open_path_input: NodeID,
    recent_labels: Vec<NodeID>,
    recent: Vec<String>,
}

lifecycle!({
    fn on_init(
        &self,
        _ctx: &mut ScriptContext<'_, API>,
    ) {
    }

    fn on_all_init(
        &self,
        ctx: &mut ScriptContext<'_, API>,
    ) {
        let top_bar = child(ctx, ctx.id, "top_bar");
        let content = child(ctx, ctx.id, "content");
        let left_panel = child(ctx, content, "left_panel");
        let left_stack = child(ctx, left_panel, "left_stack");
        let right_panel = child(ctx, content, "right_panel");
        let right_stack = child(ctx, right_panel, "right_stack");

        let status_label = child(ctx, top_bar, "status");
        let project_name_input = child(ctx, left_stack, "project_name_input");
        let project_root_input = child(ctx, left_stack, "project_root_input");
        let open_path_input = child(ctx, left_stack, "open_path_input");
        let recent = read_recent();
        let recent_0 = child(ctx, right_stack, "recent_0");
        let recent_1 = child(ctx, right_stack, "recent_1");
        let recent_2 = child(ctx, right_stack, "recent_2");
        let recent_labels = vec![
            child(ctx, recent_0, "recent_0_label"),
            child(ctx, recent_1, "recent_1_label"),
            child(ctx, recent_2, "recent_2_label"),
        ];

        let _ = with_state_mut!(ctx.run, ProjectManagerState, ctx.id, |state| {
            state.status_label = status_label;
            state.project_name_input = project_name_input;
            state.project_root_input = project_root_input;
            state.open_path_input = open_path_input;
            state.recent_labels = recent_labels.clone();
            state.recent = recent.clone();
        });

        write_recent_labels(ctx, &recent_labels, &recent);
        set_status(ctx, status_label, "Pick project");

        signal_connect!(
            ctx.run,
            ctx.id,
            signal!("create_project_button_click"),
            func!("on_create_project")
        );
        signal_connect!(
            ctx.run,
            ctx.id,
            signal!("open_project_button_click"),
            func!("on_open_project")
        );
        signal_connect!(
            ctx.run,
            ctx.id,
            signal!("recent_0_click"),
            func!("on_recent_0")
        );
        signal_connect!(
            ctx.run,
            ctx.id,
            signal!("recent_1_click"),
            func!("on_recent_1")
        );
        signal_connect!(
            ctx.run,
            ctx.id,
            signal!("recent_2_click"),
            func!("on_recent_2")
        );
    }

    fn on_update(
        &self,
        _ctx: &mut ScriptContext<'_, API>,
    ) {
    }

    fn on_fixed_update(
        &self,
        _ctx: &mut ScriptContext<'_, API>,
    ) {
    }

    fn on_removal(
        &self,
        _ctx: &mut ScriptContext<'_, API>,
    ) {
    }
});

methods!({
    fn on_create_project(
        &self,
        ctx: &mut ScriptContext<'_, API>,
        _button: NodeID,
    ) {
        let (name_id, root_id, status_id) =
            with_state!(ctx.run, ProjectManagerState, ctx.id, |state| {
                (
                    state.project_name_input,
                    state.project_root_input,
                    state.status_label,
                )
            });
        let name = text_box_text(ctx, name_id);
        let root = text_box_text(ctx, root_id);
        let project_name = clean_name(&name);
        let parent = resolve_path(root.trim());
        let project_dir = parent.join(&project_name);

        set_status(ctx, status_id, "Create project...");
        match create_editor_project(&project_dir, &project_name) {
            Ok(()) => open_project(ctx, ctx.id, project_dir),
            Err(err) => set_status(ctx, status_id, &format!("Create fail: {err}")),
        }
    }

    fn on_open_project(
        &self,
        ctx: &mut ScriptContext<'_, API>,
        _button: NodeID,
    ) {
        let path_id = with_state!(ctx.run, ProjectManagerState, ctx.id, |state| {
            state.open_path_input
        });
        let path = text_box_text(ctx, path_id);
        open_project(ctx, ctx.id, resolve_path(path.trim()));
    }

    fn on_recent_0(
        &self,
        ctx: &mut ScriptContext<'_, API>,
        _button: NodeID,
    ) {
        open_recent(ctx, 0);
    }

    fn on_recent_1(
        &self,
        ctx: &mut ScriptContext<'_, API>,
        _button: NodeID,
    ) {
        open_recent(ctx, 1);
    }

    fn on_recent_2(
        &self,
        ctx: &mut ScriptContext<'_, API>,
        _button: NodeID,
    ) {
        open_recent(ctx, 2);
    }
});

fn child<API: ScriptAPI + ?Sized>(
           ctx: &mut ScriptContext<'_, API>,
    parent: NodeID,
    name: &str,
) -> NodeID {
    if parent.is_nil() {
        return NodeID::default();
    }
    ctx.run.Nodes()
        .get_child_by_name(parent, name)
        .unwrap_or_default()
}

fn set_status<API: ScriptAPI + ?Sized>(
    ctx: &mut ScriptContext<'_, API>,
    label_id: NodeID,
    text: &str,
) {
    if label_id.is_nil() {
        return;
    }
    let _ = with_node_mut!(ctx.run, UiLabel, label_id, |label| {
        label.text = Cow::Owned(text.to_string());
    });
}

fn text_box_text<API: ScriptAPI + ?Sized>(
    ctx: &mut ScriptContext<'_, API>,
    id: NodeID,
) -> String {
    if id.is_nil() {
        return String::new();
    }
    with_node!(ctx.run, UiTextBox, id, |input| input.text.to_string())
}

fn read_recent() -> Vec<String> {
    FileMod::load_string(RECENT_PROJECTS)
        .unwrap_or_default()
        .lines()
        .map(|line| normalize_display_path(line.trim()))
        .filter(|line| !line.is_empty())
        .take(8)
        .collect()
}

fn save_recent(path: &Path) -> Vec<String> {
    let normalized = normalize(path);
    let mut recent = read_recent();
    recent.retain(|item| item != &normalized);
    recent.insert(0, normalized);
    recent.truncate(8);
    let _ = FileMod::save_string(RECENT_PROJECTS, &recent.join("\n"));
    recent
}

fn write_recent_labels<API: ScriptAPI + ?Sized>(
    ctx: &mut ScriptContext<'_, API>,
    labels: &[NodeID],
    recent: &[String],
) {
    for (index, label_id) in labels.iter().copied().enumerate() {
        let text = recent
            .get(index)
            .map(String::as_str)
            .unwrap_or("No recent project");
        set_status(ctx, label_id, text);
    }
}

fn open_recent<API: ScriptAPI + ?Sized>(
    ctx: &mut ScriptContext<'_, API>,
    index: usize,
) {
    let path = with_state!(ctx.run, ProjectManagerState, ctx.id, |state| {
        state.recent.get(index).cloned()
    });
    if let Some(path) = path {
        open_project(ctx, ctx.id, PathBuf::from(path));
    }
}

fn open_project<API: ScriptAPI + ?Sized>(
    ctx: &mut ScriptContext<'_, API>,
    self_id: NodeID,
    project_dir: PathBuf,
) {
    let status_id = with_state!(ctx.run, ProjectManagerState, ctx.id, |state| state
        .status_label);
    let project_dir = project_dir.canonicalize().unwrap_or(project_dir);
    if !project_dir.join("project.toml").exists() {
        set_status(ctx, status_id, "Open fail: project.toml missing");
        return;
    }

    let recent = save_recent(&project_dir);
    let labels = with_state!(ctx.run, ProjectManagerState, ctx.id, |state| state
        .recent_labels
        .clone());
    write_recent_labels(ctx, &labels, &recent);
    let _ = with_state_mut!(ctx.run, ProjectManagerState, ctx.id, |state| {
        state.recent = recent;
    });

    let _ = FileMod::save_string(ACTIVE_PROJECT, &normalize(&project_dir));
    match scene_load!(ctx.run, "res://editor.scn") {
        Ok(_) => {
            let _ = remove_node!(ctx.run, ctx.id);
        }
        Err(err) => set_status(ctx, status_id, &format!("Editor load fail: {err}")),
    }
}

fn clean_name(raw: &str) -> String {
    let out: String = raw
        .trim()
        .chars()
        .map(|c| match c {
            '<' | '>' | ':' | '"' | '/' | '\\' | '|' | '?' | '*' => '_',
            _ => c,
        })
        .collect();
    if out.trim().is_empty() {
        "perro_project".to_string()
    } else {
        out.trim_matches('.').to_string()
    }
}

fn resolve_path(raw: &str) -> PathBuf {
    let path = PathBuf::from(raw);
    if path.is_absolute() {
        path
    } else {
        repo_root().join(path)
    }
}

fn create_editor_project(project_dir: &Path, name: &str) -> Result<(), String> {
    create_new_project(project_dir, name).map_err(|err| err.to_string())?;
    std::fs::write(project_dir.join("res").join("main.scn"), blank_main_scene())
        .map_err(|err| err.to_string())
}

fn blank_main_scene() -> &'static str {
    r#"@root = main

[main]

[Node3D]
    position = (0, 0, 0)
[/Node3D]
[/main]
"#
}

fn repo_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("..")
        .join("..")
        .join("..")
}

fn normalize(path: &Path) -> String {
    normalize_display_path(
        &path
            .canonicalize()
            .unwrap_or_else(|_| path.to_path_buf())
            .to_string_lossy()
            .replace('\\', "/"),
    )
}

fn normalize_display_path(raw: &str) -> String {
    let mut out = raw.replace('\\', "/");
    if let Some(stripped) = out.strip_prefix("//?/") {
        out = stripped.to_string();
    }
    if let Some(stripped) = out.strip_prefix("//./") {
        out = stripped.to_string();
    }
    out
}


