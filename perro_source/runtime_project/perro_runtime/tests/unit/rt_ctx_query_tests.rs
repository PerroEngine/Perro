use super::*;
use perro_ids::TagID;
use perro_nodes::{MeshInstance3D, Node3D, SceneNodeData};
use std::hint::black_box;

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

#[test]
#[ignore]
fn bench_clause_costs() {
    let arena_size = 100_000;
    let mut arena = NodeArena::with_capacity(arena_size + 1);
    for i in 0..arena_size {
        let mut node = if i % 5 == 0 {
            SceneNode::new(SceneNodeData::MeshInstance3D(MeshInstance3D::new()))
        } else {
            SceneNode::new(SceneNodeData::Node3D(Node3D::new()))
        };
        if i % 4 == 0 {
            node.set_name("enemy".to_string());
        } else {
            node.set_name("npc".to_string());
        }
        if i % 3 == 0 {
            node.add_tag(TagID::from_string("enemy"));
        }
        if i % 7 == 0 {
            node.add_tag(TagID::from_string("alive"));
        }
        if i % 19 == 0 {
            node.add_tag(TagID::from_string("boss"));
        }
        let _ = arena.insert(node);
    }

    let q = TagQuery {
        expr: Some(QueryExpr::All(vec![
            QueryExpr::Tags(vec![
                TagID::from_string("enemy"),
                TagID::from_string("alive"),
                TagID::from_string("boss"),
            ]),
            QueryExpr::Name(vec!["enemy".to_string()]),
            QueryExpr::IsType(vec![NodeType::MeshInstance3D]),
        ])),
        scope: QueryScope::Root,
    };
    let start = std::time::Instant::now();
    let matches = black_box(query_node_ids(&arena, q, None));
    let elapsed = start.elapsed().as_millis();
    println!(
        "bench_clause_costs: {} matches in {} ms",
        matches.len(),
        elapsed
    );

    fn scan_raw(arena: &NodeArena, expr: &QueryExpr) -> usize {
        let mut matched = 0_usize;
        for idx in 1..arena.slot_count() {
            let Some((_, node)) = arena.slot_get(idx) else {
                continue;
            };
            if eval_expr(expr, node) {
                matched += 1;
            }
        }
        matched
    }

    let a = QueryExpr::All(vec![
        QueryExpr::IsType(vec![NodeType::MeshInstance3D]),
        QueryExpr::Name(vec!["enemy".to_string()]),
        QueryExpr::Tags(vec![
            TagID::from_string("enemy"),
            TagID::from_string("alive"),
            TagID::from_string("boss"),
        ]),
    ]);
    let b = QueryExpr::All(vec![
        QueryExpr::Tags(vec![
            TagID::from_string("enemy"),
            TagID::from_string("alive"),
            TagID::from_string("boss"),
        ]),
        QueryExpr::Name(vec!["enemy".to_string()]),
        QueryExpr::IsType(vec![NodeType::MeshInstance3D]),
    ]);
    let c = QueryExpr::All(vec![
        QueryExpr::Name(vec!["enemy".to_string()]),
        QueryExpr::IsType(vec![NodeType::MeshInstance3D]),
        QueryExpr::Tags(vec![
            TagID::from_string("enemy"),
            TagID::from_string("alive"),
            TagID::from_string("boss"),
        ]),
    ]);

    let t0 = std::time::Instant::now();
    let ma = black_box(scan_raw(&arena, &a));
    let da = t0.elapsed().as_millis();
    let t1 = std::time::Instant::now();
    let mb = black_box(scan_raw(&arena, &b));
    let db = t1.elapsed().as_millis();
    let t2 = std::time::Instant::now();
    let mc = black_box(scan_raw(&arena, &c));
    let dc = t2.elapsed().as_millis();
    println!(
        "raw-order bench(ms): is->name->tags={} tags->name->is={} name->is->tags={} (matches {}/{}/{})",
        da, db, dc, ma, mb, mc
    );
}

#[test]
#[ignore]
fn bench_parallel_threshold_sweep() {
    fn build_arena(arena_size: usize) -> NodeArena {
        let mut arena = NodeArena::with_capacity(arena_size + 1);
        for i in 0..arena_size {
            let mut node = if i % 5 == 0 {
                SceneNode::new(SceneNodeData::MeshInstance3D(MeshInstance3D::new()))
            } else {
                SceneNode::new(SceneNodeData::Node3D(Node3D::new()))
            };
            if i % 4 == 0 {
                node.set_name("enemy".to_string());
            } else {
                node.set_name("npc".to_string());
            }
            if i % 2 == 0 {
                node.add_tag(TagID::from_string("enemy"));
            }
            if i % 7 == 0 {
                node.add_tag(TagID::from_string("alive"));
            }
            if i % 19 == 0 {
                node.add_tag(TagID::from_string("boss"));
            }
            let _ = arena.insert(node);
        }
        arena
    }

    fn avg_ms(arena: &NodeArena, query: &TagQuery, workers: usize, rounds: usize) -> (u128, usize) {
        let mut total = 0_u128;
        let mut matches = 0_usize;
        for _ in 0..rounds {
            let start = std::time::Instant::now();
            let result =
                query_node_ids_with_worker_override(arena, query.clone(), Some(workers), None);
            total += start.elapsed().as_micros();
            matches = result.len();
            black_box(&result);
        }
        (total / rounds as u128, matches)
    }

    let parallel_workers = std::thread::available_parallelism()
        .map(|n| n.get())
        .unwrap_or(4)
        .max(2);
    let rounds = 6;
    let counts = [2_500, 5_000, 10_000, 20_000, 50_000, 100_000];

    let selective = TagQuery {
        expr: Some(QueryExpr::All(vec![
            QueryExpr::IsType(vec![NodeType::MeshInstance3D]),
            QueryExpr::Name(vec!["enemy".to_string()]),
            QueryExpr::Tags(vec![
                TagID::from_string("enemy"),
                TagID::from_string("alive"),
                TagID::from_string("boss"),
            ]),
        ])),
        scope: QueryScope::Root,
    };

    let broad = TagQuery {
        expr: Some(QueryExpr::Any(vec![QueryExpr::Tags(vec![
            TagID::from_string("enemy"),
            TagID::from_string("alive"),
        ])])),
        scope: QueryScope::Root,
    };

    println!(
        "parallel sweep (avg over {} runs, parallel workers={})",
        rounds, parallel_workers
    );
    for &count in &counts {
        let arena = build_arena(count);

        let (sel_serial_us, sel_matches) = avg_ms(&arena, &selective, 1, rounds);
        let (sel_parallel_us, _) = avg_ms(&arena, &selective, parallel_workers, rounds);

        let (broad_serial_us, broad_matches) = avg_ms(&arena, &broad, 1, rounds);
        let (broad_parallel_us, _) = avg_ms(&arena, &broad, parallel_workers, rounds);

        let sel_speedup = sel_serial_us as f64 / sel_parallel_us.max(1) as f64;
        let broad_speedup = broad_serial_us as f64 / broad_parallel_us.max(1) as f64;
        println!(
            "nodes={count:>6} | selective serial/par={}us/{}us speedup={:.2}x matches={} | broad serial/par={}us/{}us speedup={:.2}x matches={}",
            sel_serial_us,
            sel_parallel_us,
            sel_speedup,
            sel_matches,
            broad_serial_us,
            broad_parallel_us,
            broad_speedup,
            broad_matches,
        );
    }
}

#[test]
#[ignore]
fn bench_tag_indexed_candidates() {
    use ahash::{AHashMap, AHashSet};

    let arena_size = 100_000;
    let mut arena = NodeArena::with_capacity(arena_size + 1);
    for i in 0..arena_size {
        let mut node = if i % 5 == 0 {
            SceneNode::new(SceneNodeData::MeshInstance3D(MeshInstance3D::new()))
        } else {
            SceneNode::new(SceneNodeData::Node3D(Node3D::new()))
        };
        if i % 3 == 0 {
            node.add_tag(TagID::from_string("enemy"));
        }
        if i % 7 == 0 {
            node.add_tag(TagID::from_string("alive"));
        }
        if i % 19 == 0 {
            node.add_tag(TagID::from_string("boss"));
        }
        let _ = arena.insert(node);
    }

    let mut tag_index: AHashMap<TagID, AHashSet<perro_ids::NodeID>> = AHashMap::default();
    for (id, node) in arena.iter() {
        for &tag in node.tags_slice() {
            tag_index.entry(tag).or_default().insert(id);
        }
    }

    let selective = TagQuery {
        expr: Some(QueryExpr::All(vec![QueryExpr::Tags(vec![
            TagID::from_string("enemy"),
            TagID::from_string("alive"),
            TagID::from_string("boss"),
        ])])),
        scope: QueryScope::Root,
    };

    let rounds = 10;
    let mut plain_us = 0u128;
    let mut indexed_us = 0u128;
    let mut plain_matches = 0usize;
    let mut indexed_matches = 0usize;

    for _ in 0..rounds {
        let t0 = std::time::Instant::now();
        let plain = query_node_ids_with_worker_override(&arena, selective.clone(), Some(1), None);
        plain_us += t0.elapsed().as_micros();
        plain_matches = plain.len();
        black_box(&plain);

        let t1 = std::time::Instant::now();
        let indexed = query_node_ids_with_worker_override(
            &arena,
            selective.clone(),
            Some(1),
            Some(&tag_index),
        );
        indexed_us += t1.elapsed().as_micros();
        indexed_matches = indexed.len();
        black_box(&indexed);
    }

    println!(
        "bench_tag_indexed_candidates: plain={}us indexed={}us speedup={:.2}x matches={}/{}",
        plain_us / rounds,
        indexed_us / rounds,
        (plain_us as f64 / indexed_us.max(1) as f64),
        plain_matches,
        indexed_matches,
    );
}
