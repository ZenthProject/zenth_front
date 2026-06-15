# ============================================================
# build_windows.ps1 - Build Zenth pour Windows (natif)
#
# À lancer depuis la racine du projet sur une machine Windows :
#   .\scripts\build_windows.ps1
#   .\scripts\build_windows.ps1 -Force
#   .\scripts\build_windows.ps1 -Bundle nsis        # NSIS seulement
#   .\scripts\build_windows.ps1 -Bundle msi         # MSI seulement
#   .\scripts\build_windows.ps1 -Bundle nsis,msi    # Les deux (défaut)
#
# Prérequis :
#   - Rust stable  : https://rustup.rs
#   - Bun          : https://bun.sh
#   - NSIS 3.x     : https://nsis.sourceforge.io  (pour le bundle nsis)
#   - WebView2     : déjà installé sur Win10 22H2+ / Win11
# ============================================================
param(
    [switch]$Force,
    [string]$Bundle = "nsis,msi"
)

Set-StrictMode -Version Latest
$ErrorActionPreference = "Stop"

# ── Couleurs ─────────────────────────────────────────────────
function Write-Ok($msg)   { Write-Host "[build] $msg" -ForegroundColor Green }
function Write-Info($msg) { Write-Host "[info]  $msg" -ForegroundColor Cyan }
function Write-Warn($msg) { Write-Host "[warn]  $msg" -ForegroundColor Yellow }
function Write-Fail($msg) { Write-Host "[error] $msg" -ForegroundColor Red; exit 1 }
function Write-Section($msg) {
    Write-Host ""
    Write-Host "══════════════════════════════════════" -ForegroundColor Blue
    Write-Host "  $msg" -ForegroundColor White
    Write-Host "══════════════════════════════════════" -ForegroundColor Blue
    Write-Host ""
}

# ── Répertoire racine ─────────────────────────────────────────
$ScriptDir = Split-Path -Parent $MyInvocation.MyCommand.Path
$ProjectDir = Split-Path -Parent $ScriptDir
Set-Location $ProjectDir

# ── Version ──────────────────────────────────────────────────
try {
    $VERSION = (Get-Content "src-tauri\tauri.conf.json" | ConvertFrom-Json).version
} catch {
    $VERSION = (Select-String -Path "src-tauri\Cargo.toml" -Pattern '^version\s*=\s*"(.+)"' |
                Select-Object -First 1).Matches.Groups[1].Value
}
Write-Info "Version : $VERSION"

$OutDir = "releases"
New-Item -ItemType Directory -Force -Path $OutDir | Out-Null

# ── Vérification des prérequis ────────────────────────────────
Write-Section "Vérification des prérequis"

if (-not (Get-Command "cargo" -ErrorAction SilentlyContinue)) {
    Write-Fail "Rust/cargo introuvable. Installe via https://rustup.rs"
}
Write-Info "Rust : $(cargo --version)"

if (-not (Get-Command "bun" -ErrorAction SilentlyContinue)) {
    Write-Fail "Bun introuvable. Installe via https://bun.sh"
}
Write-Info "Bun : $(bun --version)"

# Vérifie que la target MSVC est installée
$targets = rustup target list --installed 2>&1
if ($targets -notmatch "x86_64-pc-windows-msvc") {
    Write-Warn "Target x86_64-pc-windows-msvc non installée - installation..."
    rustup target add x86_64-pc-windows-msvc
}
Write-Info "Target x86_64-pc-windows-msvc : OK"

# ── Skip si artefact déjà présent ────────────────────────────
$ExeDest = "$OutDir\Zenth_${VERSION}_windows_setup.exe"
$MsiDest  = "$OutDir\Zenth_${VERSION}_windows.msi"

if (-not $Force) {
    $allPresent = $true
    if ($Bundle -match "nsis" -and -not (Test-Path $ExeDest)) { $allPresent = $false }
    if ($Bundle -match "msi"  -and -not (Test-Path $MsiDest))  { $allPresent = $false }
    if ($allPresent) {
        Write-Info "Artefacts déjà présents dans $OutDir\, skip. (utilise -Force pour rebuilder)"
        exit 0
    }
}

# ── Installation des dépendances frontend ─────────────────────
Write-Section "Dépendances frontend"
Write-Ok "bun install..."
bun install
if ($LASTEXITCODE -ne 0) { Write-Fail "bun install a échoué." }

# ── Build Tauri Windows ───────────────────────────────────────
Write-Section "Build Tauri Windows ($Bundle)"
Write-Ok "bunx tauri build --target x86_64-pc-windows-msvc --bundles $Bundle"

$env:TAURI_SIGNING_PRIVATE_KEY = ""
$env:TAURI_SIGNING_PRIVATE_KEY_PASSWORD = ""

bunx tauri build --target x86_64-pc-windows-msvc --bundles $Bundle
if ($LASTEXITCODE -ne 0) { Write-Fail "tauri build a échoué." }

# ── Copie des artefacts dans releases/ ───────────────────────
Write-Section "Copie des artefacts"

$BundleBase = "src-tauri\target\x86_64-pc-windows-msvc\release\bundle"

# NSIS → setup .exe
if ($Bundle -match "nsis") {
    $ExeSrc = Get-ChildItem "$BundleBase\nsis\*.exe" -ErrorAction SilentlyContinue | Select-Object -First 1
    if ($ExeSrc) {
        Copy-Item $ExeSrc.FullName $ExeDest -Force
        $size = [math]::Round($ExeSrc.Length / 1MB, 1)
        Write-Ok "EXE : $ExeDest  (${size} MB)"
    } else {
        Write-Warn "EXE NSIS introuvable dans $BundleBase\nsis\"
    }
}

# MSI
if ($Bundle -match "msi") {
    $MsiSrc = Get-ChildItem "$BundleBase\msi\*.msi" -ErrorAction SilentlyContinue | Select-Object -First 1
    if ($MsiSrc) {
        Copy-Item $MsiSrc.FullName $MsiDest -Force
        $size = [math]::Round($MsiSrc.Length / 1MB, 1)
        Write-Ok "MSI : $MsiDest  (${size} MB)"
    } else {
        Write-Warn "MSI introuvable dans $BundleBase\msi\"
    }
}

# ── Résumé ────────────────────────────────────────────────────
Write-Section "Artefacts produits"
Get-ChildItem $OutDir | Where-Object { $_.Name -match "windows" } | ForEach-Object {
    $size = [math]::Round($_.Length / 1MB, 1)
    Write-Host "  ✓ $($_.Name)  (${size} MB)" -ForegroundColor Green
}

Write-Info "Terminé. Artefacts dans : $ProjectDir\$OutDir\"
