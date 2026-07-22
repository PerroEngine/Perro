# Perro Docs Writing Standard

## Goal

Teach how Perro works, why one API fits, and what owns each value.

Write for humans and AI agents. A reader should leave with a correct mental
model and enough exact detail to build the feature without guessing.

## Page Profiles

### Concept Guide

Use for workflows, formats, architecture, and cross-system behavior.

Order:

1. purpose
2. mental model
3. ownership and data flow
4. when to use
5. when not to use
6. use cases with reasons
7. feature walkthrough
8. failure and edge behavior
9. performance or borrow notes
10. related concepts and verified examples

### Concept Plus API Reference

Use for runtime, resource, input, node, and script APIs.

Use the concept-guide order first. Keep exact signatures and function tables
under `API Reference`. Do not make readers cross the reference tables to learn
the core model.

### Index Or Map

Use for navigation pages. State who the area serves, give a task-to-page map,
and explain how neighboring groups differ. Do not add a fake feature example to
an index.

## Use Cases

Each use case must contain four facts:

- situation: what the game needs
- choice: which Perro type, API, or format to use
- reason: why it fits ownership, lifetime, or data flow
- tradeoff: when a nearby choice fits better

Bad:

> Use `query!` to find nodes.

Good:

> Find all spawned enemies with `query!` because membership changes at runtime.
> Store a scene-injected `NodeID` instead when exactly one fixed target exists.

## Feature Walkthroughs

Major examples use one complete feature story.

Include:

- goal
- node, script, resource, and state owners
- scene or asset wiring
- lifecycle hook for each step
- full data and event flow
- relevant complete source, or links to verified source
- expected return and failure result
- why each API fits
- why the nearest alternative does not fit
- safe extension paths

Keep small code snippets only when the page teaches one small operation.

## AI Contract

State these facts when they matter:

| Fact | Required detail |
| --- | --- |
| owner | script, node, scene, resource cache, or game system |
| source | state, scene var, query, relation, input, resource, or callback param |
| target | `ctx.id`, fixed `NodeID`, relation, query result, or dynamic member |
| time | init, all-init, update, fixed update, signal, method, timer, or removal |
| failure | `None`, nil ID, `false`, `Variant::Null`, kept default, or logged error |
| access mode | typed state/node access or dynamic `Variant` access |

Never rely on an unnamed runtime lookup or an implied owner.

## API Reference Rules

- copy signatures from source
- describe concrete params and return values
- name the exact nil, empty, optional, or failure result
- replace generic `Use when gameplay needs...` text with API-specific guidance
- preserve useful anchors and active routes
- keep scene-only coercion separate from strict runtime `set_var!`
- label editor-only metadata such as `#[expose]` and `#[node_ref]`

## Source And Example Rules

- prefer verified demo source over copied large scripts
- keep copied snippets short and complete for the point they teach
- use fixed `NodeID` refs for scene-known targets
- use parent/child access for structural targets
- use queries only for dynamic sets
- keep `ctx.run` calls outside state/node closures
- use methods for targeted calls and replies
- use signals for events and fan-out
- use dynamic vars only when type or member choice happens at runtime
- use named timers for delays and cooldown completion

## Verification

Before merge:

- check relative links and anchors
- scan stale paths and macro names
- scan generic placeholder reference text
- inspect examples for nested runtime borrows
- run `perro check`, doctor, and clippy for linked demo projects
- compare claims and signatures with current source
- keep dated audits and historical records unchanged unless a link breaks
