# Word Assist — Security Review Pack

**Audience:** an independent reviewer who did not write this code.
**Tree:** `feature/word-assist-integration` @ `44fee78` (+ the CARD W `--locked` commit; line
numbers are from the tip this pack was written on — see the commit line in §0).
**How to use:** §1 tells you where the security-critical code lives (file + lines). §2 lists
each invariant and the test that is supposed to prove it — run the suites yourself and treat
any mismatch between this table and the code as a finding. §3 is the threat model in 10
lines. §4 lists what is *not* tested; that list is part of the review surface, not an
admission to skip.

Evidence status: every file/line reference below was read from the tree named above this
session. Suite-green claims are NOT made here — see §0 for what to run and what is still
owner-pending.

## 0. What to run

```sh
cd src-tauri
cargo fmt -- --check
cargo clippy --locked --features word-assist --all-targets -- -D warnings
cargo test --locked --features word-assist          # lib (unit) + integration suites
cargo test --locked                                  # default config (feature stays opt-in)
cd .. && npm test                                    # add-in + frontend gates, incl. dist integrity
```

Platform legs (Windows, macOS arm64/int64) are defined in `.github/workflows/word-assist-ci.yml`.
CI green on all legs is **owner-pending** (DoD item D1). This pack makes no CI claim.

## 1. Security-critical files and line ranges

All Rust paths relative to `src-tauri/`. Line ranges were verified against the tip named
above; re-derive with `grep -n` if the tree moved.

### 1.1 Listener — `src/assist/server.rs`

| Concern | Lines |
|---|---|
| Bind `127.0.0.1:<port>` only (never `0.0.0.0`) | 307–319 |
| Production resource limits (`NetLimits::production`) | 76–115 |
| Connection cap: excess streams closed, not queued | 346–368 |
| Per-connection guard decrements on completion | 402–410 |
| TLS handshake bounded by timeout | 372–380 |
| hyper 1.x: header-read timeout, max header count/size | 382–391 |
| Request dispatch + global sliding-window rate limit | 414–445 |
| Route-specific `/v1/pair` limiter | 447–458 |
| `Host` pin (exactly `localhost:<port>`) | 460–465 |
| Auth model: Origin pin + bearer on `/v1/*` (health/pair exempt, with reasons) | 467–488 |
| `route_requires_bearer` classification | 546–548 |
| `check_origin` — exact `https://localhost:<port>`, missing Origin rejected | 550–557 |
| `check_bearer` — constant-time match against stored hash | 559–574 |
| 64 KiB body cap (`BODY_CAP`) | 65–67 |
| `read_body` — cap + wall-clock body timeout (both → 413) | 944–955 |
| 20 000-char cap on `/v1/check` text | 713–718 |
| Add-in CSP (`default-src 'none'`; office.js CDN only) | 117 |
| `static_response` — nosniff, no-store, CSP attachment | 195–215 |
| Embedded add-in module table (single source of served assets) | 124–176 |
| Module serving under the pane CSP | 512–528 |
| No text in audit: counts + hashes only at every record site | 338–344, 606–608, 906–940, 818–829 |

### 1.2 TLS and certificate chain — `src/assist/server.rs`

| Concern | Lines |
|---|---|
| TLS config: explicit ring provider, TLS 1.3 + 1.2, no client auth, single cert, ALPN `http/1.1` only | 293–305 |
| Validity constants (CA 3 y + 1 d, leaf 365 d, 24 h backdate) | 1037–1048 |
| File names + keychain entry (`word-assist-tls-key-v1`) | 1045–1048 |
| `ensure_certificate_chain` — load-or-regenerate entry point | 1066–1086 |
| `load_existing_chain` — parse + validity check; expired/corrupt → regenerate | 1091–1122 |
| `load_leaf_key` — keychain first; 0600 file on **unix only**, audited; **refused on Windows** | 1134–1181 |
| `generate_chain` — ephemeral CA (CA:TRUE, pathlen 0, keyCertSign ONLY, nameConstraints DNS `localhost` + IP 127.0.0.1/32) | 1197–1217 |
| Leaf (serverAuth EKU only, SAN `localhost` + `127.0.0.1`, validity inside CA) | 1219–1233 |
| CA key destroyed after single use | 1236–1241 |
| Keychain write; fallback path with audit event; Windows refusal | 1249–1281 |

### 1.3 Pairing — `src/assist/settings.rs`

| Concern | Lines |
|---|---|
| Constants: TTL 300 s, 5-failure global budget, cooldown 30 s doubling → 1800 s cap, `/v1/pair` 10 req/60 s | 189–204 |
| `PairOutcome` state vocabulary | 206–218 |
| `PairingState::issue_new` — cooldown gate on new codes | 264–285 |
| `PairingState::take` — constant-time compare, burn-on-success, burn-on-5th-failure, doubling cooldown | 289–327 |
| `constant_time_eq` (length not secret: fixed 6-char alphabet) | 815–824 |
| Global rate limiter constants (120 req/60 s) | 527–529 |

### 1.4 Token store — `src/assist/settings.rs`

| Concern | Lines |
|---|---|
| `StoredToken` — SHA-256(token) + metadata only; raw token never persisted | 374–411 |
| Sliding-expiry pure predicate (clock-backward = fresh) | 392–402 |
| `TokenStore` — keychain first; 0600 file fallback (unix); **no file fallback on Windows** | 419–501 |
| `write_private` — 0600 write; refuses on non-unix | 503–525 |
| Restore on enable; expired record revoked at restore; fallback audited | 563–599 |
| `issue_token` — 32 CSPRNG bytes; persist-first (fail = pair fails) | 664–696 |
| `token_matches` — hash compare, sliding 30-day expiry revokes on the spot, throttled `last_used` write | 700–748 |
| `revoke_token` / `rotate_token` — memory + keychain + file, idempotent | 753–793 |

### 1.5 Ledger — `src/assist/ledger.rs`

| Concern | Lines |
|---|---|
| `LedgerEntry` — hashes and counts only; no text field exists in the schema | 33–58 |
| `canonical_entry` — fixed field order, tab-joined (deterministic input to the hash) | 62–79 |
| `compute_hash` — SHA-256 over the canonical form, `prev_hash` included | 81–84 |
| `open` — whole chain verified at open time | 113–129 |
| `scan_and_verify` — per-entry `prev_hash` link + hash check; first broken entry aborts | 133–166 |
| `append` — paragraph_id hashed here (no call site can store text); pre-edit hash truncated to 12 chars | 171–222 |
| `chain_head` — the value the export signs | 234–236 |
| `aggregate` — counts only (disclosure pre-fill) | 242–272 |

### 1.6 Export — `src/assist/export.rs`

| Concern | Lines |
|---|---|
| `LanguageAssistExport` + `ChainHead` — counts + chain-head commitment; no text | 43–72 |
| `canonical_payload` — deterministic serialization; signature covers exactly this | 77–100 |
| `build_export` — opens the ledger FRESH (tampered ledger fails the export), signs with ed25519 (provenance key, `assist_commands.rs:370`) | 110–139 |
| `verify_export` — parse → canonical payload → signature + fingerprint checks | 141–180 |

### 1.7 Add-in pairing code — `word-addin/src/`

| Concern | Lines |
|---|---|
| Failure classification: server message treated as untrusted formatting input; the pane shows client-owned wording | `pairing.ts` 46–102 |
| Check/accepted failure mapping (401 → clear token + re-pair) | `pairing.ts` 111–131 |
| `parsePairOk` — token extracted, shape-checked | `pairing.ts` 133–140 |
| `sanitizeForDisplay` — token/code-shaped strings redacted before any display | `pairing.ts` 142–157 |
| Token persistence: browser `localStorage` — OWNER-ACCEPTED same-user boundary, documented | `taskpane.ts` 129, 144, 158, 673; `settings.rs` 374–378; `docs/LEDGER_THREAT_MODEL.md` |

## 2. Invariants and the tests that prove them

Unit tests live inside the module files (`#[cfg(test)]`); E2E tests drive the real TLS
listener in `tests/word_assist_e2e.rs` (17 tests, tight injectable limits via
`test_limits()`/`spawn_server`, lines 70–118). Add-in tests are in `word-addin/test/`.

| # | Invariant | Proving test(s) |
|---|---|---|
| I1 | Host pin: only `localhost:<port>` accepted | E2E `origin_and_host_pins_reject_mismatches` (e2e.rs:455) |
| I2 | Origin pin: only `https://localhost:<port>`; missing Origin rejected | E2E `origin_and_host_pins_reject_mismatches` (e2e.rs:455) |
| I3 | `/v1/check` and `/v1/accepted` refuse unauthenticated calls; `/v1/health` open; `/v1/pair` authenticates by the code | E2E `health_is_open_but_pair_and_check_are_guarded` (e2e.rs:253); unit `pair_route_is_bearer_exempt_but_v1_routes_are_not` (server.rs:1338) |
| I4 | Body > 64 KiB → 413, connection still sane | E2E `body_over_64k_is_413` (e2e.rs:509); unit `body_cap_is_64k` (server.rs:1319) |
| I5 | Slow-header client closed inside the timeout | E2E `slow_headers_are_closed_within_the_timeout` (e2e.rs:556) |
| I6 | Connection cap closes excess immediately | E2E `connection_cap_closes_excess_immediately` (e2e.rs:593); unit `production_net_limits_match_the_stated_values` (server.rs:1348) |
| I7 | Global 120/min + `/v1/pair` 10/min limiters fire | E2E `pair_route_limiter_bites_at_ten_per_minute` (e2e.rs:911); unit `rate_limiter_windows` (settings.rs:925) |
| I8 | Embedded assets only; nosniff + no-store; CSP on HTML and modules | E2E `static_routes_serve_only_embedded_assets` (e2e.rs:635), `embedded_modules_served_with_secure_headers_and_graph_loads` (e2e.rs:674); unit `csp_blocks_everything_but_self_and_cdn` (server.rs:1324) |
| I9 | Paragraph text never lands in audit log or ledger (sentinel-based) | E2E `sentinel_text_never_lands_in_audit_log_or_ledger` (e2e.rs:955); unit `canonical_is_stable_and_text_free` (ledger.rs:364) |
| I10 | TLS 1.3 negotiable; ALPN is `http/1.1` only | E2E `alpn_is_http11_and_tls13_negotiates` (e2e.rs:1241) |
| I11 | Generated chain matches the model: CA:TRUE, pathlen 0, keyCertSign only, no serverAuth on CA, nameConstraints = DNS localhost + IP 127.0.0.1/32; leaf CA:FALSE, serverAuth-only EKU, SANs exactly localhost+127.0.0.1, validity 3 y/365 d, leaf ⊆ CA, issuer = CA | unit `generated_chain_matches_the_d5_model` (server.rs:1361) |
| I12 | Second `ensure_certificate_chain` call reloads, does not regenerate | unit `chain_load_is_stable_across_calls` (server.rs:1497) |
| I13 | Private-file fallback is 0600 on unix; refused on non-unix | unit `p6_write_private_uses_0600_on_unix` (settings.rs:913), `p6_write_private_refused_on_windows` (settings.rs:894) |
| I14 | Pairing code: one-time (burn on success) | unit `pairing_code_lifecycle_burn_on_success` (settings.rs:937) |
| I15 | Pairing code: TTL 300 s | unit `pairing_code_expires_after_ttl` (settings.rs:959); E2E `pairing_code_expires_after_ttl` (e2e.rs:422) |
| I16 | 5 wrong attempts burn the code; next code gated by doubling cooldown | unit `pairing_burns_on_fifth_failure_and_starts_cooldown` (settings.rs:980); E2E `wrong_code_lockout_and_cooldown` (e2e.rs:351) |
| I17 | No-code state reports NoCode, not LockedOut (no state oracle) | unit `pairing_no_code_reports_nocode_not_lockout` (settings.rs:1021) |
| I18 | Pairing + bearer compares are constant-time | unit `constant_time_eq_basics` (settings.rs:881); call sites `settings.rs:305` and `settings.rs:711` |
| I19 | Token: 32 CSPRNG bytes, persisted as SHA-256 only; round-trips | unit `token_hash_is_sha256_and_persistence_round_trips` (settings.rs:1031), `issue_and_match_token_hash_only` (settings.rs:1067); E2E `pair_success_issues_persistent_token` (e2e.rs:287) |
| I20 | Token: sliding 30-day expiry revokes (including at restore after long closure) | unit `token_sliding_expiry_pure_math` (settings.rs:1054); E2E `token_persists_across_restart_and_revokes` (e2e.rs:817) |
| I21 | Ledger: each entry commits to its predecessor; any tampering detected at open | unit `chain_links_commit_to_their_predecessor` (ledger.rs:387), `tampering_is_detected` (ledger.rs:342) |
| I22 | Export: round-trip verifies; tampered payload, wrong key, swapped signature, malformed input all fail | unit `round_trip_verifies` (export.rs:233), `tampered_payload_fails` (252), `wrong_key_fails` (261), `swapped_signature_fails` (278), `malformed_input_is_err` (293) |
| I23 | Export: text-free, deterministic canonical payload, empty ledger → genesis head | unit `export_is_text_free` (export.rs:307), `canonical_payload_is_deterministic` (333), `empty_ledger_exports_a_genesis_head` (239) |
| I24 | Export cannot sign a tampered ledger (fresh re-open + verify) | code path `export.rs:107–114`; tamper detection itself proven by `ledger.rs:342` |
| I25 | Add-in: every server failure message maps to exactly one client decision; wording pinned | `word-addin/test/pairing.test.ts` (13 cases) |
| I26 | Add-in: token/code-shaped strings never reach the pane display | `sanitizeForDisplay` covered in `word-addin/test/pairing.test.ts` |
| I27 | Tier-1 (model) output is parsed, validated and diffed before any reach into the response cache; stale results discarded | E2E `tier1_mock_loop_updates_cache` (e2e.rs:1028), `stale_result_is_discarded_newest_wins` (e2e.rs:1136) |

## 3. Threat model (10 lines)

1. **Attacker = any local process** on the user's machine (malware, another app). It can reach
   every loopback port; the listener assumes exactly this attacker for the auth model.
2. **Attacker = any web page** in any browser (CSRF/drive-by against `localhost`): blocked by
   the Host pin, the Origin pin, and the absence of any CORS headers.
3. **Attacker = network-adjacent** (same LAN, evil AP): TLS only; the service binds loopback
   and is unreachable off-host in the shipping configuration.
4. **Attacker = hostile document content** flowing through `/v1/check`: 64 KiB + 20 k-char
   caps, timeouts, and tier-0 rules that treat text as data, never as instructions.
5. **Attacker = hostile local model (tier 1)**: every model reply is parsed, validated,
   diffed, ledgered as rejected/failed; nothing reaches the cache unvalidated (I27).
6. **Attacker = compromised/malicious web content in the pane**: strict CSP (`default-src
   'none'`), nosniff, embedded-assets-only serving; scripts only from self + office.js CDN.
7. **Attacker with the pairing UI open**: 6-char code, 5-min TTL, 5-failure global burn,
   doubling cooldown, 10/min route limiter; wrong-guess feedback reveals nothing about state.
8. **Attacker steals the bearer token** (reads `localStorage` — owner-accepted same-user
   boundary): revocation/rotation from the app; 30-day sliding expiry; engine-side store
   keeps the hash only.
9. **Attacker tampers with on-disk state** (ledger.jsonl, exported disclosure): SHA-256 chain
   verified at open and export; export signature binds the aggregate + chain head.
10. **Out of scope / trusted**: the OS keychain, the user's Word process, the local Ollama
    daemon, and the office.js CDN — each is a named trust anchor, not an attacker position.

## 4. NOT tested

Honest gaps. Anything absent here is either covered in §2 or not claimed anywhere.

1. **nameConstraints enforcement by the real Word webview runtime** (spike item (q)). The
   ignored test `generate_owner_test_ca` (server.rs:1519) produces a manual, owner-run test
   kit; nothing automated proves the webview rejects a wrong-SAN leaf under a constrained CA.
2. **TLS 1.2-only / 1.3-only behaviour inside real Word panes** (spike item (l)): which
   protocol the production webview negotiates is a spike outcome, not a test result.
3. **The 20 000-char `/v1/check` cap** (server.rs:713–718) has no dedicated test.
4. **Expired-cert regeneration path** (`load_existing_chain` returning Err → regenerate):
   the load/validate code is exercised, but no test drives a genuinely expired pair.
5. **Real OS keychain behaviour** (Windows Credential Manager, macOS Keychain): unit tests
   cover the seam (`write_private`) and the pure logic; the keyring crate's platform
   backends are exercised only on the CI legs and in owner QA, which are pending.
6. **External-interface unreachability** (that `0.0.0.0` is never bound is structural at
   server.rs:307; no test connects from a second interface — loopback-only sandboxes cannot).
7. **Rate limiter behaviour under real Word load patterns** (bursty pane traffic): the
   windows are unit-tested in isolation; the production calibration is a QA judgement.
8. **Anything in Word itself**: pane rendering, apply/accept flows against the real object
   model, Mac behaviour, and accuracy — all owner QA / spike territory. Not simulated, not
   claimed.
9. **CI on Windows/macOS legs** (D1) and the release dry-run pipeline (D2): workflows exist
   (P5), no green run exists yet. Nothing in this pack may be cited as "CI-verified".
10. **Side channels**: the constant-time compares cover code and token equality; timing of
    the pairing lockout path, cache hits, and rate limiter is not analysed.
