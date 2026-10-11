// The first-run flow's shared state (ADR-0073, ADR-0074): whether the daemon answers, whether
// the config still needs folders, and the two actions the welcome screen and the drop zone share,
// which are adding folders through the daemon and starting the project scan.

import { createMemo, createSignal } from "solid-js";
import { load as loadCollections } from "../collections/state";
import { load as loadPlugins } from "../plugins/state";
import { load as loadProjects } from "../projects/state";
import { load as loadSamples } from "../samples/state";
import { reloadStatistics } from "../stats/state";
import { api, loadChrome, loadCollectionNames, route, runScan, setNotice, shell } from "./shell";

export type Daemon = "unknown" | "up" | "down";
export const [daemon, setDaemon] = createSignal<Daemon>("unknown");
export const [needsSetup, setNeedsSetup] = createSignal(false);

/** The welcome screen is showing: the daemon answers, the config has no folders, and the
 *  user has not said "Not now" this session. */
export const welcomeOpen = createMemo(() => daemon() === "up" && needsSetup() && !shell.skipOnboarding);

/** Every view's data, read afresh. Used when the daemon comes back and after a scan. */
export async function reloadAll() {
  await Promise.allSettled([
    loadChrome(), loadProjects(), loadCollections(), loadPlugins(), loadSamples(), loadCollectionNames(),
    reloadStatistics(),
  ]);
}

/** Asks the daemon how the config stands. When it was unreachable and now answers, every view is
 *  reloaded so nothing stays stale. Returns whether the daemon answered. */
export async function probe(): Promise<boolean> {
  const was = daemon();
  try {
    const status = await api.configStatus();
    setNeedsSetup(status.needs_setup);
    setDaemon("up");
    if (was === "down") void reloadAll();
    return true;
  } catch {
    setDaemon("down");
    return false;
  }
}

/** The daemon's error text without the request line around it. */
export function reason(e: unknown): string {
  const text = e instanceof Error ? e.message : String(e);
  const body = /^[A-Z]+ \S+: \d{3} ([\s\S]*)$/.exec(text)?.[1].trim();
  return body || text;
}

export interface AddResult { added: string[]; failed: { path: string; reason: string }[]; warnings: Record<string, string[]> }

/** Adds each folder through the config routes. A failure on one does not stop the others. */
export async function addFolders(paths: string[]): Promise<AddResult> {
  const out: AddResult = { added: [], failed: [], warnings: {} };
  for (const path of paths) {
    try {
      const warnings = await api.addProjectPath(path);
      out.added.push(path);
      if (warnings.length) out.warnings[path] = warnings;
    } catch (e) {
      out.failed.push({ path, reason: reason(e) });
    }
  }
  await probe();
  return out;
}

/** Starts the project scan, whose progress the status bar shows, and reloads when it ends. */
async function afterScan() { await probe(); await reloadAll(); }
export function scanProjects() {
  void runScan("projects", afterScan);
}

/** Tells the user which folders could not be added, naming each. */
export function reportFailures(failed: AddResult["failed"]) {
  if (failed.length) setNotice(failed.map((f) => `Could not add ${f.path}: ${f.reason}`).join(" "));
}

export const dropAllowed = createMemo(() => daemon() === "up" && !shell.about && !shell.settingsOpen && (welcomeOpen() || route() === "projects"));
