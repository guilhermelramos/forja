//! Instalação idempotente de hooks e leitura dos eventos de status.

use crate::profile::Profile;
use forja_contracts::HelperStatus;
use forja_kernel::store::write_atomic_bytes;
use serde_json::{json, Value};
use std::path::{Path, PathBuf};

const MARKER: &str = "forja-hook";
pub const HOOK_TOKEN_ENV: &str = "FORJA_HOOK_TOKEN";

/// Estado da instalação de hooks de um CLI.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum InstallState {
    /// Todas as entradas gerenciadas estão presentes.
    Installed,
    /// Não há entradas gerenciadas.
    NotInstalled,
    /// Só parte das entradas está presente.
    Partial,
    /// O perfil não declara hooks em JSON.
    Unsupported,
    /// Arquivo inválido ou erro de escrita.
    Error,
}

impl InstallState {
    /// Texto estável para o RPC e a interface.
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Installed => "installed",
            Self::NotInstalled => "not_installed",
            Self::Partial => "partial",
            Self::Unsupported => "unsupported",
            Self::Error => "error",
        }
    }
}

/// Resultado de status/instalação.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct HookStatus {
    /// Estado.
    pub state: InstallState,
    /// Arquivo usado.
    pub config_path: Option<PathBuf>,
    /// Explicação curta.
    pub detail: Option<String>,
}

/// Verdadeiro só fora de testes/capturas e sem dados isolados, salvo opt-in.
pub fn global_writes_allowed_with(
    opt_in: bool,
    data_dir_overridden: bool,
    testing: bool,
    screenshot: bool,
) -> bool {
    !testing && !screenshot && (opt_in || !data_dir_overridden)
}

/// Decide se este processo pode escrever configs globais de outros CLIs.
pub fn global_writes_allowed() -> bool {
    global_writes_allowed_with(
        std::env::var_os("FORJA_ALLOW_GLOBAL_CONFIG").is_some(),
        std::env::var_os("FORJA_DATA_DIR").is_some(),
        cfg!(test),
        std::env::var_os("FORJA_SCREENSHOT").is_some(),
    )
}

/// Chave de consentimento que muda quando muda o perfil, sua origem ou o arquivo externo.
pub fn consent_key(profile: &Profile) -> String {
    let target = config_path(profile)
        .map(|path| path.display().to_string())
        .unwrap_or_default();
    format!(
        "{}|{}|{}|{}",
        profile.id,
        profile.source_path.display(),
        profile.content_hash,
        target
    )
}

/// Cria um segredo de 256 bits para correlacionar hooks a uma execução.
pub fn new_run_token() -> Result<String, String> {
    #[cfg(windows)]
    {
        use windows_sys::Win32::Security::Cryptography::{
            BCryptGenRandom, BCRYPT_USE_SYSTEM_PREFERRED_RNG,
        };
        let mut bytes = [0_u8; 32];
        // SAFETY: the buffer is writable for its full length; the system RNG
        // does not retain the pointer.
        let status = unsafe {
            BCryptGenRandom(
                std::ptr::null_mut(),
                bytes.as_mut_ptr(),
                bytes.len() as u32,
                BCRYPT_USE_SYSTEM_PREFERRED_RNG,
            )
        };
        if status < 0 {
            return Err(format!(
                "could not create hook authorization token ({status})"
            ));
        }
        let mut token = String::with_capacity(bytes.len() * 2);
        for byte in bytes {
            use std::fmt::Write;
            let _ = write!(token, "{byte:02x}");
        }
        Ok(token)
    }
    #[cfg(not(windows))]
    {
        Err("helper hook tokens are unavailable on this platform".to_owned())
    }
}

/// Status das entradas gerenciadas.
pub fn status(profile: &Profile) -> HookStatus {
    if profile.hooks.events.is_empty() {
        return HookStatus {
            state: InstallState::Unsupported,
            config_path: None,
            detail: Some("profile declares no hook events".to_owned()),
        };
    }
    let Some(path) = config_path(profile) else {
        return HookStatus {
            state: InstallState::Unsupported,
            config_path: None,
            detail: None,
        };
    };
    let text = match std::fs::read_to_string(&path) {
        Ok(text) => text,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
            return HookStatus {
                state: InstallState::NotInstalled,
                config_path: Some(path),
                detail: None,
            }
        }
        Err(error) => {
            return HookStatus {
                state: InstallState::Error,
                config_path: Some(path),
                detail: Some(error.to_string()),
            }
        }
    };
    let root = match serde_json::from_str::<Value>(&text) {
        Ok(root) => root,
        Err(error) => {
            return HookStatus {
                state: InstallState::Error,
                config_path: Some(path),
                detail: Some(error.to_string()),
            }
        }
    };
    let found = profile
        .hooks
        .events
        .iter()
        .filter(|event| {
            root.get("hooks")
                .and_then(|hooks| hooks.get(*event))
                .and_then(Value::as_array)
                .is_some_and(|groups| {
                    groups
                        .iter()
                        .any(|group| group_has_marker(group, &profile.id))
                })
        })
        .count();
    let state = if found == 0 {
        InstallState::NotInstalled
    } else if found == profile.hooks.events.len() {
        InstallState::Installed
    } else {
        InstallState::Partial
    };
    HookStatus {
        state,
        config_path: Some(path),
        detail: None,
    }
}

/// Instala hooks com consentimento explícito.
pub fn install(profile: &Profile, consent: bool, timeout_s: u64) -> Result<HookStatus, String> {
    if !consent {
        return Err("hook consent is required".to_owned());
    }
    if !global_writes_allowed() {
        return Err("global config writes are disabled in isolated runs".to_owned());
    }
    install_at(profile, consent, true, timeout_s)
}

/// Remove hooks com consentimento explícito.
pub fn remove(profile: &Profile, consent: bool) -> Result<HookStatus, String> {
    if !consent {
        return Err("hook consent is required".to_owned());
    }
    if !global_writes_allowed() {
        return Err("global config writes are disabled in isolated runs".to_owned());
    }
    install_at(profile, consent, false, 10)
}

fn install_at(
    profile: &Profile,
    consent: bool,
    install: bool,
    timeout_s: u64,
) -> Result<HookStatus, String> {
    if !consent {
        return Err("hook consent is required".to_owned());
    }
    let Some(path) = config_path(profile) else {
        return Err("profile has no JSON hook configuration".to_owned());
    };
    let mut root = match std::fs::read_to_string(&path) {
        Ok(text) => serde_json::from_str::<Value>(&text)
            .map_err(|error| format!("{}: {error}", path.display()))?,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => json!({}),
        Err(error) => return Err(error.to_string()),
    };
    if !root.is_object() {
        return Err(format!("{} is not a JSON object", path.display()));
    }
    if path.exists() {
        let backup = path.with_extension("json.forja-bak");
        if !backup.exists() {
            let _ = std::fs::copy(&path, &backup);
        }
    }
    let hooks = root
        .as_object_mut()
        .expect("checked object")
        .entry("hooks")
        .or_insert_with(|| json!({}));
    if !hooks.is_object() {
        return Err(format!(
            "{} has a non-object \"hooks\" value",
            path.display()
        ));
    }
    let hooks = hooks.as_object_mut().expect("checked object");
    for event in &profile.hooks.events {
        let groups = hooks.entry(event.clone()).or_insert_with(|| json!([]));
        let Some(groups) = groups.as_array_mut() else {
            return Err(format!("{} hooks.{event} is not an array", path.display()));
        };
        groups.retain(|group| !group_has_marker(group, &profile.id));
        if install {
            groups.push(json!({
                "hooks": [{
                    "type": "command",
                    "command": hook_command(profile, event),
                    "timeout": timeout_s
                }]
            }));
        }
        if groups.is_empty() {
            hooks.remove(event);
        }
    }
    let bytes = serde_json::to_vec_pretty(&root).map_err(|error| error.to_string())?;
    write_atomic_bytes(&path, &bytes).map_err(|error| error.to_string())?;
    Ok(status(profile))
}

fn hook_command(profile: &Profile, event: &str) -> String {
    let exe = hook_exe();
    format!(
        "\"{}\" --agent {} --event {} --pipe forja-helper",
        exe.display(),
        profile.id,
        event
    )
}

fn hook_exe() -> PathBuf {
    std::env::current_exe()
        .ok()
        .and_then(|exe| exe.parent().map(|dir| dir.join("forja-hook.exe")))
        .unwrap_or_else(|| PathBuf::from("forja-hook.exe"))
}

pub fn config_path(profile: &Profile) -> Option<PathBuf> {
    profile
        .hooks
        .config
        .iter()
        .find(|path| path.ends_with(".json") && !path.starts_with('<'))
        .map(|path| expand_home(path))
}

fn expand_home(path: &str) -> PathBuf {
    if let Some(rest) = path.strip_prefix("~/") {
        if let Some(home) = std::env::var_os("USERPROFILE").or_else(|| std::env::var_os("HOME")) {
            return PathBuf::from(home).join(rest);
        }
    }
    Path::new(path).to_path_buf()
}

fn group_has_marker(group: &Value, profile_id: &str) -> bool {
    let marker = format!("--agent {profile_id}");
    group
        .get("hooks")
        .and_then(Value::as_array)
        .is_some_and(|hooks| {
            hooks.iter().any(|hook| {
                hook.get("command")
                    .and_then(Value::as_str)
                    .is_some_and(|command| command.contains(MARKER) && command.contains(&marker))
            })
        })
}

/// Mapeia um payload de hook para o status grosso do ajudante.
pub fn map_event(payload: &Value) -> Option<HelperStatus> {
    let event = [
        "forja_event",
        "hook_event_name",
        "event_name",
        "event",
        "type",
        "name",
    ]
    .iter()
    .find_map(|key| payload.get(key).and_then(Value::as_str))
    .or_else(|| {
        payload
            .get("hook")
            .and_then(|hook| hook.get("event"))
            .and_then(Value::as_str)
    })?;
    let event = event.to_ascii_lowercase();
    if event.contains("permission")
        || event.contains("elicitation")
        || event.contains("notification")
        || event.contains("ask")
    {
        Some(HelperStatus::NeedsYou)
    } else if event.contains("stop")
        || event.contains("sessionend")
        || event.contains("complete")
        || event.contains("failure")
        || event.contains("error")
    {
        Some(HelperStatus::Finished)
    } else if event.contains("sessionstart")
        || event.contains("pretool")
        || event.contains("posttool")
        || event.contains("promptsubmit")
        || event.contains("subagentstart")
        || event.contains("premodel")
        || event.contains("postmodel")
    {
        Some(HelperStatus::Working)
    } else if event.contains("idle") {
        Some(HelperStatus::Idle)
    } else {
        None
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::profile::Layer;
    use std::path::PathBuf;

    fn profile() -> Profile {
        let mut profile = Profile::parse(
            r#"
id = "claude-code"
name = "Claude Code"
maker = "Test"
site = "https://example.test"
[detect]
executables = ["claude"]
[start]
command = "claude"
[prompt]
mode = "argument"
[hooks]
supported = true
mechanism = "command"
config = ["~/.forja-test-claude/settings.json"]
events = ["PreToolUse", "Notification", "Stop"]
"#,
            PathBuf::from("claude-code.toml"),
            Layer::User,
        )
        .unwrap();
        profile.trusted = true;
        profile
    }

    #[test]
    fn consent_is_required_before_any_write() {
        let profile = profile();
        let error = install_at(&profile, false, true, 10).unwrap_err();
        assert!(error.contains("consent"));
    }

    struct TempGuard(std::path::PathBuf);

    impl Drop for TempGuard {
        fn drop(&mut self) {
            let _ = std::fs::remove_dir_all(&self.0);
        }
    }

    #[test]
    fn install_and_remove_only_managed_entries() {
        let dir = TempGuard(std::env::temp_dir().join(format!(
            "forja-helper-hooks-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        )));
        std::fs::create_dir_all(&dir.0).unwrap();
        let path = dir.0.join("settings.json");
        std::fs::write(
            &path,
            r#"{"keep":true,"hooks":{"PreToolUse":[{"matcher":"Bash","hooks":[{"type":"command","command":"other"}]}]}}"#,
        )
        .unwrap();
        let mut profile = profile();
        profile.hooks.config = vec![path.display().to_string()];
        let installed = install_at(&profile, true, true, 10).unwrap();
        assert_eq!(installed.state, InstallState::Installed);
        let root: Value = serde_json::from_str(&std::fs::read_to_string(&path).unwrap()).unwrap();
        assert_eq!(root["keep"], true);
        assert_eq!(
            root["hooks"]["PreToolUse"][0]["hooks"][0]["command"],
            "other"
        );
        assert!(root["hooks"]["Notification"][0]["hooks"][0]["command"]
            .as_str()
            .unwrap()
            .contains("forja-hook"));
        let removed = install_at(&profile, true, false, 10).unwrap();
        assert_eq!(removed.state, InstallState::NotInstalled);
        let root: Value = serde_json::from_str(&std::fs::read_to_string(&path).unwrap()).unwrap();
        assert_eq!(
            root["hooks"]["PreToolUse"][0]["hooks"][0]["command"],
            "other"
        );
    }

    #[test]
    fn isolated_runs_never_write_global_configs() {
        assert!(!global_writes_allowed_with(false, true, false, false));
        assert!(global_writes_allowed_with(true, true, false, false));
        assert!(!global_writes_allowed_with(true, false, true, false));
    }

    #[test]
    fn screenshot_never_allows_global_config_writes_even_with_opt_in() {
        assert!(!global_writes_allowed_with(true, true, false, true));
        assert!(!global_writes_allowed_with(true, false, false, true));
    }

    #[test]
    fn hook_consent_is_bound_to_profile_content() {
        let first = profile();
        let mut changed = profile();
        changed.content_hash = "changed-hash".into();
        assert_ne!(consent_key(&first), consent_key(&changed));
    }

    #[test]
    fn hook_events_map_to_status() {
        assert_eq!(
            map_event(&json!({"hook_event_name": "PermissionRequest"})),
            Some(HelperStatus::NeedsYou)
        );
        assert_eq!(
            map_event(&json!({"event": "PreToolUse"})),
            Some(HelperStatus::Working)
        );
        assert_eq!(
            map_event(&json!({"hook_event_name": "Stop"})),
            Some(HelperStatus::Finished)
        );
        assert_eq!(
            map_event(&json!({"event": "TeammateIdle"})),
            Some(HelperStatus::Idle)
        );
        assert_eq!(map_event(&json!({"event": "Unknown"})), None);
    }
}
