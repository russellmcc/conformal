---
conformal_poly: major
---

# Made poly generic on the max voice count. Synths with a lower max voice count should lower this to improve overhead

`Poly::new` no longer takes a `max_voices` argument. The voice count is now determined by the `MAX_VOICES` const generic parameter on `Poly`. Update call sites from `Poly::new(env, N)` to `Poly::<V, N>::new(env)` (or `Poly<V, N>` in the type position).
