# Scenes + Nodes

Scenes are trees of nodes.

Nodes own transform, render data, physics data, UI data, or resource refs.

## Goal

Know where behavior lives and how nodes relate.

## Ownership Model

The scene owns topology: which nodes exist, which node contains another, and
which fixed refs connect authored objects. A script owns behavior for its
attached node. Runtime queries discover sets that scene authoring cannot know in
advance.

Use a fixed `NodeID` ref when changing a node name should not break behavior.
Use a relation when topology itself expresses ownership. Use a query when the
membership changes. A name lookup is useful for tools and one-off discovery,
but it hides a dependency and repeats search work in gameplay code.

## Scene Tree

A scene is a root node plus children.

Common roots:

- `Node`
- `Node2D`
- `Node3D`
- `UiNode`

Common children:

- sprite or mesh nodes
- camera nodes
- light nodes
- physics nodes
- UI nodes
- script-bearing gameplay nodes

## Node Identity

Runtime code uses `NodeID`.

Names and tags are lookup helpers.

Use `NodeID` for stored refs when possible.

Do not rely on names for hot update paths.

## Typed Access

Use typed node macros when changing node data:

```rust
let _ = with_base_node_mut!(ctx.run, Node2D, ctx.id, |node| {
    node.transform.position.x += 12.0;
});
```

Use query APIs when finding other nodes:

```rust
query_each!(ctx.run, all(tags["enemy"], tags["alive"]), |id| {
    call_method!(ctx.run, id, method!("wake"), params![]);
});
```

## 2D Nodes

Use 2D nodes for:

- sprites
- 2D cameras
- 2D physics
- 2D lights
- tile maps
- 2D water
- world-space buttons

## 3D Nodes

Use 3D nodes for:

- meshes
- 3D cameras
- 3D lights
- particles
- skeletons
- physics bodies
- water

## UI Nodes

Use UI nodes for:

- panels
- labels
- buttons
- images
- scroll containers
- layout nodes
- text input

## Reference

- [Node Types](/docs/scripting/nodes.md)
- [Scene Node Templates](/docs/scripting/scene_node_templates/index.md)
- [Runtime Nodes Module](/docs/scripting/contexts/runtime_modules/nodes.md)
- [Node Query Module](/docs/scripting/contexts/runtime_modules/node_query.md)
- [Node Collections](/docs/scripting/node_collections.md)
