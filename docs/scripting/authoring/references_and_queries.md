# References, Relations, And Queries

## Purpose

Choose how a script finds another node from how stable that relationship is.

## Decision

| Relationship | Use | Why | Tradeoff |
| --- | --- | --- | --- |
| exact scene dependency | state `NodeID` + `script_vars` | explicit and stable | scene must wire it |
| optional exact dependency | `Option<NodeID>` | absence is modeled | caller must guard |
| real hierarchy dependency | parent/child API | topology is the contract | reparenting changes result |
| spawned/dynamic membership | query/tag | set changes at runtime | lookup cost + zero/many results |

Do not search by node name each frame when a scene already knows the target.
Do not inject every child when the behavior genuinely means "my parent" or
"current children." Do not cache a query result unless lifetime invalidation is
handled.

## Failure Behavior

A stored ID can become stale after node removal. Check nil where applicable.
Typed reads and writes return `None` on a missing, stale, or wrong-type target.
A query can return no targets; empty is a normal result. Optional features
should quietly skip absent targets.

## Performance

Read fixed refs from state once per callback. Copy IDs out of state before node
access. Query at the cadence the feature needs, not automatically every frame.

## Related

- [Node And State Access](node_and_state_access.md)
- [Node queries](../query_system.md)
- [Manager And Spawned Enemies](examples/spawned_enemies.md)
