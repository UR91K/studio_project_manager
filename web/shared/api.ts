// The daemon's HTTP client: plain fetch, as ADR-0048 says. No Tauri IPC for data.

import type {
  Collection, CollectionRow, CollectionSortKey, CollectionStats, CollectionTask, ConfigInfo, ConfigSettingsUpdate, ConfigStatus,
  FailedPluginFile, FormatRollup,
  PluginDetails, PluginRow,
  Project, SampleFile, SampleFormat, SampleRow, ScanProgress, Scope, Statistics, StatsScope, SystemInfo, VendorRollup,
} from "./types";

/** The daemon's default `http_port` (src/config/defaults.rs), which is where an installed
 *  build finds it. `just dev` and `just tauri` set VITE_SEULA_URL, to the mock daemon
 *  `mockup/data/generate.py` seeds (its HTTP_PORT, 50152) or to this one. */
export const DEFAULT_URL = "http://127.0.0.1:50052";

/** Rows per request when reading a whole list. The list routes page, and most answer 1000
 *  rows when no limit is given (`limit.unwrap_or(1000)` in src/database), so leaving the
 *  limit out does not mean everything. */
const PAGE = 5000;

/** The error for a failed response. The daemon answers `{"error": "..."}`, and that text is what a
 *  person should read (bugs.md `api-errors-shown-raw`); anything else keeps the request line and body. */
async function failure(method: string, path: string, res: Response): Promise<Error> {
  const text = await res.text();
  try {
    const error = (JSON.parse(text) as { error?: unknown } | null)?.error;
    if (typeof error === "string" && error) return new Error(error);
  } catch { /* not JSON */ }
  return new Error(`${method} ${path}: ${res.status} ${text}`);
}

export class Api {
  constructor(readonly base: string = DEFAULT_URL) {}

  private async json<T>(path: string, init?: RequestInit): Promise<T> {
    const res = await fetch(this.base + path, init);
    if (!res.ok) throw await failure(init?.method ?? "GET", path, res);
    return res.json() as Promise<T>;
  }

  /** Every row of a list route, read a page at a time. Throws when the rows read do not add
   *  up to the route's own `total_count`: fixed limits (200, then 10000) once cut search and
   *  samples short with nothing on screen to say so. */
  private async all<T>(path: string, key: string): Promise<T[]> {
    const sep = path.includes("?") ? "&" : "?";
    const rows: T[] = [];
    for (;;) {
      const page = await this.json<{ total_count: number } & Record<string, unknown>>(
        `${path}${sep}limit=${PAGE}&offset=${rows.length}`);
      const got = page[key] as T[];
      rows.push(...got);
      if (got.length < PAGE || rows.length >= page.total_count) {
        if (rows.length !== page.total_count) {
          throw new Error(`GET ${path}: read ${rows.length} rows, the daemon counts ${page.total_count}`);
        }
        return rows;
      }
    }
  }

  /** A write whose answer carries nothing the screen needs. */
  private async send(path: string, init: RequestInit): Promise<void> {
    const res = await fetch(this.base + path, init);
    if (!res.ok) throw await failure(init.method ?? "GET", path, res);
  }

  projects(scope: Scope): Promise<Project[]> {
    return this.all(scope === "archived" ? "/api/v1/projects?scope=deleted" : "/api/v1/projects", "projects");
  }

  search(query: string): Promise<Project[]> {
    return this.all(`/api/v1/search?query=${encodeURIComponent(query)}`, "projects");
  }

  collections(): Promise<Collection[]> {
    return this.all("/api/v1/collections", "collections");
  }

  systemInfo() { return this.json<SystemInfo>("/api/v1/system/info"); }

  /** The config routes (ADR-0072, ADR-0074). Every change applies to the running daemon. */
  async config() { return (await this.json<{ config: ConfigInfo }>("/api/v1/config")).config; }
  configStatus() { return this.json<ConfigStatus>("/api/v1/config/status"); }
  /** Resolves with the daemon's validation warnings; rejects when the path cannot be used. */
  async addProjectPath(path: string) {
    return (await this.json<{ validation_warnings: string[] }>("/api/v1/config/paths", post({ path }))).validation_warnings;
  }
  async removeProjectPath(path: string) { await this.send("/api/v1/config/paths/remove", post({ path })); }
  async updateSettings(settings: ConfigSettingsUpdate) { await this.send("/api/v1/config/settings", put(settings)); }
  async reloadConfig() { await this.send("/api/v1/config/reload", post({})); }

  /** Asks the daemon to stop, the way the tray's Quit does (ADR-0076). Answers before it goes. */
  async shutdownDaemon() { await this.send("/api/v1/system/shutdown", post({})); }

  /** Starts or stops the folder watcher. */
  async startWatcher() { await this.send("/api/v1/system/watcher/start", post({})); }
  async stopWatcher() { await this.send("/api/v1/system/watcher/stop", post({})); }

  /** The library's figures in a project scope (ADR-0045). */
  statistics(scope: StatsScope) { return this.json<Statistics>(`/api/v1/system/statistics?scope=${scope}`); }

  /** Where the same figures download as CSV. */
  statisticsExportUrl(scope: StatsScope) { return `${this.base}/api/v1/system/statistics/export?scope=${scope}`; }

  async setNotes(id: string, notes: string) {
    await this.send(`/api/v1/projects/${id}/notes`, put({ notes }));
  }

  async setName(id: string, name: string) {
    await this.send(`/api/v1/projects/${id}/name`, put({ name }));
  }

  /** The total the daemon reports for a list route, without fetching the list. */
  async count(route: "projects" | "plugins" | "samples" | "collections"): Promise<number> {
    return (await this.json<{ total_count: number }>(`/api/v1/${route}?limit=1`)).total_count;
  }

  // ---------------------------------------------------------------- collections

  /** The server sorts this list (unlike plugins and samples), counting in the active scope
   *  (ADR-0043). */
  collectionList(sortBy: CollectionSortKey, desc: boolean): Promise<CollectionRow[]> {
    return this.all(`/api/v1/collections?sort_by=${sortBy}&sort_desc=${desc}`, "collections");
  }

  searchCollections(query: string): Promise<CollectionRow[]> {
    return this.all(`/api/v1/collections/search?query=${encodeURIComponent(query)}`, "collections");
  }

  collectionStats(id: string) { return this.json<CollectionStats>(`/api/v1/collections/${id}/statistics`); }

  collectionTasks(id: string) {
    return this.json<{ tasks: CollectionTask[]; total_tasks: number; completed_tasks: number }>(`/api/v1/collections/${id}/tasks`);
  }

  /** The tracklist, in collection order. */
  async collectionProjects(id: string): Promise<Project[]> {
    return this.json<Project[]>(`/api/v1/collections/${id}/projects`);
  }

  createCollection(name: string, description: string | null) {
    return this.json<CollectionRow>("/api/v1/collections", post({ name, description }));
  }

  updateCollection(id: string, change: { name?: string; description?: string; notes?: string }) {
    return this.json<CollectionRow>(`/api/v1/collections/${id}`, put(change));
  }

  duplicateCollection(id: string, newName: string) {
    return this.json<CollectionRow>(`/api/v1/collections/${id}/duplicate`, post({ new_name: newName }));
  }

  async deleteCollection(id: string) { await this.send(`/api/v1/collections/${id}`, { method: "DELETE" }); }

  async reorderCollection(id: string, projectIds: string[]) {
    await this.send(`/api/v1/collections/${id}/reorder`, put({ project_ids: projectIds }));
  }

  async removeFromCollection(id: string, projectIds: string[]) {
    await this.send(`/api/v1/collections/${id}/batch-remove`, post({ project_ids: projectIds }));
  }

  /** Send an image's bytes and make it the collection's cover (ADR-0061). The daemon keeps
   *  its own copy. A failure is an HTTP error, so `json` and `send` throw it. */
  async setCollectionCover(id: string, file: File): Promise<void> {
    const up = await this.json<{ media_file_id: string }>(
      `/api/v1/media/cover-art?collection_id=${id}&filename=${encodeURIComponent(file.name)}`,
      { method: "POST", headers: { "content-type": file.type || "application/octet-stream" }, body: file });
    await this.send(`/api/v1/collections/${id}/cover-art`, put({ media_file_id: up.media_file_id }));
  }

  async removeCollectionCover(id: string): Promise<void> {
    await this.send(`/api/v1/collections/${id}/cover-art`, { method: "DELETE" });
  }

  /** A cover's URL: media is served over HTTP (ADR-0033). */
  mediaUrl(id: string) { return `${this.base}/api/v1/media/${id}`; }

  // ---------------------------------------------------------------- plugins

  plugins(): Promise<PluginRow[]> {
    return this.all("/api/v1/plugins", "plugins");
  }

  searchPlugins(query: string): Promise<PluginRow[]> {
    return this.all(`/api/v1/plugins/search?query=${encodeURIComponent(query)}`, "plugins");
  }

  vendors(): Promise<VendorRollup[]> {
    return this.all("/api/v1/plugins/vendors", "vendors");
  }

  async formats(): Promise<FormatRollup[]> {
    return (await this.json<{ formats: FormatRollup[] }>("/api/v1/plugins/formats")).formats;
  }

  async pluginDetails(id: string): Promise<PluginDetails> {
    return (await this.json<{ details: PluginDetails }>(`/api/v1/plugins/${id}`)).details;
  }

  /** Plugin files that failed and have never loaded (ADR-0067). Few, so not paged. */
  async failedPluginFiles(): Promise<FailedPluginFile[]> {
    return (await this.json<{ files: FailedPluginFile[] }>("/api/v1/plugins/failed-files")).files;
  }

  pluginProjects(id: string): Promise<Project[]> {
    return this.all(`/api/v1/plugins/${id}/projects`, "projects");
  }

  // ---------------------------------------------------------------- samples

  samples(): Promise<SampleRow[]> {
    return this.all("/api/v1/samples", "samples");
  }

  searchSamples(query: string): Promise<SampleRow[]> {
    return this.all(`/api/v1/samples/search?query=${encodeURIComponent(query)}`, "samples");
  }

  async sampleFormats(): Promise<SampleFormat[]> {
    return (await this.json<{ formats: SampleFormat[] }>("/api/v1/samples/formats")).formats;
  }

  /** The file record the last check made; null when no check has found the file. */
  async sampleFile(id: string): Promise<SampleFile | null> {
    return (await this.json<{ file: SampleFile | null }>(`/api/v1/samples/${id}`)).file;
  }

  sampleProjects(id: string): Promise<Project[]> {
    return this.all(`/api/v1/samples/${id}/projects`, "projects");
  }

  // ---------------------------------------------------------------- scans

  /** Start a scan and follow it. A scan's `POST` answers with the SSE stream itself, which
   *  `EventSource` cannot read (it only makes GET requests), so the body is read by hand
   *  (ADR-0056). A scan that is already running answers 409 and this throws. */
  scan(kind: "projects" | "plugins" | "samples", signal?: AbortSignal, mode?: PluginScanMode): AsyncGenerator<ScanProgress> {
    const plugins = mode === "all" ? "/api/v1/plugins/scan?mode=all" : "/api/v1/plugins/scan";
    const route = { projects: "/api/v1/system/scan", plugins, samples: "/api/v1/samples/check" }[kind];
    return sse<ScanProgress>(this.base + route, { method: "POST", signal });
  }
}

/** A plugin scan loads only new and changed files, or every file (ADR-0067). */
export type PluginScanMode = "changes" | "all";

const put = (body: unknown): RequestInit => ({
  method: "PUT", headers: { "content-type": "application/json" }, body: JSON.stringify(body),
});

const post = (body: unknown): RequestInit => ({
  method: "POST", headers: { "content-type": "application/json" }, body: JSON.stringify(body),
});

/** Parse a text/event-stream response body into its `data:` payloads. */
export async function* sse<T>(url: string, init: RequestInit): AsyncGenerator<T> {
  const res = await fetch(url, init);
  if (!res.ok) throw await failure(init.method ?? "GET", url, res);
  const reader = res.body!.pipeThrough(new TextDecoderStream()).getReader();
  let buf = "";
  try {
    for (;;) {
      const { value, done } = await reader.read();
      if (done) return;
      buf += value;
      let end: number;
      while ((end = buf.indexOf("\n\n")) >= 0) {
        const block = buf.slice(0, end);
        buf = buf.slice(end + 2);
        const data = block.split("\n").filter((l) => l.startsWith("data:")).map((l) => l.slice(5).trimStart()).join("\n");
        if (data) yield JSON.parse(data) as T;
      }
    }
  } finally {
    // The consumer may stop early; release the connection rather than leave it open.
    await reader.cancel().catch(() => {});
  }
}

/** A stand-in scan for judging how each framework takes fast pushes: `total` events at
 *  about `perSecond`, shaped like the real stream. Dev only; the real scan is `Api.scan`. */
export async function* simulatedScan(total = 600, perSecond = 40, signal?: AbortSignal): AsyncGenerator<ScanProgress> {
  for (let i = 1; i <= total; i++) {
    if (signal?.aborted) return;
    await new Promise((r) => setTimeout(r, 1000 / perSecond));
    const done = i === total;
    yield { completed: i, total, progress: i / total, status: done ? "completed" : "parsing",
      message: done ? `Scanned ${total} projects` : `Parsed simulated project ${i}.als (${i}/${total})` };
  }
}
