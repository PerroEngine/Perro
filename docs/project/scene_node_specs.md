# Scene Node Specs

Add a node field here when scene authors need stable serialized data. Keep
transient runtime IDs and derived caches on the runtime node instead. This split
lets the editor, parser, runtime builder, and static compiler share one authored
contract without coupling scenes to one backend representation.

## Decision Guide

- normal authored scalar/vector/asset -> `scene_node_fields!`
- field also decoded by runtime code -> `scene_field_group!`
- construction needs full nested scene node -> `data_apply`
- exceptional small build step -> `custom`
- new backend primitive -> new render command; otherwise reuse extractor output

Aliases preserve old scene input. Canonical names define new output. Unknown
fields stay available for scripts/custom data rather than disappearing silently.

## Split

Keep node data, authored scene fields, runtime build hooks, and render extraction separate.

```text
core node registry
-> .scn field spec
-> runtime constructor
-> render extractor
```

The core registry in `perro_nodes` owns `NodeType`, `SceneNodeData`, storage, parent type, and update/render flags.

The scene spec in `perro_scene` owns authored names, value kinds, defaults, aliases, and asset kinds. It must not contain runtime IDs.

The runtime constructor in `perro_runtime` turns authored values into node data. Custom hooks handle complex objects. Asset references become pending bindings and resolve to runtime IDs later.

The render extractor turns runtime node state into existing render primitives. Add a render command only when the backend needs a new primitive.

## Field Spec

Use `scene_node_fields!` for normal authored fields:

```rust,ignore
scene_node_fields!(fields, "Image", {
    texture: Asset(Texture);
    texture_region: Option<Vec4>;
    flip_x: bool [default(SceneValue::Bool(false))] [aliases["mirror_x"]];
});
```

Supported compact types include scalar/vector types, `Option<T>`, `Vec<T>`, `Asset(Kind)`, and `Vec<Asset(Kind)>`.

`Option<T>` controls authored presence. It uses the same editor value schema as `T`.

`Asset(Texture)` means a source reference such as `res://hero.png`. The runtime loader owns the later `TextureID` mapping.

## Runtime Spec

Use `define_scene_node_builder!` for normal scene-to-node builders:

```rust,ignore
define_scene_node_builder! {
    fn build_sprite_2d -> Sprite2D = Sprite2D::new();
    base node_2d;
    apply [apply_sprite_2d_fields];
}
```

The builder macro owns construction, inherited base-data order, field-hook order,
and the return. Use `data_apply` for hooks that need the full nested scene node,
and `custom` for a small exceptional step.

The parser reads the same scene spec. It canonicalizes declared aliases and uses
the declared value kind for input normalization. Unknown fields remain intact for
scripts and custom data.

Use `scene_field_group!` when runtime code also consumes fields. Each row owns the
canonical name, aliases, editor type, and decoded Rust type:

```rust,ignore
scene_field_group! {
    pub mod audio_portal_fields("Audio") {
        ACTIVE: bool = "active" => NodeFieldType::Bool, aliases ["enabled"];
        STRENGTH: f32 = "strength" => NodeFieldType::F32;
    }
}

apply_scene_fields!(data, {
    audio_portal_fields::ACTIVE => |value| { node.active = value; },
    audio_portal_fields::STRENGTH => |value| { node.strength = value; },
});
```

Runtime apply code contains assignments and special behavior only. It does not
repeat authored names, aliases, or scalar conversion calls.

Register each constructor once in `define_runtime_scene_node_specs!`.

```rust,ignore
define_runtime_scene_node_specs! {
    plain {
        Sprite2D => build_sprite_2d,
    }
    styled {
        UiPanel => build_ui_panel,
    }
}
```

`plain` constructors receive scene data.

`styled` constructors also receive the static UI style lookup.

The macro uses the core `From<Node> for SceneNodeData` impl. Core storage changes between inline and boxed forms do not require scene-loader edits.

## Add Node

1. Add runtime node struct.
2. Add core node-registry row.
3. Add authored field spec.
4. Add runtime constructor hook.
5. Add render extractor hook when visual.
6. Add a new render command only for a new backend primitive.

Coverage tests scan `NodeType::ALL` and fail when a runtime constructor is absent.
