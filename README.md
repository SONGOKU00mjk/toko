# toko

A terminal wallpaper browser and manager for Wayland.

Browse wallpapers with a live image preview, set them with a single
keypress, and keep a persistent preview cache so revisiting your
collection is instant. Built in Rust with `ratatui`.

> **Vibe coded.** This project was built iteratively with an AI coding
> assistant. The code is functional and tested, but treat it as a
> hobby project rather than a production-grade tool.

---

## Table of contents

- [Features](#features)
- [Requirements](#requirements)
- [Installation](#installation)
  - [Option 1 — one-line install (no clone)](#option-1--one-line-install-no-clone)
  - [Option 2 — clone and run install.sh](#option-2--clone-and-run-installsh)
  - [Option 3 — cargo install from git](#option-3--cargo-install-from-git)
  - [Option 4 — manual build](#option-4--manual-build)
- [Running](#running)
- [Configuration](#configuration)
- [Keyboard controls](#keyboard-controls)
- [Supported image formats](#supported-image-formats)
- [Cache](#cache)
- [GIF animation](#gif-animation)
- [Development](#development)
- [License](#license)

---

## Features

- **Two-pane TUI** — wallpaper list on the left, live image preview on
  the right.
- **Fullscreen preview** — press `Enter` to view the selected image
  filling the entire terminal, preserving aspect ratio.
- **Instant revisits** — three-layer preview cache (in-memory,
  on-disk, cold path) means navigating back to an image you've seen is
  effectively free.
- **Animated GIF playback** — multi-frame GIFs play at their native
  frame delays, encoded on a background thread so the UI stays
  responsive.
- **Persistent disk cache** — survives across sessions and is shared
  between terminals with different fonts and graphics protocols.
- **Backend abstraction** — automatically uses `awww` when its daemon
  is reachable, falls back to `swaybg`.
- **Sandboxed Lua configuration** — three settings in a Lua file; no
  I/O, no `os`, no plugins, no FFI.
- **Wayland-native terminal graphics** — Kitty, iTerm2, Sixel, or
  Unicode halfblocks as a universal fallback.
- **Preview metadata** — filename and pixel dimensions shown in the
  preview panel header.

---

## Requirements

| Requirement | Notes |
|---|---|
| Wayland session | X11 is not supported. |
| Compositor with `wlr-layer-shell` | sway, Hyprland, niri, river, Wayfire, etc. GNOME and KDE are not supported for wallpaper setting. |
| Terminal with graphics support | One of: Kitty graphics, iTerm2 inline images, Sixel, or Unicode halfblocks (works everywhere). |
| Rust toolchain | Only for building from source. Not required once installed. |
| A wallpaper backend | `awww` **or** `swaybg`, installed separately. |

`toko` does **not** install, configure, or manage the wallpaper
backends. Install them through your distribution's package manager:

```bash
# Example: Arch
sudo pacman -S awww     # or: sudo pacman -S swaybg

# Example: Debian/Ubuntu
sudo apt install swaybg
```

Then make sure your compositor supports them (see above).

---

## Installation

### Option 1 — one-line install (no clone)

```bash
curl -fsSL https://raw.githubusercontent.com/SONGOKU00mjk/toko/main/install-remote.sh | bash
```

Downloads the latest source into a temporary directory, builds it, and
installs the binary to `/usr/local/bin/toko`. You'll be prompted for
your password once (for the `sudo` install step). The default config is
written to `~/.config/toko/config.lua` on first install; your existing
config is never touched.

Because `/usr/local/bin` is already on the `PATH` of every user and
shell, no `.bashrc` edits or logout/login are needed. `toko` works
immediately in any new terminal.

### Option 2 — clone and run install.sh

```bash
git clone https://github.com/SONGOKU00mjk/toko.git
cd toko
./install.sh
```

The installer:

1. Verifies that `cargo` and `rustc` are on `PATH`.
2. Runs `cargo build --release` from the repo root.
3. Installs the built binary to `/usr/local/bin/toko` via `sudo`.
4. Creates `~/.config/toko/config.lua` with defaults **only if the
   file does not already exist**.
5. Prints the binary path, config path, and how to run `toko`.

It never launches `toko`, never touches shell config files, and is safe
to run repeatedly — upgrades replace only the binary, leaving your
config alone.

### Option 3 — `cargo install` from git

If you'd rather use Cargo's own install mechanism, which places the
binary in `~/.cargo/bin` (already on `PATH` for anyone with rustup):

```bash
cargo install --git https://github.com/SONGOKU00mjk/toko.git --locked
```

This does not create a config file. On first run, `toko` uses built-in
defaults (`~/Pictures/wallpapers`, backend `auto`, preview size 1280).
Create a config manually later if you want to override them — see
[Configuration](#configuration).

To uninstall:

```bash
cargo uninstall toko
```

### Option 4 — manual build

```bash
git clone https://github.com/SONGOKU00mjk/toko.git
cd toko
cargo build --release
sudo install -Dm755 target/release/toko /usr/local/bin/toko
mkdir -p ~/.config/toko
```

Then create `~/.config/toko/config.lua` manually if you want to
override the defaults. See [Configuration](#configuration) for the
schema.

### Verifying the install

```bash
which toko
toko
```

If `toko` runs and shows the two-pane UI, you're done.

---

## Running

```bash
toko
```

---

## Configuration

Config path, in order of precedence:

1. `$XDG_CONFIG_HOME/toko/config.lua`
2. `~/.config/toko/config.lua`

If the file doesn't exist, built-in defaults apply. Invalid Lua or an
invalid value prints a clear error and exits with status 1 — `toko`
will not start with a broken config.

### Example `config.lua`

```lua
-- Toko configuration

-- Directory scanned for wallpaper images.
wallpaper_dir = "~/Pictures/wallpapers"

-- Backend preference:
--   "auto"   prefer awww if usable, otherwise swaybg
--   "awww"   use awww only (no fallback)
--   "swaybg" use swaybg only (no fallback)
backend = "auto"

-- Longest side, in pixels, of the pre-scaled preview cache.
-- Larger = sharper previews, more disk usage, slower to build.
preview_max_dim = 1280
```

### Settings

| Key | Type | Default | Range / Values |
|---|---|---|---|
| `wallpaper_dir` | string | `"~/Pictures/wallpapers"` | Non-empty. `~` and `~/` expand to `$HOME`. |
| `backend` | string | `"auto"` | `"auto"`, `"awww"`, or `"swaybg"`. |
| `preview_max_dim` | integer | `1280` | `1`–`8192`. |

### How backend selection works

| Config value | Behavior |
|---|---|
| `"auto"` | Try `awww` first (verified with `awww query`). If the daemon is not running, fall back to `swaybg` (verified with `swaybg --version`). |
| `"awww"` | Use `awww` only. If its daemon is not running, no backend is available. |
| `"swaybg"` | Use `swaybg` only. If the binary is missing, no backend is available. |

When no backend is available, `toko` still starts and runs normally.
Pressing `s` shows an error in the status bar instead of setting the
wallpaper.

### Sandboxing

The Lua environment exposes only `string`, `table`, `math`, and `utf8`.
There is no `io`, `os`, `package`, `debug`, `ffi`, or network access.
The config chunk's `_ENV` is a private table pre-populated with the
defaults, so config code cannot read or write real Lua globals.

---

## Keyboard controls

| Key | Action |
|---|---|
| `↑` / `k` | Previous wallpaper |
| `↓` / `j` | Next wallpaper |
| `Home` | First wallpaper |
| `End` | Last wallpaper |
| `Enter` | Toggle fullscreen preview |
| `s` | Set the currently selected wallpaper |
| `q` / `Esc` | Quit, or exit fullscreen preview |

`Enter` toggles fullscreen preview and nothing else. It does **not**
set the wallpaper. Only `s` sets the wallpaper.

---

## Supported image formats

- PNG
- JPEG / JPG
- WebP
- GIF — animated GIFs play at their native frame rate; single-frame
  GIFs use the static path.

Files with other extensions are ignored during directory scanning.

---

## Cache

`toko` keeps previews fast with three layers:

1. **In-memory `HashMap<PreviewKey, Protocol>`** — per-session, instant.
   Cleared on exit.
2. **On-disk PNG + `.meta` sidecar** in `~/.cache/toko/` — persistent
   across sessions and shared between terminals.
3. **Cold path** — decode the source, pre-scale to `preview_max_dim`
   on the longest side, save to disk.

The on-disk cache is keyed on the source path and `preview_max_dim`, so
changing the dimension produces fresh entries without invalidating the
old ones. Source changes are detected via mtime + size in the sidecar;
stale entries are regenerated automatically.

To clear the cache:

```bash
rm -rf ~/.cache/toko
```

---

## GIF animation

Animated GIFs are decoded once (all frames, pre-scaled to
`preview_max_dim`) and played back using each frame's delay. Frame
encoding happens on a dedicated background thread, so playback never
blocks the event loop and `j`/`k` navigation stays responsive.

Frame delays below 20 ms are clamped to prevent malformed GIFs from
pinning a CPU core. Single-frame GIFs skip the animation path entirely
and use the standard static preview.

---

## Development

```bash
cargo check
cargo clippy -- -W clippy::all
cargo test
cargo build --release
cargo run --release
```

Development builds (`cargo run` without `--release`) are 20–40× slower
for the image pipeline. Use `--release` whenever you're benchmarking or
actually using the app.

### Project layout

```
src/
├── main.rs        entry point and event loop
├── app.rs         application state
├── ui.rs          rendering
├── animation.rs   animated GIF playback
├── cache.rs       on-disk preview cache
├── config.rs      Lua configuration loader
├── discover.rs    wallpaper directory scanning
├── preview.rs     background preview worker pool
└── backend/
    ├── mod.rs     backend trait and detection
    ├── awww.rs    awww implementation
    └── swaybg.rs  swaybg implementation
```

---

## License

BSD 2-Clause. See [LICENSE](LICENSE).
