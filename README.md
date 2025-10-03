# 🐕 Perro Game Engine

**Perro** is an experimental, open-source game engine written in **Rust**, designed
as a modern alternative to engines like Godot and Unity.

It focuses on **performance, flexibility, and developer freedom**, while introducing
unique ideas that make game development faster and more enjoyable:

- ⚡ **Managed Runtime** – no Rust installation required. Just download Perro and start making games.
- 🐶 **Pup DSL** – a lightweight scripting language that compiles to Rust, giving you native performance with a clean, approachable syntax.
- 🎨 **FUR (Flexible UI Rules)** – a declarative UI system inspired by XAML/JSX, with Tailwind-style utility classes for styling.
- 🏎 **Static Release Builds** – Pup scripts are compiled away into optimized machine code, giving you a **10–25% performance uplift** and extra protection in final builds.
- 🛠 **Rust-first core** – safe, fast, and modern systems programming under the hood, but hidden from game developers.

---

## 👩‍💻 For Game Developers

Game developers never need to install Rust or manage compilers.  
Perro provides a **managed runtime** that handles everything automatically.

### Quick Start

1. **Download Perro**

2. **Open the Editor**  
   Run `Perro.exe` (or the platform equivalent).

   - You’ll see the **Project Manager**.
   - Create or open a project to start editing.

3. **Make a Game**
   - Write gameplay in **Pup DSL** or your language of choice.
   - Design UI with **FUR**.
   - Hit **Play** in the editor — Perro automatically:
     - Transpiles Pup → Rust
     - Compiles Rust → DLL (in dev)
     - Hotloads the DLL into the running game

👉 **You never need Rust installed.** The editor and runtime handle everything for you.

---

## 🐶 Pup DSL

**Pup** is Perro’s scripting language.  
It is designed to be concise and readable, while compiling directly into **Rust** for native performance.

- Familiar, high-level syntax
- Compiles to Rust, then to a native DLL (in dev mode)
- Hot-reload support for instant iteration
- Safe by design, leveraging Rust’s guarantees

### Example

```pup
extends Sprite2D
    let speed = 7.5

    fn init() {
        print("Player is ready!")
    }

    fn update(delta: float) {
        if input.is_key_down("Left") {
          self.position.x -= speed * delta
        }
        if input.is_key_down("Right") {
          self.position.x += speed * delta
        }
    }
```

---

## 🎨 FUR (Flexible UI Rules)

**FUR** is Perro’s declarative UI system, inspired by XAML and JSX, with styling conventions similar to Tailwind.

- Attributes use `=` for values
- Spaces in names are replaced with `-`
- Styles are composable and utility-driven

### Example

```fur
[UI]
    [Panel bg=sea-5 padding=4]
        [Text font-weight=bold text-color=white text-size=xl]
          Hello Perro!
        [/Text]
    [/Panel]
[/UI]
```

---

## ⚡ Fast Iteration

Perro is designed for **rapid iteration**:

- Script compilation >> game start in **1–3 seconds!** in DEV
- Make a change to gameplay or UI → see it instantly when you hit play

If you don’t change scripts and just want to test?  
**Startup is literally instantaneous** due to caching.

---

## 🏗️ Static Compilation of Gameplay Scripts

When you hit **Release Build**, Perro shifts gears:  
all Pup scripts are **transpiled → Rust → optimized → machine code** and **statically linked into the engine binary**.

That means:

- 🚫 **No loose scripts or DLLs** — Script code is compiled away.
- 🔒 **Secure by default** — the shipped executable contains no copy of your logic in source form.
- ⚡ **Real performance uplift** — release builds run **10–25% faster** than dev builds:
  - Scripts are **inlined** into the engine core
  - Rust + LLVM optimizations kick in
  - No dynamic DLL lookup overhead
- 🐾 **One-binary output** — ship a single executable. No interpreters, no add-ons, no runtime baggage.

It’s the best of both worlds: **dynamic hot reload in dev,** and **blazing-fast, secure static builds in release.**

---

### 🔍 Example: Static Script Registry

During project compilation, Perro auto-generates a central registry that integrates scripts into the engine core:

```rust
use perro_core::script::{CreateFn, Script};
use std::collections::HashMap;

// Example user scripts
pub mod player;
pub mod enemy;
pub mod ui_mainmenu;

// Auto-generated imports
use player::player_create_script;
use enemy::enemy_create_script;
use ui_mainmenu::ui_mainmenu_create_script;

pub fn get_script_registry() -> HashMap<String, CreateFn> {
    let mut map: HashMap<String, CreateFn> = HashMap::new();
    // Auto-inserted per script module
    map.insert("Player".to_string(), player_create_script as CreateFn);
    map.insert("Enemy".to_string(), enemy_create_script as CreateFn);
    map.insert("MainMenuUI".to_string(), ui_mainmenu_create_script as CreateFn);
    map
}
```

The result: gameplay logic is **compiled into the engine binary itself**, not loaded from an external library.

---

## 🔧 How Scripts Work


```mermaid
flowchart TD
    A[Pup DSL] --> B[Transpiler]
    B --> C[Rust Code]
    C --> D[Compiler]

    D -->|Dev Build| E[📦 Script DLL]
    E --> F[🔄 Dynamic Loading (Hot Reload)]
    F --> G[🎮 Running Game]

    D -->|Release Build| X[⚡ Statically Linked Binary]
    X --> G

    classDef dev fill:#cce6ff,stroke:#003366,color:#003366;
    classDef release fill:#ccffeb,stroke:#006633,color:#003300;

    class E,F dev
    class X release
```

---

### 🔄 Dev vs ⚡ Release at a Glance

| Mode        | Output           | Loading Style        | Benefits                       |
| ----------- | ---------------- | -------------------- | ------------------------------ |
| **Dev**     | Scripts in a DLL | Dynamic (hot reload) | Instant iteration (1–3s)       |
| **Release** | Single exe       | Static (inlined)     | +10–25% perf, secure, portable |

---

## 🛠 For Engine Contributors

If you want to work on **Perro itself** (not just make games with it), you’ll need:

- [Rust (GNU toolchain preferred)](https://www.rust-lang.org/)
- Cargo

### Contributor Workflow

- **Runtime**  
  `perro_runtime` is the launcher used when building from source.

  - With no arguments → opens the editor
  - With a project path → runs that project directly as a game

- **Core**  
  The editor and engine logic live in `perro_core`. To rebuild the editor's scripts:

  ```bash
  cargo run -p perro_core
  ```

- **Running a Project (from source)**

  ```bash
  cargo run -p perro_runtime ./examples/hello_world
  ```

---

## 🛠 Roadmap

- [x] Core engine loop
- [x] FUR MVP (UI files referenced in scene files)
- [x] Pup DSL transpiler (basic)
- [ ] Complete Pup transpiler + full Rust API coverage
- [ ] Pup API polish
- [ ] Scene editor (dogfooding in progress)
- [ ] Asset pipeline
- [ ] Plugin System as self-contained Rust crates
- [ ] Additional language support (C#, TypeScript, etc...)

---

## 🤝 Contributing

Contributions are welcome!  
If you’d like to help shape Perro, check out the [CONTRIBUTING.md](CONTRIBUTING.md) and join the discussions.

---

## 📜 License

Perro is licensed under the **Apache 2.0 License**.  
See [LICENSE](LICENSE) for details.

---

## 🐾 Why "Perro"?

It's the game engine that just makes sense.
