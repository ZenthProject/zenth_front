#!/usr/bin/env bash
# ============================================================
# sign_release.sh - Signe les binaires et génère manifest.json
#
# Usage :
#   bash scripts/sign_release.sh <version> <private_key.pem> <dist_dir> [base_url]
#
# Exemple :
#   bash scripts/sign_release.sh "0.1.2" /mnt/usbkey/ed25519_private.pem releases/ \
#     "https://files.lucas-sanchez.fr/zenth"
#
# Génère dist/manifest.json avec :
#   - version, notes, pub_date
#   - une entrée par plateforme (sha256, size, signature Ed25519, url)
#
# Signature : Ed25519 sur le sha256_hex (cohérent avec update.rs::verify_signature)
#
# Dépendances : openssl, sha256sum (coreutils)
# ============================================================
set -euo pipefail

VERSION="${1:?Usage: $0 <version> <private_key.pem> <dist_dir> [base_url]}"
PRIVKEY="${2:?Usage: $0 <version> <private_key.pem> <dist_dir> [base_url]}"
DIST="${3:?Usage: $0 <version> <private_key.pem> <dist_dir> [base_url]}"
BASE_URL="${4:-}"   # optionnel — ex: https://files.lucas-sanchez.fr/zenth

[ -f "$PRIVKEY" ] || { echo "Clé privée introuvable : $PRIVKEY"; exit 1; }
[ -d "$DIST"    ] || { echo "Dossier dist introuvable : $DIST"; exit 1; }

MANIFEST="$DIST/manifest.json"
PUB_DATE=$(date -u +"%Y-%m-%dT%H:%M:%SZ")

echo "Signature des artefacts v${VERSION}..." >&2

sign_file() {
    local file="$1"
    local platform="$2"
    local fname
    fname=$(basename "$file")

    local sha256
    sha256=$(sha256sum "$file" | awk '{print $1}')

    local size
    size=$(stat -c '%s' "$file" 2>/dev/null || stat -f '%z' "$file")

    # Signe le sha256_hex avec Ed25519 (cohérent avec update.rs::verify_signature)
    # -rawin nécessite un fichier (pas stdin) pour déterminer la taille
    local tmp_hash
    tmp_hash=$(mktemp)
    printf '%s' "$sha256" > "$tmp_hash"
    local sig_b64
    sig_b64=$(openssl pkeyutl -sign -inkey "$PRIVKEY" -rawin -in "$tmp_hash" | base64 | tr -d '\n')
    rm -f "$tmp_hash"

    # URL de téléchargement (vide si BASE_URL non fourni)
    local url=""
    if [ -n "$BASE_URL" ]; then
        url="${BASE_URL}/${fname}"
    fi

    # Écrit aussi le fichier .sig à côté du binaire
    printf '%s' "$sig_b64" > "${file}.sig"

    echo "  [$platform] $fname  sha256=${sha256:0:16}…  → ${fname}.sig" >&2

    if [ -n "$url" ]; then
        printf '"%s": {"file": "%s", "url": "%s", "sha256": "%s", "size": %s, "signature": "%s"}' \
            "$platform" "$fname" "$url" "$sha256" "$size" "$sig_b64"
    else
        printf '"%s": {"file": "%s", "sha256": "%s", "size": %s, "signature": "%s"}' \
            "$platform" "$fname" "$sha256" "$size" "$sig_b64"
    fi
}

ENTRIES=""
SEP=""

for f in "$DIST"/Zenth_"${VERSION}"_linux_amd64.deb; do
    [ -f "$f" ] || continue
    entry=$(sign_file "$f" "linux-x86_64-deb")
    ENTRIES="${ENTRIES}${SEP}    ${entry}"; SEP=",\n"
done

for f in "$DIST"/Zenth_"${VERSION}"_linux.AppImage; do
    [ -f "$f" ] || continue
    entry=$(sign_file "$f" "linux-x86_64-appimage")
    ENTRIES="${ENTRIES}${SEP}    ${entry}"; SEP=",\n"
done

for f in "$DIST"/Zenth_"${VERSION}"_windows_setup.exe; do
    [ -f "$f" ] || continue
    entry=$(sign_file "$f" "windows-x86_64")
    ENTRIES="${ENTRIES}${SEP}    ${entry}"; SEP=",\n"
done

for f in "$DIST"/Zenth_"${VERSION}"_android.apk; do
    [ -f "$f" ] || continue
    entry=$(sign_file "$f" "android-aarch64")
    ENTRIES="${ENTRIES}${SEP}    ${entry}"; SEP=",\n"
done

if [ -z "$ENTRIES" ]; then
    echo "Aucun artefact trouvé pour la version ${VERSION} dans $DIST" >&2
    exit 1
fi

cat > "$MANIFEST" <<JSON
{
  "version": "${VERSION}",
  "latest_version": "${VERSION}",
  "pub_date": "${PUB_DATE}",
  "notes": "Zenth v${VERSION}",
$(printf '%b' "$ENTRIES")
}
JSON

echo "" >&2
echo "manifest.json → $MANIFEST" >&2
