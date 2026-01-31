# PUP scripting reference

PUP is the primary scripting language for Perro. This doc is the reference for **what to expect** in PUP: script kinds (`@script`, `@global`, `@root`, `@module`), lifecycle hooks (`on init()`, `on update()`, `on fixed_update()`), signal shorthand (`on SIGNALNAME() { }`), dynamic access (`::`), types, node fields/methods, resource APIs (Texture, Mesh, Quaternion, Signal, etc.), and global modules (Time, Console, Math, …). PUP uses **snake_case** for API names and **`self`** for the current node.

---

## Script kinds and attributes

- **`@script Name extends NodeType`** — Script attached to a node (e.g. `Sprite2D`, `MeshInstance3D`). Use `self` for the node. The node is linked in the scene via **script_path** on the base Node (e.g. `res://scripts/player.pup`).
- **`@global Name`** — Global script (extends Node internally). The engine **automatically creates** a node for it and attaches the script; you do not add it to the scene. Use it like any other node: **`GlobalName`**, **`GlobalName.name`**, **`GlobalName::var_name`**, **`GlobalName::method(args)`**.
- **`@root`** — Script attached to the **singular game root** (NodeID 1). The engine **automatically attaches** this script to the root node when present; you do not add it to the scene. Use **`Root`** like any other node: **`Root`**, **`Root.name`**, **`Root::some_var`**, **`Root::some_fn(args)`**.
- **`@module Name`** — Module: **compile-time** constants and free functions only. No node, no `self`, no runtime instance. Use **`.`** to access constants and functions: **`ModuleName.CONST`**, **`ModuleName.function(args)`**.
- **Attributes:** Put **`@NAME`** before a top-level variable or function to add an attribute. **`@expose`** is special: it binds that variable to a value defined in the `.scn` file for the node this script is attached to (the script is attached by the node’s **script_path** in the scene).

**Difference in short:** **Script** = per-node, linked in the scene. **Global** and **Root** = auto-created / auto-attached; use **`GlobalName`** or **`Root`** and **`::`** for script vars and methods like any other node. **Module** = compile-time only; use **`.`** for constants and functions (no `::`, no node).

**Dynamic script access:** Use **`::`** to access script variables or call script methods on a node by name (strings). Example: `child_node::var_name`, `child_node::method_name(args)`, or `node::[expr]` for a dynamic name. Dot (`.`) is for native node fields and methods; `::` is for script-defined vars/functions.

**Example (PUP):**

```pup
@script Player extends Sprite2D
    @expose
    var speed = 5.0

    on update() {
        self.transform.position.x += self.speed * Time.get_delta()
    }

    on init() {
        self.texture = Texture.load("res://icon.png")
    }
```

Lifecycle hooks are `on init()`, `on update()`, `on fixed_update()`. They take no arguments; the engine calls them automatically.

### Full script example (PUP)

Below is a **full PUP script** showing script kind, attributes, lifecycle, node access, resources, and dynamic access. This is what a complete script looks like in PUP.

```pup
@script Player extends Sprite2D
    @expose
    var speed = 5.0
    var health: int = 100

    on init() {
        self.texture = Texture.load("res://icon.png")
        self.name = "Player"
    }

    on update() {
        var delta = Time.get_delta()
        self.transform.position.x += self.speed * delta
    }

    fn take_damage(amount: int) {
        health -= amount
        if health <= 0 {
            Console.print("Player died")
        }
    }

    fn get_child_health(): int {
        var child = self.get_node("HUD")
        if child != null {
            return child.get_var("health")
        }
        return 0
    }
```

- **`@script`** + **`extends Sprite2D`** — script attached to a Sprite2D node; **`self`** is that node.
- **`@expose var speed`** — value can be set in the scene (`.scn`) for this node.
- **`on init()`** / **`on update()`** — lifecycle; no parameters; engine calls them automatically.
- **`self.texture`**, **`self.transform`**, **`self.name`** — node fields and methods (snake_case in PUP).
- **`Texture.load(...)`**, **`Time.get_delta()`** — resource and module APIs; call directly.
- **`get_node("HUD")`**, **`get_var("health")`** — node methods; **`call("method_name", args)`** or **`node::method_name(args)`** for dynamic script access.

---

## Lifecycle and signals: what to expect

### Lifecycle hooks

- **`on init()`** — Called once when the node (and its script) is ready.
- **`on update()`** — Called every frame. Use for per-frame logic (e.g. movement, input).
- **`on fixed_update()`** — Called at a fixed timestep. Use for physics or deterministic logic.

None of these receive parameters. Call global modules directly (e.g. `Time.get_delta()`, `Console.print("hello")`).

### Signal shorthand: `on SIGNALNAME() { }`

To react to a **signal** (e.g. a button press, custom event), define a function whose name **exactly matches** the signal name using **`on`**:

```pup
on start_button_Pressed() {
    Console.print("button was pressed")
}

on PlayerDied() {
    Console.print("Player died somewhere")
}
```

**What to expect:** The compiler automatically connects that function to the signal at init. So when something emits the signal named `bob_Pressed`, the code inside `on bob_Pressed()` runs. You do not write `Signal.connect(...)` yourself for this case — the shorthand does it. Signal names are global strings (e.g. UI buttons often use `id_Pressed`, `id_Released`, `id_Hovered`).

### Manual signal connection and emit

If you need to connect or emit signals yourself:

- **Create / get a signal:** `var s = Signal.new("my_signal")` — the string is the global signal name. You can store `s` and reuse it.
- **Connect:** `Signal.connect("my_signal", FUNCTION_CALL)` connects the signal to a function **on this script** named `my_handler`. To connect to another node’s script function, use **`::`**: e.g. `Signal.connect("my_signal", other_node::other_handler)`.
- **Emit:** Call `s.emit()` on a Signal value, or emit by name (see Signal API below). Use `emit_deferred` when you need to emit from inside a callback that shouldn’t run listeners immediately.

So: **`on SIGNALNAME() { }`** = shorthand that both defines the handler and connects it to the signal named `SIGNALNAME`. For anything else (connecting to other nodes, emitting from code), use **Signal.new**, **Signal.connect**, and **Signal.emit** / **emit_deferred**.

---

## Script-side types (engine structs)

These are the types you see in PUP. They correspond to engine structs; some are ID handles (Texture, Mesh), others are value types (Vector2, Quaternion).

| PUP type    | Meaning / use |
|-------------|----------------|
| `Vector2`   | 2D position/size (x, y). |
| `Vector3`   | 3D position/direction (x, y, z). |
| `Transform2D` | 2D transform (position, rotation, scale). |
| `Transform3D` | 3D transform: `position` (Vector3), `rotation` (Quaternion), `scale` (Vector3). |
| `Quaternion` | 3D rotation. Use `Quaternion.identity()`, `from_euler_xyz`, `rotate_x`, `rotate_y`, `rotate_z`, `as_euler`, etc. |
| `Color`     | RGBA color. |
| `Rect`      | Rectangle. |
| `Texture`   | Texture handle (e.g. from `Texture.load("res://...")`). Used as `Sprite2D.texture`. |
| `Mesh`      | Mesh handle (e.g. from `Mesh.load("res://...")` or `Mesh.cube()`, `Mesh.sphere()`, etc.). Used as `MeshInstance3D.mesh`. |
| `Shape2D`   | 2D shape (Rectangle, Circle, Square, Triangle from `Shape.rectangle(...)` etc.). |

---

## Node API (what you can access on nodes)

Script **field** names are what you type in PUP (e.g. `texture`, `mesh`, `transform`). They map internally to Rust fields; the types below are the **script-side** types.

### Base Node (all nodes)

- **Fields:** `name` (string).
- **Methods:** `get_var(name)`, `set_var(name, value)`, `call(method_name, ...args)`, `get_node(name)`, `get_parent()`, `add_child(node)`, `clear_children()`, `get_type()`, `get_parent_type()`, `remove()`. Use **`call`** for dynamic script method invocation (e.g. `node.call("method_name", arg)`); same as `node::method_name(arg)`.

### Node2D (inherits Node)

- **Fields:** `transform` (Transform2D), `global_transform` (Transform2D), `pivot` (Vector2), `visible` (bool), `z_index` (int).

### Sprite2D (inherits Node2D)

- **Fields:** `texture` (Texture), `region` (optional rect).
- Use `Texture.load("res://path.png")` and assign to `self.texture`.

### ShapeInstance2D (inherits Node2D)

- **Fields:** `shape` (Shape2D, from `Shape.rectangle`, `.circle`, etc.), `color` (Color), `filled` (bool).

### Camera2D (inherits Node2D)

- **Fields:** `zoom`, `active` (bool).

### Node3D (inherits Node)

- **Fields:** `transform` (Transform3D), `global_transform` (Transform3D), `pivot` (Vector3), `visible` (bool).
- **Transform3D** has `position`, `rotation` (Quaternion), `scale`. Rotation is a **quaternion**; use Quaternion helpers, not raw `rotation.x += ...`.

### MeshInstance3D (inherits Node3D)

- **Fields:** `mesh` (Mesh). Set via `Mesh.load("res://...")` or assign a preloaded Mesh handle.
- The engine keeps a runtime mesh handle; you work with the script-side `Mesh` type.

### Camera3D (inherits Node3D)

- **Fields:** (camera-specific; see engine for fov, near, far, active if exposed).

### Lights (DirectionalLight3D, OmniLight3D, SpotLight3D)

- Inherit Node3D; color, intensity, range, angles etc. as defined in the engine.

### UINode (inherits Node)

- UI root; FUR and UI elements are managed separately.

---

## 3D rotation (Transform3D and Quaternion)

- On **Node3D** / **MeshInstance3D**, `self.transform` is a **Transform3D** with:
  - `position` (Vector3)
  - `rotation` (Quaternion) — **do not** do `self.transform.rotation.x += d`; that breaks the quaternion.
- **Correct way to rotate:**
  - Use **Quaternion** resource API on the rotation value:
    - `Quaternion.identity()`, `Quaternion.from_euler_xyz(pitch_deg, yaw_deg, roll_deg)`
    - `Quaternion.rotate_x(q, delta_deg)`, `rotate_y`, `rotate_z`, `rotate_euler_xyz(q, dp, dy, dr)`
    - `Quaternion.as_euler(q)` → Vector3 (degrees)
  - You can call these on the field itself; the compiler emits a writeback so the result is assigned back:
    - `self.transform.rotation.rotate_x(d)` → same as assigning the result back to `self.transform.rotation` (via mutation under the hood).

So: **rotation is a Quaternion on Transform3D**; use Quaternion methods (static or instance) and avoid touching quaternion components directly.

---

## Resource APIs (static and instance)

You can call these as **Type.method(...)** or on an **instance** (e.g. `self.texture` used like a Texture). The parser normalizes instance calls to the same resource module/binding.

### Signal

- **`Signal.new("signal_name")`** → Signal (handle). The string is the global signal name; use it to connect or emit.
- **`Signal.connect("signal_name", target)`** — Connect the signal to a handler. `target` can be a string (function name on this script) or `node::function_name` (script function on another node). The **`on SIGNALNAME() { }`** shorthand does this automatically for a function whose name matches the signal.
- **`signal.emit()`** / **`Signal.emit(signal, ...params)`** — Emit the signal so all connected handlers run.
- **`signal.emit_deferred()`** / **`Signal.emit_deferred(signal, ...params)`** — Emit at end of frame (use when emitting from inside a callback to avoid re-entrancy).

### Texture

- `Texture.load(path)` → Texture  
- `Texture.preload(path)` → Texture  
- `Texture.remove(texture)`  
- `Texture.create_from_bytes(bytes, width, height)` → Texture  
- `Texture.get_width(texture)`, `get_height(texture)`, `get_size(texture)`  

Script type **Texture** is the handle; e.g. `Sprite2D.texture` is of type Texture.

### Mesh

- `Mesh.load(path)` → Mesh — use `res://model.glb` if the file has one mesh; use `res://model.glb:0`, `res://model.glb:1`, … for multiple meshes (by index)  
- `Mesh.preload(path)` → Mesh  
- `Mesh.remove(mesh)`  
- **Primitives:** `Mesh.cube()`, `Mesh.sphere()`, `Mesh.plane()`, `Mesh.cylinder()`, `Mesh.capsule()`, `Mesh.cone()`, `Mesh.sq_pyramid()`, `Mesh.tri_pyramid()`  

Script type **Mesh** is the handle; e.g. `MeshInstance3D.mesh` is of type Mesh.

### Quaternion

- `Quaternion.identity()` → Quaternion  
- `Quaternion.from_euler(euler_deg: Vector3)` → Quaternion  
- `Quaternion.from_euler_xyz(pitch_deg, yaw_deg, roll_deg)` → Quaternion  
- `Quaternion.as_euler(q)` → Vector3 (degrees)  
- `Quaternion.rotate_x(q, delta_pitch_deg)` (and `rotate_y`, `rotate_z`) → Quaternion  
- `Quaternion.rotate_euler_xyz(q, dp, dy, dr)` → Quaternion  

Use these for 3D rotation; you can call them on `self.transform.rotation` and the result is written back.

### Shape

- `Shape.rectangle(width, height)`, `.circle(radius)`, `.square(size)`, `.triangle(base, height)` → Shape2D  

Used with ShapeInstance2D’s `shape` field.

### Array

- `push`, `append`, `insert`, `remove`, `pop`, `len`/`size`, `new`

### Map

- `insert`, `remove`, `get`, `contains`/`contains_key`, `len`/`size`, `clear`, `new`

---

## Global modules (no `api` on script side)

Modules like **Time**, **Console**, **Math** exist at top level. You call them directly; Examples: `Time.get_delta()`, `Console.print("hello")`, `Texture.load("res://icon.png")`.

### Console

- `Console.print` / `Console.log`, `Console.warn`, `Console.error`, `Console.info` — e.g. `Console.print("hello")`.

### Time

- `Time.get_delta`, `Time.sleep_msec`, `Time.get_unix_time_msec`

### JSON

- `JSON.parse`, `JSON.stringify`

### OS

- `OS.get_env`, `OS.get_platform_name`

### Input

- Input actions, controller, keyboard, mouse (e.g. `GetAction`, `IsKeyPressed`, `GetMousePosition`, etc. as defined in the Input API).

### Math

- `Math.random`, `Math.random_range(min, max)`, `Math.random_int(min, max)`  
- `Math.lerp(a, b, t)`, `Math.lerp_vec2`, `Math.lerp_vec3`, `Math.slerp` (for Quaternion)

---

## Summary

- **Nodes:** Use script field names (`texture`, `mesh`, `transform`, `rotation`, etc.) and the types listed above. Use **`::`** for dynamic script access (vars/methods by name); **`.`** for native node fields/methods.  
- **Rotation:** Always go through **Quaternion** (identity, from_euler_xyz, rotate_x/y/z, as_euler); never mutate quaternion components directly.  
- **Resources:** Same API whether you call `Texture.load(...)` or use a value from `self.texture`; same for Mesh, Quaternion, etc.  
- **Modules:** Time, Console, JSON, OS, Input, Math — call them directly (e.g. `Time.get_delta()`, `Texture.load("res://...")`). There is no `api` object on the script side.
