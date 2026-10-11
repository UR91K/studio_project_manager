// The page's zoom state (ADR-0077): the factor from web/shared/zoom.ts, kept current and
// published as `--zoom` on the root element, with signals for the About dialog and the snap
// overlay. A change in zoom changes the pixel ratio and resizes the viewport; a change in
// display scale arrives as a scale event. All of them re-run the same update.

import { createSignal } from "solid-js";
import { computeZoom, zoomOverride } from "../../../shared/zoom";
import { getScaleFactor, inTauri, onWindowResized, onWindowScaleChanged } from "../../../shared/os";

const [zoom, setZoom] = createSignal(1);
const [displayScale, setDisplayScale] = createSignal(1);
/** The current zoom factor, and the OS display scale it was worked out against. */
export { zoom, displayScale };

/** Works out the factor now and on every change, and keeps `--zoom` current. Returns a stop function. */
export function trackZoom(): () => void {
  let stopped = false;
  let scale = 1;
  const update = () => {
    const override = zoomOverride();
    const z = override ?? (inTauri() ? computeZoom(window.devicePixelRatio, scale) : 1);
    document.documentElement.style.setProperty("--zoom", String(z));
    setDisplayScale(scale);
    setZoom(z);
  };
  const refreshScale = async () => { scale = await getScaleFactor(); if (!stopped) update(); };

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
