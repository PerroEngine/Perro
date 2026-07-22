# Lifecycle Choice

## Purpose

Choose the earliest callback that has the dependencies an action needs, and no
earlier. This keeps initialization deterministic and frame work small.

## Mental Model

```text
construct state -> apply script_vars -> on_init for each script
-> on_all_init after scene scripts exist -> fixed/update callbacks -> teardown
```

`on_init` can read injected state and initialize its own node. `on_all_init`
fits signal connections and work that assumes the rest of the scene has been
initialized. Update callbacks fit behavior that must react every frame.

## Decision Guide

| Need | Callback | Why |
| --- | --- | --- |
| validate injected state or initialize self | `on_init` | scene vars already apply |
| connect signals among scene scripts | `on_all_init` | all receivers exist |
| visual/input behavior each rendered frame | `on_update` | frame delta and input are current |
| deterministic physics mutation | `on_fixed_update` | fixed cadence |
| delayed one-shot work | named timer handler | no idle per-frame clock |

Do not use `on_update` to repeat fixed lookup or one-time setup. Do not assume
another script completed `on_init` unless the behavior is defined by the engine;
defer cross-script setup to `on_all_init`.

## Failure And Edge Behavior

Treat injected optional refs as absent even during init. A target may also be
removed after init. Skip it or return a neutral result. Avoid panic-based scene
validation in gameplay scripts; doctor provides authoring diagnostics.

## Related

- [State And References](state_and_refs.md)
- [Timers And Borrows](timers_and_borrows.md)
- [Script lifecycle API](../lifecycle.md)

