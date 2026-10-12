# 0080. The zoom level is remembered in the page's local storage, for now

- **Status:** Accepted
- **Recognized:** 2026-10-11, checking zoom in the installed build (the bug `zoom-not-remembered`)
- **Decided:** 2026-10-12
- **Recorded:** 2026-10-12
- **Confidence:** decided now
- **Evidence:** ADR-0073 (zoom is the webview's own, and left remembering it open, waiting for
  the preferences endpoint); ADR-0031 (preferences live in the `app_state` table); ADR-0062
  (UI state is kept in memory until that endpoint exists); ADR-0077 and ADR-0078 (the page
  already works out the zoom factor, as `--zoom`); `web/shared/os.ts`; the maintainer's
  check in the Windows Sandbox that the zoom resets on every launch

## Context

Ctrl plus, minus and zero zoom the window through the webview's own hotkeys (ADR-0073). The
webview does not keep the level between launches, so every start is at 100 percent. The
maintainer wants it remembered.

ADR-0073 said it would wait for the preferences endpoint (ADR-0031, ADR-0062), and that
endpoint does not exist. Waiting leaves zoom resetting for an unknown time, and zoom is a
setting of this window on this machine, which is not the kind of thing the app-state table
exists for: it would be wrong to sync or share it with another install.

## Decision

**The page remembers the zoom factor in its own `localStorage`**, under one key, and applies it
at startup.

- Whenever the zoom factor the page already computes (ADR-0077) changes, it saves the value.
- At startup, before it saves anything, it reads the saved value and asks the webview to apply it,
  through Tauri's webview zoom call. The shell's capability file grants that one permission
  and nothing wider.
- Ctrl plus, minus and zero stay the webview's own hotkeys. The page only stores and restores
  the factor, so nothing about how zoom is used changes.
- A missing, unreadable or out-of-range stored value is ignored and the zoom stays at 100
  percent. Reads and writes of `localStorage` can throw, and the page must work without it.
- Outside the shell there is no webview zoom to restore and nothing is stored.

**This is an interim home, not a preference.** When the preferences endpoint exists, the factor
moves there if it is wanted across reinstalls, and this record is superseded. It is stored
per window and per machine on purpose, and it goes when the app's webview data goes, which
includes uninstalling.

## Rejected alternatives

- **Waiting for the preferences endpoint.** Open-ended, for a setting that is not a shared
  preference anyway.
- **Storing it in the daemon's database or `config.toml`.** The factor belongs to the window,
  and the daemon may be on another machine in time. It would also make the shell depend on the
  daemon being up at the moment it opens.
- **Storing it in the shell (Rust).** ADR-0048 and ADR-0073 keep the shell stateless.
- **Replacing the hotkeys with page-handled zoom.** It would rebuild what the webview does and
  put the title bar's counter-zoom (ADR-0078) at risk for no gain.

## Consequences

This is the first use of `localStorage` in the frontend, so ADR-0062's "in memory until the
endpoint exists" is no longer the whole rule: it holds for layout and selection state, and
this one window-level setting is the exception.

The window may open at 100 percent and jump to the saved level a moment later, because the
restore happens after the page loads. If that shows, hiding the page until the zoom is applied
is the next step, and is not part of this record.

The restore has to run before the page saves anything, or the first save would overwrite the
stored value with 100 percent.

## Notes

If the factor read back from the webview does not match what was set, for instance after a
display scale change, the page saves what the webview reports, not what it asked for.
