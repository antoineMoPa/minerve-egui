#!/bin/sh
# Installs minerve-gui and the minerve terminal app it depends on (minerve-gui starts `minerve --serve`).
set -eu
REPO=antoineMoPa/minerve-egui
TUI_REPO=antoineMoPa/minerve
INSTALL_DIR=${MINERVE_INSTALL_DIR:-"$HOME/.local/bin"}
VERSION=${MINERVE_GUI_VERSION:-latest}
case "$VERSION" in
    latest) BASE="https://github.com/$REPO/releases/latest/download" ;;
    v[0-9]*.[0-9]*.[0-9]*) BASE="https://github.com/$REPO/releases/download/$VERSION" ;;
    *) echo 'MINERVE_GUI_VERSION must be latest or vMAJOR.MINOR.PATCH' >&2; exit 1 ;;
esac
# Optional mirror, also useful for offline installer tests.
BASE=${MINERVE_GUI_DOWNLOAD_BASE_URL:-$BASE}
# minerve's own installer, run as is: it knows how to install minerve. MINERVE_VERSION picks its version.
case "${MINERVE_VERSION:-latest}" in
    latest) TUI_INSTALLER="https://github.com/$TUI_REPO/releases/latest/download/install.sh" ;;
    v[0-9]*.[0-9]*.[0-9]*) TUI_INSTALLER="https://github.com/$TUI_REPO/releases/download/$MINERVE_VERSION/install.sh" ;;
    *) echo 'MINERVE_VERSION must be latest or vMAJOR.MINOR.PATCH' >&2; exit 1 ;;
esac
TUI_INSTALLER=${MINERVE_INSTALLER_URL:-$TUI_INSTALLER}
case "$(uname -s):$(uname -m)" in
    Darwin:arm64) TARGET=aarch64-apple-darwin ;;
    Linux:x86_64|Linux:amd64) TARGET=x86_64-unknown-linux-gnu ;;
    Linux:aarch64|Linux:arm64) TARGET=aarch64-unknown-linux-gnu ;;
    *) echo 'Supported: macOS Apple Silicon, Linux x64/ARM64.' >&2; exit 1 ;;
esac
for cmd in curl tar mktemp install; do
    command -v "$cmd" >/dev/null || { echo "Missing required command: $cmd" >&2; exit 1; }
done
if command -v sha256sum >/dev/null; then
    SUM=sha256sum
elif command -v shasum >/dev/null; then
    SUM='shasum -a 256'
else
    echo 'Missing SHA-256 tool (sha256sum or shasum)' >&2; exit 1
fi
TMP_DIR=$(mktemp -d)
trap 'rm -rf "$TMP_DIR"' 0
trap 'exit 1' 1 2 15

# 1. The terminal app, first: the window cannot work without it. Minerve's own installer
#    does the work; this repo adds nothing to it.
if [ "${MINERVE_NO_TUI:-}" != 1 ]; then
    echo 'Installing minerve (terminal app)...'
    curl -fsSL "$TUI_INSTALLER" -o "$TMP_DIR/install-minerve.sh" || { echo "Download failed: $TUI_INSTALLER" >&2; exit 1; }
    MINERVE_INSTALL_DIR="$INSTALL_DIR" sh "$TMP_DIR/install-minerve.sh" || { echo 'minerve install failed' >&2; exit 1; }
fi

# 2. The window.
ARCHIVE="minerve-gui-$TARGET.tar.gz"
echo "Downloading minerve-gui ($TARGET)..."
curl -fsSL "$BASE/$ARCHIVE" -o "$TMP_DIR/$ARCHIVE"
curl -fsSL "$BASE/$ARCHIVE.sha256" -o "$TMP_DIR/$ARCHIVE.sha256"
# Verify just this archive, without interpreting filenames from the checksum file.
EXPECTED=$(awk 'NR == 1 { print $1 }' "$TMP_DIR/$ARCHIVE.sha256")
ACTUAL=$($SUM "$TMP_DIR/$ARCHIVE" | awk '{ print $1 }')
[ ${#EXPECTED} -eq 64 ] && [ "$EXPECTED" = "$ACTUAL" ] || { echo 'Checksum verification failed' >&2; exit 1; }
# Extract only the executable from our flat release archive.
tar -xzf "$TMP_DIR/$ARCHIVE" -C "$TMP_DIR" minerve-gui
[ -f "$TMP_DIR/minerve-gui" ] && [ ! -L "$TMP_DIR/minerve-gui" ] || { echo 'Archive has no regular minerve-gui executable' >&2; exit 1; }
mkdir -p "$INSTALL_DIR"
install -m 0755 "$TMP_DIR/minerve-gui" "$INSTALL_DIR/.minerve-gui-install-$$"
mv -f "$INSTALL_DIR/.minerve-gui-install-$$" "$INSTALL_DIR/minerve-gui"
echo "Installed $INSTALL_DIR/minerve-gui"
case ":$PATH:" in
    *":$INSTALL_DIR:"*) ;;
    *)
        # Avoid editing shell configuration from a piped script. Print an exact command.
        echo 'Add the install directory to PATH in your shell configuration:'
        if [ "$INSTALL_DIR" = "$HOME/.local/bin" ]; then
            echo '  export PATH="$HOME/.local/bin:$PATH"'
        else
            printf '  Add %s to PATH\n' "$INSTALL_DIR"
        fi
        ;;
esac
echo 'Run minerve-gui to open the window, or minerve for the terminal app.'
