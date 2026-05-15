# Demo2D Docs

Run:

```text
cargo run -p perro_cli -- dev --path demos\Demo2D
```

## Flow Parity

- use hub menu scene
- use pause menu scene
- use same button lane names as `Demo3D`
- hub btn jump cam 2 mirror zone now

## Stress Map

| Zone | Goal | Load |
| --- | --- | --- |
| Static sprites | batch + fill | `256 + 1024 + 4096` sprites |
| Animated sprites | atlas anim cost | `64 + 256 + 1024` animated sprites |
| Lights | additive 2D light overlap | `12` lights over sprite field |
| Water | sim + buoyancy | `3` pools + `48` floaters |
| Physics | rigid body broad/narrow phase | `240` dynamic bodies |
| Animation players | clip playback scale | `48` actors + `48` `AnimationPlayer` |
| Skeletal | bone anim + physics chain | `12` rigs + `12` tail chains |

## Demo3D Parity

| Demo3D lane | Demo2D lane |
| --- | --- |
| Mesh + Materials | Static sprite atlas fields at `256/1024/4096` |
| Lights | Point + spot + ray light overlap over sprite field |
| Water | 3 water pools + 48 buoyant rigid bodies |
| Animations | Animated sprite strips + 48 `AnimationPlayer` actors |
| Physics Bones | 12 `Skeleton2D` rigs + `PhysicsBoneChain2D` tails |
| Physics Collisions | 240 rigid bodies on shared floor |
| MultiMesh | Dense retained sprite batch stress |

## Gap Map

| Demo3D lane | Demo2D state |
| --- | --- |
| Sky | no 2D analog |
| Mesh Blending | no 2D analog |
| Particles | not added yet |
| Positional Audio | not added yet |

## Compare Use

- run profiler overlay/tooling
- pan each zone
- chk fps + frametime deltas
- cmp static sprite zone vs anim sprite zone
- cmp anim player zone vs skeletal zone
- cmp dry physics stack vs water pool floaters

## Assets

- `res/sprite_sheet.png` => 8x8 atlas
- `res/hero_sheet.png` => 4-frame actor strip
- `res/light_disc.png` => soft light marker
- `res/rigs/demo_tail.pskel2d` => simple 2d rig
- `res/animations/*.panim` => player + rig clips
