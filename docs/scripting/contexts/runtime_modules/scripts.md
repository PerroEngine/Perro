# Scripts Module

Mental model:

- Scripts extend node functionality.
- Variables and methods are treated as being "on" the node.
- You access script behavior through `NodeID`

Attach/detach:

- `script_attach!(ctx, node_id, script_path) -> bool`
- `script_detach!(ctx, node_id) -> bool`

Self-state access (your own script):

- `with_state!(ctx.run, StateType, self_node_id, |state| -> V { ... }) -> V`
- `with_state_mut!(ctx.run, StateType, self_node_id, |state| -> V { ... }) -> Option<V>`

`with_state!` returns `V::default()` if the node/state is missing or type-mismatched.

Use this for your own script because:

- You have the concrete Rust `StateType`.
- You want strongly typed, compile-time checked access.
- `node_id` here is `ctx.id`, which is the value that is properly attached to your own defined state.

Cross-script access (other nodes):

- `get_var!(ctx.run, node_id, member) -> Variant`
- `set_var!(ctx.run, node_id, member, value) -> ()`
- `call_method!(ctx.run, node_id, method, params) -> Variant`

Use this for other nodes because:

- You usually know their `NodeID` (from query, parent/child traversal, stored refs, etc.).
- You usually do not have their concrete Rust state type (you can import it, but you must know ahead of time the script attached to it and it can fail)
- The API is dynamic by member name/ID (`Variant` based).

Examples:

```rust
// Self: typed state access
with_state_mut!(ctx.run, MyState, self_id, |state| {
    state.hp -= 1;
});

// Other node: dynamic access through NodeID
let enemy_id = query_first!(ctx.run, all(name["Enemy1"])).unwrap();
set_var!(ctx.run, enemy_id, var!("alert"), variant!(true));
call_method!(ctx.run, enemy_id, method!("on_alert"), params![]);
```

Borrow rule:

- `ctx.run` stays mutable handle.
- `with_state_mut!` + `with_node_mut!` hold that mutable borrow for full closure body.
- Any 2nd `ctx.run` use inside same closure fails borrow chk.

Bad: 2nd `ctx.run` borrow inside `with_state_mut!` closure.

```rust
let do_refresh = with_state_mut!(ctx.run, MyState, ctx.id, |state| {
    state.timer += delta_time!(ctx.run);
    //              ^^^^^^^^
    // 2nd mutable borrow of `ctx.run`
    state.timer > 1.0
});
```

Typical rustc err:

```text
error[E0500]: closure requires unique access to `*ctx.run` but it is already borrowed
  --> res://scripts/example.rs:10:56
   |
10 | let do_refresh = with_state_mut!(ctx.run, MyState, ctx.id, |state| {
   |                    ------------  -------                  ^^^^^^^ closure construction occurs here
   |                    |             |
   |                    |             borrow occurs here
11 |     state.timer += delta_time!(ctx.run);
   |                                 ------- 2nd borrow occurs here
```

Good: pull `Copy` vals out b4 closure.

```rust
let dt = delta_time!(ctx.run).max(0.0);
let root = get_var!(ctx.run, ctx.id, var!("active_demo_root"))
    .as_node()
    .unwrap_or(NodeID::nil());

let do_refresh = with_state_mut!(ctx.run, MyState, ctx.id, |state| {
    state.last_root = root;
    state.timer += dt;
    state.timer > 1.0
})
.unwrap_or(false);
```

Bad: owned val use both inside closure + aft closure.

```rust
let demo = get_var!(ctx.run, ctx.id, var!("active_demo"))
    .as_str()
    .unwrap_or("none")
    .to_string();

with_state_mut!(ctx.run, MyState, ctx.id, |state| {
    let changed = state.last_demo != demo;
    state.last_demo = demo;
    changed
});

set_label_text(ctx, body_label, demo);
```

Typical rustc err:

```text
error[E0382]: use of moved value: `demo`
  --> res://scripts/example.rs:17:31
   |
12 | let demo = ... .to_string();
   |     ---- move occurs cuz `demo` type `String`, !Copy
...
15 |     state.last_demo = demo;
   |                       ---- val mv here
...
17 | set_label_text(ctx, body_label, demo);
   |                               ^^^^ val use aft mv
```

Good: clone owned val b4 mutable closure if later code still needs same data.

```rust
let demo = get_var!(ctx.run, ctx.id, var!("active_demo"))
    .as_str()
    .unwrap_or("none")
    .to_string();

let changed = with_state_mut!(ctx.run, MyState, ctx.id, |state| {
    let changed = state.last_demo != demo;
    state.last_demo = demo.clone();
    changed
})
.unwrap_or(false);

set_label_text(ctx, body_label, demo);
```

Bad: nested fn args borrow `ctx` mut 2x in 1 expr.

```rust
set_label_text(ctx, body_label, demo_info_text(ctx, root, &demo));
```

Typical rustc err:

```text
error[E0499]: cannot borrow value as mutable more than once at a time
  --> res://scripts/example.rs:22:41
   |
22 | set_label_text(ctx, body_label, demo_info_text(ctx, root, &demo));
   | -------------- ---             ^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^ 2nd mutable borrow
   | |              |
   | |              1st mutable borrow
   | 1st borrow later use by call
```

Good: split nested call into local.

```rust
let body = demo_info_text(ctx, root, &demo);
set_label_text(ctx, body_label, body);
```

Rule:

- Copy out if type `Copy`.
- Clone out if type owns data + later code still needs same val.
- Split nested calls into locals if fn arg list borrows `ctx` twice.
