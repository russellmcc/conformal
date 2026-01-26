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
