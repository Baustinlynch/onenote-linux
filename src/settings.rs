//! A small preferences window.
//!
//! It deliberately does not use Tauri IPC: `withGlobalTauri` is off, so the
//! OneNote page (third-party content) has no access to host commands. Instead
//! the current config is injected at construction time, and saving navigates
//! to an internal scheme the navigation handler consumes.

use tauri::{AppHandle, Manager, WebviewUrl, WebviewWindowBuilder};

use crate::config::{Config, WindowState};
use crate::window;

pub const SCHEME: &str = "onenote-cfg";

const INJECT_CONFIG_JS: &str = r#"
window.__ONENOTE_CONFIG__ = __CONFIG_JSON__;
window.__onenoteSave = function (cfg) {
  var p = new URLSearchParams();
  for (var k in cfg) p.set(k, cfg[k]);
  var a = document.createElement("a");
  a.href = "onenote-cfg://save?" + p.toString();
  document.documentElement.appendChild(a);
  a.click();
  a.remove();
};
"#;

/// Returns true when the URL was ours and navigation should be denied.
pub fn handle(app: &AppHandle, window_label: &str, url: &tauri::Url) -> bool {
    if url.scheme() != SCHEME {
        return false;
    }

    if window_label == window::SETTINGS_LABEL && url.host_str() == Some("save") {
        let params: std::collections::HashMap<String, String> = url
            .query_pairs()
            .map(|(k, v)| (k.into_owned(), v.into_owned()))
            .collect();

        let mut cfg = Config::load(app);
        cfg.close_to_tray = flag(&params, "close_to_tray");
        cfg.start_minimized = flag(&params, "start_minimized");
        cfg.notifications = flag(&params, "notifications");
        cfg.allow_federated_hosts = flag(&params, "allow_federated_hosts");
        cfg.zoom = params
            .get("zoom")
            .and_then(|z| z.parse::<f64>().ok())
            .unwrap_or(1.0)
            .clamp(0.25, 5.0);
        cfg.allowed_hosts = params
            .get("allowed_hosts")
            .map(|raw| {
                raw.split(',')
                    .map(|h| h.trim().to_ascii_lowercase())
                    .filter(|h| !h.is_empty())
                    .collect()
            })
            .unwrap_or_default();
        cfg.save(app);

        // The window is hidden but still running: state changes apply now.
        persist_main_window(app);

        if let Some(win) = app.get_webview_window(window::SETTINGS_LABEL) {
            let _ = win.eval(
                "document.getElementById('status').textContent = \
                 'Saved. Applied immediately.';",
            );
        }
    }

    true
}

fn flag(params: &std::collections::HashMap<String, String>, key: &str) -> bool {
    params.get(key).map(String::as_str) == Some("1")
}

/// Capture the main window geometry so the next run restores it.
fn persist_main_window(app: &AppHandle) {
    let Some(win) = app.get_webview_window(window::MAIN_LABEL) else {
        return;
    };
    let scale = win.scale_factor().unwrap_or(1.0);
    let size = win.inner_size().unwrap_or_default();
    let position = win.outer_position().ok();

    Config::save_window_state(
        app,
        &WindowState {
            width: size.width as f64 / scale,
            height: size.height as f64 / scale,
            x: position.map(|p| p.x),
            y: position.map(|p| p.y),
        },
    );
}

pub fn show_settings_window(app: &AppHandle) {
    if let Some(win) = app.get_webview_window(window::SETTINGS_LABEL) {
        let _ = win.show();
        let _ = win.set_focus();
        return;
    }

    let cfg = Config::load(app);
    let json = serde_json::to_string(&cfg).unwrap_or_else(|_| "{}".into());
    let script = INJECT_CONFIG_JS.replace("__CONFIG_JSON__", &json);
    let owner = app.clone();

    let _ = WebviewWindowBuilder::new(
        app,
        window::SETTINGS_LABEL,
        WebviewUrl::App("settings.html".into()),
    )
    .title("OneNote Settings")
    .inner_size(540.0, 580.0)
    .min_inner_size(420.0, 420.0)
    .resizable(true)
    .initialization_script(script)
    .on_navigation(move |url| !handle(&owner, window::SETTINGS_LABEL, url))
    .build();
}
