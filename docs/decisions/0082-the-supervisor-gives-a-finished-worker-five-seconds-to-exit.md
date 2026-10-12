# 0082. The supervisor gives a finished worker five seconds to exit

- **Status:** Accepted
- **Recognized:** 2026-10-12, a tester's first-run plugin scan that never finished (the bug
  `plugin-scan-hangs-after-last-plugin`)
- **Decided:** 2026-10-12
- **Recorded:** 2026-10-12
- **Confidence:** decided now
- **Evidence:** ADR-0004; ADR-0081; `src/scan/plugins/spawner.rs` (`wait_for_exit`);
  `tests/plugin_scanner_tests.rs`
  (`a_worker_that_never_exits_after_done_is_killed_and_the_scan_completes`, which times out
  against the code before this)

## Context

The supervisor's timeout (`vst_scan_timeout_secs`) covers reading the worker's output. Once
`done` arrives the output is over, and the supervisor reaped the process with
`child.wait()`, which has no limit. When the worker hung on exit (ADR-0081) the scan stopped
right there. No timeout fired, no error appeared, and the results it already held were
never saved. The tray app then logged that it had exited, while the process stayed alive
waiting on the scan thread.

## Decision

**After a worker's output ends, the supervisor waits at most five seconds for it to exit,
then kills it. If the kill does not take within another five seconds, it logs an error and
moves on.**

- The results are already collected when the wait starts, so a worker that has to be killed
  loses nothing. It is not a plugin failure, and does not count as a restart.
- Five seconds is generous. Since ADR-0081 a finished worker exits in milliseconds, so
  hitting the limit means something new is wrong, and it is logged as a warning.
- Giving up after the kill accepts leaking one process in exchange for not blocking the
  scan. A process that survives a kill is stuck in the kernel, and nothing the supervisor
  does will help it.
- It is a constant (`EXIT_GRACE`), not a setting. Nobody has a reason to tune it.

## Rejected alternatives

- **The unbounded `wait()`.** The bug.
- **Reusing `vst_scan_timeout_secs`.** That is how long a plugin may take to *load*. Thirty
  seconds of nothing at the end of every scan that hit this would be wasted.
- **Not waiting at all.** It would leave zombie handles, and lose the exit code from the log,
  which is how a crash gets told apart from a clean exit.

## Consequences

No worker behaviour can stop a scan from completing once its results are in. The worst case
costs ten seconds and a logged error.

This is the backstop. ADR-0081 is the fix. If the warning ever appears, the next step is to
find out why a worker that skips all exit code still did not exit.
