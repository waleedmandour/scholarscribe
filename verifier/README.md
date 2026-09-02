# ScholarScribe Provenance Verifier

A **standalone, offline HTML page** that verifies Writing Provenance export
packages produced by ScholarScribe v2.1.0+.

No installation, no build step, no server, no network calls: open
`index.html` in any modern browser. The JavaScript libraries it needs are
bundled in `vendor/` (JSZip, js-sha256, tweetnacl — all MIT-licensed). If the
`vendor/` folder is missing, the page falls back to CDN copies (that fallback
mode does require internet; the vendored mode does not).

## What it checks

1. **Hash chain integrity** — every session record's `record_hash` is
   recomputed from its own fields (canonical string, SHA-256) and every
   record must link to the previous record *by the same author* (or the
   genesis marker for an author's first session).
2. **Ed25519 signature** — the manifest's signature is verified over the
   canonical manifest payload using the author's public key. The verifier
   also checks that `sha256(public key)` equals the fingerprint embedded in
   the manifest, so the key and fingerprint cannot be mixed and matched.
3. **Document binding (optional)** — for a Word-sourced package, the
   verifier hashes `word/document.xml` from the original `.docx` and
   compares it to the manifest's `document_hash`. For a Google-Docs-sourced
   package, export the document as plain text (`.txt`) from Google and load
   it; the hash of that text is compared instead.

## How to use

1. Open `index.html` (double-click is fine).
2. Load the exported `.zip` package.
3. Paste the author's public key (`ed25519-pub:…`) or load the
   `…-signing-key.pub.json` file the author exported from the app.
4. Optionally, load the original `.docx` (or the exported `.txt`) to bind
   the package to the exact document.
5. Read the result panel.

## What a passing verification means — and what it does not

A passing check means: **the package is byte-for-byte intact since it was
produced by the holder of the signing key**, and its internal revision chain
is consistent.

It does **not** mean:

- that the text was written by a human (this is not an AI-detection score),
- that authorship is proven (it records the revision history the document
  carried — nothing more),
- that the history is complete (only edits made while Track Changes /
  version history was enabled are recorded),
- that the underlying timestamps are true (they come from the document's
  own metadata; a determined actor with the original file could hand-edit
  revision XML *before* analysis — treat this as one piece of evidence
  alongside drafts, notes and version history).

See `disclosure.txt` inside any package and `docs/PROVENANCE_SPEC.md` in the
repository for the full specification and limitations.

## Privacy

The page runs entirely client-side. The files you select never leave your
machine; there are no analytics, no fonts, no calls home.
