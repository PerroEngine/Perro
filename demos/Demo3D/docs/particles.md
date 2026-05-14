# Particles Demo

Scene:

- `res://scenes/demos/particles.scn`

Profiles:

- `res://particles/fire_spiral.ppart`
- `res://particles/spark_orbit.ppart`
- `res://particles/portal_vortex.ppart`
- `res://particles/ember_fountain.ppart`
- `res://particles/ice_mist.ppart`
- `res://particles/laser_rain.ppart`
- `res://particles/orbit_ribbon.ppart`
- `res://particles/shockwave_ring.ppart`

Animation:

- `res://animations/particle_emitter_orbit.panim`

Shows:

- `ParticleEmitter3D`
- looping/prewarm emitters
- profile-driven particle settings
- custom `x/y/z` expressions
- params + `t` / `life` / `age_left` / `emitter_time`
- `prev_x/y/z` and `hash()` path jitter
- billboard render mode
- point render mode
- one `AnimationPlayer` moving an emitter rig
- local emitter placement

Why scene works this way:

- Particle behavior stays in `.ppart` resource files.
- Scene only picks profile, rate, seed, mode, transform.
- Prewarm avoids empty first seconds after load.
- Different seeds keep emitters visually distinct.
- Dark floor and fill light make particles readable.

Scene map:

| Node                   | Role                                 |
| ---------------------- | ------------------------------------ |
| `PortalVortexEmitter`  | Animated billboard vortex portal.    |
| `FireEmitter`          | Billboard fire spiral.               |
| `SparkEmitter`         | Point orbit sparks.                  |
| `EmberFountainEmitter` | Billboard ember burst.               |
| `IceMistEmitter`       | Billboard cold mist.                 |
| `LaserRainEmitter`     | Point green rain streaks.            |
| `OrbitRibbonEmitter`   | Billboard ribbon orbit.              |
| `ShockwaveRingEmitter` | Point expanding ring pulses.         |
| `PortalRigPlayer`      | Moves `PortalRig` by `.panim` clip.  |
| `Fill` / `WarmFill`    | Colored point lights for contrast.   |
| `Floor`                | Dark background plane.               |

Controls:

| Input             | Action    |
| ----------------- | --------- |
| Mouse             | Look      |
| `W` `A` `S` `D`   | Move      |
| `Space` / `Shift` | Up / down |
| `Esc`             | Pause     |
