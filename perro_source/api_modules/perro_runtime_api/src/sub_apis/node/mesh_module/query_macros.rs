use super::*;
/// Gets node tags.
/// Usage: `get_node_tags!(ctx, node_id) -> Option<Vec<Cow<'static, str>>>`.
/// Arguments:
/// - `ctx`: `&mut RuntimeWindow<_>`
/// - `node_id`: `NodeID`
#[macro_export]
macro_rules! get_node_tags {
    ($ctx:expr, $id:expr) => {
        $ctx.Nodes().get_node_tags($id)
    };
}

/// Sets or clears node tags.
/// Usage:
/// - `set_tags!(ctx, node_id, tags)` where `tags` converts into node tag data.
/// - `set_tags!(ctx, node_id)` clears all tags.
///
/// Arguments:
/// - `ctx`: `&mut RuntimeWindow<_>`
/// - `node_id`: `NodeID`
/// - `tags`: usually from `tags![...]`, or string/id tag collections
#[macro_export]
macro_rules! set_tags {
    ($ctx:expr, $id:expr, $tags:expr) => {
        $ctx.Nodes().set_tags($id, Some($tags))
    };
    ($ctx:expr, $id:expr) => {
        $ctx.Nodes()
            .set_tags::<&'static [$crate::perro_ids::TagID]>($id, None)
    };
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct CameraRay3D {
    pub origin: Vector3,
    pub direction: Vector3,
    pub max_distance: f32,
}

/// Sets or clears node tags, matching the `tag_*` macro family.
///
/// `set_tags!` is the method-style spelling; `tag_set!` sits next to
/// `tag_add!` and `tag_remove!` for script call sites.
#[macro_export]
macro_rules! tag_set {
    ($ctx:expr, $id:expr, $tags:expr) => {
        $crate::set_tags!($ctx, $id, $tags)
    };
    ($ctx:expr, $id:expr) => {
        $crate::set_tags!($ctx, $id)
    };
}

/// Adds one or more tags to a node.
/// Usage:
/// - `tag_add!(ctx, node_id, "enemy")`
/// - `tag_add!(ctx, node_id, tags!["enemy", "alive"])`
/// - `tag_add!(ctx, node_id, ["enemy", "alive"])`
///
/// Arguments:
/// - `ctx`: `&mut RuntimeWindow<_>`
/// - `node_id`: `NodeID`
/// - tags: `TagID`, `&str`, `String`, slices/arrays/vectors of those
#[macro_export]
macro_rules! tag_add {
    ($ctx:expr, $id:expr, $tags:expr) => {
        $ctx.Nodes().add_node_tags($id, $tags)
    };
}

/// Removes tag(s) from node.
/// Usage:
/// - `tag_remove!(ctx, node_id, tag) -> bool`
/// - `tag_remove!(ctx, node_id)` clears all tags.
///
/// Arguments:
/// - `ctx`: `&mut RuntimeWindow<_>`
/// - `node_id`: `NodeID`
/// - `tag` (3-arg form): `TagID`, `&str`, or `String`
#[macro_export]
macro_rules! tag_remove {
    ($ctx:expr, $id:expr, $tag:expr) => {
        $ctx.Nodes().remove_node_tag($id, $tag)
    };
    ($ctx:expr, $id:expr) => {
        $ctx.Nodes()
            .set_tags::<&'static [$crate::perro_ids::TagID]>($id, None)
    };
}

/// Builds a query expression without executing it.
#[macro_export]
macro_rules! query_expr {
    ($kind:ident $args:tt $(,)?) => {
        $crate::query!(@expr $kind $args)
    };
}

/// Builds a reusable node query without executing it.
#[macro_export]
macro_rules! query_builder {
    ($kind:ident $args:tt, in_subtree($parent:expr) $(,)?) => {{
        let __expr = $crate::query_expr!($kind $args);
        $crate::sub_apis::NodeQuery::new()
            .where_expr(__expr)
            .in_subtree($parent)
    }};
    ($kind:ident $args:tt $(,)?) => {{
        let __expr = $crate::query_expr!($kind $args);
        $crate::sub_apis::NodeQuery::new().where_expr(__expr)
    }};
}

/// Executes a node query and returns `Vec<NodeID>`.
///
/// Preferred syntax:
/// - `query!(ctx, all(name[...], tags[...], ...))`
/// - `query!(ctx, any(...))`
/// - `query!(ctx, not(...))`
/// - Optional scope: `query!(ctx, all(...), in_subtree(parent_id))`
///
/// Predicate groups:
/// - `name[...]` OR-list of names
/// - `tags[...]` list of tags; interpretation comes from wrapper:
///   `all(tags[...])`, `any(tags[...])`, or `not(tags[...])`
/// - `node_type[...]`
/// - `base_type[...]`
/// - `layers[...]` render layer allow-list for 2D/3D nodes
/// - `mask[...]` render layer deny-list for 2D/3D nodes
/// - `within[origin, size]` global-space box filter; `origin` is box center,
///   `size` is full extent. `Vector2` pair matches 2D nodes, `Vector3` pair
///   matches 3D nodes.
///
/// Boolean combinators:
/// - `all(expr, expr, ...)`
/// - `any(expr, expr, ...)`
/// - `not(expr)`
#[macro_export]
///   R is the return type of the underlying API method call this macro expands to.
macro_rules! query {
    ($ctx:expr, tags[$($tag:tt)*], in_subtree($parent:expr) $(,)?) => {{
        let _ = &$ctx;
        let _ = &$parent;
        compile_error!("tags[...] must be wrapped by all(...), any(...), or not(...)");
    }};
    ($ctx:expr, tags[$($tag:tt)*] $(,)?) => {{
        let _ = &$ctx;
        compile_error!("tags[...] must be wrapped by all(...), any(...), or not(...)");
    }};
    ($ctx:expr, $kind:ident $args:tt, in_subtree($parent:expr) $(,)?) => {{
        let __expr = $crate::query!(@expr $kind $args);
        let __query = $crate::sub_apis::NodeQuery::new()
            .where_expr(__expr)
            .in_subtree($parent);
        $ctx.NodeQuery().query(&__query)
    }};
    ($ctx:expr, $kind:ident $args:tt $(,)?) => {{
        let __expr = $crate::query!(@expr $kind $args);
        let __query = $crate::sub_apis::NodeQuery::new().where_expr(__expr);
        $ctx.NodeQuery().query(&__query)
    }};
    ($ctx:expr, $query:expr, in_subtree($parent:expr) $(,)?) => {{
        let __query = $query;
        let __query_view = (&__query).as_view().in_subtree($parent);
        $ctx.NodeQuery().query_view(__query_view)
    }};
    ($ctx:expr, $query:expr $(,)?) => {{
        let __query = $query;
        let __query_view = (&__query).as_view();
        $ctx.NodeQuery().query_view(__query_view)
    }};

    (@expr all($($kind:ident $args:tt),* $(,)?)) => {
        $crate::sub_apis::QueryExpr::All(vec![$($crate::query!(@expr $kind $args)),*])
    };
    (@expr any($($kind:ident $args:tt),* $(,)?)) => {
        $crate::sub_apis::QueryExpr::Any(vec![$($crate::query!(@expr $kind $args)),*])
    };
    (@expr not($kind:ident $args:tt)) => {
        $crate::sub_apis::QueryExpr::Not(Box::new($crate::query!(@expr $kind $args)))
    };

    (@expr name[$($name:expr),* $(,)?]) => {
        $crate::sub_apis::QueryExpr::Name(vec![$(($name).to_string()),*])
    };

    (@expr tags[$($tag:literal),* $(,)?]) => {
        $crate::sub_apis::QueryExpr::Tags(vec![$(const { $crate::perro_ids::TagID::from_string($tag) }),*])
    };

    (@expr tags[$($tag:expr),* $(,)?]) => {
        $crate::sub_apis::QueryExpr::Tags(vec![$($crate::perro_ids::IntoTagID::into_tag_id($tag)),*])
    };

    (@expr node_type[$($ty:ident),* $(,)?]) => {
        $crate::sub_apis::QueryExpr::IsTypeMask(const {
            $crate::sub_apis::__query_type_mask(&[$($crate::perro_nodes::NodeType::$ty),*])
        })
    };
    (@expr node_type[$($ty:path),* $(,)?]) => {
        $crate::sub_apis::QueryExpr::IsTypeMask(const {
            $crate::sub_apis::__query_type_mask(&[$($ty),*])
        })
    };
    (@expr base_type[$($ty:ident),* $(,)?]) => {
        $crate::sub_apis::QueryExpr::BaseTypeMask(const {
            $crate::sub_apis::__query_base_type_mask(&[$($crate::perro_nodes::NodeType::$ty),*])
        })
    };
    (@expr base_type[$($ty:path),* $(,)?]) => {
        $crate::sub_apis::QueryExpr::BaseTypeMask(const {
            $crate::sub_apis::__query_base_type_mask(&[$($ty),*])
        })
    };

    (@expr layers[] ) => {
        $crate::sub_apis::QueryExpr::Layers($crate::perro_structs::BitMask::NONE)
    };
    (@expr layers[$($layer:literal),* $(,)?]) => {
        $crate::sub_apis::QueryExpr::Layers(const {
            $crate::perro_structs::BitMask::with([$($layer),*])
        })
    };
    (@expr layers[$($layer:expr),* $(,)?]) => {
        $crate::sub_apis::QueryExpr::Layers(
            $crate::perro_structs::BitMask::from_layers([$($layer),*])
        )
    };
    (@expr mask[] ) => {
        $crate::sub_apis::QueryExpr::Mask($crate::perro_structs::BitMask::NONE)
    };
    (@expr mask[$($layer:literal),* $(,)?]) => {
        $crate::sub_apis::QueryExpr::Mask(const {
            $crate::perro_structs::BitMask::with([$($layer),*])
        })
    };
    (@expr mask[$($layer:expr),* $(,)?]) => {
        $crate::sub_apis::QueryExpr::Mask(
            $crate::perro_structs::BitMask::from_layers([$($layer),*])
        )
    };

    (@expr within[$origin:expr, $size:expr $(,)?]) => {
        $crate::sub_apis::QueryExpr::Within(
            $crate::sub_apis::IntoQueryBounds::into_query_bounds($origin, $size)
        )
    };
}

/// Executes a node query and returns owned `NodeID`s as an iterator.
///
/// This has the same syntax as [`query!`](macro@crate::query). It still uses
/// the runtime's owned query result internally, then returns `Vec::into_iter()`.
#[macro_export]
macro_rules! query_iter {
    ($ctx:expr, $kind:ident $args:tt, in_subtree($parent:expr) $(,)?) => {{
        $crate::query!($ctx, $kind $args, in_subtree($parent)).into_iter()
    }};
    ($ctx:expr, $kind:ident $args:tt $(,)?) => {{
        $crate::query!($ctx, $kind $args).into_iter()
    }};
    ($ctx:expr, $query:expr, in_subtree($parent:expr) $(,)?) => {{
        let __query = $query;
        let __query_view = (&__query).as_view().in_subtree($parent);
        $ctx.NodeQuery().query_view_iter(__query_view)
    }};
    ($ctx:expr, $query:expr $(,)?) => {{
        let __query = $query;
        $ctx.NodeQuery().query_iter(&__query)
    }};
}

/// Executes a node query and runs a closure once for each matching `NodeID`.
///
/// This has the same query syntax as [`query!`](macro@crate::query).
#[macro_export]
macro_rules! query_each {
    ($ctx:expr, $kind:ident $args:tt, in_subtree($parent:expr), $f:expr $(,)?) => {{
        for __node_id in $crate::query_iter!($ctx, $kind $args, in_subtree($parent)) {
            $f(__node_id);
        }
    }};
    ($ctx:expr, $kind:ident $args:tt, $f:expr $(,)?) => {{
        for __node_id in $crate::query_iter!($ctx, $kind $args) {
            $f(__node_id);
        }
    }};
    ($ctx:expr, $query:expr, in_subtree($parent:expr), $f:expr $(,)?) => {{
        for __node_id in $crate::query_iter!($ctx, $query, in_subtree($parent)) {
            $f(__node_id);
        }
    }};
    ($ctx:expr, $query:expr, $f:expr $(,)?) => {{
        for __node_id in $crate::query_iter!($ctx, $query) {
            $f(__node_id);
        }
    }};
}

/// Executes a node query and maps each matching `NodeID` into a collected `Vec`.
///
/// This has the same query syntax as [`query!`](macro@crate::query).
#[macro_export]
macro_rules! query_map {
    ($ctx:expr, $kind:ident $args:tt, in_subtree($parent:expr), $f:expr $(,)?) => {{
        $crate::query_iter!($ctx, $kind $args, in_subtree($parent))
            .map($f)
            .collect::<Vec<_>>()
    }};
    ($ctx:expr, $kind:ident $args:tt, $f:expr $(,)?) => {{
        $crate::query_iter!($ctx, $kind $args)
            .map($f)
            .collect::<Vec<_>>()
    }};
    ($ctx:expr, $query:expr, in_subtree($parent:expr), $f:expr $(,)?) => {{
        $crate::query_iter!($ctx, $query, in_subtree($parent))
            .map($f)
            .collect::<Vec<_>>()
    }};
    ($ctx:expr, $query:expr, $f:expr $(,)?) => {{
        $crate::query_iter!($ctx, $query)
            .map($f)
            .collect::<Vec<_>>()
    }};
}

/// Executes a node query and returns the first result as owned `NodeID`.
///
/// Usage:
/// - `query_first!(ctx, all(name["Enemy1"])) -> Option<NodeID>`
/// - `query_first!(ctx, all(tags["enemy"]), in_subtree(parent_id)) -> Option<NodeID>`
#[macro_export]
///   R is the return type of the underlying API method call this macro expands to.
macro_rules! query_first {
    ($ctx:expr, $kind:ident $args:tt, in_subtree($parent:expr) $(,)?) => {{
        let __expr = $crate::query!(@expr $kind $args);
        let __query = $crate::sub_apis::NodeQuery::new()
            .where_expr(__expr)
            .in_subtree($parent);
        $ctx.NodeQuery().query_first(&__query)
    }};
    ($ctx:expr, $kind:ident $args:tt $(,)?) => {{
        let __expr = $crate::query!(@expr $kind $args);
        let __query = $crate::sub_apis::NodeQuery::new().where_expr(__expr);
        $ctx.NodeQuery().query_first(&__query)
    }};
    ($ctx:expr, $query:expr, in_subtree($parent:expr) $(,)?) => {{
        let __query = $query;
        let __query_view = (&__query).as_view().in_subtree($parent);
        $ctx.NodeQuery().query_view_first(__query_view)
    }};
    ($ctx:expr, $query:expr $(,)?) => {{
        let __query = $query;
        $ctx.NodeQuery().query_first(&__query)
    }};
}
