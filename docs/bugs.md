# Bugs

One line per bug, newest last. `just bug <name> <description>` appends one; `just bugs`
lists the open ones. Indented lines under an entry are for anything more to say.

- **Format:** ``- [ ] 2026-10-07 `name` description``. The name is short kebab-case and unique:
  commits and `//` comments cite it. No numbers, so two branches cannot pick the same one.
- **Fixing:** tick it (`[x]`) and add the commit, as `Fixed 1a2b3c4.` The cause and the
  fix belong in that commit's message, not here.
- **Not a bug after all:** tick it and say so, as `Not a bug: …` or `Won't fix: …`.

Accepted limitations that will not be fixed live in `docs/status.md` under Known issues.
Bugs fixed before this list existed are written up there under Resolved. See ADR-0065.

- [ ] 2026-10-07 `appledouble-sidecars-scanned` The scan picks up macOS `._*.als` AppleDouble sidecars as projects; they fail to parse, and `test_process_projects_integration` fails on them. tests/integration/scanning.rs
  Moved from `docs/status.md`, found before 2026-09-16. Environment-dependent: it needs
  sidecars in the configured project folders. Fixing it means deciding whether the scanner
  skips `._` files.
  2026-10-11: found on two of the maintainer's real projects (4096 bytes each, magic
  `00 05 16 07`). They now fail with their own error, `FileError::AppleDoubleSidecar`,
  matched on the bytes and not the name (a real set could be called `._x.als`), instead of
  "invalid gzip header". Still picked up, still counted as failures, so still open.
- [ ] 2026-10-07 `real-folder-tests-collide` `test_process_projects_with_progress` fails when run alongside `test_process_projects_integration`: both scan the real configured folders into the same real database at once. tests/integration/scanning.rs
  Moved from `docs/status.md`. Passes serially (`--test-threads=1`) and alone. Both are
  `#[ignore]`d, so it only shows under `cargo test --workspace --tests -- --ignored`.
- [ ] 2026-10-07 `list-default-limit-1000` Most list routes answer 1000 rows when no `limit` is given (`limit.unwrap_or(1000)` across src/database/), while projects and search answer every row.
  Unclear whether 1000 was deliberate; asked 2026-10-07. The frontend no longer depends on
  it: it pages to the route's `total_count`.
- [x] 2026-10-07 `headers-rendering-over-sidebar` Headers render on top of sidebar at window widths lower than 1144px
  Fixed 58a9f95.
- [x] 2026-10-08 `search-negative-offset` A negative offset wraps in `as usize`, so the search page comes back empty; clamp or reject it, and check the other offset casts (project listing too). src/services/search.rs
  Fixed 8b79a6a. Refused with a 400 on search, the project list and the tag's
  project list, through `services::paginate`; a negative limit wrapped the other way.
- [x] 2026-10-08 `media-mutations-200-on-failure` The media routes report failure as 200 with success:false instead of a status code, unlike every other route; web/shared/api.ts works around it. src/http/handlers/media.rs
  The config mutation routes (`src/http/handlers/config.rs`) do the same. The maintainer
  thinks it was chosen because an earlier frontend read better with it, but does not
  remember for certain (2026-10-08). Both need a status code and a matching change in
  `web/shared/api.ts`.
  Fixed 8b79a6a. Failures are 404/400/500; mutations with nothing to return
  are 204; `success` and `error_message` are gone from the media and config responses.
- [x] 2026-10-11 `api-errors-shown-raw` A failed request is shown to the user as the raw `PUT /api/v1/config/settings: 400 {"error":"..."}` text, in Settings and in the status bar notice ("Could not add C:\bad\folder: {...}"). web/shared/api.ts
  Found checking the alpha.2 screens in a browser. `Api.json` and `Api.send` build the message
  from method, path, status and body. The screens want the daemon's `error` text alone.
  Fixed 9876e56. A `failure()` helper in `api.ts` returns the body's `error` when it is JSON.
- [ ] 2026-10-11 `welcome-not-centred` The welcome screen's content sits at the top of the window instead of being centred like the other empty states. Welcome.tsx
- [ ] 2026-10-11 `failed-folder-notice-hidden` After "Scan" on the welcome screen, a folder the daemon rejected is reported only as a status bar notice, which the scan progress then covers, and the welcome screen is already gone. Welcome.tsx
  The folder is dropped from the list when the others succeed, so nothing on screen says it failed.
- [x] 2026-10-11 `project-folders-probed-for-writing` Validating the config creates and deletes a .temp_write_test file inside every project folder, on every load and reload, and warns 'Directory is not writable' for a read-only one. Seula only reads .als files, so neither the probe nor the warning is wanted. src/config/validator.rs
  Maintainer, 2026-10-11: "that shouldn't be doing that, not sure why it ever did that." No ADR
  covers it. Fix: drop the probe and the warning for project folders; keep the read check.
  Origin, both 2025-07-21, neither message gives a reason. `c567773` added the check as a
  `metadata().permissions().readonly()` test under the comment "directories that need to be
  writable", in a general "validation of read/write permissions" pass. `0239457` replaced it
  with the create-and-delete probe because `readonly()` is unreliable on Windows, and never
  asked whether project folders needed checking at all. No ADR or doc says they must be writable.
  Fixed def83dd. Regression test: `validating_project_paths_writes_nothing_into_them`.
- [x] 2026-10-11 `titlebar-breaks-at-extreme-zoom` At extreme zoom levels (found in the Windows Sandbox, 2026-10-11) the title bar breaks: it can shrink below a usable height and the window controls change width and icon size. Wanted: a minimum height on the title bar, a constant width for the window controls, and their icons always the same size. Frame.tsx, parts.tsx
  Fixed 4c9fdb6, 4960450, 47c913d and 47221c3 (ADR-0077, ADR-0078); the first attempt (72148a8)
  did nothing under real zoom. Checked by hand in the Windows Sandbox, 2026-10-12.
- [x] 2026-10-11 `zoom-not-remembered` The window's zoom (Ctrl plus, minus, zero) resets on every launch; checked in the Windows Sandbox 2026-10-11. ADR-0073 left this open: the webview does not keep it, so it waits for the preferences endpoint (ADR-0031, ADR-0062) or the page storing the factor itself.
  Fixed 2ddf68e (ADR-0080): the page stores the factor in `localStorage` and restores it at
  startup. Checked by hand in the Windows Sandbox, 2026-10-12.
- [x] 2026-10-11 `vst2-name-blank` SoundTheory Gullfoss, Gullfoss Live and Gullfoss Master (VST2) were stored with an empty name; a think-aloud test showed the same on other plugins. crates/vst/src/host.rs
  Cause: the `vst` crate reads the name from `effGetProductString` (48) alone. These plugins
  answer 48 with an empty string and success; only `effGetEffectName` (45) has the name
  (probed against the three DLLs, 2026-10-11). The crate cannot send 45 (`dispatch` is private).
  Fixed 04c02ae, with the crate vendored (ADR-0079). `a42cae2` adds the file name as a last
  resort for a plugin that answers neither, and logs what it did answer.
- [x] 2026-10-11 `vst2-main-entry-not-found` A VST2 that exports only the classic `main` entry point was reported as "NotAPlugin (invalid format)", so it never scanned. crates/vst/src/host.rs
  The loader looked for `VSTPluginMain` only. On the maintainer's machine, 2 of about 217 DLLs
  (`FL Studio VSTi.dll`, `FL Studio VSTi (Multi).dll`) export only `main`. A tester with a few
  hundred plugins is likely to have more. Fixed 350838c: try `VSTPluginMain`, then `main`.
- [x] 2026-10-11 `vst-load-handoff-race-and-leak` Loading a VST2 passed the host to the plugin through a `static mut` that two simultaneous loads could overwrite with each other's, and leaked two boxed host handles per plugin. crates/vst/src/host.rs
  Latent for Seula (the worker loads one plugin at a time, and is short-lived), found reading
  the vendored crate's `unsafe` code. Fixed bc4c748. A lock and an `AtomicPtr`; the handles are
  freed when the plugin shuts down. Regression tests, one of which fails without the lock.
- [x] 2026-10-11 `vst-host-callback-panic-aborts` A panic in a host callback unwound out of an `extern "C"` function into the plugin, which aborts the whole worker process, and the supervisor would blame the plugin in flight. crates/vst/src/host.rs
  Also: `get_input_info` and `get_output_info` read a buffer the plugin might never write
  (`assume_init`), which is undefined behaviour. Neither is reachable from the scanner today.
  Fixed ba0a9c1. The panic is caught and answered as 0, and the buffer is zeroed first.
  Regression test: `a_panicking_host_callback_does_not_cross_the_ffi_boundary`, which aborted
  the test process before the fix.
- [ ] 2026-10-12 `plugin-stdout-desyncs-scan` A plugin that prints to stdout without a trailing newline swallows the worker's next protocol line, and the supervisor, which attributes results by position, then records every following result against the plugin before it. src/scan/plugins/spawner.rs
  Seen on a tester's Windows 10 machine: Hadron (`hadron64.dll`) prints `ftable 1:` to
  `ftable 172:` and ends on a bare `ft`, which glued onto its own `result` line. The ~150
  plugins after it were stored with their neighbour's metadata, in both runs.
- [ ] 2026-10-12 `plugin-scan-hangs-after-last-plugin` The first-run plugin scan stops after the last plugin's `begin` and never completes, and the timeout does not fire. src/scan/plugins/spawner.rs
  Same tester, both runs. Tracing added across the supervisor and the worker
  (`seula::scan::plugins::worker`) to confirm the cause before fixing.
  2026-10-12, confirmed on the tester's machine: both workers were still alive hours later
  with one thread left, inside `ExitProcess`. cdb shows it in `LdrShutdownProcess` running
  `ni-platform.dll`'s (Native Instruments NTK, pulled in by Kontakt, Massive, Reaktor, FM8)
  CRT exit table, which blocks forever in `WSAPoll`. NI plugins stay loaded after
  `FreeLibrary`, so the detach only runs at exit. The worker had sent `done`, so the
  supervisor sat in `child.wait()`, which has no timeout. The tray app logged "exited
  normally" and its process stayed alive too, holding the scan thread.
- [ ] 2026-10-12 `version-tests-fail-on-tester-machine` `test_version_parsing` and `test_beta_version` fail on a tester's Windows 10 machine: `patch` comes back 0 for `MinorVersion="12.0_12049"`, expected 12049. tests/scan/parser/basic.rs
  Fails with and without the plugin-scan tracing changes, so it predates them. Not seen on
  the maintainer's machine. Unverified guess: this checkout has `core.autocrlf` converting
  to CRLF, and both tests embed their XML in raw strings.
- [ ] 2026-10-12 `tray-exit-leaves-process-running` The tray app logs "Tray app exited normally" and its process stays alive while a scan's blocking thread has not finished. src/main.rs
  Seen on the tester's machine: both `seula.exe` processes from the `plugin-scan-hangs-after-last-plugin`
  runs were still alive hours after logging that line. Inferred, not traced: the tokio runtime
  waits for `spawn_blocking` tasks when it shuts down. Harmless since ADR-0082 bounds the scan's
  wait, but any other blocking task that never ends would do the same.
