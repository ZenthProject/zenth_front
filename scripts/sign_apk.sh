#!/usr/bin/env bash
# ============================================================
# sign_apk.sh - Build, signe et installe l'APK Zenth
#
# Usage :
#   bash scripts/sign_apk.sh             # build + signe
#   bash scripts/sign_apk.sh --install   # build + signe + installe via adb
# ============================================================
set -euo pipefail

KEYSTORE="src-tauri/gen/android/zenth-release.keystore"
KEY_ALIAS="zenth-key"
KEY_PROPS="src-tauri/gen/android/key.properties"
APK_UNSIGNED="src-tauri/gen/android/app/build/outputs/apk/universal/release/app-universal-release-unsigned.apk"
APK_SIGNED="src-tauri/gen/android/app/build/outputs/apk/universal/release/app-universal-release.apk"
INSTALL="${1:-}"

GREEN='\033[0;32m'; YELLOW='\033[1;33m'; RED='\033[0;31m'; BLUE='\033[0;34m'; NC='\033[0m'
log()   { echo -e "${GREEN}[sign]${NC} $*"; }
info()  { echo -e "${BLUE}[info]${NC}  $*"; }

# ── Android SDK / NDK ─────────────────────────────────────────
if [ -z "${ANDROID_HOME:-}" ]; then
    export ANDROID_HOME="${HOME}/Android/Sdk"
fi
if [ -z "${ANDROID_NDK_HOME:-}" ]; then
    # Prend le NDK le plus récent installé
    NDK_DIR=$(ls -d "${ANDROID_HOME}/ndk/"* 2>/dev/null | sort -V | tail -1)
    [ -n "$NDK_DIR" ] || error "NDK introuvable dans ${ANDROID_HOME}/ndk/. Lance : sdkmanager 'ndk;27.0.12077973'"
    export ANDROID_NDK_HOME="$NDK_DIR"
fi
info "ANDROID_HOME     : $ANDROID_HOME"
info "ANDROID_NDK_HOME : $ANDROID_NDK_HOME"
warn()  { echo -e "${YELLOW}[warn]${NC}  $*"; }
error() { echo -e "${RED}[error]${NC} $*" >&2; exit 1; }

# ── Vérifie les outils requis ─────────────────────────────────
for cmd in bun apksigner; do
    command -v "$cmd" &>/dev/null || error "'$cmd' introuvable. Installe le SDK Android (apksigner) et bun."
done

# ── Mot de passe ──────────────────────────────────────────────
# Priorité : variable d'env ZENTH_KEYSTORE_PASS → key.properties → saisie interactive
if [ -n "${ZENTH_KEYSTORE_PASS:-}" ]; then
    STORE_PASS="$ZENTH_KEYSTORE_PASS"
    KEY_PASS="$ZENTH_KEYSTORE_PASS"
    info "Mot de passe lu depuis \$ZENTH_KEYSTORE_PASS"
elif [ -f "$KEY_PROPS" ]; then
    STORE_PASS=$(grep '^storePassword=' "$KEY_PROPS" | cut -d'=' -f2-)
    KEY_PASS=$(grep '^keyPassword=' "$KEY_PROPS" | cut -d'=' -f2-)
    info "Mot de passe lu depuis key.properties"
else
    echo -n "Mot de passe du keystore : "
    read -rs STORE_PASS
    echo ""
    KEY_PASS="$STORE_PASS"
fi

[ -n "$STORE_PASS" ] || error "Mot de passe vide."
[ -f "$KEYSTORE" ]   || error "Keystore introuvable : $KEYSTORE"

# ── Build ─────────────────────────────────────────────────────
# Note: on ne lance PAS gradlew clean séparément - cela laisse un daemon Gradle
# actif qui bloque le WebSocket server du CLI Tauri lors du vrai build.
log "Build release APK (aarch64)..."
BUILD_LOG=$(mktemp)
GRADLE_OPTS="-Dorg.gradle.daemon=false" \
    bun run tauri android build --target aarch64 2>&1 | tee "$BUILD_LOG" | \
    grep -E "BUILD|Compiling|Finished|error\[|warning\[|apk|Bundling" || true

# Vérifie si le build a réellement échoué
if grep -q "BUILD FAILED\|error: script\|Connection refused" "$BUILD_LOG"; then
    echo ""
    echo -e "${RED}══ Gradle full log (dernières 80 lignes) ══${NC}"
    tail -80 "$BUILD_LOG"
    rm -f "$BUILD_LOG"
    error "Build APK échoué (voir log ci-dessus)."
fi
rm -f "$BUILD_LOG"

# Cherche l'APK produit (signé ou non signé)
APK_OUT=""
for candidate in \
    "src-tauri/gen/android/app/build/outputs/apk/arm64/release/app-arm64-release.apk" \
    "src-tauri/gen/android/app/build/outputs/apk/arm64/release/app-arm64-release-unsigned.apk" \
    "src-tauri/gen/android/app/build/outputs/apk/universal/release/app-universal-release.apk" \
    "src-tauri/gen/android/app/build/outputs/apk/universal/release/app-universal-release-unsigned.apk"; do
    [ -f "$candidate" ] && APK_OUT="$candidate" && break
done

[ -n "$APK_OUT" ] || error "APK introuvable après le build."
info "APK trouvé : $APK_OUT"

# ── Nom final : Zenth_{version}_android.apk ───────────────────
VERSION=$(python3 -c "import json; print(json.load(open('src-tauri/tauri.conf.json'))['version'])" 2>/dev/null \
    || grep '^version' src-tauri/Cargo.toml | head -1 | cut -d'"' -f2)
OUT_DIR="${ZENTH_OUT:-releases}"
APK_FINAL="${OUT_DIR}/Zenth_${VERSION}_android.apk"
mkdir -p "$OUT_DIR"

# ── Vérification de signature existante ───────────────────────
ALREADY_SIGNED=false
if apksigner verify "$APK_OUT" &>/dev/null; then
    ALREADY_SIGNED=true
fi

if [ "$ALREADY_SIGNED" = false ]; then
    log "Signature de l'APK..."
    apksigner sign \
        --ks "$KEYSTORE" \
        --ks-key-alias "$KEY_ALIAS" \
        --ks-pass "pass:${STORE_PASS}" \
        --key-pass "pass:${KEY_PASS}" \
        --out "$APK_FINAL" \
        "$APK_OUT"
    log "APK signé."
else
    info "APK déjà signé par Gradle (key.properties détecté)."
    cp "$APK_OUT" "$APK_FINAL"
fi

APK_OUT="$APK_FINAL"

# ── Vérification finale ───────────────────────────────────────
log "Vérification de la signature..."
apksigner verify --verbose "$APK_OUT" | grep -E "Verified|v[0-9] scheme"

# ── Taille et chemin ──────────────────────────────────────────
SIZE=$(du -sh "$APK_OUT" | cut -f1)
echo ""
log "APK prêt : $APK_OUT ($SIZE)"

# ── Installation ADB (optionnelle) ───────────────────────────
if [ "$INSTALL" = "--install" ]; then
    command -v adb &>/dev/null || error "adb introuvable."
    DEVICE=$(adb devices | grep -v "List" | grep "device$" | awk '{print $1}' | head -1)
    [ -n "$DEVICE" ] || error "Aucun appareil ADB connecté. Active le débogage USB."

    info "Appareil : $DEVICE"
    log "Installation sur l'appareil..."
    adb -s "$DEVICE" install -r "$APK_OUT"
    log "Installation terminée."
fi

echo ""
echo -e "${BLUE}Pour installer manuellement :${NC}"
echo -e "  adb install -r \"$APK_OUT\""
echo -e "  # ou transfère le fichier via USB sans passer par FromSmash"
