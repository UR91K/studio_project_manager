# 0073. The shell gains start-daemon, a folder picker, drop paths and zoom

- **Status:** Accepted, not yet implemented
- **Recognized:** 2026-10-11, from a think-aloud session with a tester
- **Decided:** 2026-10-11
- **Recorded:** 2026-10-11
- **Confidence:** decided now
- **Evidence:** ADR-0048 (the shell is thin: its IPC is for the OS calls only, and anything
  beyond them needs its own ADR); ADR-0069 (the shell starts the daemon at startup, and
  rejected a command the page could call to do it); ADR-0057 (the snap-layout overlay
  places a native window over the maximise button, in physical pixels); ADR-0061 (cover art
  has a drop target); `web/shared/os.ts`; `web/solid/src-tauri/src/main.rs`;
  `web/solid/src-tauri/tauri.conf.json`

## Context

The tester's session asked for four things that only the shell can do, or that need the
shell's cooperation:

- A "Start daemon" button on the "Cannot reach the daemon" screen. The shell starts the
  daemon at launch (ADR-0069), but not again if the daemon quits or crashes while the
  window is open, or the user chooses Quit from the tray.
- A folder browser for choosing project folders in onboarding and in Settings (ADR-0074).
- Dropping a folder on the Projects tab to add it.
- Zoom with Ctrl plus and Ctrl minus.

A page in a webview cannot do the first three on its own. It has no way to run a program,
a file input does not yield an absolute folder path, and a drop gives a page a file name
but not its location.

## Decision

**A command to start the daemon, `start_daemon`.** It takes no arguments and does what the
shell does at startup: ask `/health`, and if nothing answers start the daemon from the
shell's own folder in tray mode, then wait up to fifteen seconds for it. It is idempotent: a
daemon that is already answering means it returns at once. It returns success once the daemon
answers and an error with the reason otherwise. After it succeeds the page reloads its data.

This reverses one of ADR-0069's rejected alternatives. That record rejected a command the
page can call, because it would put an action that launches programs behind the page's own
code. The objection loses most of its force here: the command launches one fixed executable
with no arguments, so the page cannot choose what runs, and it is the same function the shell
already runs at startup. The case it covers cannot be covered otherwise.

**A command to pick folders, `pick_folders`.** It opens the native folder dialog, allows
several folders, and returns their absolute paths, or an empty list if the user cancels. It
is written in Rust over Tauri's dialog plugin. The plugin's own permissions are not granted
to the page: the page can call this one command and nothing else the plugin offers.

**Drags and drops arrive as Tauri's drag events.** The window keeps Tauri's file-drop handling
on (`dragDropEnabled`, which is the default). Tauri then reports a drag as enter, over, leave
and drop events, each with the cursor position, and the drop with absolute paths. The page
subscribes through `onFolderDrag` in `os.ts`, which wraps them. The shell adds no code for this
and does not interpret a drop. What a drag means is the page's decision (ADR-0074).

**The page shows its own drop zone.** While a drag is over the window and the Projects view (or
the welcome screen) is showing, the page raises a full-height drop zone with a plain message,
"Drop folders to add them". Dropping anywhere on it adds the folders. This is a page element
driven by the shell's events, not an HTML5 drop target: with Tauri's handling on, the webview
no longer delivers HTML5 drag events to the page, and a plain browser drop would not carry the
absolute path anyway (only a name, or a path relative to the dropped folder). Outside the shell
there are no drag events and no zone, and the text box or a picker is used.

**Zoom is the webview's own.** The window enables the webview's zoom hotkeys, so Ctrl plus,
Ctrl minus, Ctrl zero and Ctrl with the mouse wheel work with no page code. Whether the
chosen zoom is remembered between launches is not decided here: it depends on whether the
webview keeps it, which the work should find out. If it does not, the factor waits for the
preferences endpoint (ADR-0031, ADR-0062) and is not remembered until then.

**The shell still holds no state and still does not touch the API.** The two new commands act
on the OS, and the only thing the shell reads about the daemon is `/health`, as before.

## Rejected alternatives

- **A folder browser inside the app, backed by a daemon endpoint that lists directories.**
  It would add an endpoint on the loopback port that reveals any folder on the machine to any
  local process, which ADR-0048 and ADR-0024 steer away from. It would also reproduce, worse,
  what the native dialog already is.
- **Granting the page the dialog plugin directly.** It would let page code open any file
  dialog and read what it returns. The command keeps the surface to the one purpose.
- **A file input with `webkitdirectory`.** It returns a list of files with relative paths and
  no absolute path for the folder, which is what the daemon needs.
- **Zoom by scaling the page's own units.** It would redo what the webview does, and the
  layout maths that already corrects for zoom in the stats view would have to be repeated
  everywhere that measures the screen.
- **A watchdog that restarts the daemon when it dies.** That is autostart's cousin, a
  background behaviour the user has not asked for. It belongs with the tray menu (ADR-0050).

## Consequences

The shell's list of commands grows from two to four, and ADR-0048's thin-shell rule now has
a longer list of exceptions. Each is an OS action with no state and no knowledge of the API.

One new dependency, Tauri's dialog plugin.

**Zoom and the snap overlay.** The overlay is placed in physical pixels relative to the
client area (ADR-0057), and zoom changes the ratio between page pixels and physical pixels.
Whether it stays over the maximise button at 50 percent and 150 percent has to be tested, and
fixed if not.

**Drops and the cover art drop target.** With Tauri's drop handling on, the page's own file
drops stop firing on Windows, so ADR-0061's cover art drop target may already not work in the
shell. It has to be checked. If it is broken, the same events serve it: the page hit-tests the
cursor position against the cover art element, highlights it during the drag and takes the
file on drop. The path from a Tauri drop is read by the daemon or the page as a local file,
which needs a decision of its own if it comes to that, and is not made here.

The daemon must be stoppable and startable while the window is open without the page being
left in a bad state, so the error screen has to reload every view's data after
`start_daemon` succeeds.

## Notes

Each part is separable and cheap to reverse. The one with a standing cost is the
drag-drop handling, because of the cover art interaction above.
