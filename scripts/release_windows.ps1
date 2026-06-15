# ============================================================
# release_windows.ps1 - Build + Signature + manifest.json (Windows)
#
# Fait tout en une seule commande :
#   1. Build Tauri Windows (NSIS .exe et/ou MSI)
#   2. Calcule SHA-256 de chaque artefact
#   3. Signe avec la clé privée Ed25519
#   4. Génère releases/<artefact>.sig  (fichier de signature individuel)
#   5. Génère releases/manifest.json  (pour déploiement DHT)
#
# Usage :
#   .\scripts\release_windows.ps1 -PrivKey "C:\keys\ed25519_private.pem"
#   .\scripts\release_windows.ps1 -PrivKey "D:\usb\ed25519_private.pem" -Bundle nsis
#   .\scripts\release_windows.ps1 -PrivKey "C:\keys\ed25519_private.pem" -Force
#
# Prérequis :
#   - Rust stable  : https://rustup.rs
#   - Bun          : https://bun.sh
#   - NSIS 3.x     : https://nsis.sourceforge.io  (pour -Bundle nsis)
#   - OpenSSL      : winget install ShiningLight.OpenSSL
#     ou Git for Windows l'inclut déjà dans C:\Program Files\Git\usr\bin\
# ============================================================
param(
    [Parameter(Mandatory)][string]$PrivKey,
    [switch]$Force,
    [string]$Bundle = "nsis,msi"
)

Set-StrictMode -Version Latest
$ErrorActionPreference = "Stop"

function Write-Ok($msg)      { Write-Host "[ok]    $msg" -ForegroundColor Green }
function Write-Info($msg)    { Write-Host "[info]  $msg" -ForegroundColor Cyan }
function Write-Warn($msg)    { Write-Host "[warn]  $msg" -ForegroundColor Yellow }
function Write-Fail($msg)    { Write-Host "[error] $msg" -ForegroundColor Red; exit 1 }
function Write-Section($msg) {
    Write-Host ""
    Write-Host "══════════════════════════════════════" -ForegroundColor Blue
    Write-Host "  $msg" -ForegroundColor White
    Write-Host "══════════════════════════════════════" -ForegroundColor Blue
    Write-Host ""
}

# ── Répertoire racine ─────────────────────────────────────────
$ScriptDir  = Split-Path -Parent $MyInvocation.MyCommand.Path
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

# ════════════════════════════════════════════════════════════════
# ÉTAPE 1 — Vérification des prérequis
# ════════════════════════════════════════════════════════════════
Write-Section "Vérification des prérequis"

if (-not (Test-Path $PrivKey)) { Write-Fail "Clé privée introuvable : $PrivKey" }

foreach ($tool in @("cargo", "bun")) {
    if (-not (Get-Command $tool -ErrorAction SilentlyContinue)) {
        Write-Fail "'$tool' introuvable. cargo → https://rustup.rs  /  bun → https://bun.sh"
    }
}

# OpenSSL — peut être dans Git for Windows
$opensslCmd = Get-Command "openssl" -ErrorAction SilentlyContinue
if (-not $opensslCmd) {
    $gitOpenssl = "C:\Program Files\Git\usr\bin\openssl.exe"
    if (Test-Path $gitOpenssl) {
        $env:PATH = "C:\Program Files\Git\usr\bin;" + $env:PATH
        Write-Info "OpenSSL trouvé via Git for Windows"
    } else {
        Write-Fail "OpenSSL introuvable.`n  Installe : winget install ShiningLight.OpenSSL`n  Ou installe Git for Windows qui l'inclut."
    }
}

$targets = rustup target list --installed 2>&1
if ($targets -notmatch "x86_64-pc-windows-msvc") {
    Write-Warn "Target x86_64-pc-windows-msvc non installée - installation..."
    rustup target add x86_64-pc-windows-msvc
}

Write-Ok "Rust   : $(cargo --version)"
Write-Ok "Bun    : $(bun --version)"
Write-Ok "OpenSSL: $(openssl version)"

# ════════════════════════════════════════════════════════════════
# ÉTAPE 2 — Build Tauri Windows
# ════════════════════════════════════════════════════════════════
Write-Section "Build Tauri Windows ($Bundle)"

$ExeDest = "$OutDir\Zenth_${VERSION}_windows_setup.exe"
$MsiDest  = "$OutDir\Zenth_${VERSION}_windows.msi"

$needBuild = $false
if ($Force) {
    $needBuild = $true
} else {
    if ($Bundle -match "nsis" -and -not (Test-Path $ExeDest)) { $needBuild = $true }
    if ($Bundle -match "msi"  -and -not (Test-Path $MsiDest))  { $needBuild = $true }
}

if (-not $needBuild) {
    Write-Info "Artefacts déjà présents, skip du build. (-Force pour rebuilder)"
} else {
    Write-Ok "bun install..."
    bun install
    if ($LASTEXITCODE -ne 0) { Write-Fail "bun install a échoué." }

    $env:TAURI_SIGNING_PRIVATE_KEY = ""
    $env:TAURI_SIGNING_PRIVATE_KEY_PASSWORD = ""

    Write-Ok "bunx tauri build --target x86_64-pc-windows-msvc --bundles $Bundle"
    bunx tauri build --target x86_64-pc-windows-msvc --bundles $Bundle
    if ($LASTEXITCODE -ne 0) { Write-Fail "tauri build a échoué." }

    $BundleBase = "src-tauri\target\x86_64-pc-windows-msvc\release\bundle"

    if ($Bundle -match "nsis") {
        $src = Get-ChildItem "$BundleBase\nsis\*.exe" -ErrorAction SilentlyContinue | Select-Object -First 1
        if ($src) {
            Copy-Item $src.FullName $ExeDest -Force
            Write-Ok "EXE copié : $ExeDest  ($([math]::Round($src.Length/1MB,1)) MB)"
        } else {
            Write-Warn "EXE NSIS introuvable dans $BundleBase\nsis\"
        }
    }

    if ($Bundle -match "msi") {
        $src = Get-ChildItem "$BundleBase\msi\*.msi" -ErrorAction SilentlyContinue | Select-Object -First 1
        if ($src) {
            Copy-Item $src.FullName $MsiDest -Force
            Write-Ok "MSI copié : $MsiDest  ($([math]::Round($src.Length/1MB,1)) MB)"
        } else {
            Write-Warn "MSI introuvable dans $BundleBase\msi\"
        }
    }
}

# ════════════════════════════════════════════════════════════════
# ÉTAPE 3 — Signature des artefacts
# ════════════════════════════════════════════════════════════════
Write-Section "Signature Ed25519 + génération manifest.json"

$PubDate = (Get-Date -Format "yyyy-MM-ddTHH:mm:ssZ")

# Signe un artefact, génère un .sig, retourne un objet pour manifest.json
function Sign-Artifact {
    param([string]$FilePath, [string]$Platform)

    if (-not (Test-Path $FilePath)) { return $null }

    $fname  = Split-Path -Leaf $FilePath
    $sha256 = (Get-FileHash -Path $FilePath -Algorithm SHA256).Hash.ToLower()
    $size   = (Get-Item $FilePath).Length

    # Signature Ed25519 sur sha256_hex (cohérent avec update.rs::verify_signature)
    # openssl -rawin exige un fichier, pas stdin → mktemp workaround
    $tmpHash = [System.IO.Path]::GetTempFileName()
    $tmpSig  = [System.IO.Path]::GetTempFileName()
    try {
        [System.IO.File]::WriteAllText($tmpHash, $sha256, [System.Text.Encoding]::ASCII)
        $errOut = & openssl pkeyutl -sign -inkey $PrivKey -rawin -in $tmpHash -out $tmpSig 2>&1
        if ($LASTEXITCODE -ne 0) { Write-Fail "openssl a échoué pour $fname : $errOut" }
        $sigBytes = [System.IO.File]::ReadAllBytes($tmpSig)
        $sigB64   = [Convert]::ToBase64String($sigBytes)
    } finally {
        Remove-Item $tmpHash -ErrorAction SilentlyContinue
        Remove-Item $tmpSig  -ErrorAction SilentlyContinue
    }

    # Écrit le fichier .sig individuel à côté du binaire
    $sigPath = "$FilePath.sig"
    [System.IO.File]::WriteAllText($sigPath, $sigB64, [System.Text.Encoding]::ASCII)

    Write-Ok "[$Platform]"
    Write-Host "        fichier    : $fname"
    Write-Host "        sha256     : $($sha256.Substring(0,32))…"
    Write-Host "        signature  : $($sigB64.Substring(0,32))…"
    Write-Host "        .sig       : $(Split-Path -Leaf $sigPath)"

    return @{
        platform  = $Platform
        file      = $fname
        sha256    = $sha256
        size      = $size
        signature = $sigB64
    }
}

# Table plateforme → fichier
$artifacts = @(
    @{ File = $ExeDest; Platform = "windows-x86_64" },
    @{ File = $MsiDest; Platform = "windows-x86_64-msi" }
)

# Si d'autres artefacts sont présents dans releases/ (Linux/Android buildés ailleurs)
$extras = @(
    @{ File = "$OutDir\Zenth_${VERSION}_linux_amd64.deb";   Platform = "linux-x86_64-deb" },
    @{ File = "$OutDir\Zenth_${VERSION}_linux.AppImage";    Platform = "linux-x86_64-appimage" },
    @{ File = "$OutDir\Zenth_${VERSION}_android.apk";       Platform = "android-aarch64" }
)
$artifacts += $extras

$entries = @()
foreach ($a in $artifacts) {
    $result = Sign-Artifact -FilePath $a.File -Platform $a.Platform
    if ($result) { $entries += $result }
}

if ($entries.Count -eq 0) {
    Write-Fail "Aucun artefact Zenth_${VERSION}_* trouvé dans $OutDir\"
}

# ════════════════════════════════════════════════════════════════
# ÉTAPE 4 — Génération manifest.json
# ════════════════════════════════════════════════════════════════
Write-Section "manifest.json"

$platformLines = ($entries | ForEach-Object {
    $p = $_
    "    `"$($p.platform)`": {`"file`": `"$($p.file)`", `"sha256`": `"$($p.sha256)`", `"size`": $($p.size), `"signature`": `"$($p.signature)`"}"
}) -join ",`n"

$manifest = @"
{
  "version": "$VERSION",
  "latest_version": "$VERSION",
  "pub_date": "$PubDate",
  "notes": "Zenth v$VERSION",
$platformLines
}
"@

$ManifestPath = "$OutDir\manifest.json"
[System.IO.File]::WriteAllText($ManifestPath, $manifest, (New-Object System.Text.UTF8Encoding $false))

Write-Ok "manifest.json écrit : $ManifestPath"

# ════════════════════════════════════════════════════════════════
# Résumé
# ════════════════════════════════════════════════════════════════
Write-Section "Artefacts produits dans $OutDir\"
Get-ChildItem $OutDir | Sort-Object Name | ForEach-Object {
    $size = if ($_.Length -gt 1MB) { "$([math]::Round($_.Length/1MB,1)) MB" } else { "$([math]::Round($_.Length/1KB,0)) KB" }
    Write-Host "  $($_.Name)  ($size)" -ForegroundColor Green
}
Write-Host ""
Write-Info "Déploie releases\manifest.json dans le DHT pour activer la mise à jour."
