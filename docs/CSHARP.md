# C# scripting reference (experimental)

**C# support is experimental.** The C# language integration with the Perro/Rust pipeline is **not feature-complete**. Use PUP for full scripting support; C# is for experimentation and gradual integration.

**Important:** C# (and TypeScript) **may not parse or support the same features as PUP.** Lifecycle hooks, signal shorthand, attributes, and other constructs may differ or be unsupported. The baseline is there for **contributors to experiment with**; the runtime is the same (transpiled Rust). Think of C# as a **frontend syntax** — be mindful of how constructs translate to Rust (closures, method calls, literals) when contributing or debugging.

For **concepts** (node hierarchy, script-side types, rotation as Quaternion, Texture/Mesh handles, resource vs module APIs), see [PUP.md](PUP.md). The engine semantics are the same; only the **syntax** differs.

---

## This, types, and naming

- **`this`** refers to the **node** (the engine node this script is attached to), like **`self`** in PUP. **Script variables are not accessed with `this`** — use them as plain names (e.g. `Speed`, `Health`). Use `this` only for node fields and methods (e.g. `this.Texture`, `this.GetNode(...)`).
- Inheritance and type annotations use **`:`** (e.g. `class Player : Sprite2D`, `float speed`).
- **PascalCase** for methods and public API (e.g. `Load`, `Preload`, `GetNode`, `RotateX`).
- **Attributes:** Use C# attribute syntax **`[Name]`** before a member (e.g. `[Expose]` on a field).
- **Globals / modules / root:** Equivalent of PUP’s `@global`, `@module`, `@root` may differ or be incomplete; see the codebase as support evolves.

---

## Full script example (C#)

Below is a **full C# script** showing what a complete script looks like in C#. Lifecycle hook names and support may differ from PUP; this example uses method names that correspond to PUP’s `on init()` / `on update()` where the C# parser/codegen wires them.

```csharp
class Player : Sprite2D {
    public float Speed = 5.0f;
    public int Health = 100;

    public void Init() {
        this.Texture = Texture.Load("res://icon.png");
        this.Name = "Player";
    }

    public void Update() {
        float delta = Time.GetDelta();
        this.Transform.Position.X += Speed * delta;
    }

    public void TakeDamage(int amount) {
        Health -= amount;
        if (Health <= 0) {
            Console.Print("Player died");
        }
    }

    public int GetChildHealth() {
        var child = this.GetNode("HUD");
        if (child != null) {
            return child.GetVar("health");
        }
        return 0;
    }
}
```

- **`class Player : Sprite2D`** — script attached to a Sprite2D node; **`this`** is that node (like `self` in PUP).
- **Script variables** (`Speed`, `Health`) — use plain names, **not** `this.Speed`; `this` is only for the node.
- **`this.Texture`**, **`this.Transform`**, **`this.Name`** — node fields; PascalCase in C#.
- **`Texture.Load(...)`**, **`Time.GetDelta()`** — resource and module APIs (PascalCase).
- **`GetNode("HUD")`**, **`GetVar("health")`** — node methods; **`Call("MethodName", args)`** for dynamic script method invocation.
- Lifecycle: naming and wiring of `Init()` / `Update()` depend on the C# parser and codegen; they may not match PUP’s `on init()` / `on update()` exactly.
- Exact field names (e.g. `Position.X` vs `position.x`) and return types (e.g. `GetVar` may return `object` and require a cast) depend on the C# resolver and codegen.

---

## Node API (script-side)

Same node types and fields as PUP; field names follow C# conventions where exposed. See [PUP.md](PUP.md) for the list of nodes and fields (Sprite2D.texture → Texture, MeshInstance3D.mesh → Mesh, Node3D.transform.rotation → Quaternion, etc.). **Dynamic script access:** `GetVar(name)`, `SetVar(name, value)`, **`Call(methodName, ...args)`** — same as PUP’s `get_var`/`set_var`/`call` and `node::method_name(args)`.

---

## Resource APIs (C# naming)

Same resources as PUP; method names are **PascalCase**.

| Resource   | Methods (C#) |
|-----------|---------------|
| Signal    | `New` / `Create`, `Connect`, `Emit`, `EmitDeferred` |
| Texture   | `Load`, `Preload`, `Remove`, `CreateFromBytes`, `GetWidth`, `GetHeight`, `GetSize` |
| Mesh      | `Load`, `Preload`, `Remove` |
| Quaternion| (Identity, FromEuler, FromEulerXyz, AsEuler, RotateX, RotateY, RotateZ, RotateEulerXyz — exact names in C# resolver) |
| Shape2D   | Rectangle, Circle, Square, Triangle |
| Array     | Push / Add, Insert, Remove / RemoveAt, Pop, Length / Count, New, Create |
| Map       | Add / Insert / Set, Remove / Delete, Get / TryGetValue, … |

---

## Module APIs (global)

Same modules as PUP (JSON, Time, OS, Console, Input, Math). Method names use **PascalCase** (e.g. `GetDelta`, `GetPlatformName`). See the C# API resolver in the codebase for the exact names.

---

## Rotation (same as PUP)

- **Transform3D.rotation** is a **Quaternion**. Do not mutate components directly.
- Use **Quaternion** methods: `Identity()`, `FromEulerXyz(...)`, `RotateX` / `RotateY` / `RotateZ`, `AsEuler()`, etc. Instance calls on the rotation field are normalized to the same resource API and written back when used as statements.

---

## Status

C# parsing and codegen exist for a subset of the API; not all PUP features or node/resource methods may be wired. For a complete reference, use [PUP.md](PUP.md) and the C# language API files in `perro_core/src/scripting/lang/csharp/`.
