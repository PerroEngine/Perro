# Generic Structs

## Page Map

| Header                | Link                                            |
| --------------------- | ----------------------------------------------- |
| Struct Table          | [Struct Table](#struct-table)                   |
| Variant Support       | [Variant Support](#variant-support)             |
| Matrix Structs        | [Matrix Structs](#matrix-structs)               |
| Color                 | [Color](#color)                                 |
| Masks And Collision   | [Masks And Collision](#masks-and-collision)     |
| Audio Structs         | [Audio Structs](#audio-structs)                 |
| Post Process Structs  | [Post Process Structs](#post-process-structs)   |
| Accessibility Structs | [Accessibility Structs](#accessibility-structs) |
| Misc Structs          | [Misc Structs](#misc-structs)                   |

## Struct Table

| Type                                                                       | Shape / stored data                                                 | Where it appears / why documented                       |
| -------------------------------------------------------------------------- | ------------------------------------------------------------------- | ------------------------------------------------------- |
| `Color`                                                                    | `r/g/b/a: Unit`; input/output commonly `[f32; 4]` in `0.0..=1.0`.   | Script APIs, UI/style values, tint/modulate fields.     |
| `BitMask`                                                                  | `u32` bit field; public layers use `1..=32`.                        | Collision, input, audio, and custom category filters.   |
| `CollisionPolicy`                                                          | `layers: BitMask`, `mask: BitMask`; mask means ignored layers.      | Physics node config and collision compatibility checks. |
| `Vector4`, `IVector4`, `UVector4`                                          | Four `x/y/z/w` lanes; float, signed int, or unsigned int.            | Generic four-value data, not rotation-specific like `Quaternion`. |
| `Matrix<ROWS, COLS, T>`                                                     | Row-major matrices.                                                  | Packed math data, scene/resource values, and dynamic data. |
| `SqMatrix<SZ, T>`                                                           | Alias for `Matrix<SZ, SZ, T>`.                                       | Square matrix shorthand when rows and columns match.     |
| `Matrix2`, `Matrix3`, `Matrix4`                                            | Glam-backed fast `f32` matrices.                                     | Hot matrix ops and transform conversion.                  |
| `AudioMaterial`, `AudioEffect`, `AudioInteraction`, `AudioListenerOptions` | `f32` tuning fields plus `BitMask` and effect lists.                | Built-in audio node/resource/listener config.           |
| `PostProcessEffect`, `PostProcessEntry`, `PostProcessSet`                  | enum effects plus named/unnamed effect entries.                     | Render effect stacks and resource API config.           |
| `ColorBlindFilter`, `ColorBlindSetting`, `VisualAccessibilitySettings`     | enum filter plus `strength: f32` and optional setting.              | Display accessibility state and resource API config.    |
| `ConstParamValue`                                                          | enum: `F32`, `I32`, `Bool`, `Vec2`, `Vec3`, `Vec4`.                 | Shader/material/post-process constant values.           |
| `IKTargetParams`, `IKTargetSolver`                                         | IK target fields plus solver enum.                                  | Built-in skeletal IK node config.                       |
| `Unit`, `UnitVector2`, `UnitVector3`, `UnitVector4`                        | Values clamped to `0.0..=1.0`; scalar and vector components store as `u8`. | Unit controls + packed color/weight data.              |

## Variant Support

These engine structs can be passed through `Variant` with `Variant::from(value)` and decoded with `parse::<T>()` or the listed `as_*` accessor.

| Type                                  | Accessor                         | JSON/object shape          | Scene/editor type                         | Notes                                      |
| ------------------------------------- | -------------------------------- | -------------------------- | ----------------------------------------- | ------------------------------------------ |
| `Vector2`, `Vector3`, `Vector4`       | `as_vec2()`, `as_vec3()`, `as_vec4()` | `{ x, y }`, `{ x, y, z }`, `{ x, y, z, w }` | `Vec2`, `Vec3`, `Vec4`                    | Float lanes.                               |
| `IVector2`, `IVector3`, `IVector4`    | `as_ivec2()`, `as_ivec3()`, `as_ivec4()` | `{ x, y }`, `{ x, y, z }`, `{ x, y, z, w }` | `IVec2`, `IVec3`, `IVec4`                 | Signed integer lanes.                     |
| `UVector2`, `UVector3`, `UVector4`    | `as_uvec2()`, `as_uvec3()`, `as_uvec4()` | `{ x, y }`, `{ x, y, z }`, `{ x, y, z, w }` | `UVec2`, `UVec3`, `UVec4`                 | Unsigned integer lanes.                   |
| `UnitVector2`, `UnitVector3`, `UnitVector4` | `as_unit_vec2()`, `as_unit_vec3()`, `as_unit_vec4()` | `{ x, y }`, `{ x, y, z }`, `{ x, y, z, w }` | `UnitVector2`, `UnitVector3`, `UnitVector4` | Each lane clamps to `0.0..=1.0` and stores as `u8`. |
| `Matrix2`, `Matrix3`, `Matrix4` | `as_matrix2()`, `as_matrix3()`, `as_matrix4()` | Row arrays like `[[1.0, 0.0], [0.0, 1.0]]` or flat row-major arrays. | `Matrix2`, `Matrix3`, `Matrix4` | Fast glam-backed values. |
| `Matrix<ROWS, COLS, T>`, `SqMatrix<SZ, T>` | `parse::<T>()`, `into_parse::<T>()`, `matrix_shape()` | Row arrays, flat row-major arrays, or `{ rows: [...] }`. | Matrix grid | Any const size; `matrix_shape()` returns rows, cols, and `cell_type`; cells must support Variant when crossing runtime state/method boundaries. |
| `Matrix<2, 2>`, `Matrix<3, 3>`, `Matrix<4, 4>`, `SqMatrix<2>`, `SqMatrix<3>`, `SqMatrix<4>` | `as_matrix2x2()`, `as_matrix3x3()`, `as_matrix4x4()` | Row arrays, flat row-major arrays, or `{ rows: [...] }`. | `Matrix2`, `Matrix3`, `Matrix4` | `f32` square matrices use glam-backed fast Variant storage. |

`UnitVector*` means vector of unit-range values, not a length-normalized direction vector.

## Matrix Structs

Perro has one row-major generic matrix plus glam-backed square wrappers.

Use `Matrix<ROWS, COLS, T>` when row-major storage matters.

Use `SqMatrix<SZ, T>` when row and column count match.

`T` can be any element type.

Use `Matrix2`, `Matrix3`, and `Matrix4` when you want glam-backed fast math.

Row-major means `rows[row][col]`.

Variant and JSON row arrays also use row-major order.

Common use:

| Type | Use |
| ---- | --- |
| `Matrix<2, 2>` | Small 2D math values and compact dynamic data. |
| `Matrix<3, 3>` | 2D transform math, normal basis, and row-major scene data. |
| `Matrix<4, 4>` | 3D transform/projection math and packed resource data. |
| `SqMatrix<5, u8>` | 5x5 compact unsigned byte matrix shorthand. |
| `Matrix2`/`Matrix3`/`Matrix4` | Hot math ops; backed by `glam::Mat2`, `glam::Mat3`, `glam::Mat4`. |

Common APIs:

| Access | Signature | Use |
| ------ | --------- | --- |
| Constructor | `Matrix::<R, C>::new(rows)` | Build row-major matrix. |
| Constructor | `Matrix::<N, N>::identity()` | Build square identity. |
| Shape | `rows_len()`, `cols_len()`, `shape()`, `cell_count()`, `flat_len()`, `is_square()` | Read compile-time matrix shape as values. |
| Accessor | `rows()`, `rows_mut()`, `row(i)`, `row_mut(i)`, `as_slice()`, `as_mut_slice()` | Read/write row-major storage. |
| Position | `flat_index(row, col)`, `row_col(index)`, `in_bounds(row, col)` | Convert between `row,col` and flat row-major index. |
| Accessor | `get(row, col)`, `get_mut(row, col)`, `get_flat(index)`, `get_flat_mut(index)`, `set(row, col, value)` | Safe checked element access. |
| Iter | `iter()`, `iter_mut()`, `cells()`, `cells_mut()`, `for_each(fn)`, `for_each_mut(fn)` | Walk cells in row-major order without alloc. |
| Row/col iter | `rows_iter()`, `rows_iter_mut()`, `row_iter(row)`, `row_iter_mut(row)`, `col_iter(col)`, `col_iter_mut(col)` | Read/write rows or columns with checked iterator setup. |
| Query | `any_cell(fn)`, `all_cells(fn)`, `count_cells(fn)`, `find_cell(fn)` | Query cells without building temporary arrays. |
| Search | `find_position(value)`, `find_flat_index(value)` | Find first matching element. |
| Fill/copy | `fill(value)`, `fill_with(fn)`, `copy_from_slice(input)`, `copy_to_slice(out)`, `clone_from_matrix(src)` | Reuse matrix storage and avoid temporary Vecs. |
| Swap | `swap_cells((r, c), (r, c))`, `swap_flat(a, b)` | Swap values with checked indices. |
| Pack | `write_flat(out)`, `from_slice(input)`, `from_vec(input)`, `from_vec_offset(input, offset)` | Copy row-major data. Vec input may contain extra tail values. |
| Pack rows | `from_vec_rows(rows)`, `from_vec_rows_offset(rows, row_offset, col_offset)` | Build from row vecs. Extra rows/columns are ignored. |
| Pack f32 | `packed_len()`, `write_packed(out)`, `read_packed(input)` | Copy row-major `f32` data without alloc. |
| Bytes | `as_bytes()` | View packed `f32` bytes for upload/cache keys. |
| Convert rows/cols | `into_rows()`, `to_rows()`, `into_cols()`, `to_cols()` | Convert to fixed arrays. `to_*` copies, `into_*` consumes. |
| Convert vec | `to_vec()`, `into_vec()` | Build row-major Vec when dynamic storage is required. |
| Resize/map | `resize::<R, C>(fill)`, `resize_default::<R, C>()`, `resize_with::<R, C>(fn)`, `map_cells(fn)` | Build resized or mapped matrices. |
| Aggregate | `sum()`, `product()`, `fold_cells(init, fn)`, `min_cell()`, `max_cell()` | Reduce matrix values without temporary collections. |
| Math | `+`, `-`, `+=`, `-=`, scalar `*`, scalar `/`, matrix `*` | Uses engine-optimized paths where useful and supported. |
| Integer math | `<< u32`, `>> u32`, `<<= u32`, `>>= u32` | Element-wise bit shifts for integer matrices. |
| Compat aliases | `add_fast`, `sub_fast`, `scale_fast`, `add_f32`, `sub_f32`, `scale_f32`, `mul_f32` | Kept for older code; normal operators are preferred. |
| Convert | `to_glam()`, `from_glam(mat)` | Bridge generic square matrices to glam. |
| Fast convert | `Matrix3::from_rows(rows)`, `to_rows()` | Bridge row-major data to glam-backed values. |

SIMD coverage:

| Element type | SIMD ops |
| ------------ | -------- |
| `f32` | add, sub, scale |
| `f64` | add, sub, scale |
| `i8`, `u8` | add, sub |
| `i16`, `u16` | add, sub, scale |
| `i32`, `u32` | add, sub, scale |
| `i64`, `u64` | add, sub |
| `i128`, `u128`, `isize`, `usize` | scalar fast path |

Other engine math structs already use vectorized paths through `glam` where possible.

`Vector2`, `Vector3`, `Vector4`, `Quaternion`, `Transform2D`, and `Transform3D` route hot float math through glam-backed values.

Grid shape and checked access:

```rust
let mut tiles = Matrix::<3, 4, u8>::new([
    [0, 0, 1, 1],
    [0, 2, 2, 1],
    [3, 3, 0, 0],
]);

assert_eq!(Matrix::<3, 4, u8>::shape(), (3, 4));
assert_eq!(Matrix::<3, 4, u8>::cell_count(), 12);
assert!(!Matrix::<3, 4, u8>::is_square());

let flat = Matrix::<3, 4, u8>::flat_index(1, 2).unwrap();
assert_eq!(flat, 6);
assert_eq!(Matrix::<3, 4, u8>::row_col(flat), Some((1, 2)));

assert_eq!(tiles.get(1, 2), Some(&2));
assert!(tiles.set(0, 2, 4));
assert_eq!(tiles.get_flat(2), Some(&4));
```

Iteration and queries:

```rust
let mut damage = Matrix::<2, 3, i32>::new([
    [0, 4, 0],
    [2, 0, 8],
]);

let total_damage: i32 = damage.iter().copied().sum();
assert_eq!(total_damage, 14);

let cells: Vec<(usize, usize, i32)> =
    damage.cells().map(|(row, col, value)| (row, col, *value)).collect();
assert_eq!(cells[5], (1, 2, 8));

damage.for_each_mut(|_, _, value| {
    *value = (*value - 1).max(0);
});

assert!(damage.any_cell(|_, _, value| *value > 0));
assert!(damage.all_cells(|_, _, value| *value >= 0));
assert_eq!(damage.count_cells(|_, _, value| *value > 0), 3);
assert_eq!(damage.find_cell(|_, _, value| *value >= 7), Some((1, 2)));
```

Row and column iteration:

```rust
let mut spawn_weights = Matrix::<3, 3, i32>::new([
    [1, 1, 1],
    [2, 2, 2],
    [3, 3, 3],
]);

let middle_row: Vec<i32> = spawn_weights.row_iter(1).unwrap().copied().collect();
let right_col: Vec<i32> = spawn_weights.col_iter(2).unwrap().copied().collect();

assert_eq!(middle_row, vec![2, 2, 2]);
assert_eq!(right_col, vec![1, 2, 3]);

spawn_weights.row_iter_mut(0).unwrap().for_each(|value| *value += 1);
spawn_weights.col_iter_mut(2).unwrap().for_each(|value| *value *= 2);

assert_eq!(spawn_weights.to_rows(), [[2, 2, 4], [2, 2, 4], [3, 3, 6]]);
```

Fill, copy, and swap:

```rust
let mut costs = Matrix::<2, 3, u16>::default();

costs.fill(1);
costs.fill_with(|row, col| (row * 10 + col) as u16);
assert_eq!(costs.to_rows(), [[0, 1, 2], [10, 11, 12]]);

assert!(costs.copy_from_slice(&[5, 5, 2, 9, 9, 1]));

let mut out = [0; 6];
assert_eq!(costs.copy_to_slice(&mut out), Some(6));
assert_eq!(out, [5, 5, 2, 9, 9, 1]);

let flat = Matrix::<2, 3, u16>::from_vec(vec![7, 8, 9, 10, 11, 12, 99]).unwrap();
assert_eq!(flat.to_rows(), [[7, 8, 9], [10, 11, 12]]);

let offset = Matrix::<2, 3, u16>::from_vec_offset(vec![0, 7, 8, 9, 10, 11, 12], 1).unwrap();
assert_eq!(offset.to_rows(), [[7, 8, 9], [10, 11, 12]]);

let rows = Matrix::<2, 3, u16>::from_vec_rows(vec![
    vec![7, 8, 9, 99],
    vec![10, 11, 12, 99],
])
.unwrap();
assert_eq!(rows.to_rows(), [[7, 8, 9], [10, 11, 12]]);

let row_window = Matrix::<2, 3, u16>::from_vec_rows_offset(
    vec![vec![0, 0, 0, 0], vec![0, 7, 8, 9], vec![0, 10, 11, 12]],
    1,
    1,
)
.unwrap();
assert_eq!(row_window.to_rows(), [[7, 8, 9], [10, 11, 12]]);

let imported = Matrix::<2, 3, u16>::new([[7, 8, 9], [10, 11, 12]]);
costs.clone_from_matrix(&imported);

assert!(costs.swap_cells((0, 0), (1, 2)));
assert!(costs.swap_flat(1, 4));
```

Convert, resize, map, aggregate:

```rust
let threat = Matrix::<2, 3, i32>::new([
    [1, 0, 3],
    [4, 2, 0],
]);

assert_eq!(threat.to_rows(), [[1, 0, 3], [4, 2, 0]]);
assert_eq!(threat.to_cols(), [[1, 4], [0, 2], [3, 0]]);
assert_eq!(threat.to_vec(), vec![1, 0, 3, 4, 2, 0]);

let small = threat.resize::<1, 2>(0);
let large = threat.resize_with::<3, 4>(|_, _| -1);
let scaled = threat.map_cells(|_, _, value| value * 2);

assert_eq!(small.to_rows(), [[1, 0]]);
assert_eq!(large.to_rows(), [[1, 0, 3, -1], [4, 2, 0, -1], [-1, -1, -1, -1]]);
assert_eq!(scaled.to_rows(), [[2, 0, 6], [8, 4, 0]]);

assert_eq!(threat.sum(), 10);
assert_eq!(threat.min_cell(), Some((0, 1, 0)));
assert_eq!(threat.max_cell(), Some((1, 0, 4)));

let diagonal_threat = threat.fold_cells(0, |sum, row, col, value| {
    if row == col { sum + *value } else { sum }
});
assert_eq!(diagonal_threat, 3);
```

Variant forms:

```rust
let rows = Variant::from(Matrix::<3, 3>::identity());

let same = Variant::Array(vec![
    Variant::Array(vec![1.0_f32.into(), 0.0_f32.into(), 0.0_f32.into()]),
    Variant::Array(vec![0.0_f32.into(), 1.0_f32.into(), 0.0_f32.into()]),
    Variant::Array(vec![0.0_f32.into(), 0.0_f32.into(), 1.0_f32.into()]),
]);

let flat = Variant::Array(vec![
    1.0_f32.into(), 0.0_f32.into(), 0.0_f32.into(),
    0.0_f32.into(), 1.0_f32.into(), 0.0_f32.into(),
    0.0_f32.into(), 0.0_f32.into(), 1.0_f32.into(),
]);
```

## Color

`Color` stores four `Unit` channels, not four `f32` fields.

Use `Color` when an API or resource needs RGBA color as typed data. The float constructors take channel values in the `0.0..=1.0` range, clamp out-of-range values, and round to bytes for storage.

Signature:

```rust
pub struct Color {
    pub r: Unit,
    pub g: Unit,
    pub b: Unit,
    pub a: Unit,
}
```

Storage:

| Public input/output                                     | Internal storage                           | Edge behavior                                                       |
| ------------------------------------------------------- | ------------------------------------------ | ------------------------------------------------------------------- |
| `f32` channels use `0.0..=1.0`.                         | Each channel stores `u8` through `Unit`. | Values below `0.0` clamp to `0`; values above `1.0` clamp to `255`. |
| Hex strings use `RGB`, `RGBA`, `RRGGBB`, or `RRGGBBAA`. | Hex parse stores exact bytes.              | Invalid length or digit returns `None`.                             |
| Float slice output uses `[f32; 4]`.                     | Bytes convert back to normalized floats.   | Round trip through bytes can quantize.                              |

Common APIs:

| Access      | Signature                                                  | Params                                                              | Returns         | Use when                                                             | Why / edge behavior                                |
| ----------- | ---------------------------------------------------------- | ------------------------------------------------------------------- | --------------- | -------------------------------------------------------------------- | -------------------------------------------------- |
| Constructor | `pub const fn new(r: f32, g: f32, b: f32, a: f32) -> Self` | `r/g/b/a`: normalized `f32` channels.                               | `Color`         | Build explicit RGBA.                                                 | Clamps to `0.0..=1.0`, rounds, stores as `Unit`. |
| Constructor | `pub const fn rgb(r: f32, g: f32, b: f32) -> Self`         | `r/g/b`: normalized `f32` channels.                                 | `Color`         | Build opaque color.                                                  | Sets alpha to `1.0`.                               |
| Constructor | `pub const fn from_rgba(v: [f32; 4]) -> Self`              | `[r, g, b, a]` normalized floats.                                   | `Color`         | Convert from array data.                                             | Same clamp/round/storage as `new`.                 |
| Constructor | `pub const fn from_float_slice(v: [f32; 4]) -> Self`       | Normalized RGBA floats.                                             | `Color`         | Convert from float slice/array data.                                 | Alias of `from_rgba`.                              |
| Constructor | `pub const fn from_rgba_u8(v: [u8; 4]) -> Self`            | Exact byte channels.                                                | `Color`         | Preserve imported byte color.                                        | Stores exact channel bytes.                        |
| Constructor | `pub const fn from_unit_vector4(v: UnitVector4) -> Self`          | Packed normalized bytes.                                            | `Color`         | Convert from compact normalized color.                               | Uses exact stored bytes.                           |
| Constructor | `pub const fn from_unit_slice(v: UnitVector4) -> Self`       | Packed normalized bytes.                                            | `Color`         | Convert from APIs that name packed normalized data as a slice value. | Alias of `from_unit_vector4`.                          |
| Parser      | `pub fn from_hex(hex: &str) -> Option<Self>`               | `"#RGB"`, `"#RGBA"`, `"#RRGGBB"`, `"#RRGGBBAA"`, with optional `#`. | `Option<Color>` | Parse author-facing color text.                                      | Returns `None` for bad length or bad hex digit.    |
| Parser      | `pub const fn from_hex_const(hex: &str) -> Self`           | Same hex forms as `from_hex`.                                      | `Color`         | Compile-time hex behind the `color!` macro.                          | Panics (compile error in `const`) on malformed input. |
| Builder     | `pub const fn with_alpha(self, a: f32) -> Self`            | `a`: normalized alpha.                                             | `Color`         | Override alpha on an existing color without a parse/alloc.           | Clamps `a` to `0.0..=1.0`; RGB kept. `const`.      |
| Accessor    | `pub const fn r(self) -> f32` and `g/b/a`                  | none                                                                | `f32`           | Read one normalized channel.                                         | Converts stored byte to `0.0..=1.0`.               |
| Accessor    | `pub const fn to_rgba(self) -> [f32; 4]`                   | none                                                                | `[f32; 4]`      | Feed APIs that expect float RGBA arrays.                             | Converts stored bytes to normalized floats.        |
| Accessor    | `pub const fn to_rgb(self) -> [f32; 3]`                    | none                                                                | `[f32; 3]`      | Feed RGB-only APIs.                                                  | Drops alpha.                                       |
| Accessor    | `pub const fn to_float_slice(self) -> [f32; 4]`            | none                                                                | `[f32; 4]`      | Feed float-slice/array APIs.                                         | Alias of `to_rgba`.                                |
| Accessor    | `pub const fn to_rgba_u8(self) -> [u8; 4]`                 | none                                                                | `[u8; 4]`       | Save or compare exact stored bytes.                                  | No float conversion loss.                          |
| Accessor    | `pub const fn to_unit_vector4(self) -> UnitVector4`               | none                                                                | `UnitVector4`      | Pass compact normalized bytes.                                       | Uses exact stored bytes.                           |
| Accessor    | `pub const fn to_unit_slice(self) -> UnitVector4`            | none                                                                | `UnitVector4`      | Feed APIs that name packed normalized data as a slice value.         | Alias of `to_unit_vector4`.                            |
| Formatter   | `pub fn to_hex_rgb(self) -> String`                        | none                                                                | `String`        | Save/debug opaque color text.                                        | Alpha omitted.                                     |
| Formatter   | `pub fn to_hex_rgba(self) -> String`                       | none                                                                | `String`        | Save/debug full color text.                                          | Alpha included.                                    |

Constants:

`WHITE`, `BLACK`, `GRAY`, `GREY`, `LIGHT_GRAY`, `LIGHT_GREY`, `DARK_GRAY`, `DARK_GREY`, `RED`, `MAROON`, `CRIMSON`, `GREEN`, `LIME`, `FOREST_GREEN`, `OLIVE`, `MINT`, `BLUE`, `NAVY`, `ROYAL_BLUE`, `SKY_BLUE`, `CORNFLOWER_BLUE`, `ORANGE`, `YELLOW`, `INDIGO`, `VIOLET`, `CYAN`, `TEAL`, `TURQUOISE`, `MAGENTA`, `PINK`, `PURPLE`, `BROWN`, `GOLD`, `TRANSPARENT`.

Example:

```rust
let exact = Color::from_rgba_u8([0x33, 0x66, 0x99, 0xCC]);
let from_packed = Color::from_unit_vector4(UnitVector4::from_u8([0x33, 0x66, 0x99, 0xCC]));
let accent = Color::new(0.2, 0.4, 0.6, 0.8);
let clamped = Color::new(1.5, 0.5, -1.0, 2.0);

assert_eq!(exact.to_hex_rgba(), "#336699CC");
assert_eq!(from_packed.to_rgba_u8(), [0x33, 0x66, 0x99, 0xCC]);
assert_eq!(clamped.to_rgba_u8(), [255, 128, 0, 255]);

let rgba: [f32; 4] = accent.to_float_slice();
let packed: UnitVector4 = accent.to_unit_slice();
```

Use the `color!` macro for compile-time-validated hex literals, and `with_alpha`
to fade a base color without re-parsing each frame:

```rust
const PANEL_BG: Color = color!("#0B1018");

// per-frame fade: no String alloc, no runtime parse
node.style.fill = PANEL_BG.with_alpha(0.92 * t);
```

## Masks And Collision

`BitMask` and `CollisionPolicy` document the layer/mask data used by collision and other category-filtered systems.

Layer numbers passed to public helpers are one-based: layer `1` maps to bit `0`, layer `32` maps to bit `31`.

`CollisionPolicy.mask` is an ignore mask. `can_collide` returns false when either policy masks out the other policy's layers.

These types often appear as fields inside physics nodes and other built-ins. They are documented so the public shape and collision rules are clear even when a script only sees them indirectly.

Common APIs:

| Access      | Signature                                                    | Params                         | Returns           | Use when                                    | Why / edge behavior                       |
| ----------- | ------------------------------------------------------------ | ------------------------------ | ----------------- | ------------------------------------------- | ----------------------------------------- |
| Constant    | `pub const NONE: BitMask`                                    | none                           | `BitMask`         | Match no layers.                            | Bits all zero.                            |
| Constant    | `pub const ALL: BitMask`                                     | none                           | `BitMask`         | Match all layers.                           | Bits all one.                             |
| Constructor | `pub const fn from_bits(bits: u32) -> Self`                  | Raw bit field.                 | `BitMask`         | Load saved mask bits.                       | Uses bits exactly.                        |
| Accessor    | `pub const fn bits(self) -> u32`                             | none                           | `u32`             | Save/debug raw mask bits.                   | Raw storage value.                        |
| Constructor | `pub const fn layer(layer: u8) -> Self`                      | One-based layer `1..=32`.      | `BitMask`         | Build one layer.                            | Panics if layer outside `1..=32`.         |
| Constructor | `pub const fn try_layer(layer: u8) -> Option<Self>`          | One-based layer.               | `Option<BitMask>` | Parse user data safely.                     | Returns `None` outside `1..=32`.          |
| Constructor | `pub const fn with<const N: usize>(layers: [u8; N]) -> Self` | One-based layer array.         | `BitMask`         | Build const mask.                           | Panics if any layer outside `1..=32`.     |
| Constructor | `pub fn from_layers<I, L>(layers: I) -> Self`                | Layer iterator.                | `BitMask`         | Build runtime mask from arrays/slices/vecs. | Panics if any layer outside `1..=32`.     |
| Constructor | `pub fn try_from_layers<I, L>(layers: I) -> Option<Self>`    | Layer iterator.                | `Option<BitMask>` | Parse runtime mask safely.                  | Returns `None` if any layer invalid.      |
| Mutator     | `pub fn push<L>(&mut self, layers: L)`                       | One layer or layer collection. | `()`              | Add layers in place.                        | Panics on invalid layer.                  |
| Builder     | `pub fn pushed<L>(self, layers: L) -> Self`                  | One layer or layer collection. | `BitMask`         | Get mask with added layers.                 | Panics on invalid layer.                  |
| Mutator     | `pub fn pop<L>(&mut self, layers: L)`                        | One layer or layer collection. | `()`              | Remove layers in place.                     | Panics on invalid layer.                  |
| Builder     | `pub fn popped<L>(self, layers: L) -> Self`                  | One layer or layer collection. | `BitMask`         | Get mask with removed layers.               | Panics on invalid layer.                  |
| Builder     | `pub fn without<L>(layers: L) -> Self`                       | One layer or layer collection. | `BitMask`         | Start from all layers minus some.           | Panics on invalid layer.                  |
| Query       | `pub const fn contains(self, other: Self) -> bool`           | Other mask.                    | `bool`            | Check full inclusion.                       | True only if every bit in `other` exists. |
| Query       | `pub const fn intersects(self, other: Self) -> bool`         | Other mask.                    | `bool`            | Check any overlap.                          | True if any bit overlaps.                 |
| Query       | `pub const fn is_empty(self) -> bool`                        | none                           | `bool`            | Check no layers.                            | True only for zero bits.                  |
| Constructor | `pub const fn new(layers: BitMask, mask: BitMask) -> Self`   | Layer mask + ignore mask.      | `CollisionPolicy` | Build collision policy directly.            | `mask` means ignored layers.              |
| Query       | `pub const fn can_collide(self, other: Self) -> bool`        | Other policy.                  | `bool`            | Test two policy values.                     | False if either side ignores the other.   |

Example:

```rust
let enemy_layers = BitMask::with([2, 5]);
let player_policy = CollisionPolicy::new(BitMask::with([1]), enemy_layers);
let wall_policy = CollisionPolicy::new(BitMask::with([3]), BitMask::NONE);

let hit_wall = player_policy.can_collide(wall_policy);
let has_enemy = enemy_layers.intersects(BitMask::layer(2));
```

## `Unit` And `UnitVector4`

Use `Unit` when one normalized float needs compact byte storage.

Use `UnitVector2` or `UnitVector3` when each component is a `0.0..=1.0` unit value.

`UnitVector2` and `UnitVector3` are not length-normalized direction vectors. They are vectors of unit-range values.

Use `UnitVector4` when four normalized floats need packed byte storage.

| Access      | Signature                                            | Params                  | Returns    | Use when                                     | Why / edge behavior                      |
| ----------- | ---------------------------------------------------- | ----------------------- | ---------- | -------------------------------------------- | ---------------------------------------- |
| Constructor | `pub const fn Unit::new(v: f32) -> Self`            | Normalized `f32`.       | `Unit`    | Pack one normalized value.                   | Clamps to `0.0..=1.0`, rounds to `u8`.   |
| Constructor | `pub const fn Unit::from_u8(v: u8) -> Self`         | Exact byte.             | `Unit`    | Keep imported byte value exact.              | No clamp needed.                         |
| Accessor    | `pub const fn to_u8(self) -> u8`                     | none                    | `u8`       | Save exact byte.                             | No conversion loss.                      |
| Accessor    | `pub const fn to_f32(self) -> f32`                   | none                    | `f32`      | Feed normalized float APIs.                  | Returns `byte / 255.0`.                  |
| Constructor | `pub const fn UnitVector2::new(x: f32, y: f32) -> Self` | Unit-range components. | `UnitVector2` | Store two unit values.                   | Clamps each component to `0.0..=1.0`.    |
| Constructor | `pub const fn UnitVector3::new(x: f32, y: f32, z: f32) -> Self` | Unit-range components. | `UnitVector3` | Store three unit values.              | Clamps each component to `0.0..=1.0`.    |
| Constructor | `pub const fn UnitVector4::new(v: [f32; 4]) -> Self`    | Four normalized floats. | `UnitVector4` | Pack RGBA-like data.                         | Clamps and rounds each channel.          |
| Constructor | `pub const fn UnitVector4::from_u8(v: [u8; 4]) -> Self` | Exact bytes.            | `UnitVector4` | Keep byte data exact.                        | No clamp needed.                         |
| Accessor    | `pub const fn to_u8(self) -> [u8; 4]`                | none                    | `[u8; 4]`  | Save exact packed bytes.                     | No conversion loss.                      |
| Accessor    | `pub const fn to_f32(self) -> [f32; 4]`              | none                    | `[f32; 4]` | Feed normalized float APIs.                  | Converts each byte to `byte / 255.0`.    |
| Accessor    | `pub const fn to_le_u32(self) -> u32`                | none                    | `u32`      | Pack four bytes into one little-endian word. | Byte order follows `u32::from_le_bytes`. |

Example:

```rust
let packed = UnitVector4::new([1.0, 0.5, -1.0, 2.0]);

assert_eq!(packed.to_u8(), [255, 128, 0, 255]);
assert_eq!(packed.to_le_u32(), 0xFF00_80FF);
```

## Audio Structs

Audio structs document propagation, occlusion, material response, and listener data used by built-in audio systems.

These values usually sit inside audio nodes, audio resources, or listener options. Some scripts build them directly; other code reads them through node/resource APIs.

Signatures:

```rust
pub struct AudioMaterial {
    pub absorption: f32,
    pub reflection: f32,
    pub transmission: f32,
    pub diffusion: f32,
    pub low_pass_strength: f32,
    pub thickness_multiplier: f32,
    pub audio_mask: BitMask,
}

pub struct AudioDiffusion {
    pub damping: f32,
    pub compression: f32,
    pub hardness: f32,
}

pub struct AudioInteraction {
    pub material: AudioMaterial,
    pub diffusion: AudioDiffusion,
}

pub struct AudioEffect {
    pub reverb_send: f32,
    pub echo: f32,
    pub dampening: f32,
}

pub struct AudioListenerOptions {
    pub audio_mask: BitMask,
    pub effects: Vec<AudioEffect>,
}
```

Common APIs:

| Access      | Signature                                          | Params | Returns                | Use when                            | Why / edge behavior                                                                                                                         |
| ----------- | -------------------------------------------------- | ------ | ---------------------- | ----------------------------------- | ------------------------------------------------------------------------------------------------------------------------------------------- |
| Constructor | `pub const fn AudioMaterial::new() -> Self`        | none   | `AudioMaterial`        | Start material tuning.              | Defaults absorption/reflection to `0.35`, transmission/diffusion to `0.15`, low-pass to `0.5`, thickness to `1.0`, mask to `BitMask::NONE`. |
| Constructor | `pub const fn AudioDiffusion::new() -> Self`       | none   | `AudioDiffusion`       | Start diffusion tuning.             | Defaults damping `0.35`, compression `0.15`, hardness `0.5`.                                                                                |
| Constructor | `pub const fn AudioInteraction::new() -> Self`     | none   | `AudioInteraction`     | Bundle material plus diffusion.     | Uses both default constructors.                                                                                                             |
| Constructor | `pub const fn AudioEffect::new() -> Self`          | none   | `AudioEffect`          | Start listener/zone effect tuning.  | Defaults reverb `0.35`, echo `0.0`, dampening `0.0`.                                                                                        |
| Constructor | `pub const fn AudioListenerOptions::new() -> Self` | none   | `AudioListenerOptions` | Build listener mask/effects config. | Starts with `BitMask::NONE` and empty effect list.                                                                                          |

Example:

```rust
let mut material = AudioMaterial::new();
material.absorption = 0.8;
material.transmission = 0.05;
material.audio_mask = BitMask::with([1, 4]);

let mut listener = AudioListenerOptions::new();
listener.effects.push(AudioEffect {
    reverb_send: 0.4,
    echo: 0.1,
    dampening: 0.2,
});
```

## Post Process Structs

Post-process structs document global render effect config.

`PostProcessSet` is a stack-like value. Some scripts may build one directly; other code may only encounter it as renderer/resource state.

Signatures:

```rust
pub enum PostProcessEffect {
    Blur { strength: f32 },
    Pixelate { size: f32 },
    Warp { waves: f32, strength: f32 },
    Vignette { strength: f32, radius: f32, softness: f32 },
    Crt { scanline_strength: f32, curvature: f32, chromatic: f32, vignette: f32 },
    ColorFilter { color: [f32; 3], strength: f32 },
    ReverseFilter { color: [f32; 3], strength: f32, softness: f32 },
    Bloom { strength: f32, threshold: f32, radius: f32 },
    Saturate { amount: f32 },
    BlackWhite { amount: f32 },
    ColorGrade { exposure: f32, contrast: f32, brightness: f32, saturation: f32, gamma: f32, temperature: f32, tint: f32, hue_shift: f32, vibrance: f32, lift: [f32; 3], gain: [f32; 3], offset: [f32; 3] },
    Lut2D { texture_path: Cow<'static, str>, size: u32, strength: f32 },
    Lut3D { texture_path: Cow<'static, str>, size: u32, strength: f32 },
    Custom { shader_path: Cow<'static, str>, params: Vec<CustomPostParam> },
}

pub struct PostProcessEntry {
    pub name: Option<Cow<'static, str>>,
    pub effect: PostProcessEffect,
}
```

Common APIs:

| Access      | Signature                                                                   | Params                       | Returns                      | Use when                         | Why / edge behavior                         |
| ----------- | --------------------------------------------------------------------------- | ---------------------------- | ---------------------------- | -------------------------------- | ------------------------------------------- |
| Constructor | `pub fn PostProcessSet::new() -> Self`                                      | none                         | `PostProcessSet`             | Start an empty effect stack.     | No entries.                                 |
| Constructor | `pub fn from_effects(effects: Vec<PostProcessEffect>) -> Self`              | Effect list.                 | `PostProcessSet`             | Build unnamed stack.             | Wraps each effect as unnamed entry.         |
| Constructor | `pub fn from_entries(entries: Vec<PostProcessEntry>) -> Self`               | Entries.                     | `PostProcessSet`             | Preserve names.                  | Uses entries exactly.                       |
| Constructor | `pub fn from_pairs(effects, names) -> Self`                                 | Effects plus optional names. | `PostProcessSet`             | Pair separate effect/name lists. | Names pad/truncate to match effects length. |
| Constructor | `pub fn PostProcessEntry::named(name, effect) -> Self`                      | Name + effect.               | `PostProcessEntry`           | Address effect later by name.    | Stores name as `Cow<'static, str>`.         |
| Constructor | `pub fn PostProcessEntry::unnamed(effect) -> Self`                          | Effect.                      | `PostProcessEntry`           | Add simple ordered effect.       | Name is `None`.                             |
| Query       | `pub fn entries(&self) -> &[PostProcessEntry]`                              | none                         | slice                        | Inspect stack.                   | Borrow only.                                |
| Query       | `pub fn get(&self, name: &str) -> Option<&PostProcessEffect>`               | Name.                        | `Option<&PostProcessEffect>` | Read named effect.               | Returns `None` if absent.                   |
| Mutator     | `pub fn add(&mut self, name, effect)`                                       | Name + effect.               | `()`                         | Upsert named effect.             | Replaces same name or pushes new entry.     |
| Mutator     | `pub fn add_unnamed(&mut self, effect)`                                     | Effect.                      | `()`                         | Append ordered unnamed effect.   | Always pushes.                              |
| Mutator     | `pub fn remove(&mut self, name: &str) -> Option<PostProcessEffect>`         | Name.                        | `Option<PostProcessEffect>`  | Remove by name.                  | Returns removed effect or `None`.           |
| Mutator     | `pub fn remove_index(&mut self, index: usize) -> Option<PostProcessEffect>` | Index.                       | `Option<PostProcessEffect>`  | Remove by order.                 | Returns `None` if out of range.             |
| Mutator     | `pub fn rename(&mut self, old: &str, new) -> bool`                          | Old/new names.               | `bool`                       | Rename named effect.             | False if old name absent.                   |

Example:

```rust
lifecycle!({
    fn on_init(&self, ctx: &mut ScriptContext<'_, API>) {
        let mut set = PostProcessSet::new();
        set.add("low-health", PostProcessEffect::Vignette {
            strength: 0.6,
            radius: 0.8,
            softness: 0.25,
        });

        post_processing_set!(ctx.res, set);
    }
});
```

## Accessibility Structs

Accessibility structs document player-facing visual correction/display settings.

Signatures:

```rust
pub enum ColorBlindFilter {
    Protan,
    Deuteran,
    Tritan,
    Achroma,
}

pub struct ColorBlindSetting {
    pub filter: ColorBlindFilter,
    pub strength: f32,
}

pub struct VisualAccessibilitySettings {
    pub color_blind: Option<ColorBlindSetting>,
}
```

Common APIs:

| Access      | Signature                                                                            | Params             | Returns                       | Use when                             | Why / edge behavior                        |
| ----------- | ------------------------------------------------------------------------------------ | ------------------ | ----------------------------- | ------------------------------------ | ------------------------------------------ |
| Constructor | `pub fn ColorBlindSetting::new(filter: ColorBlindFilter, strength: f32) -> Self`     | Filter + strength. | `ColorBlindSetting`           | Build one correction setting.        | Strength is passed through as `f32`.       |
| Constructor | `pub const fn VisualAccessibilitySettings::new() -> Self`                            | none               | `VisualAccessibilitySettings` | Start with no correction.            | `color_blind` is `None`.                   |
| Builder     | `pub fn with_color_blind(mut self, filter: ColorBlindFilter, strength: f32) -> Self` | Filter + strength. | `VisualAccessibilitySettings` | Enable correction in settings value. | Replaces any existing color-blind setting. |
| Mutator     | `pub fn clear_color_blind(&mut self)`                                                | none               | `()`                          | Disable correction.                  | Sets `color_blind` to `None`.              |

Example:

```rust
lifecycle!({
    fn on_init(&self, ctx: &mut ScriptContext<'_, API>) {
        enable_colorblind_filter!(ctx.res, ColorBlindFilter::Protan, 0.75);
    }
});
```

## Misc Structs

`ConstParamValue` documents strongly typed constant values passed to material, shader, or post-process systems.

Signature:

```rust
pub enum ConstParamValue {
    F32(f32),
    I32(i32),
    Bool(bool),
    Vec2([f32; 2]),
    Vec3([f32; 3]),
    Vec4([f32; 4]),
}
```

`IKTargetParams` and `IKTargetSolver` document skeletal IK target data used by built-in IK nodes.

Signature:

```rust
pub struct IKTargetParams {
    pub skeleton: NodeID,
    pub bone_index: i32,
    pub chain_length: u32,
    pub iterations: u32,
    pub tolerance: f32,
    pub weight: f32,
    pub match_rotation: bool,
    pub solver: IKTargetSolver,
}

pub enum IKTargetSolver {
    FABRIK,
    CCD,
}
```

Common APIs:

| Access      | Signature                                    | Params | Returns          | Use when                | Why / edge behavior                                                                                                                                                                   |
| ----------- | -------------------------------------------- | ------ | ---------------- | ----------------------- | ------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| Constructor | `pub const fn IKTargetParams::new() -> Self` | none   | `IKTargetParams` | Start IK target config. | Defaults skeleton to `NodeID::nil()`, bone index to `-1`, chain length to `2`, iterations to `8`, tolerance to `0.01`, weight to `1.0`, match rotation to `true`, solver to `FABRIK`. |

Example:

```rust
let mut params = IKTargetParams::new();
params.skeleton = skeleton_id;
params.bone_index = 3;
params.chain_length = 4;
params.solver = IKTargetSolver::CCD;

let color_param = ConstParamValue::Vec4(Color::GOLD.to_rgba());
```
