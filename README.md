# Perro Engine

<div align="center">
  <img src="icon.png" alt="Perro Logo" width="200"/>
</div>

**Perro** is an experimental, open-source game engine written in **Rust**. Built with a focus on **simplicity** and **performance**, without sacrificing either.

## Design Goals

- **Powerful Rendering**: build 2D, 3D, and UI-heavy games in one engine.
- **Simple Start**: get first scene and script running quickly, with minimal setup and no script-registration boilerplate.
- **Split Model**: scripts are just Rust files (lifecycle + methods); they store #[State] structs which each instance gets a copy of.
- **Safe Mutation**: access through `NodeID` closures and engine-managed storage avoids borrow-contention edge cases in normal gameplay code (no "try_get_mut" fails).
- **Fast Access**: flat ID lookups keep common node/script operations efficient, with room to cache IDs for hot paths.
- **Quick Iteration**: project scripts build and reload in usually less than 1 second after initial compilation.

For more details, see the full documentation: [perroengine.com/docs](https://www.perroengine.com/docs).

Local reference:

- [Docs Index](docs/index.md)
- [Perro CLI](docs/perro_cli.md)

## Major Features

- **Behavior Scripts + Per-Node State**: a script is function entry points (lifecycle hooks + methods), not a mutable script object. When a node binds that script, runtime uses that node’s `ctx.id` to run behavior and resolve that node’s own `#[State]` via `with_state!`/`with_state_mut!`.
- **Object-Centric Scene Model**: parent/child relationships, concrete node types, and traditional game-object structure stay front and center.
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
