# Hardening round H0–H9 — completion report

Status: COMPLETE on the branch `feature/word-assist` (tip `54dba31`, 5
new commits over `ccdd905`). Every commit compiled locally; nothing is
UNCOMPILED. DELIVERED BY BUNDLE per H9 — nothing pushed, no credentials
held or requested. CI has NOT run on these commits, so nothing here says
"CI-verified": the phrase used everywhere is LOCAL-VERIFIED, with the
toolchain named in H0. Phase 2 stays blocked on the spike; no work
outside H0–H9 and the held-out machinery directive was done.

## H0 — Environment (LOCAL-VERIFIED, not CI)

The sandbox had no Rust toolchain (and no sudo). Built one from scratch:

- rustup 1.99.1 (stable, minimal profile + rustfmt/clippy), installed to
  `~/.rustup` / `~/.cargo`.
- The `cargo check --lib` default build failed on missing system GTK:
  `gdk-3.0`, `webkit2gtk-4.1` absent, `sudo` requires a password. Fixed
  WITHOUT root: 288 Debian trixie `.deb`s (libgtk-3-dev,
  libwebkit2gtk-4.1-dev + closure) extracted into a local sysroot at
  `<agent-sysroot>` (a user-level directory outside the repo), `.pc` prefixes rewritten to the
  sysroot, dev symlinks repointed at the system runtime libs
  (`scripts/setup_gtk_sysroot.sh`, `scripts/fix_sysroot_symlinks.sh`).
- Baseline BEFORE any change: `cargo check --lib` PASS;
  `cargo check --lib --features word-assist` PASS; tests 61 (default) /
  139 (feature) PASS, 0 failed.
- Final state: default 61 PASS; feature lib 158 PASS + 1 ignored (the
  TEST-ONLY generator) + golden 8 + cross-language 2 + E2E 16 = 185
  feature-config tests PASS, 0 failed. `cargo clippy --features
  word-assist --all-targets` and default clippy: 0 warnings. svelte-check:
  0 errors, no new warnings.
- Per-commit compile check: each of `bb03faa`, `cd76d6c`, `e14dcc0`,
  `827b7cc`, `54dba31` passes `cargo check --lib --features
  word-assist` at its own tree (checked out and verified one by one).

## H1 — Pairing (D-3 as ruled)

- **CSPRNG:** both the code and the token draw from the OS CSPRNG via
  `getrandom::getrandom` — `scheduler.rs::rand_compat::fill_random_bytes`
  (scheduler.rs:128-131), consumed by `random_pairing_code`
  (scheduler.rs:151-167) and `random_token` (scheduler.rs:169-178).
  **No modulo bias:** the byte→index mapping is rejection-sampled —
  256 % 31 == 8, so bytes ≥ 248 (= 31×8) are redrawn and each of the 31
  indices is hit by exactly 8 byte values. Proven EXACTLY (not
  statistically) by `pairing_code_mapping_has_no_modulo_bias`, which
  enumerates all 256 byte values and asserts every index occurs exactly
  8 times.
- **Code length:** 6 characters over the 31-symbol unambiguous alphabet
  (no 0/O/1/I/L) — ≈ 2^29.7, strictly stronger than six decimal digits
  (2^19.9). If the owner reads "6 digits" literally as decimal, this
  exceeds it; recorded in DECISIONS.
- **Global 5-failure budget, burn:** `PAIRING_MAX_FAILURES = 5`
  (settings.rs:194); counted in `PairingState::take` (settings.rs:289,
  burn at settings.rs:312-324). Everything arrives from loopback, so
  there is no per-IP bucket anywhere.
- **Explicit user action + doubling cooldown:** new codes are issued
  only by the tab command → `PairingState::issue_new`
  (settings.rs:264-287), refused while a burn cooldown is active;
  cooldown starts at 30 s (`PAIRING_COOLDOWN_INITIAL`,
  settings.rs:197) and doubles per burn, capped at 30 min
  (`PAIRING_COOLDOWN_CAP`, settings.rs:198).
- **TTL 300 s, injectable clock:** `PAIRING_CODE_TTL`
  (settings.rs:190); `Clock` trait + `RealClock`/`FakeClock`
  (settings.rs:147-192); `PairingState::with_ttl` lets tests shrink it.
  Expiry tested with the fake clock
  (`pairing_code_expires_after_ttl`) and over real TLS with a 150 ms
  TTL (`pairing_code_expires_after_ttl` E2E).
- **Constant-time compares of fixed-length values:**
  `constant_time_eq` (settings.rs) on the 6-char code and the 64-char
  hex token hash.
- **Success burns the code:** `PairOutcome::Accepted` clears the entry
  (settings.rs:301-303); replay returns `NoCode`.
- **Route-specific limiter on /v1/pair:** 10 req/60 s
  (`PAIR_ROUTE_MAX`, settings.rs:203) enforced in `handle`
  (server.rs:407-419) on top of the global 120/60 s. E2E:
  `pair_route_limiter_bites_at_ten_per_minute` (11th → 429). Flood hits
  go to the app log, NOT the audit log (an attacker must not be able to
  drown the audit trail) — stated as a deliberate choice.
- **Audit without the code value:** success
  (`assist_pair`), failure (`assist_pair_failed` — includes only the
  attempt count), lockout (`assist_pair_lockout`), expiry
  (`assist_pair_failed` "expired") — E2E asserts the presented code
  string appears in NO audit detail (`wrong_code_lockout_and_cooldown`).

## H2 — Token persistence (resolves the D-3 conflict)

- **32 random bytes; hash-only at rest:** `issue_token`
  (settings.rs:664) stores `SHA-256(token)` + created_at / last_used /
  label (`StoredToken`, settings.rs:371-414). E2E
  `pair_success_issues_persistent_token` asserts the raw token appears
  in NO persistence backend.
- **Keychain first:** `scholarscribe`/`word-assist-client-token-v1`;
  0600-file fallback on unix WITH audit event
  (`assist_token_store_fallback`); NO file fallback on Windows
  (keychain only — same refusal policy as the TLS key, stated below).
- **Survives restart:** `attach_token_store` + restore at enable
  (settings.rs:563-601); E2E `token_persists_across_restart_and_revokes`
  builds a second state over the same data dir and the old token keeps
  working over real TLS.
- **Revoke / rotate / rotate-on-disable:** `revoke_token`
  (settings.rs:740) wipes memory + keychain + file (E2E: 401 after
  revoke); `rotate_token` = revoke + fresh pairing code (the token is
  never pushed, so this is the exact rotation shape) wired as the
  `word_assist_rotate_token` command; `word_assist_disable` rotates
  (revokes) on disable. `word_assist_unpair` is now a true revoke.
- **Sliding 30-day expiry:** `TOKEN_SLIDING_TTL` (settings.rs:366),
  checked on restore and on every bearer match (settings.rs:715);
  `last_used` refreshes in memory per request, persisted at most once
  per minute (`TOKEN_LAST_USED_WRITE_THROTTLE`). Pure math unit-tested
  (`token_sliding_expiry_pure_math`).
- **localStorage acceptance documented:** docs/LEDGER_THREAT_MODEL.md
  §5 states the owner-accepted same-user trust boundary in plain
  language, including what it does NOT defend against.

## H3 — Resource limits (`NetLimits::production`, server.rs:92-116)

| Limit | Value | Why |
|---|---|---|
| TLS handshake | 5 s | loopback handshakes take ms; 5 s is 3 orders of magnitude of headroom and caps slowloris-at-TLS |
| Header read | 30 s | hyper 1.x has NO separate keep-alive idle timeout; `header_read_timeout` bounds both slow headers AND dormant keep-alive sockets (one value, two properties — this is stated, not hidden) |
| Body read | 30 s | 64 KiB over loopback lands in ms; only fires on stall/abuse |
| Max concurrent connections | 16 | Word's pane uses ≤6 sockets/host; excess closed IMMEDIATELY (server.rs:282-290) |
| Max headers | 32 | legit requests carry ≤8 |
| Max header bytes | 16 KiB | ~10× the largest legitimate header set; floods rejected at parse |

- Enforcement: handshake wrapped in `tokio::time::timeout`
  (server.rs:297-306); `.timer(TokioTimer)` +
  `.header_read_timeout()` + `.max_headers()` + `.max_header_size()`
  on the http1 builder (server.rs:317-325); body read timeout wraps
  `read_body` (server.rs:853-860).
- Tier 1 handlers keep their OWN deadline: `TIER1_TIMEOUT = 120 s`
  (server.rs:60) inside the scheduler path — independent of the socket
  limits, per H3.
- Constants pinned by `production_net_limits_match_the_stated_values`;
  behavior by E2E `slow_headers_are_closed_within_the_timeout` (EOF
  within 4 s at a 500 ms limit) and
  `connection_cap_closes_excess_immediately` (cap 1: excess closed,
  slot freed after release).
- Tests drive tighter limits via `start_with_limits` (pub) so nothing
  waits on production durations.

## H4 — TLS floor (server.rs:222-243)

- `ServerConfig::builder_with_provider(Arc::new(ring::default_provider()))`
  — explicit provider, NOT the process default.
- `.with_protocol_versions([&TLS13, &TLS12])` — explicit list, not
  defaults.
- `alpn_protocols = [http/1.1]` only (server.rs:243).
- E2E `alpn_is_http11_and_tls13_negotiates`: ALPN selects http/1.1,
  TLS 1.3 negotiates by default, and a TLS1.2-ONLY client still
  completes a handshake (the owner's floor until spike (l)).
- **What is NOT testable, stated:** rejecting TLS 1.0/1.1 — rustls 0.23
  cannot represent those versions at all, so no client can even offer
  them through this stack; the rejection is structural, not a runtime
  behavior.

## H5 — Ephemeral CA (replaces the self-signed leaf; D-8 in DECISIONS)

- **CA:** `generate_chain` (server.rs:1106) — CA:TRUE with
  `BasicConstraints::Constrained(0)` (pathlen 0), KeyUsage =
  keyCertSign ONLY, nameConstraints permitting DNS `localhost` + IP
  127.0.0.1/32 (addr+mask asserted byte-for-byte in the test), validity
  3 years (`CA_VALIDITY_DAYS = 1096`, server.rs:938).
- **CA key destroyed:** used exactly once to sign the leaf
  (server.rs:1148-1152), then dropped (server.rs:1154-1159). It is
  never serialized, never written to disk or keychain; this code holds
  no copy. Honest caveat, recorded in DECISIONS: rcgen exposes no
  zeroize hook for its internal key buffers, so "destroyed" is
  best-effort in memory (ring zeroizes its own secret allocations on
  drop) — we can assert "never persisted, never serialized by us", not
  "every byte of every internal buffer is zeroed".
- **Leaf:** serverAuth EKU ONLY, SANs localhost + 127.0.0.1 and no
  other DNS SAN, validity 365 days, not_after ≤ CA's not_after;
  issuer == CA subject (all asserted in
  `generated_chain_matches_the_d5_model`, which parses BOTH certs with
  x509-parser and checks CA flag, pathlen, KU, NC, EKU, SANs, validity
  bounds and the issuer binding).
- **Key storage:** leaf key in the OS keychain
  (`scholarscribe`/`word-assist-tls-key-v1`); 0600-file fallback ONLY
  on unix and ONLY with an `assist_tls_key_fallback` audit event
  (server.rs:1160-1171, load path 1057-1090 emits the event too); on
  Windows the fallback is **REFUSED** (the owner's stated choice
  between "owner-only ACL or refuse": this build refuses — no ACL
  dependency added, Windows Credential Manager is always present).
- **The user trusts the CA, never the leaf:** `word_assist_registration`
  returns the CA cert + instructions to trust it, removal and renewal
  wording (assist_commands.rs:415-456); the app UI says the same.
- **Renewal regenerates both and re-asks consent:** expired/corrupt/
  missing material regenerates CA+leaf together
  (`load_existing_chain` fails → `generate_chain`); the user re-trusts
  the NEW CA (instructions say so explicitly).
- **Reload stability:** `chain_load_is_stable_across_calls` proves the
  second `ensure_certificate_chain` LOADS instead of regenerating.
- **TEST-ONLY generator for spike (q):** `cargo test --features
  word-assist generate_owner_test_ca -- --ignored --nocapture` writes
  `target/word-assist-test-ca/` (TEST CA + leaf-localhost + a
  WRONG-NAME leaf + README with the manual owner-run steps). Never
  committed, writes only under `target/`, trust-store changes stay
  manual and owner-run. Spike items (q)/(q2)/(r) added to
  `word-addin/spike/OBSERVATIONS.md`.
- **Blast radius if name constraints are unsupported** (owner decision
  after spike (q), pre-recorded): a locally trusted CA holder could
  mint certs for OTHER names; the constraint is the containment, so
  its failure is material.
- **Dependency accounting:** x509-parser 0.18.1 was ALREADY in
  Cargo.lock as rcgen's own dependency; it is now declared directly but
  OPTIONAL and wired into `word-assist` only. The default build still
  resolves neither rcgen nor x509-parser (H7 step proves it in CI;
  proven locally now). A zeroize dependency was considered and NOT
  added — the CA key is never serialized, so there was nothing to
  zeroize; shipping an unused dependency would have been worse.
- **Release blocker kept:** THIRD_PARTY_LICENSES.md regenerates from a
  complete registry on the release machine (DECISIONS, unchanged).

## H6 — E2E tests (`tests/word_assist_e2e.rs`, 16 tests, all green)

Real listener (`start_with_limits` on port 0), TEST-ONLY ephemeral-CA
fixtures generated by the shipped chain code into a temp dir, a rustls
client trusting that CA, a mock Ollama daemon for Tier 1. No listener
mocks. Owner matrix → test names:

| Matrix item | Test |
|---|---|
| health | `health_is_open_but_pair_and_check_are_guarded` |
| pair success | `pair_success_issues_persistent_token` |
| wrong code | `wrong_code_lockout_and_cooldown` (+ `pair_success…` NoCode replay) |
| lockout | `wrong_code_lockout_and_cooldown` |
| expiry | `pairing_code_expires_after_ttl` |
| unauthenticated /v1/check → 401 | `health_is_open_but_pair_and_check_are_guarded` |
| Origin mismatch | `origin_and_host_pins_reject_mismatches` |
| Host mismatch (DNS-rebinding) | `origin_and_host_pins_reject_mismatches` |
| body over 64 KiB → 413 | `body_over_64k_is_413` |
| slow header closed within timeout | `slow_headers_are_closed_within_the_timeout` |
| connection cap | `connection_cap_closes_excess_immediately` |
| static route only embedded assets; /..%2f, //, percent-encoded → 404 | `static_routes_serve_only_embedded_assets` (8 path variants) |
| stop() closes the port | `stop_closes_the_port` |
| token persistence, revoke, rotation | `token_persists_across_restart_and_revokes` |
| route limiter | `pair_route_limiter_bites_at_ten_per_minute` |
| sentinel (text nowhere in audit/log/ledger) | `sentinel_text_never_lands_in_audit_log_or_ledger` (Trace-level log capture) |
| stale-result discard + abort (1b ruling) | `stale_result_is_discarded_newest_wins` (deterministic: start-gated caller proves a RUNNING spawn_blocking task's late verdict is dropped and its callback never runs) |
| Tier 1 mock loop | `tier1_mock_loop_updates_cache` (sentence is PROBE-VERIFIED to trip harper's dev profile — PronounVerbAgreement) |
| TLS/ALPN | `alpn_is_http11_and_tls13_negotiates` |

Defect found and fixed while building this suite: `start_with_limits`
now publishes the resolved-port settings snapshot (port 0 support +
handler Host/Origin coherence).

## H7 — CI

One step added to `word-assist-gates`: asserts `cargo tree -i rcgen`
and `cargo tree -i x509-parser` find NOTHING in the default config
(cargo tree -i exits non-zero when nothing matches). hyper/rustls
deliberately unchecked per the owner's note (they enter the default
tree through reqwest/tauri). Proven locally today: both invocations
report "did not match any packages". A second step runs the freeze
check (held-out directive). Both steps are additive; no permissions,
secrets, third-party actions or `pull_request_target` introduced.

## H8 — Evidence and fixes

- **Strictness defaults a fresh install gets:**
  `persistence.rs:95-97` (`default_word_assist_strictness` → "dev"),
  applied at `persistence.rs:109`; profile semantics
  `validate.rs:60-66` — grammar ON, spelling OFF, clarity OFF
  (E-1 provisional). Test: `settings_from_persisted_validates`.
- **Tier 1 OFF until an explicit model pick:** default OFF
  (`persistence.rs:110`); the "empty = last used chat model" shortcut
  is REMOVED — `AssistSettings::from_persisted` now coerces
  tier1_enabled=false when the model name is empty/whitespace
  (settings.rs:56-64; test `tier1_requires_an_explicit_model_name`),
  and the UI enforces the same at save time (WordAssist.svelte
  persistSettings). Before this fix, enabling Tier 1 without a model
  produced per-sentence failures at check time — now it is simply OFF.
- **Experimental label in the UI:** the rewrite block is labelled
  "· Experimental" and the hint states "No precision evaluation exists
  for this feature yet, so suggestions may be wrong." No numbers
  appear (E-1 respected). svelte-check: 0 errors, no new warnings.
- **Exact accepted values:** Host = exactly `localhost:<port>`
  (server.rs:396-399); Origin = exactly `https://localhost:<port>`
  (server.rs:470-477) for every /v1 route except health (GET, same-
  origin exempt) and pair (Origin required, bearer exempt);
  `route_requires_bearer` (server.rs:466). Both are strict equality
  after `.to_str()` — no suffix/endsWith matching anywhere.
- **::1 answer:** if the webview resolves `localhost` to ::1 first, the
  TCP connect to [::1]:PORT fails (the listener binds only
  127.0.0.1); standard dual-stack fallback then uses the IPv4 A
  record and the request succeeds — and the Host/Origin HEADERS are
  unaffected either way, because they carry the literal name
  `localhost`, which is exactly what the pins compare. The residual
  risk (a webview that does NOT fall back) is spike item **(r)** in
  OBSERVATIONS.md with the observation steps and both expected
  outcomes; the fallback mitigation (also pinning ::1 or switching the
  add-in to 127.0.0.1) would change the accepted values and is an
  owner decision if (r) ever shows a failure.

## Held-out set (machinery now, data later)

- **Scoring tool:** `assist::eval` (feature-gated, pure std):
  `score_sentence` + `aggregate` → per-category n/TP/FP/FN with the
  matching policy documented in the module (span overlap AND same
  category; one label consumes one issue). `wilson_interval_95` with
  8 unit tests (known value 8/10 → 0.490–0.943; degenerate inputs
  never NaN — 0/0 is "no data", not 0%; monotone in successes; wider
  intervals for smaller samples; policy/double-count/folding tables).
  The tests caught two real defects during development (double-counted
  TP; wrong expected constant) — both fixed before commit.
- **Schema:** `docs/EVAL/heldout.schema.json` (JSONL; id, text,
  category, labels with scalar spans + expected note, source, license).
- **Freeze + CI check:** `docs/EVAL/FROZEN.txt` currently freezes the
  schema AND `src-tauri/tests/assist/golden.json` (hash
  `717819165f3a…` — matches the DECISIONS re-freeze record exactly).
  `scripts/check-eval-freeze.mjs` runs in CI and was tamper-tested
  locally (a modified file FAILS with frozen vs actual hashes).
- **Labelling protocol:** `docs/EVAL/LABELLING_PROTOCOL.md` — 8
  numbered rules: labels written BEFORE any checker run (rule 1);
  source + license recorded per sentence (rule 2); license terms
  checked BEFORE any public GEC set is used, non-commercial sets
  scored locally and NEVER committed, no third-party corpora vendored
  into the repo (rules 2-3); ≥150 sentences with per-category
  minimums (rule 4); frozen means frozen, new version = new file
  (rule 7); reporting with Wilson intervals only (rule 8).
- **Golden v1 relabelled:** DECISIONS (hardening round) states golden
  v1 is a SMOKE TEST, not evidence of precision.
- **What remains owner-side:** the ≥150 approved sentences; the Tier 1
  harness RUN against real models (blocked on the spike anyway).

## Listener self-review table — every row carries its proving test

| # | Property | Test (LOCAL-VERIFIED) |
|---|----------|----------------------|
| 1 | Binds 127.0.0.1 only | `server.rs:245` (`SocketAddr::from(([127,0,0,1], port))`); structural; E2E suite connects only to 127.0.0.1 and never observes a non-loopback bind — no dedicated negative test possible without a second interface |
| 2 | Origin strict equality incl. DNS-rebinding | `origin_and_host_pins_reject_mismatches` (wrong Origin 403, missing Origin 403) |
| 3 | Host strict equality (DNS-rebinding case) | `origin_and_host_pins_reject_mismatches` (Host evil.test → 400) |
| 4 | Constant-time token compare | `constant_time_eq_basics` (unit) + `issue_and_match_token_hash_only`; compares fixed-length hex hashes |
| 5 | Pairing: one-time, 5 min, 5 tries, lockout | `pairing_code_lifecycle_burn_on_success`, `pairing_burns_on_fifth_failure_and_starts_cooldown`, `wrong_code_lockout_and_cooldown` (E2E) |
| 6 | Token persisted as hash only | `issue_and_match_token_hash_only` + E2E `pair_success_issues_persistent_token` (raw token absent from storage) |
| 7 | Rate limit (global) | `rate_limiter_windows` (unit); global 120/60 unchanged |
| 8 | Rate limit (/v1/pair route) | `pair_route_limiter_bites_at_ten_per_minute` |
| 9 | Body cap | `body_over_64k_is_413` (E2E) + `body_cap_is_64k` |
| 10 | Read/header timeouts + max concurrency | `slow_headers_are_closed_within_the_timeout`, `connection_cap_closes_excess_immediately`, `production_net_limits_match_the_stated_values` |
| 11 | Static route traversal safety | `static_routes_serve_only_embedded_assets` (exact-match dispatch; 8 variants 404) |
| 12 | Audit/log/ledger free of document text | `sentinel_text_never_lands_in_audit_log_or_ledger` + audit-no-code assertions in `wrong_code_lockout_and_cooldown` |
| 13 | Disable/stop stops the listener | `stop_closes_the_port` (port confirmed closed by polling) |
| 14 | TLS floor [1.3, 1.2] + ALPN http/1.1 | `alpn_is_http11_and_tls13_negotiates` (1.0/1.1 structurally impossible in rustls — stated, not testable) |
| 15 | Default build excludes feature crates | `cargo tree -i rcgen` / `-i x509-parser` → "did not match any packages" (proven locally; CI step lands with H7) |
| 16 | Cert model (CA:TRUE, pathlen 0, KU, NC, EKU, SANs, validity) | `generated_chain_matches_the_d5_model`, `chain_load_is_stable_across_calls` |
| 17 | Stale-result discard + request abort (1b) | `stale_result_is_discarded_newest_wins` (deterministic start-gate) |

## Test decomposition (this round's totals)

- Default build: **61** unit tests (lib) — unchanged from baseline.
- Feature build: **185** = 158 lib (139 baseline + 19 new: pairing×5,
  token×4, scheduler uniformity + shape + net limits + cert×2 + eval×8
  minus overlaps) + 8 golden + 2 cross-language + **16 E2E** + 1
  ignored (TEST-ONLY owner generator).
- Add-in client automated coverage: **ZERO** — unchanged, and that is
  still accurate: the add-in (word-addin/) is untouched this round
  (H-rules said no add-in changes; only OBSERVATIONS.md gained rows).
  C4 suppression remains untested pending the spike (deferred ruling).
- Tests touching the REAL TLS path: all 16 E2E tests (rustls client ↔
  tokio-rustls server, real handshakes, real certs); 2 of them
  additionally pin ALPN/protocol versions.

## Delivery (H9)

- `download/scholarscribe-word-assist-hardening.bundle` — full
  `git bundle` of `feature/word-assist` (self-contained; works whether
  or not the owner's clone already has `ccdd905`).
- `download/patches/` — 16 `git format-patch` files
  (`0001-…0016-…`), one per commit main..feature/word-assist.
- `download/README-BUNDLE.md` — exact fetch / merge --ff-only / push
  commands.
- Nothing was pushed; no token was requested or used; the working tree
  is clean at `54dba31`.

## Honest notes for the owner

1. The sandbox test runs are LOCAL-VERIFIED only. The phrase
   "CI-verified" appears nowhere and CI has not seen these commits.
2. `wrong_code_lockout_and_cooldown` (E2E) uses the REAL 30 s cooldown
   only to assert it GATES (it never waits it out); the doubling math
   is fake-clock-tested at unit level.
3. The E2E `pair_route_limiter…` test and the lockout test share one
   state per test — no cross-test coupling (each test builds its own
   AssistState and temp dir).
4. Token-store tests serialize on a process-local mutex because
   machines with a live OS keychain share one keyring entry; on
   keychain-less CI (this sandbox, ubuntu runners) the 0600-file
   fallback path is exercised and per-test isolated.
5. `clear_session` no longer deletes the persisted token (enable needs
   the restore); disable/unpair/rotate revoke explicitly. This is the
   exact resolution of the H2 conflict and is commented at the call
   sites.
6. rcgen CA-key "zeroization" is best-effort in memory (never
   serialized/persisted by us); see H5 for the precise wording.
