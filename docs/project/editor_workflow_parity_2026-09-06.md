# Editor workflow parity — 2026-09-06

Keep Perro scene text, Rust scripts, `root_of`, and disk asset refs. Add multi-selection, batch scene operations, shared inspector fields, spatial/UI gesture transactions, branch export/source navigation, metadata subref pickers, reference-aware asset moves, and explicit animation conversion options. See [editor workflow docs](../tools/perro_editor.md).

## View preparation measurements

Run the ignored `audit_editor_views` test with `--ignored --nocapture`. The fixture holds 10,000 asset paths and varies the node count. Compare full view preparation with selection-only and files-only preparation in the same process and build. Values below use the local Windows development/test profile, not a release build.

| Nodes | Full p50 / p95, ms | Selection p50 / p95, ms | Files p50 / p95, ms |
| ---: | ---: | ---: | ---: |
| 100 | 6.136 / 6.231 | 0.429 / 0.431 | 5.877 / 5.931 |
| 1,000 | 6.700 / 6.791 | 0.980 / 0.990 | 5.871 / 5.986 |
| 10,000 | 13.395 / 13.898 | 7.611 / 7.913 | 5.956 / 6.042 |

Selection refresh skips file-list preparation; file refresh skips scene-tree and inspector preparation. Existing document/index caches remain in use. Drag samples patch preview transforms from the gesture baseline; idle spatial axes skip unchanged property writes. Batch commit uses the central no-op guard and one history snapshot.

These figures measure view data preparation only. They exclude GPU work, native UI layout, filesystem scans, and end-to-end input latency. They do not establish an FPS improvement. At 10,000 nodes, selection refresh still walks tree data; deeper virtualization is follow-up work.

## Verification and limits

Final local checks: editor `check` and `clippy` pass; editor tests 61 pass / 3 optional perf probes ignored; CLI options tests 2 pass; compiler tests 52 pass / 6 ignored. Project doctor reports 0 errors and 174 script-member warnings. No workspace-wide test result is claimed.

Run `perro_cli check`, `clippy`, and `test` against `perro_editor`, plus the CLI animation-options tests. Regression coverage includes selection ranges, ancestor deduplication, batch undo, clipboard snapshots, copied refs, shared inspector commits, source branch round trips, nested asset refs, staged move conflicts/rollback, and transform conversion under a rotated parent.

The local GUI runner launched and initialized Vulkan, but Windows screen capture failed with `SetIsBorderRequired failed: No such interface supported (0x80004002)` on both attempts. No end-to-end visual smoke pass is claimed. Verify popup layout, axis hit areas, multi-node UI resize/rotate, source navigation, and native file dialogs before release.

Windows link verification used a temporary `_LINK_=/DEBUG:NONE` process environment after PDB/disk failures. No permanent build-profile change is required. This disables link PDB output as documented by [Microsoft](https://learn.microsoft.com/en-us/cpp/build/reference/debug-generate-debug-info?view=msvc-170).

Asset reference discovery runs off the UI thread. The final validated write/rename/rollback step remains synchronous. Conversion runs in a child process. Moves preserve supported refs but can reformat scene/TOML text; unsupported refs block the operation. There is no import database, asset conversion on file change, or new instance-child override model.
