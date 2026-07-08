# Audio Nodes

Audio geometry nodes and collision shapes are separate scene nodes.
`AudioMask2D`, `AudioEffectZone2D`, `AudioPortal2D`, `AudioMask3D`, `AudioEffectZone3D`, and `AudioPortal3D` hold audio behavior.
`CollisionShape2D` and `CollisionShape3D` hold the volume/portal/mask shape.

In scene files, put collision shapes in separate top-level node blocks.
Set each shape `parent` to the audio node key.

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
