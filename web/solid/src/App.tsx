import { Show, onMount } from "solid-js";
import { Dynamic } from "solid-js/web";
import { collectionsView } from "./collections/view";
import { pluginsView } from "./plugins/view";
import { projectsView } from "./projects/view";
import { samplesView } from "./samples/view";
import { statsView } from "./stats/view";
import { TbBtn } from "./shell/parts";
import { AboutDialog } from "./shell/About";
import { DropZone } from "./shell/DropZone";
import { ErrorScreen } from "./shell/ErrorScreen";
import { SettingsDialog } from "./shell/Settings";
import { Welcome } from "./shell/Welcome";
import { Sidebar, Statusbar, Topbar } from "./shell/Frame";
import { closePopups, loadChrome, route, setShell, shell, type RouteId } from "./shell/shell";
import type { View } from "./shell/view";

const VIEWS: Record<RouteId, View> = {
  projects: projectsView, collections: collectionsView, plugins: pluginsView, samples: samplesView, stats: statsView,
};

export function App() {
  onMount(loadChrome);
  const view = () => VIEWS[route()];
  const inspector = () => view().Inspector;
  const shown = () => !!inspector() && shell.inspectorOpen;

  return (
    <div class="win" classList={{ "sidebar-collapsed": shell.sidebarCollapsed, "no-inspector": !shown() }}
      onKeyDown={(e) => { if (e.key === "Escape") closePopups(); view().onKey?.(e); }} tabIndex={-1}
      onClick={(e) => {
        // A click outside any popover closes the open ones.
        if (!(e.target as HTMLElement).closest(".pop, [data-keep]")) closePopups();
      }}>
      <Topbar view={view()} />
      <div class="cols">
        <Sidebar />
        <section class="main">
          <div class="viewbar">
            <Dynamic component={view().Viewbar} />
            <span class="grow" />
            <Show when={inspector()}>
              <TbBtn icon={shown() ? "right_panel_close" : "right_panel_open"} title="Inspector" on={shown()}
                onClick={() => setShell("inspectorOpen", (o) => !o)} />
            </Show>
          </div>
          <div class="content"><Dynamic component={view().Content} /></div>
        </section>
        <Show when={shown()}><Dynamic component={inspector()!} /></Show>
      </div>
      <Statusbar keys={view().keys}><Dynamic component={view().Status} /></Statusbar>
      <Dynamic component={view().Popovers} />
      <Show when={shell.about}><AboutDialog /></Show>
      <Welcome />
      <ErrorScreen />
      <DropZone />
      <SettingsDialog />
    </div>
  );
}
