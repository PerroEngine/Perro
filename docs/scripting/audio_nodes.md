# Audio Nodes

## Page Map

| Header | Link |
| --- | --- |
| Purpose | [Purpose](#purpose) |
| Use Cases | [Use Cases](#use-cases) |
| 2D Effect Zone | [2D Effect Zone](#2d-effect-zone) |
| 3D Audio Mask | [3D Audio Mask](#3d-audio-mask) |
| Notes | [Notes](#notes) |

## Purpose

Audio geometry nodes shape how spatial sound travels through a scene. `AudioMask` blocks or muffles sound, `AudioEffectZone` applies reverb, echo, and dampening inside a volume, and `AudioPortal` lets sound leak between otherwise blocked spaces. Like physics bodies, each audio node pairs with a child `CollisionShape` that defines its volume, in both 2D and 3D.

## Use Cases

- Muffle sound behind a wall or floor so covered players hear less: `AudioMask2D` / `AudioMask3D` with a child shape; `audio_mask` selects which emitted `audio_layer` sources it affects.
- Add room reverb, echo, or dampening inside a cave, hall, or tunnel: `AudioEffectZone2D` / `AudioEffectZone3D` with `effects = [{ reverb_send, echo, dampening }]`.
- Let sound pass through a doorway or window in an otherwise blocked wall: `AudioPortal2D` / `AudioPortal3D`.
- Restrict which emitters a zone or mask touches by layer: emitted `audio_layer` versus listener/geometry `audio_mask` (see [BitMask](bitmask.md)).

Audio geometry nodes and collision shapes are separate scene nodes.
`AudioMask2D`, `AudioEffectZone2D`, `AudioPortal2D`, `AudioMask3D`, `AudioEffectZone3D`, and `AudioPortal3D` hold audio behavior.
`CollisionShape2D` and `CollisionShape3D` hold the volume/portal/mask shape.

In scene files, put collision shapes in separate top-level node blocks.
Set each shape `parent` to the audio node key.

## Ownership And Choice

Audio nodes own world position, range, and spatial filtering. The resource audio API owns one-shot playback and bus control. Use a node when sound must follow scene geometry; use the API when a sound has no persistent world object. Gameplay emits the event, while the audio layer chooses clip, bus, and spatial treatment.

## 2D Effect Zone

```text
[Zone]
parent = $root
    [AudioEffectZone2D]
        active = true
        audio_mask = []
        effects = [{ reverb_send = 0.35 echo = 0.0 dampening = 0.0 }]
        [Node2D/]
    [/AudioEffectZone2D]
[/Zone]

[ZoneShape]
parent = @Zone
    [CollisionShape2D]
        shape = { type = quad width = 4.0 height = 4.0 }
    [/CollisionShape2D]
[/ZoneShape]
```

## 3D Audio Mask

```text
[AudioWall]
parent = $root
    [AudioMask3D]
        active = true
    [/AudioMask3D]
[/AudioWall]

[AudioWallShape]
parent = @AudioWall
    [CollisionShape3D]
        shape = { type = cube, size = (1, 2, 0.2) }
    [/CollisionShape3D]
[/AudioWallShape]
```

## Notes

- Audio masks, effect zones, and portals need child collision shapes.
- Audio nodes can have more than one child collision shape.
- Shape local transform comes from the shape node's `Node2D` or `Node3D` data.
- `audio_mask` ignores matching emitted `audio_layer`.
