# Perro Engine: Repo Rules Overview

This repo is a Rust game engine split into multiple crates under `/perro_source`.

Primary goals:

- Maintain clean crate boundaries and layering.
- Preserve performance-focused architecture while keeping code safe and testable.
- Make minimal, intentional changes.

If there is any ambiguity in requested changes:

- Ask for clarification OR propose 1–2 options with tradeoffs, then proceed with the least invasive option.

Hard constraints:

- Do not add features unless explicitly requested.
- Do not refactor unless explicitly requested.
- Do not use `unsafe`.
- Do not propose expensive operations.
- Changes must keep tests passing (or add/adjust tests when required by the change).
