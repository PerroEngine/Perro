# Csv Module

## Page Map

| Header | Link |
| --- | --- |
| Overview | [Overview](#overview) |
| Context | [Context](#context) |
| Runtime Bytes | [Runtime Bytes](#runtime-bytes) |
| API Reference | [API Reference](#api-reference) |
| `load` | [`load`](#load) |
| `load_hashed` | [`load_hashed`](#load_hashed) |
| `load_hashed_with_source` | [`load_hashed_with_source`](#load_hashed_with_source) |
| `save` | [`save`](#save) |
| `save_hashed` | [`save_hashed`](#save_hashed) |
| `csv_load` | [`csv_load`](#csv_load) |
| `csv_save` | [`csv_save`](#csv_save) |

## Overview

This resource module belongs to `ctx.res` and documents csv calls.

## Context

- Script context path: `ctx.res`
- Module access: `ctx.res.Csv()`
- Lifecycle examples stay inside `lifecycle!` because script hooks get `API` from the macro expansion.

## Runtime Bytes

Use runtime bytes when CSV data is already in memory.

| Call | Return | Notes |
| --- | --- | --- |
| `ctx.res.Csv().load_bytes(bytes)` | `&'static Csv` | Parses CSV bytes immediately. |
| `csv_load_bytes!(ctx.res, bytes)` | `&'static Csv` | Macro form. |

See [Runtime Bytes Resources](../../../resources/runtime_bytes.md).

## API Reference

### `load`

| Field | Detail |
| --- | --- |
| Access | `ctx.res.Csv()` |
| Signature | `pub fn load<S: ResPathSource>(&self, source: S) -> &'static Csv` |
| Params | `&self, source: S` |
| Returns | `&'static Csv` |
| Use when | Use when code needs an ID or prepared asset before gameplay uses it. |
| Fails when / edge behavior | Returns the documented empty value when backing runtime data is missing, stale, or the target type does not match. |

### `load_hashed`

| Field | Detail |
| --- | --- |
| Access | `ctx.res.Csv()` |
| Signature | `pub fn load_hashed(&self, source_hash: u64) -> &'static Csv` |
| Params | `&self, source_hash: u64` |
| Returns | `&'static Csv` |
| Use when | Use when code needs an ID or prepared asset before gameplay uses it. |
| Fails when / edge behavior | Returns the documented empty value when backing runtime data is missing, stale, or the target type does not match. |

### `load_hashed_with_source`

| Field | Detail |
| --- | --- |
| Access | `ctx.res.Csv()` |
| Signature | `pub fn load_hashed_with_source<S: ResPathSource>( &self, source_hash: u64, source: S, ) -> &'static Csv` |
| Params | `&self, source_hash: u64, source: S,` |
| Returns | `&'static Csv` |
| Use when | Use when code needs an ID or prepared asset before gameplay uses it. |
| Fails when / edge behavior | Returns the documented empty value when backing runtime data is missing, stale, or the target type does not match. |

### `save`

| Field | Detail |
| --- | --- |
| Access | `ctx.res.Csv()` |
| Signature | `pub fn save<S: ResPathSource>(&self, source: S, csv: &CsvBuf) -> Result<(), String>` |
| Params | `&self, source: S, csv: &CsvBuf) -> Result<(` |
| Returns | `Result<(), String>` |
| Use when | Use when script code needs this exact engine read or write. |
| Fails when / edge behavior | Returns the documented empty value when backing runtime data is missing, stale, or the target type does not match. |

### `save_hashed`

| Field | Detail |
| --- | --- |
| Access | `ctx.res.Csv()` |
| Signature | `pub fn save_hashed<S: ResPathSource>( &self, source_hash: u64, source: S, csv: &CsvBuf, ) -> Result<(), String>` |
| Params | `&self, source_hash: u64, source: S, csv: &CsvBuf, ) -> Result<(` |
| Returns | `Result<(), String>` |
| Use when | Use when script code needs this exact engine read or write. |
| Fails when / edge behavior | Returns the documented empty value when backing runtime data is missing, stale, or the target type does not match. |

### `csv_load`

| Field | Detail |
| --- | --- |
| Access | `ctx.res.Csv()` |
| Signature | `csv_load!(ctx.res.res, source)` |
| Params | `ctx.res, source` |
| Returns | `resource/runtime ID or `Result` as shown by backing method` |
| Use when | Use when code needs an ID or prepared asset before gameplay uses it. |
| Fails when / edge behavior | Returns the documented empty value when backing runtime data is missing, stale, or the target type does not match. |

### `csv_save`

| Field | Detail |
| --- | --- |
| Access | `ctx.res.Csv()` |
| Signature | `csv_save!(ctx.res.res, source, csv)` |
| Params | `ctx.res, source, csv` |
| Returns | `same as backing method` |
| Use when | Use when script code needs this exact engine read or write. |
| Fails when / edge behavior | Returns the documented empty value when backing runtime data is missing, stale, or the target type does not match. |

