# Documentation

## The rule

**One fact, one home.** Every kind of information has exactly one authoritative
location. When something is true in two places, it is wrong in one of them within a
month.

| Kind of information | Home | Mutable? |
|---|---|---|
| What state a subsystem is in | `status.md` | Yes — this is the only status source |
| What is broken, and whether it is fixed | `bugs.md` | Yes — one line per bug (ADR-0065) |
| Why a choice was made | `decisions/` (ADRs) | **No** — append-only |
| How the pieces fit together | `architecture/` | Yes |
| How one module works | the `//!` header in that file | Yes |
| How to install and use Seula | `/README.md` | Yes |
| What changed in each release | `/CHANGELOG.md` | Yes — a section per release, written at release time from the commits and ADRs since the last tag |

If you want to write down a fact, find its home above and put it there. Do not
summarise it somewhere else "for convenience" — that is how the previous generation of
docs in this folder rotted.

## Decisions (ADRs)

One decision per file, numbered, in `decisions/`. Start from
`decisions/0000-template.md`.

ADRs are **immutable**. A decision that is later reversed does not get edited — it gets
its `Status` changed to `Superseded by NNNN` and a new ADR is written. An ADR is a
record of what was believed and why at a point in time; rewriting it destroys the only
thing it was for.

`Status` is one of:

- **Accepted** — decided and implemented.
- **Accepted (retrospective)** — a real decision, reconstructed after the fact from git
  and memory. Most of the early ones.
- **Reaffirmed** — considered, deliberately *not* changed. These produce no diff and are
  invisible to git, so they only exist if someone writes them down. They are the most
  valuable kind, because they pre-empt a suggestion rather than merely recording one.
- **Incidental** — not a deliberate choice; this is just what got written. Recorded
  because something now depends on it. Honest about its own origin.
- **Superseded by NNNN** — no longer in force. Kept, never deleted.

Every ADR carries a `Confidence` line: *decided now*, *remembered*, *reconstructed from
git*, or *inferred from code*. That word tells the reader how much weight the stated
rationale can bear, which is the thing retrospective ADRs most often get wrong. Do not
invent a rationale you do not have — an `Incidental` record is more useful than a
plausible fiction, because a fiction manufactures confidence in a choice nobody made.

## Bugs

`bugs.md` is a checklist, one line per bug, so a bug can be written down in the middle
of a review without stopping: `just bug <name> <description>` appends one, and
`just bugs` lists the open ones. A fixed bug is ticked with its commit; why it broke
belongs in that commit's message. Accepted limitations that will not be fixed stay in
`status.md` under Known issues. See ADR-0065.

## Writing paths

`tests/docs_tests.rs` checks that every repo-relative source path named in `CLAUDE.md`
or under `docs/` still resolves. That is what stops these documents rotting silently
when a file moves.

It means one convention matters:

- A path to a **live** file gets its full form — `src/scan/parser.rs`. Greppable,
  clickable, and checked.
- A path to a **deleted** file — common in retrospective ADRs, where the evidence is
  something that no longer exists — is written without the root prefix, so it reads as
  prose: `utils/tempo.rs`. Pair it with the command to recover it, e.g.
  `git show 8ee0622^:src/utils/tempo.rs`.

`docs/archive/` is exempt; its paths are expected to be stale.

## Architecture

`architecture/overview.md` is the map. `architecture/plugins.md` covers plugin
identity and scanning, which is the most intricate part of the system and the one most
likely to be misunderstood.

Architecture docs describe *shape*. When you catch yourself explaining *why* a shape was
chosen, that belongs in an ADR — link to it instead.

## Archive

`archive/` holds superseded planning documents, kept only so links do not rot. Nothing
in there is current and some of it is actively wrong. Do not read it for orientation.

## Not part of this system

`.kiro/` is a separate tool's configuration (steering files and a spec). It overlaps
with `CLAUDE.md` and `architecture/`, and its `steering/structure.md` is already stale.
It has deliberately been left alone — running two documentation systems in parallel is
what produced the mess this one replaces, so it needs a decision rather than an edit.
See the triage entry in `status.md`.

`FRONTEND_SPEC.md`, `REQUIRED_FEATURES.md`, `TUI_ARCHITECTURE_ANALYSIS.md` and
`TUI_PROJECT_PLAN.md` at the repo root are older planning documents whose status has not
been verified. Also listed for triage in `status.md`.
