## 0.6.0 (2026-02-26)

### Breaking Changes

- Use unreleased wry to fix some bugs

## 0.5.0 (2026-02-22)

### Breaking Changes

- API Change: UI Resource Root is now an argument to UI creation
- MSRV is now 1.93.1
- use builder api for class info

### Features

- Allow basic resizable plug-ins
- Builds on windows
- Don't require vst3 dependency in all users of the wrap factory macro
- Remove conformal_macos_bundle, fold functionality to conformal_core
- support getting preference path from DLL metadata on windows
- support getting ui resource path on windows vst3 bundles

### Fixes

- Supports VST3 scale factor api

## 0.4.1 (2026-02-09)

### Features

- Make hash_id `const`, optimize pzip to not hash at runtime

#### Instead of guarding against host invariant violations, check them in debug builds only with `debug_assert`.

This decreases safety a bit but improves perf.

## 0.4.0 (2026-02-08)

### Breaking Changes

- Add dependency to zlib licensed code
- Change API for NoteID for format adapters (should not affect plug-in writers)
- New API for mpe
- New more extensible context-based api for process
- We now always piecewise-linear interpolate MPE expression events

### Fixes

- Optimizations to mpe handling

## 0.3.10 (2026-01-26)

### Fixes

- Fix bug where height was always 400

## 0.3.9 (2025-12-27)

### Fixes

- upgrade to vst3-rs 0.3.0

## 0.3.8 (2025-11-02)

### Features

- bumping vst3-rs

## 0.3.7 (2025-09-23)

### Features

- Minimum support rust version (MSRV) is now 1.90

#### Fix for VST3 format violation

Fixes a subtle issue where we did not allow activating a processor unless the audio busses had been activated. It turns out compliant plug-ins must support being activated with no audio present.

## 0.3.6 (2025-02-23)

### Features

- Add support for persistant UI state
