# Editor project flow and input recovery — 2026-09-14

## Editor layout and viewport follow-up

The shell uses a compact toolbar, scene and file trees on the left, a central
viewport, and an inspector on the right. Rare tree and file operations remain
available through the command palette. Inspector initialization no longer
overwrites the shell's pane widths. Inspector fields use fixed readable text
sizes, and unchanged inspector data skips redundant UI writes. Nested row
height accumulation runs in one reverse pass.

Viewport input reads the runtime's completed UI layout bounds. Camera streams
track the pane's size, with a bounded render target, and picking uses the engine's
camera ray calculation. Preview rebuilds restore the active stream after clearing
the old scene. The 2D grid uses repeating world-aligned spacing across zoom levels.

Integration tests exercise the real shell layout and a selected Node3D inspector
at three window sizes. A separate scene-open test checks the 3D camera, framing,
and a mesh draw in the extracted camera stream. These checks validate runtime
layout and render commands; they do not replace visual GPU verification.

## Project and input recovery

All seven editor signal handlers were private methods. The compiler deliberately
generates dynamic dispatch only for `pub fn`, so the editor could draw its UI but
its buttons and tree callbacks did nothing. The handlers are public, preserving
the scripting visibility contract. A test loads the real manager scene with the
generated script registry, emits its Create signal, and checks the editor state.

Project selection accepts a folder containing `project.toml`; generated `.perro/`
build output is not a prerequisite. Invalid selections leave the current preview
and scene sessions intact. Dirty scenes must be saved before a project reload or
switch.

Button input also handles a press and release received within one frame. Previously,
the final mouse-up state could skip the pressed visual state and lose the click.
The retained UI input gate observes press/release edges even when the pointer
does not move. Releasing outside a focused button cancels its mouse click.
Regression tests cover the button event path and the manager's scene-to-script
signal connection.

File browser sort keys keep a folder's descendants together before a sibling
file with the same prefix. Long Unicode paths shorten at character boundaries.
Unchanged file-watch snapshots skip index allocation; changed snapshots use hash
indexes and retain sorted output.

## CPU probe

Windows, optimized standalone Rust probe, median of 18 samples. Inputs contain
10,000 or 50,000 synthetic paths. These figures measure diff and sort work only,
not disk I/O, GPU work, or interactive frame rate.

Reproduce with [the standalone probe](../../tools/editor_perf_probe.rs); build and
run commands are at the top of that file. Timings vary with machine load.

| Work | Files | Before, ms | After, ms |
| --- | ---: | ---: | ---: |
| Unchanged signature diff | 10,000 | 5.674 | 0.225 |
| Signature diff, 1% changed | 10,000 | 6.035 | 4.874 |
| Unchanged signature diff | 50,000 | 33.511 | 1.142 |
| Signature diff, 1% changed | 50,000 | 33.290 | 32.535 |
| Browser sort | 10,000 | 4.465 | 2.874 |
| Browser sort | 50,000 | 27.187 | 15.373 |

## GUI verification limit

The development editor starts with Vulkan on an AMD Radeon RX 7800 XT. An idle
manager sample reports approximately 140 FPS at a 144 Hz cap. This is a baseline,
not an improvement measurement. Windows screenshot capture fails with
`SetIsBorderRequired: No such interface supported (0x80004002)`. The custom UI
does not expose its controls through accessibility, so no visual click-through
or native-folder-dialog smoke result is claimed.

## Automated checks

Editor `check`, `clippy`, and script tests pass: 83 tests passed, 4 optional probes
ignored. Project doctor reports 0 errors and 171 advisory warnings about public
script members. Compiler tests pass: 59 passed, 6 optional checks ignored. These
are scoped checks, not a workspace-wide test result. The runtime UI suite passes
all 155 tests, including completed UI bounds, stationary fast taps, normal clicks, off-button release,
disabled controls, overlays, keyboard navigation, layout, and styling.
