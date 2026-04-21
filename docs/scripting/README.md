# Scripting Overview

Perro scripts are authored in Rust and compiled into script modules.

Core pieces:
- `#[State]` data struct
- `lifecycle!` for engine entry points
- `methods!` for callable behavior methods
- script contexts (`RuntimeContext`, `ResourceContext`, `InputContext`)

Script dependencies:
- Add extra crates to `deps.toml` in your project root under `[dependencies]`.
- On `perro check`, `perro dev`, and `perro build`, Perro merges those entries into `.perro/scripts/Cargo.toml`.
- Keep `perro` managed by Perro; do not override it in `deps.toml`.

See:
- [Script Contexts](contexts/README.md)
- [Script Utility Modules](modules.md)
- [Math Types](math_types.md)
- [Node Types](nodes.md)
- [Scene Node Templates](scene_node_templates.md)
  - Includes `root_of` scene composition (scenes inside scenes), merge rules, and examples.
- [Script State](state.md)
- [Script Lifecycle](lifecycle.md)
- [Script Methods](methods.md)
