# Perro Animation: Closed vs Open Keyframes

Perro animation keys now support two key modes:

- **Closed keyframe** (`[FrameN]`): authoritative authored value.
- **Open keyframe** (`[FrameN?]`): continuity marker; treat start value as runtime/current state.

## Philosophy

Closed keys define exact animation state.
Open keys preserve continuity and let the animation adapt to current runtime motion.

This is designed to avoid snaps/pops and support blending, procedural layers, IK, interruptible transitions, and gameplay-driven state changes.

## `.panim` syntax

```text
[Frame0]    // closed (default)
[Frame0?]   // open
```

Open-frame shorthand applies to keys authored in that frame.

## Runtime semantics

For a segment that starts at an open key and ends at a later closed key:

- The **origin** of interpolation should be the current runtime value when entering that open frame.
- The key's stored authored value is **not authoritative for sampling**.
- Sampling on open keys should be treated as **preserve current / runtime value**, not a forced pose.

In code, `AnimationObjectKey::sampled_value()` returns:

- `Some(&value)` for `Closed`
- `None` for `Open`

## Static/compile-time behavior

Static deterministic optimizations must only run on fully-closed tracks.

Tracks containing open keys are runtime-dependent and should not be reduced using closed-key sampling assumptions.
