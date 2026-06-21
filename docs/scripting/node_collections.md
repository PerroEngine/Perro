# Node Collections

## Purpose

`NodeCollection` is an in-code scene tree.

Use it to build live runtime nodes from Rust data.

Use `.scn` files for asset/editor scenes.

Use `node_collection!` for script-built prefabs, UI chunks, spawn packs, debug trees, and generated scene parts.

## Shape

Each entry uses plain fields:

```rust
{
    name = "node_name",
    tags = tags!["tag_a", "tag_b"],
    node = Node2D::new(),
    children = [
        { name = "child", node = Node2D::new() },
    ],
}
```

Fields:

- `node = ...` required.
- `scene = ...` loads a `.scn` scene.
- `name = ...` optional.
- `tags = ...` optional.
- `script = ...` optional single script resource.
- `children = [...]` optional.
- `collection = ...` splices another collection.

`node = ...` is real Rust node data.

Use a type name for default node data:

```rust
{ name = "button", node = UiButton }
```

Use a type body to set only the fields you care about:

```rust
{
    name = "title",
    node = UiLabel {
        text: {"Paused".into()},
        font_size: 32.0,
    },
}
```

Missing fields use `Default::default()`.

Nested type bodies work the same way:

```rust
{
    name = "actor",
    node = Node2D {
        transform: Transform2D {
            position: Vector2::new(5.0, 7.0),
        },
    },
}
```

Use `{ expr }` for arbitrary Rust expressions.

Use `node = { expr }` to keep the old escape hatch for full custom values.

`scene = ...` uses any scene path value accepted by the scene API.

`name` and `tags` are scene graph metadata.

Use scene object form to patch the loaded scene root:

```rust
{
    name = "player",
    scene = {
        path = res_path!("res://scenes/player.scn"),
        patch = Node2D {
            transform: Transform2D {
                position: Vector2::new(10.0, 0.0),
            },
        },
    },
    script = res_path!("res://scripts/player.rs"),
}
```

`patch` applies only if the loaded scene root type matches.

Use patch lists when a root needs more than one typed patch:

```rust
scene = {
    path = res_path!("res://scenes/player.scn"),
    patch = [
        Node2D {
            transform: spawn_xform,
        },
    ],
}
```

Use script object form to inject vars before script init:

```rust
{
    node = Node2D,
    script = {
        path = res_path!("res://scripts/player.rs"),
        vars = {
            hp: 100_i32,
            title: {"Player".to_string()},
        },
    },
}
```

`script` still means one script per node.

Use `@key` in script vars to pass a spawned node id:

```rust
node_collection![
    player: { node = Node2D },
    camera: {
        node = Camera2D,
        script = {
            path = res_path!("res://scripts/follow.rs"),
            vars = { target: @player },
        },
    },
]
```

`@key` vars resolve during spawn.

Keys are local macro labels.

Names are runtime strings.

## Create

Use `create_nodes!`.

```rust
let nodes = node_collection! {
    {
        name = "root",
        node = Node2D::new(),
        children = [
            { name = "sprite", node = Sprite2D::new() },
        ],
    }
};

let ids = create_nodes!(ctx.run, nodes, ctx.id);
let root = ids[0];
```

Result:

```text
ctx.id
  root
    sprite
```

IDs return in preorder.

Top-level collection nodes become children of the parent passed to `create_nodes!`.

If parent is `NodeID::nil()`, top-level nodes stay roots.

## Flat Batch

Use array form when no nesting is needed.

```rust
let wave = node_collection![
    { name = "enemy_a", tags = tags!["enemy"], node = Node2D::new() },
    { name = "enemy_b", tags = tags!["enemy"], node = Node2D::new() },
    { name = "enemy_c", tags = tags!["enemy"], node = Node2D::new() },
];

let ids = create_nodes!(ctx.run, wave, ctx.id);
```

Result:

```text
ctx.id
  enemy_a
  enemy_b
  enemy_c
```

Use keyed entries when a flat list needs parent refs.

```rust
let actor = node_collection![
    root: { node = Node2D },
    sprite: { parent = @root, node = Sprite2D },
    camera: { parent = @root, node = Camera2D },
];
```

Keys are compile-time macro refs.

If `name` is omitted, keyed entries use the key text as node name.

Use `name = ...` to override it.

`parent = @key` is only for flat entries.

Inside `children = [...]`, parent is implicit.

Keys inside children are allowed as name shorthand, but cannot be referenced.

Use `root = @key` when a collection splice should return a non-first root:

```rust
let actor = node_collection![
    shell: { node = Node2D },
    body: { node = Node2D },
    root = @body,
];
```

## Tree

Use object form for one tree.

```rust
let actor = node_collection! {
    {
        name = "actor",
        tags = tags!["player"],
        node = Node2D::new(),
        children = [
            { name = "sprite", node = Sprite2D::new() },
            { name = "camera", node = Camera2D::new() },
        ],
    }
};

let ids = create_nodes!(ctx.run, actor, ctx.id);
```

Result:

```text
ctx.id
  actor
    sprite
    camera
```

## Multiple Trees

Use array form with nested entries.

```rust
let pack = node_collection![
    {
        name = "hud",
        node = UiPanel::new(),
        children = [
            { name = "score", node = UiLabel::new() },
        ],
    },
    {
        name = "actor",
        node = Node2D::new(),
        children = [
            { name = "sprite", node = Sprite2D::new() },
        ],
    },
    { name = "camera_anchor", node = Node2D::new() },
];

let ids = create_nodes!(ctx.run, pack, ctx.id);
```

Result:

```text
ctx.id
  hud
    score
  actor
    sprite
  camera_anchor
```

## Child Collections

Use `collection = expr` to splice reusable parts.

```rust
fn toolbar() -> NodeCollection {
    node_collection![
        { name = "inventory", node = UiButton::new() },
        { name = "map", node = UiButton::new() },
    ]
}

fn hud() -> NodeCollection {
    node_collection! {
        {
            name = "hud",
            node = UiPanel::new(),
            children = [
                { collection = toolbar() },
                {
                    name = "hp",
                    node = UiLabel {
                        text: "HP".into(),
                        ..UiLabel::new()
                    },
                },
            ],
        }
    }
}

let ids = create_nodes!(ctx.run, hud(), ctx.id);
```

Result:

```text
ctx.id
  hud
    inventory
    map
    hp
```

## Top-Level Collections

Collections can be spliced at the top level too.

```rust
let scene_bits = node_collection![
    { collection = hud() },
    { collection = actor_debug() },
    { name = "marker", node = Node2D::new() },
];

let ids = create_nodes!(ctx.run, scene_bits, ctx.id);
```

Result:

```text
ctx.id
  hud
    ...
  actor
    ...
  marker
```

## Scene Refs

Use `scene = ...` to splice a `.scn` scene into a collection.

```rust
let pack = node_collection![
    {
        name = "player",
        tags = tags!["actor"],
        scene = res_path!("res://scenes/player.scn"),
    },
    {
        name = "hud",
        scene = "res://ui/hud.scn",
    },
];

let ids = create_nodes!(ctx.run, pack, ctx.id);
```

Result:

```text
ctx.id
  player
    ...scene nodes
  hud
    ...scene nodes
```

## Scene Children

Scenes can have code children.

```rust
let actor = node_collection! {
    {
        name = "ship",
        scene = res_path!("res://scenes/ship.scn"),
        children = [
            { name = "debug_anchor", node = Node3D::new() },
            {
                name = "nameplate",
                node = UiLabel {
                    text: "Ship".into(),
                    ..UiLabel::new()
                },
            },
        ],
    }
};

let ids = create_nodes!(ctx.run, actor, ctx.id);
```

Result:

```text
ctx.id
  ship
    ...scene nodes
    debug_anchor
    nameplate
```

## Scene And Collections

Scenes can contain collection children.

Collections can contain scene children.

```rust
fn loadout_ui() -> NodeCollection {
    node_collection![
        { name = "weapon_slot", node = UiButton::new() },
        { name = "item_slot", node = UiButton::new() },
    ]
}

let squad = node_collection![
    {
        name = "leader",
        scene = res_path!("res://scenes/player.scn"),
        children = [
            { collection = loadout_ui() },
        ],
    },
    {
        name = "followers",
        node = Node2D::new(),
        children = [
            { scene = res_path!("res://scenes/follower.scn") },
            { scene = res_path!("res://scenes/follower.scn") },
        ],
    },
];

let ids = create_nodes!(ctx.run, squad, ctx.id);
```

Result:

```text
ctx.id
  leader
    ...player scene nodes
    weapon_slot
    item_slot
  followers
    ...follower scene nodes
    ...follower scene nodes
```

## Multi Scene Tree

Use many scene refs, nested scenes, and code children in one collection.

```rust
let multi = node_collection![
    {
        name = "ship_a",
        tags = tags!["ship", "player"],
        scene = res_path!("res://scenes/ship.scn"),
        children = [
            {
                name = "ship_a_debug",
                node = Node3D::new(),
                children = [
                    {
                        name = "ship_a_nested_hud",
                        scene = res_path!("res://ui/ship_hud.scn"),
                        children = [
                            { name = "fps_label", node = UiLabel::new() },
                        ],
                    },
                ],
            },
        ],
    },
    {
        name = "squad_root",
        node = Node2D::new(),
        children = [
            {
                name = "ship_b",
                scene = res_path!("res://scenes/ship.scn"),
            },
            {
                name = "ship_c",
                scene = res_path!("res://scenes/ship.scn"),
                children = [
                    { name = "marker", node = Sprite2D::new() },
                ],
            },
        ],
    },
];

let ids = create_nodes!(ctx.run, multi, ctx.id);
```

ID order:

```text
ids[0] ship_a
ids[1] ship_a_debug
ids[2] ship_a_nested_hud
ids[3] fps_label
ids[4] squad_root
ids[5] ship_b
ids[6] ship_c
ids[7] marker
```

Live tree:

```text
ctx.id
  ship_a
    ...ship.scn nodes
    ship_a_debug
      ship_a_nested_hud
        ...ship_hud.scn nodes
        fps_label
  squad_root
    ship_b
      ...ship.scn nodes
    ship_c
      ...ship.scn nodes
      marker
```

## Cross Domain

2D, 3D, and UI nodes can live in one collection.

Parenting is graph-level.

```rust
let mixed = node_collection! {
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
                            { name = "node_2d_under_ui", node = Node2D::new() },
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

let ids = create_nodes!(ctx.run, mixed, ctx.id);
```

Result:

```text
ctx.id
  node_2d_root
    node_3d_child
      ui_under_3d
        node_2d_under_ui
    ui_sibling
```

## Real Node Data

Use struct update syntax when setting fields.

```rust
let menu = node_collection! {
    {
        name = "pause_menu",
        tags = tags!["ui", "menu"],
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
                node = UiLabel {
                    text: "Paused".into(),
                    font_size: 32.0,
                    ..UiLabel::new()
                },
            },
            { name = "resume", node = UiButton::new() },
            { name = "quit", node = UiButton::new() },
        ],
    }
};

let ids = create_nodes!(ctx.run, menu, ctx.id);
```

Result:

```text
ctx.id
  pause_menu
    title
    resume
    quit
```

## Return Order

IDs return in preorder.

```text
0 root
1 first child
2 first grandchild
3 next child
```

For one-root collections:

```rust
let root = ids[0];
```

For multi-root collections:

```rust
let roots = [ids[0], ids[3], ids[7]];
```

Keep indices local to the collection shape.

Use names/tags to find nodes when tree shape can change.
