# Tests and Validation

## General

- If a change affects logic, add or update tests in the most local crate possible.
- Do not add heavy integration tests unless requested.

## What to run (typical)

- `cargo test` at workspace level (or relevant subset) should pass.

## When tests cannot be run here

- Describe the exact commands to run and what success looks like.
- Call out any likely failure points introduced by the change.
