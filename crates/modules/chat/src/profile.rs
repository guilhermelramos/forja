//! Leitura mínima dos perfis para montar turnos de chat.

use crate::transcript::Format;
use serde::Deserialize;
use sha2::{Digest, Sha256};
use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

/// Perfil resumido.
#[derive(Clone, Debug, Default, Deserialize)]
pub struct ChatProfile {
    /// Id.
    pub id: String,
    /// Nome.
    pub name: String,
    /// Início.
    #[serde(default)]
    pub start: Start,
    /// Prompt headless.
    #[serde(default)]
    pub prompt: Prompt,
    /// Permissões.
    #[serde(default)]
    pub permissions: BTreeMap<String, Permission>,
    /// Retomar.
    #[serde(default)]
    pub resume: Resume,
    /// Modelos.
    #[serde(default)]
    pub models: Models,
    /// Transcrições.
    #[serde(default)]
    pub sessions: Sessions,
    /// Hook support makes project profiles require trust before use.
    #[serde(default)]
    pub hooks: Hooks,
    /// Source of a user or project profile; built-ins have no source file.
    #[serde(skip)]
    pub source_path: Option<PathBuf>,
    /// Exact content hash used by the trust registry.
    #[serde(skip)]
    pub content_hash: String,
    /// Project profiles need explicit approval when they carry risk.
    #[serde(skip)]
    pub requires_trust: bool,
}

#[derive(Clone, Debug, Default, Deserialize)]
pub struct Hooks {
    #[serde(default)]
    pub supported: bool,
}

/// `[sessions]`.
#[derive(Clone, Debug, Default, Deserialize)]
pub struct Sessions {
    /// Formato do arquivo.
    #[serde(default)]
    pub format: String,
    /// Caminho ou padrão.
    #[serde(default)]
    pub path: String,
    /// Variável que muda a base.
    #[serde(default)]
    pub dir_env: Option<String>,
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
    /// Argumentos headless.
    #[serde(default)]
    pub headless: Vec<String>,
    /// Aceita stdin.
    #[serde(default)]
    pub headless_stdin: bool,
}

/// `[permissions.<modo>]`.
#[derive(Clone, Debug, Default, Deserialize)]
pub struct Permission {
    /// Flags.
    #[serde(default)]
    pub args: Vec<String>,
    /// Variáveis.
    #[serde(default)]
    pub env: BTreeMap<String, String>,
}

/// `[resume]`.
#[derive(Clone, Debug, Default, Deserialize)]
pub struct Resume {
    /// Última.
    #[serde(default)]
    pub last: Vec<String>,
    /// Por id.
    #[serde(default)]
    pub id: Vec<String>,
    /// Headless.
    #[serde(default)]
    pub headless: ResumeHeadless,
}

/// `[resume.headless]`.
#[derive(Clone, Debug, Default, Deserialize)]
pub struct ResumeHeadless {
    /// Última.
    #[serde(default)]
    pub last: Vec<String>,
    /// Por id.
    #[serde(default)]
    pub id: Vec<String>,
}

/// `[models]`.
#[derive(Clone, Debug, Default, Deserialize)]
pub struct Models {
    /// Model choices declared by this helper profile.
    #[serde(default)]
    pub values: Vec<String>,
    /// Flags de modelo.
    #[serde(default)]
    pub model_args: Vec<String>,
    /// Flags de esforço.
    #[serde(default)]
    pub effort_args: Vec<String>,
    /// Valores aceitos de esforço.
    #[serde(default)]
    pub effort_values: Vec<String>,
    /// Modelo por ambiente.
    #[serde(default)]
    pub model_env: Option<String>,
    /// Esforço por ambiente.
    #[serde(default)]
    pub effort_env: Option<String>,
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

impl ChatProfile {
    pub(crate) fn parse(text: &str, source_path: Option<PathBuf>, project: bool) -> Option<Self> {
        let mut profile: ChatProfile = toml::from_str(text).ok()?;
        if profile.id.is_empty() || profile.start.command.is_empty() {
            return None;
        }
        profile.requires_trust = project
            && (profile.permissions.contains_key("automatic")
                || profile.permissions.contains_key("full")
                || profile.hooks.supported);
        profile.content_hash = sha256_hex(text);
        profile.source_path = source_path;
        Some(profile)
    }

    /// Suporta stream-json.
    pub fn supports_stream(&self) -> bool {
        matches!(self.id.as_str(), "claude-code" | "codex" | "opencode")
    }
}

/// Texto de um perfil embutido.
#[cfg(test)]
pub fn builtin_text(id: &str) -> Option<&'static str> {
    BUILTIN.iter().copied().find(|text| {
        text.lines()
            .find_map(|line| line.strip_prefix("id = "))
            .map(|value| value.trim().trim_matches('"') == id)
            .unwrap_or(false)
    })
}

/// Pasta de conteúdo do usuário (`FORJA_CONTENT_DIR` ou `~/.forja`).
pub fn user_content_root() -> PathBuf {
    user_root()
}

/// Carrega os perfis embutidos, do usuário e do projeto.
pub fn load_profiles(bench: Option<&Path>) -> Vec<ChatProfile> {
    let mut by_id: BTreeMap<String, ChatProfile> = BTreeMap::new();
    for text in BUILTIN {
        if let Some(profile) = ChatProfile::parse(text, None, false) {
            by_id.insert(profile.id.clone(), profile);
        }
    }
    for (directory, project) in [
        (user_root().join("helpers"), false),
        (
            bench
                .map(|bench| bench.join(".forja").join("helpers"))
                .unwrap_or_default(),
            true,
        ),
    ] {
        let Ok(entries) = std::fs::read_dir(directory) else {
            continue;
        };
        let mut files: Vec<PathBuf> = entries
            .filter_map(Result::ok)
            .map(|entry| entry.path())
            .filter(|path| path.extension().is_some_and(|ext| ext == "toml"))
            .collect();
        files.sort();
        for file in files {
            if let Ok(text) = std::fs::read_to_string(&file) {
                if let Some(profile) = ChatProfile::parse(&text, Some(file), project) {
                    by_id.insert(profile.id.clone(), profile);
                }
            }
        }
    }
    by_id.into_values().collect()
}

fn sha256_hex(text: &str) -> String {
    let digest = Sha256::digest(text.as_bytes());
    let mut output = String::with_capacity(64);
    for byte in digest {
        use std::fmt::Write;
        let _ = write!(output, "{byte:02x}");
    }
    output
}

/// Busca por id.
#[cfg(test)]
pub fn find<'a>(profiles: &'a [ChatProfile], id: &str) -> Option<&'a ChatProfile> {
    profiles.iter().find(|profile| profile.id == id)
}

/// Argumentos de um turno headless.
pub fn turn_args(
    profile: &ChatProfile,
    session_id: Option<&str>,
    permission: &str,
    model: Option<&str>,
    effort: Option<&str>,
) -> Result<Vec<String>, String> {
    if !profile.supports_stream() {
        return Err(format!(
            "profile `{}` has no structured chat runner",
            profile.id
        ));
    }
    let mut args = if profile.id == "codex" {
        vec!["exec".to_owned(), "--json".to_owned()]
    } else if profile.id == "opencode" {
        vec!["run".to_owned(), "--format".to_owned(), "json".to_owned()]
    } else {
        vec![
            "-p".to_owned(),
            "--output-format".to_owned(),
            "stream-json".to_owned(),
            "--verbose".to_owned(),
        ]
    };
    if let Some(model) = model {
        if !profile.models.model_args.is_empty() {
            args.extend(
                profile
                    .models
                    .model_args
                    .iter()
                    .map(|arg| arg.replace("{model}", model)),
            );
        } else if let Some(name) = &profile.models.model_env {
            args.push("--env".to_owned());
            args.push(format!("{name}={model}"));
        }
    }
    if let Some(effort) = effort {
        if !profile.models.effort_args.is_empty() {
            args.extend(
                profile
                    .models
                    .effort_args
                    .iter()
                    .map(|arg| arg.replace("{effort}", effort)),
            );
        } else if let Some(name) = &profile.models.effort_env {
            args.push("--env".to_owned());
            args.push(format!("{name}={effort}"));
        }
    }
    if let Some(session_id) = session_id {
        let headless = &profile.resume.headless.id;
        let source = if headless.is_empty() {
            &profile.resume.id
        } else {
            headless
        };
        let mut resume: Vec<String> = source
            .iter()
            .map(|arg| arg.replace("{id}", session_id))
            .collect();
        if profile.id == "codex" && resume.first().is_some_and(|token| token == "exec") {
            // `turn_args` já começa com `exec --json`.
            resume.remove(0);
        } else if profile.id == "opencode" && resume.first().is_some_and(|token| token == "run") {
            // `turn_args` já começa com `run --format json`.
            resume.remove(0);
        }
        args.extend(resume);
    }
    let mode = match permission {
        "accept_edits" => "accept-edits",
        "auto" => "automatic",
        "total" => "full",
        other => other,
    };
    if let Some(spec) = profile.permissions.get(mode) {
        args.extend(spec.args.clone());
    }
    Ok(args)
}

/// Ambiente de um turno.
pub fn turn_env(profile: &ChatProfile, permission: &str) -> BTreeMap<String, String> {
    let mode = match permission {
        "accept_edits" => "accept-edits",
        "auto" => "automatic",
        "total" => "full",
        other => other,
    };
    profile
        .permissions
        .get(mode)
        .map(|spec| spec.env.clone())
        .unwrap_or_default()
}

/// Formato de transcrição do perfil.
pub fn transcript_format(profile: &ChatProfile) -> Format {
    Format::for_profile(&profile.id)
}

/// Pastas onde o CLI guarda transcrições.
pub fn session_roots(profile: &ChatProfile) -> Vec<PathBuf> {
    match profile.id.as_str() {
        "claude-code" => vec![claude_config().join("projects")],
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
        _ => generic_roots(&profile.sessions),
    }
}

/// Procura a transcrição de uma sessão.
pub fn find_transcript(profile: &ChatProfile, session_id: &str) -> Option<PathBuf> {
    for root in session_roots(profile) {
        for path in walk_files(&root, 8) {
            if matches_session_file(profile, &path, session_id) {
                return Some(path);
            }
        }
    }
    None
}

fn matches_session_file(profile: &ChatProfile, path: &Path, session_id: &str) -> bool {
    let extension = path
        .extension()
        .and_then(|extension| extension.to_str())
        .unwrap_or("");
    if profile.id == "opencode" {
        if extension != "json" {
            return false;
        }
        let Ok(text) = std::fs::read_to_string(path) else {
            return false;
        };
        let Ok(value) = serde_json::from_str::<serde_json::Value>(&text) else {
            return false;
        };
        return value
            .get("info")
            .and_then(|info| info.get("id"))
            .and_then(serde_json::Value::as_str)
            == Some(session_id);
    }
    if !matches!(extension, "jsonl" | "json") {
        return false;
    }
    let name = path
        .file_name()
        .and_then(|name| name.to_str())
        .unwrap_or("");
    name.contains(session_id)
}

fn generic_roots(sessions: &Sessions) -> Vec<PathBuf> {
    let mut path = expand(&sessions.path, sessions.dir_env.as_deref());
    if path.as_os_str().is_empty() {
        return Vec::new();
    }
    while let Some(component) = path.file_name() {
        let text = component.to_string_lossy();
        if text.contains('<') || text.contains('*') || text.contains('{') {
            path.pop();
            continue;
        }
        break;
    }
    if path.extension().is_some() {
        path.pop();
    }
    vec![path]
}

fn expand(path: &str, dir_env: Option<&str>) -> PathBuf {
    if path.is_empty() {
        return PathBuf::new();
    }
    let mut text = path.replace('\\', "/");
    for (key, value) in std::env::vars() {
        text = text
            .replace(&format!("%{key}%"), &value)
            .replace(&format!("${key}"), &value);
    }
    if let Some(rest) = text.strip_prefix("~/") {
        text = home().join(rest).display().to_string().replace('\\', "/");
    }
    if let Some(name) = dir_env {
        if let Some(root) = std::env::var_os(name) {
            let root = PathBuf::from(root);
            if let Some(rest) = text.strip_prefix(&home().display().to_string().replace('\\', "/"))
            {
                let rest = rest.trim_start_matches('/');
                let mut parts = rest.splitn(2, '/');
                let _first = parts.next();
                let tail = parts.next().unwrap_or("");
                text = root.join(tail).display().to_string().replace('\\', "/");
            }
        }
    }
    PathBuf::from(text)
}

fn claude_config() -> PathBuf {
    std::env::var_os("CLAUDE_CONFIG_DIR")
        .map(PathBuf::from)
        .unwrap_or_else(|| home().join(".claude"))
}

fn codex_home() -> PathBuf {
    std::env::var_os("CODEX_HOME")
        .map(PathBuf::from)
        .unwrap_or_else(|| home().join(".codex"))
}

fn home() -> PathBuf {
    std::env::var_os("USERPROFILE")
        .or_else(|| std::env::var_os("HOME"))
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from("."))
}

fn walk_files(root: &Path, depth: usize) -> Vec<PathBuf> {
    let mut files = Vec::new();
    if depth == 0 {
        return files;
    }
    let Ok(entries) = std::fs::read_dir(root) else {
        return files;
    };
    for entry in entries.flatten() {
        let path = entry.path();
        let Ok(file_type) = entry.file_type() else {
            continue;
        };
        if file_type.is_dir() {
            files.extend(walk_files(&path, depth - 1));
        } else if file_type.is_file() {
            files.push(path);
        }
    }
    files
}

fn user_root() -> PathBuf {
    if let Some(content) = std::env::var_os("FORJA_CONTENT_DIR") {
        return PathBuf::from(content);
    }
    std::env::var_os("USERPROFILE")
        .or_else(|| std::env::var_os("HOME"))
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from("."))
        .join(".forja")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn profiles_load_and_structured_ones_are_known() {
        let profiles = load_profiles(None);
        assert!(profiles.len() >= 16);
        let claude = find(&profiles, "claude-code").unwrap();
        assert!(claude.supports_stream());
        assert!(!find(&profiles, "gemini").unwrap().supports_stream());
    }

    #[test]
    fn claude_argv_has_stream_and_permission_before_resume() {
        let profiles = load_profiles(None);
        let claude = find(&profiles, "claude-code").unwrap();
        let args = turn_args(claude, Some("abc"), "full", Some("opus"), None).unwrap();
        assert!(args.starts_with(&["-p".into(), "--output-format".into(), "stream-json".into()]));
        assert!(args.windows(2).any(|pair| pair == ["--resume", "abc"]));
        assert!(args.contains(&"--dangerously-skip-permissions".into()));
    }

    #[test]
    fn codex_argv_uses_exec_json_and_resume_subcommand() {
        let profiles = load_profiles(None);
        let codex = find(&profiles, "codex").unwrap();
        let args = turn_args(codex, Some("t1"), "full", None, None).unwrap();
        assert_eq!(args[0], "exec");
        assert_eq!(args[1], "--json");
        assert!(args.windows(2).any(|pair| pair == ["resume", "t1"]));
    }
}
