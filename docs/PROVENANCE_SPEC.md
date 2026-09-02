# Writing Provenance — Technical Specification (v2.1.0)

Status: **implemented** (Phase 1 + Phase 1.5) · Version: manifest format **1.0**

Writing Provenance produces a **cryptographically signed, hash-chained record
of a document's real revision history** — from Microsoft Word's Track Changes
(Phase 1) or Google Docs' version history (Phase 1.5) — so an author can
*offer* verifiable evidence of their writing process.

This document specifies exactly what is produced and how it is verified. It
is the normative reference for `src-tauri/src/provenance.rs` and
`verifier/verify.js`. **Any change to the canonical formats below must update
the Rust implementation, the JavaScript verifier, and this file in the same
commit.**

---

## 0. Design principles (non-negotiable)

1. **Evidence, not verdict.** Nothing in this feature is an AI-detection
   score, a humanness percentage, or a "verified human" badge. The style
   statistic is descriptive, with interpretation bands — never pass/fail.
2. **No fabrication.** Every number comes from revision history the
   document itself carries. ScholarScribe never generates or edits history
   (docs/ETHICS.md §2.3).
3. **Local-first / privacy-preserving.** Phase 1 performs zero network
   calls. The manifest contains hashes, counts, timestamps, author display
   names — **never document text**. Phase 1.5's Google import downloads
   revision text into memory for diffing and discards it; every outbound
   call is recorded in the in-app privacy audit log.
4. **Opt-in.** The feature is off until the user accepts the disclosure
   dialog (persisted as `provenance_enabled` in settings.json).
5. **Independent verification.** The `verifier/` folder is a standalone
   offline HTML page; no installation, no ScholarScribe, no network needed.

## 1. Components

| Component | Location | Role |
|---|---|---|
| Core (pure) | `src-tauri/src/provenance.rs` | grouping, hashing, signing, zip builder |
| .docx reader | `src-tauri/src/docx_reading.rs` | `w:ins`/`w:del` extraction, `document_hash` |
| Command layer | `src-tauri/src/provenance_commands.rs` | keychain key, opt-in gate, export, audit |
| Google bridge | `src-tauri/src/google_docs*.rs` | OAuth (PKCE, loopback), revisions → same pipeline |
| UI | `src/components/Provenance*.svelte` | tab, disclosure dialog, timeline, export |
| Verifier | `verifier/` | offline HTML/JS verification |

## 2. Data flow

```
.docx ──(w:ins / w:del)──▶ RawRevision[] ─┐
                                          ├─▶ group_into_sessions ─▶ RawSession[]
Google revisions ──(prefix/suffix diff)──▶ RawRevision[] ─┘        │
                                                                   ▼
                                              SessionRecord[] (hash chain, per author)
                                                                   │
                                              ProvenanceManifest (Ed25519-signed)
                                                                   │
                                        .zip { manifest.json, disclosure.txt,
                                               style_analysis.json,
                                               citation_validation.json, README.txt }
```

## 3. Inputs

### 3.1 .docx (Phase 1)

* `word/document.xml` is parsed (quick-xml). `w:ins` elements contribute
  `<w:t>` text as **insertions**; `w:del` elements contribute `<w:delText>`
  as **deletions**. Formatting-only changes (`w:rPrChange`, `w:pPrChange`)
  are ignored. `w:author` and `w:date` are taken verbatim from the markup.
* **`document_hash`** = `sha256:` + SHA-256 hex of the raw
  `word/document.xml` bytes — so a verifier holding the original `.docx`
  can bind manifest to document.
* Edge cases: no `w:ins`/`w:del` at all → error "No tracked changes
  found…". Markup present but no usable revisions → error "Track Changes
  is on, but no revisions were recorded…". Unparseable XML → "Could not
  parse Track Changes data…". Revisions with unparseable dates are skipped
  and reported (`skipped_unparsable_dates`).

### 3.2 Google Docs (Phase 1.5)

* OAuth 2.0 installed-app flow: PKCE (S256), loopback redirect on
  `http://127.0.0.1:{random port}`, scope **`drive.readonly`** only.
  The system browser is used (never the webview) — the CSP is therefore
  unchanged. The refresh token is stored in the OS keychain
  (`scholarscribe` / `google-refresh-token-v1`).
* Revisions listed via `GET /drive/v3/files/{id}/revisions`; text fetched
  per revision (Drive `alt=media`, falling back to the legacy
  docs.google.com plain-text export endpoint). Capped at 500 revisions.
* Each consecutive snapshot pair is diffed (common prefix/suffix, char
  level): pure insertions/deletions map directly; replacements produce a
  deletion followed by an insertion at the same timestamp, attributed to
  the revision's `lastModifyingUser.displayName` (else
  `Unknown (Google Drive)`).
* **`document_hash`** = `sha256:` + SHA-256 of the final revision's text.
  A verifier can recompute it from a plain-text export of the current
  document (byte-identical export assumed).

## 4. Session grouping

* Revisions are stably sorted by timestamp (ties keep markup order).
* Consecutive revisions by the **same author** within **30 minutes**
  (`SESSION_GAP_SECS = 1800`) extend that author's open session.
* Different authors never share a session; each author owns an independent
  session sequence and hash chain.
* A gap of more than **7 days** (`ANOMALY_GAP_SECS = 604800`) between
  consecutive sessions of one author, or a revision timestamped in the
  future, is reported in `anomalies` (informational, not an accusation).

## 5. Canonical forms and hashes

### 5.1 Snapshot hash (per session)

```
scholarscribe-session-v1 \n
{author} \n {start_time} \n {end_time} \n
for each revision (chronological): {kind} \n {date} \n {byte_len}:{text}
```
→ SHA-256 → `sha256:{hex}`. `{kind}` is `insertion`/`deletion`; `{byte_len}`
is the UTF-8 byte length of `{text}` (length-prefixing defeats concatenation
ambiguity). **The text itself never leaves memory** — only this hash is
published.

### 5.2 Record hash (per session)

```
scholarscribe-record-v1 \n
{session_id} \n {author} \n {start_time} \n {end_time} \n
{snapshot_hash} \n {chars_added} \n {chars_removed} \n {largest_insertion} \n
{prev_record_hash}
```
→ SHA-256 → `sha256:{hex}`. `prev_record_hash` is the previous record by
the **same author**, or the genesis marker
`sha256:000…000` (64 zeros) for that author's first record.
`session_id` is a random UUID v4 (not derived from content).

## 6. Manifest and signature

```json
{
  "version": "1.0",
  "document_hash": "sha256:…",
  "author_key_fingerprint": "ed25519:sha256(pubkey)hex",
  "sessions": [ { "session_id", "author", "start_time", "end_time",
                  "snapshot_hash", "chars_added", "chars_removed",
                  "largest_insertion", "prev_record_hash", "record_hash" } ],
  "style_consistency": { "distance_score", "baseline_source",
                         "metrics_compared", "interpretation" },
  "signature": "ed25519:hex",
  "generated_at": unix_seconds,
  "generator": "scholarscribe-2.1.0",
  "revision_count": n
}
```

**Signing.** The Ed25519 key is generated on first export; the 32-byte seed
is stored in the OS keychain (`scholarscribe` / `provenance-signing-key-v1`).
The fingerprint is `ed25519:` + SHA-256 hex of the 32-byte public key.

The signature covers the **canonical manifest payload** — a deterministic
string derived from the manifest's values, *not* the JSON file bytes (this
avoids float-formatting and key-ordering ambiguity between Rust and JS):

```
SCHOLARSCRIBE-MANIFEST-v1 \n
{version} \n {document_hash} \n {author_key_fingerprint} \n
{generated_at} \n {generator} \n {revision_count} \n
for each session (in order):
  {session_id}|{author}|{start_time}|{end_time}|{snapshot_hash}|
  {chars_added}|{chars_removed}|{largest_insertion}|{prev_record_hash}|
  {record_hash} \n
{distance_score formatted %.4f, or the literal string null} \n
{baseline_source} \n {metrics_compared joined with ","} \n
```

The signature is `ed25519:{hex}` over the UTF-8 bytes of that payload.
`verifier/verify.js` rebuilds the payload with `Number(score).toFixed(4)`
and verifies with tweetnacl; byte-equality with the Rust builder is covered
by an automated cross-language test (fixture + Node script).

## 7. Style consistency (descriptive only)

`distance_score = 1 − cosine similarity` between 12 style.rs metrics of the
baseline and the final text (rounded to 4 decimals): avg_sentence_length,
sentence_length_stdev, type_token_ratio, avg_paragraph_length, passive_ratio,
hedge_density, connector_density, first_person_singular_ratio,
first_person_plural_ratio, citation_density, flesch_reading_ease,
gunning_fog. Raw counts are excluded (they scale with length, not style).

* Baseline: a reference text the user supplies (`user_reference_text`) or
  the earliest tracked session's inserted text (`first_tracked_session`);
  with no usable sample the score is `null` (`baseline_source:
  "unavailable"`).
* Interpretation bands (reported as text, never pass/fail):
  0.0–0.2 very close · 0.2–0.4 broadly consistent · 0.4–0.6 noticeable ·
  0.6–1.0 substantially different.
* **This is not an AI-detection score.** It measures drift between two
  samples of writing; it cannot attribute text to humans or machines.

## 8. Export bundle (.zip)

`manifest.json` · `disclosure.txt` · `style_analysis.json` ·
`citation_validation.json` · `README.txt`. All contents are hashes, counts,
timestamps, author display names and aggregate metrics — no document text.
`citation_validation.json` is an inventory (counts of author-year/numeric
citation-like patterns), explicitly not a validity check.

## 9. Verification procedure (mirrored by verifier/)

1. Re-hash every record (§5.2) → `chain_intact` + anomalies.
2. Check per-author linkage (§5.2) — first record must equal genesis.
3. Rebuild the canonical payload (§6) and verify the Ed25519 signature
   with the author's public key; check `sha256(pubkey) = fingerprint`.
4. Optionally bind the document: SHA-256 of `word/document.xml` (docx) or
   of the plain-text export (Google) vs `document_hash`.

## 10. Threat model & honest limitations

* The signature proves **package integrity and producer**, not the truth of
  the timestamps inside. Timestamps come from the document's own metadata.
* A determined actor with access to the original file could hand-edit
  revision XML **before** analysis; provenance captures what the document
  carried at analysis time. Use alongside drafts, notes, version history.
* Only tracked edits are visible; untracked edits leave no evidence.
* Author names are included as recorded by the editor (part of the
  evidence); users should review them before sharing packages.
* Google metadata fidelity varies; the manifest records the source used.

## 11. Audit requirements

* Phase 1 pipeline: **zero** outbound HTTP calls. Any future change must
  be reflected in the privacy audit log, SECURITY.md and this spec.
* Phase 1.5 logs every outbound call (token, list, per-revision fetch) to
  the in-app audit log with exact URLs.
