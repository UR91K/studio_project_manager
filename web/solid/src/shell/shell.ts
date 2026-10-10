// The state that belongs to the window, not to a view: which view is showing, the chrome
// flags, the one open popover or menu, notices, and the running scan. Split by the speed
// it changes at (ADR-0047):
//
//   click-speed  `shell`, a store.
//   push-speed   `scan`, its own signal, read only by the status bar's scan segment.

import { createMemo, createSignal } from "solid-js";
import { createStore } from "solid-js/store";
import { Api, DEFAULT_URL, simulatedScan, type PluginScanMode } from "../../../shared/api";
import { revealInExplorer } from "../../../shared/os";
import { PAGE_SIZE } from "../../../shared/projects";
import type { Collection, KeySpelling, ScanKind, ScanProgress, StatsScope, SystemInfo } from "../../../shared/types";

export const api = new Api(import.meta.env.VITE_SEULA_URL ?? DEFAULT_URL);

// ---------------------------------------------------------------- routing (ADR-0055)

export type RouteId = "projects" | "collections" | "plugins" | "samples" | "stats";
/** The routes that list things, and so carry a count in the sidebar. */
export type CountedRoute = Exclude<RouteId, "stats">;
export const [route, setRoute] = createSignal<RouteId>("projects");

/** The one way a view hands off to the projects view: switch to it and search. The projects
 *  state registers its search here, so no view imports another's state. */
let searchProjects: (query: string) => void = () => {};
export const onProjectsSearch = (fn: (query: string) => void) => { searchProjects = fn; };
export function showProjectsMatching(query: string) {
  setShell({ menu: null, popover: null });
  setRoute("projects");
  searchProjects(query);
}

// ---------------------------------------------------------------- click-speed state

export const [shell, setShell] = createStore({
  sidebarCollapsed: false,
  inspectorOpen: true,
  /** Help > About is open. A dialog, so Escape and the scrim close it through Dialog itself. */
  about: false,
  spelling: "sharp" as KeySpelling,
  /** The one open picker, by name. A view owns the names it uses. */
  popover: null as string | null,
  /** The open row context menu; `id` is the row's, whichever view it belongs to. */
  menu: null as null | { id: string; x: number; y: number },
});

/** Which projects the library-wide figures count (ADR-0045). The stats view is its first
 *  reader; it is held here so the other views can share it. In memory only, until the
 *  preferences endpoint exists (ADR-0062). */
export const [projectScope, setProjectScope] = createSignal<StatsScope>("active");

/** Rows per page, one choice for every list view. In memory only, until the preferences
 *  endpoint exists (ADR-0062). */
export const PAGE_SIZES = [50, 100, 200, 500];
export const [pageSize, setPageSize] = createSignal(PAGE_SIZE);

export const closePopups = () => setShell({ menu: null, popover: null });

/** Every collection's id and name, for the project inspector's "Collections" lists. The
 *  collections view keeps this current after its own edits. */
export const [collectionNames, setCollectionNames] = createSignal<Collection[]>([]);
export const collectionById = createMemo(() => new Map(collectionNames().map((c) => [c.id, c])));
export async function loadCollectionNames() {
  try { setCollectionNames(await api.collections()); } catch { /* the inspector shows none */ }
}

export const [notice, setNotice] = createSignal<string | null>(null);
export const [system, setSystem] = createSignal<SystemInfo | null>(null);

/** Sidebar counts. A view keeps its own up to date once it has loaded its list. */
export const [counts, setCounts] = createStore<Record<CountedRoute, number | null>>({ projects: null, collections: null, plugins: null, samples: null });

export async function loadChrome() {
  const [s, p, c, q, r] = await Promise.allSettled([
    api.systemInfo(), api.count("projects"), api.count("collections"), api.count("plugins"), api.count("samples"),
  ]);
  if (s.status === "fulfilled") setSystem(s.value);
  if (p.status === "fulfilled") setCounts("projects", p.value);
  if (c.status === "fulfilled") setCounts("collections", c.value);
  if (q.status === "fulfilled") setCounts("plugins", q.value);
  if (r.status === "fulfilled") setCounts("samples", r.value);
}

// ---------------------------------------------------------------- push-speed state

export const [scan, setScan] = createSignal<ScanProgress | null>(null);
/** The last scan's final event, kept until the next scan starts. */
export const [lastScan, setLastScan] = createSignal<{ kind: ScanKind; event: ScanProgress } | null>(null);
let scanAbort: AbortController | null = null;

/** Follow a scan to its end, writing each event to `scan` and nothing else. `after` runs
 *  once the stream ends, to reload whatever the scan changed. */
export async function runScan(kind: ScanKind, after?: () => void | Promise<void>, mode?: PluginScanMode) {
  if (scanAbort) return;
  scanAbort = new AbortController();
  const { signal } = scanAbort;
  setLastScan(null);
  let last: ScanProgress | null = null;
  try {
    const events = kind === "simulated" ? simulatedScan(600, 40, signal) : api.scan(kind, signal, mode);
    for await (const ev of events) { last = ev; setScan(ev); }
    if (last) setLastScan({ kind, event: last });
    await after?.();
  } catch (e) {
    if (!signal.aborted) setNotice(e instanceof Error ? e.message : String(e));
  } finally {
    scanAbort = null;
    setTimeout(() => { if (!scanAbort) setScan(null); }, 1500);
  }
}

export async function showInExplorer(path: string) {
  try { await revealInExplorer(path); setNotice(null); }
  catch (e) { setNotice(e instanceof Error ? e.message : String(e)); }
}
