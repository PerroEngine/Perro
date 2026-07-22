# Feature Story: Timer-Driven Cooldown

## Goal

An ability becomes unavailable on use and available two seconds later. No UI
needs continuous remaining-time progress.

## Owners And Flow

```text
ability method -> State.ready = false -> start "dash_cd"
timer finish signal -> ability handler -> State.ready = true
optional ability_ready signal -> HUD/audio listeners
```

The ability script owns readiness. It connects its timer-finished signal during
`on_all_init`. `try_dash` mutates readiness and returns success. If successful,
it starts the timer after the state borrow ends. The finish handler restores
readiness and may announce `ability_ready`.

```rust
#[State]
struct DashState {
    #[default = true]
    ready: bool,
    #[default = String::new()]
    timer_name: String,
}

lifecycle!({
    fn on_all_init(&self, ctx: &mut ScriptContext<'_, API>) {
        let name = format!("dash_cd_{}", ctx.id.index());
        with_state_mut!(ctx.run, DashState, ctx.id, |state| {
            state.timer_name = name.clone();
        });
        signal_connect!(ctx.run, ctx.id, timer_finished!(name.as_str()), func!("finish_dash"));
    }

    fn on_removal(&self, ctx: &mut ScriptContext<'_, API>) {
        let name = with_state!(ctx.run, DashState, ctx.id, |state| state.timer_name.clone()).unwrap_or_default();
        timer_cancel!(ctx.run, name.as_str());
    }
});

methods!({
    fn try_dash(&self, ctx: &mut ScriptContext<'_, API>) -> bool {
        let result = with_state_mut!(ctx.run, DashState, ctx.id, |state| {
            if !state.ready { return None; }
            state.ready = false;
            Some(state.timer_name.clone())
        }).flatten();
        let Some(name) = result else { return false; };
        timer_start!(ctx.run, Duration::from_secs(2), name.as_str());
        true
    }

    fn finish_dash(&self, ctx: &mut ScriptContext<'_, API>) {
        with_state_mut!(ctx.run, DashState, ctx.id, |state| state.ready = true);
        signal_emit!(ctx.run, signal!("ability_ready"), params![ctx.id]);
    }
});
```

Scene wiring needs no external ref. Each ability instance owns readiness and its
derived timer name. Callers target `try_dash`; optional HUD/audio listeners use
`ability_ready`.

## Why This API

A named timer represents delayed completion directly and performs no manual
per-frame decrement. One runtime-global timer exists per name, so each instance
derives a unique name. A single global ability may use one literal name instead.

Do not store a countdown updated every frame unless UI or interpolation needs
continuous progress. Do not start the timer inside the state closure because it
nests a runtime call under a runtime borrow.

## Failure And Extensions

Repeated rejected calls leave the active timer unchanged. Removal cancels the
owned timer so its global slot does not outlive the script. Extend with a separate progress clock only
when a visible radial indicator needs remaining time each frame.

Verified timer/lifecycle shape: [ScriptPatterns controller](../../../../demos/ScriptPatterns/res/scripts/controller.rs).
