use crate::service;
use crate::state::Shared;
use forja_kernel::{CommandField, CommandFieldKind, Registrar};
use serde_json::{json, Value};
use crate::kernel_error;

/// The Anvil asks for a bench from `bench.list`; the same mechanism serves
/// every command that takes a `bench_id`.
fn bench_field() -> CommandField {
    CommandField {
        arg: "bench_id",
        label_key: "bench.ask.bench",
        kind: CommandFieldKind::Choice {
            command: "bench.list",
            list: "benches",
            label: "name",
            value: "bench_id",
            detail: Some("branch"),
        },
    }
}

/// Projects come from `project.list`, so the user never has to type an id.
fn project_field() -> CommandField {
    CommandField {
        arg: "project_id",
        label_key: "bench.ask.project",
        kind: CommandFieldKind::Choice {
            command: "project.list",
            list: "projects",
            label: "name",
            value: "id",
            detail: Some("path"),
        },
    }
}

/// Stages come from `bench.stage.list`, including project extras.
fn stage_field() -> CommandField {
    CommandField {
        arg: "stage",
        label_key: "bench.ask.stage",
        kind: CommandFieldKind::Choice {
            command: "bench.stage.list",
            list: "stages",
            label: "label",
            value: "id",
            detail: None,
        },
    }
}

fn name_field() -> CommandField {
    CommandField {
        arg: "name",
        label_key: "bench.ask.name",
        kind: CommandFieldKind::Text,
    }
}

pub(crate) fn register_commands(r: &mut Registrar<'_>, shared: &Shared) {
    let s = shared.clone();
    r.command("bench.current", "bench.command.current", move |cx, _| {
        s.ensure_loaded(cx);
        service::ensure_main(cx, &s).map_err(kernel_error)?;
        Ok(serde_json::to_value(service::current(&s))?)
    });

    let s = shared.clone();
    r.command("bench.list", "bench.command.list", move |cx, args| {
        let project_id = args.str("project_id");
        let include_archived = args.bool("include_archived").unwrap_or(false);
        let benches = service::refs(cx, &s, project_id, include_archived).map_err(kernel_error)?;
        Ok(json!({ "benches": benches }))
    });

    let s = shared.clone();
    r.command(
        "bench.list_cards",
        "bench.command.list_cards",
        move |cx, args| {
            let project_id = args.str("project_id");
            let include_archived = args.bool("include_archived").unwrap_or(true);
            let benches =
                service::cards(cx, &s, project_id, include_archived).map_err(kernel_error)?;
            Ok(json!({ "benches": benches }))
        },
    );

    let s = shared.clone();
    r.command_asking(
        "bench.activate",
        "bench.command.activate",
        vec![bench_field()],
        move |cx, args| {
            let id = args.required_str("bench_id")?;
            let bench = service::activate(cx, &s, id).map_err(kernel_error)?;
            Ok(serde_json::to_value(bench)?)
        },
    );

    let s = shared.clone();
    r.command_asking(
        "bench.create",
        "bench.command.create",
        vec![project_field(), name_field()],
        move |cx, args| {
            let project_id = args.required_str("project_id")?;
            let name = args.required_str("name")?;
            let from_request = args.str("from_request").or_else(|| {
                args.get("from_request")
                    .and_then(|value| value.get("key"))
                    .and_then(Value::as_str)
            });
            let bench = crate::create::create(
                cx,
                &s,
                project_id,
                name,
                args.str("base"),
                from_request,
                args.bool("fetch"),
            )
            .map_err(kernel_error)?;
            let mut value = serde_json::to_value(bench)?;
            if let Some(map) = value.as_object_mut() {
                map.insert("pending".to_owned(), json!(true));
            }
            Ok(value)
        },
    );

    let s = shared.clone();
    r.command(
        "bench.preview_forge",
        "bench.command.preview_forge",
        move |cx, args| {
            let preview_id = args.required_str("preview_id")?.to_owned();
            let project_id = args.required_str("project_id")?.to_owned();
            let name = args.required_str("name")?.to_owned();
            let from_request = args
                .str("from_request")
                .map(str::to_owned)
                .or_else(|| {
                    args.get("from_request")
                        .and_then(|value| value.get("key"))
                        .and_then(Value::as_str)
                        .map(str::to_owned)
                })
                .ok_or_else(|| forja_kernel::KernelError::message("from_request is required"))?;

            let projects = service::projects(cx);
            let project = projects
                .iter()
                .find(|project| project.id == project_id)
                .cloned()
                .ok_or_else(|| forja_kernel::KernelError::message("project is not registered"))?;
            let bench_root = service::bench_root(cx, &project);
            let job = crate::preview::PreviewJob {
                project,
                project_ids: projects.into_iter().map(|project| project.id).collect(),
                request_key: from_request,
                request_name: name,
                name_template: service::setting_str(
                    cx,
                    "bench.worktree.name_template",
                    "{pedido}-{slug}",
                ),
                branch_prefix: service::setting_str(
                    cx,
                    "bench.worktree.branch_prefix",
                    "forja/",
                ),
                bench_root,
                store_path: cx.store().path(),
                base_mode: args
                    .str("base")
                    .filter(|base| !base.trim().is_empty())
                    .map(str::to_owned)
                    .unwrap_or_else(|| {
                        service::setting_str(cx, "bench.worktree.base_branch", "auto")
                    }),
                setup_policy: service::setting_str(cx, "bench.setup.policy", "ask"),
                fetch_before: service::setting_bool(cx, "bench.worktree.fetch_before", true),
                helper: args.bool("helper").unwrap_or(false),
            };
            s.start_preview(preview_id.clone());
            let state = s.clone();
            let callback_id = preview_id.clone();
            cx.spawn(
                move || crate::preview::build(job),
                move |_cx, result| state.finish_preview(&callback_id, result),
            );
            Ok(json!({ "started": true, "preview_id": preview_id }))
        },
    );

    let s = shared.clone();
    r.command(
        "bench.preview_status",
        "bench.command.preview_status",
        move |_cx, args| {
            let preview_id = args.required_str("preview_id")?;
            Ok(match s.preview(preview_id) {
                None => json!({ "state": "missing" }),
                Some(None) => json!({ "state": "pending" }),
                Some(Some(Ok(preview))) => json!({ "state": "ready", "preview": preview }),
                Some(Some(Err(error))) => json!({ "state": "error", "error": error }),
            })
        },
    );

    let s = shared.clone();
    r.command(
        "bench.preview_cancel",
        "bench.command.preview_cancel",
        move |_cx, args| {
            let preview_id = args.required_str("preview_id")?;
            s.cancel_preview(preview_id);
            Ok(json!({ "cancelled": true }))
        },
    );

    let s = shared.clone();
    r.command_asking(
        "bench.store",
        "bench.command.store",
        vec![bench_field()],
        move |cx, args| {
            let id = args.required_str("bench_id")?;
            crate::store::store(cx, &s, id, args.bool("delete_branch"), args.bool("force"))
                .map_err(kernel_error)?;
            Ok(json!({ "pending": true }))
        },
    );

    let s = shared.clone();
    r.command_asking(
        "bench.stage.set",
        "bench.command.stage_set",
        vec![bench_field(), stage_field()],
        move |cx, args| {
            let id = args.required_str("bench_id")?;
            let stage = args.required_str("stage")?;
            let card = service::set_stage(
                cx,
                &s,
                id,
                stage,
                args.str("reason"),
                args.bool("confirm_backward").unwrap_or(false),
            )
            .map_err(kernel_error)?;
            Ok(serde_json::to_value(card)?)
        },
    );

    let s = shared.clone();
    r.command(
        "bench.stage.list",
        "bench.command.stage_list",
        move |cx, args| service::stage_list(cx, &s, args.str("project_id")).map_err(kernel_error),
    );

    let s = shared.clone();
    r.command_asking(
        "bench.sleep",
        "bench.command.sleep",
        vec![bench_field()],
        move |cx, args| {
            let id = args.required_str("bench_id")?;
            let card = service::sleep(cx, &s, id).map_err(kernel_error)?;
            Ok(serde_json::to_value(card)?)
        },
    );

    let s = shared.clone();
    r.command_asking(
        "bench.wake",
        "bench.command.wake",
        vec![bench_field()],
        move |cx, args| {
            let id = args.required_str("bench_id")?;
            let card = service::wake(cx, &s, id).map_err(kernel_error)?;
            Ok(serde_json::to_value(card)?)
        },
    );

    let s = shared.clone();
    r.command_asking(
        "bench.pin",
        "bench.command.pin",
        vec![bench_field()],
        move |cx, args| {
            let id = args.required_str("bench_id")?;
            let card = service::pin(cx, &s, id, args.bool("pinned")).map_err(kernel_error)?;
            Ok(serde_json::to_value(card)?)
        },
    );

    let s = shared.clone();
    r.command_asking(
        "bench.read",
        "bench.command.read",
        vec![bench_field()],
        move |cx, args| {
            let id = args.required_str("bench_id")?;
            let card = service::read(cx, &s, id, args.bool("unread")).map_err(kernel_error)?;
            Ok(serde_json::to_value(card)?)
        },
    );

    let s = shared.clone();
    r.command("bench.refresh", "bench.command.refresh", move |cx, _| {
        crate::refresh::refresh(cx, &s).map_err(kernel_error)?;
        Ok(json!({ "pending": true }))
    });

    let s = shared.clone();
    r.command_asking(
        "bench.script.approve",
        "bench.command.script_approve",
        vec![project_field()],
        move |cx, args| {
            let project_id = args.required_str("project_id")?;
            service::approve(cx, &s, project_id, args.str("kind")).map_err(kernel_error)
        },
    );

    let s = shared.clone();
    r.command(
        "bench.create_dialog",
        "bench.command.create_dialog",
        move |cx, args| {
            service::open_create_dialog(cx, &s, args.str("project_id"));
            Ok(json!({}))
        },
    );

    let s = shared.clone();
    r.command_asking(
        "bench.store_dialog",
        "bench.command.store_dialog",
        vec![bench_field()],
        move |_cx, args| {
            let id = args.required_str("bench_id")?;
            if let Some(bench) = s.find(id) {
                service::open_store_dialog(&s, &bench);
            }
            Ok(json!({}))
        },
    );

    let s = shared.clone();
    r.command_asking(
        "bench.approve_dialog",
        "bench.command.approve_dialog",
        vec![project_field()],
        move |cx, args| {
            let project_id = args.required_str("project_id")?;
            service::open_approve_dialog(cx, &s, project_id);
            Ok(json!({}))
        },
    );

    let s = shared.clone();
    r.command("bench.next", "bench.command.next", move |cx, _| {
        activate_step(cx, &s, 1)
    });
    let s = shared.clone();
    r.command("bench.previous", "bench.command.previous", move |cx, _| {
        activate_step(cx, &s, -1)
    });
    for index in 1..=10usize {
        let s = shared.clone();
        let id = format!("bench.switch_{index}");
        r.command(&id, "bench.command.switch", move |cx, _| {
            activate_index(cx, &s, index - 1)
        });
    }
}

fn activate_step(
    cx: &mut forja_kernel::Ctx<'_>,
    shared: &Shared,
    delta: isize,
) -> forja_kernel::Result<Value> {
    let benches = service::refs(cx, shared, None, false).map_err(kernel_error)?;
    if benches.is_empty() {
        return Ok(Value::Null);
    }
    let current = shared.current_id();
    let position = current
        .and_then(|id| benches.iter().position(|bench| bench.bench_id == id))
        .unwrap_or(0) as isize;
    let next = (position + delta).rem_euclid(benches.len() as isize) as usize;
    let bench = service::activate(cx, shared, &benches[next].bench_id).map_err(kernel_error)?;
    Ok(serde_json::to_value(bench)?)
}

fn activate_index(
    cx: &mut forja_kernel::Ctx<'_>,
    shared: &Shared,
    index: usize,
) -> forja_kernel::Result<Value> {
    let benches = service::refs(cx, shared, None, false).map_err(kernel_error)?;
    let Some(bench) = benches.get(index) else {
        return Ok(Value::Null);
    };
    let bench = service::activate(cx, shared, &bench.bench_id).map_err(kernel_error)?;
    Ok(serde_json::to_value(bench)?)
}
