//! Bridges the page's `Notification` API to the desktop notification daemon.
//!
//! The webview runs without `withGlobalTauri`, so there is no IPC bridge to
//! the host. Instead the injected script signals us through a custom scheme
//! (`onenote-notify://`) that the navigation handler intercepts and denies.

use std::io::BufRead;
use std::process::{Command, Stdio};

use tauri::AppHandle;

use crate::config::Config;
use crate::window;

pub const SCHEME: &str = "onenote-notify";

/// Injected into every page load in the main frame. Rewrites
/// `new Notification(title, options)` into a scheme navigation.
pub const BRIDGE_JS: &str = r#"
(function () {
  if (window.__onenoteNotifyPatched) return;
  window.__onenoteNotifyPatched = true;

  function encode(value) {
    return encodeURIComponent(String(value == null ? "" : value).slice(0, 2000));
  }

  function send(title, options) {
    var opts = options || {};
    var link = opts.data && opts.data.onenoteUrl;
    if (typeof link !== "string" || !/^https?:/i.test(link)) link = "";

    var href = "onenote-notify://notify?title=" + encode(title) +
               "&body=" + encode(opts.body || "") +
               "&tag=" + encode(opts.tag || "") +
               (link ? "&url=" + encode(link) : "");

    // A throwaway anchor keeps this out of the SPA's router and history.
    var a = document.createElement("a");
    a.href = href;
    a.style.display = "none";
    document.documentElement.appendChild(a);
    a.click();
    a.remove();
  }

  function FakeNotification(title, options) {
    send(title, options);
    this.title = title;
    this.options = options;
  }

  FakeNotification.permission = "granted";
  FakeNotification.requestPermission = function (cb) {
    if (cb) cb("granted");
    return Promise.resolve("granted");
  };
  FakeNotification.close = function () {};
  FakeNotification.maxActions = 2;
  ["onclick", "onclose", "onerror", "onshow"].forEach(function (k) {
    Object.defineProperty(FakeNotification, k, { set: function () {}, get: function () { return null; } });
  });

  try {
    Object.defineProperty(window, "Notification", {
      value: FakeNotification,
      writable: true,
      configurable: true,
    });
  } catch (e) {
    window.Notification = FakeNotification;
  }
})();
"#;

/// Returns true when the URL was ours, so the caller should deny navigation.
pub fn handle(app: &AppHandle, url: &tauri::Url) -> bool {
    if url.scheme() != SCHEME {
        return false;
    }

    if url.host_str() == Some("notify") {
        let params: std::collections::HashMap<String, String> = url
            .query_pairs()
            .map(|(k, v)| (k.into_owned(), v.into_owned()))
            .collect();

        if Config::get().map(|c| c.notifications).unwrap_or(true) {
            spawn_toast(
                app,
                params.get("title").map(String::as_str).unwrap_or("OneNote"),
                params.get("body").map(String::as_str).unwrap_or(""),
                params.get("tag").map(String::as_str).unwrap_or(""),
            );
        }
    }

    // Never let the scheme reach the page.
    true
}

/// Fire a native toast. Clicking the action raises the main window.
fn spawn_toast(app: &AppHandle, title: &str, body: &str, tag: &str) {
    let mut cmd = Command::new("notify-send");
    cmd.args([
        "--app-name=OneNote",
        "--expire-time=8000",
        "--action=open,Open OneNote",
    ])
    .arg("--icon=onenote-linux")
    .arg("--urgency=normal");

    if !tag.is_empty() {
        cmd.arg(format!("--replace-id={}", sanitize_tag(tag)));
    }

    cmd.arg(title).arg(body);
    cmd.stdin(Stdio::null());
    cmd.stdout(Stdio::piped());

    let mut child = match cmd.spawn() {
        Ok(child) => child,
        Err(err) => {
            eprintln!("notify-send failed to spawn: {err}");
            return;
        }
    };

    // Block on the child's stdout in a worker so the action can focus us.
    let handle = app.clone();
    std::thread::spawn(move || {
        if let Some(stdout) = child.stdout.take() {
            let mut line = String::new();
            let mut reader = std::io::BufReader::new(stdout);
            if reader.read_line(&mut line).is_ok() && line.trim() == "open" {
                window::show_main(&handle);
            }
        }
        let _ = child.wait();
    });
}

/// `notify-send` replace-ids must be short and free of separators.
fn sanitize_tag(tag: &str) -> String {
    let cleaned: String = tag
        .chars()
        .map(|c| if c.is_ascii_alphanumeric() { c } else { '-' })
        .take(64)
        .collect();
    if cleaned.is_empty() {
        "onenote".to_string()
    } else {
        cleaned
    }
}
