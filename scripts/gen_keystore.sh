#!/usr/bin/env bash
# ============================================================
# gen_keystore.sh - Génère un nouveau keystore Android pour
# signer les APK Zenth et met à jour key.properties.
#
# Usage :
#   bash scripts/gen_keystore.sh
#
# Produit :
#   src-tauri/gen/android/zenth-release.keystore
#   src-tauri/gen/android/key.properties
#
# ATTENTION : changer le keystore invalide les mises à jour OTA
# pour les appareils qui ont l'ancienne version installée.
# Les utilisateurs devront désinstaller et réinstaller l'app.
# ============================================================
set -euo pipefail

# Toujours s'exécuter depuis la racine du projet
SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
cd "$SCRIPT_DIR/.."

GREEN='\033[0;32m'; YELLOW='\033[1;33m'; RED='\033[0;31m'; NC='\033[0m'
log()  { echo -e "${GREEN}[keystore]${NC} $*"; }
warn() { echo -e "${YELLOW}[warn]${NC}  $*"; }
err()  { echo -e "${RED}[error]${NC} $*"; exit 1; }

command -v keytool &>/dev/null || err "'keytool' introuvable. Installe le JDK Android."

KEYSTORE="src-tauri/gen/android/zenth-release.keystore"
KEY_PROPS="src-tauri/gen/android/key.properties"
KEY_ALIAS="zenth-key"

# ── Sauvegarde de l'ancien keystore s'il existe ───────────────
if [ -f "$KEYSTORE" ]; then
    BACKUP="${KEYSTORE}.bak.$(date +%Y%m%d%H%M%S)"
    warn "Ancien keystore sauvegardé : $BACKUP"
    mv "$KEYSTORE" "$BACKUP"
fi

# ── Saisie du mot de passe (avec retry) ──────────────────────
echo ""
while true; do
    read -rsp "  Nouveau mot de passe (min 6 car., ASCII uniquement) : " STORE_PASS; echo ""
    read -rsp "  Confirme le mot de passe                            : " STORE_PASS2; echo ""

    if [ "$STORE_PASS" != "$STORE_PASS2" ]; then
        echo -e "${RED}[error]${NC} Les mots de passe ne correspondent pas. Réessaie."
        continue
    fi
    if [ ${#STORE_PASS} -lt 6 ]; then
        echo -e "${RED}[error]${NC} Mot de passe trop court (min 6 caractères). Réessaie."
        continue
    fi
    if ! python3 -c "import sys; s=sys.argv[1]; sys.exit(0 if all(ord(c)<128 for c in s) else 1)" "$STORE_PASS" 2>/dev/null; then
        echo -e "${RED}[error]${NC} Caractères non-ASCII détectés (pas d'accents, pas d'émojis). Réessaie."
        continue
    fi
    break
done

# ── Infos du certificat ───────────────────────────────────────
echo ""
read -rp "  Nom complet (CN)    [Zenth]       : " CN;       CN="${CN:-Zenth}"
read -rp "  Organisation (O)    [Zenth]       : " ORG;      ORG="${ORG:-Zenth}"
read -rp "  Pays (C, 2 lettres) [FR]          : " COUNTRY;  COUNTRY="${COUNTRY:-FR}"

DNAME="CN=${CN}, O=${ORG}, C=${COUNTRY}"

# ── Génération du keystore ────────────────────────────────────
echo ""
log "Génération du keystore..."

keytool -genkeypair \
    -v \
    -keystore "$KEYSTORE" \
    -alias "$KEY_ALIAS" \
    -keyalg RSA \
    -keysize 4096 \
    -validity 10000 \
    -storepass "$STORE_PASS" \
    -keypass  "$STORE_PASS" \
    -dname "$DNAME"

chmod 600 "$KEYSTORE"
log "Keystore créé : $KEYSTORE"

# ── Mise à jour de key.properties ────────────────────────────
cat > "$KEY_PROPS" <<PROPS
storePassword=${STORE_PASS}
keyPassword=${STORE_PASS}
keyAlias=${KEY_ALIAS}
storeFile=zenth-release.keystore
PROPS

chmod 600 "$KEY_PROPS"
log "key.properties mis à jour."

# ── Vérification ──────────────────────────────────────────────
echo ""
log "Contenu du keystore :"
keytool -list -v -keystore "$KEYSTORE" -storepass "$STORE_PASS" \
    | grep -E "Alias|Valid|SHA|Owner" || true

# ── Résumé ────────────────────────────────────────────────────
echo ""
log "=== Résumé ==="
echo "  Keystore : $KEYSTORE"
echo "  Alias    : $KEY_ALIAS"
echo "  Validité : 10 000 jours (~27 ans)"
echo ""
warn "Sauvegarde le keystore en lieu sûr (hors du dépôt git) :"
echo "  cp $KEYSTORE ~/backup/zenth-release.keystore"
echo ""
warn "key.properties est dans .gitignore - ne jamais le committer."
echo ""
log "Tu peux maintenant builder l'APK :"
echo "  bash scripts/sign_apk.sh"
