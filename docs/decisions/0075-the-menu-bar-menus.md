# 0075. The menu bar's five menus

- **Status:** Accepted, not yet implemented
- **Recognized:** 2026-10-11, from a think-aloud session with a tester
- **Decided:** 2026-10-11
- **Recorded:** 2026-10-11
- **Confidence:** decided now
- **Evidence:** ADR-0032 (the shell and its density); ADR-0047 (state split by the speed it
  changes at); ADR-0057 (the title bar is the page's own); ADR-0059 (the one dialog);
  ADR-0073 (folder picker, drops, zoom); ADR-0074 (Settings and onboarding); ADR-0076 (daemon
  controls and the paths the daemon reports); `web/shared/os.ts`

## Context

The title bar has had the words File, Edit, View, Tools and Help since the first mockup, and
only Help does anything. A tester tried each of them. Nothing says what they should hold, so
this record lists it, from what the app can already do. Each item below is an action that
works today somewhere else, or one that ADR-0073, 0074 and 0076 add.

## Decision

Menus open on a click. While one is open, moving over another title switches to it. Escape
or a click outside closes it, and arrow keys, Home, End and Enter work inside it. Only one
is open at a time, using the existing one-popover state (ADR-0047). Shortcuts are shown at
the right of each item and are handled in one place, so they work with every menu closed.
An item that needs the daemon is disabled while the daemon is unreachable; an item that makes
sense only in one view is disabled in the others, not hidden.

**File**

- Add project folder... (the native folder dialog, then the same add as a drop)
- Settings... `Ctrl+,`
- Open config file (shows `config.toml` in Explorer)
- Open logs folder
- Close window `Alt+F4` (the daemon keeps running in the tray)

**Edit**

- Find `Ctrl+F` (focuses the search box)

**View**

- Projects, Collections, Plugins, Samples, Stats: `Ctrl+1` to `Ctrl+5`, a mark on the current one
- Sidebar `Ctrl+B`
- Inspector `Ctrl+Alt+B`
- Key names, Sharps or Flats, a mark on the current one

Zoom is not in the menu. Ctrl plus, minus and zero and Ctrl with the wheel belong to the
webview (ADR-0073) and need no item.

**Tools**

- Scan projects
- Scan plugins: new and changed only
- Scan plugins: all
- Check samples
- Start folder watcher, or Stop folder watcher (one item, labelled by the state the status
  bar already shows)
- Export statistics as CSV (the stats view's current scope; disabled elsewhere)
- Start daemon, Stop daemon, Restart daemon (ADR-0076): Start is enabled only while the
  daemon is unreachable, Stop and Restart only while it answers

**Help**

- About Seula

The scan items are disabled while a scan runs, as the toolbar buttons are.

**Where the code lives.** A new file `menus.tsx` holds the menu data and the shortcut handler;
`Frame.tsx` renders the bar and holds no menu logic. Actions call what the toolbars already
call (`runScan`, the view setters, the config calls from ADR-0074), so a menu item and its
button cannot drift apart.

**Open config file and Open logs folder** show the paths the daemon reports (ADR-0076) with
the shell's existing `reveal_in_explorer` call.

## Rejected alternatives

- **Dropping Edit.** It holds one item. Kept anyway, by the maintainer: it is where people
  look for Find, and cut, copy and paste already work in text boxes without a menu.
- **Zoom items in View.** They would call something the webview does by itself.
- **Ctrl+R and F5 for scanning.** The webview treats them as reload.
- **A "Quit Seula" that also stops the daemon.** The tray has Quit, and Tools has Stop daemon.
- **An Open source link in Help.** It needs a shell call to open a URL. About already shows the
  address with a Copy button.

## Consequences

Seven keyboard shortcuts become part of the app: `Ctrl+,`, `Ctrl+F`, `Ctrl+1` to `Ctrl+5`,
`Ctrl+B` and `Ctrl+Alt+B`. `Ctrl+F` has to stop the webview's own find from opening, and
that needs checking inside the shell. A shortcut that collides with a text box (typing in
the search box) must not fire, except `Ctrl+F` itself.

Disabling items by daemon state means the menus read the same reachability signal as the
error screen (ADR-0074), and it must change without a reload.

## Notes

The list is a first pass from what exists. Adding an item later is cheap; removing one a
person has learnt is not, so the list is deliberately short.
