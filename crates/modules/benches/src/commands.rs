use crate::service;
use crate::state::Shared;
use forja_kernel::Registrar;
use serde_json::{json, Value};

use crate::kernel_error;
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
    r.command(
        "bench.activate",
        "bench.command.activate",
        move |cx, args| {
            let id = args.required_str("bench_id")?;
            let bench = service::activate(cx, &s, id).map_err(kernel_error)?;
            Ok(serde_json::to_value(bench)?)
        },
    );

    let s = shared.clone();
    r.command("bench.create", "bench.command.create", move |cx, args| {
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
    });

    let s = shared.clone();
    r.command("bench.store", "bench.command.store", move |cx, args| {
        let id = args.required_str("bench_id")?;
        crate::store::store(cx, &s, id, args.bool("delete_branch"), args.bool("force"))
            .map_err(kernel_error)?;
        Ok(json!({ "pending": true }))
    });

    let s = shared.clone();
    r.command(
        "bench.stage.set",
        "bench.command.stage_set",
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
    r.command("bench.sleep", "bench.command.sleep", move |cx, args| {
        let id = args.required_str("bench_id")?;
        let card = service::sleep(cx, &s, id).map_err(kernel_error)?;
        Ok(serde_json::to_value(card)?)
    });

    let s = shared.clone();
    r.command("bench.wake", "bench.command.wake", move |cx, args| {
        let id = args.required_str("bench_id")?;
        let card = service::wake(cx, &s, id).map_err(kernel_error)?;
        Ok(serde_json::to_value(card)?)
    });

    let s = shared.clone();
    r.command("bench.pin", "bench.command.pin", move |cx, args| {
        let id = args.required_str("bench_id")?;
        let card = service::pin(cx, &s, id, args.bool("pinned")).map_err(kernel_error)?;
        Ok(serde_json::to_value(card)?)
    });

    let s = shared.clone();
    r.command("bench.read", "bench.command.read", move |cx, args| {
        let id = args.required_str("bench_id")?;
        let card = service::read(cx, &s, id, args.bool("unread")).map_err(kernel_error)?;
        Ok(serde_json::to_value(card)?)
    });

    let s = shared.clone();
    r.command("bench.refresh", "bench.command.refresh", move |cx, _| {
        crate::refresh::refresh(cx, &s).map_err(kernel_error)?;
        Ok(json!({ "pending": true }))
    });

    let s = shared.clone();
    r.command(
        "bench.script.approve",
        "bench.command.script_approve",
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
    r.command(
        "bench.store_dialog",
        "bench.command.store_dialog",
        move |_cx, args| {
            let id = args.required_str("bench_id")?;
            if let Some(bench) = s.find(id) {
                service::open_store_dialog(&s, &bench);
            }
            Ok(json!({}))
        },
    );

    let s = shared.clone();
    r.command(
        "bench.approve_dialog",
        "bench.command.approve_dialog",
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
