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
- [ ] 2026-10-11 `titlebar-breaks-at-extreme-zoom` At extreme zoom levels (found in the Windows Sandbox, 2026-10-11) the title bar breaks: it can shrink below a usable height and the window controls change width and icon size. Wanted: a minimum height on the title bar, a constant width for the window controls, and their icons always the same size. Frame.tsx, parts.tsx
- [ ] 2026-10-11 `zoom-not-remembered` The window's zoom (Ctrl plus, minus, zero) resets on every launch; checked in the Windows Sandbox 2026-10-11. ADR-0073 left this open: the webview does not keep it, so it waits for the preferences endpoint (ADR-0031, ADR-0062) or the page storing the factor itself.
