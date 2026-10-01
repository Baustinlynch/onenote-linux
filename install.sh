#!/usr/bin/env bash
# ═══════════════════════════════════════════════════════════════════════
# onenote-linux — One-liner installer
# ═══════════════════════════════════════════════════════════════════════
# ⚠️  WARNING: This project is VIBE-CODED.
#    It was written with AI assistance and has NOT been thoroughly audited.
#    Use at your own risk. Review the code before running on sensitive systems.
# ═══════════════════════════════════════════════════════════════════════
# Installs system dependencies (Arch/Arch-based) and the latest release binary
# from GitHub. Run with: curl -fsSL <url> | bash
# ═══════════════════════════════════════════════════════════════════════

set -euo pipefail

REPO="Baustinlynch/onenote-linux"
BINARY_NAME="onenote-linux"
INSTALL_DIR="${HOME}/.local/bin"
DESKTOP_DIR="${HOME}/.local/share/applications"
ICON_DIR="${HOME}/.local/share/icons/hicolor"

# ── colours ──────────────────────────────────────────────────────────────
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
BLUE='\033[0;34m'
NC='\033[0m' # No Colour

log()   { printf "${BLUE}[%s]${NC} %s\n" "$(date '+%H:%M:%S')" "$*"; }
warn()  { printf "${YELLOW}[WARN]${NC} %s\n" "$*"; }
err()   { printf "${RED}[ERR]${NC} %s\n" "$*" >&2; }
ok()    { printf "${GREEN}[OK]${NC} %s\n" "$*"; }

# ── vibe warning ────────────────────────────────────────────────────────
cat <<'EOF'
╔═══════════════════════════════════════════════════════════════════════╗
║  ⚠️  THIS PROJECT IS VIBE-CODED                                       ║
║                                                                      ║
║  This software was generated with AI assistance and has NOT been    ║
║  thoroughly audited for security, correctness, or completeness.     ║
║                                                                      ║
║  • Review the source before running on sensitive systems            ║
║  • No warranty, express or implied                                  ║
║  • Not affiliated with Microsoft Corporation                        ║
║                                                                      ║
║  Source: https://github.com/Baustinlynch/onenote-linux              ║
╚═══════════════════════════════════════════════════════════════════════╝
EOF

read -rp $'\nContinue installation? [y/N] ' -n 1
echo
[[ $REPLY =~ ^[Yy]$ ]] || { err "Aborted."; exit 1; }

# ── detect distro ───────────────────────────────────────────────────────
if ! command -v pacman &>/dev/null; then
    err "This installer only supports Arch-based distros (uses pacman)."
    err "For other distros, install dependencies manually and download the binary from:"
    err "  https://github.com/${REPO}/releases"
    exit 1
fi

# ── install deps ────────────────────────────────────────────────────────
log "Installing system dependencies via pacman..."
sudo pacman -S --needed --noconfirm \
    webkit2gtk-4.1 \
    gtk4 \
    libayatana-appindicator \
    libnotify \
    xdg-utils

# ── fetch latest release ────────────────────────────────────────────────
log "Fetching latest release info from GitHub..."
LATEST_URL="https://api.github.com/repos/${REPO}/releases/latest"
ASSET_URL=$(curl -fsSL "${LATEST_URL}" |
    grep -o '"browser_download_url": *"[^"]*'"${BINARY_NAME}"'[^"]*"' |
    head -1 |
    cut -d'"' -f4)

if [[ -z "${ASSET_URL}" ]]; then
    err "Could not find ${BINARY_NAME} in latest release assets."
    err "Check https://github.com/${REPO}/releases"
    exit 1
fi

log "Downloading binary from ${ASSET_URL}..."
mkdir -p "${INSTALL_DIR}"
curl -fsSL "${ASSET_URL}" -o "${INSTALL_DIR}/${BINARY_NAME}"
chmod +x "${INSTALL_DIR}/${BINARY_NAME}"
ok "Binary installed to ${INSTALL_DIR}/${BINARY_NAME}"

# ── desktop entry & icons ───────────────────────────────────────────────
log "Installing desktop entry and icons..."
mkdir -p "${DESKTOP_DIR}"
mkdir -p "${ICON_DIR}/32x32/apps"
mkdir -p "${ICON_DIR}/128x128/apps"
mkdir -p "${ICON_DIR}/scalable/apps"

# Download icons from the repo (raw)
RAW_BASE="https://raw.githubusercontent.com/${REPO}/master"
curl -fsSL "${RAW_BASE}/dist/onenote-linux.desktop" -o "${DESKTOP_DIR}/onenote-linux.desktop"
curl -fsSL "${RAW_BASE}/icons/32x32.png" -o "${ICON_DIR}/32x32/apps/onenote-linux.png"
curl -fsSL "${RAW_BASE}/icons/128x128.png" -o "${ICON_DIR}/128x128/apps/onenote-linux.png"
curl -fsSL "${RAW_BASE}/icons/icon.svg" -o "${ICON_DIR}/scalable/apps/onenote-linux.svg"

# Fix Exec path in desktop file
sed -i "s|^Exec=.*|Exec=${INSTALL_DIR//\//\\/}/${BINARY_NAME}|" "${DESKTOP_DIR}/onenote-linux.desktop"

# Update desktop database and icon cache
command -v update-desktop-database &>/dev/null && update-desktop-database "${DESKTOP_DIR}" || true
command -v gtk-update-icon-cache &>/dev/null && gtk-update-icon-cache -f "${ICON_DIR}" || true

ok "Desktop entry and icons installed."

# ── PATH check ──────────────────────────────────────────────────────────
if [[ ":${PATH}:" != *":${INSTALL_DIR}:"* ]]; then
    warn "${INSTALL_DIR} is not in your PATH."
    warn "Add this to your shell config (~/.bashrc, ~/.zshrc, etc.):"
    warn "  export PATH=\"\${PATH}:${INSTALL_DIR}\""
fi

# ── done ────────────────────────────────────────────────────────────────
cat <<EOF

════════════════════════════════════════════════════════════════════════
${GREEN}Installation complete!${NC}
════════════════════════════════════════════════════════════════════════

Run OneNote from your application menu, or run:
  ${BINARY_NAME}

First launch will open a setup wizard to discover your organisation's
sign-in server (work/school accounts). Personal accounts work immediately.

Configuration: ~/.config/onenote-linux/config.json
Cache:         ~/.local/share/dev.onenoteweb.linux/

Source code: https://github.com/${REPO}
Issues:      https://github.com/${REPO}/issues

EOF