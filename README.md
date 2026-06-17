# Perro Engine

<div align="center" style="margin-bottom: 16px;">
  <img src="perro.svg" alt="Perro Logo" width="300"/>
</div>

**Perro** is an experimental, open-source game engine written in **Rust**. With a focus on performance and simplicity without sacrificing either.

## Philosophy

- **Simple To Learn**: start with scenes, nodes, and Rust scripts without large registration steps or boilerplate.
- **Flexible To Use**: removes a lot of borrow checker issues by having direct apis and scoped closures for when we need mutable access, seperating state from the script behavior means we will never have a runtime borrow fail.
- **Fast In Release**: nodes and scripts are laid out for efficient node and state access, all resources are statically baked in release for efficient and quick retrieval

## Design Goals

- **Full Game-Making Scope**: 2D, 3D, and UI all matter. Perro aims to support both 2D and 3D performantly, with high frame rates and a workflow that stays simple.
- **Simple Start**: get first scene and script running quickly, with minimal setup and no script-registration boilerplate.
- **Compiler-Managed Workflow**: let Perro sync scripts, generate glue code, and prepare supported assets so project setup stays small.
- **Split Model**: scripts are just Rust files (lifecycle + methods); they store #[State] structs which each instance gets a copy of.
- **Safe Mutation**: access through `NodeID` closures and engine-managed storage avoids borrow-contention edge cases in normal gameplay code (no "try_get_mut" fails).
- **Fast Access**: flat ID lookups keep common node/script operations efficient, with room to cache IDs for hot paths.
- **Quick Iteration**: project scripts build and reload in usually less than 1 second after initial compilation.

For more details, see the full documentation: [perroengine.com/docs](https://www.perroengine.com/docs).

Local reference:

- [Docs Index](docs/index.md)
- [WASM / Web Target](docs/WASM.md)
- [ResPath](docs/resources/respath.md)
- [`project.toml`](docs/project/project_toml.md)
- [Feature Matrix](docs/project/feature_matrix.md)
- [Performance + Flexibility Philosophy](docs/project/performance_philosophy.md)
- [Perro CLI](docs/tools/perro_cli.md)

## Dev Checks

- `cargo check --workspace --all-targets`
- `cargo test --workspace`
- `cargo clippy --workspace --all-targets -- -D warnings -F clippy::all`

## Major Features

- **Behavior Scripts + Per-Node State**: a script is function entry points (lifecycle hooks + methods), not a mutable script object. When a node binds that script, runtime uses that node’s `ctx.id` to run behavior and resolve that node’s own `#[State]` via `with_state!`/`with_state_mut!`.
- **Object-Centric Scene Model**: parent/child relationships, concrete node types, and traditional game-object structure stay front and center.
- **Compiler-Backed Asset Flow**: dev stays flexible with plain files, while build/export bakes supported assets into fast static lookup paths and packs the rest.
- **Powerful UI System**: UI is built as a real engine system with relative sizing, clamping, and layouts designed to scale from simple menus to larger game interfaces.
- **Flat ID-Based Runtime Access**: node and script data are addressed by `NodeID`, enabling constant-time lookups for common operations and efficient cross-system interaction.
- **Predictable Failure Modes**: most runtime misses come from real-world state changes (deleted node, missing tag/name match, unbound script), not from borrow contention between unrelated systems. (NoT "try_get_mut" runtime errors)
- **Powerful Query Layer**: if you prefer query-style access, filter by type, base type, tag, name, and subtree to gather `NodeID`s, then operate directly through script/node APIs. See [Query System](docs/scripting/query_system.md).

## Contributions

Perro is, of course, **open source**, and contributions are always appreciated: issue reports, new features, system optimizations, and other improvements. Everyone is welcome to join the project.

## Support Perro

Donations help fund full-time development, faster features, and better tooling. If you want to support the project:

- [Support Directly](https://perroengine.com/sponsor)
- [Support on Ko-fi](https://ko-fi.com/perroengine)

---

## License

Perro is licensed under the **Apache 2.0 License**. See [LICENSE](LICENSE) for details.

---
