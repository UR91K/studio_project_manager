# Vendored `vst` 0.4.0

Copied from crates.io (MIT, see LICENSE), minus `examples/`. Seula patches two things:

- `src/host.rs`, `PluginInstance::new`: read `effGetEffectName` (45) first and fall back
  to `effGetProductString` (48). SoundTheory Gullfoss answers 48 with an empty string
  and success; only 45 carries the name. Marked `SEULA PATCH`.
- `src/host.rs`, `Dispatch::dispatch`: read the dispatcher field as an `Option` so the
  "not loaded correctly" null guard can fire. A null check on a plain `extern "C" fn` is
  optimised away (rustc `useless_ptr_null_checks`). Marked `SEULA PATCH`.

Stays MIT; it is not part of the AGPL/MPL crates. See ADR-0079.
