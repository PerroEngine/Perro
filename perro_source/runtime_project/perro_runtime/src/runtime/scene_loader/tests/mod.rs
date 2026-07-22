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
        fs::create_dir_all(&path).expect("test or bench setup must succeed");
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
    fs::create_dir(&cache).expect("test or bench setup must succeed");

    let target = write_dlc_cache_file(&cache, "scripts/lib.bin", b"one")
        .expect("test or bench setup must succeed");
    write_dlc_cache_file(&cache, "scripts/lib.bin", b"two")
        .expect("test or bench setup must succeed");

    assert_eq!(target, cache.join("scripts/lib.bin"));
    assert_eq!(
        fs::read(target).expect("test or bench setup must succeed"),
        b"two"
    );
}

#[cfg(unix)]
#[test]
fn dlc_cache_rejects_linked_dir() {
    use std::os::unix::fs::symlink;

    let temp = CacheTempDir::new("linked-dir");
    let cache = temp.0.join("cache");
    let outside = temp.0.join("outside");
    fs::create_dir(&cache).expect("test or bench setup must succeed");
    fs::create_dir(&outside).expect("test or bench setup must succeed");
    symlink(&outside, cache.join("scripts")).expect("test or bench setup must succeed");

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
    fs::create_dir(&cache).expect("test or bench setup must succeed");
    fs::write(&outside, b"safe").expect("test or bench setup must succeed");
    symlink(&outside, cache.join("lib.bin")).expect("test or bench setup must succeed");

    assert!(write_dlc_cache_file(&cache, "lib.bin", b"bad").is_err());
    assert_eq!(
        fs::read(outside).expect("test or bench setup must succeed"),
        b"safe"
    );
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
    fs::create_dir(&cache).expect("test or bench setup must succeed");
    fs::create_dir(&outside).expect("test or bench setup must succeed");
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
    fs::create_dir(&cache).expect("test or bench setup must succeed");
    fs::write(&outside, b"safe").expect("test or bench setup must succeed");
    if !try_cache_symlink_file(&outside, &cache.join("lib.bin")) {
        return;
    }

    assert!(write_dlc_cache_file(&cache, "lib.bin", b"bad").is_err());
    assert_eq!(
        fs::read(outside).expect("test or bench setup must succeed"),
        b"safe"
    );
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
        .expect_err("invalid test input must fail");

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

    let err = runtime
        .apply_route_change("/bad")
        .expect_err("invalid test input must fail");
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
    let scene = Parser::new("$root = @root\n\n[root]\n[Node]\n[/Node]\n[/root]\n").parse_scene();
    let mut prepared = prepare_scene_with_loader_and_styles(&scene, &|_| unreachable!(), None)
        .expect("test or bench setup must succeed");
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
    let scene =
        Parser::new("[first]\n[Node]\n[/Node]\n[/first]\n[second]\n[Node]\n[/Node]\n[/second]\n")
            .parse_scene();
    let mut prepared = prepare_scene_with_loader_and_styles(&scene, &|_| unreachable!(), None)
        .expect("test or bench setup must succeed");
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
    let prepared = prepare_scene_with_loader_and_styles(&scene, &|_| unreachable!(), None)
        .expect("test or bench setup must succeed");

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
fn preload_compiles_once_and_spawns_distinct_instances() {
    let scene = Parser::new("$root = @root\n\n[root]\n[Node]\n[/Node]\n[/root]\n").parse_scene();
    let path = "res://cached_spawn.scn";
    let mut runtime = Runtime::new();
    runtime.project = Some(Arc::new(RuntimeProject::new("Scene Test", ".")));
    runtime
        .scene_cache
        .borrow_mut()
        .insert(path.to_string(), Arc::new(scene));

    let id = runtime
        .preload_scene_at_runtime(path)
        .expect("test or bench setup must succeed");
    assert_eq!(runtime.prepared_scene_cache.borrow().len(), 1);
    assert_eq!(runtime.preloaded_prepared_scenes.len(), 1);

    let first = runtime
        .load_preloaded_scene_at_runtime(id)
        .expect("test or bench setup must succeed");
    let second = runtime
        .load_preloaded_scene_at_runtime(id)
        .expect("test or bench setup must succeed");
    assert_ne!(first, second);
    assert_eq!(runtime.prepared_scene_cache.borrow().len(), 1);

    assert!(runtime.free_preloaded_scene_at_runtime(id));
    assert!(runtime.preloaded_prepared_scenes.is_empty());
    assert!(runtime.prepared_scene_cache.borrow().is_empty());
}

#[test]
fn scene_load_updates_tag_index_during_merge() {
    let scene = Parser::new(
        "$root = @root\n\n[root]\ntags = [\"scene_loaded\"]\n[Node]\n[/Node]\n[/root]\n",
    )
    .parse_scene();
    let prepared = prepare_scene_with_loader_and_styles(&scene, &|_| unreachable!(), None)
        .expect("test or bench setup must succeed");
    let mut runtime = Runtime::new();

    let merged =
        merge_prepared_scene(&mut runtime, prepared).expect("test or bench setup must succeed");
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
    let _project_root_guard = crate::rs_ctx::PROJECT_ROOT_TEST_LOCK
        .lock()
        .expect("test or bench setup must succeed");
    let test_root = std::env::temp_dir().join(format!(
        "perro_runtime_static_dlc_scene_{}",
        std::process::id()
    ));
    let _ = fs::remove_dir_all(&test_root);
    let dlc_scene_dir = test_root.join("dlcs").join("test").join("scenes");
    fs::create_dir_all(&dlc_scene_dir).expect("test or bench setup must succeed");
    fs::write(
        dlc_scene_dir.join("main.scn"),
        "$root = @main\n\n[main]\n[Node]\n[/Node]\n[/main]\n",
    )
    .expect("test or bench setup must succeed");

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
