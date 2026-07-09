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
- `Particles` -> 4 `ParticleEmitter2D` stress emitters
- `Positional Audio` -> 3 MIDI speakers + masks + effect zones

No-analog note:

- `Sky` and `Mesh Blending` stay 3D-only lanes

Controls:

- `WASD` / arrows => pan
- mouse wheel => zoom
- `R` => rebuild stress sections
- `T` => toggle audio debug rays in positional-audio lane

Docs: `docs/README.md`
