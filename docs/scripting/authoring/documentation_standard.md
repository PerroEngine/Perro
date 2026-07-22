# Script Documentation Standard

Project-wide contract: [Writing Standard](../../writing_standard.md). This page
adds scripting-specific checks.

Use this order for active scripting guide and API pages:

1. purpose
2. mental model
3. ownership and data flow
4. when to use
5. when not to use
6. use cases with reasons and tradeoffs
7. feature walkthrough
8. failure and edge behavior
9. performance and borrow notes
10. exact API reference
11. related concepts and verified examples

Simple APIs may combine sections. Complex topics should keep them visible.

Every major example must state the goal, owners, scene wiring, state shape,
complete flow, reason for each API, missing-ref behavior, rejected alternative,
and extension paths. Link runnable source rather than copying large files across
many pages.

Avoid generic text such as "use when gameplay needs this." Name the situation,
choice, reason, and cost. Do not imply that `#[expose]` gates runtime access, use
runtime lookup for a fixed ref, hide a nested `ctx.run` borrow, or pass an asset
path string through runtime `set_var!`.

## Audit Checklist

- [ ] owner, source, target, lifecycle, and failure result are explicit
- [ ] typed vs dynamic choice has a reason
- [ ] fixed refs use scene-injected `NodeID`
- [ ] asset paths resolve through scene injection or Resource API
- [ ] method, signal, and dynamic var examples match their semantics
- [ ] runtime closures end before the next runtime API call
- [ ] links and anchors resolve
- [ ] code matches a checked demo or current engine API
