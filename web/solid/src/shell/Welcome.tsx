// The onboarding screen (ADR-0074): shown in place of the view while the config has no project
// folders. The folders are chosen first and written to the config only on "Scan", so leaving
// with "Not now" changes nothing.

import { createSignal, For, Show } from "solid-js";
import { inTauri, pickFolders } from "../../../shared/os";
import { addFolders, reportFailures, scanProjects, welcomeOpen } from "./flow";
import { Icon } from "./parts";
import { setShell } from "./shell";

export function Welcome() {
  const [chosen, setChosen] = createSignal<string[]>([]);
  const [problems, setProblems] = createSignal<Record<string, string>>({});
  const [typed, setTyped] = createSignal("");
  const [busy, setBusy] = createSignal(false);

  const include = (paths: string[]) => setChosen((c) => {
    const next = [...c];
    for (const p of paths.map((x) => x.trim())) if (p && !next.includes(p)) next.push(p);
    return next;
  });
  const browse = async () => include(await pickFolders());
  const typeIn = () => { include([typed()]); setTyped(""); };
  const remove = (path: string) => {
    setChosen((c) => c.filter((p) => p !== path));
    setProblems((all) => Object.fromEntries(Object.entries(all).filter(([p]) => p !== path)));
  };

  const scan = async () => {
    setBusy(true);
    const { added, failed, warnings } = await addFolders(chosen());
    setBusy(false);
    setChosen((c) => c.filter((p) => !added.includes(p)));
    setProblems(Object.fromEntries([
      ...failed.map((f) => [f.path, f.reason]), ...Object.entries(warnings).map(([p, w]) => [p, w.join(" ")]),
    ]));
    if (added.length) { reportFailures(failed); scanProjects(); }
  };

  return (
    <Show when={welcomeOpen()}>
      <div class="flow-screen">
        <div class="welcome">
          <Icon name="folder_open" />
          <h2>Welcome to Seula</h2>
          <p>Seula keeps track of your Ableton Live projects. Choose the folders where they live and it will list what it finds.</p>
          <Show when={inTauri()}
            fallback={
              <div class="row">
                <label class="field">
                  <input type="text" placeholder="Path to a folder" value={typed()} aria-label="Folder path"
                    onInput={(e) => setTyped(e.currentTarget.value)}
                    onKeyDown={(e) => { if (e.key === "Enter") typeIn(); }} />
                </label>
                <button class="btn" disabled={!typed().trim()} onClick={typeIn}>Add</button>
              </div>}>
            <div><button class="btn" onClick={browse}>Add project folders</button></div>
          </Show>
          <Show when={chosen().length}>
            <ul class="folders">
              <For each={chosen()}>{(path) => (
                <li>
                  <div class="path"><span title={path}>{path}</span>
                    <Show when={problems()[path]}><span class="flow-error">{problems()[path]}</span></Show></div>
                  <button class="btn" title="Remove" aria-label={`Remove ${path}`} onClick={() => remove(path)}>
                    <Icon name="close" /></button>
                </li>)}
              </For>
            </ul>
          </Show>
          <div class="flow-actions">
            <button class="btn primary" disabled={busy() || !chosen().length} onClick={scan}>{busy() ? "Adding..." : "Scan"}</button>
            <button class="btn" disabled={busy()} onClick={() => setShell("skipOnboarding", true)}>Not now</button>
          </div>
        </div>
      </div>
    </Show>
  );
}
