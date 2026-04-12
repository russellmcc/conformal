## 0.6.4 (2026-04-12)

### Fixes

- Workaround rounding errors when re-scaling windows hosts

## 0.6.3 (2026-04-06)

### Fixes

- Allow re-entrancy in performEdit -> setParamNormalized to prevent tracktion crashes
- Fix issue where ui would be blank on some windows DAWs

## 0.6.2 (2026-03-10)

### Fixes

- Bump wry

## 0.6.1 (2026-02-27)

### Fixes

- Oops can't publish crate patches

## 0.6.0 (2026-02-26)

### Breaking Changes

- Use unreleased wry to fix some bugs

## 0.5.0 (2026-02-22)

### Breaking Changes

- API Change: UI Resource Root is now an argument to UI creation
- MSRV is now 1.93.1

### Features

- Remove conformal_macos_bundle, fold functionality to conformal_core

## 0.4.0 (2026-02-08)

### Breaking Changes

- bump to new conformal_component api

## 0.3.7 (2025-09-23)

### Features

- Minimum support rust version (MSRV) is now 1.90

## 0.3.6 (2025-02-23)

### Features

- Add support for persistant UI state
- Workaround https://github.com/3Hren/msgpack-rust/issues/363 by changing UI server protocol
