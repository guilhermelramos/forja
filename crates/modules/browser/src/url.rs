//! Address-bar input normalization and search engines.

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Target {
    Url(String),
    Search { engine: String, url: String },
}

impl Target {
    pub fn url(&self) -> &str {
        match self {
            Self::Url(url) => url,
            Self::Search { url, .. } => url,
        }
    }
}

pub const ENGINES: &[(&str, &str)] = &[
    ("duckduckgo", "https://duckduckgo.com/?q="),
    ("bing", "https://www.bing.com/search?q="),
    ("google", "https://www.google.com/search?q="),
];

fn engine_base(engine: &str) -> &'static str {
    ENGINES
        .iter()
        .find(|(id, _)| *id == engine)
        .map(|(_, base)| *base)
        .unwrap_or(ENGINES[0].1)
}

/// A URL passes through; a bare host gets a scheme (localhost gets HTTP);
/// anything else becomes a search in the configured engine.
pub fn normalize(input: &str, engine: &str) -> Option<Target> {
    let input = input.trim();
    if input.is_empty() {
        return None;
    }
    if let Some(rest) = input.strip_prefix("forja:") {
        return Some(Target::Url(format!("forja:{rest}")));
    }
    if input.starts_with("http://")
        || input.starts_with("https://")
        || input.starts_with("file://")
        || input.starts_with("about:")
        || input.starts_with("data:")
    {
        return Some(Target::Url(input.to_owned()));
    }
    let host_part = input.split(['/', '?', '#']).next().unwrap_or(input);
    let looks_like_host = !input.contains(char::is_whitespace)
        && (host_part == "localhost"
            || host_part.starts_with("localhost:")
            || host_part.parse::<std::net::IpAddr>().is_ok()
            || host_part
                .split(':')
                .next()
                .is_some_and(|host| host.contains('.') && !host.starts_with('.')));
    if looks_like_host {
        let scheme = if host_part == "localhost"
            || host_part.starts_with("localhost:")
            || host_part.starts_with("127.")
            || host_part.starts_with("[::1]")
        {
            "http://"
        } else {
            "https://"
        };
        return Some(Target::Url(format!("{scheme}{input}")));
    }
    let engine = if engine.trim().is_empty() {
        "duckduckgo"
    } else {
        engine
    };
    Some(Target::Search {
        engine: engine.to_owned(),
        url: format!("{}{}", engine_base(engine), encode(input)),
    })
}

pub fn encode(text: &str) -> String {
    let mut output = String::new();
    for byte in text.as_bytes() {
        match byte {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'_' | b'.' | b'~' => {
                output.push(*byte as char)
            }
            b' ' => output.push('+'),
            other => output.push_str(&format!("%{other:02X}")),
        }
    }
    output
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn urls_hosts_and_localhost_get_the_right_scheme() {
        assert_eq!(
            normalize("https://example.test/a", "duckduckgo"),
            Some(Target::Url("https://example.test/a".into()))
        );
        assert_eq!(
            normalize("example.test/docs", "duckduckgo"),
            Some(Target::Url("https://example.test/docs".into()))
        );
        assert_eq!(
            normalize("localhost:3000", "duckduckgo"),
            Some(Target::Url("http://localhost:3000".into()))
        );
        assert_eq!(
            normalize("127.0.0.1:8080", "duckduckgo"),
            Some(Target::Url("http://127.0.0.1:8080".into()))
        );
        assert_eq!(
            normalize("file:///C:/tmp/x.html", "duckduckgo"),
            Some(Target::Url("file:///C:/tmp/x.html".into()))
        );
    }

    #[test]
    fn phrases_search_in_the_selected_engine() {
        let target = normalize("hello world", "bing").unwrap();
        assert_eq!(target.url(), "https://www.bing.com/search?q=hello+world");
        let fallback = normalize("hello", "unknown").unwrap();
        assert!(fallback.url().starts_with("https://duckduckgo.com/?q="));
    }

    #[test]
    fn empty_input_returns_none() {
        assert!(normalize("   ", "google").is_none());
    }
}
