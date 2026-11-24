//! The shell is itself a module: it contributes its own i18n bundle. Modules
//! can depend on it without knowing about the app binary.

use crate::actions::{self, Action};
use forja_kernel::forja_protocol::{Method, MethodEffect, ParamKind, ParamSchema, ResultSchema};
use forja_kernel::{Module, ModuleManifest, Registrar};
use serde_json::json;

pub struct ShellModule;

impl Module for ShellModule {
    fn manifest(&self) -> ModuleManifest {
        ModuleManifest::new("shell", "shell.name", env!("CARGO_PKG_VERSION"))
    }

    fn register(&mut self, registrar: &mut Registrar<'_>) {
        registrar.i18n(
            include_str!("../i18n/en.ftl"),
            include_str!("../i18n/pt-BR.ftl"),
        );
        for key in CHROME_KEYS {
            registrar.uses_i18n(key);
        }
        open_rail(registrar);
        focus_bench(registrar);
        open_logs(registrar);
        toggle_diagnostics(registrar);
    }
}

fn open_logs(registrar: &mut Registrar<'_>) {
    registrar.command("shell.logs.open", "shell.command.logs-open", |cx, _args| {
        let path = forja_kernel::logging::logs_dir(cx.data_dir());
        let displayed = path.display().to_string();
        #[cfg(not(test))]
        {
            let task_path = path.clone();
            let error_path = displayed.clone();
            cx.spawn(
                move || {
                    std::fs::create_dir_all(&task_path)
                        .and_then(|()| forja_kernel::process::reveal_folder(&task_path))
                        .map_err(|error| error.to_string())
                },
                move |_cx, result| {
                    if let Err(error) = result {
                        forja_kernel::logging::log(
                            forja_kernel::logging::LogLevel::Error,
                            format!("could not open log folder `{error_path}`: {error}"),
                        );
                    }
                },
            );
        }
        Ok(json!({ "path": displayed }))
    });
}

fn focus_bench(registrar: &mut Registrar<'_>) {
    registrar.command(
        "shell.bench.focus",
        "shell.command.bench-focus",
        |_, args| {
            let project = args.required_str("project")?.to_owned();
            let bench = args.required_str("bench")?.to_owned();
            let bench_id = args.str("bench_id").map(str::to_owned);
            let keep_page = args.bool("keep_page").unwrap_or(false);
            actions::push(Action::OpenBench {
                project: project.clone(),
                bench: bench.clone(),
                bench_id,
                keep_page,
            });
            Ok(json!({ "project": project, "bench": bench }))
        },
    );
    registrar.command("shell.tab.open", "shell.command.tab-open", |_, args| {
        let view_id = args.required_str("view_id")?.to_owned();
        actions::push(Action::ShowBenchTab(view_id.clone()));
        Ok(json!({ "opening": true, "view_id": view_id }))
    });
}

/// Anvil command that flips `app.diagnostics.overlay`, the same key the panel
/// reads, so the panel survives a restart and shows up in Ajustes.
fn toggle_diagnostics(registrar: &mut Registrar<'_>) {
    registrar.command(
        "shell.diagnostics.toggle",
        "shell.command.diagnostics-toggle",
        |cx, _args| {
            let visible = !cx
                .setting("app.diagnostics.overlay")
                .and_then(forja_kernel::settings::SettingValue::as_bool)
                .unwrap_or(false);
            cx.set_setting(
                "app.diagnostics.overlay",
                forja_kernel::settings::SettingValue::Bool(visible),
            )?;
            cx.request_repaint();
            Ok(json!({ "visible": visible }))
        },
    );
}

fn open_rail(registrar: &mut Registrar<'_>) {
    registrar.command("shell.rail.open", "shell.command.rail-open", |_, args| {
        let view_id = args.required_str("view_id")?.to_owned();
        actions::push(Action::OpenRail(view_id.clone()));
        Ok(json!({ "opening": true, "view_id": view_id }))
    });
    registrar.rpc(
        Method::new("shell.rail.open", "shell.method.rail-open.description")
            .effect(MethodEffect::Write)
            .param(ParamSchema::required("view_id", ParamKind::String))
            .result(
                ResultSchema::new("shell.method.rail-open.result").schema(json!({
                    "type": "object",
                    "properties": {
                        "opening": { "type": "boolean" },
                        "view_id": { "type": "string" }
                    },
                    "required": ["opening", "view_id"]
                })),
            ),
        |_, params| {
            let view_id = params
                .get("view_id")
                .and_then(serde_json::Value::as_str)
                .ok_or_else(|| forja_kernel::forja_protocol::RpcError::app("missing view_id"))?
                .to_owned();
            actions::push(Action::OpenRail(view_id.clone()));
            Ok(json!({ "opening": true, "view_id": view_id }))
        },
    );
}

const CHROME_KEYS: &[&str] = &[
    "shell.top.find",
    "shell.top.find-hint",
    "shell.top.anvil",
    "shell.top.settings",
    "shell.top.right-panel",
    "shell.top.theme",
    "shell.top.no-project",
    "shell.window.minimize",
    "shell.window.maximize",
    "shell.window.close",
    "shell.window.minimize-glyph",
    "shell.window.maximize-glyph",
    "shell.window.close-glyph",
    "shell.rail.title",
    "shell.rail.empty",
    "shell.rail.dot",
    "shell.rail.settings",
    "shell.rail.bench",
    "shell.tabs.more",
    "shell.tabs.overflow",
    "shell.tabs.add",
    "shell.tabs.close",
    "shell.sidebar.title",
    "shell.sidebar.empty",
    "shell.bench.empty-title",
    "shell.bench.empty-hint",
    "shell.anvil.title",
    "shell.anvil.search",
    "shell.anvil.ask-search",
    "shell.anvil.ask-text-hint",
    "shell.anvil.ask-empty",
    "shell.anvil.ask-empty-new-bench",
    "shell.anvil.ask-empty-add-project",
    "shell.anvil.command-arguments",
    "shell.anvil.ask-footer",
    "shell.anvil.choose-rail",
    "shell.anvil.muster-title",
    "shell.anvil.helper-not-installed",
    "shell.anvil.helper-none-installed",
    "shell.anvil.helper-install-guide",
    "shell.anvil.helper-open-helpers",
    "shell.anvil.helper-copy-install",
    "shell.anvil.helper-copied",
    "shell.anvil.empty",
    "shell.anvil.footer",
    "shell.anvil.group.all",
    "shell.anvil.group.commands",
    "shell.anvil.group.benches",
    "shell.anvil.group.files",
    "shell.anvil.group.settings",
    "shell.anvil.benches-missing",
    "shell.anvil.files-missing",
    "shell.anvil.sources",
    "shell.settings.title",
    "shell.settings.search",
    "shell.settings.empty",
    "shell.status.ready",
    "shell.status.module",
    "shell.status.modules",
    "shell.status.error",
    "shell.status.task",
    "shell.settings.save-failed",
    "shell.command.failed",
    "shell.rail.not-found",
    "shell.status.details-title",
    "shell.status.copy",
    "shell.slot.empty",
    "shell.tray.show",
    "shell.tray.quit",
    "shell.tray.tip",
    "shell.close.confirm.title",
    "shell.close.confirm.body",
    "shell.close.confirm.helpers",
    "shell.close.confirm.routines",
    "shell.close.confirm.close",
    "shell.close.confirm.tray",
    "shell.close.confirm.cancel",
    "shell.toast.title",
    "shell.toast.body",
    "shell.theme.system",
    "shell.theme.dark",
    "shell.theme.light",
    "shell.command.rail-open",
    "shell.method.rail-open.description",
    "shell.method.rail-open.result",
    "shell.command.bench-focus",
    "shell.command.tab-open",
    "shell.command.logs-open",
    "shell.command.diagnostics-toggle",
    "shell.diagnostics.title",
    "shell.diagnostics.fps",
    "shell.diagnostics.frame-time",
    "shell.diagnostics.cause",
    "shell.diagnostics.frames",
    "shell.diagnostics.memory",
    "shell.diagnostics.threads",
    "shell.crash.title",
    "shell.crash.body",
    "shell.view-error.title",
    "shell.view-error.message",
    "shell.view-error.retry",
    "shell.view-error.unknown",
    "shell.view-error.debug-panic",
    "setting-app-log-level-label",
    "setting-app-log-level-help",
    "setting-app-log-level-option-error",
    "setting-app-log-level-option-warn",
    "setting-app-log-level-option-info",
    "setting-app-log-level-option-debug",
    "setting-app-log-level-option-trace",
    "setting-app-log-max_mb-label",
    "setting-app-log-max_mb-help",
    "setting-app-log-keep-label",
    "setting-app-log-keep-help",
    "setting-app-log-include_terminal_output-label",
    "setting-app-log-include_terminal_output-help",
    "setting-app-performance-max_fps-label",
    "setting-app-performance-max_fps-help",
    "setting-app-performance-unfocused_fps-label",
    "setting-app-performance-unfocused_fps-help",
    "setting-app-diagnostics-overlay-label",
    "setting-app-diagnostics-overlay-help",
    "shell.storage.restart-required",
    "shell.storage.choose",
    "shell.storage.apply",
    "shell.storage.copy-title",
    "shell.storage.copy-body",
    "shell.storage.copy-confirm",
    "shell.storage.copy-empty",
    "shell.storage.copy-cancel",
    "shell.storage.copy-done",
    "shell.storage.copy-failed",
];

#[cfg(test)]
mod tests {
    use super::*;
    use forja_kernel::{contract, Runtime, RuntimeOptions};
    use serde_json::Map;

    #[test]
    fn rail_open_command_queues_the_page_action() {
        let dir = std::env::temp_dir().join(format!(
            "forja-shell-rail-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        std::fs::create_dir_all(&dir).unwrap();
        let mut runtime = Runtime::new(
            vec![Box::new(ShellModule)],
            RuntimeOptions {
                data_dir: Some(dir.clone()),
                user_dir: Some(dir.join("user")),
                locale: Some("pt-BR".to_owned()),
                wake: None,
            },
        )
        .expect("shell module registers");
        contract::assert_ok(runtime.registry(), runtime.i18n());
        let _ = actions::drain();
        runtime
            .call_method(
                "shell.rail.open",
                Map::from_iter([("view_id".to_owned(), json!("muster.page"))]),
            )
            .expect("rail command");
        assert!(actions::drain().contains(&Action::OpenRail("muster.page".to_owned())));
        drop(runtime);
        let _ = std::fs::remove_dir_all(dir);
    }

    #[test]
    fn bench_focus_and_tab_open_queue_actions() {
        let dir = std::env::temp_dir().join(format!(
            "forja-shell-bench-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        std::fs::create_dir_all(&dir).unwrap();
        let mut runtime = Runtime::new(
            vec![Box::new(ShellModule)],
            RuntimeOptions {
                data_dir: Some(dir.clone()),
                user_dir: Some(dir.join("user")),
                locale: Some("pt-BR".to_owned()),
                wake: None,
            },
        )
        .expect("shell module registers");
        let _ = actions::drain();
        runtime
            .run_command(
                "shell.bench.focus",
                forja_kernel::Args::from_map(Map::from_iter([
                    ("project".to_owned(), json!("projeto")),
                    ("bench".to_owned(), json!("bancada")),
                ])),
            )
            .expect("focus command");
        runtime
            .run_command(
                "shell.tab.open",
                forja_kernel::Args::from_map(Map::from_iter([(
                    "view_id".to_owned(),
                    json!("editor.tab"),
                )])),
            )
            .expect("tab command");
        assert_eq!(
            actions::drain(),
            vec![
                Action::OpenBench {
                    project: "projeto".to_owned(),
                    bench: "bancada".to_owned(),
                    bench_id: None,
                    keep_page: false,
                },
                Action::ShowBenchTab("editor.tab".to_owned()),
            ]
        );
        drop(runtime);
        let _ = std::fs::remove_dir_all(dir);
    }

    #[test]
    fn open_logs_command_targets_the_runtime_log_folder_without_opening_it() {
        let dir = std::env::temp_dir().join(format!(
            "forja-shell-logs-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        let mut runtime = Runtime::new(
            vec![Box::new(ShellModule)],
            RuntimeOptions {
                data_dir: Some(dir.clone()),
                user_dir: Some(dir.join("user")),
                locale: Some("pt-BR".to_owned()),
                wake: None,
            },
        )
        .expect("shell module registers");
        assert!(runtime.registry().command("shell.logs.open").is_some());
        let result = runtime
            .run_command("shell.logs.open", forja_kernel::Args::new())
            .expect("log folder command");
        let expected = forja_kernel::logging::logs_dir(&dir)
            .to_string_lossy()
            .into_owned();
        assert_eq!(result["path"].as_str(), Some(expected.as_str()));
        drop(runtime);
        let _ = std::fs::remove_dir_all(dir);
    }

    #[test]
    fn diagnostics_command_flips_the_overlay_setting() {
        let dir = std::env::temp_dir().join(format!(
            "forja-shell-diagnostics-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        std::fs::create_dir_all(&dir).unwrap();
        let mut runtime = Runtime::new(
            vec![
                Box::new(ShellModule),
                Box::new(crate::hosts::Host {
                    id: "app",
                    name_key: "app.name",
                }),
            ],
            RuntimeOptions {
                data_dir: Some(dir.clone()),
                user_dir: Some(dir.join("user")),
                locale: Some("pt-BR".to_owned()),
                wake: None,
            },
        )
        .expect("shell module registers");
        contract::assert_ok(runtime.registry(), runtime.i18n());
        let off = runtime
            .run_command("shell.diagnostics.toggle", forja_kernel::Args::new())
            .expect("toggle command");
        let on = runtime
            .run_command("shell.diagnostics.toggle", forja_kernel::Args::new())
            .expect("toggle command");
        assert_eq!(off["visible"].as_bool(), Some(true));
        assert_eq!(on["visible"].as_bool(), Some(false));
        assert_eq!(
            runtime
                .settings()
                .get("app.diagnostics.overlay")
                .and_then(forja_kernel::SettingValue::as_bool),
            Some(false)
        );
        drop(runtime);
        let _ = std::fs::remove_dir_all(dir);
    }

    #[test]
    fn command_status_errors_hide_raw_os_codes() {
        let dir = std::env::temp_dir().join(format!("forja-shell-err-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let mut runtime = Runtime::new(
            vec![Box::new(ShellModule)],
            RuntimeOptions {
                data_dir: Some(dir.clone()),
                user_dir: Some(dir.join("user")),
                locale: Some("pt-BR".to_owned()),
                wake: None,
            },
        )
        .expect("shell module registers");
        let err = forja_kernel::KernelError::Command {
            id: "git.status".to_owned(),
            message: "Git operation failed (detail: could not run git: The directory name is invalid. (os error 267)).".to_owned(),
        };
        let human = forja_kernel::human_error::humanize_kernel_error(runtime.i18n(), &err);
        let status_pt = runtime.i18n().t_args(
            "shell.command.failed",
            &[("id", "Atualizar"), ("message", &human.summary)],
        );
        assert!(!status_pt.contains("os error"), "{status_pt}");
        assert!(!status_pt.to_lowercase().contains("failed"), "{status_pt}");
        assert!(human
            .detail
            .as_deref()
            .is_some_and(|detail| detail.contains("os error 267")));

        runtime.set_locale(forja_kernel::Locale::En);
        let human_en = forja_kernel::human_error::humanize_kernel_error(runtime.i18n(), &err);
        let status_en = runtime.i18n().t_args(
            "shell.command.failed",
            &[("id", "Refresh"), ("message", &human_en.summary)],
        );
        assert!(!status_en.contains("os error"), "{status_en}");
        assert!(!status_en.to_lowercase().contains("failed"), "{status_en}");

        let helper_err = forja_kernel::KernelError::Command {
            id: "content.run".to_owned(),
            message: "command `helper.launch` failed: No installed helpers found on PATH."
                .to_owned(),
        };
        let helper_en =
            forja_kernel::human_error::humanize_kernel_error(runtime.i18n(), &helper_err);
        assert!(
            helper_en
                .summary
                .contains("No installed helper was found on PATH"),
            "{}",
            helper_en.summary
        );
        assert!(!helper_en.summary.contains('`'));
        let helper_detail = helper_en.detail.as_deref().unwrap_or("");
        assert!(helper_detail.contains("No installed helpers found"), "{helper_detail}");
        // Detalhes recolhidos mostram o original, inclusive o comando interno.
        assert!(helper_detail.contains("helper.launch"), "{helper_detail}");

        drop(runtime);
        let _ = std::fs::remove_dir_all(dir);
    }
}
