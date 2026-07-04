use crate::{
    Runtime,
    runtime_project::{ProviderMode, RuntimeProject},
};
use perro_ids::{NodeID, ScriptMemberID, tags};
use perro_nodes::{
    Bone3D, BoneAttachment3D, Camera2D, Node2D, Node3D, NodeType, SceneNode, SceneNodeData,
    Skeleton3D, Sprite2D, UiButton, UiLabel, UiNode, UiPanel,
};
use perro_runtime_api::node_collection;
use perro_runtime_api::sub_apis::{NodeAPI, NodeScriptSpec, NodeScriptVar, NodeSpec};
use perro_scene::{Scene, SceneKey, SceneNodeEntry};
use perro_structs::{Quaternion, Transform2D, Transform3D, Vector2, Vector3};
use std::borrow::Cow;

const EMPTY_FIELDS: &[perro_scene::SceneObjectField] = &[];
const EMPTY_KEYS: &[SceneKey] = &[];
const EMPTY_TAGS: &[Cow<'static, str>] = &[];
const SCENE_KEY_NAMES: &[Cow<'static, str>] =
    &[Cow::Borrowed("Bob"), Cow::Borrowed("scene_builtin")];
const SCENE_ROOT_CHILDREN: &[SceneKey] = &[SceneKey(1)];
const SCENE_ROOT_DATA: perro_scene::SceneNodeData = perro_scene::SceneNodeData::new(
    perro_nodes::NodeType::Node3D,
    Cow::Borrowed(EMPTY_FIELDS),
    None,
);
const SCENE_CHILD_DATA: perro_scene::SceneNodeData = perro_scene::SceneNodeData::new(
    perro_nodes::NodeType::Node3D,
    Cow::Borrowed(EMPTY_FIELDS),
    None,
);
const STATIC_SCENE_NODES: &[SceneNodeEntry] = &[
    SceneNodeEntry {
        data: SCENE_ROOT_DATA,
        has_data_override: true,
        key: SceneKey(0),
        name: Some(Cow::Borrowed("Bob")),
        tags: Cow::Borrowed(EMPTY_TAGS),
        children: Cow::Borrowed(SCENE_ROOT_CHILDREN),
        parent: None,
        script: None,
        clear_script: false,
        root_of: None,
        script_vars: Cow::Borrowed(EMPTY_FIELDS),
    },
    SceneNodeEntry {
        data: SCENE_CHILD_DATA,
        has_data_override: true,
        key: SceneKey(1),
        name: Some(Cow::Borrowed("scene_builtin")),
        tags: Cow::Borrowed(EMPTY_TAGS),
        children: Cow::Borrowed(EMPTY_KEYS),
        parent: Some(SceneKey(0)),
        script: None,
        clear_script: false,
        root_of: None,
        script_vars: Cow::Borrowed(EMPTY_FIELDS),
    },
];
static STATIC_SCENE: Scene = Scene {
    nodes: Cow::Borrowed(STATIC_SCENE_NODES),
    root: Some(SceneKey(0)),
    key_names: Cow::Borrowed(SCENE_KEY_NAMES),
};

fn static_scene_lookup(_path_hash: u64) -> &'static Scene {
    &STATIC_SCENE
}

fn child_with_name(
    runtime: &mut Runtime,
    parent: perro_ids::NodeID,
    name: &str,
) -> perro_ids::NodeID {
    let children = runtime.get_node_children_ids(parent).unwrap_or_default();
    let names = children
        .iter()
        .map(|&child| {
            runtime
                .get_node_name(child)
                .unwrap_or_default()
                .into_owned()
        })
        .collect::<Vec<_>>();
    children
        .into_iter()
        .find(|&child| runtime.get_node_name(child).as_deref() == Some(name))
        .unwrap_or_else(|| panic!("missing child `{name}`; children={names:?}"))
}

fn approx(a: f32, b: f32) -> bool {
    (a - b).abs() <= 1e-4
}

#[test]
fn create_nodes_batches_parent_names_and_tags() {
    let mut runtime = Runtime::new();
    let parent_id = runtime.create::<Node2D>();
    let requests = [
        NodeSpec::new(Node2D::new())
            .name("EnemyA")
            .tags(tags!["enemy"]),
        NodeSpec::new(Node2D::new())
            .name("EnemyB")
            .tags(tags!["enemy"]),
    ];
    let ids = runtime.create_nodes(&requests, parent_id);

    assert_eq!(ids.len(), 2);
    assert_eq!(runtime.get_node_children_ids(parent_id), Some(ids.clone()));
    assert_eq!(runtime.get_node_parent_id(ids[0]), Some(parent_id));
    assert_eq!(runtime.get_node_name(ids[0]).as_deref(), Some("EnemyA"));
    assert_eq!(
        runtime.get_node_tags(ids[1]).as_deref(),
        Some([std::borrow::Cow::Borrowed("enemy")].as_slice())
    );
}

#[test]
fn create_nodes_supports_root_requests_without_metadata() {
    let mut runtime = Runtime::new();
    let ids = runtime.create_nodes(
        &[
            NodeSpec::new(Node2D::new()),
            NodeSpec::new(Node2D::new()).name("RootOnly"),
        ],
        perro_ids::NodeID::nil(),
    );

    assert_eq!(ids.len(), 2);
    assert_eq!(
        runtime.get_node_parent_id(ids[0]),
        Some(perro_ids::NodeID::nil())
    );
    assert_eq!(runtime.get_node_name(ids[0]).as_deref(), Some("Node"));
    assert_eq!(runtime.get_node_name(ids[1]).as_deref(), Some("RootOnly"));
    assert_eq!(runtime.get_node_tags(ids[0]), Some(Vec::new()));
}

#[test]
fn create_nodes_accepts_recursive_node_collection() {
    let mut runtime = Runtime::new();
    let parent_id = runtime.create::<Node2D>();
    let collection = node_collection! {
        {
            name = "root",
            tags = tags!["root"],
            node = Node2D::new(),
            children = [
                {
                    name = "panel",
                    node = UiPanel {
                        base: UiNode {
                            clip_children: true,
                            ..UiNode::new()
                        },
                        ..UiPanel::new()
                    },
                    children = [
                        {
                            name = "title",
                            tags = tags!["title"],
                            node = UiLabel {
                                text: "Paused".into(),
                                font_size: 32.0,
                                ..UiLabel::new()
                            },
                        },
                    ],
                },
            ],
        }
    };

    let ids = runtime.create_nodes(collection, parent_id);

    assert_eq!(ids.len(), 3);
    assert_eq!(runtime.get_node_children_ids(parent_id), Some(vec![ids[0]]));
    assert_eq!(runtime.get_node_children_ids(ids[0]), Some(vec![ids[1]]));
    assert_eq!(runtime.get_node_children_ids(ids[1]), Some(vec![ids[2]]));
    assert_eq!(runtime.get_node_parent_id(ids[2]), Some(ids[1]));
    assert_eq!(runtime.get_node_name(ids[0]).as_deref(), Some("root"));
    assert_eq!(
        runtime.get_node_tags(ids[0]).as_deref(),
        Some([std::borrow::Cow::Borrowed("root")].as_slice())
    );
    assert_eq!(runtime.get_node_name(ids[2]).as_deref(), Some("title"));

    assert!(runtime.with_node::<UiPanel, _>(ids[1], |panel| panel.base.clip_children));
    assert_eq!(
        runtime.with_node::<UiLabel, _>(ids[2], |label| { (label.text.clone(), label.font_size) }),
        ("Paused".into(), 32.0)
    );
}

#[test]
fn create_nodes_accepts_multi_root_mixed_collection() {
    let mut runtime = Runtime::new();
    let parent_id = runtime.create::<Node2D>();
    let collection = node_collection![
        {
            name = "hud",
            tags = tags!["ui"],
            node = UiPanel {
                base: UiNode {
                    clip_children: true,
                    ..UiNode::new()
                },
                ..UiPanel::new()
            },
            children = [
                {
                    name = "score",
                    node = UiLabel {
                        text: "000".into(),
                        font_size: 24.0,
                        ..UiLabel::new()
                    },
                },
                {
                    name = "pause",
                    tags = tags!["button", "menu"],
                    node = UiButton::new(),
                },
            ],
        },
        {
            name = "actor",
            tags = tags!["player"],
            node = Node2D {
                transform: Transform2D {
                    position: Vector2::new(5.0, 7.0),
                    ..Transform2D::IDENTITY
                },
                ..Node2D::new()
            },
            children = [
                {
                    name = "sprite",
                    node = Sprite2D::new(),
                    children = [
                        {
                            name = "muzzle",
                            node = Node2D::new(),
                        },
                    ],
                },
                {
                    name = "camera",
                    node = Camera2D::new(),
                },
            ],
        },
        {
            name = "loose_marker",
            node = Node2D::new(),
        },
    ];

    let ids = runtime.create_nodes(collection, parent_id);

    assert_eq!(ids.len(), 8);
    assert_eq!(
        runtime.get_node_children_ids(parent_id),
        Some(vec![ids[0], ids[3], ids[7]])
    );
    assert_eq!(
        runtime.get_node_children_ids(ids[0]),
        Some(vec![ids[1], ids[2]])
    );
    assert_eq!(
        runtime.get_node_children_ids(ids[3]),
        Some(vec![ids[4], ids[6]])
    );
    assert_eq!(runtime.get_node_children_ids(ids[4]), Some(vec![ids[5]]));
    assert_eq!(runtime.get_node_parent_id(ids[5]), Some(ids[4]));
    assert_eq!(runtime.get_node_parent_id(ids[7]), Some(parent_id));
    assert_eq!(runtime.get_node_name(ids[5]).as_deref(), Some("muzzle"));
    assert_eq!(
        runtime.with_node::<Node2D, _>(ids[3], |node| node.transform.position),
        Vector2::new(5.0, 7.0)
    );
}

#[test]
fn create_nodes_accepts_collections_inside_collections() {
    let mut runtime = Runtime::new();
    let parent_id = runtime.create::<Node2D>();
    let toolbar = node_collection![
        {
            name = "inventory",
            tags = tags!["tool"],
            node = UiButton::new(),
        },
        {
            name = "map",
            tags = tags!["tool"],
            node = UiButton::new(),
        },
    ];
    let stats = node_collection! {
        {
            name = "stats",
            node = UiPanel::new(),
            children = [
                {
                    name = "hp",
                    node = UiLabel {
                        text: "HP".into(),
                        ..UiLabel::new()
                    },
                },
                {
                    name = "mp",
                    node = UiLabel {
                        text: "MP".into(),
                        ..UiLabel::new()
                    },
                },
            ],
        }
    };
    let hud = node_collection! {
        {
            name = "hud_root",
            node = UiPanel::new(),
            children = [
                { collection = toolbar.clone() },
                { collection = stats },
                {
                    name = "footer",
                    node = UiLabel {
                        text: "Ready".into(),
                        ..UiLabel::new()
                    },
                },
            ],
        }
    };
    let scene = node_collection![
        { collection = hud },
        {
            name = "world_root",
            node = Node2D::new(),
            children = [
                { collection = toolbar },
                { name = "spawn", node = Node2D::new() },
            ],
        },
    ];

    let ids = runtime.create_nodes(scene, parent_id);

    assert_eq!(ids.len(), 11);
    assert_eq!(
        runtime.get_node_children_ids(parent_id),
        Some(vec![ids[0], ids[7]])
    );
    assert_eq!(
        runtime.get_node_children_ids(ids[0]),
        Some(vec![ids[1], ids[2], ids[3], ids[6]])
    );
    assert_eq!(
        runtime.get_node_children_ids(ids[3]),
        Some(vec![ids[4], ids[5]])
    );
    assert_eq!(
        runtime.get_node_children_ids(ids[7]),
        Some(vec![ids[8], ids[9], ids[10]])
    );
    assert_eq!(runtime.get_node_parent_id(ids[4]), Some(ids[3]));
    assert_eq!(runtime.get_node_parent_id(ids[8]), Some(ids[7]));
    assert_eq!(runtime.get_node_name(ids[1]).as_deref(), Some("inventory"));
    assert_eq!(runtime.get_node_name(ids[4]).as_deref(), Some("hp"));
    assert_eq!(runtime.get_node_name(ids[10]).as_deref(), Some("spawn"));
}

#[test]
fn create_nodes_accepts_scene_refs_inside_collections() {
    // from_project boots the scene, which writes the process-global project
    // root; serialize with every other test that touches it.
    let _project_root_guard = crate::rs_ctx::PROJECT_ROOT_TEST_LOCK.lock().unwrap();
    let mut project = RuntimeProject::new("Scene Collection Test", ".");
    project.static_scene_lookup = Some(static_scene_lookup);
    let mut runtime = Runtime::from_project(project, ProviderMode::Static);
    let parent_id = runtime.create::<Node2D>();
    let collection = node_collection![
        {
            name = "ship_instance",
            tags = tags!["spawned_scene"],
            scene = "res://scenes/ship.scn",
            children = [
                {
                    name = "scene_child",
                    node = Node2D::new(),
                    children = [
                        {
                            name = "nested_scene",
                            scene = "res://scenes/ship.scn",
                            children = [
                                { name = "nested_leaf", node = Node3D::new() },
                            ],
                        },
                        { name = "scene_leaf", node = Node3D::new() },
                    ],
                },
            ],
        },
        {
            name = "code_root",
            node = Node2D::new(),
            children = [
                { scene = "res://scenes/ship.scn" },
            ],
        },
    ];

    let ids = runtime.create_nodes(collection, parent_id);

    assert_eq!(ids.len(), 7);
    assert_eq!(
        runtime.get_node_children_ids(parent_id),
        Some(vec![ids[0], ids[5]])
    );
    assert_eq!(
        runtime.get_node_name(ids[0]).as_deref(),
        Some("ship_instance")
    );
    assert_eq!(runtime.get_node_type(ids[0]), Some(NodeType::Node3D));
    assert!(
        runtime
            .get_node_tags(ids[0])
            .is_some_and(|tags| tags.iter().any(|tag| tag.as_ref() == "spawned_scene"))
    );
    let ship_scene_child = child_with_name(&mut runtime, ids[0], "scene_builtin");
    assert_eq!(
        runtime.get_node_children_ids(ids[0]),
        Some(vec![ship_scene_child, ids[1]])
    );
    assert_eq!(
        runtime.get_node_name(ship_scene_child).as_deref(),
        Some("scene_builtin")
    );
    assert_eq!(runtime.get_node_parent_id(ids[1]), Some(ids[0]));
    assert_eq!(runtime.get_node_parent_id(ids[2]), Some(ids[1]));
    assert_eq!(
        runtime.get_node_name(ids[2]).as_deref(),
        Some("nested_scene")
    );
    assert_eq!(runtime.get_node_type(ids[2]), Some(NodeType::Node3D));
    assert_eq!(runtime.get_node_parent_id(ids[3]), Some(ids[2]));
    assert_eq!(runtime.get_node_parent_id(ids[4]), Some(ids[1]));
    let nested_scene_child = child_with_name(&mut runtime, ids[2], "scene_builtin");
    assert_eq!(
        runtime.get_node_children_ids(ids[2]),
        Some(vec![nested_scene_child, ids[3]])
    );
    assert_eq!(
        runtime.get_node_children_ids(ids[1]),
        Some(vec![ids[2], ids[4]])
    );
    assert_eq!(runtime.get_node_parent_id(ids[6]), Some(ids[5]));
    let code_scene_child = child_with_name(&mut runtime, ids[6], "scene_builtin");
    assert_eq!(runtime.get_node_type(ids[6]), Some(NodeType::Node3D));
    assert_eq!(
        runtime.get_node_children_ids(ids[6]),
        Some(vec![code_scene_child])
    );
}

#[test]
fn create_nodes_collection_root_marker_controls_splice_parent_refs() {
    let mut runtime = Runtime::new();
    let parent_id = runtime.create::<Node2D>();
    let child = node_collection![
        shell: { node = Node2D::new() },
        actual_root: {
            node = Node2D::new(),
            children = [
                leaf: { node = Node2D::new() },
            ],
        },
        root = @actual_root,
    ];
    let collection = node_collection![
        subroot: { collection = child },
        follower: {
            parent = @subroot,
            node = Node2D::new(),
        },
    ];

    let ids = runtime.create_nodes(collection, parent_id);

    assert_eq!(ids.len(), 4);
    assert_eq!(
        runtime.get_node_children_ids(parent_id),
        Some(vec![ids[0], ids[1]])
    );
    assert_eq!(runtime.get_node_children_ids(ids[0]), Some(Vec::new()));
    assert_eq!(
        runtime.get_node_children_ids(ids[1]),
        Some(vec![ids[2], ids[3]])
    );
    assert_eq!(
        runtime.get_node_name(ids[1]).as_deref(),
        Some("actual_root")
    );
    assert_eq!(runtime.get_node_parent_id(ids[3]), Some(ids[1]));
}

#[test]
fn create_nodes_applies_scene_patch_list_to_loaded_root() {
    // from_project boots the scene, which writes the process-global project
    // root; serialize with every other test that touches it.
    let _project_root_guard = crate::rs_ctx::PROJECT_ROOT_TEST_LOCK.lock().unwrap();
    let mut project = RuntimeProject::new("Scene Patch List Test", ".");
    project.static_scene_lookup = Some(static_scene_lookup);
    let mut runtime = Runtime::from_project(project, ProviderMode::Static);
    let parent_id = runtime.create::<Node2D>();
    let collection = node_collection![{
        scene = {
            path = "res://scenes/ship.scn",
            patch = [
                Node3D {
                    transform: Transform3D {
                        position: Vector3::new(3.0, 4.0, 5.0),
                        scale: Vector3::new(2.0, 2.0, 2.0),
                    },
                    visible: false,
                },
            ],
        },
    }];

    let ids = runtime.create_nodes(collection, parent_id);

    assert_eq!(ids.len(), 1);
    assert_eq!(
        runtime.with_node::<Node3D, _>(ids[0], |node| (node.transform, node.visible)),
        (
            Transform3D {
                position: Vector3::new(3.0, 4.0, 5.0),
                rotation: Quaternion::IDENTITY,
                scale: Vector3::new(2.0, 2.0, 2.0),
            },
            false,
        )
    );
}

#[test]
fn create_nodes_rejects_unresolved_script_node_ref_var() {
    let mut runtime = Runtime::new();
    let bad = NodeSpec::new(Node2D::new()).script(NodeScriptSpec {
        path: Cow::Borrowed("res://scripts/missing.rs"),
        vars: vec![(
            ScriptMemberID::from_string("target"),
            NodeScriptVar::NodeRef(99),
        )],
    });

    let ids = runtime.create_nodes(vec![bad], NodeID::nil());

    assert!(ids.is_empty());
}

#[test]
fn create_nodes_accepts_cross_domain_parenting() {
    let mut runtime = Runtime::new();
    let parent_id = runtime.create::<Node2D>();
    let collection = node_collection! {
        {
            name = "node_2d_root",
            node = Node2D::new(),
            children = [
                {
                    name = "node_3d_child",
                    node = Node3D::new(),
                    children = [
                        {
                            name = "ui_under_3d",
                            node = UiPanel::new(),
                            children = [
                                {
                                    name = "node_2d_under_ui",
                                    node = Node2D::new(),
                                    children = [
                                        {
                                            name = "node_3d_leaf",
                                            node = Node3D::new(),
                                        },
                                    ],
                                },
                            ],
                        },
                    ],
                },
                {
                    name = "ui_sibling",
                    node = UiLabel {
                        text: "Mixed".into(),
                        ..UiLabel::new()
                    },
                },
            ],
        }
    };

    let ids = runtime.create_nodes(collection, parent_id);

    assert_eq!(ids.len(), 6);
    assert_eq!(runtime.get_node_children_ids(parent_id), Some(vec![ids[0]]));
    assert_eq!(
        runtime.get_node_children_ids(ids[0]),
        Some(vec![ids[1], ids[5]])
    );
    assert_eq!(runtime.get_node_children_ids(ids[1]), Some(vec![ids[2]]));
    assert_eq!(runtime.get_node_children_ids(ids[2]), Some(vec![ids[3]]));
    assert_eq!(runtime.get_node_children_ids(ids[3]), Some(vec![ids[4]]));
    assert_eq!(runtime.get_node_parent_id(ids[4]), Some(ids[3]));
    assert_eq!(
        runtime.get_node_type(ids[0]),
        Some(perro_nodes::NodeType::Node2D)
    );
    assert_eq!(
        runtime.get_node_type(ids[1]),
        Some(perro_nodes::NodeType::Node3D)
    );
    assert_eq!(
        runtime.get_node_type(ids[2]),
        Some(perro_nodes::NodeType::UiPanel)
    );
    assert_eq!(
        runtime.get_node_type(ids[3]),
        Some(perro_nodes::NodeType::Node2D)
    );
    assert_eq!(
        runtime.get_node_type(ids[4]),
        Some(perro_nodes::NodeType::Node3D)
    );
    assert_eq!(
        runtime.get_node_type(ids[5]),
        Some(perro_nodes::NodeType::UiLabel)
    );
}

#[test]
fn create_nodes_accepts_owned_flat_specs() {
    let mut runtime = Runtime::new();
    let parent_id = runtime.create::<Node2D>();
    let specs = vec![
        NodeSpec::new(Node2D::new()).name("a"),
        NodeSpec::new(Node2D::new()).name("b"),
        NodeSpec::new(Node2D::new()).name("c"),
    ];

    let ids = runtime.create_nodes(specs, parent_id);

    assert_eq!(ids.len(), 3);
    assert_eq!(runtime.get_node_children_ids(parent_id), Some(ids.clone()));
    assert_eq!(runtime.get_node_name(ids[2]).as_deref(), Some("c"));
}

#[test]
fn create_nodes_rejects_invalid_collection_parent_indices() {
    let mut runtime = Runtime::new();
    let parent_id = runtime.create::<Node2D>();
    let bad_forward_parent = vec![
        NodeSpec::new(Node2D::new()).parent(Some(1)),
        NodeSpec::new(Node2D::new()),
    ];
    let bad_self_parent = vec![NodeSpec::new(Node2D::new()).parent(Some(0))];

    assert!(
        runtime
            .create_nodes(bad_forward_parent, parent_id)
            .is_empty()
    );
    assert!(runtime.create_nodes(bad_self_parent, parent_id).is_empty());
    assert_eq!(runtime.get_node_children_ids(parent_id), Some(Vec::new()));
    assert_eq!(runtime.nodes.len(), 1);
}

#[test]
fn create_nodes_handles_10k_children_and_transform_propagation() {
    let mut runtime = Runtime::new();
    let parent_id = runtime.create::<Node2D>();
    runtime
        .with_node_mut::<Node2D, _, _>(parent_id, |parent| {
            parent.transform.position = Vector2::new(12.0, 34.0);
        })
        .expect("parent exists");

    let templates = vec![NodeSpec::new(Node2D::new()); 10_000];
    let ids = runtime.create_nodes(&templates, parent_id);

    assert_eq!(ids.len(), 10_000);
    assert_eq!(runtime.nodes.len(), 10_001);
    assert_eq!(
        runtime
            .get_node_children_ids(parent_id)
            .map(|ids| ids.len()),
        Some(10_000)
    );

    runtime.propagate_pending_transform_dirty();
    runtime.refresh_dirty_global_transforms();

    let first_global = runtime
        .get_global_transform_2d(ids[0])
        .expect("first child global");
    let last_global = runtime
        .get_global_transform_2d(ids[9_999])
        .expect("last child global");
    assert_eq!(first_global.position, Vector2::new(12.0, 34.0));
    assert_eq!(last_global.position, Vector2::new(12.0, 34.0));
}

#[test]
fn skeleton_bone_lookup_helpers_return_name_and_index() {
    let mut runtime = Runtime::new();

    let mut skeleton = Skeleton3D::new();
    skeleton.bones = vec![
        Bone3D {
            name: "Root".into(),
            ..Bone3D::new()
        },
        Bone3D {
            name: "Spine".into(),
            ..Bone3D::new()
        },
    ];
    let skeleton_id = runtime
        .nodes
        .insert(SceneNode::new(SceneNodeData::Skeleton3D(skeleton)));
    let node_id = runtime
        .nodes
        .insert(SceneNode::new(SceneNodeData::Node3D(Node3D::new())));

    let name = runtime.get_skeleton_bone_name(skeleton_id, 1);
    assert_eq!(name.as_deref(), Some("Spine"));
    assert_eq!(
        runtime.get_skeleton_bone_index(skeleton_id, "Root"),
        Some(0)
    );
    assert_eq!(runtime.get_skeleton_bone_name(skeleton_id, 99), None);
    assert_eq!(
        runtime.get_skeleton_bone_index(skeleton_id, "Missing"),
        None
    );
    assert_eq!(runtime.get_skeleton_bone_name(node_id, 0), None);
}

#[test]
fn get_set_global_transform_3d_works_under_scaled_parent() {
    let mut runtime = Runtime::new();

    let mut parent = Node3D::new();
    parent.transform.position = Vector3::new(0.0, 1.0, 0.0);
    parent.transform.scale = Vector3::new(15.0, 15.0, 15.0);
    let parent_id = runtime
        .nodes
        .insert(SceneNode::new(SceneNodeData::Node3D(parent)));

    let child_id = runtime
        .nodes
        .insert(SceneNode::new(SceneNodeData::Node3D(Node3D::new())));

    if let Some(parent_node) = runtime.nodes.get_mut(parent_id) {
        parent_node.add_child(child_id);
    }
    if let Some(child_node) = runtime.nodes.get_mut(child_id) {
        child_node.parent = parent_id;
    }
    runtime.mark_transform_dirty_recursive(parent_id);

    let desired = Transform3D::new(
        Vector3::new(0.0, 0.0, 0.0),
        Quaternion::IDENTITY,
        Vector3::ONE,
    );
    assert!(runtime.set_global_transform_3d(child_id, desired));

    let child_global = runtime
        .get_global_transform_3d(child_id)
        .expect("child global must exist");
    assert!(approx(child_global.position.x, 0.0));
    assert!(approx(child_global.position.y, 0.0));
    assert!(approx(child_global.position.z, 0.0));

    let child_local = runtime
        .with_base_node::<Node3D, _, _>(child_id, |node| node.transform)
        .expect("child local must exist");
    assert!(approx(child_local.position.x, 0.0));
    assert!(approx(child_local.position.y, -1.0 / 15.0));
    assert!(approx(child_local.position.z, 0.0));
}

#[test]
fn bone_attachment_3d_follows_skeleton_bone_global_transform() {
    let mut runtime = Runtime::new();

    let mut skeleton = Skeleton3D::new();
    skeleton.transform.position = Vector3::new(10.0, 0.0, 0.0);
    skeleton.bones = vec![
        Bone3D {
            rest: Transform3D::new(
                Vector3::new(0.0, 2.0, 0.0),
                Quaternion::IDENTITY,
                Vector3::ONE,
            ),
            pose: Transform3D::new(
                Vector3::new(0.0, 2.0, 0.0),
                Quaternion::IDENTITY,
                Vector3::ONE,
            ),
            ..Bone3D::new()
        },
        Bone3D {
            parent: 0,
            rest: Transform3D::new(
                Vector3::new(0.0, 0.0, 3.0),
                Quaternion::IDENTITY,
                Vector3::ONE,
            ),
            pose: Transform3D::new(
                Vector3::new(0.0, 0.0, 3.0),
                Quaternion::IDENTITY,
                Vector3::ONE,
            ),
            ..Bone3D::new()
        },
    ];
    let skeleton_id = runtime
        .nodes
        .insert(SceneNode::new(SceneNodeData::Skeleton3D(skeleton)));
    runtime.register_internal_node_schedules(
        skeleton_id,
        runtime.nodes.get(skeleton_id).unwrap().node_type(),
    );

    let mut attachment = BoneAttachment3D::new();
    attachment.skeleton = skeleton_id;
    attachment.bone_index = 1;
    let attachment_id = runtime
        .nodes
        .insert(SceneNode::new(SceneNodeData::BoneAttachment3D(attachment)));
    runtime.register_internal_node_schedules(
        attachment_id,
        runtime.nodes.get(attachment_id).unwrap().node_type(),
    );
    runtime.mark_transform_dirty_recursive(skeleton_id);
    runtime.mark_transform_dirty_recursive(attachment_id);

    runtime.update(1.0 / 60.0);

    let global = runtime
        .get_global_transform_3d(attachment_id)
        .expect("attachment global must exist");
    assert!(approx(global.position.x, 10.0));
    assert!(approx(global.position.y, 2.0));
    assert!(approx(global.position.z, 3.0));
}

#[test]
fn bone_attachment_3d_child_follows_bone_global_transform() {
    let mut runtime = Runtime::new();

    let mut skeleton = Skeleton3D::new();
    skeleton.transform.position = Vector3::new(10.0, 0.0, 0.0);
    skeleton.bones = vec![Bone3D {
        rest: Transform3D::new(
            Vector3::new(0.0, 2.0, 0.0),
            Quaternion::IDENTITY,
            Vector3::ONE,
        ),
        pose: Transform3D::new(
            Vector3::new(0.0, 2.0, 0.0),
            Quaternion::IDENTITY,
            Vector3::ONE,
        ),
        ..Bone3D::new()
    }];
    let skeleton_id = runtime
        .nodes
        .insert(SceneNode::new(SceneNodeData::Skeleton3D(skeleton)));
    runtime.register_internal_node_schedules(
        skeleton_id,
        runtime.nodes.get(skeleton_id).unwrap().node_type(),
    );

    let mut attachment = BoneAttachment3D::new();
    attachment.skeleton = skeleton_id;
    attachment.bone_index = 0;
    let attachment_id = runtime
        .nodes
        .insert(SceneNode::new(SceneNodeData::BoneAttachment3D(attachment)));
    runtime.register_internal_node_schedules(
        attachment_id,
        runtime.nodes.get(attachment_id).unwrap().node_type(),
    );

    let mut child = Node3D::new();
    child.transform.position = Vector3::new(0.0, 0.0, 5.0);
    let child_id = runtime
        .nodes
        .insert(SceneNode::new(SceneNodeData::Node3D(child)));
    if let Some(attachment_node) = runtime.nodes.get_mut(attachment_id) {
        attachment_node.add_child(child_id);
    }
    if let Some(child_node) = runtime.nodes.get_mut(child_id) {
        child_node.parent = attachment_id;
    }
    runtime.mark_transform_dirty_recursive(skeleton_id);
    runtime.mark_transform_dirty_recursive(attachment_id);

    runtime.update(1.0 / 60.0);

    let child_global = runtime
        .get_global_transform_3d(child_id)
        .expect("child global must exist");
    assert!(approx(child_global.position.x, 10.0));
    assert!(approx(child_global.position.y, 2.0));
    assert!(approx(child_global.position.z, 5.0));

    let _ = runtime.with_base_node_mut::<Skeleton3D, _, _>(skeleton_id, |skeleton| {
        skeleton.bones[0].pose.position = Vector3::new(0.0, 4.0, 0.0);
    });
    runtime.update(1.0 / 60.0);

    let child_global = runtime
        .get_global_transform_3d(child_id)
        .expect("child global must exist after bone move");
    assert!(approx(child_global.position.x, 10.0));
    assert!(approx(child_global.position.y, 4.0));
    assert!(approx(child_global.position.z, 5.0));
}

#[test]
fn to_global_and_to_local_points_3d_roundtrip() {
    let mut runtime = Runtime::new();

    let mut parent = Node3D::new();
    parent.transform.position = Vector3::new(0.0, 1.0, 0.0);
    parent.transform.scale = Vector3::new(15.0, 15.0, 15.0);
    let parent_id = runtime
        .nodes
        .insert(SceneNode::new(SceneNodeData::Node3D(parent)));

    let mut child = Node3D::new();
    child.transform.position = Vector3::new(0.0, -1.0, 0.0);
    let child_id = runtime
        .nodes
        .insert(SceneNode::new(SceneNodeData::Node3D(child)));

    if let Some(parent_node) = runtime.nodes.get_mut(parent_id) {
        parent_node.add_child(child_id);
    }
    if let Some(child_node) = runtime.nodes.get_mut(child_id) {
        child_node.parent = parent_id;
    }
    runtime.mark_transform_dirty_recursive(parent_id);

    let world = runtime
        .to_global_point_3d(child_id, Vector3::ZERO)
        .expect("global point must exist");
    assert!(approx(world.x, 0.0));
    assert!(approx(world.y, -14.0));
    assert!(approx(world.z, 0.0));

    let local = runtime
        .to_local_point_3d(child_id, world)
        .expect("local point must exist");
    assert!(approx(local.x, 0.0));
    assert!(approx(local.y, 0.0));
    assert!(approx(local.z, 0.0));
}

#[test]
fn get_set_global_transform_2d_and_point_conversion() {
    let mut runtime = Runtime::new();

    let mut parent = Node2D::new();
    parent.transform.position = Vector2::new(10.0, 0.0);
    parent.transform.scale = Vector2::new(2.0, 2.0);
    let parent_id = runtime
        .nodes
        .insert(SceneNode::new(SceneNodeData::Node2D(parent)));

    let child_id = runtime
        .nodes
        .insert(SceneNode::new(SceneNodeData::Node2D(Node2D::new())));

    if let Some(parent_node) = runtime.nodes.get_mut(parent_id) {
        parent_node.add_child(child_id);
    }
    if let Some(child_node) = runtime.nodes.get_mut(child_id) {
        child_node.parent = parent_id;
    }
    runtime.mark_transform_dirty_recursive(parent_id);

    let desired = Transform2D::new(Vector2::new(16.0, 0.0), 0.0, Vector2::ONE);
    assert!(runtime.set_global_transform_2d(child_id, desired));

    let child_global = runtime
        .get_global_transform_2d(child_id)
        .expect("child global must exist");
    assert!(approx(child_global.position.x, 16.0));
    assert!(approx(child_global.position.y, 0.0));

    let world = runtime
        .to_global_point_2d(child_id, Vector2::new(1.0, 0.0))
        .expect("global point must exist");
    assert!(approx(world.x, 17.0));
    assert!(approx(world.y, 0.0));

    let local = runtime
        .to_local_point_2d(child_id, world)
        .expect("local point must exist");
    assert!(approx(local.x, 1.0));
    assert!(approx(local.y, 0.0));
}

#[test]
fn reparent_preserves_child_global_transform_3d() {
    let mut runtime = Runtime::new();

    let mut parent_a = Node3D::new();
    parent_a.transform.position = Vector3::new(10.0, 0.0, 0.0);
    let parent_a_id = runtime
        .nodes
        .insert(SceneNode::new(SceneNodeData::Node3D(parent_a)));

    let mut parent_b = Node3D::new();
    parent_b.transform.position = Vector3::new(-5.0, 0.0, 0.0);
    let parent_b_id = runtime
        .nodes
        .insert(SceneNode::new(SceneNodeData::Node3D(parent_b)));

    let mut child = Node3D::new();
    child.transform.position = Vector3::new(2.0, 0.0, 0.0);
    let child_id = runtime
        .nodes
        .insert(SceneNode::new(SceneNodeData::Node3D(child)));

    if let Some(parent) = runtime.nodes.get_mut(parent_a_id) {
        parent.add_child(child_id);
    }
    if let Some(child) = runtime.nodes.get_mut(child_id) {
        child.parent = parent_a_id;
    }
    runtime.mark_transform_dirty_recursive(parent_a_id);

    let before = runtime
        .get_global_transform_3d(child_id)
        .expect("child global before reparent must exist");
    assert!(runtime.reparent(parent_b_id, child_id));

    let after = runtime
        .get_global_transform_3d(child_id)
        .expect("child global after reparent must exist");
    assert!(approx(before.position.x, after.position.x));
    assert!(approx(before.position.y, after.position.y));
    assert!(approx(before.position.z, after.position.z));

    let local = runtime
        .with_base_node::<Node3D, _, _>(child_id, |node| node.transform)
        .expect("child local must exist");
    assert!(approx(local.position.x, 17.0));
}

#[test]
fn reparent_to_zero_scale_parent_uses_safe_inverse_3d() {
    let mut runtime = Runtime::new();

    let mut parent = Node3D::new();
    parent.transform.position = Vector3::new(3.0, 0.0, 0.0);
    parent.transform.scale = Vector3::ZERO;
    let parent_id = runtime
        .nodes
        .insert(SceneNode::new(SceneNodeData::Node3D(parent)));

    let mut child = Node3D::new();
    child.transform.position = Vector3::new(8.0, 2.0, -4.0);
    let child_id = runtime
        .nodes
        .insert(SceneNode::new(SceneNodeData::Node3D(child)));

    assert!(runtime.reparent(parent_id, child_id));

    let local = runtime
        .with_base_node::<Node3D, _, _>(child_id, |node| node.transform)
        .expect("child local must exist");
    assert!(local.position.x.is_finite());
    assert!(local.position.y.is_finite());
    assert!(local.position.z.is_finite());
    assert_eq!(local.scale, Vector3::ONE);
}

#[test]
fn set_global_transform_under_zero_scale_parent_uses_safe_inverse_3d() {
    let mut runtime = Runtime::new();

    let mut parent = Node3D::new();
    parent.transform.position = Vector3::new(3.0, 0.0, 0.0);
    parent.transform.scale = Vector3::ZERO;
    let parent_id = runtime
        .nodes
        .insert(SceneNode::new(SceneNodeData::Node3D(parent)));

    let child_id = runtime
        .nodes
        .insert(SceneNode::new(SceneNodeData::Node3D(Node3D::new())));

    if let Some(parent) = runtime.nodes.get_mut(parent_id) {
        parent.add_child(child_id);
    }
    if let Some(child) = runtime.nodes.get_mut(child_id) {
        child.parent = parent_id;
    }
    runtime.mark_transform_dirty_recursive(parent_id);

    assert!(runtime.set_global_transform_3d(
        child_id,
        Transform3D::new(
            Vector3::new(8.0, 2.0, -4.0),
            Quaternion::IDENTITY,
            Vector3::ONE,
        ),
    ));

    let local = runtime
        .with_base_node::<Node3D, _, _>(child_id, |node| node.transform)
        .expect("child local must exist");
    assert!(local.position.x.is_finite());
    assert!(local.position.y.is_finite());
    assert!(local.position.z.is_finite());
    assert_eq!(local.scale, Vector3::ONE);
}

#[test]
fn remove_node_removes_entire_subtree() {
    let mut runtime = Runtime::new();

    let root_id = runtime
        .nodes
        .insert(SceneNode::new(SceneNodeData::Node3D(Node3D::new())));
    let child_id = runtime
        .nodes
        .insert(SceneNode::new(SceneNodeData::Node3D(Node3D::new())));
    let grandchild_id = runtime
        .nodes
        .insert(SceneNode::new(SceneNodeData::Node3D(Node3D::new())));

    if let Some(root) = runtime.nodes.get_mut(root_id) {
        root.add_child(child_id);
    }
    if let Some(child) = runtime.nodes.get_mut(child_id) {
        child.parent = root_id;
        child.add_child(grandchild_id);
    }
    if let Some(grandchild) = runtime.nodes.get_mut(grandchild_id) {
        grandchild.parent = child_id;
    }

    assert!(runtime.remove_node(root_id));
    assert!(runtime.nodes.get(root_id).is_none());
    assert!(runtime.nodes.get(child_id).is_none());
    assert!(runtime.nodes.get(grandchild_id).is_none());
    assert!(!runtime.remove_node(root_id));
}

#[test]
fn remove_node_unlinks_root_from_live_parent() {
    // Removing a subtree must unlink its root from a live parent outside the
    // subtree, while descendants disappear entirely.
    let mut runtime = Runtime::new();

    let live_parent = runtime
        .nodes
        .insert(SceneNode::new(SceneNodeData::Node3D(Node3D::new())));
    let sibling = runtime
        .nodes
        .insert(SceneNode::new(SceneNodeData::Node3D(Node3D::new())));
    let root_id = runtime
        .nodes
        .insert(SceneNode::new(SceneNodeData::Node3D(Node3D::new())));
    let child_id = runtime
        .nodes
        .insert(SceneNode::new(SceneNodeData::Node3D(Node3D::new())));

    if let Some(parent) = runtime.nodes.get_mut(live_parent) {
        parent.add_child(sibling);
        parent.add_child(root_id);
    }
    if let Some(node) = runtime.nodes.get_mut(sibling) {
        node.parent = live_parent;
    }
    if let Some(node) = runtime.nodes.get_mut(root_id) {
        node.parent = live_parent;
        node.add_child(child_id);
    }
    if let Some(node) = runtime.nodes.get_mut(child_id) {
        node.parent = root_id;
    }

    assert!(runtime.remove_node(root_id));
    assert!(runtime.nodes.get(root_id).is_none());
    assert!(runtime.nodes.get(child_id).is_none());
    // The live parent stays, and its child list no longer references the removed
    // root but still holds the untouched sibling.
    let children = runtime
        .get_node_children_ids(live_parent)
        .expect("live parent still exists");
    assert_eq!(children, vec![sibling]);
}

#[test]
fn remove_node_unlinks_from_parent_outside_subtree() {
    // Edge case: a node inside the traversed subtree whose `parent` field points
    // to a node OUTSIDE the subtree must still be unlinked from that live parent.
    // Traversal follows children lists, so `inner` is reached via `root_id`, but
    // its parent pointer targets `outside_parent` (a stale/reparent artifact).
    let mut runtime = Runtime::new();

    let outside_parent = runtime
        .nodes
        .insert(SceneNode::new(SceneNodeData::Node3D(Node3D::new())));
    let root_id = runtime
        .nodes
        .insert(SceneNode::new(SceneNodeData::Node3D(Node3D::new())));
    let inner = runtime
        .nodes
        .insert(SceneNode::new(SceneNodeData::Node3D(Node3D::new())));

    // `root_id` lists `inner` as a child (drives traversal into the subtree)...
    if let Some(node) = runtime.nodes.get_mut(root_id) {
        node.add_child(inner);
    }
    // ...but `inner.parent` points at the live outside node, which also lists it.
    if let Some(node) = runtime.nodes.get_mut(inner) {
        node.parent = outside_parent;
    }
    if let Some(node) = runtime.nodes.get_mut(outside_parent) {
        node.add_child(inner);
    }

    assert!(runtime.remove_node(root_id));
    assert!(runtime.nodes.get(root_id).is_none());
    assert!(runtime.nodes.get(inner).is_none());
    // The outside parent survives and its stale child link is scrubbed.
    let children = runtime
        .get_node_children_ids(outside_parent)
        .expect("outside parent still exists");
    assert!(
        children.is_empty(),
        "outside parent must be unlinked from removed node, got {children:?}"
    );
}

#[test]
fn create_nodes_borrowed_invalid_batch_creates_nothing() {
    // Borrowed-slice path: an invalid forward parent reference must create zero
    // nodes (validation runs before any clone or insert).
    let mut runtime = Runtime::new();
    let parent_id = runtime.create::<Node2D>();
    let bad_forward_parent = [
        NodeSpec::new(Node2D::new()).parent(Some(1)),
        NodeSpec::new(Node2D::new()),
    ];

    let ids = runtime.create_nodes(&bad_forward_parent, parent_id);
    assert!(ids.is_empty());
    assert_eq!(runtime.get_node_children_ids(parent_id), Some(Vec::new()));
    assert_eq!(runtime.nodes.len(), 1);
}

#[test]
fn create_nodes_borrowed_valid_batch_builds_tree() {
    // Borrowed-slice path: a valid batch builds the expected parent/child tree.
    let mut runtime = Runtime::new();
    let parent_id = runtime.create::<Node2D>();
    let specs = [
        NodeSpec::new(Node2D::new()).name("root"),
        NodeSpec::new(Node2D::new()).name("leaf").parent(Some(0)),
    ];

    let ids = runtime.create_nodes(&specs, parent_id);
    assert_eq!(ids.len(), 2);
    // First spec attaches under `parent_id`, second nests under the first.
    assert_eq!(runtime.get_node_parent_id(ids[0]), Some(parent_id));
    assert_eq!(runtime.get_node_parent_id(ids[1]), Some(ids[0]));
    assert_eq!(runtime.get_node_children_ids(parent_id), Some(vec![ids[0]]));
    assert_eq!(runtime.get_node_children_ids(ids[0]), Some(vec![ids[1]]));
}
