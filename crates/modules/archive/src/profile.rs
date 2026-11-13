//! Perfis de ajudante vistos pelo Acervo.
//!
//! Só o que o Acervo precisa: onde ficam as transcrições e como retomar ou
//! bifurcar uma sessão. O formato completo é `design/helper-profile.md`.

use crate::parse::SessionFormat;
use serde::Deserialize;
use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

/// Perfil resumido.
#[derive(Clone, Debug, Default, Deserialize)]
pub struct ArchiveProfile {
    /// Id.
    pub id: String,
    /// Nome visível.
    pub name: String,
    /// Executável do modo interativo.
    #[serde(default)]
    pub start: Start,
    /// Sessões.
    #[serde(default)]
    pub sessions: Sessions,
    /// Retomar.
    #[serde(default)]
    pub resume: Resume,
    /// Bifurcar.
    #[serde(default)]
    pub fork: Fork,
    /// Interpretação do prompt, usada ao continuar pelo terminal.
    #[serde(default)]
    pub prompt: Prompt,
    /// Permissões, usadas ao abrir o terminal.
    #[serde(default)]
    pub permissions: BTreeMap<String, Permission>,
}

/// `[start]`.
#[derive(Clone, Debug, Default, Deserialize)]
pub struct Start {
    /// Executável.
    #[serde(default)]
    pub command: String,
    /// Argumentos.
    #[serde(default)]
    pub args: Vec<String>,
}

/// `[prompt]`.
#[derive(Clone, Debug, Default, Deserialize)]
pub struct Prompt {
    /// `argument`, `flag`, `stdin` ou `none`.
    #[serde(default)]
    pub mode: String,
    /// Flag do modo.
    #[serde(default)]
    pub flag: Option<String>,
}

/// `[permissions.<modo>]`.
#[derive(Clone, Debug, Default, Deserialize)]
pub struct Permission {
    /// Flags.
    #[serde(default)]
    pub args: Vec<String>,
}

/// `[sessions]`.
#[derive(Clone, Debug, Default, Deserialize)]
pub struct Sessions {
    /// `jsonl`, `json` ou `sqlite`.
    #[serde(default)]
    pub format: String,
    /// Caminho ou padrão.
    #[serde(default)]
    pub path: String,
    /// Variável que muda a base.
    #[serde(default)]
    pub dir_env: Option<String>,
    /// Subcomandos de sessão.
    #[serde(default)]
    pub list: Vec<String>,
    /// Exporta.
    #[serde(default)]
    pub export: Vec<String>,
    /// Apaga.
    #[serde(default)]
    pub delete: Vec<String>,
}

/// `[resume]`.
#[derive(Clone, Debug, Default, Deserialize)]
pub struct Resume {
    /// Sessão mais recente.
    #[serde(default)]
    pub last: Vec<String>,
    /// Por id.
    #[serde(default)]
    pub id: Vec<String>,
}

/// `[fork]`.
#[derive(Clone, Debug, Default, Deserialize)]
pub struct Fork {
    /// Mais recente.
    #[serde(default)]
    pub last: Vec<String>,
    /// Por id.
    #[serde(default)]
    pub id: Vec<String>,
    /// Somado ao retomar.
    #[serde(default)]
    pub adds: Vec<String>,
}

impl ArchiveProfile {
    /// Formato de transcrição, quando suportado.
    pub fn format(&self) -> Option<SessionFormat> {
        match self.id.as_str() {
            "claude-code" => Some(SessionFormat::Claude),
            "codex" => Some(SessionFormat::Codex),
            "opencode" => Some(SessionFormat::OpenCode),
            _ => None,
        }
    }

    /// Retomar por id.
    pub fn resume_argv(&self, session_id: &str) -> Vec<String> {
        let source = if self.resume.id.is_empty() {
            &self.resume.last
        } else {
            &self.resume.id
        };
        source
            .iter()
            .map(|token| token.replace("{id}", session_id))
            .collect()
    }

    /// Bifurcar por id.
    pub fn fork_argv(&self, session_id: &str) -> Vec<String> {
        let mut argv: Vec<String> = if !self.fork.id.is_empty() {
            self.fork
                .id
                .iter()
                .map(|token| token.replace("{id}", session_id))
                .collect()
        } else if !self.fork.last.is_empty() {
            self.fork
                .last
                .iter()
                .map(|token| token.replace("{id}", session_id))
                .collect()
        } else {
            self.resume_argv(session_id)
        };
        argv.extend(
            self.fork
                .adds
                .iter()
                .map(|token| token.replace("{id}", session_id)),
        );
        argv
    }

    /// Pastas onde procurar arquivos de sessão.
    pub fn roots(&self) -> Vec<PathBuf> {
        match self.id.as_str() {
            "claude-code" => vec![claude_home().join("projects")],
            "codex" => vec![codex_home().join("sessions")],
            "opencode" => {
                let db = std::env::var_os("OPENCODE_DB")
                    .map(PathBuf::from)
                    .unwrap_or_else(|| {
                        home()
                            .join(".local")
                            .join("share")
                            .join("opencode")
                            .join("opencode.db")
                    });
                db.parent()
                    .map(|parent| vec![parent.join("storage")])
                    .unwrap_or_default()
            }
            _ => Vec::new(),
        }
    }
}

const BUILTIN: &[&str] = &[
    include_str!("../../../../content/helpers/aider.toml"),
    include_str!("../../../../content/helpers/amp.toml"),
    include_str!("../../../../content/helpers/antigravity.toml"),
    include_str!("../../../../content/helpers/claude-code.toml"),
    include_str!("../../../../content/helpers/cline.toml"),
    include_str!("../../../../content/helpers/codex.toml"),
    include_str!("../../../../content/helpers/continue.toml"),
    include_str!("../../../../content/helpers/copilot.toml"),
    include_str!("../../../../content/helpers/crush.toml"),
    include_str!("../../../../content/helpers/cursor-agent.toml"),
    include_str!("../../../../content/helpers/gemini.toml"),
    include_str!("../../../../content/helpers/goose.toml"),
    include_str!("../../../../content/helpers/kimi-code.toml"),
    include_str!("../../../../content/helpers/kiro.toml"),
    include_str!("../../../../content/helpers/opencode.toml"),
    include_str!("../../../../content/helpers/qwen-code.toml"),
];

impl ArchiveProfile {
    /// Lê um perfil TOML.
    pub fn parse(text: &str) -> Option<Self> {
        let profile: ArchiveProfile = toml::from_str(text).ok()?;
        (!profile.id.is_empty() && !profile.start.command.is_empty()).then_some(profile)
    }
}

/// Carrega as três camadas.
pub fn load_profiles(bench: Option<&Path>) -> Vec<ArchiveProfile> {
    let mut by_id: BTreeMap<String, ArchiveProfile> = BTreeMap::new();
    for text in BUILTIN {
        if let Some(profile) = ArchiveProfile::parse(text) {
            by_id.insert(profile.id.clone(), profile);
        }
    }
    for directory in [
        user_content_root().join("helpers"),
        bench
            .map(|bench| bench.join(".forja").join("helpers"))
            .unwrap_or_default(),
    ] {
        let Ok(entries) = std::fs::read_dir(directory) else {
            continue;
        };
        let mut files: Vec<PathBuf> = entries
            .filter_map(Result::ok)
            .map(|entry| entry.path())
            .filter(|path| path.extension().is_some_and(|extension| extension == "toml"))
            .collect();
        files.sort();
        for file in files {
            if let Ok(text) = std::fs::read_to_string(file) {
                if let Some(profile) = ArchiveProfile::parse(&text) {
                    by_id.insert(profile.id.clone(), profile);
                }
            }
        }
    }
    by_id.into_values().collect()
}

/// Perfil por id.
pub fn find<'a>(profiles: &'a [ArchiveProfile], id: &str) -> Option<&'a ArchiveProfile> {
    profiles.iter().find(|profile| profile.id == id)
}

/// Arquivos de sessão de um perfil, com profundidade limitada.
pub fn discover(profile: &ArchiveProfile, limit: usize) -> Vec<PathBuf> {
    let mut files = Vec::new();
    for root in profile.roots() {
        walk_files(&root, 8, &mut files);
        if files.len() >= limit {
            break;
        }
    }
    files.truncate(limit);
    files
}

/// Caminhos de todos os perfis suportados.
pub fn discover_all(profiles: &[ArchiveProfile], limit: usize) -> Vec<(String, PathBuf)> {
    let mut files = Vec::new();
    for profile in profiles {
        if profile.format().is_none() {
            continue;
        }
        for path in discover(profile, limit) {
            if archive_file(profile, &path) {
                files.push((profile.id.clone(), path));
            }
            if files.len() >= limit {
                return files;
            }
        }
    }
    files
}

fn archive_file(profile: &ArchiveProfile, path: &Path) -> bool {
    let extension = path
        .extension()
        .and_then(|extension| extension.to_str())
        .unwrap_or("");
    if profile.id == "codex" {
        return extension == "jsonl"
            && path
                .file_name()
                .and_then(|name| name.to_str())
                .is_some_and(|name| name.starts_with("rollout-"));
    }
    if profile.id == "opencode" {
        return extension == "json";
    }
    extension == "jsonl"
}

fn walk_files(root: &Path, depth: usize, files: &mut Vec<PathBuf>) {
    if depth == 0 {
        return;
    }
    let Ok(entries) = std::fs::read_dir(root) else {
        return;
    };
    for entry in entries.flatten() {
        let path = entry.path();
        let Ok(file_type) = entry.file_type() else {
            continue;
        };
        if file_type.is_dir() {
            walk_files(&path, depth - 1, files);
        } else if file_type.is_file() {
            files.push(path);
        }
    }
}

fn user_content_root() -> PathBuf {
    if let Some(content) = std::env::var_os("FORJA_CONTENT_DIR") {
        return PathBuf::from(content);
    }
    home().join(".forja")
}

fn home() -> PathBuf {
    std::env::var_os("USERPROFILE")
        .or_else(|| std::env::var_os("HOME"))
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from("."))
}

fn claude_home() -> PathBuf {
    std::env::var_os("CLAUDE_CONFIG_DIR")
        .map(PathBuf::from)
        .unwrap_or_else(|| home().join(".claude"))
}

fn codex_home() -> PathBuf {
    std::env::var_os("CODEX_HOME")
        .map(PathBuf::from)
        .unwrap_or_else(|| home().join(".codex"))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn profile(id: &str) -> ArchiveProfile {
        let text = BUILTIN
            .iter()
            .find(|text| {
                text.lines()
                    .find_map(|line| line.strip_prefix("id = "))
                    .map(|value| value.trim().trim_matches('"') == id)
                    .unwrap_or(false)
            })
            .expect("builtin profile");
        ArchiveProfile::parse(text).unwrap()
    }

    #[test]
    fn resume_and_fork_argv_come_from_the_profile() {
        let claude = profile("claude-code");
        assert_eq!(claude.resume_argv("s1"), vec!["--resume", "s1"]);
        assert_eq!(
            claude.fork_argv("s1"),
            vec!["--resume", "s1", "--fork-session"]
        );
        let codex = profile("codex");
        assert_eq!(codex.resume_argv("t1"), vec!["resume", "t1"]);
        assert_eq!(codex.fork_argv("t1"), vec!["fork", "t1"]);
        let opencode = profile("opencode");
        assert_eq!(opencode.resume_argv("o1"), vec!["--session", "o1"]);
        assert_eq!(
            opencode.fork_argv("o1"),
            vec!["--session", "o1", "--fork"]
        );
    }

    #[test]
    fn only_confirmed_formats_are_discovered() {
        assert_eq!(profile("claude-code").format(), Some(SessionFormat::Claude));
        assert_eq!(profile("codex").format(), Some(SessionFormat::Codex));
        assert_eq!(
            profile("opencode").format(),
            Some(SessionFormat::OpenCode)
        );
        assert_eq!(profile("gemini").format(), None);
        assert!(profile("gemini").roots().is_empty());
    }
}
