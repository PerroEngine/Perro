use super::*;
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
