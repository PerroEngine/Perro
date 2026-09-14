# Bug audit fixes — 2026-09-13

Use [BUG_AUDIT.md](BUG_AUDIT.md) as source audit; kp original findings.

| ID | Fix |
| --- | --- |
| AUD-001 | Bound headless fixed tick to 1–1000 Hz; reject nonfinite rate; also guard app headless + winit tiny steps. |
| AUD-002 | Cap typed scene nesting at 128; return parse err. |
| AUD-003 | Stop MultiMesh grid at 100,000 instances. |
| AUD-004 | Set native TLS WebSocket socket nonblocking; bound handshake socket waits + retain max 3 redirects; reject unsupported TLS backend. |
| AUD-005 | Add nonce + short-epoch challenge/token flow, replay cache + data token checks; cap peers; refresh reconnect tokens; free stale peer state. |
| AUD-006 | Limit raster source bytes, decoder dims/alloc + RGBA output bytes, incl glTF images. |
| AUD-007 | Use fallible audio trim duration conversion. |
| AUD-008 | Cap mic ring samples + checked power-of-two capacity. |
| AUD-009 | Validate glTF primitive indices + checked base vertex add. |
| AUD-010 | Validate index count vs file bytes + 1M count cap; cap sidecar rows + bytes b4 map growth. |
| AUD-011 | Pass release/debug profile through DLC build + script stage + runner launch. |
| AUD-012 | Cap DSP layout + delay samples. |
| AUD-013 | Validate PMESH indices + ranges; check arena offsets + upload sizes. |
| AUD-014 | Saturate MIDI release deadline add. |
| AUD-015 | Return init err for user/demo paths b4 root setup; avoid shared fallback save dir. |
| AUD-016 | Bound static path pool to 65,536 entries + 8 MiB path bytes; keep existing refs valid. |
| AUD-017 | Decode quote, slash, LF, CR + tab escapes; reject unknown escapes. |
| AUD-018 | Return Result from heartbeat float ctor; reject invalid/oversized seconds. |
| AUD-019 | Emit TCP EOF disconnect once. |
| AUD-020 | Cap device/player indices at 256 slots; return None from invalid mutable slot access; ignore invalid queued commands. |
| AUD-021 | Cap particle + custom post-process param indices below 1024. |
| AUD-022 | Cap particle expression recursion at 64; cover unary, parens, calls + params. |
| AUD-023 | Bound HTTP workers + queue; timeout stalled clients; reject Windows drive/ADS paths + canonical outside-root reads. |
| AUD-024 | Reject scaffold drive/prefix/parent paths + check path containment. |
| AUD-025 | Skip links + bound doctor walks. |
| AUD-026 | Add stable unique animation symbol suffix. |
| AUD-027 | Use Rust string escapes in animation-tree codegen. |
| AUD-028 | Reject shader parent/prefix paths + check canonical containment. |
| AUD-029 | Parse lifecycle ownership via syn AST. |
| AUD-030 | Emit Variant conversion only for known supported return types. |
| AUD-031 | Keep build mode in thread-local scope; prevent guard transfer across threads. |

Dependency fixes:

- Upd `wayland-scanner` 0.31.11 -> `quick-xml` 0.41.0; cover [RUSTSEC-2026-0194](https://rustsec.org/advisories/RUSTSEC-2026-0194.html) + [RUSTSEC-2026-0195](https://rustsec.org/advisories/RUSTSEC-2026-0195.html).
- Upd `event-listener` 5.4.2; cover [RUSTSEC-2026-0221](https://rustsec.org/advisories/RUSTSEC-2026-0221.html).
- Upd yanked `chacha20` + `wide`; retain current toolchain needs (`wide` already requires Rust 1.89).
- Retain 4 upstream unmaintained warnings: `gcc`, `paste`, `rustybuzz`, `ttf-parser`.

API/format notes:

- Chg `InputSnapshot::{gamepad_mut,joycon_mut,player_mut}` -> `Option<&mut ...>`.
- Chg `HeartbeatConfig::from_secs_f32` -> `Result<HeartbeatConfig, String>`.
- Add `ResolvedPath::Uninitialized` + `ResPathError::InternLimit`.
- Use `ResPathBuf` for dynamic paths beyond static pool budget.
- Chg LAN wire protocol; upd both host + client together.
- Add WebSocket client handshake timeout option; default 3 seconds.
- Bound LAN tokens against blind/off-path spoof + stale handshake replay; plaintext tokens remain visible to on-path peers. No encryption or peer identity proof.
- Chg unsupported script method returns -> call method + return `Variant::Null`.

QA � final code:

| Check | Result |
| --- | --- |
| `cargo fmt --all -- --check` | pass |
| `cargo test --workspace --no-fail-fast` | pass: 2711 tests; 0 fail; 58 opt/manual tests ignored |
| `cargo clippy --workspace --all-targets -- -D warnings` | pass |
| `cargo check --workspace --all-features --all-targets` | pass; SDK/libclang env below |
| `cargo audit` | pass; 0 vuln/unsound findings; 4 upstream unmaintained warnings |
| Local net integration | pass: 14 normally ignored native tests + 2 timeout/redirect tests |

Keep test scope explicit: Windows default workspace tests + all-feature compile; no cross-OS build, full fuzz run, or hardware capture/device-loss validation.

Keep pre-existing crash-hook edit in `perro_app/src/entry.rs`; add only tick fix in that file.

Build env:

- Use Windows x86_64 MSVC + rustc 1.97.1.
- Fetch ASIO SDK from `https://www.steinberg.net/asiosdk`; old temp SDK dirs contain no headers.
- Set `CPAL_ASIO_DIR` to `%TEMP%/perro-audit-asio/ASIOSDK`.
- Install libclang in temp dir; set `LIBCLANG_PATH` to `%TEMP%/perro-audit-libclang/clang/native`.
- Keep SDK/libclang outside repo; env vars apply only to check process.
