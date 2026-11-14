//! Módulo `update` da Forja.
//!
//! A verificação de versão é local: o chamador informa a versão mais recente
//! que já conhece e o módulo apenas compara com a versão em execução. Nada é
//! enviado pela rede e nenhuma telemetria é coletada.

use forja_kernel::forja_protocol::{Method, MethodEffect, ParamKind, ParamSchema, ResultSchema};
use forja_kernel::{
    Ctx, Module, ModuleManifest, Registrar, SettingKind, SettingSpec, SettingValue,
};
use serde_json::{json, Value};
use std::sync::{Arc, Mutex};
use std::time::{SystemTime, UNIX_EPOCH};

#[forja_kernel::module]
static UPDATE: fn() -> Box<dyn Module> = || {
    Box::new(UpdateModule {
        state: Shared::new(),
    })
};

#[derive(Default)]
struct State {
    last_checked_at: Option<i64>,
}

#[derive(Clone)]
struct Shared(Arc<Mutex<State>>);

impl Shared {
    fn new() -> Self {
        Self(Arc::new(Mutex::new(State::default())))
    }

    fn last_checked_at(&self) -> Option<i64> {
        self.0.lock().ok().and_then(|state| state.last_checked_at)
    }

    fn record_check(&self, at: i64) {
        if let Ok(mut state) = self.0.lock() {
            state.last_checked_at = Some(at);
        }
    }
}

/// Módulo `update`: canal, frequência e política de download.
pub struct UpdateModule {
    state: Shared,
}

impl Module for UpdateModule {
    fn manifest(&self) -> ModuleManifest {
        ModuleManifest::new("update", "update.name", env!("CARGO_PKG_VERSION"))
    }

    fn register(&mut self, r: &mut Registrar<'_>) {
        r.i18n(
            include_str!("../i18n/en.ftl"),
            include_str!("../i18n/pt-BR.ftl"),
        );
        register_settings(r);

        let check_state = self.state.clone();
        r.command("update.check", "update.command.check", move |cx, args| {
            let latest = args.str("latest_version").map(str::to_owned);
            Ok(check_json(cx, &check_state, latest))
        });

        let status_state = self.state.clone();
        r.command(
            "update.status",
            "update.command.status",
            move |cx, _args| Ok(status_json(cx, &status_state)),
        );

        let rpc_state = self.state.clone();
        r.rpc(
            Method::new("update.check", "update.method.check.description")
                .effect(MethodEffect::Read)
                .idempotent(false)
                .param(ParamSchema::optional("latest_version", ParamKind::String))
                .result(ResultSchema::new("update.method.check.result")),
            move |cx, params| {
                let latest = params
                    .get("latest_version")
                    .and_then(Value::as_str)
                    .map(str::to_owned);
                Ok(check_json(cx, &rpc_state, latest))
            },
        );

        let rpc_state = self.state.clone();
        r.rpc(
            Method::new("update.status", "update.method.status.description")
                .effect(MethodEffect::Read)
                .idempotent(true)
                .result(ResultSchema::new("update.method.status.result")),
            move |cx, _params| Ok(status_json(cx, &rpc_state)),
        );
    }
}

fn register_settings(r: &mut Registrar<'_>) {
    r.setting(SettingSpec::new(
        "update.channel.name",
        SettingKind::Choice {
            options: vec!["stable".to_owned(), "beta".to_owned()],
        },
        SettingValue::Text("stable".to_owned()),
        "setting-update-channel-name-label",
        "setting-update-channel-name-help",
    ));
    r.setting(SettingSpec::new(
        "update.check.frequency",
        SettingKind::Choice {
            options: vec![
                "startup".to_owned(),
                "daily".to_owned(),
                "weekly".to_owned(),
                "never".to_owned(),
            ],
        },
        SettingValue::Text("daily".to_owned()),
        "setting-update-check-frequency-label",
        "setting-update-check-frequency-help",
    ));
    r.setting(SettingSpec::new(
        "update.download.auto",
        SettingKind::Bool,
        SettingValue::Bool(true),
        "setting-update-download-auto-label",
        "setting-update-download-auto-help",
    ));
}

fn bool_setting(cx: &Ctx<'_>, key: &str, default: bool) -> bool {
    cx.setting(key)
        .and_then(SettingValue::as_bool)
        .unwrap_or(default)
}

fn text_value(cx: &Ctx<'_>, key: &str, default: &str) -> String {
    cx.setting(key)
        .and_then(SettingValue::as_str)
        .unwrap_or(default)
        .to_owned()
}

fn check_json(cx: &Ctx<'_>, state: &Shared, latest: Option<String>) -> Value {
    let channel = text_value(cx, "update.channel.name", "stable");
    let frequency = text_value(cx, "update.check.frequency", "daily");
    let auto_download = bool_setting(cx, "update.download.auto", true);
    let current = env!("CARGO_PKG_VERSION");
    let now = now_seconds();
    let due = due(&frequency, state.last_checked_at(), now);
    let latest = latest.unwrap_or_else(|| current.to_owned());
    let checked = due && frequency != "never";
    let update_available = checked && version_is_newer(&latest, current);
    if checked {
        state.record_check(now);
    }
    json!({
        "channel": channel,
        "frequency": frequency,
        "auto_download": auto_download,
        "current_version": current,
        "latest_version": latest,
        "update_available": update_available,
        "checked": checked,
        "will_download": update_available && auto_download,
    })
}

fn status_json(cx: &Ctx<'_>, state: &Shared) -> Value {
    json!({
        "channel": text_value(cx, "update.channel.name", "stable"),
        "frequency": text_value(cx, "update.check.frequency", "daily"),
        "auto_download": bool_setting(cx, "update.download.auto", true),
        "current_version": env!("CARGO_PKG_VERSION"),
        "last_checked_at": state.last_checked_at(),
    })
}

fn now_seconds() -> i64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|elapsed| elapsed.as_secs() as i64)
        .unwrap_or(0)
}

/// `startup` verifica uma vez por sessão; `daily` e `weekly` respeitam o intervalo.
pub fn due(frequency: &str, last_checked_at: Option<i64>, now: i64) -> bool {
    match frequency {
        "never" => false,
        "startup" => last_checked_at.is_none(),
        "weekly" => last_checked_at.is_none_or(|last| now - last >= 7 * 86_400),
        "daily" => last_checked_at.is_none_or(|last| now - last >= 86_400),
        _ => true,
    }
}

/// Compara versões `major.minor.patch`; pré-lançamentos contam como a base.
pub fn version_is_newer(candidate: &str, current: &str) -> bool {
    match (parse_version(candidate), parse_version(current)) {
        (Some(candidate), Some(current)) => candidate > current,
        _ => false,
    }
}

fn parse_version(text: &str) -> Option<(u64, u64, u64)> {
    let core = text.trim().trim_start_matches('v');
    let core = core.split(['-', '+']).next().unwrap_or(core);
    let mut parts = core.split('.');
    let major = parts.next()?.parse().ok()?;
    let minor = parts.next().unwrap_or("0").parse().ok()?;
    let patch = parts.next().unwrap_or("0").parse().ok()?;
    if parts.next().is_some() {
        return None;
    }
    Some((major, minor, patch))
}

#[cfg(test)]
mod tests {
    use super::*;
    use forja_kernel::{contract, Args, Runtime, RuntimeOptions};
    use serde_json::Map;
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
            "forja-update-{label}-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        )))
    }

    fn runtime(dir: &std::path::Path) -> Runtime {
        Runtime::new(
            vec![Box::new(UpdateModule {
                state: Shared::new(),
            })],
            RuntimeOptions {
                data_dir: Some(dir.to_path_buf()),
                user_dir: Some(dir.join("user")),
                locale: Some("pt-BR".to_owned()),
                wake: None,
            },
        )
        .expect("update module registers cleanly")
    }

    #[test]
    fn contract_and_local_check_defaults() {
        let dir = temp_dir("contract");
        let mut rt = runtime(&dir);
        contract::assert_ok(rt.registry(), rt.i18n());
        let value = rt.run_command("update.status", Args::new()).unwrap();
        assert_eq!(value["channel"], "stable");
        assert_eq!(value["frequency"], "daily");
        assert_eq!(value["auto_download"], true);
        assert_eq!(value["current_version"], env!("CARGO_PKG_VERSION"));
        assert_eq!(value["last_checked_at"], Value::Null);
    }

    #[test]
    fn check_compares_versions_and_respects_the_frequency() {
        let dir = temp_dir("check");
        let mut rt = runtime(&dir);
        rt.set_setting(
            "update.check.frequency",
            SettingValue::Text("startup".into()),
        )
        .unwrap();
        let mut args = Args::new();
        args.insert("latest_version", json!("99.0.0"));
        let value = rt.run_command("update.check", args).unwrap();
        assert_eq!(value["checked"], true);
        assert_eq!(value["update_available"], true);
        assert_eq!(value["will_download"], true);

        let mut args = Args::new();
        args.insert("latest_version", json!("99.0.0"));
        let value = rt.run_command("update.check", args).unwrap();
        assert_eq!(value["checked"], false);
        assert_eq!(value["update_available"], false);

        rt.set_setting("update.check.frequency", SettingValue::Text("never".into()))
            .unwrap();
        let mut args = Args::new();
        args.insert("latest_version", json!("99.0.0"));
        let value = rt.run_command("update.check", args).unwrap();
        assert_eq!(value["checked"], false);
        assert_eq!(value["update_available"], false);

        rt.set_setting("update.download.auto", SettingValue::Bool(false))
            .unwrap();
        let status = rt.run_command("update.status", Args::new()).unwrap();
        assert_eq!(status["auto_download"], false);
        assert_eq!(status["frequency"], "never");
    }

    #[test]
    fn version_and_due_rules_are_explicit() {
        assert!(version_is_newer("0.2.0", "0.1.0"));
        assert!(version_is_newer("v1.0.1", "1.0.0"));
        assert!(!version_is_newer("1.0.0", "1.0.0"));
        assert!(!version_is_newer("not-a-version", "1.0.0"));
        assert!(due("daily", None, 100));
        assert!(!due("daily", Some(100), 100 + 3600));
        assert!(due("daily", Some(100), 100 + 86_400));
        assert!(!due("weekly", Some(100), 100 + 86_400));
        assert!(due("weekly", Some(100), 100 + 7 * 86_400));
        assert!(!due("never", None, 0));
        assert!(due("startup", None, 0));
        assert!(!due("startup", Some(1), 2));
    }

    #[test]
    fn rpc_check_returns_the_same_contract() {
        let dir = temp_dir("rpc");
        let mut rt = runtime(&dir);
        let mut params = Map::new();
        params.insert("latest_version".into(), json!("0.0.1"));
        let value = rt.call_method("update.check", params).unwrap();
        assert!(value.get("checked").is_some());
        assert!(value.get("will_download").is_some());
    }
}
