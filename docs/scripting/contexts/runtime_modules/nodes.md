# Nodes Module

## Page Map

| Header | Link |
| --- | --- |
| Purpose | [Purpose](#purpose) |
| Use Cases | [Use Cases](#use-cases) |
| Context | [Context](#context) |
| Practical Example | [Practical Example](#practical-example) |
| API Reference | [API Reference](#api-reference) |
| `create` | [`create`](#create) |
| `create_nodes` | [`create_nodes`](#create_nodes) |
| `with_node_mut` | [`with_node_mut`](#with_node_mut) |
| `with_node` | [`with_node`](#with_node) |
| `with_base_node` | [`with_base_node`](#with_base_node) |
| `with_base_node_mut` | [`with_base_node_mut`](#with_base_node_mut) |
| `get_node_name` | [`get_node_name`](#get_node_name) |
| `set_node_name` | [`set_node_name`](#set_node_name) |
| `get_skeleton_bone_name` | [`get_skeleton_bone_name`](#get_skeleton_bone_name) |
| `get_skeleton_bone_index` | [`get_skeleton_bone_index`](#get_skeleton_bone_index) |
| `set_ui_rotation` | [`set_ui_rotation`](#set_ui_rotation) |
| `bind_locale_text` | [`bind_locale_text`](#bind_locale_text) |
| `bind_locale_placeholder` | [`bind_locale_placeholder`](#bind_locale_placeholder) |
| `get_node_parent_id` | [`get_node_parent_id`](#get_node_parent_id) |
| `get_node_children_ids` | [`get_node_children_ids`](#get_node_children_ids) |
| `get_children` | [`get_children`](#get_children) |
| `get_child_at` | [`get_child_at`](#get_child_at) |
| `get_child_by_name` | [`get_child_by_name`](#get_child_by_name) |
| `get_children_by_name` | [`get_children_by_name`](#get_children_by_name) |
| `get_child` | [`get_child`](#get_child) |
| `get_node_type` | [`get_node_type`](#get_node_type) |
| `reparent` | [`reparent`](#reparent) |
| `force_rerender` | [`force_rerender`](#force_rerender) |
| `mark_needs_rerender` | [`mark_needs_rerender`](#mark_needs_rerender) |
| `reparent_multi` | [`reparent_multi`](#reparent_multi) |
| `remove_node` | [`remove_node`](#remove_node) |
| `get_node_tags` | [`get_node_tags`](#get_node_tags) |
| `tag_set` | [`tag_set`](#tag_set) |
| `add_node_tag` | [`add_node_tag`](#add_node_tag) |
| `add_node_tags` | [`add_node_tags`](#add_node_tags) |
| `remove_node_tag` | [`remove_node_tag`](#remove_node_tag) |
| `get_global_transform_2d` | [`get_global_transform_2d`](#get_global_transform_2d) |
| `get_global_transform_3d` | [`get_global_transform_3d`](#get_global_transform_3d) |
| `get_local_transform_2d` | [`get_local_transform_2d`](#get_local_transform_2d) |
| `get_local_transform_3d` | [`get_local_transform_3d`](#get_local_transform_3d) |
| `set_local_transform_2d` | [`set_local_transform_2d`](#set_local_transform_2d) |
| `set_local_transform_3d` | [`set_local_transform_3d`](#set_local_transform_3d) |
| `set_global_transform_2d` | [`set_global_transform_2d`](#set_global_transform_2d) |
| `set_global_transform_3d` | [`set_global_transform_3d`](#set_global_transform_3d) |
| `get_local_pos_2d` | [`get_local_pos_2d`](#get_local_pos_2d) |
| `get_local_pos_3d` | [`get_local_pos_3d`](#get_local_pos_3d) |
| `set_local_pos_2d` | [`set_local_pos_2d`](#set_local_pos_2d) |
| `set_local_pos_3d` | [`set_local_pos_3d`](#set_local_pos_3d) |
| `get_global_pos_2d` | [`get_global_pos_2d`](#get_global_pos_2d) |
| `get_global_pos_3d` | [`get_global_pos_3d`](#get_global_pos_3d) |
| `set_global_pos_2d` | [`set_global_pos_2d`](#set_global_pos_2d) |
| `set_global_pos_3d` | [`set_global_pos_3d`](#set_global_pos_3d) |
| `get_local_rot_2d` | [`get_local_rot_2d`](#get_local_rot_2d) |
| `get_local_rot_3d` | [`get_local_rot_3d`](#get_local_rot_3d) |
| `set_local_rot_2d` | [`set_local_rot_2d`](#set_local_rot_2d) |
| `set_local_rot_3d` | [`set_local_rot_3d`](#set_local_rot_3d) |
| `get_global_rot_2d` | [`get_global_rot_2d`](#get_global_rot_2d) |
| `get_global_rot_3d` | [`get_global_rot_3d`](#get_global_rot_3d) |
| `set_global_rot_2d` | [`set_global_rot_2d`](#set_global_rot_2d) |
| `set_global_rot_3d` | [`set_global_rot_3d`](#set_global_rot_3d) |
| `get_local_scale_2d` | [`get_local_scale_2d`](#get_local_scale_2d) |
| `get_local_scale_3d` | [`get_local_scale_3d`](#get_local_scale_3d) |
| `set_local_scale_2d` | [`set_local_scale_2d`](#set_local_scale_2d) |
| `set_local_scale_3d` | [`set_local_scale_3d`](#set_local_scale_3d) |
| `get_global_scale_2d` | [`get_global_scale_2d`](#get_global_scale_2d) |
| `get_global_scale_3d` | [`get_global_scale_3d`](#get_global_scale_3d) |
| `set_global_scale_2d` | [`set_global_scale_2d`](#set_global_scale_2d) |
| `set_global_scale_3d` | [`set_global_scale_3d`](#set_global_scale_3d) |
| `to_global_point_2d` | [`to_global_point_2d`](#to_global_point_2d) |
| `to_local_point_2d` | [`to_local_point_2d`](#to_local_point_2d) |
| `to_global_point_3d` | [`to_global_point_3d`](#to_global_point_3d) |
| `to_local_point_3d` | [`to_local_point_3d`](#to_local_point_3d) |
| `to_global_transform_2d` | [`to_global_transform_2d`](#to_global_transform_2d) |
| `to_local_transform_2d` | [`to_local_transform_2d`](#to_local_transform_2d) |
| `to_global_transform_3d` | [`to_global_transform_3d`](#to_global_transform_3d) |
| `to_local_transform_3d` | [`to_local_transform_3d`](#to_local_transform_3d) |
| `mesh_instance_surface_at_global_point` | [`mesh_instance_surface_at_global_point`](#mesh_instance_surface_at_global_point) |
| `mesh_instance_surface_on_global_ray` | [`mesh_instance_surface_on_global_ray`](#mesh_instance_surface_on_global_ray) |
| `mesh_instance_surfaces_on_global_rays` | [`mesh_instance_surfaces_on_global_rays`](#mesh_instance_surfaces_on_global_rays) |
| `mesh_instance_material_regions` | [`mesh_instance_material_regions`](#mesh_instance_material_regions) |
| `mesh_data_surface_at_local_point` | [`mesh_data_surface_at_local_point`](#mesh_data_surface_at_local_point) |
| `mesh_data_surface_on_local_ray` | [`mesh_data_surface_on_local_ray`](#mesh_data_surface_on_local_ray) |
| `mesh_data_surface_regions` | [`mesh_data_surface_regions`](#mesh_data_surface_regions) |
| `with_node_mut` | [`with_node_mut`](#with_node_mut) |
| `with_node` | [`with_node`](#with_node) |
| `with_base_node` | [`with_base_node`](#with_base_node) |
| `with_base_node_mut` | [`with_base_node_mut`](#with_base_node_mut) |
| `create_node` | [`create_node`](#create_node) |
| `spawn` | [`spawn`](#spawn) |
| `node_collection` | [`node_collection`](#node_collection) |
| `create_nodes` | [`create_nodes`](#create_nodes) |
| `find_node` | [`find_node`](#find_node) |
| `descendants` | [`descendants`](#descendants) |
| `set_tree_visible` | [`set_tree_visible`](#set_tree_visible) |
| `broadcast_var` | [`broadcast_var`](#broadcast_var) |
| `look_at_3d` | [`look_at_3d`](#look_at_3d) |
| `get_node_name` | [`get_node_name`](#get_node_name) |
| `set_node_name` | [`set_node_name`](#set_node_name) |
| `get_skeleton_bone_name` | [`get_skeleton_bone_name`](#get_skeleton_bone_name) |
| `get_skeleton_bone_index` | [`get_skeleton_bone_index`](#get_skeleton_bone_index) |
| `set_ui_rotation` | [`set_ui_rotation`](#set_ui_rotation) |
| `bind_locale_text` | [`bind_locale_text`](#bind_locale_text) |
| `bind_locale_placeholder` | [`bind_locale_placeholder`](#bind_locale_placeholder) |
| `get_node_parent_id` | [`get_node_parent_id`](#get_node_parent_id) |
| `get_node_children_ids` | [`get_node_children_ids`](#get_node_children_ids) |
| `get_children` | [`get_children`](#get_children) |
| `get_child` | [`get_child`](#get_child) |
| `get_node_type` | [`get_node_type`](#get_node_type) |
| `reparent` | [`reparent`](#reparent) |
| `force_rerender` | [`force_rerender`](#force_rerender) |
| `reparent_multi` | [`reparent_multi`](#reparent_multi) |
| `remove_node` | [`remove_node`](#remove_node) |
| `get_global_transform_2d` | [`get_global_transform_2d`](#get_global_transform_2d) |
| `get_global_transform_3d` | [`get_global_transform_3d`](#get_global_transform_3d) |
| `get_local_transform_2d` | [`get_local_transform_2d`](#get_local_transform_2d) |
| `get_local_transform_3d` | [`get_local_transform_3d`](#get_local_transform_3d) |
| `set_global_transform_2d` | [`set_global_transform_2d`](#set_global_transform_2d) |
| `set_global_transform_3d` | [`set_global_transform_3d`](#set_global_transform_3d) |
| `set_local_transform_2d` | [`set_local_transform_2d`](#set_local_transform_2d) |
| `set_local_transform_3d` | [`set_local_transform_3d`](#set_local_transform_3d) |
| `get_local_pos_2d` | [`get_local_pos_2d`](#get_local_pos_2d) |
| `get_local_pos_3d` | [`get_local_pos_3d`](#get_local_pos_3d) |
| `set_local_pos_2d` | [`set_local_pos_2d`](#set_local_pos_2d) |
| `set_local_pos_3d` | [`set_local_pos_3d`](#set_local_pos_3d) |
| `get_global_pos_2d` | [`get_global_pos_2d`](#get_global_pos_2d) |
| `get_global_pos_3d` | [`get_global_pos_3d`](#get_global_pos_3d) |
| `set_global_pos_2d` | [`set_global_pos_2d`](#set_global_pos_2d) |
| `set_global_pos_3d` | [`set_global_pos_3d`](#set_global_pos_3d) |
| `get_local_rot_2d` | [`get_local_rot_2d`](#get_local_rot_2d) |
| `get_local_rot_3d` | [`get_local_rot_3d`](#get_local_rot_3d) |
| `set_local_rot_2d` | [`set_local_rot_2d`](#set_local_rot_2d) |
| `set_local_rot_3d` | [`set_local_rot_3d`](#set_local_rot_3d) |
| `get_global_rot_2d` | [`get_global_rot_2d`](#get_global_rot_2d) |
| `get_global_rot_3d` | [`get_global_rot_3d`](#get_global_rot_3d) |
| `set_global_rot_2d` | [`set_global_rot_2d`](#set_global_rot_2d) |
| `set_global_rot_3d` | [`set_global_rot_3d`](#set_global_rot_3d) |
| `get_local_scale_2d` | [`get_local_scale_2d`](#get_local_scale_2d) |
| `get_local_scale_3d` | [`get_local_scale_3d`](#get_local_scale_3d) |
| `set_local_scale_2d` | [`set_local_scale_2d`](#set_local_scale_2d) |
| `set_local_scale_3d` | [`set_local_scale_3d`](#set_local_scale_3d) |
| `get_global_scale_2d` | [`get_global_scale_2d`](#get_global_scale_2d) |
| `get_global_scale_3d` | [`get_global_scale_3d`](#get_global_scale_3d) |
| `set_global_scale_2d` | [`set_global_scale_2d`](#set_global_scale_2d) |
| `set_global_scale_3d` | [`set_global_scale_3d`](#set_global_scale_3d) |
| `to_global_point_2d` | [`to_global_point_2d`](#to_global_point_2d) |
| `to_local_point_2d` | [`to_local_point_2d`](#to_local_point_2d) |
| `to_global_point_3d` | [`to_global_point_3d`](#to_global_point_3d) |
| `to_local_point_3d` | [`to_local_point_3d`](#to_local_point_3d) |
| `to_global_transform_2d` | [`to_global_transform_2d`](#to_global_transform_2d) |
| `to_local_transform_2d` | [`to_local_transform_2d`](#to_local_transform_2d) |
| `to_global_transform_3d` | [`to_global_transform_3d`](#to_global_transform_3d) |
| `to_local_transform_3d` | [`to_local_transform_3d`](#to_local_transform_3d) |
| `get_node_tags` | [`get_node_tags`](#get_node_tags) |
| `tag_set` | [`tag_set`](#tag_set) |
| `tag_add` | [`tag_add`](#tag_add) |
| `tag_remove` | [`tag_remove`](#tag_remove) |

## Purpose

The nodes module is the workhorse for touching the scene graph at runtime. It
creates and destroys nodes, moves/rotates/scales them (in 2D or 3D, in local or
global space), walks and edits the parent/child hierarchy, sets names and tags,
and reads or writes a node's typed fields through `with_node` / `with_node_mut`.
Almost any gameplay that repositions a character, spawns a projectile, picks up
an item, or aims a turret goes through here.

Transform helpers come in matched pairs — `get_*` / `set_*`, `local` / `global`,
`2d` / `3d` — plus `to_global_*` / `to_local_*` for converting points and
transforms between a node's space and the world.

## Use Cases

| Situation | Choice | Why | Tradeoff |
| --- | --- | --- | --- |
| Script edits its own known camera/sprite/body | `with_node_mut!` + `ctx.id` | Typed closure exposes the concrete node fields | Returns no value on missing ID or wrong node type |
| Script reads shared base fields from an unknown concrete node | `with_base_node!` | Base dispatch avoids guessing its concrete type | Only base fields are available |
| Scene has one fixed target | state `NodeID` | Scene wiring gives a stable explicit dependency | Target may later be removed; guard every access |
| Projectile or pickup appears at runtime | `spawn!` under an owned parent | Creation, name, tags, and initial data stay together | Caller owns later cleanup and any attached script setup |
| Item changes hierarchy | `reparent!` then local transform | Parent defines the new transform space | Preserving world pose requires reading/writing the intended transform explicitly |
| System needs all current enemies | tags + query | Membership follows runtime tags/spawns | Query is weaker and costlier than a known ref |
| Muzzle point must enter world space | `to_global_point_3d!` | Conversion includes the node hierarchy | Missing/wrong-dimensional node returns the helper's empty value |
| Node dies | `remove_node!` | Runtime removes the owned scene object | Stored IDs become stale; users must tolerate absence |

## Context

- Script context path: `ctx.run`
- Module access: `ctx.run.Nodes()`
- Lifecycle examples stay inside `lifecycle!` because script hooks get `API` from the macro expansion.

## Practical Example

A simple homing turret: keep its fixed target as a scene-injected `NodeID`,
rotate toward it, and fire a bullet on a cooldown.

```rust
#[State]
struct TurretState {
    #[expose]
    #[node_ref(Node3D)]
    pub target: Option<NodeID>,

}

lifecycle!({
    fn on_all_init(&self, ctx: &mut ScriptContext<'_, API>) {
        signal_connect!(ctx.run, ctx.id, timer_finished!("fire"), func!("fire"));
        timer_start!(ctx.run, Duration::from_millis(500), "fire");
    }

    fn on_update(&self, ctx: &mut ScriptContext<'_, API>) {
        let Some(target_id) = with_state!(ctx.run, TurretState, ctx.id, |s| s.target) else {
            return;
        };
        let Some(target) = get_global_pos_3d!(ctx.run, target_id) else {
            return;
        };
        let _ = look_at_3d!(ctx.run, ctx.id, target);
    }
});

methods!({
    fn fire(&self, ctx: &mut ScriptContext<'_, API>) {
        if let Some(muzzle) = get_global_pos_3d!(ctx.run, ctx.id) {
            let bullet = spawn!(ctx.run, Node3D, "Bullet", tags!["bullet"], ctx.id, |node| {
                let _ = node; // set velocity, mesh, lifetime, etc.
            });
            let _ = set_global_pos_3d!(ctx.run, bullet, muzzle);
        }
        timer_start!(ctx.run, Duration::from_millis(500), "fire");
    }
});
```

## API Reference

### `create`

| Field | Detail |
| --- | --- |
| Access | `ctx.run.Nodes()` |
| Signature | `pub fn create<T>(&mut self) -> NodeID where T: Default + Into<SceneNodeData>,` |
| Params | `&mut self` |
| Returns | `NodeID where T: Default + Into<SceneNodeData>,` |
| Use when | Use `create` to create on the scene graph; guard stale IDs and concrete/base type mismatches. |
| Fails when / edge behavior | Has no optional/error return; `create` returns the documented value directly. |

### `create_nodes`

| Field | Detail |
| --- | --- |
| Access | `ctx.run.Nodes()` |
| Signature | `pub fn create_nodes<'a, B>(&mut self, requests: B, parent_id: NodeID) -> Vec<NodeID> where B: IntoNodeCreateBatch<'a>` |
| Params | `&mut self, requests: NodeCollection / NodeSpec slice, parent_id: NodeID` |
| Returns | `Vec<NodeID>` |
| Use when | Use `create_nodes` to create nodes on the scene graph; guard stale IDs and concrete/base type mismatches. |
| Fails when / edge behavior | Returns an empty vector when `create_nodes` finds no values; callers must treat zero results as normal. |

### `with_node_mut`

| Field | Detail |
| --- | --- |
| Access | `ctx.run.Nodes()` |
| Signature | `pub fn with_node_mut<T, V, F>(&mut self, id: NodeID, f: F) -> Option<V> where T: NodeTypeDispatch, F: FnOnce(&mut T) -> V,` |
| Params | `&mut self, id: NodeID, f: F) -> Option<V> where T: NodeTypeDispatch, F: FnOnce(&mut T` |
| Returns | `Option<V> where T: NodeTypeDispatch, F: FnOnce(&mut T) -> V,` |
| Use when | Use `with_node_mut` to with node mut on the scene graph; guard stale IDs and concrete/base type mismatches. |
| Fails when / edge behavior | Returns `None` when `with_node_mut` cannot produce a value for the supplied target or inputs. |

### `with_node`

| Field | Detail |
| --- | --- |
| Access | `ctx.run.Nodes()` |
| Signature | `pub fn with_node<T, V: Clone + Default>( &mut self, node_id: NodeID, f: impl FnOnce(&T) -> V, ) -> V where T: NodeTypeDispatch,` |
| Params | `&mut self, node_id: NodeID, f: impl FnOnce(&T) -> V,` |
| Returns | `V, ) -> V where T: NodeTypeDispatch,` |
| Use when | Use `with_node` to with node on the scene graph; guard stale IDs and concrete/base type mismatches. |
| Fails when / edge behavior | Uses `with_node`'s documented return type as its failure channel; no extra wrapper fallback or coercion is added. |

### `with_base_node`

| Field | Detail |
| --- | --- |
| Access | `ctx.run.Nodes()` |
| Signature | `pub fn with_base_node<T, V, F>(&mut self, id: NodeID, f: F) -> Option<V> where T: NodeBaseDispatch, F: FnOnce(&T) -> V,` |
| Params | `&mut self, id: NodeID, f: F) -> Option<V> where T: NodeBaseDispatch, F: FnOnce(&T` |
| Returns | `Option<V> where T: NodeBaseDispatch, F: FnOnce(&T) -> V,` |
| Use when | Use `with_base_node` to with base node on the scene graph; guard stale IDs and concrete/base type mismatches. |
| Fails when / edge behavior | Returns `None` when `with_base_node` cannot produce a value for the supplied target or inputs. |

### `with_base_node_mut`

| Field | Detail |
| --- | --- |
| Access | `ctx.run.Nodes()` |
| Signature | `pub fn with_base_node_mut<T, V, F>(&mut self, id: NodeID, f: F) -> Option<V> where T: NodeBaseDispatch, F: FnOnce(&mut T) -> V,` |
| Params | `&mut self, id: NodeID, f: F) -> Option<V> where T: NodeBaseDispatch, F: FnOnce(&mut T` |
| Returns | `Option<V> where T: NodeBaseDispatch, F: FnOnce(&mut T) -> V,` |
| Use when | Use `with_base_node_mut` to with base node mut on the scene graph; guard stale IDs and concrete/base type mismatches. |
| Fails when / edge behavior | Returns `None` when `with_base_node_mut` cannot produce a value for the supplied target or inputs. |

### `get_node_name`

| Field | Detail |
| --- | --- |
| Access | `ctx.run.Nodes()` |
| Signature | `pub fn get_node_name(&mut self, node_id: NodeID) -> Option<Cow<'static, str>>` |
| Params | `&mut self, node_id: NodeID` |
| Returns | `Option<Cow<'static, str>>` |
| Use when | Use `get_node_name` to get node name on the scene graph; guard stale IDs and concrete/base type mismatches. |
| Fails when / edge behavior | Returns `None` when `get_node_name` cannot produce a value for the supplied target or inputs. |

### `set_node_name`

| Field | Detail |
| --- | --- |
| Access | `ctx.run.Nodes()` |
| Signature | `pub fn set_node_name<S>(&mut self, node_id: NodeID, name: S) -> bool where S: Into<Cow<'static, str>>,` |
| Params | `&mut self, node_id: NodeID, name: S` |
| Returns | `bool where S: Into<Cow<'static, str>>,` |
| Use when | Use `set_node_name` to set node name on the scene graph; guard stale IDs and concrete/base type mismatches. |
| Fails when / edge behavior | Returns `false` when `set_node_name` cannot apply to the supplied target or inputs; `true` confirms success. |

### `get_skeleton_bone_name`

| Field | Detail |
| --- | --- |
| Access | `ctx.run.Nodes()` |
| Signature | `pub fn get_skeleton_bone_name( &mut self, skeleton_id: NodeID, bone_index: usize, ) -> Option<Cow<'static, str>>` |
| Params | `&mut self, skeleton_id: NodeID, bone_index: usize,` |
| Returns | `Option<Cow<'static, str>>` |
| Use when | Use `get_skeleton_bone_name` to get skeleton bone name on the scene graph; guard stale IDs and concrete/base type mismatches. |
| Fails when / edge behavior | Returns `None` when `get_skeleton_bone_name` cannot produce a value for the supplied target or inputs. |

### `get_skeleton_bone_index`

| Field | Detail |
| --- | --- |
| Access | `ctx.run.Nodes()` |
| Signature | `pub fn get_skeleton_bone_index<S>(&mut self, skeleton_id: NodeID, bone_name: S) -> Option<usize> where S: AsRef<str>,` |
| Params | `&mut self, skeleton_id: NodeID, bone_name: S` |
| Returns | `Option<usize> where S: AsRef<str>,` |
| Use when | Use `get_skeleton_bone_index` to get skeleton bone index on the scene graph; guard stale IDs and concrete/base type mismatches. |
| Fails when / edge behavior | Returns `None` when `get_skeleton_bone_index` cannot produce a value for the supplied target or inputs. |

### `set_ui_rotation`

| Field | Detail |
| --- | --- |
| Access | `ctx.run.Nodes()` |
| Signature | `pub fn set_ui_rotation(&mut self, node_id: NodeID, rotation: f32) -> bool` |
| Params | `&mut self, node_id: NodeID, rotation: f32` |
| Returns | `bool` |
| Use when | Use `set_ui_rotation` to set ui rotation on the scene graph; guard stale IDs and concrete/base type mismatches. |
| Fails when / edge behavior | Returns `false` when `set_ui_rotation` cannot apply to the supplied target or inputs; `true` confirms success. |

### `bind_locale_text`

| Field | Detail |
| --- | --- |
| Access | `ctx.run.Nodes()` |
| Signature | `pub fn bind_locale_text<S>(&mut self, node_id: NodeID, key: S) -> bool where S: AsRef<str>,` |
| Params | `&mut self, node_id: NodeID, key: S` |
| Returns | `bool where S: AsRef<str>,` |
| Use when | Bind locale text on `UiLabel`, `Label2D`, or `Label3D`. |
| Fails when / edge behavior | Returns `false` when backing runtime data is missing, stale, or the target type does not support locale text. |

### `bind_locale_placeholder`

| Field | Detail |
| --- | --- |
| Access | `ctx.run.Nodes()` |
| Signature | `pub fn bind_locale_placeholder<S>(&mut self, node_id: NodeID, key: S) -> bool where S: AsRef<str>,` |
| Params | `&mut self, node_id: NodeID, key: S` |
| Returns | `bool where S: AsRef<str>,` |
| Use when | Use `bind_locale_placeholder` to bind locale placeholder on the scene graph; guard stale IDs and concrete/base type mismatches. |
| Fails when / edge behavior | Returns `false` when `bind_locale_placeholder` cannot apply to the supplied target or inputs; `true` confirms success. |

### `get_node_parent_id`

| Field | Detail |
| --- | --- |
| Access | `ctx.run.Nodes()` |
| Signature | `pub fn get_node_parent_id(&mut self, node_id: NodeID) -> Option<NodeID>` |
| Params | `&mut self, node_id: NodeID` |
| Returns | `Option<NodeID>` |
| Use when | Use `get_node_parent_id` to get node parent id on the scene graph; guard stale IDs and concrete/base type mismatches. |
| Fails when / edge behavior | Returns `None` when `get_node_parent_id` cannot produce a value for the supplied target or inputs. |

### `get_node_children_ids`

| Field | Detail |
| --- | --- |
| Access | `ctx.run.Nodes()` |
| Signature | `pub fn get_node_children_ids(&mut self, node_id: NodeID) -> Option<Vec<NodeID>>` |
| Params | `&mut self, node_id: NodeID` |
| Returns | `Option<Vec<NodeID>>` |
| Use when | Use `get_node_children_ids` to get node children ids on the scene graph; guard stale IDs and concrete/base type mismatches. |
| Fails when / edge behavior | Returns `None` when `get_node_children_ids` cannot produce a value for the supplied target or inputs. |

### `get_children`

| Field | Detail |
| --- | --- |
| Access | `ctx.run.Nodes()` |
| Signature | `pub fn get_children(&mut self, node_id: NodeID) -> Vec<NodeID>` |
| Params | `&mut self, node_id: NodeID` |
| Returns | `Vec<NodeID>` |
| Use when | Use `get_children` to get children on the scene graph; guard stale IDs and concrete/base type mismatches. |
| Fails when / edge behavior | Returns an empty vector when `get_children` finds no values; callers must treat zero results as normal. |

### `get_child_at`

| Field | Detail |
| --- | --- |
| Access | `ctx.run.Nodes()` |
| Signature | `pub fn get_child_at(&mut self, parent_id: NodeID, index: usize) -> Option<NodeID>` |
| Params | `&mut self, parent_id: NodeID, index: usize` |
| Returns | `Option<NodeID>` |
| Use when | Use `get_child_at` to get child at on the scene graph; guard stale IDs and concrete/base type mismatches. |
| Fails when / edge behavior | Returns `None` when `get_child_at` cannot produce a value for the supplied target or inputs. |

### `get_child_by_name`

| Field | Detail |
| --- | --- |
| Access | `ctx.run.Nodes()` |
| Signature | `pub fn get_child_by_name<S>(&mut self, parent_id: NodeID, name: S) -> Option<NodeID> where S: AsRef<str>,` |
| Params | `&mut self, parent_id: NodeID, name: S` |
| Returns | `Option<NodeID> where S: AsRef<str>,` |
| Use when | Use `get_child_by_name` to get child by name on the scene graph; guard stale IDs and concrete/base type mismatches. |
| Fails when / edge behavior | Returns `None` when `get_child_by_name` cannot produce a value for the supplied target or inputs. |

### `get_children_by_name`

| Field | Detail |
| --- | --- |
| Access | `ctx.run.Nodes()` |
| Signature | `pub fn get_children_by_name<S>(&mut self, parent_id: NodeID, name: S) -> Vec<NodeID> where S: AsRef<str>,` |
| Params | `&mut self, parent_id: NodeID, name: S` |
| Returns | `Vec<NodeID> where S: AsRef<str>,` |
| Use when | Use `get_children_by_name` to get children by name on the scene graph; guard stale IDs and concrete/base type mismatches. |
| Fails when / edge behavior | Returns an empty vector when `get_children_by_name` finds no values; callers must treat zero results as normal. |

### `get_child`

| Field | Detail |
| --- | --- |
| Access | `ctx.run.Nodes()` |
| Signature | `pub fn get_child<T>(&mut self, parent_id: NodeID, selector: T) -> Option<NodeID> where T: IntoChildSelector,` |
| Params | `&mut self, parent_id: NodeID, selector: T` |
| Returns | `Option<NodeID> where T: IntoChildSelector,` |
| Use when | Use `get_child` to get child on the scene graph; guard stale IDs and concrete/base type mismatches. |
| Fails when / edge behavior | Returns `None` when `get_child` cannot produce a value for the supplied target or inputs. |

### `get_node_type`

| Field | Detail |
| --- | --- |
| Access | `ctx.run.Nodes()` |
| Signature | `pub fn get_node_type(&mut self, node_id: NodeID) -> Option<NodeType>` |
| Params | `&mut self, node_id: NodeID` |
| Returns | `Option<NodeType>` |
| Use when | Use `get_node_type` to get node type on the scene graph; guard stale IDs and concrete/base type mismatches. |
| Fails when / edge behavior | Returns `None` when `get_node_type` cannot produce a value for the supplied target or inputs. |

### `reparent`

| Field | Detail |
| --- | --- |
| Access | `ctx.run.Nodes()` |
| Signature | `pub fn reparent(&mut self, parent_id: NodeID, child_id: NodeID) -> bool` |
| Params | `&mut self, parent_id: NodeID, child_id: NodeID` |
| Returns | `bool` |
| Use when | Use `reparent` to reparent on the scene graph; guard stale IDs and concrete/base type mismatches. |
| Fails when / edge behavior | Returns `false` when `reparent` cannot apply to the supplied target or inputs; `true` confirms success. |

### `force_rerender`

| Field | Detail |
| --- | --- |
| Access | `ctx.run.Nodes()` |
| Signature | `pub fn force_rerender(&mut self, root_id: NodeID) -> bool` |
| Params | `&mut self, root_id: NodeID` |
| Returns | `bool` |
| Use when | Use `force_rerender` to force rerender on the scene graph; guard stale IDs and concrete/base type mismatches. |
| Fails when / edge behavior | Returns `false` when `force_rerender` cannot apply to the supplied target or inputs; `true` confirms success. |

### `mark_needs_rerender`

| Field | Detail |
| --- | --- |
| Access | `ctx.run.Nodes()` |
| Signature | `pub fn mark_needs_rerender(&mut self, node_id: NodeID) -> bool` |
| Params | `&mut self, node_id: NodeID` |
| Returns | `bool` |
| Use when | Use `mark_needs_rerender` to mark needs rerender on the scene graph; guard stale IDs and concrete/base type mismatches. |
| Fails when / edge behavior | Returns `false` when `mark_needs_rerender` cannot apply to the supplied target or inputs; `true` confirms success. |

### `reparent_multi`

| Field | Detail |
| --- | --- |
| Access | `ctx.run.Nodes()` |
| Signature | `pub fn reparent_multi<I>(&mut self, parent_id: NodeID, child_ids: I) -> usize where I: IntoIterator<Item = NodeID>,` |
| Params | `&mut self, parent_id: NodeID, child_ids: I` |
| Returns | `usize where I: IntoIterator<Item = NodeID>,` |
| Use when | Use `reparent_multi` to reparent multi on the scene graph; guard stale IDs and concrete/base type mismatches. |
| Fails when / edge behavior | Has no optional/error return; `reparent_multi` returns the documented value directly. |

### `remove_node`

| Field | Detail |
| --- | --- |
| Access | `ctx.run.Nodes()` |
| Signature | `pub fn remove_node(&mut self, node_id: NodeID) -> bool` |
| Params | `&mut self, node_id: NodeID` |
| Returns | `bool` |
| Use when | Use `remove_node` to remove node on the scene graph; guard stale IDs and concrete/base type mismatches. |
| Fails when / edge behavior | Returns `false` when `remove_node` cannot apply to the supplied target or inputs; `true` confirms success. |

### `get_node_tags`

| Field | Detail |
| --- | --- |
| Access | `ctx.run.Nodes()` |
| Signature | `pub fn get_node_tags(&mut self, node_id: NodeID) -> Option<Vec<Cow<'static, str>>>` |
| Params | `&mut self, node_id: NodeID` |
| Returns | `Option<Vec<Cow<'static, str>>>` |
| Use when | Use `get_node_tags` to get node tags on the scene graph; guard stale IDs and concrete/base type mismatches. |
| Fails when / edge behavior | Returns `None` when `get_node_tags` cannot produce a value for the supplied target or inputs. |

### `tag_set`

| Field | Detail |
| --- | --- |
| Access | `ctx.run.Nodes()` |
| Signature | `pub fn tag_set<T>(&mut self, node_id: NodeID, tags: Option<T>) -> bool where T: IntoNodeTags,` |
| Params | `&mut self, node_id: NodeID, tags: Option<T>` |
| Returns | `bool where T: IntoNodeTags,` |
| Use when | Use `tag_set` to tag set on the scene graph; guard stale IDs and concrete/base type mismatches. |
| Fails when / edge behavior | Returns `false` when `tag_set` cannot apply to the supplied target or inputs; `true` confirms success. |

### `add_node_tag`

| Field | Detail |
| --- | --- |
| Access | `ctx.run.Nodes()` |
| Signature | `pub fn add_node_tag<T>(&mut self, node_id: NodeID, tag: T) -> bool where T: IntoNodeTag,` |
| Params | `&mut self, node_id: NodeID, tag: T` |
| Returns | `bool where T: IntoNodeTag,` |
| Use when | Use `add_node_tag` to add node tag on the scene graph; guard stale IDs and concrete/base type mismatches. |
| Fails when / edge behavior | Returns `false` when `add_node_tag` cannot apply to the supplied target or inputs; `true` confirms success. |

### `add_node_tags`

| Field | Detail |
| --- | --- |
| Access | `ctx.run.Nodes()` |
| Signature | `pub fn add_node_tags<T>(&mut self, node_id: NodeID, tags: T) -> bool where T: IntoNodeTags,` |
| Params | `&mut self, node_id: NodeID, tags: T` |
| Returns | `bool where T: IntoNodeTags,` |
| Use when | Use `add_node_tags` to add node tags on the scene graph; guard stale IDs and concrete/base type mismatches. |
| Fails when / edge behavior | Returns `false` when `add_node_tags` cannot apply to the supplied target or inputs; `true` confirms success. |

### `remove_node_tag`

| Field | Detail |
| --- | --- |
| Access | `ctx.run.Nodes()` |
| Signature | `pub fn remove_node_tag<T>(&mut self, node_id: NodeID, tag: T) -> bool where T: IntoTagID,` |
| Params | `&mut self, node_id: NodeID, tag: T` |
| Returns | `bool where T: IntoTagID,` |
| Use when | Use `remove_node_tag` to remove node tag on the scene graph; guard stale IDs and concrete/base type mismatches. |
| Fails when / edge behavior | Returns `false` when `remove_node_tag` cannot apply to the supplied target or inputs; `true` confirms success. |

### `get_global_transform_2d`

| Field | Detail |
| --- | --- |
| Access | `ctx.run.Nodes()` |
| Signature | `pub fn get_global_transform_2d(&mut self, node_id: NodeID) -> Option<Transform2D>` |
| Params | `&mut self, node_id: NodeID` |
| Returns | `Option<Transform2D>` |
| Use when | Use `get_global_transform_2d` to get global transform 2d on the scene graph; guard stale IDs and concrete/base type mismatches. |
| Fails when / edge behavior | Returns `None` when `get_global_transform_2d` cannot produce a value for the supplied target or inputs. |

### `get_global_transform_3d`

| Field | Detail |
| --- | --- |
| Access | `ctx.run.Nodes()` |
| Signature | `pub fn get_global_transform_3d(&mut self, node_id: NodeID) -> Option<Transform3D>` |
| Params | `&mut self, node_id: NodeID` |
| Returns | `Option<Transform3D>` |
| Use when | Use `get_global_transform_3d` to get global transform 3d on the scene graph; guard stale IDs and concrete/base type mismatches. |
| Fails when / edge behavior | Returns `None` when `get_global_transform_3d` cannot produce a value for the supplied target or inputs. |

### `get_local_transform_2d`

| Field | Detail |
| --- | --- |
| Access | `ctx.run.Nodes()` |
| Signature | `pub fn get_local_transform_2d(&mut self, node_id: NodeID) -> Option<Transform2D>` |
| Params | `&mut self, node_id: NodeID` |
| Returns | `Option<Transform2D>` |
| Use when | Use `get_local_transform_2d` to get local transform 2d on the scene graph; guard stale IDs and concrete/base type mismatches. |
| Fails when / edge behavior | Returns `None` when `get_local_transform_2d` cannot produce a value for the supplied target or inputs. |

### `get_local_transform_3d`

| Field | Detail |
| --- | --- |
| Access | `ctx.run.Nodes()` |
| Signature | `pub fn get_local_transform_3d(&mut self, node_id: NodeID) -> Option<Transform3D>` |
| Params | `&mut self, node_id: NodeID` |
| Returns | `Option<Transform3D>` |
| Use when | Use `get_local_transform_3d` to get local transform 3d on the scene graph; guard stale IDs and concrete/base type mismatches. |
| Fails when / edge behavior | Returns `None` when `get_local_transform_3d` cannot produce a value for the supplied target or inputs. |

### `set_local_transform_2d`

| Field | Detail |
| --- | --- |
| Access | `ctx.run.Nodes()` |
| Signature | `pub fn set_local_transform_2d(&mut self, node_id: NodeID, transform: Transform2D) -> bool` |
| Params | `&mut self, node_id: NodeID, transform: Transform2D` |
| Returns | `bool` |
| Use when | Use `set_local_transform_2d` to set local transform 2d on the scene graph; guard stale IDs and concrete/base type mismatches. |
| Fails when / edge behavior | Returns `false` when `set_local_transform_2d` cannot apply to the supplied target or inputs; `true` confirms success. |

### `set_local_transform_3d`

| Field | Detail |
| --- | --- |
| Access | `ctx.run.Nodes()` |
| Signature | `pub fn set_local_transform_3d(&mut self, node_id: NodeID, transform: Transform3D) -> bool` |
| Params | `&mut self, node_id: NodeID, transform: Transform3D` |
| Returns | `bool` |
| Use when | Use `set_local_transform_3d` to set local transform 3d on the scene graph; guard stale IDs and concrete/base type mismatches. |
| Fails when / edge behavior | Returns `false` when `set_local_transform_3d` cannot apply to the supplied target or inputs; `true` confirms success. |

### `set_global_transform_2d`

| Field | Detail |
| --- | --- |
| Access | `ctx.run.Nodes()` |
| Signature | `pub fn set_global_transform_2d(&mut self, node_id: NodeID, global: Transform2D) -> bool` |
| Params | `&mut self, node_id: NodeID, global: Transform2D` |
| Returns | `bool` |
| Use when | Use `set_global_transform_2d` to set global transform 2d on the scene graph; guard stale IDs and concrete/base type mismatches. |
| Fails when / edge behavior | Returns `false` when `set_global_transform_2d` cannot apply to the supplied target or inputs; `true` confirms success. |

### `set_global_transform_3d`

| Field | Detail |
| --- | --- |
| Access | `ctx.run.Nodes()` |
| Signature | `pub fn set_global_transform_3d(&mut self, node_id: NodeID, global: Transform3D) -> bool` |
| Params | `&mut self, node_id: NodeID, global: Transform3D` |
| Returns | `bool` |
| Use when | Use `set_global_transform_3d` to set global transform 3d on the scene graph; guard stale IDs and concrete/base type mismatches. |
| Fails when / edge behavior | Returns `false` when `set_global_transform_3d` cannot apply to the supplied target or inputs; `true` confirms success. |

### `get_local_pos_2d`

| Field | Detail |
| --- | --- |
| Access | `ctx.run.Nodes()` |
| Signature | `pub fn get_local_pos_2d(&mut self, node_id: NodeID) -> Option<Vector2>` |
| Params | `&mut self, node_id: NodeID` |
| Returns | `Option<Vector2>` |
| Use when | Use `get_local_pos_2d` to get local pos 2d on the scene graph; guard stale IDs and concrete/base type mismatches. |
| Fails when / edge behavior | Returns `None` when `get_local_pos_2d` cannot produce a value for the supplied target or inputs. |

### `get_local_pos_3d`

| Field | Detail |
| --- | --- |
| Access | `ctx.run.Nodes()` |
| Signature | `pub fn get_local_pos_3d(&mut self, node_id: NodeID) -> Option<Vector3>` |
| Params | `&mut self, node_id: NodeID` |
| Returns | `Option<Vector3>` |
| Use when | Use `get_local_pos_3d` to get local pos 3d on the scene graph; guard stale IDs and concrete/base type mismatches. |
| Fails when / edge behavior | Returns `None` when `get_local_pos_3d` cannot produce a value for the supplied target or inputs. |

### `set_local_pos_2d`

| Field | Detail |
| --- | --- |
| Access | `ctx.run.Nodes()` |
| Signature | `pub fn set_local_pos_2d(&mut self, node_id: NodeID, pos: Vector2) -> bool` |
| Params | `&mut self, node_id: NodeID, pos: Vector2` |
| Returns | `bool` |
| Use when | Use `set_local_pos_2d` to set local pos 2d on the scene graph; guard stale IDs and concrete/base type mismatches. |
| Fails when / edge behavior | Returns `false` when `set_local_pos_2d` cannot apply to the supplied target or inputs; `true` confirms success. |

### `set_local_pos_3d`

| Field | Detail |
| --- | --- |
| Access | `ctx.run.Nodes()` |
| Signature | `pub fn set_local_pos_3d(&mut self, node_id: NodeID, pos: Vector3) -> bool` |
| Params | `&mut self, node_id: NodeID, pos: Vector3` |
| Returns | `bool` |
| Use when | Use `set_local_pos_3d` to set local pos 3d on the scene graph; guard stale IDs and concrete/base type mismatches. |
| Fails when / edge behavior | Returns `false` when `set_local_pos_3d` cannot apply to the supplied target or inputs; `true` confirms success. |

### `get_global_pos_2d`

| Field | Detail |
| --- | --- |
| Access | `ctx.run.Nodes()` |
| Signature | `pub fn get_global_pos_2d(&mut self, node_id: NodeID) -> Option<Vector2>` |
| Params | `&mut self, node_id: NodeID` |
| Returns | `Option<Vector2>` |
| Use when | Use `get_global_pos_2d` to get global pos 2d on the scene graph; guard stale IDs and concrete/base type mismatches. |
| Fails when / edge behavior | Returns `None` when `get_global_pos_2d` cannot produce a value for the supplied target or inputs. |

### `get_global_pos_3d`

| Field | Detail |
| --- | --- |
| Access | `ctx.run.Nodes()` |
| Signature | `pub fn get_global_pos_3d(&mut self, node_id: NodeID) -> Option<Vector3>` |
| Params | `&mut self, node_id: NodeID` |
| Returns | `Option<Vector3>` |
| Use when | Use `get_global_pos_3d` to get global pos 3d on the scene graph; guard stale IDs and concrete/base type mismatches. |
| Fails when / edge behavior | Returns `None` when `get_global_pos_3d` cannot produce a value for the supplied target or inputs. |

### `set_global_pos_2d`

| Field | Detail |
| --- | --- |
| Access | `ctx.run.Nodes()` |
| Signature | `pub fn set_global_pos_2d(&mut self, node_id: NodeID, pos: Vector2) -> bool` |
| Params | `&mut self, node_id: NodeID, pos: Vector2` |
| Returns | `bool` |
| Use when | Use `set_global_pos_2d` to set global pos 2d on the scene graph; guard stale IDs and concrete/base type mismatches. |
| Fails when / edge behavior | Returns `false` when `set_global_pos_2d` cannot apply to the supplied target or inputs; `true` confirms success. |

### `set_global_pos_3d`

| Field | Detail |
| --- | --- |
| Access | `ctx.run.Nodes()` |
| Signature | `pub fn set_global_pos_3d(&mut self, node_id: NodeID, pos: Vector3) -> bool` |
| Params | `&mut self, node_id: NodeID, pos: Vector3` |
| Returns | `bool` |
| Use when | Use `set_global_pos_3d` to set global pos 3d on the scene graph; guard stale IDs and concrete/base type mismatches. |
| Fails when / edge behavior | Returns `false` when `set_global_pos_3d` cannot apply to the supplied target or inputs; `true` confirms success. |

### `get_local_rot_2d`

| Field | Detail |
| --- | --- |
| Access | `ctx.run.Nodes()` |
| Signature | `pub fn get_local_rot_2d(&mut self, node_id: NodeID) -> Option<f32>` |
| Params | `&mut self, node_id: NodeID` |
| Returns | `Option<f32>` |
| Use when | Use `get_local_rot_2d` to get local rot 2d on the scene graph; guard stale IDs and concrete/base type mismatches. |
| Fails when / edge behavior | Returns `None` when `get_local_rot_2d` cannot produce a value for the supplied target or inputs. |

### `get_local_rot_3d`

| Field | Detail |
| --- | --- |
| Access | `ctx.run.Nodes()` |
| Signature | `pub fn get_local_rot_3d(&mut self, node_id: NodeID) -> Option<Quaternion>` |
| Params | `&mut self, node_id: NodeID` |
| Returns | `Option<Quaternion>` |
| Use when | Use `get_local_rot_3d` to get local rot 3d on the scene graph; guard stale IDs and concrete/base type mismatches. |
| Fails when / edge behavior | Returns `None` when `get_local_rot_3d` cannot produce a value for the supplied target or inputs. |

### `set_local_rot_2d`

| Field | Detail |
| --- | --- |
| Access | `ctx.run.Nodes()` |
| Signature | `pub fn set_local_rot_2d(&mut self, node_id: NodeID, rot: f32) -> bool` |
| Params | `&mut self, node_id: NodeID, rot: f32` |
| Returns | `bool` |
| Use when | Use `set_local_rot_2d` to set local rot 2d on the scene graph; guard stale IDs and concrete/base type mismatches. |
| Fails when / edge behavior | Returns `false` when `set_local_rot_2d` cannot apply to the supplied target or inputs; `true` confirms success. |

### `set_local_rot_3d`

| Field | Detail |
| --- | --- |
| Access | `ctx.run.Nodes()` |
| Signature | `pub fn set_local_rot_3d(&mut self, node_id: NodeID, rot: Quaternion) -> bool` |
| Params | `&mut self, node_id: NodeID, rot: Quaternion` |
| Returns | `bool` |
| Use when | Use `set_local_rot_3d` to set local rot 3d on the scene graph; guard stale IDs and concrete/base type mismatches. |
| Fails when / edge behavior | Returns `false` when `set_local_rot_3d` cannot apply to the supplied target or inputs; `true` confirms success. |

### `get_global_rot_2d`

| Field | Detail |
| --- | --- |
| Access | `ctx.run.Nodes()` |
| Signature | `pub fn get_global_rot_2d(&mut self, node_id: NodeID) -> Option<f32>` |
| Params | `&mut self, node_id: NodeID` |
| Returns | `Option<f32>` |
| Use when | Use `get_global_rot_2d` to get global rot 2d on the scene graph; guard stale IDs and concrete/base type mismatches. |
| Fails when / edge behavior | Returns `None` when `get_global_rot_2d` cannot produce a value for the supplied target or inputs. |

### `get_global_rot_3d`

| Field | Detail |
| --- | --- |
| Access | `ctx.run.Nodes()` |
| Signature | `pub fn get_global_rot_3d(&mut self, node_id: NodeID) -> Option<Quaternion>` |
| Params | `&mut self, node_id: NodeID` |
| Returns | `Option<Quaternion>` |
| Use when | Use `get_global_rot_3d` to get global rot 3d on the scene graph; guard stale IDs and concrete/base type mismatches. |
| Fails when / edge behavior | Returns `None` when `get_global_rot_3d` cannot produce a value for the supplied target or inputs. |

### `set_global_rot_2d`

| Field | Detail |
| --- | --- |
| Access | `ctx.run.Nodes()` |
| Signature | `pub fn set_global_rot_2d(&mut self, node_id: NodeID, rot: f32) -> bool` |
| Params | `&mut self, node_id: NodeID, rot: f32` |
| Returns | `bool` |
| Use when | Use `set_global_rot_2d` to set global rot 2d on the scene graph; guard stale IDs and concrete/base type mismatches. |
| Fails when / edge behavior | Returns `false` when `set_global_rot_2d` cannot apply to the supplied target or inputs; `true` confirms success. |

### `set_global_rot_3d`

| Field | Detail |
| --- | --- |
| Access | `ctx.run.Nodes()` |
| Signature | `pub fn set_global_rot_3d(&mut self, node_id: NodeID, rot: Quaternion) -> bool` |
| Params | `&mut self, node_id: NodeID, rot: Quaternion` |
| Returns | `bool` |
| Use when | Use `set_global_rot_3d` to set global rot 3d on the scene graph; guard stale IDs and concrete/base type mismatches. |
| Fails when / edge behavior | Returns `false` when `set_global_rot_3d` cannot apply to the supplied target or inputs; `true` confirms success. |

### `get_local_scale_2d`

| Field | Detail |
| --- | --- |
| Access | `ctx.run.Nodes()` |
| Signature | `pub fn get_local_scale_2d(&mut self, node_id: NodeID) -> Option<Vector2>` |
| Params | `&mut self, node_id: NodeID` |
| Returns | `Option<Vector2>` |
| Use when | Use `get_local_scale_2d` to get local scale 2d on the scene graph; guard stale IDs and concrete/base type mismatches. |
| Fails when / edge behavior | Returns `None` when `get_local_scale_2d` cannot produce a value for the supplied target or inputs. |

### `get_local_scale_3d`

| Field | Detail |
| --- | --- |
| Access | `ctx.run.Nodes()` |
| Signature | `pub fn get_local_scale_3d(&mut self, node_id: NodeID) -> Option<Vector3>` |
| Params | `&mut self, node_id: NodeID` |
| Returns | `Option<Vector3>` |
| Use when | Use `get_local_scale_3d` to get local scale 3d on the scene graph; guard stale IDs and concrete/base type mismatches. |
| Fails when / edge behavior | Returns `None` when `get_local_scale_3d` cannot produce a value for the supplied target or inputs. |

### `set_local_scale_2d`

| Field | Detail |
| --- | --- |
| Access | `ctx.run.Nodes()` |
| Signature | `pub fn set_local_scale_2d(&mut self, node_id: NodeID, scale: Vector2) -> bool` |
| Params | `&mut self, node_id: NodeID, scale: Vector2` |
| Returns | `bool` |
| Use when | Use `set_local_scale_2d` to set local scale 2d on the scene graph; guard stale IDs and concrete/base type mismatches. |
| Fails when / edge behavior | Returns `false` when `set_local_scale_2d` cannot apply to the supplied target or inputs; `true` confirms success. |

### `set_local_scale_3d`

| Field | Detail |
| --- | --- |
| Access | `ctx.run.Nodes()` |
| Signature | `pub fn set_local_scale_3d(&mut self, node_id: NodeID, scale: Vector3) -> bool` |
| Params | `&mut self, node_id: NodeID, scale: Vector3` |
| Returns | `bool` |
| Use when | Use `set_local_scale_3d` to set local scale 3d on the scene graph; guard stale IDs and concrete/base type mismatches. |
| Fails when / edge behavior | Returns `false` when `set_local_scale_3d` cannot apply to the supplied target or inputs; `true` confirms success. |

### `get_global_scale_2d`

| Field | Detail |
| --- | --- |
| Access | `ctx.run.Nodes()` |
| Signature | `pub fn get_global_scale_2d(&mut self, node_id: NodeID) -> Option<Vector2>` |
| Params | `&mut self, node_id: NodeID` |
| Returns | `Option<Vector2>` |
| Use when | Use `get_global_scale_2d` to get global scale 2d on the scene graph; guard stale IDs and concrete/base type mismatches. |
| Fails when / edge behavior | Returns `None` when `get_global_scale_2d` cannot produce a value for the supplied target or inputs. |

### `get_global_scale_3d`

| Field | Detail |
| --- | --- |
| Access | `ctx.run.Nodes()` |
| Signature | `pub fn get_global_scale_3d(&mut self, node_id: NodeID) -> Option<Vector3>` |
| Params | `&mut self, node_id: NodeID` |
| Returns | `Option<Vector3>` |
| Use when | Use `get_global_scale_3d` to get global scale 3d on the scene graph; guard stale IDs and concrete/base type mismatches. |
| Fails when / edge behavior | Returns `None` when `get_global_scale_3d` cannot produce a value for the supplied target or inputs. |

### `set_global_scale_2d`

| Field | Detail |
| --- | --- |
| Access | `ctx.run.Nodes()` |
| Signature | `pub fn set_global_scale_2d(&mut self, node_id: NodeID, scale: Vector2) -> bool` |
| Params | `&mut self, node_id: NodeID, scale: Vector2` |
| Returns | `bool` |
| Use when | Use `set_global_scale_2d` to set global scale 2d on the scene graph; guard stale IDs and concrete/base type mismatches. |
| Fails when / edge behavior | Returns `false` when `set_global_scale_2d` cannot apply to the supplied target or inputs; `true` confirms success. |

### `set_global_scale_3d`

| Field | Detail |
| --- | --- |
| Access | `ctx.run.Nodes()` |
| Signature | `pub fn set_global_scale_3d(&mut self, node_id: NodeID, scale: Vector3) -> bool` |
| Params | `&mut self, node_id: NodeID, scale: Vector3` |
| Returns | `bool` |
| Use when | Use `set_global_scale_3d` to set global scale 3d on the scene graph; guard stale IDs and concrete/base type mismatches. |
| Fails when / edge behavior | Returns `false` when `set_global_scale_3d` cannot apply to the supplied target or inputs; `true` confirms success. |

### `to_global_point_2d`

| Field | Detail |
| --- | --- |
| Access | `ctx.run.Nodes()` |
| Signature | `pub fn to_global_point_2d(&mut self, node_id: NodeID, local: Vector2) -> Option<Vector2>` |
| Params | `&mut self, node_id: NodeID, local: Vector2` |
| Returns | `Option<Vector2>` |
| Use when | Use when converting points or transforms between local node space and world space. |
| Fails when / edge behavior | Returns `None` when `to_global_point_2d` cannot produce a value for the supplied target or inputs. |

### `to_local_point_2d`

| Field | Detail |
| --- | --- |
| Access | `ctx.run.Nodes()` |
| Signature | `pub fn to_local_point_2d(&mut self, node_id: NodeID, global: Vector2) -> Option<Vector2>` |
| Params | `&mut self, node_id: NodeID, global: Vector2` |
| Returns | `Option<Vector2>` |
| Use when | Use when converting points or transforms between local node space and world space. |
| Fails when / edge behavior | Returns `None` when `to_local_point_2d` cannot produce a value for the supplied target or inputs. |

### `to_global_point_3d`

| Field | Detail |
| --- | --- |
| Access | `ctx.run.Nodes()` |
| Signature | `pub fn to_global_point_3d(&mut self, node_id: NodeID, local: Vector3) -> Option<Vector3>` |
| Params | `&mut self, node_id: NodeID, local: Vector3` |
| Returns | `Option<Vector3>` |
| Use when | Use when converting points or transforms between local node space and world space. |
| Fails when / edge behavior | Returns `None` when `to_global_point_3d` cannot produce a value for the supplied target or inputs. |

### `to_local_point_3d`

| Field | Detail |
| --- | --- |
| Access | `ctx.run.Nodes()` |
| Signature | `pub fn to_local_point_3d(&mut self, node_id: NodeID, global: Vector3) -> Option<Vector3>` |
| Params | `&mut self, node_id: NodeID, global: Vector3` |
| Returns | `Option<Vector3>` |
| Use when | Use when converting points or transforms between local node space and world space. |
| Fails when / edge behavior | Returns `None` when `to_local_point_3d` cannot produce a value for the supplied target or inputs. |

### `to_global_transform_2d`

| Field | Detail |
| --- | --- |
| Access | `ctx.run.Nodes()` |
| Signature | `pub fn to_global_transform_2d( &mut self, node_id: NodeID, local: Transform2D, ) -> Option<Transform2D>` |
| Params | `&mut self, node_id: NodeID, local: Transform2D,` |
| Returns | `Option<Transform2D>` |
| Use when | Use when converting points or transforms between local node space and world space. |
| Fails when / edge behavior | Returns `None` when `to_global_transform_2d` cannot produce a value for the supplied target or inputs. |

### `to_local_transform_2d`

| Field | Detail |
| --- | --- |
| Access | `ctx.run.Nodes()` |
| Signature | `pub fn to_local_transform_2d( &mut self, node_id: NodeID, global: Transform2D, ) -> Option<Transform2D>` |
| Params | `&mut self, node_id: NodeID, global: Transform2D,` |
| Returns | `Option<Transform2D>` |
| Use when | Use when converting points or transforms between local node space and world space. |
| Fails when / edge behavior | Returns `None` when `to_local_transform_2d` cannot produce a value for the supplied target or inputs. |

### `to_global_transform_3d`

| Field | Detail |
| --- | --- |
| Access | `ctx.run.Nodes()` |
| Signature | `pub fn to_global_transform_3d( &mut self, node_id: NodeID, local: Transform3D, ) -> Option<Transform3D>` |
| Params | `&mut self, node_id: NodeID, local: Transform3D,` |
| Returns | `Option<Transform3D>` |
| Use when | Use when converting points or transforms between local node space and world space. |
| Fails when / edge behavior | Returns `None` when `to_global_transform_3d` cannot produce a value for the supplied target or inputs. |

### `to_local_transform_3d`

| Field | Detail |
| --- | --- |
| Access | `ctx.run.Nodes()` |
| Signature | `pub fn to_local_transform_3d( &mut self, node_id: NodeID, global: Transform3D, ) -> Option<Transform3D>` |
| Params | `&mut self, node_id: NodeID, global: Transform3D,` |
| Returns | `Option<Transform3D>` |
| Use when | Use when converting points or transforms between local node space and world space. |
| Fails when / edge behavior | Returns `None` when `to_local_transform_3d` cannot produce a value for the supplied target or inputs. |

### `camera_screen_ray_3d`

| Field | Detail |
| --- | --- |
| Access | `ctx.run.Nodes()` |
| Signature | `pub fn camera_screen_ray_3d(&mut self, camera_id: NodeID, pixel: Vector2, viewport_size: Vector2) -> Option<CameraRay3D>` |
| Returns | Global ray origin, normalized direction, and projection far limit. |
| Coordinates | Top-left pixel origin; supports perspective, orthographic, and frustum cameras. |
| Fails when | Camera ID/type or viewport size is invalid. |

### `mesh_instance_surface_at_global_point`

| Field | Detail |
| --- | --- |
| Access | `ctx.run.Nodes()` |
| Signature | `pub fn mesh_instance_surface_at_global_point( &mut self, node_id: NodeID, global_point: Vector3, ) -> Option<MeshSurfaceHit3D>` |
| Params | `&mut self, node_id: NodeID, global_point: Vector3,` |
| Returns | `Option<MeshSurfaceHit3D>` |
| Use when | Use `mesh_instance_surface_at_global_point` to mesh instance surface at global point on the scene graph; guard stale IDs and concrete/base type mismatches. |
| Fails when / edge behavior | Returns `None` when `mesh_instance_surface_at_global_point` cannot produce a value for the supplied target or inputs. |

### `mesh_instance_surface_on_global_ray`

| Field | Detail |
| --- | --- |
| Access | `ctx.run.Nodes()` |
| Signature | `pub fn mesh_instance_surface_on_global_ray( &mut self, node_id: NodeID, ray_origin: Vector3, ray_direction: Vector3, max_distance: f32, ) -> Option<MeshSurfaceHit3D>` |
| Params | `&mut self, node_id: NodeID, ray_origin: Vector3, ray_direction: Vector3, max_distance: f32,` |
| Returns | `Option<MeshSurfaceHit3D>` |
| Use when | Use `mesh_instance_surface_on_global_ray` to mesh instance surface on global ray on the scene graph; guard stale IDs and concrete/base type mismatches. |
| Fails when / edge behavior | Returns `None` when `mesh_instance_surface_on_global_ray` cannot produce a value for the supplied target or inputs. |

### `mesh_instance_surfaces_on_global_rays`

| Field | Detail |
| --- | --- |
| Access | `ctx.run.Nodes()` |
| Signature | `pub fn mesh_instance_surfaces_on_global_rays( &mut self, node_id: NodeID, rays: &[MeshSurfaceRay3D], resolve_material: bool, ) -> Vec<Option<MeshSurfaceHit3D>>` |
| Params | `&mut self, node_id: NodeID, rays: &[MeshSurfaceRay3D], resolve_material: bool,` |
| Returns | `Vec<Option<MeshSurfaceHit3D>>` |
| Use when | Use `mesh_instance_surfaces_on_global_rays` to mesh instance surfaces on global rays on the scene graph; guard stale IDs and concrete/base type mismatches. |
| Fails when / edge behavior | Returns `None` when `mesh_instance_surfaces_on_global_rays` cannot produce a value for the supplied target or inputs. |

### `mesh_instance_material_regions`

| Field | Detail |
| --- | --- |
| Access | `ctx.run.Nodes()` |
| Signature | `pub fn mesh_instance_material_regions( &mut self, node_id: NodeID, material: MaterialID, ) -> Vec<MeshMaterialRegion3D>` |
| Params | `&mut self, node_id: NodeID, material: MaterialID,` |
| Returns | `Vec<MeshMaterialRegion3D>` |
| Use when | Use `mesh_instance_material_regions` to mesh instance material regions on the scene graph; guard stale IDs and concrete/base type mismatches. |
| Fails when / edge behavior | Returns an empty vector when `mesh_instance_material_regions` finds no values; callers must treat zero results as normal. |

### `mesh_data_surface_at_local_point`

| Field | Detail |
| --- | --- |
| Access | `ctx.run.Nodes()` |
| Signature | `pub fn mesh_data_surface_at_local_point( &mut self, mesh_id: MeshID, local_point: Vector3, ) -> Option<MeshDataSurfaceHit3D>` |
| Params | `&mut self, mesh_id: MeshID, local_point: Vector3,` |
| Returns | `Option<MeshDataSurfaceHit3D>` |
| Use when | Use `mesh_data_surface_at_local_point` to mesh data surface at local point on the scene graph; guard stale IDs and concrete/base type mismatches. |
| Fails when / edge behavior | Returns `None` when `mesh_data_surface_at_local_point` cannot produce a value for the supplied target or inputs. |

### `mesh_data_surface_on_local_ray`

| Field | Detail |
| --- | --- |
| Access | `ctx.run.Nodes()` |
| Signature | `pub fn mesh_data_surface_on_local_ray( &mut self, mesh_id: MeshID, ray_origin_local: Vector3, ray_direction_local: Vector3, max_distance: f32, ) -> Option<MeshDataSurfaceHit3D>` |
| Params | `&mut self, mesh_id: MeshID, ray_origin_local: Vector3, ray_direction_local: Vector3, max_distance: f32,` |
| Returns | `Option<MeshDataSurfaceHit3D>` |
| Use when | Use `mesh_data_surface_on_local_ray` to mesh data surface on local ray on the scene graph; guard stale IDs and concrete/base type mismatches. |
| Fails when / edge behavior | Returns `None` when `mesh_data_surface_on_local_ray` cannot produce a value for the supplied target or inputs. |

### `mesh_data_surface_regions`

| Field | Detail |
| --- | --- |
| Access | `ctx.run.Nodes()` |
| Signature | `pub fn mesh_data_surface_regions( &mut self, mesh_id: MeshID, surface_index: u32, ) -> Vec<MeshDataSurfaceRegion3D>` |
| Params | `&mut self, mesh_id: MeshID, surface_index: u32,` |
| Returns | `Vec<MeshDataSurfaceRegion3D>` |
| Use when | Use `mesh_data_surface_regions` to mesh data surface regions on the scene graph; guard stale IDs and concrete/base type mismatches. |
| Fails when / edge behavior | Returns an empty vector when `mesh_data_surface_regions` finds no values; callers must treat zero results as normal. |

### `with_node_mut`

| Field | Detail |
| --- | --- |
| Access | `ctx.run.Nodes()` |
| Signature | `with_node_mut!(ctx.run, node_ty, id, f)` |
| Params | `ctx, node_ty, id, f` |
| Returns | `same as backing method` |
| Use when | Use `with_node_mut` to with node mut on the scene graph; guard stale IDs and concrete/base type mismatches. |
| Fails when / edge behavior | Uses the backing `with_node_mut` return and failure behavior unchanged; the wrapper adds no coercion or fallback. |

### `with_node`

| Field | Detail |
| --- | --- |
| Access | `ctx.run.Nodes()` |
| Signature | `with_node!(ctx.run, node_ty, id, f)` |
| Params | `ctx, node_ty, id, f` |
| Returns | `same as backing method` |
| Use when | Use `with_node` to with node on the scene graph; guard stale IDs and concrete/base type mismatches. |
| Fails when / edge behavior | Uses the backing `with_node` return and failure behavior unchanged; the wrapper adds no coercion or fallback. |

### `with_base_node`

| Field | Detail |
| --- | --- |
| Access | `ctx.run.Nodes()` |
| Signature | `with_base_node!(ctx.run, base_ty, id, f)` |
| Params | `ctx, base_ty, id, f` |
| Returns | `same as backing method` |
| Use when | Use `with_base_node` to with base node on the scene graph; guard stale IDs and concrete/base type mismatches. |
| Fails when / edge behavior | Uses the backing `with_base_node` return and failure behavior unchanged; the wrapper adds no coercion or fallback. |

### `with_base_node_mut`

| Field | Detail |
| --- | --- |
| Access | `ctx.run.Nodes()` |
| Signature | `with_base_node_mut!(ctx.run, base_ty, id, f)` |
| Params | `ctx, base_ty, id, f` |
| Returns | `same as backing method` |
| Use when | Use `with_base_node_mut` to with base node mut on the scene graph; guard stale IDs and concrete/base type mismatches. |
| Fails when / edge behavior | Uses the backing `with_base_node_mut` return and failure behavior unchanged; the wrapper adds no coercion or fallback. |

### `create_node`

| Field | Detail |
| --- | --- |
| Access | `ctx.run.Nodes()` |
| Signature | `create_node!(ctx.run, node_ty)` |
| Params | `ctx, node_ty` |
| Returns | `resource/runtime ID or `Result` as shown by backing method` |
| Use when | Use `create_node` to create node on the scene graph; guard stale IDs and concrete/base type mismatches. |
| Fails when / edge behavior | Uses the backing `create_node` return and failure behavior unchanged; the wrapper adds no coercion or fallback. |

### `node_collection`

| Field | Detail |
| --- | --- |
| Access | `ctx.run.Nodes()` |
| Signature | `node_collection!({ ... })` or `node_collection![{ ... }, { ... }]` |
| Params | `name =`, `tags =`, `node =`, optional `children = [...]`, or `collection = expr` |
| Returns | `NodeCollection` |
| Use when | Use `node_collection` to node collection on the scene graph; guard stale IDs and concrete/base type mismatches. |
| Fails when / edge behavior | Has no optional/error return; `node_collection` returns the documented value directly. |

### `create_nodes`

| Field | Detail |
| --- | --- |
| Access | `ctx.run.Nodes()` |
| Signature | `create_nodes!(ctx.run, requests)` |
| Params | `ctx, NodeCollection, optional parent` |
| Returns | `resource/runtime ID or `Result` as shown by backing method` |
| Use when | Use `create_nodes` to create nodes on the scene graph; guard stale IDs and concrete/base type mismatches. |
| Fails when / edge behavior | Uses the backing `create_nodes` return and failure behavior unchanged; the wrapper adds no coercion or fallback. |

### `get_node_name`

| Field | Detail |
| --- | --- |
| Access | `ctx.run.Nodes()` |
| Signature | `get_node_name!(ctx.run, id)` |
| Params | `ctx, id` |
| Returns | `typed value from backing method` |
| Use when | Use `get_node_name` to get node name on the scene graph; guard stale IDs and concrete/base type mismatches. |
| Fails when / edge behavior | Uses the backing `get_node_name` return and failure behavior unchanged; the wrapper adds no coercion or fallback. |

### `set_node_name`

| Field | Detail |
| --- | --- |
| Access | `ctx.run.Nodes()` |
| Signature | `set_node_name!(ctx.run, id, name)` |
| Params | `ctx, id, name` |
| Returns | `bool or () as shown by backing method` |
| Use when | Use `set_node_name` to set node name on the scene graph; guard stale IDs and concrete/base type mismatches. |
| Fails when / edge behavior | Returns `false` when `set_node_name` cannot apply to the supplied target or inputs; `true` confirms success. |

### `get_skeleton_bone_name`

| Field | Detail |
| --- | --- |
| Access | `ctx.run.Nodes()` |
| Signature | `get_skeleton_bone_name!(ctx.run, id, index)` |
| Params | `ctx, id, index` |
| Returns | `typed value from backing method` |
| Use when | Use `get_skeleton_bone_name` to get skeleton bone name on the scene graph; guard stale IDs and concrete/base type mismatches. |
| Fails when / edge behavior | Uses the backing `get_skeleton_bone_name` return and failure behavior unchanged; the wrapper adds no coercion or fallback. |

### `get_skeleton_bone_index`

| Field | Detail |
| --- | --- |
| Access | `ctx.run.Nodes()` |
| Signature | `get_skeleton_bone_index!(ctx.run, id, name)` |
| Params | `ctx, id, name` |
| Returns | `typed value from backing method` |
| Use when | Use `get_skeleton_bone_index` to get skeleton bone index on the scene graph; guard stale IDs and concrete/base type mismatches. |
| Fails when / edge behavior | Uses the backing `get_skeleton_bone_index` return and failure behavior unchanged; the wrapper adds no coercion or fallback. |

### `set_ui_rotation`

| Field | Detail |
| --- | --- |
| Access | `ctx.run.Nodes()` |
| Signature | `set_ui_rotation!(ctx.run, id, rotation)` |
| Params | `ctx, id, rotation` |
| Returns | `bool or () as shown by backing method` |
| Use when | Use `set_ui_rotation` to set ui rotation on the scene graph; guard stale IDs and concrete/base type mismatches. |
| Fails when / edge behavior | Returns `false` when `set_ui_rotation` cannot apply to the supplied target or inputs; `true` confirms success. |

### `bind_locale_text`

| Field | Detail |
| --- | --- |
| Access | `ctx.run.Nodes()` |
| Signature | `bind_locale_text!(ctx.run, id, key)` |
| Params | `ctx, id, key` |
| Returns | `bool or () as shown by backing method` |
| Use when | Bind locale text on `UiLabel`, `Label2D`, or `Label3D`. |
| Fails when / edge behavior | Returns `false` when backing runtime data is missing, stale, or the target type does not support locale text. |

### `bind_locale_placeholder`

| Field | Detail |
| --- | --- |
| Access | `ctx.run.Nodes()` |
| Signature | `bind_locale_placeholder!(ctx.run, id, key)` |
| Params | `ctx, id, key` |
| Returns | `bool or () as shown by backing method` |
| Use when | Use `bind_locale_placeholder` to bind locale placeholder on the scene graph; guard stale IDs and concrete/base type mismatches. |
| Fails when / edge behavior | Returns `false` when `bind_locale_placeholder` cannot apply to the supplied target or inputs; `true` confirms success. |

### `get_node_parent_id`

| Field | Detail |
| --- | --- |
| Access | `ctx.run.Nodes()` |
| Signature | `get_node_parent_id!(ctx.run, id)` |
| Params | `ctx, id` |
| Returns | `typed value from backing method` |
| Use when | Use `get_node_parent_id` to get node parent id on the scene graph; guard stale IDs and concrete/base type mismatches. |
| Fails when / edge behavior | Uses the backing `get_node_parent_id` return and failure behavior unchanged; the wrapper adds no coercion or fallback. |

### `get_node_children_ids`

| Field | Detail |
| --- | --- |
| Access | `ctx.run.Nodes()` |
| Signature | `get_node_children_ids!(ctx.run, id)` |
| Params | `ctx, id` |
| Returns | `typed value from backing method` |
| Use when | Use `get_node_children_ids` to get node children ids on the scene graph; guard stale IDs and concrete/base type mismatches. |
| Fails when / edge behavior | Uses the backing `get_node_children_ids` return and failure behavior unchanged; the wrapper adds no coercion or fallback. |

### `get_children`

| Field | Detail |
| --- | --- |
| Access | `ctx.run.Nodes()` |
| Signature | `get_children!(ctx.run, id)` |
| Params | `ctx, id` |
| Returns | `typed value from backing method` |
| Use when | Use `get_children` to get children on the scene graph; guard stale IDs and concrete/base type mismatches. |
| Fails when / edge behavior | Uses the backing `get_children` return and failure behavior unchanged; the wrapper adds no coercion or fallback. |

### `get_child`

| Field | Detail |
| --- | --- |
| Access | `ctx.run.Nodes()` |
| Signature | `get_child!(ctx.run, id, all[name] $(,)?)` |
| Params | `ctx, id, all[name] $(,)?` |
| Returns | `typed value from backing method` |
| Use when | Use `get_child` to get child on the scene graph; guard stale IDs and concrete/base type mismatches. |
| Fails when / edge behavior | Uses the backing `get_child` return and failure behavior unchanged; the wrapper adds no coercion or fallback. |

### `get_node_type`

| Field | Detail |
| --- | --- |
| Access | `ctx.run.Nodes()` |
| Signature | `get_node_type!(ctx.run, id)` |
| Params | `ctx, id` |
| Returns | `typed value from backing method` |
| Use when | Use `get_node_type` to get node type on the scene graph; guard stale IDs and concrete/base type mismatches. |
| Fails when / edge behavior | Uses the backing `get_node_type` return and failure behavior unchanged; the wrapper adds no coercion or fallback. |

### `reparent`

| Field | Detail |
| --- | --- |
| Access | `ctx.run.Nodes()` |
| Signature | `reparent!(ctx.run, parent, child)` |
| Params | `ctx, parent, child` |
| Returns | `same as backing method` |
| Use when | Use `reparent` to reparent on the scene graph; guard stale IDs and concrete/base type mismatches. |
| Fails when / edge behavior | Uses the backing `reparent` return and failure behavior unchanged; the wrapper adds no coercion or fallback. |

### `force_rerender`

| Field | Detail |
| --- | --- |
| Access | `ctx.run.Nodes()` |
| Signature | `force_rerender!(ctx.run, id)` |
| Params | `ctx, id` |
| Returns | `same as backing method` |
| Use when | Use `force_rerender` to force rerender on the scene graph; guard stale IDs and concrete/base type mismatches. |
| Fails when / edge behavior | Uses the backing `force_rerender` return and failure behavior unchanged; the wrapper adds no coercion or fallback. |

### `reparent_multi`

| Field | Detail |
| --- | --- |
| Access | `ctx.run.Nodes()` |
| Signature | `reparent_multi!(ctx.run, parent, child_ids)` |
| Params | `ctx, parent, child_ids` |
| Returns | `same as backing method` |
| Use when | Use `reparent_multi` to reparent multi on the scene graph; guard stale IDs and concrete/base type mismatches. |
| Fails when / edge behavior | Uses the backing `reparent_multi` return and failure behavior unchanged; the wrapper adds no coercion or fallback. |

### `remove_node`

| Field | Detail |
| --- | --- |
| Access | `ctx.run.Nodes()` |
| Signature | `remove_node!(ctx.run, id)` |
| Params | `ctx, id` |
| Returns | `bool or () as shown by backing method` |
| Use when | Use `remove_node` to remove node on the scene graph; guard stale IDs and concrete/base type mismatches. |
| Fails when / edge behavior | Returns `false` when `remove_node` cannot apply to the supplied target or inputs; `true` confirms success. |

### `get_global_transform_2d`

| Field | Detail |
| --- | --- |
| Access | `ctx.run.Nodes()` |
| Signature | `get_global_transform_2d!(ctx.run, id)` |
| Params | `ctx, id` |
| Returns | `typed value from backing method` |
| Use when | Use `get_global_transform_2d` to get global transform 2d on the scene graph; guard stale IDs and concrete/base type mismatches. |
| Fails when / edge behavior | Uses the backing `get_global_transform_2d` return and failure behavior unchanged; the wrapper adds no coercion or fallback. |

### `get_global_transform_3d`

| Field | Detail |
| --- | --- |
| Access | `ctx.run.Nodes()` |
| Signature | `get_global_transform_3d!(ctx.run, id)` |
| Params | `ctx, id` |
| Returns | `typed value from backing method` |
| Use when | Use `get_global_transform_3d` to get global transform 3d on the scene graph; guard stale IDs and concrete/base type mismatches. |
| Fails when / edge behavior | Uses the backing `get_global_transform_3d` return and failure behavior unchanged; the wrapper adds no coercion or fallback. |

### `get_local_transform_2d`

| Field | Detail |
| --- | --- |
| Access | `ctx.run.Nodes()` |
| Signature | `get_local_transform_2d!(ctx.run, id)` |
| Params | `ctx, id` |
| Returns | `typed value from backing method` |
| Use when | Use `get_local_transform_2d` to get local transform 2d on the scene graph; guard stale IDs and concrete/base type mismatches. |
| Fails when / edge behavior | Uses the backing `get_local_transform_2d` return and failure behavior unchanged; the wrapper adds no coercion or fallback. |

### `get_local_transform_3d`

| Field | Detail |
| --- | --- |
| Access | `ctx.run.Nodes()` |
| Signature | `get_local_transform_3d!(ctx.run, id)` |
| Params | `ctx, id` |
| Returns | `typed value from backing method` |
| Use when | Use `get_local_transform_3d` to get local transform 3d on the scene graph; guard stale IDs and concrete/base type mismatches. |
| Fails when / edge behavior | Uses the backing `get_local_transform_3d` return and failure behavior unchanged; the wrapper adds no coercion or fallback. |

### `set_global_transform_2d`

| Field | Detail |
| --- | --- |
| Access | `ctx.run.Nodes()` |
| Signature | `set_global_transform_2d!(ctx.run, id, transform)` |
| Params | `ctx, id, transform` |
| Returns | `bool or () as shown by backing method` |
| Use when | Use `set_global_transform_2d` to set global transform 2d on the scene graph; guard stale IDs and concrete/base type mismatches. |
| Fails when / edge behavior | Returns `false` when `set_global_transform_2d` cannot apply to the supplied target or inputs; `true` confirms success. |

### `set_global_transform_3d`

| Field | Detail |
| --- | --- |
| Access | `ctx.run.Nodes()` |
| Signature | `set_global_transform_3d!(ctx.run, id, transform)` |
| Params | `ctx, id, transform` |
| Returns | `bool or () as shown by backing method` |
| Use when | Use `set_global_transform_3d` to set global transform 3d on the scene graph; guard stale IDs and concrete/base type mismatches. |
| Fails when / edge behavior | Returns `false` when `set_global_transform_3d` cannot apply to the supplied target or inputs; `true` confirms success. |

### `set_local_transform_2d`

| Field | Detail |
| --- | --- |
| Access | `ctx.run.Nodes()` |
| Signature | `set_local_transform_2d!(ctx.run, id, transform)` |
| Params | `ctx, id, transform` |
| Returns | `bool or () as shown by backing method` |
| Use when | Use `set_local_transform_2d` to set local transform 2d on the scene graph; guard stale IDs and concrete/base type mismatches. |
| Fails when / edge behavior | Returns `false` when `set_local_transform_2d` cannot apply to the supplied target or inputs; `true` confirms success. |

### `set_local_transform_3d`

| Field | Detail |
| --- | --- |
| Access | `ctx.run.Nodes()` |
| Signature | `set_local_transform_3d!(ctx.run, id, transform)` |
| Params | `ctx, id, transform` |
| Returns | `bool or () as shown by backing method` |
| Use when | Use `set_local_transform_3d` to set local transform 3d on the scene graph; guard stale IDs and concrete/base type mismatches. |
| Fails when / edge behavior | Returns `false` when `set_local_transform_3d` cannot apply to the supplied target or inputs; `true` confirms success. |

### `get_local_pos_2d`

| Field | Detail |
| --- | --- |
| Access | `ctx.run.Nodes()` |
| Signature | `get_local_pos_2d!(ctx.run, id)` |
| Params | `ctx, id` |
| Returns | `typed value from backing method` |
| Use when | Use `get_local_pos_2d` to get local pos 2d on the scene graph; guard stale IDs and concrete/base type mismatches. |
| Fails when / edge behavior | Uses the backing `get_local_pos_2d` return and failure behavior unchanged; the wrapper adds no coercion or fallback. |

### `get_local_pos_3d`

| Field | Detail |
| --- | --- |
| Access | `ctx.run.Nodes()` |
| Signature | `get_local_pos_3d!(ctx.run, id)` |
| Params | `ctx, id` |
| Returns | `typed value from backing method` |
| Use when | Use `get_local_pos_3d` to get local pos 3d on the scene graph; guard stale IDs and concrete/base type mismatches. |
| Fails when / edge behavior | Uses the backing `get_local_pos_3d` return and failure behavior unchanged; the wrapper adds no coercion or fallback. |

### `set_local_pos_2d`

| Field | Detail |
| --- | --- |
| Access | `ctx.run.Nodes()` |
| Signature | `set_local_pos_2d!(ctx.run, id, pos)` |
| Params | `ctx, id, pos` |
| Returns | `bool or () as shown by backing method` |
| Use when | Use `set_local_pos_2d` to set local pos 2d on the scene graph; guard stale IDs and concrete/base type mismatches. |
| Fails when / edge behavior | Returns `false` when `set_local_pos_2d` cannot apply to the supplied target or inputs; `true` confirms success. |

### `set_local_pos_3d`

| Field | Detail |
| --- | --- |
| Access | `ctx.run.Nodes()` |
| Signature | `set_local_pos_3d!(ctx.run, id, pos)` |
| Params | `ctx, id, pos` |
| Returns | `bool or () as shown by backing method` |
| Use when | Use `set_local_pos_3d` to set local pos 3d on the scene graph; guard stale IDs and concrete/base type mismatches. |
| Fails when / edge behavior | Returns `false` when `set_local_pos_3d` cannot apply to the supplied target or inputs; `true` confirms success. |

### `get_global_pos_2d`

| Field | Detail |
| --- | --- |
| Access | `ctx.run.Nodes()` |
| Signature | `get_global_pos_2d!(ctx.run, id)` |
| Params | `ctx, id` |
| Returns | `typed value from backing method` |
| Use when | Use `get_global_pos_2d` to get global pos 2d on the scene graph; guard stale IDs and concrete/base type mismatches. |
| Fails when / edge behavior | Uses the backing `get_global_pos_2d` return and failure behavior unchanged; the wrapper adds no coercion or fallback. |

### `get_global_pos_3d`

| Field | Detail |
| --- | --- |
| Access | `ctx.run.Nodes()` |
| Signature | `get_global_pos_3d!(ctx.run, id)` |
| Params | `ctx, id` |
| Returns | `typed value from backing method` |
| Use when | Use `get_global_pos_3d` to get global pos 3d on the scene graph; guard stale IDs and concrete/base type mismatches. |
| Fails when / edge behavior | Uses the backing `get_global_pos_3d` return and failure behavior unchanged; the wrapper adds no coercion or fallback. |

### `set_global_pos_2d`

| Field | Detail |
| --- | --- |
| Access | `ctx.run.Nodes()` |
| Signature | `set_global_pos_2d!(ctx.run, id, pos)` |
| Params | `ctx, id, pos` |
| Returns | `bool or () as shown by backing method` |
| Use when | Use `set_global_pos_2d` to set global pos 2d on the scene graph; guard stale IDs and concrete/base type mismatches. |
| Fails when / edge behavior | Returns `false` when `set_global_pos_2d` cannot apply to the supplied target or inputs; `true` confirms success. |

### `set_global_pos_3d`

| Field | Detail |
| --- | --- |
| Access | `ctx.run.Nodes()` |
| Signature | `set_global_pos_3d!(ctx.run, id, pos)` |
| Params | `ctx, id, pos` |
| Returns | `bool or () as shown by backing method` |
| Use when | Use `set_global_pos_3d` to set global pos 3d on the scene graph; guard stale IDs and concrete/base type mismatches. |
| Fails when / edge behavior | Returns `false` when `set_global_pos_3d` cannot apply to the supplied target or inputs; `true` confirms success. |

### `get_local_rot_2d`

| Field | Detail |
| --- | --- |
| Access | `ctx.run.Nodes()` |
| Signature | `get_local_rot_2d!(ctx.run, id)` |
| Params | `ctx, id` |
| Returns | `typed value from backing method` |
| Use when | Use `get_local_rot_2d` to get local rot 2d on the scene graph; guard stale IDs and concrete/base type mismatches. |
| Fails when / edge behavior | Uses the backing `get_local_rot_2d` return and failure behavior unchanged; the wrapper adds no coercion or fallback. |

### `get_local_rot_3d`

| Field | Detail |
| --- | --- |
| Access | `ctx.run.Nodes()` |
| Signature | `get_local_rot_3d!(ctx.run, id)` |
| Params | `ctx, id` |
| Returns | `typed value from backing method` |
| Use when | Use `get_local_rot_3d` to get local rot 3d on the scene graph; guard stale IDs and concrete/base type mismatches. |
| Fails when / edge behavior | Uses the backing `get_local_rot_3d` return and failure behavior unchanged; the wrapper adds no coercion or fallback. |

### `set_local_rot_2d`

| Field | Detail |
| --- | --- |
| Access | `ctx.run.Nodes()` |
| Signature | `set_local_rot_2d!(ctx.run, id, rot)` |
| Params | `ctx, id, rot` |
| Returns | `bool or () as shown by backing method` |
| Use when | Use `set_local_rot_2d` to set local rot 2d on the scene graph; guard stale IDs and concrete/base type mismatches. |
| Fails when / edge behavior | Returns `false` when `set_local_rot_2d` cannot apply to the supplied target or inputs; `true` confirms success. |

### `set_local_rot_3d`

| Field | Detail |
| --- | --- |
| Access | `ctx.run.Nodes()` |
| Signature | `set_local_rot_3d!(ctx.run, id, rot)` |
| Params | `ctx, id, rot` |
| Returns | `bool or () as shown by backing method` |
| Use when | Use `set_local_rot_3d` to set local rot 3d on the scene graph; guard stale IDs and concrete/base type mismatches. |
| Fails when / edge behavior | Returns `false` when `set_local_rot_3d` cannot apply to the supplied target or inputs; `true` confirms success. |

### `get_global_rot_2d`

| Field | Detail |
| --- | --- |
| Access | `ctx.run.Nodes()` |
| Signature | `get_global_rot_2d!(ctx.run, id)` |
| Params | `ctx, id` |
| Returns | `typed value from backing method` |
| Use when | Use `get_global_rot_2d` to get global rot 2d on the scene graph; guard stale IDs and concrete/base type mismatches. |
| Fails when / edge behavior | Uses the backing `get_global_rot_2d` return and failure behavior unchanged; the wrapper adds no coercion or fallback. |

### `get_global_rot_3d`

| Field | Detail |
| --- | --- |
| Access | `ctx.run.Nodes()` |
| Signature | `get_global_rot_3d!(ctx.run, id)` |
| Params | `ctx, id` |
| Returns | `typed value from backing method` |
| Use when | Use `get_global_rot_3d` to get global rot 3d on the scene graph; guard stale IDs and concrete/base type mismatches. |
| Fails when / edge behavior | Uses the backing `get_global_rot_3d` return and failure behavior unchanged; the wrapper adds no coercion or fallback. |

### `set_global_rot_2d`

| Field | Detail |
| --- | --- |
| Access | `ctx.run.Nodes()` |
| Signature | `set_global_rot_2d!(ctx.run, id, rot)` |
| Params | `ctx, id, rot` |
| Returns | `bool or () as shown by backing method` |
| Use when | Use `set_global_rot_2d` to set global rot 2d on the scene graph; guard stale IDs and concrete/base type mismatches. |
| Fails when / edge behavior | Returns `false` when `set_global_rot_2d` cannot apply to the supplied target or inputs; `true` confirms success. |

### `set_global_rot_3d`

| Field | Detail |
| --- | --- |
| Access | `ctx.run.Nodes()` |
| Signature | `set_global_rot_3d!(ctx.run, id, rot)` |
| Params | `ctx, id, rot` |
| Returns | `bool or () as shown by backing method` |
| Use when | Use `set_global_rot_3d` to set global rot 3d on the scene graph; guard stale IDs and concrete/base type mismatches. |
| Fails when / edge behavior | Returns `false` when `set_global_rot_3d` cannot apply to the supplied target or inputs; `true` confirms success. |

### `get_local_scale_2d`

| Field | Detail |
| --- | --- |
| Access | `ctx.run.Nodes()` |
| Signature | `get_local_scale_2d!(ctx.run, id)` |
| Params | `ctx, id` |
| Returns | `typed value from backing method` |
| Use when | Use `get_local_scale_2d` to get local scale 2d on the scene graph; guard stale IDs and concrete/base type mismatches. |
| Fails when / edge behavior | Uses the backing `get_local_scale_2d` return and failure behavior unchanged; the wrapper adds no coercion or fallback. |

### `get_local_scale_3d`

| Field | Detail |
| --- | --- |
| Access | `ctx.run.Nodes()` |
| Signature | `get_local_scale_3d!(ctx.run, id)` |
| Params | `ctx, id` |
| Returns | `typed value from backing method` |
| Use when | Use `get_local_scale_3d` to get local scale 3d on the scene graph; guard stale IDs and concrete/base type mismatches. |
| Fails when / edge behavior | Uses the backing `get_local_scale_3d` return and failure behavior unchanged; the wrapper adds no coercion or fallback. |

### `set_local_scale_2d`

| Field | Detail |
| --- | --- |
| Access | `ctx.run.Nodes()` |
| Signature | `set_local_scale_2d!(ctx.run, id, scale)` |
| Params | `ctx, id, scale` |
| Returns | `bool or () as shown by backing method` |
| Use when | Use `set_local_scale_2d` to set local scale 2d on the scene graph; guard stale IDs and concrete/base type mismatches. |
| Fails when / edge behavior | Returns `false` when `set_local_scale_2d` cannot apply to the supplied target or inputs; `true` confirms success. |

### `set_local_scale_3d`

| Field | Detail |
| --- | --- |
| Access | `ctx.run.Nodes()` |
| Signature | `set_local_scale_3d!(ctx.run, id, scale)` |
| Params | `ctx, id, scale` |
| Returns | `bool or () as shown by backing method` |
| Use when | Use `set_local_scale_3d` to set local scale 3d on the scene graph; guard stale IDs and concrete/base type mismatches. |
| Fails when / edge behavior | Returns `false` when `set_local_scale_3d` cannot apply to the supplied target or inputs; `true` confirms success. |

### `get_global_scale_2d`

| Field | Detail |
| --- | --- |
| Access | `ctx.run.Nodes()` |
| Signature | `get_global_scale_2d!(ctx.run, id)` |
| Params | `ctx, id` |
| Returns | `typed value from backing method` |
| Use when | Use `get_global_scale_2d` to get global scale 2d on the scene graph; guard stale IDs and concrete/base type mismatches. |
| Fails when / edge behavior | Uses the backing `get_global_scale_2d` return and failure behavior unchanged; the wrapper adds no coercion or fallback. |

### `get_global_scale_3d`

| Field | Detail |
| --- | --- |
| Access | `ctx.run.Nodes()` |
| Signature | `get_global_scale_3d!(ctx.run, id)` |
| Params | `ctx, id` |
| Returns | `typed value from backing method` |
| Use when | Use `get_global_scale_3d` to get global scale 3d on the scene graph; guard stale IDs and concrete/base type mismatches. |
| Fails when / edge behavior | Uses the backing `get_global_scale_3d` return and failure behavior unchanged; the wrapper adds no coercion or fallback. |

### `set_global_scale_2d`

| Field | Detail |
| --- | --- |
| Access | `ctx.run.Nodes()` |
| Signature | `set_global_scale_2d!(ctx.run, id, scale)` |
| Params | `ctx, id, scale` |
| Returns | `bool or () as shown by backing method` |
| Use when | Use `set_global_scale_2d` to set global scale 2d on the scene graph; guard stale IDs and concrete/base type mismatches. |
| Fails when / edge behavior | Returns `false` when `set_global_scale_2d` cannot apply to the supplied target or inputs; `true` confirms success. |

### `set_global_scale_3d`

| Field | Detail |
| --- | --- |
| Access | `ctx.run.Nodes()` |
| Signature | `set_global_scale_3d!(ctx.run, id, scale)` |
| Params | `ctx, id, scale` |
| Returns | `bool or () as shown by backing method` |
| Use when | Use `set_global_scale_3d` to set global scale 3d on the scene graph; guard stale IDs and concrete/base type mismatches. |
| Fails when / edge behavior | Returns `false` when `set_global_scale_3d` cannot apply to the supplied target or inputs; `true` confirms success. |

### `to_global_point_2d`

| Field | Detail |
| --- | --- |
| Access | `ctx.run.Nodes()` |
| Signature | `to_global_point_2d!(ctx.run, id, point)` |
| Params | `ctx, id, point` |
| Returns | `same as backing method` |
| Use when | Use when converting points or transforms between local node space and world space. |
| Fails when / edge behavior | Uses the backing `to_global_point_2d` return and failure behavior unchanged; the wrapper adds no coercion or fallback. |

### `to_local_point_2d`

| Field | Detail |
| --- | --- |
| Access | `ctx.run.Nodes()` |
| Signature | `to_local_point_2d!(ctx.run, id, point)` |
| Params | `ctx, id, point` |
| Returns | `same as backing method` |
| Use when | Use when converting points or transforms between local node space and world space. |
| Fails when / edge behavior | Uses the backing `to_local_point_2d` return and failure behavior unchanged; the wrapper adds no coercion or fallback. |

### `to_global_point_3d`

| Field | Detail |
| --- | --- |
| Access | `ctx.run.Nodes()` |
| Signature | `to_global_point_3d!(ctx.run, id, point)` |
| Params | `ctx, id, point` |
| Returns | `same as backing method` |
| Use when | Use when converting points or transforms between local node space and world space. |
| Fails when / edge behavior | Uses the backing `to_global_point_3d` return and failure behavior unchanged; the wrapper adds no coercion or fallback. |

### `to_local_point_3d`

| Field | Detail |
| --- | --- |
| Access | `ctx.run.Nodes()` |
| Signature | `to_local_point_3d!(ctx.run, id, point)` |
| Params | `ctx, id, point` |
| Returns | `same as backing method` |
| Use when | Use when converting points or transforms between local node space and world space. |
| Fails when / edge behavior | Uses the backing `to_local_point_3d` return and failure behavior unchanged; the wrapper adds no coercion or fallback. |

### `to_global_transform_2d`

| Field | Detail |
| --- | --- |
| Access | `ctx.run.Nodes()` |
| Signature | `to_global_transform_2d!(ctx.run, id, transform)` |
| Params | `ctx, id, transform` |
| Returns | `same as backing method` |
| Use when | Use when converting points or transforms between local node space and world space. |
| Fails when / edge behavior | Uses the backing `to_global_transform_2d` return and failure behavior unchanged; the wrapper adds no coercion or fallback. |

### `to_local_transform_2d`

| Field | Detail |
| --- | --- |
| Access | `ctx.run.Nodes()` |
| Signature | `to_local_transform_2d!(ctx.run, id, transform)` |
| Params | `ctx, id, transform` |
| Returns | `same as backing method` |
| Use when | Use when converting points or transforms between local node space and world space. |
| Fails when / edge behavior | Uses the backing `to_local_transform_2d` return and failure behavior unchanged; the wrapper adds no coercion or fallback. |

### `to_global_transform_3d`

| Field | Detail |
| --- | --- |
| Access | `ctx.run.Nodes()` |
| Signature | `to_global_transform_3d!(ctx.run, id, transform)` |
| Params | `ctx, id, transform` |
| Returns | `same as backing method` |
| Use when | Use when converting points or transforms between local node space and world space. |
| Fails when / edge behavior | Uses the backing `to_global_transform_3d` return and failure behavior unchanged; the wrapper adds no coercion or fallback. |

### `to_local_transform_3d`

| Field | Detail |
| --- | --- |
| Access | `ctx.run.Nodes()` |
| Signature | `to_local_transform_3d!(ctx.run, id, transform)` |
| Params | `ctx, id, transform` |
| Returns | `same as backing method` |
| Use when | Use when converting points or transforms between local node space and world space. |
| Fails when / edge behavior | Uses the backing `to_local_transform_3d` return and failure behavior unchanged; the wrapper adds no coercion or fallback. |

### `get_node_tags`

| Field | Detail |
| --- | --- |
| Access | `ctx.run.Nodes()` |
| Signature | `get_node_tags!(ctx.run, id)` |
| Params | `ctx, id` |
| Returns | `typed value from backing method` |
| Use when | Use `get_node_tags` to get node tags on the scene graph; guard stale IDs and concrete/base type mismatches. |
| Fails when / edge behavior | Uses the backing `get_node_tags` return and failure behavior unchanged; the wrapper adds no coercion or fallback. |

### `tag_set`

| Field | Detail |
| --- | --- |
| Access | `ctx.run.Nodes()` |
| Signature | `tag_set!(ctx.run, id, tags)` |
| Params | `ctx, id, tags` |
| Returns | `same as backing method` |
| Use when | Use `tag_set` to tag set on the scene graph; guard stale IDs and concrete/base type mismatches. |
| Fails when / edge behavior | Uses the backing `tag_set` return and failure behavior unchanged; the wrapper adds no coercion or fallback. |

### `tag_add`

| Field | Detail |
| --- | --- |
| Access | `ctx.run.Nodes()` |
| Signature | `tag_add!(ctx.run, id, tags)` |
| Params | `ctx, id, tags` |
| Returns | `bool or () as shown by backing method` |
| Use when | Use `tag_add` to tag add on the scene graph; guard stale IDs and concrete/base type mismatches. |
| Fails when / edge behavior | Returns `false` when `tag_add` cannot apply to the supplied target or inputs; `true` confirms success. |

### `tag_remove`

| Field | Detail |
| --- | --- |
| Access | `ctx.run.Nodes()` |
| Signature | `tag_remove!(ctx.run, id, tag)` |
| Params | `ctx, id, tag` |
| Returns | `bool or () as shown by backing method` |
| Use when | Use `tag_remove` to tag remove on the scene graph; guard stale IDs and concrete/base type mismatches. |
| Fails when / edge behavior | Returns `false` when `tag_remove` cannot apply to the supplied target or inputs; `true` confirms success. |

### `spawn`

| Field | Detail |
| --- | --- |
| Access | `ctx.run.Nodes()` |
| Signature | `spawn!(ctx.run, NodeTy, name, tags, parent, \|node\| { ... }) -> NodeID` |
| Params | `ctx, node_ty, [name], [tags], [parent], closure` |
| Returns | `NodeID` |
| Use when | Use when a single dynamically-computed node needs create + configure in one step (`create_node!` then `with_node_mut!`). Name/tags/parent are optional, matching `create_node!` arms. |
| Fails when / edge behavior | Configuration closure is a no-op if the created node type does not match; the id is still returned. |

### `find_node`

| Field | Detail |
| --- | --- |
| Access | `ctx.run.Nodes()` |
| Signature | `find_node!(ctx.run, root, name) -> Option<NodeID>` |
| Params | `ctx, root, name` |
| Returns | `Option<NodeID>` |
| Use when | Use to locate a node by name inside `root`'s subtree (including `root`). Index-backed: O(nodes sharing the name), not a tree walk. Pass `NodeID::nil()` as `root` to search the whole scene. |
| Fails when / edge behavior | Returns `None` when no node in scope has the name. |

### `descendants`

| Field | Detail |
| --- | --- |
| Access | `ctx.run.Nodes()` |
| Signature | `descendants!(ctx.run, root) -> Vec<NodeID>` |
| Params | `ctx, root` |
| Returns | `Vec<NodeID>` |
| Use when | Use to iterate `root` plus every descendant without a manual stack walk. |
| Fails when / edge behavior | Returns an empty vec when `root` is nil. |

### `set_tree_visible`

| Field | Detail |
| --- | --- |
| Access | `ctx.run.Nodes()` |
| Signature | `set_tree_visible!(ctx.run, root, visible) -> usize` |
| Params | `ctx, root, visible` |
| Returns | `usize` (count of UI nodes updated) |
| Use when | Use to toggle visibility across a whole `UiNode` subtree in one runtime call. Non-UI nodes are skipped. |
| Fails when / edge behavior | Returns `0` when `root` is nil or contains no UI nodes. |

### `broadcast_var`

| Field | Detail |
| --- | --- |
| Access | `ctx.run.Nodes()` + `ctx.run.Scripts()` |
| Signature | `broadcast_var!(ctx.run, root, member, value) -> usize` |
| Params | `ctx, root, member, value` |
| Returns | `usize` (count of nodes visited) |
| Use when | Use to set one script var across an entire subtree (for example pushing a shared setting into every demo node). `value` is cloned per node. |
| Fails when / edge behavior | Returns `0` when `root` is nil; nodes without the var silently ignore the set. |

### `look_at_3d`

| Field | Detail |
| --- | --- |
| Access | `ctx.run.Nodes()` |
| Signature | `look_at_3d!(ctx.run, node, target[, up]) -> bool` |
| Params | `ctx, node, target, [up]` |
| Returns | `bool` |
| Use when | Use to rotate a 3D spatial node to face a world-space point. Default up is world `+Y`. |
| Fails when / edge behavior | Returns `false` when the node has no 3D global transform. |

