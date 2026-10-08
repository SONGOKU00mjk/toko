#!/usr/bin/env bash
#
# Installer for toko. Builds the release binary and installs it to
# /usr/local/bin/toko, which is already on PATH for every user and
# every shell.
#
# Requires sudo for the final install step. Never launches toko.

set -euo pipefail

SCRIPT_DIR="$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")" && pwd)"
cd "$SCRIPT_DIR"

BIN_DIR="/usr/local/bin"
BIN_PATH="$BIN_DIR/toko"
CONFIG_DIR="$HOME/.config/toko"
CONFIG_FILE="$CONFIG_DIR/config.lua"

# --- 1. Verify Rust toolchain ------------------------------------------------

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

# --- 2. Build ----------------------------------------------------------------

echo "Building toko in release mode (this may take a few minutes)..."
cargo build --release

if [ ! -x "target/release/toko" ]; then
    echo "error: build completed but target/release/toko was not produced." >&2
    exit 1
fi

# --- 3. Install the binary (needs sudo for /usr/local/bin) -------------------

echo "Installing to $BIN_PATH (sudo required)..."
sudo install -Dm755 "target/release/toko" "$BIN_PATH"
echo "Installed binary: $BIN_PATH"

# --- 4. Create the config on first install -----------------------------------

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

# --- 5. Success message ------------------------------------------------------

echo
echo "Toko installed successfully!"
echo
echo "Run:"
echo "    toko"
echo
echo "Binary:"
echo "    $BIN_PATH"
echo
echo "Config:"
echo "    $CONFIG_FILE"
echo
