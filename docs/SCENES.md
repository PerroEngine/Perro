# Building scenes from source (.scn files)

Scene files (`.scn`) are **JSON** and define the node tree for a scene. There is **no visual editor** yet — you build scenes by editing these files. This doc describes the syntax and how each node composes its ancestor (naming, parenting, and script attachment live on the **base Node** level).

---

## Top-level structure

Every `.scn` file has:

| Field     | Type   | Description |
|-----------|--------|-------------|
| `root_id` | number | Scene key (ID) of the root node. Must match a key in `nodes`. |
| `nodes`   | object | Map of **scene key** (string number, e.g. `"0"`, `"123"`) → node object. |

**Scene keys** are the numeric IDs you use to identify nodes in the file. They are strings in JSON (`"0"`, `"1"`, `"4"`). The **root** node is the one whose key equals `root_id`. Every other node should reference its **parent** by that parent’s scene key, the scene key is not the runtime id, that's derived at runtime, but parent child relationships as well as "exposed node variable references" are preserved.

---

## Node composition: base chain

Each node type **composes its ancestor**. The concrete type (e.g. `Sprite2D`, `Camera3D`) has a `base` field that is the next type up the hierarchy; that type has its own `base`, and so on until the innermost **`Node`**.

- **Naming**, **parenting**, and **script attachment** are all on the **innermost base** — the **Node**.
- Type-specific data (transform, texture, shape, etc.) live on the node type that adds them (Node2D, Node3D, Sprite2D, etc.).

**Chain examples:**

- **Node** (root or plain node): only `type: "Node"` and base fields (name, parent, script_path, …).
- **Node2D**: `type: "Node2D"`, `base: { type: "Node", name, parent, … }`, plus `transform`, `z_index`, etc.
- **Sprite2D**: `type: "Sprite2D"`, `texture_path`, `base: { type: "Node2D", base: { type: "Node", name, parent, script_path }, transform, … }`.
- **Node3D**: `type: "Node3D"`, `base: { type: "Node", name, parent, … }`, plus `transform` (3D).
- **MeshInstance3D**: `type: "MeshInstance3D"`, `mesh_path`, `base: { type: "Node3D", base: { type: "Node", … }, transform }, …`. Use `mesh_path`: `"res://model.glb"` if the file has one mesh; if it has multiple, use `"res://model.glb:0"`, `"res://model.glb:1"`, etc. (index, not internal names).

So: **all naming, parenting, and script attachment are done on the base Node level** (the innermost `base` in the chain).

---

## Base Node fields (naming, parenting, scripts)

The **innermost** `base` object has `type: "Node"` and these fields:

| Field              | Type   | Description |
|--------------------|--------|-------------|
| `type`             | string | Must be `"Node"`. |
| `name`             | string | Display name (e.g. for `get_node("name")`). |
| `parent`           | number | **Scene key** of the parent node. Omit for the root node. |
| `script_path`      | string | Optional. Path to script file (e.g. `"res://player.pup"`). Attaches the script to this node. |
| `script_exp_vars`  | object | Optional. **Exposed script variables.** Map of **variable name** (string) → **value** (number, string, bool, array, object). For **node references** (e.g. `@expose var n: Node2D`) use **`{"@node": <scene_key>}`** so the engine does not treat plain numbers as scene keys. Applied before `init()` runs. |
| `is_root_of`       | string | Optional. Path to another scene file (e.g. `"res://second.scn"`). This node acts as the root of that sub-scene. |

- **Scene root node:** the node whose key is `root_id`.
- **Children:** set `parent` to the scene key of the parent (e.g. `"parent": 72`).
- **Scripts:** set `script_path` on the Node that should run the script; ensure the node type matches the script’s `extends` type (e.g. `"script_path": "res://player.pup"`).
- **Exposed script variables:** in the script, use **`@expose`** on a variable (e.g. `@expose var speed = 5.0`). Then in the scene, set **`script_exp_vars`** on the **base Node** to an object: keys = variable names (strings), values = JSON values. At runtime the engine converts each name to an ID, passes the map to the script’s **APPLY_TABLE**, and runs that **before** `on init()` so `init()` sees the values.

### Exposed script variables example

**Script (e.g. `player.pup`):**

```pup
@script Player extends Sprite2D
    @expose
    var speed = 5.0
    @expose
    var time = 0

    on init() {
        self.texture = Texture.load("res://icon.png")
    }
    on update() {
        self.transform.position.x += speed * Time.get_delta()
    }
```

**Scene (base Node with `script_exp_vars`):** set values per instance; keys are the **variable names** (strings), values are JSON (number, string, bool, array, object).

```json
"base": {
  "type": "Node2D",
  "base": {
    "type": "Node",
    "name": "Player",
    "parent": 72,
    "script_path": "res://player.pup",
    "script_exp_vars": {
      "speed": 10.0,
      "time": 100
    }
  },
  "transform": { "position": [100.0, 0.0], "rotation": 0.0, "scale": [1.0, 1.0] }
}
```

At runtime the engine parses `script_exp_vars` (VARNAME → VALUE), hashes each variable name to a u64, and calls the script’s `apply_exposed` with that map so the script’s APPLY_TABLE sets each exposed field before `on init()` runs.

### Exposed node references

If an exposed variable is a **node type** (e.g. `@expose var target: Node2D`), you must use an explicit **node reference** in the scene so the engine can tell it apart from plain numbers. Plain numbers like `time: 100` are never treated as node references (so `100` stays an f32, not scene key 100).

Use the object form **`{"@node": <scene_key>}`** where `<scene_key>` is the scene key of the target node (the same numeric ID used in `nodes` and `parent`).

**Example:** script has `@expose var target: Node2D`; in the scene, the node you want is stored under scene key `4`. Then set:

```json
"script_exp_vars": {
  "speed": 10.0,
  "time": 100,
  "target": { "@node": 4 }
}
```

- On **load**, the engine replaces `{"@node": 4}` with that scene node’s NodeID for proper referencing at runtime

---

## Node2D and transform

If the node type is **Node2D** (or extends it: Sprite2D, Area2D, Camera2D, etc.), the **Node2D** part of the base chain has:

- **`base`** — the inner Node (name, parent, script_path).
- **`transform`** — optional object:
  - `position`: `[x, y]`
  - `rotation`: number (degrees)
  - `scale`: `[x, y]`
- **`z_index`** — optional number (default 0).

Example Node2D base:

```json
"base": {
  "type": "Node2D",
  "transform": {
    "position": [100.0, 0.0],
    "rotation": 25.0,
    "scale": [0.5, 0.5]
  }
  "base": {
    "type": "Node",
    "name": "Player",
    "parent": 72,
    "script_path": "res://player.pup"
  },
}
```

---

## Node3D and transform

If the node type is **Node3D** (or Camera3D, MeshInstance3D, lights, etc.), the **Node3D** part has:

- **`base`** — the inner Node.
- **`transform`** — optional object:
  - `position`: `[x, y, z]`
  - `rotation`: `[x, y, z, w]` (quaternion)
  - `scale`: `[x, y, z]`

Example Node3D base:

```json
"base": {
  "type": "Node3D",
  "transform": {
    "position": [0, 0, -3],
    "rotation": [0, 0.2, 0, 1],
    "scale": [1, 1, 1]
  },
  "base": { "type": "Node", "name": "Cube", "parent": 0, "script_path": "res://threed.pup" }
}
```

---

## Type-specific fields (per node type)

These sit **next to** `type` and `base` on the node object (not inside base).

| Node type           | Extra fields |
|---------------------|--------------|
| **Sprite2D**        | `texture_path`: string (e.g. `"res://icon.png"`) |
| **MeshInstance3D**  | `mesh_path`: string — `"res://model.glb"` (single mesh), `"res://model.glb:0"` / `"res://model.glb:1"` (multiple meshes by index), or `"__cube__"` / `"__sphere__"` for built-ins |
| **CollisionShape2D**| `shape`: e.g. `{ "Rectangle": { "width": 200, "height": 1000 } }` or `{ "Circle": { "radius": 120 } }` |
| **ShapeInstance2D** | `shape`, `color`: `{ "r", "g", "b", "a" }`, `filled`: bool |
| **Camera2D**        | `active`: bool |
| **Camera3D**        | `active`: bool |
| **UINode**          | `fur_path`: string (e.g. `"res://ui.fur"`) |
| **Lights (3D)**     | `color`, `intensity`, `range`, etc. as defined by the engine |

---

## Minimal example (2D root + child)

```json
{
  "root_id": 0,
  "nodes": {
    "0": {
      "type": "Node2D",
      "base": {
        "type": "Node",
        "name": "World"
      }
    },
    "1": {
      "type": "Sprite2D",
      "texture_path": "res://icon.png",
      "base": {
        "type": "Node2D",
        "base": {
          "type": "Node",
          "name": "Player",
          "parent": 0,
          "script_path": "res://player.pup"
        },
        "transform": {
          "position": [100.0, 0.0],
          "rotation": 0.0,
          "scale": [1.0, 1.0]
        }
      }
    }
  }
}
```

- Root is node `0` (Node2D, name `"World"`, no parent).
- Node `1` is a Sprite2D; its **base Node** has `name`, `parent`: `0`, and `script_path`. So naming, parenting, and script attachment are all on the base Node.

---

## 3D example (world, camera, mesh, light)

```json
{
  "root_id": 0,
  "nodes": {
    "0": {
      "type": "Node3D",
      "base": { "type": "Node", "name": "World" }
    },
    "1": {
      "type": "Camera3D",
      "active": true,
      "base": {
        "type": "Node3D",
        "transform": { "position": [0, 0, 0], "rotation": [0, 0, 0, 1], "scale": [1, 1, 1] },
        "base": { "type": "Node", "name": "Camera", "parent": 0 }
      }
    },
    "2": {
      "type": "MeshInstance3D",
      "mesh_path": "__cube__",
      "base": {
        "type": "Node3D",
        "transform": { "position": [0, 0, -3], "rotation": [0, 0.2, 0, 1], "scale": [1, 1, 1] },
        "base": { "type": "Node", "name": "Cube", "parent": 0, "script_path": "res://threed.pup" }
      }
    },
    "3": {
      "type": "OmniLight3D",
      "color": { "r": 255, "g": 255, "b": 255, "a": 255 },
      "intensity": 1.0,
      "range": 10.0,
      "base": {
        "type": "Node3D",
        "transform": { "position": [0, 0, 0], "rotation": [0, 0, 0, 1], "scale": [1, 1, 1] },
        "base": { "type": "Node", "name": "Light", "parent": 0 }
      }
    }
  }
}
```

---

## Sub-scene root (`is_root_of`)

A node can act as the **root of another scene**: the engine loads that scene and parents its **children** under this node, and **does not instantiate** the sub-scene file's own root. So the node with `is_root_of` effectively **overrides** the sub-scene's root. On the **base Node**, set `is_root_of` to the path of that scene (e.g. `res://second.scn`). The node's **type** should match the root node type of the sub-scene (e.g. Node2D for a Node2D root).

**Parent scene (e.g. main.scn)** — node that "hosts" the sub-scene:

```json
"7": {
  "type": "Node2D",
  "base": {
    "type": "Node",
    "name": "SceneWithRoot",
    "is_root_of": "res://second.scn",
    "script_path": "res://p2.pup",
    "parent": 72
  }
}
```

**Sub-scene file (e.g. second.scn)** — the file pointed to by `is_root_of`; it has its own root and children:

```json
{
  "root_id": 172,
  "nodes": {
    "172": {
      "type": "Node2D",
      "base": {
        "type": "Node",
        "name": "Node2DWorld"
      }
    },
    "13": {
      "type": "Sprite2D",
      "texture_path": "res://icon.png",
      "base": {
        "type": "Node2D",
        "base": {
          "type": "Node",
          "name": "Sprite",
          "parent": 172
        },
        "transform": {
          "position": [0.0, 0.0],
          "rotation": 0.0,
          "scale": [0.1, 0.1]
        }
      }
    }
  }
}
```

**What happens at runtime**

1. The engine loads `res://second.scn` when it encounters the node with `is_root_of: "res://second.scn"` (e.g. **SceneWithRoot**).
2. The **sub-scene's root node** (here, key `172` / `Node2DWorld`) is **not** instantiated. The node that has `is_root_of` (**SceneWithRoot**) is treated as the root of that scene.
3. All **children** of the sub-scene's root (here, the Sprite key `13`) are instantiated and reparented so their **parent** is **SceneWithRoot**.
4. Result in the runtime tree: **SceneWithRoot** has the Sprite as a direct child. The sub-scene file's root (Node2DWorld) does not appear — SceneWithRoot effectively overrides it.

So: the `is_root_of` node **replaces** the sub-scene's root; the sub-scene's content is instanced **under** that node.

---

## UI root (UINode)

A **UINode** has no Node2D/Node3D; it has `type: "UINode"`, `fur_path`, and a base Node:

```json
"12": {
  "type": "UINode",
  "fur_path": "res://ui.fur",
  "base": {
    "type": "Node",
    "name": "UI",
    "parent": 9
  }
}
```

---

## More examples in the repo

- **`projects/MessAround/res/main.scn`** — 2D world, Area2D, CollisionShape2D, ShapeInstance2D, Sprite2D with script, Camera2D, sub-scene root, UINode.
- **`projects/ThreeD/res/main.scn`** — Node3D world, Camera3D, MeshInstance3D, OmniLight3D.
- **`projects/MessAround/res/crazy.scn`** — Minimal UINode-only scene.
- **`projects/MessAround/res/second.scn`** — Simple 2D scene with one Sprite2D child.

Use these as reference when building or debugging your own `.scn` files.
