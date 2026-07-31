// Shrink policy for grow-only GPU buffers.
//
// Every ensure-capacity helper in the renderer doubles and never shrinks, so
// one heavy scene pins the high-water mark for the whole session (brutal on
// shared-memory iGPUs). Each shrinkable buffer owns a `ShrinkTracker`; writes
// note the content length, and on the periodic GC tick a buffer whose usage
// stayed under a quarter of capacity for `SHRINK_LOW_TICKS` consecutive ticks
// reallocates to `max(used * 2, min_capacity)`.
//
// Shrinks always preserve content with a GPU->GPU prefix copy (the new
// capacity always covers the tracked content), so retained-content buffers and
// skip-upload gates stay valid without per-buffer semantic analysis. Buffers
// that participate must carry COPY_SRC | COPY_DST usage. Bind groups that
// reference a shrunk buffer must be rebuilt by the caller.

/// Consecutive GC ticks (1 tick ~= 60 frames) a buffer must stay under a
/// quarter of its capacity before it shrinks.
pub(crate) const SHRINK_LOW_TICKS: u32 = 3;

#[derive(Debug, Default, Clone, Copy)]
pub(crate) struct ShrinkTracker {
    /// Peak content length noted since the last GC tick.
    peak: usize,
    /// Most recent content length; persists across ticks so buffers whose
    /// prepare is skipped (retained content still drawn) never shrink below
    /// their live content.
    last: usize,
    low_ticks: u32,
}

impl ShrinkTracker {
    /// Note the buffer's current content length (a full length, not a delta).
    /// Call from the ensure-capacity helper or wherever the frame's content
    /// length is known.
    #[inline]
    pub fn note_used(&mut self, used: usize) {
        self.last = used;
        if used > self.peak {
            self.peak = used;
        }
    }

    /// Periodic GC tick. Returns the target capacity when the buffer stayed
    /// under a quarter of `capacity` for `SHRINK_LOW_TICKS` consecutive ticks.
    /// The returned capacity is `max(used * 2, min_capacity)`, strictly less
    /// than `capacity`, and always covers the tracked content.
    pub fn tick(&mut self, capacity: usize, min_capacity: usize) -> Option<usize> {
        let used = self.peak.max(self.last);
        self.peak = self.last;
        if capacity <= min_capacity || used >= capacity / 4 {
            self.low_ticks = 0;
            return None;
        }
        self.low_ticks += 1;
        if self.low_ticks < SHRINK_LOW_TICKS {
            return None;
        }
        self.low_ticks = 0;
        Some(used.saturating_mul(2).max(min_capacity).min(capacity))
    }
}

/// Reallocate `old` at `new_size_bytes` and copy the prefix that still fits.
/// `new_size_bytes` must not exceed the old buffer's size; the copy length is
/// rounded down to wgpu's 4-byte copy alignment.
pub(crate) fn shrink_buffer_preserving(
    device: &wgpu::Device,
    queue: &wgpu::Queue,
    old: &wgpu::Buffer,
    label: &'static str,
    new_size_bytes: u64,
    usage: wgpu::BufferUsages,
) -> wgpu::Buffer {
    let new_buffer = device.create_buffer(&wgpu::BufferDescriptor {
        label: Some(label),
        size: new_size_bytes.max(4),
        usage,
        mapped_at_creation: false,
    });
    let copy_bytes = new_size_bytes.min(old.size()) & !3;
    if copy_bytes > 0 {
        let mut encoder = device.create_command_encoder(&wgpu::CommandEncoderDescriptor {
            label: Some("perro_buffer_shrink_copy"),
        });
        encoder.copy_buffer_to_buffer(old, 0, &new_buffer, 0, copy_bytes);
        queue.submit(Some(encoder.finish()));
    }
    new_buffer
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn tracker_shrinks_after_consecutive_low_ticks() {
        let mut tracker = ShrinkTracker::default();
        tracker.note_used(10);
        assert_eq!(tracker.tick(1024, 8), None);
        assert_eq!(tracker.tick(1024, 8), None);
        assert_eq!(tracker.tick(1024, 8), Some(20));
    }

    #[test]
    fn tracker_resets_on_spike() {
        let mut tracker = ShrinkTracker::default();
        tracker.note_used(10);
        assert_eq!(tracker.tick(1024, 8), None);
        tracker.note_used(600);
        assert_eq!(tracker.tick(1024, 8), None);
        // Spike within the window resets the streak even after usage drops,
        // and the carried peak keeps one extra tick conservative.
        tracker.note_used(10);
        assert_eq!(tracker.tick(1024, 8), None);
        assert_eq!(tracker.tick(1024, 8), None);
        assert_eq!(tracker.tick(1024, 8), None);
        assert_eq!(tracker.tick(1024, 8), Some(20));
    }

    #[test]
    fn tracker_last_used_persists_across_ticks() {
        let mut tracker = ShrinkTracker::default();
        tracker.note_used(300);
        // No further writes (prepare skipped); the retained content length
        // keeps the buffer from shrinking below it.
        for _ in 0..10 {
            assert_eq!(tracker.tick(1024, 8), None);
        }
    }

    #[test]
    fn tracker_never_returns_capacity_at_or_above_current() {
        let mut tracker = ShrinkTracker::default();
        for _ in 0..SHRINK_LOW_TICKS {
            tracker.note_used(0);
            let result = tracker.tick(64, 64);
            assert_eq!(result, None, "capacity at min must never shrink");
        }
        let mut tracker = ShrinkTracker::default();
        tracker.note_used(0);
        tracker.tick(256, 16);
        tracker.tick(256, 16);
        assert_eq!(tracker.tick(256, 16), Some(16));
    }
}
