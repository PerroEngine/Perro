use super::*;

/// Keep stable, lowest-free handles and an ordered snapshot for socket polls.
/// Membership changes invalidate the snapshot; value changes do not.
pub(crate) struct Slots<T> {
    values: Vec<Option<T>>,
    live: Vec<u64>,
    live_count: usize,
    first_free: usize,
    snapshot: Option<Arc<[u32]>>,
}

impl<T> Default for Slots<T> {
    fn default() -> Self {
        Self {
            values: Vec::new(),
            live: Vec::new(),
            live_count: 0,
            first_free: 0,
            snapshot: None,
        }
    }
}

impl<T> Slots<T> {
    pub(crate) fn get(&self, id: u32) -> Option<&T> {
        self.values.get(id as usize).and_then(Option::as_ref)
    }

    pub(crate) fn get_mut(&mut self, id: u32) -> Option<&mut T> {
        self.values.get_mut(id as usize).and_then(Option::as_mut)
    }

    /// Reconnect may restore a removed handle, but may not extend the table.
    pub(crate) fn replace(&mut self, id: u32, value: T) -> bool {
        let Some(slot) = self.values.get_mut(id as usize) else {
            return false;
        };
        if slot.replace(value).is_none() {
            self.mark_live(id as usize);
            self.snapshot = None;
        }
        true
    }

    pub(crate) fn live_ids(&mut self) -> Option<Arc<[u32]>> {
        if self.live_count == 0 {
            return None;
        }
        let snapshot = self.snapshot.get_or_insert_with(|| {
            let mut ids = Vec::with_capacity(self.live_count);
            for (word_index, &word) in self.live.iter().enumerate() {
                let mut remaining = word;
                while remaining != 0 {
                    ids.push((word_index * 64 + remaining.trailing_zeros() as usize) as u32);
                    remaining &= remaining - 1;
                }
            }
            ids.into()
        });
        Some(Arc::clone(snapshot))
    }

    fn mark_live(&mut self, index: usize) {
        self.live[index / 64] |= 1 << (index % 64);
        self.live_count += 1;
        // Filling the sole hole needs no scan, even in a large table.
        if self.live_count == self.values.len() {
            self.first_free = self.values.len();
        } else if index == self.first_free {
            // All earlier slots are occupied. Scan 64 slots per word.
            self.first_free = self.live[index / 64..]
                .iter()
                .position(|&word| word != u64::MAX)
                .map_or(self.values.len(), |offset| {
                    let word_index = index / 64 + offset;
                    word_index * 64 + (!self.live[word_index]).trailing_zeros() as usize
                });
        }
    }
}

pub(crate) fn insert_slot<T>(slots: &mut Slots<T>, value: T) -> u32 {
    let index = slots.first_free;
    if index < slots.values.len() {
        slots.values[index] = Some(value);
    } else {
        slots.values.push(Some(value));
        if index.is_multiple_of(64) {
            slots.live.push(0);
        }
    }
    slots.mark_live(index);
    slots.snapshot = None;
    index as u32
}

pub(crate) fn remove_slot<T>(slots: &mut Slots<T>, id: u32) -> bool {
    let Some(slot) = slots.values.get_mut(id as usize) else {
        return false;
    };
    if slot.take().is_none() {
        return false;
    }
    let index = id as usize;
    slots.live[index / 64] &= !(1 << (index % 64));
    slots.live_count -= 1;
    slots.first_free = slots.first_free.min(index);
    slots.snapshot = None;
    true
}

pub(crate) fn get_slot<'a, T>(slots: &'a Slots<T>, id: u32, label: &str) -> NetResult<&'a T> {
    slots
        .get(id)
        .ok_or_else(|| NetError::new(NetErrorKind::MissingHandle, format!("missing {label} {id}")))
}

pub(crate) fn get_slot_mut<'a, T>(
    slots: &'a mut Slots<T>,
    id: u32,
    label: &str,
) -> NetResult<&'a mut T> {
    slots
        .get_mut(id)
        .ok_or_else(|| NetError::new(NetErrorKind::MissingHandle, format!("missing {label} {id}")))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn slots_reuse_lowest_free_id_and_keep_held_snapshots() {
        let mut slots = Slots::default();
        for value in 0..6 {
            assert_eq!(insert_slot(&mut slots, value), value);
        }
        let original = slots.live_ids().expect("live slots");
        assert_eq!(&*original, &[0, 1, 2, 3, 4, 5]);
        assert!(Arc::ptr_eq(
            &original,
            &slots.live_ids().expect("cached slots")
        ));
        assert!(remove_slot(&mut slots, 4));
        assert!(remove_slot(&mut slots, 1));
        assert!(!remove_slot(&mut slots, 1));
        assert!(!remove_slot(&mut slots, 100));
        assert_eq!(&*slots.live_ids().expect("sparse slots"), &[0, 2, 3, 5]);
        assert_eq!(insert_slot(&mut slots, 10), 1);
        assert_eq!(insert_slot(&mut slots, 40), 4);
        assert_eq!(insert_slot(&mut slots, 60), 6);
        assert_eq!(*get_slot(&slots, 1, "slot").expect("slot 1"), 10);
        assert_eq!(&*original, &[0, 1, 2, 3, 4, 5]);
    }

    #[test]
    fn replacement_reactivates_removed_id_without_free_list_aliasing() {
        let mut slots = Slots::default();
        assert_eq!(insert_slot(&mut slots, 0), 0);
        assert_eq!(insert_slot(&mut slots, 1), 1);
        assert!(remove_slot(&mut slots, 0));
        assert!(slots.replace(0, 10));
        assert_eq!(insert_slot(&mut slots, 2), 2);
        assert!(!slots.replace(3, 30));
        let ids = slots.live_ids().expect("live slots");
        assert!(slots.replace(1, 11));
        *get_slot_mut(&mut slots, 2, "slot").expect("slot 2") = 22;
        assert!(Arc::ptr_eq(
            &ids,
            &slots.live_ids().expect("same membership")
        ));
        assert_eq!(&*ids, &[0, 1, 2]);
        for id in 0..3 {
            assert!(remove_slot(&mut slots, id));
        }
        assert!(slots.live_ids().is_none());
        assert_eq!(insert_slot(&mut slots, 100), 0);
        assert_eq!(&*slots.live_ids().expect("restored slot"), &[0]);
    }

    #[test]
    fn slots_cross_bitset_words_in_lowest_free_order() {
        let mut slots = Slots::default();
        for id in 0..193 {
            assert_eq!(insert_slot(&mut slots, id), id);
        }
        let original = slots.live_ids().expect("full table");
        for id in [192, 128, 127, 64, 63, 0] {
            assert!(remove_slot(&mut slots, id));
        }
        assert!(slots.replace(64, 640));
        assert!(slots.replace(0, 1000));
        for id in [63, 127, 128, 192] {
            assert_eq!(insert_slot(&mut slots, id + 1000), id);
        }
        assert_eq!(insert_slot(&mut slots, 193), 193);
        assert_eq!(&*original, &(0..193).collect::<Vec<_>>());
        assert_eq!(
            &*slots.live_ids().expect("refilled table"),
            &(0..194).collect::<Vec<_>>()
        );

        for id in 0..194 {
            assert!(remove_slot(&mut slots, id));
        }
        assert!(slots.live_ids().is_none());
        assert!(slots.replace(192, 1920));
        assert_eq!(&*slots.live_ids().expect("reconnected slot"), &[192]);
        for id in 0..192 {
            assert_eq!(insert_slot(&mut slots, id), id);
        }
        assert_eq!(insert_slot(&mut slots, 1930), 193);
        assert_eq!(insert_slot(&mut slots, 1940), 194);
    }

    #[test]
    fn slot_membership_matches_reference_across_mixed_edits() {
        fn assert_send_sync<T: Send + Sync>() {}
        assert_send_sync::<Slots<u32>>();

        let mut slots = Slots::default();
        let mut reference = Vec::new();
        for id in 0..256 {
            insert_slot(&mut slots, id);
            reference.push(Some(id));
        }
        let mut seed = 0x1234_5678u32;
        for value in 0..2000 {
            seed = seed.wrapping_mul(1_664_525).wrapping_add(1_013_904_223);
            let id = (seed >> 16) as usize % (reference.len() + 8);
            match seed % 3 {
                0 => {
                    let expected = reference
                        .get_mut(id)
                        .is_some_and(|slot| slot.take().is_some());
                    assert_eq!(remove_slot(&mut slots, id as u32), expected);
                }
                1 => {
                    assert_eq!(slots.replace(id as u32, value), id < reference.len());
                    if let Some(slot) = reference.get_mut(id) {
                        *slot = Some(value);
                    }
                }
                _ => {
                    let expected = reference
                        .iter()
                        .position(Option::is_none)
                        .unwrap_or(reference.len());
                    assert_eq!(insert_slot(&mut slots, value) as usize, expected);
                    if expected == reference.len() {
                        reference.push(Some(value));
                    } else {
                        reference[expected] = Some(value);
                    }
                }
            }
            let expected: Vec<_> = reference
                .iter()
                .enumerate()
                .filter_map(|(id, value)| value.as_ref().map(|_| id as u32))
                .collect();
            assert_eq!(&*slots.live_ids().expect("nonempty table"), &expected);
            assert_eq!(
                slots.get(id as u32),
                reference.get(id).and_then(Option::as_ref)
            );
        }
    }
}

#[cfg(test)]
#[path = "slot_perf_tests.rs"]
mod perf_tests;
