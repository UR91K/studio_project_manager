// The OS calls, the only use of Tauri's IPC (ADR-0048). Everything else is HTTP.

type Tauri = { core: { invoke(cmd: string, args?: Record<string, unknown>): Promise<unknown> } };

export const inTauri = () => "__TAURI__" in window;

export async function revealInExplorer(path: string): Promise<void> {
  const tauri = (window as unknown as { __TAURI__?: Tauri }).__TAURI__;
  if (!tauri) throw new Error("Show in Explorer needs the Tauri shell");
  await tauri.core.invoke("reveal_in_explorer", { path });
}

// The window itself (ADR-0057): the title bar is ours, so the buttons call these. They use
// Tauri's window API through the global, which the capability file grants one call at a time.

type TauriWindow = {
  minimize(): Promise<void>;
  toggleMaximize(): Promise<void>;
  close(): Promise<void>;
  isMaximized(): Promise<boolean>;
  onResized(handler: () => void): Promise<() => void>;
};

const appWindow = (): TauriWindow | null =>
  (window as unknown as { __TAURI__?: { window: { getCurrentWindow(): TauriWindow } } }).__TAURI__
    ?.window.getCurrentWindow() ?? null;

export const minimizeWindow = () => void appWindow()?.minimize();
export const toggleMaximizeWindow = () => void appWindow()?.toggleMaximize();
export const closeWindow = () => void appWindow()?.close();
export const isWindowMaximized = async () => (await appWindow()?.isMaximized()) ?? false;

/** Calls `handler` whenever the window is resized, which includes being maximised or restored
 * by any means. Returns a function that stops listening. */
export async function onWindowResized(handler: () => void): Promise<() => void> {
  return (await appWindow()?.onResized(handler)) ?? (() => {});
}

// Snap layouts (ADR-0057): a native overlay sits over the maximise button, so the page tells
// the shell where the button is and hears about hover from the shell.

type TauriEvents = {
  core: { invoke(cmd: string, args?: Record<string, unknown>): Promise<unknown> };
  event: { listen(event: string, handler: (e: { payload: unknown }) => void): Promise<() => void> };
};

const tauri = () => (window as unknown as { __TAURI__?: TauriEvents }).__TAURI__ ?? null;

/** The button's rectangle in physical pixels, relative to the window's client area. */
export const setSnapBounds = (x: number, y: number, width: number, height: number) =>
  void tauri()?.core.invoke("set_snap_bounds", { x, y, width, height }).catch(() => {});

/** Calls `handler(true)` when the cursor enters the overlay and `handler(false)` when it leaves. */
export async function onSnapHover(handler: (hovering: boolean) => void): Promise<() => void> {
  return (await tauri()?.event.listen("snap-hover", (e) => handler(e.payload === true))) ?? (() => {});
}

// Starting the daemon, choosing folders and dropping folders (ADR-0073).

/** Starts the daemon if nothing answers `/health`, and resolves once it does. Idempotent.
 *  Rejects with the reason on failure, and outside the shell. */
export async function startDaemon(): Promise<void> {
  const t = tauri();
  if (!t) throw new Error("Starting the daemon needs the Tauri shell");
  await t.core.invoke("start_daemon");
}

/** The native folder dialog, several folders allowed. Absolute paths; empty if cancelled, and
 *  empty outside the shell (the page falls back to a text box). */
export async function pickFolders(): Promise<string[]> {
  const t = tauri();
  if (!t) return [];
  return ((await t.core.invoke("pick_folders")) as string[] | null) ?? [];
}

export type FolderDragEvent =
  | { kind: "enter" | "over"; position: { x: number; y: number } }
  | { kind: "leave" }
  | { kind: "drop"; paths: string[]; position: { x: number; y: number } };

/** Follows a file drag over the window: Tauri's `tauri://drag-enter`, `drag-over`, `drag-leave`
 *  and `drag-drop` events, in the page's CSS pixels (the physical position divided by the device
 *  pixel ratio). The page draws its own drop zone from these, because the webview delivers no
 *  HTML5 drag events while Tauri handles drops. Returns a function that stops listening; outside
 *  the shell it listens to nothing. */
export async function onFolderDrag(handler: (e: FolderDragEvent) => void): Promise<() => void> {
  const t = tauri();
  if (!t) return () => {};
  type Payload = { paths?: string[]; position?: { x: number; y: number } };
  const css = (p?: { x: number; y: number }) => ({ x: (p?.x ?? 0) / devicePixelRatio, y: (p?.y ?? 0) / devicePixelRatio });
  const on = (name: string, fn: (p: Payload) => void) => t.event.listen(name, (e) => fn((e.payload ?? {}) as Payload));
  const offs = await Promise.all([
    on("tauri://drag-enter", (p) => handler({ kind: "enter", position: css(p.position) })),
    on("tauri://drag-over", (p) => handler({ kind: "over", position: css(p.position) })),
    on("tauri://drag-leave", () => handler({ kind: "leave" })),
    on("tauri://drag-drop", (p) => handler({ kind: "drop", paths: p.paths ?? [], position: css(p.position) })),
  ]);
  return () => offs.forEach((off) => off());
}
