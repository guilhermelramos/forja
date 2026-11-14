//! Leitura e validação dos perfis de ajudante.

use crate::profile_data::BUILTIN;
use serde::Deserialize;
use sha2::{Digest, Sha256};
use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

/// Onde o perfil foi lido.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Layer {
    /// `content/helpers` embutido no produto.
    Builtin,
    /// `~/.forja/helpers`.
    User,
    /// `<projeto>/.forja/helpers`.
    Project,
}

/// Seção `[detect]`.
#[derive(Clone, Debug, Default, Deserialize)]
pub struct Detect {
    /// Nomes procurados no PATH.
    #[serde(default)]
    pub executables: Vec<String>,
    /// Argumentos que imprimem a versão.
    #[serde(default)]
    pub version: Vec<String>,
}

/// Seção `[start]`.
#[derive(Clone, Debug, Default, Deserialize)]
pub struct Start {
    /// Executável do modo interativo.
    #[serde(default)]
    pub command: String,
    /// Argumentos padrão.
    #[serde(default)]
    pub args: Vec<String>,
}

/// Seção `[prompt]`.
#[derive(Clone, Debug, Default, Deserialize)]
pub struct Prompt {
    /// `argument`, `flag`, `stdin` ou `none`.
    #[serde(default)]
    pub mode: String,
    /// Flag do modo `flag`.
    #[serde(default)]
    pub flag: Option<String>,
    /// Argumentos completos do modo não interativo.
    #[serde(default)]
    pub headless: Vec<String>,
    /// Aceita prompt por stdin no modo não interativo.
    #[serde(default)]
    pub headless_stdin: bool,
}

/// Uma entrada de `[permissions.<modo>]`.
#[derive(Clone, Debug, Default, Deserialize)]
pub struct PermissionSpec {
    /// Flags do modo.
    #[serde(default)]
    pub args: Vec<String>,
    /// Variáveis do modo.
    #[serde(default)]
    pub env: BTreeMap<String, String>,
}

/// Seção `[resume]`.
#[derive(Clone, Debug, Default, Deserialize)]
pub struct Resume {
    /// Argumentos para a sessão mais recente.
    #[serde(default)]
    pub last: Vec<String>,
    /// Argumentos para uma sessão por id.
    #[serde(default)]
    pub id: Vec<String>,
}

/// Seção `[fork]`.
#[derive(Clone, Debug, Default, Deserialize)]
pub struct Fork {
    /// Argumentos para bifurcar a sessão mais recente.
    #[serde(default)]
    pub last: Vec<String>,
    /// Argumentos para bifurcar por id.
    #[serde(default)]
    pub id: Vec<String>,
    /// Flag somada ao retomar.
    #[serde(default)]
    pub adds: Vec<String>,
}

/// Seção `[models]`.
#[derive(Clone, Debug, Default, Deserialize)]
pub struct Models {
    /// Flags de modelo com `{model}`.
    #[serde(default)]
    pub model_args: Vec<String>,
    /// Flags de esforço com `{effort}`.
    #[serde(default)]
    pub effort_args: Vec<String>,
    /// Valores de esforço aceitos.
    #[serde(default)]
    pub effort_values: Vec<String>,
    /// Variável que escolhe o modelo.
    #[serde(default)]
    pub model_env: Option<String>,
    /// Variável que escolhe o esforço.
    #[serde(default)]
    pub effort_env: Option<String>,
}

/// Seção `[sessions]`.
#[derive(Clone, Debug, Default, Deserialize)]
pub struct Sessions {
    /// Formato das transcrições.
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
    /// Exporta uma sessão.
    #[serde(default)]
    pub export: Vec<String>,
    /// Cria uma sessão.
    #[serde(default)]
    pub create: Vec<String>,
    /// Apaga uma sessão.
    #[serde(default)]
    pub delete: Vec<String>,
    /// Confiança da seção.
    #[serde(default)]
    pub confidence: String,
}

/// Seção `[accounts]`.
#[derive(Clone, Debug, Default, Deserialize)]
pub struct Accounts {
    /// Variável de ambiente que aponta para a home de configuração.
    #[serde(default)]
    pub env: Option<String>,
    /// Argumentos do comando de login do CLI.
    #[serde(default)]
    pub login: Vec<String>,
}

/// Seção `[hooks]`.
#[derive(Clone, Debug, Default, Deserialize)]
pub struct Hooks {
    /// O CLI chama hooks externos.
    #[serde(default)]
    pub supported: bool,
    /// `command` ou `plugin`.
    #[serde(default)]
    pub mechanism: String,
    /// Arquivos que declaram hooks.
    #[serde(default)]
    pub config: Vec<String>,
    /// Chave dentro desses arquivos.
    #[serde(default)]
    pub key: String,
    /// Eventos documentados.
    #[serde(default)]
    pub events: Vec<String>,
}

/// Seção `[sources]`.
#[derive(Clone, Debug, Default, Deserialize)]
pub struct Sources {
    /// Data de leitura.
    #[serde(default)]
    pub date: String,
    /// Páginas oficiais.
    #[serde(default)]
    pub urls: Vec<String>,
    /// Comandos executados.
    #[serde(default)]
    pub commands: Vec<String>,
    /// Confiança da fonte.
    #[serde(default)]
    pub confidence: String,
}

/// Um perfil de ajudante validado.
#[derive(Clone, Debug, Default, Deserialize)]
pub struct Profile {
    /// Id estável em inglês.
    pub id: String,
    /// Nome visível.
    pub name: String,
    /// Mantenedor do CLI.
    pub maker: String,
    /// Documentação principal.
    pub site: String,
    /// `discontinued` ou `migrated`, quando houver.
    #[serde(default)]
    pub status: Option<String>,
    /// Detecção.
    #[serde(default)]
    pub detect: Detect,
    /// Início.
    #[serde(default)]
    pub start: Start,
    /// Prompt inicial.
    #[serde(default)]
    pub prompt: Prompt,
    /// Modos de permissão.
    #[serde(default)]
    pub permissions: BTreeMap<String, PermissionSpec>,
    /// Retomar.
    #[serde(default)]
    pub resume: Resume,
    /// Bifurcar.
    #[serde(default)]
    pub fork: Fork,
    /// Modelos.
    #[serde(default)]
    pub models: Models,
    /// Sessões.
    #[serde(default)]
    pub sessions: Sessions,
    /// Hooks.
    #[serde(default)]
    pub hooks: Hooks,
    /// Contas com home de configuração separada.
    #[serde(default)]
    pub accounts: Accounts,
    /// Fontes.
    #[serde(default)]
    pub sources: Sources,
    /// Arquivo de origem.
    #[serde(skip)]
    pub source_path: PathBuf,
    /// Camada.
    #[serde(skip)]
    pub layer: Option<Layer>,
    /// Projeto com risco foi aprovado pelo trust.
    #[serde(skip)]
    pub trusted: bool,
    /// Motivo de bloqueio de um perfil de projeto.
    #[serde(skip)]
    pub risk_reason: Option<String>,
    /// Hash do arquivo.
    #[serde(skip)]
    pub content_hash: String,
}

impl Profile {
    /// Lê um texto TOML e valida os campos obrigatórios.
    pub fn parse(text: &str, source: PathBuf, layer: Layer) -> Result<Self, String> {
        let mut profile: Profile =
            toml::from_str(text).map_err(|error| format!("{}: {error}", source.display()))?;
        profile.source_path = source;
        profile.layer = Some(layer);
        profile.content_hash = sha256_hex(text);
        profile.validate()?;
        Ok(profile)
    }

    fn validate(&self) -> Result<(), String> {
        for (field, value) in [
            ("id", &self.id),
            ("name", &self.name),
            ("maker", &self.maker),
            ("site", &self.site),
        ] {
            if value.trim().is_empty() {
                return Err(format!(
                    "{}: `{field}` is required",
                    self.source_path.display()
                ));
            }
        }
        if !self
            .id
            .chars()
            .all(|ch| ch.is_ascii_lowercase() || ch.is_ascii_digit() || ch == '-')
        {
            return Err(format!(
                "{}: id `{}` must be lowercase letters, digits and hyphens",
                self.source_path.display(),
                self.id
            ));
        }
        if self.detect.executables.is_empty() {
            return Err(format!(
                "{}: detect.executables cannot be empty",
                self.source_path.display()
            ));
        }
        if self.start.command.trim().is_empty() {
            return Err(format!(
                "{}: start.command is required",
                self.source_path.display()
            ));
        }
        let mode = self.prompt.mode.as_str();
        if !matches!(mode, "" | "argument" | "flag" | "stdin" | "none") {
            return Err(format!(
                "{}: prompt.mode `{mode}` is not argument, flag, stdin or none",
                self.source_path.display()
            ));
        }
        if mode == "flag"
            && self
                .prompt
                .flag
                .as_deref()
                .unwrap_or_default()
                .trim()
                .is_empty()
        {
            return Err(format!(
                "{}: prompt.flag is required with mode = flag",
                self.source_path.display()
            ));
        }
        for key in self.permissions.keys() {
            if !matches!(key.as_str(), "ask" | "accept-edits" | "automatic" | "full") {
                return Err(format!(
                    "{}: unknown permission mode `{key}`",
                    self.source_path.display()
                ));
            }
        }
        for event in &self.hooks.events {
            if event.is_empty()
                || event.len() > 64
                || !event
                    .chars()
                    .all(|ch| ch.is_ascii_alphanumeric() || matches!(ch, '_' | '-' | '.' | ':'))
            {
                return Err(format!(
                    "{}: hook event `{event}` contains unsupported characters",
                    self.source_path.display()
                ));
            }
        }
        Ok(())
    }

    /// Perfil marcado como descontinuado.
    pub fn discontinued(&self) -> bool {
        self.status.as_deref().is_some_and(|status| {
            status.eq_ignore_ascii_case("discontinued") || status.eq_ignore_ascii_case("migrated")
        })
    }

    /// Perfil de projeto que pede permissões elevadas ou escreve configuração externa.
    pub fn has_risk(&self) -> bool {
        self.permissions.contains_key("automatic")
            || self.permissions.contains_key("full")
            || self.hooks.supported
    }

    /// Quantos recursos confirmados, de 0 a 10.
    pub fn completeness(&self) -> usize {
        let mut count = 0;
        for mode in ["ask", "accept-edits", "automatic", "full"] {
            if self.permissions.contains_key(mode) {
                count += 1;
            }
        }
        if !self.resume.last.is_empty() {
            count += 1;
        }
        if !self.resume.id.is_empty() {
            count += 1;
        }
        if !(self.fork.last.is_empty() && self.fork.id.is_empty() && self.fork.adds.is_empty()) {
            count += 1;
        }
        if !self.models.model_args.is_empty() {
            count += 1;
        }
        if !self.sessions.path.is_empty() {
            count += 1;
        }
        if self.hooks.supported {
            count += 1;
        }
        count
    }

    /// Rank da confiança da fonte (maior é melhor).
    pub fn confidence_rank(&self) -> u8 {
        match self.sources.confidence.as_str() {
            "high" => 2,
            "medium" => 1,
            _ => 0,
        }
    }
}

/// Carrega todas as camadas. `trust_check` decide perfis de projeto com risco.
pub fn load_all(
    project: Option<&Path>,
    mut trust_check: impl FnMut(&Profile) -> bool,
) -> Vec<Profile> {
    let mut by_id: BTreeMap<String, Profile> = BTreeMap::new();
    for text in BUILTIN {
        if let Ok(mut profile) =
            Profile::parse(text, PathBuf::from("content/helpers"), Layer::Builtin)
        {
            profile.trusted = true;
            by_id.insert(profile.id.clone(), profile);
        }
    }
    let user = user_root().join("helpers");
    for mut profile in read_dir(&user, Layer::User) {
        profile.trusted = true;
        by_id.insert(profile.id.clone(), profile);
    }
    if let Some(project) = project {
        for mut profile in read_dir(&project.join(".forja").join("helpers"), Layer::Project) {
            if profile.has_risk() && !trust_check(&profile) {
                profile.trusted = false;
                profile.risk_reason = Some("project profile needs trust approval".to_owned());
            } else {
                profile.trusted = true;
            }
            by_id.insert(profile.id.clone(), profile);
        }
    }
    by_id.into_values().collect()
}

fn read_dir(directory: &Path, layer: Layer) -> Vec<Profile> {
    let Ok(entries) = std::fs::read_dir(directory) else {
        return Vec::new();
    };
    let mut files: Vec<PathBuf> = entries
        .filter_map(Result::ok)
        .map(|entry| entry.path())
        .filter(|path| {
            path.extension()
                .is_some_and(|extension| extension == "toml")
        })
        .collect();
    files.sort();
    files
        .into_iter()
        .filter_map(|path| {
            let text = std::fs::read_to_string(&path).ok()?;
            Profile::parse(&text, path, layer).ok()
        })
        .collect()
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

/// SHA-256 do texto do perfil.
pub fn sha256_hex(text: &str) -> String {
    let digest = Sha256::digest(text.as_bytes());
    let mut output = String::with_capacity(64);
    for byte in digest {
        use std::fmt::Write;
        let _ = write!(output, "{byte:02x}");
    }
    output
}

/// Primeiro perfil instalado na ordem de `design/helpers.md`.
pub fn pick_default<'a>(
    profiles: &'a [Profile],
    installed: impl Fn(&str) -> bool,
    explicit: &str,
) -> Option<&'a Profile> {
    if explicit != "auto" {
        if let Some(profile) = profiles.iter().find(|profile| profile.id == explicit) {
            return Some(profile);
        }
    }
    let mut candidates: Vec<&Profile> = profiles
        .iter()
        .filter(|profile| !profile.discontinued() && profile.trusted && installed(&profile.id))
        .collect();
    candidates.sort_by(|a, b| {
        b.completeness()
            .cmp(&a.completeness())
            .then_with(|| b.confidence_rank().cmp(&a.confidence_rank()))
            .then_with(|| a.id.cmp(&b.id))
    });
    candidates.first().copied()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn all_sixteen_builtin_profiles_are_valid() {
        let profiles = load_all(None, |_| true);
        assert_eq!(profiles.len(), 16);
        assert!(profiles
            .iter()
            .all(|profile| !profile.start.command.is_empty()));
        assert!(profiles
            .iter()
            .all(|profile| profile.source_path.exists() || profile.layer == Some(Layer::Builtin)));
    }

    struct TempGuard(PathBuf);

    impl Drop for TempGuard {
        fn drop(&mut self) {
            let _ = std::fs::remove_dir_all(&self.0);
        }
    }

    #[test]
    fn project_risk_requires_trust() {
        let dir = TempGuard(std::env::temp_dir().join(format!(
            "forja-helper-profile-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        )));
        let helpers = dir.0.join(".forja").join("helpers");
        std::fs::create_dir_all(&helpers).unwrap();
        std::fs::write(
            helpers.join("custom.toml"),
            r#"
id = "custom"
name = "Custom"
maker = "Test"
site = "https://example.test"
[detect]
executables = ["custom"]
[start]
command = "custom"
[prompt]
mode = "argument"
[permissions.full]
args = ["--yes"]
"#,
        )
        .unwrap();
        std::fs::write(
            helpers.join("hooked.toml"),
            r#"
id = "hooked"
name = "Hooked"
maker = "Test"
site = "https://example.test"
[detect]
executables = ["hooked"]
[start]
command = "hooked"
[hooks]
supported = true
mechanism = "command"
config = ["~/.fake/settings.json"]
events = ["Stop"]
"#,
        )
        .unwrap();
        let denied = load_all(Some(&dir.0), |_| false);
        let custom = denied
            .iter()
            .find(|profile| profile.id == "custom")
            .unwrap();
        assert!(!custom.trusted);
        assert!(custom.risk_reason.is_some());
        assert!(!denied
            .iter()
            .find(|profile| profile.id == "hooked")
            .unwrap()
            .trusted);
        let allowed = load_all(Some(&dir.0), |_| true);
        assert!(allowed.iter().find(|profile| profile.id == "custom").unwrap().trusted);
        assert!(allowed
            .iter()
            .find(|profile| profile.id == "hooked")
            .unwrap()
            .trusted);
    }

    #[test]
    fn validation_rejects_unknown_prompt_mode() {
        let error = Profile::parse(
            "id = \"x\"\nname = \"X\"\nmaker = \"M\"\nsite = \"S\"\n[detect]\nexecutables=[\"x\"]\n[start]\ncommand=\"x\"\n[prompt]\nmode=\"weird\"\n",
            PathBuf::from("x.toml"),
            Layer::User,
        )
        .unwrap_err();
        assert!(error.contains("prompt.mode"));
    }

    #[test]
    fn validation_rejects_shell_metacharacters_in_hook_events() {
        let text = r#"
id = "hooked"
name = "Hooked"
maker = "Test"
site = "https://example.test"
[detect]
executables = ["hooked"]
[start]
command = "hooked"
[hooks]
supported = true
events = ["Stop; malicious"]
"#;
        let error = Profile::parse(text, PathBuf::from("hooked.toml"), Layer::User).unwrap_err();
        assert!(error.contains("hook event"));
    }
}
