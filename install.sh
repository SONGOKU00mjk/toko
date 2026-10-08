#!/usr/bin/env bash
#
# Installer for toko. Builds the release binary, installs it to
# ~/.local/bin/toko, and creates a default config at
# ~/.config/toko/config.lua on first install.
#
# Safe to run repeatedly. Never overwrites an existing config.
# Does not require root.

set -euo pipefail

SCRIPT_DIR="$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")" && pwd)"
cd "$SCRIPT_DIR"

BIN_DIR="$HOME/.local/bin"
CONFIG_DIR="$HOME/.config/toko"
CONFIG_FILE="$CONFIG_DIR/config.lua"
BIN_PATH="$BIN_DIR/toko"

if ! command -v cargo >/dev/null 2>&1; then
    cat >&2 <<'EOF'
error: `cargo` was not found on your PATH.

toko is written in Rust. Install the Rust toolchain first:

    curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh

Then restart your shell (or run `source "$HOME/.cargo/env"`) and try again.
EOF
    exit 1
fi

if ! command -v rustc >/dev/null 2>&1; then
    echo "error: rustc was not found. Your Rust installation may be incomplete." >&2
    exit 1
fi

echo "Using $(cargo --version) and $(rustc --version)"

echo "Building toko in release mode (this may take a few minutes)..."
cargo build --release

if [ ! -f "target/release/toko" ]; then
    echo "error: build completed but target/release/toko was not produced." >&2
    exit 1
fi

mkdir -p "$BIN_DIR"
install -m 0755 "target/release/toko" "$BIN_PATH"
echo "Installed binary: $BIN_PATH"

mkdir -p "$CONFIG_DIR"

if [ -e "$CONFIG_FILE" ]; then
    echo "Existing config preserved: $CONFIG_FILE"
else
    cat > "$CONFIG_FILE" <<'EOF'
-- Toko configuration

-- Wallpaper directory
wallpaper_dir = "~/Pictures/wallpapers"

-- Wallpaper backend: "auto", "awww", or "swaybg"
backend = "auto"

-- Maximum dimension of cached preview images
preview_max_dim = 1280
EOF
    echo "Created default config: $CONFIG_FILE"
fi

cat <<EOF

toko installed successfully.

  Binary : $BIN_PATH
  Config : $CONFIG_FILE

Run it with:

  toko

If 'toko' is not found, add ~/.local/bin to your PATH:

  export PATH="\$HOME/.local/bin:\$PATH"

Wallpaper backends ('awww', 'swaybg') are NOT installed by this script.
Install one of them separately and use a Wayland compositor that
supports wlr-layer-shell (sway, Hyprland, niri, river, etc.).
EOF
