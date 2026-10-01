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

## Architecture Snapshot

- **Tauri v2** + **WebKitGTK** (GTK 4). Product name `OneNote`, identifier `dev.onenoteweb.linux`.
- Single binary, ~8 MB stripped (`profile.release`: opt-level="s", LTO, strip).
- `withGlobalTauri: false` — remote OneNote content has no Tauri IPC access.
- Main window built from `src/window.rs:build_main()`; all navigation policy in `src/nav.rs`.
- First-run wizard in `src/setup.rs`; Microsoft UserRealm discovery in `src/discovery.rs`.
- Config at `~/.config/onenote-linux/config.json`; window state in `~/.local/share/dev.onenoteweb.linux/window-state.json`.
- In-window navigation bar injected via `src/toolbar.rs` (shadow DOM, Back/Forward/Reload + `Alt+Left/Right`).
- Custom schemes: `onenote-setup://`, `onenote-cfg://`, `onenote-keys://`, `onenote-notify://`.

## Key Files to Know

| File | Purpose |
|------|---------|
| `src/main.rs` | App entry, single-instance plugin, window event loop, tray init |
| `src/window.rs` | Main webview builder, navigation policy hook, popup handling, geometry persistence |
| `src/nav.rs` | In-app allow-list, federation heuristic (`sts`/`login.*`/`adfs.*`/...), host pinning |
| `src/setup.rs` | First-run wizard, Microsoft UserRealm lookup, host validation |
| `src/discovery.rs` | Microsoft UserRealm HTTP call (`/common/userrealm/{domain}?api-version=2.1`) |
| `src/config.rs` | Config struct, RwLock cache, `setup_complete` flag, window state paths |
| `src/toolbar.rs` | Injected shadow-DOM nav bar (Back/Forward/Reload + Alt+Left/Right) |
| `src/keys.rs` | Keyboard shortcuts (`Ctrl+Q` quit, `Ctrl+H` hide) |
| `src/tray.rs` | System tray: show/hide, reload, settings, clear cache, quit |
| `dist/setup.html` | First-run wizard UI |
| `install.sh` / `uninstall.sh` | Arch one-liner (fetches from `master` raw) |

## Installer / Uninstaller

- Both scripts are fetched from `master` raw, not from releases.
- `install.sh`: Arch-only, prompts (use `--yes` to skip), installs deps via pacman, downloads latest release binary.
- `uninstall.sh`: removes binary/desktop/icons, **keeps config/session by default**; `--purge` deletes `~/.config/onenote-linux`, `~/.local/share/dev.onenoteweb.linux`, `~/.local/state/onenote-linux`; `--packages` removes pacman deps.
- Both: `-y/--yes`, `-h/--help`; `--yes` **does not** auto-run destructive steps (requires `--purge` / `--packages`).

## CI / Release

- `.github/workflows/build.yml`: `check` job → `bundle` job (single job builds all targets once).
- Tag `v*` triggers draft release with assets: raw binary `onenote-linux`, `.deb`, `.rpm`, `.AppImage`, `SHA256SUMS`.
- Version sync: tag must match `Cargo.toml` version (`cargo metadata` check).
- Permissions: `contents: write` required for draft release creation.
- Release artifacts uploaded; `softprops/action-gh-release` attaches `SHA256SUMS`.

## Common Pitfalls

- **`ETXTBSY` on reinstall**: `curl -o` to the running binary fails. Installer downloads to temp, verifies ELF magic (`0x7fELF`), then `mv` (atomic rename) into place.
- **`read` in `curl \| bash`**: stdin is the script itself. Installer/uninstaller open `/dev/tty` for confirmation (probe with subshell `exec </dev/tty`).
- **Config cache**: `Config::get()` returns `Option<Config>` from a `RwLock`; `Config::load_cached()` called once at startup. `setup_complete` + file existence determine first-run.
- **First-run**: keyed off config file existence, NOT `setup_complete` alone (legacy configs without the field would otherwise re-trigger wizard).
- **No system certs**: reqwest uses `rustls` + `system-proxy` + native roots (via `rustls-tls-webpki-roots`? actually features: `rustls`, `system-proxy`).
- **Tauri CSP** applies only to local `dist/` assets; remote OneNote uses its own headers.
- **Multi-webview not used**: navigation bar is injected JS to avoid unstable `add_child` migration.

## Data Paths (for debugging)

| Data | Path |
|------|------|
| Config | `~/.config/onenote-linux/config.json` |
| Window state | `~/.local/share/dev.onenoteweb.linux/window-state.json` |
| Webview storage | `~/.local/share/dev.onenoteweb.linux/webview/` |
| Navigation log | `~/.local/state/onenote-linux/navigation.log` |

## Version Bump Checklist

1. Bump `version` in `Cargo.toml`.
2. `cargo build` updates `Cargo.lock`.
3. Update `pkgver` in `PKGBUILD` (sha256 will be regenerated after tag push).
3. Commit, push, tag `vX.Y.Z`, push tag.
4. CI builds draft release → verify assets → publish.

## Testing Notes

- Unit tests in `src/nav.rs` (host recognition, federation heuristic, IdP matching).
- Run `cargo test` (17 tests, fast).
- No integration/UI tests; headless WebKit not available in CI.