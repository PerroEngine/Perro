use leptos::prelude::*;
use perro_nodes::{InternalFixedUpdate, InternalUpdate, NodeType, Renderable};
use perro_scene::scene_node_fields;
use std::sync::LazyLock;

use crate::layout::PageFrame;
use crate::shared::{Seo, SeoInfo};

#[derive(Clone, Copy, PartialEq, Eq)]
enum NodeFamily {
    Base,
    TwoD,
    ThreeD,
    Ui,
    Resource,
}

impl NodeFamily {
    const fn label(self) -> &'static str {
        match self {
            Self::Base => "Base",
            Self::TwoD => "2D",
            Self::ThreeD => "3D",
            Self::Ui => "UI",
            Self::Resource => "Resource",
        }
    }

    const fn class(self) -> &'static str {
        match self {
            Self::Base => "base",
            Self::TwoD => "two-d",
            Self::ThreeD => "three-d",
            Self::Ui => "ui",
            Self::Resource => "resource",
        }
    }
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum NodeRole {
    Core,
    Camera,
    Visual,
    Light,
    Skeletal,
    Physics,
    Audio,
    Layout,
    Animation,
}

impl NodeRole {
    const fn label(self) -> &'static str {
        match self {
            Self::Core => "Core",
            Self::Camera => "Camera",
            Self::Visual => "Visual",
            Self::Light => "Light",
            Self::Skeletal => "Skeletal",
            Self::Physics => "Physics",
            Self::Audio => "Audio",
            Self::Layout => "Layout",
            Self::Animation => "Animation",
        }
    }
}

struct NodeInfo {
    name: &'static str,
    family: NodeFamily,
    role: NodeRole,
    parent: Option<&'static str>,
    renderable: bool,
    update: bool,
    fixed_update: bool,
    fields: usize,
    search: String,
}

static NODES: LazyLock<Vec<NodeInfo>> = LazyLock::new(|| {
    NodeType::ALL
        .iter()
        .copied()
        .map(|node_type| {
            let name = node_type.name();
            let family = node_family(node_type);
            let role = node_role(node_type);
            let parent = node_type.parent_type().map(|parent| parent.name());
            let fields = scene_node_fields(node_type).len();
            let renderable = node_renderable(node_type);
            let update = node_update(node_type);
            let fixed_update = node_fixed_update(node_type);
            let search = format!(
                "{} {} {} {} {}",
                name,
                family.label(),
                role.label(),
                parent.unwrap_or(""),
                if renderable { "renderable" } else { "" },
            )
            .to_ascii_lowercase();
            NodeInfo {
                name,
                family,
                role,
                parent,
                renderable,
                update,
                fixed_update,
                fields,
                search,
            }
        })
        .collect()
});

#[component]
pub fn NodesPage() -> impl IntoView {
    let total = NODES.len();

    view! {
        <Seo info=SeoInfo::new(
            "Scene Node Registry",
            "Search Perro scene nodes by 2D, 3D, UI, resource, physics, audio, animation, camera, light, visual role, parent type, fields, and update flags.",
            &node_keywords(),
            "/nodes",
        ).with_schema(node_schema()) />
        <PageFrame eyebrow="Nodes" title="Node registry">
            <section class="node-browser">
                <div class="node-tools">
                    <input
                        class="search"
                        id="node-search"
                        type="search"
                        placeholder="Search nodes"
                    />
                    <select class="node-select" id="node-family">
                        <option value="all">"All groups"</option>
                        <option value="2D">"2D"</option>
                        <option value="3D">"3D"</option>
                        <option value="UI">"UI"</option>
                        <option value="Resource">"Resource"</option>
                        <option value="Base">"Base"</option>
                    </select>
                    <select class="node-select" id="node-role">
                        <option value="all">"All roles"</option>
                        <option value="Core">"Core"</option>
                        <option value="Camera">"Camera"</option>
                        <option value="Visual">"Visual"</option>
                        <option value="Light">"Light"</option>
                        <option value="Skeletal">"Skeletal"</option>
                        <option value="Physics">"Physics"</option>
                        <option value="Audio">"Audio"</option>
                        <option value="Layout">"Layout"</option>
                        <option value="Animation">"Animation"</option>
                    </select>
                    <span class="node-count">{total}" total"</span>
                </div>
                <div class="node-trees">
                    <NodeTreeGroup title="2D" class="two-d" root="Node2D" />
                    <NodeTreeGroup title="3D" class="three-d" root="Node3D" />
                    <NodeTreeGroup title="UI" class="ui" root="UiNode" />
                    <NodeTreeGroup title="Resource" class="resource" root="Resource" />
                    <NodeTreeGroup title="Base" class="base" root="Node" />
                </div>
                <script>
                    {r#"
const nodeSearch = document.getElementById("node-search");
const nodeFamily = document.getElementById("node-family");
const nodeRole = document.getElementById("node-role");
const nodeGroups = Array.from(document.querySelectorAll(".node-tree-group"));
const nodeBranches = Array.from(document.querySelectorAll(".node-branch"));
const nodeRows = Array.from(document.querySelectorAll(".node-row"));
function filterNodes() {
  const q = (nodeSearch.value || "").trim().toLowerCase();
  const family = nodeFamily.value;
  const role = nodeRole.value;
  for (const row of nodeRows) {
    const okSearch = !q || row.dataset.search.includes(q);
    const okFamily = family === "all" || row.dataset.family === family;
    const okRole = role === "all" || row.dataset.role === role;
    row.dataset.match = okSearch && okFamily && okRole ? "1" : "0";
  }
  for (const branch of [...nodeBranches].reverse()) {
    const row = branch.querySelector(":scope > .node-row");
    const childMatch = Array.from(branch.querySelectorAll(":scope > .node-children > .node-branch")).some(child => child.dataset.visible === "1");
    const visible = row.dataset.match === "1" || childMatch;
    branch.dataset.visible = visible ? "1" : "0";
    branch.hidden = !visible;
    row.classList.toggle("ancestor-match", row.dataset.match !== "1" && childMatch);
  }
  for (const group of nodeGroups) {
    const visible = Array.from(group.querySelectorAll(".node-row")).filter(row => row.dataset.match === "1").length;
    group.hidden = visible === 0;
    const count = group.querySelector("[data-node-count]");
    if (count) count.textContent = `${visible} nodes`;
  }
}
nodeSearch.addEventListener("input", filterNodes);
nodeFamily.addEventListener("input", filterNodes);
nodeRole.addEventListener("input", filterNodes);
filterNodes();
"#}
                </script>
            </section>
        </PageFrame>
    }
}

fn node_keywords() -> String {
    let names = NODES
        .iter()
        .map(|node| node.name)
        .take(80)
        .collect::<Vec<_>>()
        .join(", ");
    format!(
        "Perro nodes, Perro scene nodes, node registry, 2D nodes, 3D nodes, UI nodes, physics nodes, audio nodes, animation nodes, camera nodes, light nodes, {names}"
    )
}

fn node_schema() -> String {
    let items = NODES
        .iter()
        .take(40)
        .enumerate()
        .map(|(index, node)| {
            format!(
                r#"{{
    "@type": "ListItem",
    "position": {},
    "name": {},
    "description": {}
  }}"#,
                index + 1,
                json_string(node.name),
                json_string(&format!(
                    "{} {} node, parent {}, {} fields",
                    node.family.label(),
                    node.role.label(),
                    node.parent.unwrap_or("none"),
                    node.fields
                ))
            )
        })
        .collect::<Vec<_>>()
        .join(",\n  ");
    format!(
        r#"{{
  "@context": "https://schema.org",
  "@type": "ItemList",
  "name": "Perro scene nodes",
  "itemListElement": [
  {items}
  ]
}}"#
    )
}

fn json_string(text: &str) -> String {
    serde_json::to_string(text).unwrap_or_else(|_| "\"Perro node\"".to_string())
}

#[component]
fn NodeTreeGroup(title: &'static str, class: &'static str, root: &'static str) -> impl IntoView {
    let count = NODES
        .iter()
        .filter(|node| node.family.label() == title)
        .count();
    let roots = if root == "Resource" {
        NODES
            .iter()
            .filter(|node| node.family == NodeFamily::Resource && node.parent.is_none())
            .map(|node| node.name)
            .collect::<Vec<_>>()
    } else {
        vec![root]
    };

    view! {
        <section class=format!("node-tree-group {class}")>
            <div class="node-group-head">
                <h2>{title}</h2>
                <span data-node-count>{count}" nodes"</span>
            </div>
            <div class="node-tree">
                {roots.into_iter().map(|name| view! {
                    <NodeBranch name include_children=root != "Node" />
                }).collect_view()}
            </div>
        </section>
    }
}

#[component]
fn NodeBranch(name: &'static str, include_children: bool) -> impl IntoView {
    let node = NODES.iter().find(|node| node.name == name);
    let children = if include_children {
        NODES
            .iter()
            .filter(|node| node.parent == Some(name))
            .map(|node| node.name)
            .collect::<Vec<_>>()
    } else {
        Vec::new()
    };

    match node {
        Some(node) => view! {
            <div class=format!("node-branch {}", node.family.class())>
                <div
                    class=format!("node-row {}", node.family.class())
                    data-search=node.search.as_str()
                    data-family=node.family.label()
                    data-role=node.role.label()
                >
                    <div class="node-row-main">
                        <strong>{node.name}</strong>
                        <span>{node.role.label()}</span>
                    </div>
                    <div class="node-meta">
                        <span>"parent: "{node.parent.unwrap_or("none")}</span>
                        <span>{node.fields}" fields"</span>
                    </div>
                    <div class="node-flags">
                        <span class:off=!node.renderable>"render"</span>
                        <span class:off=!node.update>"upd"</span>
                        <span class:off=!node.fixed_update>"fixed"</span>
                    </div>
                </div>
                <div class="node-children">
                    {children.into_iter().map(|name| view! {
                        <NodeBranch name include_children=true />
                    }).collect_view()}
                </div>
            </div>
        }
        .into_any(),
        None => ().into_any(),
    }
}

const fn node_family(node_type: NodeType) -> NodeFamily {
    if node_type.is_2d() {
        NodeFamily::TwoD
    } else if node_type.is_3d() {
        NodeFamily::ThreeD
    } else if node_type.is_a(NodeType::UiNode) {
        NodeFamily::Ui
    } else {
        match node_type {
            NodeType::Node => NodeFamily::Base,
            _ => NodeFamily::Resource,
        }
    }
}

const fn node_role(node_type: NodeType) -> NodeRole {
    match node_type {
        NodeType::Node | NodeType::Node2D | NodeType::Node3D | NodeType::UiNode => NodeRole::Core,
        NodeType::Camera2D
        | NodeType::Camera3D
        | NodeType::CameraStream2D
        | NodeType::CameraStream3D
        | NodeType::UiCameraStream => NodeRole::Camera,
        NodeType::AmbientLight2D
        | NodeType::RayLight2D
        | NodeType::PointLight2D
        | NodeType::SpotLight2D
        | NodeType::AmbientLight3D
        | NodeType::RayLight3D
        | NodeType::PointLight3D
        | NodeType::SpotLight3D => NodeRole::Light,
        NodeType::Skeleton2D
        | NodeType::BoneAttachment2D
        | NodeType::IKTarget2D
        | NodeType::PhysicsBoneChain2D
        | NodeType::BoneCollider2D
        | NodeType::Skeleton3D
        | NodeType::BoneAttachment3D
        | NodeType::IKTarget3D
        | NodeType::PhysicsBoneChain3D
        | NodeType::BoneCollider3D => NodeRole::Skeletal,
        NodeType::CollisionShape2D
        | NodeType::StaticBody2D
        | NodeType::Area2D
        | NodeType::RigidBody2D
        | NodeType::CharacterBody2D
        | NodeType::PhysicsForceEmitter2D
        | NodeType::PinJoint2D
        | NodeType::DistanceJoint2D
        | NodeType::FixedJoint2D
        | NodeType::CollisionShape3D
        | NodeType::StaticBody3D
        | NodeType::Area3D
        | NodeType::RigidBody3D
        | NodeType::CharacterBody3D
        | NodeType::PhysicsForceEmitter3D
        | NodeType::BallJoint3D
        | NodeType::HingeJoint3D
        | NodeType::FixedJoint3D => NodeRole::Physics,
        NodeType::AudioMask2D
        | NodeType::AudioEffectZone2D
        | NodeType::AudioPortal2D
        | NodeType::AudioMask3D
        | NodeType::AudioEffectZone3D
        | NodeType::AudioPortal3D => NodeRole::Audio,
        NodeType::UiScrollContainer
        | NodeType::UiLayout
        | NodeType::UiHLayout
        | NodeType::UiVLayout
        | NodeType::UiGrid
        | NodeType::UiTreeList => NodeRole::Layout,
        NodeType::AnimationPlayer | NodeType::AnimationTree => NodeRole::Animation,
        _ => NodeRole::Visual,
    }
}

const fn node_renderable(node_type: NodeType) -> bool {
    matches!(node_type.get_renderable(), Renderable::True)
}

const fn node_update(node_type: NodeType) -> bool {
    matches!(node_type.get_internal_update(), InternalUpdate::True)
}

const fn node_fixed_update(node_type: NodeType) -> bool {
    matches!(
        node_type.get_internal_fixed_update(),
        InternalFixedUpdate::True
    )
}
