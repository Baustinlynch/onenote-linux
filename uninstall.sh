#!/usr/bin/env bash
# ═══════════════════════════════════════════════════════════════════════
# onenote-linux — Uninstaller
# ═══════════════════════════════════════════════════════════════════════
# Removes the files install.sh created. Settings, the signed-in session and
# the system packages are left alone unless you ask for them, because losing
# the session means signing in again and some of those packages may be used
# by other applications.
# ═══════════════════════════════════════════════════════════════════════

set -euo pipefail

BINARY_NAME="onenote-linux"

INSTALL_DIR="${HOME}/.local/bin"
BINARY="${INSTALL_DIR}/${BINARY_NAME}"
DESKTOP_DIR="${HOME}/.local/share/applications"
DESKTOP_FILE="${DESKTOP_DIR}/${BINARY_NAME}.desktop"
ICON_DIR="${HOME}/.local/share/icons/hicolor"
CONFIG_DIR="${HOME}/.config/onenote-linux"
DATA_DIR="${HOME}/.local/share/dev.onenoteweb.linux"
STATE_DIR="${HOME}/.local/state/onenote-linux"

PACKAGES=(webkit2gtk-4.1 gtk4 libayatana-appindicator libnotify xdg-utils)

ASSUME_YES=0
PURGE=0
REMOVE_PACKAGES=0
STEP=0
TOTAL_STEPS=4

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

info() { printf '%s[%s]%s %s\n' "$BLUE" "$(date '+%H:%M:%S')" "$NC" "$*"; }
warn() { printf '%s[WARN]%s %s\n' "$YELLOW" "$NC" "$*" >&2; }
err()  { printf '%s[ERR]%s %s\n' "$RED" "$NC" "$*" >&2; }
ok()   { printf '%s[OK]%s %s\n' "$GREEN" "$NC" "$*"; }
step() { STEP=$((STEP + 1)); printf '\n%s%s[%d/%d]%s %s\n' "$BOLD" "$CYAN" "$STEP" "$TOTAL_STEPS" "$NC" "$*"; }

run() {
    printf '%s  $ %s%s\n' "$DIM" "$*" "$NC"
    "$@"
}

usage() {
    cat <<EOF
onenote-linux uninstaller

Usage:
  bash uninstall.sh [options]
  curl -fsSL <url>/uninstall.sh | bash -s -- --yes

Options:
  -y, --yes       Do not ask for confirmation. Optional destructive steps
                  (purging data, removing packages) are still skipped unless
                  the matching flag is given.
  -p, --purge     Also delete settings and the signed-in session.
  -k, --packages  Also remove the system packages installed by install.sh.
  -h, --help      Show this help.

Without --purge, these are kept so a reinstall stays signed in:
  ${CONFIG_DIR}
  ${DATA_DIR}
EOF
}

for arg in "$@"; do
    case "$arg" in
        -y | --yes) ASSUME_YES=1 ;;
        -p | --purge) PURGE=1 ;;
        -k | --packages) REMOVE_PACKAGES=1 ;;
        -h | --help) usage; exit 0 ;;
        *) err "Unknown option: $arg"; usage; exit 2 ;;
    esac
done

# ── confirmation ─────────────────────────────────────────────────────────
# Read from /dev/tty: with `curl | bash` stdin is the script itself.
confirm() {
    [[ "$ASSUME_YES" == "1" ]] && return 0
    if ! (exec </dev/tty) 2>/dev/null; then
        err "No terminal available to confirm."
        err "Re-run with --yes, for example:  curl -fsSL <url>/uninstall.sh | bash -s -- --yes"
        exit 1
    fi
    printf '%s [y/N] ' "$1"
    local reply=""
    IFS= read -r -n 1 reply </dev/tty || true
    printf '\n'
    [[ "$reply" =~ ^[Yy]$ ]]
}

# True when an optional step should run: requested by flag, or confirmed.
# With --yes alone, optional steps are skipped rather than assumed.
maybe_do() {
    local wanted="$1" question="$2"
    [[ "$wanted" == "1" ]] && return 0
    [[ "$ASSUME_YES" == "1" ]] && return 1
    confirm "$question"
}

# Never let a stray empty or over-broad path turn into `rm -rf` of $HOME.
remove_path() {
    local path="$1"
    if [[ -z "$path" || "$path" == "/" || "$path" == "$HOME" ]]; then
        err "Refusing to remove suspicious path: '${path}'"
        return 1
    fi
    if [[ -e "$path" ]]; then
        rm -rf -- "$path"
        info "removed ${path}"
    else
        info "not present: ${path}"
    fi
}

info "Uninstaller starting"
info "Binary:   ${BINARY}"
info "Config:   ${CONFIG_DIR}"

# ── 1. stop the app ──────────────────────────────────────────────────────
step "Checking whether OneNote is running"
if pgrep -x "${BINARY_NAME}" &>/dev/null; then
    warn "${BINARY_NAME} is running."
    if [[ "$ASSUME_YES" == "1" ]] || confirm "Stop it now?"; then
        run pkill -x "${BINARY_NAME}" || true
        # Give it a moment to exit so it cannot rewrite config on the way out.
        for _ in 1 2 3 4 5; do
            pgrep -x "${BINARY_NAME}" &>/dev/null || break
            sleep 0.4
        done
        if pgrep -x "${BINARY_NAME}" &>/dev/null; then
            warn "Process is still running; continuing anyway."
        else
            ok "Stopped."
        fi
    else
        warn "Leaving it running. Files are removed regardless."
    fi
else
    ok "${BINARY_NAME} is not running."
fi

# ── 2. remove the app ────────────────────────────────────────────────────
step "Removing the application"
remove_path "${BINARY}"
    remove_path "${DESKTOP_FILE}"
    for size in 32x32 128x128; do
        remove_path "${ICON_DIR}/${size}/apps/${BINARY_NAME}.png"
    done
    remove_path "${ICON_DIR}/scalable/apps/${BINARY_NAME}.svg"

# Refresh the cosmetic caches so the entry stops showing up in menus.
if command -v update-desktop-database &>/dev/null; then
    run update-desktop-database "${DESKTOP_DIR}" ||
        warn "update-desktop-database failed (non-fatal)."
fi
if command -v gtk-update-icon-cache &>/dev/null; then
    run gtk-update-icon-cache -f "${ICON_DIR}" ||
        warn "gtk-update-icon-cache failed (non-fatal)."
fi
ok "Application files removed."

# ── 3. settings and session ──────────────────────────────────────────────
step "Removing settings and session data"
if maybe_do "$PURGE" "Also delete settings and sign you out?"; then
    remove_path "${CONFIG_DIR}"
    remove_path "${DATA_DIR}"
    remove_path "${STATE_DIR}"
    ok "Settings and session data removed."
else
    info "kept: ${CONFIG_DIR}"
    info "kept: ${DATA_DIR}"
    info "Reinstalling will reuse them, so you stay signed in."
fi

# ── 4. system packages ───────────────────────────────────────────────────
step "Removing system packages"
if [[ "$REMOVE_PACKAGES" == "1" ]] || maybe_do 0 "Remove the system packages too?"; then
    if command -v pacman &>/dev/null; then
        warn "Only removing them if nothing else on the system needs them."
        run sudo pacman -R --noconfirm "${PACKAGES[@]}" || true
        ok "Package removal attempted."
    else
        warn "pacman not found; skipping package removal."
    fi
else
    info "kept: ${PACKAGES[*]}"
fi

# ── done ────────────────────────────────────────────────────────────────
cat <<EOF

═══════════════════════════════════════════════════════════════════════
${GREEN}Uninstall complete!${NC}
═══════════════════════════════════════════════════════════════════════

If ${INSTALL_DIR:-$HOME/.local/bin} is now empty you can remove it, and you
can drop it from your PATH in ~/.bashrc or ~/.zshrc.

To reinstall:
  curl -fsSL https://raw.githubusercontent.com/Baustinlynch/onenote-linux/master/install.sh | bash

EOF
