/// SceneNode metadata macros.
///
/// These macros expose node identity/relationship/metadata access:
/// - name (`get_node_name!`, `set_node_name!`)
/// - hierarchy (`get_node_parent_id!`, `get_node_children_ids!`)
/// - runtime typing (`get_node_type!`)
/// - tags (`get_node_tags!`, `set_tags!`, `tag_set!`, `tag_add!`, `tag_remove!`)
/// - global transform helpers (`get_global_transform_*`, `set_global_transform_*`, `to_*`)
///
/// Gets node display name.
/// Usage: `get_node_name!(ctx, node_id) -> Option<Cow<'static, str>>`.
///
/// Arguments:
/// - `ctx`: `&mut RuntimeWindow<_>`
/// - `node_id`: `NodeID`
#[macro_export]
macro_rules! get_node_name {
    ($ctx:expr, $id:expr) => {
        $ctx.Nodes().get_node_name($id)
    };
}

/// Sets node display name.
/// Usage: `set_node_name!(ctx, node_id, name) -> bool`.
/// Arguments:
/// - `ctx`: `&mut RuntimeWindow<_>`
/// - `node_id`: `NodeID`
/// - `name`: `&str`, `String`, or `Cow<'static, str>`
#[macro_export]
macro_rules! set_node_name {
    ($ctx:expr, $id:expr, $name:expr) => {
        $ctx.Nodes().set_node_name($id, $name)
    };
}

/// Gets skeleton bone name by index.
/// Usage: `get_skeleton_bone_name!(ctx, skeleton_id, bone_index) -> Option<Cow<'static, str>>`.
#[macro_export]
macro_rules! get_skeleton_bone_name {
    ($ctx:expr, $id:expr, $index:expr) => {
        $ctx.Nodes().get_skeleton_bone_name($id, $index)
    };
}

/// Gets first skeleton bone index by name.
/// Usage: `get_skeleton_bone_index!(ctx, skeleton_id, bone_name) -> Option<usize>`.
#[macro_export]
macro_rules! get_skeleton_bone_index {
    ($ctx:expr, $id:expr, $name:expr) => {
        $ctx.Nodes().get_skeleton_bone_index($id, $name)
    };
}

#[macro_export]
macro_rules! set_ui_rotation {
    ($ctx:expr, $id:expr, $rotation:expr) => {
        $ctx.Nodes().set_ui_rotation($id, $rotation)
    };
}

#[macro_export]
macro_rules! bind_locale_text {
    ($ctx:expr, $id:expr, $key:expr) => {
        $ctx.Nodes().bind_locale_text($id, $key)
    };
}

#[macro_export]
macro_rules! bind_locale_placeholder {
    ($ctx:expr, $id:expr, $key:expr) => {
        $ctx.Nodes().bind_locale_placeholder($id, $key)
    };
}

/// Gets node parent id.
/// Usage: `get_node_parent_id!(ctx, node_id) -> Option<NodeID>`.
/// Arguments:
/// - `ctx`: `&mut RuntimeWindow<_>`
/// - `node_id`: `NodeID`
#[macro_export]
macro_rules! get_node_parent_id {
    ($ctx:expr, $id:expr) => {
        $ctx.Nodes().get_node_parent_id($id)
    };
}

/// Gets children ids for a node.
/// Usage: `get_node_children_ids!(ctx, node_id) -> Option<Vec<NodeID>>`.
/// Arguments:
/// - `ctx`: `&mut RuntimeWindow<_>`
/// - `node_id`: `NodeID`
#[macro_export]
macro_rules! get_node_children_ids {
    ($ctx:expr, $id:expr) => {
        $ctx.Nodes().get_node_children_ids($id)
    };
}

/// Gets direct children ids; invalid parent returns empty vec.
/// Usage: `get_children!(ctx, parent_id) -> Vec<NodeID>`.
#[macro_export]
macro_rules! get_children {
    ($ctx:expr, $id:expr) => {
        $ctx.Nodes().get_children($id)
    };
}

/// Gets one direct child by index or name, or many by name.
/// Usage:
/// - `get_child!(ctx, parent_id, 0usize) -> Option<NodeID>`
/// - `get_child!(ctx, parent_id, "Player") -> Option<NodeID>`
/// - `get_child!(ctx, parent_id, all["Enemy"]) -> Vec<NodeID>`
#[macro_export]
macro_rules! get_child {
    ($ctx:expr, $id:expr, all[$name:expr] $(,)?) => {
        $ctx.Nodes().get_children_by_name($id, $name)
    };
    ($ctx:expr, $id:expr, $selector:expr $(,)?) => {
        $ctx.Nodes().get_child($id, $selector)
    };
}

/// Gets concrete runtime node type.
/// Usage: `get_node_type!(ctx, node_id) -> Option<NodeType>`.
/// Arguments:
/// - `ctx`: `&mut RuntimeWindow<_>`
/// - `node_id`: `NodeID`
#[macro_export]
macro_rules! get_node_type {
    ($ctx:expr, $id:expr) => {
        $ctx.Nodes().get_node_type($id)
    };
}

/// Reparents a child under parent (`parent = nil` detaches).
/// Usage: `reparent!(ctx, parent_id, child_id) -> bool`.
/// Arguments:
/// - `ctx`: `&mut RuntimeWindow<_>`
/// - `parent_id`: `NodeID` (`NodeID::nil()` detaches child)
/// - `child_id`: `NodeID`
#[macro_export]
macro_rules! reparent {
    ($ctx:expr, $parent:expr, $child:expr) => {
        $ctx.Nodes().reparent($parent, $child)
    };
}

/// Marks node subtree dirty for render extraction this frame.
/// Usage: `force_rerender!(ctx, root_id) -> bool`.
#[macro_export]
macro_rules! force_rerender {
    ($ctx:expr, $id:expr) => {
        $ctx.Nodes().force_rerender($id)
    };
}

/// Checks whether a MeshInstance3D/MultiMeshInstance3D has a ready retained draw.
/// Usage: `is_mesh_instance_ready!(ctx, node_id) -> bool`.
#[macro_export]
macro_rules! is_mesh_instance_ready {
    ($ctx:expr, $id:expr) => {
        $ctx.Nodes().is_mesh_instance_ready($id)
    };
}

/// Batch reparent.
/// Usage: `reparent_multi!(ctx, parent_id, child_ids_iter) -> usize`.
/// Arguments:
/// - `ctx`: `&mut RuntimeWindow<_>`
/// - `parent_id`: `NodeID` (`NodeID::nil()` detaches)
/// - `child_ids_iter`: iterator of `NodeID`
#[macro_export]
macro_rules! reparent_multi {
    ($ctx:expr, $parent:expr, $child_ids:expr) => {
        $ctx.Nodes().reparent_multi($parent, $child_ids)
    };
}

/// Removes a node from the scene graph.
/// Usage: `remove_node!(ctx, node_id) -> bool`.
#[macro_export]
macro_rules! remove_node {
    ($ctx:expr, $id:expr) => {
        $ctx.Nodes().remove_node($id)
    };
}
