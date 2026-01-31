# TypeScript scripting reference (experimental)

**TypeScript support is experimental.** The TypeScript language integration with the Perro/Rust pipeline is **not feature-complete**. Use PUP for full scripting support; TS is for experimentation and gradual integration.

**Important:** TypeScript (and C#) **may not parse or support the same features as PUP.** Lifecycle hooks, signal shorthand, attributes, and other constructs may differ or be unsupported. The baseline is there for **contributors to experiment with**; the runtime is the same (transpiled Rust). Think of TS as a **frontend syntax** — be mindful of how constructs translate to Rust (closures, method calls, literals) when contributing or debugging.

For **concepts** (node hierarchy, script-side types, rotation as Quaternion, Texture/Mesh handles, resource vs module APIs), see [PUP.md](PUP.md). The engine semantics are the same; only the **syntax** differs.

---

## This, types, and naming

- **`this`** refers to the **node** (the engine node this script is attached to), like **`self`** in PUP. **Script variables are not accessed with `this`** — use them as plain names (e.g. `speed`, `health`). Use `this` only for node fields and methods (e.g. `this.texture`, `this.getNode(...)`).
- **Inheritance:** Use **`extends`** (e.g. `class Player extends Sprite2D`). Type annotations use **`:`** (e.g. `x: number`, `node: Sprite2D`).
- **camelCase** for methods and variables (e.g. `getNode`, `globalTransform`, `rotateX`, `load`). Type/class names are PascalCase (e.g. `Texture`, `Quaternion`).
- **Dynamic script access:** Use **`getVar`**, **`setVar`**, and **`call`** as methods on nodes (e.g. `node.getVar("speed")`, `node.setVar("speed", 5)`, `node.call("methodName", arg)`). These take strings and distinguish script vars/methods from native `.` access. **`call`** invokes a script method by name (same as PUP’s `node::method_name(args)`).
- **Globals / modules / root:** Equivalent of PUP’s `@global`, `@module`, `@root` may differ or be incomplete; see the codebase as support evolves.
- **Attributes:** Attribute or decorator support for script metadata is experimental.

**Example (TypeScript style):**

```ts
// this = the node only; script vars (speed) are plain names, no "this"
this.transform.position.x += speed * Time.getDelta();
let tex = Texture.load("res://icon.png");   // camelCase
let child = this.getNode("Enemy");          // native method
let cName = child.name                      // native field
let val = child.getVar("health");           // dynamic script access (string)
```

---

## Full script example (TypeScript)

Below is a **full TypeScript script** showing what a complete script looks like in TS. Lifecycle hook names and support may differ from PUP; this example uses method names that correspond to PUP’s `on init()` / `on update()` where the TS parser/codegen wires them.

```ts
class Player extends Sprite2D {
    speed: number = 5.0;
    health: number = 100;

    init(): void {
        this.texture = Texture.load("res://icon.png");
        this.name = "Player";
    }

    update(): void {
        let delta = Time.getDelta();
        this.transform.position.x += speed * delta;
    }

    takeDamage(amount: number): void {
        health -= amount;
        if (health <= 0) {
            Console.log("Player died");
        }
    }

    getChildHealth(): number {
        let child = this.getNode("HUD");
        if (child != null) {
            return child.getVar("health");
        }
        return 0;
    }
}
```

- **`class Player extends Sprite2D`** — script attached to a Sprite2D node; **`this`** is that node (like `self` in PUP).
- **Script variables** (`speed`, `health`) — use plain names, **not** `this.speed`; `this` is only for the node.
- **`this.texture`**, **`this.transform`**, **`this.name`** — node fields; camelCase in TS.
- **`Texture.load(...)`**, **`Time.getDelta()`** — resource and module APIs (camelCase).
- **`getNode("HUD")`**, **`getVar("health")`** — node methods; **`call("methodName", args)`** for dynamic script method invocation.
- Lifecycle: naming and wiring of `init()` / `update()` depend on the TS parser and codegen; they may not match PUP’s `on init()` / `on update()` exactly.

---

## Node API

Field names follow TS conventions (e.g. `texture`, `mesh`, `transform`, `globalTransform` if exposed). See [PUP.md](PUP.md) for the list of nodes and fields (Sprite2D.texture → Texture, MeshInstance3D.mesh → Mesh, Node3D.transform.rotation → Quaternion, etc.).

---

## Resource APIs 

Method names are **camelCase**

| Resource   | Methods (TS) |
|-----------|---------------|
| Signal    | `new` / `create`, `connect`, `emit`, `emitDeferred` / `emit_deferred` |
| Texture   | `load`, `preload`, `remove`, `createFromBytes` / `create_from_bytes`, `getWidth`, `getHeight`, `getSize` |
| Mesh      | `load`, `preload`, `remove` |
| Quaternion| (identity, fromEuler, fromEulerXyz, asEuler, rotateX, rotateY, rotateZ, rotateEulerXyz — exact names in TS resolver) |
| Shape2D   | rectangle, circle, square, triangle |
| Array     | push, insert, remove, pop, length/len/size, new, create |
| Map       | (insert, remove, get, contains, len, clear, new) |

---

## Module APIs (global)

Same modules as PUP (JSON, Time, OS, Console, Input, Math). Method names use **camelCase** where applicable (e.g. `getDelta`, `getPlatformName`). See the TypeScript API resolver in the codebase for the exact names.

---

## Rotation (same as PUP)

- **Transform3D.rotation** is a **Quaternion**. Do not mutate `.x`/`.y`/`.z` directly.
- Use **Quaternion** methods: `identity()`, `fromEulerXyz(...)`, `rotateX` / `rotateY` / `rotateZ`, `asEuler()`, etc. Instance calls on `this.transform.rotation` are normalized to the same resource API and written back when used as statements.

---

## Status

TS parsing and codegen exist for a subset of the API; not all PUP features or node/resource methods may be wired. For a complete reference, use [PUP.md](PUP.md) and the TypeScript language API files in `perro_core/src/scripting/lang/typescript/`.
