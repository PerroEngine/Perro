# Parallel Jobs

## Page Map

| Header | Link |
| --- | --- |
| Purpose | [Purpose](#purpose) |
| Use Cases | [Use Cases](#use-cases) |
| Import | [Import](#import) |
| API | [API](#api) |
| How It Works | [How It Works](#how-it-works) |
| Full Script Example | [Full Script Example](#full-script-example) |
| Shared Module Example | [Shared Module Example](#shared-module-example) |
| Join | [Join](#join) |
| Parallel Map | [Parallel Map](#parallel-map) |
| Context Rules | [Context Rules](#context-rules) |
| Errors | [Errors](#errors) |
| Performance | [Performance](#performance) |
| Web | [Web](#web) |

## Purpose

Use `perro_api::jobs` for custom CPU work from scripts or shared project modules.

Good uses:

- pathfinding
- AI scoring and planning
- procedural world, terrain, or mesh generation
- bulk math
- parsing, compression, and data transforms
- independent work over many owned values

Do not use jobs for small getters, a few arithmetic operations, or direct engine mutation.

No project dependency is needed. Perro exposes jobs through `perro_api` whenever `perro check`, `perro dev`, or `perro build` regenerates the script crate.

## Use Cases

- Pathfinding for a large crowd off the main thread (200 agents recomputing routes at once): snapshot positions with `query!` + `get_global_pos_3d!`, `jobs::spawn` the search, poll with `Job::try_take`, then apply results on the script thread.
- Bulk AI scoring and target selection each planning tick: `jobs::par_map` over an owned `Vec` of candidate data, preserving input order.
- Procedural generation without a frame hitch (chunk terrain, dungeon layout, mesh building): `jobs::spawn` returning the generated data, stored in `#[State]` and consumed in a later frame.
- Two independent heavy results needed before a function returns (build a nav grid and score choices together): `jobs::join`.
- Bulk data transforms (parsing, compressing, or decompressing a save blob or level payload): `jobs::par_map` or `jobs::spawn` on owned bytes.

## Import

Use the module path for clear call sites:

```rust
use perro_api::{jobs, prelude::*};
```

The prelude also exports `Job`, `JobError`, `spawn`, `join`, and `par_map` directly.

## API

| Item | Signature shape | Result |
| --- | --- | --- |
| `jobs::spawn` | `FnOnce() -> T + Send + 'static` | Start owned work and return `Job<T>` |
| `Job::try_take` | `&mut self` | `Ok(None)` while busy, `Ok(Some(T))` when done |
| `Job::take` | `self` | Block until result is ready |
| `jobs::join` | two `FnOnce` closures | Run two tasks and return both values |
| `jobs::par_map` | `Vec<T>` plus `Fn(T) -> R` | Map values in parallel and preserve order |

Native calls use Perro's shared Rayon worker pool. Perro does not create one pool per script or module.

## How It Works

Async job flow:

```text
script callback -> copy/move owned input -> worker calculation -> poll Job<T> -> apply output with ctx
```

`spawn` returns at once on native builds.

Store `Job<T>` in script state when work spans frames. Poll it with `try_take` during later callbacks.

Use `take` only when blocking is intentional. Do not call `take` in a normal frame callback because it can stall the frame.

## Full Script Example

This example starts owned work, stores its handle, polls without blocking, and applies the result on the script thread.

```rust
use perro_api::{jobs, prelude::*};

type ScoreOutput = Vec<(NodeID, f32)>;

#[State]
pub struct EnemyPlannerState {
    pending: Option<Job<ScoreOutput>>,
}

fn score_enemy(position: Vector3) -> f32 {
    // Custom CPU-heavy calculation.
    position.length_squared()
}

lifecycle!({
    fn on_update(&self, ctx: &mut ScriptContext<'_, API>) {
        let finished = with_state_mut!(ctx.run, EnemyPlannerState, ctx.id, |state| {
            let job = state.pending.as_mut()?;
            match job.try_take() {
                Ok(Some(output)) => {
                    state.pending = None;
                    Some(Ok(output))
                }
                Ok(None) => None,
                Err(error) => {
                    state.pending = None;
                    Some(Err(error))
                }
            }
        })
        .flatten();

        if let Some(result) = finished {
            match result {
                Ok(scores) => {
                    for (id, score) in scores {
                        // Apply with ctx on script thread.
                        set_var!(ctx.run, id, var!("ai_score"), score);
                    }
                }
                Err(error) => log_error!("enemy score job fail: {error}"),
            }
            return;
        }

        let busy = with_state!(ctx.run, EnemyPlannerState, ctx.id, |state| {
            state.pending.is_some()
        }).unwrap_or_default();
        if busy {
            return;
        }

        // Read engine data before spawn. Move only owned values into worker.
        let input: Vec<(NodeID, Vector3)> = query!(ctx.run, all(tag["enemy"]))
            .into_iter()
            .filter_map(|id| get_global_pos_3d!(ctx.run, id).map(|position| (id, position)))
            .collect();

        let job = jobs::spawn(move || {
            jobs::par_map(input, |(id, position)| (id, score_enemy(position)))
        });

        with_state_mut!(ctx.run, EnemyPlannerState, ctx.id, |state| {
            state.pending = Some(job);
        });
    }
});
```

Important split:

- read nodes before `spawn`
- calculate inside job
- mutate nodes after `try_take`

## Shared Module Example

Jobs work in plain project modules under `res/**/*.rs` too.

```rust
use perro_api::jobs::{self, Job};

pub struct NavInput {
    pub width: usize,
    pub cells: Vec<u8>,
}

pub struct NavGrid {
    pub costs: Vec<u32>,
}

pub fn build_nav_async(input: NavInput) -> Job<NavGrid> {
    jobs::spawn(move || {
        let costs = jobs::par_map(input.cells, |cell| build_cell_cost(cell));
        NavGrid { costs }
    })
}
```

Inputs and outputs must satisfy `Send + 'static` for `spawn`.

Use owned `Vec`, `String`, math structs, IDs, or `Arc<T>` for large immutable shared data.

## Join

Use `join` for two medium or large results needed before the current function returns.

```rust
let (paths, choices) = jobs::join(
    || build_paths(&map),
    || score_choices(&agents),
);
```

`join` waits for both tasks. Its closures may borrow local data because both finish before `join` returns.

Do not use `join` for work that must continue across frames. Use `spawn` for that.

## Parallel Map

Use `par_map` for owned bulk data.

```rust
let scores = jobs::par_map(agents, score_agent);
```

Output order matches input order.

Prefer one bulk `par_map` over thousands of separate `spawn` calls.

## Context Rules

Do not move `ScriptContext`, `ctx.run`, `ctx.res`, or `ctx.ipt` into `spawn`.

Why:

- `ctx.run` holds unique mutable runtime access
- context lifetime ends when callback returns
- native job may outlive callback
- engine stores are not one atomic thread-safe object
- locking the full runtime would serialize jobs and stall the main thread

Atomic and lock types are valid for custom shared data:

- `Arc<AtomicBool>` for cancel flags
- `Arc<AtomicU32>` for progress counters
- `Arc<Mutex<T>>` for small custom shared output
- `Arc<RwLock<T>>` for read-heavy custom data

Do not wrap the full context or runtime in a lock.

Safe engine access pattern:

```text
ctx read -> owned snapshot -> job -> owned result -> ctx apply
```

Node IDs may become invalid while a job runs. Validate or tolerate missing nodes when applying results.

## Errors

`try_take` returns:

- `Ok(None)` when work is not done
- `Ok(Some(value))` when work is done
- `Err(JobError::Panic(message))` when native worker code panics
- `Err(JobError::Canceled)` when worker stops before sending a result

Dropping `Job<T>` drops the result handle. It does not stop work already running.

## Performance

Measured native overhead on the current benchmark machine:

| Bench | Time |
| --- | ---: |
| `spawn` plus `take` | about `5.4 µs` |
| `join` | about `8.1 µs` |
| `par_map`, 256 items | about `47.8 µs` |
| `par_map`, 4096 items | about `232 µs` |

These values include benchmark work and vary by CPU, pool load, build, and OS.

Practical rules:

- use jobs for work above roughly `50–200 µs`
- batch tiny tasks
- keep input copies and output merges smaller than saved CPU time
- avoid unbounded long jobs because engine work shares the pool
- use `try_take` to keep frame callbacks non-blocking
- benchmark real game work in release mode

Run job benchmarks:

```powershell
cargo bench -p perro_jobs --bench jobs
```

## Web

Stable web builds run job closures inline.

`spawn`, `join`, and `par_map` keep the same API and results, but do not add browser threads.

Generic Rust closures cannot move into browser Web Workers on stable Rust. A future worker API needs explicit serializable task types instead of arbitrary closures.
