# Localization Module

## Page Map

| Header | Link |
| --- | --- |
| Overview | [Overview](#overview) |
| Context | [Context](#context) |
| API Reference | [API Reference](#api-reference) |
| `set_locale` | [`set_locale`](#set_locale) |
| `locale` | [`locale`](#locale) |
| `get` | [`get`](#get) |
| `get_by_hash` | [`get_by_hash`](#get_by_hash) |
| `get_for_locale` | [`get_for_locale`](#get_for_locale) |
| `get_for_locale_by_hash` | [`get_for_locale_by_hash`](#get_for_locale_by_hash) |
| `locale_set` | [`locale_set`](#locale_set) |
| `locale_get_current` | [`locale_get_current`](#locale_get_current) |
| `locale` | [`locale`](#locale) |
| `locale_in` | [`locale_in`](#locale_in) |

## Overview

This resource module belongs to `ctx.res` and documents localization calls.

## Context

- Script context path: `ctx.res`
- Module access: `ctx.res.Localization()`
- Lifecycle examples stay inside `lifecycle!` because script hooks get `API` from the macro expansion.

## API Reference

### `set_locale`

| Field | Detail |
| --- | --- |
| Access | `ctx.res.Localization()` |
| Signature | `pub fn set_locale(&self, locale: Locale) -> bool` |
| Params | `&self, locale: Locale` |
| Returns | `bool` |
| Use when | Use when gameplay must change engine state or queue an action this frame. |
| Fails when / edge behavior | Returns the documented empty value when backing runtime data is missing, stale, or the target type does not match. |

### `locale`

| Field | Detail |
| --- | --- |
| Access | `ctx.res.Localization()` |
| Signature | `pub fn locale(&self) -> Locale` |
| Params | `&self` |
| Returns | `Locale` |
| Use when | Use when gameplay needs to read typed engine data and react without owning the storage. |
| Fails when / edge behavior | Returns the documented empty value when backing runtime data is missing, stale, or the target type does not match. |

### `get`

| Field | Detail |
| --- | --- |
| Access | `ctx.res.Localization()` |
| Signature | `pub fn get<S: AsRef<str>>(&self, key: S) -> Option<&'static str>` |
| Params | `&self, key: S` |
| Returns | `Option<&'static str>` |
| Use when | Use when script code needs this exact engine read or write. |
| Fails when / edge behavior | Returns the documented empty value when backing runtime data is missing, stale, or the target type does not match. |

### `get_by_hash`

| Field | Detail |
| --- | --- |
| Access | `ctx.res.Localization()` |
| Signature | `pub fn get_by_hash(&self, key_hash: u64) -> Option<&'static str>` |
| Params | `&self, key_hash: u64` |
| Returns | `Option<&'static str>` |
| Use when | Use when gameplay needs to read typed engine data and react without owning the storage. |
| Fails when / edge behavior | Returns the documented empty value when backing runtime data is missing, stale, or the target type does not match. |

### `get_for_locale`

| Field | Detail |
| --- | --- |
| Access | `ctx.res.Localization()` |
| Signature | `pub fn get_for_locale<S: AsRef<str>>(&self, locale: Locale, key: S) -> Option<&'static str>` |
| Params | `&self, locale: Locale, key: S` |
| Returns | `Option<&'static str>` |
| Use when | Use when gameplay needs to read typed engine data and react without owning the storage. |
| Fails when / edge behavior | Returns the documented empty value when backing runtime data is missing, stale, or the target type does not match. |

### `get_for_locale_by_hash`

| Field | Detail |
| --- | --- |
| Access | `ctx.res.Localization()` |
| Signature | `pub fn get_for_locale_by_hash(&self, locale: Locale, key_hash: u64) -> Option<&'static str>` |
| Params | `&self, locale: Locale, key_hash: u64` |
| Returns | `Option<&'static str>` |
| Use when | Use when gameplay needs to read typed engine data and react without owning the storage. |
| Fails when / edge behavior | Returns the documented empty value when backing runtime data is missing, stale, or the target type does not match. |

### `locale_set`

| Field | Detail |
| --- | --- |
| Access | `ctx.res.Localization()` |
| Signature | `locale_set!(ctx.res.res, locale)` |
| Params | `ctx.res, locale` |
| Returns | `same as backing method` |
| Use when | Use when gameplay needs to read typed engine data and react without owning the storage. |
| Fails when / edge behavior | Returns the documented empty value when backing runtime data is missing, stale, or the target type does not match. |

### `locale_get_current`

| Field | Detail |
| --- | --- |
| Access | `ctx.res.Localization()` |
| Signature | `locale_get_current!(ctx.res.res)` |
| Params | `ctx.res` |
| Returns | `same as backing method` |
| Use when | Use when gameplay needs to read typed engine data and react without owning the storage. |
| Fails when / edge behavior | Returns the documented empty value when backing runtime data is missing, stale, or the target type does not match. |

### `locale`

| Field | Detail |
| --- | --- |
| Access | `ctx.res.Localization()` |
| Signature | `locale!(ctx.res.res, key)` |
| Params | `ctx.res, key` |
| Returns | `same as backing method` |
| Use when | Use when gameplay needs to read typed engine data and react without owning the storage. |
| Fails when / edge behavior | Returns the documented empty value when backing runtime data is missing, stale, or the target type does not match. |

### `locale_in`

| Field | Detail |
| --- | --- |
| Access | `ctx.res.Localization()` |
| Signature | `locale_in!(ctx.res.res, locale, key)` |
| Params | `ctx.res, locale, key` |
| Returns | `same as backing method` |
| Use when | Use when gameplay needs to read typed engine data and react without owning the storage. |
| Fails when / edge behavior | Returns the documented empty value when backing runtime data is missing, stale, or the target type does not match. |

