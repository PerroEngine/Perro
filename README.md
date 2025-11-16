# 🐕 Perro Game Engine

**Perro** is an experimental, open-source game engine written in **Rust**, designed as a modern alternative to engines like Unreal, Godot, and Unity.

It focuses on **performance, flexibility, and ease of use** with a unique multi-language scripting system:

- 🐶 **Pup DSL** – a beginner-friendly, lightweight scripting language that compiles to Rust for native performance.
- 🎨 **FUR (Flexible UI Rules)** – a declarative UI system with layouts, panels, and boxing for easy UI design.
- 🌐 **Multi-Language Scripts** – write gameplay in **Pup**, **C#**, **TypeScript**, or **pure Rust** — everything transpiles to Rust under the hood.
- 📦 **Type-Safe Transpilation** – full type checking and casting during code generation.
- ⚡ **Optimized Release Builds** – scripts and assets statically link into your final binary.
- 🔗 **Cross-Script Communication** – global signal system + accessing functions and variables on scripts attached to other nodes.

---

## 👩‍💻 For Game Developers

### Quick Start

**Clone the repository and build from source:**

```bash
git clone https://github.com/PerroEngine/Perro.git
cd perro
cargo run -p perro_dev
```

This launches the **Perro Editor** in dev mode.

### Making Your First Game

1. **Create a new project** via the Project Manager
2. **Write scripts** in Pup, C#, TypeScript, or Rust
3. **Design UI** with FUR
4. **Hit Play** — Perro automatically:
   - Transpiles your scripts → Rust
   - Compiles Rust → DLL (dev mode)
   - Hot-loads the DLL into the running game
5. **Make changes** → recompile (~1–3s on decent hardware) → see updates instantly

---

## 🐶 Pup DSL

**Pup** is Perro's built-in scripting language — simple, readable, and compiles to Rust.

Currently supports **variables, functions, and cross-script communication**:

```pup
@script Player extends Sprite2D
    var speed = 7.5
    var is_moving = false

    fn init() {
        print("Player is ready!")
    }

    fn set_speed(new_speed: float) {
        speed = new_speed
    }

    fn update() {
        var delta = Time.get_delta()
        self.position.x += speed * delta
    }
```

---

## 🌐 Multi-Language Scripting

You can write scripts in multiple languages. Languages using **Tree Sitter** for parsing have their full syntax supported:

- **Pup** (native DSL, hand-written parser)
- **C#** (full syntax via Tree Sitter CST → Perro AST; not all AST bindings implemented yet)
- **TypeScript** (planned, same Tree Sitter pipeline)
- **Rust** (direct, no transpilation)

The transpilation pipeline:

1. **Parse** – Tree Sitter CST → Perro AST (or native parser for Pup)
2. **Codegen** – AST → type-checked Rust
3. **Compile** – Rust → DLL (Dev) or static binary (Release)
4. **Load** – DLL hot-load (Dev) or direct calls (Release)

---

## 🎨 FUR (Flexible UI Rules)

**FUR** is Perro's declarative UI system for building layouts and UI panels.

```fur
[UI]
    [Panel bg=sea-5 padding=4]
        [Text font-weight=bold text-color=white text-size=xl]
            Hello Perro!
        [/Text]
    [/Panel]
[/UI]
```

**Current Features:**

- Layouts and child layouts
- Panels and boxing
- Styling and padding

See `perro_editor/res/fur` for real examples of FUR in use.

---

## 🔄 Dev vs Release

### Dev Mode (Hot-Reload via DLL)

- Scripts are transpiled to Rust, compiled into a **DLL**
- Engine loads the DLL at runtime
- Make changes → recompile (~1–3s) → see updates instantly without restarting

### Release Mode (Static Linking)

- All scripts transpile → Rust
- Statically linked into final binary
- **Result:**
  - Single executable (no DLLs, no source included)
  - Optimized machine code from LLVM
  - Your source scripts are protected

---

## 🛠️ For Engine Contributors & Development

This repository contains the **Perro engine source code**. To build and work on the engine itself:

### Prerequisites

- **Rust** (GNU preferred as that's what ships with the editor binary for compilation)
- **Cargo**

### Repository Structure

```
perro/
├── perro_core/          # Core engine (structs, scene, render graph)
├── perro_dev/           # Dev wrapper binary (loads DLLs, runs projects with --path)
├── perro_editor/        # Editor game project
│   ├── .perro/
│   │   ├── project/     # Editor project crate
│   │   └── scripts/     # Editor scripts crate (contains transpiled rust + builds DLL)
│   └── res/             # Resources (FUR files, scenes, assets, scripts)
└── examples/            # Example game projects
```

### Building & Running

**Open the Editor in Dev Mode:**

```bash
cargo run -p perro_dev
```

This:

1. Compiles `perro_core`
2. Compiles `perro_editor/scripts` → DLL
3. Runs the editor with hot-loadable scripts

**Build the Core Alone:**

```bash
cargo build -p perro_core
```

All projects share a build cache (the main workspace target/ in source mode), so the core only compiles once.

### Toolchain & Versioning

The editors are pinned to specific versions of the toolchain, (eg. 1.0 => 1.90.0), toolchains will not be updated each engine update, as to not clog the end user's system with multiple toolchains they don't need. (1.0 and 1.1 could support the same toolchain, even if users update it only is installed once)

**Project Compatibility:**

- Old projects use their original editor version by default
- The Project Manager auto-updates to the latest version
- You can manually upgrade a project to a newer editor version if desired
- Older editor versions remain available for projects that haven't upgraded

### Stabilized Features

- ✅ Scripting system (Pup, C# via Tree Sitter CST)
- ✅ Signal system & cross-script communication
- ✅ Type checking and casting during Rust codegen
- ✅ C# → Rust transpilation (Tree Sitter → AST → codegen)
- ✅ DLL loading & dynamic script loading
- ✅ FUR layouts, panels, child layouts, and boxing

### In Progress / Planned

- 🔄 Pup DSL expansion (control flow, standard library)
- 🔄 C# AST bindings completion
- 🔄 TypeScript support (Tree Sitter pipeline)
- 🔄 FUR runtime editing & editor viewer
- 📋 Scene editor
- 📋 Asset pipeline

---

## 🤝 Contributing

Contributions are welcome! You can work on:

- **Engine** – `perro_core` (rendering, scene, runtime)
- **Editor** – Edit the source code and UI of the editor at `perro_editor/res`
- **Scripting** – Pup DSL expansion, transpiler improvements, new language support as needed
- **Tooling** – build system, asset pipeline

See [CONTRIBUTING.md](CONTRIBUTING.md) for guidelines.

---

## 📜 License

Perro is licensed under the **Apache 2.0 License**. See [LICENSE](LICENSE) for details.

---

## 🐾 Why "Perro"?

Every developer needs a loyal partner, just like a dog — and that's what Perro means in Spanish.
