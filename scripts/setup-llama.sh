#!/usr/bin/env bash
# setup-llama.sh
# Downloads the llama.cpp built-in runtime binaries for macOS or Linux.
# Run once from the repo root before starting the dev environment:
#   bash scripts/setup-llama.sh

set -euo pipefail

RELEASE="b9859"
DEST="$(dirname "$0")/../src-tauri/binaries/llama"
TMP="$(mktemp -d)"

mkdir -p "$DEST"

OS="$(uname -s)"
ARCH="$(uname -m)"

if [[ "$OS" == "Darwin" ]]; then
  if [[ "$ARCH" == "arm64" ]]; then
    ARCHIVE="llama-${RELEASE}-bin-macos-arm64.tar.gz"
  else
    ARCHIVE="llama-${RELEASE}-bin-macos-x64.tar.gz"
  fi
elif [[ "$OS" == "Linux" ]]; then
  if [[ "$ARCH" == "aarch64" ]]; then
    ARCHIVE="llama-${RELEASE}-bin-ubuntu-arm64.tar.gz"
  else
    ARCHIVE="llama-${RELEASE}-bin-ubuntu-x64.tar.gz"
  fi
else
  echo "Unsupported OS: $OS. Use setup-llama.ps1 on Windows." >&2
  exit 1
fi

URL="https://github.com/ggerganov/llama.cpp/releases/download/${RELEASE}/${ARCHIVE}"

echo "Downloading llama.cpp ${RELEASE} (${OS} ${ARCH})..."
curl -L --progress-bar "$URL" -o "$TMP/$ARCHIVE"
echo "Extracting..."
tar -xzf "$TMP/$ARCHIVE" -C "$TMP"

SRC_DIR="$TMP/llama-${RELEASE}"

# Copy llama-server and its shared libs
cp "$SRC_DIR/llama-server" "$DEST/llama-server"
chmod +x "$DEST/llama-server"

find "$SRC_DIR" \( -name "*.so" -o -name "*.so.*" -o -name "*.dylib" \) | while read -r f; do
  cp "$f" "$DEST/$(basename "$f")"
done

# Set RPATH so llama-server finds its .so/.dylib siblings at runtime
if [[ "$OS" == "Darwin" ]]; then
  install_name_tool -add_rpath "@executable_path" "$DEST/llama-server" 2>/dev/null || true
elif command -v patchelf &>/dev/null; then
  patchelf --set-rpath '$ORIGIN' "$DEST/llama-server" || true
fi

echo ""
echo "Done. Files in src-tauri/binaries/llama/:"
ls -lh "$DEST" | grep -v ".gitkeep"
echo "Built-in runtime ready. Run 'pnpm tauri:dev' to start Ark."

rm -rf "$TMP"
