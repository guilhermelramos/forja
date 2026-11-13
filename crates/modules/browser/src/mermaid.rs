//! Offline Mermaid rendering and fenced-block extraction.
//!
//! The Mermaid 11.4.1 build is vendored under `assets/mermaid` (MIT, see
//! `LICENSE` and `VERSION`) and served through the WebView2 custom protocol, so
//! diagrams never require the network.

pub const VERSION: &str = include_str!("../assets/mermaid/VERSION");
pub const LICENSE: &str = include_str!("../assets/mermaid/LICENSE");
const BUNDLE: &str = include_str!("../assets/mermaid/mermaid.min.js");

pub fn bundled_js() -> &'static str {
    BUNDLE
}

pub fn license() -> &'static str {
    LICENSE
}

pub fn extract_blocks(markdown: &str) -> Vec<String> {
    let mut blocks = Vec::new();
    let mut current: Option<Vec<&str>> = None;
    for line in markdown.lines() {
        let trimmed = line.trim();
        match &mut current {
            None if trimmed.starts_with("```mermaid") => current = Some(Vec::new()),
            Some(lines) if trimmed.starts_with("```") => {
                blocks.push(lines.join("\n"));
                current = None;
            }
            Some(lines) => lines.push(line),
            None => {}
        }
    }
    blocks
}

pub fn preview_html(blocks: &[String]) -> String {
    let diagrams = blocks
        .iter()
        .map(|block| format!("<pre class=\"mermaid\">{}</pre>", escape(block)))
        .collect::<Vec<_>>()
        .join("\n");
    format!(
        "<!doctype html><html><head><meta charset=\"utf-8\"><title>Mermaid</title>\
         <style>html,body{{margin:0;background:#141210;color:#EDE7E1;font:14px system-ui;}}\
         body{{padding:16px;}}pre.mermaid{{background:#1B1816;border:1px solid #2E2925;\
         border-radius:6px;padding:12px;overflow:auto;}}</style></head><body>{diagrams}\
         <script src=\"http://forja.local/mermaid.min.js\"></script>\
         <script>mermaid.initialize({{startOnLoad:true,theme:'dark',securityLevel:'strict'}});</script>\
         </body></html>"
    )
}

fn escape(text: &str) -> String {
    text.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn only_mermaid_fences_are_extracted() {
        let markdown = "# T\n\n```mermaid\ngraph TD\nA-->B\n```\n\n```rust\nfn x() {}\n```";
        let blocks = extract_blocks(markdown);
        assert_eq!(blocks.len(), 1);
        assert!(blocks[0].contains("graph TD"));
        assert!(!blocks.join("\n").contains("fn x"));
    }

    #[test]
    fn preview_uses_the_vendored_bundle_and_version_is_known() {
        let html = preview_html(&["graph TD\nA-->B".into()]);
        assert!(html.contains("forja.local/mermaid.min.js"));
        assert!(html.contains("graph TD"));
        assert!(VERSION.trim().starts_with("11."));
        assert!(LICENSE.contains("MIT License"));
        assert!(bundled_js().starts_with("\"use strict\""));
    }

    #[test]
    fn bundled_version_and_license_are_known() {
        assert!(VERSION.trim().starts_with("11."));
        assert!(license().contains("MIT License"));
        assert!(bundled_js().starts_with("\"use strict\""));
    }
}
