use std::fs;
use std::path::PathBuf;

use serde::{Deserialize, Serialize};
use tauri::{AppHandle, Manager};
use url::Url;

use crate::nav;

pub const START_URL: &str = "https://www.onenote.com/";

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

    pub fn path(app: &AppHandle) -> PathBuf {
        let _ = app;
        let dir = Self::dir();
        fs::create_dir_all(&dir).ok();
        dir.join("config.json")
    }

    /// Whether a config file has ever been written. Used to decide if this is
    /// a first run, before any defaults are filled in.
    pub fn path_exists(app: &AppHandle) -> bool {
        Self::path(app).is_file()
    }

    pub fn window_state_path(app: &AppHandle) -> PathBuf {
        let dir = app
            .path()
            .app_data_dir()
            .unwrap_or_else(|_| Self::path(app).parent().unwrap().to_path_buf());
        fs::create_dir_all(&dir).ok();
        dir.join("window-state.json")
    }

    pub fn load(app: &AppHandle) -> Self {
        let path = Self::path(app);
        fs::read_to_string(&path)
            .ok()
            .and_then(|raw| serde_json::from_str(&raw).ok())
            .unwrap_or_default()
    }

    pub fn save(&self, app: &AppHandle) {
        if let Ok(raw) = serde_json::to_string_pretty(self) {
            fs::write(Self::path(app), raw).ok();
        }
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
