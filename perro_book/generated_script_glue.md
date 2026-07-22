# Generated Script Glue

`perro check`, `perro dev`, and `perro build` create Rust glue under `.perro/`.

## Goal

Know what Perro generates and why you usually ignore it.

## Mental Model

Author scripts under `res/`; inspect generated glue to diagnose expansion or
dispatch, never as the source of a fix. `perro check` may overwrite generated
files, so an edit under `.perro/scripts` has no ownership and no stable lifetime.

The glue preserves two boundaries: typed Rust inside a script and `Variant`
conversion at dynamic method/var dispatch. Prefer typed calls until code crosses
that boundary.

## `.perro/scripts`

Project scripts live under `res/**/*.rs`.

Perro syncs them into `.perro/scripts/src/*.gen.rs`.

The generated scripts crate adds:

- module exports
- script registry
- behavior impls
- state creation
- var get/set dispatch
- method call dispatch

Source path:

- `perro_source/build_pipeline/perro_compiler/src/script_writer.rs`
- `perro_source/build_pipeline/perro_compiler/src/script_codegen.rs`
- `perro_source/build_pipeline/perro_compiler/src/script_methods.rs`

## What A Script Becomes

Your script:

```rust
#[State]
pub struct DoorState {
    #[default(false)]
    open: bool,
}

methods!({
    fn set_open(&self, ctx: &mut ScriptContext<'_, API>, open: bool) {
        with_state_mut!(ctx.run, DoorState, ctx.id, |state| {
            state.open = open;
        });
    }
});
```

Generated glue adds behavior code that can:

- `create_state` -> make `DoorState`
- `get_var` -> read fields as `Variant`
- `set_var` -> write fields from `Variant`
- `call_method` -> parse params + call `set_open`
- `perro_create_script` -> return behavior object

## `perro check`

`perro check` syncs scripts and builds generated script code.

Use it for editor/IDE feedback.

It is not the game runner.

## `perro dev`

Native dev builds `.perro/dev_runner`.

The runner loads the generated scripts registry dynamically.

Web dev builds a wasm bundle from `.perro/project`.

Web dev uses static embedded assets so browser behavior matches web build more closely.

## `perro build`

Build creates `.perro/project` and emits release output.

Native output goes to `.output/`.

Web output goes to `.output/web/`.

Supported assets go through the static pipeline.

Generic assets use the packed asset path.

## Static Embed

Static export bakes supported assets into generated lookup data.

The runtime can use hash lookup and already-shaped data instead of file IO and source parsing.

Examples:

- scenes
- materials
- UI styles
- CSV
- meshes
- textures
- audio

## Reference

- [Perro CLI](/docs/tools/perro_cli)
- [WASM / Web Target](/docs/WASM)
- [Performance + Flexibility Philosophy](/docs/project/performance_philosophy)
