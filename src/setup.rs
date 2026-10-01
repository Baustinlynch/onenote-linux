//! First-run setup.
//!
//! Shown once, before the main window, when the user has work or school
//! credentials. It asks for the organisation domain, asks Microsoft which
//! identity provider that tenant uses, and pins the result so single sign-on
//! stays inside the app window.
//!
//! Like the settings window, it uses no Tauri IPC: `withGlobalTauri` is off so
//! third-party content cannot reach host commands. The page talks to us by
//! navigating to the `onenote-setup` scheme, which the navigation handler
//! consumes.

use serde::Serialize;
use tauri::{AppHandle, Manager, WebviewUrl, WebviewWindowBuilder};

use crate::config::Config;
use crate::discovery::{self, Federation};
use crate::window;

pub const SCHEME: &str = "onenote-setup";
pub const LABEL: &str = "setup";

/// Serialised to the page once discovery finishes.
#[derive(Serialize)]
struct DiscoveryView {
    state: &'static str,
    domain: String,
    idp_host: Option<String>,
    brand: Option<String>,
    message: String,
}

const BRIDGE_JS: &str = r#"
window.__onenoteSetup = {
  discover: function (domain) {
    var a = document.createElement("a");
    a.href = "onenote-setup://discover?domain=" + encodeURIComponent(domain);
    document.documentElement.appendChild(a);
    a.click();
    a.remove();
  },
  finish: function (host) {
    var a = document.createElement("a");
    a.href = "onenote-setup://finish?idp_host=" + encodeURIComponent(host || "");
    document.documentElement.appendChild(a);
    a.click();
    a.remove();
  }
};
window.__onenoteDiscoveryResult = function (json) {
  if (window.__onenoteOnDiscovery) window.__onenoteOnDiscovery(json);
};
"#;

/// True when the user has never completed setup.
pub fn is_first_run(app: &AppHandle) -> bool {
    !Config::path_exists(app)
}

/// Returns true when the URL was ours and navigation should be denied.
pub fn handle(app: &AppHandle, url: &tauri::Url) -> bool {
    if url.scheme() != SCHEME {
        return false;
    }

    if url.host_str() == Some("finish") {
        let host = url
            .query_pairs()
            .find(|(k, _)| k == "idp_host")
            .map(|(_, v)| v.into_owned())
            .unwrap_or_default();
        complete(app, &host);
        return true;
    }

    if url.host_str() != Some("discover") {
        return true;
    }

    let domain = url
        .query_pairs()
        .find(|(k, _)| k == "domain")
        .map(|(_, v)| v.into_owned())
        .unwrap_or_default();

    let owner = app.clone();
    tauri::async_runtime::spawn(async move {
        // Tell the page we started, so it can show progress.
        push(
            &owner,
            &DiscoveryView {
                state: "loading",
                domain: domain.clone(),
                idp_host: None,
                brand: None,
                message: "Asking Microsoft which sign-in service your organisation uses...".into(),
            },
        );
        let result = discovery::discover(&domain).await;
        push(&owner, &to_view(result));
    });

    true
}

fn to_view(result: discovery::Discovery) -> DiscoveryView {
    let (state, message) = match (result.federation, &result.idp_host) {
        (Federation::Managed, _) => (
            "managed",
            "Your organisation uses Microsoft sign-in directly. No extra host is needed."
                .to_string(),
        ),
        (Federation::Federated, Some(host)) => (
            "federated",
            format!(
                "Your organisation signs you in at {host}. It will be trusted inside this \
                 window so single sign-on works."
            ),
        ),
        (Federation::Federated, None) => (
            "untrusted",
            "Your organisation federates to a sign-in server, but it was not shaped like a \
             standard identity provider, so it will not be trusted automatically. Enter the \
             hostname below if you want to allow it."
                .to_string(),
        ),
        (Federation::Unknown, _) => (
            "unknown",
            "Microsoft does not recognise that domain, or the lookup failed. You can enter \
             your sign-in hostname manually below, or skip this and look it up later in \
             Settings."
                .to_string(),
        ),
    };
    DiscoveryView {
        state,
        domain: result.domain,
        idp_host: result.idp_host,
        brand: result.brand,
        message,
    }
}

/// Hand a result back to the page. Results are passed as JSON in a quoted
/// string so no escaping is needed beyond JSON string encoding.
fn push(app: &AppHandle, view: &DiscoveryView) {
    let Some(window) = app.get_webview_window(LABEL) else {
        return;
    };
    let Ok(json) = serde_json::to_string(view) else {
        return;
    };
    let encoded = serde_json::to_string(&json).unwrap_or_else(|_| "\"\"".into());
    let _ = window.eval(format!("window.__onenoteDiscoveryResult({encoded})"));
}

/// Persist the discovered host and open OneNote.
pub fn complete(app: &AppHandle, idp_host: &str) {
    let mut cfg = Config::load(app);

    let host = idp_host
        .trim()
        .trim_start_matches("https://")
        .trim_matches('/');
    let host = host.split('/').next().unwrap_or("").to_ascii_lowercase();
    // Only persist a host the navigation policy already accepts, so the
    // allow-list can never be widened past what settings would permit.
    if !host.is_empty() && crate::nav::is_trusted_exact_host(&host) {
        if !cfg.allowed_hosts.contains(&host) {
            cfg.allowed_hosts.push(host);
        }
        // The user's own IdP is now pinned explicitly, so the broad heuristic
        // can be switched off. This is the tighter of the two options.
        cfg.allow_federated_hosts = false;
    }

    // Mark setup as done even when no host was found, so the wizard does not
    // reappear on every launch.
    cfg.setup_complete = true;
    cfg.save(app);

    if let Some(window) = app.get_webview_window(LABEL) {
        let _ = window.close();
    }
    window::show_main(app);
    let _ = app
        .get_webview_window(window::MAIN_LABEL)
        .map(|w| w.set_focus());
}

pub fn show_setup_window(app: &AppHandle) {
    if let Some(window) = app.get_webview_window(LABEL) {
        let _ = window.show();
        let _ = window.set_focus();
        return;
    }

    let owner = app.clone();
    let script = BRIDGE_JS.to_string();

    let _ = WebviewWindowBuilder::new(app, LABEL, WebviewUrl::App("setup.html".into()))
        .title("Set up OneNote")
        .inner_size(560.0, 640.0)
        .min_inner_size(460.0, 520.0)
        .resizable(true)
        .center()
        .initialization_script(script)
        .on_navigation(move |url| !handle(&owner, url))
        .build();
}
