# Script registry backend design

Status: design for follow-up implementation.

## Goal

Expose one safe Rust registry facade to the runtime. Keep registry acquisition behind two backends:

- embedded native slice for generated static projects
- versioned dynamic ABI for dev builds and DLC libraries

The runtime should resolve constructors by script hash without knowing where each constructor came from.

## Current paths

### Static project

The compiler generates `scripts::SCRIPT_REGISTRY` as a static slice of `(u64, ScriptConstructor<RuntimeScriptApi>)` pairs. Generated native, Android, and web entry points pass that slice through `StaticEmbeddedAssetsConfig::static_script_registry`. `perro_app` passes it to `Runtime::from_project_with_script_registry`, which copies entries into `ScriptRuntimeState::dynamic_script_registry`.

Problems:

- public app config exposes the storage shape instead of a registry abstraction
- `StaticScriptRegistry` aliases are duplicated in `perro_app` and `perro_runtime`
- the destination map is named `dynamic_script_registry` even when all entries are static
- `ProviderMode` and `Option<StaticScriptRegistry>` form an invalid-state pair

### Dev dynamic library

The runtime finds the newest scripts library under `target/{debug,release}`, loads it with `libloading`, injects project-root data, reads `perro_script_registry_len` and `perro_script_registry_get`, and copies constructors into the same map. The loaded library is retained so copied function pointers remain valid.

### DLC dynamic library

Mounted DLC script libraries use the same symbol pair and append constructors to the same map. Library handles must outlive every constructor, behavior object, and state object created from them. DLC is an overlay source, not a third lookup backend.

## Proposed public seam

Define the facade in `perro_runtime`, where `RuntimeScriptApi` is concrete:

```rust
pub type RuntimeScriptRegistryEntry =
    (u64, perro_scripting::ScriptConstructor<RuntimeScriptApi>);

pub type EmbeddedScriptRegistry = &'static [RuntimeScriptRegistryEntry];

#[non_exhaustive]
pub enum ScriptRegistrySource {
    Embedded(EmbeddedScriptRegistry),
    DynamicAbi,
}

pub struct RuntimeScriptRegistry {
    // private: hash map, source state, and live dynamic-library owners
}
```

Use constructors instead of public enum fields where future source types may grow:

```rust
impl ScriptRegistrySource {
    pub const fn embedded(entries: EmbeddedScriptRegistry) -> Self;
    pub const fn dynamic_abi() -> Self;
}

impl Runtime {
    pub fn from_project_with_registry(
        project: RuntimeProject,
        provider_mode: ProviderMode,
        registry: ScriptRegistrySource,
    ) -> Self;
}
```

Keep asset `ProviderMode` separate. Static assets plus a dynamic script source is unusual but valid for tests and tooling.

Generated static code becomes:

```rust
Runtime::from_project_with_registry(
    project,
    ProviderMode::Static,
    ScriptRegistrySource::embedded(scripts::SCRIPT_REGISTRY),
)
```

Dev code becomes:

```rust
Runtime::from_project_with_registry(
    project,
    ProviderMode::Dynamic,
    ScriptRegistrySource::dynamic_abi(),
)
```

`StaticEmbeddedAssetsConfig` should hold `ScriptRegistrySource`, not `Option` plus a raw slice. Web and mobile builds should reject `DynamicAbi` with a typed unsupported-backend error before boot-scene load.

## Internal facade duties

`RuntimeScriptRegistry` should own all registry state and expose a small internal API:

```rust
fn get(&self, path_hash: u64) -> Option<RuntimeScriptCtor>;
fn install_embedded(&mut self, entries: EmbeddedScriptRegistry) -> Result<(), RegistryError>;
fn ensure_base_dynamic_loaded(&mut self, project: &RuntimeProject) -> Result<(), RegistryError>;
fn mount_dynamic_overlay(&mut self, key: &str, path: &Path) -> Result<(), RegistryError>;
fn clear_dynamic(&mut self);
```

Required invariants:

- retain each dynamic library until all values created from it are dropped
- validate the complete incoming registry before mutating the live map
- reject duplicate hashes inside one source
- define overlay precedence explicitly; recommended rule: DLC mount overrides base only for scripts addressed through that mount
- remove an overlay only after its script instances, cached behaviors, and states are gone
- never expose `libloading::Library` or ABI function pointers in the public API

Rename `dynamic_script_registry` to `constructors` when the facade lands.

## Dynamic ABI contract

Replace independent symbol discovery with one versioned descriptor symbol:

```rust
#[repr(C)]
pub struct ScriptRegistryAbiV1 {
    pub abi_version: u32,
    pub struct_size: u32,
    pub registry_len: unsafe extern "C" fn() -> usize,
    pub registry_get: unsafe extern "C" fn(
        index: usize,
        path_hash_out: *mut u64,
        ctor_out: *mut RuntimeScriptCtor,
    ) -> bool,
    pub set_project_root: Option<unsafe extern "C" fn(/* byte spans */) -> bool>,
}
```

Export one `perro_script_registry_abi_v1` getter. Check version and `struct_size` before calling fields. Keep the old symbols for one compatibility window, with the loader trying the descriptor first.

The current constructor returns a Rust trait-object pointer. `extern "C"` does not make the trait-object representation a stable cross-toolchain ABI. Until a C-compatible behavior vtable replaces it, dynamic libraries must match the engine's Rust compiler, target, profile ABI settings, and Perro crate revision. Encode a build fingerprint in the descriptor and fail before reading constructors when it differs.

All raw-pointer validation belongs in a single ABI adapter. The rest of the runtime receives validated `RuntimeScriptRegistryEntry` values.

## Errors

Use a typed `RegistryError` with at least:

- `UnsupportedBackend`
- `LibraryOpen`
- `MissingDescriptor`
- `AbiVersionMismatch`
- `BuildFingerprintMismatch`
- `InvalidEntry { index }`
- `DuplicateHash { hash }`
- `ProjectRootRejected`

Include library path and source key as context. Do not collapse these errors to `bool` or report them as a missing script hash.

## Performance

- copy static and ABI entries into one `AHashMap` once per source load
- keep attach lookup as one hash-map read
- validate and reserve map capacity before insertion
- avoid ABI calls during frame update and callback dispatch
- keep dynamic reload work outside hot paths
- keep the embedded slice available only during initialization; no second permanent copy is needed

## Migration

1. Add public entry aliases, `ScriptRegistrySource`, `RegistryError`, and private `RuntimeScriptRegistry`.
2. Move constructor lookup and library ownership from `ScriptRuntimeState` into the facade.
3. Add `Runtime::from_project_with_registry`; keep current constructors as compatibility shims.
4. Change `perro_app` embedded config to the source facade.
5. Change compiler templates and generated-project tests to `ScriptRegistrySource::embedded`.
6. Generate ABI descriptor plus old symbols.
7. Load descriptor first; fall back to old symbols with a warning.
8. Remove fallback after the compatibility window.

Do not combine this migration with script lifecycle or DLC mount semantics changes.

## Tests

- embedded source resolves each generated constructor hash
- dynamic adapter rejects wrong ABI version and short descriptor
- dynamic adapter rejects build-fingerprint mismatch before registry reads
- invalid index/get result leaves live registry unchanged
- duplicate source hash fails deterministically
- failed overlay load leaves base registry intact
- library owner outlives constructed behavior and state
- dynamic reload drops behavior/state before library owner
- generated native, Android, and web crates compile with embedded source
- dev runner loads descriptor backend
- legacy-symbol fallback works during compatibility window
- constructor lookup benchmark shows no frame-path regression

