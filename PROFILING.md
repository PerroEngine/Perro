# Profiling Guide

## Quick Start

Enable profiling with the `--profile` flag (primary) or `--flamegraph` (alias). This runs the game in **headless mode** (no window shown) and generates profiling data:

```bash
# Profile dev runtime (builds scripts + runs with headless profiling)
cargo run -p perro_core --features profiling -- --path projects/MessAround --profile

# Or use --flamegraph (same thing, kept for backwards compatibility)
cargo run -p perro_core --features profiling -- --path projects/MessAround --flamegraph
```

**Example for your MessAround project:**
```bash
cargo run -p perro_core --features profiling -- --path projects/MessAround --profile
```

This will:
1. Build your scripts (like `--dev` does)
2. Run `perro_dev` with profiling enabled (normal window, game runs normally)
3. Write profiling data to `flamegraph.folded` in the project directory
4. Automatically convert to `flamegraph.svg` when done (no need to run `--convert-flamegraph` manually)

## Viewing the Flamegraph

After running with `--profile`, you'll automatically get both:
- `flamegraph.folded` - Raw profiling data
- `flamegraph.svg` - Visual flamegraph (auto-generated)

Open `flamegraph.svg` in your browser to view it.

### Manual Conversion

If you need to convert an existing `flamegraph.folded` file manually:

```bash
cargo run -p perro_core --features profiling -- --path projects/MessAround --convert-flamegraph
```

Or use the `flamegraph` tool directly:
```bash
# Install flamegraph if you haven't
cargo install flamegraph

# Generate and open the flamegraph
flamegraph flamegraph.folded
```

## What Gets Profiled

The following functions are instrumented:
- `process_game()` - Main game loop
- `process_commands()` - App command processing
- `reset_scroll_delta()` - Input reset
- `scene_update()` - Scene update loop
- `render_frame()` - Frame rendering
- `Scene::update()` - Scene update
- `script_updates` - All script updates (with count)
- `script_update` - Individual script update (with script ID)

## Performance Impact

When the `profiling` feature is **disabled** (default), there is **zero overhead** - all profiling code is compiled out.

When enabled, there is minimal overhead from tracing spans, but it's designed to be lightweight.

## Alternative: cargo-flamegraph

You can also use `cargo-flamegraph` directly (doesn't require the feature flag):

```bash
cargo install flamegraph
cd perro_dev
cargo flamegraph --bin PerroDevRuntime -- --path ../projects/YourProject
```

This uses system-level profiling (perf/dtrace) which has different overhead characteristics.

