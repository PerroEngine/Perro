# Transpiler: what to expect

This doc describes how PUP (and other script languages) are transpiled to Rust: type mapping, identifier renaming, and when the compiler introduces temporary reads. **End users** only need to know that their script compiles under the hood and that the **source code is the source of truth**; the generated Rust exists so the engine can run the script and is not something you have to read or edit. For **debugging** or contributor work, inspecting the emitted Rust can help see what the transpiler did.

---

## Pipeline overview

The same idea used for **syntax → module → binding → real Rust** (see CONTRIBUTING.md) applies to **types**:

- **Script-side type** — What you write in PUP: `Node2D`, `Texture`, `Signal`, `Mesh`, `int`, `string`, custom struct names, etc.
- **TypeAST** — Internal `Type` (and engine struct / node type) used by the compiler.
- **Real Rust** — The type that actually appears in the generated Rust (e.g. `NodeID`, `SignalID`, `Option<TextureID>`).

The transpiler tries to **preserve the meaning** of your script and only renames or rewrites where needed so the result compiles and doesn’t collide with internal names. If it didn’t do this, it would have to invent a lot of arbitrary IR; the renames and type mappings are predictable and documented below.

---

## Type mapping: Script-side → Real Rust

Types that represent **handles/IDs** at runtime become `*ID` in Rust and get consistent naming.

| Script-side type | TypeAST (internal) | Real Rust type |
|------------------|--------------------|-----------------|
| `Node2D`, `Sprite2D`, `Node`, etc. | `Type::Node(NodeType)` | `NodeID` |
| `Node` (dynamiclly returned from get_node() or get_parent()) | `Type::DynNode` | `NodeID` |
| `Texture` | `Type::EngineStruct(Texture)` | `Option<TextureID>` |
| `Mesh` | `Type::EngineStruct(Mesh)` | `Option<MeshID>` |
| `Signal` | `Type::Signal` | `SignalID` |
| Custom struct (user-defined) | `Type::Custom(name)` | `__t_Name` (see below) |
| Primitives, `Vector2`, `Transform3D` etc. | Various | Same or similar to Rust (e.g. `i32`, `f32`, `Vector2`, `Transform3D`) |

So: **`var b: Signal`** → Rust field type **`SignalID`**. **`var n: Node2D`** → **`NodeID`**. **`var tex: Texture`** → **`Option<TextureID>`**. The rule is: **script-side types that are “handle” types become `*ID` (or `Option<*ID>`) in Rust.**

---

## Identifier renaming

To avoid clashing with Rust keywords and engine internals, the transpiler renames script identifiers in a consistent way.

### Variables whose type becomes an ID

For **variables** whose type maps to an ID type (nodes, textures, meshes, signals), the **variable name** gets an **`_id`** suffix in Rust:

- `var n: Node2D` → `n_id: NodeID`
- `var tex: Texture` → `tex_id: Option<TextureID>` (or similar)
- `var mesh: Mesh` → `mesh_id: Option<MeshID>`
- `var sig: Signal` → `sig_id: SignalID`

So: **things that become IDs get affixed with `NAME_id`** (the name you gave the variable, then `_id`). If the name already ends with `_id`, it is left as-is.

### Other variables and functions

All other script variables and function names are prefixed with **`__t_`** so they don’t collide with internal names:

- `var health: int` → field `__t_health: i32`
- `fn take_damage(...)` → `__t_take_damage` (except reserved lifecycle names, see below)

So: **other variables and functions become `__t_NAME`**.

Reserved function names like **`init`**, **`update`**, **`fixed_update`** are **not** prefixed, so they stay as the lifecycle hooks the engine expects.

### Custom types (user structs)

User-defined struct types (custom types in the script) are represented in Rust with the **`__t_`** prefix on the **type name** (e.g. `__t_MyStruct`), so script-side type names don’t clash with engine or standard names.

### Summary

- **ID-like variables:** `name` → `name_id` (e.g. nodes, textures, meshes, signals).
- **Other vars/functions:** `name` → `__t_name`.
- **Custom type names:** `Name` → `__t_Name`.
- **`self`** is special: it becomes `self.id` (the current node’s `NodeID`) where needed.

---

## Temporary reads when calls can’t be composed

Sometimes an expression would require **nesting one API call inside another** in a way that doesn’t fit Rust (e.g. borrowing or evaluation order). In those cases the transpiler emits a **temporary variable**, assigns the inner result to it, then uses that variable in the outer call.

- Example: a node method that returns a `Node` (e.g. `get_node("x")`) is used as the receiver of another call. The inner call is generated as something like `let __temp_api_<hash>: NodeID = ...;` and the outer call uses that variable.
- These names are internal (e.g. **`__temp_api_<hash>`**) and are only there so the generated code is valid Rust. You don’t need to name or reference them in script; they’re an implementation detail of “we couldn’t compose this as a single expression.”

So: **temp reads** are used when we can’t compose an API call inside another one; the behavior of your script is preserved, and the source code remains the single place that defines behavior.

---

## What you need to know in practice

- **The code compiles under the hood.** The transpiler turns your script into Rust that the engine runs; you don’t run or edit that Rust yourself in normal use.
- **Context is preserved.** Renames (`_id`, `__t_`) and type mappings (e.g. `Signal` → `SignalID`) are done so the meaning of your script is kept and names don’t collide.
- **Source code is sufficient.** For end users (and in the eventual editor), the **script source is the source of truth**. If it compiles, you don’t have to look at the generated Rust.
- **Debugging:** If something behaves oddly, looking at the generated Rust (e.g. in the project’s `.perro/scripts` output) can help see what the transpiler produced (e.g. which temps were introduced, how a type was lowered). In normal workflow, “it compiles” is enough.

---

## Reference: type pipeline (reminder)

Same mental model as **Syntax → Module → Binding → RealRust** for API/resource calls:

- **ScriptSideType** → what you write (`Node2D`, `Signal`, `Texture`, `Mesh`, custom types, primitives).
- **TypeAST** → internal `Type` (and engine/node types) used during compilation.
- **RealRust** → the type in the generated code (`NodeID`, `SignalID`, `Option<TextureID>`, `Option<MeshID>`, `__t_MyStruct`, etc.).

So e.g. **`var b: Signal`** is ScriptSideType **Signal** → TypeAST **`Type::Signal`** → RealRust **`SignalID`**. Same idea for nodes, textures, and meshes: script-side handle types become the corresponding `*ID` (or `Option<*ID>`) in Rust.
