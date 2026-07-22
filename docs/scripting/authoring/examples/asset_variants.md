# Feature Story: Scene-Injected Asset Variants

## Goal

Several character instances share one script but select different portraits,
materials, and optional audio data in their scenes.

## State And Scene Wiring

```rust
#[derive(Clone, Default, Variant)]
struct CharacterLook {
    portrait: TextureID,
    materials: Vec<MaterialID>,
    hurt_font: Option<SoundFontID>,
}

#[State]
struct CharacterState {
    #[default = CharacterLook::default()]
    look: CharacterLook,
}
```

```text
script_vars = { look = {
    portrait = "res://characters/scout.png",
    materials = ["res://materials/body.pmat", "res://materials/trim.pmat"],
    hurt_font = nil
} }
```

## Complete Flow

The scene parser builds the nested value. Resource-aware scene decode resolves
each path through normal Resource API caches, constructs `CharacterLook`, and
applies it before `on_init`. The script copies required IDs from typed state and
sets known node fields after the state borrow ends.

The character script owns `CharacterState`. The scene owns per-instance look
choices. A fixed portrait `NodeID` in the same state identifies the target
sprite; `on_init` copies `(portrait_node, look.portrait)` out, skips either nil
ID, and applies `sprite.texture` through `with_node_mut!`. Resource caches own
the loaded resource lifetime; state keeps stable typed handles.

## Why This API

Typed IDs make resource kind explicit and reuse stable cached handles. Nested
state keeps one cohesive per-instance look. Scene injection makes variants
authorable without code branches.

Do not keep a global constant path when instances differ. Do not use runtime
`set_var!` with a string path; runtime assignment is strict and expects an
already resolved typed ID.

## Failure And Extensions

Invalid type/path keeps the field default. Loader nil/failure semantics remain
unchanged. Guard nil optional resources. Extend the custom struct with animation
or mesh IDs; recursive scene decode uses the same rule.

Verified direct form: [ScriptPatterns scene](../../../../demos/ScriptPatterns/res/main.scn)
injects a texture path, [controller](../../../../demos/ScriptPatterns/res/scripts/controller.rs)
passes the typed ID, and [player](../../../../demos/ScriptPatterns/res/scripts/player.rs)
applies it to a fixed sprite ref.
