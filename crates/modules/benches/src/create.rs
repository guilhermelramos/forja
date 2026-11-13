use crate::config;
use crate::git;
use crate::model::{self, now_millis, BenchRecord, ScriptApproval};
use crate::scripts;
use crate::service::{
    bench_path, ensure_main, notify, project_by_id, setting_bool, setting_i64, setting_list,
    setting_str,
};
use crate::tabs::open_tabs;
use crate::stages;
use crate::state::{ApproveForm, Dialog, Shared};
use forja_contracts::{BenchCreated, BenchRef, ProjectRef};
use forja_kernel::Ctx;
use std::path::{Path, PathBuf};
pub fn create(
    cx: &mut Ctx<'_>,
    shared: &Shared,
    project_id: &str,
    name: &str,
    base: Option<&str>,
    from_request: Option<&str>,
    fetch: Option<bool>,
) -> Result<BenchRef, String> {
    ensure_main(cx, shared)?;
    if name.trim().is_empty() {
        return Err("bench name is required".to_owned());
    }
    let project = project_by_id(cx, project_id)
        .ok_or_else(|| format!("project `{project_id}` is not registered"))?;
    let template = setting_str(cx, "bench.worktree.name_template", "{pedido}-{slug}");
    let number = shared.file().benches.len() as u32 + 1;
    let bench_name = model::bench_name(&template, from_request, name, number);
    let prefix = setting_str(cx, "bench.worktree.branch_prefix", "forja/");
    let branch_wanted = format!("{prefix}{bench_name}");
    let path = bench_path(cx, &project, &bench_name);
    if path.exists() {
        return Err(format!(
            "the bench folder already exists: {}",
            path.display()
        ));
    }
    if shared
        .file()
        .benches
        .iter()
        .any(|bench| !bench.archived && git::same_path(Path::new(&bench.path), &path))
    {
        return Err("a bench already uses this folder".to_owned());
    }
    let bench_id = model::bench_id(project_id, &path, now_millis());
    let base_mode = base
        .filter(|value| !value.trim().is_empty())
        .map(str::to_owned)
        .unwrap_or_else(|| setting_str(cx, "bench.worktree.base_branch", "auto"));
    let fetch = fetch.unwrap_or_else(|| setting_bool(cx, "bench.worktree.fetch_before", true));
    let job = CreateJob {
        project,
        path: path.clone(),
        bench_id: bench_id.clone(),
        name: bench_name,
        branch_wanted: branch_wanted.clone(),
        base_mode,
        fetch,
        setup_policy: setting_str(cx, "bench.setup.policy", "ask"),
        timeout_s: setting_i64(cx, "bench.setup.timeout_s", 300),
        fallback_tabs: setting_list(cx, "bench.layout.tabs", &["terminal", "helper"]),
        approval: shared.approval(project_id),
        created_at: now_millis(),
        creation_stage: setting_str(cx, "bench.stage.creation_default", stages::DRAFT),
        from_request: from_request.map(str::to_owned),
    };
    let state = shared.clone();
    cx.spawn(
        move || run_create(job),
        move |cx, result| finish_create(cx, &state, result),
    );
    Ok(BenchRef {
        project_id: project_id.to_owned(),
        bench_id,
        name: path
            .file_name()
            .and_then(|name| name.to_str())
            .unwrap_or("bancada")
            .to_owned(),
        path: path.to_string_lossy().into_owned(),
        branch: Some(branch_wanted),
    })
}

struct CreateJob {
    project: ProjectRef,
    path: PathBuf,
    bench_id: String,
    name: String,
    branch_wanted: String,
    base_mode: String,
    fetch: bool,
    setup_policy: String,
    timeout_s: i64,
    fallback_tabs: Vec<String>,
    approval: ScriptApproval,
    created_at: u64,
    creation_stage: String,
    from_request: Option<String>,
}

struct Created {
    record: BenchRecord,
    setup: scripts::ScriptOutcome,
    setup_script: String,
    setup_hash: Option<String>,
    setup_skipped: Option<&'static str>,
    tabs: Vec<String>,
    warnings: Vec<String>,
}

fn run_create(job: CreateJob) -> Result<Created, String> {
    let project_path = Path::new(&job.project.path);
    git::ensure_repo(project_path)?;
    let mut warnings = Vec::new();
    if job.fetch {
        if let Err(error) = git::fetch(project_path) {
            warnings.push(error);
        }
    }
    let base = git::resolve_base(project_path, &job.base_mode)?;
    let branch = git::unique_branch(project_path, &job.branch_wanted);
    git::add_worktree(project_path, &job.path, &branch, &base.git_ref)?;
    let head = git::head(&job.path).ok();
    let loaded = config::load(&job.project.path);
    warnings.extend(loaded.warnings.clone());
    let mut setup = scripts::ScriptOutcome::default();
    let mut setup_skipped = None;
    if job.setup_policy == "never" {
        if !loaded.config.setup.is_empty() {
            setup_skipped = Some("policy");
        }
    } else if let Some(hash) = &loaded.setup_hash {
        if job.approval.setup_hash.as_deref() == Some(hash.as_str()) {
            setup = scripts::run(&loaded.config.setup, &job.path, job.timeout_s);
        } else {
            setup_skipped = Some("unapproved");
        }
    }
    let tabs = config::tabs(&loaded.config, &job.fallback_tabs);
    let request_command = job
        .from_request
        .as_deref()
        .and_then(|request| config::request_command(&loaded.config, request));
    let record = BenchRecord {
        bench_id: job.bench_id,
        project_id: job.project.id,
        name: job.name,
        path: job.path.to_string_lossy().into_owned(),
        branch: Some(branch),
        is_main: false,
        stage: job.creation_stage,
        archived: false,
        pinned: false,
        unread: true,
        sleeping: false,
        created_at: job.created_at,
        activated_at: 0,
        base_branch: Some(base.name),
        parent_bench_id: None,
        created_head: head,
        request_command,
        open_tabs: Vec::new(),
        request_key: job.from_request.clone(),
    };
    Ok(Created {
        record,
        setup,
        setup_script: loaded.config.setup,
        setup_hash: loaded.setup_hash,
        setup_skipped,
        tabs,
        warnings,
    })
}

fn finish_create(cx: &mut Ctx<'_>, shared: &Shared, result: Result<Created, String>) {
    let created = match result {
        Ok(created) => created,
        Err(error) => {
            shared.set_error(Some(error.clone()));
            notify(cx, cx.t("bench.notify.create-failed.title"), error, "error");
            return;
        }
    };
    let record = created.record.clone();
    if let Err(error) = shared.mutate(cx, |file| {
        if file.find(&record.bench_id).is_none() {
            file.benches.push(record.clone());
        }
    }) {
        shared.set_error(Some(error));
        return;
    }
    cx.emit(&BenchCreated(record.to_ref()));
    open_tabs(cx, &record, &created.tabs);
    if created.setup.ran {
        if created.setup.success {
            notify(
                cx,
                cx.t("bench.notify.setup-ok.title"),
                cx.t_args("bench.notify.setup-ok.body", &[("name", &record.name)]),
                "success",
            );
        } else {
            let output = if created.setup.timed_out {
                cx.t("bench.notify.setup-timeout")
            } else {
                created.setup.output.clone()
            };
            shared.set_notice(Some(output.clone()));
            notify(
                cx,
                cx.t("bench.notify.setup-failed.title"),
                cx.t_args(
                    "bench.notify.setup-failed.body",
                    &[("name", &record.name), ("message", &output)],
                ),
                "error",
            );
        }
    } else if let Some(reason) = created.setup_skipped {
        let key = if reason == "policy" {
            "bench.notify.setup-policy.body"
        } else {
            "bench.notify.setup-unapproved.body"
        };
        let body = cx.t_args(key, &[("name", &record.name)]);
        shared.set_notice(Some(body.clone()));
        notify(
            cx,
            cx.t("bench.notify.setup-skipped.title"),
            body,
            "attention",
        );
        if reason == "unapproved" {
            shared.set_dialog(Dialog::Approve(ApproveForm {
                project_id: record.project_id.clone(),
                setup: created.setup_script.clone(),
                teardown: String::new(),
                setup_hash: created.setup_hash.clone(),
                teardown_hash: None,
                error: None,
            }));
        }
    }
    if !created.warnings.is_empty() {
        shared.set_notice(Some(created.warnings.join("; ")));
    }
}

