# Perro scripting docs

Reference for scripting in Perro: node types, resources, and APIs you use in your scripts.

## Engine overview

For a high-level picture of how the engine works — why we transpile, dev vs release, static assets, and the `.perro` folder — see **[ENGINE.md](ENGINE.md)**.

## Transpiler architecture: scripts become Rust, one way or another

No matter which language you write in (PUP, TypeScript, or C#), **none of them are run directly.** The transpiler turns your scripts into **Rust**; that Rust is written into the **`.perro/scripts/src`** crate. The engine then runs that Rust:

- **Dev mode:** The scripts crate is compiled as a **DLL**. The engine (e.g. `perro_dev`) loads that DLL at runtime. You never run PUP/TS/C# — you run the compiled Rust in that DLL. Updating script sources recompiles in about 3–5 seconds; only changing engine core takes longer.
- **Export / release:** The scripts crate is built as a **normal dependency** of your project binary (one compilation unit with the engine). The result is a **single executable** with no DLLs; all script logic is statically linked in.

So in both cases you are running **transpiled Rust** that lives under `.perro/scripts/src`. If your script syntax is correct but you get errors in that generated Rust, the fix belongs in **codegen or bindings** (the transpiler and API layers), not always in your script. **Contributions are welcome and appreciated** — finding edge cases and reporting them or fixing them properly and dynamically in modules, APIs, and bindings (rather than with one-off hardcoding) helps everyone. Most behavior should be driven by the language APIs, resource APIs, and bindings so the system stays consistent and maintainable.

### Frontend syntax vs runtime

**PUP, TypeScript, and C# are frontend syntaxes.** The runtime is always the same: the generated Rust in `.perro/scripts/src`. TypeScript and C# are **experimental baselines** for contributors to experiment with; they **may not parse or support the same features as PUP** (e.g. lifecycle hooks, signal shorthand, `::` vs `call`, attribute syntax). When adding or fixing TS/C# support, be mindful of **how constructs transpile to Rust** — what becomes a closure, what becomes a method call on the script trait, what stays as a literal. The language docs below describe what each frontend aims to expose; the canonical behavior is defined by PUP and the engine.

## Language docs

| Doc | Description |
|-----|--------------|
| [**PUP**](PUP.md) | **Primary language.** Full reference: nodes, resources, modules, syntax, and examples. |
| [**TypeScript**](TYPESCRIPT.md) | **Experimental.** Some syntax and APIs work; you'll be experimenting with what's supported and what isn't. |
| [**C#**](CSHARP.md) | **Experimental.** Some syntax and APIs work; you'll be experimenting with what's supported and what isn't. |

## UI

| Doc | Description |
|-----|--------------|
| [**UI**](UI.md) | **FUR UI reference.** Elements and attributes parsed by the engine. |

## Scenes (source mode)

| Doc | Description |
|-----|--------------|
| [**SCENES**](SCENES.md) | **Building scenes from source.** `.scn` file syntax: node composition (base chain), naming, parenting, script attachment on the base Node, transforms, and examples from the repo. |

## Editor / tooling

**perro-lsp** — A Language Server Protocol implementation for `.pup` and `.fur` files lives in `perro_lsp/` with a VSCode extension in `.vscode-extensions/perro-lsp/`. **It is currently non-functional** but exists for anyone who wants to look at it or contribute. The intention is to use the engine’s API bindings, resource bindings, engine bindings, and Pup APIs for autocomplete and type checking; LSP development experience is limited, so contributions are welcome. See `perro_lsp/README.md` and `.vscode-extensions/perro-lsp/README.md` for details.
