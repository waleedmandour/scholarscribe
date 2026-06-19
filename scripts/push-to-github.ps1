# Initialize git, commit, and push ScholarScribe to a new GitHub repo.
#
# IMPORTANT: use a fresh personal access token you generated AFTER the previous
# one was revoked. Never paste a token directly into a script — instead, this
# script uses the GitHub CLI (`gh`) which stores credentials in your OS
# credential manager. If you don't have `gh`, install it from
# https://cli.github.com/ and run `gh auth login` once before running this.

param(
    [Parameter(Mandatory=$true)]
    [string]$RepoName = "scholarscribe",
    [string]$Description = "Privacy-first local LLM writing companion for researchers",
    [switch]$Public  # default: private repo. Pass -Public to make it public.
)

$ErrorActionPreference = "Stop"

function Write-Step($msg) { Write-Host "`n==> $msg" -ForegroundColor Cyan }
function Write-Ok($msg)   { Write-Host "    OK: $msg" -ForegroundColor Green }
function Write-Warn($msg) { Write-Host "    !! $msg" -ForegroundColor Yellow }

$projectRoot = Split-Path -Parent $PSScriptRoot
Push-Location $projectRoot
try {
    Write-Step "Initializing git repository"
    if (-not (Test-Path ".git")) {
        git init
        git branch -M main
        Write-Ok "git initialized"
    } else {
        Write-Ok "git already initialized"
    }

    Write-Step "Staging files"
    git add .
    $status = git status --porcelain
    if (-not $status) {
        Write-Ok "Working tree clean — nothing to commit"
    } else {
        git commit -m "v0.1.0 pre-release: ScholarScribe local LLM writing companion

- Tauri 2 + Svelte 4 + Ollama backend (Windows .msi)
- Four modules: Models, Style Analysis, Disclosure Assistant, Detector Literacy
- Privacy: zero telemetry, local-only inference, single outbound host (registry.ollama.ai for model downloads)
- Explicit ethical-use policy: no detection-evasion features (see docs/ETHICS.md)"
        Write-Ok "Initial commit created"
    }

    Write-Step "Checking GitHub CLI"
    if (-not (Get-Command gh -ErrorAction SilentlyContinue)) {
        Write-Warn "GitHub CLI (gh) not found. Install from https://cli.github.com/"
        Write-Warn "Then run 'gh auth login' and re-run this script."
        exit 1
    }

    $authStatus = gh auth status 2>&1
    if ($LASTEXITCODE -ne 0) {
        Write-Warn "Not logged in to GitHub. Run 'gh auth login' first."
        exit 1
    }
    Write-Ok "GitHub CLI authenticated"

    Write-Step "Creating GitHub repository: $RepoName"
    $createArgs = @("repo", "create", $RepoName, "--source=.", "--description=$Description", "--push")
    if ($Public) { $createArgs += "--public" } else { $createArgs += "--private" }
    & gh @createArgs

    if ($LASTEXITCODE -eq 0) {
        Write-Ok "Repository created and code pushed"
    } else {
        Write-Warn "Repository creation or push failed. Check the error above."
        exit 1
    }

    Write-Step "Creating v0.1.0 pre-release tag"
    git tag -a v0.1.0-pre -m "v0.1.0 pre-release"
    git push origin v0.1.0-pre

    $releaseUrl = gh release create v0.1.0-pre `
        --title "v0.1.0 (Pre-release)" `
        --notes "First pre-release of ScholarScribe.

## What's included

- **Models tab** — Download and manage open LLMs (Gemma 2, Qwen 2.5, Llama 3.1, Phi-3) via Ollama.
- **Chat tab** — Local-only chat with system-prompt guardrail (refuses evasion requests).
- **Style Analysis tab** — Compare a draft to your own prior writing (sentence length, hedging, passive voice, etc.).
- **Disclosure Assistant tab** — Generate venue-compliant AI-use statements for ICMJE, Nature, IEEE, Elsevier, ACL.
- **Detector Literacy tab** — Educational content on how detectors work and where they fail.

## Install

1. Install [Ollama](https://ollama.com/download).
2. Download and run the ScholarScribe .msi installer (attached to this release).
3. Open ScholarScribe → Models tab → download a model.
4. See USER_MANUAL.md for the full walkthrough.

## Privacy

Zero telemetry. Single outbound host (registry.ollama.ai, only when downloading a model). All inference runs on your device via Ollama. See SECURITY.md for the full audit guide.

## Ethical use

ScholarScribe does NOT include detection-evasion features. See docs/ETHICS.md for the full policy." `
        --prerelease

    if ($LASTEXITCODE -eq 0) {
        Write-Ok "Pre-release created: $releaseUrl"
    } else {
        Write-Warn "Release creation failed. You can create it manually from the GitHub web UI."
    }

    Write-Host "`n=============================" -ForegroundColor White
    Write-Host "ScholarScribe is now on GitHub." -ForegroundColor White
    Write-Host "Repo:    https://github.com/$(gh api user --jq .login)/$RepoName" -ForegroundColor White
    Write-Host "Release: $releaseUrl" -ForegroundColor White
} finally {
    Pop-Location
}
