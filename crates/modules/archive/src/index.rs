//! Índice tantivy do Acervo.

use crate::parse::{self, ArchiveSession, SessionFormat};
use serde::Serialize;
use std::collections::{BTreeMap, BTreeSet};
use std::path::{Path, PathBuf};
use std::time::UNIX_EPOCH;
use tantivy::collector::TopDocs;
use tantivy::query::{AllQuery, Query, QueryParser};
use tantivy::schema::{
    Field, IndexRecordOption, Schema, TextFieldIndexing, TextOptions, Value as _, INDEXED, STORED,
    STRING, TEXT,
};
use tantivy::snippet::SnippetGenerator;
use tantivy::{Index, IndexWriter, TantivyDocument, Term};

/// Filtros da busca.
#[derive(Clone, Debug, Default)]
pub struct Filters {
    /// Id exato do perfil.
    pub profile_id: Option<String>,
    /// Projeto contido no texto.
    pub project: Option<String>,
    /// Início mínimo em ms.
    pub since_ms: Option<i64>,
    /// Início máximo em ms.
    pub until_ms: Option<i64>,
}

/// Resultado da busca.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize)]
pub struct Hit {
    /// Sessão.
    pub session_id: String,
    /// Perfil.
    pub profile_id: String,
    /// Projeto.
    pub project: String,
    /// Arquivo.
    pub path: String,
    /// Início.
    pub started_ms: i64,
    /// Atualização.
    pub updated_ms: i64,
    /// Título.
    pub title: String,
    /// Trecho.
    pub snippet: String,
    /// Pontuação.
    pub score: u64,
}

/// Relatório de indexação.
#[derive(Clone, Debug, Default, Serialize)]
pub struct Report {
    /// Arquivos descobertos.
    pub discovered: usize,
    /// Arquivos analisados.
    pub parsed: usize,
    /// Arquivos removidos do índice.
    pub removed: usize,
    /// Erros.
    pub errors: Vec<String>,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
struct Fingerprint {
    len: u64,
    modified_ms: u128,
}

#[derive(Clone, Debug)]
struct Entry {
    fingerprint: Fingerprint,
    session: ArchiveSession,
}

/// Cache incremental entre varreduras.
#[derive(Clone, Debug, Default)]
pub struct SessionCache {
    files: BTreeMap<PathBuf, Entry>,
    keep_text: Option<bool>,
}

impl SessionCache {
    /// Sessões conhecidas, da mais recente para a mais antiga.
    pub fn sessions(&self) -> Vec<ArchiveSession> {
        let mut sessions: Vec<ArchiveSession> = self
            .files
            .values()
            .map(|entry| entry.session.clone())
            .collect();
        sessions.sort_by_key(|session| std::cmp::Reverse(session.updated_ms));
        sessions
    }

    /// Quantidade de arquivos.
    pub fn file_count(&self) -> usize {
        self.files.len()
    }

    /// Esquece um arquivo removido.
    pub fn forget(&mut self, path: &Path) {
        self.files.remove(path);
    }
}

struct ArchiveSchema {
    doc_key: Field,
    session_id: Field,
    helper_id: Field,
    project: Field,
    path: Field,
    started: Field,
    updated: Field,
    title: Field,
    text: Field,
}

/// Índice aberto.
pub struct ArchiveIndex {
    index: Index,
    schema: ArchiveSchema,
}

impl ArchiveIndex {
    /// Abre ou recria o índice.
    pub fn open(dir: &Path) -> Result<Self, String> {
        std::fs::create_dir_all(dir).map_err(|error| error.to_string())?;
        let expected = schema();
        let index = match Index::open_in_dir(dir) {
            Ok(index) if schema_matches(&index) => index,
            _ => {
                let _ = std::fs::remove_dir_all(dir);
                std::fs::create_dir_all(dir).map_err(|error| error.to_string())?;
                Index::create_in_dir(dir, expected).map_err(|error| error.to_string())?
            }
        };
        let fields = ArchiveSchema {
            doc_key: field(&index, "doc_key")?,
            session_id: field(&index, "session_id")?,
            helper_id: field(&index, "helper_id")?,
            project: field(&index, "project")?,
            path: field(&index, "path")?,
            started: field(&index, "started")?,
            updated: field(&index, "updated")?,
            title: field(&index, "title")?,
            text: field(&index, "text")?,
        };
        Ok(Self {
            index,
            schema: fields,
        })
    }

    /// Atualiza o índice de forma incremental.
    pub fn scan(
        &self,
        cache: &mut SessionCache,
        files: &[(String, PathBuf)],
        keep_text: bool,
    ) -> Result<Report, String> {
        let mut report = Report::default();
        let policy_changed = cache.keep_text != Some(keep_text);
        let mut present = BTreeSet::new();
        let mut changed: Vec<(PathBuf, ArchiveSession)> = Vec::new();
        for (helper_id, path) in files {
            present.insert(path.clone());
            report.discovered += 1;
            let Ok(metadata) = std::fs::metadata(path) else {
                continue;
            };
            let fingerprint = Fingerprint {
                len: metadata.len(),
                modified_ms: metadata
                    .modified()
                    .ok()
                    .and_then(|time| time.duration_since(UNIX_EPOCH).ok())
                    .map(|duration| duration.as_millis())
                    .unwrap_or(0),
            };
            if !policy_changed && cache
                .files
                .get(path)
                .is_some_and(|entry| entry.fingerprint == fingerprint)
            {
                continue;
            }
            let Some(format) = format_for(helper_id) else {
                report
                    .errors
                    .push(format!("{helper_id}: formato sem parser"));
                continue;
            };
            match parse::parse_file(format, helper_id, path) {
                Ok(Some(session)) => changed.push((path.clone(), session)),
                Ok(None) => report
                    .errors
                    .push(format!("{}: sem texto legível", path.display())),
                Err(error) => report.errors.push(format!("{}: {error}", path.display())),
            }
        }
        let removed: Vec<PathBuf> = cache
            .files
            .keys()
            .filter(|path| !present.contains(*path))
            .cloned()
            .collect();
        report.removed = removed.len();
        if changed.is_empty() && removed.is_empty() && !policy_changed {
            cache.keep_text = Some(keep_text);
            return Ok(report);
        }
        let mut writer: IndexWriter = self
            .index
            .writer(50_000_000)
            .map_err(|error| error.to_string())?;
        if policy_changed {
            writer.delete_all_documents().map_err(|error| error.to_string())?;
        }
        for path in &removed {
            if let Some(entry) = cache.files.get(path) {
                writer.delete_term(self.doc_key_term(&entry.session, path));
            }
        }
        for (path, session) in &changed {
            writer.delete_term(self.doc_key_term(session, path));
            writer
                .add_document(self.document(session, path, keep_text))
                .map_err(|error| error.to_string())?;
        }
        writer.commit().map_err(|error| error.to_string())?;
        report.parsed = changed.len();
        if policy_changed {
            cache.files.clear();
        }
        for path in removed {
            cache.files.remove(&path);
        }
        for (path, mut session) in changed {
            let metadata = std::fs::metadata(&path).ok();
            let fingerprint = Fingerprint {
                len: metadata
                    .as_ref()
                    .map(|metadata| metadata.len())
                    .unwrap_or(0),
                modified_ms: metadata
                    .and_then(|metadata| metadata.modified().ok())
                    .and_then(|time| time.duration_since(UNIX_EPOCH).ok())
                    .map(|duration| duration.as_millis())
                    .unwrap_or(0),
            };
            if !keep_text {
                session.text.clear();
            }
            cache.files.insert(path, Entry { fingerprint, session });
        }
        cache.keep_text = Some(keep_text);
        Ok(report)
    }

    /// Busca com filtros.
    pub fn search(
        &self,
        query_text: &str,
        filters: &Filters,
        limit: usize,
    ) -> Result<Vec<Hit>, String> {
        if limit == 0 {
            return Ok(Vec::new());
        }
        let reader = self.index.reader().map_err(|error| error.to_string())?;
        let searcher = reader.searcher();
        let trimmed = query_text.trim();
        let query: Box<dyn Query> = if trimmed.is_empty() {
            Box::new(AllQuery)
        } else {
            let parser =
                QueryParser::for_index(&self.index, vec![self.schema.title, self.schema.text]);
            Box::new(
                parser
                    .parse_query(trimmed)
                    .map_err(|error| format!("invalid query: {error}"))?,
            )
        };
        let snippet_generator =
            SnippetGenerator::create(&searcher, &*query, self.schema.text).ok();
        let top = searcher
            .search(
                &*query,
                &TopDocs::with_limit(limit.saturating_mul(8).max(limit)).order_by_score(),
            )
            .map_err(|error| error.to_string())?;
        let mut hits: Vec<Hit> = Vec::new();
        let mut seen = BTreeSet::new();
        for (score, address) in top {
            let document: TantivyDocument =
                searcher.doc(address).map_err(|error| error.to_string())?;
            let profile_id = text_field(&document, self.schema.helper_id);
            let session_id = text_field(&document, self.schema.session_id);
            if !seen.insert((profile_id.clone(), session_id.clone())) {
                continue;
            }
            let project = text_field(&document, self.schema.project);
            let started_ms = i64_field(&document, self.schema.started);
            if !filters
                .profile_id
                .as_deref()
                .is_none_or(|filter| filter == profile_id)
            {
                continue;
            }
            if let Some(needle) = &filters.project {
                if !needle.is_empty()
                    && !project
                        .to_ascii_lowercase()
                        .contains(&needle.to_ascii_lowercase())
                {
                    continue;
                }
            }
            if filters.since_ms.is_some_and(|since| started_ms < since) {
                continue;
            }
            if filters.until_ms.is_some_and(|until| started_ms > until) {
                continue;
            }
            let snippet = snippet_generator
                .as_ref()
                .map(|generator| {
                    generator
                        .snippet_from_doc(&document)
                        .fragment()
                        .chars()
                        .take(240)
                        .collect::<String>()
                })
                .filter(|snippet| !snippet.is_empty())
                .unwrap_or_else(|| {
                    text_field(&document, self.schema.text)
                        .chars()
                        .take(240)
                        .collect()
                });
            hits.push(Hit {
                session_id,
                profile_id,
                project,
                path: text_field(&document, self.schema.path),
                started_ms,
                updated_ms: i64_field(&document, self.schema.updated),
                title: text_field(&document, self.schema.title),
                snippet,
                score: score.to_bits() as u64,
            });
            if hits.len() >= limit {
                break;
            }
        }
        Ok(hits)
    }

    /// Remove um arquivo do índice.
    pub fn delete_path(&self, path: &Path) -> Result<(), String> {
        let mut writer: IndexWriter = self
            .index
            .writer(50_000_000)
            .map_err(|error| error.to_string())?;
        writer.delete_term(Term::from_field_text(
            self.schema.path,
            &path.display().to_string(),
        ));
        writer.commit().map_err(|error| error.to_string())?;
        Ok(())
    }

    fn doc_key_term(&self, session: &ArchiveSession, path: &Path) -> Term {
        Term::from_field_text(
            self.schema.doc_key,
            &format!("{}|{}", session.helper_id, path.display()),
        )
    }

    fn document(
        &self,
        session: &ArchiveSession,
        path: &Path,
        keep_text: bool,
    ) -> TantivyDocument {
        let mut document = TantivyDocument::default();
        document.add_text(
            self.schema.doc_key,
            format!("{}|{}", session.helper_id, path.display()),
        );
        document.add_text(self.schema.session_id, &session.session_id);
        document.add_text(self.schema.helper_id, &session.helper_id);
        document.add_text(self.schema.project, &session.project);
        document.add_text(self.schema.path, path.display().to_string());
        document.add_i64(self.schema.started, session.started_ms);
        document.add_i64(self.schema.updated, session.updated_ms);
        document.add_text(self.schema.title, &session.title);
        document.add_text(
            self.schema.text,
            if keep_text { &session.text } else { "" },
        );
        document
    }
}

fn format_for(helper_id: &str) -> Option<SessionFormat> {
    match helper_id {
        "claude-code" => Some(SessionFormat::Claude),
        "codex" => Some(SessionFormat::Codex),
        "opencode" => Some(SessionFormat::OpenCode),
        _ => None,
    }
}

fn schema() -> Schema {
    let mut builder = Schema::builder();
    builder.add_text_field("doc_key", STRING | STORED);
    builder.add_text_field("session_id", STRING | STORED);
    builder.add_text_field("helper_id", STRING | STORED);
    builder.add_text_field("project", STRING | STORED);
    builder.add_text_field("path", STRING | STORED);
    builder.add_i64_field("started", INDEXED | STORED);
    builder.add_i64_field("updated", INDEXED | STORED);
    builder.add_text_field("title", TEXT | STORED);
    let text_indexing = TextFieldIndexing::default()
        .set_tokenizer("default")
        .set_index_option(IndexRecordOption::WithFreqsAndPositions);
    let text_options = TextOptions::default()
        .set_indexing_options(text_indexing)
        .set_stored();
    builder.add_text_field("text", text_options);
    builder.build()
}

fn schema_matches(index: &Index) -> bool {
    let schema = index.schema();
    [
        "doc_key",
        "session_id",
        "helper_id",
        "project",
        "path",
        "started",
        "updated",
        "title",
        "text",
    ]
    .iter()
    .all(|name| schema.get_field(name).is_ok())
}

fn field(index: &Index, name: &str) -> Result<Field, String> {
    index
        .schema()
        .get_field(name)
        .map_err(|error| format!("índice do Acervo sem {name}: {error}"))
}

fn text_field(document: &TantivyDocument, field: Field) -> String {
    document
        .get_first(field)
        .and_then(|value| value.as_str())
        .unwrap_or("")
        .to_owned()
}

fn i64_field(document: &TantivyDocument, field: Field) -> i64 {
    document
        .get_first(field)
        .and_then(|value| value.as_i64())
        .unwrap_or(0)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;

    struct Fixture {
        dir: PathBuf,
        files: Vec<(String, PathBuf)>,
    }

    impl Drop for Fixture {
        fn drop(&mut self) {
            let _ = std::fs::remove_dir_all(&self.dir);
        }
    }

    fn fixture() -> Fixture {
        // Tests run in parallel and the clock can repeat: a counter keeps dirs unique.
        static NEXT: std::sync::atomic::AtomicUsize = std::sync::atomic::AtomicUsize::new(0);
        let dir = std::env::temp_dir().join(format!(
            "forja-archive-index-{}-{}-{}",
            std::process::id(),
            NEXT.fetch_add(1, std::sync::atomic::Ordering::Relaxed),
            std::time::SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        let claude = dir.join("claude").join("session.jsonl");
        let codex = dir.join("codex").join("rollout-session.jsonl");
        let opencode = dir.join("opencode").join("session.json");
        for (path, content) in [
            (
                &claude,
                include_str!("../fixtures/claude-session.jsonl"),
            ),
            (&codex, include_str!("../fixtures/codex-session.jsonl")),
            (
                &opencode,
                include_str!("../fixtures/opencode-session.json"),
            ),
        ] {
            std::fs::create_dir_all(path.parent().unwrap()).unwrap();
            std::fs::write(path, content).unwrap();
        }
        Fixture {
            files: vec![
                ("claude-code".to_owned(), claude),
                ("codex".to_owned(), codex),
                ("opencode".to_owned(), opencode),
            ],
            dir,
        }
    }

    #[test]
    fn scans_incrementally_and_searches_with_filters() {
        let fixture = fixture();
        let index = ArchiveIndex::open(&fixture.dir.join("index")).unwrap();
        let mut cache = SessionCache::default();
        let report = index
            .scan(&mut cache, &fixture.files, true)
            .expect("first scan");
        assert_eq!(report.discovered, 3);
        assert_eq!(report.parsed, 3);
        assert_eq!(cache.file_count(), 3);

        let report = index
            .scan(&mut cache, &fixture.files, true)
            .expect("second scan");
        assert_eq!(report.parsed, 0);

        let hits = index
            .search("parser", &Filters::default(), 10)
            .expect("search claude");
        assert_eq!(hits.len(), 1);
        assert_eq!(hits[0].profile_id, "claude-code");
        assert!(hits[0].snippet.to_ascii_lowercase().contains("parser"));

        let hits = index
            .search("painel", &Filters::default(), 10)
            .expect("search codex");
        assert_eq!(hits.len(), 1);
        assert_eq!(hits[0].profile_id, "codex");

        let hits = index
            .search("tabela", &Filters::default(), 10)
            .expect("search opencode");
        assert_eq!(hits.len(), 1);
        assert_eq!(hits[0].profile_id, "opencode");

        let hits = index
            .search(
                "",
                &Filters {
                    profile_id: Some("codex".to_owned()),
                    ..Filters::default()
                },
                10,
            )
            .expect("filter by helper");
        assert_eq!(hits.len(), 1);
        assert_eq!(hits[0].profile_id, "codex");

        let hits = index
            .search(
                "",
                &Filters {
                    project: Some("FORJA".to_owned()),
                    ..Filters::default()
                },
                10,
            )
            .expect("filter by project");
        assert_eq!(hits.len(), 3);

        let hits = index
            .search(
                "",
                &Filters {
                    since_ms: Some(4_000_000_000_000),
                    ..Filters::default()
                },
                10,
            )
            .expect("future filter");
        assert!(hits.is_empty());

        let mut file = std::fs::OpenOptions::new()
            .append(true)
            .open(&fixture.files[0].1)
            .unwrap();
        writeln!(
            file,
            r#"{{"type":"assistant","timestamp":"2026-11-12T10:10:00.000Z","message":{{"role":"assistant","content":[{{"type":"text","text":"Nova linha incremental sobre o índice."}}]}}}}"#
        )
        .unwrap();
        let report = index
            .scan(&mut cache, &fixture.files, true)
            .expect("incremental scan");
        assert_eq!(report.parsed, 1);
        let hits = index
            .search("incremental", &Filters::default(), 10)
            .expect("search new text");
        assert_eq!(hits.len(), 1);
        assert_eq!(hits[0].profile_id, "claude-code");

        index
            .delete_path(&fixture.files[0].1)
            .expect("delete from index");
        let hits = index
            .search("parser", &Filters::default(), 10)
            .expect("search after delete");
        assert!(hits.is_empty());
    }

    #[test]
    fn removing_a_file_drops_it_from_the_index() {
        let fixture = fixture();
        let index = ArchiveIndex::open(&fixture.dir.join("index")).unwrap();
        let mut cache = SessionCache::default();
        index
            .scan(&mut cache, &fixture.files, true)
            .expect("first scan");
        let mut files = fixture.files.clone();
        files.remove(1);
        let report = index.scan(&mut cache, &files, true).expect("remove scan");
        assert_eq!(report.removed, 1);
        assert_eq!(cache.file_count(), 2);
        let hits = index
            .search("painel", &Filters::default(), 10)
            .expect("codex gone");
        assert!(hits.is_empty());
    }

    #[test]
    fn keep_text_false_still_indexes_the_title() {
        let fixture = fixture();
        let index = ArchiveIndex::open(&fixture.dir.join("index")).unwrap();
        let mut cache = SessionCache::default();
        index
            .scan(&mut cache, &fixture.files, false)
            .expect("scan without text");
        let hits = index
            .search("Acervo", &Filters::default(), 10)
            .expect("title search");
        assert!(!hits.is_empty());
    }

    #[test]
    fn changing_keep_text_reindexes_unchanged_files() {
        let fixture = fixture();
        let index = ArchiveIndex::open(&fixture.dir.join("index")).unwrap();
        let mut cache = SessionCache::default();
        index.scan(&mut cache, &fixture.files, true).unwrap();
        assert!(!cache.sessions().iter().all(|session| session.text.is_empty()));
        assert!(!index.search("indexação", &Filters::default(), 10).unwrap().is_empty());
        let report = index.scan(&mut cache, &fixture.files, false).unwrap();
        assert_eq!(report.parsed, fixture.files.len());
        assert!(cache.sessions().iter().all(|session| session.text.is_empty()));
        assert!(index.search("indexação", &Filters::default(), 10).unwrap().is_empty());
        let report = index.scan(&mut cache, &fixture.files, true).unwrap();
        assert_eq!(report.parsed, fixture.files.len());
        assert!(!cache.sessions().iter().all(|session| session.text.is_empty()));
        assert!(!index.search("indexação", &Filters::default(), 10).unwrap().is_empty());
    }
}
