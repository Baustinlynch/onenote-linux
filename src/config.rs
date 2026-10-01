use std::fs;
use std::path::PathBuf;
use std::sync::RwLock;

use serde::{Deserialize, Serialize};
use tauri::{AppHandle, Manager};
use url::Url;

use crate::nav;

pub const START_URL: &str = "https://www.onenote.com/";

// A `OnceLock` cannot be reassigned, so a config saved after startup (settings
// changes, first-run setup) would never reach the running app. The lock keeps
// the value readable everywhere and writable from `save`.
static CONFIG_CACHE: RwLock<Option<Config>> = RwLock::new(None);

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct Config {
    pub close_to_tray: bool,
    pub start_minimized: bool,
    pub zoom: f64,
    pub start_url: String,
    /// Extra hosts to treat as in-app (federated SSO tenants, intranets).
    /// Supports exact hosts and `*.suffix` wildcards.
    pub allowed_hosts: Vec<String>,
    /// Allow unlisted HTTPS hosts that look like an organisation identity
    /// provider (`login.*`, `adfs.*`, `sso.*`, `idp.*`, `auth.*`, `owa.*`).
    /// Needed for most work and school accounts; see the security note in
    /// `nav::is_in_app` before leaving this on.
    pub allow_federated_hosts: bool,
    pub notifications: bool,
    /// Whether the first-run wizard has been completed or explicitly skipped.
    /// Its absence is what makes the config file count as "not yet written",
    /// so the wizard appears exactly once.
    pub setup_complete: bool,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            close_to_tray: true,
            start_minimized: false,
            zoom: 1.0,
            start_url: START_URL.to_string(),
            allowed_hosts: Vec::new(),
            allow_federated_hosts: true,
            notifications: true,
            setup_complete: false,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WindowState {
    pub width: f64,
    pub height: f64,
    pub x: Option<i32>,
    pub y: Option<i32>,
}

impl Default for WindowState {
    fn default() -> Self {
        Self {
            width: 1400.0,
            height: 900.0,
            x: None,
            y: None,
        }
    }
}

impl Config {
    /// Config lives in a stable `onenote-linux` directory rather than under
    /// the app identifier, so the path does not change if the identifier is
    /// ever revised and stays predictable for packaging and documentation.
    pub fn dir() -> PathBuf {
        let base = std::env::var("XDG_CONFIG_HOME")
            .ok()
            .filter(|p| !p.is_empty())
            .map(PathBuf::from)
            .or_else(|| {
                std::env::var("HOME")
                    .ok()
                    .filter(|h| !h.is_empty())
                    .map(|home| PathBuf::from(home).join(".config"))
            })
            .unwrap_or_else(|| PathBuf::from("."));
        base.join("onenote-linux")
    }

    pub fn path() -> PathBuf {
        let dir = Self::dir();
        fs::create_dir_all(&dir).ok();
        dir.join("config.json")
    }

    /// Whether a config file has ever been written. Older builds never wrote
    /// `setup_complete`, so file presence - not that flag - is what tells us
    /// the wizard has already been dealt with.
    pub fn exists() -> bool {
        Self::path().is_file()
    }

    /// Stable per-user data directory, used when the framework cannot resolve
    /// one. Mirrors the XDG data layout rather than falling back to /tmp,
    /// which is cleared on reboot and would lose the webview session.
    pub fn data_dir() -> PathBuf {
        let base = std::env::var("XDG_DATA_HOME")
            .ok()
            .filter(|p| !p.is_empty())
            .map(PathBuf::from)
            .or_else(|| {
                std::env::var("HOME")
                    .ok()
                    .filter(|h| !h.is_empty())
                    .map(|home| PathBuf::from(home).join(".local/share"))
            })
            .unwrap_or_else(|| PathBuf::from("."));
        base.join("dev.onenoteweb.linux")
    }

    pub fn window_state_path(app: &AppHandle) -> PathBuf {
        let dir = app
            .path()
            .app_data_dir()
            .unwrap_or_else(|_| Self::dir().parent().unwrap().to_path_buf());
        fs::create_dir_all(&dir).ok();
        dir.join("window-state.json")
    }

    /// Load config from disk, caching it globally. Call once at startup.
    pub fn load_cached(_app: &AppHandle) -> Self {
        let path = Self::path();
        let config: Config = fs::read_to_string(&path)
            .ok()
            .and_then(|raw| serde_json::from_str(&raw).ok())
            .unwrap_or_default();
        *CONFIG_CACHE.write().unwrap_or_else(|e| e.into_inner()) = Some(config.clone());
        config
    }

    /// Get cached config, loading if necessary.
    pub fn get() -> Option<Config> {
        CONFIG_CACHE
            .read()
            .unwrap_or_else(|e| e.into_inner())
            .clone()
    }

    /// Get cached config or default, returning owned value.
    pub fn get_or_default() -> Config {
        Self::get().unwrap_or_default()
    }

    pub fn save(&self, _app: &AppHandle) {
        if let Ok(raw) = serde_json::to_string_pretty(self) {
            fs::write(Self::path(), raw).ok();
        }
        *CONFIG_CACHE.write().unwrap_or_else(|e| e.into_inner()) = Some(self.clone());
    }

    pub fn load_window_state(app: &AppHandle) -> WindowState {
        fs::read_to_string(Self::window_state_path(app))
            .ok()
            .and_then(|raw| serde_json::from_str(&raw).ok())
            .unwrap_or_default()
    }

    pub fn save_window_state(app: &AppHandle, state: &WindowState) {
        if let Ok(raw) = serde_json::to_string(state) {
            fs::write(Self::window_state_path(app), raw).ok();
        }
    }

    /// Whether a URL should load inside the app window.
    pub fn is_in_app(&self, url: &Url) -> bool {
        nav::is_in_app(url, &self.allowed_hosts, self.allow_federated_hosts)
    }
}
