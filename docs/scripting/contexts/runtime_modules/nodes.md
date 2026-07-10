# Nodes Module

## Page Map

| Header | Link |
| --- | --- |
| Overview | [Overview](#overview) |
| Context | [Context](#context) |
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

## Overview

This runtime module belongs to `ctx.run` and documents nodes calls.

## Context

- Script context path: `ctx.run`
- Module access: `ctx.run.Nodes()`
- Lifecycle examples stay inside `lifecycle!` because script hooks get `API` from the macro expansion.

## API Reference

### `create`

| Field | Detail |
| --- | --- |
| Access | `ctx.run.Nodes()` |
| Signature | `pub fn create<T>(&mut self) -> NodeID where T: Default + Into<SceneNodeData>,` |
| Params | `&mut self` |
| Returns | `NodeID where T: Default + Into<SceneNodeData>,` |
| Use when | Use when gameplay needs a new runtime/resource object built from typed data. |
| Fails when / edge behavior | Returns the documented empty value when backing runtime data is missing, stale, or the target type does not match. |

### `create_nodes`

| Field | Detail |
| --- | --- |
| Access | `ctx.run.Nodes()` |
| Signature | `pub fn create_nodes<'a, B>(&mut self, requests: B, parent_id: NodeID) -> Vec<NodeID> where B: IntoNodeCreateBatch<'a>` |
| Params | `&mut self, requests: NodeCollection / NodeSpec slice, parent_id: NodeID` |
| Returns | `Vec<NodeID>` |
| Use when | Use when gameplay needs a new runtime/resource object built from typed data. |
| Fails when / edge behavior | Returns the documented empty value when backing runtime data is missing, stale, or the target type does not match. |

### `with_node_mut`

| Field | Detail |
| --- | --- |
| Access | `ctx.run.Nodes()` |
| Signature | `pub fn with_node_mut<T, V, F>(&mut self, id: NodeID, f: F) -> Option<V> where T: NodeTypeDispatch, F: FnOnce(&mut T) -> V,` |
| Params | `&mut self, id: NodeID, f: F) -> Option<V> where T: NodeTypeDispatch, F: FnOnce(&mut T` |
| Returns | `Option<V> where T: NodeTypeDispatch, F: FnOnce(&mut T) -> V,` |
| Use when | Use when script code needs this exact engine read or write. |
| Fails when / edge behavior | Returns the documented empty value when backing runtime data is missing, stale, or the target type does not match. |

### `with_node`

| Field | Detail |
| --- | --- |
| Access | `ctx.run.Nodes()` |
| Signature | `pub fn with_node<T, V: Clone + Default>( &mut self, node_id: NodeID, f: impl FnOnce(&T) -> V, ) -> V where T: NodeTypeDispatch,` |
| Params | `&mut self, node_id: NodeID, f: impl FnOnce(&T) -> V,` |
| Returns | `V, ) -> V where T: NodeTypeDispatch,` |
| Use when | Use when script code needs this exact engine read or write. |
| Fails when / edge behavior | Returns the documented empty value when backing runtime data is missing, stale, or the target type does not match. |

### `with_base_node`

| Field | Detail |
| --- | --- |
| Access | `ctx.run.Nodes()` |
| Signature | `pub fn with_base_node<T, V, F>(&mut self, id: NodeID, f: F) -> Option<V> where T: NodeBaseDispatch, F: FnOnce(&T) -> V,` |
| Params | `&mut self, id: NodeID, f: F) -> Option<V> where T: NodeBaseDispatch, F: FnOnce(&T` |
| Returns | `Option<V> where T: NodeBaseDispatch, F: FnOnce(&T) -> V,` |
| Use when | Use when script code needs this exact engine read or write. |
| Fails when / edge behavior | Returns the documented empty value when backing runtime data is missing, stale, or the target type does not match. |

### `with_base_node_mut`

| Field | Detail |
| --- | --- |
| Access | `ctx.run.Nodes()` |
| Signature | `pub fn with_base_node_mut<T, V, F>(&mut self, id: NodeID, f: F) -> Option<V> where T: NodeBaseDispatch, F: FnOnce(&mut T) -> V,` |
| Params | `&mut self, id: NodeID, f: F) -> Option<V> where T: NodeBaseDispatch, F: FnOnce(&mut T` |
| Returns | `Option<V> where T: NodeBaseDispatch, F: FnOnce(&mut T) -> V,` |
| Use when | Use when script code needs this exact engine read or write. |
| Fails when / edge behavior | Returns the documented empty value when backing runtime data is missing, stale, or the target type does not match. |

### `get_node_name`

| Field | Detail |
| --- | --- |
| Access | `ctx.run.Nodes()` |
| Signature | `pub fn get_node_name(&mut self, node_id: NodeID) -> Option<Cow<'static, str>>` |
| Params | `&mut self, node_id: NodeID` |
| Returns | `Option<Cow<'static, str>>` |
| Use when | Use when gameplay needs to read typed engine data and react without owning the storage. |
| Fails when / edge behavior | Returns the documented empty value when backing runtime data is missing, stale, or the target type does not match. |

### `set_node_name`

| Field | Detail |
| --- | --- |
| Access | `ctx.run.Nodes()` |
| Signature | `pub fn set_node_name<S>(&mut self, node_id: NodeID, name: S) -> bool where S: Into<Cow<'static, str>>,` |
| Params | `&mut self, node_id: NodeID, name: S` |
| Returns | `bool where S: Into<Cow<'static, str>>,` |
| Use when | Use when gameplay must change engine state or queue an action this frame. |
| Fails when / edge behavior | Returns the documented empty value when backing runtime data is missing, stale, or the target type does not match. |

### `get_skeleton_bone_name`

| Field | Detail |
| --- | --- |
| Access | `ctx.run.Nodes()` |
| Signature | `pub fn get_skeleton_bone_name( &mut self, skeleton_id: NodeID, bone_index: usize, ) -> Option<Cow<'static, str>>` |
| Params | `&mut self, skeleton_id: NodeID, bone_index: usize,` |
| Returns | `Option<Cow<'static, str>>` |
| Use when | Use when gameplay needs to read typed engine data and react without owning the storage. |
| Fails when / edge behavior | Returns the documented empty value when backing runtime data is missing, stale, or the target type does not match. |

### `get_skeleton_bone_index`

| Field | Detail |
| --- | --- |
| Access | `ctx.run.Nodes()` |
| Signature | `pub fn get_skeleton_bone_index<S>(&mut self, skeleton_id: NodeID, bone_name: S) -> Option<usize> where S: AsRef<str>,` |
| Params | `&mut self, skeleton_id: NodeID, bone_name: S` |
| Returns | `Option<usize> where S: AsRef<str>,` |
| Use when | Use when gameplay needs to read typed engine data and react without owning the storage. |
| Fails when / edge behavior | Returns the documented empty value when backing runtime data is missing, stale, or the target type does not match. |

### `set_ui_rotation`

| Field | Detail |
| --- | --- |
| Access | `ctx.run.Nodes()` |
| Signature | `pub fn set_ui_rotation(&mut self, node_id: NodeID, rotation: f32) -> bool` |
| Params | `&mut self, node_id: NodeID, rotation: f32` |
| Returns | `bool` |
| Use when | Use when gameplay must change engine state or queue an action this frame. |
| Fails when / edge behavior | Returns the documented empty value when backing runtime data is missing, stale, or the target type does not match. |

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
| Use when | Use when gameplay must change engine state or queue an action this frame. |
| Fails when / edge behavior | Returns the documented empty value when backing runtime data is missing, stale, or the target type does not match. |

### `get_node_parent_id`

| Field | Detail |
| --- | --- |
| Access | `ctx.run.Nodes()` |
| Signature | `pub fn get_node_parent_id(&mut self, node_id: NodeID) -> Option<NodeID>` |
| Params | `&mut self, node_id: NodeID` |
| Returns | `Option<NodeID>` |
| Use when | Use when gameplay needs to read typed engine data and react without owning the storage. |
| Fails when / edge behavior | Returns the documented empty value when backing runtime data is missing, stale, or the target type does not match. |

### `get_node_children_ids`

| Field | Detail |
| --- | --- |
| Access | `ctx.run.Nodes()` |
| Signature | `pub fn get_node_children_ids(&mut self, node_id: NodeID) -> Option<Vec<NodeID>>` |
| Params | `&mut self, node_id: NodeID` |
| Returns | `Option<Vec<NodeID>>` |
| Use when | Use when gameplay needs to read typed engine data and react without owning the storage. |
| Fails when / edge behavior | Returns the documented empty value when backing runtime data is missing, stale, or the target type does not match. |

### `get_children`

| Field | Detail |
| --- | --- |
| Access | `ctx.run.Nodes()` |
| Signature | `pub fn get_children(&mut self, node_id: NodeID) -> Vec<NodeID>` |
| Params | `&mut self, node_id: NodeID` |
| Returns | `Vec<NodeID>` |
| Use when | Use when gameplay needs to read typed engine data and react without owning the storage. |
| Fails when / edge behavior | Returns the documented empty value when backing runtime data is missing, stale, or the target type does not match. |

### `get_child_at`

| Field | Detail |
| --- | --- |
| Access | `ctx.run.Nodes()` |
| Signature | `pub fn get_child_at(&mut self, parent_id: NodeID, index: usize) -> Option<NodeID>` |
| Params | `&mut self, parent_id: NodeID, index: usize` |
| Returns | `Option<NodeID>` |
| Use when | Use when gameplay needs to read typed engine data and react without owning the storage. |
| Fails when / edge behavior | Returns the documented empty value when backing runtime data is missing, stale, or the target type does not match. |

### `get_child_by_name`

| Field | Detail |
| --- | --- |
| Access | `ctx.run.Nodes()` |
| Signature | `pub fn get_child_by_name<S>(&mut self, parent_id: NodeID, name: S) -> Option<NodeID> where S: AsRef<str>,` |
| Params | `&mut self, parent_id: NodeID, name: S` |
| Returns | `Option<NodeID> where S: AsRef<str>,` |
| Use when | Use when gameplay needs to read typed engine data and react without owning the storage. |
| Fails when / edge behavior | Returns the documented empty value when backing runtime data is missing, stale, or the target type does not match. |

### `get_children_by_name`

| Field | Detail |
| --- | --- |
| Access | `ctx.run.Nodes()` |
| Signature | `pub fn get_children_by_name<S>(&mut self, parent_id: NodeID, name: S) -> Vec<NodeID> where S: AsRef<str>,` |
| Params | `&mut self, parent_id: NodeID, name: S` |
| Returns | `Vec<NodeID> where S: AsRef<str>,` |
| Use when | Use when gameplay needs to read typed engine data and react without owning the storage. |
| Fails when / edge behavior | Returns the documented empty value when backing runtime data is missing, stale, or the target type does not match. |

### `get_child`

| Field | Detail |
| --- | --- |
| Access | `ctx.run.Nodes()` |
| Signature | `pub fn get_child<T>(&mut self, parent_id: NodeID, selector: T) -> Option<NodeID> where T: IntoChildSelector,` |
| Params | `&mut self, parent_id: NodeID, selector: T` |
| Returns | `Option<NodeID> where T: IntoChildSelector,` |
| Use when | Use when gameplay needs to read typed engine data and react without owning the storage. |
| Fails when / edge behavior | Returns the documented empty value when backing runtime data is missing, stale, or the target type does not match. |

### `get_node_type`

| Field | Detail |
| --- | --- |
| Access | `ctx.run.Nodes()` |
| Signature | `pub fn get_node_type(&mut self, node_id: NodeID) -> Option<NodeType>` |
| Params | `&mut self, node_id: NodeID` |
| Returns | `Option<NodeType>` |
| Use when | Use when gameplay needs to read typed engine data and react without owning the storage. |
| Fails when / edge behavior | Returns the documented empty value when backing runtime data is missing, stale, or the target type does not match. |

### `reparent`

| Field | Detail |
| --- | --- |
| Access | `ctx.run.Nodes()` |
| Signature | `pub fn reparent(&mut self, parent_id: NodeID, child_id: NodeID) -> bool` |
| Params | `&mut self, parent_id: NodeID, child_id: NodeID` |
| Returns | `bool` |
| Use when | Use when script code needs this exact engine read or write. |
| Fails when / edge behavior | Returns the documented empty value when backing runtime data is missing, stale, or the target type does not match. |

### `force_rerender`

| Field | Detail |
| --- | --- |
| Access | `ctx.run.Nodes()` |
| Signature | `pub fn force_rerender(&mut self, root_id: NodeID) -> bool` |
| Params | `&mut self, root_id: NodeID` |
| Returns | `bool` |
| Use when | Use when gameplay must change engine state or queue an action this frame. |
| Fails when / edge behavior | Returns the documented empty value when backing runtime data is missing, stale, or the target type does not match. |

### `mark_needs_rerender`

| Field | Detail |
| --- | --- |
| Access | `ctx.run.Nodes()` |
| Signature | `pub fn mark_needs_rerender(&mut self, node_id: NodeID) -> bool` |
| Params | `&mut self, node_id: NodeID` |
| Returns | `bool` |
| Use when | Use when script code needs this exact engine read or write. |
| Fails when / edge behavior | Returns the documented empty value when backing runtime data is missing, stale, or the target type does not match. |

### `reparent_multi`

| Field | Detail |
| --- | --- |
| Access | `ctx.run.Nodes()` |
| Signature | `pub fn reparent_multi<I>(&mut self, parent_id: NodeID, child_ids: I) -> usize where I: IntoIterator<Item = NodeID>,` |
| Params | `&mut self, parent_id: NodeID, child_ids: I` |
| Returns | `usize where I: IntoIterator<Item = NodeID>,` |
| Use when | Use when script code needs this exact engine read or write. |
| Fails when / edge behavior | Returns the documented empty value when backing runtime data is missing, stale, or the target type does not match. |

### `remove_node`

| Field | Detail |
| --- | --- |
| Access | `ctx.run.Nodes()` |
| Signature | `pub fn remove_node(&mut self, node_id: NodeID) -> bool` |
| Params | `&mut self, node_id: NodeID` |
| Returns | `bool` |
| Use when | Use when code must release, remove, stop, or disconnect existing engine state. |
| Fails when / edge behavior | Returns the documented empty value when backing runtime data is missing, stale, or the target type does not match. |

### `get_node_tags`

| Field | Detail |
| --- | --- |
| Access | `ctx.run.Nodes()` |
| Signature | `pub fn get_node_tags(&mut self, node_id: NodeID) -> Option<Vec<Cow<'static, str>>>` |
| Params | `&mut self, node_id: NodeID` |
| Returns | `Option<Vec<Cow<'static, str>>>` |
| Use when | Use when gameplay needs to read typed engine data and react without owning the storage. |
| Fails when / edge behavior | Returns the documented empty value when backing runtime data is missing, stale, or the target type does not match. |

### `tag_set`

| Field | Detail |
| --- | --- |
| Access | `ctx.run.Nodes()` |
| Signature | `pub fn tag_set<T>(&mut self, node_id: NodeID, tags: Option<T>) -> bool where T: IntoNodeTags,` |
| Params | `&mut self, node_id: NodeID, tags: Option<T>` |
| Returns | `bool where T: IntoNodeTags,` |
| Use when | Use when script code needs this exact engine read or write. |
| Fails when / edge behavior | Returns the documented empty value when backing runtime data is missing, stale, or the target type does not match. |

### `add_node_tag`

| Field | Detail |
| --- | --- |
| Access | `ctx.run.Nodes()` |
| Signature | `pub fn add_node_tag<T>(&mut self, node_id: NodeID, tag: T) -> bool where T: IntoNodeTag,` |
| Params | `&mut self, node_id: NodeID, tag: T` |
| Returns | `bool where T: IntoNodeTag,` |
| Use when | Use when script code needs this exact engine read or write. |
| Fails when / edge behavior | Returns the documented empty value when backing runtime data is missing, stale, or the target type does not match. |

### `add_node_tags`

| Field | Detail |
| --- | --- |
| Access | `ctx.run.Nodes()` |
| Signature | `pub fn add_node_tags<T>(&mut self, node_id: NodeID, tags: T) -> bool where T: IntoNodeTags,` |
| Params | `&mut self, node_id: NodeID, tags: T` |
| Returns | `bool where T: IntoNodeTags,` |
| Use when | Use when script code needs this exact engine read or write. |
| Fails when / edge behavior | Returns the documented empty value when backing runtime data is missing, stale, or the target type does not match. |

### `remove_node_tag`

| Field | Detail |
| --- | --- |
| Access | `ctx.run.Nodes()` |
| Signature | `pub fn remove_node_tag<T>(&mut self, node_id: NodeID, tag: T) -> bool where T: IntoTagID,` |
| Params | `&mut self, node_id: NodeID, tag: T` |
| Returns | `bool where T: IntoTagID,` |
| Use when | Use when code must release, remove, stop, or disconnect existing engine state. |
| Fails when / edge behavior | Returns the documented empty value when backing runtime data is missing, stale, or the target type does not match. |

### `get_global_transform_2d`

| Field | Detail |
| --- | --- |
| Access | `ctx.run.Nodes()` |
| Signature | `pub fn get_global_transform_2d(&mut self, node_id: NodeID) -> Option<Transform2D>` |
| Params | `&mut self, node_id: NodeID` |
| Returns | `Option<Transform2D>` |
| Use when | Use when gameplay needs to read typed engine data and react without owning the storage. |
| Fails when / edge behavior | Returns the documented empty value when backing runtime data is missing, stale, or the target type does not match. |

### `get_global_transform_3d`

| Field | Detail |
| --- | --- |
| Access | `ctx.run.Nodes()` |
| Signature | `pub fn get_global_transform_3d(&mut self, node_id: NodeID) -> Option<Transform3D>` |
| Params | `&mut self, node_id: NodeID` |
| Returns | `Option<Transform3D>` |
| Use when | Use when gameplay needs to read typed engine data and react without owning the storage. |
| Fails when / edge behavior | Returns the documented empty value when backing runtime data is missing, stale, or the target type does not match. |

### `get_local_transform_2d`

| Field | Detail |
| --- | --- |
| Access | `ctx.run.Nodes()` |
| Signature | `pub fn get_local_transform_2d(&mut self, node_id: NodeID) -> Option<Transform2D>` |
| Params | `&mut self, node_id: NodeID` |
| Returns | `Option<Transform2D>` |
| Use when | Use when gameplay needs to read typed engine data and react without owning the storage. |
| Fails when / edge behavior | Returns the documented empty value when backing runtime data is missing, stale, or the target type does not match. |

### `get_local_transform_3d`

| Field | Detail |
| --- | --- |
| Access | `ctx.run.Nodes()` |
| Signature | `pub fn get_local_transform_3d(&mut self, node_id: NodeID) -> Option<Transform3D>` |
| Params | `&mut self, node_id: NodeID` |
| Returns | `Option<Transform3D>` |
| Use when | Use when gameplay needs to read typed engine data and react without owning the storage. |
| Fails when / edge behavior | Returns the documented empty value when backing runtime data is missing, stale, or the target type does not match. |

### `set_local_transform_2d`

| Field | Detail |
| --- | --- |
| Access | `ctx.run.Nodes()` |
| Signature | `pub fn set_local_transform_2d(&mut self, node_id: NodeID, transform: Transform2D) -> bool` |
| Params | `&mut self, node_id: NodeID, transform: Transform2D` |
| Returns | `bool` |
| Use when | Use when gameplay must change engine state or queue an action this frame. |
| Fails when / edge behavior | Returns the documented empty value when backing runtime data is missing, stale, or the target type does not match. |

### `set_local_transform_3d`

| Field | Detail |
| --- | --- |
| Access | `ctx.run.Nodes()` |
| Signature | `pub fn set_local_transform_3d(&mut self, node_id: NodeID, transform: Transform3D) -> bool` |
| Params | `&mut self, node_id: NodeID, transform: Transform3D` |
| Returns | `bool` |
| Use when | Use when gameplay must change engine state or queue an action this frame. |
| Fails when / edge behavior | Returns the documented empty value when backing runtime data is missing, stale, or the target type does not match. |

### `set_global_transform_2d`

| Field | Detail |
| --- | --- |
| Access | `ctx.run.Nodes()` |
| Signature | `pub fn set_global_transform_2d(&mut self, node_id: NodeID, global: Transform2D) -> bool` |
| Params | `&mut self, node_id: NodeID, global: Transform2D` |
| Returns | `bool` |
| Use when | Use when gameplay must change engine state or queue an action this frame. |
| Fails when / edge behavior | Returns the documented empty value when backing runtime data is missing, stale, or the target type does not match. |

### `set_global_transform_3d`

| Field | Detail |
| --- | --- |
| Access | `ctx.run.Nodes()` |
| Signature | `pub fn set_global_transform_3d(&mut self, node_id: NodeID, global: Transform3D) -> bool` |
| Params | `&mut self, node_id: NodeID, global: Transform3D` |
| Returns | `bool` |
| Use when | Use when gameplay must change engine state or queue an action this frame. |
| Fails when / edge behavior | Returns the documented empty value when backing runtime data is missing, stale, or the target type does not match. |

### `get_local_pos_2d`

| Field | Detail |
| --- | --- |
| Access | `ctx.run.Nodes()` |
| Signature | `pub fn get_local_pos_2d(&mut self, node_id: NodeID) -> Option<Vector2>` |
| Params | `&mut self, node_id: NodeID` |
| Returns | `Option<Vector2>` |
| Use when | Use when gameplay needs to read typed engine data and react without owning the storage. |
| Fails when / edge behavior | Returns the documented empty value when backing runtime data is missing, stale, or the target type does not match. |

### `get_local_pos_3d`

| Field | Detail |
| --- | --- |
| Access | `ctx.run.Nodes()` |
| Signature | `pub fn get_local_pos_3d(&mut self, node_id: NodeID) -> Option<Vector3>` |
| Params | `&mut self, node_id: NodeID` |
| Returns | `Option<Vector3>` |
| Use when | Use when gameplay needs to read typed engine data and react without owning the storage. |
| Fails when / edge behavior | Returns the documented empty value when backing runtime data is missing, stale, or the target type does not match. |

### `set_local_pos_2d`

| Field | Detail |
| --- | --- |
| Access | `ctx.run.Nodes()` |
| Signature | `pub fn set_local_pos_2d(&mut self, node_id: NodeID, pos: Vector2) -> bool` |
| Params | `&mut self, node_id: NodeID, pos: Vector2` |
| Returns | `bool` |
| Use when | Use when gameplay must change engine state or queue an action this frame. |
| Fails when / edge behavior | Returns the documented empty value when backing runtime data is missing, stale, or the target type does not match. |

### `set_local_pos_3d`

| Field | Detail |
| --- | --- |
| Access | `ctx.run.Nodes()` |
| Signature | `pub fn set_local_pos_3d(&mut self, node_id: NodeID, pos: Vector3) -> bool` |
| Params | `&mut self, node_id: NodeID, pos: Vector3` |
| Returns | `bool` |
| Use when | Use when gameplay must change engine state or queue an action this frame. |
| Fails when / edge behavior | Returns the documented empty value when backing runtime data is missing, stale, or the target type does not match. |

### `get_global_pos_2d`

| Field | Detail |
| --- | --- |
| Access | `ctx.run.Nodes()` |
| Signature | `pub fn get_global_pos_2d(&mut self, node_id: NodeID) -> Option<Vector2>` |
| Params | `&mut self, node_id: NodeID` |
| Returns | `Option<Vector2>` |
| Use when | Use when gameplay needs to read typed engine data and react without owning the storage. |
| Fails when / edge behavior | Returns the documented empty value when backing runtime data is missing, stale, or the target type does not match. |

### `get_global_pos_3d`

| Field | Detail |
| --- | --- |
| Access | `ctx.run.Nodes()` |
| Signature | `pub fn get_global_pos_3d(&mut self, node_id: NodeID) -> Option<Vector3>` |
| Params | `&mut self, node_id: NodeID` |
| Returns | `Option<Vector3>` |
| Use when | Use when gameplay needs to read typed engine data and react without owning the storage. |
| Fails when / edge behavior | Returns the documented empty value when backing runtime data is missing, stale, or the target type does not match. |

### `set_global_pos_2d`

| Field | Detail |
| --- | --- |
| Access | `ctx.run.Nodes()` |
| Signature | `pub fn set_global_pos_2d(&mut self, node_id: NodeID, pos: Vector2) -> bool` |
| Params | `&mut self, node_id: NodeID, pos: Vector2` |
| Returns | `bool` |
| Use when | Use when gameplay must change engine state or queue an action this frame. |
| Fails when / edge behavior | Returns the documented empty value when backing runtime data is missing, stale, or the target type does not match. |

### `set_global_pos_3d`

| Field | Detail |
| --- | --- |
| Access | `ctx.run.Nodes()` |
| Signature | `pub fn set_global_pos_3d(&mut self, node_id: NodeID, pos: Vector3) -> bool` |
| Params | `&mut self, node_id: NodeID, pos: Vector3` |
| Returns | `bool` |
| Use when | Use when gameplay must change engine state or queue an action this frame. |
| Fails when / edge behavior | Returns the documented empty value when backing runtime data is missing, stale, or the target type does not match. |

### `get_local_rot_2d`

| Field | Detail |
| --- | --- |
| Access | `ctx.run.Nodes()` |
| Signature | `pub fn get_local_rot_2d(&mut self, node_id: NodeID) -> Option<f32>` |
| Params | `&mut self, node_id: NodeID` |
| Returns | `Option<f32>` |
| Use when | Use when gameplay needs to read typed engine data and react without owning the storage. |
| Fails when / edge behavior | Returns the documented empty value when backing runtime data is missing, stale, or the target type does not match. |

### `get_local_rot_3d`

| Field | Detail |
| --- | --- |
| Access | `ctx.run.Nodes()` |
| Signature | `pub fn get_local_rot_3d(&mut self, node_id: NodeID) -> Option<Quaternion>` |
| Params | `&mut self, node_id: NodeID` |
| Returns | `Option<Quaternion>` |
| Use when | Use when gameplay needs to read typed engine data and react without owning the storage. |
| Fails when / edge behavior | Returns the documented empty value when backing runtime data is missing, stale, or the target type does not match. |

### `set_local_rot_2d`

| Field | Detail |
| --- | --- |
| Access | `ctx.run.Nodes()` |
| Signature | `pub fn set_local_rot_2d(&mut self, node_id: NodeID, rot: f32) -> bool` |
| Params | `&mut self, node_id: NodeID, rot: f32` |
| Returns | `bool` |
| Use when | Use when gameplay must change engine state or queue an action this frame. |
| Fails when / edge behavior | Returns the documented empty value when backing runtime data is missing, stale, or the target type does not match. |

### `set_local_rot_3d`

| Field | Detail |
| --- | --- |
| Access | `ctx.run.Nodes()` |
| Signature | `pub fn set_local_rot_3d(&mut self, node_id: NodeID, rot: Quaternion) -> bool` |
| Params | `&mut self, node_id: NodeID, rot: Quaternion` |
| Returns | `bool` |
| Use when | Use when gameplay must change engine state or queue an action this frame. |
| Fails when / edge behavior | Returns the documented empty value when backing runtime data is missing, stale, or the target type does not match. |

### `get_global_rot_2d`

| Field | Detail |
| --- | --- |
| Access | `ctx.run.Nodes()` |
| Signature | `pub fn get_global_rot_2d(&mut self, node_id: NodeID) -> Option<f32>` |
| Params | `&mut self, node_id: NodeID` |
| Returns | `Option<f32>` |
| Use when | Use when gameplay needs to read typed engine data and react without owning the storage. |
| Fails when / edge behavior | Returns the documented empty value when backing runtime data is missing, stale, or the target type does not match. |

### `get_global_rot_3d`

| Field | Detail |
| --- | --- |
| Access | `ctx.run.Nodes()` |
| Signature | `pub fn get_global_rot_3d(&mut self, node_id: NodeID) -> Option<Quaternion>` |
| Params | `&mut self, node_id: NodeID` |
| Returns | `Option<Quaternion>` |
| Use when | Use when gameplay needs to read typed engine data and react without owning the storage. |
| Fails when / edge behavior | Returns the documented empty value when backing runtime data is missing, stale, or the target type does not match. |

### `set_global_rot_2d`

| Field | Detail |
| --- | --- |
| Access | `ctx.run.Nodes()` |
| Signature | `pub fn set_global_rot_2d(&mut self, node_id: NodeID, rot: f32) -> bool` |
| Params | `&mut self, node_id: NodeID, rot: f32` |
| Returns | `bool` |
| Use when | Use when gameplay must change engine state or queue an action this frame. |
| Fails when / edge behavior | Returns the documented empty value when backing runtime data is missing, stale, or the target type does not match. |

### `set_global_rot_3d`

| Field | Detail |
| --- | --- |
| Access | `ctx.run.Nodes()` |
| Signature | `pub fn set_global_rot_3d(&mut self, node_id: NodeID, rot: Quaternion) -> bool` |
| Params | `&mut self, node_id: NodeID, rot: Quaternion` |
| Returns | `bool` |
| Use when | Use when gameplay must change engine state or queue an action this frame. |
| Fails when / edge behavior | Returns the documented empty value when backing runtime data is missing, stale, or the target type does not match. |

### `get_local_scale_2d`

| Field | Detail |
| --- | --- |
| Access | `ctx.run.Nodes()` |
| Signature | `pub fn get_local_scale_2d(&mut self, node_id: NodeID) -> Option<Vector2>` |
| Params | `&mut self, node_id: NodeID` |
| Returns | `Option<Vector2>` |
| Use when | Use when gameplay needs to read typed engine data and react without owning the storage. |
| Fails when / edge behavior | Returns the documented empty value when backing runtime data is missing, stale, or the target type does not match. |

### `get_local_scale_3d`

| Field | Detail |
| --- | --- |
| Access | `ctx.run.Nodes()` |
| Signature | `pub fn get_local_scale_3d(&mut self, node_id: NodeID) -> Option<Vector3>` |
| Params | `&mut self, node_id: NodeID` |
| Returns | `Option<Vector3>` |
| Use when | Use when gameplay needs to read typed engine data and react without owning the storage. |
| Fails when / edge behavior | Returns the documented empty value when backing runtime data is missing, stale, or the target type does not match. |

### `set_local_scale_2d`

| Field | Detail |
| --- | --- |
| Access | `ctx.run.Nodes()` |
| Signature | `pub fn set_local_scale_2d(&mut self, node_id: NodeID, scale: Vector2) -> bool` |
| Params | `&mut self, node_id: NodeID, scale: Vector2` |
| Returns | `bool` |
| Use when | Use when gameplay must change engine state or queue an action this frame. |
| Fails when / edge behavior | Returns the documented empty value when backing runtime data is missing, stale, or the target type does not match. |

### `set_local_scale_3d`

| Field | Detail |
| --- | --- |
| Access | `ctx.run.Nodes()` |
| Signature | `pub fn set_local_scale_3d(&mut self, node_id: NodeID, scale: Vector3) -> bool` |
| Params | `&mut self, node_id: NodeID, scale: Vector3` |
| Returns | `bool` |
| Use when | Use when gameplay must change engine state or queue an action this frame. |
| Fails when / edge behavior | Returns the documented empty value when backing runtime data is missing, stale, or the target type does not match. |

### `get_global_scale_2d`

| Field | Detail |
| --- | --- |
| Access | `ctx.run.Nodes()` |
| Signature | `pub fn get_global_scale_2d(&mut self, node_id: NodeID) -> Option<Vector2>` |
| Params | `&mut self, node_id: NodeID` |
| Returns | `Option<Vector2>` |
| Use when | Use when gameplay needs to read typed engine data and react without owning the storage. |
| Fails when / edge behavior | Returns the documented empty value when backing runtime data is missing, stale, or the target type does not match. |

### `get_global_scale_3d`

| Field | Detail |
| --- | --- |
| Access | `ctx.run.Nodes()` |
| Signature | `pub fn get_global_scale_3d(&mut self, node_id: NodeID) -> Option<Vector3>` |
| Params | `&mut self, node_id: NodeID` |
| Returns | `Option<Vector3>` |
| Use when | Use when gameplay needs to read typed engine data and react without owning the storage. |
| Fails when / edge behavior | Returns the documented empty value when backing runtime data is missing, stale, or the target type does not match. |

### `set_global_scale_2d`

| Field | Detail |
| --- | --- |
| Access | `ctx.run.Nodes()` |
| Signature | `pub fn set_global_scale_2d(&mut self, node_id: NodeID, scale: Vector2) -> bool` |
| Params | `&mut self, node_id: NodeID, scale: Vector2` |
| Returns | `bool` |
| Use when | Use when gameplay must change engine state or queue an action this frame. |
| Fails when / edge behavior | Returns the documented empty value when backing runtime data is missing, stale, or the target type does not match. |

### `set_global_scale_3d`

| Field | Detail |
| --- | --- |
| Access | `ctx.run.Nodes()` |
| Signature | `pub fn set_global_scale_3d(&mut self, node_id: NodeID, scale: Vector3) -> bool` |
| Params | `&mut self, node_id: NodeID, scale: Vector3` |
| Returns | `bool` |
| Use when | Use when gameplay must change engine state or queue an action this frame. |
| Fails when / edge behavior | Returns the documented empty value when backing runtime data is missing, stale, or the target type does not match. |

### `to_global_point_2d`

| Field | Detail |
| --- | --- |
| Access | `ctx.run.Nodes()` |
| Signature | `pub fn to_global_point_2d(&mut self, node_id: NodeID, local: Vector2) -> Option<Vector2>` |
| Params | `&mut self, node_id: NodeID, local: Vector2` |
| Returns | `Option<Vector2>` |
| Use when | Use when converting points or transforms between local node space and world space. |
| Fails when / edge behavior | Returns the documented empty value when backing runtime data is missing, stale, or the target type does not match. |

### `to_local_point_2d`

| Field | Detail |
| --- | --- |
| Access | `ctx.run.Nodes()` |
| Signature | `pub fn to_local_point_2d(&mut self, node_id: NodeID, global: Vector2) -> Option<Vector2>` |
| Params | `&mut self, node_id: NodeID, global: Vector2` |
| Returns | `Option<Vector2>` |
| Use when | Use when converting points or transforms between local node space and world space. |
| Fails when / edge behavior | Returns the documented empty value when backing runtime data is missing, stale, or the target type does not match. |

### `to_global_point_3d`

| Field | Detail |
| --- | --- |
| Access | `ctx.run.Nodes()` |
| Signature | `pub fn to_global_point_3d(&mut self, node_id: NodeID, local: Vector3) -> Option<Vector3>` |
| Params | `&mut self, node_id: NodeID, local: Vector3` |
| Returns | `Option<Vector3>` |
| Use when | Use when converting points or transforms between local node space and world space. |
| Fails when / edge behavior | Returns the documented empty value when backing runtime data is missing, stale, or the target type does not match. |

### `to_local_point_3d`

| Field | Detail |
| --- | --- |
| Access | `ctx.run.Nodes()` |
| Signature | `pub fn to_local_point_3d(&mut self, node_id: NodeID, global: Vector3) -> Option<Vector3>` |
| Params | `&mut self, node_id: NodeID, global: Vector3` |
| Returns | `Option<Vector3>` |
| Use when | Use when converting points or transforms between local node space and world space. |
| Fails when / edge behavior | Returns the documented empty value when backing runtime data is missing, stale, or the target type does not match. |

### `to_global_transform_2d`

| Field | Detail |
| --- | --- |
| Access | `ctx.run.Nodes()` |
| Signature | `pub fn to_global_transform_2d( &mut self, node_id: NodeID, local: Transform2D, ) -> Option<Transform2D>` |
| Params | `&mut self, node_id: NodeID, local: Transform2D,` |
| Returns | `Option<Transform2D>` |
| Use when | Use when converting points or transforms between local node space and world space. |
| Fails when / edge behavior | Returns the documented empty value when backing runtime data is missing, stale, or the target type does not match. |

### `to_local_transform_2d`

| Field | Detail |
| --- | --- |
| Access | `ctx.run.Nodes()` |
| Signature | `pub fn to_local_transform_2d( &mut self, node_id: NodeID, global: Transform2D, ) -> Option<Transform2D>` |
| Params | `&mut self, node_id: NodeID, global: Transform2D,` |
| Returns | `Option<Transform2D>` |
| Use when | Use when converting points or transforms between local node space and world space. |
| Fails when / edge behavior | Returns the documented empty value when backing runtime data is missing, stale, or the target type does not match. |

### `to_global_transform_3d`

| Field | Detail |
| --- | --- |
| Access | `ctx.run.Nodes()` |
| Signature | `pub fn to_global_transform_3d( &mut self, node_id: NodeID, local: Transform3D, ) -> Option<Transform3D>` |
| Params | `&mut self, node_id: NodeID, local: Transform3D,` |
| Returns | `Option<Transform3D>` |
| Use when | Use when converting points or transforms between local node space and world space. |
| Fails when / edge behavior | Returns the documented empty value when backing runtime data is missing, stale, or the target type does not match. |

### `to_local_transform_3d`

| Field | Detail |
| --- | --- |
| Access | `ctx.run.Nodes()` |
| Signature | `pub fn to_local_transform_3d( &mut self, node_id: NodeID, global: Transform3D, ) -> Option<Transform3D>` |
| Params | `&mut self, node_id: NodeID, global: Transform3D,` |
| Returns | `Option<Transform3D>` |
| Use when | Use when converting points or transforms between local node space and world space. |
| Fails when / edge behavior | Returns the documented empty value when backing runtime data is missing, stale, or the target type does not match. |

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
| Use when | Use when gameplay needs to read typed engine data and react without owning the storage. |
| Fails when / edge behavior | Returns the documented empty value when backing runtime data is missing, stale, or the target type does not match. |

### `mesh_instance_surface_on_global_ray`

| Field | Detail |
| --- | --- |
| Access | `ctx.run.Nodes()` |
| Signature | `pub fn mesh_instance_surface_on_global_ray( &mut self, node_id: NodeID, ray_origin: Vector3, ray_direction: Vector3, max_distance: f32, ) -> Option<MeshSurfaceHit3D>` |
| Params | `&mut self, node_id: NodeID, ray_origin: Vector3, ray_direction: Vector3, max_distance: f32,` |
| Returns | `Option<MeshSurfaceHit3D>` |
| Use when | Use when gameplay needs to read typed engine data and react without owning the storage. |
| Fails when / edge behavior | Returns the documented empty value when backing runtime data is missing, stale, or the target type does not match. |

### `mesh_instance_surfaces_on_global_rays`

| Field | Detail |
| --- | --- |
| Access | `ctx.run.Nodes()` |
| Signature | `pub fn mesh_instance_surfaces_on_global_rays( &mut self, node_id: NodeID, rays: &[MeshSurfaceRay3D], resolve_material: bool, ) -> Vec<Option<MeshSurfaceHit3D>>` |
| Params | `&mut self, node_id: NodeID, rays: &[MeshSurfaceRay3D], resolve_material: bool,` |
| Returns | `Vec<Option<MeshSurfaceHit3D>>` |
| Use when | Use when gameplay needs to read typed engine data and react without owning the storage. |
| Fails when / edge behavior | Returns the documented empty value when backing runtime data is missing, stale, or the target type does not match. |

### `mesh_instance_material_regions`

| Field | Detail |
| --- | --- |
| Access | `ctx.run.Nodes()` |
| Signature | `pub fn mesh_instance_material_regions( &mut self, node_id: NodeID, material: MaterialID, ) -> Vec<MeshMaterialRegion3D>` |
| Params | `&mut self, node_id: NodeID, material: MaterialID,` |
| Returns | `Vec<MeshMaterialRegion3D>` |
| Use when | Use when gameplay needs to read typed engine data and react without owning the storage. |
| Fails when / edge behavior | Returns the documented empty value when backing runtime data is missing, stale, or the target type does not match. |

### `mesh_data_surface_at_local_point`

| Field | Detail |
| --- | --- |
| Access | `ctx.run.Nodes()` |
| Signature | `pub fn mesh_data_surface_at_local_point( &mut self, mesh_id: MeshID, local_point: Vector3, ) -> Option<MeshDataSurfaceHit3D>` |
| Params | `&mut self, mesh_id: MeshID, local_point: Vector3,` |
| Returns | `Option<MeshDataSurfaceHit3D>` |
| Use when | Use when gameplay needs to read typed engine data and react without owning the storage. |
| Fails when / edge behavior | Returns the documented empty value when backing runtime data is missing, stale, or the target type does not match. |

### `mesh_data_surface_on_local_ray`

| Field | Detail |
| --- | --- |
| Access | `ctx.run.Nodes()` |
| Signature | `pub fn mesh_data_surface_on_local_ray( &mut self, mesh_id: MeshID, ray_origin_local: Vector3, ray_direction_local: Vector3, max_distance: f32, ) -> Option<MeshDataSurfaceHit3D>` |
| Params | `&mut self, mesh_id: MeshID, ray_origin_local: Vector3, ray_direction_local: Vector3, max_distance: f32,` |
| Returns | `Option<MeshDataSurfaceHit3D>` |
| Use when | Use when gameplay needs to read typed engine data and react without owning the storage. |
| Fails when / edge behavior | Returns the documented empty value when backing runtime data is missing, stale, or the target type does not match. |

### `mesh_data_surface_regions`

| Field | Detail |
| --- | --- |
| Access | `ctx.run.Nodes()` |
| Signature | `pub fn mesh_data_surface_regions( &mut self, mesh_id: MeshID, surface_index: u32, ) -> Vec<MeshDataSurfaceRegion3D>` |
| Params | `&mut self, mesh_id: MeshID, surface_index: u32,` |
| Returns | `Vec<MeshDataSurfaceRegion3D>` |
| Use when | Use when gameplay needs to read typed engine data and react without owning the storage. |
| Fails when / edge behavior | Returns the documented empty value when backing runtime data is missing, stale, or the target type does not match. |

### `with_node_mut`

| Field | Detail |
| --- | --- |
| Access | `ctx.run.Nodes()` |
| Signature | `with_node_mut!(ctx.run, node_ty, id, f)` |
| Params | `ctx, node_ty, id, f` |
| Returns | `same as backing method` |
| Use when | Use when script code needs this exact engine read or write. |
| Fails when / edge behavior | Returns the documented empty value when backing runtime data is missing, stale, or the target type does not match. |

### `with_node`

| Field | Detail |
| --- | --- |
| Access | `ctx.run.Nodes()` |
| Signature | `with_node!(ctx.run, node_ty, id, f)` |
| Params | `ctx, node_ty, id, f` |
| Returns | `same as backing method` |
| Use when | Use when script code needs this exact engine read or write. |
| Fails when / edge behavior | Returns the documented empty value when backing runtime data is missing, stale, or the target type does not match. |

### `with_base_node`

| Field | Detail |
| --- | --- |
| Access | `ctx.run.Nodes()` |
| Signature | `with_base_node!(ctx.run, base_ty, id, f)` |
| Params | `ctx, base_ty, id, f` |
| Returns | `same as backing method` |
| Use when | Use when script code needs this exact engine read or write. |
| Fails when / edge behavior | Returns the documented empty value when backing runtime data is missing, stale, or the target type does not match. |

### `with_base_node_mut`

| Field | Detail |
| --- | --- |
| Access | `ctx.run.Nodes()` |
| Signature | `with_base_node_mut!(ctx.run, base_ty, id, f)` |
| Params | `ctx, base_ty, id, f` |
| Returns | `same as backing method` |
| Use when | Use when script code needs this exact engine read or write. |
| Fails when / edge behavior | Returns the documented empty value when backing runtime data is missing, stale, or the target type does not match. |

### `create_node`

| Field | Detail |
| --- | --- |
| Access | `ctx.run.Nodes()` |
| Signature | `create_node!(ctx.run, node_ty)` |
| Params | `ctx, node_ty` |
| Returns | `resource/runtime ID or `Result` as shown by backing method` |
| Use when | Use when gameplay needs a new runtime/resource object built from typed data. |
| Fails when / edge behavior | Returns the documented empty value when backing runtime data is missing, stale, or the target type does not match. |

### `node_collection`

| Field | Detail |
| --- | --- |
| Access | `ctx.run.Nodes()` |
| Signature | `node_collection!({ ... })` or `node_collection![{ ... }, { ... }]` |
| Params | `name =`, `tags =`, `node =`, optional `children = [...]`, or `collection = expr` |
| Returns | `NodeCollection` |
| Use when | Use when gameplay needs an in-code scene graph with typed node data. |
| Fails when / edge behavior | Returns the documented empty value when backing runtime data is missing, stale, or the target type does not match. |

### `create_nodes`

| Field | Detail |
| --- | --- |
| Access | `ctx.run.Nodes()` |
| Signature | `create_nodes!(ctx.run, requests)` |
| Params | `ctx, NodeCollection, optional parent` |
| Returns | `resource/runtime ID or `Result` as shown by backing method` |
| Use when | Use when gameplay needs a new runtime/resource object built from typed data. |
| Fails when / edge behavior | Returns the documented empty value when backing runtime data is missing, stale, or the target type does not match. |

### `get_node_name`

| Field | Detail |
| --- | --- |
| Access | `ctx.run.Nodes()` |
| Signature | `get_node_name!(ctx.run, id)` |
| Params | `ctx, id` |
| Returns | `typed value from backing method` |
| Use when | Use when gameplay needs to read typed engine data and react without owning the storage. |
| Fails when / edge behavior | Returns the documented empty value when backing runtime data is missing, stale, or the target type does not match. |

### `set_node_name`

| Field | Detail |
| --- | --- |
| Access | `ctx.run.Nodes()` |
| Signature | `set_node_name!(ctx.run, id, name)` |
| Params | `ctx, id, name` |
| Returns | `bool or () as shown by backing method` |
| Use when | Use when gameplay must change engine state or queue an action this frame. |
| Fails when / edge behavior | Returns the documented empty value when backing runtime data is missing, stale, or the target type does not match. |

### `get_skeleton_bone_name`

| Field | Detail |
| --- | --- |
| Access | `ctx.run.Nodes()` |
| Signature | `get_skeleton_bone_name!(ctx.run, id, index)` |
| Params | `ctx, id, index` |
| Returns | `typed value from backing method` |
| Use when | Use when gameplay needs to read typed engine data and react without owning the storage. |
| Fails when / edge behavior | Returns the documented empty value when backing runtime data is missing, stale, or the target type does not match. |

### `get_skeleton_bone_index`

| Field | Detail |
| --- | --- |
| Access | `ctx.run.Nodes()` |
| Signature | `get_skeleton_bone_index!(ctx.run, id, name)` |
| Params | `ctx, id, name` |
| Returns | `typed value from backing method` |
| Use when | Use when gameplay needs to read typed engine data and react without owning the storage. |
| Fails when / edge behavior | Returns the documented empty value when backing runtime data is missing, stale, or the target type does not match. |

### `set_ui_rotation`

| Field | Detail |
| --- | --- |
| Access | `ctx.run.Nodes()` |
| Signature | `set_ui_rotation!(ctx.run, id, rotation)` |
| Params | `ctx, id, rotation` |
| Returns | `bool or () as shown by backing method` |
| Use when | Use when gameplay must change engine state or queue an action this frame. |
| Fails when / edge behavior | Returns the documented empty value when backing runtime data is missing, stale, or the target type does not match. |

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
| Use when | Use when gameplay must change engine state or queue an action this frame. |
| Fails when / edge behavior | Returns the documented empty value when backing runtime data is missing, stale, or the target type does not match. |

### `get_node_parent_id`

| Field | Detail |
| --- | --- |
| Access | `ctx.run.Nodes()` |
| Signature | `get_node_parent_id!(ctx.run, id)` |
| Params | `ctx, id` |
| Returns | `typed value from backing method` |
| Use when | Use when gameplay needs to read typed engine data and react without owning the storage. |
| Fails when / edge behavior | Returns the documented empty value when backing runtime data is missing, stale, or the target type does not match. |

### `get_node_children_ids`

| Field | Detail |
| --- | --- |
| Access | `ctx.run.Nodes()` |
| Signature | `get_node_children_ids!(ctx.run, id)` |
| Params | `ctx, id` |
| Returns | `typed value from backing method` |
| Use when | Use when gameplay needs to read typed engine data and react without owning the storage. |
| Fails when / edge behavior | Returns the documented empty value when backing runtime data is missing, stale, or the target type does not match. |

### `get_children`

| Field | Detail |
| --- | --- |
| Access | `ctx.run.Nodes()` |
| Signature | `get_children!(ctx.run, id)` |
| Params | `ctx, id` |
| Returns | `typed value from backing method` |
| Use when | Use when gameplay needs to read typed engine data and react without owning the storage. |
| Fails when / edge behavior | Returns the documented empty value when backing runtime data is missing, stale, or the target type does not match. |

### `get_child`

| Field | Detail |
| --- | --- |
| Access | `ctx.run.Nodes()` |
| Signature | `get_child!(ctx.run, id, all[name] $(,)?)` |
| Params | `ctx, id, all[name] $(,)?` |
| Returns | `typed value from backing method` |
| Use when | Use when gameplay needs to read typed engine data and react without owning the storage. |
| Fails when / edge behavior | Returns the documented empty value when backing runtime data is missing, stale, or the target type does not match. |

### `get_node_type`

| Field | Detail |
| --- | --- |
| Access | `ctx.run.Nodes()` |
| Signature | `get_node_type!(ctx.run, id)` |
| Params | `ctx, id` |
| Returns | `typed value from backing method` |
| Use when | Use when gameplay needs to read typed engine data and react without owning the storage. |
| Fails when / edge behavior | Returns the documented empty value when backing runtime data is missing, stale, or the target type does not match. |

### `reparent`

| Field | Detail |
| --- | --- |
| Access | `ctx.run.Nodes()` |
| Signature | `reparent!(ctx.run, parent, child)` |
| Params | `ctx, parent, child` |
| Returns | `same as backing method` |
| Use when | Use when script code needs this exact engine read or write. |
| Fails when / edge behavior | Returns the documented empty value when backing runtime data is missing, stale, or the target type does not match. |

### `force_rerender`

| Field | Detail |
| --- | --- |
| Access | `ctx.run.Nodes()` |
| Signature | `force_rerender!(ctx.run, id)` |
| Params | `ctx, id` |
| Returns | `same as backing method` |
| Use when | Use when gameplay must change engine state or queue an action this frame. |
| Fails when / edge behavior | Returns the documented empty value when backing runtime data is missing, stale, or the target type does not match. |

### `reparent_multi`

| Field | Detail |
| --- | --- |
| Access | `ctx.run.Nodes()` |
| Signature | `reparent_multi!(ctx.run, parent, child_ids)` |
| Params | `ctx, parent, child_ids` |
| Returns | `same as backing method` |
| Use when | Use when script code needs this exact engine read or write. |
| Fails when / edge behavior | Returns the documented empty value when backing runtime data is missing, stale, or the target type does not match. |

### `remove_node`

| Field | Detail |
| --- | --- |
| Access | `ctx.run.Nodes()` |
| Signature | `remove_node!(ctx.run, id)` |
| Params | `ctx, id` |
| Returns | `bool or () as shown by backing method` |
| Use when | Use when code must release, remove, stop, or disconnect existing engine state. |
| Fails when / edge behavior | Returns the documented empty value when backing runtime data is missing, stale, or the target type does not match. |

### `get_global_transform_2d`

| Field | Detail |
| --- | --- |
| Access | `ctx.run.Nodes()` |
| Signature | `get_global_transform_2d!(ctx.run, id)` |
| Params | `ctx, id` |
| Returns | `typed value from backing method` |
| Use when | Use when gameplay needs to read typed engine data and react without owning the storage. |
| Fails when / edge behavior | Returns the documented empty value when backing runtime data is missing, stale, or the target type does not match. |

### `get_global_transform_3d`

| Field | Detail |
| --- | --- |
| Access | `ctx.run.Nodes()` |
| Signature | `get_global_transform_3d!(ctx.run, id)` |
| Params | `ctx, id` |
| Returns | `typed value from backing method` |
| Use when | Use when gameplay needs to read typed engine data and react without owning the storage. |
| Fails when / edge behavior | Returns the documented empty value when backing runtime data is missing, stale, or the target type does not match. |

### `get_local_transform_2d`

| Field | Detail |
| --- | --- |
| Access | `ctx.run.Nodes()` |
| Signature | `get_local_transform_2d!(ctx.run, id)` |
| Params | `ctx, id` |
| Returns | `typed value from backing method` |
| Use when | Use when gameplay needs to read typed engine data and react without owning the storage. |
| Fails when / edge behavior | Returns the documented empty value when backing runtime data is missing, stale, or the target type does not match. |

### `get_local_transform_3d`

| Field | Detail |
| --- | --- |
| Access | `ctx.run.Nodes()` |
| Signature | `get_local_transform_3d!(ctx.run, id)` |
| Params | `ctx, id` |
| Returns | `typed value from backing method` |
| Use when | Use when gameplay needs to read typed engine data and react without owning the storage. |
| Fails when / edge behavior | Returns the documented empty value when backing runtime data is missing, stale, or the target type does not match. |

### `set_global_transform_2d`

| Field | Detail |
| --- | --- |
| Access | `ctx.run.Nodes()` |
| Signature | `set_global_transform_2d!(ctx.run, id, transform)` |
| Params | `ctx, id, transform` |
| Returns | `bool or () as shown by backing method` |
| Use when | Use when gameplay must change engine state or queue an action this frame. |
| Fails when / edge behavior | Returns the documented empty value when backing runtime data is missing, stale, or the target type does not match. |

### `set_global_transform_3d`

| Field | Detail |
| --- | --- |
| Access | `ctx.run.Nodes()` |
| Signature | `set_global_transform_3d!(ctx.run, id, transform)` |
| Params | `ctx, id, transform` |
| Returns | `bool or () as shown by backing method` |
| Use when | Use when gameplay must change engine state or queue an action this frame. |
| Fails when / edge behavior | Returns the documented empty value when backing runtime data is missing, stale, or the target type does not match. |

### `set_local_transform_2d`

| Field | Detail |
| --- | --- |
| Access | `ctx.run.Nodes()` |
| Signature | `set_local_transform_2d!(ctx.run, id, transform)` |
| Params | `ctx, id, transform` |
| Returns | `bool or () as shown by backing method` |
| Use when | Use when gameplay must change engine state or queue an action this frame. |
| Fails when / edge behavior | Returns the documented empty value when backing runtime data is missing, stale, or the target type does not match. |

### `set_local_transform_3d`

| Field | Detail |
| --- | --- |
| Access | `ctx.run.Nodes()` |
| Signature | `set_local_transform_3d!(ctx.run, id, transform)` |
| Params | `ctx, id, transform` |
| Returns | `bool or () as shown by backing method` |
| Use when | Use when gameplay must change engine state or queue an action this frame. |
| Fails when / edge behavior | Returns the documented empty value when backing runtime data is missing, stale, or the target type does not match. |

### `get_local_pos_2d`

| Field | Detail |
| --- | --- |
| Access | `ctx.run.Nodes()` |
| Signature | `get_local_pos_2d!(ctx.run, id)` |
| Params | `ctx, id` |
| Returns | `typed value from backing method` |
| Use when | Use when gameplay needs to read typed engine data and react without owning the storage. |
| Fails when / edge behavior | Returns the documented empty value when backing runtime data is missing, stale, or the target type does not match. |

### `get_local_pos_3d`

| Field | Detail |
| --- | --- |
| Access | `ctx.run.Nodes()` |
| Signature | `get_local_pos_3d!(ctx.run, id)` |
| Params | `ctx, id` |
| Returns | `typed value from backing method` |
| Use when | Use when gameplay needs to read typed engine data and react without owning the storage. |
| Fails when / edge behavior | Returns the documented empty value when backing runtime data is missing, stale, or the target type does not match. |

### `set_local_pos_2d`

| Field | Detail |
| --- | --- |
| Access | `ctx.run.Nodes()` |
| Signature | `set_local_pos_2d!(ctx.run, id, pos)` |
| Params | `ctx, id, pos` |
| Returns | `bool or () as shown by backing method` |
| Use when | Use when gameplay must change engine state or queue an action this frame. |
| Fails when / edge behavior | Returns the documented empty value when backing runtime data is missing, stale, or the target type does not match. |

### `set_local_pos_3d`

| Field | Detail |
| --- | --- |
| Access | `ctx.run.Nodes()` |
| Signature | `set_local_pos_3d!(ctx.run, id, pos)` |
| Params | `ctx, id, pos` |
| Returns | `bool or () as shown by backing method` |
| Use when | Use when gameplay must change engine state or queue an action this frame. |
| Fails when / edge behavior | Returns the documented empty value when backing runtime data is missing, stale, or the target type does not match. |

### `get_global_pos_2d`

| Field | Detail |
| --- | --- |
| Access | `ctx.run.Nodes()` |
| Signature | `get_global_pos_2d!(ctx.run, id)` |
| Params | `ctx, id` |
| Returns | `typed value from backing method` |
| Use when | Use when gameplay needs to read typed engine data and react without owning the storage. |
| Fails when / edge behavior | Returns the documented empty value when backing runtime data is missing, stale, or the target type does not match. |

### `get_global_pos_3d`

| Field | Detail |
| --- | --- |
| Access | `ctx.run.Nodes()` |
| Signature | `get_global_pos_3d!(ctx.run, id)` |
| Params | `ctx, id` |
| Returns | `typed value from backing method` |
| Use when | Use when gameplay needs to read typed engine data and react without owning the storage. |
| Fails when / edge behavior | Returns the documented empty value when backing runtime data is missing, stale, or the target type does not match. |

### `set_global_pos_2d`

| Field | Detail |
| --- | --- |
| Access | `ctx.run.Nodes()` |
| Signature | `set_global_pos_2d!(ctx.run, id, pos)` |
| Params | `ctx, id, pos` |
| Returns | `bool or () as shown by backing method` |
| Use when | Use when gameplay must change engine state or queue an action this frame. |
| Fails when / edge behavior | Returns the documented empty value when backing runtime data is missing, stale, or the target type does not match. |

### `set_global_pos_3d`

| Field | Detail |
| --- | --- |
| Access | `ctx.run.Nodes()` |
| Signature | `set_global_pos_3d!(ctx.run, id, pos)` |
| Params | `ctx, id, pos` |
| Returns | `bool or () as shown by backing method` |
| Use when | Use when gameplay must change engine state or queue an action this frame. |
| Fails when / edge behavior | Returns the documented empty value when backing runtime data is missing, stale, or the target type does not match. |

### `get_local_rot_2d`

| Field | Detail |
| --- | --- |
| Access | `ctx.run.Nodes()` |
| Signature | `get_local_rot_2d!(ctx.run, id)` |
| Params | `ctx, id` |
| Returns | `typed value from backing method` |
| Use when | Use when gameplay needs to read typed engine data and react without owning the storage. |
| Fails when / edge behavior | Returns the documented empty value when backing runtime data is missing, stale, or the target type does not match. |

### `get_local_rot_3d`

| Field | Detail |
| --- | --- |
| Access | `ctx.run.Nodes()` |
| Signature | `get_local_rot_3d!(ctx.run, id)` |
| Params | `ctx, id` |
| Returns | `typed value from backing method` |
| Use when | Use when gameplay needs to read typed engine data and react without owning the storage. |
| Fails when / edge behavior | Returns the documented empty value when backing runtime data is missing, stale, or the target type does not match. |

### `set_local_rot_2d`

| Field | Detail |
| --- | --- |
| Access | `ctx.run.Nodes()` |
| Signature | `set_local_rot_2d!(ctx.run, id, rot)` |
| Params | `ctx, id, rot` |
| Returns | `bool or () as shown by backing method` |
| Use when | Use when gameplay must change engine state or queue an action this frame. |
| Fails when / edge behavior | Returns the documented empty value when backing runtime data is missing, stale, or the target type does not match. |

### `set_local_rot_3d`

| Field | Detail |
| --- | --- |
| Access | `ctx.run.Nodes()` |
| Signature | `set_local_rot_3d!(ctx.run, id, rot)` |
| Params | `ctx, id, rot` |
| Returns | `bool or () as shown by backing method` |
| Use when | Use when gameplay must change engine state or queue an action this frame. |
| Fails when / edge behavior | Returns the documented empty value when backing runtime data is missing, stale, or the target type does not match. |

### `get_global_rot_2d`

| Field | Detail |
| --- | --- |
| Access | `ctx.run.Nodes()` |
| Signature | `get_global_rot_2d!(ctx.run, id)` |
| Params | `ctx, id` |
| Returns | `typed value from backing method` |
| Use when | Use when gameplay needs to read typed engine data and react without owning the storage. |
| Fails when / edge behavior | Returns the documented empty value when backing runtime data is missing, stale, or the target type does not match. |

### `get_global_rot_3d`

| Field | Detail |
| --- | --- |
| Access | `ctx.run.Nodes()` |
| Signature | `get_global_rot_3d!(ctx.run, id)` |
| Params | `ctx, id` |
| Returns | `typed value from backing method` |
| Use when | Use when gameplay needs to read typed engine data and react without owning the storage. |
| Fails when / edge behavior | Returns the documented empty value when backing runtime data is missing, stale, or the target type does not match. |

### `set_global_rot_2d`

| Field | Detail |
| --- | --- |
| Access | `ctx.run.Nodes()` |
| Signature | `set_global_rot_2d!(ctx.run, id, rot)` |
| Params | `ctx, id, rot` |
| Returns | `bool or () as shown by backing method` |
| Use when | Use when gameplay must change engine state or queue an action this frame. |
| Fails when / edge behavior | Returns the documented empty value when backing runtime data is missing, stale, or the target type does not match. |

### `set_global_rot_3d`

| Field | Detail |
| --- | --- |
| Access | `ctx.run.Nodes()` |
| Signature | `set_global_rot_3d!(ctx.run, id, rot)` |
| Params | `ctx, id, rot` |
| Returns | `bool or () as shown by backing method` |
| Use when | Use when gameplay must change engine state or queue an action this frame. |
| Fails when / edge behavior | Returns the documented empty value when backing runtime data is missing, stale, or the target type does not match. |

### `get_local_scale_2d`

| Field | Detail |
| --- | --- |
| Access | `ctx.run.Nodes()` |
| Signature | `get_local_scale_2d!(ctx.run, id)` |
| Params | `ctx, id` |
| Returns | `typed value from backing method` |
| Use when | Use when gameplay needs to read typed engine data and react without owning the storage. |
| Fails when / edge behavior | Returns the documented empty value when backing runtime data is missing, stale, or the target type does not match. |

### `get_local_scale_3d`

| Field | Detail |
| --- | --- |
| Access | `ctx.run.Nodes()` |
| Signature | `get_local_scale_3d!(ctx.run, id)` |
| Params | `ctx, id` |
| Returns | `typed value from backing method` |
| Use when | Use when gameplay needs to read typed engine data and react without owning the storage. |
| Fails when / edge behavior | Returns the documented empty value when backing runtime data is missing, stale, or the target type does not match. |

### `set_local_scale_2d`

| Field | Detail |
| --- | --- |
| Access | `ctx.run.Nodes()` |
| Signature | `set_local_scale_2d!(ctx.run, id, scale)` |
| Params | `ctx, id, scale` |
| Returns | `bool or () as shown by backing method` |
| Use when | Use when gameplay must change engine state or queue an action this frame. |
| Fails when / edge behavior | Returns the documented empty value when backing runtime data is missing, stale, or the target type does not match. |

### `set_local_scale_3d`

| Field | Detail |
| --- | --- |
| Access | `ctx.run.Nodes()` |
| Signature | `set_local_scale_3d!(ctx.run, id, scale)` |
| Params | `ctx, id, scale` |
| Returns | `bool or () as shown by backing method` |
| Use when | Use when gameplay must change engine state or queue an action this frame. |
| Fails when / edge behavior | Returns the documented empty value when backing runtime data is missing, stale, or the target type does not match. |

### `get_global_scale_2d`

| Field | Detail |
| --- | --- |
| Access | `ctx.run.Nodes()` |
| Signature | `get_global_scale_2d!(ctx.run, id)` |
| Params | `ctx, id` |
| Returns | `typed value from backing method` |
| Use when | Use when gameplay needs to read typed engine data and react without owning the storage. |
| Fails when / edge behavior | Returns the documented empty value when backing runtime data is missing, stale, or the target type does not match. |

### `get_global_scale_3d`

| Field | Detail |
| --- | --- |
| Access | `ctx.run.Nodes()` |
| Signature | `get_global_scale_3d!(ctx.run, id)` |
| Params | `ctx, id` |
| Returns | `typed value from backing method` |
| Use when | Use when gameplay needs to read typed engine data and react without owning the storage. |
| Fails when / edge behavior | Returns the documented empty value when backing runtime data is missing, stale, or the target type does not match. |

### `set_global_scale_2d`

| Field | Detail |
| --- | --- |
| Access | `ctx.run.Nodes()` |
| Signature | `set_global_scale_2d!(ctx.run, id, scale)` |
| Params | `ctx, id, scale` |
| Returns | `bool or () as shown by backing method` |
| Use when | Use when gameplay must change engine state or queue an action this frame. |
| Fails when / edge behavior | Returns the documented empty value when backing runtime data is missing, stale, or the target type does not match. |

### `set_global_scale_3d`

| Field | Detail |
| --- | --- |
| Access | `ctx.run.Nodes()` |
| Signature | `set_global_scale_3d!(ctx.run, id, scale)` |
| Params | `ctx, id, scale` |
| Returns | `bool or () as shown by backing method` |
| Use when | Use when gameplay must change engine state or queue an action this frame. |
| Fails when / edge behavior | Returns the documented empty value when backing runtime data is missing, stale, or the target type does not match. |

### `to_global_point_2d`

| Field | Detail |
| --- | --- |
| Access | `ctx.run.Nodes()` |
| Signature | `to_global_point_2d!(ctx.run, id, point)` |
| Params | `ctx, id, point` |
| Returns | `same as backing method` |
| Use when | Use when converting points or transforms between local node space and world space. |
| Fails when / edge behavior | Returns the documented empty value when backing runtime data is missing, stale, or the target type does not match. |

### `to_local_point_2d`

| Field | Detail |
| --- | --- |
| Access | `ctx.run.Nodes()` |
| Signature | `to_local_point_2d!(ctx.run, id, point)` |
| Params | `ctx, id, point` |
| Returns | `same as backing method` |
| Use when | Use when converting points or transforms between local node space and world space. |
| Fails when / edge behavior | Returns the documented empty value when backing runtime data is missing, stale, or the target type does not match. |

### `to_global_point_3d`

| Field | Detail |
| --- | --- |
| Access | `ctx.run.Nodes()` |
| Signature | `to_global_point_3d!(ctx.run, id, point)` |
| Params | `ctx, id, point` |
| Returns | `same as backing method` |
| Use when | Use when converting points or transforms between local node space and world space. |
| Fails when / edge behavior | Returns the documented empty value when backing runtime data is missing, stale, or the target type does not match. |

### `to_local_point_3d`

| Field | Detail |
| --- | --- |
| Access | `ctx.run.Nodes()` |
| Signature | `to_local_point_3d!(ctx.run, id, point)` |
| Params | `ctx, id, point` |
| Returns | `same as backing method` |
| Use when | Use when converting points or transforms between local node space and world space. |
| Fails when / edge behavior | Returns the documented empty value when backing runtime data is missing, stale, or the target type does not match. |

### `to_global_transform_2d`

| Field | Detail |
| --- | --- |
| Access | `ctx.run.Nodes()` |
| Signature | `to_global_transform_2d!(ctx.run, id, transform)` |
| Params | `ctx, id, transform` |
| Returns | `same as backing method` |
| Use when | Use when converting points or transforms between local node space and world space. |
| Fails when / edge behavior | Returns the documented empty value when backing runtime data is missing, stale, or the target type does not match. |

### `to_local_transform_2d`

| Field | Detail |
| --- | --- |
| Access | `ctx.run.Nodes()` |
| Signature | `to_local_transform_2d!(ctx.run, id, transform)` |
| Params | `ctx, id, transform` |
| Returns | `same as backing method` |
| Use when | Use when converting points or transforms between local node space and world space. |
| Fails when / edge behavior | Returns the documented empty value when backing runtime data is missing, stale, or the target type does not match. |

### `to_global_transform_3d`

| Field | Detail |
| --- | --- |
| Access | `ctx.run.Nodes()` |
| Signature | `to_global_transform_3d!(ctx.run, id, transform)` |
| Params | `ctx, id, transform` |
| Returns | `same as backing method` |
| Use when | Use when converting points or transforms between local node space and world space. |
| Fails when / edge behavior | Returns the documented empty value when backing runtime data is missing, stale, or the target type does not match. |

### `to_local_transform_3d`

| Field | Detail |
| --- | --- |
| Access | `ctx.run.Nodes()` |
| Signature | `to_local_transform_3d!(ctx.run, id, transform)` |
| Params | `ctx, id, transform` |
| Returns | `same as backing method` |
| Use when | Use when converting points or transforms between local node space and world space. |
| Fails when / edge behavior | Returns the documented empty value when backing runtime data is missing, stale, or the target type does not match. |

### `get_node_tags`

| Field | Detail |
| --- | --- |
| Access | `ctx.run.Nodes()` |
| Signature | `get_node_tags!(ctx.run, id)` |
| Params | `ctx, id` |
| Returns | `typed value from backing method` |
| Use when | Use when gameplay needs to read typed engine data and react without owning the storage. |
| Fails when / edge behavior | Returns the documented empty value when backing runtime data is missing, stale, or the target type does not match. |

### `tag_set`

| Field | Detail |
| --- | --- |
| Access | `ctx.run.Nodes()` |
| Signature | `tag_set!(ctx.run, id, tags)` |
| Params | `ctx, id, tags` |
| Returns | `same as backing method` |
| Use when | Use when script code needs this exact engine read or write. |
| Fails when / edge behavior | Returns the documented empty value when backing runtime data is missing, stale, or the target type does not match. |

### `tag_add`

| Field | Detail |
| --- | --- |
| Access | `ctx.run.Nodes()` |
| Signature | `tag_add!(ctx.run, id, tags)` |
| Params | `ctx, id, tags` |
| Returns | `bool or () as shown by backing method` |
| Use when | Use when script code needs this exact engine read or write. |
| Fails when / edge behavior | Returns the documented empty value when backing runtime data is missing, stale, or the target type does not match. |

### `tag_remove`

| Field | Detail |
| --- | --- |
| Access | `ctx.run.Nodes()` |
| Signature | `tag_remove!(ctx.run, id, tag)` |
| Params | `ctx, id, tag` |
| Returns | `bool or () as shown by backing method` |
| Use when | Use when code must release, remove, stop, or disconnect existing engine state. |
| Fails when / edge behavior | Returns the documented empty value when backing runtime data is missing, stale, or the target type does not match. |

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

