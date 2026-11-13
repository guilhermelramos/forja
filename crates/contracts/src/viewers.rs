//! Viewer contract: extension registry and the read-only document model used
//! by the `viewers` module and by the editor preview tabs.

use serde::{Deserialize, Serialize};

/// One viewer contribution. `extensions` are lowercase without the dot.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct ViewerDescriptor {
    pub id: String,
    pub title_key: String,
    pub extensions: Vec<String>,
}

#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct TocEntry {
    pub level: u8,
    pub text: String,
}

/// One block of the read-only Markdown model. `kind` is `heading`,
/// `paragraph`, `code`, `callout`, `quote`, `list_item` or `rule`.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct MarkdownBlock {
    pub kind: String,
    #[serde(default)]
    pub level: u8,
    #[serde(default)]
    pub text: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub language: Option<String>,
    /// `note`, `tip`, `important`, `warning` or `caution` for callouts.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub callout: Option<String>,
}

#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct MarkdownView {
    pub blocks: Vec<MarkdownBlock>,
    pub toc: Vec<TocEntry>,
}

#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct TableView {
    pub headers: Vec<String>,
    pub rows: Vec<Vec<String>>,
    pub truncated: bool,
    pub delimiter: String,
}

#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct NotebookCell {
    /// `markdown`, `code` or `raw`.
    pub kind: String,
    #[serde(default)]
    pub source: String,
    #[serde(default)]
    pub outputs: Vec<String>,
}

#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct NotebookView {
    pub cells: Vec<NotebookCell>,
    pub language: String,
}

#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct ImageView {
    pub path: String,
    pub width: u32,
    pub height: u32,
    pub frames: usize,
}

#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct PdfView {
    pub path: String,
    pub pages: u32,
}

/// The result of `viewers.preview`. Exactly one of the payload fields is set
/// according to `kind`; `error` is set when the file could not be read.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct ViewerDocument {
    pub viewer: String,
    pub path: String,
    pub title: String,
    /// `markdown`, `table`, `notebook`, `image` or `pdf`.
    pub kind: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub markdown: Option<MarkdownView>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub table: Option<TableView>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub notebook: Option<NotebookView>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub image: Option<ImageView>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub pdf: Option<PdfView>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub error: Option<String>,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn viewer_document_roundtrips() {
        let doc = ViewerDocument {
            viewer: "markdown".into(),
            path: "C:/a.md".into(),
            title: "a.md".into(),
            kind: "markdown".into(),
            markdown: Some(MarkdownView {
                blocks: vec![MarkdownBlock {
                    kind: "callout".into(),
                    text: "olá".into(),
                    callout: Some("note".into()),
                    ..MarkdownBlock::default()
                }],
                toc: vec![TocEntry {
                    level: 1,
                    text: "Título".into(),
                }],
            }),
            ..ViewerDocument::default()
        };
        let json = serde_json::to_string(&doc).unwrap();
        let back: ViewerDocument = serde_json::from_str(&json).unwrap();
        assert_eq!(back, doc);
    }
}
