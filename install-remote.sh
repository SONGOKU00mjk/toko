#!/usr/bin/env bash
#
# Remote installer for toko. Fetches the source into a temporary
# directory, builds it, and installs the binary. Intended to be run as:
#
#   curl -fsSL https://raw.githubusercontent.com/SONGOKU00mjk/toko/main/install-remote.sh | bash
#
# Does not require a prior clone. Never overwrites an existing config.
# Does not require root.

set -euo pipefail

REPO_URL="${TOKO_REPO_URL:-https://github.com/SONGOKU00mjk/toko.git}"
BRANCH="${TOKO_BRANCH:-main}"

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

TMPDIR="$(mktemp -d -t toko-install.XXXXXX)"
trap 'rm -rf "$TMPDIR"' EXIT

echo "Fetching toko from $REPO_URL ($BRANCH)..."
if command -v git >/dev/null 2>&1; then
    git clone --depth 1 --branch "$BRANCH" "$REPO_URL" "$TMPDIR/toko"
else
    TARBALL="$TMPDIR/toko.tar.gz"
    TAR_URL="${REPO_URL%.git}/archive/refs/heads/${BRANCH}.tar.gz"
    curl -fsSL "$TAR_URL" -o "$TARBALL"
    mkdir -p "$TMPDIR/toko"
    tar -xzf "$TARBALL" -C "$TMPDIR/toko" --strip-components=1
fi

cd "$TMPDIR/toko"

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
