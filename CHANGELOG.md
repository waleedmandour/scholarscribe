# Changelog

All notable changes to ScholarScribe are documented in this file. The format
is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/) and the
project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

Statistics and citations carry "Last verified" dates inside the app. Dates
older than six months should prompt a re-check before relying on the figure
in a real submission. See [README.md](README.md) "Statistics and citations".

## [2.2.0] - 2026-09-29

Improvement sprint: sidebar regrouping, legible risk-profile passages,
strengthened statistics with source links and Last verified dates, scalable
font sizes, no em dashes anywhere in user-facing copy, and a two-target
macOS release build (Apple Silicon and Intel).

User-facing copy throughout the codebase, including the README, CHANGELOG,
USER_GUIDE, USER_MANUAL, docs/, source doc comments, and the standalone
verifier, no longer contains em dashes (U+2014) or en dashes (U+2013). A
grep across user-facing files returns zero matches.

### Added
- Sidebar tabs are now grouped into 7 workflow phases: Get started, Prepare
  the draft, Understand the draft, AI writing help, Authenticity and style,
  Evidence and compliance, Privacy and app. Detector Literacy appears
  before Risk Profile so the mechanism is explained before a score is
  shown. Writing Coach and Chat are adjacent. Style Fingerprint is adjacent
  to Style Analysis. Tab IDs, routes, and types are unchanged.
- Comfortable / Compact density toggle in the sidebar footer. Compact
  reduces sidebar and nav-item padding only; never changes font size.
  Persisted in localStorage key `scholarscribe-density`.
- A- / A / A+ font-scale control in the sidebar footer. Three steps: 0.9,
  1.0, 1.15. Sets `--font-scale` on `document.documentElement` and
  persists in localStorage key `scholarscribe-font-scale`. A pre-paint
  inline script in `index.html` applies the persisted value before first
  render to avoid a flash of default size.
- Risk Profile: click-to-expand detail panel beneath the heatmap,
  replacing the previous hover-only native tooltip. Each 40x40px block is
  now a `<button>` with a visible focus ring and an accessible aria-label
  (e.g. "Passage 3, medium risk, 200 words"). The panel shows the excerpt,
  word count, tier badge, plain-language reason, the spec's verbatim tier
  copy, a Jump to passage button, and a Copy reason button. Keyboard
  support: arrow keys move between blocks, Enter or Space opens the panel,
  Escape closes it.
- Risk Profile: four proxy signals per passage (perplexity, vocabulary
  uniformity, burstiness, sentence-length variance). The combined risk
  score averages all four. The plain-language reason is derived from
  whichever signal contributed most to the score. The reason never asserts
  AI authorship; describes surface patterns only.
- Risk Profile: `excerpt` and `reason` fields on every `SectionRisk`. The
  excerpt is the first ~15 words of the passage, sliced from the original
  text via `start_char..end_char`. The reason is a one-sentence plain-
  language cause derived from the dominant proxy signal.
- Risk Profile: Jump to passage button scrolls the user's textarea to the
  passage range via `setSelectionRange(start_char, end_char)` and briefly
  adds a border glow. Scoped to RiskProfiler's own textarea (no cross-tab
  jump) because each tab manages its own input.
- Risk Profile: Copy reason button copies the plain-language cause to the
  clipboard via `navigator.clipboard.writeText`.
- Detector Literacy: concrete figures for Liang et al. (2023) and
  Weber-Wulff et al. (2023), rendered via the new shared `VerifiedStat`
  component. Liang: 7 detectors, 91 TOEFL essays, 61.3% average
  false-positive rate, up to 97.8% peak. Weber-Wulff: 14 detectors, no
  tool exceeded 80% accuracy, only five exceeded 70%, six of 14 false
  positives, 13 of 14 false negatives.
- Risk Profile: Liang et al. (2023) figure added to the ethical-note
  callout via the same `VerifiedStat` component.
- Document Statistics: Nature word count updated from the stale
  "approximately 5,000 words" to a range, "2,500 to 4,300 words", with a
  source link to the Nature author formatting guide and a Last verified
  date.
- Document Statistics: ICMJE word count updated from a single hard number
  (3,500) to a range (3,000 to 6,000) with a caveat that ICMJE does not
  set word limits, a source link to the ICMJE Recommendations page, and a
  Last verified date.
- Document Statistics: journal-target comparison table now has a "Source"
  column with clickable links for venues that have a public author guide.
  Below the table, a `VerifiedStat` block per sourced venue shows the
  full figure with prominent source link and Last verified date.
- Shared `VerifiedStat` component (src/components/VerifiedStat.svelte)
  for rendering any statistic with a clickable source link and a "Last
  verified: YYYY-MM-DD" caption. Used by DetectorLiteracy, RiskProfiler,
  and DocumentStats.
- AI Text Cleaner: the existing `normalize_dashes` option is relabeled
  "Normalize dashes to plain hyphens" and its behavior is changed. It now
  converts `--`, em dash (U+2014), and en dash (U+2013) all to a single
  plain hyphen (U+002D). Previously it converted `--` to an em dash,
  which violated the project's no-em-dashes copy rule. The fix_mojibake
  option's mojibake-em-dash and mojibake-en-dash rules are also updated
  to produce plain hyphens.
- CONTRIBUTING.md: new "Copy style" section documenting the no-em-dashes
  rule, the number-range plain hyphen rule, the Unicode codepoint range
  plain hyphen rule, and the AI Text Cleaner's role in enforcing the rule
  for imported text. PR checklist updated with a "User-facing copy contains
  no em dashes" item.
- README.md: new "Statistics and citations" section explaining the
  `VerifiedStat` pattern and the six-month re-check recommendation.
- README.md: new "Scalable font sizes and density toggle" feature in the
  features list.
- CHANGELOG.md: this file.

### Changed
- `:root` font-size in `public/app.css` moves from hard-coded 15px to
  `var(--font-md)` (16px). `line-height` moves from 1.6 to
  `var(--line-body)` (1.5). Both are intentional accessibility
  improvements: 16px is the WCAG-recommended floor for body text, 1.5 is
  the WCAG-recommended line-height minimum.
- Every hard-coded px font-size in `public/app.css` and all `.svelte`
  files (both `<style>` blocks and inline `style=` attributes) has been
  replaced with one of the new CSS variables (`--font-sm` 14px,
  `--font-md` 16px, `--font-lg` 20px, `--font-xl` 24px, `--font-stat`
  28px, `--font-stat-xl` 34px at scale 1.0). After this sweep, no
  `.svelte` file contains a hard-coded px font-size. Body text meets the
  16px floor at scale 1.0; labels and tabs meet the 14px floor at scale
  1.0.
- Sidebar tabs reordered to match the new grouping. Tab IDs, routes, and
  types are unchanged; only presentation changes.
- `RiskProfile.section_profiles` in `src/lib/api.ts` adds
  `vocabulary_uniformity_proxy`, `sentence_length_variance_proxy`,
  `excerpt`, and `reason` fields.
- `JournalComparison` in `src-tauri/src/document_stats.rs` and
  `src/lib/api.ts` changes from a single `typical_word_count: usize` to
  `min_word_count` + `max_word_count` + `source_url` + `source_label` +
  `last_verified`. Venues that publish a single hard limit have
  `min_word_count == max_word_count` and empty source fields. The
  `status` logic is updated: for ranged venues, "near" means inside the
  range; for single-number venues, the original "within 10%" tolerance
  rule is preserved. `delta` now measures distance from the nearest
  endpoint of the range (0 means inside the range).
- Em dashes (U+2014) and en dashes (U+2013) removed from all user-facing
  copy across 39 files. Rewrites use a period, comma, colon, or
  parentheses per the surrounding sentence. Number ranges use plain
  hyphens (e.g. "12-16"). Unicode codepoint ranges in source use plain
  hyphens (e.g. "U+FE00-FE0F").
- README.md "20 tools" lines updated to reflect the 7-group organization.
- README.md "Comparison panel" line updated with the new Nature and ICMJE
  ranges and Last verified dates.
- README.md "Risk Profile tab" section expanded to describe the new
  detail panel, four proxy signals, Jump to passage, Copy reason, and
  keyboard support.
- macOS release build now produces two `.dmg` files (Apple Silicon and
  Intel) instead of one. The CI matrix adds `macos-13` (Intel runner)
  alongside `macos-latest` (Apple Silicon). See "Releases" below.

### Fixed
- `split_into_passages` in `src-tauri/src/risk_profiler.rs` previously
  used `text.find(first_word)` to compute `start_char`, which returned
  the first occurrence of that word anywhere in the document rather than
  the passage's actual location. Fixed to walk the text with
  `char_indices()` and track real character offsets. This fix makes the
  new `excerpt` and the existing `Jump to passage` feature both correct.

### Releases
- macOS: two `.dmg` files (Apple Silicon and Intel) per approved decision
  1 = C. The CI matrix adds `macos-13` for Intel-native builds alongside
  `macos-latest` for Apple Silicon-native builds. The cargo cache key is
  now `${runner.os}-${runner.arch}-cargo-${hash}` to prevent Intel and
  ARM caches from colliding.
- Windows: `.msi` and `.exe` installers (unchanged).
- Linux: `.deb` and `.AppImage` (unchanged).
- Code signing: still unsigned, matching the existing workflow. Signing
  is a future task that requires Apple Developer ID and Windows
  certificate secrets to be provisioned in GitHub.

### Verification
- `npm run check`: 0 errors, 11 pre-existing warnings (baseline).
- `npm run build`: success.
- `cargo fmt --check`: clean.
- `cargo check` and `cargo clippy -- -D warnings`: run by CI's check-rust
  job (Linux sandbox here lacks Tauri's webkit system deps; CI is
  authoritative).
- `rg '[\u2014\u2013]' src/ src-tauri/src/ public/ docs/ *.md *.html
  verifier/`: zero matches.

### Decisions
The following judgment calls were made during this sprint, with the
reasoning recorded here for future maintainers:

1. macOS build target: chose (C) two separate `.dmg` files (Apple Silicon
   and Intel) instead of (A) one universal binary, because universal
   builds roughly double build time and produce a much larger `.dmg` for
   a niche use case. Two separate files are easier to debug and ship.
2. Code signing: ship unsigned, matching the existing workflow.
3. AI Text Cleaner dash handling: chose (A) to merge the existing
   `normalize_dashes` option with the new em-dash-stripping rule into
   one "Normalize dashes to plain hyphens" toggle, rather than keep two
   separate toggles that would fight each other.
4. Reason field proxies: chose (B) to track four sub-signals (perplexity,
   vocabulary uniformity, burstiness, sentence-length variance) rather
   than two. This required refactoring `PassageMetrics` and re-tuning
   `classify_risk` to average all four signals, but gives finer-grained
   reasons for the user.
5. Journal comparison type: chose (A) to change the `JournalComparison`
   struct to expose `min_word_count`, `max_word_count`, `source_url`,
   `source_label`, and `last_verified` rather than keep the single
   `typical_word_count` and hardcode ranges in the UI. This is a single
   source of truth and future-proofs other venues that may need ranges
   later.
6. Em dash sweep scope: rewrote em dashes in all user-facing text plus
   all source doc comments (because they surface in IDE hovers and
   generated docs). Left vendored third-party code (`package-lock.json`,
   `Cargo.lock`, `verifier/vendor/`) untouched to avoid unreviewable
   diffs.
7. Body font baseline: accepted the bump from 15px to 16px to meet the
   WCAG-recommended body text floor.
8. Shared `VerifiedStat` component: added to enforce the "every
   statistic carries a source link and Last verified date" rule
   structurally rather than per page.
9. `cargo clippy`: intended to flip from `continue-on-error: true` to
   fail the build, but the codebase carries approximately 21 pre-existing
   clippy warnings across 9 source files (commands.rs, document_stats.rs,
   docx_reading.rs, persistence.rs, structure_analyzer.rs, text_cleaner.rs,
   voice_consistency.rs, writing_journal.rs, plus one in risk_profiler.rs
   that was fixed). Lints include manual_inspect, if_same_then_else,
   useless_format, collapsible_if, unnecessary_sort_by,
   manual_pattern_char_comparison, regex_creation_in_loops, manual_strip,
   let_and_return. Fixing all of them is a separate refactor sprint.
   For this sprint, the clippy step keeps `continue-on-error: true` with a
   code comment pointing at this CHANGELOG entry. Cargo fmt --check and
   cargo check still fail the build; clippy is informational only.
10. `Jump to passage`: scoped to RiskProfiler's own textarea (no
    cross-tab jump) because each tab manages its own input. A
    cross-editor jump would require a shared editor instance and is a
    larger architecture change for a later sprint.

## [2.1.0] - previous release

See git history for the 2.1.0 release notes.
