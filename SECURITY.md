# Security Policy

Applies to ScholarScribe v2.2.1 r5. The authoritative machine-readable list of hosts this app can contact is `docs/NETWORK_SURFACE.json`; this file and that JSON are kept in sync and CI fails if they drift (see `.github/workflows/ci.yml`, job `network-surface`).

## Reporting a vulnerability

If you find a security vulnerability in ScholarScribe, please report it responsibly:

1. **Do not open a public GitHub issue.**
2. **Use GitHub's private vulnerability reporting**, the Security tab of this repository, button "Report a vulnerability". This is the only reporting channel. There is no security email address; none has been published by the maintainer.
3. Include:
   - ScholarScribe version
   - Operating system and version
   - A description of the issue and its impact
   - Steps to reproduce, if applicable

You will receive an acknowledgement within 72 hours. A fix or mitigation will be prioritized based on severity.

## Scope

### Outbound network surface (complete list)

ScholarScribe distinguishes **which process opens the connection**. Both classes are auditable in the app's Privacy Audit tab.

**Connections made by the ScholarScribe process itself:**

| Host | Purpose | When |
|---|---|---|
| `http://127.0.0.1:11434` (and `http://localhost:11434`, CSP only) | The local Ollama daemon: status, model list, pull/delete, chat, GGUF import | Default for status; user action for everything else |
| `https://oauth2.googleapis.com/token` | Google OAuth code exchange and refresh | Only after the user explicitly connects a Google Doc (opt-in) |
| `https://accounts.google.com/o/oauth2/v2/auth` | Google OAuth consent page, opened in the system browser (never the webview) | Only after the user explicitly connects a Google Doc (opt-in) |
| `https://www.googleapis.com/drive/v3` | Drive revision list and revision download | Only after the user explicitly connects a Google Doc (opt-in) |
| `https://docs.google.com/feeds/download/documents/export/Export` | Fallback plain-text revision export | Only as a fallback in the same opt-in flow |
| `http://127.0.0.1:{random port}` (inbound listener) | Loopback receiver for the OAuth redirect | Only during the same opt-in flow |
| `https://localhost:38080` (inbound listener, 127.0.0.1 only; port user-configurable) | Word Assist loopback listener (feature `word-assist`): serves the add-in pages and the `/v1` check API to Word's task pane over TLS with a per-install self-signed certificate scoped to `localhost` | Only after the Word Assist disclosure dialog is accepted; one click stops it and wipes the pairing |
| `https://appsforoffice.microsoft.com` (fetched by Word's webview, not by ScholarScribe) | The Word Assist task pane loads the office.js library from Microsoft's CDN; this fetch never carries document text | Only while the Word Assist pane is open in Word |

**Connections made by the Ollama daemon at the user's request** (the app only tells the local daemon what to fetch; these hosts are contacted by the daemon, not by ScholarScribe):

| Host | Purpose | When |
|---|---|---|
| `registry.ollama.ai` | Model library downloads | Only when the user pulls a library model |
| `hf.co` / `huggingface.co` | Hugging Face GGUF pulls | Only when the user pulls a model referenced as `hf.co/{user}/{repo}` |

### Word Assist privacy statement (Preview, feature builds)

- **Document text never leaves the device through ScholarScribe.** While Word Assist is on, text flows Word task pane → `https://localhost:<port>` (loopback, TLS) → the ScholarScribe process → (for Tier 1 rewrites only) the local Ollama daemon at `127.0.0.1:11434`. Every hop is on the user's machine. There is no other destination for document text; the ledger, the audit log, and the signed `language_assist.json` export contain counts and hashes, never text.
- **office.js CDN fetch.** The task-pane page loads the office.js library from `https://appsforoffice.microsoft.com` (Microsoft's supported CDN for add-ins). That fetch is made by **Word's webview**, not by ScholarScribe, and carries no document text (the library is fetched before any document access). This host is the single `addin-webview` entry in `docs/NETWORK_SURFACE.json`.
- **Word's own traffic.** Word itself makes its own connections (telemetry, document sync, Copilot, and other Microsoft services) that ScholarScribe does not initiate, proxy, or log. The add-in spike protocol includes a network capture (spike item (h)) whose owner-run results are recorded in `word-addin/spike/OBSERVATIONS.md`; until that capture lands and is reviewed, this document makes **no claim** about Word's own traffic beyond stating that ScholarScribe is not party to it.
- **What the listener exposes.** Only `/v1` endpoints (pair/check/accepted/health), pinned to `Host: localhost:<port>` with a matching `Origin` check, a bearer token obtained by a one-time 6-character pairing code (5-minute TTL, burned on use), a 64 KiB body cap, and a 120-requests/60-seconds rate limit. No CORS headers are emitted; the add-in page and the API share one origin.
- **Known hardening gaps in this preview** (tracked in `docs/RELEASE_PLAN_v0.1.md`): no failed-pairing-attempt lockout beyond the global rate limit; the paired token lives in memory only (a restart requires re-pairing); the TLS listener uses rustls defaults rather than an explicit version floor; the certificate is a self-signed leaf (rcgen default validity) rather than the ruled temporary-CA model. All are preview-accepted with the residual risks stated there.

**Standalone verifier page (`verifier/index.html`, not part of the installed app):** loads crypto libraries from local vendored files by default; falls back to `cdn.jsdelivr.net` only if the vendored copies are missing.

Everything else that appears in the repository (developer PowerShell scripts, npm/crates.io at build time, documentation links opened in the user's browser) is outside the app's runtime surface and is inventoried in `docs/NETWORK_SURFACE.json`.

The following are considered security issues:

- Any way for text the user pastes, opens, or chats about to leave the device without explicit user action (the only designed exception is the opt-in Google Docs import, which inherently requires the document's revision text to be fetched from Google; this is disclosed in the Provenance tab and logged in the Privacy Audit tab).
- Any outbound connection from the ScholarScribe process or the Ollama daemon to a host not listed above and not in `docs/NETWORK_SURFACE.json`.
- Local file access without user consent (i.e., reading files the user did not explicitly pick in a file dialog or open through the app's documented open-file commands).
- Remote code execution, sandbox escapes, or privilege escalation.

The following are **not** security issues for this project:

- The app not preventing users from doing things the user is fully authorized to do (e.g., deleting their own models).
- Slow performance or crashes that don't expose data.
- "The local LLM produced harmful text": the model is the user's choice and runs locally; ScholarScribe is not responsible for model outputs. Note that arbitrary user-installed models (including uncensored ones) may not follow the app's prompt-level guardrails; guardrail behavior is best-effort and depends on the model.

## Where the network code lives

- Local Ollama client: `src-tauri/src/ollama.rs` and `src-tauri/src/abstract_generator.rs`
- Google Docs bridge (the only outbound HTTPS besides Ollama): `src-tauri/src/google_docs_net.rs`, wired by `src-tauri/src/google_docs_commands.rs`
- Word Assist loopback listener (inbound, opt-in): `src-tauri/src/assist/server.rs`, wired by `src-tauri/src/assist_commands.rs`; the add-in pages it serves live in `word-addin/dist/`
- Tauri CSP restricting the webview to loopback Ollama: `src-tauri/tauri.conf.json`
- Tauri capabilities: `src-tauri/capabilities/default.json` (only `core`, `dialog`, and `shell` defaults; no http or fs permissions)

## Privacy commitments (security-relevant)

These are also stated in the README but repeated here because they are security commitments:

- **No telemetry.** Ever. No analytics SDK, no crash reporter that uploads data, no "anonymous" usage tracking.
- **No cloud LLM APIs.** The app does not contact OpenAI, Anthropic, Google AI, or any other hosted inference provider. Model inference happens only through the user's local Ollama daemon.
- **Local files only.** The app reads only files the user explicitly selects via the OS file dialog or opens through the app's documented commands.
- **Auditable network surface.** Every outbound call site is listed above and in `docs/NETWORK_SURFACE.json`. Each ScholarScribe-initiated call is appended to the in-memory audit log shown live in the Privacy Audit tab. The CSP in `src-tauri/tauri.conf.json` blocks the frontend webview from making any request the Rust backend doesn't authorize (`connect-src` is loopback Ollama only).
- **ScholarScribe vs Ollama traffic.** The Privacy Audit tab labels ScholarScribe's own connections. Connections that the Ollama daemon makes to `registry.ollama.ai` or `hf.co` happen inside the daemon process and are initiated only by the user's explicit pull request; they are not visible in the app's audit log, and the app never sends manuscript text to any of those hosts.

If you find evidence that any of these commitments is violated in code, that is a security issue and should be reported as above.
