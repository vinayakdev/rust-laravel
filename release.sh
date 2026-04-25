#!/usr/bin/env bash
set -euo pipefail

REPO_ROOT="$(cd "$(dirname "$0")" && pwd)"
ZED_EXT_DIR="$REPO_ROOT/zed-extension"
VERSION=$(grep '^version' "$ZED_EXT_DIR/extension.toml" | head -1 | sed 's/.*= *"\(.*\)"/\1/')
DIST_DIR="$REPO_ROOT/dist"
BUNDLE_NAME="rust-laravel-v$VERSION"
BUNDLE_DIR="$DIST_DIR/$BUNDLE_NAME"

echo "==> Building rust-laravel v$VERSION release bundle"

# ── 1. Build the LSP binary ──────────────────────────────────────────────────
echo "==> Building LSP binary (rust-php)..."
cargo build --release --bin rust-php --manifest-path "$REPO_ROOT/Cargo.toml"
LSP_BIN="$REPO_ROOT/target/release/rust-php"

# ── 2. Assemble bundle ───────────────────────────────────────────────────────
echo "==> Assembling bundle at $BUNDLE_DIR"
rm -rf "$BUNDLE_DIR"
mkdir -p "$BUNDLE_DIR/extension"

# Determine platform binary name (must match bundled_binary_path() in lib.rs)
OS_RAW="$(uname -s)"
ARCH_RAW="$(uname -m)"
case "$OS_RAW-$ARCH_RAW" in
  Darwin-arm64)  BIN_FILENAME="rust-php-macos-aarch64" ;;
  Darwin-x86_64) BIN_FILENAME="rust-php-macos-x86_64" ;;
  Linux-x86_64)  BIN_FILENAME="rust-php-linux-x86_64" ;;
  *)
    echo "Unsupported platform: $OS_RAW-$ARCH_RAW"
    exit 1
    ;;
esac

# Extension files (bin/ lives inside extension so Zed can find it)
mkdir -p "$BUNDLE_DIR/extension/bin"
mkdir -p "$BUNDLE_DIR/extension/src"
cp "$ZED_EXT_DIR/extension.toml"       "$BUNDLE_DIR/extension/"
cp "$ZED_EXT_DIR/Cargo.toml"           "$BUNDLE_DIR/extension/"
cp "$ZED_EXT_DIR/Cargo.lock"           "$BUNDLE_DIR/extension/"
cp "$ZED_EXT_DIR/src/lib.rs"           "$BUNDLE_DIR/extension/src/"

# Tell Cargo this is a standalone package (not part of any workspace)
echo "" >> "$BUNDLE_DIR/extension/Cargo.toml"
echo "[workspace]" >> "$BUNDLE_DIR/extension/Cargo.toml"
cp -r "$ZED_EXT_DIR/languages"         "$BUNDLE_DIR/extension/"
cp -r "$ZED_EXT_DIR/snippets"          "$BUNDLE_DIR/extension/"
cp "$LSP_BIN"                          "$BUNDLE_DIR/extension/bin/$BIN_FILENAME"
chmod +x                               "$BUNDLE_DIR/extension/bin/$BIN_FILENAME"

# Install instructions
cat > "$BUNDLE_DIR/INSTALL.md" << EOF
# rust-laravel v$VERSION — Local Install

## Install

1. Open Zed
2. Open the **Extensions** panel (Cmd+Shift+X)
3. Click **Install Dev Extension**
4. Select the \`extension/\` folder inside this archive

That's it — the LSP binary is bundled inside the extension, no PATH changes or config needed.
EOF

# ── 4. Zip ───────────────────────────────────────────────────────────────────
(
  cd "$DIST_DIR"
  zip -r "${BUNDLE_NAME}.zip" "$BUNDLE_NAME"
)

echo ""
echo "Done! Release bundle: $DIST_DIR/${BUNDLE_NAME}.zip"
echo ""
echo "Contents:"
find "$BUNDLE_DIR" -type f | sed "s|$BUNDLE_DIR/||"
