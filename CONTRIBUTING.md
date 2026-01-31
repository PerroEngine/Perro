# Contributing to Perro

Thank you for helping improve Perro. Keep contributions small, tested, and focused on the engine's goals: performance, ease of use, and practical game development.

## Experimental status & compiler issues

Perro's scripting and codegen are **experimental**. Compiler errors can occur even when the source (PUP, FUR, etc.) looks syntactically correct—e.g. due to transpiler bugs, API mismatches, or missing bindings. If you hit such issues, fixing them or reporting them (with minimal repro and expected vs actual behavior) is very helpful. When fixing bugs, make sure changes are applied in the right layer (see below).

## Where to fix scripting / API issues

When fixing scripting, API, or codegen bugs, apply changes in the appropriate place so all languages and call sites stay consistent. For how these layers fit together (modules, bindings, language→module resolution), see *Transpiler architecture* below.

- **Central API & module definitions** — Shared API surface and module definitions used by all languages (e.g. in `perro_core`).
- **Per-language API input** — Language-specific API definitions and codegen input (e.g. C#, TypeScript, PUP under `scripting/lang/`).
- **Bindings & engine registry** — Resource bindings and the engine struct/type registry that the runtime and codegen rely on.

Fixes that belong in the central API or module definitions should not be done only in one language’s API or bindings; keep the central layer authoritative and the per-language layers and bindings aligned.

## Transpiler architecture (how it fits together)

The transpiler is designed so that **syntax and language are only at the edges**; the middle is **type-agnostic** and shared. The pipeline is:

**Source string → (parser) → AST / types → (modules & bindings) → Rust code string**

### Pipeline overview

1. **Input:** Source text (PUP, TypeScript, C#, etc.) — language-specific syntax.
2. **Parsing:** Each language has its own parser under `scripting/lang/<lang>/`. The parser produces a **language-agnostic AST** (`Expr`, `Type`, `Script`, etc.) and resolves API/node calls into **module references**, not raw strings.
3. **Middle (type-agnostic):** The AST and type system are shared: no PUP- or TS-specific types in the core. All calls go through a single abstraction: **modules** and **bindings**.
4. **Output:** A **binding** (keyed by module or engine ref) turns typed arguments into a **Rust code string**. So the flow is:

   **string (syntax) → module (or engine ref) → binding → rust_string_code_output**

   The "module" and "binding" layers are type-agnostic; only the parser (string → AST/module) and the binding (AST + types → Rust string) are language- or target-specific.

### What separates API modules, resource modules, nodes/engine registry, and enums

Four different kinds of “callable” or “script-visible” things exist; each has a different role and a different place for logic, module/ref, binding, and frontend:

| Kind | What it is | Where the **real logic** lives | Where the **module/ref** lives | Where the **binding** (codegen → Rust string) lives | Where the **frontend** (syntax → module/ref) lives |
|------|------------|----------------------------------|--------------------------------|------------------------------------------------------|-----------------------------------------------------|
| **API modules** | Global utilities, no receiver: `JSON.parse(x)`, `Time.get_delta()`. | `scripting/api.rs` — actual Rust impl (e.g. `JsonApi::parse`). | `scripting/api_modules.rs` — enum of operations (e.g. `ApiModule::JSON(JSONApi::Parse)`). | `scripting/api_bindings.rs` — `ModuleCodegen` + `ModuleTypes` per variant; emits e.g. `api.json_parse(...)`. | Per language: `scripting/lang/pup/api.rs`, `typescript/api.rs`, `csharp/api.rs` — e.g. `"parse"` under `"JSON"` → `ApiModule::JSON(JSONApi::Parse)`. |
| **Resource modules** | Types you construct or call static methods on: `Signal.new(...)`, `Texture.load(...)`, `Array.push(...)`. | Runtime lives in engine (texture/mesh/signal APIs, etc.); no single “api.rs” for resources. | `scripting/resource_modules.rs` — enum of operations (e.g. `ResourceModule::Signal(SignalResource::Connect)`). | `scripting/resource_bindings.rs` — `ModuleCodegen` / `ModuleTypes` per variant; emits calls into engine/API. | Per language: `scripting/lang/pup/resource_api.rs`, etc. — e.g. `"Signal"` + `"connect"` → `ResourceModule::Signal(SignalResource::Connect)`. |
| **Nodes / engine registry** | Scene nodes and their methods/fields: `node.get_parent()`, `node.get_var("x")`. Receiver is a node. | Node behavior lives in `nodes/` and `scripting/api.rs` (e.g. `get_script_var_id`). | `structs/engine_registry.rs` — `NodeType`, `NodeFieldRef`, `NodeMethodRef`; `register_node` / `register_node_methods` map script names to refs and signatures. | `structs/engine_bindings.rs` — `EngineMethodCodegen` for each `NodeMethodRef`; emits e.g. `api.get_script_var_id(...)`. | Per language: `scripting/lang/pup/node_api.rs`, etc. — script name (e.g. `"get_var"`) → `NodeMethodRef::GetVar`. Parser also uses engine registry to resolve method names. |
| **Built-in enums** | Script-visible enum constants, e.g. `NODE_TYPE.Sprite2D`. | Rust enum (e.g. `NodeType`) in `nodes/node_registry.rs`; no separate “logic” file. | AST: `BuiltInEnumVariant::NodeType(NodeType)` in `ast.rs`; codegen turns variant into Rust enum literal. | Codegen in expression/literal code (emit `NodeType::Sprite2D`, etc.); no separate binding trait. | Per language: `scripting/lang/pup/enums.rs`, etc. — e.g. script name `"Sprite2D"` under `NODE_TYPE` → `NodeType::Sprite2D`. |

Summary:

- **API modules** = global, free-standing functions; **logic** in `api.rs`, **module** in `api_modules.rs`, **binding** in `api_bindings.rs`, **frontend** in each language’s `api.rs`.
- **Resource modules** = static/constructor-style calls on types (Signal, Texture, Array, …); **module** in `resource_modules.rs`, **binding** in `resource_bindings.rs`, **frontend** in each language’s `resource_api.rs`; runtime is spread across engine/API.
- **Nodes / engine registry** = methods on a node instance; **logic** in nodes + `api.rs`, **refs and signatures** in `engine_registry.rs`, **binding** in `engine_bindings.rs`, **frontend** in each language’s `node_api.rs` (+ parser using registry).
- **Enums** = built-in constants; **logic** is the Rust enum; **frontend** is each language’s `enums.rs` (and parser); codegen is inline, no separate binding trait.

### What the modules are and why they exist

**Modules** (and node method refs) are the central, language-agnostic names for "what kind of call this is":

- **API modules** (`api_modules.rs`): Global utilities — e.g. `JSON`, `Time`, `OS`, `Console`, `Input`, `Math`. Each is an enum of operations (e.g. `JSONApi::Parse`, `TimeApi::DeltaTime`). They exist so every language can express "call JSON.parse" without the core caring about keyword or naming differences.
- **Resource modules** (`resource_modules.rs`): Types/resources that can be constructed or used — e.g. `Signal`, `Texture`, `Mesh`, `Shape`, `Array`, `Map`, `Quaternion`. Again, enums of operations (e.g. `SignalResource::Connect`, `ArrayResource::Push`). Same idea: one central set of operations, many surface syntaxes.
- **Node methods (engine registry):** Script calls like `get_parent`, `get_node`, `get_var` are not free-form strings in the middle layer. They are **node method refs** (`NodeMethodRef`) coming from the **engine registry** (`engine_registry.rs`). The registry maps (node type, script method name) → `NodeMethodRef` and stores signatures (param/return types, param names). So "language syntax for a node method" is turned into a ref, and codegen only sees that ref.

So: **modules (and node method refs) exist to make the middle layer independent of language.** All languages resolve "their" syntax to the same enums/refs; codegen only cares about those.

### What the bindings are and how they attach to modules

**Bindings** are the implementations that turn a **module variant** (or `NodeMethodRef`) plus **typed arguments** into **Rust code**:

- **API bindings** (`api_bindings.rs`): Implement `ModuleCodegen` and `ModuleTypes` for each `ApiModule` variant. They define param/return types and emit the actual Rust (e.g. `api.json_parse(...)`, `api.get_delta_time()`).
- **Resource bindings** (`resource_bindings.rs`): Same idea for `ResourceModule`: `ModuleCodegen` / `ModuleTypes` for `SignalResource`, `TextureResource`, etc. They emit Rust that uses the engine's resource APIs.
- **Engine bindings** (`structs/engine_bindings.rs`): Implement `EngineMethodCodegen` for `NodeMethodRef`. Node methods like `get_var`, `set_var`, `get_parent` are implemented here; they get param types from the **engine registry** and emit the right `api.*` or method calls.

So: **bindings attach to modules (and to node method refs)**: one implementation per module variant or ref. Adding a new API or resource means: (1) extend the module enum, (2) add the binding impl, (3) add per-language resolution from syntax to that enum/ref.

### How language-side syntax corresponds to modules or engine refs

- **Per-language API** (e.g. `scripting/lang/pup/api.rs`, `resource_api.rs`): Each language exposes names (e.g. `"JSON"`, `"parse"`, `"Signal"`, `"connect"`). The parser calls something like `PupAPI::resolve(module, func)` or `PupResourceAPI::resolve(module, func)` and gets back an `ApiModule` or `ResourceModule` variant. So **language string → central module**.
- **Node methods:** The parser (and sometimes type inference) uses the **engine registry** to map script method names (e.g. `get_node`, `get_parent`) to `NodeMethodRef`. So **language syntax for a node method → NodeMethodRef**; the binding then maps **NodeMethodRef → Rust string**.

The AST stores **resolved** references (`Expr::ApiCall(CallModule, args)` where `CallModule` is `Module(api)`, `Resource(resource)`, or `NodeMethod(ref)`), not the original keywords. So downstream passes (type inference, codegen) only see modules and refs.

### AST and type system (type-agnostic)

- **AST** (`ast.rs`): `Expr`, `Type`, `Script`, `Function`, etc. are shared. There are no language specific types in the core AST. Literals, binary ops, member access, calls, etc. are represented once; each language's parser fills that representation.
- **Type system:** `Type` is a single enum (numbers, bool, string, containers, node types, engine structs, custom, etc.). Type inference and codegen use this only. So: **syntax on input → typed AST (shared types) → Rust output.** The type layer is intentionally agnostic so that adding a language is "parser + string→module resolution," not a new type system or codegen path.

When you add or fix something in the transpiler, place it in the right layer: **syntax/resolution** in the per-language API or registry, **semantics/signatures** in modules and registry, **code emission** in the corresponding bindings (API, resource, or engine).

### How to add onto the transpiler

Use the table above to see which layer gets the **logic**, **module/ref**, **binding**, and **frontend** for each kind of feature.

**Adding a new node type** — touch all of these:

1. **Node registry** (`nodes/node_registry.rs`): The **`define_nodes!` macro** declares all node types and generates the `NodeType` enum, the `SceneNode` enum, and the `BaseNode` impls. Add one line to the macro invocation:

   **`define_nodes!`** — Syntax: `NodeName(FixedUpdate, RenderUpdate, Renderable) => path::to::Type`

   - **FixedUpdate** (`FixedUpdate::True` / `FixedUpdate::False`): Whether this node runs **internal fixed update** at the project's XPS (fixed timestep) rate. Used for physics, etc. If `FixedUpdate::True`, your node type must have an **inherent** method `fn internal_fixed_update(&mut self, api: &mut ScriptApi)` in its `impl` block; the macro-generated `BaseNode::internal_fixed_update` will call it when the flag is true. You can also implement the trait `NodeWithInternalFixedUpdate` for clarity.
   - **RenderUpdate** (`RenderUpdate::True` / `RenderUpdate::False`): Whether this node runs **internal render update** every frame (e.g. UI input). If `RenderUpdate::True`, your node type must have an inherent method `fn internal_render_update(&mut self, api: &mut ScriptApi)`; the macro will call it. You can also implement `NodeWithInternalRenderUpdate`.
   - **Renderable** (`Renderable::True` / `Renderable::False`): Whether this node is **drawn to the screen or interfaces with graphics (cameras)**. Only renderable nodes are added to `needs_rerender` and participate in the render pass; the macro uses this for `is_renderable()` and `NodeType::is_renderable()`.

   The macro also expands into **`impl_scene_node!`** for each type, which implements `BaseNode` for the concrete type (get_id, get_children, etc.) and uses the three flags for `is_renderable()`, `needs_internal_fixed_update()`, and `needs_internal_render_update()`. So: add your node struct under `nodes/`, then add one line like `MyNode(FixedUpdate::False, RenderUpdate::False, Renderable::True) => crate::nodes::my_node::MyNode` to `define_nodes!`.
2. **Engine registry** (`structs/engine_registry.rs`): Call `reg.register_node(...)` for the new `NodeType` with its fields (and base type). For any script-callable methods (including NodeSugar like `get_var`, `get_parent`), call `reg.register_node_methods(...)` with param types, return type, and a **NodeMethodRef**. If the method needs custom codegen, add a new variant to the `NodeMethodRef` enum and ensure it is registered in `method_ref_map` / `method_ref_reverse_map` / `method_defs`.
3. **Engine bindings** (`structs/engine_bindings.rs`): Implement `EngineMethodCodegen::to_rust_prepared` for every **NodeMethodRef** that has custom codegen (e.g. `GetVar`, `SetVar`, `GetParent`). This is where you emit the actual Rust string (e.g. `api.get_script_var_id(...)`).
4. **Node API per language**: Each language must map script-visible names to the same refs. Add an entry in **PUP** (`scripting/lang/pup/node_api.rs`), **TypeScript** (`scripting/lang/typescript/node_api.rs`), and **C#** (`scripting/lang/csharp/node_api.rs`): register the node’s fields (script name → `NodeFieldRef`) and methods (script name → `NodeMethodRef`). That way `node.get_parent()` in PUP and the equivalent in TS/CS all resolve to the same `NodeMethodRef` and binding.

So: **node registry (runtime) → engine registry (signatures + refs) → engine bindings (Rust output) → node_api for each language (syntax → ref).**

**Adding a new resource module or function** — follow the same four layers (logic → module → binding → frontend):

1. **Runtime / engine** (where the real logic lives): Ensure the engine or `scripting/api.rs` already exposes the behavior you need (e.g. signal connect/emit, texture load, array/map ops). Resource modules don’t have a single “resource api.rs”; runtime is spread across engine code and the script API. If something new is required, add or extend the runtime API first.
2. **Module and variant** (`scripting/resource_modules.rs`): If it’s a new resource type (e.g. a new “Audio” resource), add a variant to `ResourceModule` and a new enum (e.g. `AudioResource { Load, Play }`). If it’s an existing resource (e.g. Signal, Texture, Array), add a variant to that enum (e.g. `SignalResource::Disconnect`). The resource name is what scripts use (e.g. `Signal`, `Texture`, `Array`).
3. **Binding** (`scripting/resource_bindings.rs`): Implement `ModuleCodegen` and `ModuleTypes` for that variant: `param_types()`, `return_type()`, optional `param_names()`, and `to_rust_prepared(...)` that builds the Rust call string (e.g. `api.connect_signal_id(...)`, or whatever the engine/API expects). The binding only emits the call; argument conversion is handled by the framework.
4. **Wire syntax → module in each language**: In **PUP** (`scripting/lang/pup/resource_api.rs`), **TypeScript** (`scripting/lang/typescript/resource_api.rs`), and **C#** (`scripting/lang/csharp/resource_api.rs`), add resolution so that the language’s resource name + method name maps to your `ResourceModule` variant. For example in PUP: under the `Signal` resource, `"connect" => Some(ResourceModule::Signal(SignalResource::Connect))`. Each language can use different keywords (e.g. `emit` vs `fire`) as long as they resolve to the same variant.

So: **engine/runtime (logic) → resource_modules (enum variant) → resource_bindings (param/return types + Rust call string) → per-language resource_api (syntax => module).**

**Adding a new API module or function** — follow the same four layers (logic → module → binding → frontend):

1. **Implement the runtime function** (`scripting/api.rs`): Add or extend the sub-API struct (e.g. `JsonApi`, `TimeApi`) and implement the actual Rust function there. This is what runs at runtime; the generated script code will call it.
2. **Give it a module and variant** (`scripting/api_modules.rs`): If it’s a new module, add an `ApiModule` variant and a new enum (e.g. `MyApi`). If it’s an existing module, add a variant to that enum (e.g. `JSONApi::NewThing`). The module name is what scripts use (e.g. `JSON`, `Time`).
3. **Write the binding** (`scripting/api_bindings.rs`): Implement `ModuleCodegen` and `ModuleTypes` for that variant:
   - `param_types()`: expected argument types (for type inference and casting).
   - `return_type()`: return type.
   - `param_names()` (optional): script-side parameter names.
   - `to_rust_prepared(...)`: build the Rust code string that calls your API. Typically this is **`api.MODULE.function(args...)`** — e.g. `format!("api.json_parse({})", args_strs[0])` for `JSON.parse(text)`. The binding only emits the call; argument conversion is already done by the framework.
4. **Wire syntax → module in each language**: In **PUP** (`scripting/lang/pup/api.rs`), **TypeScript** (`scripting/lang/typescript/api.rs`), and **C#** (`scripting/lang/csharp/api.rs`), add resolution so that the language’s syntax (module name + function name) maps to your `ApiModule` variant. For example in PUP: `"parse" => Some(ApiModule::JSON(JSONApi::Parse))` under the `JSON` module. Each language can use different keywords (e.g. `log` vs `print`) as long as they resolve to the same module variant.

So: **api.rs (runtime impl) → api_modules (enum variant) → api_bindings (param/return types + `"api.MODULE.function()"`) → per-language api (syntax => module).**

## Version 0.1.0 & generated code (proof of concept)

Perro is at **0.1.0** and is intentionally a proof of concept. The generated Rust can be:

- **Inefficient** — More allocations or work than strictly necessary.
- **Verbose** — Extra temporaries, clones, or string operations that could be avoided.

If you can remove clones or string allocations when they are not needed (e.g. when a value is not used later) and can **deterministically show** it works across a variety of cases (including the test project), such optimizations are welcome. The codegen can be updated to emit leaner code as long as:

- `cargo run -p perro_core -- --tests --scripts` still **compile** and **behave correctly**.
- The change is justified by concrete cases and does not break existing scripts or tests.

Prefer changes that keep the test suite green and the generated code understandable; we can tighten allocations and reduce clones incrementally as the pipeline stabilizes.

## Quick rules (must follow)

- A PR must fix or directly implement an existing issue. For design changes open an issue or a draft PR first.
- In the PR description include:
  - Linked issue
  - What the issue was
  - What you changed to solve it
  - Why the change fits Perro's goals (performance, ease of use, engine mission)
- Ensure the repository and test projects build before opening a PR.

## Build & test locally (required for PRs)

From repository root:

- Build workspace:

```bash
cargo build --workspace
```

- Run tests:

```bash
cargo test --workspace
```

- Required: build the special test project that covers language edge-cases:

```bash
cargo run -p perro_core -- --tests --scripts
```

test_projects\test contains scripts exercising edge cases across languages; if it compiles, your changes most likely won't break user scripts. That build is mainly for **transpiler/compile-time validation**: valid frontend (PUP, etc.) should produce Rust that compiles (end users don't control generated code); runtime panics are a separate concern. Every PR must ensure one of the above succeeds.

## Formatting & linting (recommended)

- `cargo fmt --all` formats code to standard Rust style (keeps diffs consistent).
- `cargo clippy --all-targets --all-features -- -D warnings` runs a linter that finds common mistakes and style issues; fix warnings where reasonable.

You don't need to be an expert — run them before opening a PR to reduce review friction.

## Workflow

1. Fork the repo and create a branch:
   - feature: `feature/<short-desc>`
   - bugfix: `bugfix/<short-desc>`
   - docs: `docs/<short-desc>`
2. Make small, focused commits with clear messages.
3. Run build & test steps above (including building test_projects\test).
4. Push branch and open a PR against `main` (or the branch referenced by the issue).
5. In the PR body:
   - Link the issue the PR fixes.
   - Describe the problem, approach, and why this solution is correct.
   - Include a test plan and reproduction steps.
   - Attach example scripts / FUR files if relevant.

## PR checklist (must be satisfied)

- [ ] PR fixes or is linked to an issue.
- [ ] Branch builds: `cargo build --workspace`
- [ ] test_projects\test builds: `cargo run -p perro_core -- --tests --scripts`
- [ ] Tests pass: `cargo test --workspace`
- [ ] Formatted: `cargo fmt --all`
- [ ] Linted (recommended): `cargo clippy --all-targets --all-features -- -D warnings`
- [ ] PR description documents: issue, intent, implementation, and alignment with Perro's goals
- [ ] If scripting/transpiler changes: include sample input scripts and expected generated Rust output

## Tests & examples

- For transpiler or language work include small example scripts and their generated Rust output.
- If a change affects runtime behavior, add or update a minimal scenario in `test_projects/`.

## Reporting issues

Compiler or codegen errors can happen even when source syntax looks correct (see *Experimental status & compiler issues*). Reporting these with a minimal repro helps a lot.

When opening an issue include:

- Minimal reproduction steps
- Expected vs actual behavior
- Platform and Rust toolchain used
- Attach sample project/files when relevant

## Communication & large changes

- For large API or design changes open a proposal issue or draft PR and discuss before major work.
- Use GitHub Issues and PR comments for discussion.

## License

By submitting a PR you agree to license your contribution under the project's Apache 2.0 license.

Thank you — concise, tested contributions help Perro move faster.
