// Thin typed wrapper around the Tauri `invoke` bridge.
// Every call corresponds to a `#[tauri::command]` in src-tauri/src/commands.rs.

import { invoke } from "@tauri-apps/api/core";
import { listen, type UnlistenFn } from "@tauri-apps/api/event";

export interface ModelInfo {
  name: string;
  size: number;
  digest: string;
  modified_at: string;
}

export interface RecommendedModel {
  name: string;
  label: string;
  size_gb: number;
  min_ram_gb: number;
  tags: string[];
  description: string;
}

export interface ChatMessage {
  role: string;
  content: string;
}

export interface ChatRequest {
  model: string;
  messages: ChatMessage[];
  temperature?: number;
}

export interface StyleProfile {
  word_count: number;
  sentence_count: number;
  avg_sentence_length: number;
  sentence_length_stdev: number;
  type_token_ratio: number;
  avg_paragraph_length: number;
  passive_ratio: number;
  hedge_density: number;
  connector_density: number;
  first_person_singular_ratio: number;
  first_person_plural_ratio: number;
  citation_density: number;
  // v0.1.1+
  flesch_reading_ease: number;
  flesch_kincaid_grade: number;
  gunning_fog: number;
  avg_syllables_per_word: number;
  complex_word_ratio: number;
}

export interface FeatureDistance {
  feature: string;
  draft_value: number;
  reference_value: number;
  relative_diff_pct: number;
  interpretation: string;
}

export interface StyleComparison {
  draft: StyleProfile;
  reference: StyleProfile;
  overall_distance: number;
  feature_distances: FeatureDistance[];
  notes: string[];
}

export interface VenueTemplate {
  id: string;
  label: string;
  policy_url: string;
  requires_in_manuscript: boolean;
  requires_in_cover_letter: boolean;
  template: string;
  notes: string;
}

export interface DisclosureInput {
  venue_id: string;
  tool_name: string;
  task_description: string;
  model_used?: string | null;
  author_name?: string | null;
}

export interface DisclosureOutput {
  venue: VenueTemplate;
  statement: string;
  where_to_include: string;
  warnings: string[];
}

export interface PullProgress {
  model: string;
  status: string;
  completed: number;
  total: number;
}

// v0.1.1+

export interface SystemInfo {
  total_ram_gb: number;
  available_ram_gb: number;
  cpu_brand: string;
  cpu_cores: number;
  os_name: string;
}

export interface GgufCompatResult {
  file_path: string;
  file_size_gb: number;
  recommended_ram_gb: number;
  total_ram_gb: number;
  available_ram_gb: number;
  verdict: string; // "ok" | "tight" | "insufficient"
  message: string;
}

export interface AuditEntry {
  timestamp: number;
  kind: string;
  target: string;
  detail: string;
  bytes_in: number;
  bytes_out: number;
}

export interface AuditSummary {
  total_events: number;
  file_reads: number;
  http_calls: number;
  ollama_commands: number;
  bytes_in: number;
  bytes_out: number;
  outbound_hosts: string[];
}

// v0.1.3+

export interface CleanOptions {
  collapse_whitespace: boolean;
  join_hyphenated_words: boolean;
  join_broken_lines: boolean;
  expand_ligatures: boolean;
  strip_zero_width: boolean;
  strip_control_chars: boolean;
  remove_page_numbers: boolean;
  normalize_quotes: boolean;
  normalize_dashes: boolean;
  fix_mojibake: boolean;
  join_broken_urls: boolean;
  fix_broken_citations: boolean;
}

export interface CleanStats {
  whitespace_collapsed: number;
  line_breaks_joined: number;
  hyphenated_words_joined: number;
  ligatures_expanded: number;
  zero_width_chars_stripped: number;
  control_chars_stripped: number;
  page_numbers_removed: number;
  quotes_normalized: number;
  dashes_normalized: number;
  mojibake_fixed: number;
  urls_joined: number;
  citations_fixed: number;
}

export interface CleanResult {
  cleaned: string;
  original_length: number;
  cleaned_length: number;
  transformations_applied: string[];
  stats: CleanStats;
}

export interface Settings {
  persistence_enabled: boolean;
  theme: string;
  version: string;
}

export interface Draft {
  id: string;
  title: string;
  kind: string;
  content: string;
  created_at: number;
  updated_at: number;
}

export interface DraftMeta {
  id: string;
  title: string;
  kind: string;
  created_at: number;
  updated_at: number;
  size_bytes: number;
}

export const defaultCleanOptions: CleanOptions = {
  collapse_whitespace: true,
  join_hyphenated_words: true,
  join_broken_lines: true,
  expand_ligatures: true,
  strip_zero_width: true,
  strip_control_chars: true,
  remove_page_numbers: true,
  normalize_quotes: false,
  normalize_dashes: true,
  fix_mojibake: true,
  join_broken_urls: true,
  fix_broken_citations: true,
};

export const api = {
  appInfo: () => invoke<Record<string, unknown>>("app_info"),
  ollamaStatus: () => invoke<boolean>("ollama_status"),
  ollamaListModels: () => invoke<ModelInfo[]>("ollama_list_models"),
  ollamaPullModel: (name: string) => invoke<void>("ollama_pull_model", { name }),
  ollamaDeleteModel: (name: string) => invoke<void>("ollama_delete_model", { name }),
  ollamaChat: (request: ChatRequest) => invoke<ChatMessage>("ollama_chat", { request }),
  recommendedModels: () => invoke<RecommendedModel[]>("recommended_models"),
  readTextFile: (path: string) => invoke<string>("read_text_file", { args: { path } }),
  analyzeStyle: (text: string) => invoke<StyleProfile>("analyze_style", { text }),
  compareStyle: (draft: StyleProfile, reference: StyleProfile) =>
    invoke<StyleComparison>("compare_style", { draft, reference }),
  listVenueTemplates: () => invoke<VenueTemplate[]>("list_venue_templates"),
  generateDisclosure: (input: DisclosureInput) =>
    invoke<DisclosureOutput>("generate_disclosure", { input }),
  // v0.1.1+
  systemInfo: () => invoke<SystemInfo>("system_info"),
  checkGgufCompatibility: (path: string) =>
    invoke<GgufCompatResult>("check_gguf_compatibility", { path }),
  ollamaImportGguf: (path: string, modelName: string) =>
    invoke<void>("ollama_import_gguf", { args: { path, model_name: modelName } }),
  auditList: () => invoke<AuditEntry[]>("audit_list"),
  auditClear: () => invoke<void>("audit_clear"),
  auditSummary: () => invoke<AuditSummary>("audit_summary"),
  // v0.1.3+
  cleanText: (text: string, options?: CleanOptions) =>
    invoke<CleanResult>("clean_text", { args: { text, options: options || defaultCleanOptions } }),
  // v0.1.4+
  cleanDocxFile: (path: string, options?: CleanOptions) =>
    invoke<{ source_path: string; extracted: CleanResult }>("clean_docx_file", {
      args: { path, options: options || defaultCleanOptions },
    }),
  settingsGet: () => invoke<Settings>("settings_get"),
  settingsSet: (settings: Settings) => invoke<void>("settings_set", { settings }),
  persistenceEnable: () => invoke<void>("persistence_enable"),
  persistenceDisable: () => invoke<void>("persistence_disable"),
  persistenceStatus: () => invoke<boolean>("persistence_status"),
  draftSave: (title: string, kind: string, content: string) =>
    invoke<Draft>("draft_save", { title, kind, content }),
  draftUpdate: (id: string, title?: string, content?: string) =>
    invoke<Draft>("draft_update", { id, title, content }),
  draftLoad: (id: string) => invoke<Draft>("draft_load", { id }),
  draftList: () => invoke<DraftMeta[]>("draft_list"),
  draftDelete: (id: string) => invoke<void>("draft_delete", { id }),
  draftDeleteAll: () => invoke<number>("draft_delete_all"),
  dataDirPath: () => invoke<string>("data_dir_path"),
  onPullProgress: (cb: (p: PullProgress) => void): Promise<UnlistenFn> =>
    listen<PullProgress>("ollama://pull-progress", (e) => cb(e.payload)),
  onPullStart: (cb: (name: string) => void): Promise<UnlistenFn> =>
    listen<string>("ollama://pull-start", (e) => cb(e.payload)),
  onPullEnd: (cb: (info: { model: string; ok: boolean }) => void): Promise<UnlistenFn> =>
    listen<{ model: string; ok: boolean }>("ollama://pull-end", (e) => cb(e.payload)),
};
