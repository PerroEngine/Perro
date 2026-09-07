use super::*;
use std::sync::{Arc, Weak};

/// Weak ownership prevents asset retention and makes pointer reuse impossible
/// while a plan is cached. Resource replacement gets its own plan.
pub(super) struct GraphPlan {
    asset: Weak<AnimationTreeAsset>,
    order: Vec<usize>,
    shared: Vec<bool>,
    acyclic: bool,
}

impl GraphPlan {
    fn new(asset: &Arc<AnimationTreeAsset>) -> Self {
        let mut order: Vec<_> = (0..asset.nodes.len()).collect();
        order
            .sort_unstable_by(|&a, &b| asset.nodes[a].key.cmp(&asset.nodes[b].key).then(a.cmp(&b)));
        let resolve = |key: &str| {
            let at = order.partition_point(|&index| asset.nodes[index].key.as_ref() < key);
            order
                .get(at)
                .copied()
                .filter(|&index| asset.nodes[index].key.as_ref() == key)
        };
        let mut indegree = vec![0usize; asset.nodes.len()];
        let mut edges = vec![Vec::new(); asset.nodes.len()];
        for (index, node) in asset.nodes.iter().enumerate() {
            let mut add = |key: &str| {
                if let Some(target) = resolve(key) {
                    edges[index].push(target);
                    indegree[target] += 1;
                }
            };
            match &node.kind {
                AnimationTreeNodeKind::Blend { inputs, .. } => {
                    for input in inputs.iter() {
                        add(input);
                    }
                }
                AnimationTreeNodeKind::Add { base, inputs, .. } => {
                    add(base);
                    for input in inputs.iter() {
                        add(input);
                    }
                }
                AnimationTreeNodeKind::Invert { input, .. } => add(input),
            }
        }
        let shared = indegree.iter().map(|&count| count > 1).collect();
        let mut ready: Vec<_> = indegree
            .iter()
            .enumerate()
            .filter_map(|(index, &count)| (count == 0).then_some(index))
            .collect();
        let mut visited = 0usize;
        while let Some(index) = ready.pop() {
            visited += 1;
            for &target in &edges[index] {
                indegree[target] -= 1;
                if indegree[target] == 0 {
                    ready.push(target);
                }
            }
        }
        Self {
            asset: Arc::downgrade(asset),
            order,
            shared,
            acyclic: visited == asset.nodes.len(),
        }
    }

    pub(super) fn memoize(&self, index: usize) -> bool {
        self.acyclic && self.shared[index]
    }

    fn resolve(&self, asset: &AnimationTreeAsset, key: &str) -> Option<usize> {
        let at = self
            .order
            .partition_point(|&index| asset.nodes[index].key.as_ref() < key);
        self.order
            .get(at)
            .copied()
            .filter(|&index| asset.nodes[index].key.as_ref() == key)
    }
}

impl EvalScratch {
    pub(super) fn prepare_graph(&mut self, asset: &Arc<AnimationTreeAsset>) {
        if asset.nodes.is_empty() {
            return;
        }
        let plan = if let Some(index) = self
            .graph_plans
            .iter()
            .position(|plan| plan.asset.as_ptr() == Arc::as_ptr(asset))
        {
            self.graph_plans.swap_remove(index)
        } else {
            self.graph_plans
                .retain(|plan| plan.asset.strong_count() > 0);
            GraphPlan::new(asset)
        };
        if plan.acyclic && plan.shared.iter().any(|&shared| shared) {
            self.memo.resize_with(asset.nodes.len(), || None);
        }
        self.active_plan = Some(plan);
    }

    pub(super) fn resolve_graph_node(
        &self,
        asset: &AnimationTreeAsset,
        key: &str,
    ) -> Option<usize> {
        match &self.active_plan {
            Some(plan) if asset.nodes.len() > 8 => plan.resolve(asset, key),
            _ => asset.nodes.iter().position(|node| node.key.as_ref() == key),
        }
    }
}
