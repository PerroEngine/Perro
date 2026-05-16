# WASM / Web Target

Perro web target create browser-ready web bundle + use browser canvas runner.

## Toolchain

Req:

- web build toolchain installed

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

## Troubleshoot

Common errs:

- `web build toolchain missing`
  - install req web build tools
- `No available adapters.`
  - browser/WebGPU path fail
  - chk real desktop browser + GPU support
