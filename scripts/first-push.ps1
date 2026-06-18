# ScholarScribe — minimal first-push script.
#
# This is the EASIEST way to get ScholarScribe onto GitHub. It:
#   - Does NOT require Rust, Node, or MSVC locally.
#   - Does NOT build anything.
#   - Just commits the code and pushes to a new GitHub repo.
#
# The .msi / .exe installers are then built AUTOMATICALLY by GitHub Actions
# (see .github/workflows/release.yml) when you push a v* tag.
#
# Prerequisites:
#   1. Revoke any previously-shared tokens at https://github.com/settings/tokens
#   2. Install GitHub CLI:  winget install --id GitHub.cli
#   3. Log in:  gh auth login
#      (choose GitHub.com → HTTPS → "Login with a browser" — no token pasting)
#
# Then run:
#   .\scripts\first-push.ps1 -RepoName scholarscribe -Public

param(
    [Parameter(Mandatory=$true)]
    [string]$RepoName,
    [string]$Description = "Privacy-first local LLM writing companion for researchers",
    [switch]$Public
)

$ErrorActionPreference = "Stop"

function Step($m) { Write-Host "`n==> $m" -ForegroundColor Cyan }
function Ok($m)   { Write-Host "    OK: $m" -ForegroundColor Green }
function Warn($m) { Write-Host "    !! $m" -ForegroundColor Yellow }
function Err($m)  { Write-Host "    ERROR: $m" -ForegroundColor Red; exit 1 }

$root = Split-Path -Parent $PSScriptRoot
Push-Location $root
try {
    Write-Host "ScholarScribe — first push" -ForegroundColor White
    Write-Host "==========================" -ForegroundColor White

    Step "Preflight"
    Get-Command git -ErrorAction SilentlyContinue | Out-Null || Err "git not found. Install from https://git-scm.com/download/win"
    Ok "git present"
    Get-Command gh -ErrorAction SilentlyContinue | Out-Null || Err "gh not found. Install: winget install --id GitHub.cli"
    Ok "gh present"

    gh auth status 2>&1 | Out-Null
    if ($LASTEXITCODE -ne 0) {
        Err "Not logged in to GitHub. Run: gh auth login"
    }
    $ghUser = gh api user --jq .login
    Ok "authenticated as @$ghUser"

    Step "git init + first commit"
    if (-not (Test-Path ".git")) {
        git init | Out-Null
        git branch -M main
        Ok "git initialized"
    } else {
        Ok "git already initialized"
    }

    git add .
    if (git status --porcelain) {
        $msg = @"
v0.1.0 pre-release: ScholarScribe local LLM writing companion

- Tauri 2 + Svelte 4 + Ollama backend (Windows .msi/.exe)
- Four modules: Models, Style Analysis, Disclosure Assistant, Detector Literacy
- Privacy: zero telemetry, local-only inference, single outbound host (registry.ollama.ai)
- Explicit ethical-use policy: no detection-evasion features (see docs/ETHICS.md)
- GitHub Actions workflows for CI + automated release builds
"@
        git commit -m $msg | Out-Null
        Ok "commit created"
    } else {
        Ok "nothing to commit"
    }

    Step "Create GitHub repo and push"
    $args = @("repo","create",$RepoName,"--source=.","--description=$Description","--push")
    if ($Public) { $args += "--public" } else { $args += "--private" }
    & gh @args
    if ($LASTEXITCODE -ne 0) {
        Warn "gh repo create failed (maybe repo already exists). Pushing to existing remote..."
        git remote remove origin 2>$null
        git remote add origin "https://github.com/$ghUser/$RepoName.git"
        git push -u origin main | Out-Null
    }
    $repoUrl = "https://github.com/$ghUser/$RepoName"
    Ok "repo ready: $repoUrl"

    Step "Tag v0.1.0-pre and push (this triggers the release build on Actions)"
    git tag -d v0.1.0-pre 2>$null | Out-Null
    git tag -a v0.1.0-pre -m "v0.1.0 pre-release"
    git push origin v0.1.0-pre
    Ok "tag pushed"

    Write-Host ""
    Write-Host "==================================================" -ForegroundColor Green
    Write-Host "DONE. Code is on GitHub and the build is running." -ForegroundColor Green
    Write-Host "==================================================" -ForegroundColor Green
    Write-Host ""
    Write-Host "Repo:  $repoUrl" -ForegroundColor White
    Write-Host "Build: $repoUrl/actions" -ForegroundColor White
    Write-Host "Release (will appear once build finishes, ~10-15 min):" -ForegroundColor White
    Write-Host "       $repoUrl/releases/tag/v0.1.0-pre" -ForegroundColor White
    Write-Host ""
    Write-Host "The Actions workflow will:" -ForegroundColor White
    Write-Host "  1. Spin up a Windows runner" -ForegroundColor White
    Write-Host "  2. Install Rust + Node" -ForegroundColor White
    Write-Host "  3. Build ScholarScribe_0.1.0_x64_en-US.msi + .exe" -ForegroundColor White
    Write-Host "  4. Attach both to the v0.1.0-pre release" -ForegroundColor White
    Write-Host ""
    Write-Host "Watch the Actions tab. When the run turns green, your installers" -ForegroundColor White
    Write-Host "will be on the Releases page, downloadable by anyone (if public)." -ForegroundColor White
    Write-Host ""
    Write-Host "If the build fails, click into the failed run for logs. Common" -ForegroundColor White
    Write-Host "causes during pre-release: Tauri API name drift, missing icon files." -ForegroundColor White
    Write-Host "Send the failing step's log and I'll patch the code." -ForegroundColor White
}
finally {
    Pop-Location
}
