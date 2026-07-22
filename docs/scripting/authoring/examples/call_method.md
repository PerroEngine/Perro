# Example: Switch Calls Door

Use `call_method!` because the switch has one fixed target and asks that target
to perform behavior.

## Goal, Owners, And Flow

```text
scene -> SwitchState.door = @ExitDoor
interaction -> switch.activate -> door.toggle -> bool(open)
```

The switch owns interaction and the dependency. The door owns open state and
transition behavior. The scene chooses which door this switch controls.

## Door Script

```rust
#[State]
struct DoorState {
    #[default = false]
    open: bool,
}

lifecycle!({});

methods!({
    fn toggle(&self, ctx: &mut ScriptContext<'_, API>) -> bool {
        with_state_mut!(ctx.run, DoorState, ctx.id, |state| {
            state.open = !state.open;
            state.open
        }).unwrap_or(false)
    }
});
```

## Switch Script

```rust
#[State]
struct SwitchState {
    #[expose]
    #[node_ref(Node3D)]
    door: Option<NodeID>,
}

lifecycle!({});

methods!({
    fn activate(&self, ctx: &mut ScriptContext<'_, API>) -> bool {
        let Some(door) = with_state!(ctx.run, SwitchState, ctx.id, |state| state.door)
        else {
            return false;
        };

        call_method!(ctx.run, door, method!("toggle"), params![])
            .as_bool()
            .unwrap_or(false)
    }
});
```

## Scene Injection

```text
script_vars = {
    door = @ExitDoor
}
```

The switch owns the dependency. The door owns door state and behavior. The
return value tells the switch whether the door ended open.

## Why, Failure, And Tradeoff

A method fits one known target, a command, and a reply. A signal would lose the
request/reply relation and could reach many doors. Direct typed state mutation
would let the switch bypass door animation, locks, and invariants.

Missing/nil door returns `false`; the switch does not panic. A missing method or
wrong return type also decodes to the neutral `false` used here. Extend the door
method with a key parameter, or emit `door_opened` after success for loose audio
and quest listeners.

## Verified Equivalent

ScriptPatterns uses the same fixed-ref + reply shape:
[scene wiring](../../../../demos/ScriptPatterns/res/main.scn),
[controller caller](../../../../demos/ScriptPatterns/res/scripts/controller.rs),
and [player target](../../../../demos/ScriptPatterns/res/scripts/player.rs).

[Back To Examples](index.md)
