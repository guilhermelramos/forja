//! Módulo `integrations` da Forja: webhook de saída.
//!
//! Com `integrations.webhook.enabled` ligado, os eventos escolhidos em
//! `integrations.webhook.events` são enviados por POST para
//! `integrations.webhook.url`. Os eventos vêm dos contratos (`stage.changed`,
//! `bench.created`). Falha de envio aparece como aviso. Ver `MODULE.md`.

mod webhook;

use forja_contracts::{names, BenchCreated, StageChanged};
use forja_kernel::{
    Args, Ctx, Module, ModuleManifest, Registrar, SettingKind, SettingSpec, SettingValue,
};
use serde::Serialize;
use serde_json::json;
use std::time::{SystemTime, UNIX_EPOCH};

#[forja_kernel::module]
static INTEGRATIONS: fn() -> Box<dyn Module> = || Box::new(IntegrationsModule);

/// O módulo não tem estado: as configurações ficam no store do kernel.
#[derive(Default)]
pub struct IntegrationsModule;

impl Module for IntegrationsModule {
    fn manifest(&self) -> ModuleManifest {
        ModuleManifest::new("integrations", "integrations.name", env!("CARGO_PKG_VERSION"))
    }

    fn register(&mut self, r: &mut Registrar<'_>) {
        r.i18n(
            include_str!("../i18n/en.ftl"),
            include_str!("../i18n/pt-BR.ftl"),
        );
        r.keymap(include_str!("../keymap.toml"));
        register_settings(r);

        r.contract_event::<StageChanged>(names::STAGE_CHANGED);
        r.contract_event::<BenchCreated>(names::BENCH_CREATED);
        r.subscribe(|cx, event: &StageChanged| send_event(cx, "stage.changed", event));
        r.subscribe(|cx, event: &BenchCreated| send_event(cx, "bench.created", event));

        for key in [
            "integrations.notice.webhook-title",
            "integrations.notice.webhook-failed",
        ] {
            r.uses_i18n(key);
        }
    }
}

fn register_settings(r: &mut Registrar<'_>) {
    r.setting(SettingSpec::new(
        "integrations.webhook.enabled",
        SettingKind::Bool,
        SettingValue::Bool(false),
        "setting-integrations-webhook-enabled-label",
        "setting-integrations-webhook-enabled-help",
    ));
    r.setting(SettingSpec::new(
        "integrations.webhook.url",
        SettingKind::Text {
            max_len: Some(2048),
        },
        SettingValue::Text(String::new()),
        "setting-integrations-webhook-url-label",
        "setting-integrations-webhook-url-help",
    ));
    r.setting(SettingSpec::new(
        "integrations.webhook.events",
        SettingKind::List {
            item: "evento".to_owned(),
        },
        SettingValue::List(Vec::new()),
        "setting-integrations-webhook-events-label",
        "setting-integrations-webhook-events-help",
    ));
}

/// Envia um evento ao webhook, se ele estiver ligado, escolhido e com URL válida.
fn send_event<T: Serialize>(cx: &mut Ctx<'_>, name: &str, event: &T) {
    let enabled = cx
        .setting("integrations.webhook.enabled")
        .and_then(SettingValue::as_bool)
        .unwrap_or(false);
    if !enabled {
        return;
    }
    let events = cx
        .setting("integrations.webhook.events")
        .and_then(SettingValue::as_list)
        .map(|list| list.to_vec())
        .unwrap_or_default();
    if !webhook::wanted(&events, name) {
        return;
    }
    let url = cx
        .setting("integrations.webhook.url")
        .and_then(SettingValue::as_str)
        .unwrap_or_default()
        .trim()
        .to_owned();
    if !webhook::valid_url(&url) {
        return;
    }
    let Ok(data) = serde_json::to_value(event) else {
        return;
    };
    let body = webhook::envelope(name, data, now_ms());
    cx.spawn(
        move || webhook::send(&url, &body),
        move |cx, result| {
            if let Err(message) = result {
                notify_failure(cx, &message);
            }
        },
    );
}

/// Falha de envio vira aviso, para não sumir em silêncio.
fn notify_failure(cx: &mut Ctx<'_>, message: &str) {
    let mut args = Args::new();
    args.insert("title", json!(cx.t("integrations.notice.webhook-title")));
    args.insert(
        "body",
        json!(cx.t_args(
            "integrations.notice.webhook-failed",
            &[("message", message)]
        )),
    );
    args.insert("level", json!("error"));
    let _ = cx.run_command("notify.show", args);
}

fn now_ms() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|elapsed| elapsed.as_millis() as u64)
        .unwrap_or(0)
}

#[cfg(test)]
mod tests {
    use super::*;
    use forja_kernel::{contract, Runtime, RuntimeOptions};

    #[test]
    fn contract_holds_and_the_webhook_settings_are_declared() {
        let dir = std::env::temp_dir().join(format!("forja-integrations-{}", std::process::id()));
        let runtime = Runtime::new(
            vec![Box::new(IntegrationsModule)],
            RuntimeOptions {
                data_dir: Some(dir.clone()),
                user_dir: Some(dir.join("user")),
                locale: Some("en".to_owned()),
                wake: None,
            },
        )
        .expect("integrations registers cleanly");
        contract::assert_ok(runtime.registry(), runtime.i18n());
        let keys: Vec<&str> = runtime
            .registry()
            .settings()
            .iter()
            .map(|spec| spec.key.as_str())
            .collect();
        for key in [
            "integrations.webhook.enabled",
            "integrations.webhook.url",
            "integrations.webhook.events",
        ] {
            assert!(keys.contains(&key), "missing setting {key}");
        }
        let _ = std::fs::remove_dir_all(dir);
    }
}
