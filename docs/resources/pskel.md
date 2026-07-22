# `.pskel2d` / `.pskel3d` Formats

## Page Map

| Header | Link |
| --- | --- |
| Purpose | [Purpose](#purpose) |
| Use Cases | [Use Cases](#use-cases) |
| Example | [Example](#example) |
| Reference | [Reference](#reference) |

## Purpose

`.pskel2d` and `.pskel3d` store bone rest data as text so you can hand-author or tool-generate a rig without a full glTF export. Each bone has a parent index and a local rest transform; the file loads straight into `Skeleton2D.bones` or `Skeleton3D.bones`. Use these when you want a lightweight, diff-friendly rig for cutout characters, limbs, ropes, or props that `.panim` bone tracks and IK targets then pose.

## Use Cases

- 2D cutout character: a `Hip -> Spine -> UpperArm -> LowerArm -> Hand` chain with `parent` indices and `rest_pos`, loaded into `Skeleton2D` for paper-doll animation.
- Foot/hand IK: point an `IKTarget2D` at the skeleton with `bone = 4` and `chain_length = 3` so the last three bones reach a target.
- Simple 3D limb or tail rig: a `Hip -> Spine -> Head` chain in `.pskel3d` with quaternion `rest_rot` (or `rest_rot_deg` Euler) driven by `.panim` bone tracks.
- Tool-exported rigs: generate `.pskel*` text from an editor, then bake to binary `PSKEL` v1 in static builds by rerunning the static compiler.
- Runtime-generated bones: build a `Vec<Bone2D>`/`Vec<Bone3D>` from bytes with `skeleton_load_bones_2d_from_bytes!` / `skeleton_load_bones_3d_from_bytes!` for procedural or modded rigs.

## Choice Guide

Use `.pskel*` for authored rigs that benefit from static baking and readable
asset refs. Use runtime bone creation for generated or downloaded rigs. Keep
bone hierarchy/rest pose in the skeleton; keep playback in animation resources
and physics response in physics-bone nodes.

## Example

Author `res://rigs/arm.pskel2d` (bones in file order, `-1` = root):

```text
[bone "Hip"]
    parent = -1
    rest_pos = (0, 0)
[/bone]

[bone "Spine"]
    parent = 0
    rest_pos = (0, 24)
[/bone]

[bone "UpperArm"]
    parent = 1
    rest_pos = (18, 8)
[/bone]

[bone "LowerArm"]
    parent = 2
    rest_pos = (24, 0)
[/bone]

[bone "Hand"]
    parent = 3
    rest_pos = (18, 0)
[/bone]
```

Load it into a `Skeleton2D` and attach a 3-bone IK chain to the hand:

```text
[Rig2D]
    [Skeleton2D]
        skeleton = "res://rigs/arm.pskel2d"
    [/Skeleton2D]
[/Rig2D]

[HandTarget]
parent = @Rig2D
    [IKTarget2D]
        skeleton = @Rig2D
        bone = 4
        chain_length = 3
    [/IKTarget2D]
[/HandTarget]
```

## Reference

# `.pskel2d` / `.pskel3d` Formats

Perro skeleton resources store bone data for scene skeleton nodes.

- `.pskel2d` loads into `Skeleton2D.bones`.
- `.pskel3d` loads into `Skeleton3D.bones`.
- glTF skeleton import still uses `res://model.gltf:skeleton[0]`.

## Scene Usage

```text
[Rig2D]
    [Skeleton2D]
        skeleton = "res://rigs/hero.pskel2d"
    [/Skeleton2D]
[/Rig2D]

[Rig3D]
    [Skeleton3D]
        skeleton = "res://models/hero.pskel3d"
    [/Skeleton3D]
[/Rig3D]
```

## `.pskel2d` Text

```text
[bone "Root"]
    parent = -1
    rest_pos = (0, 0)
    rest_scale = (1, 1)
    rest_rot = 0.0        // radians
    rest_rot_deg = 0.0    // degrees alternative
    inv_pos = (0, 0)
    inv_scale = (1, 1)
    inv_rot = 0.0         // radians
    inv_rot_deg = 0.0     // degrees alternative
[/bone]
```

## 2D Chain Example

Bones are stored in file order.
`parent` points to another bone index in that order.
Use `-1` for root.

```text
[bone "Hip"]
    parent = -1
    rest_pos = (0, 0)
[/bone]

[bone "Spine"]
    parent = 0
    rest_pos = (0, 24)
[/bone]

[bone "UpperArm"]
    parent = 1
    rest_pos = (18, 8)
[/bone]

[bone "LowerArm"]
    parent = 2
    rest_pos = (24, 0)
[/bone]

[bone "Hand"]
    parent = 3
    rest_pos = (18, 0)
[/bone]
```

Chain:

- `Hip` index `0`, root
- `Spine` index `1`, child of `Hip`
- `UpperArm` index `2`, child of `Spine`
- `LowerArm` index `3`, child of `UpperArm`
- `Hand` index `4`, child of `LowerArm`

`rest_pos` is local to parent bone.
So `LowerArm.rest_pos = (24, 0)` means 24 units from `UpperArm`, not from skeleton root.

Use in scene:

```text
[Rig2D]
    [Skeleton2D]
        skeleton = "res://rigs/arm.pskel2d"
    [/Skeleton2D]
[/Rig2D]

[HandTarget]
parent = @Rig2D
    [IKTarget2D]
        skeleton = @Rig2D
        bone = 4
        chain_length = 3
    [/IKTarget2D]
[/HandTarget]
```

`chain_length = 3` on `Hand` uses:

- `UpperArm`
- `LowerArm`
- `Hand`

## `.pskel3d` Text

```text
[bone "Root"]
    parent = -1
    rest_pos = (0, 0, 0)
    rest_scale = (1, 1, 1)
    rest_rot = (0, 0, 0, 1)   // quaternion
    rest_rot_deg = (0, 0, 0)  // Euler XYZ degrees alternative
    inv_pos = (0, 0, 0)
    inv_scale = (1, 1, 1)
    inv_rot = (0, 0, 0, 1)    // quaternion
    inv_rot_deg = (0, 0, 0)   // Euler XYZ degrees alternative
[/bone]
```

## 3D Chain Example

Same parent-index rule as 2D.
Only transform fields differ.

```text
[bone "Hip"]
    parent = -1
    rest_pos = (0, 0, 0)
[/bone]

[bone "Spine"]
    parent = 0
    rest_pos = (0, 1, 0)
[/bone]

[bone "Head"]
    parent = 1
    rest_pos = (0, 1, 0)
[/bone]
```

## Notes

- `parent` is the parent bone index.
- `-1` means root bone.
- `rest_*` is local rest transform.
- `inv_*` is inverse bind transform.
- Missing fields default to identity.
- Static builds compile text into binary `PSKEL` v1.
- Old binary version numbers are unsupported before public asset compatibility starts.
- Rerun the static compiler to regenerate packed bytes.
- Put parent bones before children.
- Bad parent index is ignored by most runtime paths, but rig tools should treat it as invalid.
