# 0074. Onboarding and Settings edit the config through the daemon

- **Status:** Accepted
- **Recognized:** 2026-10-11, from a think-aloud session with a tester
- **Decided:** 2026-10-11
- **Recorded:** 2026-10-11
- **Confidence:** decided now
- **Evidence:** ADR-0072 (config changes reach the running daemon); ADR-0073 (the folder
  picker, drops and the start command); ADR-0059 (the one dialog component); ADR-0062 (UI
  state is kept in memory until the preferences endpoint exists); `src/config/defaults.rs`
  (the generated config starts with `paths = []`); `src/http/handlers/config.rs` (the config
  routes already exist); the tester's session notes

## Context

A fresh install writes a `config.toml` with no project folders and opens to an empty library.
The only way to fill it is to find that file, edit it by hand, and restart the daemon. The
tester hit exactly that wall, and asked for four things in this area: an onboarding screen
with a folder browser, a Settings screen for editing the config easily, dropping a folder on
the Projects tab, and menus (which need their own design and are not decided here).

The daemon already has routes that add and remove paths and update settings. They did not
take effect in a running daemon until ADR-0072. The folder picker and the drop event come
from ADR-0073.

## Decision

**The config file is edited only through the daemon.** The page uses the existing
`/api/v1/config` routes. It never writes the file, and the shell holds no config. The file
stays hand-editable, and ADR-0072's watcher means an edit made there shows up in the app.

**Onboarding is a full-window welcome shown when the config needs setup.** The page asks
`GET /api/v1/config/status` once the daemon is reachable, and when `needs_setup` is true it
shows a welcome in place of the view's content. It has one sentence about what Seula does and
one button, "Add project folders", which opens the folder picker (ADR-0073) and lists what was
chosen, each with a remove button. A "Scan" button adds the chosen folders through the config
routes and starts the project scan, whose progress the status bar already shows. The welcome
closes once at least one folder is configured. Any validation warning from the daemon is
shown beside the folder it concerns. In a browser, where there is no Tauri, the picker is
replaced by a text box for a path, so the screen can be built and checked against the mock
daemon.

**"Skip for now" lasts for the session.** It is held in memory (ADR-0062), so a user who
deliberately has no folders is asked again at the next launch. An app with no folders does
nothing, so being asked again is acceptable, and it avoids a preferences endpoint this record
does not add.

**Dropping a folder on the Projects tab adds it as a project folder.** It is the same action
as the picker: the folder is appended to the config's `paths` and a scan starts. It works
only while the Projects view is showing, or while the welcome is. A drop of something that
is not a folder is refused with a notice that only folders can be dropped. Dropping a single
project file is not part of this.

**Settings is one dialog from the gear in the top bar,** built on the shared dialog
(ADR-0059), with Save and Cancel. Its sections:

- **Project folders:** the list, with add (picker) and remove.
- **Plugin search folders:** the list. When it is empty the dialog says that the default
  Windows locations are used and names them.
- **Plugin scanning:** the per-plugin time limit.
- **File size limits:** cover art and audio, in megabytes.
- **Logging:** the level.
- **Locations:** the database path and the media folder. The HTTP port is shown as text,
  because it is not changed through the API.

Every field that ADR-0072 lists as needing a restart carries the label "Applies after
restart", and after a save the dialog says which keys are waiting, from the status route's
`restart_required`. If the config route reports a `reload_error`, the dialog shows it at the
top, so a bad hand edit is not silent.

**Nothing here guesses at the user's folders.** Seula does not look for Ableton's usual
folders and offer them. Reading a user's disk to suggest paths is a step they have not asked
for.

## Rejected alternatives

- **A text editor for `config.toml` inside the app.** One misplaced quote and the daemon
  stops accepting the file. The request was for easy editing, and a form is that.
- **A multi-step wizard.** The only thing a new user has to do is choose folders. Everything
  else has a working default.
- **Remembering "onboarding done" in the database.** The real trigger is that the config has
  no folders, and a flag could disagree with it. The preferences endpoint does not exist
  (ADR-0062).
- **Applying each Settings field the moment it changes.** Several are numbers a user is still
  typing, and a change that fails validation midway is worse than one that is held until Save.
- **Having the shell write the config.** The shell holds no state (ADR-0048), and the daemon
  already owns the file and its validation.
- **Suggesting Ableton's default folders.** See the last paragraph of the decision.

## Consequences

A fresh install no longer needs a text editor. A user can reach a populated library from the
first window.

The daemon's status and config routes grow two fields, `reload_error` and `restart_required`
(ADR-0072). That is the only change to the HTTP surface.

A user who edits the file by hand and makes a mistake sees the change ignored. The Settings
dialog shows why, and until they open it, the log has it.

Everything the Settings dialog does is also what a File or Tools menu item would do, which is
why the menus can come after it and link to it. Their contents are not decided here.

## Notes

The drop behaviour is the part most likely to be revisited, if testers expect dropping a
single project file to work too. The daemon already has an add-one-project route for that.
