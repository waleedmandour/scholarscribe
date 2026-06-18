# Security Policy

## Reporting a vulnerability

If you find a security vulnerability in ScholarScribe, please report it responsibly:

1. **Do not open a public GitHub issue.**
2. Email the maintainer at: **[TBD — maintainer to add a contact address in this file before v1.0 release]**. (For the pre-release, please use GitHub's private vulnerability reporting: Security tab → "Report a vulnerability".)
3. Include:
   - ScholarScribe version
   - Operating system and version
   - A description of the issue and its impact
   - Steps to reproduce, if applicable

You will receive an acknowledgement within 72 hours. A fix or mitigation will be prioritized based on severity.

## Scope

The following are considered security issues:

- Any way for text the user pastes, opens, or chats about to leave the device without explicit user action.
- Any outbound network connection to a host other than `127.0.0.1:11434` (Ollama) or `registry.ollama.ai` (model downloads).
- Local file access without user consent (i.e., reading files the user did not explicitly pick in a file dialog).
- Remote code execution, sandbox escapes, or privilege escalation.

The following are **not** security issues for this project:

- The app not preventing users from doing things the user is fully authorized to do (e.g., deleting their own models).
- Slow performance or crashes that don't expose data.
- "The local LLM produced harmful text" — the model is the user's choice and runs locally; ScholarScribe is not responsible for model outputs.

## Privacy commitments (security-relevant)

These are also stated in the README but repeated here because they are security commitments:

- **No telemetry.** Ever. No analytics SDK, no crash reporter that uploads data, no "anonymous" usage tracking.
- **No cloud LLM APIs.** The app does not contact OpenAI, Anthropic, Google AI, or any other hosted inference provider.
- **Local files only.** The app reads only files the user explicitly selects via the OS file dialog.
- **Auditable network surface.** All outbound HTTP is in `src-tauri/src/ollama.rs`. The CSP in `src-tauri/tauri.conf.json` blocks the frontend from making any request the Rust backend doesn't authorize.

If you find evidence that any of these commitments is violated in code, that is a security issue and should be reported as above.
