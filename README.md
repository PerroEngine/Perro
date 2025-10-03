# 🐕 Perro Game Engine

**Perro** is an experimental, open-source game engine written in **Rust**, designed  
as a modern alternative to engines like Godot and Unity.  

It focuses on **performance, flexibility, and developer freedom**, while introducing  
unique ideas that make game development faster and more enjoyable:

- ⚡ **Managed Runtime** – no Rust installation required. Just download Perro and start making games.  
- 🐶 **Pup DSL** – a lightweight scripting language that compiles to Rust, giving you native performance with a clean, approachable syntax.  
- 🎨 **FUR (Flexible UI Rules)** – a declarative UI system inspired by XAML/JSX, with Tailwind-style utility classes for styling.  
- 🏎 **Static Release Builds** – game scripts are compiled away into optimized machine code, giving you a **10–25% performance uplift** and extra protection in final builds.  
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
   - Write gameplay in **Pup DSL**, **C#**, **TypeScript**, or **pure Rust**.  
   - Design UI with **FUR**.  
   - Hit **Play** in the editor — Perro automatically:
     1. Transpiles your scripts (Pup/C#/TS/Rust) → Rust  
     2. Compiles Rust → DEV DLL  
     3. Hot-loads the DLL into the running game  

👉 **You never need Rust installed.** The editor and runtime handle everything for you.

---

## 🐶 Pup DSL

**Pup** is Perro’s built-in scripting language.  
It is concise and readable, but ultimately compiles to **Rust** and then into your build.

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

**FUR** is Perro’s declarative UI system, inspired by XAML and JSX.

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

- Script compilation → game start in **1–3 seconds** (DEV)  
- Change gameplay or UI → hit **Play** → see updates instantly  
- No scripts changed? **Startup is literally instantaneous** due to aggressive caching

---

## 🔄 Dev vs ⚡ Release

### 1. Dev Mode (Hot-Reload via DLL)
- Your game scripts (Pup, C#, TS, Rust) are **transpiled** to Rust, then compiled into a **DLL**.
- The engine **loads** this DLL at runtime so you can:
  - Make changes
  - Recompile in ~1–3 s
  - See changes immediately without restarting the whole editor

### 2. Release Mode (Static Linking for Maximum Performance)
- When you build for **Release**, Perro:
  1. Transpiles all scripts → Rust modules  
  2. Runs the Rust compiler with **–release**  
  3. **Statically links** every script function into the final binary via a generated registry  

```rust
// Auto-generated script registry in Release
use perro_core::script::{CreateFn, Script};
use std::collections::HashMap;

pub mod player;
pub mod enemy;
pub mod ui_mainmenu;

use player::player_create_script;
use enemy::enemy_create_script;
use ui_mainmenu::ui_mainmenu_create_script;

pub fn get_script_registry() -> HashMap<String, CreateFn> {
    let mut map = HashMap::new();
    map.insert("Player".to_string(), player_create_script as CreateFn);
    map.insert("Enemy".to_string(), enemy_create_script as CreateFn);
    map.insert("MainMenuUI".to_string(), ui_mainmenu_create_script as CreateFn);
    map
}
```

- **Result:**
  - One single executable (no DLLs, no scripts shipped).  
  - **10–25% performance uplift** thanks to inlining and LLVM optimizations.  
  - Your source scripts are **not** distributed—only optimized machine code lives in the binary.

---

## 🌐 Multi-Language Scripting

Perro’s **Transpiler System** isn’t limited to Pup! You can write gameplay logic in:

- Pup (our DSL)  
- C#  
- TypeScript  
- Pure Rust  

The pipeline is always:

1. **Transpile** (C#/TS/Pup → Rust)  
2. **Compile** (Rust → DLL in Dev, Rust → static binary in Release)  
3. **Load** (DLL hot-reload in Dev, direct function calls in Release)

You get the freedom to pick your favorite language, with the performance of Rust under the hood.

---

## 🛠️ For Engine Contributors

To work on **Perro itself** (the engine/editor):

- Install **Rust** (GNU toolchain preferred)  
- Have **Cargo** available  

### Building from Source

- **Runtime** (editor + game runner):  
  ```bash
  cargo run -p perro_runtime ./examples/hello_world
  ```
- **Core** (editor UI, windowing, build system):  
  ```bash
  cargo run -p perro_core
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
- [ ] Additional language support (C#, TypeScript, etc…)

---

## 🤝 Contributing

Contributions are welcome! See [CONTRIBUTING.md](CONTRIBUTING.md) and join the discussions.

---

## 📜 License

Perro is licensed under the **Apache 2.0 License**. See [LICENSE](LICENSE) for details.

---

## 🐾 Why “Perro”?

Every developer needs a loyal partner, just like a dog, and that's what Perro means in Spanish.
