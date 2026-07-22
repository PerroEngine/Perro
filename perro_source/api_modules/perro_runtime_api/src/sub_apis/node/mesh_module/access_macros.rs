/// Node access macros.
///
/// These macros expose typed node access via closure-scoped borrows.
///
/// Finds a node by name inside `root`'s subtree (index-backed, includes `root`).
/// Pass `NodeID::nil()` as `root` to search the whole scene.
///
/// Usage: `find_node!(ctx, root, "DemoCamera") -> Option<NodeID>`.
///
/// Arguments:
/// - `ctx`: `&mut RuntimeWindow<_>`
/// - `root`: subtree root `NodeID` (or `NodeID::nil()` for whole scene)
/// - `name`: `&str`, `String`, or `Cow<str>`
#[macro_export]
macro_rules! find_node {
    ($ctx:expr, $root:expr, $name:expr) => {
        $ctx.Nodes().find_node_by_name($root, $name)
    };
}

/// Collects `root` plus every descendant (depth-first, `root` included).
/// Empty when `root` is nil.
///
/// Usage: `for id in descendants!(ctx, root) { ... }` returning `Vec<NodeID>`.
#[macro_export]
macro_rules! descendants {
    ($ctx:expr, $root:expr) => {
        $ctx.Nodes().subtree_node_ids($root)
    };
}

/// Sets `visible` on every `UiNode` in `root`'s subtree (including `root`).
/// The walk runs in one runtime borrow. Returns count of UI nodes updated.
///
/// Usage: `set_tree_visible!(ctx, menu_root, show_menu) -> usize`.
#[macro_export]
macro_rules! set_tree_visible {
    ($ctx:expr, $root:expr, $visible:expr) => {
        $ctx.Nodes().set_subtree_visible($root, $visible)
    };
}

/// Sets one script var on every node in `root`'s subtree (including `root`).
/// The `value` is cloned per node. Returns the number of nodes visited.
///
/// Usage: `broadcast_var!(ctx, root, var!("mouse_sensitivity"), variant!(s)) -> usize`.
///
/// Arguments:
/// - `ctx`: `&mut RuntimeWindow<_>`
/// - `root`: subtree root `NodeID`
/// - `member`: `var!("...")`, `ScriptMemberID`, `&str`, `String`, or `Cow<str>`
/// - `value`: `Variant`
#[macro_export]
macro_rules! broadcast_var {
    ($ctx:expr, $root:expr, $member:expr, $value:expr) => {{
        let __member = $crate::sub_apis::IntoScriptMemberID::into_script_member($member);
        let __value = $value;
        let mut __count = 0usize;
        for __id in $ctx.Nodes().subtree_node_ids($root) {
            $ctx.Scripts()
                .set_var(__id, __member, ::core::clone::Clone::clone(&__value));
            __count += 1;
        }
        __count
    }};
}

/// Creates a node and configures it in one expression, returning its `NodeID`.
/// Combines [`create_node!`] with [`with_node_mut!`]: the trailing closure
/// receives `&mut ConcreteType`.
///
/// Usage:
/// - `spawn!(ctx, Sprite2D, |s| { ... }) -> NodeID`
/// - `spawn!(ctx, Sprite2D, name, |s| { ... }) -> NodeID`
/// - `spawn!(ctx, Sprite2D, name, tags, |s| { ... }) -> NodeID`
/// - `spawn!(ctx, Sprite2D, name, tags, parent, |s| { ... }) -> NodeID`
#[macro_export]
macro_rules! spawn {
    ($ctx:expr, $node_ty:ty, $f:expr) => {{
        let __id = $crate::create_node!($ctx, $node_ty);
        let _ = $crate::with_node_mut!($ctx, $node_ty, __id, $f);
        __id
    }};
    ($ctx:expr, $node_ty:ty, $name:expr, $f:expr) => {{
        let __id = $crate::create_node!($ctx, $node_ty, $name);
        let _ = $crate::with_node_mut!($ctx, $node_ty, __id, $f);
        __id
    }};
    ($ctx:expr, $node_ty:ty, $name:expr, $tags:expr, $f:expr) => {{
        let __id = $crate::create_node!($ctx, $node_ty, $name, $tags);
        let _ = $crate::with_node_mut!($ctx, $node_ty, __id, $f);
        __id
    }};
    ($ctx:expr, $node_ty:ty, $name:expr, $tags:expr, $parent:expr, $f:expr) => {{
        let __id = $crate::create_node!($ctx, $node_ty, $name, $tags, $parent);
        let _ = $crate::with_node_mut!($ctx, $node_ty, __id, $f);
        __id
    }};
}

/// Rotates a 3D spatial node to face a world-space point.
///
/// Usage:
/// - `look_at_3d!(ctx, turret, target_pos) -> bool` (world `+Y` up)
/// - `look_at_3d!(ctx, turret, target_pos, up) -> bool`
///
/// Returns `false` if the node has no 3D global transform.
#[macro_export]
macro_rules! look_at_3d {
    ($ctx:expr, $node:expr, $target:expr) => {
        $crate::look_at_3d!(
            $ctx,
            $node,
            $target,
            $crate::perro_structs::Vector3::new(0.0, 1.0, 0.0)
        )
    };
    ($ctx:expr, $node:expr, $target:expr, $up:expr) => {{
        let __node = $node;
        let __target = $target;
        match $ctx.Nodes().get_global_transform_3d(__node) {
            Some(__t) => {
                let __rot =
                    $crate::perro_structs::Quaternion::looking_at(__target - __t.position, $up);
                $ctx.Nodes().set_global_rot_3d(__node, __rot)
            }
            None => false,
        }
    }};
}

/// Exact-type mutable node access.
/// Usage: `with_node_mut!(ctx, ConcreteType, node_id, |node| { ... }) -> Option<V>`.
/// Internals:
/// - The runtime looks up `node_id`, verifies exact type equality with `ConcreteType`,
///   then invokes your closure while holding a short-lived mutable borrow.
/// - The borrow cannot escape the closure, which keeps access compile-time safe.
///
/// Arguments:
/// - `ctx`: `&mut RuntimeWindow<_>`
/// - `ConcreteType`: concrete node struct type (exact match only)
/// - `node_id`: `NodeID`
/// - closure arg: `&mut ConcreteType`
#[macro_export]
macro_rules! with_node_mut {
    ($ctx:expr, $node_ty:ty, $id:expr, $f:expr) => {
        $ctx.Nodes().with_node_mut::<$node_ty, _, _>($id, $f)
    };
    ($($invalid:tt)*) => {
        compile_error!(
            "invalid with_node_mut! call; use: with_node_mut!(ctx, ConcreteType, node_id, |node| { ... })"
        )
    };
}

/// Exact-type read node access.
/// Usage: `with_node!(ctx, ConcreteType, node_id, |node| -> V { ... }) -> V`.
/// Internals:
/// - The runtime does an exact concrete-type check, then calls the closure with `&ConcreteType`.
/// - The read borrow is scoped to the closure call and cannot outlive it.
///
/// Arguments:
/// - `ctx`: `&mut RuntimeWindow<_>`
/// - `ConcreteType`: concrete node struct type (exact match only)
/// - `node_id`: `NodeID`
/// - closure arg: `&ConcreteType`
#[macro_export]
macro_rules! with_node {
    ($ctx:expr, $node_ty:ty, $id:expr, $f:expr) => {
        $ctx.Nodes().with_node::<$node_ty, _>($id, $f)
    };
}

/// Base/inheritance-aware read node access.
/// Usage: `with_base_node!(ctx, BaseType, node_id, |base| { ... }) -> Option<V>`.
/// Internals:
/// - The runtime checks `node.node_type().is_a(BaseType)`, then dispatches the closure as `&BaseType`.
/// - This keeps one runtime check while still giving typed field/method access in the closure body.
///
/// Arguments:
/// - `ctx`: `&mut RuntimeWindow<_>`
/// - `BaseType`: base node struct type (descendants allowed)
/// - `node_id`: `NodeID`
/// - closure arg: `&BaseType`
#[macro_export]
macro_rules! with_base_node {
    ($ctx:expr, $base_ty:ty, $id:expr, $f:expr) => {
        $ctx.Nodes().with_base_node::<$base_ty, _, _>($id, $f)
    };
}

/// Base/inheritance-aware mutable node access.
/// Usage: `with_base_node_mut!(ctx, BaseType, node_id, |base| { ... }) -> Option<V>`.
/// Internals:
/// - Same `is_a` runtime check as `with_base_node!`, then executes your closure with `&mut BaseType`.
/// - Mutable borrow is closure-scoped so references cannot escape.
///
/// Arguments:
/// - `ctx`: `&mut RuntimeWindow<_>`
/// - `BaseType`: base node struct type (descendants allowed)
/// - `node_id`: `NodeID`
/// - closure arg: `&mut BaseType`
#[macro_export]
macro_rules! with_base_node_mut {
    ($ctx:expr, $base_ty:ty, $id:expr, $f:expr) => {
        $ctx.Nodes().with_base_node_mut::<$base_ty, _, _>($id, $f)
    };
}

/// Creates a node from default concrete type.
/// Usage:
/// - `create_node!(ctx, ConcreteType) -> NodeID`
/// - `create_node!(ctx, ConcreteType, name) -> NodeID`
/// - `create_node!(ctx, ConcreteType, name, tags) -> NodeID`
/// - `create_node!(ctx, ConcreteType, name, tags, parent_id) -> NodeID`
/// - `create_nodes!(ctx, requests, parent_id) -> Vec<NodeID>`
///
/// Arguments:
/// - `ctx`: `&mut RuntimeWindow<_>`
/// - `ConcreteType`: ie Node2D, MeshInstance3D, Sprite2D
/// - `name` (optional): `&str`, `String`, or `Cow<'static, str>`
/// - `tags` (optional): usually from `tags![...]`, or string/id tag collections
/// - `parent_id` (optional): `NodeID`
#[macro_export]
macro_rules! create_node {
    ($ctx:expr, $node_ty:ty) => {
        $ctx.Nodes().create::<$node_ty>()
    };
    ($ctx:expr, $node_ty:ty, $name:expr) => {{
        let __id = $ctx.Nodes().create::<$node_ty>();
        let _ = $ctx.Nodes().set_node_name(__id, $name);
        __id
    }};
    ($ctx:expr, $node_ty:ty, $name:expr, $tags:expr) => {{
        let __id = $ctx.Nodes().create::<$node_ty>();
        let _ = $ctx.Nodes().set_node_name(__id, $name);
        let _ = $ctx.Nodes().set_tags(__id, Some($tags));
        __id
    }};
    ($ctx:expr, $node_ty:ty, $name:expr, $tags:expr, $parent:expr) => {{
        let __id = $ctx.Nodes().create::<$node_ty>();
        let _ = $ctx.Nodes().set_node_name(__id, $name);
        let _ = $ctx.Nodes().set_tags(__id, Some($tags));
        let _ = $ctx.Nodes().reparent($parent, __id);
        __id
    }};
}

/// Creates many nodes from a [`NodeCollection`](crate::sub_apis::NodeCollection).
/// Usage:
/// - `create_nodes!(ctx, requests) -> Vec<NodeID>`
/// - `create_nodes!(ctx, requests, parent_id) -> Vec<NodeID>`
#[macro_export]
macro_rules! create_nodes {
    ($ctx:expr, $requests:expr) => {
        $ctx.Nodes()
            .create_nodes(&$requests, $crate::perro_ids::NodeID::nil())
    };
    ($ctx:expr, $requests:expr, $parent:expr) => {
        $ctx.Nodes().create_nodes(&$requests, $parent)
    };
}

/// Builds a flat node collection from recursive node specs.
///
/// Fields:
/// - `node = value` required
/// - `name = value` optional
/// - `tags = value` optional
/// - `children = [ ... ]` optional
#[macro_export]
macro_rules! node_collection {
    (@push $collection:ident, $parent:expr, {
        collection = $child_collection:expr $(,)?
    }) => {{
        $collection.extend($child_collection, $parent)
    }};

    (@struct_expr $ty:ident { $($fields:tt)* }) => {
        $crate::node_collection!(@struct_fields $ty [] $($fields)*, ;)
    };
    (@struct_fields $ty:ident [$($out:tt)*] ;) => {
        $ty {
            $($out)*
            ..::std::default::Default::default()
        }
    };
    (@struct_fields $ty:ident [$($out:tt)*] , $($rest:tt)*) => {
        $crate::node_collection!(@struct_fields $ty [$($out)*] $($rest)*)
    };
    (@struct_fields $ty:ident [$($out:tt)*] $field:ident : { $value:expr }, $($rest:tt)*) => {
        $crate::node_collection!(@struct_fields $ty [$($out)* $field: $value,] $($rest)*)
    };
    (@struct_fields $ty:ident [$($out:tt)*] $field:ident : $field_ty:ident { $($fields:tt)* }, $($rest:tt)*) => {
        $crate::node_collection!(@struct_fields $ty [
            $($out)* $field: $crate::node_collection!(@struct_expr $field_ty { $($fields)* }),
        ] $($rest)*)
    };
    (@struct_fields $ty:ident [$($out:tt)*] $field:ident : $value:expr, $($rest:tt)*) => {
        $crate::node_collection!(@struct_fields $ty [$($out)* $field: $value,] $($rest)*)
    };

    (@node_expr { $node:expr }) => {
        $node
    };
    (@node_expr $ty:ident { $($fields:tt)* }) => {
        $crate::node_collection!(@node_struct $ty [] $($fields)*)
    };
    (@node_expr $ty:ident) => {
        <$ty as ::std::default::Default>::default()
    };
    (@node_expr $node:expr) => {
        $node
    };
    (@node_struct $ty:ident [$($out:tt)*] .. $base:expr) => {
        $ty { $($out)* .. $base }
    };
    (@node_struct $ty:ident [$($out:tt)*] .. $base:expr,) => {
        $ty { $($out)* .. $base }
    };
    (@node_struct $ty:ident [$($out:tt)*] $next:tt $($rest:tt)*) => {
        $crate::node_collection!(@node_struct $ty [$($out)* $next] $($rest)*)
    };
    (@node_struct $ty:ident [$($out:tt)*]) => {
        $crate::node_collection!(@struct_expr $ty { $($out)* })
    };

    (@patch_lets $($fields:tt)*) => {
        $crate::node_collection!(@patch_lets_inner [] $($fields)*, ;)
    };
    (@patch_lets_inner [$($out:tt)*] ;) => {
        $($out)*
    };
    (@patch_lets_inner [$($out:tt)*] , $($rest:tt)*) => {
        $crate::node_collection!(@patch_lets_inner [$($out)*] $($rest)*)
    };
    (@patch_lets_inner [$($out:tt)*] $field:ident : { $value:expr }, $($rest:tt)*) => {
        $crate::node_collection!(@patch_lets_inner [$($out)* let $field = $value;] $($rest)*)
    };
    (@patch_lets_inner [$($out:tt)*] $field:ident : $ty:ident { $($fields:tt)* }, $($rest:tt)*) => {
        $crate::node_collection!(@patch_lets_inner [
            $($out)* let $field = $crate::node_collection!(@struct_expr $ty { $($fields)* });
        ] $($rest)*)
    };
    (@patch_lets_inner [$($out:tt)*] $field:ident : $value:expr, $($rest:tt)*) => {
        $crate::node_collection!(@patch_lets_inner [$($out)* let $field = $value;] $($rest)*)
    };
    (@patch_assigns $node:ident, $($fields:tt)*) => {
        $crate::node_collection!(@patch_assigns_inner $node [] $($fields)*, ;)
    };
    (@patch_assigns_inner $node:ident [$($out:tt)*] ;) => {
        $($out)*
    };
    (@patch_assigns_inner $node:ident [$($out:tt)*] , $($rest:tt)*) => {
        $crate::node_collection!(@patch_assigns_inner $node [$($out)*] $($rest)*)
    };
    (@patch_assigns_inner $node:ident [$($out:tt)*] $field:ident : { $value:expr }, $($rest:tt)*) => {
        $crate::node_collection!(@patch_assigns_inner $node [$($out)* $node.$field = $field.clone();] $($rest)*)
    };
    (@patch_assigns_inner $node:ident [$($out:tt)*] $field:ident : $ty:ident { $($fields:tt)* }, $($rest:tt)*) => {
        $crate::node_collection!(@patch_assigns_inner $node [$($out)* $node.$field = $field.clone();] $($rest)*)
    };
    (@patch_assigns_inner $node:ident [$($out:tt)*] $field:ident : $value:expr, $($rest:tt)*) => {
        $crate::node_collection!(@patch_assigns_inner $node [$($out)* $node.$field = $field.clone();] $($rest)*)
    };
    (@root_patch $ty:ident { $($fields:tt)* }) => {{
        $crate::node_collection!(@patch_lets $($fields)*);
        $crate::sub_apis::NodeRootPatch::new::<$ty, _>(move |__node| {
            $crate::node_collection!(@patch_assigns __node, $($fields)*);
        })
    }};
    (@patch_list [$($out:expr,)*]) => {
        vec![$($out,)*]
    };
    (@patch_list [$($out:expr,)*] , $($rest:tt)*) => {
        $crate::node_collection!(@patch_list [$($out,)*] $($rest)*)
    };
    (@patch_list [$($out:expr,)*] $ty:ident $fields:tt, $($rest:tt)*) => {
        $crate::node_collection!(
            @patch_list [
                $($out,)*
                $crate::node_collection!(@root_patch $ty $fields),
            ] $($rest)*
        )
    };
    (@patch_list [$($out:expr,)*] $ty:ident $fields:tt) => {
        $crate::node_collection!(
            @patch_list [
                $($out,)*
                $crate::node_collection!(@root_patch $ty $fields),
            ]
        )
    };

    (@script_spec { path = $path_macro:ident ! ( $path_lit:literal ), vars = { $($vars:tt)* } $(,)? }) => {
        $crate::sub_apis::NodeScriptSpec::new($path_macro!($path_lit))
            .raw_vars($crate::node_collection!(@script_vars [] $($vars)*, ;))
    };
    (@script_spec { path = { $path:expr }, vars = { $($vars:tt)* } $(,)? }) => {
        $crate::sub_apis::NodeScriptSpec::new($path)
            .raw_vars($crate::node_collection!(@script_vars [] $($vars)*, ;))
    };
    (@script_spec { path = $path:expr, vars = { $($vars:tt)* } $(,)? }) => {
        $crate::sub_apis::NodeScriptSpec::new($path)
            .raw_vars($crate::node_collection!(@script_vars [] $($vars)*, ;))
    };
    (@script_spec { path = $path:expr $(,)? }) => {
        $crate::sub_apis::NodeScriptSpec::new($path)
    };
    (@script_spec $path:expr) => {
        $crate::sub_apis::NodeScriptSpec::new($path)
    };
    (@script_vars [$($out:tt)*] ;) => {
        vec![$($out)*]
    };
    (@script_vars [$($out:tt)*] , $($rest:tt)*) => {
        $crate::node_collection!(@script_vars [$($out)*] $($rest)*)
    };
    (@script_vars [$($out:tt)*] $key:ident : @ $target:ident, $($rest:tt)*) => {
        $crate::node_collection!(@script_vars [
            $($out)*
            (
                $crate::perro_ids::ScriptMemberID::from_string(::std::stringify!($key)),
                $crate::sub_apis::NodeScriptVar::NodeRef($target),
            ),
        ] $($rest)*)
    };
    (@script_vars [$($out:tt)*] $key:ident : { $value:expr }, $($rest:tt)*) => {
        $crate::node_collection!(@script_vars [
            $($out)*
            (
                $crate::perro_ids::ScriptMemberID::from_string(::std::stringify!($key)),
                $crate::sub_apis::NodeScriptVar::Value($crate::perro_variant::Variant::from($value)),
            ),
        ] $($rest)*)
    };
    (@script_vars [$($out:tt)*] $key:ident : $value:expr, $($rest:tt)*) => {
        $crate::node_collection!(@script_vars [
            $($out)*
            (
                $crate::perro_ids::ScriptMemberID::from_string(::std::stringify!($key)),
                $crate::sub_apis::NodeScriptVar::Value($crate::perro_variant::Variant::from($value)),
            ),
        ] $($rest)*)
    };
    (@script_vars [$($out:tt)*] $key:literal : @ $target:ident, $($rest:tt)*) => {
        $crate::node_collection!(@script_vars [
            $($out)*
            (
                $crate::perro_ids::ScriptMemberID::from_string($key),
                $crate::sub_apis::NodeScriptVar::NodeRef($target),
            ),
        ] $($rest)*)
    };
    (@script_vars [$($out:tt)*] $key:literal : { $value:expr }, $($rest:tt)*) => {
        $crate::node_collection!(@script_vars [
            $($out)*
            (
                $crate::perro_ids::ScriptMemberID::from_string($key),
                $crate::sub_apis::NodeScriptVar::Value($crate::perro_variant::Variant::from($value)),
            ),
        ] $($rest)*)
    };
    (@script_vars [$($out:tt)*] $key:literal : $value:expr, $($rest:tt)*) => {
        $crate::node_collection!(@script_vars [
            $($out)*
            (
                $crate::perro_ids::ScriptMemberID::from_string($key),
                $crate::sub_apis::NodeScriptVar::Value($crate::perro_variant::Variant::from($value)),
            ),
        ] $($rest)*)
    };

    (@parse $collection:ident, $parent:expr, [$($node:tt)+], [], [$($mods:tt)*], [$($children:tt)*], ;) => {{
        let __idx = $collection.push(
            $crate::sub_apis::NodeSpec::new($crate::node_collection!(@node_expr $($node)+))
                $($mods)*
                .parent($parent)
        );
        $crate::node_collection!(@child_items_parent $collection, Some(__idx), $($children)* ;);
        __idx
    }};
    (@parse $collection:ident, $parent:expr, [], [$($scene:tt)+], [$($mods:tt)*], [$($children:tt)*], ;) => {{
        let __idx = $collection.push_scene(
            $crate::sub_apis::NodeSceneSpec::new($($scene)+)
                $($mods)*
                .parent($parent)
        );
        $crate::node_collection!(@child_items_parent $collection, Some(__idx), $($children)* ;);
        __idx
    }};
    (@parse $collection:ident, $parent:expr, [$($node:tt)*], [$($scene:tt)*], [$($mods:tt)*], [$($children:tt)*], , $($rest:tt)*) => {
        $crate::node_collection!(@parse $collection, $parent, [$($node)*], [$($scene)*], [$($mods)*], [$($children)*], $($rest)*)
    };
    (@parse $collection:ident, $parent:expr, [$($node:tt)*], [$($scene:tt)*], [$($mods:tt)*], [$($children:tt)*], name = $name:expr, $($rest:tt)*) => {
        $crate::node_collection!(@parse $collection, $parent, [$($node)*], [$($scene)*], [$($mods)* .name($name)], [$($children)*], $($rest)*)
    };
    (@parse $collection:ident, $parent:expr, [$($node:tt)*], [$($scene:tt)*], [$($mods:tt)*], [$($children:tt)*], tags = $tags:expr, $($rest:tt)*) => {
        $crate::node_collection!(@parse $collection, $parent, [$($node)*], [$($scene)*], [$($mods)* .tags($tags)], [$($children)*], $($rest)*)
    };
    (@parse $collection:ident, $parent:expr, [$($node:tt)*], [$($scene:tt)*], [$($mods:tt)*], [$($children:tt)*], script = { $($script:tt)* }, $($rest:tt)*) => {
        $crate::node_collection!(@parse $collection, $parent, [$($node)*], [$($scene)*], [$($mods)* .script($crate::node_collection!(@script_spec { $($script)* }))], [$($children)*], $($rest)*)
    };
    (@parse $collection:ident, $parent:expr, [$($node:tt)*], [$($scene:tt)*], [$($mods:tt)*], [$($children:tt)*], script = $script:expr, $($rest:tt)*) => {
        $crate::node_collection!(@parse $collection, $parent, [$($node)*], [$($scene)*], [$($mods)* .script($script)], [$($children)*], $($rest)*)
    };
    (@parse $collection:ident, $parent:expr, [$($node:tt)*], [$($scene:tt)*], [$($mods:tt)*], [$($children:tt)*], children = [ $( $child:tt ),* $(,)? ], $($rest:tt)*) => {
        $crate::node_collection!(@parse $collection, $parent, [$($node)*], [$($scene)*], [$($mods)*], [$($child),*], $($rest)*)
    };
    (@parse $collection:ident, $parent:expr, [$($node:tt)*], [$($scene:tt)*], [$($mods:tt)*], [$($children:tt)*], children = [ $($child:tt)* ], $($rest:tt)*) => {
        $crate::node_collection!(@parse $collection, $parent, [$($node)*], [$($scene)*], [$($mods)*], [$($child)*], $($rest)*)
    };
    (@parse $collection:ident, $parent:expr, [$($node:tt)*], [$($scene:tt)*], [$($mods:tt)*], [$($children:tt)*], parent = @ $parent_key:ident, $($rest:tt)*) => {
        $crate::node_collection!(@parse $collection, Some($parent_key), [$($node)*], [$($scene)*], [$($mods)*], [$($children)*], $($rest)*)
    };
    (@parse $collection:ident, $parent:expr, [$($node:tt)*], [$($scene:tt)*], [$($mods:tt)*], [$($children:tt)*], node = $ty:ident, $($rest:tt)*) => {
        $crate::node_collection!(@parse $collection, $parent, [$ty], [$($scene)*], [$($mods)*], [$($children)*], $($rest)*)
    };
    (@parse $collection:ident, $parent:expr, [$($node:tt)*], [$($scene:tt)*], [$($mods:tt)*], [$($children:tt)*], node = $ty:ident $fields:tt, $($rest:tt)*) => {
        $crate::node_collection!(@parse $collection, $parent, [$ty $fields], [$($scene)*], [$($mods)*], [$($children)*], $($rest)*)
    };
    (@parse $collection:ident, $parent:expr, [$($node:tt)*], [$($scene:tt)*], [$($mods:tt)*], [$($children:tt)*], node = $node_value:expr, $($rest:tt)*) => {
        $crate::node_collection!(@parse $collection, $parent, [$node_value], [$($scene)*], [$($mods)*], [$($children)*], $($rest)*)
    };
    (@parse $collection:ident, $parent:expr, [$($node:tt)*], [$($scene:tt)*], [$($mods:tt)*], [$($children:tt)*], scene = { path = $path_macro:ident ! ( $($path_args:tt)* ), patch = $ty:ident $fields:tt $(,)? }, $($rest:tt)*) => {
        $crate::node_collection!(@parse $collection, $parent, [$($node)*], [$path_macro!($($path_args)*)], [$($mods)* .patch($crate::node_collection!(@root_patch $ty $fields))], [$($children)*], $($rest)*)
    };
    (@parse $collection:ident, $parent:expr, [$($node:tt)*], [$($scene:tt)*], [$($mods:tt)*], [$($children:tt)*], scene = { path = $path_macro:ident ! ( $($path_args:tt)* ), patch = [ $($patches:tt)* ] $(,)? }, $($rest:tt)*) => {
        $crate::node_collection!(@parse $collection, $parent, [$($node)*], [$path_macro!($($path_args)*)], [$($mods)* .patches($crate::node_collection!(@patch_list [] $($patches)*))], [$($children)*], $($rest)*)
    };
    (@parse $collection:ident, $parent:expr, [$($node:tt)*], [$($scene:tt)*], [$($mods:tt)*], [$($children:tt)*], scene = { path = { $path:expr }, patch = $ty:ident $fields:tt $(,)? }, $($rest:tt)*) => {
        $crate::node_collection!(@parse $collection, $parent, [$($node)*], [$path], [$($mods)* .patch($crate::node_collection!(@root_patch $ty $fields))], [$($children)*], $($rest)*)
    };
    (@parse $collection:ident, $parent:expr, [$($node:tt)*], [$($scene:tt)*], [$($mods:tt)*], [$($children:tt)*], scene = { path = { $path:expr }, patch = [ $($patches:tt)* ] $(,)? }, $($rest:tt)*) => {
        $crate::node_collection!(@parse $collection, $parent, [$($node)*], [$path], [$($mods)* .patches($crate::node_collection!(@patch_list [] $($patches)*))], [$($children)*], $($rest)*)
    };
    (@parse $collection:ident, $parent:expr, [$($node:tt)*], [$($scene:tt)*], [$($mods:tt)*], [$($children:tt)*], scene = { path = $path:expr, patch = $ty:ident $fields:tt $(,)? }, $($rest:tt)*) => {
        $crate::node_collection!(@parse $collection, $parent, [$($node)*], [$path], [$($mods)* .patch($crate::node_collection!(@root_patch $ty $fields))], [$($children)*], $($rest)*)
    };
    (@parse $collection:ident, $parent:expr, [$($node:tt)*], [$($scene:tt)*], [$($mods:tt)*], [$($children:tt)*], scene = { path = $path:expr, patch = [ $($patches:tt)* ] $(,)? }, $($rest:tt)*) => {
        $crate::node_collection!(@parse $collection, $parent, [$($node)*], [$path], [$($mods)* .patches($crate::node_collection!(@patch_list [] $($patches)*))], [$($children)*], $($rest)*)
    };
    (@parse $collection:ident, $parent:expr, [$($node:tt)*], [$($scene:tt)*], [$($mods:tt)*], [$($children:tt)*], scene = { path = $path:expr }, $($rest:tt)*) => {
        $crate::node_collection!(@parse $collection, $parent, [$($node)*], [$path], [$($mods)*], [$($children)*], $($rest)*)
    };
    (@parse $collection:ident, $parent:expr, [$($node:tt)*], [$($scene:tt)*], [$($mods:tt)*], [$($children:tt)*], scene = $scene_macro:ident ! ( $($scene_args:tt)* ), $($rest:tt)*) => {
        $crate::node_collection!(@parse $collection, $parent, [$($node)*], [$scene_macro!($($scene_args)*)], [$($mods)*], [$($children)*], $($rest)*)
    };
    (@parse $collection:ident, $parent:expr, [$($node:tt)*], [$($scene:tt)*], [$($mods:tt)*], [$($children:tt)*], scene = { $scene_value:expr }, $($rest:tt)*) => {
        $crate::node_collection!(@parse $collection, $parent, [$($node)*], [$scene_value], [$($mods)*], [$($children)*], $($rest)*)
    };
    (@parse $collection:ident, $parent:expr, [$($node:tt)*], [$($scene:tt)*], [$($mods:tt)*], [$($children:tt)*], scene = $scene_value:literal, $($rest:tt)*) => {
        $crate::node_collection!(@parse $collection, $parent, [$($node)*], [$scene_value], [$($mods)*], [$($children)*], $($rest)*)
    };
    (@parse $collection:ident, $parent:expr, [$($node:tt)*], [$($scene:tt)*], [$($mods:tt)*], [$($children:tt)*], scene = $scene_value:ident, $($rest:tt)*) => {
        $crate::node_collection!(@parse $collection, $parent, [$($node)*], [$scene_value], [$($mods)*], [$($children)*], $($rest)*)
    };
    (@push $collection:ident, $parent:expr, { $($body:tt)* }) => {{
        $crate::node_collection!(@parse $collection, $parent, [], [], [], [], $($body)*, ;)
    }};
    (@push_key $collection:ident, $parent:expr, $key:ident, {
        collection = $child_collection:expr $(,)?
    }) => {{
        $collection.extend($child_collection, $parent)
    }};
    (@push_key $collection:ident, $parent:expr, $key:ident, { $($body:tt)* }) => {{
        $crate::node_collection!(
            @parse $collection, $parent, [], [], [.name(::std::stringify!($key))], [], $($body)*, ;
        )
    }};

    (@items_parent $collection:ident, [$($parent:tt)*], ;) => {};
    (@items_parent $collection:ident, [$($parent:tt)*], root = @ $root_key:ident, $($rest:tt)*) => {
        $collection.set_root($root_key);
        $crate::node_collection!(@items_parent $collection, [$($parent)*], $($rest)*)
    };
    (@items_parent $collection:ident, [$($parent:tt)*], root = @ $root_key:ident ;) => {
        $collection.set_root($root_key);
    };
    (@items_parent $collection:ident, [$($parent:tt)*], $key:ident : $entry:tt, $($rest:tt)*) => {
        #[allow(unused_variables)]
        let $key = $crate::node_collection!(@push_key $collection, $($parent)*, $key, $entry);
        $crate::node_collection!(@items_parent $collection, [$($parent)*], $($rest)*)
    };
    (@items_parent $collection:ident, [$($parent:tt)*], $key:ident : $entry:tt ;) => {
        #[allow(unused_variables)]
        let $key = $crate::node_collection!(@push_key $collection, $($parent)*, $key, $entry);
    };
    (@items_parent $collection:ident, [$($parent:tt)*], $entry:tt, $($rest:tt)*) => {
        $crate::node_collection!(@push $collection, $($parent)*, $entry);
        $crate::node_collection!(@items_parent $collection, [$($parent)*], $($rest)*)
    };
    (@items_parent $collection:ident, [$($parent:tt)*], $entry:tt ;) => {
        $crate::node_collection!(@push $collection, $($parent)*, $entry);
    };

    (@child_items_parent $collection:ident, $parent:expr, ;) => {};
    (@child_items_parent $collection:ident, $parent:expr, $key:ident : $entry:tt, $($rest:tt)*) => {
        $crate::node_collection!(@push_key $collection, $parent, $key, $entry);
        $crate::node_collection!(@child_items_parent $collection, $parent, $($rest)*)
    };
    (@child_items_parent $collection:ident, $parent:expr, $key:ident : $entry:tt ;) => {
        $crate::node_collection!(@push_key $collection, $parent, $key, $entry);
    };
    (@child_items_parent $collection:ident, $parent:expr, { parent = @ $parent_key:ident, $($body:tt)* }, $($rest:tt)*) => {
        ::std::compile_error!("node_collection children do not support parent = @key; child parent is implicit");
    };
    (@child_items_parent $collection:ident, $parent:expr, { parent = @ $parent_key:ident, $($body:tt)* } ;) => {
        ::std::compile_error!("node_collection children do not support parent = @key; child parent is implicit");
    };
    (@child_items_parent $collection:ident, $parent:expr, $entry:tt, $($rest:tt)*) => {
        $crate::node_collection!(@push $collection, $parent, $entry);
        $crate::node_collection!(@child_items_parent $collection, $parent, $($rest)*)
    };
    (@child_items_parent $collection:ident, $parent:expr, $entry:tt ;) => {
        $crate::node_collection!(@push $collection, $parent, $entry);
    };

    (@push $collection:ident, $parent:expr, {
        name = $name:expr,
        tags = $tags:expr,
        scene = $scene:expr,
        children = [ $( $child:tt ),* $(,)? ] $(,)?
    }) => {{
        let __idx = $collection.push_scene(
            $crate::sub_apis::NodeSceneSpec::new($scene)
                .name($name)
                .tags($tags)
                .parent($parent)
        );
        $(
            $crate::node_collection!(@push $collection, Some(__idx), $child);
        )*
        __idx
    }};
    (@push $collection:ident, $parent:expr, {
        name = $name:expr,
        tags = $tags:expr,
        scene = $scene:expr $(,)?
    }) => {{
        $collection.push_scene(
            $crate::sub_apis::NodeSceneSpec::new($scene)
                .name($name)
                .tags($tags)
                .parent($parent)
        )
    }};
    (@push $collection:ident, $parent:expr, {
        name = $name:expr,
        scene = $scene:expr,
        children = [ $( $child:tt ),* $(,)? ] $(,)?
    }) => {{
        let __idx = $collection.push_scene(
            $crate::sub_apis::NodeSceneSpec::new($scene)
                .name($name)
                .parent($parent)
        );
        $(
            $crate::node_collection!(@push $collection, Some(__idx), $child);
        )*
        __idx
    }};
    (@push $collection:ident, $parent:expr, {
        name = $name:expr,
        scene = $scene:expr $(,)?
    }) => {{
        $collection.push_scene(
            $crate::sub_apis::NodeSceneSpec::new($scene)
                .name($name)
                .parent($parent)
        )
    }};
    (@push $collection:ident, $parent:expr, {
        tags = $tags:expr,
        scene = $scene:expr,
        children = [ $( $child:tt ),* $(,)? ] $(,)?
    }) => {{
        let __idx = $collection.push_scene(
            $crate::sub_apis::NodeSceneSpec::new($scene)
                .tags($tags)
                .parent($parent)
        );
        $(
            $crate::node_collection!(@push $collection, Some(__idx), $child);
        )*
        __idx
    }};
    (@push $collection:ident, $parent:expr, {
        tags = $tags:expr,
        scene = $scene:expr $(,)?
    }) => {{
        $collection.push_scene(
            $crate::sub_apis::NodeSceneSpec::new($scene)
                .tags($tags)
                .parent($parent)
        )
    }};
    (@push $collection:ident, $parent:expr, {
        scene = $scene:expr,
        children = [ $( $child:tt ),* $(,)? ] $(,)?
    }) => {{
        let __idx = $collection.push_scene(
            $crate::sub_apis::NodeSceneSpec::new($scene).parent($parent)
        );
        $(
            $crate::node_collection!(@push $collection, Some(__idx), $child);
        )*
        __idx
    }};
    (@push $collection:ident, $parent:expr, {
        scene = $scene:expr $(,)?
    }) => {{
        $collection.push_scene($crate::sub_apis::NodeSceneSpec::new($scene).parent($parent))
    }};

    (@push $collection:ident, $parent:expr, {
        name = $name:expr,
        tags = $tags:expr,
        node = $node:expr,
        children = [ $( $child:tt ),* $(,)? ] $(,)?
    }) => {{
        let __idx = $collection.push(
            $crate::sub_apis::NodeSpec::new($node)
                .name($name)
                .tags($tags)
                .parent($parent)
        );
        $(
            $crate::node_collection!(@push $collection, Some(__idx), $child);
        )*
        __idx
    }};
    (@push $collection:ident, $parent:expr, {
        name = $name:expr,
        tags = $tags:expr,
        node = $node:expr $(,)?
    }) => {{
        $collection.push(
            $crate::sub_apis::NodeSpec::new($node)
                .name($name)
                .tags($tags)
                .parent($parent)
        )
    }};
    (@push $collection:ident, $parent:expr, {
        name = $name:expr,
        node = $node:expr,
        children = [ $( $child:tt ),* $(,)? ] $(,)?
    }) => {{
        let __idx = $collection.push(
            $crate::sub_apis::NodeSpec::new($node)
                .name($name)
                .parent($parent)
        );
        $(
            $crate::node_collection!(@push $collection, Some(__idx), $child);
        )*
        __idx
    }};
    (@push $collection:ident, $parent:expr, {
        name = $name:expr,
        node = $node:expr $(,)?
    }) => {{
        $collection.push(
            $crate::sub_apis::NodeSpec::new($node)
                .name($name)
                .parent($parent)
        )
    }};
    (@push $collection:ident, $parent:expr, {
        tags = $tags:expr,
        node = $node:expr,
        children = [ $( $child:tt ),* $(,)? ] $(,)?
    }) => {{
        let __idx = $collection.push(
            $crate::sub_apis::NodeSpec::new($node)
                .tags($tags)
                .parent($parent)
        );
        $(
            $crate::node_collection!(@push $collection, Some(__idx), $child);
        )*
        __idx
    }};
    (@push $collection:ident, $parent:expr, {
        tags = $tags:expr,
        node = $node:expr $(,)?
    }) => {{
        $collection.push(
            $crate::sub_apis::NodeSpec::new($node)
                .tags($tags)
                .parent($parent)
        )
    }};
    (@push $collection:ident, $parent:expr, {
        node = $node:expr,
        children = [ $( $child:tt ),* $(,)? ] $(,)?
    }) => {{
        let __idx = $collection.push(
            $crate::sub_apis::NodeSpec::new($node).parent($parent)
        );
        $(
            $crate::node_collection!(@push $collection, Some(__idx), $child);
        )*
        __idx
    }};
    (@push $collection:ident, $parent:expr, {
        node = $node:expr $(,)?
    }) => {{
        $collection.push($crate::sub_apis::NodeSpec::new($node).parent($parent))
    }};

    (@push $collection:ident, $parent:expr, $child_collection:expr) => {{
        $collection.extend($child_collection, $parent)
    }};

    ($key:ident : $entry:tt $(, $($rest:tt)*)?) => {{
        let mut __collection = $crate::sub_apis::NodeCollection::new();
        #[allow(unused_variables)]
        let $key = $crate::node_collection!(@push_key __collection, None, $key, $entry);
        $(
            $crate::node_collection!(@items_parent __collection, [None], $($rest)* ;);
        )?
        __collection
    }};

    ($entry:tt, $($rest:tt)*) => {{
        let mut __collection = $crate::sub_apis::NodeCollection::new();
        $crate::node_collection!(@push __collection, None, $entry);
        $crate::node_collection!(@items_parent __collection, [None], $($rest)* ;);
        __collection
    }};

    ($entry:tt) => {{
        let mut __collection = $crate::sub_apis::NodeCollection::new();
        $crate::node_collection!(@push __collection, None, $entry);
        __collection
    }};

    ({ $($body:tt)* }) => {{
        let mut __collection = $crate::sub_apis::NodeCollection::new();
        $crate::node_collection!(@push __collection, None, { $($body)* });
        __collection
    }};
}
