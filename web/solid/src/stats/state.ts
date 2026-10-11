// The stats view's state. The figures are read-only and come whole from one route, so there
// is no store: a resource keyed by the project scope, and the mark the pointer is over.

import { createResource, createSignal } from "solid-js";
import { api, projectScope, route, setNotice, setProjectScope, setShell } from "../shell/shell";
import type { StatsScope } from "../../../shared/types";

// The source is false while another view is showing, so nothing is fetched until the page is
// opened and the figures are read afresh each time it is (a scan may have changed them).
export const [statistics, { refetch: reloadStatistics }] = createResource(
  () => (route() === "stats" ? projectScope() : false),
  (scope) => api.statistics(scope),
);

/** The mark under the pointer: its element, which carries the tooltip's words. */
export const [hover, setHover] = createSignal<HTMLElement | null>(null);

export function setScope(scope: StatsScope) {
  setShell({ menu: null, popover: null });
  setProjectScope(scope);
}

/** The CSV of every figure, in the scope on screen. A plain download: the daemon names the
 *  file, and the browser or webview saves it. */
export function exportCsv() {
  const a = document.createElement("a");
  a.href = api.statisticsExportUrl(projectScope());
  a.download = "";
  document.body.append(a);
  a.click();
  a.remove();
  setNotice(null);
}
