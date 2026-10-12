# ScholarScribe Agent Observations

Out-of-scope items noticed during milestone work, logged per N7 (scope discipline). None of these were changed without a maintainer decision. Items move out of this file when they are picked up by a milestone.

## Added in M0 (2026-10-13)

1. **`style_fingerprint.rs:137` hardcodes `"generated": "2026"`** in the JSON export. Every 2027 export will carry a stale year. One-line fix candidate (use the actual clock); deferred to keep M0 behavior-free.
2. **README.md:16 arithmetic**: "24 rule-based transformations (12 default + 11 strict)" sums to 23, while README.md:163 says "Strict (24 ops)". One of the two numbers is wrong against `text_cleaner.rs`; needs a count-and-fix (M1 wording pass or later).
3. **Cargo.lock contains two reqwest versions** (0.12.28 and 0.13.4). Dependency-bot PR #9 (reqwest 0.13) would collapse this; alternatively a maintainer-run `cargo update -p reqwest@0.12.28 --precise` after review. Left to the maintainer.
4. **`commands.rs` `read_text_file` custom command reads any path with an allowed extension** without a dialog-scope requirement. This is a deliberate convenience (drag-in flows) but is broader than the Tauri `fs` scope would allow; worth a explicit security-model note in SECURITY.md during M1's T1.1 rewrite of the scope section.
5. **`writing_journal.rs:3` and `citation_manager.rs:3`** have `#![allow(...)]` inner attributes placed after doc comments (cosmetic smell; rustfmt does not flag it).
6. **CSP `script-src 'unsafe-inline'`** is required by the theme boot script and documented inline (`index.html:9-11`); a nonce-based CSP would be tighter but is a behavior-affecting change (deferred; not needed by any current finding).
7. **Verifier CDN fallback** (`verifier/index.html:17-23`) activates only when `vendor/` files are missing; the vendor dir is committed, so the shipped verifier is offline. The fallback is documented in verifier/README.md but not in SECURITY.md; T1.1/T1.2 doc pass will mention the verifier's network posture.
8. **`docs/ARCHIVED-README-for-humanizeai.md`** references a former project name ("humanizeai") and contains old claims; it is archived history and was left untouched, but if it is ever linked publicly it should carry a banner. Currently not linked from README.
9. **Two Ollama base-URL constants** (`ollama.rs:11`, `abstract_generator.rs:8`) can drift; consolidation is a refactor (N7 forbids unrelated refactors in M0/M1). Logged for M2 preflight work (LM-1) which touches these constants anyway.
10. **PR #27 (Proofread tab)** pre-dates this work order and was not reviewed line-by-line; it overlaps M2 (LLM tab wiring, LM-6) and M4 (ledger, P4.1). Flagged so the maintainer sequences it against M2.
