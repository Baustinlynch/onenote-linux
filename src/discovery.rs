//! First-run discovery of an organisation's federation endpoint.
//!
//! Work and school Microsoft 365 accounts often federate to an on-premises
//! identity provider (typically ADFS). The OneNote web client redirects there
//! during sign-in, so the host has to be trusted in-app or the flow is bounced
//! out to the system browser and never comes back.
//!
//! Rather than guessing hostnames with DNS probing, we ask Microsoft directly.
//! The UserRealm endpoint is unauthenticated and returns the authoritative
//! `AuthURL` for a domain, including the exact host to trust and the tenant
//! branding name for confirmation.

use std::time::Duration;

use serde::Deserialize;

const USERREALM: &str = "https://login.microsoftonline.com/common/userrealm";
const TIMEOUT: Duration = Duration::from_secs(10);

/// Microsoft's response for a domain.
#[derive(Debug, Deserialize)]
struct UserRealm {
    #[serde(rename = "NameSpaceType")]
    namespace_type: String,
    #[serde(rename = "AuthURL")]
    auth_url: Option<String>,
    #[serde(rename = "FederationBrandName")]
    brand: Option<String>,
    #[serde(rename = "DomainName")]
    domain: Option<String>,
}

/// How a tenant authenticates users.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Federation {
    /// Microsoft hosts the accounts. No extra host is needed.
    Managed,
    /// The tenant redirects to an external identity provider.
    Federated,
    /// The domain is not a known tenant, or the endpoint said nothing useful.
    Unknown,
}

#[derive(Debug, Clone)]
pub struct Discovery {
    pub federation: Federation,
    /// The exact identity provider host to trust in-app, if federated.
    pub idp_host: Option<String>,
    /// Tenant name from Microsoft, shown to the user as a sanity check.
    pub brand: Option<String>,
    /// The domain that was actually queried, echoed back for display.
    pub domain: String,
}

/// Reduce arbitrary user input to a bare domain name.
///
/// Accepts a full email address, a URL, or a domain, because users copy
/// whichever they happen to have to hand. Returns `None` for input that
/// cannot be a plausible public domain, so we never issue a request built
/// from something that is obviously not a domain.
pub fn normalise_domain(input: &str) -> Option<String> {
    let mut value = input.trim().to_ascii_lowercase();

    // Strip a URL prefix down to its host.
    for prefix in ["https://", "http://"] {
        if let Some(rest) = value.strip_prefix(prefix) {
            value = rest.to_string();
            break;
        }
    }
    if let Some((host, _)) = value.split_once('/') {
        value = host.to_string();
    }
    if let Some((host, _)) = value.split_once('?') {
        value = host.to_string();
    }

    // An email address tells us the domain part.
    if let Some((_, domain)) = value.rsplit_once('@') {
        value = domain.to_string();
    }

    // Drop an explicit port and any trailing root dot.
    if let Some((host, _)) = value.rsplit_once(':') {
        if host
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || c == '.' || c == '-')
        {
            value = host.to_string();
        }
    }
    value = value.trim_end_matches('.').to_string();

    // A valid domain has at least one dot, only hostname characters, and no
    // label longer than 63 characters (RFC 1035).
    if !value.contains('.') || value.len() > 253 {
        return None;
    }
    if !value
        .split('.')
        .all(|label| !label.is_empty() && label.len() <= 63 && valid_label(label))
    {
        return None;
    }

    // Require an alphabetic TLD. This rejects IP addresses and typos, and
    // keeps us from sending a bare hostname to Microsoft.
    let tld = value.rsplit('.').next()?;
    if tld.len() < 2 || !tld.chars().all(|c| c.is_ascii_alphabetic()) {
        return None;
    }

    Some(value)
}

/// A single DNS label: letters, digits and hyphens, not starting or ending
/// with a hyphen (RFC 1035). Keeping this strict means only a syntactically
/// real hostname is ever sent to Microsoft.
fn valid_label(label: &str) -> bool {
    let mut chars = label.chars();
    match (chars.next(), chars.next_back()) {
        (Some(first), Some(last)) => {
            (first.is_ascii_alphanumeric() && last.is_ascii_alphanumeric())
                && label.chars().all(|c| c.is_ascii_alphanumeric() || c == '-')
        }
        // A single-character label cannot both start and end with a hyphen.
        (Some(_), None) | (None, _) => false,
    }
}

/// Ask Microsoft which identity provider a domain uses.
///
/// A network or parsing failure is reported as `Unknown` rather than an error:
/// the caller can still let the user type a host by hand, and refusing to
/// continue would be worse than an imprecise answer.
pub async fn discover(input: &str) -> Discovery {
    let domain = normalise_domain(input).unwrap_or_default();
    if domain.is_empty() {
        return Discovery {
            federation: Federation::Unknown,
            idp_host: None,
            brand: None,
            domain: String::new(),
        };
    }

    let client = match reqwest::Client::builder()
        .timeout(TIMEOUT)
        .user_agent(concat!("onenote-linux/", env!("CARGO_PKG_VERSION")))
        .build()
    {
        Ok(client) => client,
        Err(_) => return unknown(&domain),
    };

    let url = format!("{USERREALM}/{domain}?api-version=2.1");
    let realm: UserRealm = match client.get(&url).send().await {
        Ok(response) if response.status().is_success() => match response.json().await {
            Ok(realm) => realm,
            Err(_) => return unknown(&domain),
        },
        _ => return unknown(&domain),
    };

    let brand = realm.brand.clone().filter(|b| !b.is_empty());

    match realm.namespace_type.as_str() {
        "Managed" => Discovery {
            federation: Federation::Managed,
            idp_host: None,
            brand,
            domain: realm.domain.unwrap_or(domain),
        },
        _ => {
            // Federated, or anything else unexpected. Only trust a host we can
            // actually read out of the AuthURL and that passes the same
            // navigation policy as every other in-app host.
            let idp_host = realm
                .auth_url
                .as_deref()
                .and_then(|auth| tauri::Url::parse(auth).ok())
                .and_then(|parsed| {
                    let host = parsed.host_str()?.to_ascii_lowercase();
                    crate::nav::is_trusted_exact_host(&host).then_some(host)
                });

            match idp_host {
                Some(host) => Discovery {
                    federation: Federation::Federated,
                    idp_host: Some(host),
                    brand,
                    domain: realm.domain.unwrap_or(domain),
                },
                None => Discovery {
                    federation: if realm.auth_url.is_some() {
                        // Federated, but the endpoint did not give us a host
                        // we are willing to trust in-app.
                        Federation::Federated
                    } else {
                        Federation::Unknown
                    },
                    idp_host: None,
                    brand,
                    domain: realm.domain.unwrap_or(domain),
                },
            }
        }
    }
}

fn unknown(domain: &str) -> Discovery {
    Discovery {
        federation: Federation::Unknown,
        idp_host: None,
        brand: None,
        domain: domain.to_string(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn emails_reduce_to_the_domain() {
        assert_eq!(
            normalise_domain("160688300@ea.edin.sch.uk").as_deref(),
            Some("ea.edin.sch.uk")
        );
        assert_eq!(
            normalise_domain("  Someone@EA.Edin.SCH.uk ").as_deref(),
            Some("ea.edin.sch.uk")
        );
    }

    #[test]
    fn urls_and_ports_are_stripped() {
        assert_eq!(
            normalise_domain("https://login.example.edu/Account/Login").as_deref(),
            Some("login.example.edu")
        );
        assert_eq!(
            normalise_domain("sts.example.edu:443/adfs/ls/").as_deref(),
            Some("sts.example.edu")
        );
        assert_eq!(
            normalise_domain("example.com.").as_deref(),
            Some("example.com")
        );
    }

    #[test]
    fn hosts_and_addresses_are_rejected() {
        for input in [
            "localhost",
            "192.168.1.10",
            "10.0.0.1",
            "",
            "   ",
            "not_a_domain",
            "example",
            "@",
            "user@",
        ] {
            assert_eq!(normalise_domain(input), None, "{input} should be rejected");
        }
    }

    #[test]
    fn messy_input_is_recovered_rather_than_rejected() {
        // Pasted input is often imperfect. Recover the domain rather than
        // making the user retype it.
        assert_eq!(
            normalise_domain("@example.com").as_deref(),
            Some("example.com")
        );
        assert_eq!(
            normalise_domain("example.com..").as_deref(),
            Some("example.com")
        );
        assert_eq!(
            normalise_domain("user@sub@corp.example.com").as_deref(),
            Some("corp.example.com")
        );
    }

    #[test]
    fn malformed_labels_are_rejected() {
        for input in [
            "example..com",
            ".example.com",
            "-example.com",
            "exa mple.com",
            "example.c0m",
            "exa$mple.com",
        ] {
            assert_eq!(normalise_domain(input), None, "{input} should be rejected");
        }
    }

    #[test]
    fn overlong_labels_are_rejected() {
        let long = "a".repeat(64);
        assert_eq!(normalise_domain(&format!("{long}.com")), None);
        // 63 is the limit and is still accepted.
        let ok = "a".repeat(63);
        assert_eq!(
            normalise_domain(&format!("{ok}.com")).as_deref(),
            Some(format!("{ok}.com").as_str())
        );
    }

    #[test]
    fn a_discovered_host_must_satisfy_the_navigation_policy() {
        // A host Microsoft hands back still has to be one we are willing to
        // render in a window titled OneNote.
        assert!(crate::nav::is_trusted_exact_host("sts.ea.edin.sch.uk"));
        assert!(!crate::nav::is_trusted_exact_host(
            "microsoft.com.attacker.net"
        ));
        assert!(!crate::nav::is_trusted_exact_host("localhost"));
    }
}
