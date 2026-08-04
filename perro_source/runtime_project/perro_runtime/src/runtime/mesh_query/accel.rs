use super::*;

/// Byte budget for one [`Runtime`]'s query-mesh cache. Query geometry is
/// rebuilt on demand from the mesh source / runtime mesh data, so eviction
/// only costs a re-decode + BVH rebuild on the next query against that mesh.
/// Mirrors the decoded-texture budget pattern in `perro_graphics` resources.
const QUERY_MESH_CACHE_MAX_BYTES: usize = 64 * 1024 * 1024;

#[inline]
pub(super) fn runtime_mesh_query_cache_key(mesh_id: MeshID, revision: u64) -> u64 {
    mesh_id
        .as_u64()
        .wrapping_mul(0x9e37_79b9_7f4a_7c15)
        .rotate_left(17)
        ^ revision
        ^ 0x5bf0_3635_6ad6_22d5
}

struct QueryMeshCacheEntry {
    mesh: Arc<QueryMeshData>,
    bytes: usize,
    /// `mesh_id.as_u64()` for revision-keyed runtime meshes, `None` for
    /// source-path entries. Used to keep `latest_runtime_key_by_mesh` in sync
    /// when the entry is removed or evicted.
    runtime_mesh: Option<u64>,
    /// LRU stamp: `clock` value at the last hit (or insert).
    last_used: u64,
}

/// Cache of decoded query geometry (vertices + triangles + BVH), keyed by
/// either `string_to_u64(source)` (source-path meshes) or
/// [`runtime_mesh_query_cache_key`] (runtime meshes, key encodes revision).
///
/// Lives on [`Runtime`], NOT in a static. Both key spaces are only unique
/// within one runtime: `MeshID`s and revisions restart per `Runtime`, so a
/// process-global map handed the second `Runtime`'s meshes the first one's
/// geometry for every query, and a source path resolves against the runtime's
/// own project. Any host with two live runtimes -- editor + play-mode preview,
/// tooling, multi-instance embedding, one test binary -- hit that. Per-runtime
/// also means the entries die with the runtime instead of squatting the budget
/// until LRU pressure evicts them.
///
/// Two leak guards on top of the plain map it replaced:
/// - a runtime mesh's revision bump inserts under a NEW key, so the previous
///   revision's entry is dropped explicitly via `latest_runtime_key_by_mesh`
///   instead of lingering forever, and
/// - total resident bytes are capped at [`QUERY_MESH_CACHE_MAX_BYTES`] with
///   least-recently-used eviction (touch-on-get), enforced only on inserts
///   that push the cache over budget.
#[derive(Default)]
pub(crate) struct QueryMeshCache {
    entries: AHashMap<u64, QueryMeshCacheEntry>,
    /// `mesh_id.as_u64()` -> cache key of the latest inserted revision.
    latest_runtime_key_by_mesh: AHashMap<u64, u64>,
    total_bytes: usize,
    clock: u64,
}

impl QueryMeshCache {
    pub(super) fn get(&mut self, key: u64) -> Option<Arc<QueryMeshData>> {
        self.clock = self.clock.wrapping_add(1);
        let now = self.clock;
        let entry = self.entries.get_mut(&key)?;
        entry.last_used = now;
        Some(entry.mesh.clone())
    }

    /// Insert a source-path mesh (`string_to_u64(source)` key). The key is
    /// stable for a given source, so a re-insert replaces in place.
    pub(super) fn insert_source_mesh(&mut self, key: u64, mesh: Arc<QueryMeshData>) {
        self.insert_entry(key, mesh, None);
    }

    /// Insert a runtime mesh under its `(mesh_id, revision)` key, dropping the
    /// previously cached revision (if any) so revision bumps never accumulate.
    pub(super) fn insert_runtime_mesh(
        &mut self,
        mesh_id: MeshID,
        revision: u64,
        mesh: Arc<QueryMeshData>,
    ) {
        let key = runtime_mesh_query_cache_key(mesh_id, revision);
        if let Some(old_key) = self
            .latest_runtime_key_by_mesh
            .insert(mesh_id.as_u64(), key)
            && old_key != key
        {
            self.remove_entry(old_key);
        }
        self.insert_entry(key, mesh, Some(mesh_id.as_u64()));
    }

    fn insert_entry(&mut self, key: u64, mesh: Arc<QueryMeshData>, runtime_mesh: Option<u64>) {
        self.clock = self.clock.wrapping_add(1);
        self.remove_entry(key);
        let bytes = query_mesh_data_bytes(&mesh);
        self.total_bytes = self.total_bytes.saturating_add(bytes);
        self.entries.insert(
            key,
            QueryMeshCacheEntry {
                mesh,
                bytes,
                runtime_mesh,
                last_used: self.clock,
            },
        );
        self.enforce_budget(key);
    }

    fn remove_entry(&mut self, key: u64) {
        if let Some(old) = self.entries.remove(&key) {
            self.total_bytes = self.total_bytes.saturating_sub(old.bytes);
            if let Some(mesh_id) = old.runtime_mesh
                && self.latest_runtime_key_by_mesh.get(&mesh_id) == Some(&key)
            {
                self.latest_runtime_key_by_mesh.remove(&mesh_id);
            }
        }
    }

    /// LRU eviction: when resident bytes exceed the budget, drop the
    /// least-recently-used entries until back under (or only `keep` remains).
    /// `keep` (the entry that triggered enforcement) is never evicted. Runs
    /// only on over-budget inserts, so steady-state lookups scan nothing.
    fn enforce_budget(&mut self, keep: u64) {
        if self.total_bytes <= QUERY_MESH_CACHE_MAX_BYTES {
            return;
        }
        let now = self.clock;
        let mut candidates: Vec<(u64, u64)> = self
            .entries
            .iter()
            .filter(|(key, _)| **key != keep)
            .map(|(key, entry)| (now.wrapping_sub(entry.last_used), *key))
            .collect();
        // oldest (largest stamp age) first.
        candidates.sort_unstable_by_key(|candidate| std::cmp::Reverse(candidate.0));
        for (_, key) in candidates {
            if self.total_bytes <= QUERY_MESH_CACHE_MAX_BYTES {
                break;
            }
            self.remove_entry(key);
        }
    }
}

/// Resident-byte estimate for one cache entry: per-vertex position/uv/skin
/// lanes, per-triangle index + acceleration data, and the BVH arrays.
fn query_mesh_data_bytes(mesh: &QueryMeshData) -> usize {
    use std::mem::size_of;
    size_of::<QueryMeshData>()
        + mesh.vertices.len() * size_of::<Vec3>()
        + mesh.uv0.len() * size_of::<Vec2>()
        + mesh.paint_uv.len() * size_of::<Vec2>()
        + mesh.joints.len() * size_of::<[u16; 4]>()
        + mesh.weights.len() * size_of::<[f32; 4]>()
        + mesh.triangles.len() * size_of::<QueryTri>()
        + mesh.tri_accel.len() * size_of::<QueryTriAccel>()
        + mesh.bvh_nodes.len() * size_of::<QueryBvhNode>()
        + mesh.bvh_tri_indices.len() * size_of::<u32>()
}

thread_local! {
    pub(super) static GLTF_POS_SCRATCH: RefCell<Vec<[f32; 3]>> = const { RefCell::new(Vec::new()) };
    pub(super) static GLTF_INDEX_SCRATCH: RefCell<Vec<u32>> = const { RefCell::new(Vec::new()) };
}

pub(super) struct QueryNodeData {
    pub(super) mesh_id: MeshID,
    pub(super) source: Option<String>,
    pub(super) surfaces: Vec<MeshSurfaceBinding>,
    pub(super) instance_local: Vec<Mat4>,
    pub(super) skeleton: Option<NodeID>,
    /// Lazily built instance-level acceleration structure (see
    /// [`InstanceAccel`]). Lives on the snapshot so it inherits the snapshot's
    /// `(structural_revision, node_change_stamp)` invalidation for free: a
    /// write to this node retires the snapshot, which drops the accel with it.
    pub(super) instance_accel: Mutex<InstanceAccelSlot>,
}

/// Per-node cache entry for [`QueryNodeData`]. `instance_local` is the
/// expensive-to-rebuild part on `MultiMeshInstance3D` (one
/// `Mat4::from_scale_rotation_translation` per instance, plus a
/// `surfaces.clone()`), so point/ray/region queries reuse it across calls
/// instead of rebuilding on every query.
struct QueryNodeDataCacheEntry {
    data: Arc<QueryNodeData>,
    /// Resident-byte estimate, held so budget accounting never re-walks the
    /// snapshot.
    bytes: usize,
    /// `nodes.structural_revision()` when `data` was built. Covers what the
    /// snapshot reads from OUTSIDE the node (`render_3d.mesh_sources`, which
    /// is only written alongside scene-graph inserts).
    built_at_structural: u64,
    /// `nodes.node_change_stamp()` when `data` was built. Moves only when THIS
    /// node is written, so an unrelated node's per-frame mutation no longer
    /// retires the entry.
    built_at_stamp: u64,
}

/// Byte budget for the per-node snapshot cache. A `MultiMeshInstance3D`
/// snapshot is 64B per instance, so a few 100k-instance nodes would otherwise
/// pin hundreds of MB. Same rebuild-on-demand tradeoff as
/// [`QUERY_MESH_CACHE_MAX_BYTES`].
const QUERY_NODE_CACHE_MAX_BYTES: usize = 32 * 1024 * 1024;

/// Per-node [`QueryNodeData`] snapshots keyed by [`NodeID`].
///
/// Entry validity is `(structural_revision, node_change_stamp)`. The stamp is
/// per node, so the common live-scene case — some other node moved since the
/// last query — is now a hit. The previous key was the GLOBAL mutation
/// revision, which any node write moved, so the hit rate was ~0 and every miss
/// additionally dropped every other node's entry.
#[derive(Default)]
pub(crate) struct QueryNodeDataCache {
    entries: AHashMap<NodeID, QueryNodeDataCacheEntry>,
    total_bytes: usize,
}

impl QueryNodeDataCache {
    /// Cached snapshot for `id`, or `None` when absent or retired.
    pub(super) fn get(
        &self,
        id: NodeID,
        structural: u64,
        stamp: u64,
    ) -> Option<Arc<QueryNodeData>> {
        let entry = self.entries.get(&id)?;
        (entry.built_at_structural == structural && entry.built_at_stamp == stamp)
            .then(|| entry.data.clone())
    }

    /// Store a freshly built snapshot. `still_fresh(id, structural, stamp)`
    /// decides which existing entries survive an over-budget sweep; it only
    /// runs when this insert pushes the cache past its budget.
    pub(super) fn insert(
        &mut self,
        id: NodeID,
        data: Arc<QueryNodeData>,
        structural: u64,
        stamp: u64,
        still_fresh: impl Fn(NodeID, u64, u64) -> bool,
    ) {
        let bytes = query_node_data_bytes(&data);
        let replaced = self.entries.insert(
            id,
            QueryNodeDataCacheEntry {
                data,
                bytes,
                built_at_structural: structural,
                built_at_stamp: stamp,
            },
        );
        if let Some(old) = replaced {
            self.total_bytes = self.total_bytes.saturating_sub(old.bytes);
        }
        self.total_bytes = self.total_bytes.saturating_add(bytes);
        if self.total_bytes > QUERY_NODE_CACHE_MAX_BYTES {
            self.enforce_budget(id, still_fresh);
        }
    }

    /// Drop entries that can never hit again (node dead, or its data moved),
    /// then — only if that did not reclaim enough — everything but the entry
    /// that triggered enforcement. Runs on over-budget inserts only, so a
    /// steady-state miss touches one key instead of the whole map.
    fn enforce_budget(&mut self, keep: NodeID, still_fresh: impl Fn(NodeID, u64, u64) -> bool) {
        let mut live_bytes = 0;
        self.entries.retain(|id, entry| {
            let live =
                *id == keep || still_fresh(*id, entry.built_at_structural, entry.built_at_stamp);
            if live {
                live_bytes += entry.bytes;
            }
            live
        });
        self.total_bytes = live_bytes;
        if self.total_bytes <= QUERY_NODE_CACHE_MAX_BYTES {
            return;
        }
        // Everything resident is still valid: no LRU signal to pick from, so
        // keep only the snapshot this insert just paid for.
        let mut kept_bytes = 0;
        self.entries.retain(|id, entry| {
            if *id != keep {
                return false;
            }
            kept_bytes += entry.bytes;
            true
        });
        self.total_bytes = kept_bytes;
    }

    pub(crate) fn clear(&mut self) {
        self.entries.clear();
        self.total_bytes = 0;
    }
}

/// Resident-byte estimate for one node snapshot. Per-instance `Mat4`s dominate
/// (64B each on `MultiMeshInstance3D`).
///
/// Snapshots big enough to build an [`InstanceAccel`] are charged for it up
/// front, before the lazy build runs: the cache measures at insert time and
/// never re-measures, so budgeting the projected size is the only way the accel
/// bytes are ever accounted. Over-charging an accel that is never built only
/// makes eviction slightly earlier.
fn query_node_data_bytes(data: &QueryNodeData) -> usize {
    use std::mem::size_of;
    let instances = data.instance_local.len();
    let accel = if instances >= INSTANCE_ACCEL_MIN_INSTANCES {
        instances * INSTANCE_ACCEL_BYTES_PER_INSTANCE
    } else {
        0
    };
    size_of::<QueryNodeData>()
        + data.source.as_ref().map_or(0, String::len)
        + data.surfaces.len() * size_of::<MeshSurfaceBinding>()
        + instances * size_of::<Mat4>()
        + accel
}

/// Minimum instance count before a node builds an [`InstanceAccel`]. Below
/// this the plain per-instance scan costs less than the build would amortize
/// (build is ~16ns/instance against ~400ns/instance for a full instance test,
/// so the crossover is far below this; the margin covers cheap meshes).
pub(super) const INSTANCE_ACCEL_MIN_INSTANCES: usize = 32;

/// Instances per [`InstanceAccel`] BVH leaf.
const INSTANCE_BVH_LEAF: usize = 4;

/// Resident bytes charged per instance for an [`InstanceAccel`]: two AABB
/// corners, one order slot, and (leaf size 4, median split) roughly half a BVH
/// node.
const INSTANCE_ACCEL_BYTES_PER_INSTANCE: usize = 2 * std::mem::size_of::<Vec3>()
    + std::mem::size_of::<u32>()
    + std::mem::size_of::<InstanceBvhNode>() / 2;

/// Relative slack applied when a global-space distance bound is converted into
/// the node-space `t` used for instance-box culling. The conversion
/// `global_t = k * node_t` is exact in real arithmetic (see [`NodeSpaceRay`]);
/// the slack absorbs f32 drift so culling can never reject a box the exact
/// per-instance path would have accepted.
const INSTANCE_ACCEL_T_SLACK: f32 = 1.0 + 1.0e-3;

/// Absolute node-space floor added on top of [`INSTANCE_ACCEL_T_SLACK`], for
/// bounds near zero where a relative slack vanishes.
const INSTANCE_ACCEL_T_EPSILON: f32 = 1.0e-4;

/// Relative + absolute padding applied to each instance AABB, so f32 rounding
/// in the 8-corner transform can never shrink a box below the geometry it
/// bounds.
const INSTANCE_AABB_PAD_RELATIVE: f32 = 1.0e-4;
const INSTANCE_AABB_PAD_ABSOLUTE: f32 = 1.0e-5;

#[cfg(test)]
thread_local! {
    /// Test hook: forces the linear per-instance path so a test can compare it
    /// against the accelerated path on the same runtime. Thread-local, so
    /// parallel test threads never observe each other's setting.
    static INSTANCE_ACCEL_DISABLED: std::cell::Cell<bool> = const { std::cell::Cell::new(false) };
}

/// Run `body` with the instance acceleration structure forced off. Restores on
/// unwind so a failing assertion inside `body` cannot leak the flag into the
/// next test on this thread.
#[cfg(test)]
pub(super) fn without_instance_accel<T>(body: impl FnOnce() -> T) -> T {
    struct Restore;
    impl Drop for Restore {
        fn drop(&mut self) {
            INSTANCE_ACCEL_DISABLED.with(|flag| flag.set(false));
        }
    }
    INSTANCE_ACCEL_DISABLED.with(|flag| flag.set(true));
    let _restore = Restore;
    body()
}

#[inline]
fn instance_accel_disabled() -> bool {
    #[cfg(test)]
    {
        INSTANCE_ACCEL_DISABLED.with(std::cell::Cell::get)
    }
    #[cfg(not(test))]
    {
        false
    }
}

/// Lazily filled [`InstanceAccel`] slot. `built_for` records the mesh-local
/// AABB the boxes were derived from, so a mesh revision that changes the mesh
/// bounds rebuilds instead of culling against stale boxes. A build that fails
/// (degenerate instance matrix) is remembered as `Some(bounds)` with
/// `accel: None`, so the failure is not retried on every query.
#[derive(Default)]
pub(super) struct InstanceAccelSlot {
    built_for: Option<(Vec3, Vec3)>,
    accel: Option<Arc<InstanceAccel>>,
}

#[derive(Clone, Copy)]
struct InstanceBvhNode {
    aabb_min: Vec3,
    aabb_max: Vec3,
    /// `u32::MAX` marks a leaf.
    left: u32,
    right: u32,
    start: u32,
    count: u32,
}

/// BVH over a `MultiMeshInstance3D`'s per-instance AABBs, in the NODE's local
/// space.
///
/// Node-local (not global) is deliberate: the snapshot is keyed on this node's
/// change stamp, but the node's global transform also moves when a PARENT
/// moves, which the stamp does not see. Building in node space keeps the
/// structure independent of `get_global_transform_3d`, so a parent moving costs
/// nothing.
///
/// Ray queries transform the ray into node space once, cull instances against
/// this BVH, then run the unchanged per-instance test on the survivors — the
/// structure only decides WHICH instances are tested, never what a test
/// returns.
pub(super) struct InstanceAccel {
    /// Per-instance node-local AABB, indexed by instance index.
    aabb_min: Vec<Vec3>,
    aabb_max: Vec<Vec3>,
    /// Instance indices in BVH leaf order.
    order: Vec<u32>,
    nodes: Vec<InstanceBvhNode>,
}

/// A global-space ray expressed in a node's local space, plus the exact scalar
/// that maps node-space `t` back to the global distance the query reports.
///
/// For a fixed ray direction `d` in node space, every hit point is
/// `o + t*d`, so its global position is `M*o + t*(L*d)` where `M` is the node's
/// global transform and `L` its linear part. The reported metric is
/// `|hit_global - origin_global| = t * |L*d|`, i.e. `global_t = k * node_t` with
/// `k = |L*d|` CONSTANT for the ray. That identity is what lets node-space box
/// culling prune on a global-space best-so-far.
pub(super) struct NodeSpaceRay {
    origin: Vec3,
    dir: Vec3,
    /// Node-space limit matching the query's global `max_distance`.
    max_t: f32,
    /// `k` above: global distance per unit of node-space `t`.
    global_per_node_t: f32,
}

impl NodeSpaceRay {
    /// `None` when the node transform is degenerate or the mapping is not
    /// finite — callers then fall back to the exact linear path.
    pub(super) fn new(
        node_global: Mat4,
        origin_global: Vec3,
        dir_global: Vec3,
        max_t_global: f32,
    ) -> Option<Self> {
        let determinant = node_global.determinant();
        if !determinant.is_finite() || determinant.abs() <= 1.0e-20 {
            return None;
        }
        let node_from_global = node_global.inverse();
        let origin = node_from_global.transform_point3(origin_global);
        let dir_raw = node_from_global.transform_vector3(dir_global);
        let dir_len = dir_raw.length();
        if !origin.is_finite() || !dir_len.is_finite() || dir_len <= 1.0e-12 {
            return None;
        }
        let dir = dir_raw / dir_len;
        let global_per_node_t = (Mat3::from_mat4(node_global) * dir).length();
        if !global_per_node_t.is_finite() || global_per_node_t <= 1.0e-12 {
            return None;
        }
        let max_t = if max_t_global.is_finite() {
            node_limit(max_t_global, global_per_node_t)
        } else {
            f32::INFINITY
        };
        (dir.is_finite() && max_t > 0.0).then_some(Self {
            origin,
            dir,
            max_t,
            global_per_node_t,
        })
    }

    /// Node-space `t` bound corresponding to a global-space distance, widened
    /// so culling stays conservative under f32 drift.
    #[inline]
    pub(super) fn node_limit_for(&self, global_t: f32) -> f32 {
        node_limit(global_t, self.global_per_node_t)
    }
}

#[inline]
fn node_limit(global_t: f32, global_per_node_t: f32) -> f32 {
    (global_t / global_per_node_t) * INSTANCE_ACCEL_T_SLACK + INSTANCE_ACCEL_T_EPSILON
}

impl QueryNodeData {
    /// Instance acceleration structure for this snapshot against `mesh`, built
    /// on first use. `None` means "use the linear path" — too few instances, a
    /// degenerate instance matrix, or an unusable mesh AABB.
    pub(super) fn ray_instance_accel(&self, mesh: &QueryMeshData) -> Option<Arc<InstanceAccel>> {
        if self.instance_local.len() < INSTANCE_ACCEL_MIN_INSTANCES || instance_accel_disabled() {
            return None;
        }
        let root = mesh.bvh_nodes.first()?;
        let bounds = (root.aabb_min, root.aabb_max);
        if !bounds.0.is_finite() || !bounds.1.is_finite() || bounds.0.cmpgt(bounds.1).any() {
            return None;
        }
        let mut slot = self.instance_accel.lock().ok()?;
        if slot.built_for == Some(bounds) {
            return slot.accel.clone();
        }
        let built = InstanceAccel::build(&self.instance_local, bounds.0, bounds.1).map(Arc::new);
        slot.built_for = Some(bounds);
        slot.accel = built.clone();
        built
    }
}

impl InstanceAccel {
    fn build(instance_local: &[Mat4], mesh_min: Vec3, mesh_max: Vec3) -> Option<Self> {
        let count = instance_local.len();
        if count == 0 {
            return None;
        }
        let mut aabb_min = Vec::with_capacity(count);
        let mut aabb_max = Vec::with_capacity(count);
        let mut centroids = Vec::with_capacity(count);
        for local in instance_local {
            let mut lo = Vec3::splat(f32::INFINITY);
            let mut hi = Vec3::splat(f32::NEG_INFINITY);
            for x in [mesh_min.x, mesh_max.x] {
                for y in [mesh_min.y, mesh_max.y] {
                    for z in [mesh_min.z, mesh_max.z] {
                        let corner = local.transform_point3(Vec3::new(x, y, z));
                        lo = lo.min(corner);
                        hi = hi.max(corner);
                    }
                }
            }
            if !lo.is_finite() || !hi.is_finite() {
                return None;
            }
            let pad =
                (hi - lo) * INSTANCE_AABB_PAD_RELATIVE + Vec3::splat(INSTANCE_AABB_PAD_ABSOLUTE);
            let lo = lo - pad;
            let hi = hi + pad;
            centroids.push((lo + hi) * 0.5);
            aabb_min.push(lo);
            aabb_max.push(hi);
        }

        let mut order: Vec<u32> = (0..count as u32).collect();
        let mut nodes = Vec::with_capacity(2 * count.div_ceil(INSTANCE_BVH_LEAF) + 1);
        build_instance_bvh(
            &aabb_min, &aabb_max, &centroids, &mut order, &mut nodes, 0, count,
        );
        Some(Self {
            aabb_min,
            aabb_max,
            order,
            nodes,
        })
    }

    /// Push `(node_space_tmin, instance_index)` for every instance whose box
    /// the ray enters within `ray.max_t`. Instances not pushed cannot produce a
    /// hit: their box conservatively bounds all of the instance's geometry.
    pub(super) fn gather_ray_candidates(&self, ray: &NodeSpaceRay, out: &mut Vec<(f32, u32)>) {
        out.clear();
        if self.nodes.is_empty() {
            return;
        }
        let mut stack = QueryBvhStack::root();
        while let Some(node_idx) = stack.pop() {
            let Some(node) = self.nodes.get(node_idx as usize) else {
                continue;
            };
            if ray_aabb_tmin(ray.origin, ray.dir, node.aabb_min, node.aabb_max, ray.max_t).is_none()
            {
                continue;
            }
            if node.left == u32::MAX {
                let start = node.start as usize;
                let end = start + node.count as usize;
                for &instance in self.order.get(start..end).unwrap_or_default() {
                    let index = instance as usize;
                    let (Some(lo), Some(hi)) = (self.aabb_min.get(index), self.aabb_max.get(index))
                    else {
                        continue;
                    };
                    if let Some(tmin) = ray_aabb_tmin(ray.origin, ray.dir, *lo, *hi, ray.max_t) {
                        out.push((tmin, instance));
                    }
                }
            } else {
                stack.push(node.left);
                stack.push(node.right);
            }
        }
    }

    #[cfg(test)]
    pub(super) fn instance_count(&self) -> usize {
        self.aabb_min.len()
    }
}

/// Median-split BVH over instance boxes. Uses `select_nth_unstable_by` instead
/// of a full sort per level, so the build stays cheap enough to be worth paying
/// even for a multimesh that is rewritten every frame.
fn build_instance_bvh(
    aabb_min: &[Vec3],
    aabb_max: &[Vec3],
    centroids: &[Vec3],
    order: &mut [u32],
    nodes: &mut Vec<InstanceBvhNode>,
    start: usize,
    count: usize,
) -> u32 {
    let node_index = nodes.len() as u32;
    let mut node_min = Vec3::splat(f32::INFINITY);
    let mut node_max = Vec3::splat(f32::NEG_INFINITY);
    let mut centroid_min = Vec3::splat(f32::INFINITY);
    let mut centroid_max = Vec3::splat(f32::NEG_INFINITY);
    for &instance in &order[start..start + count] {
        let index = instance as usize;
        node_min = node_min.min(aabb_min[index]);
        node_max = node_max.max(aabb_max[index]);
        centroid_min = centroid_min.min(centroids[index]);
        centroid_max = centroid_max.max(centroids[index]);
    }
    nodes.push(InstanceBvhNode {
        aabb_min: node_min,
        aabb_max: node_max,
        left: u32::MAX,
        right: u32::MAX,
        start: start as u32,
        count: count as u32,
    });
    if count <= INSTANCE_BVH_LEAF {
        return node_index;
    }

    let extent = centroid_max - centroid_min;
    let axis = if extent.x >= extent.y && extent.x >= extent.z {
        0
    } else if extent.y >= extent.z {
        1
    } else {
        2
    };
    let key = |instance: &u32| {
        let centroid = centroids[*instance as usize];
        match axis {
            0 => centroid.x,
            1 => centroid.y,
            _ => centroid.z,
        }
    };
    let left_count = count / 2;
    order[start..start + count]
        .select_nth_unstable_by(left_count, |a, b| key(a).total_cmp(&key(b)));

    let left = build_instance_bvh(
        aabb_min, aabb_max, centroids, order, nodes, start, left_count,
    );
    let right = build_instance_bvh(
        aabb_min,
        aabb_max,
        centroids,
        order,
        nodes,
        start + left_count,
        count - left_count,
    );
    nodes[node_index as usize].left = left;
    nodes[node_index as usize].right = right;
    node_index
}

#[derive(Clone, Copy)]
pub(super) struct QueryHitCandidate {
    pub(super) instance_index: u32,
    pub(super) surface_index: u32,
    pub(super) triangle_index: u32,
    pub(super) barycentric: Vec3,
    pub(super) uv0: Vec2,
    pub(super) paint_uv: Vec2,
    pub(super) global_point: Vec3,
    pub(super) local_point: Vec3,
    pub(super) global_normal: Vec3,
    pub(super) local_normal: Vec3,
    pub(super) metric: f32,
}

#[inline]
fn hit_attrs(mesh: &QueryMeshData, tri_idx: usize, point: Vec3) -> Option<(Vec3, Vec2, Vec2)> {
    let tri = *mesh.triangles.get(tri_idx)?;
    let barycentric = barycentric_on_triangle(
        point,
        mesh.vertices[tri.a as usize],
        mesh.vertices[tri.b as usize],
        mesh.vertices[tri.c as usize],
    );
    let interpolate = |values: &[Vec2]| {
        values[tri.a as usize] * barycentric.x
            + values[tri.b as usize] * barycentric.y
            + values[tri.c as usize] * barycentric.z
    };
    Some((
        barycentric,
        interpolate(&mesh.uv0),
        interpolate(&mesh.paint_uv),
    ))
}

#[derive(Clone, Copy)]
pub(super) struct QueryRegionAcc {
    pub(super) tri_count: u32,
    pub(super) sum_local: Vec3,
    pub(super) sum_global: Vec3,
    pub(super) local_min: Vec3,
    pub(super) local_max: Vec3,
    pub(super) global_min: Vec3,
    pub(super) global_max: Vec3,
}

pub(super) struct QueryBvhStack {
    inline: [u32; 64],
    len: usize,
    spill: Vec<u32>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum QueryMeshStrategy {
    Linear,
    Bvh,
}

#[inline]
pub(super) fn query_mesh_strategy(tri_count: usize) -> QueryMeshStrategy {
    if tri_count <= QUERY_LINEAR_TRI_THRESHOLD {
        QueryMeshStrategy::Linear
    } else {
        QueryMeshStrategy::Bvh
    }
}

impl QueryBvhStack {
    #[inline]
    pub(super) fn root() -> Self {
        let mut inline = [0; 64];
        inline[0] = 0;
        Self {
            inline,
            len: 1,
            spill: Vec::new(),
        }
    }

    #[inline]
    pub(super) fn push(&mut self, node: u32) {
        if self.len < self.inline.len() {
            self.inline[self.len] = node;
            self.len += 1;
        } else {
            self.spill.push(node);
        }
    }

    #[inline]
    pub(super) fn pop(&mut self) -> Option<u32> {
        if let Some(node) = self.spill.pop() {
            return Some(node);
        }
        if self.len == 0 {
            return None;
        }
        self.len -= 1;
        Some(self.inline[self.len])
    }
}

pub(super) fn query_point_tri_local(
    mesh: &QueryMeshData,
    tri_idx: usize,
    p_local: Vec3,
    best: Option<QueryHitCandidate>,
) -> Option<Option<QueryHitCandidate>> {
    let tri = *mesh.triangles.get(tri_idx)?;
    let acc = *mesh.tri_accel.get(tri_idx)?;
    if let Some(hit) = best {
        let tri_d2 = aabb_distance2(p_local, acc.aabb_min, acc.aabb_max);
        if tri_d2 >= hit.metric {
            return Some(best);
        }
    }
    let a = mesh.vertices[tri.a as usize];
    let b = mesh.vertices[tri.b as usize];
    let c = mesh.vertices[tri.c as usize];
    let nearest_local = closest_point_on_triangle(p_local, a, b, c);
    let d2 = nearest_local.distance_squared(p_local);
    if let Some(hit) = best
        && d2 >= hit.metric
    {
        return Some(best);
    }
    let (barycentric, uv0, paint_uv) = hit_attrs(mesh, tri_idx, nearest_local)?;
    Some(Some(QueryHitCandidate {
        instance_index: 0,
        surface_index: tri.surface_index,
        triangle_index: tri_idx as u32,
        barycentric,
        uv0,
        paint_uv,
        global_point: nearest_local,
        local_point: nearest_local,
        global_normal: acc.normal,
        local_normal: acc.normal,
        metric: d2,
    }))
}

pub(super) fn query_point_tri_global(
    mesh: &QueryMeshData,
    tri_idx: usize,
    p_local: Vec3,
    instance_index: u32,
    global_from_mesh: Mat4,
    global_normal_basis: Mat3,
    best: Option<QueryHitCandidate>,
) -> Option<Option<QueryHitCandidate>> {
    let tri = *mesh.triangles.get(tri_idx)?;
    let acc = *mesh.tri_accel.get(tri_idx)?;
    if let Some(hit) = best {
        let tri_d2 = aabb_distance2(p_local, acc.aabb_min, acc.aabb_max);
        if tri_d2 >= hit.metric {
            return Some(best);
        }
    }
    let a = mesh.vertices[tri.a as usize];
    let b = mesh.vertices[tri.b as usize];
    let c = mesh.vertices[tri.c as usize];
    let nearest_local = closest_point_on_triangle(p_local, a, b, c);
    let d2 = nearest_local.distance_squared(p_local);
    if let Some(hit) = best
        && d2 >= hit.metric
    {
        return Some(best);
    }
    let nearest_global = global_from_mesh.transform_point3(nearest_local);
    let global_normal = (global_normal_basis * acc.normal).normalize_or_zero();
    let (barycentric, uv0, paint_uv) = hit_attrs(mesh, tri_idx, nearest_local)?;
    Some(Some(QueryHitCandidate {
        instance_index,
        surface_index: tri.surface_index,
        triangle_index: tri_idx as u32,
        barycentric,
        uv0,
        paint_uv,
        global_point: nearest_global,
        local_point: nearest_local,
        global_normal,
        local_normal: acc.normal,
        metric: d2,
    }))
}

pub(super) fn query_point_mesh_bvh(
    mesh: &QueryMeshData,
    p_local: Vec3,
    mut best: Option<QueryHitCandidate>,
) -> Option<Option<QueryHitCandidate>> {
    let mut stack = QueryBvhStack::root();
    while let Some(node_idx) = stack.pop() {
        let bvh = *mesh.bvh_nodes.get(node_idx as usize)?;
        let node_d2 = aabb_distance2(p_local, bvh.aabb_min, bvh.aabb_max);
        if let Some(hit) = best
            && node_d2 >= hit.metric
        {
            continue;
        }
        if bvh.left == u32::MAX || bvh.right == u32::MAX {
            let start = bvh.tri_start as usize;
            let end = start + bvh.tri_count as usize;
            for &tri_idx in &mesh.bvh_tri_indices[start..end] {
                best = query_point_tri_local(mesh, tri_idx as usize, p_local, best)?;
            }
        } else {
            let left = *mesh.bvh_nodes.get(bvh.left as usize)?;
            let right = *mesh.bvh_nodes.get(bvh.right as usize)?;
            let ld2 = aabb_distance2(p_local, left.aabb_min, left.aabb_max);
            let rd2 = aabb_distance2(p_local, right.aabb_min, right.aabb_max);
            if ld2 < rd2 {
                stack.push(bvh.right);
                stack.push(bvh.left);
            } else {
                stack.push(bvh.left);
                stack.push(bvh.right);
            }
        }
    }
    Some(best)
}

pub(super) fn query_point_mesh_bvh_global(
    mesh: &QueryMeshData,
    p_local: Vec3,
    instance_index: u32,
    global_from_mesh: Mat4,
    global_normal_basis: Mat3,
    mut best: Option<QueryHitCandidate>,
) -> Option<Option<QueryHitCandidate>> {
    let mut stack = QueryBvhStack::root();
    while let Some(node_idx) = stack.pop() {
        let bvh = *mesh.bvh_nodes.get(node_idx as usize)?;
        let node_d2 = aabb_distance2(p_local, bvh.aabb_min, bvh.aabb_max);
        if let Some(hit) = best
            && node_d2 >= hit.metric
        {
            continue;
        }
        if bvh.left == u32::MAX || bvh.right == u32::MAX {
            let start = bvh.tri_start as usize;
            let end = start + bvh.tri_count as usize;
            for &tri_idx in &mesh.bvh_tri_indices[start..end] {
                best = query_point_tri_global(
                    mesh,
                    tri_idx as usize,
                    p_local,
                    instance_index,
                    global_from_mesh,
                    global_normal_basis,
                    best,
                )?;
            }
        } else {
            let left = *mesh.bvh_nodes.get(bvh.left as usize)?;
            let right = *mesh.bvh_nodes.get(bvh.right as usize)?;
            let ld2 = aabb_distance2(p_local, left.aabb_min, left.aabb_max);
            let rd2 = aabb_distance2(p_local, right.aabb_min, right.aabb_max);
            if ld2 < rd2 {
                stack.push(bvh.right);
                stack.push(bvh.left);
            } else {
                stack.push(bvh.left);
                stack.push(bvh.right);
            }
        }
    }
    Some(best)
}

pub(super) fn query_ray_tri_local(
    mesh: &QueryMeshData,
    tri_idx: usize,
    ray_origin_local: Vec3,
    ray_dir_local: Vec3,
    max_t: f32,
    best: Option<QueryHitCandidate>,
) -> Option<Option<QueryHitCandidate>> {
    let tri = *mesh.triangles.get(tri_idx)?;
    let acc = *mesh.tri_accel.get(tri_idx)?;
    let limit = best.map_or(max_t, |h| h.metric.min(max_t));
    if ray_aabb_tmin(
        ray_origin_local,
        ray_dir_local,
        acc.aabb_min,
        acc.aabb_max,
        limit,
    )
    .is_none()
    {
        return Some(best);
    }
    let a = mesh.vertices[tri.a as usize];
    let b = mesh.vertices[tri.b as usize];
    let c = mesh.vertices[tri.c as usize];
    // A miss on THIS triangle keeps `best` and moves on. `?` here would fold
    // the miss into the outer `None`, which callers read as "abandon this
    // mesh/instance" -- one missed triangle used to silently drop every
    // triangle after it.
    let Some(t) = ray_intersect_triangle(ray_origin_local, ray_dir_local, a, b, c) else {
        return Some(best);
    };
    if t > max_t {
        return Some(best);
    }
    if let Some(hit) = best
        && t >= hit.metric
    {
        return Some(best);
    }
    let hit_local = ray_origin_local + ray_dir_local * t;
    let (barycentric, uv0, paint_uv) = hit_attrs(mesh, tri_idx, hit_local)?;
    Some(Some(QueryHitCandidate {
        instance_index: 0,
        surface_index: tri.surface_index,
        triangle_index: tri_idx as u32,
        barycentric,
        uv0,
        paint_uv,
        global_point: hit_local,
        local_point: hit_local,
        global_normal: acc.normal,
        local_normal: acc.normal,
        metric: t,
    }))
}

#[allow(clippy::too_many_arguments)]
pub(super) fn query_ray_tri_global(
    mesh: &QueryMeshData,
    tri_idx: usize,
    ray_origin_local: Vec3,
    ray_dir_local: Vec3,
    ray_origin_global: Vec3,
    max_t: f32,
    instance_index: u32,
    global_from_mesh: Mat4,
    global_normal_basis: Mat3,
    best: Option<QueryHitCandidate>,
) -> Option<Option<QueryHitCandidate>> {
    let tri = *mesh.triangles.get(tri_idx)?;
    let acc = *mesh.tri_accel.get(tri_idx)?;
    let limit = best.map_or(max_t, |h| h.metric.min(max_t));
    if ray_aabb_tmin(
        ray_origin_local,
        ray_dir_local,
        acc.aabb_min,
        acc.aabb_max,
        limit,
    )
    .is_none()
    {
        return Some(best);
    }
    let a = mesh.vertices[tri.a as usize];
    let b = mesh.vertices[tri.b as usize];
    let c = mesh.vertices[tri.c as usize];
    // Same as `query_ray_tri_local`: a triangle miss is not a scan abort.
    let Some(t) = ray_intersect_triangle(ray_origin_local, ray_dir_local, a, b, c) else {
        return Some(best);
    };
    if t > max_t {
        return Some(best);
    }
    let hit_local = ray_origin_local + ray_dir_local * t;
    let hit_global = global_from_mesh.transform_point3(hit_local);
    let global_t = (hit_global - ray_origin_global).length();
    if global_t > max_t {
        return Some(best);
    }
    if let Some(hit) = best
        && global_t >= hit.metric
    {
        return Some(best);
    }
    let global_normal = (global_normal_basis * acc.normal).normalize_or_zero();
    let (barycentric, uv0, paint_uv) = hit_attrs(mesh, tri_idx, hit_local)?;
    Some(Some(QueryHitCandidate {
        instance_index,
        surface_index: tri.surface_index,
        triangle_index: tri_idx as u32,
        barycentric,
        uv0,
        paint_uv,
        global_point: hit_global,
        local_point: hit_local,
        global_normal,
        local_normal: acc.normal,
        metric: global_t,
    }))
}

pub(super) fn query_ray_mesh_bvh(
    mesh: &QueryMeshData,
    ray_origin_local: Vec3,
    ray_dir_local: Vec3,
    max_t: f32,
    mut best: Option<QueryHitCandidate>,
) -> Option<Option<QueryHitCandidate>> {
    let mut stack = QueryBvhStack::root();
    while let Some(node_idx) = stack.pop() {
        let bvh = *mesh.bvh_nodes.get(node_idx as usize)?;
        let limit = best.map_or(max_t, |h| h.metric.min(max_t));
        if ray_aabb_tmin(
            ray_origin_local,
            ray_dir_local,
            bvh.aabb_min,
            bvh.aabb_max,
            limit,
        )
        .is_none()
        {
            continue;
        }
        if bvh.left == u32::MAX || bvh.right == u32::MAX {
            let start = bvh.tri_start as usize;
            let end = start + bvh.tri_count as usize;
            for &tri_idx in &mesh.bvh_tri_indices[start..end] {
                best = query_ray_tri_local(
                    mesh,
                    tri_idx as usize,
                    ray_origin_local,
                    ray_dir_local,
                    max_t,
                    best,
                )?;
            }
        } else {
            let limit = best.map_or(max_t, |h| h.metric.min(max_t));
            let left = *mesh.bvh_nodes.get(bvh.left as usize)?;
            let right = *mesh.bvh_nodes.get(bvh.right as usize)?;
            let lt = ray_aabb_tmin(
                ray_origin_local,
                ray_dir_local,
                left.aabb_min,
                left.aabb_max,
                limit,
            )
            .unwrap_or(f32::INFINITY);
            let rt = ray_aabb_tmin(
                ray_origin_local,
                ray_dir_local,
                right.aabb_min,
                right.aabb_max,
                limit,
            )
            .unwrap_or(f32::INFINITY);
            if lt < rt {
                stack.push(bvh.right);
                stack.push(bvh.left);
            } else {
                stack.push(bvh.left);
                stack.push(bvh.right);
            }
        }
    }
    Some(best)
}

#[allow(clippy::too_many_arguments)]
pub(super) fn query_ray_mesh_bvh_global(
    mesh: &QueryMeshData,
    ray_origin_local: Vec3,
    ray_dir_local: Vec3,
    ray_origin_global: Vec3,
    max_t: f32,
    instance_index: u32,
    global_from_mesh: Mat4,
    global_normal_basis: Mat3,
    mut best: Option<QueryHitCandidate>,
) -> Option<Option<QueryHitCandidate>> {
    let mut stack = QueryBvhStack::root();
    while let Some(node_idx) = stack.pop() {
        let bvh = *mesh.bvh_nodes.get(node_idx as usize)?;
        let limit = best.map_or(max_t, |h| h.metric.min(max_t));
        if ray_aabb_tmin(
            ray_origin_local,
            ray_dir_local,
            bvh.aabb_min,
            bvh.aabb_max,
            limit,
        )
        .is_none()
        {
            continue;
        }
        if bvh.left == u32::MAX || bvh.right == u32::MAX {
            let start = bvh.tri_start as usize;
            let end = start + bvh.tri_count as usize;
            for &tri_idx in &mesh.bvh_tri_indices[start..end] {
                best = query_ray_tri_global(
                    mesh,
                    tri_idx as usize,
                    ray_origin_local,
                    ray_dir_local,
                    ray_origin_global,
                    max_t,
                    instance_index,
                    global_from_mesh,
                    global_normal_basis,
                    best,
                )?;
            }
        } else {
            let limit = best.map_or(max_t, |h| h.metric.min(max_t));
            let left = *mesh.bvh_nodes.get(bvh.left as usize)?;
            let right = *mesh.bvh_nodes.get(bvh.right as usize)?;
            let lt = ray_aabb_tmin(
                ray_origin_local,
                ray_dir_local,
                left.aabb_min,
                left.aabb_max,
                limit,
            )
            .unwrap_or(f32::INFINITY);
            let rt = ray_aabb_tmin(
                ray_origin_local,
                ray_dir_local,
                right.aabb_min,
                right.aabb_max,
                limit,
            )
            .unwrap_or(f32::INFINITY);
            if lt < rt {
                stack.push(bvh.right);
                stack.push(bvh.left);
            } else {
                stack.push(bvh.left);
                stack.push(bvh.right);
            }
        }
    }
    Some(best)
}

impl QueryRegionAcc {
    pub(super) fn empty() -> Self {
        Self {
            tri_count: 0,
            sum_local: Vec3::ZERO,
            sum_global: Vec3::ZERO,
            local_min: Vec3::splat(f32::INFINITY),
            local_max: Vec3::splat(f32::NEG_INFINITY),
            global_min: Vec3::splat(f32::INFINITY),
            global_max: Vec3::splat(f32::NEG_INFINITY),
        }
    }
}

pub(super) fn nearer_hit(
    a: Option<QueryHitCandidate>,
    b: Option<QueryHitCandidate>,
) -> Option<QueryHitCandidate> {
    match (a, b) {
        (Some(left), Some(right)) => {
            if right.metric < left.metric {
                Some(right)
            } else {
                Some(left)
            }
        }
        (Some(hit), None) | (None, Some(hit)) => Some(hit),
        (None, None) => None,
    }
}

/// Order-independent form of [`nearer_hit`]: lexicographic min on
/// `(metric, instance_index)`.
///
/// The linear path folds instances in index order with [`nearer_hit`], which
/// keeps the accumulator on a metric tie — i.e. the LOWEST instance index wins
/// ties. The accelerated path visits instances in BVH/distance order, so it
/// must spell that tie-break out to return the same instance.
pub(super) fn nearer_hit_by_index(
    a: Option<QueryHitCandidate>,
    b: Option<QueryHitCandidate>,
) -> Option<QueryHitCandidate> {
    match (a, b) {
        (Some(left), Some(right)) => {
            let take_right = right.metric < left.metric
                || (right.metric == left.metric && right.instance_index < left.instance_index);
            Some(if take_right { right } else { left })
        }
        (Some(hit), None) | (None, Some(hit)) => Some(hit),
        (None, None) => None,
    }
}

pub(super) fn merge_region_acc(a: QueryRegionAcc, b: QueryRegionAcc) -> QueryRegionAcc {
    if a.tri_count == 0 {
        return b;
    }
    if b.tri_count == 0 {
        return a;
    }
    QueryRegionAcc {
        tri_count: a.tri_count.saturating_add(b.tri_count),
        sum_local: a.sum_local + b.sum_local,
        sum_global: a.sum_global + b.sum_global,
        local_min: a.local_min.min(b.local_min),
        local_max: a.local_max.max(b.local_max),
        global_min: a.global_min.min(b.global_min),
        global_max: a.global_max.max(b.global_max),
    }
}

#[inline]
pub(super) fn should_parallel_instances(instance_count: usize, tri_count: usize) -> bool {
    instance_count >= QUERY_INSTANCE_PAR_THRESHOLD
        && tri_count >= QUERY_TRI_PAR_THRESHOLD
        && instance_count.saturating_mul(tri_count) >= QUERY_PAR_WORK_THRESHOLD
}

#[inline]
pub(super) fn should_parallel_triangles(instance_parallel: bool, tri_count: usize) -> bool {
    !instance_parallel && tri_count >= QUERY_TRI_PAR_THRESHOLD
}

#[inline]
pub(super) fn should_parallel_regions(
    instance_count: usize,
    tri_count: usize,
    surface_count: usize,
) -> bool {
    if instance_count >= QUERY_INSTANCE_PAR_THRESHOLD && tri_count >= QUERY_TRI_PAR_THRESHOLD {
        return true;
    }
    let surface_gate = surface_count >= QUERY_REGION_SURFACE_PAR_THRESHOLD;
    surface_gate
        && tri_count >= QUERY_TRI_PAR_THRESHOLD
        && instance_count
            .saturating_mul(tri_count)
            .saturating_mul(surface_count)
            >= QUERY_PAR_WORK_THRESHOLD
}

#[inline]
pub(super) fn aabb_distance2(p: Vec3, min: Vec3, max: Vec3) -> f32 {
    simd::aabb_distance2(p, min, max)
}

#[inline]
pub(super) fn ray_aabb_tmin(
    origin: Vec3,
    dir: Vec3,
    min: Vec3,
    max: Vec3,
    max_t: f32,
) -> Option<f32> {
    simd::ray_aabb_tmin(origin, dir, min, max, max_t)
}

pub(super) fn build_query_mesh_data(
    vertices: Vec<Vec3>,
    uv0: Vec<Vec2>,
    paint_uv: Vec<Vec2>,
    triangles: Vec<QueryTri>,
) -> Option<QueryMeshData> {
    let len = vertices.len();
    build_query_mesh_data_with_skin(
        vertices,
        uv0,
        paint_uv,
        vec![[0; 4]; len],
        vec![[0.0; 4]; len],
        triangles,
    )
}

pub(super) fn skin_query_mesh_with_palette(
    mesh: &QueryMeshData,
    palette: &[Mat4],
) -> Option<QueryMeshData> {
    let vertices = (0..mesh.vertices.len())
        .map(|index| skin_query_vertex_with_palette(mesh, index, palette))
        .collect::<Option<Vec<_>>>()?;
    build_query_mesh_data_with_skin(
        vertices,
        mesh.uv0.clone(),
        mesh.paint_uv.clone(),
        mesh.joints.clone(),
        mesh.weights.clone(),
        mesh.triangles.clone(),
    )
}

pub(super) fn skin_query_vertex_with_palette(
    mesh: &QueryMeshData,
    index: usize,
    palette: &[Mat4],
) -> Option<Vec3> {
    let position = *mesh.vertices.get(index)?;
    let joints = *mesh.joints.get(index)?;
    let weights = *mesh.weights.get(index)?;
    let mut posed = Vec3::ZERO;
    let mut total = 0.0;
    for lane in 0..4 {
        let weight = weights[lane];
        if weight > 0.0
            && let Some(matrix) = palette.get(joints[lane] as usize)
        {
            posed += matrix.transform_point3(position) * weight;
            total += weight;
        }
    }
    Some(if total > 0.0 { posed } else { position })
}

pub(super) fn build_query_mesh_data_with_skin(
    vertices: Vec<Vec3>,
    uv0: Vec<Vec2>,
    paint_uv: Vec<Vec2>,
    joints: Vec<[u16; 4]>,
    weights: Vec<[f32; 4]>,
    triangles: Vec<QueryTri>,
) -> Option<QueryMeshData> {
    if vertices.is_empty()
        || triangles.is_empty()
        || uv0.len() != vertices.len()
        || paint_uv.len() != vertices.len()
        || joints.len() != vertices.len()
        || weights.len() != vertices.len()
    {
        return None;
    }
    let mut tri_accel = Vec::with_capacity(triangles.len());
    for tri in &triangles {
        let a = *vertices.get(tri.a as usize)?;
        let b = *vertices.get(tri.b as usize)?;
        let c = *vertices.get(tri.c as usize)?;
        let normal = (b - a).cross(c - a).normalize_or_zero();
        let aabb_min = a.min(b).min(c);
        let aabb_max = a.max(b).max(c);
        tri_accel.push(QueryTriAccel {
            normal,
            aabb_min,
            aabb_max,
            centroid: (a + b + c) * (1.0 / 3.0),
        });
    }
    let mut bvh_tri_indices: Vec<u32> = (0..triangles.len() as u32).collect();
    let mut bvh_nodes = Vec::new();
    build_bvh_recursive(
        &tri_accel,
        &mut bvh_tri_indices,
        &mut bvh_nodes,
        0,
        triangles.len(),
    );
    Some(QueryMeshData {
        vertices,
        uv0,
        paint_uv,
        joints,
        weights,
        triangles,
        tri_accel,
        bvh_nodes,
        bvh_tri_indices,
    })
}

pub(super) fn build_bvh_recursive(
    tri_accel: &[QueryTriAccel],
    tri_indices: &mut [u32],
    nodes: &mut Vec<QueryBvhNode>,
    start: usize,
    count: usize,
) -> u32 {
    let node_index = nodes.len() as u32;
    nodes.push(QueryBvhNode {
        aabb_min: Vec3::splat(f32::INFINITY),
        aabb_max: Vec3::splat(f32::NEG_INFINITY),
        left: u32::MAX,
        right: u32::MAX,
        tri_start: start as u32,
        tri_count: count as u32,
    });
    let mut node_min = Vec3::splat(f32::INFINITY);
    let mut node_max = Vec3::splat(f32::NEG_INFINITY);
    let mut cmin = Vec3::splat(f32::INFINITY);
    let mut cmax = Vec3::splat(f32::NEG_INFINITY);
    for &idx in &tri_indices[start..start + count] {
        let acc = tri_accel[idx as usize];
        node_min = node_min.min(acc.aabb_min);
        node_max = node_max.max(acc.aabb_max);
        cmin = cmin.min(acc.centroid);
        cmax = cmax.max(acc.centroid);
    }
    if count <= 12 {
        nodes[node_index as usize].aabb_min = node_min;
        nodes[node_index as usize].aabb_max = node_max;
        return node_index;
    }
    let extent = cmax - cmin;
    let axis = if extent.x >= extent.y && extent.x >= extent.z {
        0
    } else if extent.y >= extent.z {
        1
    } else {
        2
    };
    tri_indices[start..start + count].sort_unstable_by(|a, b| {
        let ca = tri_accel[*a as usize].centroid;
        let cb = tri_accel[*b as usize].centroid;
        let va = if axis == 0 {
            ca.x
        } else if axis == 1 {
            ca.y
        } else {
            ca.z
        };
        let vb = if axis == 0 {
            cb.x
        } else if axis == 1 {
            cb.y
        } else {
            cb.z
        };
        va.partial_cmp(&vb).unwrap_or(std::cmp::Ordering::Equal)
    });
    let left_count = count / 2;
    let right_count = count - left_count;
    let left = build_bvh_recursive(tri_accel, tri_indices, nodes, start, left_count);
    let right = build_bvh_recursive(
        tri_accel,
        tri_indices,
        nodes,
        start + left_count,
        right_count,
    );
    nodes[node_index as usize] = QueryBvhNode {
        aabb_min: node_min,
        aabb_max: node_max,
        left,
        right,
        tri_start: start as u32,
        tri_count: count as u32,
    };
    node_index
}
