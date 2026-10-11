# 0079. Vendor the `vst` crate to read the effect name

- **Status:** Accepted
- **Recognized:** 2026-10-11, SoundTheory Gullfoss, Gullfoss Live and Gullfoss Master (VST2)
  came back from the scanner with an empty name; a think-aloud test showed the same on
  several other plugins
- **Decided:** 2026-10-11
- **Recorded:** 2026-10-11
- **Confidence:** decided now
- **Evidence:** `crates/vst/src/host.rs` (`PluginInstance::new`, marked `SEULA PATCH`);
  `crates/vst/SEULA.md`; `fill_empty_name` in `crates/vst-meta/src/scan.rs`; a standalone
  probe, run once and not kept, that sent opcodes 45, 47 and 48 straight to the three
  Gullfoss DLLs. ADR-0004 (the scanner worker), ADR-0006 (`vst-meta` is the only source of
  plugin metadata), ADR-0054 (licence boundary)

## Context

The scanner fills a VST2 plugin's name from the `vst` crate's `Info::name`. In `vst` 0.4.0
that comes from one host query, `effGetProductString` (opcode 48), and nothing else.

The three Gullfoss DLLs answer that query with an empty string and a success return. The probe
showed what they do answer:

| Opcode | Gullfoss |
|---|---|
| 45 `effGetEffectName` | `Gullfoss` (likewise `Gullfoss Live`, `Gullfoss Master`) |
| 48 `effGetProductString` | empty string, success |
| 47 `effGetVendorString` | `Soundtheory` |

The result was the same before and after `effOpen`, so the order of calls is not the cause.
The plugins have a real name; the crate asks for the wrong one and cannot ask for the right
one, because its `dispatch` method is private.

Hosts that read opcode 45, or fall back to it, show these names correctly. The other plugins
from the think-aloud test were not probed, so that they share this cause is likely, not
confirmed.

## Decision

**Vendor `vst` 0.4.0 into `crates/vst` and read `effGetEffectName` first**, falling back to
`effGetProductString` when it is empty. The change is one block in `PluginInstance::new`.
`vst-meta` depends on the vendored copy by path, and the crate's `disable_deprecation_warning`
feature is on so its upstream warnings do not appear in every workspace build.

**The file-name fallback stays** in `fill_empty_name`, with a debug log of what the plugin
did answer. It now applies only to a plugin that returns a blank name on both queries.

The vendored crate stays MIT, with its `LICENSE` kept. It is not part of the AGPL root and not
part of the MPL scanner crates, so the boundary in ADR-0054 is untouched; `crates/vst/SEULA.md`
says so and lists the patch.

## Rejected alternatives

- **Only the file-name fallback.** Cheap, and it gives the right text for Gullfoss because the
  files are named after the plugin. But it is a guess: a plugin whose file is named differently
  from its product would show the file name, and the real name sat unread. Kept as the last
  resort only.
- **Make the opcode 45 call from `vst-meta`, no fork.** It needs the raw `AEffect` pointer,
  which `PluginInstance` does not expose, so it would mean a second load of the binary or an
  unsafe workaround around the crate's types. Not tried; the vendored one-line change is simpler
  and keeps the plugin loaded once.
- **Fork on GitHub and use a git dependency.** The upstream crate is deprecated and archived, so
  nothing will be merged back. A git dependency adds a network fetch and a second repository to
  keep alive for a change this small. A path dependency is checkable in one place.
- **Switch to another VST2 host crate.** None considered in depth; replacing the loader for
  one query is out of proportion, and ADR-0004's worker is built around this crate's types.

## Consequences

- Gullfoss, Gullfoss Live and Gullfoss Master scan with their real names (verified with the
  scanner on 2026-10-11).
- The repository now carries about a crate's worth of third-party source that nobody maintains
  upstream. Security and soundness fixes to `vst` will not arrive by themselves.
- The vendored crate has its own tests and some upstream warnings; only `--workspace` runs
  include it. It adds to build time, though it was already being compiled as a dependency.
- Existing database rows with an empty name do not change until the next plugin scan
  (ADR-0012: only a scan writes plugin data).
- Other plugins that implement only one of the two name queries are covered in either
  direction: opcode 45 first, then 48.

## Notes

Reversal is cheap if upstream ever ships the same change: delete `crates/vst`, point
`vst-meta` back at crates.io, keep `fill_empty_name`. Anyone updating the vendored copy should
carry the `SEULA PATCH` block in `host.rs` across; `crates/vst/SEULA.md` is the checklist.

A plugin that returns a blank name on both queries still gets the file name. If that turns out to
happen often, the debug log line `empty product name for ...` in the scanner output is where to
look first.
