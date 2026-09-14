# Perro Bug Audit

date: 2026-09-13
scope: full workspace Rust/TOML/MD scan + Luna swarm review
mode: read-only audit; no code fix

code refs -> source-confirmed control-flow risks
runtime repro -> ! run for every item

## QA

- `cargo test --workspace` -> pass
- `cargo fmt --all -- --check` -> pass
- `cargo clippy --workspace --all-targets -- -D warnings` -> fail 1 test lint
- `cargo check --workspace --all-features --all-targets` -> block; ASIO SDK `asiodrivers.h` miss
- `cargo audit` -> 2 high `quick-xml` advisories

## High

### AUD-001 — headless fixed-step hang

- ref: `perro_source/runtime_project/perro_headless/src/lib.rs:115-121`
- trg -> `target_fixed_update` huge positive or `+INF`
- ev -> `fps > 0` accept; `1 / fps` -> zero `Duration`; loop subtract zero step
- impact -> headless loop never exit
- fx -> reject nonfinite; bound minimum step
- conf: high

### AUD-002 — typed scene recursion stack overflow

- ref: `perro_source/runtime_project/perro_scene/src/parser.rs:422-449`
- trg -> deeply nested typed blocks
- ev -> `parse_type_block_after_lbracket` recurse on nested `[`; no type-depth cap
- impact -> parser stack overflow; process abort
- fx -> depth cap or iterative parse
- conf: high

### AUD-003 — MultiMesh count load blowup

- ref: `perro_source/runtime_project/perro_runtime/src/runtime/scene_loader/prepare/nodes/three_d/base.rs:1118-1170`
- trg -> scene `counts` near `u32::MAX`
- ev -> capacity cap only; spawn loops use raw x/y/z counts
- impact -> load hang or OOM
- fx -> clamp counts before every loop
- conf: high

### AUD-004 — TLS WebSocket poll block

- ref: `perro_source/api_modules/perro_networking/src/websocket.rs:439-444,734-743`
- trg -> `wss://` poll w/o inbound data
- ev -> async API wraps sync poll in `spawn_blocking`; TLS nonblocking helper handles `Plain` only
- impact -> worker block; repeated calls exhaust net workers
- fx -> async TLS read or TLS socket nonblocking + timeout
- conf: high

### AUD-005 — LAN peer spoof + state growth

- ref: `perro_source/api_modules/perro_networking/src/multiplayer/lan_transport.rs:69-109`; `host_session.rs:197-223`
- trg -> junk datagrams from unique source ports
- ev -> host `add_peer` every non-discovery packet; slot alloc before frame auth; no peer cap
- impact -> peer/slot growth; forged payload + join state
- fx -> authenticated handshake; max peers; rate limit
- conf: high

### AUD-006 — raster decode bomb

- ref: `perro_source/render_stack/perro_graphics_assets/src/texture.rs:149-194`
- trg -> huge compressed PNG/JPEG/WebP
- ev -> `image::load_from_memory` alloc/decode b4 `max_dim` resize; no pixel/decoder cap
- impact -> OOM or process abort
- fx -> decoder limits; compressed/raw/pixel caps b4 decode
- conf: high

### AUD-007 — audio trim panic on nonfinite float

- ref: `perro_source/audio_stack/perro_pawdio/src/player/playback.rs:133-136`
- trg -> `from_start` or `from_end` `+INF`
- ev -> direct `Duration::from_secs_f32`
- impact -> play request panic; audio worker loss
- fx -> reject nonfinite; bound trim values
- conf: high

### AUD-008 — mic ring alloc blowup

- ref: `perro_source/audio_stack/perro_pawdio/src/mic.rs:1007-1021,1453-1466`
- trg -> huge or `+INF` `max_seconds`
- ev -> float cast -> `usize`; `next_power_of_two`; uncapped atomic ring alloc
- impact -> panic or OOM; capture fail
- fx -> finite/max sample cap; checked power-of-two
- conf: high

### AUD-009 — glTF index overflow

- ref: `perro_source/render_stack/perro_graphics_assets/src/mesh.rs:1038-1058`
- trg -> malformed glTF index near `u32::MAX` + prior vertex base
- ev -> `idx + base_vertex` unchecked
- impact -> debug panic; release wrapped GPU index
- fx -> checked add + vertex bounds
- conf: high

### AUD-010 — reused archive count OOM

- ref: `perro_source/io_stack/perro_assets/src/packer.rs:118-130`
- trg -> reused archive header `file_count = u32::MAX`
- ev -> `HashMap::with_capacity(file_count)` b4 index row validation
- impact -> build OOM or abort
- fx -> cap count against file size + sane max
- conf: high

### AUD-011 — release dev stages debug scripts

- ref: `perro_source/devtools/perro_cli/src/project.rs:360-380`
- trg -> `perro dev --release`
- ev -> runner build use release; DLC compile + launch/stage use hard-coded `Debug`/`debug`
- impact -> stale/missing scripts; release dev launch fail
- fx -> pass selected profile through DLC + stage paths
- conf: high

## Medium

### AUD-012 — wet DSP metadata alloc blowup

- ref: `perro_source/audio_stack/perro_pawdio/src/dsp.rs:238-245,420-440`
- trg -> source reports huge sample rate/channel count + wet DSP
- ev -> delay sizes derive from raw layout; no total byte cap
- impact -> multi-GB delay alloc; audio failure
- fx -> bound layout + total delay samples
- conf: medium

### AUD-013 — PMESH range draw OOB

- ref: `perro_source/render_stack/perro_graphics_assets/src/mesh.rs:852-866`; `perro_source/render_stack/perro_graphics/src/three_d/gpu/buffers/mesh_arena.rs:150-176`
- trg -> crafted surface/meshlet start/count past index block
- ev -> PMESH ranges load w/o index-count check; upload adds arena offset unchecked
- impact -> GPU OOB draw; debug arithmetic panic/device error
- fx -> validate `start + count <= index_count`; checked offsets
- conf: medium

### AUD-014 — MIDI deadline overflow

- ref: `perro_source/audio_stack/perro_pawdio/src/midi.rs:1037,1358-1359`
- trg -> unheld note + `Duration::MAX` sustain after mixer sample > 0
- ev -> duration conversion saturate to `u64::MAX`; unchecked sample add
- impact -> debug worker panic; release wrapped deadline
- fx -> checked/saturating add; sustain cap
- conf: medium

### AUD-015 — user path before project root

- ref: `perro_source/io_stack/perro_io/src/asset_io.rs:344-352,461-476`
- trg -> `user://` or `demo://` load b4 project-root setup
- ev -> `user_app_name` use `expect("Project root not set")`
- impact -> runtime panic
- fx -> return init error or set root b4 user path access
- conf: high

### AUD-016 — `ResPath::intern` permanent leak

- ref: `perro_source/api_modules/perro_resource_api/src/res_path.rs:95`
- trg -> repeated unique valid paths
- ev -> `Box::leak` + static `HashSet` retain every string forever
- impact -> unbounded process mem; eventual OOM
- fx -> bounded interner or owned managed path
- conf: high

### AUD-017 — scene string escape roundtrip loss

- ref: `perro_source/runtime_project/perro_scene/src/scene_doc.rs:740-752`; `lexer.rs:155-175`
- trg -> saved string w/ newline, tab, CR, or backslash
- ev -> writer emit escapes; lexer decode only `\"`
- impact -> scene value corruption aft save/load
- fx -> decode all emitted escapes; reject unknown escape
- conf: high

### AUD-018 — heartbeat float ctor panic

- ref: `perro_source/api_modules/perro_networking/src/multiplayer/heartbeat.rs:34-37`
- trg -> negative, nonfinite, or oversized seconds
- ev -> direct `Duration::from_secs_f32`
- impact -> public config ctor panic
- fx -> finite/range check; return `Result`
- conf: high

### AUD-019 — repeated TCP disconnect event

- ref: `perro_source/api_modules/perro_networking/src/tcp.rs:154-175,215-223`
- trg -> call `poll_event` after peer EOF
- ev -> empty read emit disconnect; `disconnect_emitted` gate apply only to frame path
- impact -> duplicate disconnect each poll
- fx -> set/read EOF gate in `poll_event`
- conf: high

### AUD-020 — input index alloc/panic

- ref: `perro_source/api_modules/perro_input_api/src/snapshot.rs:241-267`; `window.rs:75-89`
- trg -> huge queued gamepad/Joy-Con/player index
- ev -> `index + 1` resize w/o cap
- impact -> huge alloc or arithmetic panic
- fx -> cap device/player indices
- conf: high

### AUD-021 — particle param vector blowup

- ref: `perro_source/runtime_project/perro_runtime/src/runtime/scene_loader/prepare/nodes/three_d/particles.rs:134-149`; `prepare/common/post_processing.rs:374-382`
- trg -> `p<usize::MAX>` or huge param key
- ev -> parse any `usize`; alloc `max + 1`
- impact -> OOM, overflow, or index panic during scene load
- fx -> cap param index + use sparse map
- conf: high

### AUD-022 — particle expression recursion overflow

- ref: `perro_source/core/perro_particle_math/src/lib.rs:415-432`
- trg -> deeply nested unary/parenthesized expression
- ev -> recursive `parse_unary` + `parse_primary`; no depth cap
- impact -> compiler stack overflow
- fx -> depth limit or iterative parser
- conf: high

### AUD-023 — static server client stall

- ref: `perro_source/devtools/perro_cli/src/project.rs:1508-1519`
- trg -> client connect + send partial/no HTTP request
- ev -> blocking `TcpStream::read` inside accept loop
- impact -> server stop accept new clients
- fx -> read timeout; bounded worker per connection
- conf: high

### AUD-024 — Windows scaffold path escape

- ref: `perro_source/devtools/perro_cli/src/scaffold.rs:135-153`
- trg -> Windows `--res C:\outside`
- ev -> drive-prefixed path skip absolute/prefix checks; `root.join` discard root
- impact -> scaffold write outside project res dir
- fx -> reject prefix/absolute; verify containment
- conf: high

### AUD-025 — doctor symlink recursion

- ref: `perro_source/devtools/perro_cli/src/doctor.rs:499-513`
- trg -> `res/loop` symlink/junction to parent
- ev -> `path.is_dir()` follow links; recursion lacks visited/depth guard
- impact -> scan hang, stack overflow, or out-of-scope read
- fx -> skip links; canonical visited set + depth cap
- conf: high

### AUD-026 — animation generated-name collision

- ref: `perro_source/build_pipeline/perro_static_pipeline/src/animations.rs:698-708`
- trg -> assets `foo-bar.panim` + `foo_bar.panim`
- ev -> nonalnum chars map to `_`; no generated identifier collision check
- impact -> duplicate generated Rust symbols; build fail
- fx -> add hash suffix or reject collision
- conf: high

### AUD-027 — animation tree string codegen break

- ref: `perro_source/build_pipeline/perro_static_pipeline/src/animation_trees.rs:227-228`
- trg -> object/bone/mask text w/ newline or control char
- ev -> `esc` handle slash + quote only
- impact -> generated Rust syntax fail
- fx -> full Rust string escaping
- conf: high

### AUD-028 — material shader path traversal

- ref: `perro_source/build_pipeline/perro_static_pipeline/src/materials.rs:192-216`; `lib.rs:283-285`
- trg -> material shader path `res://../outside.wgsl`
- ev -> prefix strip + `root.join`; no parent/containment check
- impact -> build read outside res root
- fx -> reject parent segments; canonical containment
- conf: high

### AUD-029 — lifecycle text scan false positive

- ref: `perro_source/build_pipeline/perro_compiler/src/script_codegen.rs:329-371`
- trg -> helper impl method named `on_update` w/ `&self` + `ScriptContext`
- ev -> source-wide text scan; no impl/trait ownership parse
- impact -> wrong `HAS_UPDATE` flag; missing trait impl or silent no-op schedule
- fx -> parse lifecycle macro/target AST only
- conf: medium

### AUD-030 — unsupported script return codegen

- ref: `perro_source/build_pipeline/perro_compiler/src/script_fields.rs:453-477`
- trg -> public script method return type w/o `Variant::from` impl, e.g. `Duration`
- ev -> every non-unit return mark convertible; emit `Variant::from(call)`
- impact -> generated script compile fail
- fx -> whitelist convertible types or use `DeriveVariant`
- conf: high

### AUD-031 — concurrent build mode race

- ref: `perro_source/build_pipeline/perro_static_pipeline/src/lib.rs:67-102`; `perro_source/build_pipeline/perro_compiler/src/project_bundle.rs:120-121`
- trg -> concurrent compile calls w/ differing demo/playtest flags
- ev -> process-global atomics hold per-build mode
- impact -> build A read/filter state from build B; wrong generated output
- fx -> per-call context or serialize builds
- conf: medium

## Dependency risks

- `quick-xml 0.39.4` -> `RUSTSEC-2026-0194` high; quadratic duplicate-attribute check; fix `>=0.41.0`
- `quick-xml 0.39.4` -> `RUSTSEC-2026-0195` high; unbounded namespace allocation; fix `>=0.41.0`
- `event-listener 5.4.1` -> unsound advisory `RUSTSEC-2026-0221`
- unmaintained/yanked -> `gcc`, `paste`, `rustybuzz`, `ttf-parser`, `chacha20`, `wide`

## Limits

- default Windows target only
- no GPU device-loss run
- no network fuzz
- no malformed asset corpus run
- all-feature build block -> missing ASIO SDK header `asiodrivers.h`
- strict clippy block -> test-only `drop(ctx)` lint @ `perro_source/api_modules/perro_runtime_api/src/sub_apis/script.rs:427`
