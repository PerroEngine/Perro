# Skeletons Module

## Page Map

| Header | Link |
| --- | --- |
| Overview | [Overview](#overview) |
| Context | [Context](#context) |
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

