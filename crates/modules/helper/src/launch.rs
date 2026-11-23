//! Montagem da linha de comando de um ajudante.

use crate::profile::Profile;
use std::collections::BTreeMap;
use std::path::PathBuf;

/// Pedido aceito por `helper.launch`.
#[derive(Clone, Debug, Default)]
pub struct LaunchRequest {
    /// Perfil escolhido; vazio usa o padrão instalado.
    pub profile_id: Option<String>,
    /// Bancada onde o terminal abre.
    pub bench_path: Option<String>,
    /// Prompt inicial.
    pub prompt: Option<String>,
    /// Modo de permissão do catálogo ou do perfil.
    pub permission: Option<String>,
    /// Modelo opcional.
    pub model: Option<String>,
    /// Esforço opcional.
    pub effort: Option<String>,
    /// Referência do pedido associada à bancada (`from_request`).
    pub from_request: Option<forja_contracts::RequestRef>,
    /// Sobrescrita opcional para `helper.launch.initial_context`.
    pub initial_context: Option<String>,
}

/// Ajustes do usuário para um perfil.
#[derive(Clone, Debug, Default, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct ProfileOverride {
    /// Perfil desabilitado.
    pub disabled: bool,
    /// Argumentos extras.
    pub extra_args: Vec<String>,
    /// Variáveis extras.
    pub environment: BTreeMap<String, String>,
}

/// Plano pronto para `terminal.open`.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct LaunchPlan {
    /// Perfil usado.
    pub profile_id: String,
    /// Pasta do processo.
    pub cwd: PathBuf,
    /// Executável.
    pub program: String,
    /// Argumentos em ordem.
    pub args: Vec<String>,
    /// Variáveis extras.
    pub environment: BTreeMap<String, String>,
    /// Prompt que precisa ser digitado depois que a TUI abrir.
    pub delayed_prompt: Option<String>,
}

impl LaunchPlan {
    /// Merges an `accounts.env` response (`{ "env": { ... } }`) into the plan.
    pub fn merge_env(&mut self, source: &serde_json::Value) {
        if let Some(environment) = source.get("env").and_then(serde_json::Value::as_object) {
            for (key, value) in environment {
                if let Some(value) = value.as_str() {
                    self.environment.insert(key.clone(), value.to_owned());
                }
            }
        }
    }

    /// Texto exibido no relatório da entrega.
    pub fn command_line(&self) -> String {
        let mut line = quote_arg(&self.program);
        for arg in &self.args {
            line.push(' ');
            line.push_str(&quote_arg(arg));
        }
        line
    }

    /// Argumentos num único texto para o campo `args` de `terminal.open`.
    pub fn args_text(&self) -> String {
        self.args
            .iter()
            .map(|arg| quote_arg(arg))
            .collect::<Vec<_>>()
            .join(" ")
    }
}

/// Monta o plano do perfil.
pub fn build(
    profile: &Profile,
    request: &LaunchRequest,
    default_permission: &str,
    cwd: PathBuf,
    overrides: &ProfileOverride,
) -> Result<LaunchPlan, String> {
    if overrides.disabled {
        return Err(format!("helper profile `{}` is disabled", profile.id));
    }
    if !profile.trusted {
        return Err(profile
            .risk_reason
            .clone()
            .unwrap_or_else(|| format!("helper profile `{}` is not trusted", profile.id)));
    }
    let permission =
        normalize_permission(request.permission.as_deref().unwrap_or(default_permission));
    let mut args = profile.start.args.clone();
    let mut environment = BTreeMap::new();
    if let Some(spec) = profile.permissions.get(permission) {
        args.extend(spec.args.clone());
        environment.extend(spec.env.clone());
    }
    let prompt = request
        .prompt
        .as_deref()
        .map(str::trim)
        .filter(|prompt| !prompt.is_empty());
    // Nenhum texto não confiável entra no argv: shims `.cmd`/`.bat` são
    // interpretados pelo cmd.exe, onde `&` e `>` executam comandos. O prompt
    // completa vai para o PTY depois que o terminal abrir.
    let mut delayed_prompt = None;
    if let Some(prompt) = prompt {
        if profile.prompt.mode == "flag" {
            if let Some(flag) = &profile.prompt.flag {
                args.push(flag.clone());
            }
        }
        delayed_prompt = Some(prompt.to_owned());
    }
    if let Some(model) = &request.model {
        if !profile.models.model_args.is_empty() {
            args.extend(
                profile
                    .models
                    .model_args
                    .iter()
                    .map(|arg| arg.replace("{model}", model)),
            );
        } else if let Some(name) = &profile.models.model_env {
            environment.insert(name.clone(), model.clone());
        }
    }
    if let Some(effort) = &request.effort {
        if !profile.models.effort_args.is_empty() {
            args.extend(
                profile
                    .models
                    .effort_args
                    .iter()
                    .map(|arg| arg.replace("{effort}", effort)),
            );
        } else if let Some(name) = &profile.models.effort_env {
            environment.insert(name.clone(), effort.clone());
        }
    }
    args.extend(overrides.extra_args.clone());
    environment.extend(overrides.environment.clone());
    Ok(LaunchPlan {
        profile_id: profile.id.clone(),
        cwd,
        program: profile.start.command.clone(),
        args,
        environment,
        delayed_prompt,
    })
}

/// Monta o plano de retomada para o perfil quando `[resume]` declara argumentos válidos.
pub fn build_resume(
    profile: &Profile,
    session_id: Option<&str>,
    cwd: PathBuf,
    overrides: &ProfileOverride,
) -> Result<LaunchPlan, String> {
    if overrides.disabled {
        return Err(format!("helper profile `{}` is disabled", profile.id));
    }
    if !profile.trusted {
        return Err(profile
            .risk_reason
            .clone()
            .unwrap_or_else(|| format!("helper profile `{}` is not trusted", profile.id)));
    }
    let clean_session = session_id
        .map(str::trim)
        .filter(|value| !value.is_empty());
    let mut args = if let Some(id) = clean_session {
        if !profile.resume.id.is_empty() {
            profile
                .resume
                .id
                .iter()
                .map(|arg| arg.replace("{id}", id))
                .collect()
        } else {
            profile.resume.last.clone()
        }
    } else {
        profile.resume.last.clone()
    };
    if args.is_empty() {
        return Err(format!(
            "helper profile `{}` does not support resuming sessions",
            profile.id
        ));
    }
    args.extend(overrides.extra_args.clone());
    Ok(LaunchPlan {
        profile_id: profile.id.clone(),
        cwd,
        program: profile.start.command.clone(),
        args,
        environment: overrides.environment.clone(),
        delayed_prompt: None,
    })
}

/// Traduz o valor do catálogo para a chave do perfil.
pub fn normalize_permission(value: &str) -> &'static str {
    match value.trim().to_ascii_lowercase().as_str() {
        "accept-edits" | "accept_edits" => "accept-edits",
        "automatic" | "auto" => "automatic",
        "full" | "full-access" | "total" => "full",
        _ => "ask",
    }
}

/// Cita um argumento para a linha de comando do Windows.
pub fn quote_arg(value: &str) -> String {
    if !value.is_empty() && !value.chars().any(|ch| ch.is_whitespace() || ch == '"') {
        return value.to_owned();
    }
    let mut result = String::from("\"");
    let mut slashes = 0;
    for ch in value.chars() {
        if ch == '\\' {
            slashes += 1;
            continue;
        }
        if ch == '"' {
            result.push_str(&"\\".repeat(slashes * 2 + 1));
        } else {
            result.push_str(&"\\".repeat(slashes));
        }
        slashes = 0;
        result.push(ch);
    }
    result.push_str(&"\\".repeat(slashes * 2));
    result.push('"');
    result
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::profile::Layer;
    use std::path::Path;

    fn profile(id: &str) -> Profile {
        let text = crate::profile_data::BUILTIN
            .iter()
            .find(|text| text.contains(&format!("id = \"{id}\"")))
            .expect("profile");
        let mut profile =
            Profile::parse(text, PathBuf::from("content/helpers"), Layer::Builtin).unwrap();
        profile.trusted = true;
        profile
    }

    #[test]
    fn every_builtin_builds_a_line_with_its_prompt_mode() {
        for text in crate::profile_data::BUILTIN {
            let mut profile =
                Profile::parse(text, PathBuf::from("content/helpers"), Layer::Builtin)
                    .expect("valid profile");
            profile.trusted = true;
            let permission = profile
                .permissions
                .keys()
                .next()
                .cloned()
                .unwrap_or_else(|| "ask".to_owned());
            let plan = build(
                &profile,
                &LaunchRequest {
                    prompt: Some("do the thing".to_owned()),
                    permission: Some(permission),
                    ..LaunchRequest::default()
                },
                "ask",
                Path::new("C:/work").to_path_buf(),
                &ProfileOverride::default(),
            )
            .expect("plan");
            assert!(!plan.program.is_empty());
            assert!(
                !plan.args.iter().any(|arg| arg.contains("do the thing")),
                "untrusted prompt must never enter argv: {}",
                plan.command_line()
            );
            assert_eq!(plan.delayed_prompt.as_deref(), Some("do the thing"));
        }
    }

    #[cfg(windows)]
    #[test]
    fn untrusted_request_text_never_reaches_a_cmd_shim_argv() {
        let dir = std::env::temp_dir().join(format!(
            "forja-helper-shim-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        std::fs::create_dir_all(&dir).unwrap();
        let shim = dir.join("fake-helper.cmd");
        let log = dir.join("args.txt");
        let pwned = dir.join("pwned.txt");
        let shim_path = shim.display().to_string().replace('\\', "/");
        let log_path = log.display().to_string().replace('\\', "/");
        let pwned_path = pwned.display().to_string().replace('\\', "/");
        std::fs::write(
            &shim,
            format!("@echo off\r\n> \"{log_path}\" echo %*\r\n"),
        )
        .unwrap();
        let malicious = format!("& echo PWNED > \"{pwned_path}\"");

        let profile = Profile::parse(
            &format!(
                "id = \"shim\"\nname = \"Shim\"\nmaker = \"Test\"\nsite = \"https://example.test\"\n[detect]\nexecutables=[\"fake-helper.cmd\"]\n[start]\ncommand = \"{shim_path}\"\n[prompt]\nmode = \"argument\"\n"
            ),
            PathBuf::from("shim.toml"),
            Layer::User,
        )
        .unwrap();
        let mut profile = profile;
        profile.trusted = true;
        let plan = build(
            &profile,
            &LaunchRequest {
                prompt: Some(malicious.clone()),
                ..LaunchRequest::default()
            },
            "ask",
            dir.clone(),
            &ProfileOverride::default(),
        )
        .expect("plan");
        assert!(!plan.args_text().contains("PWNED"));
        assert!(!plan.args.iter().any(|arg| arg.contains('&')));

        // É exatamente esta linha que o ConPTY monta para um shim .cmd.
        let conpty_line = format!(
            "\"cmd.exe\" /d /s /c \"\"{}\" {}\"",
            shim_path,
            plan.args_text()
        );
        assert!(!conpty_line.contains("PWNED"), "{conpty_line}");
        assert!(!pwned.exists(), "request text must not create files");
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn permission_names_and_quotes_are_stable() {
        assert_eq!(normalize_permission("accept_edits"), "accept-edits");
        assert_eq!(normalize_permission("auto"), "automatic");
        assert_eq!(normalize_permission("total"), "full");
        assert_eq!(normalize_permission("ask"), "ask");
        assert_eq!(quote_arg("plain"), "plain");
        assert_eq!(quote_arg("two words"), "\"two words\"");
        let profile = profile("claude-code");
        let plan = build(
            &profile,
            &LaunchRequest {
                permission: Some("full".to_owned()),
                ..LaunchRequest::default()
            },
            "ask",
            PathBuf::from("C:/work"),
            &ProfileOverride::default(),
        )
        .unwrap();
        assert!(plan
            .args
            .iter()
            .any(|arg| arg == "--dangerously-skip-permissions"));
    }

    #[test]
    fn account_env_is_merged_into_the_launch_plan() {
        let mut plan = LaunchPlan {
            profile_id: "claude-code".into(),
            cwd: PathBuf::from("C:/work"),
            program: "claude".into(),
            args: Vec::new(),
            environment: BTreeMap::new(),
            delayed_prompt: None,
        };
        plan.merge_env(&serde_json::json!({
            "env": { "CLAUDE_CONFIG_DIR": "C:/accounts/ana" }
        }));
        assert_eq!(
            plan.environment
                .get("CLAUDE_CONFIG_DIR")
                .map(String::as_str),
            Some("C:/accounts/ana")
        );
    }

    #[test]
    fn resume_plan_uses_profile_flags_and_rejects_unsupported_profiles() {
        let claude = profile("claude-code");
        let by_id = build_resume(
            &claude,
            Some("sess-42"),
            PathBuf::from("C:/work"),
            &ProfileOverride::default(),
        )
        .expect("claude supports id resume");
        assert_eq!(by_id.args, vec!["--resume", "sess-42"]);

        let last = build_resume(
            &claude,
            None,
            PathBuf::from("C:/work"),
            &ProfileOverride::default(),
        )
        .expect("claude supports last resume");
        assert_eq!(last.args, vec!["--continue"]);

        let plain = Profile::parse(
            "id = \"no-resume\"\nname = \"NoResume\"\nmaker = \"Test\"\nsite = \"https://example.test\"\n[detect]\nexecutables=[\"x\"]\n[start]\ncommand=\"x\"\n",
            PathBuf::from("no-resume.toml"),
            Layer::User,
        )
        .unwrap();
        let mut plain = plain;
        plain.trusted = true;
        let err = build_resume(
            &plain,
            None,
            PathBuf::from("C:/work"),
            &ProfileOverride::default(),
        )
        .unwrap_err();
        assert!(err.contains("does not support resuming"));
    }
}
