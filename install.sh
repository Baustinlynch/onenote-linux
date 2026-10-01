#!/usr/bin/env bash
# ═══════════════════════════════════════════════════════════════════════
# onenote-linux — One-liner installer
# ═══════════════════════════════════════════════════════════════════════
# ⚠️  WARNING: This project is VIBE-CODED.
#    It was written with AI assistance and has NOT been thoroughly audited.
#    Use at your own risk. Review the code before running on sensitive systems.
# ═══════════════════════════════════════════════════════════════════════
# Installs system dependencies (Arch/Arch-based) and the latest release binary
# from GitHub.
#
#   curl -fsSL <url> | bash                 # prompts before changing anything
#   curl -fsSL <url> | bash -s -- --yes     # non-interactive
#   bash install.sh --yes
# ═══════════════════════════════════════════════════════════════════════

set -euo pipefail

REPO="Baustinlynch/onenote-linux"
BINARY_NAME="onenote-linux"
INSTALL_DIR="${HOME}/.local/bin"
DESKTOP_DIR="${HOME}/.local/share/applications"
ICON_DIR="${HOME}/.local/share/icons/hicolor"

ASSUME_YES=0
STEP=0
TOTAL_STEPS=6

# ── colours (only when attached to a terminal) ───────────────────────────
if [[ -t 1 ]]; then
    RED=$'\033[0;31m'
    GREEN=$'\033[0;32m'
    YELLOW=$'\033[1;33m'
    BLUE=$'\033[0;34m'
    CYAN=$'\033[0;36m'
    DIM=$'\033[2m'
    BOLD=$'\033[1m'
    NC=$'\033[0m'
else
    RED='' GREEN='' YELLOW='' BLUE='' CYAN='' DIM='' BOLD='' NC=''
fi

info()  { printf '%s[%s]%s %s\n' "$BLUE" "$(date '+%H:%M:%S')" "$NC" "$*"; }
warn()  { printf '%s[WARN]%s %s\n' "$YELLOW" "$NC" "$*" >&2; }
err()   { printf '%s[ERR]%s %s\n' "$RED" "$NC" "$*" >&2; }
ok()    { printf '%s[OK]%s %s\n' "$GREEN" "$NC" "$*"; }
step()  { STEP=$((STEP + 1)); printf '\n%s%s[%d/%d]%s %s\n' "$BOLD" "$CYAN" "$STEP" "$TOTAL_STEPS" "$NC" "$*"; }

# Echo a command before running it, so a failure is always traceable.
run() {
    printf '%s  $ %s%s\n' "$DIM" "$*" "$NC"
    "$@"
}

usage() {
    cat <<EOF
onenote-linux installer

Usage:
  curl -fsSL <url> | bash               Prompt before installing
  curl -fsSL <url> | bash -s -- --yes   Install without prompting
  bash install.sh [--yes]

Options:
  -y, --yes   Do not ask for confirmation
  -h, --help  Show this help

Installs to: ${INSTALL_DIR}
EOF
}

for arg in "$@"; do
    case "$arg" in
        -y | --yes) ASSUME_YES=1 ;;
        -h | --help) usage; exit 0 ;;
        *) err "Unknown option: $arg"; usage; exit 2 ;;
    esac
done

# ── confirmation ─────────────────────────────────────────────────────────
# Must read from /dev/tty, not stdin: with `curl | bash` stdin is the script
# itself, and reading it swallows the next lines of the script. Open the tty
# explicitly, because it can exist yet be unopenable with no controlling
# terminal.
confirm() {
    [[ "$ASSUME_YES" == "1" ]] && return 0
    # Probe in a subshell so a failed open does not print a shell error or
    # leave the caller's descriptors changed.
    if ! (exec </dev/tty) 2>/dev/null; then
        err "No terminal available to confirm."
        err "Re-run with --yes, for example:  curl -fsSL <url> | bash -s -- --yes"
        exit 1
    fi
    printf '%s [y/N] ' "$1"
    local reply=""
    IFS= read -r -n 1 reply </dev/tty || true
    printf '\n'
    if [[ ! "$reply" =~ ^[Yy]$ ]]; then
        err "Aborted by user."
        exit 1
    fi
}

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

info "Installer starting (repository: ${REPO})"
info "Install target:     ${INSTALL_DIR}/${BINARY_NAME}"
info "Desktop entry:      ${DESKTOP_DIR}/onenote-linux.desktop"

confirm $'\nContinue installation?'

# ── detect distro ───────────────────────────────────────────────────────
step "Checking the system"
if ! command -v pacman &>/dev/null; then
    err "This installer only supports Arch-based distros (it uses pacman)."
    err "For other distros, install the dependencies manually and download from:"
    err "  https://github.com/${REPO}/releases"
    exit 1
fi
if ! command -v curl &>/dev/null; then
    err "curl is required but was not found."
    exit 1
fi
ok "Detected an Arch-based system."

# ── install deps ────────────────────────────────────────────────────────
step "Installing system dependencies via pacman"
info "webkit2gtk-4.1, gtk4, libayatana-appindicator, libnotify, xdg-utils"
run sudo pacman -S --needed --noconfirm \
    webkit2gtk-4.1 \
    gtk4 \
    libayatana-appindicator \
    libnotify \
    xdg-utils
ok "System dependencies are installed."

# ── fetch latest release ────────────────────────────────────────────────
step "Finding the latest release"
LATEST_URL="https://api.github.com/repos/${REPO}/releases/latest"
info "Querying ${LATEST_URL}"
RELEASE_JSON=$(curl -fsSL "${LATEST_URL}")
RELEASE_TAG=$(printf '%s' "${RELEASE_JSON}" |
    grep -o '"tag_name": *"[^"]*"' | head -1 | cut -d'"' -f4 || true)
[[ -n "${RELEASE_TAG}" ]] && info "Latest release: ${RELEASE_TAG}"

# Match the asset whose file name is exactly the binary. A looser match also
# hit every bundle, because the repository path itself contains the name.
ASSET_URL=$(printf '%s' "${RELEASE_JSON}" |
    grep -o '"browser_download_url": *"[^"]*/'"${BINARY_NAME}"'"' |
    head -1 |
    cut -d'"' -f4 || true)

if [[ -z "${ASSET_URL}" ]]; then
    err "Could not find ${BINARY_NAME} in the latest release assets."
    err "Check https://github.com/${REPO}/releases"
    exit 1
fi
info "Asset URL: ${ASSET_URL}"

# ── download binary ─────────────────────────────────────────────────────
step "Downloading the application binary"
run mkdir -p "${INSTALL_DIR}"
info "Saving to ${INSTALL_DIR}/${BINARY_NAME}"

# Download beside the target and rename into place. Writing the destination
# directly fails with "Text file busy" while the app is running; a rename
# swaps the directory entry, so the running process keeps the old inode.
TMP_BIN="${INSTALL_DIR}/.${BINARY_NAME}.tmp.$$"
trap 'rm -f "${TMP_BIN}"' EXIT

run curl -fL --progress-bar "${ASSET_URL}" -o "${TMP_BIN}"

# A 200 response can still be an HTML error page, and clobbering a working
# binary with that would be worse than failing here.
if [[ "$(head -c 4 "${TMP_BIN}" | od -An -tx1 | tr -d ' \n')" != "7f454c46" ]]; then
    err "Downloaded file is not an ELF binary; refusing to install it."
    err "This usually means the download was intercepted or corrupted."
    exit 1
fi

run chmod +x "${TMP_BIN}"
run mv -f "${TMP_BIN}" "${INSTALL_DIR}/${BINARY_NAME}"
trap - EXIT
ok "Installed ${INSTALL_DIR}/${BINARY_NAME}"

# ── desktop entry & icons ───────────────────────────────────────────────
step "Installing desktop entry and icons"
run mkdir -p "${DESKTOP_DIR}"
run mkdir -p "${ICON_DIR}/32x32/apps"
run mkdir -p "${ICON_DIR}/128x128/apps"
run mkdir -p "${ICON_DIR}/scalable/apps"

# Download icons from the repo (raw)
RAW_BASE="https://raw.githubusercontent.com/${REPO}/master"
info "Fetching ${RAW_BASE}/dist/onenote-linux.desktop"
run curl -fL --progress-bar "${RAW_BASE}/dist/onenote-linux.desktop" \
    -o "${DESKTOP_DIR}/onenote-linux.desktop"
info "Fetching icons"
run curl -fL --progress-bar "${RAW_BASE}/icons/32x32.png" \
    -o "${ICON_DIR}/32x32/apps/onenote-linux.png"
run curl -fL --progress-bar "${RAW_BASE}/icons/128x128.png" \
    -o "${ICON_DIR}/128x128/apps/onenote-linux.png"
run curl -fL --progress-bar "${RAW_BASE}/icons/icon.svg" \
    -o "${ICON_DIR}/scalable/apps/onenote-linux.svg"

# Fix Exec path in desktop file
info "Pointing the desktop entry at ${INSTALL_DIR}/${BINARY_NAME}"
sed -i "s|^Exec=.*|Exec=${INSTALL_DIR//\//\\/}/${BINARY_NAME}|" "${DESKTOP_DIR}/onenote-linux.desktop"

# Update desktop database and icon cache. These are cosmetic caches and can
# fail (e.g. a user-local hicolor dir has no index.theme), so never let them
# abort an otherwise successful install.
if command -v update-desktop-database &>/dev/null; then
    run update-desktop-database "${DESKTOP_DIR}" ||
        warn "update-desktop-database failed (non-fatal)."
fi
if command -v gtk-update-icon-cache &>/dev/null; then
    run gtk-update-icon-cache -f "${ICON_DIR}" ||
        warn "gtk-update-icon-cache failed (non-fatal)."
fi
ok "Desktop entry and icons installed."

# ── PATH check ──────────────────────────────────────────────────────────
step "Checking your PATH"
if [[ ":${PATH}:" != *":${INSTALL_DIR}:"* ]]; then
    warn "${INSTALL_DIR} is not in your PATH."
    warn "Add this to your shell config (~/.bashrc, ~/.zshrc, etc.):"
    warn "  export PATH=\"\${PATH}:${INSTALL_DIR}\""
else
    ok "${INSTALL_DIR} is already in your PATH."
fi

# ── done ────────────────────────────────────────────────────────────────
cat <<EOF

═══════════════════════════════════════════════════════════════════════
${GREEN}Installation complete!${NC}
═══════════════════════════════════════════════════════════════════════

Run OneNote from your application menu, or run:
  ${BINARY_NAME}

If OneNote is already running, close and reopen it to use the new version.

First launch will open a setup wizard to discover your organisation's
sign-in server (work/school accounts). Personal accounts work immediately.

Configuration: ~/.config/onenote-linux/config.json
Cache:         ~/.local/share/dev.onenoteweb.linux/

Source code: https://github.com/${REPO}
Issues:      https://github.com/${REPO}/issues

EOF
