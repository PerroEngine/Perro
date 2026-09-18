# Spawn And Runtime Script Attach

## Purpose

Author every reusable or reviewable subtree as a `.scn` file. Create or load
content at runtime only for generated/transient leaf data, tooling, tests, or
user-generated topology. Projectiles, pooled effects, enemies, and other
gameplay objects still use authored `.scn` prefabs when they have reusable
shape, child nodes, assets, tags, scripts, or `script_vars`.

## Mental Model

```text
manager chooses when/where -> load authored .scn -> attach/configure
spawned script owns its state -> signal reports lifecycle facts
query/registry tracks dynamic set
```

Prefer a pre-authored `.scn` for every reusable gameplay spawn. Load or
preload the scene, attach its returned root, and let the scene own its
composition. Use direct node creation only for a narrow transient leaf with
no reusable authored topology; do not use it as a shorter way to build a game
scene.

Keep runtime attachment intentional. `script_attach!` creates default state and
runs the attached script's `on_init` synchronously. It accepts no scene vars.
The caller may set dynamic vars or call an explicit init method only after
`on_init`, but before queued `on_all_init` and update work. If `on_init` needs
required config, spawn an authored `.scn` with `script_vars` instead.

## Failure And Cleanup

Spawn APIs may return nil/failure. Do not register a failed spawn. Remove dead
IDs from registries, or query current tags when correctness matters more than a
cached set. Let spawned objects emit facts such as `enemy_died`; do not give
every enemy a manager ref unless it needs targeted manager behavior.

## Related

- [Manager And Spawned Enemies](examples/spawned_enemies.md)
- [Nodes runtime module](../contexts/runtime_modules/nodes.md)
- [Scripts runtime module](../contexts/runtime_modules/scripts.md)
