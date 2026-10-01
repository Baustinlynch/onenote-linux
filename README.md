# onenote-linux

> ⚠️ **VIBE-CODED WARNING**
>
> This project was written with AI assistance ("vibe-coded") and has **NOT**
> been thoroughly audited for security, correctness, or completeness.
> **Use at your own risk.** Review the source code before running on sensitive
> systems. No warranty, express or implied. Not affiliated with Microsoft
> Corporation.
>
> Source: https://github.com/Baustinlynch/onenote-linux

# onenote-linux

A thin, native desktop window around the Microsoft OneNote web client for
Linux. Built with [Tauri v2](https://v2.tauri.app) and WebKitGTK.

**This is not a Microsoft product and is not affiliated with Microsoft.** It is
a wrapper: it loads `onenote.com` in a real webview, so you get the real
OneNote with the real sync, including ink, handwriting, and notebooks hosted on
SharePoint or OneDrive for Business.

## Why

Microsoft has no Linux client and states there are no plans for one. Your
options are a browser tab among many, or Electron wrappers that ship a whole
second browser. This keeps a dedicated window, a tray icon, and native
notifications while using the webview already on your system.

## Features

- Dedicated window, no browser tab clutter
- In-window navigation bar (Back, Forward, Reload) plus `Alt+Left` /
  `Alt+Right`; notebook links open in the same window rather than a blank one
- System tray with show/hide, reload, settings, and quit
- Close to tray; optional start minimized
- Single instance: a second launch focuses the existing window
- `Ctrl+Q` quits, `Ctrl+H` hides to the tray
- Session persists across restarts, so you sign in once
- Desktop notifications for page and notebook activity
- External links open in your default browser
- Window size and position remembered
- ~10 MB, versus 200 MB+ for Electron-based wrappers
- **First-run wizard** asks for your work/school domain, queries Microsoft
  directly to find your identity provider, and pins it so single sign-on
  completes inside the app

## Quick install (Arch / Arch-based)

```sh
curl -fsSL https://raw.githubusercontent.com/Baustinlynch/onenote-linux/master/install.sh | bash
```

The script installs system dependencies via `pacman`, downloads the latest
release binary, and sets up the desktop entry + icons. You will be prompted
before anything runs.

## Requirements

- WebKitGTK 4.1, GTK 4
- `libayatana-appindicator` for the tray icon
- `libnotify` (`notify-send`) for notifications
- `xdg-utils` (for opening external links)

On Arch:

```sh
sudo pacman -S --needed webkit2gtk-4.1 gtk4 libayatana-appindicator libnotify xdg-utils
```

## Build and run

```sh
cargo run
```

Release bundles (AppImage, deb, rpm):

```sh
cargo tauri build
```

Run the tests:

```sh
cargo test
```

## First run — work and school accounts

On the very first launch you are asked for your organisation domain or email
(e.g. `ea.edin.sch.uk` or `you@ea.edin.sch.uk`). The app queries Microsoft's
UserRealm endpoint to get the authoritative identity-provider URL, shows the
tenant name for confirmation ("The City of Edinburgh Council"), and pins that
host. The broad federation heuristic is then turned off automatically so only
Microsoft-owned domains and your exact IdP can load in-app.

If the lookup cannot determine a host, you can enter it manually — copy the
hostname from the browser address bar the next time OneNote kicks you out (e.g.
`sts.your-school.ac.uk`). The wizard is skippable and the same settings are
reachable later from the tray menu.

## Signing in

Start the app and sign in at the Microsoft sign-in page. Personal
(`outlook.com`, `hotmail.com`) and work or school accounts both work, as long
as the sign-in flow stays inside the app window.

**Federated single sign-on.** If your organisation uses ADFS, SAML, or another
identity provider, Microsoft redirects to a host that cannot be predicted in
advance. The app recognises sign-in hosts by shape (`login.*`, `adfs.*`,
`sso.*`, `idp.*`, `auth.*`, `owa.*`, `autodiscover.*`, `sts.*`) and keeps them
in-app so the flow can complete. If your IdP does not match that shape, add it
under **Settings → Extra allowed hosts**, for example `login.contoso.edu` or
`*.contoso.edu`.

## Settings

Reachable from the tray menu, or `~/.config/onenote-linux/config.json`:

```json
{
  "close_to_tray": true,
  "start_minimized": false,
  "zoom": 1.0,
  "start_url": "https://www.onenote.com/",
  "allowed_hosts": [],
  "allow_federated_hosts": true,
  "notifications": true,
  "setup_complete": true
}
```

| Option | Default | Meaning |
| --- | --- | --- |
| `close_to_tray` | `true` | Close button hides to the tray instead of quitting |
| `start_minimized` | `false` | Launch hidden in the tray |
| `zoom` | `1.0` | Page zoom, 0.25 to 5.0 |
| `start_url` | `https://www.onenote.com/` | Landing page |
| `allowed_hosts` | `[]` | Extra hosts kept in-app; supports `*.example.com` |
| `allow_federated_hosts` | `true` | Recognise organisation IdP hosts by name shape |
| `notifications` | `true` | Show desktop notifications |
| `setup_complete` | `false` | Internal: first-run wizard completed |

### A note on the federation heuristic

`allow_federated_hosts` cannot be fully safe. A hostile host such as
`sso.phishing.example` is shaped identically to a legitimate `sso.contoso.edu`,
and no amount of pattern matching separates them. The heuristic is narrowed as
far as it can reasonably go: HTTPS only, at least three DNS labels, a known
marker as the first label, and rejection of any name carrying a Microsoft token
(which is what stops `login.microsoft.com.attacker.net`).

If your organisation's IdP host is known, turn the heuristic off and list that
host explicitly. Then only Microsoft-owned domains and hosts you named can load
in-app; everything else goes to your browser. The first-run wizard does this
automatically for you.

## Security posture

- The webview runs **without** `withGlobalTauri`, so the OneNote page has no
  access to Tauri commands or the host filesystem.
- Only Microsoft-owned and explicitly allowed hosts load in-app. Everything
  else is handed to the system browser.
- `javascript:`, `file:`, and other unexpected schemes are refused.
- User-agent is not spoofed, so the site serves its normal application.
- The notification and keyboard bridges use internal `onenote-*://` schemes
  that the navigation handler consumes and never loads.

## Layout

```
src/
  main.rs       app wiring, window event handling
  config.rs     persisted config and window state
  nav.rs        host allow-list, federation heuristic, tests
  window.rs     main window construction and navigation policy
  notify.rs     Notification API bridge to the desktop daemon
  keys.rs       Ctrl+Q / Ctrl+H handling
  settings.rs   preferences window
  tray.rs       system tray icon and menu
  setup.rs      first-run wizard window
  discovery.rs  Microsoft UserRealm lookup and host validation
dist/           bundled pages (settings dialog, setup wizard)
scripts/        icon generation
```

## Licence

MIT. See [LICENSE](LICENSE).
