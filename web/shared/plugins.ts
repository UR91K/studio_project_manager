// The plugins view's logic with no framework in it: the status filter, the columns,
// filtering, sorting, grouping and the counts.

import type { FailedPluginFile, FormatRollup, PluginRow, Sort, VendorRollup } from "./types";

/** A plugin's status: the tri-state installed flag (ADR-0012) under its install_states names
 *  (ADR-0025), and failed: its file is there but did not load in the last scan (ADR-0067). */
export type InstallState = "installed" | "absent" | "failed" | "unscanned";

export const INSTALL_STATES: { id: InstallState; label: string }[] = [
  { id: "installed", label: "Installed" },
  { id: "absent", label: "Missing" },
  { id: "failed", label: "Failed" },
  { id: "unscanned", label: "Not scanned" },
];

/** A failure outranks the installed flag: a plugin that loaded once and fails now is still
 *  marked installed, because its file is there. */
export const stateOf = (p: Pick<PluginRow, "installed" | "scan_error">): InstallState =>
  p.scan_error ? "failed" : p.installed === true ? "installed" : p.installed === false ? "absent" : "unscanned";

export const stateLabel = (p: Pick<PluginRow, "installed" | "scan_error">) =>
  INSTALL_STATES.find((s) => s.id === stateOf(p))!.label;

/** How a plugin file failed, in words (vst_meta's ErrorType). */
export function failureLabel(code: string): string {
  const labels: Record<string, string> = {
    crashed: "Crashed the scanner", timeout: "Timed out", load_failed: "Would not load",
    result_lost: "Its scan result was lost; scan again",
    invalid_format: "Not a plugin the scanner recognises",
  };
  return labels[code] ?? code;
}

/** A row for a file that failed and never loaded. No plugin row stands for it, because a
 *  plugin's identity only comes from loading it, so it is named after its file. */
export function fileRow(f: FailedPluginFile): PluginRow {
  const base = f.path.split(/[\\/]/).pop() ?? f.path;
  const dot = base.lastIndexOf(".");
  return {
    id: `file:${f.path}`, dev_identifier: "", name: dot > 0 ? base.slice(0, dot) : base,
    format: base.toLowerCase().endsWith(".vst3") ? "VST3" : "VST2",
    installed: null, vendor: null, version: null, project_count: 0, scan_error: f.error_type, file: f,
  };
}

export type Group = "vendor" | "format";

export interface PluginFilters {
  vendor: string | null;
  format: string | null;
  states: InstallState[];
  group: Group | null;
  sort: Sort | null;
}

/** Sorts after any name, so plugins with no vendor come last. */
const SORTS_LAST = String.fromCharCode(0xffff);
const versionKey = (v: string | null) => (v ?? "").split(".").map((n) => n.padStart(4, "0")).join(".");

export interface PluginColumn {
  id: string;
  label: string;
  w: number;       // default width, in --u
  num?: boolean;
  dim?: boolean;
  sort(p: PluginRow): string | number;
}

export const PLUGIN_COLUMNS: PluginColumn[] = [
  { id: "name", label: "Name", w: 124, sort: (p) => p.name.toLowerCase() },
  { id: "status", label: "Status", w: 50, sort: (p) => INSTALL_STATES.findIndex((s) => s.id === stateOf(p)) },
  { id: "vendor", label: "Vendor", w: 90, dim: true, sort: (p) => (p.vendor ?? SORTS_LAST).toLowerCase() },
  { id: "format", label: "Format", w: 64, dim: true, sort: (p) => p.format },
  { id: "version", label: "Version", w: 36, dim: true, sort: (p) => versionKey(p.version) },
  { id: "projects", label: "Projects", w: 36, num: true, sort: (p) => p.project_count },
];

export const DEFAULT_PLUGIN_SORT: Sort = { col: "projects", desc: true };

/** A click on a header: the same column flips, a new one starts at its natural direction
 *  (most-used first). */
export const nextPluginSort = (current: Sort | null, col: string): Sort =>
  current && current.col === col ? { col, desc: !current.desc } : { col, desc: col === "projects" };

export const GROUPS: Record<Group, { label: string; key(p: PluginRow): string | null; name(k: string | null): string }> = {
  vendor: { label: "Vendor", key: (p) => p.vendor, name: (k) => k ?? "No vendor" },
  format: { label: "Format", key: (p) => p.format, name: (k) => k ?? "" },
};

const cmp = (x: string | number, y: string | number) => (x < y ? -1 : x > y ? 1 : 0);

/** The plugins the list shows: the toolbar filters, then the sort, grouped when grouping
 *  is on. `list` is the whole library, or a search's hits. */
export function pluginRows(list: PluginRow[], f: PluginFilters): PluginRow[] {
  let rows = list.filter((p) => (!f.vendor || p.vendor === f.vendor) && (!f.format || p.format === f.format)
    && (!f.states.length || f.states.includes(stateOf(p))));
  const col = PLUGIN_COLUMNS.find((c) => c.id === f.sort?.col) ?? PLUGIN_COLUMNS[0];
  const dir = f.sort?.desc ? -1 : 1;
  rows = [...rows].sort((a, b) => cmp(col.sort(a), col.sort(b)) * dir || cmp(a.name.toLowerCase(), b.name.toLowerCase()));
  if (f.group) {
    // Groups in name order, the no-vendor group last; the sort applies inside each.
    const g = GROUPS[f.group];
    const key = (p: PluginRow) => (g.key(p) ?? SORTS_LAST).toLowerCase();
    rows = [...rows].sort((a, b) => cmp(key(a), key(b)));
  }
  return rows;
}

/** An entry of the table body: a plugin row, or the header that opens its group. */
export type GroupHeader = { group: true; key: string | null; shown: number };
export type Entry = PluginRow | GroupHeader;
export const isHeader = (e: Entry): e is GroupHeader => "group" in e;

/** One page of rows laid out for the table, with group headers where the group changes
 *  and a folded group's rows left out. `rows` is the whole filtered list: a header's
 *  "shown" count is the group's size in it, not on this page. */
export function layout(rows: PluginRow[], page: PluginRow[], group: Group | null, collapsed: Record<string, boolean>): Entry[] {
  if (!group) return page;
  const g = GROUPS[group];
  const out: Entry[] = [];
  let current: string | null | undefined;
  for (const p of page) {
    const key = g.key(p);
    if (key !== current || out.length === 0) {
      current = key;
      out.push({ group: true, key, shown: rows.filter((x) => g.key(x) === key).length });
    }
    if (!collapsed[String(key)]) out.push(p);
  }
  return out;
}

export interface PluginStats {
  total: number; installed: number; missing: number; failed: number; unscanned: number; vendors: number;
}

/** The status bar's counts for the rows the filters list. The stats route counts the same
 *  rows server-side; counting them here saves a request per keystroke. */
export function pluginStats(rows: PluginRow[]): PluginStats {
  const n = (s: InstallState) => rows.filter((p) => stateOf(p) === s).length;
  return {
    total: rows.length, installed: n("installed"), missing: n("absent"), failed: n("failed"), unscanned: n("unscanned"),
    vendors: new Set(rows.map((p) => p.vendor).filter(Boolean)).size,
  };
}

/** A group header's totals, from the vendor or format rollup when it has one. The vendor
 *  rollup leaves out plugins with no vendor (it groups WHERE vendor IS NOT NULL). */
export function rollupFor(group: Group, key: string | null, vendors: VendorRollup[], formats: FormatRollup[]) {
  return group === "vendor" ? vendors.find((v) => v.vendor === key) : formats.find((f) => f.format === key);
}
