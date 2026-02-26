# Runtime Context

Type:

- `ctx: &mut RuntimeContext<'_, RT>`

Runtime macros take `ctx` as argument 1.

## Time Macros

### `delta_time!(ctx) -> f32`

- Returns frame delta time in seconds.

### `fixed_delta_time!(ctx) -> f32`

- Returns fixed timestep delta in seconds.

### `elapsed_time!(ctx) -> f32`

- Returns elapsed runtime time in seconds.

## Node Macros

### `create_node!(ctx, NodeType) -> NodeID`

### `create_node!(ctx, NodeType, name) -> NodeID`

### `create_node!(ctx, NodeType, name, tags) -> NodeID`

### `create_node!(ctx, NodeType, name, tags, parent_id) -> NodeID`

- `NodeType`: concrete node type implementing `Default + Into<SceneNodeData>`.
- `name`: `&str | String | Cow<'static, str>`.
- `tags`: value accepted by `Option<T>` where `T: Into<Cow<'static, [TagID]>>` (use `tags![...]`).
- `parent_id`: `NodeID`.

### `with_node_mut!(ctx, NodeType, node_id, |node| -> V { /* body */ }) -> Option<V>`

- Exact-type mutable access.
- `NodeType`: concrete type implementing `NodeTypeDispatch`.
- Returns `None` when `node_id` is invalid or type does not match exactly.

### `with_node!(ctx, NodeType, node_id, |node| -> V { /* body */ }) -> V`

- Exact-type read access.
- `NodeType`: concrete type implementing `NodeTypeDispatch`.
- `V` must implement `Clone + Default`.
- Returns `V::default()` when lookup/type check fails.

### `with_base_node!(ctx, BaseType, node_id, |node| -> V { /* body */ }) -> Option<V>`

- Inheritance-aware read access (`is_a` check).
- `BaseType`: base type implementing `NodeBaseDispatch`.

### `with_base_node_mut!(ctx, BaseType, node_id, |node| -> V { /* body */ }) -> Option<V>`

- Inheritance-aware mutable access (`is_a` check).
- `BaseType`: base type implementing `NodeBaseDispatch`.

### `get_node_name!(ctx, node_id) -> Option<Cow<'static, str>>`

### `set_node_name!(ctx, node_id, name) -> bool`

- `name`: `&str | String | Cow<'static, str>`.

### `get_node_parent_id!(ctx, node_id) -> Option<NodeID>`

### `get_node_children_ids!(ctx, node_id) -> Option<Vec<NodeID>>`

### `get_node_type!(ctx, node_id) -> Option<NodeType>`

### `reparent!(ctx, parent_id, child_id) -> bool`

- `parent_id = NodeID::nil()` detaches `child_id` to root.

### `reparent_multi!(ctx, parent_id, child_ids) -> usize`

- `child_ids`: any `IntoIterator<Item = NodeID>`.
- Returns number of successful reparent operations.

### `remove_node!(ctx, node_id) -> bool`

### `get_node_tags!(ctx, node_id) -> Option<Vec<TagID>>`

### `tag_set!(ctx, node_id, tags) -> bool`

### `tag_set!(ctx, node_id) -> bool`

- 3-arg form sets tags.
- 2-arg form clears tags.

### `tag_add!(ctx, node_id, tags) -> bool`

- Accepts one or many tags via `IntoNodeTags`.
- Common forms: `"enemy"`, `tag!("enemy")`, `["enemy", "alive"]`, `tags!["enemy", "alive"]`, `Vec<TagID>`.

### `tag_remove!(ctx, node_id, tag) -> bool`

### `tag_remove!(ctx, node_id) -> bool`

- 3-arg form removes one tag (`TagID | &str | String`).
- 2-arg form clears all tags.

### `query!(ctx, expr) -> Vec<NodeID>`

### `query!(ctx, expr, in_subtree(parent_id)) -> Vec<NodeID>`

- Boolean forms: `all(...)`, `any(...)`, `not(...)`.
- Predicates: `name[...]`, `tags[...]`, `is[...]` / `is_type[...]`, `base[...]` / `base_type[...]`.
- `tags[...]` must be wrapped in `all/any/not`.

#### Query Forms

- `all(expr1, expr2, ...)`
- `any(expr1, expr2, ...)`
- `not(expr)`
- `in_subtree(parent_id)` (optional second argument to `query!`)

#### Query Predicates

- `name["Player", "Boss"]`
- `tags["enemy", "alive"]`
- `is[Camera3D, MeshInstance3D]`
- `is_type[Camera3D, MeshInstance3D]`
- `base[Node3D]`
- `base_type[Node3D]`

#### Query Examples

```rust
// tag include + exclude
let enemies = query!(ctx, all(tags["enemy"], not(tags["dead"])));

// exact type
let cameras = query!(ctx, all(is[Camera3D]));

// base type + exact type in subtree
let subtree_meshes = query!(ctx, all(base[Node3D], is[MeshInstance3D]), in_subtree(root_id));

// name OR
let named = query!(ctx, any(name["Player", "Boss"]));
```

## Script Macros

### `with_state!(ctx, StateType, script_id, |state| -> V { /* body */ }) -> Option<V>`

- `StateType`: concrete script state type (`'static`).
- Closure arg type: `&StateType`.

### `with_state_mut!(ctx, StateType, script_id, |state| -> V { /* body */ }) -> Option<V>`

- Closure arg type: `&mut StateType`.

### `script_attach!(ctx, node_id, script_path) -> bool`

- `script_path: &str`.

### `script_detach!(ctx, node_id) -> bool`

### `get_var!(ctx, script_id, member) -> Variant`

- `member`: any `IntoScriptMemberID` value.
- Common forms: `sid!("x")`, `var!("x")`, `member!("x")`, `"x"`, `String`.

### `set_var!(ctx, script_id, member, value) -> ()`

- `member`: any `IntoScriptMemberID`.
- `value`: `Variant` (use `variant!(...)`).

### `call_method!(ctx, script_id, method, params) -> Variant`

- `method`: any `IntoScriptMemberID`.
- `params`: `&[Variant]` (use `params![...]`).

### `attributes_of!(ctx, script_id, member) -> &'static [Attribute]`

- `member`: `&str | String | Member`.

### `members_with!(ctx, script_id, attribute) -> &'static [Member]`

- `attribute`: `&str | String | Attribute`.

### `has_attribute!(ctx, script_id, member, attribute) -> bool`

- `member`: `&str | String | Member`.
- `attribute`: `&str | String | Attribute`.

## Signal Macros

### `signal_connect!(ctx, script_id, signal, function) -> bool`

- `signal`: `SignalID` (use `signal!("...")`).
- `function`: `ScriptMemberID` (use `method!("...")` or `func!("...")`).

### `signal_disconnect!(ctx, script_id, signal, function) -> bool`

- Same input types as `signal_connect!`.

### `signal_emit!(ctx, signal, params) -> usize`

### `signal_emit!(ctx, signal) -> usize`

- 3-arg form emits with params (`&[Variant]`, use `params![...]`).
- 2-arg form emits with empty params.
- Returns number of handlers invoked.

## Helper Macros

### `sid!("name") -> ScriptMemberID`

### `var!("name") -> ScriptMemberID`

### `method!("name") -> ScriptMemberID`

### `func!("name") -> ScriptMemberID`

### `signal!("name") -> SignalID`

### `tag!("name") -> TagID`

### `tags!["a", "b"] -> Vec<TagID>`

### `variant!(value) -> Variant`

### `params![a, b, c] -> Vec<Variant>`
