use super::*;
use perro_ids::TagID;
use perro_nodes::{Node3D, SceneNodeData};

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
