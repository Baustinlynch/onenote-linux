# AGENTS.md — onenote-linux

## Quick Commands

| Task | Command |
|------|---------|
| Run dev | `cargo run` |
| Release bundles (AppImage, deb, rpm) | `cargo tauri build` |
| Test | `cargo test` |
| Lint (clippy) | `cargo clippy --all-targets -- -D warnings` |
| Format check | `cargo fmt --check` |
| CI pipeline order | `fmt → clippy → test → bundle` |

## Architecture Notes

- Tauri v2 + WebKitGTK (GTK 4). Product name `OneNote`, identifier `dev.onenoteweb.linux`.
- Single binary, ~8 MB stripped (`profile.release`: opt-level="s", LTO, strip).
- `withGlobalTauri: false` — remote OneNote content has no Tauri IPC access.
- Main window built from `src/window.rs:build_main()`; all navigation policy in `src/nav.rs`.
- First-run wizard in `src/setup.rs`; Microsoft UserRealm discovery in `src/discovery.rs`.
- Config at `~/.config/onenote-linux/config.json`; window state in `~/.local/share/dev.onenoteweb.linux/window-state.json`.
- In-window navigation bar injected via `src/toolbar.rs` (shadow DOM, Back/Forward/Reload + `Alt+Left/Right`).
- Custom schemes: `onenote-setup://`, `onenote-cfg://`, `onenote-keys://`, `onenote-notify://`.

## Build / CI Flow

- CI: `fmt → clippy → test → bundle` (single job builds all targets once)
- Releases: tag `v*` triggers draft release with assets (raw binary, `.deb`, `.rpm`, `.AppImage`, `SHA256SUMS`)
- Version sync: tag must match `Cargo.toml` version (`cargo metadata` check)

## Development Notes

- **Dependencies**: `webkit2gtk-4.1-dev`, `libayatana-appindicator3-dev`, `libnotify`, `xdg-utils`
- **Nix**: `nix develop github:Baustinlynch/onenote-linux`
- **Tests**: 17 unit tests in `src/nav.rs`; no UI tests (headless WebKit not available)
- **Entry points**: `src/main.rs` runs the app; `src/window.rs` builds the main webview; `src/nav.rs` implements host allow-list and federation heuristic

## First-Run Wizard

- Shown once on startup if config file doesn't exist
- Queries Microsoft's UserRealm endpoint (`/common/userrealm/{domain}?api-version=2.1`)
- Pins discovered IdP hosts; turns off `allow_federated_hosts` heuristic
- Uses custom scheme `onenote-setup://` for page communication

## Navigation Security

- Only Microsoft-owned domains and federated IdP hosts load in-app
- Federation heuristic: hosts starting with `login.*`, `adfs.*`, `sso.*`, `idp.*`, `auth.*`, `owa.*`, `sts.*` are trusted if HTTPS, multi-label, and no Microsoft impersonation tokens
- External links go to system browser; unsafe schemes (`javascript:`, `file:`, etc.) are blocked

## Data Paths

- Config: `~/.config/onenote-linux/config.json`
- Window state: `~/.local/share/dev.onenoteweb.linux/window-state.json`
- Webview storage: `~/.local/share/dev.onenoteweb.linux/webview/`
- Navigation log: `~/.local/state/onenote-linux/navigation.log`

## Version Bumping

1. Bump `version` in `Cargo.toml`
2. `cargo build` updates `Cargo.lock`
3. Update `pkgver` in `PKGBUILD`
4. Commit, push, tag `vX.Y.Z`

## Installer Notes

- Arch-only scripts (`install.sh`/`uninstall.sh`) fetched from `master` raw
- Scripts open `/dev/tty` for confirmation (check with `exec </dev/tty`)
- `ETXTBSY` on reinstall: downloads to temp, verifies ELF magic (`0x7fELF`), then `mv` into place