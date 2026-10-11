// The folder drop zone (ADR-0073, ADR-0074): raised over the Projects view, or the welcome, while
// a file drag is over the window. The webview delivers no HTML5 drag events while Tauri handles
// drops, so the page draws this itself from `onFolderDrag`.

import { createSignal, onCleanup, onMount, Show } from "solid-js";
import { onFolderDrag } from "../../../shared/os";
import { addFolders, dropAllowed, reportFailures, scanProjects } from "./flow";
import { Icon } from "./parts";
import { setNotice } from "./shell";

export function DropZone() {
  const [over, setOver] = createSignal(false);

  onMount(() => {
    let stop: (() => void) | null = null;
    let unmounted = false;
    onCleanup(() => { unmounted = true; stop?.(); });
    void onFolderDrag(async (e) => {
      if (e.kind === "leave") { setOver(false); return; }
      if (!dropAllowed()) { setOver(false); return; }
      if (e.kind !== "drop") { setOver(true); return; }
      setOver(false);
      if (!e.paths.length) return;
      setNotice(null);
      const { added, failed } = await addFolders(e.paths);
      reportFailures(failed);
      if (added.length) scanProjects();
    }).then((off) => { if (unmounted) off(); else stop = off; });
  });

  return (
    <Show when={over() && dropAllowed()}>
      <div class="flow-screen drop-zone" aria-hidden="true">
        <div class="empty"><Icon name="create_new_folder" /><h2>Drop folders to add them</h2></div>
      </div>
    </Show>
  );
}
