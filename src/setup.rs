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
        let _ = complete(app, &host);
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
    // The page does `JSON.parse`, so the result is handed over as a JSON string
    // literal. Encoding it a second time is what quotes it.
    let Ok(quoted) = serde_json::to_string(&json) else {
        return;
    };
    let _ = window.eval(format!("window.__onenoteDiscoveryResult({quoted})"));
}

/// Reduce whatever the page sent to a bare lowercase host.
///
/// Accepts the shapes a user is likely to paste: a bare host, a full URL, or
/// one with a trailing path. Returns an empty string when the user skipped.
fn normalize_host(input: &str) -> String {
    let mut host = input.trim();
    for prefix in ["https://", "http://"] {
        if let Some(rest) = host.strip_prefix(prefix) {
            host = rest;
        }
    }
    host.split(['/', '?', '#'])
        .next()
        .unwrap_or("")
        .trim()
        .to_ascii_lowercase()
}

/// Persist the discovered host and open OneNote.
///
/// A blank host means the user skipped, or their organisation signs in through
/// Microsoft directly. Either way setup is done: we record that the wizard ran
/// and keep the `sts` label heuristic enabled so a federated personal account
/// still resolves. A host we do not recognise is refused, and the wizard stays
/// open so it can be corrected.
pub fn complete(app: &AppHandle, idp_host: &str) -> bool {
    let host = normalize_host(idp_host);

    if !host.is_empty() && !crate::nav::is_trusted_exact_host(&host) {
        push(
            app,
            &DiscoveryView {
                state: "invalid",
                domain: String::new(),
                idp_host: None,
                brand: None,
                message: format!(
                    "{host} is not a sign-in host I recognise. Check it with your IT team, \
                     or skip setup and add it later in Settings."
                ),
            },
        );
        return false;
    }

    let mut cfg = Config::get_or_default();
    if !host.is_empty() {
        if !cfg.allowed_hosts.contains(&host) {
            cfg.allowed_hosts.push(host);
        }
        // Pin the exact host instead of trusting a label that merely says "sts".
        cfg.allow_federated_hosts = false;
    }
    cfg.setup_complete = true;
    cfg.save(app);

    if let Some(window) = app.get_webview_window(LABEL) {
        let _ = window.close();
    }
    window::show_main(app);
    if let Some(main) = app.get_webview_window(window::MAIN_LABEL) {
        let _ = main.set_focus();
    }
    true
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

#[cfg(test)]
mod tests {
    use super::normalize_host;

    #[test]
    fn blank_input_means_skipped() {
        assert_eq!(normalize_host(""), "");
        assert_eq!(normalize_host("   "), "");
    }

    #[test]
    fn bare_host_is_lowercased() {
        assert_eq!(normalize_host("STS.EA.EDIN.SCH.UK"), "sts.ea.edin.sch.uk");
    }

    #[test]
    fn url_shapes_are_reduced_to_the_host() {
        assert_eq!(normalize_host("https://login.sch.uk/"), "login.sch.uk");
        assert_eq!(normalize_host("http://login.sch.uk"), "login.sch.uk");
        assert_eq!(
            normalize_host("  https://a.example.com/adfs/  "),
            "a.example.com"
        );
        assert_eq!(normalize_host("login.sch.uk?x=1"), "login.sch.uk");
    }
}
