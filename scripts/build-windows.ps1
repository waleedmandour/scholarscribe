# Bootstrap ScholarScribe on Windows (PowerShell)
# Run this once on a fresh Windows machine to install prerequisites and build the .msi installer.
# Requires: Windows 10 1809+ or Windows 11, administrative rights for installing prerequisites.

param(
    [switch]$SkipRustInstall,
    [switch]$SkipNodeInstall
)

$ErrorActionPreference = "Stop"

function Write-Step($msg) { Write-Host "`n==> $msg" -ForegroundColor Cyan }
function Write-Ok($msg)   { Write-Host "    OK: $msg" -ForegroundColor Green }
function Write-Warn($msg) { Write-Host "    !! $msg" -ForegroundColor Yellow }

Write-Host "ScholarScribe build bootstrap" -ForegroundColor White
Write-Host "=============================" -ForegroundColor White

# --- 1. Rust ---
if (-not $SkipRustInstall) {
    Write-Step "Checking Rust toolchain"
    if (Get-Command cargo -ErrorAction SilentlyContinue) {
        Write-Ok "cargo already installed: $(cargo --version)"
    } else {
        Write-Warn "Rust not found. Installing via rustup..."
        Invoke-WebRequest -Uri https://win.rustup.rs/x86_64 -OutFile "$env:TEMP\rustup-init.exe"
        & "$env:TEMP\rustup-init.exe" -y --default-toolchain stable
        $env:Path += ";$env:USERPROFILE\.cargo\bin"
        Write-Ok "Rust installed: $(cargo --version)"
    }
} else {
    Write-Warn "Skipping Rust install per -SkipRustInstall"
}

# --- 2. Node.js ---
if (-not $SkipNodeInstall) {
    Write-Step "Checking Node.js"
    if (Get-Command node -ErrorAction SilentlyContinue) {
        $nodeVer = node --version
        if ($nodeVer -lt "v18") {
            Write-Warn "Node $nodeVer is older than v18. Please upgrade from https://nodejs.org/"
        } else {
            Write-Ok "Node $nodeVer"
        }
    } else {
        Write-Warn "Node.js not found. Please install from https://nodejs.org/ (LTS) and re-run this script."
        exit 1
    }
} else {
    Write-Warn "Skipping Node check per -SkipNodeInstall"
}

# --- 3. Visual Studio C++ Build Tools ---
Write-Step "Checking MSVC build tools (required by Tauri)"
$vswhere = "${env:ProgramFiles(x86)}\Microsoft Visual Studio\Installer\vswhere.exe"
if (Test-Path $vswhere) {
    $vs = & $vswhere -latest -products * -requires Microsoft.VisualStudio.Component.VC.Tools.x86.x64 -property displayName
    if ($vs) {
        Write-Ok "MSVC found: $vs"
    } else {
        Write-Warn "Visual Studio installed but no C++ tools. Run the VS Installer and add 'Desktop development with C++'."
    }
} else {
    Write-Warn "Visual Studio Installer not found. Install 'Build Tools for Visual Studio 2022' from:"
    Write-Warn "  https://visualstudio.microsoft.com/visual-cpp-build-tools/"
    Write-Warn "Select 'Desktop development with C++' workload, then re-run this script."
}

# --- 4. WebView2 ---
Write-Step "Checking WebView2 runtime"
$wv2Key = "HKLM:\SOFTWARE\WOW6432Node\Microsoft\EdgeUpdate\Clients\{F3017226-FE2A-4295-8BDF-00C3A9A7E4C5}"
if (Test-Path $wv2Key) {
    Write-Ok "WebView2 runtime installed"
} else {
    Write-Warn "WebView2 runtime not found (Windows 11 has it built-in; Windows 10 may need to install it)."
    Write-Warn "The Tauri installer will fetch it automatically if missing."
}

# --- 5. Ollama ---
Write-Step "Checking Ollama"
if (Get-Command ollama -ErrorAction SilentlyContinue) {
    Write-Ok "Ollama installed: $(ollama --version)"
} else {
    Write-Warn "Ollama not found. Download from https://ollama.com/download and install, then re-run."
}

# --- 6. npm install ---
Write-Step "Installing npm dependencies"
Push-Location "$PSScriptRoot\.."
try {
    npm install --no-audit --no-fund
    Write-Ok "npm install complete"
} finally {
    Pop-Location
}

# --- 7. Verify Rust compiles ---
Write-Step "Running cargo check (this can take 5-10 minutes on first run)"
Push-Location "$PSScriptRoot\..\src-tauri"
try {
    cargo check
    if ($LASTEXITCODE -eq 0) {
        Write-Ok "cargo check passed"
    } else {
        Write-Warn "cargo check failed. Review the errors above."
        exit 1
    }
} finally {
    Pop-Location
}

# --- 8. Build the installer ---
Write-Step "Building ScholarScribe .msi installer (10-20 minutes on first run)"
Push-Location "$PSScriptRoot\.."
try {
    npm run tauri build
    if ($LASTEXITCODE -eq 0) {
        $msi = Get-ChildItem "$PSScriptRoot\..\src-tauri\target\release\bundle\msi\*.msi" -ErrorAction SilentlyContinue | Select-Object -First 1
        if ($msi) {
            Write-Ok "Installer built: $($msi.FullName)"
            Write-Host "    Size: $([math]::Round($msi.Length / 1MB, 1)) MB" -ForegroundColor Green
        } else {
            Write-Warn "Build reported success but no .msi found. Check src-tauri/target/release/bundle/"
        }
    } else {
        Write-Warn "tauri build failed. Review the errors above."
        exit 1
    }
} finally {
    Pop-Location
}

Write-Host "`n=============================" -ForegroundColor White
Write-Host "ScholarScribe build complete." -ForegroundColor White
Write-Host "Next: install the .msi, launch ScholarScribe, go to the Models tab to download a model." -ForegroundColor White
