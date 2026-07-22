# Spawn And Runtime Script Attach

## Purpose

Create nodes at runtime only when scene topology cannot know the instance in
advance: projectiles, enemies, pooled effects, or user-generated objects.

## Mental Model

```text
manager chooses when/where -> create or instantiate node -> attach/configure
spawned script owns its state -> signal reports lifecycle facts
query/registry tracks dynamic set
```

Prefer pre-authored scene instances for fixed dependencies. Use a preloaded
scene when a spawn has meaningful child structure or needs scene vars before
`on_init`; use direct node creation for a simple single node.

Keep runtime attachment intentional. `script_attach!` creates default state and
runs the attached script's `on_init` synchronously. It accepts no scene vars.
The caller may set dynamic vars or call an explicit init method only after
`on_init`, but before queued `on_all_init` and update work. If `on_init` needs
required config, spawn an authored scene with `script_vars` instead.

## Failure And Cleanup

Spawn APIs may return nil/failure. Do not register a failed spawn. Remove dead
IDs from registries, or query current tags when correctness matters more than a
cached set. Let spawned objects emit facts such as `enemy_died`; do not give
every enemy a manager ref unless it needs targeted manager behavior.

## Related

- [Manager And Spawned Enemies](examples/spawned_enemies.md)
- [Nodes runtime module](../contexts/runtime_modules/nodes.md)
- [Scripts runtime module](../contexts/runtime_modules/scripts.md)
