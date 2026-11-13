//! Remoto `origin` do repositório: lê a URL, separa host e caminho e confere o host
//! configurado antes de qualquer token sair da máquina.

use forja_contracts::CounterError;
use std::path::Path;
use std::process::{Command, Stdio};

/// Host e caminho de um repositório remoto, sem usuário, senha nem `.git`.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RemoteRepo {
    pub host: String,
    /// `dono/nome` no GitHub; `grupo/subgrupo/projeto` no GitLab.
    pub path: String,
}

/// Lê HTTPS, HTTP, SSH (`ssh://`, `git+ssh://` e `git@host:caminho`).
pub fn parse_remote(url: &str) -> Option<RemoteRepo> {
    let url = url.trim();
    let (host, path) = if let Some(rest) = url.strip_prefix("git@") {
        rest.split_once(':')?
    } else {
        let rest = url
            .strip_prefix("ssh://")
            .or_else(|| url.strip_prefix("git+ssh://"))
            .or_else(|| url.strip_prefix("https://"))
            .or_else(|| url.strip_prefix("http://"))?;
        let rest = rest.rsplit_once('@').map(|(_, rest)| rest).unwrap_or(rest);
        rest.split_once('/')?
    };
    let path = path.trim_matches('/');
    let path = path.strip_suffix(".git").unwrap_or(path);
    let segments: Vec<&str> = path.split('/').filter(|part| !part.is_empty()).collect();
    if segments.len() < 2 || !segments.iter().all(|segment| is_plain_segment(segment)) {
        return None;
    }
    let host = host.trim().to_ascii_lowercase();
    if host.is_empty() {
        return None;
    }
    Some(RemoteRepo {
        host,
        path: segments.join("/"),
    })
}

/// Segmento de caminho que pode ir para uma URL sem escapes: nem vazio, nem `.`, nem `..`.
pub fn is_plain_segment(segment: &str) -> bool {
    !segment.is_empty()
        && segment != "."
        && segment != ".."
        && segment
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || matches!(c, '-' | '_' | '.'))
}

/// Recusa o envio quando o remoto aponta para outro host do que o configurado.
pub fn check_host(remote_host: &str, configured_host: &str) -> Result<(), CounterError> {
    let configured = configured_host.trim();
    if !configured.is_empty() && remote_host.trim().eq_ignore_ascii_case(configured) {
        return Ok(());
    }
    Err(CounterError::Config {
        message: format!(
            "origin points to {remote_host}, but the counter is configured for {configured}"
        ),
    })
}

/// URL do remoto `origin` da pasta, lida com `git` sem prompt e sem stdin.
pub fn read_origin(dir: &Path) -> Result<String, CounterError> {
    let output = Command::new("git")
        .arg("-C")
        .arg(dir)
        .args(["remote", "get-url", "origin"])
        .env("GIT_TERMINAL_PROMPT", "0")
        .stdin(Stdio::null())
        .stderr(Stdio::null())
        .output()
        .map_err(|error| CounterError::Config {
            message: format!("git is not available: {error}"),
        })?;
    if !output.status.success() {
        return Err(CounterError::Config {
            message: "the project has no origin remote".to_owned(),
        });
    }
    let url = String::from_utf8_lossy(&output.stdout).trim().to_owned();
    if url.is_empty() {
        return Err(CounterError::Config {
            message: "the project has no origin remote".to_owned(),
        });
    }
    Ok(url)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_https_ssh_and_scp_forms() {
        let expected = RemoteRepo {
            host: "github.com".into(),
            path: "acme/tools".into(),
        };
        assert_eq!(parse_remote("https://github.com/acme/tools.git"), Some(expected.clone()));
        assert_eq!(parse_remote("git@github.com:acme/tools.git"), Some(expected.clone()));
        assert_eq!(parse_remote("ssh://git@GitHub.com/acme/tools/"), Some(expected));
    }

    #[test]
    fn credentials_in_the_url_are_dropped() {
        let remote = parse_remote("https://user:ghp_secret@github.com/acme/tools").unwrap();
        assert_eq!(remote.host, "github.com");
        assert!(!remote.path.contains("ghp_secret"));
    }

    #[test]
    fn gitlab_subgroups_are_kept_and_bad_paths_are_refused() {
        let remote = parse_remote("https://gitlab.com/grupo/sub/app.git").unwrap();
        assert_eq!(remote.path, "grupo/sub/app");
        assert_eq!(parse_remote("https://github.com/"), None);
        assert_eq!(parse_remote("https://github.com/acme/../x"), None);
        assert_eq!(parse_remote("file:///tmp/repo"), None);
    }

    #[test]
    fn token_never_goes_to_a_host_other_than_the_configured_one() {
        assert!(check_host("GitHub.com", "github.com").is_ok());
        let error = check_host("evil.example", "github.com").unwrap_err();
        assert!(matches!(error, CounterError::Config { .. }));
        assert!(check_host("github.com", "").is_err());
    }

    #[test]
    fn missing_origin_is_a_config_error() {
        let dir = std::env::temp_dir().join("forja-counter-kit-no-git-here");
        let _ = std::fs::create_dir_all(&dir);
        let result = read_origin(&dir);
        assert!(result.is_err());
        let _ = std::fs::remove_dir_all(dir);
    }
}
