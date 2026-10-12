# Changelog

What changed in each release, for people installing Seula. Newest first. The reasoning
behind a change is in `docs/decisions/` (the ADR numbers below), what is broken is in
`docs/bugs.md`, and what state each part is in is in `docs/status.md`.

Seula is in alpha. Versions run `0.2.0-alpha.N` until a beta, and the first stable release
is `1.0.0`. The Windows installer is not code-signed yet, so SmartScreen warns on first
run ("More info", then "Run anyway").

## 0.2.0-alpha.2 (not yet released)

### New

- **Welcome screen.** On a fresh install, Seula asks which folders hold your Live projects,
  using the normal Windows folder dialog, and scans them. "Not now" skips it. (ADR-0074)
- **Settings.** The gear in the title bar opens a dialog for project folders, plugin search
  folders, the plugin scan time limit, cover art and audio size limits, and the log level.
  It also shows where the database and media folder are, and which settings need a restart.
  (ADR-0074)
- **Drop a folder to add it.** Drag a folder from Explorer onto the Projects view and it is
  added as a project folder and scanned. (ADR-0073)
- **Menus.** File, Edit, View, Tools and Help now hold the actions the app already had, with
  shortcuts. (ADR-0075)
  - File: Add project folder, Settings (`Ctrl+,`), Open config file, Open logs folder, Close window.
  - Edit: Find (`Ctrl+F`).
  - View: the five views (`Ctrl+1` to `Ctrl+5`), Sidebar (`Ctrl+B`), Inspector (`Ctrl+Alt+B`),
    sharp or flat key names.
  - Tools: scan projects, scan plugins (new and changed, or all), check samples, start or stop
    the folder watcher, export statistics as CSV, and Start, Stop and Restart daemon.
  - Help: About.
- **Start daemon button.** If Seula's background service is not running (you quit it from the
  tray, or it stopped), the "Cannot reach the daemon" screen now has a Start button. (ADR-0073)
- **Stop and restart from the window.** Tools has Stop daemon and Restart daemon. Stopping uses
  a new `POST /api/v1/system/shutdown` that ends the daemon the same way the tray's Quit does.
  (ADR-0076)
- **Zoom.** `Ctrl` plus, minus and zero, and `Ctrl` with the mouse wheel. The title bar behaves
  like VS Code's: the window controls stay 138 pixels wide at every zoom, the bar is at least 35
  pixels tall, and zoomed out the whole bar keeps its size. Help > About shows the zoom the app
  sees. (ADR-0073, ADR-0077, ADR-0078)
- **Where things are.** `GET /api/v1/system/info` now reports the config file and the log
  folder, which Open config file and Open logs folder use. (ADR-0076)

### Changed

- **Config changes apply without a restart.** Editing `config.toml`, or changing a setting in
  the app, now reaches the running daemon. Project folders, plugin search folders, the plugin
  scan time limit, the size limits and the log level apply at once. The HTTP port, database path
  and media folder still need a restart, and Settings says so. A mistake in `config.toml` is
  reported and the last good settings stay in force. A running folder watcher follows the list of
  project folders. (ADR-0072)
- **The config file is saved safely.** It is written to a temporary file and renamed, so nothing
  reads a half-written config.
- **Project folders are only read.** Seula no longer creates and deletes a test file in each
  project folder every time the config loads, and no longer warns that a read-only folder is
  "not writable". (`project-folders-probed-for-writing`)
- **Errors read properly.** A failed request shows the daemon's own message, not
  `PUT /api/v1/... 400 {json}`. (`api-errors-shown-raw`)

### Fixed

- **Ableton Live version.** The version shown for a project came from a schema marker that lags
  behind the Live that saved it (a Live 12.2 set read as 12.0). It now comes from the file's
  `Creator`, with the old marker as the fallback.
- **VST2 plugins with no name.** Plugins such as SoundTheory Gullfoss, which answer the usual
  name query with nothing, are now named from the query they do answer. A plugin that answers
  neither falls back to its file name. This needed a vendored copy of the `vst` crate. (ADR-0079,
  `vst2-name-blank`)
- **VST2 plugins that only export `main`.** The FL Studio VSTi DLLs, for example, were reported
  as "not a plugin" and never scanned. (`vst2-main-entry-not-found`)
- **Plugin loading is safer.** A race and a memory leak when handing the host to a plugin, a panic
  that could cross into the plugin and abort the scanner, and a read of uninitialised memory are
  fixed. None was reachable in normal use, but they were found while vendoring the crate.
  (`vst-load-handoff-race-and-leak`, `vst-host-callback-panic-aborts`)
- **macOS sidecar files.** A `._name.als` file left beside a project by a copy from a Mac volume
  is now reported as an AppleDouble sidecar, not as a corrupt project ("invalid gzip header").
  They are still picked up and counted as failed scans. (`appledouble-sidecars-scanned`)

### For developers

- `just smoke` runs the installer in a clean Windows Sandbox and checks 41 things, including
  live config, the shutdown route and the reported paths. `just sandbox` opens the installer in
  an interactive sandbox with sample Live sets and pictures. Both need `just release` first.
- `just release` builds the scanner worker (`vst-meta`) along with the daemon.
- `just version` shows the version and whether all seven files that carry it agree;
  `just version 0.2.0-alpha.3` sets it in all of them.
- Debug builds keep line tables only, which roughly halves the size of `target`.
- New decision records: ADR-0072 to ADR-0079.

### Known issues

- Zoom is not remembered between launches. (`zoom-not-remembered`)
- Dragging an image onto a collection's cover art may not work in the desktop app; the button
  next to it does. It has not been tried in the installed app.
- On the welcome screen, a folder Seula rejected is reported only briefly in the status bar, and
  the content sits at the top of the window instead of the middle. (`failed-folder-notice-hidden`,
  `welcome-not-centred`)
- The title bar's sizing at extreme zoom has been checked by emulation and against VS Code's
  rules, and not yet by hand in the installed app. (`titlebar-breaks-at-extreme-zoom`)
- If `config.toml` cannot be read at startup, the window only says the daemon cannot be
  reached; the reason is on the console.
- The folder watcher is not started automatically; start it from Tools.
- Windows only. The installer is not signed.

## 0.2.0-alpha.1

The first release of the rewrite, replacing the old command-line and terminal interface with a
desktop app and a background service. Tagged on GitHub; no release page was published.

- **One Windows installer** built with Tauri. The app window, the background service and the
  plugin scanner are installed together, and the installer needs no extra runtime. (ADR-0068)
- **The window starts the service** if it is not running. (ADR-0069)
- **The database is backed up** before it is discarded for a newer schema, as
  `seula.backup-v<version>-<time>.db` beside it. The database may still be rebuilt between alpha
  versions. (ADR-0070)
- **Log files** are written daily to `%APPDATA%\Seula\logs`, seven kept. They contain project
  paths. (ADR-0071)
- Help > About with the version, licence and source address. The tray icon is the Seula "S".
  The icon font is bundled, so nothing is fetched from the internet.

Earlier releases (`0.1.0-alpha.1` and `0.1.0-alpha.2`, once called `1.0-alpha` and `1.0.1-alpha`)
were the command-line version.
