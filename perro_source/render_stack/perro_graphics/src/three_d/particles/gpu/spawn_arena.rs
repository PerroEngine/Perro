use super::*;

/// Free space share that triggers a compaction pass, as a fraction of arena
/// length (free * 2 > len == more than half the arena is holes).
const SPAWN_COMPACT_MIN_LEN: u32 = 1024;

/// One contiguous slice of the shared spawn-origin/rotation arena. One live
/// emitter owns exactly one region, sized to its alive budget.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub(super) struct SpawnRegion {
    pub(super) base: u32,
    pub(super) capacity: u32,
}

impl SpawnRegion {
    #[inline]
    fn end(&self) -> u32 {
        self.base.saturating_add(self.capacity)
    }
}

/// Data move produced by [`SpawnArena::compact`]: copy `capacity` slots from
/// `from` down to `to`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) struct SpawnMove {
    pub(super) from: u32,
    pub(super) to: u32,
    pub(super) capacity: u32,
}

/// Free-list allocator over the shared spawn-origin/rotation arrays.
///
/// The arrays used to be a pure bump allocator: every emitter appended a
/// region and nothing ever came back, so a dead node or a changed alive budget
/// orphaned its slots forever and the GPU buffers (sized from array length)
/// only ever grew. Regions now return to a free list on node death and on
/// budget change, get reused first-fit, and a compaction pass repacks the
/// arena once holes take over.
#[derive(Default)]
pub(super) struct SpawnArena {
    /// Arena tail; the origin/rotation arrays are kept at exactly this length.
    len: u32,
    /// Free regions, sorted by base, coalesced (no two touch).
    free: Vec<SpawnRegion>,
    free_slots: u32,
}

impl SpawnArena {
    #[inline]
    pub(super) fn len(&self) -> u32 {
        self.len
    }

    #[cfg(test)]
    #[inline]
    fn free_slots(&self) -> u32 {
        self.free_slots
    }

    /// First-fit over the free list, splitting the remainder back; grow the
    /// tail only when nothing fits.
    pub(super) fn alloc(&mut self, capacity: u32) -> SpawnRegion {
        let capacity = capacity.max(1);
        if let Some(i) = self.free.iter().position(|r| r.capacity >= capacity) {
            let region = self.free[i];
            if region.capacity == capacity {
                self.free.remove(i);
            } else {
                self.free[i] = SpawnRegion {
                    base: region.base + capacity,
                    capacity: region.capacity - capacity,
                };
            }
            self.free_slots -= capacity;
            return SpawnRegion {
                base: region.base,
                capacity,
            };
        }
        let base = self.len;
        self.len = self.len.saturating_add(capacity);
        SpawnRegion { base, capacity }
    }

    /// Hand a region back. Coalesces with neighbours and drops the tail so the
    /// arrays shrink without waiting for a compaction.
    pub(super) fn release(&mut self, region: SpawnRegion) {
        if region.capacity == 0 {
            return;
        }
        self.free_slots += region.capacity;
        let idx = self.free.partition_point(|r| r.base < region.base);
        self.free.insert(idx, region);
        if idx + 1 < self.free.len() && self.free[idx].end() == self.free[idx + 1].base {
            self.free[idx].capacity += self.free[idx + 1].capacity;
            self.free.remove(idx + 1);
        }
        if idx > 0 && self.free[idx - 1].end() == self.free[idx].base {
            self.free[idx - 1].capacity += self.free[idx].capacity;
            self.free.remove(idx);
        }
        self.trim_tail();
    }

    fn trim_tail(&mut self) {
        while let Some(last) = self.free.last().copied() {
            if last.end() != self.len {
                break;
            }
            self.len = last.base;
            self.free_slots -= last.capacity;
            self.free.pop();
        }
    }

    /// True once holes take over more than half of a non-trivial arena.
    pub(super) fn should_compact(&self) -> bool {
        self.len >= SPAWN_COMPACT_MIN_LEN && self.free_slots > self.len / 2
    }

    /// Repack `live` (sorted by base) to the front of the arena. Rewrites each
    /// region's base in place and appends the data moves needed to match.
    pub(super) fn compact(&mut self, live: &mut [SpawnRegion], moves: &mut Vec<SpawnMove>) {
        debug_assert!(live.windows(2).all(|w| w[0].base <= w[1].base));
        moves.clear();
        let mut next_base = 0u32;
        for region in live.iter_mut() {
            if region.base != next_base {
                moves.push(SpawnMove {
                    from: region.base,
                    to: next_base,
                    capacity: region.capacity,
                });
                region.base = next_base;
            }
            next_base += region.capacity;
        }
        self.len = next_base;
        self.free.clear();
        self.free_slots = 0;
    }
}

/// Per-emitter spawn-origin update for one prepare.
pub(super) struct SpawnRingUpdate {
    pub(super) emit_count: u32,
    pub(super) total_spawned: u32,
    pub(super) looping: bool,
    pub(super) origin: [f32; 3],
    pub(super) rotation: [f32; 4],
}

/// Resolve (allocating or re-allocating on budget change) this node's spawn
/// region, stamp it live for `generation`, and refresh the slots this frame's
/// spawn indices land on. Returns the region base for the emitter record.
#[allow(clippy::too_many_arguments)]
pub(super) fn update_spawn_ring(
    rings: &mut AHashMap<NodeID, SpawnRingState>,
    arena: &mut SpawnArena,
    origins: &mut Vec<[f32; 4]>,
    rotations: &mut Vec<[f32; 4]>,
    origin_dirty: &mut Vec<u32>,
    rotation_dirty: &mut Vec<u32>,
    node: NodeID,
    generation: u64,
    capacity: u32,
    update: SpawnRingUpdate,
) -> u32 {
    let capacity = capacity.max(1);
    let entry = rings.entry(node).or_insert_with(|| SpawnRingState {
        region: SpawnRegion::default(),
        slot_spawn_keys: Vec::new(),
        last_seen_generation: generation,
    });
    entry.last_seen_generation = generation;
    if entry.region.capacity != capacity {
        // Budget change: the old region is dead weight, hand it back before
        // taking a new one so the arena can reuse the hole.
        arena.release(entry.region);
        entry.region = arena.alloc(capacity);
        entry.slot_spawn_keys.clear();
        entry.slot_spawn_keys.resize(capacity as usize, u32::MAX);
        grow_spawn_arrays(origins, rotations, arena.len());
    }
    let base = entry.region.base;
    for i in 0..update.emit_count {
        let spawn_index = if update.looping {
            let back = update.emit_count.saturating_sub(1).saturating_sub(i);
            update.total_spawned.saturating_sub(back)
        } else {
            i
        };
        let slot = spawn_index % capacity;
        let slot_idx = slot as usize;
        if entry.slot_spawn_keys[slot_idx] == spawn_index {
            continue;
        }
        entry.slot_spawn_keys[slot_idx] = spawn_index;
        let arena_slot = base + slot;
        origins[arena_slot as usize] = [update.origin[0], update.origin[1], update.origin[2], 0.0];
        rotations[arena_slot as usize] = update.rotation;
        origin_dirty.push(arena_slot);
        rotation_dirty.push(arena_slot);
    }
    base
}

/// Release the regions of nodes that did not report in for `generation`, then
/// repack when the arena is mostly holes. Returns true when slots moved, i.e.
/// the whole origin/rotation range needs a fresh upload.
pub(super) fn sweep_spawn_rings(
    rings: &mut AHashMap<NodeID, SpawnRingState>,
    arena: &mut SpawnArena,
    origins: &mut Vec<[f32; 4]>,
    rotations: &mut Vec<[f32; 4]>,
    live_scratch: &mut Vec<(NodeID, SpawnRegion)>,
    move_scratch: &mut Vec<SpawnMove>,
    generation: u64,
) -> bool {
    rings.retain(|_, ring| {
        if ring.last_seen_generation == generation {
            return true;
        }
        arena.release(ring.region);
        false
    });
    let compacted = if arena.should_compact() {
        compact_spawn_rings(rings, arena, origins, rotations, live_scratch, move_scratch)
    } else {
        false
    };
    // Keep the arrays exactly arena-sized: the shrink trackers are fed this
    // length, so holes left behind would pin the GPU buffers forever.
    let len = arena.len() as usize;
    origins.truncate(len);
    rotations.truncate(len);
    grow_spawn_arrays(origins, rotations, arena.len());
    compacted
}

fn compact_spawn_rings(
    rings: &mut AHashMap<NodeID, SpawnRingState>,
    arena: &mut SpawnArena,
    origins: &mut [[f32; 4]],
    rotations: &mut [[f32; 4]],
    live_scratch: &mut Vec<(NodeID, SpawnRegion)>,
    move_scratch: &mut Vec<SpawnMove>,
) -> bool {
    live_scratch.clear();
    live_scratch.extend(rings.iter().map(|(node, ring)| (*node, ring.region)));
    live_scratch.sort_unstable_by_key(|(_, region)| region.base);
    let mut regions: Vec<SpawnRegion> = live_scratch.iter().map(|(_, r)| *r).collect();
    arena.compact(&mut regions, move_scratch);
    if move_scratch.is_empty() {
        return false;
    }
    for mv in move_scratch.iter() {
        let from = mv.from as usize;
        let to = mv.to as usize;
        let len = mv.capacity as usize;
        origins.copy_within(from..from + len, to);
        rotations.copy_within(from..from + len, to);
    }
    for ((node, _), region) in live_scratch.iter().zip(regions.iter()) {
        if let Some(ring) = rings.get_mut(node) {
            ring.region = *region;
        }
    }
    true
}

#[inline]
fn grow_spawn_arrays(origins: &mut Vec<[f32; 4]>, rotations: &mut Vec<[f32; 4]>, len: u32) {
    let len = len as usize;
    if origins.len() < len {
        origins.resize(len, [0.0; 4]);
    }
    if rotations.len() < len {
        rotations.resize(len, [0.0, 0.0, 0.0, 1.0]);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn region(base: u32, capacity: u32) -> SpawnRegion {
        SpawnRegion { base, capacity }
    }

    #[test]
    fn alloc_bumps_tail_when_free_list_empty() {
        let mut arena = SpawnArena::default();
        assert_eq!(arena.alloc(4), region(0, 4));
        assert_eq!(arena.alloc(2), region(4, 2));
        assert_eq!(arena.len(), 6);
        assert_eq!(arena.free_slots(), 0);
    }

    #[test]
    fn release_of_interior_region_feeds_reuse() {
        let mut arena = SpawnArena::default();
        let a = arena.alloc(4);
        let _b = arena.alloc(4);
        arena.release(a);
        assert_eq!(arena.len(), 8);
        assert_eq!(arena.free_slots(), 4);
        // exact fit reuses the hole instead of growing
        assert_eq!(arena.alloc(4), region(0, 4));
        assert_eq!(arena.len(), 8);
        assert_eq!(arena.free_slots(), 0);
    }

    #[test]
    fn release_at_tail_shrinks_arena() {
        let mut arena = SpawnArena::default();
        let a = arena.alloc(4);
        let b = arena.alloc(4);
        arena.release(b);
        assert_eq!(arena.len(), 4);
        arena.release(a);
        assert_eq!(arena.len(), 0);
        assert_eq!(arena.free_slots(), 0);
    }

    #[test]
    fn release_coalesces_neighbours() {
        let mut arena = SpawnArena::default();
        let a = arena.alloc(4);
        let b = arena.alloc(4);
        let _c = arena.alloc(4);
        arena.release(a);
        arena.release(b);
        // merged hole of 8 serves an 8-slot request without growing
        assert_eq!(arena.alloc(8), region(0, 8));
        assert_eq!(arena.len(), 12);
    }

    #[test]
    fn alloc_splits_larger_free_region() {
        let mut arena = SpawnArena::default();
        let a = arena.alloc(8);
        let _b = arena.alloc(2);
        arena.release(a);
        assert_eq!(arena.alloc(3), region(0, 3));
        assert_eq!(arena.free_slots(), 5);
        assert_eq!(arena.alloc(5), region(3, 5));
        assert_eq!(arena.free_slots(), 0);
        assert_eq!(arena.len(), 10);
    }

    #[test]
    fn budget_change_reuses_freed_region_rather_than_appending() {
        let mut arena = SpawnArena::default();
        let old = arena.alloc(16);
        let _other = arena.alloc(4);
        // budget shrinks: old region back, new one carved out of the hole
        arena.release(old);
        let new = arena.alloc(8);
        assert_eq!(new, region(0, 8));
        assert_eq!(arena.len(), 20);
        // budget grows past the hole: tail growth, hole stays reusable
        arena.release(new);
        let grown = arena.alloc(24);
        assert_eq!(grown, region(20, 24));
        assert_eq!(arena.free_slots(), 16);
    }

    #[test]
    fn should_compact_only_when_mostly_holes() {
        let mut arena = SpawnArena::default();
        let mut regions = Vec::new();
        for _ in 0..16 {
            regions.push(arena.alloc(128));
        }
        assert_eq!(arena.len(), 2048);
        assert!(!arena.should_compact());
        // free every other region: half holes, tail still live
        for r in regions.iter().step_by(2) {
            arena.release(*r);
        }
        assert_eq!(arena.free_slots(), 1024);
        assert!(!arena.should_compact());
        arena.release(regions[3]);
        assert!(arena.should_compact());
    }

    #[test]
    fn compact_rebases_regions_and_reports_moves() {
        let mut arena = SpawnArena::default();
        let a = arena.alloc(2);
        let b = arena.alloc(3);
        let c = arena.alloc(2);
        arena.release(a);
        arena.release(b);
        let mut live = vec![c];
        let mut moves = Vec::new();
        arena.compact(&mut live, &mut moves);
        assert_eq!(live, vec![region(0, 2)]);
        assert_eq!(
            moves,
            vec![SpawnMove {
                from: 5,
                to: 0,
                capacity: 2,
            }]
        );
        assert_eq!(arena.len(), 2);
        assert_eq!(arena.free_slots(), 0);
        // arena is dense again: next alloc goes straight to the tail
        assert_eq!(arena.alloc(1), region(2, 1));
    }

    #[test]
    fn compact_keeps_leading_region_in_place() {
        let mut arena = SpawnArena::default();
        let a = arena.alloc(2);
        let b = arena.alloc(3);
        let c = arena.alloc(2);
        arena.release(b);
        let mut live = vec![a, c];
        let mut moves = Vec::new();
        arena.compact(&mut live, &mut moves);
        assert_eq!(live, vec![region(0, 2), region(2, 2)]);
        assert_eq!(
            moves,
            vec![SpawnMove {
                from: 5,
                to: 2,
                capacity: 2,
            }]
        );
        assert_eq!(arena.len(), 4);
    }

    /// Bundles the state `update_spawn_ring` / `sweep_spawn_rings` thread
    /// through, so the ring-level tests read like a sequence of prepares.
    #[derive(Default)]
    struct RingFixture {
        rings: AHashMap<NodeID, SpawnRingState>,
        arena: SpawnArena,
        origins: Vec<[f32; 4]>,
        rotations: Vec<[f32; 4]>,
        origin_dirty: Vec<u32>,
        rotation_dirty: Vec<u32>,
        live_scratch: Vec<(NodeID, SpawnRegion)>,
        move_scratch: Vec<SpawnMove>,
    }

    impl RingFixture {
        fn push(
            &mut self,
            node: NodeID,
            generation: u64,
            capacity: u32,
            update: SpawnRingUpdate,
        ) -> u32 {
            update_spawn_ring(
                &mut self.rings,
                &mut self.arena,
                &mut self.origins,
                &mut self.rotations,
                &mut self.origin_dirty,
                &mut self.rotation_dirty,
                node,
                generation,
                capacity,
                update,
            )
        }

        fn sweep(&mut self, generation: u64) -> bool {
            sweep_spawn_rings(
                &mut self.rings,
                &mut self.arena,
                &mut self.origins,
                &mut self.rotations,
                &mut self.live_scratch,
                &mut self.move_scratch,
                generation,
            )
        }
    }

    fn burst(emit_count: u32, x: f32) -> SpawnRingUpdate {
        SpawnRingUpdate {
            emit_count,
            total_spawned: emit_count.saturating_sub(1),
            looping: false,
            origin: [x, 0.0, 0.0],
            rotation: [0.0, 0.0, 0.0, 1.0],
        }
    }

    #[test]
    fn sweep_releases_dead_rings_and_shrinks_arrays() {
        let mut fixture = RingFixture::default();
        let alive = NodeID::from_u64(1);
        let doomed = NodeID::from_u64(2);
        fixture.push(alive, 1, 4, burst(4, 1.0));
        fixture.push(doomed, 1, 4, burst(4, 2.0));
        assert_eq!(fixture.arena.len(), 8);
        assert_eq!(fixture.origins.len(), 8);
        assert_eq!(fixture.origin_dirty.len(), 8);

        // second prepare: only `alive` reports in
        fixture.push(alive, 2, 4, burst(4, 1.0));
        assert!(!fixture.sweep(2));
        assert_eq!(fixture.rings.len(), 1);
        assert_eq!(fixture.arena.len(), 4);
        assert_eq!(fixture.origins.len(), 4);
        assert_eq!(fixture.rotations.len(), 4);
    }

    #[test]
    fn sweep_compacts_and_rebases_surviving_ring_data() {
        let mut fixture = RingFixture::default();
        let dead = NodeID::from_u64(1);
        let alive = NodeID::from_u64(2);
        fixture.push(dead, 1, 1024, burst(1, 10.0));
        let alive_base = fixture.push(alive, 1, 512, burst(1, 20.0));
        assert_eq!(alive_base, 1024);
        assert_eq!(fixture.origins[1024][0], 20.0);

        // only `alive` reports in for generation 2 -> two thirds of the arena
        // frees, which trips compaction and slides the survivor down to base 0
        fixture
            .rings
            .get_mut(&alive)
            .expect("alive ring")
            .last_seen_generation = 2;
        assert!(fixture.sweep(2));
        assert_eq!(fixture.arena.len(), 512);
        assert_eq!(fixture.origins.len(), 512);
        assert_eq!(fixture.rotations.len(), 512);
        assert_eq!(fixture.rings[&alive].region, region(0, 512));
        assert_eq!(fixture.origins[0][0], 20.0);
    }

    #[test]
    fn budget_change_frees_old_region_instead_of_orphaning_it() {
        let mut fixture = RingFixture::default();
        let node = NodeID::from_u64(7);
        assert_eq!(fixture.push(node, 1, 8, burst(1, 0.0)), 0);
        assert_eq!(fixture.arena.len(), 8);
        // budget grows: the old 8-slot region returns and, being the tail, the
        // arena rewinds instead of appending a second region
        assert_eq!(fixture.push(node, 2, 16, burst(1, 0.0)), 0);
        assert_eq!(fixture.arena.len(), 16);
        assert_eq!(fixture.arena.free_slots(), 0);
        // budget shrinks again: reuse, no growth
        assert_eq!(fixture.push(node, 3, 4, burst(1, 0.0)), 0);
        assert_eq!(fixture.arena.len(), 4);
        // arrays only grow inside a prepare; the sweep trims them to arena len
        assert_eq!(fixture.origins.len(), 16);
        assert!(!fixture.sweep(3));
        assert_eq!(fixture.origins.len(), 4);
    }

    #[test]
    fn budget_change_reuses_hole_left_by_a_dead_neighbour() {
        let mut fixture = RingFixture::default();
        let dead = NodeID::from_u64(1);
        let grower = NodeID::from_u64(2);
        let tail = NodeID::from_u64(3);
        fixture.push(dead, 1, 16, burst(1, 1.0));
        assert_eq!(fixture.push(grower, 1, 4, burst(1, 2.0)), 16);
        fixture.push(tail, 1, 4, burst(1, 3.0));
        assert_eq!(fixture.arena.len(), 24);

        // `dead` stops reporting: its 16 slots go back on the free list
        fixture.push(grower, 2, 4, burst(1, 2.0));
        fixture.push(tail, 2, 4, burst(1, 3.0));
        assert!(!fixture.sweep(2));
        assert_eq!(fixture.arena.len(), 24);
        assert_eq!(fixture.arena.free_slots(), 16);

        // `grower` doubles its budget -> the hole serves the new region and the
        // arena stops growing where the old bump allocator would have appended
        fixture.push(grower, 3, 8, burst(1, 2.0));
        fixture.push(tail, 3, 4, burst(1, 3.0));
        assert_eq!(fixture.rings[&grower].region, region(0, 8));
        assert_eq!(fixture.arena.len(), 24);
        assert!(!fixture.sweep(3));
        assert_eq!(fixture.origins.len(), 24);
    }

    #[test]
    fn ring_slots_refresh_only_on_spawn_key_change() {
        let mut fixture = RingFixture::default();
        let node = NodeID::from_u64(3);
        let update = || SpawnRingUpdate {
            emit_count: 4,
            total_spawned: 3,
            looping: true,
            origin: [5.0, 6.0, 7.0],
            rotation: [0.0, 0.0, 0.0, 1.0],
        };
        fixture.push(node, 1, 4, update());
        assert_eq!(fixture.origin_dirty.len(), 4);
        assert_eq!(fixture.rotation_dirty.len(), 4);
        assert_eq!(fixture.origins[0], [5.0, 6.0, 7.0, 0.0]);
        fixture.origin_dirty.clear();
        fixture.rotation_dirty.clear();
        // same spawn keys next prepare -> no re-upload
        fixture.push(node, 2, 4, update());
        assert!(fixture.origin_dirty.is_empty());
        assert!(fixture.rotation_dirty.is_empty());
    }
}
