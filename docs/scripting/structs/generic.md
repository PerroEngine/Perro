# Generic Structs

## Page Map

| Header                | Link                                            |
| --------------------- | ----------------------------------------------- |
| Struct Table          | [Struct Table](#struct-table)                   |
| Color                 | [Color](#color)                                 |
| Masks And Collision   | [Masks And Collision](#masks-and-collision)     |
| Audio Structs         | [Audio Structs](#audio-structs)                 |
| Post Process Structs  | [Post Process Structs](#post-process-structs)   |
| Accessibility Structs | [Accessibility Structs](#accessibility-structs) |
| Misc Structs          | [Misc Structs](#misc-structs)                   |

## Struct Table

| Type                                                                       | Shape / stored data                                                 | Where it appears / why documented                       |
| -------------------------------------------------------------------------- | ------------------------------------------------------------------- | ------------------------------------------------------- |
| `Color`                                                                    | `r/g/b/a: Unorm8`; input/output commonly `[f32; 4]` in `0.0..=1.0`. | Script APIs, UI/style values, tint/modulate fields.     |
| `BitMask`                                                                  | `u32` bit field; public layers use `1..=32`.                        | Collision, input, audio, and custom category filters.   |
| `CollisionPolicy`                                                          | `layers: BitMask`, `mask: BitMask`; mask means ignored layers.      | Physics node config and collision compatibility checks. |
| `AudioMaterial`, `AudioEffect`, `AudioInteraction`, `AudioListenerOptions` | `f32` tuning fields plus `BitMask` and effect lists.                | Built-in audio node/resource/listener config.           |
| `PostProcessEffect`, `PostProcessEntry`, `PostProcessSet`                  | enum effects plus named/unnamed effect entries.                     | Render effect stacks and resource API config.           |
| `ColorBlindFilter`, `ColorBlindSetting`, `VisualAccessibilitySettings`     | enum filter plus `strength: f32` and optional setting.              | Display accessibility state and resource API config.    |
| `ConstParamValue`                                                          | enum: `F32`, `I32`, `Bool`, `Vec2`, `Vec3`, `Vec4`.                 | Shader/material/post-process constant values.           |
| `IKTargetParams`, `IKTargetSolver`                                         | IK target fields plus solver enum.                                  | Built-in skeletal IK node config.                       |
| `Unorm8`, `Unorm8x4`                                                       | compact normalized `u8` or `[u8; 4]`.                               | Packed normalized color/weight data.                    |

## Color

`Color` stores four `Unorm8` channels internally, not four `f32` fields.

Use `Color` when an API or resource needs RGBA color as typed data. The float constructors take channel values in the `0.0..=1.0` range, clamp out-of-range values, and round to bytes for storage.

Signature:

```rust
pub struct Color {
    pub r: Unorm8,
    pub g: Unorm8,
    pub b: Unorm8,
    pub a: Unorm8,
}
```

Storage:

| Public input/output                                     | Internal storage                           | Edge behavior                                                       |
| ------------------------------------------------------- | ------------------------------------------ | ------------------------------------------------------------------- |
| `f32` channels use `0.0..=1.0`.                         | Each channel stores `u8` through `Unorm8`. | Values below `0.0` clamp to `0`; values above `1.0` clamp to `255`. |
| Hex strings use `RGB`, `RGBA`, `RRGGBB`, or `RRGGBBAA`. | Hex parse stores exact bytes.              | Invalid length or digit returns `None`.                             |
| Float slice output uses `[f32; 4]`.                     | Bytes convert back to normalized floats.   | Round trip through bytes can quantize.                              |

Common APIs:

| Access      | Signature                                                  | Params                                                              | Returns         | Use when                                                             | Why / edge behavior                                |
| ----------- | ---------------------------------------------------------- | ------------------------------------------------------------------- | --------------- | -------------------------------------------------------------------- | -------------------------------------------------- |
| Constructor | `pub const fn new(r: f32, g: f32, b: f32, a: f32) -> Self` | `r/g/b/a`: normalized `f32` channels.                               | `Color`         | Build explicit RGBA.                                                 | Clamps to `0.0..=1.0`, rounds, stores as `Unorm8`. |
| Constructor | `pub const fn rgb(r: f32, g: f32, b: f32) -> Self`         | `r/g/b`: normalized `f32` channels.                                 | `Color`         | Build opaque color.                                                  | Sets alpha to `1.0`.                               |
| Constructor | `pub const fn from_rgba(v: [f32; 4]) -> Self`              | `[r, g, b, a]` normalized floats.                                   | `Color`         | Convert from array data.                                             | Same clamp/round/storage as `new`.                 |
| Constructor | `pub const fn from_float_slice(v: [f32; 4]) -> Self`       | Normalized RGBA floats.                                             | `Color`         | Convert from float slice/array data.                                 | Alias of `from_rgba`.                              |
| Constructor | `pub const fn from_rgba_u8(v: [u8; 4]) -> Self`            | Exact byte channels.                                                | `Color`         | Preserve imported byte color.                                        | Stores exact channel bytes.                        |
| Constructor | `pub const fn from_unorm8x4(v: Unorm8x4) -> Self`          | Packed normalized bytes.                                            | `Color`         | Convert from compact normalized color.                               | Uses exact stored bytes.                           |
| Constructor | `pub const fn from_unorm_slice(v: Unorm8x4) -> Self`       | Packed normalized bytes.                                            | `Color`         | Convert from APIs that name packed normalized data as a slice value. | Alias of `from_unorm8x4`.                          |
| Parser      | `pub fn from_hex(hex: &str) -> Option<Self>`               | `"#RGB"`, `"#RGBA"`, `"#RRGGBB"`, `"#RRGGBBAA"`, with optional `#`. | `Option<Color>` | Parse author-facing color text.                                      | Returns `None` for bad length or bad hex digit.    |
| Accessor    | `pub const fn r(self) -> f32` and `g/b/a`                  | none                                                                | `f32`           | Read one normalized channel.                                         | Converts stored byte to `0.0..=1.0`.               |
| Accessor    | `pub const fn to_rgba(self) -> [f32; 4]`                   | none                                                                | `[f32; 4]`      | Feed APIs that expect float RGBA arrays.                             | Converts stored bytes to normalized floats.        |
| Accessor    | `pub const fn to_rgb(self) -> [f32; 3]`                    | none                                                                | `[f32; 3]`      | Feed RGB-only APIs.                                                  | Drops alpha.                                       |
| Accessor    | `pub const fn to_float_slice(self) -> [f32; 4]`            | none                                                                | `[f32; 4]`      | Feed float-slice/array APIs.                                         | Alias of `to_rgba`.                                |
| Accessor    | `pub const fn to_rgba_u8(self) -> [u8; 4]`                 | none                                                                | `[u8; 4]`       | Save or compare exact stored bytes.                                  | No float conversion loss.                          |
| Accessor    | `pub const fn to_unorm8x4(self) -> Unorm8x4`               | none                                                                | `Unorm8x4`      | Pass compact normalized bytes.                                       | Uses exact stored bytes.                           |
| Accessor    | `pub const fn to_unorm_slice(self) -> Unorm8x4`            | none                                                                | `Unorm8x4`      | Feed APIs that name packed normalized data as a slice value.         | Alias of `to_unorm8x4`.                            |
| Formatter   | `pub fn to_hex_rgb(self) -> String`                        | none                                                                | `String`        | Save/debug opaque color text.                                        | Alpha omitted.                                     |
| Formatter   | `pub fn to_hex_rgba(self) -> String`                       | none                                                                | `String`        | Save/debug full color text.                                          | Alpha included.                                    |

Constants:

`WHITE`, `BLACK`, `GRAY`, `GREY`, `LIGHT_GRAY`, `LIGHT_GREY`, `DARK_GRAY`, `DARK_GREY`, `RED`, `MAROON`, `CRIMSON`, `GREEN`, `LIME`, `FOREST_GREEN`, `OLIVE`, `MINT`, `BLUE`, `NAVY`, `ROYAL_BLUE`, `SKY_BLUE`, `CORNFLOWER_BLUE`, `ORANGE`, `YELLOW`, `INDIGO`, `VIOLET`, `CYAN`, `TEAL`, `TURQUOISE`, `MAGENTA`, `PINK`, `PURPLE`, `BROWN`, `GOLD`, `TRANSPARENT`.

Example:

```rust
let exact = Color::from_rgba_u8([0x33, 0x66, 0x99, 0xCC]);
let from_packed = Color::from_unorm8x4(Unorm8x4::from_u8([0x33, 0x66, 0x99, 0xCC]));
let accent = Color::new(0.2, 0.4, 0.6, 0.8);
let clamped = Color::new(1.5, 0.5, -1.0, 2.0);

assert_eq!(exact.to_hex_rgba(), "#336699CC");
assert_eq!(from_packed.to_rgba_u8(), [0x33, 0x66, 0x99, 0xCC]);
assert_eq!(clamped.to_rgba_u8(), [255, 128, 0, 255]);

let rgba: [f32; 4] = accent.to_float_slice();
let packed: Unorm8x4 = accent.to_unorm_slice();
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

## `Unorm8` And `Unorm8x4`

Use `Unorm8` and `Unorm8x4` when normalized floats need compact byte storage.

| Access      | Signature                                            | Params                  | Returns    | Use when                                     | Why / edge behavior                      |
| ----------- | ---------------------------------------------------- | ----------------------- | ---------- | -------------------------------------------- | ---------------------------------------- |
| Constructor | `pub const fn Unorm8::new(v: f32) -> Self`           | Normalized `f32`.       | `Unorm8`   | Pack one normalized value.                   | Clamps to `0.0..=1.0`, rounds to `u8`.   |
| Constructor | `pub const fn Unorm8::from_u8(v: u8) -> Self`        | Exact byte.             | `Unorm8`   | Keep imported byte value exact.              | No clamp needed.                         |
| Accessor    | `pub const fn to_u8(self) -> u8`                     | none                    | `u8`       | Save exact byte.                             | No conversion loss.                      |
| Accessor    | `pub const fn to_f32(self) -> f32`                   | none                    | `f32`      | Feed normalized float APIs.                  | Returns `byte / 255.0`.                  |
| Constructor | `pub const fn Unorm8x4::new(v: [f32; 4]) -> Self`    | Four normalized floats. | `Unorm8x4` | Pack RGBA-like data.                         | Clamps and rounds each channel.          |
| Constructor | `pub const fn Unorm8x4::from_u8(v: [u8; 4]) -> Self` | Exact bytes.            | `Unorm8x4` | Keep byte data exact.                        | No clamp needed.                         |
| Accessor    | `pub const fn to_u8(self) -> [u8; 4]`                | none                    | `[u8; 4]`  | Save exact packed bytes.                     | No conversion loss.                      |
| Accessor    | `pub const fn to_f32(self) -> [f32; 4]`              | none                    | `[f32; 4]` | Feed normalized float APIs.                  | Converts each byte to `byte / 255.0`.    |
| Accessor    | `pub const fn to_le_u32(self) -> u32`                | none                    | `u32`      | Pack four bytes into one little-endian word. | Byte order follows `u32::from_le_bytes`. |

Example:

```rust
let packed = Unorm8x4::new([1.0, 0.5, -1.0, 2.0]);

assert_eq!(packed.to_u8(), [255, 128, 0, 255]);
assert_eq!(packed.to_le_u32(), 0xFF00_80FF);
```

## Audio Structs

Audio structs document propagation, occlusion, material response, and listener data used by built-in audio systems.

These values usually sit inside audio nodes, audio resources, or listener options. They may be script-facing in some paths and mostly internal in others.

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
