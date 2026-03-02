# Product Intent: What Perro Is Trying To Be

Perro is an experimental, open-source Rust-first game engine focused on:

- simplicity without sacrificing performance
- predictable, safe runtime scripting ergonomics
- a cohesive Rust-native stack (runtime + tooling + rendering)

Sources of truth for intent:

- Repo README (PerroEngine/Perro)
- perroengine.com mission + features pages

## Core principles

### 1) Rust-first gameplay scripting

- Scripts are authored in Rust with runtime helper macros (e.g., lifecycle-style entry points, method blocks, scoped state/node access).
- The goal is to keep gameplay code approachable while preserving memory safety and performance.

### 2) Native performance as a default, not a “mode”

- Engine design should avoid unnecessary runtime overhead.
- Release builds may use static embedding of assets/scenes for efficient retrieval (no parsing at runtime, direct usage).

### 3) Simple mental model

- Prefer APIs that read like gameplay code (clear ownership, explicit mutation, predictable entry points).
- Avoid designs that force users into heavy indirection or complex frameworks to accomplish basic gameplay tasks.

### 4) Modular and extensible architecture (ongoing refactor)

- The codebase is intentionally being refactored to reduce bloat/tech debt and improve modularity/extensibility before the project grows.

## Non-goals (unless explicitly stated otherwise)

- Do not pursue complexity for parity with larger engines.
- Do not introduce “magic” or hidden global behavior that makes debugging unclear.
- Do not add major new subsystems or big architectural shifts unless requested.

## Design preference tie-breakers

When there are multiple valid ways to implement something, prefer:

1. Maintain existing crate boundaries and layering
2. Smallest, least invasive change
3. Predictable runtime behavior over clever abstractions
4. Fewer dependencies and simpler compile story
5. Memory safety and clarity (no unsafe unless last resort/is actually safe)

## Contribution alignment checklist

A change is aligned if it:

- keeps gameplay authoring simple and consistent with the scripting approach
- preserves performance expectations (especially for hot loops)
- does not expand scope (no “bonus features”)
- does not introduce new architectural coupling across layers without clear intent
