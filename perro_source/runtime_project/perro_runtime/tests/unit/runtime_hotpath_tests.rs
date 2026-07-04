use super::*;
use perro_io::{ResolvedPath, clear_dlc_mounts, mount_dlc_disk, resolve_path};
use perro_nodes::{Node3D, SceneNode, SceneNodeData};
use perro_scripting::{ScriptBehavior, ScriptContext, ScriptFlags, ScriptLifecycle};
use perro_variant::Variant;
use std::sync::{
    Arc, LazyLock, Mutex,
    atomic::{AtomicUsize, Ordering},
};
use std::{any::Any, path::PathBuf};

struct CountScript {
    update_count: Arc<AtomicUsize>,
    fixed_count: Arc<AtomicUsize>,
}

impl ScriptLifecycle<RuntimeScriptApi> for CountScript {
    fn on_update(&self, _ctx: &mut ScriptContext<'_, RuntimeScriptApi>) {
        self.update_count.fetch_add(1, Ordering::Relaxed);
    }

    fn on_fixed_update(&self, _ctx: &mut ScriptContext<'_, RuntimeScriptApi>) {
        self.fixed_count.fetch_add(1, Ordering::Relaxed);
    }
}

impl ScriptBehavior<RuntimeScriptApi> for CountScript {
    fn script_flags(&self) -> ScriptFlags {
        ScriptFlags::new(ScriptFlags::HAS_UPDATE | ScriptFlags::HAS_FIXED_UPDATE)
    }

    fn create_state(&self) -> Box<dyn Any> {
        Box::new(())
    }

    fn get_var(&self, _state: &dyn Any, _var: perro_ids::ScriptMemberID) -> Variant {
        Variant::Null
    }

    fn set_var(&self, _state: &mut dyn Any, _var: perro_ids::ScriptMemberID, _value: Variant) {}

    fn call_method(
        &self,
        _method: perro_ids::ScriptMemberID,
        _ctx: &mut ScriptContext<'_, RuntimeScriptApi>,
        _params: &[Variant],
    ) -> Variant {
        Variant::Null
    }
}

static DLC_SELF_TEST_PATHS: LazyLock<Mutex<Vec<PathBuf>>> =
    LazyLock::new(|| Mutex::new(Vec::new()));

struct DlcSelfResolveScript;

impl ScriptLifecycle<RuntimeScriptApi> for DlcSelfResolveScript {
    fn on_init(&self, _ctx: &mut ScriptContext<'_, RuntimeScriptApi>) {
        let ResolvedPath::Disk(path) = resolve_path("dlc://self/probe.txt") else {
            panic!("expected dlc self to resolve to disk path");
        };
        DLC_SELF_TEST_PATHS.lock().unwrap().push(path);
    }
}

impl ScriptBehavior<RuntimeScriptApi> for DlcSelfResolveScript {
    fn script_flags(&self) -> ScriptFlags {
        ScriptFlags::new(ScriptFlags::HAS_INIT)
    }

    fn get_var(&self, _state: &dyn Any, _var: perro_ids::ScriptMemberID) -> Variant {
        Variant::Null
    }

    fn set_var(&self, _state: &mut dyn Any, _var: perro_ids::ScriptMemberID, _value: Variant) {}

    fn call_method(
        &self,
        _method: perro_ids::ScriptMemberID,
        _ctx: &mut ScriptContext<'_, RuntimeScriptApi>,
        _params: &[Variant],
    ) -> Variant {
        Variant::Null
    }
}

#[allow(improper_ctypes_definitions)]
extern "C" fn dlc_self_resolve_script_ctor() -> *mut dyn ScriptBehavior<RuntimeScriptApi> {
    Box::into_raw(Box::new(DlcSelfResolveScript))
}

#[test]
fn dlc_self_context_applies_only_during_script_callback() {
    // clear_dlc_mounts/mount_dlc_disk mutate process-global io state that
    // load_boot_scene tests also touch; serialize via the shared root lock.
    let _project_root_guard = crate::rs_ctx::PROJECT_ROOT_TEST_LOCK.lock().unwrap();
    clear_dlc_mounts();
    DLC_SELF_TEST_PATHS.lock().unwrap().clear();

    let root = std::env::temp_dir().join(format!("perro_runtime_dlc_self_{}", std::process::id()));
    let dlc_root = root.join("expansion");
    let _ = std::fs::remove_dir_all(&root);
    std::fs::create_dir_all(&dlc_root).unwrap();
    mount_dlc_disk("Expansion", &dlc_root).unwrap();

    let mut runtime = Runtime::new();
    let node = runtime
        .nodes
        .insert(SceneNode::new(SceneNodeData::Node3D(Node3D::new())));
    let script_hash = 0xD1C5_E1F0_u64;
    runtime
        .script_runtime
        .dynamic_script_registry
        .insert(script_hash, dlc_self_resolve_script_ctor);

    runtime
        .attach_script_instance(node, script_hash, Some("Expansion"), Vec::new())
        .unwrap();

    assert_eq!(
        DLC_SELF_TEST_PATHS.lock().unwrap().as_slice(),
        &[dlc_root.join("probe.txt")]
    );
    match resolve_path("dlc://self/probe.txt") {
        ResolvedPath::Disk(path) => assert_eq!(path, PathBuf::from("dlc://self/probe.txt")),
        other => panic!("expected cleared dlc self context, got {other:?}"),
    }

    clear_dlc_mounts();
    let _ = std::fs::remove_dir_all(&root);
}

#[test]
fn node_arena_len_tracks_active_nodes() {
    let mut arena = NodeArena::new();
    assert_eq!(arena.len(), 0);
    assert!(arena.is_empty());

    let a = arena.insert(SceneNode::new(SceneNodeData::Node3D(Node3D::new())));
    let b = arena.insert(SceneNode::new(SceneNodeData::Node3D(Node3D::new())));
    assert_eq!(arena.len(), 2);
    assert!(!arena.is_empty());

    let _ = arena.remove(a);
    assert_eq!(arena.len(), 1);
    let _ = arena.remove(b);
    assert_eq!(arena.len(), 0);
    assert!(arena.is_empty());
}

#[test]
fn script_update_schedules_toggle_at_runtime() {
    let mut runtime = Runtime::new();
    let update_count = Arc::new(AtomicUsize::new(0));
    let fixed_count = Arc::new(AtomicUsize::new(0));
    let a = runtime
        .nodes
        .insert(SceneNode::new(SceneNodeData::Node3D(Node3D::new())));
    let b = runtime
        .nodes
        .insert(SceneNode::new(SceneNodeData::Node3D(Node3D::new())));

    for id in [a, b] {
        runtime.scripts.insert(
            id,
            Arc::new(CountScript {
                update_count: Arc::clone(&update_count),
                fixed_count: Arc::clone(&fixed_count),
            }),
            Box::new(()),
        );
    }

    runtime.schedules.snapshot_update(&runtime.scripts);
    assert!(runtime.scripts.set_update_enabled(a, false));
    runtime.run_update_schedule();
    assert_eq!(update_count.load(Ordering::Relaxed), 1);

    runtime.schedules.snapshot_update(&runtime.scripts);
    runtime.run_update_schedule();
    assert_eq!(update_count.load(Ordering::Relaxed), 2);

    assert!(runtime.scripts.set_update_enabled(a, true));
    runtime.schedules.snapshot_update(&runtime.scripts);
    runtime.run_update_schedule();
    assert_eq!(update_count.load(Ordering::Relaxed), 4);

    runtime.schedules.snapshot_fixed(&runtime.scripts);
    assert!(runtime.scripts.set_fixed_update_enabled(b, false));
    runtime.run_fixed_schedule();
    assert_eq!(fixed_count.load(Ordering::Relaxed), 1);

    assert!(runtime.scripts.set_fixed_update_enabled(b, true));
    runtime.schedules.snapshot_fixed(&runtime.scripts);
    runtime.run_fixed_schedule();
    assert_eq!(fixed_count.load(Ordering::Relaxed), 3);
}
