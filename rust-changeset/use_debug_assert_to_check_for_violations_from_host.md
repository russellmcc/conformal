---
conformal_vst_wrapper: minor
---

# Instead of guarding against host invariant violations, check them in debug builds only with `debug_assert`.

This decreases safety a bit but improves perf.
