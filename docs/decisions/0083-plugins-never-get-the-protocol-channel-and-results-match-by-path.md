# 0083. Plugins never get the protocol channel, and results are matched by path

- **Status:** Accepted
- **Recognized:** 2026-10-12, a tester's first-run plugin scan (the bug
  `plugin-stdout-desyncs-scan`)
- **Decided:** 2026-10-12
- **Recorded:** 2026-10-12
- **Confidence:** decided now
- **Evidence:** ADR-0004 (the protocol, and position as the identity of a result); the
  tester's log, where `hadron64.dll`'s output ends on `ft` and its result line follows it as
  `ft{"event":"result",...}`; `crates/vst-meta/src/stdio.rs`;
  `crates/vst-meta/tests/stdout_redirect.rs`; `src/scan/plugins/spawner.rs` (`collect`);
  `tests/plugin_scanner_tests.rs`

## Context

The worker's protocol (ADR-0004) runs over its stdout, and so does anything a plugin prints.
Hadron prints Csound's function tables, `ftable 1:` to `ftable 172:`, and ends on a bare `ft`
with no newline. The worker's next line, Hadron's own result, joined onto it and could not be
parsed.

The supervisor identified each result by its position in the stream and used the echoed path
only as a check. One lost line therefore moved every later result onto the plugin before it.
On the tester's machine that was about 150 plugins, stored with their neighbour's metadata,
in both runs.

Two things were wrong, and either is enough to cause the damage: a plugin could write to the
protocol channel at all, and one lost line could shift everything after it.

## Decision

**The worker keeps stdout to itself.** Before it loads any plugin, it duplicates its stdout
pipe for the protocol and then points standard output at stderr, at every level a plugin
can reach it from:

- the Win32 standard handle, which statically linked C runtimes also read when they load;
- descriptor 1 of the shared Universal C Runtime (`ucrtbase.dll`);
- descriptor 1 of the legacy `msvcrt.dll`, which MinGW-built plugins use. It is loaded if it
  is not already, and kept loaded.

On Unix this is one `dup` and one `dup2`. From then on the protocol is written only through the
duplicated handle. What plugins print reaches the parent on stderr, where it is logged in the
in-flight plugin's span. The `--human` debugging mode is left alone.

**The supervisor matches results by path.** A `result` is recorded against the batch entry with
its own echoed path. Position only says where to start looking.

- A result whose line is lost is detected when the worker moves on: the next `begin`, or
  `done`, arrives with that plugin still in flight. The plugin is recorded with a new error
  type, **`ResultLost`** (`result_lost` in the database), rather than given a neighbour's data
  or dropped.
- A lost `begin` is harmless. The `result` still names its path.

**A `result_lost` file is never skipped as unchanged** (ADR-0067). It may have loaded fine, so
the next scan asks it again.

## Rejected alternatives

- **Only the supervisor fix.** Hadron would be recorded as `ResultLost` on every scan, forever,
  for a plugin that loads perfectly well.
- **Only the worker redirect.** It covers every route we know of, but a plugin that writes to a
  handle it opened itself, or one that slips past the redirect, would put everything back
  on a shift by one. The supervisor fix is the backstop.
- **Reusing `LoadFailed` for a lost result.** It would claim the plugin refused to load, which
  is not what happened, and it would be skipped as an unchanged failure.
- **Moving the protocol off stdout** (a named pipe or a socket). That is a bigger change to
  ADR-0004's design for the same result: the duplicated handle already is a private channel.

## Consequences

Nothing a plugin prints can reach the protocol through the standard handle or either C
runtime. If something does get through, it costs that one plugin, marked as result lost and
retried on the next scan, and nothing else.

`std::io::stdout()` in the worker now writes to stderr. Any code that prints to stdout to talk
to the parent has to use the claimed handle, which the protocol writer in `main.rs` is the
only user of.

The web UI labels `result_lost` as "Its scan result was lost; scan again".
