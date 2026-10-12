// The page's zoom state (ADR-0077): the factor from web/shared/zoom.ts, kept current and
// published as `--zoom` on the root element, with signals for the About dialog and the snap
// overlay. A change in zoom changes the pixel ratio and resizes the viewport; a change in
// display scale arrives as a scale event. All of them re-run the same update.

import { createSignal } from "solid-js";
import { computeZoom, needsRestore, readStoredZoom, writeStoredZoom, zoomOverride } from "../../../shared/zoom";
import { getScaleFactor, inTauri, onWindowResized, onWindowScaleChanged, setWebviewZoom } from "../../../shared/os";

const [zoom, setZoom] = createSignal(1);
const [displayScale, setDisplayScale] = createSignal(1);
/** The current zoom factor, and the OS display scale it was worked out against. */
export { zoom, displayScale };

// Remembering the zoom (ADR-0080). The saved factor is applied once, in the shell only, and
// nothing is saved until that has been asked for and the display scale is known: before the
// scale arrives the factor is worked out against 1, which on a 125 percent display reads 1.25.
let restored: Promise<void> | null = null;
let awaitingRestore = false;

/** Applies the saved zoom factor to the webview, once. main.tsx calls this before the first
 * render so it goes out as early as the page can; trackZoom waits for it before saving. */
export function restoreZoom(): Promise<void> {
  restored ??= (async () => {
    if (!inTauri() || zoomOverride() !== null) return;
    const stored = readStoredZoom();
    if (!needsRestore(stored)) return;
    awaitingRestore = true; // until the webview reports something other than 100 percent
    await setWebviewZoom(stored);
  })();
  return restored;
}

/** Works out the factor now and on every change, and keeps `--zoom` current. Returns a stop function. */
export function trackZoom(): () => void {
  let stopped = false;
  let scale = 1;
  let ready = false; // restore asked for, and the display scale known
  let lastSaved: number | null = inTauri() ? readStoredZoom() : null;
  const save = (z: number) => {
    if (!ready || !inTauri() || zoomOverride() !== null) return;
    if (awaitingRestore) {
      if (!needsRestore(z)) return; // the webview has not reported the restored level yet
      awaitingRestore = false;
    }
    const rounded = Math.round(z * 10000) / 10000;
    if (rounded === lastSaved) return;
    lastSaved = rounded;
    writeStoredZoom(rounded);
  };
  const update = () => {
    const override = zoomOverride();
    const z = override ?? (inTauri() ? computeZoom(window.devicePixelRatio, scale) : 1);
    document.documentElement.style.setProperty("--zoom", String(z));
    setDisplayScale(scale);
    setZoom(z);
    save(z);
  };
  const refreshScale = async () => {
    scale = await getScaleFactor();
    if (stopped) return;
    if (!ready) await restoreZoom();
    ready = true;
    if (!stopped) update();
  };

  // The pixel ratio changes with zoom: a resolution query on the current value, re-armed
  // each time it fires, since it only matches that one value.
  let mq: MediaQueryList | null = null;
  const arm = () => {
    mq?.removeEventListener("change", onDpr);
    mq = window.matchMedia(`(resolution: ${window.devicePixelRatio}dppx)`);
    mq.addEventListener("change", onDpr);
  };
  const onDpr = () => { arm(); update(); };

  update();
  void refreshScale();
  arm();
  window.addEventListener("resize", update);
  const stops = [onWindowResized(update), onWindowScaleChanged(() => void refreshScale())];
  return () => {
    stopped = true;
    mq?.removeEventListener("change", onDpr);
    window.removeEventListener("resize", update);
    stops.forEach((s) => void s.then((off) => off()));
  };
}
