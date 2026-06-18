# Contributing to ScholarScribe

Thanks for your interest in contributing. ScholarScribe is a small project with a focused ethical scope — please read this document before opening a PR.

## Read this first

- [README.md](README.md) — what the project does and doesn't do.
- [docs/ETHICS.md](docs/ETHICS.md) — the ethical-use policy. **This is load-bearing.** PRs that add features outside this policy will be closed.

## What contributions are welcome

- **Bug fixes** for any existing module.
- **New disclosure templates** for venues with public AI-use policies (please link to the official policy in your PR).
- **New recommended models** in the catalog — must be open-licensed and available via Ollama.
- **Documentation improvements** — typos, clarity, translations.
- **Accessibility fixes** — keyboard navigation, screen reader support, contrast.
- **Performance improvements** that don't compromise privacy.
- **Tests.** The project currently has very few; well-scoped test PRs are very welcome.

## What contributions are not welcome

- Features whose purpose is to evade AI-detection systems. See [docs/ETHICS.md](docs/ETHICS.md) §2.
- Telemetry, analytics, or "anonymous usage reporting" of any kind.
- Cloud LLM API integrations (OpenAI, Anthropic, Google AI, etc.). ScholarScribe is local-only by design.
- Code that phones home for updates, version checks, or "feature flags" loaded from a remote server.

## Development setup

Prerequisites: Rust 1.77+, Node.js 18+, Tauri 2 prerequisites (see <https://v2.tauri.app/start/prerequisites/>).

```powershell
git clone https://github.com/waleedmandour/scholarscribe.git
cd scholarscribe
npm install
npm run tauri dev
```

The app opens in dev mode with hot reload on the frontend. Rust changes trigger a rebuild.

## Code style

- Rust: `cargo fmt` and `cargo clippy -- -D warnings` should both pass.
- TypeScript/Svelte: `npm run check` should pass.
- Keep dependencies minimal. Every new crate or npm package adds to the installer size — justify it in the PR description.

## PR checklist

- [ ] PR description explains what changed and why.
- [ ] If the change touches `ollama.rs`, the privacy claims in the README still hold (no new outbound hosts).
- [ ] If the change adds a feature, it's consistent with `docs/ETHICS.md`.
- [ ] `cargo fmt`, `cargo clippy -- -D warnings`, and `npm run check` all pass.

## Reporting issues

Open a GitHub issue with:

- ScholarScribe version (from the sidebar footer).
- Windows version.
- Ollama version (`ollama --version` in a terminal).
- Model used.
- Steps to reproduce.
- Expected vs. actual behavior.

For suspected security issues, see [SECURITY.md](SECURITY.md) instead of opening a public issue.

## License

By contributing, you agree your contributions will be licensed under the MIT license, the same as the rest of the project.
