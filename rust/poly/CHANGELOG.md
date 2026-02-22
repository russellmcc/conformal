## 0.6.0 (2026-02-22)

### Breaking Changes

- MSRV is now 1.93.1

## 0.5.0 (2026-02-09)

### Breaking Changes

#### Made poly generic on the max voice count. Synths with a lower max voice count should lower this to improve overhead

`Poly::new` no longer takes a `max_voices` argument. The voice count is now determined by the `MAX_VOICES` const generic parameter on `Poly`. Update call sites from `Poly::new(env, N)` to `Poly::<V, N>::new(env)` (or `Poly<V, N>` in the type position).

### Features

- Small optimizations

## 0.4.0 (2026-02-08)

### Breaking Changes

- New API for mpe
- Support new global expression API

## 0.3.7 (2025-09-23)

### Features

- Minimum support rust version (MSRV) is now 1.90

## 0.3.6 (2025-02-17)

### Fixes

- please ignore this release, it's a test of the new publishing system.
