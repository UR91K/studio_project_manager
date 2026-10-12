# 0072. Config changes reach the running daemon

- **Status:** Accepted
- **Recognized:** 2026-10-11, reading the config code after a tester asked for the daemon
  to refresh its config by itself
- **Decided:** 2026-10-11 for the split between live and restart-required settings, which
  the maintainer confirmed; the mechanism below is proposed
- **Recorded:** 2026-10-11
- **Confidence:** decided now
- **Evidence:** `src/config/mod.rs` (`CONFIG` is a `Lazy` read once; `Config::reload`
  returns a new `Config` that nothing keeps); `src/services/config.rs` (the mutations clone
  the static, change the clone, write `config.toml`, and return the clone to a caller that
  drops it); `src/services/system.rs` (scans and `start_watcher` read `CONFIG`);
  `src/http/handlers/config.rs`; `src/main.rs` (`build_shared_state` reads the database
  path, media folder and media limits once at startup); the tester's session notes

## Context

The config reaches the daemon once, at startup. `CONFIG` is a `Lazy` static. The config
routes (`/api/v1/config/paths`, `/paths/remove`, `/settings`, `/reload`) all work on a
clone: they write `config.toml` and answer success, and the running daemon carries on with
the values it started with. `reload` builds a fresh `Config` and discards it. So adding a
project folder through the API changes the file and not the next scan, and editing the file
by hand changes nothing until the daemon restarts.

The tester's notes ask for the daemon to refresh its config by itself. Four other notes
need it as well: a Settings screen, onboarding that picks folders, dropping a folder on the
Projects tab, and any menu item that adds one (ADR-0074). All of them add a path and
expect the next scan to use it.

Separately, the daemon never starts its folder watcher on its own. `start_watcher` is only
reached from `POST /api/v1/system/watcher/start`. So "a watched folder" today means a folder
in `paths`, which scans read, plus a watcher the user may or may not have started.

## Decision

**The config is held in a swappable cell, read as a snapshot.** A `config::current()`
accessor returns the present `Config` behind an `Arc`. Each operation takes one snapshot
when it starts and uses it throughout. A scan already in flight keeps the snapshot it began
with. Nothing holds a snapshot across a scan's end or caches a field.

**Four things replace the cell's contents:**

- the config routes that mutate (`paths`, `paths/remove`, `settings`), after they have
  saved the file;
- `POST /api/v1/config/reload`, which now does what its name says;
- a watcher on `config.toml`, so a hand edit is picked up with no action;
- nothing else. A failed load never replaces a good config.

**The file watcher watches the folder, not the file, and waits.** Editors save by writing a
temporary file and renaming it, so a watch on the file alone loses it. Events are debounced
by half a second. The daemon's own writes are recognised, because the parsed result equals
the current config, and cause no second reload. A missing file mid-rename is ignored.

**A bad edit changes nothing.** If the file does not parse or does not validate, the
previous config stays in force, the reason is logged at `error`, and
`GET /api/v1/config/status` reports it in a new `reload_error` field until the next good
load. A half-saved file while the user is typing must not stop scans.

**Which settings apply at once, and which wait for a restart:**

| Applies live | Needs a restart |
|---|---|
| `paths`, `vst_search_paths`, `vst_scan_timeout_secs` | `http_port` |
| `max_cover_art_size_mb`, `max_audio_file_size_mb` | `database_path` |
| `log_level` | `media_storage_dir` |

The right-hand settings are held by something that is already running: the listening
socket, the open database connection, the media store's folder. For these the file is
updated and the running daemon keeps its value. `GET /api/v1/config/status` lists the keys
whose file value differs from the running one in a new `restart_required` field, so a
screen can say so. The environment overrides (`SEULA_HTTP_PORT` and the rest) still win and
are not live.

**The folder watcher follows `paths` if it is running.** On a change to `paths`, a running
watcher gets the added folders and drops the removed ones. A watcher that is not running is
left off. This record does not start the watcher by itself. That is a separate decision with
its own cost, because it holds a handle on every configured folder.

**The CLI's direct commands are unchanged.** They are short-lived and read the config once.

## Rejected alternatives

- **Restart the daemon on a config change.** It would end a scan in progress and make the
  tray icon blink, to apply a setting that can be applied in place.
- **A reload button and nothing automatic.** The tester asked for the opposite, and the
  watcher is a few dozen lines on a crate already in use.
- **Polling the file on a timer.** A watcher already exists in the tree, and polling is
  both slower to react and a wake-up every few seconds for a file that rarely changes.
- **Making every setting live.** The port, the database path and the media folder belong
  to components that hold them open. Swapping them under a running scan or request is how a
  settings screen loses someone's library.
- **Leaving it as it is and telling users to restart.** That is the behaviour the tester
  found, and the routes that look as though they work are a trap.
- **Starting the watcher automatically as part of this.** It is a larger change than a
  config refresh, and nobody has asked for it.

## Consequences

About a dozen call sites of `CONFIG` change to take a snapshot. The static stays for the
CLI's one-shot reads until the CLI is replaced (ADR-0050).

The media size limits and the log level are currently fixed at startup, in the media store
and the logging filter, and have to be made settable to be live. That is real work in two
places, not a flag.

Any test that changes the config must point `SEULA_CONFIG` at a temporary file. The save
path writes wherever the config was found, which on a developer's machine is the real
`config.toml`. A test that forgets passes locally, and on a machine with no config fails
or, worse, overwrites it.

A reload that fails is reported in two places, the log and the status route, and the daemon
carries on with stale values. A user who edits the file wrongly sees nothing change. The
Settings screen is where that should be shown (ADR-0074).

## Notes

Reversing is moderate: the accessor and the watcher are separable from the call-site
changes, but the call sites would have to go back to the static.
