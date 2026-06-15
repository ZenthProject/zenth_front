#!/usr/bin/env bash
# ============================================================
# gen_update_key.sh - Génère la paire de clés Ed25519 pour signer les releases
#
# Usage :
#   bash scripts/gen_update_key.sh
#
# Sorties :
#   ed25519_private.pem   - clé privée (à stocker dans GitLab CI variable ED25519_PRIVATE_KEY)
#   ed25519_public.pem    - clé publique (pour référence)
#   update_pubkey.hex     - 32 bytes publics en hex (à coller dans pages/update/mod.rs)
#   update_pubkey.rs      - snippet Rust prêt à coller dans UPDATE_PUBKEY
#
# NE JAMAIS COMMITTER ed25519_private.pem dans Git.
# ============================================================
set -euo pipefail

OUTDIR="${1:-.}"
mkdir -p "$OUTDIR"

PRIV="$OUTDIR/ed25519_private.pem"
PUB="$OUTDIR/ed25519_public.pem"
HEX="$OUTDIR/update_pubkey.hex"
RS="$OUTDIR/update_pubkey.rs"

echo "Génération de la paire de clés Ed25519..."
openssl genpkey -algorithm Ed25519 -out "$PRIV"
openssl pkey -in "$PRIV" -pubout -out "$PUB"

# Extrait les 32 bytes bruts de la clé publique (DER → skip 12 bytes d'en-tête)
RAW_HEX=$(openssl pkey -in "$PRIV" -pubout -outform DER 2>/dev/null \
    | tail -c 32 | xxd -p | tr -d '\n')

echo "$RAW_HEX" > "$HEX"

# Formate en tableau Rust
RUST_BYTES=$(echo "$RAW_HEX" | fold -w2 | awk '{printf "0x%s, ", $0}' | sed 's/, $//')
cat > "$RS" <<RUST
// Clé publique Ed25519 - coller dans src-tauri/src/pages/update/mod.rs
const UPDATE_PUBKEY: &[u8; 32] = &[
    $(echo "$RUST_BYTES" | fold -w48 | sed 's/^/    /')
];
RUST

echo ""
echo "Fichiers générés dans : $OUTDIR"
echo "  $PRIV   ← stocker dans GitLab CI > Variables > ED25519_PRIVATE_KEY (base64 -w0 < ed25519_private.pem)"
echo "  $PUB"
echo "  $HEX"
echo "  $RS       ← coller dans src-tauri/src/pages/update/mod.rs"
echo ""
echo "Clé publique (hex) :"
cat "$HEX"
echo ""
echo "Pour encoder la clé privée en base64 (variable CI) :"
echo "  base64 -w0 < $PRIV"
