# Scripting Overview

Perro scripts are authored in Rust and compiled into script modules.

Core pieces:
- `#[State]` data struct
- `lifecycle!` for engine entry points
- `methods!` for callable behavior methods
- script contexts (`RuntimeContext`, `ResourceContext`, `InputContext`)

See:
- [Script Contexts](contexts/README.md)
- [Node Types](nodes.md)
- [Scene Node Templates](scene_node_templates.md)
- [Script State](state.md)
- [Script Lifecycle](lifecycle.md)
- [Script Methods](methods.md)
