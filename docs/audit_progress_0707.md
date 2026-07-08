# Audit Progress 2026-07-07

## Coherence Audit

- [x] 1.4 `drop_` canonical for preloaded scene disposal
  - commit: `952e93c3`
  - verify: `cargo check -p perro_runtime_api`, `cargo test -p perro_runtime_api`, full `cargo check`, full `cargo clippy`, full `cargo test`
- [x] 3.3 `set_tags` canonical node tag setter
  - commit: `e9fa5508`
  - verify: `cargo test -p perro_runtime_api`, full `cargo check`, full `cargo clippy`, full `cargo test`
- [x] 6.1 `Variant::kind()` canonical getter
  - commit: `02513d99`
  - verify: `cargo fmt --package perro_variant`, `cargo test -p perro_variant`, full `cargo check`, full `cargo clippy`, full `cargo test`
- [x] 7.1 runtime `*_version` stragglers -> `*_revision`
  - commit: `fa16781e`
  - verify: full `cargo check`, full `cargo clippy`, full `cargo test`

## Spec Audit

- [ ] 2.1 dup dep versions
- [ ] 2.2 unsafe w/o safety comment
- [ ] 2.3 unwrap/panic in runtime paths
- [ ] 2.4 string-keyed maps in hot structs
- [ ] 2.5 per-frame str alloc in render extract
- [ ] 2.6 lock surface
- [ ] 2.7 wgsl prelude triplication
- [ ] 2.8 test coverage holes
- [ ] 2.9 misc sweep

## Coherence Audit Next

- [ ] 4.1 central scene-key alias table
- [ ] 5.1 move mis-filed bridge types
- [ ] 4.3 split 2D prepare modules
- [ ] 5.2 uniform naming
- [ ] 3.1 getter prefix policy
- [ ] 3.2 redundant module prefixes
- [ ] 3.4 typed load errors
- [ ] 1.1 color vocabulary
- [ ] 1.2 on/off vocabulary
- [ ] 2.1 light colors use `Color`
- [ ] 2.2 typed IDs for string asset refs
- [ ] 2.3 nested settings structs
- [ ] 2.4 base embedding
- [ ] 2.5 internal state fields
- [ ] 2.6 zero-arg constructors
- [ ] 3.5 dimension-generic APIs
- [ ] 6.2 variant suffix casing
- [ ] 7.2 runtime API facade name
