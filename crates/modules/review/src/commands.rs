//! Review commands: refresh, create, checks, threads, merge and AI text.

use crate::ai;
use crate::api::{self, MergeOptions, Provider, PullRequestInput, Target};
use crate::client::Client;
use crate::fixtures;
use crate::parse;
use crate::state::{Comparison, Generated, Shared, Snapshot};
use forja_kernel::{Args, Ctx, KernelError, Result, SettingValue};
use serde_json::{Value, json};
use std::path::Path;
use std::process::{Command, Stdio};

pub fn refresh(cx: &mut Ctx<'_>, shared: &Shared, args: &Args) -> Result<Value> {
    let bench = if fixtures() {
        args.str("bench_path").unwrap_or(".").to_owned()
    } else {
        bench_path(cx, shared, args)?
    };
    let compare_base = shared
        .with(|state| (!state.base.trim().is_empty()).then(|| state.base.clone()))
        .flatten();
    start_comparison(cx, shared, &bench, compare_base.as_deref());
    let github = host(cx, Provider::GitHub);
    let gitlab = host(cx, Provider::GitLab);
    let network = cx.run_command("network.proxy", Args::new()).ok();
    let state = shared.clone();
    state.set_loading(true);
    cx.spawn(
        move || load(&bench, &github, &gitlab, network.as_ref()),
        move |_cx, result| match result {
            Ok(snapshot) => state.apply(snapshot),
            Err(error) if error.contains("no origin remote") => state.set_status(None),
            Err(error) => state.set_status(Some(error)),
        },
    );
    Ok(json!({ "pending": true }))
}

pub fn start_comparison(cx: &mut Ctx<'_>, shared: &Shared, bench: &str, base: Option<&str>) {
    let matching = shared
        .with(|state| {
            state
                .comparison
                .as_ref()
                .is_some_and(|comparison| base.map(str::trim) == comparison.base.as_deref())
        })
        .unwrap_or(false);
    if !matching {
        shared.set_comparison_loading(true);
    }
    let mut args = Args::new();
    args.insert("path", json!(bench));
    if let Some(base) = base {
        args.insert("base", json!(base));
    }
    if let Err(error) = cx.run_command("git.compare", args) {
        shared.set_comparison_error(Some(error.to_string()));
    }
}

pub fn comparison_changed(cx: &mut Ctx<'_>, shared: &Shared, bench: &str) {
    if shared.bench().as_deref() != Some(bench) {
        return;
    }
    let mut args = Args::new();
    args.insert("path", json!(bench));
    match cx.run_command("git.compare.result", args) {
        Ok(value) => {
            if let Some(error) = value.get("error").and_then(Value::as_str) {
                shared.set_comparison_error(Some(error.to_owned()));
            } else if let Some(value) = value.get("comparison").filter(|value| !value.is_null()) {
                match serde_json::from_value::<Comparison>(value.clone()) {
                    Ok(comparison) => shared.set_comparison(comparison),
                    Err(error) => shared.set_comparison_error(Some(error.to_string())),
                }
            } else if value
                .get("loading")
                .and_then(Value::as_bool)
                .unwrap_or(false)
            {
                shared.set_comparison_loading(true);
            }
        }
        Err(error) => shared.set_comparison_error(Some(error.to_string())),
    }
}

fn load(
    bench: &str,
    github: &str,
    gitlab: &str,
    network: Option<&Value>,
) -> std::result::Result<Snapshot, String> {
    if fixtures() {
        return Ok(fixtures::snapshot());
    }
    let branch = local_branch(bench)?;
    let base = local_base(bench, &branch);
    let target = api::detect(Path::new(bench), github, gitlab)?;
    let owner = target.repo.split('/').next().unwrap_or("");
    let client = Client::connect_with_policy(target.clone(), network)?;
    let list = client.value(api::find_for_branch(&target, owner, &branch))?;
    let mut pr = parse::pulls(&list, target.provider).into_iter().next();
    if let Some(found) = &pr {
        if let Ok(value) = client.value(api::detail(&target, found.number)) {
            if let Some(detail) = parse::pull(&value, target.provider) {
                pr = Some(detail);
            }
        }
    }
    let mut checks = Vec::new();
    let mut threads = Vec::new();
    if let Some(found) = &pr {
        if let Ok(value) = client.value(api::checks(&target, found.number, &found.source)) {
            checks = parse::checks(&value, target.provider);
            if target.provider == Provider::GitLab && !checks.is_empty() {
                if let Ok(jobs) = client.value(api::pipeline_jobs(&target, checks[0].id)) {
                    checks = parse::jobs(&jobs);
                }
            }
        }
        if let Ok(value) = client.value(api::threads(&target, found.number)) {
            threads = parse::threads(&value, target.provider);
        }
    }
    Ok(Snapshot {
        target,
        branch,
        base,
        pr,
        checks,
        threads,
    })
}

pub fn create(cx: &mut Ctx<'_>, shared: &Shared, args: &Args) -> Result<Value> {
    let bench = bench_path(cx, shared, args)?;
    let create_block = shared
        .with(|state| {
            if state.comparison_loading {
                return Some(cx.t("review.summary.loading"));
            }
            let Some(comparison) = &state.comparison else {
                return Some(cx.t("review.summary.choose-base"));
            };
            if comparison.base.as_deref() != Some(state.base.trim()) {
                return Some(cx.t("review.summary.choose-base"));
            }
            if comparison.base.is_none() {
                return Some(cx.t("review.summary.choose-base"));
            }
            if comparison.ahead == 0 {
                return Some(cx.t("review.summary.nothing-to-send"));
            }
            if !comparison.remote {
                return Some(cx.t("review.summary.remote-missing"));
            }
            None
        })
        .flatten();
    if let Some(reason) = create_block {
        return Err(KernelError::Message(reason));
    }
    let (title, body, base, draft, reviewers, labels) = shared
        .with(|state| {
            (
                args.str("title").unwrap_or(&state.title).to_owned(),
                args.str("body").unwrap_or(&state.body).to_owned(),
                args.str("base").unwrap_or(&state.base).to_owned(),
                args.bool("draft").unwrap_or(state.draft),
                args.str("reviewers").unwrap_or(&state.reviewers).to_owned(),
                args.str("labels").unwrap_or(&state.labels).to_owned(),
            )
        })
        .unwrap_or_default();
    if title.trim().is_empty() {
        return Err(KernelError::Message(
            "the pull request needs a title".to_owned(),
        ));
    }
    let branch = shared
        .with(|state| state.branch.clone())
        .unwrap_or_default();
    let branch = if branch.is_empty() {
        local_branch(&bench).map_err(KernelError::Message)?
    } else {
        branch
    };
    let github = host(cx, Provider::GitHub);
    let gitlab = host(cx, Provider::GitLab);
    let bench_id = current_bench_id(cx);
    let stage_mode = setting_str(cx, "bench.stage.pr_open", "always");
    let network = cx.run_command("network.proxy", Args::new()).ok();
    let state = shared.clone();
    state.set_loading(true);
    cx.spawn(
        move || {
            let target = api::detect(Path::new(&bench), &github, &gitlab)?;
            let client = Client::connect_with_policy(target.clone(), network.as_ref())?;
            let input = PullRequestInput {
                title,
                body,
                source: branch,
                target: base,
                draft,
            };
            let value = client.value(api::create(&target, &input))?;
            let pr = parse::pull(&value, target.provider)
                .ok_or_else(|| "the review provider did not return a pull request".to_owned())?;
            if !reviewers.trim().is_empty() {
                let names: Vec<String> = reviewers
                    .split(',')
                    .map(str::trim)
                    .filter(|name| !name.is_empty())
                    .map(str::to_owned)
                    .collect();
                let _ = client.send(api::request_reviewers(&target, pr.number, &names));
            }
            if !labels.trim().is_empty() {
                let names: Vec<String> = labels
                    .split(',')
                    .map(str::trim)
                    .filter(|name| !name.is_empty())
                    .map(str::to_owned)
                    .collect();
                let _ = client.send(api::add_labels(&target, pr.number, &names));
            }
            Ok((pr, target))
        },
        move |cx, result: std::result::Result<(parse::PullRequest, Target), String>| match result {
            Ok((pr, _target)) => {
                let number = pr.number;
                state.set_pr(pr);
                state.set_notice(Some(cx.t("review.notice.pr-created")));
                if stage_mode != "never" {
                    if let Some(bench_id) = &bench_id {
                        let mut args = Args::new();
                        args.insert("bench_id", json!(bench_id));
                        args.insert("stage", json!("review"));
                        args.insert("reason", json!("Pull request opened"));
                        let _ = cx.run_command("bench.stage.set", args);
                    }
                }
                let _ = number;
            }
            Err(error) => state.set_status(Some(error)),
        },
    );
    Ok(json!({ "pending": true }))
}

pub fn merge(cx: &mut Ctx<'_>, shared: &Shared, args: &Args) -> Result<Value> {
    let Some(pr) = shared.with(|state| state.pr.clone()).flatten() else {
        return Err(KernelError::Message("open a pull request first".to_owned()));
    };
    let Some(target) = shared.with(|state| state.target.clone()).flatten() else {
        return Err(KernelError::Message(
            "the review provider is unknown".to_owned(),
        ));
    };
    let method = args
        .str("method")
        .map(str::to_owned)
        .or_else(|| {
            shared
                .with(|state| Some(method_name(state.merge_method)))
                .flatten()
        })
        .unwrap_or_else(|| "merge".to_owned());
    let options = MergeOptions {
        method,
        delete_branch: args.bool("delete_branch").unwrap_or(false),
        auto_merge: args.bool("auto_merge").unwrap_or(false),
    };
    let bench_id = current_bench_id(cx);
    let stage_mode = setting_str(cx, "bench.stage.pr_merge", "always");
    let network = cx.run_command("network.proxy", Args::new()).ok();
    let state = shared.clone();
    state.set_loading(true);
    cx.spawn(
        move || {
            let client = Client::connect_with_policy(target.clone(), network.as_ref())?;
            client.send(api::merge(&target, pr.number, &options))?;
            Ok(())
        },
        move |cx, result: std::result::Result<(), String>| match result {
            Ok(()) => {
                state.mark_merged();
                state.set_notice(Some(cx.t("review.notice.merged")));
                if stage_mode != "never" {
                    if let Some(bench_id) = &bench_id {
                        let mut args = Args::new();
                        args.insert("bench_id", json!(bench_id));
                        args.insert("stage", json!("ready"));
                        args.insert("reason", json!("Pull request merged"));
                        let _ = cx.run_command("bench.stage.set", args);
                    }
                }
            }
            Err(error) => state.set_status(Some(error)),
        },
    );
    Ok(json!({ "pending": true }))
}

pub fn reply(cx: &mut Ctx<'_>, shared: &Shared, args: &Args) -> Result<Value> {
    let thread = args.required_str("thread_id")?.to_owned();
    let body = args.required_str("body")?.to_owned();
    let Some(pr) = shared.with(|state| state.pr.clone()).flatten() else {
        return Err(KernelError::Message("open a pull request first".to_owned()));
    };
    let Some(target) = shared.with(|state| state.target.clone()).flatten() else {
        return Err(KernelError::Message(
            "the review provider is unknown".to_owned(),
        ));
    };
    let network = cx.run_command("network.proxy", Args::new()).ok();
    let state = shared.clone();
    state.set_loading(true);
    let thread_done = thread.clone();
    let body_done = body.clone();
    cx.spawn(
        move || {
            let client = Client::connect_with_policy(target.clone(), network.as_ref())?;
            client.send(api::reply_thread(&target, pr.number, &thread, &body))?;
            Ok(())
        },
        move |_cx, result: std::result::Result<(), String>| match result {
            Ok(()) => {
                state.set_threads(
                    state
                        .with(|state| state.threads.clone())
                        .unwrap_or_default()
                        .into_iter()
                        .map(|mut item| {
                            if item.id == thread_done {
                                item.comments.push(parse::Comment {
                                    id: String::new(),
                                    author: "you".into(),
                                    body: body_done.clone(),
                                });
                            }
                            item
                        })
                        .collect(),
                );
                state.clear_reply(&thread_done);
                state.set_notice(Some("reply sent".to_owned()));
            }
            Err(error) => state.set_status(Some(error)),
        },
    );
    Ok(json!({ "pending": true }))
}

pub fn resolve(cx: &mut Ctx<'_>, shared: &Shared, args: &Args) -> Result<Value> {
    let thread = args.required_str("thread_id")?.to_owned();
    let resolved = args.bool("resolved").unwrap_or(true);
    let Some(pr) = shared.with(|state| state.pr.clone()).flatten() else {
        return Err(KernelError::Message("open a pull request first".to_owned()));
    };
    let Some(target) = shared.with(|state| state.target.clone()).flatten() else {
        return Err(KernelError::Message(
            "the review provider is unknown".to_owned(),
        ));
    };
    let network = cx.run_command("network.proxy", Args::new()).ok();
    let state = shared.clone();
    state.set_loading(true);
    let thread_done = thread.clone();
    cx.spawn(
        move || {
            let client = Client::connect_with_policy(target.clone(), network.as_ref())?;
            client.send(api::resolve_thread(&target, pr.number, &thread, resolved))?;
            Ok(())
        },
        move |_cx, result: std::result::Result<(), String>| match result {
            Ok(()) => {
                state.set_threads(
                    state
                        .with(|state| state.threads.clone())
                        .unwrap_or_default()
                        .into_iter()
                        .map(|mut item| {
                            if item.id == thread_done {
                                item.resolved = resolved;
                            }
                            item
                        })
                        .collect(),
                );
            }
            Err(error) => state.set_status(Some(error)),
        },
    );
    Ok(json!({ "pending": true }))
}

pub fn rerun(cx: &mut Ctx<'_>, shared: &Shared, args: &Args) -> Result<Value> {
    let id = args.i64("id").unwrap_or(0) as u64;
    let Some(target) = shared.with(|state| state.target.clone()).flatten() else {
        return Err(KernelError::Message(
            "the review provider is unknown".to_owned(),
        ));
    };
    let network = cx.run_command("network.proxy", Args::new()).ok();
    let state = shared.clone();
    state.set_loading(true);
    cx.spawn(
        move || {
            let client = Client::connect_with_policy(target.clone(), network.as_ref())?;
            client.send(api::rerun_check(&target, id))?;
            Ok(())
        },
        move |cx, result: std::result::Result<(), String>| match result {
            Ok(()) => state.set_notice(Some(cx.t("review.notice.rerun"))),
            Err(error) => state.set_status(Some(error)),
        },
    );
    Ok(json!({ "pending": true }))
}

pub fn log(cx: &mut Ctx<'_>, shared: &Shared, args: &Args) -> Result<Value> {
    let id = args.i64("id").unwrap_or(0) as u64;
    let Some(check) = shared
        .with(|state| state.checks.iter().find(|check| check.id == id).cloned())
        .flatten()
    else {
        return Ok(json!({}));
    };
    if !check.log.is_empty() || !check.pipeline {
        return Ok(json!({ "log": check.log }));
    }
    let Some(target) = shared.with(|state| state.target.clone()).flatten() else {
        return Ok(json!({}));
    };
    let network = cx.run_command("network.proxy", Args::new()).ok();
    let state = shared.clone();
    cx.spawn(
        move || {
            let client = Client::connect_with_policy(target.clone(), network.as_ref())?;
            client.text(api::job_log(&target, id))
        },
        move |_cx, result: std::result::Result<String, String>| {
            if let Ok(text) = result {
                state.add_log(id.to_string(), text);
            }
        },
    );
    Ok(json!({ "pending": true }))
}

pub fn generate(cx: &mut Ctx<'_>, shared: &Shared, args: &Args) -> Result<Value> {
    let kind = args
        .str("kind")
        .and_then(ai::Kind::parse)
        .ok_or_else(|| KernelError::Message("kind must be commit, pr or branch".to_owned()))?;
    let bench = bench_path(cx, shared, args)?;
    let profile_id = default_profile(cx);
    let base = shared.with(|state| state.base.clone()).unwrap_or_default();
    let state = shared.clone();
    state.set_loading(true);
    cx.spawn(
        move || generate_blocking(&bench, kind, &profile_id, &base),
        move |cx, result| match result {
            Ok(generated) => {
                if generated.kind == "pr" {
                    if let (Some(title), Some(body)) = (&generated.title, &generated.body) {
                        state.with(|state| {
                            state.title = title.clone();
                            state.body = body.clone();
                        });
                    }
                }
                state.set_generated(Some(generated));
                state.set_notice(Some(cx.t("review.notice.generated")));
            }
            Err(error) => state.set_status(Some(error)),
        },
    );
    Ok(json!({ "pending": true }))
}

fn generate_blocking(
    bench: &str,
    kind: ai::Kind,
    profile_id: &str,
    base: &str,
) -> std::result::Result<Generated, String> {
    let diff = match kind {
        ai::Kind::Commit => git_text(bench, &["diff", "--cached", "--no-color"])
            .ok()
            .filter(|text| !text.trim().is_empty())
            .or_else(|| git_text(bench, &["diff", "HEAD", "--no-color"]).ok())
            .unwrap_or_default(),
        _ => {
            let base = if base.trim().is_empty() {
                "main"
            } else {
                base.trim()
            };
            git_text(bench, &["diff", &format!("{base}...HEAD"), "--no-color"]).unwrap_or_default()
        }
    };
    if diff.trim().is_empty() {
        return Err("there are no changes to describe".to_owned());
    }
    let commits = git_text(bench, &["log", "--oneline", "--max-count=30"]).unwrap_or_default();
    let profile = ai::profile_for(Path::new(bench), Some(profile_id))
        .ok_or_else(|| format!("helper profile `{profile_id}` has no headless mode"))?;
    let prompt = ai::prompt(kind, &diff, &commits, bench);
    let text = ai::run(&profile, &prompt, Path::new(bench))?;
    Ok(match kind {
        ai::Kind::PullRequest => {
            let parsed = ai::parse_pr(&text);
            Generated {
                kind: kind.id().into(),
                text,
                title: Some(parsed.title),
                body: Some(parsed.body),
            }
        }
        ai::Kind::Branch => Generated {
            kind: kind.id().into(),
            text: ai::branch_slug(&text),
            title: None,
            body: None,
        },
        ai::Kind::Commit => Generated {
            kind: kind.id().into(),
            text,
            title: None,
            body: None,
        },
    })
}

pub fn fix(cx: &mut Ctx<'_>, shared: &Shared, args: &Args) -> Result<Value> {
    let bench = bench_path(cx, shared, args)?;
    let log = shared
        .with(|state| {
            state
                .checks
                .iter()
                .find(|check| check.failed() && !check.log.is_empty())
                .map(|check| (check.name.clone(), check.log.clone()))
        })
        .flatten();
    let Some((name, log)) = log else {
        return Err(KernelError::Message(
            "there is no failed check with a log".to_owned(),
        ));
    };
    let trimmed: String = log.chars().take(8000).collect();
    let prompt = format!(
        "Fix the failing check `{name}` in this bench. Here is the end of the log:\n\n{trimmed}"
    );
    let mut args = Args::new();
    args.insert("bench_path", json!(bench));
    args.insert("prompt", json!(prompt));
    match cx.run_command("helper.launch", args) {
        Ok(value) => Ok(value),
        Err(KernelError::UnknownCommand(_)) => Err(KernelError::Message(
            "the helper module is not available".to_owned(),
        )),
        Err(error) => Err(error),
    }
}

pub fn default_profile(cx: &Ctx<'_>) -> String {
    cx.setting("helper.default.profile")
        .and_then(SettingValue::as_str)
        .filter(|id| *id != "auto")
        .unwrap_or("claude-code")
        .to_owned()
}

pub fn bench_path(cx: &mut Ctx<'_>, shared: &Shared, args: &Args) -> Result<String> {
    if let Some(path) = args.str("bench_path") {
        shared.set_bench(Some(path.to_owned()));
        return Ok(path.to_owned());
    }
    if let Some(path) = shared.bench() {
        return Ok(path);
    }
    let value = cx
        .run_command("bench.current", Args::new())
        .map_err(|error| KernelError::Message(error.to_string()))?;
    let path = value
        .get("path")
        .and_then(Value::as_str)
        .ok_or_else(|| KernelError::Message(cx.t("review.error.no-bench")))?
        .to_owned();
    shared.set_bench(Some(path.clone()));
    Ok(path)
}

fn current_bench_id(cx: &mut Ctx<'_>) -> Option<String> {
    cx.run_command("bench.current", Args::new())
        .ok()
        .and_then(|value| {
            value
                .get("bench_id")
                .and_then(Value::as_str)
                .map(str::to_owned)
        })
}

fn host(cx: &Ctx<'_>, provider: Provider) -> String {
    let key = match provider {
        Provider::GitHub => "counter.github.host",
        Provider::GitLab => "counter.gitlab.host",
    };
    cx.setting(key)
        .and_then(SettingValue::as_str)
        .unwrap_or(match provider {
            Provider::GitHub => "github.com",
            Provider::GitLab => "gitlab.com",
        })
        .to_owned()
}

fn setting_str(cx: &Ctx<'_>, key: &str, fallback: &str) -> String {
    cx.setting(key)
        .and_then(SettingValue::as_str)
        .unwrap_or(fallback)
        .to_owned()
}

fn method_name(index: usize) -> String {
    match index {
        1 => "squash".to_owned(),
        2 => "rebase".to_owned(),
        _ => "merge".to_owned(),
    }
}

pub fn fixtures() -> bool {
    std::env::var("FORJA_REVIEW_FIXTURE")
        .map(|value| value == "1")
        .unwrap_or(false)
        || forja_counter_kit::fixtures_enabled()
}

fn local_branch(bench: &str) -> std::result::Result<String, String> {
    git_text(bench, &["rev-parse", "--abbrev-ref", "HEAD"])
}

fn local_base(bench: &str, branch: &str) -> String {
    if let Ok(value) = git_text(
        bench,
        &["symbolic-ref", "--short", "refs/remotes/origin/HEAD"],
    ) {
        if let Some(base) = value.trim().strip_prefix("origin/") {
            return base.to_owned();
        }
    }
    if branch != "main" && git_text(bench, &["rev-parse", "--verify", "main"]).is_ok() {
        return "main".to_owned();
    }
    "main".to_owned()
}

fn git_text(bench: &str, args: &[&str]) -> std::result::Result<String, String> {
    let output = Command::new("git")
        .arg("-C")
        .arg(bench)
        .args(args)
        .env("GIT_TERMINAL_PROMPT", "0")
        .stdin(Stdio::null())
        .stderr(Stdio::null())
        .output()
        .map_err(|error| format!("git is not available: {error}"))?;
    if !output.status.success() {
        return Err("git command failed".to_owned());
    }
    Ok(String::from_utf8_lossy(&output.stdout).trim().to_owned())
}

/// Converts raw review, agent, and git errors into human-facing summaries.
pub(crate) fn humanize_error(cx: &Ctx<'_>, raw: &str) -> forja_kernel::HumanError {
    let trimmed = raw.trim();
    match trimmed {
        "open a pull request first" => {
            forja_kernel::HumanError::simple(cx.t("review.error.no-pr"))
        }
        "there are no changes to describe" => {
            forja_kernel::HumanError::simple(cx.t("review.error.no-changes"))
        }
        "there is no failed check with a log" => {
            forja_kernel::HumanError::simple(cx.t("review.error.no-check-log"))
        }
        "GitHub origin needs owner/repository" => {
            forja_kernel::HumanError::simple(cx.t("review.error.bad-origin"))
        }
        "the agent failed without an error message"
        | "the agent returned an empty response" => {
            forja_kernel::HumanError::with_detail(cx.t("review.error.agent"), trimmed)
        }
        _ if trimmed.starts_with("the agent failed:") => {
            forja_kernel::HumanError::with_detail(cx.t("review.error.agent"), trimmed)
        }
        _ => forja_kernel::human_error::humanize_ctx(cx, trimmed),
    }
}
