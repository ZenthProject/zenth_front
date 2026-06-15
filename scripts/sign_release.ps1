# ============================================================
# sign_release.ps1 - Signe les binaires et génère manifest.json (Windows)
#
# Usage :
#   .\scripts\sign_release.ps1 -Version "0.1.2" -PrivKey "C:\keys\ed25519_private.pem" -DistDir "releases"
#
# Prérequis : OpenSSL dans le PATH (livré avec Git for Windows ou installable séparément)
#   winget install ShiningLight.OpenSSL  ou  choco install openssl
# ============================================================
param(
    [Parameter(Mandatory)][string]$Version,
    [Parameter(Mandatory)][string]$PrivKey,
    [Parameter(Mandatory)][string]$DistDir
)

Set-StrictMode -Version Latest
$ErrorActionPreference = "Stop"

function Write-Ok($msg)   { Write-Host "[sign]  $msg" -ForegroundColor Green }
function Write-Info($msg) { Write-Host "[info]  $msg" -ForegroundColor Cyan }
function Write-Fail($msg) { Write-Host "[error] $msg" -ForegroundColor Red; exit 1 }

# ── Vérifications ─────────────────────────────────────────────
if (-not (Test-Path $PrivKey))  { Write-Fail "Clé privée introuvable : $PrivKey" }
if (-not (Test-Path $DistDir))  { Write-Fail "Dossier dist introuvable : $DistDir" }
if (-not (Get-Command "openssl" -ErrorAction SilentlyContinue)) {
    Write-Fail "OpenSSL introuvable dans le PATH.`n  Installe : winget install ShiningLight.OpenSSL"
}

$ManifestPath = Join-Path $DistDir "manifest.json"
$PubDate = (Get-Date -Format "yyyy-MM-ddTHH:mm:ssZ")

Write-Info "Signature des artefacts v${Version}..."

# ── Fonction de signature d'un artefact ───────────────────────
function Sign-Artifact {
    param([string]$FilePath, [string]$Platform)

    $fname = Split-Path -Leaf $FilePath

    # SHA-256
    $sha256 = (Get-FileHash -Path $FilePath -Algorithm SHA256).Hash.ToLower()

    # Taille en octets
    $size = (Get-Item $FilePath).Length

    # Signe le sha256_hex via un fichier temporaire (openssl -rawin ne supporte pas stdin)
    $tmpHash = [System.IO.Path]::GetTempFileName()
    $tmpSig  = [System.IO.Path]::GetTempFileName()
    try {
        [System.IO.File]::WriteAllText($tmpHash, $sha256, [System.Text.Encoding]::ASCII)
        $errOut = & openssl pkeyutl -sign -inkey $PrivKey -rawin -in $tmpHash -out $tmpSig 2>&1
        if ($LASTEXITCODE -ne 0) { Write-Fail "openssl a échoué pour $fname : $errOut" }
        $sigB64 = [Convert]::ToBase64String([System.IO.File]::ReadAllBytes($tmpSig))
    } finally {
        Remove-Item $tmpHash -ErrorAction SilentlyContinue
        Remove-Item $tmpSig  -ErrorAction SilentlyContinue
    }

    Write-Ok "[$Platform] $fname  sha256=$($sha256.Substring(0,16))…"

    return @{
        platform  = $Platform
        file      = $fname
        sha256    = $sha256
        size      = $size
        signature = $sigB64
    }
}

# ── Collecte des artefacts de la version ──────────────────────
$entries = @{}

$map = @{
    "Zenth_${Version}_windows_setup.exe" = "windows-x86_64"
    "Zenth_${Version}_windows.msi"       = "windows-x86_64-msi"
    "Zenth_${Version}_linux_amd64.deb"   = "linux-x86_64-deb"
    "Zenth_${Version}_linux.AppImage"    = "linux-x86_64-appimage"
    "Zenth_${Version}_android.apk"       = "android-aarch64"
}

foreach ($file in $map.Keys) {
    $full = Join-Path $DistDir $file
    if (Test-Path $full) {
        $result = Sign-Artifact -FilePath $full -Platform $map[$file]
        $entries[$result.platform] = $result
    }
}

if ($entries.Count -eq 0) {
    Write-Fail "Aucun artefact Zenth_${Version}_* trouvé dans $DistDir"
}

# ── Génération du manifest.json ───────────────────────────────
$platformsJson = ($entries.Values | ForEach-Object {
    $p = $_
    "    `"$($p.platform)`": {`"file`": `"$($p.file)`", `"sha256`": `"$($p.sha256)`", `"size`": $($p.size), `"signature`": `"$($p.signature)`"}"
}) -join ",`n"

$manifest = @"
{
  "version": "$Version",
  "latest_version": "$Version",
  "pub_date": "$PubDate",
  "notes": "Zenth v$Version",
$platformsJson
}
"@

[System.IO.File]::WriteAllText($ManifestPath, $manifest, [System.Text.Encoding]::UTF8)

Write-Host ""
Write-Ok "manifest.json → $ManifestPath"
Get-Content $ManifestPath
