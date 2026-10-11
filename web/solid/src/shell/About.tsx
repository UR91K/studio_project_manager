// Help > About: the version, the licence, and where the source is. The AGPL obliges a
// distributed build to offer its source, and this is where a user looks for that (ADR-0068).
//
// The address is shown as text with a Copy button, not a link: opening a URL is an OS call,
// and a new Tauri command needs its own ADR (ADR-0048).

import { createSignal } from "solid-js";
import { Dialog } from "./Dialog";
import { displayScale, zoom } from "./zoom";
import { setShell, system } from "./shell";

export const SOURCE_URL = "https://github.com/UR91K/seula";

export function AboutDialog() {
  const close = () => setShell("about", false);
  const [copied, setCopied] = createSignal(false);
  const copy = async () => {
    try { await navigator.clipboard.writeText(SOURCE_URL); setCopied(true); } catch { /* the text is selectable */ }
  };
  return (
    <Dialog title="About Seula" width={170} onClose={close}
      buttons={<button class="btn primary" onClick={close}>Close</button>}>
      <p><strong>Seula</strong> {system()?.version ?? ""}</p>
      <p class="dim">An Ableton Live project manager.</p>
      <label>Licence</label>
      <p>
        Free software under the GNU Affero General Public License, version 3 or later. The plugin
        scanner (vst-meta) is under the Mozilla Public License 2.0. The licence files are in the
        install folder.
      </p>
      <label>Source code</label>
      <p>The source for this version is at the tag matching the number above:</p>
      <p><code style={{ "user-select": "text" }}>{SOURCE_URL}</code></p>
      <p><button class="btn" onClick={copy}>{copied() ? "Copied" : "Copy address"}</button></p>
      <p class="dim" style={{ "font-size": "0.9em" }}>
        Display: zoom {Math.round(zoom() * 100)}%, display scale {Math.round(displayScale() * 100)}%
      </p>
    </Dialog>
  );
}
