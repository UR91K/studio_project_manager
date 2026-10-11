// The slice of the HTTP DTOs (src/http/dto/) that the projects screen reads. Hand-written
// to match what the server sends, as ADR-0029 expects; no generated code.

export interface Key { tonic: string; scale: string; sharp: string; flat: string }
export interface TimeSignature { numerator: number; denominator: number }
export interface AbletonVersion { major: number; minor: number; patch: number; beta: boolean }
export interface Tag { id: string; name: string; created_at: number }
export interface Plugin {
  id: string; name: string; format: string; vendor: string | null; version: string | null;
  installed: boolean | null;  // null: no scan has looked (ADR-0012)
}
export interface Sample { id: string; name: string; path: string; is_present: boolean }
export interface Task { id: string; description: string; completed: boolean }

export interface Project {
  id: string;
  is_active: boolean;
  name: string;
  path: string;
  notes: string;
  created_at: number;
  modified_at: number;
  tempo: number;
  time_signature: TimeSignature;
  key_signature: Key | null;
  duration_seconds: number | null;
  ableton_version: AbletonVersion;
  plugins: Plugin[];
  samples: Sample[];
  tags: Tag[];
  tasks: Task[];
  collection_ids: string[];
  audio_file_id: string | null;
}

export interface Collection { id: string; name: string; project_count: number }

export interface SystemInfo { version: string; watch_paths: string[]; watcher_active: boolean; uptime_seconds: number }

/** One event of a scan's SSE stream (ScanProgressDto). */
export interface ScanProgress {
  completed: number; total: number; progress: number; message: string; status: string;
  /** The plugin scan's final event carries its counts; the project scan sends none. */
  result?: unknown;
}

/** Which scan a stream belongs to. Only one runs at a time, of any kind (ADR-0038). */
export type ScanKind = "projects" | "plugins" | "samples" | "simulated";

// ---------------------------------------------------------------- plugins

/** A plugin as the list routes send it (PluginDto). `installed` is tri-state (ADR-0012):
 *  null means no scan has looked, which is not the same as looked and absent. */
export interface PluginRow {
  id: string; dev_identifier: string; name: string; format: string;
  installed: boolean | null; vendor: string | null; version: string | null;
  project_count: number;
  /** How its file failed the last scan, when every file of it did (ADR-0067). */
  scan_error?: string | null;
  /** Set on a row that stands for a file that never loaded, not for a plugin. */
  file?: FailedPluginFile;
}

/** GET /plugins/failed-files: a plugin file that failed and has never loaded (ADR-0067). */
export interface FailedPluginFile {
  path: string; error_type: string; error_message: string | null; scanned_at: number;
}

/** GET /plugins/vendors and /plugins/formats: one rollup row per vendor or format. */
export interface Rollup {
  plugin_count: number; installed_plugins: number; missing_plugins: number; unknown_plugins: number;
  unique_projects_using: number;
}
export interface VendorRollup extends Rollup { vendor: string }
export interface FormatRollup extends Rollup { format: string }

export interface PluginClass { name: string; category: string }
export interface PluginBus { name: string; direction: "input" | "output"; media: string; channel_count: number; bus_type: number }
export interface PluginReference {
  dev_identifier: string; ableton_name: string | null; ableton_format: string; resolved_via: string;
}

/** GET /plugins/:id `details`. Everything a scan records is null until one has looked. */
export interface PluginDetails {
  plugin_kind: "VST2" | "VST3"; uid: string; last_scanned_at: number | null; path: string | null;
  category: string | null; is_instrument: boolean | null;
  audio_in_channels: number | null; audio_out_channels: number | null;
  audio_in_buses: number | null; audio_out_buses: number | null;
  has_midi_input: boolean | null; has_midi_output: boolean | null;
  presets: number | null; parameters: number | null; latency_samples: number | null;
  has_gui: boolean | null; vendor_url: string | null;
  fourcc: string | null; preset_chunks: boolean | null; f64_precision: boolean | null;
  silent_when_stopped: boolean | null; midi_in_channels: number | null;
  classes: PluginClass[]; buses: PluginBus[]; references: PluginReference[];
}

export type Scope = "active" | "archived";
export type KeySpelling = "sharp" | "flat";
export interface Sort { col: string; desc: boolean }

// ---------------------------------------------------------------- samples

/** A sample as the list routes send it (SampleDto). `size_bytes` is null until a check has
 *  found the file, and a missing sample keeps the size it last had (ADR-0041). */
export interface SampleRow {
  id: string; name: string; path: string; is_present: boolean; project_count: number;
  size_bytes: number | null;
}

/** GET /samples/:id `file`: what the last check found. */
export interface SampleFile { size_bytes: number; modified_at: number | null; checked_at: number }

/** GET /samples/formats: every known format, empty ones included (ADR-0039). */
export interface SampleFormat {
  format: string; name: string; extensions: string[]; count: number; present_count: number;
  missing_count: number; total_size_bytes: number;
}

// ---------------------------------------------------------------- collections

/** A collection as the list routes send it (CollectionDto). Counts and length are in the
 *  project scope the request named (ADR-0043). */
export interface CollectionRow {
  id: string; name: string; description: string | null; notes: string | null;
  created_at: number; modified_at: number; project_ids: string[];
  cover_art_id: string | null; total_duration_seconds: number | null; project_count: number;
}

/** GET /collections/:id/statistics. */
export interface CollectionStats {
  project_count: number; total_duration_seconds: number | null; average_tempo: number | null;
  total_plugins: number; total_samples: number; total_tags: number;
  most_common_key: Key | null; most_common_time_signature: string | null;
}

/** A task in a collection's consolidated list: it names the project it belongs to. */
export interface CollectionTask {
  id: string; project_id: string; project_name: string; description: string;
  completed: boolean; created_at: number;
}

/** What the server sorts the list by (`sort_by`). */
export type CollectionSortKey = "name" | "project_count" | "total_duration" | "created_at" | "modified_at";

// ---------------------------------------------------------------- stats

/** The project scope the statistics count in (ADR-0045). */
export type StatsScope = "active" | "all";

/** GET /system/statistics: each count with its split (ADR-0045). Series come oldest first,
 *  empty periods as zero. */
export interface Statistics {
  projects: { total: number; active: number; archived: number };
  plugins: { total: number; installed: number; missing: number; not_scanned: number };
  samples: { total: number; present: number; missing: number };
  collections: { total: number; with_projects: number; empty: number };
  tags: { total: number; in_use: number; unused: number };
  tasks: { total: number; completed: number; pending: number; completion_rate: number };
  top_plugins: { name: string; vendor: string; usage_count: number }[];
  top_vendors: { vendor: string; plugin_count: number; usage_count: number }[];
  tempo_distribution: { tempo: number; count: number }[];
  key_distribution: { key: Key | null; count: number }[];
  time_signature_distribution: (TimeSignature & { count: number })[];
  projects_per_year: { year: number; count: number }[];
  projects_per_month: { year: number; month: number; count: number }[];
  average_monthly_projects: number;
  average_project_duration_seconds: number;
  projects_under_40_seconds: number;
  longest_project: Project | null;
  most_complex_projects: { project: Project; plugin_count: number; sample_count: number; complexity_score: number }[];
  average_plugins_per_project: number;
  average_samples_per_project: number;
  top_samples: { name: string; path: string; usage_count: number }[];
  top_tags: { name: string; usage_count: number }[];
  recent_activity: { year: number; month: number; day: number; projects_created: number; projects_modified: number }[];
  ableton_versions: { version: string; count: number }[];
  average_projects_per_collection: number;
  largest_collection: CollectionRow | null;
  task_completion_trends: { year: number; month: number; completed_tasks: number; total_tasks: number; completion_rate: number }[];
}

/** GET /config: the running config (ADR-0072). The fields marked "new" are added by the live-config
 *  work; until it lands the daemon omits them, so readers treat them as optional. */
export interface ConfigInfo {
  paths: string[];
  database_path: string | null;
  log_level: string;
  media_storage_dir: string;
  max_cover_art_size_mb: number | null;
  max_audio_file_size_mb: number | null;
  /** new: empty means the platform defaults. */
  vst_search_paths?: string[];
  /** new */
  vst_scan_timeout_secs?: number;
  /** new: read-only in the UI, it needs a restart. */
  http_port?: number;
  needs_setup: boolean;
  status_message: string;
}

/** GET /config/status. `reload_error` and `restart_required` are new (ADR-0072). */
export interface ConfigStatus {
  needs_setup: boolean;
  is_ready_for_operation: boolean;
  status_message: string;
  configured_paths_count: number;
  /** Why the last edit of config.toml was not applied, or null/absent when it was. */
  reload_error?: string | null;
  /** Keys whose file value differs from the running one, so a restart is needed. */
  restart_required?: string[];
}

/** POST /config/settings; every field optional, an absent one is left alone. */
export interface ConfigSettingsUpdate {
  database_path?: string;
  log_level?: string;
  media_storage_dir?: string;
  max_cover_art_size_mb?: number | null;
  max_audio_file_size_mb?: number | null;
  vst_search_paths?: string[];
  vst_scan_timeout_secs?: number;
}
