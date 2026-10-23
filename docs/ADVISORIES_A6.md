# Advisory table (A6) — corrected with evidence

Status: SUPERSEDES the advisory table in the Stage A gate report (A6) for the rows
flagged by the owner's Gate-1 review, and restates every other current row so the
table is complete in one place. Nothing else in A6 is changed by this document.

- Date of this correction: 2026-10-24 (Asia/Muscat).
- Audit invocation: `cargo audit` (cargo-audit 0.22.2) against
  `src-tauri/Cargo.lock` at integration tip `4d305f2f1da6485082519aca6c1b5184a2fb2ca4`.
- Advisory DB: `https://github.com/RustSec/advisory-db.git`, loaded 2026-10-23 (UTC),
  1351 advisories. Advisory pages individually read from `https://rustsec.org/...`
  on 2026-10-24 (Asia/Muscat); each row cites its page.
- Baseline result: **21 vulnerabilities, 5 warnings** (VERIFIED — full output kept by
  the agent; counts printed by `cargo audit`: "error: 21 vulnerabilities found!",
  "warning: 5 allowed warnings found").
- The same three TLS versions (rustls 0.23.45, rustls-webpki 0.103.15,
  webpki-roots 1.0.9) are locked on `main` @ 04b4719 (VERIFIED —
  `git show main:src-tauri/Cargo.lock`).

## 1. What actually resolves (this is where the old table was wrong)

All statements in this subsection are VERIFIED by the listed command, run in
`src-tauri/` at the integration tip:

| Question | Answer | Evidence |
|---|---|---|
| Does the **default build** (no `word-assist` feature) compile rustls? | **No.** rustls resolves in no feature graph of the default build on any target. | `cargo tree -i rustls --target all` → "nothing to print"; same for `cargo tree -e features -i rustls` (host) |
| Does the **word-assist build** resolve rustls? | Yes: `rustls 0.23.45` ← `scholarscribe` (direct, `Cargo.toml:65`) and `tokio-rustls 0.26.6` ← `scholarscribe`. | `cargo tree --features word-assist -i rustls --target all` |
| Which rustls features resolve? | `ring`, `logging`/`log`, `std`, `tls12`. **Not** `brotli`, **not** `aws-lc-rs`, **not** `custom-provider`. | `cargo tree --features word-assist -e features -i rustls` |
| Is rustls's `brotli` feature enabled anywhere? | **No.** `brotli 9.0.0` IS in the lock, but resolves only via `tauri-codegen 2.7.2` (asset pre-compression), which is unrelated to rustls's decompressor. | `cargo tree --features word-assist -i brotli` |
| Which rustls-webpki features resolve? | `alloc`, `ring`, `std` — pulled only by rustls. | `cargo tree --features word-assist -e features -i rustls-webpki` |
| Does `webpki-roots 1.0.9` resolve in any build? | **No.** Its only reverse edge is `hyper-rustls`, and `hyper-rustls` itself resolves in no feature graph (lockfile-only entry). | `cargo tree --features word-assist -i webpki-roots` and `-i hyper-rustls` → both "nothing to print" |
| What TLS stack carries the app's **outbound HTTPS** (reqwest)? | `reqwest 0.12.28` with its default feature set → `default-tls` → `native-tls 0.2.18` (OpenSSL on Linux / schannel on Windows / Security framework on macOS). **reqwest does not use rustls in this app.** | `cargo tree --features word-assist -e features -i native-tls` shows `reqwest feature "default-tls"` ← `reqwest feature "default"` ← `scholarscribe` |
| Who requests client certificates? | **Nobody.** The listener is configured `.with_no_client_auth()`; the app never builds a rustls `ClientConfig`. | `src/assist/server.rs:301`; no `ClientConfig`/`client_auth`/`WebPkiClientVerifier` matches in `src/` except tests asserting serverAuth-only EKU (`src/assist/server.rs:1445`) |
| Does the listener trust public web-PKI roots (emSign etc.)? | **No.** It serves a leaf signed by the app's own ephemeral CA (H5) and never builds a public root store; `webpki-roots` is not resolved. | `src/assist/server.rs:296-305`; tree evidence above |
| Does the listener use CRLs? | **No.** No `crl`/`CertRevocationList` references in `src/`. | `grep -rn "crl\|CertRevocationList" src/ --include="*.rs"` → empty |

### The exposure split (stated separately, as required)

1. **The Word Assist listener** (the only rustls code in the tree; `word-assist`
   feature only; loopback 127.0.0.1 only): a rustls **server** with explicit ring
   provider, explicit `[TLS1.3, TLS1.2]`, ALPN `http/1.1`, no client certificates,
   no CRLs, no ECH, no resumption store customization, ephemeral-CA-only trust.
2. **The app's outbound HTTPS** (`reqwest`): rides on **native-tls**, not rustls.
   rustls advisories therefore do not describe the outbound path at all. The old
   table's framing that rustls advisories touched "the app's outbound HTTPS" was
   wrong (NOT VERIFIED then; contradicted by the tree evidence above).
3. **The default build** (what most users run without the add-in): rustls is not
   even compiled.

## 2. Corrected rows — rustls 0.23.45

Every advisory page cited was read on 2026-10-24 (Asia/Muscat). "Patched" is the
advisory's own statement.

### RUSTSEC-2026-0348 — Brotli certificate decompression can allocate up to 1 GiB before authentication
- URL: https://rustsec.org/advisories/RUSTSEC-2026-0348 (read 2026-10-24). CVSS 3.7 LOW. Patched `>=0.23.46`; unaffected `<0.23.9`.
- Affected configurations per the advisory: clients with the non-default `brotli` feature connecting to a malicious server; **or** servers with `brotli` requesting client certificates. "Applications that do not enable the `brotli` feature are not affected."
- **Re-label: NOT AFFECTED (two independent reasons, both VERIFIED):**
  1. rustls's `brotli` feature is not enabled (`cargo tree` evidence, §1).
  2. Even with it, the server-side condition requires requesting client
     certificates; the listener does not (`server.rs:301`).
- The old row's implication of exposure is withdrawn. (The owner's stated
  pre-condition — non-default brotli feature plus a server requesting client
  certificates — is confirmed by the advisory text.)
- Disposition: **bump to 0.23.46** in the P2 lockfile branch anyway, to clear the
  advisory from `cargo audit` output.

### RUSTSEC-2026-0346 — `Debug` impls for client session values expose TLS secrets
- URL: https://rustsec.org/advisories/RUSTSEC-2026-0346 (read 2026-10-24). CVSS 4.0 MEDIUM (local). Patched `>=0.23.46`; unaffected `<0.21.0`.
- Affected code path per the advisory: **client-side** — custom `ClientSessionStore`
  implementations that `Debug`-format `Tls12ClientSessionValue`/`Tls13ClientSessionValue`.
- **Re-label: NOT AFFECTED (VERIFIED):** the app never builds a rustls client
  (no `ClientConfig`, no custom session store); the listener is a server and rustls
  server resumption storage is a different type; no `Debug` formatting of session
  values exists in `src/` (grep evidence, §1).
- Disposition: bump to 0.23.46 in P2 (same reason).

### RUSTSEC-2026-0349 — Unauthenticated ServerHello can downgrade later connections to classical key exchange
- URL: https://rustsec.org/advisories/RUSTSEC-2026-0349 (read 2026-10-24). CVSS 3.7 LOW. Patched `>=0.23.46`.
- Affected code path per the advisory: **client-side** — the client's
  pre-authentication record of the server-selected key exchange group in its
  session store; "Clients with resumption disabled (as nothing is stored)" are
  unaffected.
- **Re-label: NOT AFFECTED (VERIFIED):** client-side condition; no rustls client
  exists in the tree (§1). The listener's loopback-only server handshake is not
  the described path.
- Disposition: bump to 0.23.46 in P2 (same reason).

### RUSTSEC-2026-0347 — Buffered application data sent to outer server after ECH rejection
- URL: https://rustsec.org/advisories/RUSTSEC-2026-0347 (read 2026-10-24). CVSS 2.3 LOW. Patched `>=0.23.46`; unaffected `<0.23.44`.
- Affected code path per the advisory: **client-side**, only with
  `EchMode::Enable`, writing app data before handshake completion, and an
  ECH-rejecting server.
- **Re-label: NOT AFFECTED (VERIFIED):** no rustls client, no ECH usage anywhere
  (`grep -rn "EchMode\|ech" src/assist/*.rs` → no ECH API usage).
- Disposition: bump to 0.23.46 in P2 (same reason).

## 3. Corrected rows — rustls-webpki 0.103.15 and webpki-roots 1.0.9

### RUSTSEC-2026-0382 — Certificate verification may accept distrusted intermediates (emSign)
- URL: https://rustsec.org/advisories/RUSTSEC-2026-0382.html (read 2026-10-24). Aliases CVE-2026-111943. Patched `>=0.103.16` (and `>=0.104.0-alpha.8`).
- Affected code path: path validation accepts chains through distrusted emSign
  intermediate keys **"when the configured trust anchors allow such paths"**.
- **Label: NOT EXPLOITABLE in our configuration (VERIFIED); version is inside the
  advisory range (VERIFIED).** The listener's only trust anchor is the app's own
  ephemeral CA (H5); emSign public intermediates can never be in that store, and
  the server does no client-certificate verification anyway. The default build
  does not compile rustls-webpki.
- Disposition: **bump to 0.103.16** in P2 (patch bump, clears the range).

### RUSTSEC-2026-0381 — webpki-roots bundled roots include emSign ECC Root CA - G3
- URL: https://rustsec.org/advisories/RUSTSEC-2026-0381.html (read 2026-10-24). Patched `>=1.0.10`.
- **Label: NO RUNTIME EXPOSURE (VERIFIED):** `webpki-roots 1.0.9` is in
  `Cargo.lock` but resolves in no build graph (its only reverse edge,
  `hyper-rustls`, is itself unresolved) — nothing ships it. The listener does not
  use public roots at all.
- Disposition: **bump to 1.0.10** in P2 (patch bump) to silence the advisory and
  keep the lock clean if a future change ever activates `hyper-rustls`.

## 4. Redone rows — zip (2.4.2 direct; 7.2.0 and 8.6.0 transitive) and quick-xml (0.31.0 direct; 0.38.4 transitive)

**Framing change (owner ruling):** "attacker-free" is **withdrawn**. A `.docx`
or `.bib` received from a collaborator, downloaded from a journal site, or
opened from any folder is **untrusted input**. Every row below is written from
that premise. (The listener's own HTTPS-facing surface is a *different* untrusted
channel and is covered by H3 limits; this section is about document files.)

Version attribution (VERIFIED, reverse edges from `Cargo.lock`):
- `zip 2.4.2` ← `scholarscribe` (direct; `deflate` feature only, `default-features = false`, `Cargo.toml:39`)
- `zip 7.2.0` ← `candle-core 0.9.2` (transitive)
- `zip 8.6.0` ← `harper-core 2.11.0` (transitive)
- `quick-xml 0.31.0` ← `scholarscribe` (direct)
- `quick-xml 0.38.4` ← `plist 1.8.0` (transitive)

### RUSTSEC-2026-0367 — SUID/SGID bit preservation during ZIP extraction (CRITICAL)
- URL: https://rustsec.org/advisories/RUSTSEC-2026-0367.html (read 2026-10-24). Patched `>=9.0.0`.
- Condition per advisory: a privileged process calls `ZipArchive::extract()` /
  stream extraction on an attacker-controlled archive.
- **Label: version in range (VERIFIED — audit row present for 2.4.2/7.2.0/8.6.0);
  vulnerable code path NOT exercised (VERIFIED):** the app never calls
  `.extract()` / `ZipStreamReader` (grep, §4 evidence below); all zip access is
  in-memory `by_index` reads. Parsed on 2026-10-24.
- Disposition: **cannot be fixed by a lockfile-only patch bump** (fix = 9.0.0,
  a major bump). Out of P2 scope; queued in the P3 untrusted-input audit and for
  the owner's Dependabot/policy decision.

### RUSTSEC-2026-0368 — `ZipArchive::extract` creates symlinks outside the destination
- URL: https://rustsec.org/advisories/RUSTSEC-2026-0368.html (read 2026-10-24). Patched `>=9.0.0`.
- Same disposition as -0367 (extraction-only path, unused; major-bump fix).

### RUSTSEC-2026-0369 — Permissions applied through symlinks during extraction
- URL: https://rustsec.org/advisories/RUSTSEC-2026-0369.html (read 2026-10-24). Patched `>=9.0.0`.
- Same disposition as -0367/-0368.

### RUSTSEC-2025-0168 — Path canonicalization / arbitrary file write during extraction
- URL: https://rustsec.org/advisories/RUSTSEC-2025-0168.html (read 2026-10-24). Patched `>=2.3.0` for `extract`/stream-extract.
- **Label: NOT AFFECTED (VERIFIED):** all locked zip versions are ≥ 2.3.0, and
  cargo audit does not report it (consistent).

### RUSTSEC-2026-0194 — Quadratic attribute duplicate check (quick-xml)
- URL: https://rustsec.org/advisories/RUSTSEC-2026-0194.html (read 2026-10-24). CVSS 7.5 HIGH. Patched `>=0.41.0`.
- Affected code paths per advisory: attribute iteration with the default
  duplicate check, `BytesStart::try_get_attribute`, and `NsReader`.
- **Label: AFFECTED PATH REACHABLE ON UNTRUSTED INPUT (VERIFIED).** Both app
  parsers iterate attributes with default checks on XML extracted from
  collaborator-supplied `.docx` files: `src/docx_reading.rs:209`
  (`Reader::from_str`), `src/docx_reading.rs:299-307` (`e.attributes()`),
  `src/docx_to_latex.rs:131`, `src/docx_to_latex.rs:285` (same helper pattern).
  The input is the decompressed `word/document.xml` of a hostile file, so a
  single multi-megabyte start tag can pin a parsing thread (advisory: ~6 s at
  80k attributes, ~10 min at 800k, O(N²)). This is CPU exhaustion inside the
  desktop app (one core; UI stays responsive but the operation stalls).
- Disposition: fix requires **0.41.0 — a minor bump** → excluded from P2 by the
  lockfile-only rule. Owner decision needed (Dependabot has `quick-xml-0.38.4`
  open for the 0.38.4 leg only; the direct 0.31.0 leg needs a Cargo.toml
  change + code port). Interim mitigation is a P3 proposal (input-size caps);
  it does NOT cure the quadratic path.

### RUSTSEC-2026-0195 — Unbounded namespace-declaration allocation in `NsReader` (quick-xml)
- URL: https://rustsec.org/advisories/RUSTSEC-2026-0195.html (read 2026-10-24). CVSS 7.5 HIGH. Patched `>=0.41.0`.
- Affected code path per advisory: `NsReader` / direct `NamespaceResolver::push`. "A plain `Reader` that does not perform namespace resolution is not affected."
- **Label: NOT AFFECTED in our code (VERIFIED):** the app uses plain
  `quick_xml::Reader`, never `NsReader` (`grep -rn "NsReader" src/` → empty).
  The audit row exists only because the version is inside the advisory range;
  the plist crate (which pulls 0.38.4) is exercised on macOS-only plists, not
  document XML.
- Disposition: same as -0194 (minor bump required; owner/Dependabot decision).

## 5. Unchanged context rows (verified this audit, unchanged verdicts)

| Crate @ locked version | Advisory | One-line status (evidence: advisory page read 2026-10-24) |
|---|---|---|
| ammonia 4.2.3 ← harper-core | RUSTSEC-2026-0373 | CSS `URL("")` filter bypass in sanitizer; app never configures ammonia (no direct usage in `src/`); transitive via harper. INFERRED not affected; patched `>=4.2.4` if the owner wants the row clean (minor bump, not P2 scope). |
| time 0.3.36 | RUSTSEC-2026-0009 | Stack exhaustion in RFC 2822 parsing; app code parses no RFC 2822 (no `parse_from_rfc2822` in `src/`); transitive users (rcgen/x509-parser/plist/cookie/serde_with) parse DER/structured data, not hostile RFC 2822. INFERRED not affected; fix = `>=0.3.47` (minor bump, not P2 scope). |
| bincode 2.0.1 | RUSTSEC-2025-0141 (unmaintained) | Warning only; owner's dependency policy decision. |
| paste 1.0.15 | RUSTSEC-2024-0436 (unmaintained) | Warning only. |
| proc-macro-error 1.0.4 | RUSTSEC-2024-0370 (unmaintained) | Warning only. |
| glib 0.18.5 (Linux GTK leg) | RUSTSEC-2024-0429 (unsound) | Warning only; `VariantStrIter` not used by app code (no direct glib usage). |
| tokio 1.53.2 | RUSTSEC-2026-0386 (unsound) | `AsyncFd` I/O-safety; app does not call `AsyncFd::new` (no matches in `src/`); internal to tokio users. INFERRED not affected. |

## 6. What P2 fixes vs. what it cannot

- **P2 (lockfile-only, patch bumps)** fixes the audit rows for: rustls
  (0.23.45 → 0.23.46: RUSTSEC-2026-0346/-0347/-0348/-0349),
  rustls-webpki (0.103.15 → 0.103.16: RUSTSEC-2026-0382),
  webpki-roots (1.0.9 → 1.0.10: RUSTSEC-2026-0381).
- **P2 cannot** (requires minor/major bumps — excluded by the owner's rule):
  - quick-xml 0.31.0/0.38.4 → 0.41.x (RUSTSEC-2026-0194/-0195) — minor.
  - zip 2.4.2/7.2.0/8.6.0 → 9.x (RUSTSEC-2026-0367/-0368/-0369) — major.
  - time 0.3.36 → 0.3.47 (RUSTSEC-2026-0009) — minor.
  - ammonia 4.2.3 → 4.2.4 (RUSTSEC-2026-0373) — patch! But it is pulled by
    harper-core 2.11.0; whether `cargo update -p ammonia --precise 4.2.4`
    succeeds without touching harper is a P2-time check, reported if attempted.
- Dependabot status on origin (VERIFIED 2026-10-24 via `git ls-remote --heads`):
  branches exist for `crypto`, `harper-core 2.13.0`, `http`, `quick-xml 0.38.4`,
  `sysinfo`, `tauri`, `thiserror`, `time 0.3.55`, `zip 8.6.0`; **no rustls,
  rustls-webpki, webpki-roots, or ammonia branch exists** — the P2 bumps are not
  already open.
