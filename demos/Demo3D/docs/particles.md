# Particles Demo

Scene:

- `res://scenes/demos/particles.scn`

Profiles:

- `res://particles/fire_spiral.ppart`
- `res://particles/spark_orbit.ppart`

Shows:

- `ParticleEmitter3D`
- looping/prewarm emitters
- profile-driven particle settings
- billboard render mode
- local emitter placement

Why scene works this way:

- Particle behavior stays in `.ppart` resource files.
- Scene only picks profile, rate, seed, mode, transform.
- Prewarm avoids empty first seconds after load.
- Different seeds keep emitters visually distinct.
- Dark floor and fill light make particles readable.

Scene map:

| Node           | Role                           |
| -------------- | ------------------------------ |
| `FireEmitter`  | Fire spiral profile.           |
| `SparkEmitter` | Orbit spark profile.           |
| `Fill`         | Blue point light for contrast. |
| `Floor`        | Dark background plane.         |

Controls:

| Input             | Action    |
| ----------------- | --------- |
| Mouse             | Look      |
| `W` `A` `S` `D`   | Move      |
| `Space` / `Shift` | Up / down |
| `Esc`             | Pause     |
