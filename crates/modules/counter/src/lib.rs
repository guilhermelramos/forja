//! Módulo `counter` da Forja: dono das configurações de balcão (`counter.*` no
//! catálogo `design/settings.toml`) e ponto único para listar os balcões instalados.
//!
//! Um balcão é um módulo `counter_<nome>` que registra `counter_<nome>.describe`.
//! `counter.sources` chama todos eles e devolve a vitrine. Não há rede aqui: o `describe`
//! de cada balcão só lê o próprio nome. Os contratos estão em `design/contracts.md`.

mod notify;
mod poll;

use std::sync::{Arc, Mutex};

use forja_contracts::{names, CounterResult};
use forja_kernel::Slot;
use forja_kernel::{
    Args, Ctx, Module, ModuleManifest, Registrar, Result, SettingKind, SettingSpec, SettingValue,
};
use serde_json::{json, Value};

#[forja_kernel::module]
static COUNTER: fn() -> Box<dyn Module> = || Box::new(CounterModule);

/// O módulo não tem estado: as configurações ficam no store do kernel.
#[derive(Default)]
pub struct CounterModule;

impl Module for CounterModule {
    fn manifest(&self) -> ModuleManifest {
        ModuleManifest::new("counter", "counter.name", env!("CARGO_PKG_VERSION"))
    }

    fn register(&mut self, r: &mut Registrar<'_>) {
        r.i18n(
            include_str!("../i18n/en.ftl"),
            include_str!("../i18n/pt-BR.ftl"),
        );
        r.keymap(include_str!("../keymap.toml"));

        r.setting(text_setting(
            "counter.github.host",
            "github.com",
            "setting-counter-github-host-label",
            "setting-counter-github-host-help",
        ));
        r.setting(SettingSpec::new(
            "counter.github.include_drafts",
            SettingKind::Bool,
            SettingValue::Bool(false),
            "setting-counter-github-include_drafts-label",
            "setting-counter-github-include_drafts-help",
        ));
        r.setting(SettingSpec::new(
            "counter.github.label_filter",
            SettingKind::List {
                item: "rótulo".to_owned(),
            },
            SettingValue::List(Vec::new()),
            "setting-counter-github-label_filter-label",
            "setting-counter-github-label_filter-help",
        ));
        r.setting(text_setting(
            "counter.gitlab.host",
            "gitlab.com",
            "setting-counter-gitlab-host-label",
            "setting-counter-gitlab-host-help",
        ));
        r.setting(text_setting(
            "counter.linear.team",
            "",
            "setting-counter-linear-team-label",
            "setting-counter-linear-team-help",
        ));
        r.setting(text_setting(
            "counter.jira.project",
            "",
            "setting-counter-jira-project-label",
            "setting-counter-jira-project-help",
        ));
        r.setting(SettingSpec::new(
            "counter.poll.interval_s",
            SettingKind::Integer {
                min: Some(30),
                max: Some(3600),
            },
            SettingValue::Integer(300),
            "setting-counter-poll-interval_s-label",
            "setting-counter-poll-interval_s-help",
        ));
        r.setting(SettingSpec::new(
            "counter.notifications.new_request",
            SettingKind::Bool,
            SettingValue::Bool(true),
            "setting-counter-notifications-new_request-label",
            "setting-counter-notifications-new_request-help",
        ));
        r.setting(SettingSpec::new(
            "counter.notifications.checks_failed",
            SettingKind::Bool,
            SettingValue::Bool(true),
            "setting-counter-notifications-checks_failed-label",
            "setting-counter-notifications-checks_failed-help",
        ));

        r.command("counter.sources", "counter.command.sources", |cx, _args| {
            sources(cx)
        });

        r.contract_event::<CounterResult>(names::COUNTER_RESULT);
        let seen = Arc::new(Mutex::new(notify::Seen::default()));
        r.subscribe(move |cx, result: &CounterResult| {
            notify::on_result(cx, &seen, result);
        });
        r.view(Slot::Modal, poll::POLLER_VIEW, "counter.name", 0, || {
            Box::new(poll::Poller::default())
        });

        for key in ["counter.name", "counter.command.sources", "counter.notice.new-request", "counter.notice.checks-failed"] {
            r.uses_i18n(key);
        }
    }
}

fn text_setting(key: &str, default: &str, label_key: &str, help_key: &str) -> SettingSpec {
    SettingSpec::new(
        key,
        SettingKind::Text {
            max_len: Some(256),
        },
        SettingValue::Text(default.to_owned()),
        label_key,
        help_key,
    )
}

/// Chama `describe` de cada `counter_<nome>` instalado. Um balcão que falha some da lista.
pub(crate) fn sources(cx: &mut Ctx<'_>) -> Result<Value> {
    let describers: Vec<String> = cx
        .registry()
        .commands()
        .iter()
        .filter(|command| {
            command.module.starts_with("counter_") && command.id.as_str().ends_with(".describe")
        })
        .map(|command| command.id.as_str().to_owned())
        .collect();
    let mut counters: Vec<Value> = Vec::new();
    for id in describers {
        if let Ok(value) = cx.run_command(&id, Args::new()) {
            counters.push(value);
        }
    }
    counters.sort_by(|a, b| a["counter"].as_str().cmp(&b["counter"].as_str()));
    Ok(json!({ "counters": counters }))
}

#[cfg(test)]
mod tests {
    use super::*;
    use forja_kernel::{contract, Runtime, RuntimeOptions};

    fn runtime(dir: &std::path::Path) -> Runtime {
        Runtime::new(
            vec![Box::new(CounterModule)],
            RuntimeOptions {
                data_dir: Some(dir.to_path_buf()),
                user_dir: Some(dir.join("user")),
                locale: Some("pt-BR".to_owned()),
                wake: None,
            },
        )
        .expect("counter registers cleanly")
    }

    #[test]
    fn contract_holds_and_the_catalog_keys_are_declared() {
        let dir = std::env::temp_dir().join(format!("forja-counter-hub-{}", std::process::id()));
        let runtime = runtime(&dir);
        contract::assert_ok(runtime.registry(), runtime.i18n());
        let keys: Vec<&str> = runtime
            .registry()
            .settings()
            .iter()
            .map(|spec| spec.key.as_str())
            .collect();
        for key in [
            "counter.github.host",
            "counter.github.include_drafts",
            "counter.github.label_filter",
            "counter.gitlab.host",
            "counter.linear.team",
            "counter.jira.project",
            "counter.poll.interval_s",
            "counter.notifications.new_request",
            "counter.notifications.checks_failed",
        ] {
            assert!(keys.contains(&key), "missing setting {key}");
        }
        let _ = std::fs::remove_dir_all(dir);
    }

    #[test]
    fn sources_is_empty_when_no_counter_is_installed() {
        let dir = std::env::temp_dir().join(format!("forja-counter-empty-{}", std::process::id()));
        let mut runtime = runtime(&dir);
        let value = runtime.run_command("counter.sources", Args::new()).unwrap();
        assert_eq!(value["counters"], json!([]));
        let _ = std::fs::remove_dir_all(dir);
    }
}
