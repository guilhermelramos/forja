//! The shell is itself a module: it contributes its own i18n bundle. Modules
//! can depend on it without knowing about the app binary.

use crate::actions::{self, Action};
use forja_kernel::forja_protocol::{
    Method, MethodEffect, ParamKind, ParamSchema, ResultSchema,
};
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
    }
}

fn focus_bench(registrar: &mut Registrar<'_>) {
    registrar.command(
        "shell.bench.focus",
        "shell.command.bench-focus",
        |_, args| {
            let project = args.required_str("project")?.to_owned();
            let bench = args.required_str("bench")?.to_owned();
            if args.bool("keep_page").unwrap_or(false) {
                actions::push(Action::RefreshProjectCrumb);
            } else {
                actions::push(Action::OpenBench {
                    project: project.clone(),
                    bench: bench.clone(),
                });
            }
            Ok(json!({ "project": project, "bench": bench }))
        },
    );
    registrar.command("shell.tab.open", "shell.command.tab-open", |_, args| {
        let view_id = args.required_str("view_id")?.to_owned();
        actions::push(Action::ShowBenchTab(view_id.clone()));
        Ok(json!({ "opening": true, "view_id": view_id }))
    });
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
            .result(ResultSchema::new("shell.method.rail-open.result").schema(json!({
                "type": "object",
                "properties": {
                    "opening": { "type": "boolean" },
                    "view_id": { "type": "string" }
                },
                "required": ["opening", "view_id"]
            }))),
        |_, params| {
            let view_id = params
                .get("view_id")
                .and_then(serde_json::Value::as_str)
                .ok_or_else(|| {
                    forja_kernel::forja_protocol::RpcError::app("missing view_id")
                })?
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
    "shell.rail.back-to-bench",
    "shell.tabs.more",
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
                },
                Action::ShowBenchTab("editor.tab".to_owned()),
            ]
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
        let helper_en = forja_kernel::human_error::humanize_kernel_error(runtime.i18n(), &helper_err);
        assert!(
            helper_en.summary.contains("No installed helper was found on PATH"),
            "{}",
            helper_en.summary
        );
        assert!(!helper_en.summary.contains('`'));
        assert!(helper_en
            .detail
            .as_deref()
            .is_some_and(|detail| detail.contains("helper.launch")));

        drop(runtime);
        let _ = std::fs::remove_dir_all(dir);
    }
}
