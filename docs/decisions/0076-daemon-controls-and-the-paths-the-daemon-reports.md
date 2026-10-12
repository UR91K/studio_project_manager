# 0076. Daemon controls in the window, and the paths the daemon reports

- **Status:** Accepted
- **Recognized:** 2026-10-11, while settling the menus (ADR-0075)
- **Decided:** 2026-10-11
- **Recorded:** 2026-10-11
- **Confidence:** decided now
- **Evidence:** ADR-0069 (the shell starts the daemon); ADR-0073 (`start_daemon`, and the
  rule that the shell does not touch the API); ADR-0071 (log files); ADR-0072 (the config
  file); ADR-0024 (the HTTP surface, loopback only); `src/tray.rs` (Quit sends a shutdown
  message and the tray loop ends); `src/main.rs` (when the tray ends, the HTTP server task is
  aborted); `src/logging.rs` (`log_dir`); `src/http/dto/system.rs`; `web/shared/os.ts`

## Context

The maintainer wants Start, Stop and Restart for the daemon in the window's menus
(ADR-0075). Start exists: the shell's `start_daemon` (ADR-0073). Stop does not: the only way
to end the daemon is the tray's Quit, which the window cannot reach. Restart is Stop then
Start. The same menus also offer Open config file and Open logs folder, and the page does not
know where either is.

## Decision

**`POST /api/v1/system/shutdown`.** It answers `202` and then stops the daemon by the same
route as the tray's Quit: the shutdown message the tray loop already listens for, so the tray
icon is removed cleanly and the server ends as it does today. In server-only mode, which has no
tray, it ends the process the same orderly way. It takes no body. The answer is sent before
the daemon goes.

Anything on the loopback port can call it. That is already true of every route that changes
or deletes data (ADR-0024), so it adds no new class of exposure, and a daemon being stoppable
by a local program is the same as it being killable by one.

**Restart is done by the page, not the daemon.** The page posts the shutdown, waits until
`/health` stops answering (a bounded wait), then calls `start_daemon` (ADR-0073). A daemon
that restarts itself has to start its replacement while it still holds the port, which is the
fragile order. If the start fails the page shows the reason and the error screen's Start
button is there.

**During a deliberate Stop or Restart the error screen stays quiet.** The page holds a
`restarting` flag around the sequence, so the unreachable-daemon screen does not flash up for a
daemon that was stopped on purpose. After a plain Stop the screen does appear, which is the
honest state and the way back (its Start button).

**`GET /api/v1/system/info` reports two paths:** `config_path`, the `config.toml` in use, and
`log_dir`, the folder the daemon writes logs to (`null` when it has none). The shell's
`reveal_in_explorer` is extended to open a folder when given one, and to select a file as it
does now. No new shell command is needed.

## Rejected alternatives

- **A shell command that kills the process.** The shell would need the daemon's process id and
  would end it without letting the tray remove its icon. It also makes the shell act on a
  program it started and does not own.
- **A restart endpoint.** See above: the replacement would race the port.
- **Hiding Stop.** The tray can already quit the daemon, and a window that can start but not
  stop one is odd.
- **The page computing the log folder and config path itself.** It cannot: they depend on the
  platform's app data folder and on `SEULA_CONFIG` (ADR-0072).

## Consequences

One new route and two new fields. The `SystemInfo` type in `types.ts` gains `config_path` and
`log_dir`, optional until the daemon sends them.

The tray's Quit and the new route must end the daemon identically, which is a test: call the
route against a running server and check it stops.

There is still no single-instance guard (ADR-0050). Start remains safe because `start_daemon`
asks `/health` first, but two clicks that race, or a Start while another program starts the
daemon, can still make two. Restart makes the window longer by the time a stop takes, so it
adds to the case for the guard.

## Notes

Quitting the daemon from the window ends the window's data until it is started again, as the
tray's Quit already does. The Stop item in Tools is labelled to say so only if testing finds
people expect otherwise.
