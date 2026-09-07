# Engine architecture

Use this map to extend engine internals. Keep game code on `perro_api`.
Keep script syntax, scene formats and project config stable.

## Ownership

| Owner | Owns | Extension point |
| --- | --- | --- |
| Core crates | IDs, node data, metadata, math, asset formats | Node registry or format module |
| API crates | Typed contracts and facade methods | Domain trait plus domain wrapper |
| Runtime | World lifetime and execution order | Explicit phase call |
| Internal updates | Built-in per-node behavior | Typed hook dispatch |
| Scene runtime state | Routes, scene ownership, caches and preload handoff | Scene loader and scene API |
| Physics sync state | Node/world sync caches and scratch | Physics integration module |
| Physics crate | Solver state and operations | Physics system |
| Extraction state | Resource references, stream caches and invalidation state | Runtime render extraction |
| Runtime render crate | Retained draw state and exchange buffers | Render state |
| Render bridge | Typed commands, events and shared render data | Command/event pair |
| Graphics | GPU resources, preparation and pass execution | Backend/pass module |
| Resource context | CPU assets, load state and resource lifetime | Resource domain implementation |
| Build pipeline | Source preparation and static asset bake | Format-specific bake step |
| App | OS input, clock, runtime/graphics orchestration | App event/frame loop |

Keep crate boundaries. Put implementation below its owner; keep dependency direction
from implementation toward contracts/data. Run `python tools/check_architecture.py`
to check forbidden reverse dependencies and workspace dependency cycles.
The checker excludes dev dependencies; it checks normal and build edges.

## Follow one frame

Read `perro_runtime/src/runtime/phases.rs` for exact update order.
Both public update variants use this path. Const specialization removes timing
clocks from the normal path.

```text
app -> ingest input + choose fixed/update cadence
fixed -> snapshot scripts -> fixed scripts -> refresh children
      -> physics step -> internal fixed hooks -> refresh children -> transforms
update -> input startup guard + delta -> timers -> queued UI signals
       -> web route -> pending skeleton data -> preload completion
       -> script start -> script snapshot -> scripts -> Steam callbacks (if enabled)
       -> internal hooks -> children -> transforms -> audio propagation
render -> extract current world -> bridge commands -> GPU
GPU completion -> bridge events -> resource state + retained invalidation
```

Keep physics as a world step. Internal fixed hooks run after that step.
App code owns fixed-step cadence; do not infer it from the order above.
Timed entrypoints preserve existing metric boundaries.

## Add a built-in node behavior

Use `AnimatedSprite2D` as the reference path:

1. Add node data under `perro_nodes`; keep it independent of runtime implementation.
2. Add its node-registry row: parent, data type, storage, render eligibility,
   update hook, fixed hook and service membership.
3. For a new behavior, add a typed hook key in that registry module.
4. Implement behavior under `perro_internal_updates/src/nodes` and handle the key
   in `dispatch_update` or `dispatch_fixed_update`.
5. Add behavior tests plus a runtime test for lifetime/phase effects.

Example registry row:

```rust
AnimatedSprite2D => (
    Node2D, AnimatedSprite2D, Boxed, Renderable::True,
    UpdateHook::AnimatedSprite2D, FixedUpdateHook::None, NodeService::None
),
```

Exhaustive dispatch rejects missing hook implementations at compile time.
Existing update flag accessors and `NodeTypeDispatch` constants derive from hooks.
No extra runtime node-type switch is needed for ordinary behavior registration.

Use `FixedUpdateHook::PhysicsWorld` for world-step participants. Use a concrete
fixed hook for a per-node callback. Service metadata distinguishes bodies, joints
and 2D buttons. Physics-world participation does not imply a per-node fixed call.
Legacy direct particle fixed-update calls remain supported outside the schedule.

### Lifetime and mutation

Attach adds schedule membership; remove unregisters it. Dispatch snapshots retain
existing iteration order and rebuild only when membership changes. New members
join the next snapshot; removed generational IDs fail the live check.

Resolve the hook from the live node type during that check. Public `NodeMut` access
permits type replacement, so a per-instance cached hook could become stale. This
path avoids an extra hook array and redundant runtime lookups while preserving
current direct-edit behavior. It does not make direct arena edits a replacement
for runtime node create/remove APIs: those APIs own membership and cleanup.

Keep active-node guards, suspension checks and scoped mutation. Never hold a node
borrow across behavior callbacks. Node field updates retain the existing dirty
and transform rules.

## Extend an API or world service

Add operations to the relevant domain trait and its lightweight module wrapper.
Implement the trait on the runtime/resource adapter. Keep existing game accessors.
`RuntimeWindow` and `ResourceWindow` require only each used domain's trait; whole
script contexts still use the full contracts.

Use a narrow signature when possible:

```rust
fn inspect_time<R: perro_runtime_api::sub_apis::TimeAPI + ?Sized>(
    ctx: &mut perro_runtime_api::RuntimeWindow<'_, R>,
) {
    let _time = ctx.Time();
}
```

For a new world service:

1. Keep private state, scratch and invalidation rules in its owning module.
2. Give operations concrete inputs or existing narrow domain traits.
3. Add one explicit phase call at the correct dependency boundary.
4. Connect node membership through metadata if needed.
5. Test order, empty work, node removal, world changes and cleanup.

Keep cross-domain coordination on `Runtime`. Do not add a service locator,
per-node heap behavior objects or runtime scheduler dependency sorting.

## Trace a resource to the GPU

```text
resource API -> CPU load/reserve request -> existing queue
            -> bridge command -> graphics resource store
            -> bridge event -> runtime result intake
            -> resource state + extraction invalidation -> next draw extraction
```

The resource context owns CPU load/cache state. Graphics owns GPU objects.
Extraction owns node/resource reference caches and stream retention. Keep resource
completion batching and drain order in the existing render bridge path.
Keep generation checks and publish load failures through existing typed results.

For a new resource, add its shared format representation, resource domain API,
dev/static provider support, and bridge messages only when GPU work is needed.
Use the same decode helpers where representations match. Do not force dev parsing
and static baking into one execution path.

## Performance contract

Measure against a snapshot of the current worktree, including local edits.
Use separate baseline/candidate target directories and identical bench fixtures.
Do not time while compiler jobs compete for CPU.

Use `runtime_frame_memory --architecture-probe` for mixed hooks, fixed hooks,
churn and full-frame idle/sparse/all-moving fixtures. Choose `--case=...` and
`--nodes=1000` or `--nodes=10000`. Add `--architecture-alloc` for allocation counts.
Run alternating baseline/candidate processes; preserve executable hashes.
Use existing GPU-attached benchmarks for GPU claims.

Reject repeatable CPU regressions, including those below 5%. Recheck noise rather
than claiming universal zero regression. Track frame allocations, retained memory
and incremental build cost separately. Preserve recent staging, physics and asset
optimizations unless a measured regression requires a focused correction.

See [refactor measurements](architecture_refactor_results.md) for baseline, samples and validation.
