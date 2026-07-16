# Variant

## Page Map

| Header        | Link                            |
| ------------- | ------------------------------- |
| Purpose       | [Purpose](#purpose)             |
| Use Cases     | [Use Cases](#use-cases)         |
| Dynamic Calls | [Dynamic Calls](#dynamic-calls) |
| Accessors     | [Accessors](#accessors)         |
| Supported Types | [Supported Types](#supported-types) |
| Custom Types  | [Custom Types](#custom-types)   |
| Construction  | [Construction](#construction)   |

## Purpose

`Variant` is the dynamic value type used by script vars, method params, method returns, signals, JSON/network helpers, and scene injected values.

Use it when value type is only known at runtime.

## Use Cases

- Read a value whose type is only known at runtime — a script var, a signal parameter, a scene-injected field: decode with an `as_*` accessor or `parse::<T>()`.
- Pass gameplay data through dynamic calls — `call_method!` params or a `set_var!` value: build it with `variant!` or `params![...]`.
- Save/load and networking: convert game data to and from JSON with `modules::json`, which round-trips through `Variant`.
- Keep custom structs or enums in `#[State]` or send them across scripts: `#[derive(Variant)]`, then recover them with `parse::<T>()` / `into_parse::<T>()`.
- Handle a value of unknown shape safely: branch on `get_kind()` first, then use the exact `as_*` accessor for the stored subtype.

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

Use them when you care about the stored shape.

Use typed decode helpers when you care about the Rust target type.

| Helper | Result | Use |
| ------ | ------ | --- |
| `as_type::<T>()` | `Option<T>` | Borrow `Variant`, no error text |
| `is_type::<T>()` | `bool` | Cheap target-type check |
| `parse::<T>()` | `Result<T, VariantParseError>` | Borrow `Variant`, keep error text |
| `into_type::<T>()` | `Option<T>` | Consume `Variant`, no error text |
| `into_parse::<T>()` | `Result<T, VariantParseError>` | Consume `Variant`, keep error text |

All typed helpers use `DeriveVariant`.

```rust
let hp = value.as_type::<i32>().unwrap_or(0);
let pos = value.parse::<Vector3>()?;
let queue = value.into_type::<VecDeque<NodeID>>().unwrap_or_default();
```

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
| ids            | `as_node()`, `as_node_or_nil()`, `as_texture()`, `as_material()`, `as_mesh()`, `as_animation()`, `as_light()`, `as_signal()`, `as_audio_bus()`, `as_tag()`, `as_preloaded_scene()` |
| math           | `as_vec2()`, `as_vec3()`, `as_vec4()`, `as_ivec2()`, `as_ivec3()`, `as_ivec4()`, `as_uvec2()`, `as_uvec3()`, `as_uvec4()`, `as_unit_vec2()`, `as_unit_vec3()`, `as_unit_vec4()`, `as_matrix2()`, `as_matrix3()`, `as_matrix4()`, `as_matrix2x2()`, `as_matrix3x3()`, `as_matrix4x4()`, `matrix_shape()` |
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

Wrapper/container Rust types do not get unique `VariantKind` values.

They encode into stored shapes.

Examples:

| Rust value | Stored kind |
| ---------- | ----------- |
| `Box<i32>` | `Number(I32)` |
| `Cell<i32>` | `Number(I32)` |
| `Arc<String>` | `String` |
| `(i64, NodeID)` | `Array` |
| `VecDeque<T>` | `Array` |
| `HashMap<String, T>` | `Object` |
| `Duration` | `Object { secs, nanos }` |
| `PathBuf` | `String` |
| `SystemTime` | `Object { secs, nanos }` |

`Vec<u8>` has two common paths:

| Code | Stored kind |
| ---- | ----------- |
| `Variant::from(vec![1_u8, 2])` | `Bytes` |
| `vec![1_u8, 2].to_variant()` | `Array` |

Matrix variants store row-major data.

`Matrix2`/`Matrix3`/`Matrix4` decode through `as_matrix*()`.

`Matrix<ROWS, COLS, T>` and `SqMatrix<SZ, T>` decode through `parse::<T>()` or `into_parse::<T>()`.

Use `matrix_shape()` when you do not know row and column count yet.

It returns `MatrixShape { rows, cols, cell_type }`.

After shape check, branch to the typed matrix you expect.

Matrix cell type must support Variant only when the matrix crosses a Variant boundary.

Local-only matrices can store any `T`.

Accepted matrix parse shapes:

```rust
let rows = Variant::Array(vec![
    Variant::Array(vec![1.0_f32.into(), 0.0_f32.into()]),
    Variant::Array(vec![0.0_f32.into(), 1.0_f32.into()]),
]);

let flat = Variant::Array(vec![
    1.0_f32.into(), 0.0_f32.into(),
    0.0_f32.into(), 1.0_f32.into(),
]);

let matrix = rows.parse::<Matrix<2, 2>>().unwrap();
let same = flat.parse::<Matrix2>().unwrap();

let shape = rows.matrix_shape().unwrap();
let dynamic = match (shape.rows, shape.cols, shape.cell_type) {
    (2, 3, MatrixCellType::F32) => rows.parse::<Matrix<2, 3, f32>>().ok().map(|m| m.to_variant()),
    (5, 5, MatrixCellType::U8) => rows.parse::<SqMatrix<5, u8>>().ok().map(|m| m.to_variant()),
    _ => None,
};
```

## Supported Types

These types support `DeriveVariant` and can be used in `#[State]`, typed `methods!` params/returns, signal params, and typed decode helpers.

Primitive and scalar types:

| Group | Types |
| ----- | ----- |
| unit/bool/text | `()`, `bool`, `char`, `String`, `Arc<str>`, `Box<str>`, `Cow<'static, str>` |
| signed ints | `i8`, `i16`, `i32`, `i64`, `i128`, `isize` |
| unsigned ints | `u8`, `u16`, `u32`, `u64`, `u128`, `usize` |
| floats | `f32`, `f64` |
| non-zero nums | all `NonZero*` int types |
| wrappers | `Wrapping<T>`, `Saturating<T>`, `Reverse<T>` |
| atomics | `AtomicBool`, `AtomicI32`, `AtomicI64`, `AtomicU32`, `AtomicU64`, `AtomicUsize` |
| time/path | `Duration`, `SystemTime`, `PathBuf` |

Engine types:

| Group | Types |
| ----- | ----- |
| ids | `NodeID`, `TextureID`, `MaterialID`, `MeshID`, `AnimationID`, `LightID`, `SignalID`, `AudioBusID`, `TagID`, `PreloadedSceneID` |
| math | `Vector2`, `Vector3`, `Vector4`, `IVector2`, `IVector3`, `IVector4`, `UVector2`, `UVector3`, `UVector4`, `UnitVector2`, `UnitVector3`, `UnitVector4`, `Matrix2`, `Matrix3`, `Matrix4`, `Matrix<ROWS, COLS, T>`, `Quaternion`, `Transform2D`, `Transform3D` |
| misc | `Variant`, `PostProcessSet`, `VisualAccessibilitySettings` |

Std containers and pointers:

| Group | Types |
| ----- | ----- |
| optional/shared | `Option<T>`, `Box<T>`, `Arc<T>`, `Rc<T>`, `Cell<T>`, `RefCell<T>` |
| arrays/slices | `[T; N]`, `Box<[T]>`, `Arc<[T]>`, `Rc<[T]>` |
| sequences | `Vec<T>`, `VecDeque<T>`, `LinkedList<T>`, `BinaryHeap<T>` |
| sets | `BTreeSet<T>`, `HashSet<T>` |
| maps | `BTreeMap<Arc<str>, T>`, `BTreeMap<String, T>`, `BTreeMap<Box<str>, T>`, `BTreeMap<Cow<'static, str>, T>`, plus same key types for `HashMap` |
| ranges | `Range<T>`, `RangeInclusive<T>` |
| tuples | tuple length `2..=6` where each item supports `DeriveVariant` |

Skipped on purpose:

| Type | Reason |
| ---- | ------ |
| `Mutex<T>`, `RwLock<T>` | lock/poison path does not fit infallible `to_variant()` |
| `Instant` | process-local time point, bad save/load value |
| `OnceCell`, `OnceLock` | set-once state semantics do not fit script var mutation |
| `OsString` | platform encoding can fail text roundtrip |

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
