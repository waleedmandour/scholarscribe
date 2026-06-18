# ScholarScribe — one-shot build + release script.
#
# Builds the .msi installer, creates the GitHub repo, pushes the code,
# tags v0.1.0-pre, creates a pre-release, and attaches the .msi to it.
#
# Usage:
#   1. Revoke your old GitHub token at https://github.com/settings/tokens
#   2. Generate a new token with `repo` scope only.
#   3. Install GitHub CLI:  winget install --id GitHub.cli
#   4. Run:  gh auth login   (choose HTTPS, paste token when prompted)
#   5. Run this script:
#        .\scripts\release.ps1 -RepoName scholarscribe
#        .\scripts\release.ps1 -RepoName scholarscribe -Public   # for a public repo
#
# Do NOT pass your token as a parameter to this script. gh stores it in the
# OS credential manager; that's the only safe place for it.

param(
    [Parameter(Mandatory=$true)]
    [string]$RepoName,
    [string]$Description = "Privacy-first local LLM writing companion for researchers",
    [switch]$Public,
    [switch]$SkipBuild       # set if you already ran build-windows.ps1 and just want to push
)

$ErrorActionPreference = "Stop"

function Write-Step($msg) { Write-Host "`n==> $msg" -ForegroundColor Cyan }
function Write-Ok($msg)   { Write-Host "    OK: $msg" -ForegroundColor Green }
function Write-Warn($msg) { Write-Host "    !! $msg" -ForegroundColor Yellow }
function Write-Err($msg)  { Write-Host "    ERROR: $msg" -ForegroundColor Red }

$projectRoot = Split-Path -Parent $PSScriptRoot
Push-Location $projectRoot
try {
    Write-Host "ScholarScribe release pipeline" -ForegroundColor White
    Write-Host "==============================" -ForegroundColor White
    Write-Host "Project root: $projectRoot"
    Write-Host "Repo name:    $RepoName"
    Write-Host "Visibility:   $(if ($Public) { 'public' } else { 'private' })"
    if (-not $SkipBuild) {
        Write-Host "Build:        yes (use -SkipBuild to skip)"
    } else {
        Write-Host "Build:        skipped"
    }

    # --- Preflight checks ---
    Write-Step "Preflight: checking tools"

    if (-not (Get-Command git -ErrorAction SilentlyContinue)) {
        Write-Err "git not found. Install from https://git-scm.com/download/win"
        exit 1
    }
    Write-Ok "git found: $(git --version)"

    if (-not (Get-Command gh -ErrorAction SilentlyContinue)) {
        Write-Err "GitHub CLI (gh) not found. Install: winget install --id GitHub.cli"
        exit 1
    }
    Write-Ok "gh found: $(gh --version | Select-Object -First 1)"

    $authOut = gh auth status 2>&1
    if ($LASTEXITCODE -ne 0) {
        Write-Err "Not logged in to GitHub."
        Write-Host "    Run: gh auth login" -ForegroundColor Yellow
        Write-Host "    Choose: GitHub.com → HTTPS → Yes (use git) → Login with a browser" -ForegroundColor Yellow
        Write-Host "    (or paste a fresh token from https://github.com/settings/tokens/new)" -ForegroundColor Yellow
        exit 1
    }
    $ghUser = gh api user --jq .login
    Write-Ok "authenticated as @$ghUser"

    if (-not (Get-Command node -ErrorAction SilentlyContinue)) {
        Write-Err "Node.js not found. Install LTS from https://nodejs.org/"
        exit 1
    }
    Write-Ok "node found: $(node --version)"

    if (-not (Get-Command cargo -ErrorAction SilentlyContinue)) {
        Write-Err "Rust not found. Install: winget install --id Rustlang.Rustup"
        exit 1
    }
    Write-Ok "cargo found: $(cargo --version)"

    # --- Build ---
    $msiPath = $null
    if (-not $SkipBuild) {
        Write-Step "Installing npm dependencies"
        npm install --no-audit --no-fund
        if ($LASTEXITCODE -ne 0) { Write-Err "npm install failed"; exit 1 }
        Write-Ok "npm install complete"

        Write-Step "Running cargo check (first run downloads ~400 crates, 5-10 min)"
        Push-Location "$projectRoot\src-tauri"
        try {
            cargo check
            if ($LASTEXITCODE -ne 0) { Write-Err "cargo check failed"; exit 1 }
        } finally {
            Pop-Location
        }
        Write-Ok "cargo check passed"

        Write-Step "Building .msi installer (10-20 min on first run)"
        npm run tauri build
        if ($LASTEXITCODE -ne 0) { Write-Err "tauri build failed"; exit 1 }

        $msiPath = Get-ChildItem "$projectRoot\src-tauri\target\release\bundle\msi\*.msi" -ErrorAction SilentlyContinue | Select-Object -First 1
        if (-not $msiPath) {
            $nsisPath = Get-ChildItem "$projectRoot\src-tauri\target\release\bundle\nsis\*.exe" -ErrorAction SilentlyContinue | Select-Object -First 1
            if ($nsisPath) {
                Write-Warn "No .msi produced, but NSIS .exe found: $($nsisPath.FullName)"
                $msiPath = $nsisPath
            } else {
                Write-Err "Build reported success but no installer found in bundle/"
                exit 1
            }
        }
        $sizeMB = [math]::Round($msiPath.Length / 1MB, 1)
        Write-Ok "Installer built: $($msiPath.Name) ($sizeMB MB)"
    } else {
        Write-Step "Looking for existing installer"
        $msiPath = Get-ChildItem "$projectRoot\src-tauri\target\release\bundle\msi\*.msi" -ErrorAction SilentlyContinue | Select-Object -First 1
        if (-not $msiPath) {
            $msiPath = Get-ChildItem "$projectRoot\src-tauri\target\release\bundle\nsis\*.exe" -ErrorAction SilentlyContinue | Select-Object -First 1
        }
        if (-not $msiPath) {
            Write-Err "No installer found. Run without -SkipBuild first."
            exit 1
        }
        Write-Ok "Found existing installer: $($msiPath.Name)"
    }

    # --- Git init + commit ---
    Write-Step "Initializing git repository"
    if (-not (Test-Path ".git")) {
        git init
        if ($LASTEXITCODE -ne 0) { Write-Err "git init failed"; exit 1 }
        git branch -M main
        Write-Ok "git initialized"
    } else {
        Write-Ok "git already initialized"
    }

    Write-Step "Staging and committing"
    git add .
    $status = git status --porcelain
    if ($status) {
        $commitMsg = @"
v0.1.0 pre-release: ScholarScribe local LLM writing companion

- Tauri 2 + Svelte 4 + Ollama backend (Windows .msi)
- Four modules: Models, Style Analysis, Disclosure Assistant, Detector Literacy
- Privacy: zero telemetry, local-only inference, single outbound host (registry.ollama.ai for model downloads)
- Explicit ethical-use policy: no detection-evasion features (see docs/ETHICS.md)
"@
        git commit -m $commitMsg
        if ($LASTEXITCODE -ne 0) { Write-Err "git commit failed"; exit 1 }
        Write-Ok "Commit created"
    } else {
        Write-Ok "Working tree clean — nothing to commit"
    }

    # --- Create GitHub repo + push ---
    Write-Step "Creating GitHub repository: $RepoName"
    $createArgs = @("repo", "create", $RepoName, "--source=.", "--description=$Description", "--push")
    if ($Public) { $createArgs += "--public" } else { $createArgs += "--private" }
    & gh @createArgs
    if ($LASTEXITCODE -ne 0) {
        Write-Warn "gh repo create failed. If the repo already exists, this is expected."
        Write-Host "    Attempting to push to existing remote..." -ForegroundColor Yellow
        git remote remove origin 2>$null
        git remote add origin "https://github.com/$ghUser/$RepoName.git"
        git push -u origin main
        if ($LASTEXITCODE -ne 0) { Write-Err "git push failed"; exit 1 }
    }
    $repoUrl = "https://github.com/$ghUser/$RepoName"
    Write-Ok "Repository ready: $repoUrl"

    # --- Tag ---
    Write-Step "Tagging v0.1.0-pre"
    git tag -d v0.1.0-pre 2>$null
    git push origin :refs/tags/v0.1.0-pre 2>$null
    git tag -a v0.1.0-pre -m "v0.1.0 pre-release"
    git push origin v0.1.0-pre
    if ($LASTEXITCODE -ne 0) { Write-Err "git push tag failed"; exit 1 }
    Write-Ok "Tag pushed"

    # --- Release with attached .msi ---
    Write-Step "Creating GitHub pre-release and attaching installer"

    $releaseNotes = @"
First pre-release of ScholarScribe.

## What's included

- **Models tab** — Download and manage open LLMs (Gemma 2, Qwen 2.5, Llama 3.1, Phi-3) via Ollama.
- **Chat tab** — Local-only chat with system-prompt guardrail (refuses evasion requests).
- **Style Analysis tab** — Compare a draft to your own prior writing (sentence length, hedging, passive voice, etc.).
- **Disclosure Assistant tab** — Generate venue-compliant AI-use statements for ICMJE, Nature, IEEE, Elsevier, ACL.
- **Detector Literacy tab** — Educational content on how detectors work and where they fail.

## Install

1. Install [Ollama](https://ollama.com/download) (free, ~150 MB).
2. Download and run the ScholarScribe installer attached below.
3. Open ScholarScribe, go to the Models tab, download a model that fits your RAM.
4. See [USER_MANUAL.md](USER_MANUAL.md) for the full walkthrough.

## Privacy

Zero telemetry. Single outbound host (registry.ollama.ai, only when downloading a model). All inference runs on your device via Ollama. See [SECURITY.md](SECURITY.md) for the full audit guide.

## Ethical use

ScholarScribe does **not** include detection-evasion features. See [docs/ETHICS.md](docs/ETHICS.md) for the full policy.

## System requirements

- Windows 10 1809+ or Windows 11
- 8 GB RAM minimum (16 GB recommended for 7B-9B models)
- ~3 GB free disk for the app + first small model
"@

    $notesFile = New-TemporaryFile
    $releaseNotes | Out-File -FilePath $notesFile -Encoding utf8

    $releaseArgs = @("release", "create", "v0.1.0-pre",
        "--title", "v0.1.0 (Pre-release)",
        "--notes-file", $notesFile,
        "--prerelease")
    if ($msiPath) {
        $releaseArgs += @($msiPath.FullName)
    }

    $releaseOut = & gh @releaseArgs 2>&1
    Remove-Item $notesFile -ErrorAction SilentlyContinue

    if ($LASTEXITCODE -eq 0) {
        $releaseUrl = ($releaseOut | Where-Object { $_ -match "^https://" } | Select-Object -First 1)
        if (-not $releaseUrl) { $releaseUrl = "$repoUrl/releases/tag/v0.1.0-pre" }
        Write-Ok "Pre-release created: $releaseUrl"
    } else {
        Write-Warn "gh release create output:"
        Write-Host $releaseOut -ForegroundColor Yellow
        Write-Warn "Release creation failed. You can create it manually at $repoUrl/releases/new"
        Write-Host "    Tag: v0.1.0-pre (already pushed)" -ForegroundColor Yellow
        Write-Host "    Attach: $($msiPath.FullName)" -ForegroundColor Yellow
    }

    Write-Host "`n==============================" -ForegroundColor White
    Write-Host "ScholarScribe release complete." -ForegroundColor Green
    Write-Host "==============================" -ForegroundColor White
    Write-Host "Repo:    $repoUrl" -ForegroundColor White
    Write-Host "Release: $releaseUrl" -ForegroundColor White
    if ($msiPath) {
        Write-Host "Asset:   $($msiPath.Name) ($([math]::Round($msiPath.Length / 1MB, 1)) MB)" -ForegroundColor White
    }
    Write-Host ""
    Write-Host "Next steps:" -ForegroundColor White
    Write-Host "  1. Verify the release page renders correctly." -ForegroundColor White
    Write-Host "  2. Install the .msi on a test machine." -ForegroundColor White
    Write-Host "  3. Download a small model (Gemma 2 2B) to verify end-to-end." -ForegroundColor White
    Write-Host "  4. If anything's wrong, fix and re-run with -SkipBuild to re-push quickly." -ForegroundColor White

} finally {
    Pop-Location
}
