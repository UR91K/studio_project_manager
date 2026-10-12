// The webview's zoom factor (ADR-0077). The shell's title bar holds some sizes steady on the
// screen, which means dividing a screen size by the zoom, so the page works the zoom out and
// publishes it as `--zoom` on the root element.
//
// zoom = devicePixelRatio / the window's own scale factor. The pixel ratio carries both the
// operating system's display scaling and the webview's zoom; the scale factor is only the
// first, and the shell can report it. Outside the shell there is no zoom of this kind: 1.

/** The pure part: the zoom for a pixel ratio and a display scale. Anything unusable gives 1,
 * which is the old, unzoomed behaviour (ADR-0077, Notes). */
export function computeZoom(dpr: number, systemScale: number): number {
  const z = dpr / systemScale;
  return Number.isFinite(z) && z > 0 ? z : 1;
}

/** ADR-0078, VS Code's rule: below 100 percent the whole title bar is counter-zoomed, so its
 * menus, search box and controls keep their normal size; at 100 percent and above only the
 * controls hold their size and the rest grows with the page. */
export function counterZoomBar(zoom: number): boolean {
  return zoom < 1;
}

/** A test override, to exercise the factor in a browser: `?zoom=2` or `window.__SEULA_ZOOM_TEST__ = 2`. */
export function zoomOverride(): number | null {
  const w = window as unknown as { __SEULA_ZOOM_TEST__?: number };
  const q = Number(new URLSearchParams(window.location.search).get("zoom"));
  const v = w.__SEULA_ZOOM_TEST__ ?? (q > 0 ? q : undefined);
  return typeof v === "number" && Number.isFinite(v) && v > 0 ? v : null;
}

// Remembering the factor (ADR-0080): the page keeps it in its own localStorage and asks the
// webview to apply it at startup. Every access may throw, and none of it may break the page.

export const STORAGE_KEY = "seula.zoom";
export const MIN_ZOOM = 0.25;
export const MAX_ZOOM = 5;

type ZoomStorage = Pick<Storage, "getItem" | "setItem">;

const defaultStorage = (): ZoomStorage | null => {
  try {
    return window.localStorage;
  } catch {
    return null;
  }
};

/** The saved factor, or null if there is none or it is unusable (missing, not a number,
 * outside 0.25 to 5, or storage that throws). */
export function readStoredZoom(storage: ZoomStorage | null = defaultStorage()): number | null {
  try {
    const raw = storage?.getItem(STORAGE_KEY);
    if (raw == null || raw.trim() === "") return null;
    const v = Number(raw);
    return Number.isFinite(v) && v >= MIN_ZOOM && v <= MAX_ZOOM ? v : null;
  } catch {
    return null;
  }
}

/** Saves the factor, rounded to four decimals. Never throws; an unusable factor is not saved. */
export function writeStoredZoom(factor: number, storage: ZoomStorage | null = defaultStorage()): void {
  try {
    if (!Number.isFinite(factor) || factor < MIN_ZOOM || factor > MAX_ZOOM) return;
    storage?.setItem(STORAGE_KEY, String(Math.round(factor * 10000) / 10000));
  } catch {
    // Storage is full, blocked or gone: the zoom just is not remembered.
  }
}

/** Whether a restore is worth asking the webview for: a saved factor that is not 100 percent. */
export function needsRestore(stored: number | null): stored is number {
  return stored !== null && Math.abs(stored - 1) > 1e-4;
}
