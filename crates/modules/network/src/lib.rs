//! Módulo `network` da Forja: proxy HTTP, exceções, verificação TLS, tempo limite e modo offline.

use forja_contracts::NetworkPolicy;
use forja_kernel::forja_protocol::{
    Method, MethodEffect, ParamKind, ParamSchema, ResultSchema, RpcError,
};
use forja_kernel::{
    Ctx, Module, ModuleManifest, Registrar, SettingKind, SettingSpec, SettingValue,
};
use serde_json::{json, Value};

#[forja_kernel::module]
static NETWORK: fn() -> Box<dyn Module> = || Box::new(NetworkModule);

/// Módulo `network`: publica as configurações de rede e o contrato `network.proxy`.
pub struct NetworkModule;

impl Module for NetworkModule {
    fn manifest(&self) -> ModuleManifest {
        ModuleManifest::new("network", "network.name", env!("CARGO_PKG_VERSION"))
    }

    fn register(&mut self, r: &mut Registrar<'_>) {
        r.i18n(
            include_str!("../i18n/en.ftl"),
            include_str!("../i18n/pt-BR.ftl"),
        );
        r.keymap(include_str!("../keymap.toml"));
        register_settings(r);

        r.command("network.proxy", "network.command.proxy", |cx, _args| {
            let policy = read_policy(cx);
            Ok(serde_json::to_value(&policy).unwrap_or_else(|_| json!({})))
        });

        r.command("network.check", "network.command.check", |cx, args| {
            let url = args.str("url").unwrap_or("");
            let policy = read_policy(cx);
            Ok(check_json(&policy, url))
        });

        r.rpc(
            Method::new("network.proxy", "network.method.proxy.description")
                .effect(MethodEffect::Read)
                .idempotent(true)
                .result(ResultSchema::new("network.method.proxy.result")),
            |cx, _params| {
                let policy = read_policy(cx);
                serde_json::to_value(&policy).map_err(|error| RpcError::app(error.to_string()))
            },
        );

        r.rpc(
            Method::new("network.check", "network.method.check.description")
                .effect(MethodEffect::Read)
                .idempotent(true)
                .param(ParamSchema::optional("url", ParamKind::String))
                .result(ResultSchema::new("network.method.check.result")),
            |cx, params| {
                let url = params.get("url").and_then(Value::as_str).unwrap_or("");
                let policy = read_policy(cx);
                Ok(check_json(&policy, url))
            },
        );
    }
}

fn register_settings(r: &mut Registrar<'_>) {
    r.setting(SettingSpec::new(
        "network.proxy.mode",
        SettingKind::Choice {
            options: vec!["system".to_owned(), "none".to_owned(), "manual".to_owned()],
        },
        SettingValue::Text("system".to_owned()),
        "setting-network-proxy-mode-label",
        "setting-network-proxy-mode-help",
    ));
    r.setting(SettingSpec::new(
        "network.proxy.url",
        SettingKind::Text { max_len: None },
        SettingValue::Text(String::new()),
        "setting-network-proxy-url-label",
        "setting-network-proxy-url-help",
    ));
    r.setting(SettingSpec::new(
        "network.proxy.bypass",
        SettingKind::List {
            item: "host".to_owned(),
        },
        SettingValue::List(vec!["localhost".to_owned(), "127.0.0.1".to_owned()]),
        "setting-network-proxy-bypass-label",
        "setting-network-proxy-bypass-help",
    ));
    r.setting(SettingSpec::new(
        "network.proxy.auth",
        SettingKind::Choice {
            options: vec!["none".to_owned(), "basic".to_owned(), "ntlm".to_owned()],
        },
        SettingValue::Text("none".to_owned()),
        "setting-network-proxy-auth-label",
        "setting-network-proxy-auth-help",
    ));
    r.setting(SettingSpec::new(
        "network.tls.verify",
        SettingKind::Bool,
        SettingValue::Bool(true),
        "setting-network-tls-verify-label",
        "setting-network-tls-verify-help",
    ));
    r.setting(SettingSpec::new(
        "network.http.timeout_s",
        SettingKind::Number {
            min: Some(5.0),
            max: Some(120.0),
        },
        SettingValue::Number(30.0),
        "setting-network-http-timeout_s-label",
        "setting-network-http-timeout_s-help",
    ));
    r.setting(SettingSpec::new(
        "network.mode.offline",
        SettingKind::Bool,
        SettingValue::Bool(false),
        "setting-network-mode-offline-label",
        "setting-network-mode-offline-help",
    ));
}

/// Reads the seven `network.*` catalog settings and resolves the active policy.
pub fn read_policy(cx: &Ctx<'_>) -> NetworkPolicy {
    let mode = cx
        .setting("network.proxy.mode")
        .and_then(SettingValue::as_str)
        .unwrap_or("system")
        .to_owned();
    let configured_url = cx
        .setting("network.proxy.url")
        .and_then(SettingValue::as_str)
        .unwrap_or("")
        .to_owned();
    let bypass = cx
        .setting("network.proxy.bypass")
        .and_then(SettingValue::as_list)
        .map(|items| items.to_vec())
        .unwrap_or_else(|| vec!["localhost".to_owned(), "127.0.0.1".to_owned()]);
    let auth = cx
        .setting("network.proxy.auth")
        .and_then(SettingValue::as_str)
        .unwrap_or("none")
        .to_owned();
    let tls_verify = cx
        .setting("network.tls.verify")
        .and_then(SettingValue::as_bool)
        .unwrap_or(true);
    let timeout_s = cx
        .setting("network.http.timeout_s")
        .and_then(SettingValue::as_f64)
        .unwrap_or(30.0)
        .round()
        .clamp(5.0, 120.0) as u64;
    let offline = cx
        .setting("network.mode.offline")
        .and_then(SettingValue::as_bool)
        .unwrap_or(false);

    let url = match mode.as_str() {
        "manual" => configured_url.trim().to_owned(),
        "none" => String::new(),
        _ => env_proxy_url().unwrap_or_default(),
    };

    NetworkPolicy {
        mode,
        url,
        bypass,
        auth,
        tls_verify,
        timeout_s,
        offline,
    }
}

fn env_proxy_url() -> Option<String> {
    for key in ["HTTPS_PROXY", "https_proxy", "HTTP_PROXY", "http_proxy"] {
        if let Ok(value) = std::env::var(key) {
            let trimmed = value.trim();
            if !trimmed.is_empty() {
                return Some(trimmed.to_owned());
            }
        }
    }
    None
}

fn check_json(policy: &NetworkPolicy, url: &str) -> Value {
    let local_scheme = url.trim().starts_with("forja://");
    let allowed = !policy.offline || local_scheme;
    let host = extract_host(url);
    let bypassed = !host.is_empty() && matches_bypass(&host, &policy.bypass);
    let use_proxy = allowed && policy.mode != "none" && !policy.url.is_empty() && !bypassed;
    json!({
        "allowed": allowed,
        "reason": if allowed { "ok" } else { "offline" },
        "use_proxy": use_proxy,
        "proxy_url": if use_proxy { policy.url.as_str() } else { "" },
        "auth": policy.auth,
        "tls_verify": policy.tls_verify,
        "timeout_s": policy.timeout_s,
        "offline": policy.offline,
    })
}

fn extract_host(url: &str) -> String {
    let trimmed = url.trim();
    let without_scheme = trimmed
        .split_once("://")
        .map(|(_, rest)| rest)
        .unwrap_or(trimmed);
    let authority = without_scheme.split('/').next().unwrap_or("");
    let host_port = authority.rsplit('@').next().unwrap_or(authority);
    if host_port.starts_with('[') {
        host_port
            .trim_start_matches('[')
            .split(']')
            .next()
            .unwrap_or("")
            .to_ascii_lowercase()
    } else {
        host_port
            .split(':')
            .next()
            .unwrap_or("")
            .to_ascii_lowercase()
    }
}

fn matches_bypass(host: &str, patterns: &[String]) -> bool {
    patterns.iter().any(|pattern| {
        let p = pattern.trim().to_ascii_lowercase();
        if p.is_empty() {
            return false;
        }
        if let Some(suffix) = p.strip_prefix("*.") {
            host == suffix || host.ends_with(&format!(".{suffix}"))
        } else if p.starts_with('.') {
            host == p.trim_start_matches('.') || host.ends_with(&p)
        } else {
            host == p
        }
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use forja_kernel::{contract, Args, Runtime, RuntimeOptions};
    use std::path::PathBuf;

    struct TempGuard(PathBuf);

    impl std::ops::Deref for TempGuard {
        type Target = std::path::Path;

        fn deref(&self) -> &Self::Target {
            &self.0
        }
    }

    impl Drop for TempGuard {
        fn drop(&mut self) {
            for attempt in 0..20 {
                if std::fs::remove_dir_all(&self.0).is_ok() || !self.0.exists() {
                    return;
                }
                if attempt < 19 {
                    std::thread::sleep(std::time::Duration::from_millis(15));
                }
            }
        }
    }

    fn temp_dir(label: &str) -> TempGuard {
        TempGuard(std::env::temp_dir().join(format!(
            "forja-network-{label}-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        )))
    }

    fn runtime(dir: &std::path::Path) -> Runtime {
        Runtime::new(
            vec![Box::new(NetworkModule)],
            RuntimeOptions {
                data_dir: Some(dir.to_path_buf()),
                user_dir: Some(dir.join("user")),
                locale: Some("pt-BR".to_owned()),
                wake: None,
            },
        )
        .expect("network module registers cleanly")
    }

    #[test]
    fn contract_passes_and_defaults_are_exposed() {
        let dir = temp_dir("contract");
        let mut rt = runtime(&dir);
        contract::assert_ok(rt.registry(), rt.i18n());
        let value = rt.run_command("network.proxy", Args::new()).unwrap();
        let policy: NetworkPolicy = serde_json::from_value(value).unwrap();
        assert_eq!(policy.mode, "system");
        assert!(policy.tls_verify);
        assert_eq!(policy.timeout_s, 30);
        assert!(!policy.offline);
        assert_eq!(policy.auth, "none");
    }

    #[test]
    fn manual_proxy_bypass_offline_tls_and_timeout_take_effect() {
        let dir = temp_dir("policy");
        let mut rt = runtime(&dir);
        rt.set_setting("network.proxy.mode", SettingValue::Text("manual".into()))
            .unwrap();
        rt.set_setting(
            "network.proxy.url",
            SettingValue::Text("http://proxy.internal:8080".into()),
        )
        .unwrap();
        rt.set_setting(
            "network.proxy.bypass",
            SettingValue::List(vec!["localhost".into(), "*.corp.local".into()]),
        )
        .unwrap();
        rt.set_setting("network.proxy.auth", SettingValue::Text("basic".into()))
            .unwrap();
        rt.set_setting("network.tls.verify", SettingValue::Bool(false))
            .unwrap();
        rt.set_setting("network.http.timeout_s", SettingValue::Number(75.0))
            .unwrap();

        let mut ext_args = Args::new();
        ext_args.insert("url", json!("https://api.github.com/user"));
        let ext = rt.run_command("network.check", ext_args).unwrap();
        assert_eq!(ext["allowed"], true);
        assert_eq!(ext["use_proxy"], true);
        assert_eq!(ext["proxy_url"], "http://proxy.internal:8080");
        assert_eq!(ext["auth"], "basic");
        assert_eq!(ext["tls_verify"], false);
        assert_eq!(ext["timeout_s"], 75);

        let mut local_args = Args::new();
        local_args.insert("url", json!("https://git.corp.local/repo"));
        let local = rt.run_command("network.check", local_args).unwrap();
        assert_eq!(local["use_proxy"], false);

        rt.set_setting("network.mode.offline", SettingValue::Bool(true))
            .unwrap();
        let mut blocked_args = Args::new();
        blocked_args.insert("url", json!("https://api.github.com/user"));
        let blocked = rt.run_command("network.check", blocked_args).unwrap();
        assert_eq!(blocked["allowed"], false);
        assert_eq!(blocked["reason"], "offline");

    }
}
