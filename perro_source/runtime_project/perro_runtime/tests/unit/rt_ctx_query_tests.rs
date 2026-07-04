use super::*;
use ahash::{AHashMap, AHashSet};
use perro_ids::TagID;
use perro_nodes::{Node2D, Node3D, SceneNodeData};
use perro_structs::BitMask;

fn node_with_name_tags(name: &str, tags: &[&str]) -> SceneNode {
    let mut node = SceneNode::new(SceneNodeData::Node3D(Node3D::new()));
    node.set_name(name.to_string());
    for tag in tags {
        node.add_tag(TagID::from_string(tag));
    }
    node
}

#[test]
fn optimize_all_sorts_cheapest_first() {
    let expr = QueryExpr::All(vec![
        QueryExpr::Tags(vec![
            TagID::from_string("enemy"),
            TagID::from_string("alive"),
        ]),
        QueryExpr::Name(vec!["boss".to_string()]),
        QueryExpr::IsType(vec![NodeType::MeshInstance3D]),
    ]);

    let optimized = optimize_expr(&expr);
    let QueryExpr::All(children) = optimized else {
        panic!("expected all expression");
    };
    assert!(matches!(children[0], QueryExpr::IsType(_)));
    assert!(matches!(children[1], QueryExpr::Name(_)));
    assert!(matches!(children[2], QueryExpr::Tags(_)));
}

#[test]
fn type_mask_predicates_eval_as_bit_tests() {
    let node = node_with_name_tags("enemy_1", &["enemy"]);
    let exact = QueryExpr::IsTypeMask(QueryTypeMask::NONE.with_type(NodeType::Node3D));
    let wrong = QueryExpr::IsTypeMask(QueryTypeMask::NONE.with_type(NodeType::Node2D));
    let base = QueryExpr::BaseTypeMask(QueryTypeMask::NONE.with_type(NodeType::Node3D));

    assert!(eval_expr(&exact, &node));
    assert!(!eval_expr(&wrong, &node));
    assert!(eval_expr(&base, &node));
}

#[test]
fn query_plan_strips_global_type_filters_from_eval_expr() {
    let expr = QueryExpr::All(vec![
        QueryExpr::IsType(vec![NodeType::MeshInstance3D]),
        QueryExpr::BaseType(vec![NodeType::Node3D]),
        QueryExpr::Name(vec!["enemy".to_string()]),
    ]);
    let plan = QueryPlan::from_query(&Some(expr));

    assert!(plan.exact_type_mask.contains_type(NodeType::MeshInstance3D));
    assert!(!plan.exact_type_mask.contains_type(NodeType::Node3D));
    assert!(plan.base_type_mask.contains_type(NodeType::MeshInstance3D));
    assert!(matches!(plan.optimized_expr, Some(QueryExpr::Name(_))));
    assert_eq!(plan.estimated_cost_per_node, 5);
}

#[test]
fn query_plan_keeps_mixed_any_type_filters_branch_local() {
    let expr = QueryExpr::Any(vec![
        QueryExpr::IsType(vec![NodeType::MeshInstance3D]),
        QueryExpr::Name(vec!["enemy".to_string()]),
    ]);
    let plan = QueryPlan::from_query(&Some(expr));

    assert!(matches!(plan.optimized_expr, Some(QueryExpr::Any(_))));
}

#[test]
fn not_type_predicates_prune_by_complement_mask() {
    let expr = QueryExpr::Not(Box::new(QueryExpr::IsTypeMask(
        QueryTypeMask::NONE.with_type(NodeType::Node3D),
    )));
    let mask = allowed_type_mask(Some(&expr), TypeFilterKind::Exact);

    assert!(!mask.contains_type(NodeType::Node3D));
    assert!(mask.contains_type(NodeType::Node2D));
}

#[test]
fn mixed_not_predicates_keep_type_mask_conservative() {
    let expr = QueryExpr::Not(Box::new(QueryExpr::All(vec![
        QueryExpr::IsTypeMask(QueryTypeMask::NONE.with_type(NodeType::Node3D)),
        QueryExpr::Tags(vec![TagID::from_string("enemy")]),
    ])));
    let mask = allowed_type_mask(Some(&expr), TypeFilterKind::Exact);

    assert_eq!(mask, all_types_mask());
}

#[test]
fn tags_are_context_sensitive_under_combinators() {
    let node = node_with_name_tags("enemy_1", &["enemy", "alive"]);

    let all_tags = QueryExpr::All(vec![QueryExpr::Tags(vec![
        TagID::from_string("enemy"),
        TagID::from_string("alive"),
    ])]);
    assert!(eval_expr(&all_tags, &node));

    let any_tags = QueryExpr::Any(vec![QueryExpr::Tags(vec![
        TagID::from_string("enemy"),
        TagID::from_string("boss"),
    ])]);
    assert!(eval_expr(&any_tags, &node));

    let not_tags = QueryExpr::Not(Box::new(QueryExpr::Tags(vec![
        TagID::from_string("dead"),
        TagID::from_string("hidden"),
    ])));
    assert!(eval_expr(&not_tags, &node));
}

#[test]
fn layer_predicates_use_spatial_render_layers() {
    let mut node = SceneNode::new(SceneNodeData::Node2D(Node2D::new()));
    node.with_base_mut::<Node2D, _>(|node| {
        node.render_layers = BitMask::with([1, 3]);
    });

    assert!(eval_expr(&QueryExpr::Layers(BitMask::with([1])), &node));
    assert!(eval_expr(&QueryExpr::Layers(BitMask::with([2, 3])), &node));
    assert!(!eval_expr(&QueryExpr::Layers(BitMask::with([2])), &node));
    assert!(!eval_expr(&QueryExpr::Mask(BitMask::with([1])), &node));
    assert!(eval_expr(&QueryExpr::Mask(BitMask::with([2])), &node));
}

#[test]
fn indexed_all_candidates_intersect_smallest_sets_first() {
    let a = TagID::from_string("a");
    let b = TagID::from_string("b");
    let c = TagID::from_string("c");
    let id1 = NodeID::new(1);
    let id2 = NodeID::new(2);
    let id3 = NodeID::new(3);
    let id4 = NodeID::new(4);

    let mut index: AHashMap<TagID, AHashSet<NodeID>> = AHashMap::default();
    index.insert(a, [id1, id2, id3, id4].into_iter().collect());
    index.insert(b, [id2, id4].into_iter().collect());
    index.insert(c, [id4].into_iter().collect());

    let expr = QueryExpr::All(vec![
        QueryExpr::Tags(vec![a]),
        QueryExpr::Tags(vec![b]),
        QueryExpr::Tags(vec![c]),
    ]);
    let candidates = candidate_ids_from_index(&Some(expr), Some(&index), 8).expect("candidate ids");

    assert!(candidates.exact);
    assert_eq!(candidates.ids, vec![id4]);
}

#[test]
fn indexed_all_with_nonindexed_predicate_uses_small_tag_seed() {
    let common = TagID::from_string("common");
    let rare = TagID::from_string("rare");
    let id1 = NodeID::new(1);
    let id2 = NodeID::new(2);
    let id3 = NodeID::new(3);

    let mut index: AHashMap<TagID, AHashSet<NodeID>> = AHashMap::default();
    index.insert(common, [id1, id2, id3].into_iter().collect());
    index.insert(rare, [id3].into_iter().collect());

    let expr = QueryExpr::All(vec![
        QueryExpr::Tags(vec![common]),
        QueryExpr::Name(vec!["target".to_string()]),
        QueryExpr::Tags(vec![rare]),
    ]);
    let candidates = candidate_ids_from_index(&Some(expr), Some(&index), 8).expect("candidate ids");

    assert!(!candidates.exact);
    assert_eq!(candidates.ids, vec![id3]);
}

#[test]
fn indexed_missing_required_tag_returns_exact_empty() {
    let present = TagID::from_string("present");
    let missing = TagID::from_string("missing");
    let id1 = NodeID::new(1);

    let mut index: AHashMap<TagID, AHashSet<NodeID>> = AHashMap::default();
    index.insert(present, [id1].into_iter().collect());

    let expr = QueryExpr::All(vec![QueryExpr::Tags(vec![present, missing])]);
    let candidates = candidate_ids_from_index(&Some(expr), Some(&index), 8).expect("candidate ids");

    assert!(candidates.exact);
    assert!(candidates.ids.is_empty());
}

#[test]
fn within_matches_spatial_index_positions() {
    let node = SceneNode::new(SceneNodeData::Node3D(Node3D::new()));
    let node_type = node.node_type();
    let spatial = QuerySpatialIndex {
        pos_2d: vec![None, None],
        pos_3d: vec![None, Some(Vector3::new(1.0, 2.0, 3.0))],
    };

    let inside = QueryExpr::Within(QueryBounds::Box3D {
        origin: Vector3::new(0.0, 0.0, 0.0),
        size: Vector3::new(10.0, 10.0, 10.0),
    });
    assert!(eval_expr_with_type(
        &inside,
        &node,
        node_type,
        1,
        Some(&spatial)
    ));

    let outside = QueryExpr::Within(QueryBounds::Box3D {
        origin: Vector3::new(100.0, 0.0, 0.0),
        size: Vector3::new(10.0, 10.0, 10.0),
    });
    assert!(!eval_expr_with_type(
        &outside,
        &node,
        node_type,
        1,
        Some(&spatial)
    ));

    // 2D bounds never match a 3D node.
    let box_2d = QueryExpr::Within(QueryBounds::Box2D {
        origin: Vector2::new(1.0, 2.0),
        size: Vector2::new(10.0, 10.0),
    });
    assert!(!eval_expr_with_type(
        &box_2d,
        &node,
        node_type,
        1,
        Some(&spatial)
    ));

    // No spatial index -> no match.
    assert!(!eval_expr_with_type(&inside, &node, node_type, 1, None));

    // Slot without a cached position -> no match.
    assert!(!eval_expr_with_type(
        &inside,
        &node,
        node_type,
        0,
        Some(&spatial)
    ));
}

#[test]
fn within_bounds_edge_is_inclusive() {
    let bounds = QueryBounds::Box2D {
        origin: Vector2::new(0.0, 0.0),
        size: Vector2::new(4.0, 4.0),
    };
    assert!(bounds.contains_2d(Vector2::new(2.0, -2.0)));
    assert!(!bounds.contains_2d(Vector2::new(2.1, 0.0)));
    assert!(!bounds.contains_3d(Vector3::new(0.0, 0.0, 0.0)));
}

#[test]
fn within_narrows_base_type_mask_by_dimension() {
    let expr_2d = QueryExpr::Within(QueryBounds::Box2D {
        origin: Vector2::new(0.0, 0.0),
        size: Vector2::new(1.0, 1.0),
    });
    let mask = allowed_type_mask(Some(&expr_2d), TypeFilterKind::Base);
    assert!(mask.contains_type(NodeType::Node2D));
    assert!(!mask.contains_type(NodeType::Node3D));

    let expr_3d = QueryExpr::Within(QueryBounds::Box3D {
        origin: Vector3::new(0.0, 0.0, 0.0),
        size: Vector3::new(1.0, 1.0, 1.0),
    });
    let mask = allowed_type_mask(Some(&expr_3d), TypeFilterKind::Base);
    assert!(mask.contains_type(NodeType::Node3D));
    assert!(!mask.contains_type(NodeType::Node2D));
    assert!(mask.contains_type(NodeType::MeshInstance3D));
}

#[test]
fn has_spatial_detects_nested_within() {
    let expr = QueryExpr::All(vec![
        QueryExpr::Name(vec!["enemy".to_string()]),
        QueryExpr::Not(Box::new(QueryExpr::Within(QueryBounds::Box3D {
            origin: Vector3::new(0.0, 0.0, 0.0),
            size: Vector3::new(1.0, 1.0, 1.0),
        }))),
    ]);
    assert!(expr.has_spatial());
    assert!(!QueryExpr::Name(vec!["enemy".to_string()]).has_spatial());
}
