//! Módulo `usage` da Forja: consumo local de tokens dos ajudantes, lido das
//! transcrições do Claude, Codex e OpenCode.

mod model;
mod parse;
mod scan;
mod state;
mod view;

use forja_kernel::forja_protocol::{Method, MethodEffect, ResultSchema};
use forja_kernel::{
    Module, ModuleManifest, Registrar, SettingKind, SettingSpec, SettingValue, Slot,
};
use serde_json::json;
use state::Shared;

#[forja_kernel::module]
static USAGE: fn() -> Box<dyn Module> = || Box::new(UsageModule::new());

pub struct UsageModule {
    state: Shared,
}

impl UsageModule {
    pub fn new() -> Self {
        Self {
            state: Shared::new(),
        }
    }
}

impl Default for UsageModule {
    fn default() -> Self {
        Self::new()
    }
}

impl Module for UsageModule {
    fn manifest(&self) -> ModuleManifest {
        ModuleManifest::new("usage", "usage.name", env!("CARGO_PKG_VERSION"))
    }

    fn register(&mut self, r: &mut Registrar<'_>) {
        r.i18n(
            include_str!("../i18n/en.ftl"),
            include_str!("../i18n/pt-BR.ftl"),
        );
        r.keymap(include_str!("../keymap.toml"));
        register_settings(r);

        let refresh = self.state.clone();
        r.command(
            "usage.refresh",
            "usage.command.refresh",
            move |cx, _args| {
                refresh.expire();
                view::ensure_refresh(cx, &refresh);
                Ok(json!({ "pending": true }))
            },
        );

        let rpc_state = self.state.clone();
        r.rpc(
            Method::new("usage.summary", "usage.method.summary.description")
                .effect(MethodEffect::Read)
                .idempotent(true)
                .result(ResultSchema::new("usage.method.summary.result")),
            move |cx, _params| {
                view::ensure_refresh(cx, &rpc_state);
                serde_json::to_value(rpc_state.summary())
                    .map_err(|error| forja_kernel::forja_protocol::RpcError::app(error.to_string()))
            },
        );

        for key in used_keys() {
            r.uses_i18n(key);
        }

        let page = self.state.clone();
        r.view(
            Slot::Rail,
            "usage.page",
            "usage.page.title",
            70,
            move || Box::new(view::UsagePage::new(page.clone())),
        );
        r.view_glyph("usage.page", forja_ui_kit::BOARD);
        let status = self.state.clone();
        r.view(
            Slot::StatusBar,
            "usage.status",
            "usage.page.title",
            50,
            move || Box::new(view::UsageStatus::new(status.clone())),
        );
    }
}

fn register_settings(r: &mut Registrar<'_>) {
    r.setting(SettingSpec::new(
        "usage.cost.track",
        SettingKind::Bool,
        SettingValue::Bool(true),
        "setting-usage-cost-track-label",
        "setting-usage-cost-track-help",
    ));
    r.setting(SettingSpec::new(
        "usage.display.status_bar",
        SettingKind::Bool,
        SettingValue::Bool(true),
        "setting-usage-display-status_bar-label",
        "setting-usage-display-status_bar-help",
    ));
}

fn used_keys() -> Vec<&'static str> {
    vec![
        "usage.name",
        "usage.page.title",
        "usage.page.refresh",
        "usage.today",
        "usage.week",
        "usage.month",
        "usage.total",
        "usage.input",
        "usage.output",
        "usage.section.days",
        "usage.section.models",
        "usage.section.projects",
        "usage.section.sessions",
        "usage.status.suffix",
        "usage.command.refresh",
        "usage.method.summary.description",
        "usage.method.summary.result",
        "setting-usage-cost-track-label",
        "setting-usage-cost-track-help",
        "setting-usage-display-status_bar-label",
        "setting-usage-display-status_bar-help",
    ]
}

#[cfg(test)]
mod tests {
    use super::*;
    use forja_kernel::{contract, Runtime, RuntimeOptions};

    fn temp_dir() -> std::path::PathBuf {
        let dir = std::env::temp_dir().join(format!(
            "forja-usage-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        std::fs::create_dir_all(&dir).unwrap();
        dir
    }

    fn runtime(dir: &std::path::Path) -> Runtime {
        Runtime::new(
            vec![Box::new(UsageModule::new())],
            RuntimeOptions {
                data_dir: Some(dir.to_path_buf()),
                user_dir: Some(dir.join("user")),
                locale: Some("pt-BR".to_owned()),
                wake: None,
            },
        )
        .expect("usage registers cleanly")
    }

    #[test]
    fn contract_and_settings_match_the_catalog() {
        let dir = temp_dir();
        let runtime = runtime(&dir);
        contract::assert_ok(runtime.registry(), runtime.i18n());
        let keys: Vec<&str> = runtime
            .registry()
            .settings()
            .iter()
            .filter(|spec| spec.key.starts_with("usage."))
            .map(|spec| spec.key.as_str())
            .collect();
        assert_eq!(keys.len(), 2);
        assert!(keys.contains(&"usage.cost.track"));
        assert!(keys.contains(&"usage.display.status_bar"));
        let _ = std::fs::remove_dir_all(dir);
    }

    #[test]
    fn fixture_summary_has_days_models_and_sessions() {
        let summary = view::sample_summary();
        assert!(!summary.by_day.is_empty());
        assert!(summary.by_model.len() >= 3);
        assert!(!summary.sessions.is_empty());
        assert!(summary.total.total_tokens > 0);
    }
}
