# Materials Module

Access:

- `res.Materials()`

Macros:

- `material_load!(res, source) -> MaterialID`
- `material_reserve!(res, source) -> MaterialID`
- `material_drop!(res, source) -> bool`
- `material_create!(res, material) -> MaterialID`
- `material_get_data!(res, material_id) -> Option<Material3D>`
- `material_write!(res, material_id, material) -> bool`

When to use each:

- `material_load!` / `material_reserve!`: default path for preauthored material sources (`.pmat`, glTF `:mat[index]`).
- `material_create!`: create a material directly from runtime data.
- `material_get_data!` + `material_write!`: runtime mutation path for an existing material id.
- Typical `get/write` use-cases: dynamic param edits (roughness/metallic/emissive/tint) and runtime material variants.

Methods:

- `res.Materials().load(source) -> MaterialID`
- `res.Materials().reserve(source) -> MaterialID`
- `res.Materials().drop(source) -> bool`
- `res.Materials().create(material) -> MaterialID`
- `res.Materials().get_data(material_id) -> Option<Material3D>`
- `res.Materials().write(material_id, material) -> bool`

What `load` does:

- Loads material data from `source` and returns a stable `MaterialID`.
- If source is already cached, returns existing ID.
- If not cached, allocates an ID and queues renderer material creation with `reserved: false`.
- Creation is async relative to script call.

What `reserve` does:

- Same as `load`, but marks/creates as reserved (`reserved: true`).
- If already created, reserve flag is set immediately.
- If pending, reserve intent is deferred and applied after creation.

What `drop` does:

- Removes source mapping and queues renderer drop when material exists.
- If creation is pending, marks drop-pending so it is dropped right after creation resolves.
- Returns `true` when matching pending/loaded source exists.
- Returns `false` when source is not known.

What `create_material` does:

- Creates a runtime material directly from `Material3D` data.
- Does not create a source-path mapping.
- Intended for transient/generated materials.

Important behavior:

- `load/reserve/drop` are source-cache operations.
- `create_material` is data-driven and bypasses source cache lookup.
- Data APIs are copy-based; `material_get_data!` returns a cloned material value.
- `material_write!` replaces material data for that id in one write operation.
- Most projects mainly load authored materials and assign ids in scenes; `material_get_data!/material_write!` are for deliberate runtime updates.
- Reserved policy:
- `reserved: false` (from `load`) means the material can be automatically evicted from cache when no references remain.
- `reserved: true` (from `reserve`) means it will not be auto-evicted; only explicit `material_drop!` removes it.

Example:

```rust
let src_id = material_load!(res, "res://models/rig.glb:mat[0]");
let _same_id = material_reserve!(res, "res://models/rig.glb:mat[0]");
let _ = material_drop!(res, "res://models/rig.glb:mat[0]");

if let Some(mut mat) = material_get_data!(res, src_id) {
    if let Material3D::Standard(params) = &mut mat {
        params.roughness_factor = 0.2;
    }
    let _ = material_write!(res, src_id, mat);
}
```

## Material3D Presets

`Material3D` is a preset enum:

- `Material3D::Standard(StandardMaterial3D)`
- `Material3D::Unlit(UnlitMaterial3D)`
- `Material3D::Toon(ToonMaterial3D)`
- `Material3D::Custom(CustomMaterial3D)`

Each preset has its own params struct. Custom materials carry a shader path and a list of typed params.

See also: `docs/resources/shaders.md` for WGSL authoring notes and current limitations.

## Programmatic Examples

```rust
// Material types come from Perro prelude.

// Standard (PBR-ish)
let standard_id = material_create!(
    res,
    Material3D::Standard(StandardMaterial3D {
        base_color_factor: [0.8, 0.2, 0.2, 1.0],
        roughness_factor: 0.4,
        metallic_factor: 0.1,
        ..StandardMaterial3D::default()
    })
);

// Unlit
let unlit_id = material_create!(
    res,
    Material3D::Unlit(UnlitMaterial3D {
        base_color_factor: [0.2, 0.8, 0.9, 1.0],
        ..UnlitMaterial3D::default()
    })
);

// Toon
let toon_id = material_create!(
    res,
    Material3D::Toon(ToonMaterial3D {
        base_color_factor: [0.9, 0.9, 0.2, 1.0],
        band_count: 3,
        rim_strength: 0.4,
        outline_width: 1.5,
        ..ToonMaterial3D::default()
    })
);

// Custom
let custom_id = material_create!(
    res,
    Material3D::Custom(CustomMaterial3D::with_params(
        "res://shaders/custom.wgsl",
        vec![
            CustomMaterialParam3D::named("glow", CustomMaterialParamValue3D::F32(1.25)),
            CustomMaterialParam3D::named(
                "tint",
                CustomMaterialParamValue3D::Vec4([1.0, 0.2, 0.4, 1.0]),
            ),
        ],
    ))
);
```

glTF sub-asset access:

- `res://path/to/model.gltf:mat[0]`
- `res://path/to/model.glb:mat[1]`

Use the `:mat[index]` suffix to target a specific material inside a glTF/glb.

Direct `.pmat` sources:

- `res://path/to/material.pmat`
