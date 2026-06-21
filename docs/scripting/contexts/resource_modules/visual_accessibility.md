# Visual Accessibility Module

## Page Map

| Header | Link |
| --- | --- |
| Overview | [Overview](#overview) |
| Context | [Context](#context) |
| API Reference | [API Reference](#api-reference) |
| `enable_colorblind_filter` | [`enable_colorblind_filter`](#enable_colorblind_filter) |
| `disable_colorblind_filter` | [`disable_colorblind_filter`](#disable_colorblind_filter) |

## Overview

This resource module belongs to `ctx.res` and documents visual accessibility calls.

## Context

- Script context path: `ctx.res`
- Module access: `ctx.res`
- Lifecycle examples stay inside `lifecycle!` because script hooks get `API` from the macro expansion.

## API Reference

### `enable_colorblind_filter`

| Field | Detail |
| --- | --- |
| Access | `ctx.res` |
| Signature | `enable_colorblind_filter!(ctx.res.res, mode, strength)` |
| Params | `ctx.res, mode, strength` |
| Returns | `same as backing method` |
| Use when | Use when script code needs this exact engine read or write. |
| Fails when / edge behavior | Returns the documented empty value when backing runtime data is missing, stale, or the target type does not match. |

### `disable_colorblind_filter`

| Field | Detail |
| --- | --- |
| Access | `ctx.res` |
| Signature | `disable_colorblind_filter!(ctx.res.res)` |
| Params | `ctx.res` |
| Returns | `same as backing method` |
| Use when | Use when script code needs this exact engine read or write. |
| Fails when / edge behavior | Returns the documented empty value when backing runtime data is missing, stale, or the target type does not match. |

