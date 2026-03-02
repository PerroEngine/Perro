# Rust Coding Standards

## Safety / correctness

- No `unsafe` code.
- Avoid panics in runtime code; use `Result` with structured error types.

## Change discipline

- Do not refactor unless asked.
- Do not add features unless asked.
- Keep diffs minimal and localized.

## API style

- Prefer explicit types at public boundaries.
- Prefer small, composable functions over large multi-purpose ones.
- Avoid "clever" macros in non-macro crates.

## Performance hygiene

- Avoid allocations in hot loops unless already accepted by surrounding code.
- Favor iterators vs indexing only when it does not regress performance/readability.
- Avoid unnecessary cloning; prefer borrowing.

## Logging / tracing

- Use whatever logging/tracing system the repo already uses.
- Do not introduce new logging frameworks without explicit request.

## Formatting

- `rustfmt` compliant.
- Prefer `snake_case` modules and functions; `CamelCase` types; `SCREAMING_SNAKE_CASE` consts.
