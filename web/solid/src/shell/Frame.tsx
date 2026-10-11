// The frame (ADR-0032): top bar, sidebar, status bar. The status bar is where push-speed
// state lands, and it is the only reader of `scan`.

import { For, Show, type JSX } from "solid-js";
import { LOGO_PATH, LOGO_TRANSFORM, LOGO_VIEWBOX } from "../../../shared/logo";
import { plural } from "../../../shared/projects";
import { inTauri } from "../../../shared/os";
import { Icon, TbBtn, WindowControls } from "./parts";
import { closePopups, counts, notice, route, scan, setRoute, setShell, shell, system, type RouteId } from "./shell";
import type { View } from "./view";

const NAV: { id: RouteId | null; label: string; icon: string }[] = [
  { id: "projects", label: "Projects", icon: "audio_file" },
  { id: "collections", label: "Collections", icon: "album" },
  { id: "plugins", label: "Plugins", icon: "plug_connect" },
  { id: "samples", label: "Samples", icon: "earthquake" },
  { id: "stats", label: "Stats", icon: "bar_chart" },
];

export function Topbar(props: { view: View }) {
  let timer: ReturnType<typeof setTimeout> | undefined;
  // Search waits for typing to pause, then asks the server (FTS5, ADR-0024).
  const onInput = (value: string) => { clearTimeout(timer); timer = setTimeout(() => props.view.setQuery(value.trim()), 250); };
  return (
    <header class="topbar" data-tauri-drag-region>
      <svg class="logo" viewBox={LOGO_VIEWBOX} role="img" aria-label="Seula">
        <path fill="currentColor" transform={LOGO_TRANSFORM} d={LOGO_PATH} />
      </svg>
      <nav class="menubar"><span>File</span><span>Edit</span><span>View</span><span>Tools</span>
        <span role="button" tabIndex={0} title="About Seula" onClick={() => { closePopups(); setShell("about", true); }}
          onKeyDown={(e) => { if (e.key === "Enter" || e.key === " ") { closePopups(); setShell("about", true); } }}>Help</span>
      </nav>
      <label class="search">
        <Icon name="search" />
        <input placeholder="Search projects, plugins, samples, tags" value={props.view.query()}
          onInput={(e) => onInput(e.currentTarget.value)} />
      </label>
      <TbBtn icon="settings" title="Settings" onClick={() => { closePopups(); setShell("settingsOpen", true); }} />
      <Show when={inTauri()} fallback={<span style={{ width: "calc(var(--u) * 2)" }} />}>
        <WindowControls />
      </Show>
    </header>
  );
}

export function Sidebar() {
  return (
    <aside class="sidebar">
      <For each={NAV}>{(v) => {
        const active = () => v.id === route();
        return (
          <div class="nav-item" classList={{ active: active(), off: v.id == null }} title={v.label}
            onClick={() => { if (v.id) { setShell({ menu: null, popover: null }); setRoute(v.id); } }}>
            <Icon name={v.icon} fill={active()} />
            <span class="label">{v.label}</span>
            <span class="count">{v.id && v.id !== "stats" ? (counts[v.id] ?? "") : ""}</span>
          </div>
        );
      }}</For>
      <div class="spacer" />
      <button class="tb-btn collapse" title={`${shell.sidebarCollapsed ? "Expand" : "Collapse"} sidebar`}
        onClick={() => setShell("sidebarCollapsed", (c) => !c)}>
        <Icon name={shell.sidebarCollapsed ? "left_panel_open" : "left_panel_close"} />
      </button>
    </aside>
  );
}

/** The scan segment. Its own component so a progress tick re-runs this and nothing else. */
function ScanSegment() {
  return (
    <span class="grow">
      <Show when={scan()}>{(s) => (
        <>
          <span class="progress"><i style={{ width: `${(100 * (s().total ? s().completed / s().total : 0)).toFixed(1)}%` }} /></span>
          {s().message}
        </>
      )}</Show>
      <Show when={!scan() && notice()}>
        <span style={{ color: "var(--danger)" }}>{notice()}</span>
      </Show>
    </span>
  );
}

export function Statusbar(props: { keys: boolean; children: JSX.Element }) {
  const watcher = () => {
    const s = system();
    if (!s) return "";
    return s.watcher_active ? `Watching ${plural(s.watch_paths.length, "folder")}` : "Watcher off";
  };
  return (
    <footer class="statusbar">
      {props.children}
      <ScanSegment />
      <span>{watcher()}</span>
      <Show when={props.keys}>
        <span class="ks">
          <span class="keyswitch">
            <button classList={{ on: shell.spelling === "sharp" }} title="Show keys with sharps" onClick={() => setShell("spelling", "sharp")}>♯</button>
            <button classList={{ on: shell.spelling === "flat" }} title="Show keys with flats" onClick={() => setShell("spelling", "flat")}>♭</button>
          </span>
        </span>
      </Show>
    </footer>
  );
}
