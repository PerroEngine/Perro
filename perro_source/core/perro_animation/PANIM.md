# `.panim` keyframe mode extension

## Frame header modes

- `[FrameN]` = closed frame (default)
- `[FrameN?]` = open frame

## Meaning

### Closed frame
A closed frame authors concrete property values.
Sampling is deterministic from animation data alone.

### Open frame
An open frame is a timing/interpolation directive.
It marks that interpolation should start from runtime/current value rather than snapping to an authored start sample.

Think of open frames as **"begin transition from wherever the object currently is"**.

## Example

```text
[Frame0?]
@Hand {
    rotation = 0
}

[Frame20]
@Hand {
    rotation = 90
}
```

If runtime hand rotation at frame 0 is 13 degrees, interpolation is `13 -> 90` over frames `0..20`.
The frame-0 authored value is not an authoritative sampled pose when the key is open.

## Notes

- Open keys are not directly sampleable as deterministic pose values.
- Open keys may still carry interpolation/easing metadata.
- Deterministic optimization should skip tracks containing open keys.
