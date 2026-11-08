//! Bridges the page's `Notification` API to the desktop notification daemon.
//!
//! The webview runs without `withGlobalTauri`, so there is no IPC bridge to
//! the host. Instead the injected script signals us through a custom scheme
//! (`onenote-notify://`) that the navigation handler intercepts and denies.

use notify_rust::{Notification, Timeout, Urgency};
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
    let app = app.clone();
    let title = title.to_string();
    let body = body.to_string();
    let replace_id = replace_id(tag);

    std::thread::spawn(move || {
        let mut toast = Notification::new();
        toast
            .appname("OneNote")
            .icon("onenote-linux")
            .urgency(Urgency::Normal)
            .timeout(Timeout::Milliseconds(8000))
            .action("open", "Open OneNote")
            .summary(&title)
            .body(&body);

        // Only pass a replaces_id when the page tagged the notification, so
        // otherwise-untagged ones keep getting unique daemon-assigned ids.
        if let Some(id) = replace_id {
            toast.id(id);
        }

        // Send it, then block the thread until the notification times out or
        // the click action fires.
        let handle = match toast.show() {
            Ok(handle) => handle,
            Err(err) => {
                eprintln!("notification failed: {err}");
                return;
            }
        };

        handle.wait_for_action(|action| {
            if action == "open" {
                window::show_main(&app);
            }
        });
    });
}

/// Derive a stable replaces_id from the page's notification tag. The daemon
/// replaces an existing toast with the same id, so OneNote's duplicate
/// notifications collapse into one. Tagged, deterministic, short: FNV-1a over
/// the sanitized tag, clamped to a positive 31-bit id.
fn replace_id(tag: &str) -> Option<u32> {
    let cleaned: String = tag
        .chars()
        .map(|c| if c.is_ascii_alphanumeric() { c } else { '-' })
        .take(64)
        .collect();
    if cleaned.is_empty() || cleaned.chars().all(|c| c == '-') {
        return None;
    }
    let mut hash: u32 = 0x811c_9dc5;
    for byte in cleaned.as_bytes() {
        hash ^= u32::from(*byte);
        hash = hash.wrapping_mul(0x0100_0193);
    }
    Some(hash & 0x7fff_ffff)
}

#[cfg(test)]
mod tests {
    use super::replace_id;

    #[test]
    fn same_tag_replaces_same_id() {
        assert_eq!(replace_id("onenote-42"), replace_id("onenote-42"));
    }

    #[test]
    fn different_tags_get_different_ids() {
        assert_ne!(replace_id("a"), replace_id("b"));
    }

    #[test]
    fn empty_tag_is_untagged() {
        assert_eq!(replace_id(""), None);
        assert_eq!(replace_id("---"), None);
    }

    #[test]
    fn ids_are_short_positive() {
        let id = replace_id("Some/Long*Tag!@#9831").unwrap();
        assert!(id > 0);
        assert!(id <= u32::MAX / 2);
    }
}
