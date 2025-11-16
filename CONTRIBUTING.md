# Contributing to Perro

Thank you for helping improve Perro. Keep contributions small, tested, and focused on the engine's goals: performance, ease of use, and practical game development.

## Quick rules (must follow)

- A PR must fix or directly implement an existing issue. For design changes open an issue or a draft PR first.
- In the PR description include:
  - Linked issue
  - What the issue was
  - What you changed to solve it
  - Why the change fits Perro's goals (performance, ease of use, engine mission)
- Ensure the repository and test projects build before opening a PR.

## Build & test locally (required for PRs)

From repository root:

- Build workspace:

```bash
cargo build --workspace
```

- Run tests:

```bash
cargo test --workspace
```

- Required: build the special test project that covers language edge-cases:

```bash
cargo run -p perro_core -- --tests --scripts
```

test_projects\test contains scripts exercising edge cases across languages; if it compiles, your changes most likely won't break user scripts. Every PR must ensure one of the above succeeds.

## Formatting & linting (recommended)

- `cargo fmt --all` formats code to standard Rust style (keeps diffs consistent).
- `cargo clippy --all-targets --all-features -- -D warnings` runs a linter that finds common mistakes and style issues; fix warnings where reasonable.

You don't need to be an expert — run them before opening a PR to reduce review friction.

## Workflow

1. Fork the repo and create a branch:
   - feature: `feature/<short-desc>`
   - bugfix: `bugfix/<short-desc>`
   - docs: `docs/<short-desc>`
2. Make small, focused commits with clear messages.
3. Run build & test steps above (including building test_projects\test).
4. Push branch and open a PR against `main` (or the branch referenced by the issue).
5. In the PR body:
   - Link the issue the PR fixes.
   - Describe the problem, approach, and why this solution is correct.
   - Include a test plan and reproduction steps.
   - Attach example scripts / FUR files if relevant.

## PR checklist (must be satisfied)

- [ ] PR fixes or is linked to an issue.
- [ ] Branch builds: `cargo build --workspace`
- [ ] test_projects\test builds: `cargo run -p perro_core -- --tests --scripts`
- [ ] Tests pass: `cargo test --workspace`
- [ ] Formatted: `cargo fmt --all`
- [ ] Linted (recommended): `cargo clippy --all-targets --all-features -- -D warnings`
- [ ] PR description documents: issue, intent, implementation, and alignment with Perro's goals
- [ ] If scripting/transpiler changes: include sample input scripts and expected generated Rust output

## Tests & examples

- For transpiler or language work include small example scripts and their generated Rust output.
- If a change affects runtime behavior, add or update a minimal scenario in `test_projects/`.

## Reporting issues

When opening an issue include:

- Minimal reproduction steps
- Expected vs actual behavior
- Platform and Rust toolchain used
- Attach sample project/files when relevant

## Communication & large changes

- For large API or design changes open a proposal issue or draft PR and discuss before major work.
- Use GitHub Issues and PR comments for discussion.

## License

By submitting a PR you agree to license your contribution under the project's Apache 2.0 license.

Thank you — concise, tested contributions help Perro move faster.
