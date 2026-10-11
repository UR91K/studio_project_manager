// The Settings dialog (ADR-0074), on the shared Dialog (ADR-0059). Project folders apply as
// they are added or removed, because the daemon validates each one on its own. Everything
// else is held until Save, then sent as one update with only the fields that changed.
// Values are read fresh from the daemon each time the dialog opens.

import { For, Show, createSignal, onMount } from "solid-js";
import { inTauri, pickFolders } from "../../../shared/os";
import type { ConfigInfo, ConfigSettingsUpdate, ConfigStatus } from "../../../shared/types";
import { Dialog } from "./Dialog";
import { Icon } from "./parts";
import { api, setShell } from "./shell";
import "./settings.css";

const LOG_LEVELS = ["error", "warn", "info", "debug", "trace"];
const DEFAULT_SEARCH_NOTE = "No folders set, so the standard plugin locations for this system are used.";

const message = (e: unknown) => (e instanceof Error ? e.message : String(e));
const sameList = (a: string[], b: string[]) => a.length === b.length && a.every((x, i) => x === b[i]);

/** A list of folders with remove buttons and an add control: the native picker in the shell,
 *  a text box in a browser (where `pickFolders` has nothing to open). */
function FolderList(props: {
  items: string[]; empty: string; busy?: boolean; onAdd(path: string): void; onRemove(path: string): void;
  warnings?: Record<string, string[]>;
}) {
  const [text, setText] = createSignal("");
  const add = async () => {
    if (inTauri()) { for (const p of await pickFolders()) props.onAdd(p); return; }
    const p = text().trim();
    if (p) { props.onAdd(p); setText(""); }
  };
  return (
    <>
      <Show when={props.items.length} fallback={<p class="faint">{props.empty}</p>}>
        <ul class="plain setlist">
          <For each={props.items}>{(p) => (
            <li>
              <span class="path" title={p}>{p}</span>
              <button type="button" class="btn" aria-label={`Remove ${p}`} disabled={props.busy} onClick={() => props.onRemove(p)}>
                <Icon name="close" />
              </button>
              <For each={props.warnings?.[p] ?? []}>{(w) => <div class="warn-line">{w}</div>}</For>
            </li>
          )}</For>
        </ul>
      </Show>
      <div class="row setadd">
        <Show when={inTauri()} fallback={
          <>
            <label class="field focus">
              <input value={text()} placeholder="Type a folder path" aria-label="Folder path"
                onInput={(e) => setText(e.currentTarget.value)}
                onKeyDown={(e) => { if (e.key === "Enter") { e.preventDefault(); void add(); } }} />
            </label>
            <button type="button" class="btn" disabled={props.busy || !text().trim()} onClick={() => void add()}>Add</button>
          </>
        }>
          <button type="button" class="btn" disabled={props.busy} onClick={() => void add()}>Add folder…</button>
        </Show>
      </div>
    </>
  );
}

export function SettingsDialog() {
  const close = () => setShell("settingsOpen", false);
  const [cfg, setCfg] = createSignal<ConfigInfo | null>(null);
  const [status, setStatus] = createSignal<ConfigStatus | null>(null);
  const [loadError, setLoadError] = createSignal<string | null>(null);
  const [error, setError] = createSignal<string | null>(null);
  const [busy, setBusy] = createSignal(false);
  const [saved, setSaved] = createSignal(false);

  const [paths, setPaths] = createSignal<string[]>([]);
  const [warnings, setWarnings] = createSignal<Record<string, string[]>>({});
  const [search, setSearch] = createSignal<string[]>([]);
  const [timeout, setTimeoutSecs] = createSignal("");
  const [cover, setCover] = createSignal("");
  const [audio, setAudio] = createSignal("");
  const [level, setLevel] = createSignal("info");

  const fill = (c: ConfigInfo) => {
    setCfg(c);
    setPaths(c.paths);
    setSearch(c.vst_search_paths ?? []);
    setTimeoutSecs(c.vst_scan_timeout_secs == null ? "" : String(c.vst_scan_timeout_secs));
    setCover(c.max_cover_art_size_mb == null ? "" : String(c.max_cover_art_size_mb));
    setAudio(c.max_audio_file_size_mb == null ? "" : String(c.max_audio_file_size_mb));
    setLevel(c.log_level.toLowerCase());
  };
  onMount(async () => {
    try {
      const [c, s] = await Promise.all([api.config(), api.configStatus()]);
      setStatus(s);
      fill(c);
    } catch (e) { setLoadError(message(e)); }
  });

  const addPath = async (path: string) => {
    if (paths().includes(path)) return;
    setBusy(true); setError(null);
    try {
      const w = await api.addProjectPath(path);
      setPaths((p) => [...p, path]);
      if (w.length) setWarnings((m) => ({ ...m, [path]: w }));
    } catch (e) { setError(`Could not add ${path}: ${message(e)}`); }
    finally { setBusy(false); }
  };
  const removePath = async (path: string) => {
    setBusy(true); setError(null);
    try {
      await api.removeProjectPath(path);
      setPaths((p) => p.filter((x) => x !== path));
      setWarnings((m) => Object.fromEntries(Object.entries(m).filter(([k]) => k !== path)));
    } catch (e) { setError(`Could not remove ${path}: ${message(e)}`); }
    finally { setBusy(false); }
  };

  /** A megabyte field: empty is "no setting" (null), otherwise a number above zero. */
  const mb = (text: string, name: string): number | null | Error => {
    const t = text.trim();
    if (!t) return null;
    const n = Number(t);
    return Number.isFinite(n) && n > 0 ? n : new Error(`${name} must be a number above zero, or empty.`);
  };

  const save = async () => {
    const c = cfg();
    if (!c || busy()) return;
    const u: ConfigSettingsUpdate = {};
    if (c.vst_search_paths && !sameList(search(), c.vst_search_paths)) u.vst_search_paths = search();
    const t = timeout().trim();
    if (c.vst_scan_timeout_secs !== undefined && t) {
      const n = Number(t);
      if (!Number.isInteger(n) || n <= 0) { setError("The plugin time limit must be a whole number of seconds above zero."); return; }
      if (n !== c.vst_scan_timeout_secs) u.vst_scan_timeout_secs = n;
    }
    const cv = mb(cover(), "The cover art limit");
    if (cv instanceof Error) { setError(cv.message); return; }
    const au = mb(audio(), "The audio limit");
    if (au instanceof Error) { setError(au.message); return; }
    if (cv !== c.max_cover_art_size_mb) u.max_cover_art_size_mb = cv;
    if (au !== c.max_audio_file_size_mb) u.max_audio_file_size_mb = au;
    if (level() !== c.log_level.toLowerCase()) u.log_level = level();

    if (!Object.keys(u).length) { close(); return; }
    setBusy(true); setError(null);
    try { await api.updateSettings(u); }
    catch (e) { setError(message(e)); setBusy(false); return; }
    // If anything is now waiting on a restart, or the file failed to apply, stay open and say so.
    try {
      const s = await api.configStatus();
      setStatus(s);
      if (s.reload_error || s.restart_required?.length) { setSaved(true); setBusy(false); return; }
    } catch { /* the save worked; the status is only extra */ }
    setBusy(false);
    close();
  };

  const pending = () => status()?.restart_required ?? [];

  return (
    <Dialog title="Settings" width={200} onClose={close}
      buttons={
        <>
          <button type="button" class="btn" onClick={close}>Cancel</button>
          <button type="button" class="btn primary" disabled={!cfg() || busy()} onClick={() => void save()}>Save</button>
        </>
      }>
      <Show when={status()?.reload_error}>{(e) => (
        <p class="warn-line">The config file was not applied: {e()}</p>
      )}</Show>
      <Show when={pending().length}>
        <p class="note">{saved() ? "Saved. " : ""}These need a restart to take effect: {pending().join(", ")}.</p>
      </Show>
      <Show when={loadError()}>{(e) => <p class="warn-line">Could not load the settings: {e()}</p>}</Show>
      <Show when={cfg()} fallback={<Show when={!loadError()}><p class="faint">Loading…</p></Show>}>
        {(c) => (
          <>
            <label>Project folders</label>
            <FolderList items={paths()} empty="No folders yet. Add the folders that hold your Live projects."
              busy={busy()} warnings={warnings()} onAdd={(p) => void addPath(p)} onRemove={(p) => void removePath(p)} />
            <p class="faint">Changes to this list apply straight away.</p>

            <label>Plugin search folders</label>
            <Show when={c().vst_search_paths !== undefined} fallback={<p class="faint">Not available.</p>}>
              <FolderList items={search()} empty={DEFAULT_SEARCH_NOTE}
                onAdd={(p) => setSearch((l) => (l.includes(p) ? l : [...l, p]))}
                onRemove={(p) => setSearch((l) => l.filter((x) => x !== p))} />
            </Show>

            <label>Plugin time limit</label>
            <Show when={c().vst_scan_timeout_secs !== undefined} fallback={<p class="faint">Not available.</p>}>
              <div class="row">
                <label class="field focus numfield">
                  <input inputmode="numeric" value={timeout()} aria-label="Plugin time limit in seconds"
                    onInput={(e) => setTimeoutSecs(e.currentTarget.value)} />
                </label>
                <span class="dim">seconds per plugin</span>
              </div>
            </Show>

            <label>File size limits</label>
            <div class="row">
              <label class="field focus numfield">
                <input inputmode="decimal" value={cover()} placeholder="Default" aria-label="Cover art limit in megabytes"
                  onInput={(e) => setCover(e.currentTarget.value)} />
              </label>
              <span class="dim">MB for cover art</span>
            </div>
            <div class="row" style={{ "margin-top": "calc(var(--u) * 3)" }}>
              <label class="field focus numfield">
                <input inputmode="decimal" value={audio()} placeholder="No limit" aria-label="Audio file limit in megabytes"
                  onInput={(e) => setAudio(e.currentTarget.value)} />
              </label>
              <span class="dim">MB for audio files</span>
            </div>
            <p class="faint">Leave a box empty to use the default.</p>

            <label>Log level</label>
            <select class="setselect" value={level()} aria-label="Log level" onChange={(e) => setLevel(e.currentTarget.value)}>
              <For each={LOG_LEVELS}>{(l) => <option value={l} selected={l === level()}>{l}</option>}</For>
            </select>

            <label>Locations <span class="faint">(applies after restart)</span></label>
            <dl class="setloc">
              <dt>Database</dt><dd>{c().database_path ?? "Default location"}</dd>
              <dt>Media folder</dt><dd>{c().media_storage_dir}</dd>
              <dt>HTTP port</dt><dd>{c().http_port ?? "Not available"}</dd>
            </dl>
            <p class="faint">These are changed in config.toml.</p>
          </>
        )}
      </Show>
      <Show when={error()}>{(e) => <p class="warn-line" role="alert">{e()}</p>}</Show>
    </Dialog>
  );
}
