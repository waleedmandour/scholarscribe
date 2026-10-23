# WORD ASSIST: CONSOLIDATED WORK ORDER

> Verbatim copy of the owner's consolidated work order (committed per its own instruction:
> "First commit a copy of it as `docs/WORD_ASSIST_WORK_ORDER.md` on every branch you create,
> so it survives a sandbox reset"). This file currently sits UNCOMMITTED because Step 1
> (state check) stopped before any branch was created; it will be the first commit on the
> next branch created under this work order.

This supersedes every earlier prompt, ruling message and gate reply. Work only from this document. First commit a copy of it as `docs/WORD_ASSIST_WORK_ORDER.md` on every branch you create, so it survives a sandbox reset.

ROLE: senior Rust/TypeScript engineer on my repo `waleedmandour/scholarscribe` (Tauri 2, Rust, Svelte, Ollama, MIT). Goal: release v0.1 (a PREVIEW) of the Word Assist add-in for Word on Windows and Word on Mac. I am the only person who pushes, tests in Word, signs, tags or publishes. You have network access; the repo is public. Never ask for a token. You cannot test Word or a Mac.

## 0. OWNER INPUTS (the owner edits; blanks take the default)
- HARDENING SOURCE: [origin | bundle:<path> | LOST]  default: origin
- STAGE A SOURCE: [origin/word-assist-stageA | bundle:<path>]  default: whichever of origin or `download/word-assist-stageA.bundle` verifies with tip ba5a959
- Windows with Word + Microsoft 365: [YES]
- Mac with Word for Mac: [NO -> Mac build is labelled "unvalidated" or omitted; never inferred from Windows]
- Code signing: [none -> unsigned pre-release]
- Release posture: [GitHub pre-release tag `word-assist-v0.1.0`, built with `--features word-assist`; the normal app release channel is unchanged]
- Held-out evaluation set (>=150 sentences): [NOT YET -> PREVIEW: no accuracy claims anywhere]
- Spike results (OBSERVATIONS.md a to r): [PENDING]

## 1. STANDING RULES
- Tag every claim VERIFIED / INFERRED / NOT VERIFIED / NEEDS HUMAN VERIFICATION, with evidence (file:line, command output, URL plus access date).
- Do not push. Deliver by `git bundle` plus patches plus a README with exact fetch / `merge --ff-only` / push commands. No force pushes: each delivered tip must descend from `origin/feature/word-assist`.
- No new dependency without a proposal (name, version, license, why, alternatives).
- Never write "CI-verified" for anything CI has not run. Never write "no errors", "works on Mac" or "supported" without evidence. State which commits compiled locally.
- No writes to any trust store or registry by the app or by you. No secrets. Do not touch canonical provenance files (`verifier/verify.js`, PROVENANCE_SPEC.md, AUDIT.md).
- If the sandbox lacks a toolchain, build rustup plus a user-level sysroot (no sudo). Nothing may be UNCOMPILED unless labelled.
- If the owner reports a CI failure, fix it before any new work.

## 2. SETTLED RULINGS (do not reopen)
- Architecture: Office.js add-in plus a local engine in the Rust app, behind Cargo feature `word-assist` (default off; default builds unchanged).
- Local-first. No document text is persisted or logged anywhere (audit, ledger, panics, caches). Sentence cache is memory-only. Out of scope forever: humanizing or detector evasion, generating citations or content, applying edits without an explicit accept.
- Tier 0: reuse existing `language_check.rs` (harper-core =2.11.0); changes there are additive and in their own commit. Tier 1: Ollama, JSON-schema output, `think:false`, protected-span masking, word diff, validators.
- v1 is English-only. Other languages: no underlines, a status line, plus an "experimental" toggle that is off by default.
- Defaults: grammar ON (provisional), spelling OFF, clarity OFF, Tier 1 OFF until the user explicitly picks a model, labelled Experimental. No precision figures in any user-facing text. Golden set v1 (hash 717819165f3ad4704f724359c5516a964b1cc8d37e99621ddcb34020ea4b94fb) is a SMOKE TEST, frozen. Held-out set: owner-supplied, labels written before any checker runs, licenses recorded, never vendor third-party corpora, Wilson intervals.
- Ledger: SHA-256 hash chain, chain head signed at export through the existing provenance ed25519 path, separate file `language_assist.json`. NO HMAC anywhere. State that today's verifier does not check it.
- Security model: H1 to H5 below. Default port 38080 (IANA-verified unassigned), configurable. Reuse the hyper / rustls / tokio-rustls already in the lockfile with an explicit crypto provider (ring). Never hand-roll TLS.
- Governance: network-surface class `addin-webview`; the scanner covers `word-addin/` and any new crate dir; spike dirs are excluded and dev-only. CI changes are additive: only `actions/*`, `dtolnay/rust-toolchain` and actions already used; pin any non-GitHub-owned action by commit SHA; no `pull_request_target`; no new secrets.

## 3. DURABILITY PROTOCOL
Assume the sandbox resets at any time. After each numbered step: commit, write a bundle to `download/` with its README, and list the path in your reply so the owner can copy it out. Keep `docs/agent-reports/PROGRESS.md` in the repo (state, SHAs, next step). Before every delivery, clone the bundle into an empty directory and run the verification there.

## 4. STEP 1: STATE CHECK (always first)
Fetch origin with explicit refspecs (the default refspec may fetch only main); cross-check with `git ls-remote` and the GitHub API. Report the SHAs of `origin/feature/word-assist` and `origin/word-assist-stageA`. Then verify the declared sources:
- HARDENING SOURCE = origin: `0fbc012` must be an ancestor of `origin/feature/word-assist`.
- HARDENING SOURCE = bundle: `git bundle verify`; fetch to a temp branch; confirm tip 0fbc012 and that d55bc00 is its ancestor.
- STAGE A SOURCE: verify tip ba5a959 on base d55bc00.
- If a declared source fails verification, STOP and report what you did find. Do not silently use another source and do not fall back to LOST without the owner's word.
- LOST -> Step 2. Otherwise skip to Step 3.

## 5. STEP 2: REBUILD HARDENING (only if LOST)
New branch `feature/word-assist-hardening` from origin/feature/word-assist (d55bc00). Commit and bundle after each of the three groups: [H0, H-pre, H1, H2], [H3, H4, H5], [H6 to H9]. After the last group STOP: the owner pushes it to origin before Step 3. Say plainly what you re-derived versus could not reproduce.
- H0: toolchain and sysroot.
- H-pre: `/v1/pair` is authenticated by the pairing code, not a bearer token (route classification as a tested predicate); one `paired` ledger record per pairing; ledger = SHA-256 chain per section 2.
- H1 pairing: OS CSPRNG (`getrandom`); rejection sampling with no modulo bias (test all byte values); 6-symbol code; global budget of 5 failures burns the code; a new code only by explicit user action behind a cooldown of 30 s doubling to a 30 min cap; TTL 300 s on an injectable clock; constant-time compare of fixed-length values; success burns; `/v1/pair` route limiter 10/60 s on top of the global one; audit events for success, failure and lockout, never the code.
- H2 token: 32 CSPRNG bytes; persist only SHA-256(token) plus created_at, last_used, label (keychain first; 0600 file fallback on unix with an audit event; refused on Windows); survives restart; revoke and rotate in settings; rotate on feature disable; 30-day sliding expiry; plaintext token in add-in localStorage is accepted and documented in the threat model.
- H3 limits: handshake 5 s, header read 30 s, body read 30 s, at most 16 concurrent connections (excess closed immediately), 32 headers / 16 KiB, body cap 64 KiB, Tier 1 scheduler deadline 120 s.
- H4 TLS: rustls `builder_with_provider(ring)` with explicit versions [TLS1.3, TLS1.2] and ALPN http/1.1 only.
- H5 certificates: ephemeral CA (CA:TRUE, pathlen 0, keyCertSign only, nameConstraints permitting only DNS localhost and IP 127.0.0.1/32, 3 years); leaf serverAuth only, SANs localhost + 127.0.0.1, validity no longer than the CA's; the CA key is used once and never persisted; the leaf key goes in the keychain (audited file fallback, refused on Windows); the user trusts the CA, never the leaf; renewal regenerates both and re-asks consent. Tests via x509-parser (optional, feature-only; already in Cargo.lock) assert CA flag, pathlen, name constraints, EKU, SANs, validity bounds. A TEST-ONLY owner-run generator writes to `target/` and is never committed (spike q and q2).
- H6 E2E (real listener on port 0, TEST-ONLY certificate fixtures, rustls client, mock Tier 1): health; pair success; wrong code; lockout; expiry; 401 without token; Origin mismatch; Host mismatch (DNS-rebinding case); body over 64 KiB -> 413; slow header closed within the timeout; connection cap; static route returns only embedded assets and path variants (`/..%2f`, `//`, percent-encoded) 404; `stop()` closes the port; token persistence, revoke, rotate; stale-result discard; request abort; and a sentinel test proving a unique sentence sent to `/v1/check` appears nowhere in audit events, logs or the ledger.
- H7 CI: in the DEFAULT config `cargo tree -i rcgen` and `-i x509-parser` find nothing. Do not assert on hyper or rustls.
- H8 evidence: file:line of the shipped defaults; accepted Host (`localhost:<port>`) and Origin (`https://localhost:<port>`); ::1 behaviour recorded as spike item (r).
- H9 held-out machinery, no data: schema; labelling protocol doc; FROZEN.txt plus a CI check that frozen files are unchanged; scoring tool with per-category n/TP/FP/FN and Wilson interval, unit-tested.

## 6. STEP 3: INTEGRATION
Branch `feature/word-assist-integration` from origin/feature/word-assist (it must contain H0 to H9). Cherry-pick the Stage A commits in order.
- Conflicts: hardening wins on ledger, pairing, token, limits, TLS, certificates, scheduler stale-discard, `assist::eval` and held-out machinery. Stage A wins on the add-in TypeScript client and tests, manifest and install path, release workflow, docs, QA matrix and Tier 1 harness (rebased onto the hardening eval module). Keep ONE Wilson function, ONE eval module, ONE freeze checker (say which and why; the golden hash above must be unchanged). List every conflicted file and its resolution.
- Commit `scripts/check-eval-frozen.mjs`, which Stage A referenced from `package.json` and `docs/EVAL_PROTOCOL.md` but never committed.
- ci.yml: keep both the OS matrix and H7's assertions on all legs, plus the freeze check. `.gitattributes` forces LF for frozen files, tests/assist, word-addin and embedded assets; it must not change any recorded hash. Print the full diff.
- Adapt Stage A to the hardened reality: the A3 export signs the SHA-256 chain head (no HMAC); A4 trust instructions and the exported manifest refer to the CA (Windows `certutil` into the CURRENT USER store; macOS `security add-trusted-cert`, flags verified against docs), with uninstall and revoke for each step; the add-in client handles wrong code, lockout, cooldown, 429 and 401 after revoke or rotate with a clear re-pair UX, never logs codes or tokens, and has simulated-response tests.
- ES-module serving: E2E tests that every embedded module is served with `Content-Type: text/javascript`, `X-Content-Type-Options: nosniff` and the CSP header, and that path variants still 404. Add a `nomodule`/feature-detect fallback that shows a clear message in the pane instead of a blank one; cite the Microsoft doc for which Word runtimes can run ES modules; mark gaps NOT VERIFIED.
- Docs: rewrite QA_v0.1.md, USER_GUIDE troubleshooting (lockout, cooldown, revoke), SECURITY.md and RELEASE_PLAN_v0.1.md to match the hardened behaviour. Remove "residual risk" statements that H1 to H9 fixed; keep an honest list of what remains.
- Listener self-review table: every row names its proving test. Any NOT TESTED / NOT IMPLEMENTED row blocks Stage C.
- Release workflow (A5): print the full file. List triggers and confirm the tag pattern cannot match the existing release.yml tags; per-job `permissions` (read-only except `contents: write` on the publish job); every action's owner and pin; no secrets; the dry-run path never publishes.
- Advisories (A6): for each, give ID, crate, affected vs locked version, reachability in the listener or app, fix status. Cover rustls, time, quick-xml and zip. If a fix sits in a pending dependabot branch that would change a TLS or listener dependency, propose a separate PR.
- Verify default and feature configs: fmt, clippy `-D warnings`, tests, E2E, add-in tests, dist == embedded, freeze checks, `cargo tree` assertions. Give test counts by layer and explain every delta against 63 default / 175 feature / 163 (Stage A).
- Deliver the bundle and README. Report the exact SHAs and `git log --oneline` of the branch.

## 7. GATE 1 REPORT, THEN STOP
1. Summary (5 lines). 2. Commits with compiled / tested-locally / UNCOMPILED labels. 3. Acceptance against section 11. 4. Discrepancies. 5. Owner actions in order. 6. Risks. Stop until the owner reports CI results from the pushed branch.

## 8. STAGE B (starts when the owner pastes the filled OBSERVATIONS.md, items a to r)
Update the assumptions register; implement the verified strategies behind the existing interfaces. Where it says STOP, stop and ask.
- (a) offset unit: set the adapter; add parametrised tests including emoji, Arabic and combining marks.
- (b) who applies a popup accept: if Word applies it, never apply again; otherwise apply exactly once, tested.
- (j) annotation-id correlation: implement the verified mechanism. If none exists, STOP.
- (e) self-events: wire the suppression watermark, delete dead code, test.
- (f)/(k) requirement sets per OS: if annotations are unsupported on an OS, that OS is list-mode only, stated in UI and docs.
- (g)/(l) webview runtime and HTTPS: record. Do not tighten the TLS floor without owner approval.
- (q) name constraints: if unsupported on a target OS, STOP and report the blast radius.
- (h) network capture: if document text leaves the machine because of our annotations, STOP. If only Word's known traffic and the office.js CDN fetch appear, document precisely.
- (m) tables, footnotes, headers, Track Changes: implement what works, document what is not covered. (r) ::1: fix or document. (o)/(p): tune colours and docs.
- No Mac results means Mac rows stay NOT VERIFIED. Gate 2 report, then STOP.

## 9. STAGE C: RELEASE CANDIDATE
Version and changelog (numbering that does not collide with the app's own). Dry-run release build from the workflow. The owner installs it and runs QA_v0.1.md, then returns results. Fix each reported failure in its own commit with a regression test where possible. Repeat until section 11 is met. Gate 3 report.

## 10. STAGE D: OWNER ONLY
The owner pushes, tags, publishes, signs (if any) and announces. You prepare plain release notes: preview status, M365 requirement, what is and is not validated per OS, privacy facts (office.js CDN fetch), known limitations.

## 11. DEFINITION OF DONE v0.1
D1 CI green on ubuntu, windows and macOS (verify runner labels in GitHub docs): fmt, clippy `-D warnings`, tests, E2E, with and without the feature. D2 tag-triggered release workflow dry-run produces Windows installer, macOS build per architecture, SHA256SUMS, SBOM. D3 add-in TypeScript strict with tests, reproducible build, dist == embedded check, manifest validated. D4 every assumptions-register row VERIFIED or downgraded with a documented fallback. D5 owner QA matrix passed on Windows (and Mac if available). D6 docs and privacy statement match the network capture and real behaviour. D7 listener self-review has no NOT TESTED / NOT IMPLEMENTED row. D8 dependency audit done and THIRD_PARTY_LICENSES.md regenerated from a complete registry.

## 12. STOP AND ASK IF
a privacy claim would change; anything touches a trust store, registry, secrets or signing; a new dependency is needed; the listener would bind beyond loopback; document text could leave the device; a declared source fails verification; name constraints or annotations are unsupported on a target OS; an owner ruling contradicts platform evidence.

START with Step 1.
