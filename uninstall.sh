#!/bin/sh
# Removes minerve-gui installed by `make install` (cargo, ~/.cargo/bin) or install.sh (~/.local/bin).
# Only removes the minerve-gui executable; minerve (the terminal app) and config are left alone.
set -eu
CARGO_BIN="${CARGO_HOME:-"$HOME/.cargo"}/bin/minerve-gui"
LOCAL_BIN="${MINERVE_INSTALL_DIR:-"$HOME/.local/bin"}/minerve-gui"
removed=0
# Use cargo uninstall for the cargo copy so cargo's .crates.toml metadata stays in sync.
if [ -f "$CARGO_BIN" ] && command -v cargo >/dev/null 2>&1; then
    cargo uninstall minerve-gui
    removed=$((removed + 1))
fi
if [ -f "$LOCAL_BIN" ] && [ ! -L "$LOCAL_BIN" ]; then
    rm -f "$LOCAL_BIN"
    echo "Removed $LOCAL_BIN"
    removed=$((removed + 1))
fi
if [ "$removed" -eq 0 ]; then
    echo 'No minerve-gui binary found in ~/.cargo/bin or ~/.local/bin'
fi
if command -v minerve-gui >/dev/null 2>&1; then
    echo "Warning: minerve-gui is still on PATH at $(command -v minerve-gui)" >&2
    exit 1
fi
echo 'minerve-gui uninstalled.'
