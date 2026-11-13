//! File-to-viewer detection and the read-only document parsers.

use forja_contracts::{
    ImageView, MarkdownBlock, MarkdownView, NotebookCell, NotebookView, PdfView, TableView,
    TocEntry, ViewerDescriptor, ViewerDocument,
};
use pulldown_cmark::{BlockQuoteKind, CodeBlockKind, Event, Options, Parser, Tag, TagEnd};
use std::path::Path;

pub const MAX_MARKDOWN_BYTES: usize = 4 * 1024 * 1024;
pub const MAX_TABLE_ROWS: usize = 5_000;
pub const MAX_NOTEBOOK_BYTES: usize = 16 * 1024 * 1024;

pub fn descriptors() -> Vec<ViewerDescriptor> {
    vec![
        ViewerDescriptor {
            id: "markdown".to_owned(),
            title_key: "viewers.descriptor.markdown".to_owned(),
            extensions: vec!["md".into(), "markdown".into()],
        },
        ViewerDescriptor {
            id: "table".to_owned(),
            title_key: "viewers.descriptor.table".to_owned(),
            extensions: vec!["csv".into(), "tsv".into()],
        },
        ViewerDescriptor {
            id: "notebook".to_owned(),
            title_key: "viewers.descriptor.notebook".to_owned(),
            extensions: vec!["ipynb".into()],
        },
        ViewerDescriptor {
            id: "image".to_owned(),
            title_key: "viewers.descriptor.image".to_owned(),
            extensions: vec![
                "png".into(),
                "jpg".into(),
                "jpeg".into(),
                "gif".into(),
                "bmp".into(),
                "webp".into(),
                "ico".into(),
            ],
        },
        ViewerDescriptor {
            id: "pdf".to_owned(),
            title_key: "viewers.descriptor.pdf".to_owned(),
            extensions: vec!["pdf".into()],
        },
    ]
}

pub fn viewer_for(path: &str) -> Option<&'static str> {
    let extension = Path::new(path)
        .extension()
        .and_then(|extension| extension.to_str())?
        .to_ascii_lowercase();
    match extension.as_str() {
        "md" | "markdown" => Some("markdown"),
        "csv" | "tsv" => Some("table"),
        "ipynb" => Some("notebook"),
        "png" | "jpg" | "jpeg" | "gif" | "bmp" | "webp" | "ico" => Some("image"),
        "pdf" => Some("pdf"),
        _ => None,
    }
}

pub fn title(path: &str) -> String {
    Path::new(path)
        .file_name()
        .and_then(|name| name.to_str())
        .unwrap_or(path)
        .to_owned()
}

pub fn preview(path: &str) -> ViewerDocument {
    let viewer = viewer_for(path).unwrap_or("markdown");
    let mut document = ViewerDocument {
        viewer: viewer.to_owned(),
        path: path.to_owned(),
        title: title(path),
        kind: viewer.to_owned(),
        ..ViewerDocument::default()
    };
    let bytes = match std::fs::read(path) {
        Ok(bytes) => bytes,
        Err(error) => {
            document.error = Some(format!("could not read the file: {error}"));
            return document;
        }
    };
    match viewer {
        "markdown" => {
            let text = String::from_utf8_lossy(&bytes[..bytes.len().min(MAX_MARKDOWN_BYTES)]);
            document.markdown = Some(parse_markdown(&text));
        }
        "table" => {
            let text = String::from_utf8_lossy(&bytes[..bytes.len().min(MAX_MARKDOWN_BYTES)]);
            document.table = Some(parse_table(
                &text,
                if path.to_ascii_lowercase().ends_with(".tsv") {
                    '\t'
                } else {
                    ','
                },
            ));
        }
        "notebook" => {
            let text = String::from_utf8_lossy(&bytes[..bytes.len().min(MAX_NOTEBOOK_BYTES)]);
            match parse_notebook(&text) {
                Ok(notebook) => document.notebook = Some(notebook),
                Err(error) => document.error = Some(error),
            }
        }
        "image" => match image_view(path) {
            Ok(image) => document.image = Some(image),
            Err(error) => document.error = Some(error),
        },
        "pdf" => document.pdf = Some(PdfView {
            path: path.to_owned(),
            pages: pdf_pages(&bytes),
        }),
        _ => {}
    }
    document
}

fn callout_name(kind: BlockQuoteKind) -> &'static str {
    match kind {
        BlockQuoteKind::Note => "note",
        BlockQuoteKind::Tip => "tip",
        BlockQuoteKind::Important => "important",
        BlockQuoteKind::Warning => "warning",
        BlockQuoteKind::Caution => "caution",
    }
}

pub fn parse_markdown(source: &str) -> MarkdownView {
    let mut blocks = Vec::new();
    let mut toc = Vec::new();
    let mut heading: Option<(u8, String)> = None;
    let mut paragraph: Option<String> = None;
    let mut code: Option<(Option<String>, String)> = None;
    let mut quote: Option<(Option<&'static str>, String)> = None;
    let options = Options::ENABLE_GFM
        | Options::ENABLE_TABLES
        | Options::ENABLE_STRIKETHROUGH
        | Options::ENABLE_TASKLISTS
        | Options::ENABLE_FOOTNOTES;
    for event in Parser::new_ext(source, options) {
        match event {
            Event::Start(Tag::Heading { level, .. }) => {
                heading = Some((level as u8, String::new()));
            }
            Event::End(TagEnd::Heading(_)) => {
                if let Some((level, text)) = heading.take() {
                    let text = text.trim().to_owned();
                    if text.is_empty() {
                        continue;
                    }
                    if level <= 4 {
                        toc.push(TocEntry {
                            level,
                            text: text.clone(),
                        });
                    }
                    blocks.push(MarkdownBlock {
                        kind: "heading".to_owned(),
                        level,
                        text,
                        ..MarkdownBlock::default()
                    });
                }
            }
            Event::Start(Tag::CodeBlock(kind)) => {
                let language = match kind {
                    CodeBlockKind::Fenced(info) => Some(info.trim().to_owned()),
                    CodeBlockKind::Indented => None,
                }
                .filter(|language| !language.is_empty());
                code = Some((language, String::new()));
            }
            Event::End(TagEnd::CodeBlock) => {
                if let Some((language, text)) = code.take() {
                    blocks.push(MarkdownBlock {
                        kind: "code".to_owned(),
                        text: text.trim_end_matches('\n').to_owned(),
                        language,
                        ..MarkdownBlock::default()
                    });
                }
            }
            Event::Start(Tag::BlockQuote(kind)) => {
                quote = Some((kind.map(callout_name), String::new()));
            }
            Event::End(TagEnd::BlockQuote(_)) => {
                if let Some((kind, text)) = quote.take() {
                    let text = text.trim().to_owned();
                    if text.is_empty() {
                        continue;
                    }
                    let text = strip_callout_marker(text);
                    blocks.push(MarkdownBlock {
                        kind: if kind.is_some() { "callout" } else { "quote" }.to_owned(),
                        text,
                        callout: kind.map(str::to_owned),
                        ..MarkdownBlock::default()
                    });
                }
            }
            Event::Start(Tag::Paragraph) => {
                if quote.is_none() {
                    paragraph = Some(String::new());
                }
            }
            Event::End(TagEnd::Paragraph) => {
                if quote.is_none() {
                    if let Some(text) = paragraph.take() {
                        let text = text.trim().to_owned();
                        if !text.is_empty() {
                            blocks.push(MarkdownBlock {
                                kind: "paragraph".to_owned(),
                                text,
                                ..MarkdownBlock::default()
                            });
                        }
                    }
                }
            }
            Event::Start(Tag::Item) => {
                paragraph = Some(String::new());
            }
            Event::End(TagEnd::Item) => {
                if let Some(text) = paragraph.take() {
                    let text = text.trim().to_owned();
                    if !text.is_empty() {
                        blocks.push(MarkdownBlock {
                            kind: "list_item".to_owned(),
                            text,
                            ..MarkdownBlock::default()
                        });
                    }
                }
            }
            Event::Text(text) => append(&mut heading, &mut quote, &mut code, &mut paragraph, &text),
            Event::Code(text) | Event::InlineMath(text) | Event::DisplayMath(text) => {
                append(
                    &mut heading,
                    &mut quote,
                    &mut code,
                    &mut paragraph,
                    &format!("`{text}`"),
                );
            }
            Event::SoftBreak => append(
                &mut heading,
                &mut quote,
                &mut code,
                &mut paragraph,
                " ",
            ),
            Event::HardBreak => append(
                &mut heading,
                &mut quote,
                &mut code,
                &mut paragraph,
                "\n",
            ),
            Event::Rule => blocks.push(MarkdownBlock {
                kind: "rule".to_owned(),
                ..MarkdownBlock::default()
            }),
            Event::Html(text) | Event::InlineHtml(text) => {
                append(&mut heading, &mut quote, &mut code, &mut paragraph, &text)
            }
            _ => {}
        }
    }
    MarkdownView { blocks, toc }
}

fn append(
    heading: &mut Option<(u8, String)>,
    quote: &mut Option<(Option<&'static str>, String)>,
    code: &mut Option<(Option<String>, String)>,
    paragraph: &mut Option<String>,
    text: &str,
) {
    if let Some((_, buffer)) = code {
        buffer.push_str(text);
    } else if let Some((_, buffer)) = quote {
        buffer.push_str(text);
    } else if let Some((_, buffer)) = heading {
        buffer.push_str(text);
    } else if let Some(buffer) = paragraph {
        buffer.push_str(text);
    }
}

fn strip_callout_marker(text: String) -> String {
    let mut lines = text.lines();
    if let Some(first) = lines.next() {
        if first.trim_start().starts_with("[!") {
            let rest = first
                .split_once(']')
                .map(|(_, rest)| rest.trim())
                .unwrap_or("");
            let mut result = rest.to_owned();
            for line in lines {
                if !result.is_empty() {
                    result.push('\n');
                }
                result.push_str(line);
            }
            return result;
        }
    }
    text
}

pub fn parse_table(source: &str, delimiter: char) -> TableView {
    let mut rows = Vec::new();
    let mut row = Vec::new();
    let mut field = String::new();
    let mut quoted = false;
    let mut chars = source.chars().peekable();
    while let Some(character) = chars.next() {
        match character {
            '"' if quoted && chars.peek() == Some(&'"') => {
                field.push('"');
                chars.next();
            }
            '"' => quoted = !quoted,
            character if character == delimiter && !quoted => {
                row.push(std::mem::take(&mut field));
            }
            '\n' if !quoted => {
                if field.ends_with('\r') {
                    field.pop();
                }
                row.push(std::mem::take(&mut field));
                if row.iter().any(|cell| !cell.is_empty()) {
                    rows.push(std::mem::take(&mut row));
                } else {
                    row.clear();
                }
            }
            other => field.push(other),
        }
    }
    if !field.is_empty() || !row.is_empty() {
        row.push(field);
        rows.push(row);
    }
    let headers = rows.first().cloned().unwrap_or_default();
    let mut body: Vec<Vec<String>> = rows.into_iter().skip(1).collect();
    let truncated = body.len() > MAX_TABLE_ROWS;
    body.truncate(MAX_TABLE_ROWS);
    TableView {
        headers,
        rows: body,
        truncated,
        delimiter: delimiter.to_string(),
    }
}

pub fn parse_notebook(source: &str) -> Result<NotebookView, String> {
    let value: serde_json::Value =
        serde_json::from_str(source).map_err(|error| format!("notebook is not valid JSON: {error}"))?;
    let language = value
        .pointer("/metadata/kernelspec/language")
        .or_else(|| value.pointer("/metadata/language_info/name"))
        .and_then(serde_json::Value::as_str)
        .unwrap_or("python")
        .to_owned();
    let cells = value
        .get("cells")
        .and_then(serde_json::Value::as_array)
        .cloned()
        .unwrap_or_default();
    let mut result = Vec::new();
    for cell in cells {
        let kind = cell
            .get("cell_type")
            .and_then(serde_json::Value::as_str)
            .unwrap_or("raw")
            .to_owned();
        let source = join_source(cell.get("source"));
        let mut outputs = Vec::new();
        if let Some(list) = cell.get("outputs").and_then(serde_json::Value::as_array) {
            for output in list {
                let text = join_source(output.get("text"));
                if !text.is_empty() {
                    outputs.push(text);
                    continue;
                }
                if let Some(text) = output
                    .pointer("/data/text~1plain")
                    .and_then(serde_json::Value::as_str)
                {
                    outputs.push(text.to_owned());
                    continue;
                }
                if let Some(error) = output.get("ename").and_then(serde_json::Value::as_str) {
                    outputs.push(format!(
                        "{}: {}",
                        error,
                        output
                            .get("evalue")
                            .and_then(serde_json::Value::as_str)
                            .unwrap_or("")
                    ));
                }
            }
        }
        result.push(NotebookCell {
            kind,
            source,
            outputs,
        });
    }
    Ok(NotebookView {
        cells: result,
        language,
    })
}

fn join_source(value: Option<&serde_json::Value>) -> String {
    match value {
        Some(serde_json::Value::String(text)) => text.clone(),
        Some(serde_json::Value::Array(lines)) => lines
            .iter()
            .filter_map(serde_json::Value::as_str)
            .collect::<Vec<_>>()
            .join(""),
        _ => String::new(),
    }
}

pub struct Frame {
    pub width: usize,
    pub height: usize,
    pub rgba: Vec<u8>,
    pub delay: f32,
}

/// Decodes an image (or every GIF frame) off the UI thread. The view uploads
/// the returned buffers as textures.
pub fn load_frames(path: &str) -> Result<Vec<Frame>, String> {
    use image::AnimationDecoder;
    let file = std::fs::File::open(path).map_err(|error| error.to_string())?;
    if path.to_ascii_lowercase().ends_with(".gif") {
        let decoder = image::codecs::gif::GifDecoder::new(std::io::BufReader::new(file))
            .map_err(|error| error.to_string())?;
        let mut frames = Vec::new();
        for frame in decoder.into_frames().take(120) {
            let frame = frame.map_err(|error| error.to_string())?;
            let (numerator, denominator) = frame.delay().numer_denom_ms();
            let delay = if denominator == 0 {
                0.1
            } else {
                numerator as f32 / denominator as f32 / 1000.0
            };
            let buffer = frame.into_buffer();
            frames.push(Frame {
                width: buffer.width() as usize,
                height: buffer.height() as usize,
                rgba: buffer.into_raw(),
                delay: delay.max(0.02),
            });
        }
        if frames.is_empty() {
            return Err("empty GIF".to_owned());
        }
        return Ok(frames);
    }
    let decoded = image::load_from_memory(
        &std::fs::read(path).map_err(|error| error.to_string())?,
    )
    .map_err(|error| error.to_string())?
    .to_rgba8();
    Ok(vec![Frame {
        width: decoded.width() as usize,
        height: decoded.height() as usize,
        rgba: decoded.into_raw(),
        delay: 0.0,
    }])
}

fn image_view(path: &str) -> Result<ImageView, String> {
    let (width, height) =
        image::image_dimensions(path).map_err(|error| format!("could not read image: {error}"))?;
    let frames = if path.to_ascii_lowercase().ends_with(".gif") {
        image::codecs::gif::GifDecoder::new(std::io::BufReader::new(
            std::fs::File::open(path).map_err(|error| error.to_string())?,
        ))
        .map(|decoder| {
            use image::AnimationDecoder;
            decoder.into_frames().take(500).count()
        })
        .unwrap_or(1)
    } else {
        1
    };
    Ok(ImageView {
        path: path.to_owned(),
        width,
        height,
        frames,
    })
}

pub fn pdf_pages(bytes: &[u8]) -> u32 {
    let text = String::from_utf8_lossy(bytes);
    let mut count = 0u32;
    let mut rest = text.as_ref();
    while let Some(index) = rest.find("/Type") {
        rest = &rest[index + 5..];
        let tail = rest.trim_start();
        if let Some(after) = tail.strip_prefix("/Page") {
            if !after.starts_with('s') {
                count = count.saturating_add(1);
            }
        }
    }
    count.max(1)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn callouts_and_toc_are_parsed() {
        let view =
            parse_markdown("# Título\n\n> [!WARNING]\n> Cuidado com o passo.\n\n## Fim\n");
        assert_eq!(view.toc.len(), 2);
        assert_eq!(view.toc[0].text, "Título");
        assert_eq!(view.toc[1].level, 2);
        let callout = view
            .blocks
            .iter()
            .find(|block| block.kind == "callout")
            .expect("callout block");
        assert_eq!(callout.callout.as_deref(), Some("warning"));
        assert!(callout.text.contains("Cuidado"));
    }

    #[test]
    fn table_handles_quotes_and_tsv() {
        let table = parse_table("nome,idade\n\"Ana, A\",30\nBo,25\n", ',');
        assert_eq!(table.headers, vec!["nome", "idade"]);
        assert_eq!(table.rows[0][0], "Ana, A");
        let tsv = parse_table("a\tb\n1\t2", '\t');
        assert_eq!(tsv.headers, vec!["a", "b"]);
    }

    #[test]
    fn notebook_reads_streams_and_plain_text() {
        let source = r#"{"metadata":{"language_info":{"name":"rust"}},"cells":[{"cell_type":"code","source":["fn main() {}"],"outputs":[{"output_type":"stream","text":"ok\n"}]}]}"#;
        let notebook = parse_notebook(source).unwrap();
        assert_eq!(notebook.language, "rust");
        assert_eq!(notebook.cells[0].source, "fn main() {}");
        assert_eq!(notebook.cells[0].outputs[0], "ok\n");
    }

    #[test]
    fn extension_detection_is_case_insensitive() {
        assert_eq!(viewer_for("C:/a.MD"), Some("markdown"));
        assert_eq!(viewer_for("C:/a.ipynb"), Some("notebook"));
        assert_eq!(viewer_for("C:/a.zip"), None);
    }

    #[test]
    fn pdf_page_count_ignores_pages_tree() {
        let bytes = b"%PDF /Type /Pages 2 /Type /Page /Type /Page";
        assert_eq!(pdf_pages(bytes), 2);
    }
}
