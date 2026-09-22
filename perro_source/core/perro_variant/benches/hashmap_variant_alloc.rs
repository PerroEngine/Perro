//! One-shot requested-byte audit for HashMap -> Variant conversion.
use perro_variant::Variant;
use std::{
    alloc::{GlobalAlloc, Layout, System},
    collections::{BTreeMap, HashMap},
    hint::black_box,
    sync::{
        Arc,
        atomic::{AtomicBool, AtomicIsize, AtomicUsize, Ordering},
    },
};

struct TrackingAllocator;

static TRACK: AtomicBool = AtomicBool::new(false);
static LIVE: AtomicIsize = AtomicIsize::new(0);
static PEAK: AtomicIsize = AtomicIsize::new(0);
static TOTAL: AtomicUsize = AtomicUsize::new(0);

#[global_allocator]
static ALLOCATOR: TrackingAllocator = TrackingAllocator;

fn record_add(bytes: usize) {
    let live = LIVE.fetch_add(bytes as isize, Ordering::Relaxed) + bytes as isize;
    PEAK.fetch_max(live, Ordering::Relaxed);
    TOTAL.fetch_add(bytes, Ordering::Relaxed);
}

unsafe impl GlobalAlloc for TrackingAllocator {
    unsafe fn alloc(&self, layout: Layout) -> *mut u8 {
        let ptr = unsafe { System.alloc(layout) };
        if !ptr.is_null() && TRACK.load(Ordering::Relaxed) {
            record_add(layout.size());
        }
        ptr
    }

    unsafe fn alloc_zeroed(&self, layout: Layout) -> *mut u8 {
        let ptr = unsafe { System.alloc_zeroed(layout) };
        if !ptr.is_null() && TRACK.load(Ordering::Relaxed) {
            record_add(layout.size());
        }
        ptr
    }

    unsafe fn dealloc(&self, ptr: *mut u8, layout: Layout) {
        if TRACK.load(Ordering::Relaxed) {
            LIVE.fetch_sub(layout.size() as isize, Ordering::Relaxed);
        }
        unsafe { System.dealloc(ptr, layout) }
    }

    unsafe fn realloc(&self, ptr: *mut u8, layout: Layout, new_size: usize) -> *mut u8 {
        let new_ptr = unsafe { System.realloc(ptr, layout, new_size) };
        if !new_ptr.is_null() && TRACK.load(Ordering::Relaxed) {
            let delta = new_size as isize - layout.size() as isize;
            let live = LIVE.fetch_add(delta, Ordering::Relaxed) + delta;
            PEAK.fetch_max(live, Ordering::Relaxed);
            TOTAL.fetch_add(new_size, Ordering::Relaxed);
        }
        new_ptr
    }
}

fn collect_borrowed(map: &HashMap<String, i32>) -> Variant {
    Variant::Object(
        map.iter()
            .map(|(key, value)| (Arc::<str>::from(key.as_str()), Variant::from(*value)))
            .collect::<BTreeMap<_, _>>(),
    )
}

fn insert_borrowed(map: &HashMap<String, i32>) -> Variant {
    let mut out = BTreeMap::new();
    for (key, value) in map {
        out.insert(Arc::<str>::from(key.as_str()), Variant::from(*value));
    }
    Variant::Object(out)
}

fn collect_owned(map: HashMap<String, i32>) -> Variant {
    Variant::Object(
        map.into_iter()
            .map(|(key, value)| (Arc::<str>::from(key), Variant::from(value)))
            .collect::<BTreeMap<_, _>>(),
    )
}

fn insert_owned(map: HashMap<String, i32>) -> Variant {
    let mut out = BTreeMap::new();
    for (key, value) in map {
        out.insert(Arc::<str>::from(key), Variant::from(value));
    }
    Variant::Object(out)
}

fn measure(label: &str, count: usize, f: impl FnOnce() -> Variant) {
    LIVE.store(0, Ordering::Relaxed);
    PEAK.store(0, Ordering::Relaxed);
    TOTAL.store(0, Ordering::Relaxed);
    TRACK.store(true, Ordering::Relaxed);
    let output = f();
    black_box(&output);
    let peak = PEAK.load(Ordering::Relaxed);
    let retained = LIVE.load(Ordering::Relaxed);
    let total = TOTAL.load(Ordering::Relaxed);
    drop(output);
    TRACK.store(false, Ordering::Relaxed);
    println!("{label},{count},{peak},{retained},{total}");
}

fn main() {
    println!("path,count,peak_live_requested_bytes,retained_output_bytes,total_requested_bytes");
    for count in [8usize, 128, 4096] {
        let source: HashMap<String, i32> = (0..count)
            .map(|i| (format!("field_{i:04}"), i as i32))
            .collect();
        measure("borrowed_insert", count, || insert_borrowed(&source));
        measure("borrowed_collect", count, || collect_borrowed(&source));
        measure("owned_insert", count, || insert_owned(source.clone()));
        measure("owned_collect", count, || collect_owned(source.clone()));
    }
}
