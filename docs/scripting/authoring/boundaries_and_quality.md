# Script Boundaries And Quality

## Script Split

Split by ownership and lifecycle, not file size. Keep one cohesive node behavior
together. Use a controller for scene flow. Put pure math, constants, and shared
data transforms in ordinary Rust modules. Avoid one script per tiny action and
avoid a manager that reaches into every known state.

## Debug And Test

- Run `perro check` after script or scene edits.
- Run doctor to catch missing refs, type hints, and scene wiring faults.
- Run clippy for generated script crates and workspace code.
- Test pure helpers as normal Rust functions.
- Test dynamic boundaries with missing targets, wrong params, and neutral replies.
- Keep a runnable demo as the source behind major docs examples.

## Performance

- Avoid name lookup or queries for fixed dependencies.
- Avoid per-frame clocks for one-shot delays.
- Avoid cloning large state when a small copied result is enough.
- Keep runtime borrows short; never nest another `ctx.run` call in a typed access closure.
- Cache per-instance resource IDs in state; Resource API caches repeated paths.

## Common Bad Patterns

| Pattern | Problem | Replace With |
| --- | --- | --- |
| query/name lookup for fixed target | hidden wiring + repeated work | injected `NodeID` |
| `get_var!` for known state type | loses type checks | `with_state!` |
| signal used as a request/reply | emitter cannot own result | method |
| method call to every interested system | tight fan-out coupling | signal |
| nested runtime calls in closure | overlapping runtime borrow | copy out, call after |
| manual cooldown decrement | idle frame work | named timer |
| forced one-role script split | fragmented ownership | cohesive script |

