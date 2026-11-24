use crate::config;
use crate::git;
use crate::model::ScriptApproval;
use crate::scripts;
use crate::service::{
    close_terminals, ensure_main, notify, project_path, setting_bool, setting_i64, setting_str,
};
use crate::state::Shared;
use forja_contracts::{BenchStored, BenchStoreOutcome};
use forja_kernel::Ctx;
use std::path::{Path, PathBuf};
pub fn store(
    cx: &mut Ctx<'_>,
    shared: &Shared,
    bench_id: &str,
    delete_branch: Option<bool>,
    force: Option<bool>,
) -> Result<(), String> {
    ensure_main(cx, shared)?;
    let bench = shared
        .find(bench_id)
        .ok_or_else(|| "bench not found".to_owned())?;
    if bench.is_main {
        return Err(cx.t("bench.error.main-cannot-store"));
    }
    if bench.archived {
        return Ok(());
    }
    let project_path = project_path(cx, &bench.project_id)
        .ok_or_else(|| "project is not registered".to_owned())?;
    let branch_policy = setting_str(cx, "bench.teardown.branch", "ask");
    let delete_branch = delete_branch.unwrap_or(branch_policy == "delete");
    let job = StoreJob {
        bench_id: bench_id.to_owned(),
        bench_name: bench.name.clone(),
        project_path,
        path: PathBuf::from(&bench.path),
        branch: bench.branch.clone(),
        delete_branch,
        force: force.unwrap_or(false),
        confirm_dirty: setting_bool(cx, "bench.close.confirm_dirty", true),
        teardown_policy: setting_str(cx, "bench.teardown.policy", "ask"),
        timeout_s: setting_i64(cx, "bench.setup.timeout_s", 300),
        approval: shared.approval(&bench.project_id),
    };
    let state = shared.clone();
    cx.spawn(
        move || run_store(job),
        move |cx, result| finish_store(cx, &state, result),
    );
    Ok(())
}

struct StoreJob {
    bench_id: String,
    bench_name: String,
    project_path: String,
    path: PathBuf,
    branch: Option<String>,
    delete_branch: bool,
    force: bool,
    confirm_dirty: bool,
    teardown_policy: String,
    timeout_s: i64,
    approval: ScriptApproval,
}

struct Stored {
    bench_id: String,
    bench_name: String,
    branch: Option<String>,
    delete_branch: bool,
    teardown: scripts::ScriptOutcome,
    teardown_skipped: Option<&'static str>,
    warnings: Vec<String>,
}

/// Falha da tarefa de bancadas. `dirty` distingue a recusa do git das outras
/// falhas, para que o resultado publicado diga se a bancada ficou por sujeira.
struct StoreFailure {
    bench_id: String,
    message: String,
    dirty: bool,
}

fn run_store(job: StoreJob) -> Result<Stored, StoreFailure> {
    let project = Path::new(&job.project_path);
    let mut warnings = Vec::new();
    let mut teardown = scripts::ScriptOutcome::default();
    let mut teardown_skipped = None;
    if job.path.exists() {
        // O git é conferido aqui, no work closure, logo antes de apagar.
        let dirty = git::is_dirty(&job.path).unwrap_or(false);
        if dirty && job.confirm_dirty && !job.force {
            return Err(StoreFailure {
                bench_id: job.bench_id,
                message: "bench has uncommitted changes; confirm force".to_owned(),
                dirty: true,
            });
        }
        let loaded = config::load(&job.project_path);
        warnings.extend(loaded.warnings.clone());
        if job.teardown_policy == "never" {
            if !loaded.config.teardown.is_empty() {
                teardown_skipped = Some("policy");
            }
        } else if let Some(hash) = &loaded.teardown_hash {
            if job.approval.teardown_hash.as_deref() == Some(hash.as_str()) {
                teardown = scripts::run(&loaded.config.teardown, &job.path, job.timeout_s);
            } else {
                teardown_skipped = Some("unapproved");
            }
        }
        git::remove_worktree(project, &job.path, job.force || dirty).map_err(|message| {
            StoreFailure {
                bench_id: job.bench_id.clone(),
                message,
                dirty: false,
            }
        })?;
    } else {
        warnings.push("the bench folder was already missing".to_owned());
    }
    if job.delete_branch {
        if let Some(branch) = &job.branch {
            if let Err(error) = git::delete_branch(project, branch) {
                warnings.push(error);
            }
        }
    }
    Ok(Stored {
        bench_id: job.bench_id,
        bench_name: job.bench_name,
        branch: job.branch,
        delete_branch: job.delete_branch,
        teardown,
        teardown_skipped,
        warnings,
    })
}

fn finish_store(cx: &mut Ctx<'_>, shared: &Shared, result: Result<Stored, StoreFailure>) {
    let stored = match result {
        Ok(stored) => stored,
        Err(failure) => {
            // O `done` só publica o resultado; nenhum git roda aqui.
            cx.emit(&BenchStoreOutcome {
                bench_id: failure.bench_id.clone(),
                outcome: if failure.dirty {
                    "kept_dirty".to_owned()
                } else {
                    "error".to_owned()
                },
            });
            let human = crate::create::humanize_error(cx, &failure.message);
            shared.set_error(Some(failure.message));
            notify(cx, cx.t("bench.notify.store-failed.title"), human.summary, "error");
            return;
        }
    };
    cx.emit(&BenchStoreOutcome {
        bench_id: stored.bench_id.clone(),
        outcome: "stored".to_owned(),
    });
    shared.set_error(None);
    let path = if stored.delete_branch {
        None
    } else {
        stored.branch.clone()
    };
    if let Err(error) = shared.mutate(cx, |file| {
        if let Some(bench) = file.find_mut(&stored.bench_id) {
            bench.archived = true;
            bench.sleeping = false;
            bench.unread = false;
            bench.branch = path.clone();
        }
        if file.current.as_deref() == Some(stored.bench_id.as_str()) {
            file.current = None;
        }
    }) {
        shared.set_error(Some(error));
        return;
    }
    let bench = shared.find(&stored.bench_id);
    if let Some(bench) = &bench {
        close_terminals(cx, &bench.path);
        cx.emit(&BenchStored {
            bench_id: bench.bench_id.clone(),
            path: bench.path.clone(),
        });
    }
    if stored.teardown.ran && !stored.teardown.success {
        let output = if stored.teardown.timed_out {
            cx.t("bench.notify.setup-timeout")
        } else {
            stored.teardown.output.clone()
        };
        let summary = crate::create::humanize_error(cx, &output).summary;
        notify(
            cx,
            cx.t("bench.notify.teardown-failed.title"),
            cx.t_args(
                "bench.notify.teardown-failed.body",
                &[("name", &stored.bench_name), ("message", &summary)],
            ),
            "error",
        );
    } else {
        notify(
            cx,
            cx.t("bench.notify.stored.title"),
            cx.t_args("bench.notify.stored.body", &[("name", &stored.bench_name)]),
            "success",
        );
    }
    if stored.teardown_skipped == Some("unapproved") {
        shared.set_notice(Some(cx.t_args(
            "bench.notify.teardown-unapproved.body",
            &[("name", &stored.bench_name)],
        )));
    }
    if !stored.warnings.is_empty() {
        shared.set_notice(Some(stored.warnings.join("; ")));
    }
}
