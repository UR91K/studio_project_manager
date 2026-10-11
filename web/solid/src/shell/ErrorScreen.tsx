// The "Cannot reach the daemon" screen with its Start button (ADR-0073). It stands in front of
// the views while the daemon does not answer, and watches for the answer so it can leave by
// itself when the daemon starts some other way.

import { createSignal, onCleanup, onMount, Show } from "solid-js";
import { startDaemon } from "../../../shared/os";
import { daemon, probe, reason, restarting } from "./flow";
import { Icon } from "./parts";

const WATCH_MS = 4000;

export function ErrorScreen() {
  const [busy, setBusy] = createSignal(false);
  const [error, setError] = createSignal<string | null>(null);

  onMount(() => {
    void probe();
    const timer = setInterval(() => { if (!busy()) void probe(); }, WATCH_MS);
    onCleanup(() => clearInterval(timer));
  });

  const start = async () => {
    setBusy(true); setError(null);
    try { await startDaemon(); await probe(); }
    catch (e) { setError(reason(e)); }
    finally { setBusy(false); }
  };
  const retry = async () => {
    setBusy(true); setError(null);
    if (!(await probe())) setError("The daemon still does not answer.");
    setBusy(false);
  };

  return (
    <Show when={daemon() === "down" && !restarting()}>
      <div class="flow-screen" role="alert">
        <div class="empty">
          <Icon name="cloud_off" />
          <h2>Cannot reach the daemon</h2>
          <p>Seula needs its background service to list your projects. Start it and this window will fill in.</p>
          <Show when={error()}><p class="flow-error">{error()}</p></Show>
          <div class="flow-actions">
            <button class="btn primary" disabled={busy()} onClick={start}>{busy() ? "Starting..." : "Start daemon"}</button>
            <button class="btn" disabled={busy()} onClick={retry}>Retry</button>
          </div>
        </div>
      </div>
    </Show>
  );
}
