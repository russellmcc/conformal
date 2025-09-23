## 0.3.7 (2025-09-23)

### Features

- Minimum support rust version (MSRV) is now 1.90

#### Fix for VST3 format violation

Fixes a subtle issue where we did not allow activating a processor unless the audio busses had been activated. It turns out compliant plug-ins must support being activated with no audio present.

## 0.3.6 (2025-02-23)

### Features

- Add support for persistant UI state
