# Timers And Borrows

## Purpose

Use timers to represent delayed completion. Keep state/node borrows scoped to
pure reads or writes so one runtime operation finishes before another begins.

## Named Timers

Use named timers for one-shot delays and cooldown completion. Connect the
finished signal to a method.

```rust
lifecycle!({
    fn on_all_init(&self, ctx: &mut ScriptContext<'_, API>) {
        signal_connect!(ctx.run, ctx.id, timer_finished!("reload"), func!("on_reload"));
    }
});

methods!({
    fn begin_reload(&self, ctx: &mut ScriptContext<'_, API>) {
        timer_start!(ctx.run, Duration::from_secs(2), "reload");
    }

    fn on_reload(&self, ctx: &mut ScriptContext<'_, API>) {
        with_state_mut!(ctx.run, WeaponState, ctx.id, |state| {
            state.ammo = state.mag_size;
        });
    }
});
```

One runtime-global timer exists per name. Starting the same name from any script
resets that shared slot, and all listeners for its finished signal may react.
Derive dynamic names from feature + `ctx.id` when instances must stay independent,
or intentionally share one literal name for one global deadline. Cancel an owned timer in
`on_removal` when its delayed work must not outlive the script. Keep a state
clock only when each-frame progress matters, such as a blend or visible
countdown.

Connect during `on_all_init` so the handler exists before feature code starts
the timer.

## Keep Runtime Borrows Short

Never call another `ctx.run` API inside a `with_state!`, `with_state_mut!`,
`with_node!`, or `with_node_mut!` closure.

Copy or clone values out. Make the next runtime call after the closure ends.

```rust
let emit_phase_two = with_state_mut!(ctx.run, BossState, ctx.id, |state| {
    let emit = !state.phase_two && state.health <= 50.0;
    state.phase_two |= emit;
    emit
}).unwrap_or(false);

if emit_phase_two {
    signal_emit!(ctx.run, signal!("boss_phase_two"), params![]);
}
```

## Script Boundaries

Keep cohesive behavior with the node that owns it. Move scene-wide flow to a
controller script. Put shared constants, math, and pure transforms in normal
Rust modules. Do not split scripts by a fixed line or role count.

## Failure And Debug

A removed script no longer needs timer completion. A misspelled timer signal or
handler leaves work unfinished, so keep `timer_finished!` and handler names
close and run doctor/check. For borrow failures, return the smallest copied or
cloned result, then place the next runtime macro below the closure.

## Related

- [Timer-Driven Cooldown](examples/cooldown.md)
- [Time runtime API](../contexts/runtime_modules/time.md)
- [Boundaries And Quality](boundaries_and_quality.md)

[Back To Guide](index.md)
