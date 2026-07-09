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
fn node_arena_name_index_tracks_insert_rename_remove() {
    let mut arena = NodeArena::new();
    let mut named = SceneNode::new(SceneNodeData::Node3D(Node3D::new()));
    named.set_name("alpha");
    let a = arena.insert(named);
    let b = arena.insert(SceneNode::new(SceneNodeData::Node3D(Node3D::new())));

    assert_eq!(arena.named_ids("alpha"), &[a]);
    assert!(arena.named_ids("beta").is_empty());

    // Rename keeps the index in sync (old entry gone, new one present).
    assert!(arena.rename(a, "beta".into()));
    assert!(arena.named_ids("alpha").is_empty());
    assert_eq!(arena.named_ids("beta"), &[a]);

    // Naming a second node appends in insertion order.
    assert!(arena.rename(b, "beta".into()));
    assert_eq!(arena.named_ids("beta"), &[a, b]);

    // Removal drops only the removed id; empty names never indexed.
    let _ = arena.remove(a);
    assert_eq!(arena.named_ids("beta"), &[b]);
    assert!(arena.rename(b, "".into()));
    assert!(arena.named_ids("beta").is_empty());

    // Dead ids fail.
    assert!(!arena.rename(a, "gamma".into()));
}

#[test]
fn node_arena_slot_mirrors_track_insert_remove_reuse_reparent() {
    use perro_nodes::{Node2D, NodeType, Sprite2D};

    let mut arena = NodeArena::new();
    let parent = arena.insert(SceneNode::new(SceneNodeData::Node3D(Node3D::new())));
    let mut child = SceneNode::new(SceneNodeData::Sprite2D(Sprite2D::default()));
    child.parent = parent; // pre-insert parent is captured by insert
    let child = arena.insert(child);

    assert_eq!(
        arena.slot_node_type(parent.index() as usize),
        Some(NodeType::Node3D)
    );
    assert_eq!(
        arena.slot_node_type(child.index() as usize),
        Some(NodeType::Sprite2D)
    );
    assert_eq!(arena.parent_slots()[child.index() as usize], parent);
    assert_eq!(arena.parent_slots()[parent.index() as usize], NodeID::nil());
    arena.validate_mirrors();

    // Reparent through the arena keeps mirror + node in sync and moves versions.
    let other = arena.insert(SceneNode::new(SceneNodeData::Node3D(Node3D::new())));
    let before = arena.mutation_revision();
    assert!(arena.set_parent(child, other));
    assert!(arena.mutation_revision() > before);
    assert_eq!(arena.parent_slots()[child.index() as usize], other);
    assert_eq!(arena.get(child).expect("live").parent, other);
    arena.validate_mirrors();
    assert!(!arena.set_parent(NodeID::nil(), other));

    // Remove clears the parent lane; slot reuse rewrites both lanes.
    let child_slot = child.index() as usize;
    let _ = arena.remove(child);
    assert_eq!(arena.parent_slots()[child_slot], NodeID::nil());
    let reused = arena.insert(SceneNode::new(SceneNodeData::Node2D(Node2D::default())));
    assert_eq!(reused.index() as usize, child_slot);
    assert_eq!(arena.slot_node_type(child_slot), Some(NodeType::Node2D));
    arena.validate_mirrors();

    // Clear resets lanes to the nil sentinel only.
    arena.clear();
    assert_eq!(arena.node_type_slots().len(), 1);
    assert_eq!(arena.parent_slots().len(), 1);
    arena.validate_mirrors();
}

#[test]
fn node_arena_structural_revision_moves_only_on_structural_change() {
    let mut arena = NodeArena::new();
    let a = arena.insert(SceneNode::new(SceneNodeData::Node3D(Node3D::new())));

    // Data mutation via get_mut bumps mutation_revision but NOT structural.
    let sv = arena.structural_revision();
    let mv = arena.mutation_revision();
    let _ = arena.get_mut(a);
    assert_eq!(
        arena.structural_revision(),
        sv,
        "data mut must not bump structural"
    );
    assert!(
        arena.mutation_revision() > mv,
        "data mut still bumps mutation"
    );

    // Insert, reparent, remove each move structural_revision.
    let sv = arena.structural_revision();
    let b = arena.insert(SceneNode::new(SceneNodeData::Node3D(Node3D::new())));
    assert!(arena.structural_revision() > sv, "insert bumps structural");

    let sv = arena.structural_revision();
    assert!(arena.set_parent(a, b));
    assert!(
        arena.structural_revision() > sv,
        "reparent bumps structural"
    );

    let sv = arena.structural_revision();
    let _ = arena.remove(a);
    assert!(arena.structural_revision() > sv, "remove bumps structural");

    // The audio-flag bug: a remove+insert pair that leaves len() unchanged must
    // still move structural_revision so downstream gates rescan.
    let len_before = arena.len();
    let sv = arena.structural_revision();
    let _ = arena.remove(b);
    let _ = arena.insert(SceneNode::new(SceneNodeData::Node3D(Node3D::new())));
    assert_eq!(arena.len(), len_before, "count unchanged by remove+insert");
    assert!(
        arena.structural_revision() > sv,
        "count-neutral remove+insert must still bump structural"
    );
}

#[test]
fn node_arena_tag_index_tracks_insert_mutate_remove() {
    let mut arena = NodeArena::new();
    let enemy = perro_ids::NodeTag::borrowed("enemy");
    let boss = perro_ids::NodeTag::borrowed("boss");
    let enemy_id = enemy.id;
    let boss_id = boss.id;

    let mut tagged = SceneNode::new(SceneNodeData::Node3D(Node3D::new()));
    tagged.set_tags(Some(vec![enemy.clone()]));
    let a = arena.insert(tagged);
    let b = arena.insert(SceneNode::new(SceneNodeData::Node3D(Node3D::new())));

    // Insert indexes pre-set tags.
    assert!(
        arena
            .tag_index()
            .get(&enemy_id)
            .is_some_and(|s| s.contains(&a))
    );

    // add/remove keep the index in sync; adding a duplicate is a no-op.
    assert!(arena.add_node_tag(b, enemy.clone()));
    assert!(arena.add_node_tag(b, enemy.clone()));
    assert_eq!(arena.tag_index().get(&enemy_id).map(|s| s.len()), Some(2));
    assert!(arena.remove_node_tag(b, enemy_id));
    assert!(
        !arena
            .tag_index()
            .get(&enemy_id)
            .is_some_and(|s| s.contains(&b))
    );

    // set_node_tags replaces: enemy entry swaps to boss.
    assert!(arena.set_node_tags(a, Some(vec![boss.clone()])));
    assert!(arena.tag_index().get(&enemy_id).is_none());
    assert!(
        arena
            .tag_index()
            .get(&boss_id)
            .is_some_and(|s| s.contains(&a))
    );

    // Removal + clear drop entries; dead ids fail.
    let _ = arena.remove(a);
    assert!(arena.tag_index().get(&boss_id).is_none());
    assert!(!arena.add_node_tag(a, boss));
    arena.clear();
    assert!(arena.tag_index().is_empty());
}

#[test]
fn node_arena_edit_tracks_name_tags_and_parent() {
    let mut arena = NodeArena::new();
    let parent = arena.insert(SceneNode::new(SceneNodeData::Node3D(Node3D::new())));
    let other_parent = arena.insert(SceneNode::new(SceneNodeData::Node3D(Node3D::new())));
    let old_tag = perro_ids::NodeTag::borrowed("old");
    let new_tag = perro_ids::NodeTag::borrowed("new");

    let mut node = SceneNode::new(SceneNodeData::Node3D(Node3D::new()));
    node.name = "before".into();
    node.set_tags(Some(vec![old_tag.clone()]));
    node.parent = parent;
    let id = arena.insert(node);
    let structural_before = arena.structural_revision();

    let result = arena.edit(id, |node| {
        node.name = "after".into();
        node.set_tags(Some(vec![new_tag.clone()]));
        node.parent = other_parent;
        42
    });

    assert_eq!(result, Some(42));
    assert!(arena.named_ids("before").is_empty());
    assert_eq!(arena.named_ids("after"), &[id]);
    assert!(
        !arena
            .tag_index()
            .get(&old_tag.id)
            .is_some_and(|ids| ids.contains(&id))
    );
    assert!(
        arena
            .tag_index()
            .get(&new_tag.id)
            .is_some_and(|ids| ids.contains(&id))
    );
    assert_eq!(arena.parent_slots()[id.index() as usize], other_parent);
    assert_eq!(arena.get(id).expect("edited node").parent, other_parent);
    assert!(arena.structural_revision() > structural_before);
    arena.validate_mirrors();
}

#[test]
fn node_arena_edit_repairs_indices_during_unwind() {
    let mut arena = NodeArena::new();
    let parent = arena.insert(SceneNode::new(SceneNodeData::Node3D(Node3D::new())));
    let tag = perro_ids::NodeTag::borrowed("panic-safe");
    let id = arena.insert(SceneNode::new(SceneNodeData::Node3D(Node3D::new())));

    let panic = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        arena.edit(id, |node| {
            node.name = "unwound".into();
            node.set_tags(Some(vec![tag.clone()]));
            node.parent = parent;
            panic!("test panic");
        });
    }));

    assert!(panic.is_err());
    assert_eq!(arena.named_ids("unwound"), &[id]);
    assert!(
        arena
            .tag_index()
            .get(&tag.id)
            .is_some_and(|ids| ids.contains(&id))
    );
    assert_eq!(arena.parent_slots()[id.index() as usize], parent);
    arena.validate_mirrors();
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
