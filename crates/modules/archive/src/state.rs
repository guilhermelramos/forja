//! Estado do Acervo: perfis, cache de sessões e última varredura.

use crate::index::{ArchiveIndex, Filters, Hit, Report, SessionCache};
use crate::parse::ArchiveSession;
use crate::profile::{self, ArchiveProfile};
use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};

const RESCAN_INTERVAL: Duration = Duration::from_secs(60);
const MAX_FILES: usize = 20_000;

/// Estado do módulo.
#[derive(Default)]
pub struct ArchiveState {
    /// Perfis carregados.
    pub profiles: Vec<ArchiveProfile>,
    /// Bancada da última carga.
    pub loaded_for: Option<PathBuf>,
    /// Cache incremental.
    pub cache: SessionCache,
    /// Sessões conhecidas.
    pub sessions: Vec<ArchiveSession>,
    /// Último relatório.
    pub report: Report,
    /// Varredura em andamento.
    pub scanning: bool,
    /// Última varredura concluída.
    pub last_scan: Option<Instant>,
    /// Erro do índice.
    pub error: Option<String>,
    /// Prévia semeada.
    pub preview_seeded: bool,
    /// Resultados da última busca.
    pub results: Vec<Hit>,
}

impl ArchiveState {
    /// Carrega os perfis quando a bancada muda.
    pub fn ensure_profiles(&mut self, bench: Option<&Path>) {
        if self.loaded_for.as_deref() == bench && !self.profiles.is_empty() {
            return;
        }
        self.profiles = profile::load_profiles(bench);
        self.loaded_for = bench.map(Path::to_path_buf);
    }

    /// Perfil por id.
    pub fn profile(&self, id: &str) -> Option<&ArchiveProfile> {
        profile::find(&self.profiles, id)
    }

    /// Precisa varrer de novo.
    pub fn needs_scan(&self) -> bool {
        !self.scanning
            && self
                .last_scan
                .is_none_or(|last| last.elapsed() >= RESCAN_INTERVAL)
    }

    /// Marca o início da varredura e entrega o cache.
    pub fn begin_scan(&mut self) -> SessionCache {
        self.scanning = true;
        self.error = None;
        std::mem::take(&mut self.cache)
    }

    /// Conclui a varredura.
    pub fn finish(
        &mut self,
        cache: SessionCache,
        sessions: Vec<ArchiveSession>,
        report: Report,
    ) {
        self.cache = cache;
        self.sessions = sessions;
        self.report = report;
        self.scanning = false;
        self.last_scan = Some(Instant::now());
    }

    /// Falha da varredura.
    pub fn fail(&mut self, cache: SessionCache, error: String) {
        self.cache = cache;
        self.scanning = false;
        self.last_scan = Some(Instant::now());
        self.error = Some(error);
    }

    /// Sessão por perfil e id.
    pub fn find(&self, profile_id: &str, session_id: &str) -> Option<&ArchiveSession> {
        self.sessions
            .iter()
            .find(|session| session.profile_id() == profile_id && session.session_id == session_id)
    }
}

impl ArchiveSession {
    /// Id do perfil.
    pub fn profile_id(&self) -> &str {
        &self.helper_id
    }
}

/// Pasta do índice derivada do store do módulo.
pub fn index_dir(store_path: &Path) -> PathBuf {
    store_path
        .parent()
        .and_then(Path::parent)
        .map(|root| root.join("archive-index"))
        .unwrap_or_else(|| PathBuf::from("archive-index"))
}

/// Configuração lida das settings.
#[derive(Clone, Copy, Debug)]
pub struct ScanOptions {
    /// Guarda o texto completo.
    pub keep_text: bool,
    /// Retenção em dias; 0 mantém para sempre.
    pub retention_days: i64,
    /// Limite do índice em bytes.
    pub max_bytes: u64,
}

/// Varredura executada em segundo plano.
pub fn scan_task(
    profiles: Vec<ArchiveProfile>,
    index_dir: PathBuf,
    mut cache: SessionCache,
    options: ScanOptions,
) -> Result<(SessionCache, Vec<ArchiveSession>, Report), String> {
    let index = ArchiveIndex::open(&index_dir)?;
    if options.max_bytes > 0 && dir_size(&index_dir) > options.max_bytes {
        return Err(format!(
            "o índice do Acervo passou do limite de {} GB",
            options.max_bytes / (1024 * 1024 * 1024)
        ));
    }
    let files = profile::discover_all(&profiles, MAX_FILES);
    let mut report = index.scan(&mut cache, &files, options.keep_text)?;
    if options.retention_days > 0 {
        let cutoff = chrono::Utc::now().timestamp_millis()
            - options.retention_days * 24 * 60 * 60 * 1000;
        let expired: Vec<ArchiveSession> = cache
            .sessions()
            .into_iter()
            .filter(|session| {
                session.updated_ms > 0 && session.updated_ms < cutoff
            })
            .collect();
        for session in &expired {
            let path = PathBuf::from(&session.path);
            let _ = index.delete_path(&path);
            cache.forget(&path);
            report.removed += 1;
        }
    }
    let sessions = cache.sessions();
    Ok((cache, sessions, report))
}

/// Busca executada na interface.
pub fn search_task(
    index_dir: &Path,
    query: &str,
    filters: &Filters,
    limit: usize,
) -> Result<Vec<Hit>, String> {
    let index = ArchiveIndex::open(index_dir)?;
    index.search(query, filters, limit)
}

fn dir_size(path: &Path) -> u64 {
    let mut total = 0;
    let Ok(entries) = std::fs::read_dir(path) else {
        return 0;
    };
    for entry in entries.flatten() {
        let Ok(metadata) = entry.metadata() else {
            continue;
        };
        if metadata.is_dir() {
            total += dir_size(&entry.path());
        } else {
            total += metadata.len();
        }
    }
    total
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn index_dir_lives_next_to_the_modules_folder() {
        let dir = index_dir(Path::new("C:/data/Forja/modules/archive.json"));
        assert_eq!(dir, PathBuf::from("C:/data/Forja/archive-index"));
    }

    #[test]
    fn sessions_expose_the_profile_id() {
        let session = ArchiveSession {
            helper_id: "codex".to_owned(),
            ..ArchiveSession::default()
        };
        assert_eq!(session.profile_id(), "codex");
    }
}
