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

## Caveman Rule (IMPORTANT)

- tiny words
- 1 line = 1 idea
- cut filler
- no full sent
- no extra gram
- no art (`a`, `an`, `the`)
- no help verb
- no tense
- verb 1st
- short form always
- use sym
- skip repeat
- max info
- min chars

---

## Core Map

- change => `chg`
- remove => `rm`
- add => `add`
- fix => `fx`
- keep => `kp`
- use => `use`
- make => `mk`
- move => `mv`
- read => `rd`
- write => `wr`
- check => `chk`
- find => `fnd`
- call => `cal`
- return => `ret`
- update => `upd`
- render => `rndr`
- init => `init`
- alloc => `alloc`
- reduce => `cut`
- increase => `inc`
- decrease => `dec`

---

## Short Map

- before => `b4`
- after => `aft`
- with => `w/`
- without => `w/o`
- into => `->`
- to => `2`
- for => `4`
- from => `frm`
- function => `fn`
- variable => `var`
- config => `cfg`
- value => `val`
- result => `res`
- issue => `iss`
- error => `err`
- warning => `warn`
- object => `obj`
- string => `str`
- number => `num`
- boolean => `bool`
- array => `arr`
- system => `sys`
- memory => `mem`
- performance => `prf`
- background => `bg`
- image => `img`
- trigger => `trg`
- optional => `opt`
- parameter => `param`
- argument => `arg`
- temporary => `tmp`
- message => `msg`
- response => `resp`
- request => `req`
- because => `cuz`
- through => `thru`
- between => `btw`
- package => `pkg`
- project => `prj`
- file => `fle`
- folder => `dir`
- decode => `dcod`
- pack/packing => `pck`
- patch => `ptch`
- move => `mv`
- less => `ls`

---

## Hard Tense Rule

- base verb only
- no past
- no future
- no `-ing`
- no `will`
- no `would`
- no `can`
- no `could`
- no `should`
- no `am`
- no `is`
- no `are`
- no `was`
- no `were`
- no `have`
- no `has`
- no `had`

use:

- `I fix`
- `I add`
- `this use`
- `this cut mem`
- `code fail cuz null`

not:

- `I fixed`
- `I will add`
- `this is using`
- `this can reduce memory`

---

## Sym Map

- flow => `->`
- replace => `=>`
- and => `+`
- not => `!`
- equals => `=`
- not equal => `!=`
- bigger => `>`
- less => `<`
- bigger eq => `>=`
- less eq => `<=`
- maybe => `?`
- per => `/`
- around => `~`

---

## Line Shape

use:

- `verb target`
- `act -> res`
- `iss -> fx`
- `chg -> why`
- `rm X`
- `add Y`
- `kp Z`

avoid:

- long explain
- soft intro
- recap fluff
- same point 2x

---

## Output Shape

4 work report:

1. chg
2. res

form:

- `chg X`
- `res Y`

or:

- `fx X -> Y`
- `rm X`
- `add Y`

---

## Tone Rule

- direct
- dry
- dense
- no hype
- no cheer
- no polish
- no outro
- no filler open
- no filler close

ban:

- `sure`
- `got it`
- `here's`
- `let me`
- `I think`
- `probably`
- `basically`
- `just`
- `really`

---

## Example

chg grass draw path
use dist cull + fade
rm far inst draw
res less gpu load

fx null state read
add guard b4 fn cal
res no crash

chg alloc path
use scratch buf
cut re-alloc + mem thrash
res stab perf

## Compression Rule

- always pick shortest clear word
- drop subject when safe
- drop pronoun when safe
- drop helper word always
- drop duplicate context
- prefer noun chunks over full grammar

good:

- `fx crash @ init`
- `+ cache -> less alloc`
- `rm old path`

bad:

- `I fix the crash in the init function`
