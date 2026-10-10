# 0071. The daemon writes daily log files under the app data folder

- **Status:** Accepted, not yet implemented
- **Recognized:** 2026-10-09, in the installer's smoke run: the daemon wrote no log file
  anywhere, and the same day ADR-0070 made it log where a discarded database was backed up
- **Decided:** 2026-10-11
- **Recorded:** 2026-10-11
- **Confidence:** decided now
- **Evidence:** `src/main.rs` (`init_logging`, which only builds a stdout subscriber);
  `src/config/loader.rs` (`set_default_database_path`, `find_config_file`: both resolve
  under `%APPDATA%\Seula`); `src/config/defaults.rs` (`DEFAULT_LOG_LEVEL = "error"`);
  ADR-0069 (the shell starts the daemon with no console window); ADR-0070 (the rebuild
  message is logged at `error`); `scripts/smoke-installer.ps1` (reports zero log files)

## Context

The daemon logs with `tracing`, to standard output. Started from the tray, whether by the
user or by the shell (ADR-0069), it has no console, so everything it logs goes nowhere. A
tester whose library comes up empty, whose scan fails, or whose daemon exits has left no
record of why, and the maintainer cannot ask them to "run it from a terminal" without
explaining what one is.

ADR-0070 added a message that matters to the user: the database was rebuilt and the old one
is saved at a named path. With no log file, that message is lost.

The default level is `error`, so even a log file would hold almost nothing about normal
operation: no record that a scan ran, how many projects it found, or that the plugin scan
loaded anything.

## Decision

**The daemon writes a log file in the app data folder.** The folder is
`%APPDATA%\Seula\logs`, a sibling of `config.toml` and `seula.db`, found the way
`set_default_database_path` finds its folder (`dirs::data_dir()` plus `Seula`). It does not
follow `database_path` if that is moved, so the logs are always where the documentation
says they are.

**It rotates daily and keeps seven files.** One file per day, named `seula.YYYY-MM-DD.log`.
When an eighth appears the oldest is deleted. This uses the `tracing-appender` crate's
rolling writer, which does both, and a non-blocking writer so a slow disk does not stall a
scan. The guard that flushes it on exit is held for the life of the program.

**Both outputs stay.** The file layer is added to the existing standard-output layer, not
instead of it, so running `seula --server` in a terminal during development behaves as it
does now.

**It covers the daemon only.** `init_logging` is shared with the CLI commands, which keep
printing to the terminal and write no file. The Tauri shell's own messages
(ADR-0069 logs a missing or silent daemon to standard error) are not captured by this
record and stay lost in release builds. That is a known gap.

**The default level becomes `info`.** A new `config.toml` is generated with
`log_level = "info"`, so a first run records scans starting and finishing, the plugin
scan's counts, the database backup path and startup. A config that already says `error`
keeps it: a user's explicit value is not overridden. Because the level is one setting for
both outputs, a user who asks for `debug` gets it in the file as well.

**A panic is written to the log.** A panic hook logs the message and location through
`tracing` before the default hook runs, so a daemon that dies leaves a reason behind.

**Logging never prevents the daemon from starting.** If the folder cannot be created or the
file cannot be opened, the daemon prints why to standard error and carries on with
standard output only.

**Nothing is sent anywhere.** The files stay on the machine. A tester who is asked for a
log sends it by hand.

## Rejected alternatives

- **Logging beside the executables.** An upgrade replaces that folder and an uninstall
  removes it, taking the evidence with them. A per-machine install would also make it
  read-only for a normal user.
- **The Windows Event Log.** Writing to it needs a registered event source, and reading it
  needs a tool the tester does not know. A plain file can be opened and attached to a
  message.
- **One file that grows without limit.** A daemon that runs for months would fill the disk
  slowly with no one watching it. The cost of rotating is one dependency.
- **Rotation by size.** It caps the disk use exactly, which daily rotation does not. But
  `tracing-appender` does not offer it, and hand-writing rotation is more code than a
  seven-file cap is worth for an alpha. A single day's log cannot be bounded by this
  record; see Consequences.
- **Leaving the level at `error`.** A log of errors only says that something failed and
  never what the daemon was doing, which is the question a bug report asks.
- **Defaulting to `debug`.** The volume is higher and the lines are for someone reading the
  code. `debug` stays available as a setting.
- **A `/logs` endpoint on the daemon.** It would add API surface for something a user can
  reach by opening a folder, and ADR-0048 keeps the daemon's API to data.
- **Putting the files next to the database.** `database_path` is configurable, and the log
  location should not move with it.

## Consequences

A tester's report can come with the day's log, and the maintainer can see what the daemon
did before it failed. The ADR-0070 backup message is findable.

The log holds the user's file paths: project folders, project names and plugin paths. That
is acceptable for a file that never leaves the machine on its own, but a tester should know
before attaching one. The release notes say so.

A new dependency, `tracing-appender`, from the same project as `tracing`.

Seven daily files bound the number of files, not their size. A scan that logs heavily at
`info` on a very large library could still make one large file. If that happens it is a
reason to log less per project, not a reason to rotate by size.

The log folder holds user data in the sense of ADR-0068: the installer's uninstall leaves
it alone, along with the config and database.

Nothing in the app yet tells the user where the logs are. The tray menu planned in
ADR-0050 is the natural place for an "Open logs folder" item, and until then the release
notes give the path.

## Notes

Reversing is cheap: it is one function in `src/main.rs` and one dependency. The default
level is the part most likely to be revisited, once there is a sense of how noisy `info`
is on a real library.
