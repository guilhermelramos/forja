//! Módulo `review` da Forja: PR/MR, checks, threads e texto gerado por ajudante.

mod ai;
mod api;
mod bench_base;
mod client;
mod commands;
mod fixtures;
mod parse;
mod state;
mod view;
mod view_empty;

use crate::state::Shared;
use forja_contracts::{BenchActivated, GitComparisonChanged, GitStatusChanged, names};
use forja_kernel::forja_protocol::{
    Method, MethodEffect, ParamKind, ParamSchema, ResultSchema, RpcError,
};
use forja_kernel::{Args, Module, ModuleManifest, Registrar, Slot};

#[forja_kernel::module]
static REVIEW: fn() -> Box<dyn Module> = || Box::new(ReviewModule::new());

pub struct ReviewModule {
    state: Shared,
}

impl ReviewModule {
    pub fn new() -> Self {
        Self {
            state: Shared::new(),
        }
    }
}

impl Default for ReviewModule {
    fn default() -> Self {
        Self::new()
    }
}

impl Module for ReviewModule {
    fn manifest(&self) -> ModuleManifest {
        ModuleManifest::new("review", "review.name", env!("CARGO_PKG_VERSION"))
    }

    fn register(&mut self, r: &mut Registrar<'_>) {
        r.i18n(
            include_str!("../i18n/en.ftl"),
            include_str!("../i18n/pt-BR.ftl"),
        );
        r.keymap(include_str!("../keymap.toml"));

        r.contract_event::<BenchActivated>(names::BENCH_ACTIVATED);
        r.contract_event::<GitStatusChanged>(names::GIT_STATUS_CHANGED);
        r.contract_event::<GitComparisonChanged>(names::GIT_COMPARISON_CHANGED);
        let bench_state = self.state.clone();
        r.subscribe(move |_cx, event: &BenchActivated| {
            bench_state.set_bench(Some(event.0.path.clone()));
        });

        let status_state = self.state.clone();
        r.subscribe(move |cx, event: &GitStatusChanged| {
            if status_state.bench().as_deref() == Some(event.bench_path.as_str()) {
                let base = status_state
                    .with(|state| (!state.base.trim().is_empty()).then(|| state.base.clone()))
                    .flatten();
                commands::start_comparison(cx, &status_state, &event.bench_path, base.as_deref());
            }
        });

        let comparison_state = self.state.clone();
        r.subscribe(move |cx, event: &GitComparisonChanged| {
            commands::comparison_changed(cx, &comparison_state, &event.bench_path);
        });

        let refresh = self.state.clone();
        r.command(
            "review.refresh",
            "review.command.refresh",
            move |cx, args| commands::refresh(cx, &refresh, &args),
        );

        let create = self.state.clone();
        r.command(
            "review.pr.create",
            "review.command.create",
            move |cx, args| commands::create(cx, &create, &args),
        );

        let merge = self.state.clone();
        r.command("review.merge", "review.command.merge", move |cx, args| {
            commands::merge(cx, &merge, &args)
        });

        let reply = self.state.clone();
        r.command(
            "review.thread.reply",
            "review.command.reply",
            move |cx, args| commands::reply(cx, &reply, &args),
        );

        let resolve = self.state.clone();
        r.command(
            "review.thread.resolve",
            "review.command.resolve",
            move |cx, args| commands::resolve(cx, &resolve, &args),
        );

        let rerun = self.state.clone();
        r.command(
            "review.checks.rerun",
            "review.command.rerun",
            move |cx, args| commands::rerun(cx, &rerun, &args),
        );

        let log = self.state.clone();
        r.command(
            "review.checks.log",
            "review.command.log",
            move |cx, args| commands::log(cx, &log, &args),
        );

        let generate = self.state.clone();
        r.command(
            "review.generate",
            "review.command.generate",
            move |cx, args| commands::generate(cx, &generate, &args),
        );

        let fix = self.state.clone();
        r.command("review.fix", "review.command.fix", move |cx, args| {
            commands::fix(cx, &fix, &args)
        });

        register_rpc(r, &self.state);

        for key in used_keys() {
            r.uses_i18n(key);
        }

        let view_state = self.state.clone();
        r.view(
            Slot::BenchTab,
            "review.tab",
            "review.tab.title",
            bench_tab_order(45),
            move || Box::new(view::ReviewView::new(view_state.clone())),
        );
    }
}

fn register_rpc(r: &mut Registrar<'_>, shared: &Shared) {
    let refresh = shared.clone();
    r.rpc(
        Method::new("review.refresh", "review.method.refresh.description")
            .effect(MethodEffect::Read)
            .param(ParamSchema::optional("bench_path", ParamKind::String))
            .result(ResultSchema::new("review.method.refresh.result")),
        move |cx, params| {
            let args = Args::from_map(params.clone());
            commands::refresh(cx, &refresh, &args).map_err(rpc_error)
        },
    );

    let create = shared.clone();
    r.rpc(
        Method::new("review.pr.create", "review.method.create.description")
            .effect(MethodEffect::Write)
            .param(ParamSchema::optional("bench_path", ParamKind::String))
            .param(ParamSchema::required("title", ParamKind::String))
            .param(ParamSchema::required("body", ParamKind::String))
            .param(ParamSchema::optional("base", ParamKind::String))
            .param(ParamSchema::optional("draft", ParamKind::Boolean))
            .result(ResultSchema::new("review.method.create.result")),
        move |cx, params| {
            let args = Args::from_map(params.clone());
            commands::create(cx, &create, &args).map_err(rpc_error)
        },
    );

    let merge = shared.clone();
    r.rpc(
        Method::new("review.merge", "review.method.merge.description")
            .effect(MethodEffect::Write)
            .param(ParamSchema::optional("method", ParamKind::String))
            .param(ParamSchema::optional("delete_branch", ParamKind::Boolean))
            .param(ParamSchema::optional("auto_merge", ParamKind::Boolean))
            .result(ResultSchema::new("review.method.merge.result")),
        move |cx, params| {
            let args = Args::from_map(params.clone());
            commands::merge(cx, &merge, &args).map_err(rpc_error)
        },
    );

    let generate = shared.clone();
    r.rpc(
        Method::new("review.generate", "review.method.generate.description")
            .effect(MethodEffect::Write)
            .param(ParamSchema::required("kind", ParamKind::String))
            .param(ParamSchema::optional("bench_path", ParamKind::String))
            .result(ResultSchema::new("review.method.generate.result")),
        move |cx, params| {
            let args = Args::from_map(params.clone());
            commands::generate(cx, &generate, &args).map_err(rpc_error)
        },
    );
}

fn rpc_error(error: forja_kernel::KernelError) -> RpcError {
    RpcError::app(error.to_string())
}

fn bench_tab_order(default: i32) -> i32 {
    match std::env::var("FORJA_BENCH_TAB") {
        Ok(module) if module == "review" => 1,
        _ => default,
    }
}

fn used_keys() -> Vec<&'static str> {
    vec![
        "review.name",
        "review.tab.title",
        "review.command.refresh",
        "review.command.create",
        "review.command.merge",
        "review.command.reply",
        "review.command.resolve",
        "review.command.rerun",
        "review.command.log",
        "review.command.generate",
        "review.command.fix",
        "review.command.copy",
        "review.generated.use",
        "review.provider.github",
        "review.provider.gitlab",
        "review.empty.title",
        "review.empty.body",
        "review.empty.refresh",
        "review.create.title",
        "review.create.base",
        "review.create.body",
        "review.create.draft",
        "review.create.reviewers",
        "review.create.labels",
        "review.create.generate",
        "review.create.open",
        "review.create.branch",
        "review.create.source-label",
        "review.summary.title",
        "review.summary.loading",
        "review.summary.remote-missing",
        "review.summary.base",
        "review.summary.choose-base-label",
        "review.summary.head",
        "review.summary.no-base",
        "review.summary.ahead",
        "review.summary.nothing-to-send",
        "review.summary.commits",
        "review.summary.no-commits",
        "review.summary.files",
        "review.summary.no-files",
        "review.summary.binary",
        "review.summary.total",
        "review.summary.view-diff",
        "review.summary.choose-base",
        "review.summary.base-missing",
        "review.summary.use-base",
        "review.summary.provider-unavailable",
        "review.error.title-required",
        "review.pr.title",
        "review.pr.open",
        "review.pr.draft",
        "review.pr.state",
        "review.pr.target",
        "review.pr.source",
        "review.pr.reviewers",
        "review.pr.labels",
        "review.pr.open-browser",
        "review.checks.title",
        "review.checks.empty",
        "review.checks.rerun",
        "review.checks.log",
        "review.checks.fix",
        "review.checks.pending",
        "review.checks.failed",
        "review.checks.passed",
        "review.threads.title",
        "review.threads.empty",
        "review.threads.resolved",
        "review.threads.reply",
        "review.threads.send",
        "review.threads.resolve",
        "review.threads.unresolve",
        "review.merge.title",
        "review.merge.method",
        "review.merge.merge",
        "review.merge.squash",
        "review.merge.rebase",
        "review.merge.delete-branch",
        "review.merge.auto",
        "review.merge.action",
        "review.generated.title",
        "review.notice.pr-created",
        "review.notice.merged",
        "review.notice.rerun",
        "review.notice.generated",
        "review.error.no-bench",
        "review.error.no-pr",
        "review.error.no-changes",
        "review.error.no-check-log",
        "review.error.bad-origin",
        "review.error.agent",
        "review.method.refresh.description",
        "review.method.refresh.result",
        "review.method.create.description",
        "review.method.create.result",
        "review.method.merge.description",
        "review.method.merge.result",
        "review.method.generate.description",
        "review.method.generate.result",
    ]
}

#[cfg(test)]
mod tests {
    use super::*;
    use forja_kernel::{Runtime, RuntimeOptions, contract};
    use serde_json::json;

    fn temp_dir() -> std::path::PathBuf {
        let dir = std::env::temp_dir().join(format!(
            "forja-review-{}-{}",
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
            vec![Box::new(ReviewModule::new())],
            RuntimeOptions {
                data_dir: Some(dir.to_path_buf()),
                user_dir: Some(dir.join("user")),
                locale: Some("pt-BR".to_owned()),
                wake: None,
            },
        )
        .expect("review registers cleanly")
    }

    #[test]
    fn contract_holds() {
        let dir = temp_dir();
        let runtime = runtime(&dir);
        contract::assert_ok(runtime.registry(), runtime.i18n());
        let _ = std::fs::remove_dir_all(dir);
    }

    #[test]
    fn refresh_without_a_bench_degrades_with_an_error() {
        let dir = temp_dir();
        let mut runtime = runtime(&dir);
        assert!(runtime.run_command("review.refresh", Args::new()).is_err());
        let _ = std::fs::remove_dir_all(dir);
    }

    #[test]
    fn ai_generation_rejects_unknown_kinds() {
        let dir = temp_dir();
        let mut runtime = runtime(&dir);
        let mut args = Args::new();
        args.insert("kind", json!("nonsense"));
        assert!(runtime.run_command("review.generate", args).is_err());
        let _ = std::fs::remove_dir_all(dir);
    }

    #[test]
    fn raw_review_errors_are_humanized_in_pt_br_and_en() {
        let dir = temp_dir();
        let mut rt = runtime(&dir);
        {
            let cx = rt.ctx("review");
            let git_err = commands::humanize_error(&cx, "git command failed");
            assert!(!git_err.summary.to_lowercase().contains("failed"));
            assert_eq!(git_err.detail.as_deref(), Some("git command failed"));

            let no_pr = commands::humanize_error(&cx, "open a pull request first");
            assert!(no_pr.summary.contains("pull request"));
            assert!(no_pr.detail.is_none());
        }
        rt.set_locale(forja_kernel::Locale::En);
        let cx = rt.ctx("review");
        let git_err_en = commands::humanize_error(&cx, "git command failed");
        assert!(!git_err_en.summary.to_lowercase().contains("failed"));
        assert_eq!(git_err_en.detail.as_deref(), Some("git command failed"));
        let _ = std::fs::remove_dir_all(dir);
    }
}
