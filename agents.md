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

- `core/`: ids, nodes, structs, animation, variant.
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

## Caveman Rule (Ultra Short)

- use tiny words
- 1 line = 1 idea
- cut all filler
- no “now”, “then”, etc
- no full sentences
- verb first (`chg`, `rm`, `add`)
- use arrows `->` 4 flow
- use short forms always
- skip repeats
- max info, min chars

---

## Core Words

- change → `chg`
- remove → `rm`
- add → `add`
- fix → `fx`
- use → `use`
- keep → `kp`

---

## Common Short

- before → `b4`
- after → `aft`
- with → `w/`
- without → `w/o`
- function → `fn`
- variable → `var`
- config → `cfg`
- update → `upd`
- render → `rndr`
- initialize → `init`
- frame → `frm`
- logic → `log`
- background → `bg`
- image → `img`
- trigger → `trig`
- optional → `opt`
- performance → `perf`
- memory → `mem`
- allocate → `alloc`
- system → `sys`

---

## Tense Rule

- use base verb only
- no past tense
- no future tense
- no continuous tense
- no helper verbs (`will`, `would`, `can`, `could`, `am`, `is`, `are`, `was`, `were`)
- convert all action to present/base form
- prefer `I do`, `I fix`, `I add`, `this use`, `this reduce`

---

## Symbols

- flow → `->`
- replace → `=>`
- and → `+`
- same → `same`
- equals → `=`

---

## Pattern

verb target (file)
action -> result
rm/add/fix notes
keep/change state

---

## Example

chg splash (file.rs)
hold 2000ms -> fade
rm old trig
same vis

## Default Response Shape

When reporting work:

1. What changed.
2. Result

Keep it short.
