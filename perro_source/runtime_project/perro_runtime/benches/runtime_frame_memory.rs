use criterion::{BenchmarkId, Criterion, black_box};
use perro_ids::{NodeID, ScriptMemberID, SignalID, TextureID};
use perro_nodes::{IKTarget2D, Node3D, PhysicsBoneChain2D, SceneNode, SceneNodeData, Sprite2D};
use perro_render_bridge::RenderCommand;
use perro_runtime::api::bench_observability::attach_shared_frame_scripts;
use perro_runtime::{NodeArena, Runtime};
use perro_runtime_api::sub_apis::{NodeAPI, NodeSpec, SignalAPI};
use std::alloc::{GlobalAlloc, Layout, System};
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};

struct TrackingAllocator;
static LIVE: AtomicUsize = AtomicUsize::new(0);
static PEAK: AtomicUsize = AtomicUsize::new(0);
static REQUESTS: AtomicUsize = AtomicUsize::new(0);
static TRACKING: AtomicBool = AtomicBool::new(true);
#[global_allocator]
static ALLOCATOR: TrackingAllocator = TrackingAllocator;

fn note_alloc(bytes: usize) {
    REQUESTS.fetch_add(1, Ordering::Relaxed);
    let live = LIVE.fetch_add(bytes, Ordering::Relaxed) + bytes;
    PEAK.fetch_max(live, Ordering::Relaxed);
}

// SAFETY: Preserve System's pointer/layout contracts; counters never allocate.
unsafe impl GlobalAlloc for TrackingAllocator {
    unsafe fn alloc(&self, layout: Layout) -> *mut u8 {
        // SAFETY: Forward caller's valid layout unchanged.
        let ptr = unsafe { System.alloc(layout) };
        if !ptr.is_null() && TRACKING.load(Ordering::Relaxed) {
            note_alloc(layout.size());
        }
        ptr
    }
    unsafe fn dealloc(&self, ptr: *mut u8, layout: Layout) {
        if TRACKING.load(Ordering::Relaxed) {
            LIVE.fetch_sub(layout.size(), Ordering::Relaxed);
        }
        // SAFETY: Forward original live allocation with matching layout.
        unsafe { System.dealloc(ptr, layout) };
    }
    unsafe fn realloc(&self, ptr: *mut u8, layout: Layout, size: usize) -> *mut u8 {
        // SAFETY: Forward original allocation and requested nonzero size.
        let next = unsafe { System.realloc(ptr, layout, size) };
        if !next.is_null() && TRACKING.load(Ordering::Relaxed) {
            LIVE.fetch_sub(layout.size(), Ordering::Relaxed);
            note_alloc(size);
        }
        next
    }
}

fn runtime_fixture(
    count: usize,
    moving_stride: usize,
) -> (Runtime, Vec<NodeID>, Vec<RenderCommand>) {
    let mut runtime = Runtime::new();
    let mut sprite = Sprite2D::new();
    sprite.texture = TextureID::from_parts(77, 0);
    let specs = vec![NodeSpec::new(sprite); count];
    let ids = NodeAPI::create_nodes(&mut runtime, &specs, NodeID::nil());
    attach_shared_frame_scripts(&mut runtime, &ids, moving_stride);
    let mut commands = Vec::new();
    for _ in 0..3 {
        frame(&mut runtime, &mut commands);
    }
    (runtime, ids, commands)
}

fn frame(runtime: &mut Runtime, commands: &mut Vec<RenderCommand>) {
    runtime.fixed_update(1.0 / 60.0);
    runtime.update(1.0 / 60.0);
    runtime.extract_render_2d_commands();
    runtime.drain_render_commands(commands);
    black_box(commands.len());
    commands.clear();
}

fn timing(c: &mut Criterion) {
    let mut group = c.benchmark_group("runtime_frame");
    for count in [1_000, 10_000] {
        for (name, stride) in [
            ("idle_scripts", 0),
            ("one_percent_move", 100),
            ("all_move", 1),
        ] {
            let (mut runtime, _ids, mut commands) = runtime_fixture(count, stride);
            group.bench_function(BenchmarkId::new(name, count), |b| {
                b.iter(|| frame(&mut runtime, &mut commands));
            });
        }
    }
    group.finish();
}

fn memory_row(label: &str, count: usize, base: usize, slots: usize, live_nodes: usize) {
    let live = LIVE.load(Ordering::Relaxed);
    let peak = PEAK.load(Ordering::Relaxed);
    let requests = REQUESTS.load(Ordering::Relaxed);
    println!(
        "{label}/{count}: live_delta={} peak_delta={} requests={requests} slots={slots} nodes={live_nodes}",
        live.saturating_sub(base),
        peak.saturating_sub(base)
    );
}

fn reset_sample() -> usize {
    let base = LIVE.load(Ordering::Relaxed);
    PEAK.store(base, Ordering::Relaxed);
    REQUESTS.store(0, Ordering::Relaxed);
    base
}

fn memory() {
    println!(
        "layout: SceneNode={} Option<SceneNode>={} Node3D={}",
        size_of::<SceneNode>(),
        size_of::<Option<SceneNode>>(),
        size_of::<Node3D>()
    );
    // One runtime per process prevents background teardown from an earlier
    // fixture from contributing frees to a later live-byte sample.
    for count in [probe_arg("--nodes=", 10_000)] {
        let base = reset_sample();
        let mut arena = NodeArena::with_capacity(count + 1);
        let mut ids = Vec::with_capacity(count);
        memory_row(
            "arena_reserved",
            count,
            base,
            arena.slot_count(),
            arena.len(),
        );
        for _ in 0..count {
            ids.push(arena.insert(SceneNode::new(SceneNodeData::Node3D(Node3D::new()))));
        }
        memory_row("arena_filled", count, base, arena.slot_count(), arena.len());
        for &id in &ids {
            drop(arena.remove(id));
        }
        memory_row(
            "arena_removed",
            count,
            base,
            arena.slot_count(),
            arena.len(),
        );
        ids.clear();
        for _ in 0..count {
            ids.push(arena.insert(SceneNode::new(SceneNodeData::Node3D(Node3D::new()))));
        }
        memory_row(
            "arena_refilled",
            count,
            base,
            arena.slot_count(),
            arena.len(),
        );
        drop((arena, ids));

        let mut runtime = Runtime::new();
        let base = reset_sample();
        let specs = vec![NodeSpec::new(Node3D::new()); count];
        let ids = NodeAPI::create_nodes(&mut runtime, &specs, NodeID::nil());
        drop(specs);
        memory_row(
            "runtime_nodes",
            count,
            base,
            runtime.nodes.slot_count(),
            runtime.nodes.len(),
        );
        attach_shared_frame_scripts(&mut runtime, &ids, 0);
        memory_row(
            "runtime_scripts",
            count,
            base,
            runtime.nodes.slot_count(),
            runtime.nodes.len(),
        );
        let signal = SignalID::from_string("memory_probe");
        let method = ScriptMemberID::from_string("on_signal");
        for &id in &ids {
            assert!(SignalAPI::signal_connect(
                &mut runtime,
                id,
                signal,
                method,
                &[]
            ));
        }
        memory_row(
            "runtime_signal_links",
            count,
            base,
            runtime.nodes.slot_count(),
            runtime.nodes.len(),
        );
        for &id in &ids {
            assert!(NodeAPI::remove_node(&mut runtime, id));
        }
        let mut commands = Vec::new();
        runtime.drain_render_commands(&mut commands);
        commands.clear();
        memory_row(
            "runtime_removed",
            count,
            base,
            runtime.nodes.slot_count(),
            runtime.nodes.len(),
        );
        drop((runtime, ids, commands));
    }
}

fn transform_work() {
    for count in [1_000usize, 10_000] {
        for chain in [false, true] {
            let mut runtime = Runtime::new();
            let specs: Vec<_> = (0..count)
                .map(|index| {
                    let spec = NodeSpec::new(Node3D::new());
                    if chain {
                        spec.parent(index.checked_sub(1))
                    } else {
                        spec
                    }
                })
                .collect();
            let ids = NodeAPI::create_nodes(&mut runtime, &specs, NodeID::nil());
            for &id in &ids {
                black_box(NodeAPI::get_global_transform_3d(&mut runtime, id));
            }
            for case in ["clean", "leaf", "root"] {
                runtime.bench_begin_transform_work();
                if case != "clean" {
                    let id = if case == "leaf" {
                        ids[count - 1]
                    } else {
                        ids[0]
                    };
                    NodeAPI::with_base_node_mut::<Node3D, _, _>(&mut runtime, id, |node| {
                        node.transform.position.x += 1.0
                    })
                    .expect("live node");
                }
                for &id in &ids {
                    black_box(NodeAPI::get_global_transform_3d(&mut runtime, id));
                }
                let work = runtime.bench_end_transform_work();
                println!(
                    "transform/{}/{case}/{count}: dirty_visits={} queries={} cache_hits={} rebuilt_rows={}",
                    if chain { "chain" } else { "flat" },
                    work[0],
                    work[1],
                    work[2],
                    work[3]
                );
            }
        }
    }
}

fn frame_work() {
    for count in [probe_arg("--nodes=", 10_000)] {
        for (name, stride) in [
            ("idle_scripts", 0),
            ("one_percent_move", 100),
            ("all_move", 1),
        ] {
            if stride != probe_arg("--stride=", 0) {
                continue;
            }
            let (mut runtime, _ids, mut commands) = runtime_fixture(count, stride);
            let base = reset_sample();
            runtime.bench_begin_transform_work();
            frame(&mut runtime, &mut commands);
            let work = runtime.bench_end_transform_work();
            let requests = REQUESTS.load(Ordering::Relaxed);
            let live = LIVE.load(Ordering::Relaxed).saturating_sub(base);
            let peak = PEAK.load(Ordering::Relaxed).saturating_sub(base);
            println!(
                "frame/{name}/{count}: alloc_requests={requests} live_delta={live} peak_delta={peak} dirty_visits={} queries={} cache_hits={} rebuilt_rows={}",
                work[0], work[1], work[2], work[3]
            );
        }
    }
}

fn probe_arg(prefix: &str, default: usize) -> usize {
    std::env::args()
        .find_map(|arg| {
            arg.strip_prefix(prefix)
                .map(|value| value.parse().expect("integer probe arg"))
        })
        .unwrap_or(default)
}

fn internal_times() {
    TRACKING.store(false, Ordering::Relaxed);
    let count = probe_arg("--nodes=", 10_000);
    let fixed = std::env::args().any(|arg| arg == "--fixed-internal");
    let mut runtime = Runtime::new();
    for _ in 0..count {
        if fixed {
            NodeAPI::create::<PhysicsBoneChain2D>(&mut runtime);
        } else {
            NodeAPI::create::<IKTarget2D>(&mut runtime);
        }
    }
    for _ in 0..30 {
        if fixed {
            runtime.fixed_update(1.0 / 60.0);
        } else {
            runtime.update(1.0 / 60.0);
        }
    }
    let mut total = Vec::with_capacity(120);
    let mut internal = Vec::with_capacity(120);
    let mut physics = Vec::with_capacity(120);
    for _ in 0..120 {
        if fixed {
            let timing = runtime.fixed_update_timed(1.0 / 60.0);
            total.push(timing.total.as_nanos());
            internal.push(timing.internal_fixed_update.as_nanos());
            physics.push(timing.physics.as_nanos());
        } else {
            let timing = runtime.update_timed(1.0 / 60.0);
            total.push(timing.total.as_nanos());
            internal.push(timing.internal_update.as_nanos());
            physics.push(0);
        }
    }
    total.sort_unstable();
    internal.sort_unstable();
    physics.sort_unstable();
    println!(
        "internal_timing/{}/{count}: total_us={} internal_us={} physics_us={} samples=120",
        if fixed { "fixed" } else { "update" },
        total[60] as f64 / 1_000.0,
        internal[60] as f64 / 1_000.0,
        physics[60] as f64 / 1_000.0
    );
}

fn main() {
    if std::env::args().any(|arg| arg == "--architecture-probe") {
        architecture_probe();
        return;
    }
    if std::env::args().any(|arg| arg == "--internal-times") {
        internal_times();
        return;
    }
    if std::env::args().any(|arg| arg == "--memory") {
        memory();
        return;
    }
    if std::env::args().any(|arg| arg == "--transform-work") {
        transform_work();
        return;
    }
    if std::env::args().any(|arg| arg == "--frame-work") {
        frame_work();
        return;
    }
    // Keep timing free of live/peak/request atomic updates. This switch stays
    // off until process exit; the pre-main tracked balance is unused here.
    TRACKING.store(false, Ordering::Relaxed);
    let mut c = Criterion::default().configure_from_args();
    timing(&mut c);
    c.final_summary();
}

// Same fixture is copied into the preserved baseline for architecture A/B runs.
fn architecture_probe() {
    use perro_nodes::{
        AnimatedSprite, AnimatedSprite2D, BoneAttachment2D, BoneAttachment3D, IKTarget3D,
    };
    let allocations = std::env::args().any(|arg| arg == "--architecture-alloc");
    TRACKING.store(allocations, Ordering::Relaxed);
    let count = probe_arg("--nodes=", 10_000);
    let case = std::env::args()
        .find_map(|arg| arg.strip_prefix("--case=").map(str::to_owned))
        .unwrap_or_else(|| "mixed_update".into());
    let mut runtime;
    let mut commands = Vec::new();
    if case.starts_with("frame_") {
        let stride = match case.as_str() {
            "frame_idle" => 0,
            "frame_sparse" => 100,
            _ => 1,
        };
        (runtime, _, commands) = runtime_fixture(count, stride);
    } else {
        runtime = Runtime::new();
        for index in 0..count {
            if case == "mixed_fixed" {
                NodeAPI::create::<PhysicsBoneChain2D>(&mut runtime);
            } else {
                match index % 5 {
                    0 => {
                        NodeAPI::create::<IKTarget2D>(&mut runtime);
                    }
                    1 => {
                        NodeAPI::create::<IKTarget3D>(&mut runtime);
                    }
                    2 => {
                        NodeAPI::create::<BoneAttachment2D>(&mut runtime);
                    }
                    3 => {
                        NodeAPI::create::<BoneAttachment3D>(&mut runtime);
                    }
                    _ => {
                        let mut sprite = AnimatedSprite2D::new();
                        let mut anim = AnimatedSprite::new("probe");
                        anim.frame_count = 4;
                        anim.fps = 12.0;
                        sprite.animations.push(anim);
                        NodeAPI::create_nodes(
                            &mut runtime,
                            &[NodeSpec::new(sprite)],
                            NodeID::nil(),
                        );
                    }
                }
            }
        }
    }
    let mut tick = |runtime: &mut Runtime| {
        if case.starts_with("frame_") {
            frame(runtime, &mut commands);
        } else if case == "mixed_fixed" {
            runtime.fixed_update(1.0 / 60.0);
        } else {
            if case == "churn" {
                let id = NodeAPI::create::<IKTarget2D>(runtime);
                black_box(NodeAPI::remove_node(runtime, id));
            }
            runtime.update(1.0 / 60.0);
        }
    };
    for _ in 0..60 {
        tick(&mut runtime);
    }
    let mut samples = Vec::with_capacity(101);
    let base = reset_sample();
    for _ in 0..101 {
        let start = std::time::Instant::now();
        for _ in 0..10 {
            tick(&mut runtime);
        }
        samples.push(start.elapsed().as_nanos() / 10);
    }
    let live = LIVE.load(Ordering::Relaxed);
    let requests = REQUESTS.load(Ordering::Relaxed);
    samples.sort_unstable();
    println!(
        "architecture/{case}/{count}: median_ns={} p95_ns={} allocs={} live_delta={} runtime_bytes={}",
        samples[50],
        samples[95],
        requests,
        live.saturating_sub(base),
        size_of::<Runtime>()
    );
}
