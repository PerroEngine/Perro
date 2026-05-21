# Variant

## Page Map

| Header        | Link                            |
| ------------- | ------------------------------- |
| Purpose       | [Purpose](#purpose)             |
| Dynamic Calls | [Dynamic Calls](#dynamic-calls) |
| Accessors     | [Accessors](#accessors)         |
| Custom Types  | [Custom Types](#custom-types)   |
| Construction  | [Construction](#construction)   |

## Purpose

`Variant` is the dynamic value type used by script vars, method params, method returns, signals, JSON/network helpers, and scene injected values.

Use it when value type is only known at runtime.

## Dynamic Calls

`get_var!` and `call_method!` return `Variant`.

You must know expected type at call site and decode it.

```rust
let active = call_method!(ctx.run, target, method!("is_active"), params![])
    .as_bool()
    .unwrap_or(false);

let health = get_var!(ctx.run, target, var!("health"))
    .as_i32()
    .unwrap_or(0);
```

`set_var!` and `params![]` convert values into `Variant`.

```rust
set_var!(ctx.run, target, var!("health"), variant!(100_i32));
call_method!(ctx.run, target, method!("set_active"), params![true]);
```

## Accessors

`as_*` accessors are cheap checked reads.

They return `Option<T>`.

Wrong stored type returns `None`.

| Value          | Accessor                                                                                                                                                                          |
| -------------- | --------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| kind enum      | `get_kind()`                                                                                                                                                                      |
| kind name      | `get_kind().as_str()`                                                                                                                                                             |
| null check     | `is_null()`                                                                                                                                                                       |
| bool           | `as_bool()`                                                                                                                                                                       |
| number enum    | `as_number()`                                                                                                                                                                     |
| signed ints    | `as_i8()`, `as_i16()`, `as_i32()`, `as_i64()`, `as_i128()`                                                                                                                        |
| unsigned ints  | `as_u8()`, `as_u16()`, `as_u32()`, `as_u64()`, `as_u128()`                                                                                                                        |
| floats         | `as_f32()`, `as_f64()`                                                                                                                                                            |
| string         | `as_str()`                                                                                                                                                                        |
| bytes          | `as_bytes()`                                                                                                                                                                      |
| any id enum    | `as_id()`                                                                                                                                                                         |
| ids            | `as_node()`, `as_texture()`, `as_material()`, `as_mesh()`, `as_animation()`, `as_light()`, `as_signal()`, `as_audio_bus()`, `as_tag()`, `as_preloaded_scene()` |
| math           | `as_vec2()`, `as_vec3()`, `as_ivec2()`, `as_ivec3()`, `as_uvec2()`, `as_uvec3()`                                                                                                  |
| transforms     | `as_transform2d()`, `as_transform3d()`                                                                                                                                            |
| quaternions    | `as_quat()`                                                                                                                                                                       |
| engine structs | `as_post_process_set()`, `as_visual_accessibility_settings()`                                                                                                                     |
| arrays         | `as_array()`, `as_array_mut()`                                                                                                                                                    |
| objects        | `as_object()`, `as_object_mut()`                                                                                                                                                  |

`Number` also has lossy helpers:

| Value              | Accessor         |
| ------------------ | ---------------- |
| integer-ish number | `as_i64_lossy()` |
| numeric value      | `as_f64_lossy()` |

Use `get_kind()` when you need to branch before decoding.

```rust
match value.get_kind() {
    VariantKind::Bool => {
        let active = value.as_bool().unwrap_or(false);
        let _ = active;
    }
    VariantKind::Number => {
        let n = value.as_i32().or_else(|| value.as_number()?.as_i64_lossy()?.try_into().ok());
        let _ = n;
    }
    VariantKind::ID => {
        if let Some(node) = value.as_node() {
            let _ = node;
        } else if let Some(mesh) = value.as_mesh() {
            let _ = mesh;
        }
    }
    VariantKind::Object => {
        let hit = value.parse::<HitInfo>().ok();
        let _ = hit;
    }
    _ => {}
}
```

`get_kind()` tells broad storage kind.

Use exact `as_*` accessor to know which ID/math/number subtype is stored.

## Custom Types

Use `#[derive(Variant)]` for custom structs/enums used in:

- `#[State]` fields read by `get_var!`
- `set_var!` values
- `methods!` params
- `methods!` returns
- signal params

```rust
#[derive(Clone, Debug, Default, Variant)]
struct HitInfo {
    amount: i32,
}

methods!({
    fn last_hit(&self, ctx: &mut ScriptContext<'_, API>) -> HitInfo {
        HitInfo { amount: 10 }
    }
});

let hit = call_method!(ctx.run, target, method!("last_hit"), params![])
    .into_parse::<HitInfo>()
    .unwrap_or_default();
```

Use `parse::<T>()` when keeping the `Variant`.

Use `into_parse::<T>()` when consuming it.

Both use `DeriveVariant`.

Custom derived types do not get generated `as_my_type()` accessors. Decode them with `parse::<MyType>()` or `into_parse::<MyType>()`.

## Construction

Use `Variant::from(value)`, `variant!(value)`, or `params![...]`.

```rust
let a = Variant::from(true);
let b = variant!(42_i32);
let p = params![true, 42_i32, "name"];
```

For custom types, `#[derive(Variant)]` adds `From<T> for Variant`.
