# Scripts Module

Mental model:

- Scripts extend node functionality.
- Variables and methods are treated as being "on" the node.
- You access script behavior through `NodeID`

Attach/detach:

- `script_attach!(ctx, node_id, script_path) -> bool`
- `script_detach!(ctx, node_id) -> bool`

Self-state access (your own script):

- `with_state!(ctx, StateType, self_node_id, |state| -> V { ... }) -> V`
- `with_state_mut!(ctx, StateType, self_node_id, |state| -> V { ... }) -> Option<V>`

`with_state!` returns `V::default()` if the node/state is missing or type-mismatched.

Use this for your own script because:

- You have the concrete Rust `StateType`.
- You want strongly typed, compile-time checked access.
- Typical `node_id` here is `self_id`.

Cross-script access (other nodes):

- `get_var!(ctx, node_id, member) -> Variant`
- `set_var!(ctx, node_id, member, value) -> ()`
- `call_method!(ctx, node_id, method, params) -> Variant`
- `attributes_of!(ctx, node_id, member) -> &'static [Attribute]`
- `members_with!(ctx, node_id, attribute) -> &'static [Member]`
- `has_attribute!(ctx, node_id, member, attribute) -> bool`

Use this for other nodes because:

- You usually know their `NodeID` (from query, parent/child traversal, stored refs, etc.).
- You usually do not have their concrete Rust state type.
- The API is dynamic by member name/ID (`Variant` based).

Examples:

```rust
// Self: typed state access
with_state_mut!(ctx, MyState, self_id, |state| {
    state.hp -= 1;
});

// Other node: dynamic access through NodeID
let enemy_id = query_first!(ctx, all(name["Enemy1"])).unwrap();
set_var!(ctx, enemy_id, var!("alert"), variant!(true));
call_method!(ctx, enemy_id, method!("on_alert"), params![]);
```
