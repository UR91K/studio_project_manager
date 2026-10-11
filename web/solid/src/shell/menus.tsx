// The menu bar's five menus (ADR-0075): their items, the dropdown, the keyboard, and the one
// global shortcut handler. Frame.tsx draws the titles with <MenuBar/>; App.tsx mounts
// <MenuDropdown/> against the window and calls installShortcuts(). Every action calls what a
// toolbar button already calls, so an item and its button cannot drift apart.
//
// The open menu lives in the shell's one popover slot as "menu:<id>" (ADR-0047), so opening
// any picker closes it and the window's click-away and Escape handling cover it.

import { For, Show, createEffect, createSignal, onCleanup, untrack } from "solid-js";
import { place } from "../../../shared/place";
import { closeWindow, inTauri, pickFolders } from "../../../shared/os";
import { checkSamples } from "../samples/state";
import { scanPlugins } from "../plugins/state";
import { exportCsv } from "../stats/state";
import {
  addFolders, daemon, reason, reportFailures, restartDaemonNow, restarting, scanProjects, startDaemonNow, stopDaemonNow,
  welcomeOpen,
} from "./flow";
import { Icon } from "./parts";
import {
  api, closePopups, loadChrome, route, scan, setNotice, setRoute, setShell, setSystem, shell, showInExplorer, system,
  type RouteId,
} from "./shell";
import "./menus.css";

interface Item {
  label: string | (() => string);
  /** Display and match text, such as "Ctrl+Alt+B". */
  keys?: string;
  /** A tick when true, a blank when false, nothing when absent. */
  mark?: () => boolean;
  off?: () => boolean;
  run: () => void | Promise<void>;
}
type Entry = Item | "sep" | { head: string };

const isItem = (e: Entry): e is Item => typeof e === "object" && "run" in e;
const labelOf = (i: Item) => (typeof i.label === "function" ? i.label() : i.label);

const up = () => daemon() === "up" && !restarting();
const scanning = () => !!scan();
const needsDaemon = () => !up();
const NO_PATH = "The daemon did not report where it is";

async function addProjectFolder() {
  if (!inTauri()) { setShell("settingsOpen", true); return; }  // no native dialog: Settings has a text box
  const paths = await pickFolders();
  if (!paths.length) return;
  setNotice(null);
  const { added, failed } = await addFolders(paths);
  reportFailures(failed);
  if (added.length) scanProjects();
}

function reveal(path: string | null | undefined) {
  if (!path) { setNotice(NO_PATH); return; }
  void showInExplorer(path);
}

async function toggleWatcher() {
  try {
    if (system()?.watcher_active) await api.stopWatcher(); else await api.startWatcher();
    setNotice(null);
  } catch (e) { setNotice(reason(e)); }
  await loadChrome();
}

const goTo = (id: RouteId) => () => { closePopups(); setRoute(id); };
const ROUTES: { id: RouteId; label: string }[] = [
  { id: "projects", label: "Projects" }, { id: "collections", label: "Collections" }, { id: "plugins", label: "Plugins" },
  { id: "samples", label: "Samples" }, { id: "stats", label: "Stats" },
];

function focusSearch() {
  const box = document.querySelector<HTMLInputElement>(".topbar .search input");
  box?.focus();
  box?.select();
}

const MENUS: { id: string; title: string; entries: Entry[] }[] = [
  { id: "file", title: "File", entries: [
    { label: "Add project folder...", off: needsDaemon, run: addProjectFolder },
    "sep",
    { label: "Settings...", keys: "Ctrl+,", off: needsDaemon, run: () => { setShell("settingsOpen", true); } },
    { label: "Open config file", off: needsDaemon, run: () => reveal(system()?.config_path) },
    { label: "Open logs folder", off: needsDaemon, run: () => reveal(system()?.log_dir) },
    "sep",
    // The daemon keeps running in the tray. Alt+F4 is the OS's own, so it is shown and not handled.
    { label: "Close window", keys: "Alt+F4", run: closeWindow },
  ] },
  { id: "edit", title: "Edit", entries: [
    { label: "Find", keys: "Ctrl+F", run: focusSearch },
  ] },
  { id: "view", title: "View", entries: [
    ...ROUTES.map((r, i): Item => ({ label: r.label, keys: `Ctrl+${i + 1}`, mark: () => route() === r.id, run: goTo(r.id) })),
    "sep",
    { label: "Sidebar", keys: "Ctrl+B", mark: () => !shell.sidebarCollapsed, run: () => { setShell("sidebarCollapsed", (c) => !c); } },
    // The stats view has no inspector.
    { label: "Inspector", keys: "Ctrl+Alt+B", mark: () => shell.inspectorOpen && route() !== "stats", off: () => route() === "stats",
      run: () => { setShell("inspectorOpen", (o) => !o); } },
    "sep",
    { head: "Key names" },
    { label: "Sharps", mark: () => shell.spelling === "sharp", run: () => { setShell("spelling", "sharp"); } },
    { label: "Flats", mark: () => shell.spelling === "flat", run: () => { setShell("spelling", "flat"); } },
  ] },
  { id: "tools", title: "Tools", entries: [
    { label: "Scan projects", off: () => needsDaemon() || scanning(), run: scanProjects },
    { label: "Scan plugins: new and changed", off: () => needsDaemon() || scanning(), run: () => scanPlugins("changes") },
    { label: "Scan plugins: all", off: () => needsDaemon() || scanning(), run: () => scanPlugins("all") },
    { label: "Check samples", off: () => needsDaemon() || scanning(), run: checkSamples },
    "sep",
    { label: () => (system()?.watcher_active ? "Stop folder watcher" : "Start folder watcher"), off: needsDaemon, run: toggleWatcher },
    { label: "Export statistics as CSV", off: () => needsDaemon() || route() !== "stats", run: exportCsv },
    "sep",
    { label: "Start daemon", off: () => daemon() !== "down" || restarting(), run: startDaemonNow },
    { label: "Stop daemon", off: needsDaemon, run: stopDaemonNow },
    { label: "Restart daemon", off: needsDaemon, run: restartDaemonNow },
  ] },
  { id: "help", title: "Help", entries: [
    { label: "About Seula", run: () => { setShell("about", true); } },
  ] },
];

// ---------------------------------------------------------------- the open menu

const openId = () => (shell.popover?.startsWith("menu:") ? shell.popover.slice(5) : null);
const entriesOf = (id: string | null) => MENUS.find((m) => m.id === id)?.entries ?? [];
const [hot, setHot] = createSignal(-1);

const isOff = (e: Entry) => isItem(e) && !!e.off?.();
const selectable = (e: Entry) => isItem(e) && !isOff(e);

function openMenu(id: string) { setHot(-1); setShell({ menu: null, popover: `menu:${id}` }); }

async function activate(item: Item) {
  if (isOff(item)) return;
  closePopups();
  await item.run();
}

/** Moves the highlight to the next (or previous) enabled item, wrapping. */
function step(delta: 1 | -1) {
  const list = entriesOf(openId());
  let i = hot();
  for (let n = 0; n < list.length; n++) {
    i = (i + delta + list.length) % list.length;
    if (selectable(list[i])) { setHot(i); return; }
  }
}
function edge(last: boolean) {
  const list = entriesOf(openId());
  const order = list.map((_, i) => i);
  for (const i of last ? order.reverse() : order) if (selectable(list[i])) { setHot(i); return; }
}
function sideways(delta: 1 | -1) {
  const i = MENUS.findIndex((m) => m.id === openId());
  openMenu(MENUS[(i + delta + MENUS.length) % MENUS.length].id);
}

export function MenuBar() {
  return (
    <nav class="menubar" role="menubar">
      <For each={MENUS}>{(m) => (
        <span role="menuitem" aria-haspopup="menu" aria-expanded={openId() === m.id} data-keep="" data-menu={m.id}
          classList={{ open: openId() === m.id }}
          onClick={() => { if (openId() === m.id) closePopups(); else openMenu(m.id); }}
          onMouseEnter={() => { if (openId() && openId() !== m.id) openMenu(m.id); }}>{m.title}</span>
      )}</For>
    </nav>
  );
}

export function MenuDropdown() {
  let el: HTMLDivElement | undefined;
  // Whatever the daemon reports changes (the watcher, the paths), so read it fresh on opening.
  createEffect(() => {
    if (openId() && untrack(up)) void api.systemInfo().then(setSystem, () => {});
  });
  createEffect(() => {
    const id = openId();
    if (!id || !el) return;
    const anchor = document.querySelector(`.menubar [data-menu="${id}"]`);
    if (anchor) place(el.closest(".win") as HTMLElement, el, { anchor });
  });
  return (
    <Show when={openId()}>
      <div class="pop menu appmenu" role="menu" ref={el}>
        <For each={entriesOf(openId())}>{(e, i) => (
          <>
            <Show when={e === "sep"}><div class="sep" /></Show>
            <Show when={typeof e === "object" && "head" in e}><div class="head">{(e as { head: string }).head}</div></Show>
            <Show when={isItem(e)}>
              <div class="mi" role="menuitem" classList={{ off: isOff(e), hot: hot() === i() }} aria-disabled={isOff(e)}
                onMouseEnter={() => setHot(isOff(e) ? -1 : i())} onClick={() => void activate(e as Item)}>
                <Icon name="check" class={(e as Item).mark ? ((e as Item).mark!() ? "" : "blank") : "none"} />
                <span class="lbl">{labelOf(e as Item)}</span>
                <Show when={(e as Item).keys}><span class="kbd">{(e as Item).keys}</span></Show>
              </div>
            </Show>
          </>
        )}</For>
      </div>
    </Show>
  );
}

// ---------------------------------------------------------------- shortcuts

function matches(e: KeyboardEvent, keys: string): boolean {
  const parts = keys.split("+");
  const key = parts[parts.length - 1];
  const ctrl = parts.includes("Ctrl"), alt = parts.includes("Alt");
  if ((e.ctrlKey || e.metaKey) !== ctrl || e.altKey !== alt || e.shiftKey) return false;
  return e.key.toLowerCase() === key.toLowerCase() || e.code === `Key${key.toUpperCase()}`;
}

const typing = (t: EventTarget | null) =>
  t instanceof HTMLElement && (t.isContentEditable || /^(INPUT|TEXTAREA|SELECT)$/.test(t.tagName));

/** A dialog or the welcome screen is in front: the views behind it take no shortcuts. */
const blocked = () => shell.about || shell.settingsOpen || welcomeOpen();

const SHORTCUT_ITEMS = MENUS.flatMap((m) => m.entries.filter(isItem)).filter((i) => i.keys && i.keys !== "Alt+F4");

/** The one keydown handler. In the capture phase, so it runs before anything in the page and
 *  can stop the webview's own Ctrl+F. */
function onKeyDown(e: KeyboardEvent) {
  if (openId()) {
    const act: Record<string, () => void> = {
      ArrowDown: () => step(1), ArrowUp: () => step(-1), Home: () => edge(false), End: () => edge(true),
      ArrowRight: () => sideways(1), ArrowLeft: () => sideways(-1), Escape: closePopups,
      Enter: () => { const it = entriesOf(openId())[hot()]; if (it && isItem(it)) void activate(it); },
    };
    act[" "] = act.Enter;
    if (act[e.key] && !e.ctrlKey && !e.altKey) { e.preventDefault(); e.stopPropagation(); act[e.key](); return; }
  }
  if (!(e.ctrlKey || e.metaKey)) return;
  // Ctrl+F is the one shortcut that works while typing, and it always stops the webview's own find.
  if (matches(e, "Ctrl+F")) { e.preventDefault(); if (!blocked()) { closePopups(); focusSearch(); } return; }
  if (typing(e.target) || blocked()) return;
  const item = SHORTCUT_ITEMS.find((i) => matches(e, i.keys!));
  if (!item) return;
  e.preventDefault();
  void activate(item);
}

export function installShortcuts() {
  window.addEventListener("keydown", onKeyDown, true);
  onCleanup(() => window.removeEventListener("keydown", onKeyDown, true));
}
