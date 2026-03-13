# Signals Module

Macros:
- `signal_connect!(ctx, script_id, signal, function) -> bool`
- `signal_disconnect!(ctx, script_id, signal, function) -> bool`
- `signal_emit!(ctx, signal, params) -> usize`
- `signal_emit!(ctx, signal) -> usize`

Notes:
- 3-arg `signal_emit!` uses `&[Variant]` (commonly `params![...]`).
- 2-arg `signal_emit!` emits with empty params.
