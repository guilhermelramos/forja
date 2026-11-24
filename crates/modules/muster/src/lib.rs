//! Módulo `muster` da Forja: Mutirão, tarefas, recados e entregas.
//!
//! O estado persistido fica em `muster.json`; a página é uma Rail. O módulo
//! fala com bancadas, ajudantes e terminais por comandos e eventos.

mod cleanup;
mod commands;
mod cost;
mod delivery;
mod dispatch;
pub mod demo;
mod model;
mod model_ops;
mod plan;
mod preamble;
mod state;
mod status;
mod view;
mod workspace;

use forja_kernel::{
    Module, ModuleManifest, Registrar, SettingKind, SettingSpec, SettingValue, Slot,
};
use state::Shared;

#[forja_kernel::module]
static MUSTER: fn() -> Box<dyn Module> = || Box::new(MusterModule::new());

/// Módulo Mutirão.
pub struct MusterModule {
    shared: Shared,
}

impl MusterModule {
    fn new() -> Self {
        Self {
            shared: state::new_shared(),
        }
    }
}

impl Default for MusterModule {
    fn default() -> Self {
        Self::new()
    }
}

impl Module for MusterModule {
    fn manifest(&self) -> ModuleManifest {
        ModuleManifest::new("muster", "muster.name", env!("CARGO_PKG_VERSION"))
    }

    fn register(&mut self, r: &mut Registrar<'_>) {
        r.i18n(
            include_str!("../i18n/en.ftl"),
            include_str!("../i18n/pt-BR.ftl"),
        );
        r.keymap(include_str!("../keymap.toml"));
        register_settings(r);
        commands::register(r, &self.shared);
        commands::subscribe_events(r, &self.shared);

        for key in view::USED_KEYS.iter().chain(commands::USED_KEYS).copied() {
            r.uses_i18n(key);
        }

        let shared = self.shared.clone();
        let order = if std::env::var_os("FORJA_MUSTER_PREVIEW").is_some() {
            0
        } else {
            50
        };
        r.view(
            Slot::Rail,
            "muster.page",
            "muster.page.title",
            order,
            move || Box::new(view::MusterView::new(shared.clone())),
        );
        r.view_glyph("muster.page", forja_ui_kit::Icon::Muster.id());

        let tick_shared = self.shared.clone();
        r.view(
            Slot::StatusBar,
            "muster.tick",
            "muster.page.title",
            70,
            move || Box::new(view::MusterTick::new(tick_shared.clone())),
        );
    }
}

fn register_settings(r: &mut Registrar<'_>) {
    r.setting(SettingSpec::new(
        "muster.mode.enabled",
        SettingKind::Bool,
        SettingValue::Bool(true),
        "setting-muster-mode-enabled-label",
        "setting-muster-mode-enabled-help",
    ));
    r.setting(SettingSpec::new(
        "muster.coordinator.profile",
        // Espelha `helper.default.profile`; o helper ainda não expõe uma API
        // pública com a lista de perfis descobertos.
        SettingKind::Choice {
            options: [
                "auto",
                "aider",
                "amp",
                "antigravity",
                "claude-code",
                "cline",
                "codex",
                "continue",
                "copilot",
                "crush",
                "cursor-agent",
                "gemini",
                "goose",
                "kimi-code",
                "kiro",
                "opencode",
                "qwen-code",
            ]
            .into_iter()
            .map(str::to_owned)
            .collect(),
        },
        SettingValue::Text("auto".to_owned()),
        "setting-muster-coordinator-profile-label",
        "setting-muster-coordinator-profile-help",
    ));
    r.setting(SettingSpec::new(
        "muster.plan.max_tasks",
        SettingKind::Integer {
            min: Some(1),
            max: Some(100),
        },
        SettingValue::Integer(20),
        "setting-muster-plan-max_tasks-label",
        "setting-muster-plan-max_tasks-help",
    ));
    r.setting(SettingSpec::new(
        "muster.plan.require_approval",
        SettingKind::Bool,
        SettingValue::Bool(true),
        "setting-muster-plan-require_approval-label",
        "setting-muster-plan-require_approval-help",
    ));
    r.setting(SettingSpec::new(
        "muster.dispatch.max_parallel",
        SettingKind::Integer {
            min: Some(1),
            max: Some(16),
        },
        SettingValue::Integer(4),
        "setting-muster-dispatch-max_parallel-label",
        "setting-muster-dispatch-max_parallel-help",
    ));
    r.setting(SettingSpec::new(
        "muster.dispatch.target",
        SettingKind::Choice {
            options: ["ask", "current", "new"]
                .into_iter()
                .map(str::to_owned)
                .collect(),
        },
        SettingValue::Text("ask".to_owned()),
        "setting-muster-dispatch-target-label",
        "setting-muster-dispatch-target-help",
    ));
    r.setting(SettingSpec::new(
        "muster.notes.poll_s",
        SettingKind::Integer {
            min: Some(1),
            max: Some(60),
        },
        SettingValue::Integer(3),
        "setting-muster-notes-poll_s-label",
        "setting-muster-notes-poll_s-help",
    ));
    r.setting(SettingSpec::new(
        "muster.notes.retention_days",
        SettingKind::Integer {
            min: Some(1),
            max: Some(365),
        },
        SettingValue::Integer(30),
        "setting-muster-notes-retention_days-label",
        "setting-muster-notes-retention_days-help",
    ));
    r.setting(SettingSpec::new(
        "muster.handoff.require_evidence",
        SettingKind::Bool,
        SettingValue::Bool(true),
        "setting-muster-handoff-require_evidence-label",
        "setting-muster-handoff-require_evidence-help",
    ));
    r.setting(SettingSpec::new(
        "muster.escalation.max_retries",
        SettingKind::Integer {
            min: Some(0),
            max: Some(10),
        },
        SettingValue::Integer(2),
        "setting-muster-escalation-max_retries-label",
        "setting-muster-escalation-max_retries-help",
    ));
    r.setting(SettingSpec::new(
        "muster.budget.max_usd",
        SettingKind::Number {
            min: Some(0.0),
            max: Some(10000.0),
        },
        SettingValue::Number(0.0),
        "setting-muster-budget-max_usd-label",
        "setting-muster-budget-max_usd-help",
    ));
    r.setting(SettingSpec::new(
        "muster.cleanup.on_finish",
        SettingKind::Choice {
            options: ["ask", "keep", "remove"]
                .into_iter()
                .map(str::to_owned)
                .collect(),
        },
        SettingValue::Text("ask".to_owned()),
        "setting-muster-cleanup-on_finish-label",
        "setting-muster-cleanup-on_finish-help",
    ));
    r.setting(SettingSpec::new(
        "muster.workspace.isolation",
        SettingKind::Choice {
            options: ["worktree", "shared"]
                .into_iter()
                .map(str::to_owned)
                .collect(),
        },
        SettingValue::Text("worktree".to_owned()),
        "setting-muster-workspace-isolation-label",
        "setting-muster-workspace-isolation-help",
    ));
    r.setting(SettingSpec::new(
        "muster.notifications.task_failed",
        SettingKind::Bool,
        SettingValue::Bool(true),
        "setting-muster-notifications-task_failed-label",
        "setting-muster-notifications-task_failed-help",
    ));
}

#[cfg(test)]
#[cfg(test)]
mod tests_cleanup;
#[cfg(test)]
mod tests_dispatch;
#[cfg(test)]
mod tests_flow;
#[cfg(test)]
mod tests_model;
#[cfg(test)]
mod tests_poll;
#[cfg(test)]
mod tests_support;
