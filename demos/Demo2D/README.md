# Demo2D

2D stress + feature map for Perro.

Run:

```text
cargo run -p perro_cli -- dev --path demos\Demo2D
```

Flow:

- hub menu like `Demo3D`
- click lane -> jump 2 mirror zone
- `Esc` -> pause
- restart/hub btns like `Demo3D`

Parity map:

- `Mesh + Materials` -> static sprite atlas stress
- `Lights` -> point/spot/ray 2D light overlap
- `Water` -> 2D water + buoyancy pools
- `Animations` -> animated sprites + `AnimationPlayer`
- `Physics Bones` -> `Skeleton2D` + `PhysicsBoneChain2D`
- `Physics Collisions` -> rigid-body stacks
- `MultiMesh` analog -> dense sprite batches

Gap note:

- `Sky`, `Mesh Blending`, `Positional Audio`, `3D Particles` no full 2D mirror in this pass

Controls:

- `WASD` / arrows => pan
- mouse wheel => zoom
- `R` => rebuild stress sections

Docs: `docs/README.md`
