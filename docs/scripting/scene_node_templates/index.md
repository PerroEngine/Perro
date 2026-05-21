# Scene Node Templates

These are copy/paste authoring templates for `.scn` files.
Every node template lists the fields it exposes, including fields that default to nil/empty.

Conventions used below:

- `$root = @NODEKEY` is required. It marks the scene root and defines `$root` as a reusable node ref.
- `parent = @PARENTKEY` is a placeholder parent node key. Use `$root` for the scene root, or `@OtherNode` for a direct node ref.
- Use `@NodeKey` whenever a scene value needs a node ref, including `script_vars` and animation bindings.
- A node ref is `@` plus the full scene key. Examples: `[Name]` -> `@Name`, `[@Name]` -> `@@Name`, `[@@Name]` -> `@@@Name`.
- In `.panim` and `.panimtree`, `@Name` marks animation object refs or graph refs, then scene bindings map object names to `@NodeKey`.
- `script = "res://path/to/script.rs"` is an example script path.
- `res://path/to/...` placeholders show expected path shape.

Root example:

```text
$root = @Main

[Main]
    [Node2D/]
[/Main]

[Child]
parent = $root
    [Node2D/]
[/Child]
```

`$root` is a special scene variable. It must be assigned to a node ref with `@`.
Use `$root` later anywhere a node ref is accepted, such as `parent = $root`.

Escaped `@` key example:

```text
$root = @@@Main

[@@Main]
    [Node2D/]
[/@@Main]

[Child]
parent = @@@Main
    [Node2D/]
[/Child]
```

Node ref examples:

```text
script_vars = {
    target = @Enemy
}

[Anim]
    [AnimationPlayer]
        animation = "res://animations/idle.panim"
        bindings = { Hero = @PlayerRoot }
    [/AnimationPlayer]
[/Anim]
```

General wrapper (it might look like this):

```text
[name]
parent = @PARENTKEY
script = "res://path/to/script.rs"

    [Type]
        field = value
        [AncestorType]
            field = value
        [/AncestorType]
    [/Type]
[/name]
```

Inner type blocks are inheritance data, not child nodes.
Example: `[RigidBody3D] ... [Node3D] ... [/Node3D]` means `RigidBody3D` inherits `Node3D` fields.

Child nodes are separate top-level scene entries with `parent = @ParentKey`.
Example: author `CollisionShape3D` as its own node and set `parent = @RigidBodyKey`.

Templates are per node.
Composition rules, like physics/audio nodes needing collision shape children, live in [Node Types](../nodes.md).


## Template Sets

- [2D Templates](2d.md)
- [3D Templates](3d.md)
- [UI Templates](ui.md)
- [Extra Examples](examples.md)

## Base Template

```text
[node]
parent = @PARENTKEY
script = "res://path/to/script.rs"
    [Node/]
[/node]
```

## Scenes Inside Scenes (`root_of`)

`root_of` lets a node act like an **Imported Scene Instance** layered on top of a **Base Scene Template** root.
Think of it as:

- `final node = Base Scene Template root + Imported Scene Instance overrides`
- Base Scene Template children are imported under the host node
- Imported Scene Instance children still work normally

### Example

```text
$root = @Main

[Main]
root_of = "res://shared/player_base.scn"
script_vars = {
    speed = 9.5
    debug_only = __unset__
    tuning = { sprint = 2.2 }
}
    [Node2D]
        position = (16, 48)
    [/Node2D]
[/Main]

[ExtraHat]
parent = $root
    [Sprite2D]
        texture = "res://cosmetics/hat.png"
        [Node2D]
            position = (0, -6)
        [/Node2D]
    [/Sprite2D]
[/ExtraHat]
```

When you only want defaults from the Base Scene Template, you can omit the node type block entirely:

```text
[Main]
root_of = "res://shared/player_base.scn"
[/Main]
```

### Merge Rules

- `script`:
  - default: inherit from Base Scene Template root
  - if Imported Scene Instance sets `script = "..."`: instance replaces template script
  - if Imported Scene Instance sets `script = null`: inherited template script is removed
- `script_vars`:
  - default: map merge
  - Imported Scene Instance key wins on conflicts
  - use `__unset__` to remove inherited keys
  - nested objects are merged by key
- normal properties (position, rotation, etc.):
  - if Imported Scene Instance defines field: instance value wins
  - if Imported Scene Instance omits field: template value is kept
- arrays/lists:
  - Imported Scene Instance value replaces Base Scene Template value

### Notes

- Base Scene Template root type and Imported Scene Instance node type should match for field-level merging.
  - If types differ, Imported Scene Instance node data is used as-is.
- `root_of` expansion supports nesting (a Base Scene Template can itself use `root_of`).
- Cycles are rejected (`A` includes `B` includes `A`).
