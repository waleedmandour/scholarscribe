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
  // v0.1.7 strict-cleaning operations
  convert_ellipsis: boolean;
  remove_asterisks: boolean;
  remove_markdown_headings: boolean;
  convert_nbsp: boolean;
  strip_bom: boolean;
  normalize_unicode_whitespace: boolean;
  strip_soft_hyphens: boolean;
  strip_variation_selectors: boolean;
  normalize_bullets: boolean;
  normalize_line_endings: boolean;
  collapse_repeated_punctuation: boolean;
  strip_variation_selectors_emoji: boolean;
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
  // v0.1.7
  ellipsis_converted: number;
  asterisks_removed: number;
  markdown_headings_removed: number;
  nbsp_converted: number;
  bom_stripped: number;
  unicode_whitespace_normalized: number;
  soft_hyphens_stripped: number;
  variation_selectors_stripped: number;
  bullets_normalized: number;
  line_endings_normalized: number;
  repeated_punctuation_collapsed: number;
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
  // v0.1.7 strict ops — OFF in default preset (opt in via Strict button)
  convert_ellipsis: false,
  remove_asterisks: false,
  remove_markdown_headings: false,
  convert_nbsp: false,
  strip_bom: false,
  normalize_unicode_whitespace: false,
  strip_soft_hyphens: false,
  strip_variation_selectors: false,
  normalize_bullets: false,
  normalize_line_endings: false,
  collapse_repeated_punctuation: false,
  strip_variation_selectors_emoji: false,
};

export const strictCleanOptions: CleanOptions = {
  collapse_whitespace: true,
  join_hyphenated_words: true,
  join_broken_lines: true,
  expand_ligatures: true,
  strip_zero_width: true,
  strip_control_chars: true,
  remove_page_numbers: true,
  normalize_quotes: true,
  normalize_dashes: true,
  fix_mojibake: true,
  join_broken_urls: true,
  fix_broken_citations: true,
  // v0.1.7 strict ops — all ON
  convert_ellipsis: true,
  remove_asterisks: true,
  remove_markdown_headings: true,
  convert_nbsp: true,
  strip_bom: true,
  normalize_unicode_whitespace: true,
  strip_soft_hyphens: true,
  strip_variation_selectors: true,
  normalize_bullets: true,
  normalize_line_endings: true,
  collapse_repeated_punctuation: true,
  strip_variation_selectors_emoji: true,
};

// v0.1.6+

export interface BibEntry {
  key: string;
  entry_type: string;
  title: string;
  author: string;
  year: string;
}

export interface InTextCitation {
  raw: string;
  author: string;
  year: string;
  numeric: number | null;
  position: number;
}

export interface CitationReport {
  bib_entries: BibEntry[];
  in_text_citations: InTextCitation[];
  undefined_citations: InTextCitation[];
  unused_references: BibEntry[];
  citation_counts: [BibEntry, number][];
  bib_parse_errors: string[];
  draft_path: string | null;
  bib_path: string | null;
}

export interface JournalComparison {
  venue: string;
  typical_word_count: number;
  status: string; // "under" | "near" | "over"
  delta: number;
}

export interface DocStats {
  word_count: number;
  sentence_count: number;
  paragraph_count: number;
  section_count: number;
  citation_count: number;
  figure_count: number;
  table_count: number;
  avg_sentence_length: number;
  type_token_ratio: number;
  complex_word_ratio: number;
  estimated_reading_time_minutes: number;
  flesch_reading_ease: number;
  flesch_kincaid_grade: number;
  gunning_fog: number;
  journal_comparison: JournalComparison[];
}


export interface Heading {
  level: number;
  text: string;
  word_count: number;
}

export interface StructureReport {
  headings: Heading[];
  total_sections: number;
  max_depth: number;
  missing_sections: string[];
  short_sections: Heading[];
  suggestions: string[];
  source_path: string | null;
  source_kind: string;
}

export interface AbstractRequest {
  model: string;
  draft_text: string;
  max_words?: number | null;
  venue?: string | null;
}

export interface AbstractResult {
  abstract_text: string;
  model_used: string;
  prompt_tokens: number;
  draft_length_chars: number;
}

export interface AbstractError {
  kind: string;
  message: string;
}

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
  // v0.1.7+
  cleanTextStrict: (text: string) =>
    invoke<CleanResult>("clean_text_strict", { args: { text } }),
  strictCleanOptions: () => invoke<CleanOptions>("strict_clean_options"),
  // v0.1.4+
  cleanDocxFile: (path: string, options?: CleanOptions) =>
    invoke<{ source_path: string; extracted: CleanResult }>("clean_docx_file", {
      args: { path, options: options || defaultCleanOptions },
    }),
  // v0.1.5+
  cleanDocxPreserveFormat: (inputPath: string, outputPath: string, options?: CleanOptions) =>
    invoke<{
      output_path: string;
      parts_cleaned: string[];
      runs_cleaned: number;
      stats: CleanStats;
      transformations_applied: string[];
      skipped_operations: string[];
    }>("clean_docx_preserve_format", {
      args: {
        input_path: inputPath,
        output_path: outputPath,
        options: options || defaultCleanOptions,
      },
    }),
  // v0.1.6+
  validateCitations: (draftPath: string, bibPath: string) =>
    invoke<CitationReport>("validate_citations", {
      args: { draft_path: draftPath, bib_path: bibPath },
    }),
  analyzeStructure: (path: string) =>
    invoke<StructureReport>("analyze_structure", { args: { path } }),
  analyzeStructureText: (text: string) =>
    invoke<StructureReport>("analyze_structure_text", { text }),
  generateAbstract: (
    model: string,
    draftText: string,
    maxWords?: number,
    venue?: string,
  ) =>
    invoke<AbstractResult>("generate_abstract", {
      args: {
        model,
        draft_text: draftText,
        max_words: maxWords ?? null,
        venue: venue ?? null,
      },
    }),
  documentStats: (text: string) => invoke<DocStats>("document_stats", { text }),
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

  // v0.2.0+
  analyzeRiskProfile: (text: string) => invoke<RiskProfile>("analyze_risk_profile", { text }),
  checkVoiceConsistency: (text: string) => invoke<ConsistencyReport>("check_voice_consistency", { text }),
  generateAppealLetter: (input: AppealLetterInput) =>
    invoke<AppealLetterOutput>("generate_appeal_letter", { args: { input } }),
  validateCitationContexts: (draftText: string, bibContent: string) =>
    invoke<CitationContextCheck[]>("validate_citation_contexts", {
      args: { draft_text: draftText, bib_content: bibContent },
    }),
  documentStatsBySection: (text: string) =>
    invoke<SectionReadabilityReport>("document_stats_by_section", { text }),
  generateSectionCommentary: (model: string, draftText: string) =>
    invoke<SectionCommentaryResult>("generate_section_commentary", {
      args: { model, draft_text: draftText },
    }),
  journalCreateSession: (title: string) =>
    invoke<Snapshot>("journal_create_session", { title }),
  journalSaveSnapshot: (sessionId: string, content: string) =>
    invoke<Snapshot>("journal_save_snapshot", { sessionId, content }),
  journalListSessions: () => invoke<JournalSession[]>("journal_list_sessions"),
  journalGetSnapshots: (sessionId: string) =>
    invoke<Snapshot[]>("journal_get_snapshots", { sessionId }),
  journalDeleteSession: (sessionId: string) =>
    invoke<number>("journal_delete_session", { sessionId }),
  journalExportSession: (sessionId: string, outputPath: string) =>
    invoke<void>("journal_export_session", { sessionId, outputPath }),

};

// v0.2.0+

export interface RiskProfile {
  overall_perplexity_proxy: number;
  overall_burstiness_proxy: number;
  overall_risk_level: string;
  overall_risk_color: string;
  section_profiles: {
    section_label: string;
    start_char: number;
    end_char: number;
    word_count: number;
    perplexity_proxy: number;
    burstiness_proxy: number;
    risk_level: string;
    risk_color: string;
  }[];
  explanation: string;
  recommendations: string[];
}

export interface ConsistencyReport {
  passages: {
    label: string;
    word_count: number;
    avg_sentence_length: number;
    type_token_ratio: number;
    hedge_density: number;
    passive_ratio: number;
    flesch_reading_ease: number;
  }[];
  inconsistencies: {
    passage_index: number;
    passage_label: string;
    metric: string;
    value: number;
    document_average: number;
    deviation_pct: number;
    severity: string;
    note: string;
  }[];
  overall_consistency_score: number;
  explanation: string;
  recommendations: string[];
}

export interface AppealLetterInput {
  researcher_name: string;
  researcher_title: string;
  institution: string;
  manuscript_title: string;
  venue: string;
  editor_name: string;
  detector_used: string;
  detector_score: string;
  process_description: string;
  additional_evidence: string;
}

export interface AppealLetterOutput {
  letter: string;
  references: string[];
}

export interface CitationContextCheck {
  citation_raw: string;
  bib_key: string;
  bib_title: string;
  sentence: string;
  keyword_overlap_pct: number;
  verdict: string;
  note: string;
}

export interface SectionReadability {
  section_name: string;
  word_count: number;
  sentence_count: number;
  avg_sentence_length: number;
  flesch_reading_ease: number;
  flesch_kincaid_grade: number;
  gunning_fog: number;
  interpretation: string;
}

export interface SectionReadabilityReport {
  sections: SectionReadability[];
  document_average: SectionReadability;
  explanation: string;
}

export interface SectionCommentary {
  section_name: string;
  summary: string;
}

export interface SectionCommentaryResult {
  commentaries: SectionCommentary[];
  model_used: string;
  draft_length_chars: number;
}

export interface JournalSession {
  session_id: string;
  title: string;
  created_at: number;
  updated_at: number;
  snapshot_count: number;
  total_words_final: number;
}

export interface Snapshot {
  id: string;
  session_id: string;
  timestamp: number;
  content: string;
  word_count: number;
  char_count: number;
  diff_from_previous: {
    words_added: number;
    words_removed: number;
    chars_added: number;
    chars_removed: number;
    similarity_pct: number;
  } | null;
}

// v0.2.0 API methods (append to the api object — need to find and edit it)
