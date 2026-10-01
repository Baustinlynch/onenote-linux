use tauri::Url;
use url::Url as UrlParser;

/// Microsoft-owned and auth-related suffixes that must stay in-app.
const BUILT_IN: &[&str] = &[
    // OneNote surfaces
    "onenote.com",
    "office.com",
    "office.net",
    "office365.com",
    "officeapps.live.com",
    "sharepoint.com",
    "microsoft.com",
    // Identity / auth
    "microsoftonline.com",
    "msauth.net",
    "msftauth.net",
    "msftauthimages.net",
    "msidentity.com",
    "live.com",
    "aadcdn.msauth.net",
    "windows.net",
    "azureedge.net",
    "cloud.microsoft",
    // Content / media
    "microsoftusercontent.com",
    "akamaized.net",
];

/// Leading labels that suggest an organisation's own identity provider.
/// These are tenant-specific and cannot be enumerated ahead of time, so they
/// are matched heuristically during the login flow.
///
/// Matching is per DNS label, never as a bare substring: `login.contoso.edu`
/// matches, `evil-login.microsoft.com.attacker.net` does not.
const FEDERATION_LABELS: &[&str] = &[
    "adfs",
    "adfsserver",
    "auth",
    "autodiscover",
    "federated",
    "idp",
    "login",
    "owa",
    "sso",
    // ADFS exposes its Security Token Service on a `sts.*` host in most
    // deployments, and that is the endpoint a WS-Federation sign-in hits
    // before it redirects back to Microsoft.
    "sts",
];

/// `host == suffix` or `host` ends with `.suffix`
pub fn host_matches(host: &str, suffix: &str) -> bool {
    let suffix = suffix.trim_start_matches("*.").to_ascii_lowercase();
    let host = host.to_ascii_lowercase();
    host == suffix || host.ends_with(&format!(".{suffix}"))
}

/// Tokens that betray a Microsoft-impersonating host. A lookalike such as
/// `login.microsoft.com.attacker.net` carries a valid IdP-style first label
/// but a Microsoft token in the registrable part, so it is never federation.
const IMPERSONATION_TOKENS: &[&str] = &[
    "microsoft",
    "msft",
    "office",
    "office365",
    "onenote",
    "sharepoint",
    "windows",
    "outlook",
    "live.com",
];

/// True when the first DNS label is a known IdP marker.
fn has_federation_label(label: &str) -> bool {
    FEDERATION_LABELS
        .iter()
        .any(|marker| label.eq_ignore_ascii_case(marker))
}

/// Supports both `example.com` and `*.example.com` entries.
pub fn matches_any(url: &Url, extra: &[String]) -> bool {
    match url.host_str() {
        Some(host) => {
            BUILT_IN.iter().any(|s| host_matches(host, s))
                || extra.iter().any(|e| host_matches(host, e))
        }
        None => false,
    }
}

/// Whether a URL should load inside the app window.
///
/// `allow_federated` enables the org-SSO heuristic. It is a convenience for
/// work and school accounts whose identity provider cannot be enumerated in
/// advance, and it is inherently a guess: a hostile host such as
/// `sso.evil.io` is shaped identically to a legitimate `sso.campus.edu` and
/// will be treated the same. Hosts that are neither Microsoft-owned nor
/// recognisably federated are always sent to the system browser, so the
/// blast radius is limited to pages deliberately opened during a login flow.
pub fn is_in_app(url: &Url, extra_hosts: &[String], allow_federated: bool) -> bool {
    if !is_safe_scheme(url) {
        return false;
    }
    if matches_any(url, extra_hosts) {
        return true;
    }
    allow_federated && looks_like_federation(url)
}

/// Heuristic for org SSO endpoints that redirect during login.
///
/// Narrow by construction: HTTPS only, at least three labels, a known IdP
/// marker as the first label, and no Microsoft-owned token anywhere in the
/// name (which would indicate either a real Microsoft host or an
/// impersonation attempt such as `login.microsoft.com.attacker.net`).
pub fn looks_like_federation(url: &Url) -> bool {
    if !matches!(url.scheme(), "https" | "http") {
        return false;
    }
    let Some(host) = url.host_str().map(str::to_ascii_lowercase) else {
        return false;
    };
    if url.scheme() != "https" {
        return false;
    }
    if host_matches(&host, "microsoft.com") || host_matches(&host, "onmicrosoft.com") {
        return false;
    }

    let labels: Vec<&str> = host.split('.').collect();
    // Require a real multi-registrable name; no bare IPs or single labels.
    if labels.len() < 3 {
        return false;
    }

    // Reject anything carrying a Microsoft token in its own labels: that is
    // either a real Microsoft host (handled above) or an impersonation attempt.
    for label in &labels {
        let label = label.to_ascii_lowercase();
        if IMPERSONATION_TOKENS
            .iter()
            .any(|token| label.contains(token))
        {
            return false;
        }
    }

    // Numeric-only labels mean an IP literal dressed up as a domain.
    if labels
        .iter()
        .any(|l| !l.is_empty() && l.chars().all(|c| c.is_ascii_digit()))
    {
        return false;
    }

    has_federation_label(labels[0])
}

/// Whether a bare hostname is one we will render inside the app window.
///
/// This is the gate applied to hosts learned at runtime, such as an
/// organisation's federation endpoint reported by Microsoft's UserRealm
/// endpoint. Requiring the full federation heuristic means a discovered host
/// is only trusted if it looks like a genuine IdP: HTTPS-shaped, multi-label,
/// marked as an identity provider, and free of any Microsoft token that would
/// suggest impersonation. A host that fails is reported back to the user as
/// untrusted rather than being added to the allow-list.
pub fn is_trusted_exact_host(host: &str) -> bool {
    let host = host.trim().trim_end_matches('.').to_ascii_lowercase();
    if host.is_empty() {
        return false;
    }
    // Reuse the same checks, expressed against a synthetic HTTPS URL so the
    // policy cannot drift between the two call sites.
    let Ok(url) = UrlParser::parse(&format!("https://{host}/")) else {
        return false;
    };
    matches_any(&url, &[]) || looks_like_federation(&url)
}

pub fn is_http_url(url: &Url) -> bool {
    matches!(url.scheme(), "http" | "https")
}

/// Reject schemes we should never hand to the webview.
pub fn is_safe_scheme(url: &Url) -> bool {
    is_http_url(url) || matches!(url.scheme(), "about" | "blob" | "data" | "tauri")
}

/// Parse a URL string, returning None if unparseable or an unsafe scheme.
pub fn parse(raw: &str) -> Option<UrlParser> {
    UrlParser::parse(raw).ok().filter(is_safe_scheme)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn u(raw: &str) -> UrlParser {
        UrlParser::parse(raw).unwrap()
    }

    #[test]
    fn microsoft_surfaces_stay_in_app() {
        for url in [
            "https://www.onenote.com/",
            "https://login.microsoftonline.com/common/oauth2/v2.0/authorize",
            "https://login.live.com/oauth20_authorize.srf",
            "https://contoso-my.sharepoint.com/personal/x/Documents/Notebooks",
            // The embedded notebook viewer is served from a resources.office.net
            // host; without this it was pushed out to the system browser.
            "https://fa000000128.resources.office.net/1.0.0/en-us_web/index_onenotejs.html",
            "https://aadcdn.msauth.net/content/coreservices/1.4.8/en-US/favicon.ico",
        ] {
            assert!(matches_any(&u(url), &[]), "{url} should be allowed");
        }
    }

    #[test]
    fn unrelated_sites_are_external() {
        for url in ["https://gitlab.com/", "https://wikipedia.org/"] {
            assert!(!matches_any(&u(url), &[]), "{url} should be external");
        }
    }

    #[test]
    fn microsoft_lookalike_domains_are_rejected() {
        // A Microsoft token in the name means either a real Microsoft host
        // (already on the allow-list) or an impersonation attempt.
        for url in [
            "https://microsoft.com.evil.io/",
            "https://evil-login.microsoft.com.attacker.net/",
            "https://login.microsoft.com.attacker.net/",
            "https://sso.office.com.phish.net/",
        ] {
            assert!(!matches_any(&u(url), &[]), "{url} must not be allow-listed");
            assert!(
                !looks_like_federation(&u(url)),
                "{url} must not be treated as federation"
            );
        }
    }

    #[test]
    fn org_idp_hosts_are_recognised() {
        for url in [
            "https://adfs.contoso.edu/adfs/ls/",
            "https://login.contoso.edu/Account/Login",
            "https://sso.campus.edu/idp/profile",
            "https://idp.contoso.com/",
            // ADFS Security Token Service: the host a WS-Federation sign-in
            // redirects to before returning to Microsoft.
            "https://sts.ea.edin.sch.uk/adfs/ls/?wa=wsignin1.0",
            "https://sts.campus.edu/adfs/ls/",
        ] {
            assert!(looks_like_federation(&u(url)), "{url} should stay in-app");
            assert!(is_in_app(&u(url), &[], true), "{url} should stay in-app");
        }
    }

    #[test]
    fn sts_is_recognised_but_lookalikes_are_not() {
        // The real endpoint keeps working.
        assert!(looks_like_federation(&u("https://sts.campus.edu/adfs/ls/")));
        // But `sts` in a non-leading position, or a Microsoft-impersonating
        // variant, must not be trusted.
        for url in [
            "https://cdn.sts.campus.edu/",
            "https://sts.microsoft.com.attacker.net/adfs/ls/",
            "https://notsts.campus.edu/",
            "https://stsx.campus.edu/",
        ] {
            assert!(!looks_like_federation(&u(url)), "{url} must not be trusted");
        }
    }

    #[test]
    fn federation_heuristic_can_be_disabled() {
        // The documented trade-off: with the heuristic off, an unlisted IdP
        // host is handed to the browser instead.
        let idp = u("https://login.contoso.edu/Account/Login");
        assert!(!is_in_app(&idp, &[], false));
        // Microsoft-owned surfaces are unaffected by the toggle.
        assert!(is_in_app(
            &u("https://login.microsoftonline.com/common/oauth2/v2.0/authorize"),
            &[],
            false
        ));
    }

    #[test]
    fn federation_requires_https_and_real_domain() {
        assert!(!looks_like_federation(&u("http://login.contoso.edu/")));
        assert!(!looks_like_federation(&u("https://login/")));
        assert!(!looks_like_federation(&u("https://192.168.1.1/")));
        assert!(!looks_like_federation(&u("https://sso.127.0.0.1.nip.io/")));
    }

    #[test]
    fn user_supplied_hosts_are_honoured() {
        let extra = vec!["*.contoso.com".to_string(), "auth.corp.net".to_string()];
        assert!(matches_any(&u("https://sso.contoso.com/x"), &extra));
        assert!(matches_any(&u("https://auth.corp.net/x"), &extra));
        assert!(!matches_any(&u("https://contoso.com.evil.io/"), &extra));
        // Explicit entries win even with the heuristic disabled.
        assert!(is_in_app(&u("https://auth.corp.net/x"), &extra, false));
    }

    #[test]
    fn runtime_discovered_hosts_face_the_same_policy() {
        // A real IdP host, as Microsoft reports it.
        assert!(is_trusted_exact_host("sts.ea.edin.sch.uk"));
        assert!(is_trusted_exact_host("adfs.contoso.edu"));
        assert!(is_trusted_exact_host("login.microsoftonline.com"));
        // Case and a trailing root dot are normalised away.
        assert!(is_trusted_exact_host("STS.EA.EDIN.SCH.UK."));
        // Hosts that are not IdP-shaped are refused even if Microsoft returned
        // them: the allow-list is for identity providers, not arbitrary sites.
        assert!(!is_trusted_exact_host("gitlab.com"));
        assert!(!is_trusted_exact_host("attacker.net"));
        assert!(!is_trusted_exact_host("login.microsoft.com.attacker.net"));
        assert!(!is_trusted_exact_host("localhost"));
        assert!(!is_trusted_exact_host("192.168.1.1"));
        assert!(!is_trusted_exact_host(""));
        assert!(!is_trusted_exact_host("not a host"));
    }

    #[test]
    fn unsafe_schemes_are_filtered() {
        assert!(!is_safe_scheme(&u("javascript:alert(1)")));
        assert!(!is_safe_scheme(&u("file:///etc/passwd")));
        assert!(is_safe_scheme(&u("https://example.com/")));
        assert!(parse("javascript:alert(1)").is_none());
        assert!(parse("https://example.com/").is_some());
    }
}
