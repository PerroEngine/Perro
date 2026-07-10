# Perro Editor Scripts

- `editor/`: root editor script bound by scenes.
- `app/`: project boot, manager, project helpers.
- `assets/`: file tree, asset scan, file watch.
- `scene/`: scene doc, nodes, nav, viewport, gizmos, animation.
- `ui/`: editor view data, inspector data, UI sync.

## Repo Commands

Run from repo root.

| Point | Command | Gate |
| --- | --- | --- |
| Fast edit loop | `cargo run -p perro_cli -- check --path perro_editor` | Sync and compile editor scripts. |
| Ref audit | `cargo run -p perro_cli -- doctor --path perro_editor` | Compile scripts, then check project and asset refs. |
| Lint | `cargo run -p perro_cli -- clippy --path perro_editor` | Sync scripts, run project doctor, then deny Clippy warnings. |
| Script tests | `cargo run -p perro_cli -- test --path perro_editor` | Sync scripts and run generated crate tests. |
| UI smoke | `cargo run -p perro_cli -- dev --path perro_editor --timings --ui-profile` | Run editor with timing and UI profile output. |

## Handoff Gate

Run in this order:

```powershell
cargo run -p perro_cli -- check --path perro_editor
cargo run -p perro_cli -- clippy --path perro_editor
cargo run -p perro_cli -- test --path perro_editor
```

`clippy` includes `doctor`.
Use standalone `doctor` for ref-only triage after a clean script build.
