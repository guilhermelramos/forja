use crate::comparison;
use crate::diff;
use crate::engine;
use crate::helpers::{
    files_arg, kernel_error, path_arg, refresh_now, setting_list, setting_str, spawn_op,
    spawn_op_with_success, truthy, unit,
};
use crate::log;
use crate::state::Shared;
use forja_contracts::{GitComparisonChanged, GitStatusChanged};
use forja_kernel::Registrar;
use serde_json::json;
pub(crate) fn register_commands(r: &mut Registrar<'_>, shared: &Shared) {
    let s = shared.clone();
    r.command("git.status", "git.command.status", move |cx, args| {
        let path = path_arg(&args)?;
        let report = engine::status(std::path::Path::new(path)).map_err(kernel_error)?;
        if s.path().as_deref() == Some(path) {
            s.set_report(report.clone());
        }
        cx.emit(&GitStatusChanged {
            bench_path: path.to_owned(),
            summary: report.summary.clone(),
        });
        Ok(serde_json::to_value(report.summary)?)
    });

    let s = shared.clone();
    r.command("git.changes", "git.command.changes", move |_cx, args| {
        let path = path_arg(&args)?;
        let report = engine::status(std::path::Path::new(path)).map_err(kernel_error)?;
        let _ = &s;
        Ok(serde_json::to_value(report)?)
    });

    let s = shared.clone();
    r.command("git.refresh", "git.command.refresh", move |cx, args| {
        let path = path_arg(&args)?.to_owned();
        refresh_now(cx, &s, path)
    });

    r.command("git.diff", "git.command.diff", move |_cx, args| {
        let path = path_arg(&args)?;
        let file = args.required_str("file")?;
        let staged = args.bool("staged").unwrap_or(false);
        let diff = match args.str("comparison") {
            Some(range) => comparison::diff_file(std::path::Path::new(path), file, range),
            None => diff::diff(std::path::Path::new(path), file, staged),
        }
        .map_err(kernel_error)?;
        Ok(serde_json::to_value(diff)?)
    });

    let compare_state = shared.clone();
    r.command("git.compare", "git.command.compare", move |cx, args| {
        let path = path_arg(&args)?.to_owned();
        let base = args
            .str("base")
            .map(str::trim)
            .filter(|base| !base.is_empty())
            .map(str::to_owned);
        let previous = compare_state
            .comparison(&path)
            .and_then(|(_, result)| result)
            .and_then(|result| result.ok());
        let Some(generation) = compare_state.begin_comparison(path.clone(), base.clone()) else {
            return Ok(json!({ "pending": true }));
        };
        let work_path = path.clone();
        let finish_state = compare_state.clone();
        cx.spawn(
            move || {
                if let Some(report) = previous.filter(|report| {
                    comparison::is_current(
                        std::path::Path::new(&work_path),
                        base.as_deref(),
                        report,
                    )
                }) {
                    Ok((report, false))
                } else {
                    comparison::compare(std::path::Path::new(&work_path), base.as_deref())
                        .map(|report| (report, true))
                }
            },
            move |cx, result| {
                let (result, changed) = match result {
                    Ok((report, changed)) => (Ok(report), changed),
                    Err(error) => (Err(error), true),
                };
                let base = result.as_ref().ok().and_then(|report| report.base.clone());
                if let Some(bench_path) = finish_state.finish_comparison(generation, result) {
                    if !changed {
                        return;
                    }
                    cx.emit(&GitComparisonChanged { bench_path, base });
                }
            },
        );
        Ok(json!({ "pending": true }))
    });

    let result_state = shared.clone();
    r.command(
        "git.compare.result",
        "git.command.compare-result",
        move |_cx, args| {
            let path = path_arg(&args)?;
            let Some((loading, result)) = result_state.comparison(path) else {
                return Ok(json!({ "loading": false, "comparison": null, "error": null }));
            };
            match result {
                Some(Ok(report)) => Ok(json!({
                    "loading": loading,
                    "comparison": report,
                    "error": null
                })),
                Some(Err(error)) => Ok(json!({
                    "loading": loading,
                    "comparison": null,
                    "error": error
                })),
                None => Ok(json!({ "loading": loading, "comparison": null, "error": null })),
            }
        },
    );

    let s = shared.clone();
    r.command("git.stage", "git.command.stage", move |cx, args| {
        let (path, files) = files_arg(&args)?;
        spawn_op(cx, &s, move || {
            unit(engine::stage(std::path::Path::new(&path), &files))
        });
        Ok(json!({ "pending": true }))
    });

    let s = shared.clone();
    r.command("git.stage_all", "git.command.stage_all", move |cx, args| {
        let path = path_arg(&args)?.to_owned();
        spawn_op(cx, &s, move || {
            unit(engine::stage_all(std::path::Path::new(&path)))
        });
        Ok(json!({ "pending": true }))
    });

    let s = shared.clone();
    r.command("git.unstage", "git.command.unstage", move |cx, args| {
        let (path, files) = files_arg(&args)?;
        spawn_op(cx, &s, move || {
            unit(engine::unstage(std::path::Path::new(&path), &files))
        });
        Ok(json!({ "pending": true }))
    });

    let s = shared.clone();
    r.command(
        "git.unstage_all",
        "git.command.unstage_all",
        move |cx, args| {
            let path = path_arg(&args)?.to_owned();
            spawn_op(cx, &s, move || {
                unit(engine::unstage_all(std::path::Path::new(&path)))
            });
            Ok(json!({ "pending": true }))
        },
    );

    let s = shared.clone();
    r.command("git.discard", "git.command.discard", move |cx, args| {
        if !args.bool("confirm").unwrap_or(false) {
            return Err(kernel_error(
                "discard needs explicit confirmation".to_owned(),
            ));
        }
        let (path, files) = files_arg(&args)?;
        spawn_op(cx, &s, move || {
            unit(engine::discard(std::path::Path::new(&path), &files))
        });
        Ok(json!({ "pending": true }))
    });

    let s = shared.clone();
    r.command("git.commit", "git.command.commit", move |cx, args| {
        let path = path_arg(&args)?.to_owned();
        let message = args.required_str("message")?.to_owned();
        let all = args.bool("all").unwrap_or(false);
        let sign = truthy(cx, "git.commit.sign");
        let sign_off = args
            .bool("sign_off")
            .unwrap_or_else(|| truthy(cx, "git.commit.sign_off"));
        let gpg_key = setting_str(cx, "git.commit.gpg_key");
        let name = setting_str(cx, "git.identity.name");
        let email = setting_str(cx, "git.identity.email");
        let protected = setting_list(cx, "git.protected.branches", &["main", "master"]);
        let clear_message = args.bool("clear_message").unwrap_or(false);
        let submitted_message = message.clone();
        let work = move || {
            let path = std::path::Path::new(&path);
            if let Some(branch) = engine::protected_branch(path, &protected)? {
                return Err(format!("protected:{branch}"));
            }
            engine::commit(
                path,
                &engine::CommitOptions {
                    message: &message,
                    all,
                    sign,
                    gpg_key: &gpg_key,
                    sign_off,
                    name: &name,
                    email: &email,
                },
            )
        };
        if clear_message {
            let state = s.clone();
            spawn_op_with_success(cx, &s, work, move || {
                if state.commit_message() == submitted_message {
                    state.set_commit_message(String::new());
                }
            });
        } else {
            spawn_op(cx, &s, work);
        }
        Ok(json!({ "pending": true }))
    });

    let s = shared.clone();
    r.command("git.push", "git.command.push", move |cx, args| {
        let path = path_arg(&args)?.to_owned();
        let force = args.bool("force_with_lease").unwrap_or(false);
        let publish = args.bool("publish").unwrap_or(false);
        let set_upstream = truthy(cx, "git.push.auto_set_upstream");
        spawn_op(cx, &s, move || {
            engine::push(std::path::Path::new(&path), force, publish, set_upstream)
        });
        Ok(json!({ "pending": true }))
    });

    let s = shared.clone();
    r.command("git.pull", "git.command.pull", move |cx, args| {
        let path = path_arg(&args)?.to_owned();
        spawn_op(cx, &s, move || engine::pull(std::path::Path::new(&path)));
        Ok(json!({ "pending": true }))
    });

    let s = shared.clone();
    r.command("git.fetch", "git.command.fetch", move |cx, args| {
        let path = path_arg(&args)?.to_owned();
        spawn_op(cx, &s, move || engine::fetch(std::path::Path::new(&path)));
        Ok(json!({ "pending": true }))
    });

    let s = shared.clone();
    r.command("git.sync", "git.command.sync", move |cx, args| {
        let path = path_arg(&args)?.to_owned();
        let set_upstream = truthy(cx, "git.push.auto_set_upstream");
        spawn_op(cx, &s, move || {
            engine::sync(std::path::Path::new(&path), set_upstream)
        });
        Ok(json!({ "pending": true }))
    });

    let s = shared.clone();
    r.command("git.publish", "git.command.publish", move |cx, args| {
        let path = path_arg(&args)?.to_owned();
        spawn_op(cx, &s, move || engine::publish(std::path::Path::new(&path)));
        Ok(json!({ "pending": true }))
    });

    let s = shared.clone();
    r.command("git.rebase", "git.command.rebase", move |cx, args| {
        let path = path_arg(&args)?.to_owned();
        let base = args.str("base").map(str::to_owned);
        let protected = setting_list(cx, "git.protected.branches", &["main", "master"]);
        spawn_op(cx, &s, move || {
            let path = std::path::Path::new(&path);
            if let Some(branch) = engine::protected_branch(path, &protected)? {
                return Err(format!("protected:{branch}"));
            }
            engine::rebase(path, base.as_deref())
        });
        Ok(json!({ "pending": true }))
    });

    let s = shared.clone();
    r.command("git.abort", "git.command.abort", move |cx, args| {
        let path = path_arg(&args)?.to_owned();
        spawn_op(cx, &s, move || engine::abort(std::path::Path::new(&path)));
        Ok(json!({ "pending": true }))
    });

    let s = shared.clone();
    r.command(
        "git.conflicts",
        "git.command.conflicts",
        move |_cx, args| {
            let path = path_arg(&args)?;
            let files = engine::conflicts(std::path::Path::new(path)).map_err(kernel_error)?;
            let _ = &s;
            Ok(json!({ "files": files }))
        },
    );

    let _ = shared;
    let log_shared = r.module_id().to_owned();
    let _ = log_shared;
    r.command("git.log", "git.command.log", move |_cx, args| {
        let path = path_arg(&args)?;
        let limit = args.i64("limit").unwrap_or(200).clamp(1, 2000) as u32;
        let lines = log::log(std::path::Path::new(path), limit).map_err(kernel_error)?;
        Ok(json!({ "lines": lines }))
    });

    r.command("git.blame", "git.command.blame", move |_cx, args| {
        let path = path_arg(&args)?;
        let file = args.required_str("file")?;
        let lines = log::blame(std::path::Path::new(path), file).map_err(kernel_error)?;
        Ok(json!({ "lines": lines }))
    });
}
