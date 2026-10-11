// The first-run flow's shared state (ADR-0073, ADR-0074): whether the daemon answers, whether
// the config still needs folders, and the two actions the welcome screen and the drop zone share,
// which are adding folders through the daemon and starting the project scan.

import { createMemo, createSignal } from "solid-js";
import { load as loadCollections } from "../collections/state";
import { load as loadPlugins } from "../plugins/state";
import { load as loadProjects } from "../projects/state";
import { load as loadSamples } from "../samples/state";
import { reloadStatistics } from "../stats/state";
import { startDaemon } from "../../../shared/os";
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

// ---------------------------------------------------------------- daemon controls (ADR-0076)

/** A deliberate stop-then-start is under way, so the unreachable-daemon screen stays quiet. */
export const [restarting, setRestarting] = createSignal(false);

const RESTART_WAIT_MS = 10_000;

/** Waits, bounded, until the daemon stops answering. Resolves whether it did. */
async function waitUntilStopped(): Promise<boolean> {
  const until = Date.now() + RESTART_WAIT_MS;
  while (Date.now() < until) {
    try { await api.configStatus(); } catch { return true; }
    await new Promise((r) => setTimeout(r, 250));
  }
  return false;
}

/** Runs a daemon control and shows a failure as a notice, in the daemon's own words. */
async function control(act: () => Promise<void>) {
  try { await act(); setNotice(null); }
  catch (e) { setNotice(reason(e)); await probe(); }
}

async function startSteps() { await startDaemon(); await probe(); }

/** Asks the daemon to stop. The error screen appears once it has gone, as it should. */
async function stopSteps() {
  await api.shutdownDaemon();
  await waitUntilStopped();
  await probe();
}

async function restartSteps() {
  setRestarting(true);
  try {
    await api.shutdownDaemon();
    if (!(await waitUntilStopped())) throw new Error("The daemon did not stop.");
    await startDaemon();
    await probe();
    await reloadAll();
  } finally { setRestarting(false); }
}

export const startDaemonNow = () => control(startSteps);
export const stopDaemonNow = () => control(stopSteps);
export const restartDaemonNow = () => control(restartSteps);

export const dropAllowed = createMemo(() => daemon() === "up" && !shell.about && !shell.settingsOpen && (welcomeOpen() || route() === "projects"));
