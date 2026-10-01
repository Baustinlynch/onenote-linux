//! Keyboard shortcuts that the page cannot implement itself.
//!
//! `Ctrl+Q` and `Ctrl+H` have to reach the host, and the webview has no IPC
//! bridge, so the injected script raises them as a custom-scheme navigation
//! that the navigation handler consumes.

use tauri::AppHandle;

use crate::window;

pub const SCHEME: &str = "onenote-keys";

/// Injected into the main frame on every page load.
pub const BRIDGE_JS: &str = r#"
(function () {
  if (window.__onenoteKeysPatched) return;
  window.__onenoteKeysPatched = true;

  function raise(action) {
    var a = document.createElement("a");
    a.href = "onenote-keys://" + action;
    document.documentElement.appendChild(a);
    a.click();
    a.remove();
  }

  document.addEventListener(
    "keydown",
    function (e) {
      if (!e.ctrlKey || e.altKey || e.metaKey) return;
      var k = e.key.toLowerCase();
      if (k === "q") {
        e.preventDefault();
        e.stopPropagation();
        raise("quit");
      } else if (k === "h") {
        e.preventDefault();
        e.stopPropagation();
        raise("hide");
      }
    },
    true
  );
})();
"#;

/// Returns true when the URL was ours, so navigation should be denied.
pub fn handle(app: &AppHandle, url: &tauri::Url) -> bool {
    if url.scheme() != SCHEME {
        return false;
    }
    match url.host_str() {
        Some("quit") => app.exit(0),
        Some("hide") => window::hide_main(app),
        _ => {}
    }
    true
}
