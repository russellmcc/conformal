---
conformal_vst_wrapper: minor
---

# Fix for VST3 format violation

Fixes a subtle issue where we did not allow activating a processor unless the audio busses had been activated. It turns out compliant plug-ins must support being activated with no audio present.
