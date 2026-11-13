//! Módulo `privacy` da Forja.
//!
//! Publica a política local de privacidade: histórico, área de transferência,
//! logs, cofre de segredos, uso, ajudante e navegador. Nada sai da máquina: as
//! configurações são lidas do catálogo e expostas por comandos e RPC.

use forja_kernel::forja_protocol::{Method, MethodEffect, ParamKind, ParamSchema, ResultSchema};
use forja_kernel::{
    Ctx, Module, ModuleManifest, Registrar, SettingKind, SettingSpec, SettingValue,
};
use serde_json::{json, Value};
use std::time::Duration;

#[forja_kernel::module]
static PRIVACY: fn() -> Box<dyn Module> = || Box::new(PrivacyModule);

/// Módulo `privacy`: política local e utilidades de proteção de dados.
pub struct PrivacyModule;

impl Module for PrivacyModule {
    fn manifest(&self) -> ModuleManifest {
        ModuleManifest::new("privacy", "privacy.name", env!("CARGO_PKG_VERSION"))
    }

    fn register(&mut self, r: &mut Registrar<'_>) {
        r.i18n(
            include_str!("../i18n/en.ftl"),
            include_str!("../i18n/pt-BR.ftl"),
        );
        register_settings(r);

        r.command("privacy.policy", "privacy.command.policy", |cx, _args| {
            Ok(policy_json(cx))
        });

        r.command("privacy.redact", "privacy.command.redact", |cx, args| {
            let text = args.str("text").unwrap_or("");
            Ok(redact_json(cx, text))
        });

        r.command(
            "privacy.clipboard.schedule_clear",
            "privacy.command.schedule_clear",
            |cx, args| {
                let text = args.str("text").map(str::to_owned);
                Ok(schedule_clear(cx, text))
            },
        );

        r.command(
            "privacy.secrets.status",
            "privacy.command.secrets_status",
            |cx, args| {
                let idle_min = args.i64("idle_min").unwrap_or(0).max(0) as u64;
                Ok(secrets_json(cx, idle_min))
            },
        );

        r.command(
            "privacy.browser.policy",
            "privacy.command.browser_policy",
            |cx, _args| Ok(browser_json(cx)),
        );

        r.rpc(
            Method::new("privacy.policy", "privacy.method.policy.description")
                .effect(MethodEffect::Read)
                .idempotent(true)
                .result(ResultSchema::new("privacy.method.policy.result")),
            |cx, _params| Ok(policy_json(cx)),
        );

        r.rpc(
            Method::new("privacy.redact", "privacy.method.redact.description")
                .effect(MethodEffect::Read)
                .idempotent(true)
                .param(ParamSchema::required("text", ParamKind::String))
                .result(ResultSchema::new("privacy.method.redact.result")),
            |cx, params| {
                let text = params.get("text").and_then(Value::as_str).unwrap_or("");
                Ok(redact_json(cx, text))
            },
        );

        r.rpc(
            Method::new(
                "privacy.clipboard.schedule_clear",
                "privacy.method.schedule_clear.description",
            )
            .effect(MethodEffect::Write)
            .idempotent(false)
            .param(ParamSchema::optional("text", ParamKind::String))
            .result(ResultSchema::new("privacy.method.schedule_clear.result")),
            |cx, params| {
                let text = params
                    .get("text")
                    .and_then(Value::as_str)
                    .map(str::to_owned);
                Ok(schedule_clear(cx, text))
            },
        );

        r.rpc(
            Method::new(
                "privacy.secrets.status",
                "privacy.method.secrets_status.description",
            )
            .effect(MethodEffect::Read)
            .idempotent(true)
            .param(ParamSchema::optional("idle_min", ParamKind::Integer))
            .result(ResultSchema::new("privacy.method.secrets_status.result")),
            |cx, params| {
                let idle_min = params
                    .get("idle_min")
                    .and_then(Value::as_i64)
                    .unwrap_or(0)
                    .max(0) as u64;
                Ok(secrets_json(cx, idle_min))
            },
        );

        r.rpc(
            Method::new(
                "privacy.browser.policy",
                "privacy.method.browser_policy.description",
            )
            .effect(MethodEffect::Read)
            .idempotent(true)
            .result(ResultSchema::new("privacy.method.browser_policy.result")),
            |cx, _params| Ok(browser_json(cx)),
        );
    }
}

fn register_settings(r: &mut Registrar<'_>) {
    r.setting(SettingSpec::new(
        "privacy.history.enabled",
        SettingKind::Bool,
        SettingValue::Bool(true),
        "setting-privacy-history-enabled-label",
        "setting-privacy-history-enabled-help",
    ));
    r.setting(SettingSpec::new(
        "privacy.history.retention_days",
        SettingKind::Number {
            min: Some(0.0),
            max: Some(3650.0),
        },
        SettingValue::Number(180.0),
        "setting-privacy-history-retention_days-label",
        "setting-privacy-history-retention_days-help",
    ));
    r.setting(SettingSpec::new(
        "privacy.clipboard.clear_after_s",
        SettingKind::Number {
            min: Some(0.0),
            max: Some(300.0),
        },
        SettingValue::Number(0.0),
        "setting-privacy-clipboard-clear_after_s-label",
        "setting-privacy-clipboard-clear_after_s-help",
    ));
    r.setting(SettingSpec::new(
        "privacy.logs.redact_secrets",
        SettingKind::Bool,
        SettingValue::Bool(true),
        "setting-privacy-logs-redact_secrets-label",
        "setting-privacy-logs-redact_secrets-help",
    ));
    r.setting(SettingSpec::new(
        "privacy.secrets.backend",
        SettingKind::Choice {
            options: vec!["credential_manager".to_owned(), "file".to_owned()],
        },
        SettingValue::Text("credential_manager".to_owned()),
        "setting-privacy-secrets-backend-label",
        "setting-privacy-secrets-backend-help",
    ));
    r.setting(SettingSpec::new(
        "privacy.secrets.lock_on_idle_min",
        SettingKind::Number {
            min: Some(0.0),
            max: Some(120.0),
        },
        SettingValue::Number(15.0),
        "setting-privacy-secrets-lock_on_idle_min-label",
        "setting-privacy-secrets-lock_on_idle_min-help",
    ));
    r.setting(SettingSpec::new(
        "privacy.usage.local_only",
        SettingKind::Bool,
        SettingValue::Bool(true),
        "setting-privacy-usage-local_only-label",
        "setting-privacy-usage-local_only-help",
    ));
    r.setting(SettingSpec::new(
        "privacy.assistant.share_workspace",
        SettingKind::Choice {
            options: vec!["ask".to_owned(), "always".to_owned(), "never".to_owned()],
        },
        SettingValue::Text("ask".to_owned()),
        "setting-privacy-assistant-share_workspace-label",
        "setting-privacy-assistant-share_workspace-help",
    ));
    r.setting(SettingSpec::new(
        "privacy.browser.clear_on_exit",
        SettingKind::Bool,
        SettingValue::Bool(false),
        "setting-privacy-browser-clear_on_exit-label",
        "setting-privacy-browser-clear_on_exit-help",
    ));
    r.setting(SettingSpec::new(
        "privacy.browser.cookies_days",
        SettingKind::Number {
            min: Some(0.0),
            max: Some(365.0),
        },
        SettingValue::Number(30.0),
        "setting-privacy-browser-cookies_days-label",
        "setting-privacy-browser-cookies_days-help",
    ));
}

fn bool_setting(cx: &Ctx<'_>, key: &str, default: bool) -> bool {
    cx.setting(key)
        .and_then(SettingValue::as_bool)
        .unwrap_or(default)
}

fn number_setting(cx: &Ctx<'_>, key: &str, default: f64, max: f64) -> u64 {
    cx.setting(key)
        .and_then(SettingValue::as_f64)
        .unwrap_or(default)
        .round()
        .clamp(0.0, max) as u64
}

fn policy_json(cx: &Ctx<'_>) -> Value {
    json!({
        "history_enabled": bool_setting(cx, "privacy.history.enabled", true),
        "history_retention_days": number_setting(cx, "privacy.history.retention_days", 180.0, 3650.0),
        "clipboard_clear_after_s": number_setting(cx, "privacy.clipboard.clear_after_s", 0.0, 300.0),
        "logs_redact_secrets": bool_setting(cx, "privacy.logs.redact_secrets", true),
        "secrets_backend": text_value(cx, "privacy.secrets.backend", "credential_manager"),
        "secrets_lock_on_idle_min": number_setting(cx, "privacy.secrets.lock_on_idle_min", 15.0, 120.0),
        "usage_local_only": bool_setting(cx, "privacy.usage.local_only", true),
        "assistant_share_workspace": text_value(cx, "privacy.assistant.share_workspace", "ask"),
        "browser_clear_on_exit": bool_setting(cx, "privacy.browser.clear_on_exit", false),
        "browser_cookies_days": number_setting(cx, "privacy.browser.cookies_days", 30.0, 365.0),
    })
}

fn text_value(cx: &Ctx<'_>, key: &str, default: &str) -> String {
    cx.setting(key)
        .and_then(SettingValue::as_str)
        .unwrap_or(default)
        .to_owned()
}

fn redact_json(cx: &Ctx<'_>, text: &str) -> Value {
    if !bool_setting(cx, "privacy.logs.redact_secrets", true) {
        return json!({ "text": text, "redacted": false });
    }
    let (text, redacted) = redact_secrets(text);
    json!({ "text": text, "redacted": redacted })
}

/// Remove tokens, senhas e chaves conhecidas de um texto de log.
pub fn redact_secrets(text: &str) -> (String, bool) {
    let mut out = String::with_capacity(text.len());
    let mut redacted = false;
    let mut redact_next = false;
    for token in text.split_inclusive(|c: char| c.is_whitespace()) {
        let body = token.trim_end();
        let trailing = &token[body.len()..];
        if redact_next {
            out.push_str("***");
            out.push_str(trailing);
            redact_next = false;
            redacted = true;
            continue;
        }
        if body.eq_ignore_ascii_case("Bearer") {
            out.push_str(body);
            out.push_str(trailing);
            redact_next = true;
            continue;
        }
        match redact_token(body) {
            Some(replacement) => {
                out.push_str(&replacement);
                out.push_str(trailing);
                redacted = true;
            }
            None => {
                out.push_str(body);
                out.push_str(trailing);
            }
        }
    }
    (out, redacted)
}

fn redact_token(token: &str) -> Option<String> {
    for prefix in [
        "ghp_",
        "gho_",
        "ghu_",
        "ghs_",
        "ghr_",
        "github_pat_",
        "sk-",
        "xoxb-",
        "xoxp-",
        "AKIA",
    ] {
        if token.starts_with(prefix) && token.len() > prefix.len() {
            return Some(format!("{prefix}***"));
        }
    }
    let (key, value) = token.split_once('=').or_else(|| token.split_once(':'))?;
    if value.is_empty() {
        return None;
    }
    let key = key.to_ascii_lowercase();
    let sensitive = [
        "password",
        "passwd",
        "token",
        "secret",
        "api_key",
        "apikey",
        "authorization",
    ];
    sensitive
        .iter()
        .any(|needle| key.contains(needle))
        .then(|| format!("{key}=***"))
}

fn schedule_clear(cx: &Ctx<'_>, text: Option<String>) -> Value {
    let seconds = number_setting(cx, "privacy.clipboard.clear_after_s", 0.0, 300.0);
    let Some(seconds) = schedule_after(seconds) else {
        return json!({ "scheduled": false, "clear_after_s": 0 });
    };
    if let Some(text) = text {
        copy_text(&text);
    }
    cx.spawn(
        move || {
            std::thread::sleep(Duration::from_secs(seconds));
            clear_clipboard();
        },
        |_cx, ()| {},
    );
    json!({ "scheduled": true, "clear_after_s": seconds })
}

/// `0` desliga a limpeza automática; qualquer outro valor agenda.
pub fn schedule_after(seconds: u64) -> Option<u64> {
    (seconds > 0).then_some(seconds)
}

fn secrets_json(cx: &Ctx<'_>, idle_min: u64) -> Value {
    let backend = text_value(cx, "privacy.secrets.backend", "credential_manager");
    let lock_after = number_setting(cx, "privacy.secrets.lock_on_idle_min", 15.0, 120.0);
    json!({
        "backend": backend,
        "lock_on_idle_min": lock_after,
        "locked": secrets_locked(idle_min, lock_after),
    })
}

/// O cofre só bloqueia quando há um limite e a inatividade o alcançou.
pub fn secrets_locked(idle_min: u64, lock_after_min: u64) -> bool {
    lock_after_min > 0 && idle_min >= lock_after_min
}

fn browser_json(cx: &Ctx<'_>) -> Value {
    json!({
        "clear_on_exit": bool_setting(cx, "privacy.browser.clear_on_exit", false),
        "cookies_days": number_setting(cx, "privacy.browser.cookies_days", 30.0, 365.0),
    })
}

#[cfg(windows)]
fn copy_text(text: &str) {
    use windows_sys::Win32::Foundation::GlobalFree;
    use windows_sys::Win32::System::DataExchange::{
        CloseClipboard, EmptyClipboard, OpenClipboard, SetClipboardData,
    };
    use windows_sys::Win32::System::Memory::{
        GlobalAlloc, GlobalLock, GlobalUnlock, GMEM_MOVEABLE,
    };

    const CF_UNICODETEXT: u32 = 13;
    unsafe {
        if OpenClipboard(std::ptr::null_mut()) == 0 {
            return;
        }
        let _ = EmptyClipboard();
        let wide: Vec<u16> = text.encode_utf16().chain(std::iter::once(0)).collect();
        let bytes = wide.len() * std::mem::size_of::<u16>();
        let handle = GlobalAlloc(GMEM_MOVEABLE, bytes);
        if !handle.is_null() {
            let pointer = GlobalLock(handle) as *mut u16;
            if pointer.is_null() {
                let _ = GlobalFree(handle);
            } else {
                std::ptr::copy_nonoverlapping(wide.as_ptr(), pointer, wide.len());
                let _ = GlobalUnlock(handle);
                if SetClipboardData(CF_UNICODETEXT, handle).is_null() {
                    let _ = GlobalFree(handle);
                }
            }
        }
        let _ = CloseClipboard();
    }
}

#[cfg(not(windows))]
fn copy_text(_text: &str) {}

#[cfg(windows)]
fn clear_clipboard() {
    use windows_sys::Win32::System::DataExchange::{CloseClipboard, EmptyClipboard, OpenClipboard};
    unsafe {
        if OpenClipboard(std::ptr::null_mut()) != 0 {
            let _ = EmptyClipboard();
            let _ = CloseClipboard();
        }
    }
}

#[cfg(not(windows))]
fn clear_clipboard() {}

#[cfg(test)]
mod tests {
    use super::*;
    use forja_kernel::{contract, Args, Runtime, RuntimeOptions};
    use serde_json::Map;
    use std::path::PathBuf;

    fn temp_dir(label: &str) -> PathBuf {
        std::env::temp_dir().join(format!(
            "forja-privacy-{label}-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ))
    }

    fn runtime(dir: &std::path::Path) -> Runtime {
        Runtime::new(
            vec![Box::new(PrivacyModule)],
            RuntimeOptions {
                data_dir: Some(dir.to_path_buf()),
                user_dir: Some(dir.join("user")),
                locale: Some("pt-BR".to_owned()),
                wake: None,
            },
        )
        .expect("privacy module registers cleanly")
    }

    #[test]
    fn contract_and_policy_defaults() {
        let dir = temp_dir("contract");
        let mut rt = runtime(&dir);
        contract::assert_ok(rt.registry(), rt.i18n());
        let value = rt.run_command("privacy.policy", Args::new()).unwrap();
        assert_eq!(value["history_enabled"], true);
        assert_eq!(value["history_retention_days"], 180);
        assert_eq!(value["clipboard_clear_after_s"], 0);
        assert_eq!(value["logs_redact_secrets"], true);
        assert_eq!(value["secrets_backend"], "credential_manager");
        assert_eq!(value["secrets_lock_on_idle_min"], 15);
        assert_eq!(value["usage_local_only"], true);
        assert_eq!(value["assistant_share_workspace"], "ask");
        assert_eq!(value["browser_clear_on_exit"], false);
        assert_eq!(value["browser_cookies_days"], 30);
        let _ = std::fs::remove_dir_all(dir);
    }

    #[test]
    fn redaction_follows_the_setting() {
        let dir = temp_dir("redact");
        let mut rt = runtime(&dir);
        let mut args = Args::new();
        args.insert(
            "text",
            json!("token=abc123 Authorization: Bearer abc ghp_abcdefghijklmnop"),
        );
        let value = rt.run_command("privacy.redact", args).unwrap();
        assert_eq!(value["redacted"], true);
        let text = value["text"].as_str().unwrap();
        assert!(!text.contains("abc123"), "text was {text}");
        assert!(!text.contains("ghp_abcdefghijklmnop"), "text was {text}");
        assert!(text.contains("Bearer ***"), "text was {text}");

        rt.set_setting("privacy.logs.redact_secrets", SettingValue::Bool(false))
            .unwrap();
        let mut args = Args::new();
        args.insert("text", json!("token=abc123"));
        let value = rt.run_command("privacy.redact", args).unwrap();
        assert_eq!(value["redacted"], false);
        assert_eq!(value["text"], "token=abc123");
        let _ = std::fs::remove_dir_all(dir);
    }

    #[test]
    fn clipboard_schedule_is_off_by_default_and_never_touches_the_clipboard_in_tests() {
        let dir = temp_dir("clipboard");
        let mut rt = runtime(&dir);
        let value = rt
            .run_command("privacy.clipboard.schedule_clear", Args::new())
            .unwrap();
        assert_eq!(value["scheduled"], false);
        assert_eq!(value["clear_after_s"], 0);
        assert_eq!(schedule_after(0), None);
        assert_eq!(schedule_after(30), Some(30));
        let _ = std::fs::remove_dir_all(dir);
    }

    #[test]
    fn secrets_and_browser_policy_read_their_settings() {
        let dir = temp_dir("policy");
        let mut rt = runtime(&dir);
        let mut args = Args::new();
        args.insert("idle_min", json!(20));
        let value = rt.run_command("privacy.secrets.status", args).unwrap();
        assert_eq!(value["backend"], "credential_manager");
        assert_eq!(value["lock_on_idle_min"], 15);
        assert_eq!(value["locked"], true);
        assert!(secrets_locked(15, 15));
        assert!(!secrets_locked(14, 15));
        assert!(!secrets_locked(99, 0));

        rt.set_setting("privacy.secrets.backend", SettingValue::Text("file".into()))
            .unwrap();
        rt.set_setting("privacy.browser.clear_on_exit", SettingValue::Bool(true))
            .unwrap();
        rt.set_setting("privacy.browser.cookies_days", SettingValue::Number(0.0))
            .unwrap();
        let value = rt
            .run_command("privacy.browser.policy", Args::new())
            .unwrap();
        assert_eq!(value["clear_on_exit"], true);
        assert_eq!(value["cookies_days"], 0);
        let mut params = Map::new();
        params.insert("idle_min".into(), json!(1));
        let value = rt.call_method("privacy.secrets.status", params).unwrap();
        assert_eq!(value["backend"], "file");
        let _ = std::fs::remove_dir_all(dir);
    }
}
