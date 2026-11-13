//! Sign-in flows that `browser.auth.open_external` sends to the system browser.
//!
//! The rule is a heuristic over the address: a sign-in host (`login.`, `accounts.`,
//! `auth.`, `sso.`) or a sign-in path (`/login`, `/signin`, `/oauth`, `/auth/`,
//! `/sso`). It only decides where the flow opens; it never reads or copies cookies.

/// True when `url` looks like the start of a sign-in flow.
pub fn is_auth_flow(url: &str) -> bool {
    let Some(rest) = url
        .strip_prefix("https://")
        .or_else(|| url.strip_prefix("http://"))
    else {
        return false;
    };
    let (host, path) = match rest.find('/') {
        Some(index) => (&rest[..index], &rest[index..]),
        None => (rest, "/"),
    };
    let host = host.split(':').next().unwrap_or(host).to_ascii_lowercase();
    let path = path.split(['?', '#']).next().unwrap_or("").to_ascii_lowercase();
    const HOST_PREFIXES: [&str; 4] = ["login.", "accounts.", "auth.", "sso."];
    const PATH_MARKERS: [&str; 5] = ["/login", "/signin", "/oauth", "/auth/", "/sso"];
    HOST_PREFIXES.iter().any(|prefix| host.starts_with(prefix))
        || PATH_MARKERS.iter().any(|marker| path.starts_with(marker))
}

/// Script that makes `navigator.doNotTrack` report `1`. The WebView2 control does
/// not expose the request header through wry, so only pages that read the property
/// see the signal.
pub const DO_NOT_TRACK_SCRIPT: &str =
    "Object.defineProperty(Navigator.prototype, 'doNotTrack', { get: () => '1' });";

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sign_in_hosts_and_paths_are_auth_flows() {
        assert!(is_auth_flow("https://login.example.test/start"));
        assert!(is_auth_flow("https://accounts.example.test/"));
        assert!(is_auth_flow("https://example.test/oauth/authorize?x=1"));
        assert!(is_auth_flow("https://example.test/Login"));
        assert!(!is_auth_flow("https://example.test/docs/logins"));
        assert!(!is_auth_flow("https://example.test/"));
        assert!(!is_auth_flow("forja://inicio"));
    }

    #[test]
    fn do_not_track_script_sets_the_property() {
        assert!(DO_NOT_TRACK_SCRIPT.contains("doNotTrack"));
        assert!(DO_NOT_TRACK_SCRIPT.contains("'1'"));
    }
}
