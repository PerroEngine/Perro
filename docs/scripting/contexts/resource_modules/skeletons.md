# Skeletons Module

## Page Map

| Header | Link |
| --- | --- |
| Overview | [Overview](#overview) |
| Context | [Context](#context) |
| Runtime Bytes | [Runtime Bytes](#runtime-bytes) |
| API Reference | [API Reference](#api-reference) |
| `load_bones_2d` | [`load_bones_2d`](#load_bones_2d) |
| `load_bones_3d` | [`load_bones_3d`](#load_bones_3d) |
| `load_bones` | [`load_bones`](#load_bones) |
| `skeleton_load_bones` | [`skeleton_load_bones`](#skeleton_load_bones) |

## Overview

This resource module belongs to `ctx.res` and documents skeletons calls.

## Context

- Script context path: `ctx.res`
- Module access: `ctx.res.Skeletons()`
- Lifecycle examples stay inside `lifecycle!` because script hooks get `API` from the macro expansion.

## Runtime Bytes

Use runtime bytes when skeleton data is already in memory.

| Call | Return | Notes |
| --- | --- | --- |
| `ctx.res.Skeletons().load_bones_2d_from_bytes(bytes)` | `Vec<Bone2D>` | Decodes packed 2D skeleton bytes. |
| `ctx.res.Skeletons().load_bones_3d_from_bytes(bytes)` | `Vec<Bone3D>` | Decodes packed 3D skeleton bytes. |
| `skeleton_load_bones_2d_from_bytes!(ctx.res, bytes)` | `Vec<Bone2D>` | Macro form. |
| `skeleton_load_bones_3d_from_bytes!(ctx.res, bytes)` | `Vec<Bone3D>` | Macro form. |

See [Runtime Bytes Resources](../../../resources/runtime_bytes.md).

## API Reference

### `load_bones_2d`

| Field | Detail |
| --- | --- |
| Access | `ctx.res.Skeletons()` |
| Signature | `pub fn load_bones_2d<S: ResPathSource>(&self, source: S) -> Vec<Bone2D>` |
| Params | `&self, source: S` |
| Returns | `Vec<Bone2D>` |
| Use when | Use when code needs an ID or prepared asset before gameplay uses it. |
| Fails when / edge behavior | Returns the documented empty value when backing runtime data is missing, stale, or the target type does not match. |

### `load_bones_3d`

| Field | Detail |
| --- | --- |
| Access | `ctx.res.Skeletons()` |
| Signature | `pub fn load_bones_3d<S: ResPathSource>(&self, source: S) -> Vec<Bone3D>` |
| Params | `&self, source: S` |
| Returns | `Vec<Bone3D>` |
| Use when | Use when code needs an ID or prepared asset before gameplay uses it. |
| Fails when / edge behavior | Returns the documented empty value when backing runtime data is missing, stale, or the target type does not match. |

### `load_bones`

| Field | Detail |
| --- | --- |
| Access | `ctx.res.Skeletons()` |
| Signature | `pub fn load_bones<S: ResPathSource>(&self, source: S) -> Vec<Bone3D>` |
| Params | `&self, source: S` |
| Returns | `Vec<Bone3D>` |
| Use when | Use when code needs an ID or prepared asset before gameplay uses it. |
| Fails when / edge behavior | Returns the documented empty value when backing runtime data is missing, stale, or the target type does not match. |

### `skeleton_load_bones`

| Field | Detail |
| --- | --- |
| Access | `ctx.res.Skeletons()` |
| Signature | `skeleton_load_bones!(ctx.res.res, source)` |
| Params | `ctx.res, source` |
| Returns | `resource/runtime ID or `Result` as shown by backing method` |
| Use when | Use when code needs an ID or prepared asset before gameplay uses it. |
| Fails when / edge behavior | Returns the documented empty value when backing runtime data is missing, stale, or the target type does not match. |

