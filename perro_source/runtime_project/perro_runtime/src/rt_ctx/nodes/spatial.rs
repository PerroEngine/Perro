use super::*;

impl Runtime {
    /// Builds slot-indexed global positions when the query has a spatial
    /// clause. Global transforms are resolved (and cached) up front so the
    /// scan itself stays read-only and parallel-safe.
    ///
    /// Dirty transforms are refreshed once so the fill loop mostly reads the
    /// clean global-transform cache directly. Buffers are recycled between
    /// queries via [`recycle_query_spatial_index`](Self::recycle_query_spatial_index).
    pub(super) fn build_query_spatial_index(
        &mut self,
        expr: &Option<QueryExpr>,
        scope: QueryScope,
        candidates: Option<&[NodeID]>,
    ) -> Option<super::super::query::QuerySpatialIndex> {
        let (needs_2d, needs_3d) = expr
            .as_ref()
            .map_or((false, false), QueryExpr::spatial_dims);
        if !needs_2d && !needs_3d {
            return None;
        }

        self.propagate_pending_transform_dirty();
        self.refresh_dirty_global_transforms();

        let slot_count = self.nodes.slot_count();
        let mut pos_2d = std::mem::take(&mut self.node_index.query_spatial_pos_2d);
        let mut pos_3d = std::mem::take(&mut self.node_index.query_spatial_pos_3d);

        // Root-scope queries w/ a small candidate set (rare tag/name index)
        // only need positions for those ids -- filling every occupied slot
        // to then scan a handful of candidates wastes the whole arena walk.
        // Only worth it once candidates are a small fraction of slot_count;
        // otherwise the linear whole-arena walk below is more cache-friendly.
        let use_candidate_fill = matches!(scope, QueryScope::Root)
            && candidates.is_some_and(|ids| {
                ids.len() < slot_count / QUERY_SPATIAL_CANDIDATE_FILL_DIVISOR.max(1)
            });

        if use_candidate_fill {
            // Only candidate slots can be read by this query. Retain other
            // slots and reset the queried lanes individually, including type
            // changes and reused generations. Avoid clearing the entire arena
            // for a rare-tag query and allocate only requested dimensions.
            if needs_2d {
                pos_2d.resize(slot_count, None);
            }
            if needs_3d {
                pos_3d.resize(slot_count, None);
            }
            let ids = candidates.expect("checked by use_candidate_fill");
            for &id in ids {
                let slot = id.index() as usize;
                if let Some(position) = pos_2d.get_mut(slot) {
                    *position = None;
                }
                if let Some(position) = pos_3d.get_mut(slot) {
                    *position = None;
                }
                let Some(node) = self.nodes.get(id) else {
                    continue;
                };
                let kind = node.spatial();
                self.fill_query_spatial_slot(
                    kind,
                    needs_2d,
                    needs_3d,
                    id.index() as usize,
                    id,
                    &mut pos_2d,
                    &mut pos_3d,
                );
            }
            return Some(super::super::query::QuerySpatialIndex { pos_2d, pos_3d });
        }

        pos_2d.clear();
        pos_3d.clear();
        pos_2d.resize(slot_count, None);
        pos_3d.resize(slot_count, None);

        match scope {
            QueryScope::Root => {
                let workers = if slot_count >= QUERY_SPATIAL_PAR_MIN_SLOTS {
                    perro_structs::structs::devsim::worker_count()
                } else {
                    1
                };
                if workers > 1 {
                    // Parallel pass over the clean transform caches; stale
                    // slots are collected and resolved sequentially below.
                    let chunk = slot_count.div_ceil(workers);
                    let arena = &self.nodes;
                    let transforms = &self.transforms;
                    let dirty = &self.dirty;
                    let miss_lists: Vec<Vec<(usize, NodeID)>> = pos_2d
                        .par_chunks_mut(chunk)
                        .zip(pos_3d.par_chunks_mut(chunk))
                        .enumerate()
                        .map(|(chunk_index, (chunk_2d, chunk_3d))| {
                            let base = chunk_index * chunk;
                            let mut misses = Vec::new();
                            for offset in 0..chunk_2d.len() {
                                let index = base + offset;
                                if index == 0 {
                                    continue;
                                }
                                let Some((id, node)) = arena.slot_get(index) else {
                                    continue;
                                };
                                match node.spatial() {
                                    Spatial::TwoD if needs_2d => {
                                        match read_clean_global_pos_2d(transforms, dirty, id, index)
                                        {
                                            Some(position) => chunk_2d[offset] = Some(position),
                                            None => misses.push((index, id)),
                                        }
                                    }
                                    Spatial::ThreeD if needs_3d => {
                                        match read_clean_global_pos_3d(transforms, dirty, id, index)
                                        {
                                            Some(position) => chunk_3d[offset] = Some(position),
                                            None => misses.push((index, id)),
                                        }
                                    }
                                    _ => {}
                                }
                            }
                            misses
                        })
                        .collect();
                    for (index, id) in miss_lists.into_iter().flatten() {
                        let Some(node) = self.nodes.get(id) else {
                            continue;
                        };
                        let kind = node.spatial();
                        self.fill_query_spatial_slot(
                            kind,
                            needs_2d,
                            needs_3d,
                            index,
                            id,
                            &mut pos_2d,
                            &mut pos_3d,
                        );
                    }
                } else {
                    for index in 1..slot_count {
                        let Some((id, node)) = self.nodes.slot_get(index) else {
                            continue;
                        };
                        let kind = node.spatial();
                        self.fill_query_spatial_slot(
                            kind,
                            needs_2d,
                            needs_3d,
                            index,
                            id,
                            &mut pos_2d,
                            &mut pos_3d,
                        );
                    }
                }
            }
            QueryScope::Subtree(root_id) => {
                if !root_id.is_nil() {
                    let mut stack = vec![root_id];
                    while let Some(id) = stack.pop() {
                        let Some(node) = self.nodes.get(id) else {
                            continue;
                        };
                        if let Some(children) = self.nodes.children(id) {
                            stack.extend_from_slice(children);
                        }
                        let kind = node.spatial();
                        self.fill_query_spatial_slot(
                            kind,
                            needs_2d,
                            needs_3d,
                            id.index() as usize,
                            id,
                            &mut pos_2d,
                            &mut pos_3d,
                        );
                    }
                }
            }
        }
        Some(super::super::query::QuerySpatialIndex { pos_2d, pos_3d })
    }

    #[inline]
    #[allow(clippy::too_many_arguments)]
    pub(super) fn fill_query_spatial_slot(
        &mut self,
        kind: Spatial,
        needs_2d: bool,
        needs_3d: bool,
        index: usize,
        id: NodeID,
        pos_2d: &mut [Option<Vector2>],
        pos_3d: &mut [Option<Vector3>],
    ) {
        match kind {
            Spatial::TwoD if needs_2d => {
                pos_2d[index] = self
                    .cached_clean_global_2d(id)
                    .or_else(|| self.get_global_transform_2d(id))
                    .map(|transform| transform.position);
            }
            Spatial::ThreeD if needs_3d => {
                pos_3d[index] = self
                    .cached_clean_global_3d(id)
                    .or_else(|| self.get_global_transform_3d(id))
                    .map(|transform| transform.position);
            }
            _ => {}
        }
    }

    pub(super) fn recycle_query_spatial_index(
        &mut self,
        index: Option<super::super::query::QuerySpatialIndex>,
    ) {
        if let Some(index) = index {
            self.node_index.query_spatial_pos_2d = index.pos_2d;
            self.node_index.query_spatial_pos_3d = index.pos_3d;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use perro_ids::TagID;
    use perro_nodes::{Node2D, Node3D};
    use perro_runtime_api::sub_apis::{NodeQuery, QueryBounds};

    #[test]
    fn sparse_spatial_fill_tracks_moves_removed_tags_and_reused_dimensions() {
        let mut runtime = Runtime::new();
        let tag = TagID::from_string("sparse_spatial");
        for _ in 0..64 {
            NodeAPI::create::<Node3D>(&mut runtime);
        }
        let first = NodeAPI::create::<Node3D>(&mut runtime);
        assert!(runtime.add_node_tag(first, tag));
        let query = NodeQuery {
            expr: Some(QueryExpr::All(vec![
                QueryExpr::Tags(vec![tag]),
                QueryExpr::Within(QueryBounds::Box3D {
                    origin: Vector3::ZERO,
                    size: Vector3::new(2.0, 2.0, 2.0),
                }),
            ])),
            scope: QueryScope::Root,
        };
        assert_eq!(runtime.query_first_node(query.as_view()), Some(first));
        assert!(runtime.node_index.query_spatial_pos_2d.is_empty());
        let _ = runtime.with_node_mut::<Node3D, _, _>(first, |node| node.position.x = 10.0);
        assert_eq!(runtime.query_first_node(query.as_view()), None);
        assert!(runtime.remove_node(first));
        let second = NodeAPI::create::<Node2D>(&mut runtime);
        assert_eq!(first.index(), second.index());
        assert!(runtime.add_node_tag(second, tag));
        assert_eq!(runtime.query_first_node(query.as_view()), None);
        assert!(runtime.remove_node(second));
        let third = NodeAPI::create::<Node3D>(&mut runtime);
        assert!(runtime.add_node_tag(third, tag));
        assert_eq!(runtime.query_nodes(query.as_view()), vec![third]);
        assert!(runtime.remove_node_tag(third, tag));
        assert_eq!(runtime.query_first_node(query.as_view()), None);
    }
}
