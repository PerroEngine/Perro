# Global Post Processing

Access:

- Direct `ResourceContext` methods/macros (no module accessor).

Macros:

- `post_processing_set!(res, PostProcessSet)`
- `post_processing_add!(res, effect)`
- `post_processing_add!(res, "name", effect)`
- `post_processing_remove!(res, name = "name")`
- `post_processing_remove!(res, index = 0usize)`
- `post_processing_clear!(res)`

Methods:

- `res.set_global_post_processing(set)`
- `res.add_global_post_processing(effect)`
- `res.add_global_post_processing_named(name, effect)`
- `res.remove_global_post_processing_by_name(name)`
- `res.remove_global_post_processing_by_index(index)`
- `res.clear_global_post_processing()`

Behavior:

- Global post-processing is applied to the full composed frame.
- Order is: camera post-processing chain, then global post-processing chain, then visual accessibility.
- Effects use the same `PostProcessEffect` / `PostProcessSet` types as camera post-processing.

Example:

```rust
post_processing_add!(
    res,
    "crt",
    PostProcessEffect::Crt {
        scanline_strength: 0.35,
        curvature: 0.15,
        chromatic: 1.0,
        vignette: 0.25,
    }
);

post_processing_add!(res, PostProcessEffect::Bloom {
    strength: 0.7,
    threshold: 0.75,
    radius: 1.5,
});

let _ = post_processing_remove!(res, name = "crt");
let _ = post_processing_remove!(res, index = 0usize);
post_processing_clear!(res);
```
