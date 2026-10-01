use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};

use tauri::{
    AppHandle, Manager, PhysicalPosition, WebviewUrl, WebviewWindow, WebviewWindowBuilder,
};

use crate::config::{Config, WindowState, START_URL};
use crate::keys;
use crate::nav;
use crate::notify;
use crate::settings;
use crate::toolbar;

pub const MAIN_LABEL: &str = "main";
pub const SETTINGS_LABEL: &str = "settings";

/// Set once the first page has finished loading. Geometry events are ignored
/// until then: the toolkit emits Moved/Resized while constructing the window,
/// before it is realised, and reading scale/position at that point logs a GTK
/// critical.
static WINDOW_READY: AtomicBool = AtomicBool::new(false);

/// Milliseconds since the Unix epoch of the last geometry write. Used to
/// throttle the file writes that a window drag would otherwise cause.
static LAST_PERSIST_MS: AtomicU64 = AtomicU64::new(0);

/// Minimum gap between geometry writes while the user is dragging.
const PERSIST_INTERVAL_MS: u64 = 1000;

pub fn build_main(app: &AppHandle, cfg: &Config) -> tauri::Result<WebviewWindow> {
    let state = Config::load_window_state(app);
    let url = nav::parse(&cfg.start_url)
        .unwrap_or_else(|| tauri::Url::parse(START_URL).expect("valid start url"));

    // Persistent webview storage. This must outlive a /tmp cleanup, so the
    // fallback is our own data directory rather than the temporary directory.
    let data_dir = app
        .path()
        .app_data_dir()
        .unwrap_or_else(|_| Config::data_dir())
        .join("webview");

    // The callbacks outlive this function, so they capture owned handles.
    let nav_handle = app.clone();
    let popup_handle = app.clone();
    let zoom = cfg.zoom;
    let start_hidden = cfg.start_minimized;

    let mut builder = WebviewWindowBuilder::new(app, MAIN_LABEL, WebviewUrl::External(url))
        .title("OneNote")
        .inner_size(state.width, state.height)
        .min_inner_size(640.0, 480.0)
        .resizable(true)
        .visible(!start_hidden)
        .data_directory(data_dir)
        .incognito(false)
        .enable_clipboard_access()
        .zoom_hotkeys_enabled(true)
        .accept_first_mouse(true)
        .initialization_script(format!(
            "{}\n{}\n{}",
            keys::BRIDGE_JS,
            notify::BRIDGE_JS,
            toolbar::SCRIPT
        ))
        .on_navigation(move |url| on_navigation(&nav_handle, url))
        .on_new_window(move |url, _features| on_new_window(&popup_handle, url))
        .on_page_load(move |window, payload| {
            if payload.event() == tauri::webview::PageLoadEvent::Finished {
                WINDOW_READY.store(true, Ordering::Relaxed);
                if (zoom - 1.0).abs() > f64::EPSILON {
                    let _ = window.set_zoom(zoom);
                }
            }
        });

    if let (Some(x), Some(y)) = (state.x, state.y) {
        builder = builder.position(x as f64, y as f64);
    }

    builder.build()
}

/// Discard all webview data: cookies, cache and local storage. This signs the
/// user out, which is why the menu item says so. Used to recover from a broken
/// OneNote session.
pub fn clear_cache(app: &AppHandle) {
    let Some(win) = app.get_webview_window(MAIN_LABEL) else {
        return;
    };
    if let Err(err) = win.clear_all_browsing_data() {
        eprintln!("failed to clear browsing data: {err}");
        return;
    }
    let _ = win.reload();
    show_main(app);
}

/// Same-window navigation. Anything outside the allow-list is handed to the
/// system browser rather than loaded.
fn on_navigation(app: &AppHandle, url: &tauri::Url) -> bool {
    // Our internal schemes: consume them, never navigate.
    if keys::handle(app, url)
        || notify::handle(app, url)
        || settings::handle(app, MAIN_LABEL, url)
        || crate::setup::handle(app, url)
    {
        return false;
    }
    if !nav::is_safe_scheme(url) {
        trace_navigation("blocked-unsafe-scheme", url);
        return false;
    }
    let cfg = Config::get_or_default();
    if cfg.is_in_app(url) {
        true
    } else {
        trace_navigation("handed-to-browser", url);
        open_externally(app, url.as_str());
        false
    }
}

/// `target=_blank` links and `window.open` calls.
///
/// Nothing is ever opened in a second window: a new webview would start
/// without this window's session, storage and injected bridges, which is why
/// opening a notebook used to produce a blank window. In-app destinations are
/// loaded in the window we already have; everything else goes to the browser.
fn on_new_window(
    app: &AppHandle,
    url: tauri::Url,
) -> tauri::webview::NewWindowResponse<tauri::Wry> {
    use tauri::webview::NewWindowResponse;

    if keys::handle(app, &url)
        || notify::handle(app, &url)
        || settings::handle(app, MAIN_LABEL, &url)
    {
        return NewWindowResponse::Deny;
    }
    if !nav::is_safe_scheme(&url) {
        trace_navigation("popup-blocked-unsafe-scheme", &url);
        return NewWindowResponse::Deny;
    }

    let cfg = Config::get_or_default();
    if !cfg.is_in_app(&url) {
        trace_navigation("popup-handed-to-browser", &url);
        open_externally(app, url.as_str());
        return NewWindowResponse::Deny;
    }

    // In-app popup: reuse the main window rather than spawning a blank one.
    // Navigate off-thread: this callback runs on the event loop, and driving
    // a navigation synchronously from it can deadlock.
    if app.get_webview_window(MAIN_LABEL).is_some() {
        trace_navigation("popup-loaded-in-main", &url);
        let app = app.clone();
        tauri::async_runtime::spawn(async move {
            if let Some(win) = app.get_webview_window(MAIN_LABEL) {
                let _ = win.navigate(url);
            }
        });
    } else {
        trace_navigation("popup-no-main-window", &url);
    }
    NewWindowResponse::Deny
}

/// Record every navigation the policy rejects, so a misrouted single sign-on
/// host can be identified from the log rather than guessed at.
fn trace_navigation(reason: &str, url: &tauri::Url) {
    eprintln!("nav[{reason}] {url}");
    let path = trace_log_path();
    if let Ok(mut file) = std::fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(path)
    {
        use std::io::Write;
        let _ = writeln!(file, "{} {reason} {url}", now_secs());
    }
}

fn trace_log_path() -> std::path::PathBuf {
    let home = std::env::var("HOME").unwrap_or_else(|_| "/tmp".into());
    let dir = std::path::Path::new(&home).join(".local/state/onenote-linux");
    std::fs::create_dir_all(&dir).ok();
    dir.join("navigation.log")
}

fn now_secs() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0)
}

fn open_externally(_app: &AppHandle, url: &str) {
    if let Err(err) = tauri_plugin_opener::open_url(url, None::<&str>) {
        eprintln!("failed to open {url} externally: {err}");
    }
}

/// Persist geometry, throttled while the user is dragging. The final state
/// after a drag is captured by [`persist_state_now`] when the window closes.
pub fn persist_state(app: &AppHandle) {
    if !WINDOW_READY.load(Ordering::Relaxed) {
        return;
    }
    let now = now_millis();
    let last = LAST_PERSIST_MS.load(Ordering::Relaxed);
    if now.saturating_sub(last) < PERSIST_INTERVAL_MS {
        return;
    }
    persist_state_now(app);
}

/// Persist geometry immediately, ignoring the throttle.
pub fn persist_state_now(app: &AppHandle) {
    if !WINDOW_READY.load(Ordering::Relaxed) {
        return;
    }
    let Some(win) = app.get_webview_window(MAIN_LABEL) else {
        return;
    };
    let scale = win.scale_factor().unwrap_or(1.0);
    let size = win.inner_size().unwrap_or_default();
    let position: Option<PhysicalPosition<i32>> = win.outer_position().ok();

    let state = WindowState {
        width: size.width as f64 / scale,
        height: size.height as f64 / scale,
        x: position.map(|p| p.x),
        y: position.map(|p| p.y),
    };
    Config::save_window_state(app, &state);
    LAST_PERSIST_MS.store(now_millis(), Ordering::Relaxed);
}

fn now_millis() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_millis() as u64)
        .unwrap_or(0)
}

pub fn show_main(app: &AppHandle) {
    if let Some(win) = app.get_webview_window(MAIN_LABEL) {
        let _ = win.show();
        let _ = win.unminimize();
        let _ = win.set_focus();
    }
}

pub fn hide_main(app: &AppHandle) {
    if let Some(win) = app.get_webview_window(MAIN_LABEL) {
        let _ = win.hide();
    }
}

pub fn toggle_main(app: &AppHandle) {
    let Some(win) = app.get_webview_window(MAIN_LABEL) else {
        return;
    };
    match win.is_visible() {
        Ok(true) => hide_main(app),
        _ => show_main(app),
    }
}

pub fn focus_main(app: &AppHandle) {
    show_main(app);
}
