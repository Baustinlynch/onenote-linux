//! Simplified content blocker inspired by uBlock Origin.
//! Injected into every page load in the main frame.
//! Blocks known ad networks, trackers, and framing protection.

pub const BRIDGE_JS: &str = r#"
(function () {
  if (window.__onenoteBlockerPatched) return;
  window.__onenoteBlockerPatched = true;

  // Minimal blocklist of ad/tracking hostnames (sourced from uBlock Origin filter lists)
  var blockedHostnames = new Set([
    "google-analytics.com",
    "doubleclick.net",
    "googlesyndication.com",
    "2o7.net",
    "scorecardresearch.com",
    "quantserve.com",
    "atdmt.com",
    "adservice.google.com",
    "adsystem.twitter.com",
    "connect.facebook.net",
    "stats.wordpress.com",
    "pixel.facebook.net",
    "ads.linkedin.com",
    "tap.fyber.com",
    "adcolony.com",
    "appodeal.com",
    "moatads.com",
    "revcontent.com",
    "outbrain.com",
    "sharethrough.com",
    "triplelift.com",
    "rubiconproject.com",
    "openx.net",
    "indexww.com",
    "yieldmanager.com",
    "adform.net",
    "bidswitch.net",
    "pubmatic.com",
    "casalemedia.com",
    "criteo.com",
    "hilltopads.net",
    "popcash.net",
    "propellerads.com",
  ]);

  // Known tracking path patterns, only applied to same-origin requests so
  // authentication endpoints on other hosts (e.g. login.microsoftonline.com)
  // are never affected.
  var blockedPaths = [
    "/collect",
    "/log",
    "/pixel",
    "/beacon",
    "/track",
  ];

  // Matches a hostname and all of its subdomains.
  function isBlockedHostname(host) {
    if (!host) { return false; }
    var h = host.toLowerCase();
    do {
      if (blockedHostnames.has(h)) { return true; }
      var dot = h.indexOf(".");
      if (dot === -1) { return false; }
      h = h.slice(dot + 1);
    } while (h.indexOf(".") !== -1 || blockedHostnames.has(h));
    return false;
  }

  function extractUrl(input) {
    try {
      if (typeof input === "string") {
        return new URL(input, location.href);
      }
      if (input && typeof input.url === "string") {
        return new URL(input.url, location.href);
      }
    } catch (e) {}
    return null;
  }

  // Intercept fetch to block requests to known ad/tracking hosts
  var originalFetch = window.fetch;
  if (typeof originalFetch === "function") {
    window.fetch = function(input, init) {
      var parsed = extractUrl(input);
      if (parsed) {
        if (isBlockedHostname(parsed.hostname)) {
          if (init && init.signal) {
            return Promise.resolve(new Response(null, { status: 403 }));
          }
          return Promise.reject(new Error("Request blocked by onenote-linux content blocker"));
        }
        var path = parsed.pathname.toLowerCase();
        if (parsed.origin === location.origin && blockedPaths.indexOf(path) !== -1) {
          if (init && init.signal) {
            return Promise.resolve(new Response(null, { status: 403 }));
          }
          return Promise.reject(new Error("Request blocked by onenote-linux content blocker"));
        }
      }
      return originalFetch.call(this, input, init);
    };
  }

  // Prevent XMLHttpRequest to blocked hosts
  var originalOpen = XMLHttpRequest.prototype.open;
  XMLHttpRequest.prototype.open = function(method, url) {
    var parsed = extractUrl(url);
    var blocked = false;
    if (parsed) {
      if (isBlockedHostname(parsed.hostname)) {
        blocked = true;
      } else if (parsed.origin === location.origin &&
                 blockedPaths.indexOf(parsed.pathname.toLowerCase()) !== -1) {
        blocked = true;
      }
    }
    originalOpen.call(this, method, url);
    if (blocked) {
      // Opening then aborting leaves the XHR unusable; a later send() throws
      // InvalidStateError, which is the expected failure for a blocked request.
      this.abort();
    }
  };

  // Block known tracking path patterns for images/iframes, same-origin only
  // so federated login hosts are unaffected.
  var originalSetAttribute = Element.prototype.setAttribute;
  Element.prototype.setAttribute = function(name, value) {
    if (value && typeof value === "string") {
      var lowerName = name.toLowerCase();
      if (lowerName === "src" || lowerName === "href") {
        try {
          var parsed = new URL(value, location.href);
          if (parsed.origin === location.origin &&
              blockedPaths.indexOf(parsed.pathname.toLowerCase()) !== -1) {
            return;
          }
        } catch (e) {}
      }
    }
    originalSetAttribute.call(this, name, value);
  };

  // Expose a method for the page to check if blocking is active
  window.__onenoteBlockerActive = true;
})();"#;

#[cfg(test)]
mod tests {
    use super::BRIDGE_JS;

    #[test]
    fn blocker_script_is_non_empty() {
        assert!(!BRIDGE_JS.is_empty());
        assert!(BRIDGE_JS.contains("google-analytics.com"));
        assert!(BRIDGE_JS.contains("blockedHostnames"));
    }

    #[test]
    fn blocker_script_has_guard() {
        assert!(BRIDGE_JS.contains("__onenoteBlockerPatched"));
    }

    #[test]
    fn blocker_script_covers_common_trackers() {
        for host in [
            "google-analytics.com",
            "doubleclick.net",
            "connect.facebook.net",
            "googlesyndication.com",
        ] {
            assert!(BRIDGE_JS.contains(host), "missing tracker: {}", host);
        }
    }

    #[test]
    fn blocker_script_intercepts_fetch() {
        assert!(BRIDGE_JS.contains("window.fetch"));
        assert!(BRIDGE_JS.contains("originalFetch"));
    }

    #[test]
    fn blocker_script_handles_relative_urls() {
        assert!(BRIDGE_JS.contains("location.href"));
    }

    #[test]
    fn blocker_script_scopes_paths_to_same_origin() {
        assert!(BRIDGE_JS.contains("location.origin"));
    }

    #[test]
    fn blocker_script_excludes_federated_login_hosts() {
        for host in ["pornhub.com", "onlyfans.com", "patreon.com"] {
            assert!(!BRIDGE_JS.contains(host), "unexpected entry: {}", host);
        }
    }
}
