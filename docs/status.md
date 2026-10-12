# Status

The single source of truth for what is done, in progress, deliberately out of scope, or
known-broken. If status appears anywhere else in this repo, that copy is wrong.

Last reviewed: 2026-10-11.

## Subsystems

| Subsystem | State | Notes |
|---|---|---|
| `.als` parsing | **Done** | Single-pass state machine. ~160–270 MB/s. ADR-0001 |
| Parallel parsing | **Done** | Hand-rolled pool. Was silently serial until 2026-09-15; ADR-0002 |
| Project scanning / discovery | **Done** | `src/scan/project_scanner.rs` |
| SQLite storage | **Done** | 5NF schema, `src/database/core.rs` |
| FTS5 search | **Done** | Operators: `name:`, `path:`, `plugin:`, `sample:`, `tag:`, `collection:`, `key:`, `bpm:`, `ts:`, `version:`, `dc:`, `dm:`. `collection:` filters by membership, not text. Quoted values keep their spaces |
| CLI | **Done, to be replaced** | 33+ commands, table/JSON/CSV via `src/cli/output.rs`. Opens the database directly. To be replaced by a CLI client that talks to the daemon over HTTP (ADR-0050, which reverses this part of ADR-0046); `--config` and `--server` stay |
| CLI client over HTTP | **Not started, decided** | ADR-0050. A separate small program on the `PATH`, no interactive mode. Lands before the old subcommands are deleted, which serve as its reference. A client that finds no daemon starts it; the daemon needs a single-instance guard first (none exists). Starting the daemon from the CLI is disabled if it trips antivirus before the executables are signed. Executables: `seula` is the CLI and the daemon is renamed `seula-service` (not done yet). Tray menu: Open, Rescan, Start with Windows (also in the GUI's settings), Quit |
| gRPC API | **Removed** | ADR-0046. `src/grpc/`, `proto/`, `build.rs`, tonic and prost, and the `grpc_port` setting are gone. A config file that still has `grpc_port` loads fine; the key is ignored |
| HTTP API | **Done** | `src/http/`, axum 0.8, all 9 `Services` domains plus `system`. ADR-0024 |
| Tray mode | **Done** | `src/tray.rs`; default when run with no subcommand |
| File watcher | **Done** | `src/watcher/`, streams over SSE |
| Tags / collections / tasks / notes | **Done** | |
| Media (cover art, audio) | **Done** | `src/media/`, size limits configurable |
| Plugin metadata via Ableton DB | **Removed** | Retired 2026-09-15. ADR-0006 |
| Plugin scanner worker | **Done** | `crates/vst-meta`. ADR-0004 |
| Plugin scanner supervisor | **Done** | `src/scan/plugins/`. ADR-0004 |
| Plugin scan → database | **Done** | `src/database/plugin_scan.rs`. `seula plugin refresh` scans and persists: by default only new and changed files, `--all` every file. Files that fail are remembered and shown as Failed. ADR-0067 |
| First-run plugin scan | **Done** | Runs before project discovery when one has never completed. ADR-0013 |
| Plugin identity (`PluginKey`) | **Done** | `src/models.rs`. Parses `dev_identifier`, derives from a scanned uid. ADR-0005 |
| uid-based plugin matching | **Done** | In the batch layer, with the `plugin_classes` fallback. ADR-0005, ADR-0009 |
| `plugins` table restructure | **Done** | ADR-0007, ADR-0009, ADR-0010, ADR-0011; tri-state `installed` is ADR-0012 |
| Web frontend | **In progress: all five views built** | Shape in `docs/architecture/frontend.md`. Stack ADR-0029 and ADR-0047 (Solid, not React; the projects screen was built in both Solid and Svelte 5, 2026-10-04, and the maintainer chose Solid the same day: it keeps what runs close to what was written, as ADR-0047 asks, its failure mode is a visibly stale UI where Svelte's whole-array `data.projects = list` re-derives everything silently, and its bundle is smaller, 50 kB against 75 kB of JS; both builds and type checks were clean and the styling identical, no runtime numbers were taken; the Svelte copy is removed and recoverable from commit b0a89c7; the projects and plugins views are built, 2026-10-04, with the plugin scan's SSE stream exercised against a real daemon; routing is a signal, not a router, ADR-0055 (accepted 2026-10-04); the scan stream is read from the POST response body, ADR-0056 (accepted 2026-10-04); the window has no native frame, the top bar is the title bar with its own controls and a native overlay for Windows 11 snap layouts, ADR-0057 (accepted 2026-10-04, run by the maintainer and working); the samples view is built, 2026-10-05, with a real sample check exercised against a scratch copy of the mock database; it has no Play item, which needs a Tauri command and an ADR first; the collections view is built, 2026-10-05, exercised against a scratch copy of the mock database (list, sort, search, create, edit, duplicate, delete, open, drag-reorder, remove, cover upload); its decisions are ADR-0058 to ADR-0062, and its task list is read-only; the stats view is built, 2026-10-05, checked in a browser against the mock daemon in both scopes, with hover, the sharp/flat switch and an empty library (the last by faking the response); its scope signal, inert search box and plain-download CSV are ADR-0063, proposed, and the CSV download has not been tried inside Tauri; see `web/README.md`), Tauri-only thin shell over the tray daemon ADR-0048 (the Tauri trial passed 2026-10-04: opens about as fast as the same page in a browser tab, judged by the maintainer, no numbers taken; shell in `web/solid/src-tauri/`, outside the Cargo workspace; the browser build is not needed), preferences ADR-0031, shell and density ADR-0032. Static mockups come before any framework code. The first round was rejected for inconsistency and is kept on the `old-mockups` branch. Second round, as review boards (ADR-0036): all approved (2026-10-04): shell, projects, plugins, samples, collections (ADR-0044) and stats (ADR-0045) |
| Windows installer | **Done (0.2.0-alpha.1 tagged, release page not yet published; 0.2.0-alpha.2 versions set, not tagged)** | One NSIS installer built by Tauri (`just release`); the daemon and the plugin scanner worker are sidecars beside the shell. Unsigned, so SmartScreen warns. Checked by `scripts/smoke-installer.ps1` in Windows Sandbox. ADR-0068 |
| Shell starts the daemon | **Done** | At launch, if `/health` does not answer. ADR-0069. The "Start daemon" button, a folder picker and zoom hotkeys are ADR-0073 (alpha.2, built, not yet tried in the shell) |
| Database backup before a schema discard | **Done** | `VACUUM INTO` a file beside the database. ADR-0070 |
| Daemon log files | **Done** | Daily, seven kept, under the app data folder. ADR-0071 |
| Live config | **Built, alpha.2** | Config edits and API changes reach the running daemon with no restart; `http_port`, `database_path` and `media_storage_dir` still need one and are reported. A bad edit of `config.toml` keeps the old config and reports `reload_error`. ADR-0072. The folder watcher is still never started automatically |
| Onboarding, Settings, folder drop | **Built, alpha.2** | Welcome screen on a fresh config, the Settings dialog, and dropping a folder on the Projects view to add it (the page draws its own drop zone from Tauri's drag events). ADR-0073, ADR-0074. Checked in a browser against a stub daemon; the native dialog and real drags are untried. Cover-art drop is probably broken in the shell (ADR-0073 leaves the fix open) |
| Menus and daemon controls | **Built, alpha.2** | File, Edit (Find), View, Tools, Help, shortcuts, and Start, Stop and Restart daemon (`POST /api/v1/system/shutdown`). ADR-0075, ADR-0076. Checked in a browser; Explorer reveal, Alt+F4 and Ctrl+F inside the shell are untried |
| Version control for projects | **Not started** | Aspiration only |
| macOS / Linux support | **Out of scope for now** | Paths exist, untested. Windows-first |
| Licensing | **Done** | AGPL-3.0-or-later, plugin scanner crates MPL-2.0, one section 7 exception for the Steamworks SDK. ADR-0054. Not done: a trademark policy for the name. Undecided: whether Steam builds use Steamworks at all |

## Next

ADR-0024's HTTP router landed in full (2026-09-16): skeleton, then every domain in the
`Services` aggregator plus `system` (scan status, project add, watcher control, the two
SSE streaming endpoints, statistics with CSV export). Each domain has its own
hand-written DTOs in `src/http/dto/`, independent of the generated proto types, per the
ADR.

ADR-0046 (2026-09-24) decides what ADR-0028 left tentative: the gRPC server and the
CLI, interactive mode included, are removed, and HTTP is the only surface. The gRPC half
is done (2026-10-08): the behaviour `tests/grpc/` checked now lives in `tests/services/`
and `tests/http/`, `SystemService` has native scan, watcher and statistics types instead
of the generated ones, and the server, its protos and its dependencies are gone. Still to
do from the ADR: the CLI subcommands and interactive mode (after the new client, below),
and folding `SystemService` into `Services`. The frontend is a Tauri-only thin shell over
that surface (ADR-0048).

ADR-0050 (2026-10-03) reverses the CLI half of that. The shipped application is three
parts: the tray daemon, the frontend launched from the tray or a shortcut, and a CLI
client that talks to the daemon over HTTP, for scripts, people who prefer commands, and
AI agents. Every client reaches the data through the daemon. The current in-process CLI
subcommands and interactive mode are still removed, but only after the new client has
landed, so they can be ported from.

The plugin migration is complete. Phase 1 (worker + supervisor) landed in `b261200`;
phase 2 followed in three steps — schema and persistence, the first-run scan, and
retiring the Ableton database.

Plugin metadata now comes entirely from Seula's own scanner. `seula plugin refresh`
scans the system and persists into `plugins` / `plugin_classes` / `plugin_buses`, sweeps
what it no longer finds, and merges phantom rows into the bundle that owns their class.
`seula scan` runs that scan automatically when one has never completed. Parsing a
project records references in `plugin_refs` and resolves them by `(plugin_kind, uid)`,
falling back to `plugin_classes.class_id`.

Nothing in the plugin path reads Ableton's database any more, and `live_database_dir` is
gone from the configuration.

Worth picking up next, in no particular order:

- **A config flag to decline the first-run scan.** Today the only way out is pointing
  `vst_search_paths` at an empty directory (ADR-0013).
- **`plugin_paths`**, if duplicate install locations ever need surfacing (ADR-0010).

A full plugin scan is **2m19s for 276 candidates** (debug build, 2026-09-15). The cost
is plugin load time, not ours, so a release build will not change it much — which is why
it runs once on first use and thereafter only on demand (ADR-0013).

**Schema versioning:** `SCHEMA_VERSION` (now 3) is bumped only for changes to *existing*
tables. Adding a table needs no bump, because `initialize()` creates missing ones on
every open — and a bump discards the user's database (ADR-0011).

## Known issues

Seven tests are `#[ignore]`d as of 2026-10-06 — three scan the real configured project
folders, three are hardcoded to paths on the maintainer's machine, and
`test_config_validation` needs a real `config.toml` with project paths. The last two
were found by the first CI run on a clean runner.
`cargo test --workspace` no longer runs them; use
`cargo test --workspace --tests -- --ignored` for the heavy pass. See CLAUDE.md.

Accepted limitations and noise. Bugs, things that should be fixed, are in `docs/bugs.md`
(ADR-0065).

| Issue | Where | Severity |
|---|---|---|
| `4-DUNE.als` (found 2026-10-04 in a test folder) starts `AB 1E 56 78`, not a gzip header, so the parser cannot read it. Live 12 opens it fine (maintainer), and it holds Live class names (`LiveDocument`, `ClipSlot`, `RemoteableSlot`) and UTF-16 strings, so it is a real Live set in a binary container, not a corrupt file. **Probably a pre-8.2 set.** The abletoolz source (`abletoolz/versioning.py`) states that Live sets before 8.2 use a different container with the `0xab1e` magic, and rejects them. Supporting evidence, all inference: the file embeds "Programmed By Rob Lee, www.basslinerecords.com" (looks like a downloaded template) and loads the `DUNE` plugin from `C:\Program Files\VstPlugins`. The maintainer now thinks it came from a DUNE preset pack on a sample CD, not from their own work (the file name looked like one of theirs, which is why they first assumed it was). That is a recollection too. Their first guess, a Live 11.0 save from February 2022 (they have only used Live 9.7.1 to 12.2), does not fit: Live 10 and 12 saves are gzipped XML, and the date is a recollection, not something in the file. The scan now reports `FileError::UnsupportedVersion` ("sets from before Live 8.2 are not supported") for that header, not "invalid gzip header", and skips the file. **Needs a closer look later:** neither dawtool nor abletoolz reads this format. abletoolz's `.asd` reader parses the same container (`AB 1E 56 78`, version byte, class-schema table), and on this file it reads the header and all 89 class definitions, then fails about 11% into the data. Version 3 differs from the `.asd` fixtures in two known ways: strings are `00 <kind> <len> <bytes>` and list elements carry an extra u32. Whether a reader is worth writing for one file is open; to use the project now, re-save it from Live 12. | `src/utils.rs`, `tests/utils/decompress.rs` | Low |
| ~16 iZotope helper DLLs in the VST3 folder are scanned and correctly classified `invalid_format`. Noise, not a bug. Filtering by heuristic risks dropping real VST2 plugins. | `src/scan/plugins/discovery.rs` | Cosmetic |
| Two `vst` crate deprecation warnings | `crates/vst-meta/src/scan.rs` | Upstream |

Resolved, before `docs/bugs.md` existed (2026-10-07); new bugs are logged and ticked there:

- **The sample list took 8 to 14 seconds on a real library** (found 2026-10-07, fixed
  2026-10-07). `GET /api/v1/samples?limit=10000` took 14.0 s in debug and 8.1 s in
  release on the maintainer's 18,243 samples. The list query took 47 ms; the per-page
  project counts took the rest, because SQLite started from every active project and
  probed each against all 500 ids of a chunk. A `CROSS JOIN` makes it start from the ids:
  0.40 s in debug, 0.19 s in release. ADR-0064. Regression test:
  `counts_start_from_the_ids_not_from_every_project` (`src/database/project_counts.rs`).
- **Search, samples and every other list stopped at a fixed row count** (found 2026-10-07,
  fixed 2026-10-07). The frontend asked for `limit=200` on search, which the projects
  board's mockup snapshot had used, and `limit=10000` everywhere else, and showed what came
  back as the whole list. On the maintainer's library a plugin's "show the projects using
  it" stopped at 200 projects (FabFilter Pro-Q 3 is in 1,184), and the samples view at
  10,000 of 18,243. Leaving the limit out does not help: most list queries default to 1000
  rows (`limit.unwrap_or(1000)` in `src/database/`), while projects and search return
  everything. The client now reads each list in pages until it has the route's
  `total_count`, and throws if they do not match. Regression test: `web/shared/api.test.ts`
  (`npm test` in `web/solid`, also run by CI).
- **A rescan of an edited project deleted everything attached to it** (found 2026-09-24,
  fixed 2026-09-24). Every parse mints a new project id, and the batch insert wrote it
  with `INSERT OR REPLACE`. `path` is unique, so the old row was deleted, and the cascade
  took the project's tags, collections, tasks and audio files; the replace also wiped
  its notes, its primary audio file and its archived flag. A known path now keeps its
  id, and the upsert writes only what a parse knows about. This is step 1 of ADR-0052;
  renames, moves and Save As are still new projects until the later steps. Regression
  tests: `a_rescan_keeps_the_project_and_what_is_attached_to_it`
  (`tests/database/batch.rs`), `adding_a_known_project_again_returns_it_under_its_id`
  (`src/services/system.rs`).
- **A rescan reset a renamed project's name to its file name** (found 2026-10-07, fixed
  2026-10-07). The project upsert still wrote `name` from the parse, so a rename lasted
  only until the next scan. That contradicts ADR-0051: the file name is the default only
  until the user renames the project. A rescan now leaves `name` alone. Regression test:
  `a_rescan_keeps_a_renamed_project_name` (`tests/database/batch.rs`).
- **The project scan wrote through a second database connection** (found 2026-09-24,
  fixed 2026-09-24). In the daemon, `process_projects_with_progress` opened its own
  connection and held SQLite's write lock through the batch insert, so an adapter write
  arriving then waited out the five-second busy timeout and failed with "database is
  locked". The daemon's scan now goes through `process_projects_into`, on the shared
  handle, locking it only for each database step (ADR-0049). The CLI still opens its
  own. Regression test: `a_scan_writes_through_the_database_it_is_given` (`src/lib.rs`).
- **A late progress update could leave the scan looking like it was still running**
  (found 2026-09-24, fixed 2026-09-24). `start_scan` wrote each progress update's
  status from a newly spawned task, unordered against the final Completed or Error
  status, so one that ran late put the scan back to `parsing` and every later scan was
  refused until a restart. The scan now runs on a blocking thread, like the plugin scan,
  and writes every status in order. That also closes the older entry about the scan
  occupying a runtime worker. Regression test:
  `a_late_progress_update_cannot_overwrite_the_final_status` (`src/services/system.rs`).
- **A slow client lost scan updates, the final one included** (found 2026-09-24, fixed
  2026-09-24). The scan streams sent with `try_send` into a channel of 100, so a client
  that fell behind could miss the last update, which carries the result.
  `send_scan_update` (`src/services/system.rs`) still drops intermediate updates rather
  than slow the scan, but waits for room for the final one. The watcher streams keep
  `try_send`: they have no final update. Regression test:
  `a_slow_client_still_gets_the_final_update` (`src/http/handlers/samples.rs`).
- **The watcher event stream held a runtime worker** (found 2026-09-24, fixed
  2026-09-24). `start_watcher_event_stream` looped on a blocking `recv()` inside
  `tokio::spawn`; it now runs on a blocking thread. Regression test:
  `a_watcher_event_stream_leaves_the_runtime_free` (`src/services/system.rs`).
- **Adding projects parsed them on an async worker** (found 2026-09-24, fixed
  2026-09-24). `add_single_project` and `add_multiple_projects` now parse on a blocking
  thread. Regression test: `adding_projects_parses_them_off_the_runtime`
  (`src/services/system.rs`).
- **The architecture overview and one comment described code that had changed** (found
  2026-09-24, fixed 2026-09-24). `docs/architecture/overview.md` now shows the plugin
  join as built, without the Ableton database, and describes the service layer and the
  HTTP adapter, with the 12-protos-but-11-services mismatch stated rather than hidden.
  The "Parser is dropped here" comment in `process_projects_with_progress` now says
  what actually happens.
- **The CORS check refused Tauri 2's Windows origin and accepted look-alike hosts**
  (found 2026-09-24, fixed 2026-09-24). `build_router` in `src/http/server.rs` matched
  origins by prefix. It accepted `https://tauri.localhost` but not `http://tauri.localhost`,
  the default origin of a Tauri 2 webview on Windows, so every request from the planned
  shell (ADR-0048) would have been refused. The prefix match also let
  `http://localhost.example.com` through as local. The host is now matched exactly.
  Regression tests: `local_and_tauri_origins_are_allowed`,
  `hosts_that_only_start_like_a_local_one_are_refused` (`src/http/server.rs`).
- **The CSV export put the task completion rate at a hundred times its value** (found
  2026-09-24, fixed 2026-09-24). `get_task_statistics` returned a percentage and both
  CSV exports multiplied it by 100 again, so 45% exported as "4545.45%". The rate is
  now a fraction from 0 to 1, as the monthly trends' rates already were. Regression
  tests: `completion_rates_are_fractions` (`tests/database/library_stats.rs`),
  `csv_completion_rate_is_a_percentage_once` (`src/http/handlers/system.rs`).
- **The library statistics' key distribution was one string per key** (found
  2026-09-24, fixed 2026-09-24). `/system/statistics` sent `"DSharp Minor"`, the enum
  names joined, where ADR-0035 has every key carry `tonic`, `scale`, `sharp` and `flat`;
  projects with no key were a key named "Unknown". The HTTP route now sends the key
  shape, and `null` for no key. Regression test: `projects_with_no_key_have_no_key`.
- **The tempo histogram lumped every slow and every fast tempo into one bin** (found
  2026-09-24, fixed 2026-09-24). Bins were 10 BPM wide except the first, "under 90",
  labelled 80, and the last, "180 and up"; in the mock library the 80 bin held 62
  projects from 70 to 89.85 BPM. Empty bins were left out, so a chart closed the gap.
  Bins are now 10 BPM wide from the lowest tempo used to the highest, empty ones as
  zero. Regression test: `tempo_bins_are_equal_width_with_empty_bins_as_zero`.
- **"The last 12 months" was the last 12 months with a project in them** (found
  2026-09-24, fixed 2026-09-24). Projects per month and task completion trends skipped
  quiet months, so the window reached further back and the monthly average divided by
  busy months only. Both are now the 12 calendar months to date, empty ones as zero;
  projects per year and the 30-day activity are filled the same way. Regression tests:
  `projects_per_month_is_twelve_calendar_months_with_quiet_months_as_zero`,
  `projects_per_year_fills_the_years_between`, `recent_activity_has_every_day_oldest_first`.
- **The library statistics counted different things side by side** (found 2026-09-24,
  fixed 2026-09-24). Projects were active only; plugins and samples were every row in
  their tables, used or not; tasks included archived projects'; the collection figures
  counted archived projects; and plugins and samples per project were averaged over the
  projects that had any. Every figure now counts the projects in the scope and what
  they use (ADR-0045), and averages divide by every project in scope. Regression tests:
  `every_count_takes_the_scope_and_splits_into_its_states`,
  `averages_divide_by_everything_in_scope_including_zeros`.
- **The longest and most complex projects could never be archived ones** (found
  2026-09-24, fixed 2026-09-24). The statistics loaded them with `get_project_by_id`,
  which returns only active projects, and the HTTP adapter marked every one active
  because the proto `Project` had no flag. It now has `is_active`, and the statistics
  load projects of either status, which `scope=all` needs.
- **Sorting the collection list by project count failed** (found 2026-09-24, fixed
  2026-09-24). `list_collections` accepted `sort_by=project_count` and put it straight
  into `ORDER BY`, but the collections table has no such column, so SQLite answered "no
  such column: project_count". The sort is now a subquery, a `total_duration` sort was
  added beside it, and both count in the project scope (ADR-0043). Regression test:
  `the_list_sorts_by_project_count_and_length_in_either_scope`
  (`tests/database/collections.rs`).
- **Every project in a collection came back as active** (found 2026-09-24, fixed
  2026-09-24). `get_collection_projects` built each project with `is_active: true`
  instead of reading the column, so an archived project in a tracklist could not be
  marked. Regression test: `a_collections_projects_say_whether_they_are_archived`
  (`tests/database/collections.rs`).
- **A collection's tasks could not be matched to their projects** (found 2026-09-24,
  fixed 2026-09-24). `/collections/:id/tasks` put the project's name in `project_id`,
  copied from the gRPC handler, so two projects with the same name were
  indistinguishable and a task could not lead to its project. The HTTP route now sends
  the id in `project_id` and the name in `project_name`; gRPC is unchanged. Test:
  `collection_tasks_carry_the_project_id_and_name` (`tests/database/collections.rs`).
- **A quoted search operator value stopped at its first space** (found 2026-09-24, fixed
  2026-09-24). The parser split terms on spaces before looking at quotes, so
  `plugin:"Pro-Q 3"` searched for `"Pro-Q` and left `3"` as free text, and no multi-word
  plugin, sample or tag could be searched for by operator. Found adding `collection:`,
  whose values are mostly more than one word. Regression test:
  `quoted_operator_values_keep_their_spaces` (`tests/database/search.rs`).
- **Sample sizes were invented, and the presence refresh froze the app** (found
  2026-09-23, fixed 2026-09-23). The "estimated" sample size was a guessed size per
  extension, summed; and `refresh_sample_presence_status` checked every file while holding
  the shared database mutex. Sizes are now measured by a background check that lists each
  folder once, on several threads, and locks the database only to read the paths and write
  the result (ADR-0041).
- **The sample list failed on a project-count filter combined with any other filter**
  (found 2026-09-23, fixed 2026-09-23). The count query referred to the table alias `s`
  from outside the subquery that defined it, so SQLite answered "no such column:
  s.is_present". Rewritten along with ADR-0040. Test:
  `archived_projects_count_only_in_the_all_scope` (`tests/database/project_counts.rs`).
- **A plugin's or sample's project count disagreed with its used-in list** (found
  2026-09-23, fixed 2026-09-23). The count included archived projects; the list did not.
  Both now take a `scope`, active by default (ADR-0040).
- **A rescan could never mark a sample missing** (found 2026-09-23, fixed 2026-09-23). The
  batch insert preloaded every stored sample and merged a rescan's presence into it with
  OR, then wrote it back with `is_present = EXCLUDED.is_present OR samples.is_present`.
  Once a sample had been seen, only the manual presence refresh could mark it missing.
  Every batch also rewrote every stored sample, including ones it never saw. Dated to
  `6e4f8c9` (2025-01-22) with no recorded reason; the maintainer considers it a bug. Now a
  batch writes only the samples its projects reference, and its answer replaces the stored
  one (within a batch, a sample any project finds is present). Regression test:
  `a_rescan_that_finds_a_sample_gone_marks_it_missing` (`tests/database/batch.rs`).
- **A project's samples got a new random id on every read** (found 2026-09-23, fixed
  2026-09-23). The eight queries that load a project's samples (in
  `src/database/projects.rs` and `src/database/collections.rs`) built each `Sample` with
  `Uuid::new_v4()` instead of reading the stored id, so the same sample had a different id
  in every response and could not be matched to `/api/v1/samples` or to itself in another
  project. Found when the mockup generator compared two responses that embed the same
  projects. Regression test: `a_projects_samples_keep_their_stored_ids_across_reads`
  (`tests/database/sample_ids.rs`).
- **A plugin refresh over the API froze every other request for minutes** (found
  2026-09-23, fixed 2026-09-23). `refresh_plugin_installation_status` ran the whole system
  scan (2m19s on the maintainer's library) inside a database method, while the service
  held the database mutex that every service shares. The scan now runs without the
  database, which is locked only to write the result, and the plugins view gets a
  background scan with progress, `POST /api/v1/plugins/scan` (ADR-0038). At the same time,
  a second scan no longer starts while one runs; before, a second project scan reset the
  first one's status. Regression test: `no_scan_starts_while_one_is_running`
  (`tests/services/system.rs`).
- **Uploading a second audition audio lost the first** (found 2026-09-23, fixed
  2026-09-23). `store_audio_file` pointed `projects.audio_file_id` at the new upload,
  leaving the previous file referenced by nothing, so the next orphan cleanup deleted it.
  Uploads now join the project's audio list and become the primary only if there was none
  (ADR-0037). Tests: `tests/database/audio_files.rs`.
- **Every scale except Major and Minor was stored as `Empty`** (found 2026-09-23, fixed
  2026-09-23). `src/scan/parser.rs` mapped only those two of Ableton's scale names; the
  rest fell to `Scale::Empty` behind an "add other scale mappings as needed" comment. In
  the maintainer's files that was 1,048 key-detected clips, including 510 Mixolydian.
  Fixed by mapping through a full table of Ableton's names, with `Scale::Other` keeping
  any name the table lacks (ADR-0035). Regression tests: `models::key_name_tests`.
  **Projects parsed before the fix keep `Empty` until a forced rescan**, because the
  scanner re-parses only changed files.
- **`--config <path>` was accepted and ignored** (found 2026-09-23, fixed 2026-09-23).
  `Cli` declared the flag but nothing read it: `CONFIG` is a lazy static that loaded
  before `Cli::parse()` ran, from `SEULA_CONFIG` or the default locations. Fixed in
  `src/main.rs` by parsing first and exporting the flag as `SEULA_CONFIG` before `CONFIG`
  is touched. A missing path now exits with an error rather than falling through to
  another config, which the env var still does. Checked by running the mock data
  generator, which now passes `--config` and gets its own port.
- **`seula --server` was unreachable** (found 2026-09-23, fixed 2026-09-23). Server-only
  mode (2025-08-14) checked for `--server` by scanning raw argv, but a week later the clap
  CLI put `Cli::parse()` in front of that check, and clap rejects any flag it does not
  declare. So the mode could not be started for over a year. Found when the mockup data
  generator needed a headless server. Fixed by declaring `--server`/`-s` on `Cli` and
  branching on the parsed flag. Regression test: `server_flag_parses` (`src/cli/mod.rs`).

- **`plugin_count` inflation in the `plugin vendors` / `plugin formats` aggregates**
  (found 2026-09-16, fixed 2026-09-16). The `usage_stats` subquery has one row per
  `(plugin_id, project_id)` pair; the outer `LEFT JOIN` fanned each plugin row out once
  per project it's used in, so `COUNT(*)` and the installed/missing/unknown
  `SUM(CASE ...)`s counted that plugin once per project instead of once.
  `total_usage_count` and `unique_projects_using` were unaffected — they already
  summed/`COUNT(DISTINCT)`ed correctly over the fanned rows. Fixed by switching
  `plugin_count` and the three status counts to `COUNT(DISTINCT p.id)` /
  `COUNT(DISTINCT CASE WHEN ... THEN p.id END)`, which collapse back to one count per
  plugin regardless of the fan-out. Regression test:
  `vendor_and_format_plugin_count_does_not_inflate_with_project_usage`
  (`tests/database/plugin_scan.rs`) — a plugin used in three projects must still count
  once. `src/database/plugins.rs` (`vendor_stats`/`format_stats` CTEs). ADR-0026.

## Documentation triage

Resolved:

- **`.kiro/`** — moved into `docs/archive/.kiro/` (2026-09-16). Superseded by
  `crates/vst-meta` and `CLAUDE.md`/`architecture/`; kept for history rather than deleted,
  same treatment as the rest of `docs/archive/`.

Undecided, needs a call from the maintainer:

- **`FRONTEND_SPEC.md`, `REQUIRED_FEATURES.md`, `TUI_ARCHITECTURE_ANALYSIS.md`,
  `TUI_PROJECT_PLAN.md`** — root-level planning docs. No corresponding code found for the
  TUI ones; `src/cli/interactive.rs` is a rustyline prompt, not a TUI. Aspirational,
  abandoned, or active?
- **`docs/archive/`** — two AI chat transcripts and an empty file, archived rather than
  deleted. `plugin_scanner_plan.md` proposes a C++ binary and is actively misleading. Git
  history preserves them; deleting is safe.

## Decisions not yet recorded

Candidates for ADR backfill, from the archaeology pass. These have no written rationale
anywhere — only the maintainer's memory, which is the one source that decays:

- SQLite + FTS5 over alternatives
- gRPC as the API surface (vs. REST/local IPC)
- Tray daemon + CLI dual mode
- 5NF schema normalisation
- `quick-xml` over `xml`/`elementtree` (all three are still in `Cargo.toml`)
- Gzip handling and the `flate2`/`zune-inflate` split
- Per-entity database module split
- Interactive mode via `rustyline`

Backfill opportunistically — when you next work in one of these areas, write the ADR
then. A dedicated archaeology sprint is what produced `docs/archive/`.
