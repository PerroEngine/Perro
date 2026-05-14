# Positional Audio Demo

Scene:

- `res://scenes/demos/positional_audio.scn`

Script:

- `res://scripts/positional_audio_demo.rs`

Shows:

- attached MIDI note playback
- `AudioMask3D`
- `AudioEffectZone3D`
- propagation/debug rays
- occlusion material tuning
- camera audio options

Why scene works this way:

- Speaker meshes make audio sources visible.
- Wall mesh matches `AudioMask3D` collision shape.
- Script plays a repeating chord so movement changes are easy to hear.
- Reverb zone covers scene so effect routing is obvious.
- `R` toggles debug rays because ray view is useful but noisy.

Script flow:

| Step                     | Why                                       |
| ------------------------ | ----------------------------------------- |
| Cache speakers           | Avoid name lookups every sound tick.      |
| Configure wall material  | Show runtime audio material edits.        |
| Enable debug rays        | Make propagation visible by default.      |
| Timer plays chord        | Stable repeat signal for testing.         |
| On removal disables rays | Avoid debug leftovers after scene unload. |

Controls:

| Input             | Action                  |
| ----------------- | ----------------------- |
| Mouse             | Look                    |
| `W` `A` `S` `D`   | Move                    |
| `Space` / `Shift` | Up / down               |
| `R`               | Toggle audio debug rays |
| `Esc`             | Pause                   |
