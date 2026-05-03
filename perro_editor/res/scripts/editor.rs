use perro_api::prelude::*;
use std::path::Path;

type SelfNodeType = UiPanel;

const ACTIVE_PROJECT: &str = "user://perro_editor_active_project.txt";

#[State]
#[derive(Clone)]
struct EditorState {
    project_dir: String,
    main_scene: String,
    live_root: NodeID,
}

lifecycle!({
    fn on_init(
        &self,
        _ctx: &mut ScriptContext<'_, RT, RS, IP>
    ) {
    }

    fn on_all_init(
        &self,
        ctx: &mut ScriptContext<'_, RT, RS, IP>,
    ) {
        let project_dir = FileMod::load_string(ACTIVE_PROJECT)
            .unwrap_or_default()
            .trim()
            .to_string();
        if !project_dir.is_empty() {
            FileMod::set_project_root_disk(&project_dir, "perro_editor_live_project");
        }

        let main_scene = read_main_scene(&project_dir).unwrap_or_else(|| "res://main.scn".to_string());

        let _ = with_state_mut!(ctx.run, EditorState, ctx.id, |state| {
            state.project_dir = project_dir.clone();
            state.main_scene = main_scene.clone();
            state.live_root = NodeID::default();
        });

        load_preview_scene(ctx);
    }

    fn on_update(
        &self,
        _ctx: &mut ScriptContext<'_, RT, RS, IP>,
    ) {
    }

    fn on_fixed_update(
        &self,
         _ctx: &mut ScriptContext<'_, RT, RS, IP>,
    ) {
    }

    fn on_removal(
        &self,
   ctx: &mut ScriptContext<'_, RT, RS, IP>,
    ) {
        let live_root = with_state!(ctx.run, EditorState, ctx.id, |state| state.live_root);
        if !live_root.is_nil() {
            let _ = remove_node!(ctx.run, live_root);
        }
    }
});

fn load_preview_scene<RT: RuntimeAPI + ?Sized, RS: ResourceAPI + ?Sized, IP: InputAPI + ?Sized>(
   ctx: &mut ScriptContext<'_, RT, RS, IP>,
) {
    let (project_dir, main_scene, prev_root) = with_state!(ctx.run, EditorState, ctx.id, |state| {
        (state.project_dir.clone(), state.main_scene.clone(), state.live_root)
    });

    if !prev_root.is_nil() {
        let _ = remove_node!(ctx.run, prev_root);
    }

    if project_dir.is_empty() {
        return;
    }

    let Ok(doc) = scene_load_doc!(ctx.res, main_scene.clone()) else {
        return;
    };
    let Ok(live_scene_path) = write_live_scene_doc(&doc) else {
        return;
    };
    let Ok(root) = scene_load!(ctx.run, live_scene_path) else {
        return;
    };

    if !reparent!(ctx.run, ctx.id, root) {
        let _ = remove_node!(ctx.run, root);
        return;
    }

    apply_live_root(ctx, root);
    disable_physics(ctx, root);

    let _ = with_state_mut!(ctx.run, EditorState, ctx.id, |state| {
        state.live_root = root;
    });
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

fn write_live_scene_doc(doc: &perro_scene::SceneDoc) -> Result<String, String> {
    let mut live_doc = doc.clone();
    let root_key = live_doc.scene.root.clone();
    for node in live_doc.scene.nodes.to_mut() {
        node.script = None;
        node.script_hash = None;
        node.clear_script = true;
        node.root_of = None;
        node.root_of_hash = None;
        if let Some(root_key) = &root_key {
            if node.parent.is_none() && node.key.as_ref() != root_key.as_ref() {
                node.parent = Some(root_key.clone());
            }
        }
    }
    live_doc.normalize_links();
    let path = std::env::temp_dir().join("perro_editor_live_scene.scn");
    std::fs::write(&path, live_doc.to_text()).map_err(|err| err.to_string())?;
    Ok(path.to_string_lossy().to_string())
}

fn apply_live_root<RT: RuntimeAPI + ?Sized, RS: ResourceAPI + ?Sized, IP: InputAPI + ?Sized>(
    ctx: &mut ScriptContext<'_, RT, RS, IP>,
    root: NodeID
) {
    let _ = with_base_node_mut!(ctx.run, UiBox, root, |node| {
        node.layout.anchor = UiAnchor::Center;
        node.layout.size = UiVector2::ratio(1.0, 1.0);
        node.layout.h_size = UiSizeMode::Fill;
        node.layout.v_size = UiSizeMode::Fill;
        node.transform.position = UiVector2::ratio(0.5, 0.5);
        node.transform.pivot = UiVector2::ratio(0.5, 0.5);
        node.transform.translation = Vector2::ZERO;
        node.transform.rotation = 0.0;
        node.transform.scale = Vector2::ONE;
    });

    disable_ui_node_input(ctx, root);
    for id in query!(ctx.run, base[UiBox], in_subtree(root)) {
        disable_ui_node_input(ctx, id);
    }
}

fn disable_ui_node_input<RT: RuntimeAPI + ?Sized, RS: ResourceAPI + ?Sized, IP: InputAPI + ?Sized>(
    ctx: &mut ScriptContext<'_, RT, RS, IP>,
    id: NodeID
) {
    let _ = with_base_node_mut!(ctx.run, UiBox, id, |node| {
        node.input_enabled = false;
        node.mouse_filter = UiMouseFilter::Ignore;
    });
}

fn disable_physics<RT: RuntimeAPI + ?Sized, RS: ResourceAPI + ?Sized, IP: InputAPI + ?Sized>(
    ctx: &mut ScriptContext<'_, RT, RS, IP>,
    root: NodeID
) {
    let ids = query!(
        ctx.run,
        any(
            is[StaticBody2D, RigidBody2D, Area2D, StaticBody3D, RigidBody3D, Area3D]
        ),
        in_subtree(root)
    );
    for id in ids {
        let _ = with_node_mut!(ctx.run, StaticBody2D, id, |node| node.enabled = false);
        let _ = with_node_mut!(ctx.run, RigidBody2D, id, |node| node.enabled = false);
        let _ = with_node_mut!(ctx.run, Area2D, id, |node| node.enabled = false);
        let _ = with_node_mut!(ctx.run, StaticBody3D, id, |node| node.enabled = false);
        let _ = with_node_mut!(ctx.run, RigidBody3D, id, |node| node.enabled = false);
        let _ = with_node_mut!(ctx.run, Area3D, id, |node| node.enabled = false);
    }
}
