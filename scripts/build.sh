#!/usr/bin/env bash
# ============================================================
# build.sh - Build Zenth pour toutes les plateformes
#
# Usage :
#   bash scripts/build.sh [--force] [cible...]
#
# Cibles disponibles :
#   front      - Build frontend (bun)
#   linux      - DEB + AppImage (natif)
#   apk        - Android APK (ARM64)
#   windows    - EXE (cross-compilation via cargo-xwin ou Docker)
#   all        - Tout builder (défaut si aucun argument)
#
# Options :
#   --force    - Rebuild même si l'artefact existe déjà dans releases/
#
# Exemples :
#   bash scripts/build.sh all
#   bash scripts/build.sh apk --force
#   bash scripts/build.sh linux apk
#
# Artefacts produits dans releases/ (séparé de dist/ qui est le frontend Vite)
# ============================================================
set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
PROJECT_DIR="$(cd "$SCRIPT_DIR/.." && pwd)"
cd "$PROJECT_DIR"

GREEN='\033[0;32m'; YELLOW='\033[1;33m'; RED='\033[0;31m'; BLUE='\033[0;34m'; BOLD='\033[1m'; NC='\033[0m'
log()     { echo -e "${GREEN}[build]${NC} $*"; }
info()    { echo -e "${BLUE}[info]${NC}  $*"; }
warn()    { echo -e "${YELLOW}[warn]${NC}  $*"; }
error()   { echo -e "${RED}[error]${NC} $*" >&2; exit 1; }
section() { echo -e "\n${BOLD}${BLUE}══════════════════════════════════════${NC}"; echo -e "${BOLD}  $*${NC}"; echo -e "${BOLD}${BLUE}══════════════════════════════════════${NC}\n"; }

# ── Version ───────────────────────────────────────────────────
VERSION=$(python3 -c "import json; print(json.load(open('src-tauri/tauri.conf.json'))['version'])" 2>/dev/null \
    || grep '^version' src-tauri/Cargo.toml | head -1 | cut -d'"' -f2)
info "Version : $VERSION"

# Dossier de sortie des binaires - séparé de dist/ (frontend Vite)
OUT="releases"
mkdir -p "$OUT"

# ── Options + sélection des cibles ───────────────────────────
FORCE=false
TARGETS=()
for arg in "$@"; do
    if [ "$arg" = "--force" ]; then FORCE=true; else TARGETS+=("$arg"); fi
done
if [ ${#TARGETS[@]} -eq 0 ] || [[ " ${TARGETS[*]} " == *" all "* ]]; then
    TARGETS=(front linux apk)
fi
# 'publish' ne fait pas partie de 'all' — doit être demandé explicitement
$FORCE && info "Mode --force : les artefacts existants seront reconstruits."

# Supprime les artefacts ciblés si --force
if $FORCE; then
    for t in "${TARGETS[@]}"; do
        case "$t" in
            linux)   rm -f "$OUT/Zenth_${VERSION}_linux_amd64.deb" "$OUT/Zenth_${VERSION}_linux.AppImage" ;;
            apk)     rm -f "$OUT/Zenth_${VERSION}_android.apk" ;;
            windows) rm -f "$OUT/Zenth_${VERSION}_windows_setup.exe" ;;
        esac
    done
fi

# ════════════════════════════════════════════════════════════════
# FRONT - Build frontend
# ════════════════════════════════════════════════════════════════
build_front() {
    section "Frontend build"
    command -v bun &>/dev/null || error "'bun' introuvable."
    log "Installation des dépendances..."
    bun install
    log "Build frontend..."
    bun run build
    log "Frontend buildé dans dist/"
}

# ════════════════════════════════════════════════════════════════
# LINUX - DEB + AppImage
# ════════════════════════════════════════════════════════════════
build_linux() {
    section "Linux build (DEB + AppImage)"
    DEB_DEST="$OUT/Zenth_${VERSION}_linux_amd64.deb"
    APPIMAGE_DEST="$OUT/Zenth_${VERSION}_linux.AppImage"
    if [ -f "$DEB_DEST" ] && [ -f "$APPIMAGE_DEST" ]; then
        info "DEB + AppImage déjà présents dans $OUT/, skip."
        return 0
    fi
    command -v bun &>/dev/null || error "'bun' introuvable."
    command -v cargo &>/dev/null || error "'cargo' introuvable."

    log "Build Tauri Linux (DEB + AppImage)..."
    bun run tauri build --bundles deb,appimage 2>&1

    DEB_SRC=$(find src-tauri/target/release/bundle/deb -name "*.deb" 2>/dev/null | head -1)
    APPIMAGE_SRC=$(find src-tauri/target/release/bundle/appimage -name "*.AppImage" 2>/dev/null | head -1)

    if [ -n "$DEB_SRC" ]; then
        cp "$DEB_SRC" "$DEB_DEST"
        log "DEB : $DEB_DEST"
    else
        warn "DEB introuvable après le build."
    fi

    if [ -n "$APPIMAGE_SRC" ]; then
        cp "$APPIMAGE_SRC" "$APPIMAGE_DEST"
        chmod +x "$APPIMAGE_DEST"
        log "AppImage : $APPIMAGE_DEST"
    else
        warn "AppImage introuvable après le build."
    fi
}

# ════════════════════════════════════════════════════════════════
# APK - Android
# ════════════════════════════════════════════════════════════════
build_apk() {
    section "Android build (APK ARM64)"
    APK_DEST="$OUT/Zenth_${VERSION}_android.apk"
    if [ -f "$APK_DEST" ]; then
        info "APK déjà présent dans $OUT/, skip. (--force pour rebuilder)"
        return 0
    fi
    ZENTH_OUT="$OUT" bash "$SCRIPT_DIR/sign_apk.sh"
    if [ -f "$APK_DEST" ]; then
        log "APK : $APK_DEST"
    else
        warn "APK introuvable dans $OUT/ - vérifie sign_apk.sh"
    fi
}

# ════════════════════════════════════════════════════════════════
# PUBLISH - Signe + uploade sur RustFS
# ════════════════════════════════════════════════════════════════
publish() {
    section "Publish v${VERSION} → RustFS"

    # Clé privée : USB chiffré ou variable d'env CI
    local privkey="${ZENTH_SIGN_KEY:-/mnt/usbkey/ed25519_private.pem}"
    [ -f "$privkey" ] || error "Clé privée introuvable : $privkey\n  Monte la clé USB ou définis ZENTH_SIGN_KEY."

    # URL publique RustFS (ex: https://files.lucas-sanchez.fr/zenth)
    local base_url="${RUSTFS_PUBLIC_URL:-}"
    [ -n "$base_url" ] || error "Variable RUSTFS_PUBLIC_URL non définie.\n  Ex: export RUSTFS_PUBLIC_URL=https://files.lucas-sanchez.fr/zenth"

    # Endpoint S3 RustFS pour mc (MinIO Client)
    local mc_alias="${RUSTFS_MC_ALIAS:-rustfs}"
    local mc_bucket="${RUSTFS_BUCKET:-zenth-releases}"

    command -v mc &>/dev/null || error "'mc' (MinIO client) introuvable. Installe : https://min.io/docs/minio/linux/reference/minio-mc.html"

    log "Génération du manifest.json signé..."
    bash "$SCRIPT_DIR/sign_release.sh" "$VERSION" "$privkey" "$OUT" "$base_url"

    log "Upload des artefacts v${VERSION} vers ${mc_alias}/${mc_bucket}/ ..."
    # Upload chaque binaire de la version
    for f in "$OUT"/Zenth_"${VERSION}"_*; do
        [ -f "$f" ] || continue
        local fname
        fname=$(basename "$f")
        mc cp "$f" "${mc_alias}/${mc_bucket}/${fname}" --quiet
        log "  ↑ $fname"
    done

    # Upload manifest.json (écrase l'ancien)
    mc cp "$OUT/manifest.json" "${mc_alias}/${mc_bucket}/manifest.json" --quiet
    log "  ↑ manifest.json"

    # Rendre le manifest public (lecture anonyme)
    mc anonymous set download "${mc_alias}/${mc_bucket}/manifest.json" 2>/dev/null || true

    echo ""
    info "Publié : ${base_url}/manifest.json"
}

# ════════════════════════════════════════════════════════════════
# WINDOWS - Redirige vers build_windows.ps1 (à lancer sur Windows)
# ════════════════════════════════════════════════════════════════
build_windows() {
    section "Windows build"
    warn "La cible 'windows' doit être buildée sur une machine Windows."
    echo ""
    echo -e "${YELLOW}Lance ce script sur ton PC Windows :${NC}"
    echo -e "  ${BLUE}.\\scripts\\build_windows.ps1${NC}"
    echo -e "  ${BLUE}.\\scripts\\build_windows.ps1 -Force${NC}   # rebuild même si déjà présent"
    echo -e "  ${BLUE}.\\scripts\\build_windows.ps1 -Bundle nsis${NC}  # NSIS seulement"
    echo ""
    echo -e "Prérequis Windows : Rust (rustup.rs) + Bun (bun.sh) + NSIS 3.x"
    echo ""
}

# ════════════════════════════════════════════════════════════════
# Résumé final
# ════════════════════════════════════════════════════════════════
print_summary() {
    section "Artefacts produits"
    local found=false
    while IFS= read -r f; do
        SIZE=$(du -sh "$f" | cut -f1)
        echo -e "  ${GREEN}✓${NC} $f  (${SIZE})"
        found=true
    done < <(find "$OUT/" -maxdepth 1 -type f | sort)
    $found || warn "Aucun artefact trouvé dans $OUT/"
    echo ""
    info "Terminé. Artefacts dans : $PROJECT_DIR/$OUT/"
}

# ════════════════════════════════════════════════════════════════
# Dispatch
# ════════════════════════════════════════════════════════════════
START=$(date +%s)

for target in "${TARGETS[@]}"; do
    case "$target" in
        front)   build_front   ;;
        linux)   build_linux   ;;
        apk)     build_apk     ;;
        windows) build_windows ;;
        publish) publish       ;;
        all)     ;;
        *) warn "Cible inconnue : '$target' (ignorée). Cibles valides : front linux apk windows publish all" ;;
    esac
done

END=$(date +%s)
ELAPSED=$((END - START))

print_summary
info "Durée totale : ${ELAPSED}s"
