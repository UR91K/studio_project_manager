# 0081. The scanner worker ends itself without running exit code

- **Status:** Accepted
- **Recognized:** 2026-10-12, a tester's first-run plugin scan that never finished (the bug
  `plugin-scan-hangs-after-last-plugin`)
- **Decided:** 2026-10-12
- **Recorded:** 2026-10-12
- **Confidence:** decided now
- **Evidence:** ADR-0004 (the worker exists because plugins "cannot be unloaded cleanly");
  the two workers from the tester's runs, still alive hours later, and `cdb` stacks of both;
  `crates/vst-meta/src/main.rs` (`exit_without_cleanup`); `crates/vst-meta/src/diag.rs`

## Context

ADR-0004 moved plugin loading into a worker process because plugins crash, hang, and
"frequently cannot be unloaded cleanly". It assumed the worker could still *exit* cleanly.
It cannot.

On a tester's Windows 10 machine both workers had sent `done` and were still alive hours
later, with one thread left. `cdb` showed it inside `ExitProcess`:

```
vst_meta → ExitProcess → LdrShutdownProcess
  → ni_platform (its C runtime exit table)
    → WSAPoll → NtWaitForSingleObject          blocked forever
```

`ni-platform.dll` is Native Instruments' shared runtime, loaded by Kontakt, Massive, Massive X,
Reaktor and FM8. Those and about fifteen other plugin DLLs were still loaded after the worker
called `FreeLibrary` on each, because they pin themselves in memory. So their detach code
only runs when the process exits, and NI's waits on a socket that never answers.

A normal exit runs the C runtime's `atexit` handlers and then every loaded DLL's detach
handler, all of it third-party code in a process that has just loaded hundreds of plugins.
Waiting on that is exactly what ADR-0004 moved out of the main process to avoid.

## Decision

**After writing `done`, the worker ends itself with `TerminateProcess` on its own process**
(`_exit` on Unix). No `atexit` handler and no DLL detach code runs.

- Its output is already written and flushed after every line. It flushes stdout and stderr
  once more, logs that it is exiting, then terminates with exit code 0.
- It does this on the by-hand `--human` path too. A debugging run that hangs at the end
  helps nobody.
- Nothing the worker owns needs tidying. The operating system reclaims its memory, handles
  and pipes, so the skipped cleanup cannot lose anything.

## Rejected alternatives

- **Unloading plugins properly before exit.** The worker already calls `FreeLibrary`. The
  plugins that hang are the ones that ignore it.
- **Avoiding the offending plugin.** It is one vendor's runtime today, and could be anyone's
  tomorrow. Any plugin's exit code can do this.
- **Relying on the supervisor's kill alone (ADR-0082).** It costs a grace period per worker
  run and logs a warning for something we can prevent. ADR-0082 stays as the backstop for
  whatever this misses, for instance a plugin that calls `exit()` itself.
- **`std::process::exit`.** It runs the same handlers.

## Consequences

A worker that has finished exits in milliseconds however badly its plugins behave on
unload, and the `plugin-scan-hangs-after-last-plugin` hang cannot occur from this cause.

The worker's exit code no longer reflects anything the C runtime would have done at exit.
Nothing reads it beyond the supervisor's log line.

A plugin that calls `exit()` itself still goes through the full exit path. The `atexit`
hook in `diag.rs` logs when that happens, and ADR-0082 kills the worker if it then hangs.
