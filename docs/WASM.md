# WASM / Web Target

## Page Map

| Header    | Link                    |
| --------- | ----------------------- |
| Purpose   | [Purpose](#purpose)     |
| Use Cases | [Use Cases](#use-cases) |
| Example   | [Example](#example)     |
| Reference | [Reference](#reference) |

## Purpose

Use `WASM / Web Target` when this feature, type group, file format, or workflow appears in game code or assets.

## Use Cases

Use the types, APIs, file formats, and workflows in this doc when the feature matches the game system you are building. Prefer `ctx.run` for runtime state, `ctx.res` for resource/data access, and `ctx.ipt` for input state.

## Example

```rust
lifecycle!({
    fn on_update(&self, ctx: &mut ScriptContext<'_, API>) {
        let dt = delta_time!(ctx.run);
        let _ = dt;
    }
});
```

## Reference

# WASM / Web Target

Perro web target create browser-ready web bundle + use browser canvas runner.

## Toolchain

Req:

- web build toolchain installed

```powershell
rustup target add wasm32-unknown-unknown
```

`perro` CLI create web bundle for you.

## Commands

Dev:

```powershell
perro dev --path <project_dir> --target web [--profile] [--release] [--host <addr>] [--port <num>]
```

Build:

```powershell
perro build --path <project_dir> --target web [--profile]
```

## `perro dev --target web`

Flow:

1. Sync `res/**/*.rs` -> `.perro/scripts/src/*.gen.rs`.
2. Run static asset pipeline.
3. Create generated web project bundle in dev mode by default.
4. Write dev bundle into `<project_dir>/.output/web-dev/`.
5. Start built-in static server.
6. Open default browser to `http://<host>:<port>/`.

Notes:

- `--release` use release web bundle for dev server.
- default host `127.0.0.1`
- default port `8000`
- dev web path use static embedded runtime, ! native dynamic runner

Bundle files:

- `index.html`
- `boot.js`
- `app.js`
- `app_bg.wasm`

## `perro build --target web`

Flow:

1. Sync `res/**/*.rs`.
2. Run static asset pipeline.
3. Create generated web project bundle in release mode.
4. Write bundle into `<project_dir>/.output/web/`.

Build command ! start local server.
Host bundle on any static file host.

## Rust Web API

Use `perro_web` in runtime-side Rust code:

```rust
use perro_web::{
    current_href,
    get_args,
    init_router,
    pop_route,
    push_route,
    split_href,
    split_query_args,
    storage::{
        load_cookie_bytes,
        load_local_bytes,
        load_session_bytes,
        remove_cookie,
        remove_local,
        remove_session,
        save_cookie_bytes,
        save_local_bytes,
        save_session_bytes,
    },
    take_pending_route_change,
};
```

API map:

- `init_router()` -> init browser router on `wasm32`
- `current_href()` -> ret current normalized browser path
- `get_args()` -> ret current query arg arr like `["arg1", "arg2"]`
- `push_route("/docs")` -> push browser history + queue route chg
- `pop_route()` -> cal browser back path
- `split_href("/docs/api")` -> ret path seg arr
- `split_query_args("?arg1&arg2")` -> ret query arg arr
- `take_pending_route_change()` -> ret next queued href on route chg
- `save_local_bytes(key, data)` -> wr browser `localStorage`
- `load_local_bytes(key)` -> rd browser `localStorage`
- `remove_local(key)` -> rm browser `localStorage` key
- `save_session_bytes(key, data)` -> wr browser `sessionStorage`
- `load_session_bytes(key)` -> rd browser `sessionStorage`
- `remove_session(key)` -> rm browser `sessionStorage` key
- `save_cookie_bytes(key, data)` -> wr browser cookie val
- `load_cookie_bytes(key)` -> rd browser cookie val
- `remove_cookie(key)` -> rm browser cookie

Native path:

- all `perro_web` route fns use no-op stub
- all `perro_web::storage::*` fns ret `Unsupported`
- `current_href()` -> `None`
- `get_args()` -> `None`
- `push_route()` + `pop_route()` -> `false`
- `take_pending_route_change()` -> `None`

Route norm:

- add leading `/` if miss
- trim trailing `/` except root `/`
- strip query `?x=1` + hash `#part`

Ex:

- `"docs"` => `"/docs"`
- `"/docs/"` => `"/docs"`
- `"/docs?tab=api#top"` => `"/docs"`

Arg split ex:

- `"/docs?arg1&arg2"` => `get_args() = ["arg1", "arg2"]`
- `"/docs?a=1&b=2"` => `get_args() = ["a=1", "b=2"]`
- `"/"` => `[]`
- `"/docs"` => `split_href(...) = ["docs"]`
- `"/docs/api"` => `split_href(...) = ["docs", "api"]`

## `routes.toml`

Web prj route map use opt sibling fle of `project.toml`:

```text
my_game/
|- project.toml
|- routes.toml
`- res/
```

Fmt:

```toml
[[route]]
href = "/"
name = "home"
scene = "res://routes/home.scn"

[[route]]
href = "/docs"
name = "docs"
scene = "res://routes/docs.scn"
```

Rules:

- use `[[route]]` array tbl
- `href` req
- `name` req
- `scene` req
- `href` use exact match only
- dynamic params + wildcard path ! support

Boot + runtime flow:

1. web boot cal `perro_web::init_router()`
2. runtime rd browser path frm `current_href()`
3. if route match, runtime load route `scene`
4. each frame runtime poll `take_pending_route_change()`
5. if href match, runtime swap root scene 2 route scene

Miss cfg path:

- if `routes.toml` miss, runtime mk default route cfg
- default route map use `/` => `project.main_scene`

Miss href path:

- boot miss -> fall back 2 `/`
- if `/` miss too, fall back 2 `project.main_scene`
- later push/pop miss -> runtime kp current scene

## UI Button Web Route

`UiButton` support opt `web` cfg block:

```scn
[UiButton]
    text = "Docs"
    web = { href = "/docs" }
[/UiButton]
```

Click flow on web:

- button click cal `perro_web::push_route(href)`
- runtime route poll find new href
- runtime load scene frm `routes.toml`
- normal button click evt still fire

Click flow on native:

- `web = { ... }` parse ok
- route push ! run
- normal button click evt still fire

## Deploy Note

Web build output use plain static files.

Output shape:

- `/index.html`
- `/boot.js`
- `/app.js`
- `/app_bg.wasm`
- `/assets/...`
- each `routes.toml` entry also emit route html like `/docs/index.html`

Route note:

- route html + static assets use relative file refs
- route nav + route fetch use root-absolute href like `/docs`
- host bundle at site root like `https://game.example.com/`
- subdir mount like `https://example.com/games/my-game/` ! fit now

### itch.io

Use case:

- good fit 4 root-only web build
- best fit if prj use only `/` route

Upload flow:

1. run `perro build --path <project_dir> --target web`
2. open `<project_dir>/.output/web/`
3. zip contents of dir, ! dir itself
4. chk zip root has `index.html`
5. upload zip as HTML game

Notes:

- itch host game in iframe
- itch doc ask relative paths in uploaded files
- Perro asset refs fit that rule
- Perro route nav ! fit itch multi-page embed well cuz route hrefs use `/docs` root style
- if you use `routes.toml` + web route buttons, expect route nav/direct deep link issues on itch

Safe itch path now:

- use single `/` route
- keep nav in-canvas/in-game, ! browser route path
- treat itch build as 1 entry page

### Cloudflare Pages

Use case:

- good fit 4 full web deploy
- good fit 4 multi-route `routes.toml`

Deploy flow:

1. run `perro build --path <project_dir> --target web`
2. deploy `<project_dir>/.output/web/` as Pages static output
3. use Pages project root/custom domain root, ! subdir mount

Why this fit:

- Pages serve matching html file on route path
- `/docs/index.html` auto-serve on `/docs/`
- Pages also redirect html file paths 2 extension-less route paths
- if top-level `404.html` miss, Pages default SPA fallback map unknown paths -> `/`

Optional `_redirects`:

- add only if you want custom canonical/legacy path rules
- put `_redirects` in `.output/web/`

Ex:

```text
/docs /docs/ 301
/old-home / 301
```

## Support Matrix

### CLI + Build Flow

| Area                              | Status        | Notes                                                     |
| --------------------------------- | ------------- | --------------------------------------------------------- |
| `perro dev --target web`          | Supported     | create web dev bundle + start local server + open browser |
| `perro build --target web`        | Supported     | create web release bundle into `.output/web/`             |
| `--profile` on web                | Supported     | pass thru on dev + build                                  |
| `--release` on `dev --target web` | Supported     | use release web bundle for dev server                     |
| `--host` / `--port` on web dev    | Supported     | local server cfg                                          |
| `--ui-profile` on web dev         | Not yet       | cli reject                                                |
| `--csv-profile` on web dev        | Not yet       | cli reject                                                |
| `--console` on web build          | Not supported | cli reject                                                |

### Runtime + Platform

| Area                               | Status          | Notes                                             |
| ---------------------------------- | --------------- | ------------------------------------------------- |
| Browser canvas runner              | Supported       | web wnd + resize sync path                        |
| WebGPU renderer                    | Supported       | req browser + GPU adapter                         |
| Static embedded assets             | Supported       | web dev + web build both use static embedded path |
| Native dynamic dev runner          | Not used on web | web dev path ! use `.perro/dev_runner`            |
| Browser w/o WebGPU adapter         | Not supported   | boot fail like `No available adapters.`           |
| Native script dylib load           | Not supported   | compile out on wasm                               |
| Steamworks                         | Not supported   | native-only path                                  |
| EXE/path-relative native behaviors | Not supported   | browser path ! use native exe model               |

### Input + Device Features

| Area                                        | Status        | Notes               |
| ------------------------------------------- | ------------- | ------------------- |
| Keyboard + mouse                            | Supported     | browser wnd path    |
| Native gamepad backend (`gilrs` / `hidapi`) | Not supported | compile out on wasm |
| Native Joy-Con backend (`btleplug`)         | Not supported | compile out on wasm |

### Project / Content Features

| Area                                   | Status           | Notes                            |
| -------------------------------------- | ---------------- | -------------------------------- |
| Static scenes + static resources       | Supported        | main web content path            |
| DLC disk/runtime path                  | Not yet verified | treat as unsupported for now     |
| Hot dynamic asset/script load frm disk | Not supported    | browser path use embedded bundle |

## Perf Notes

| Case                                   | Perf shape                         | Notes                              |
| -------------------------------------- | ---------------------------------- | ---------------------------------- |
| Native `perro dev`                     | Best dev iteration                 | dynamic file path + native runner  |
| Web `perro dev --target web`           | Slower build/start than native dev | rebuild web bundle + browser boot  |
| Web `perro dev --target web --release` | Better runtime perf than web debug | use if frame time matter           |
| Web `perro build --target web`         | best web perf path                 | release web bundle + static bundle |
| Native build                           | best overall perf                  | no browser + no wasm bridge cost   |

More perf detail:

- web dev use static embedded runtime path, so behavior match web build more than native dev
- debug web build run much slower than release web build
- browser perf depend on WebGPU driver + browser quality + GPU
- browser main-thread limits + web runtime bridge add cost vs native
- use native build 4 final perf baselines
- use web release path 4 browser perf baselines

## Asset + Runtime Diff Vs Native

Native `perro dev`:

- run `.perro/dev_runner`
- read project files frm disk
- use native wnd/app path

Web `perro dev --target web`:

- build web bundle frm `.perro/project`
- serve static files frm `.output/web-dev`
- boot browser canvas runner
- use static embedded assets + static script registry

Native `perro build`:

- output exe into `.output/`

Web `perro build --target web`:

- output browser bundle into `.output/web/`

## Web Save Data

`user://` auto-map on web target.

Path map:

- native `user://save/slot1.json` => OS user data dir
- web `user://save/slot1.json` => browser `localStorage`
- key fmt: `perro:user:<ProjectName>:data:save/slot1.json`

Notes:

- web path use `localStorage` now
- vals store as base64 so binary data work
- same project name => same browser save namespace
- diff origins/domains keep diff browser storage
- browser clear/site-data clear => save gone
- `sessionStorage` + cookie path ! auto-bind 2 `user://`
- use `perro_web::storage::*` if you need session-only or cookie-backed vals

Pick storage:

- `user://...` / `localStorage` -> save files, settings, long-lived user data
- `sessionStorage` -> tmp per-tab state
- cookie -> tiny server-visible vals, auth-ish bridge, legacy web flows

Limits:

- `localStorage` + `sessionStorage` size small vs disk save
- cookie size very small, send on HTTP req, use only 4 tiny vals
- IndexedDB ! wire yet

## Troubleshoot

Common errs:

- `web build toolchain missing`
  - install req web build tools
- `No available adapters.`
  - browser/WebGPU path fail
  - chk real desktop browser + GPU support
