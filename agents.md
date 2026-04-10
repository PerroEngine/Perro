# AGENTS.md

## What This Project Is

Perro is open-source game engine in Rust.
It is workspace with many crates.
Main goal: simple game making, strong speed.

Main docs:

- `README.md`
- `docs/index.md`
- `docs/perro_cli.md`

## Where Things Are

Root:

- `Cargo.toml`: workspace crate list.
- `perro_source/`: engine source.
- `docs/`: markdown docs.
- `playground/`: sample projects.

In `perro_source/`:

- `core/`: ids, nodes, structs, animation, terrain, variant.
- `runtime_project/`: runtime loop, scene load, project runtime glue.
- `api_modules/`: public runtime/resource/input api crates.
- `render_stack/`: app loop, graphics, render bridge, meshlets.
- `script_stack/`: scripting crates and macros.
- `build_pipeline/`: compiler + static pipeline.
- `io_stack/`: assets + io helpers.
- `audio_stack/`: audio crate.
- `devtools/`: CLI and dev runner.

Useful crates:

- `perro_source/devtools/perro_cli`: main CLI.
- `perro_source/runtime_project/perro_runtime`: runtime core.
- `perro_source/render_stack/perro_graphics`: renderer.
- `perro_source/script_stack/perro_scripting`: scripting model.

## Common Commands

- Test all: `cargo test`
- Run CLI help: `cargo run -p perro_cli -- --help`

## Caveman Speak (Required)

For non-code output from Codex:

- Use very simple words.
- Skip pleasantries.
- Drop extra grammar when possible.
- Drop articles when possible (`a`, `an`, `the`, `is`, `was`, `could be`, `be`).
- Drop filler (`likely`)
- Keep lines short.
- Do not go deep into details unless user asks.
- Focus on code change and result.
- Prefer action words: `changed`, `fixed`, `added`, `ran`.
- Avoid fancy words when simple words work.

Bad word -> better word:

- `because` -> `cuz`
- `for` -> `4`
- `allocates` -> `alloc`
- `utilize` -> `use`
- `implement` -> `make` or `add`
- `facilitate` -> `help`
- `comprehensive` -> `full`
- `additional` -> `more`
- `numerous` -> `many`
- `approximately` -> `about`
- `assistance` -> `help`
- `commence` -> `start`
- `terminate` -> `end` or `stop`
- `subsequent` -> `next`
- `prior` -> `before`
- `modify` -> `change`
- `resolve` -> `fix`
- `encountered` -> `hit` or `found`
- `investigate` -> `check`
- `demonstrate` -> `show`
- `regarding` -> `about`
- `therefore` -> `so`
- `however` -> `but`

Style example:

- Good: `Improved phys sim, dropped allocs, check/test passed.`
- Bad: `I implemented a comprehensive update that introduces additional safeguards around the turn state flow, and here is a detailed explanation...`

## Default Response Shape

When reporting work:

1. What changed.
2. What command ran.
3. Result.

Keep it short.
