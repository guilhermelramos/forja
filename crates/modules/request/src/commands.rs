//! Comandos da página de Pedidos: lista, atualização, Forjar, comentar, abrir e fechar,
//! e pedidos locais. Os balcões são chamados pelo id dos comandos `<módulo>.*`; o que
//! eles devolvem chega depois, em `counter.result`, e é aplicado por [`apply_result`].

use crate::forge;
use crate::local::{self, LocalFile, COUNTER as LOCAL};
use crate::state::{visible, rfc3339_utc, Filter, Notice, NoticeKind, PendingForge, Project, Shared, State};
use forja_contracts::{CounterError, CounterOp, CounterResult, RequestItem, RequestState};
use forja_counter_kit::{error_key, fixtures_enabled};
use forja_kernel::{Args, Ctx, KernelError, Result, SettingValue};
use serde_json::{json, Value};
use std::time::SystemTime;

/// Acesso curto ao estado; ignora envenenamento do mutex, que nunca acontece aqui.
pub(crate) fn with_state<T>(shared: &Shared, change: impl FnOnce(&mut State) -> T) -> Option<T> {
    shared.lock().ok().map(|mut state| change(&mut state))
}

fn set_notice(shared: &Shared, kind: NoticeKind, key: &str) {
    with_state(shared, |state| {
        state.notice = Some(Notice {
            kind,
            key: key.to_owned(),
        });
    });
}

/// Balcões instalados como `(id, módulo)`, pela vitrine do módulo `counter`.
pub fn sources(cx: &mut Ctx<'_>) -> Vec<(String, String)> {
    let Ok(value) = cx.run_command("counter.sources", Args::new()) else {
        return Vec::new();
    };
    value["counters"]
        .as_array()
        .map(|counters| {
            counters
                .iter()
                .filter_map(|counter| {
                    Some((
                        counter["counter"].as_str()?.to_owned(),
                        counter["module"].as_str()?.to_owned(),
                    ))
                })
                .collect()
        })
        .unwrap_or_default()
}

fn module_of(cx: &mut Ctx<'_>, counter: &str) -> Result<String> {
    sources(cx)
        .into_iter()
        .find(|(id, _)| id == counter)
        .map(|(_, module)| module)
        .ok_or_else(|| KernelError::message(format!("unknown counter `{counter}`")))
}

/// Projeto atual: o que a página guardou, ou a bancada ativa, se o módulo existir.
fn current_project(cx: &mut Ctx<'_>, shared: &Shared) -> Option<Project> {
    if let Some(project) = with_state(shared, |state| state.project.clone()).flatten() {
        return Some(project);
    }
    let value = cx.run_command("bench.current", Args::new()).ok()?;
    let project = Project {
        id: value["project_id"].as_str()?.to_owned(),
        path: value["path"].as_str()?.to_owned(),
    };
    with_state(shared, |state| state.project = Some(project.clone()));
    Some(project)
}

fn project_path(shared: &Shared) -> String {
    with_state(shared, |state| {
        state
            .project
            .as_ref()
            .map(|project| project.path.clone())
            .unwrap_or_default()
    })
    .unwrap_or_default()
}

fn find_item(shared: &Shared, counter: &str, key: &str) -> Option<RequestItem> {
    with_state(shared, |state| {
        state
            .remote
            .values()
            .flatten()
            .chain(state.local.iter())
            .find(|item| item.counter == counter && item.key == key)
            .cloned()
    })
    .flatten()
}

/// Carrega os pedidos locais do disco, uma vez, em segundo plano.
pub fn ensure_local(cx: &mut Ctx<'_>, shared: &Shared) {
    let start = with_state(shared, |state| {
        if state.local_loaded || state.local_loading {
            false
        } else {
            state.local_loading = true;
            true
        }
    })
    .unwrap_or(false);
    if !start {
        return;
    }
    let path = cx.store().path();
    let shared = shared.clone();
    cx.spawn(move || local::load(&path), move |_cx, result| {
        with_state(&shared, |state| {
            state.local_loading = false;
            state.local_loaded = true;
            match result {
                Ok(file) => state.local = file.items,
                Err(_) => {
                    state.notice = Some(Notice {
                        kind: NoticeKind::Error,
                        key: "request-notice-local-load-failed".to_owned(),
                    });
                }
            }
        });
    });
}

/// Grava os pedidos locais em segundo plano. Falha vira aviso, nunca some em silêncio.
pub(crate) fn persist_local(cx: &mut Ctx<'_>, shared: &Shared) {
    let path = cx.store().path();
    let file = with_state(shared, |state| LocalFile {
        items: state.local.clone(),
    })
    .unwrap_or_default();
    let shared = shared.clone();
    cx.spawn(move || local::save(&path, &file), move |_cx, result| {
        if result.is_err() {
            set_notice(&shared, NoticeKind::Error, "request-notice-save-failed");
        }
    });
}

/// Balcão mostrado ao abrir a página, a partir de `request.default.source`.
fn initialize_source(cx: &Ctx<'_>, shared: &Shared) {
    let default = cx
        .setting("request.default.source")
        .and_then(SettingValue::as_str)
        .unwrap_or("local")
        .to_owned();
    with_state(shared, |state| {
        if !state.source_initialized {
            state.source_initialized = true;
            state.source = Some(default);
        }
    });
}

/// Troca o balcão mostrado (`None` mostra todos).
pub fn set_source(shared: &Shared, source: Option<String>) {
    with_state(shared, |state| {
        state.source = source;
        state.source_initialized = true;
    });
}

/// Pede a lista de cada balcão para o filtro atual. Sem projeto e sem fixtures, não há o que pedir.
pub fn refresh(cx: &mut Ctx<'_>, shared: &Shared) -> Result<()> {
    initialize_source(cx, shared);
    let project = current_project(cx, shared);
    if project.is_none() && !fixtures_enabled() {
        // Marca a tentativa: sem isso, a tela pediria de novo em todo quadro.
        set_notice(shared, NoticeKind::Info, "request-notice-no-project");
        with_state(shared, |state| {
            state.last_refresh = Some(SystemTime::now());
            state.needs_refresh = false;
        });
        return Ok(());
    }
    let path = project.map(|project| project.path).unwrap_or_default();
    let scope = with_state(shared, |state| state.filter.scope()).unwrap_or_default();
    let source = with_state(shared, |state| state.source.clone()).flatten();
    for (counter, module) in sources(cx) {
        if source.as_ref().is_some_and(|wanted| *wanted != counter) {
            continue;
        }
        with_state(shared, |state| state.pending.insert((counter.clone(), scope)));
        let mut args = Args::new();
        args.insert("project_path", json!(path));
        args.insert("scope", serde_json::to_value(scope)?);
        args.insert("op_id", json!(format!("request:list:{counter}")));
        if cx.run_command(&format!("{module}.list"), args).is_err() {
            with_state(shared, |state| state.pending.remove(&(counter.clone(), scope)));
        }
    }
    with_state(shared, |state| {
        state.last_refresh = Some(SystemTime::now());
        state.needs_refresh = false;
    });
    Ok(())
}

/// Argumentos de `request.list`: filtro e busca são opcionais.
#[derive(Debug, Default)]
pub struct ListParams {
    pub filter: Option<String>,
    pub query: Option<String>,
}

/// Lista visível agora, em JSON, e atualização disparada. É o que o CLI `forja request list` lê.
pub fn list(cx: &mut Ctx<'_>, shared: &Shared, params: ListParams) -> Result<Value> {
    let default_filter = cx
        .setting("request.filters.default")
        .and_then(SettingValue::as_str)
        .and_then(Filter::from_id);
    with_state(shared, |state| {
        if let Some(filter) = params.filter.as_deref().and_then(Filter::from_id) {
            state.filter = filter;
            state.filter_initialized = true;
        } else if !state.filter_initialized {
            state.filter_initialized = true;
            if let Some(filter) = default_filter {
                state.filter = filter;
            }
        }
        if let Some(query) = params.query {
            state.query = query;
        }
    });
    ensure_local(cx, shared);
    refresh(cx, shared)?;
    Ok(snapshot(shared))
}

/// Estado da lista em JSON. Não toca em rede nem em disco.
pub fn snapshot(shared: &Shared) -> Value {
    let now = SystemTime::now();
    with_state(shared, |state| {
        let items = visible(state, now);
        let errors: serde_json::Map<String, Value> = state
            .errors
            .iter()
            .map(|(counter, error)| (counter.clone(), json!(error_key("request", error))))
            .collect();
        json!({
            "filter": state.filter.id(),
            "query": state.query,
            "loading": !state.pending.is_empty(),
            "local_loaded": state.local_loaded,
            "refreshed": state.last_refresh.is_some(),
            "items": items,
            "errors": errors,
        })
    })
    .unwrap_or_else(|| json!({ "items": [] }))
}

/// Forjar: cria a bancada a partir do pedido e sobe o ajudante padrão, se houver.
pub fn forge(cx: &mut Ctx<'_>, shared: &Shared, counter: &str, key: &str) -> Result<Value> {
    if !forge::bench_available(cx) {
        set_notice(shared, NoticeKind::Warn, "request-notice-bench-missing");
        return Ok(json!({ "forged": false, "reason": "bench_unavailable" }));
    }
    let Some(item) = find_item(shared, counter, key) else {
        set_notice(shared, NoticeKind::Warn, "request-notice-not-listed");
        return Err(KernelError::message("request is not in the list"));
    };
    let Some(project) = current_project(cx, shared) else {
        set_notice(shared, NoticeKind::Warn, "request-notice-no-project");
        return Err(KernelError::message("open a project before forging"));
    };
    let include_title = cx
        .setting("request.create.branch_from_title")
        .and_then(SettingValue::as_bool)
        .unwrap_or(true);
    let name = forge::bench_name(&item, include_title);
    let reference = item.request_ref();
    let created = forge::create_bench(cx, &project.id, &name, &reference)?;
    let actual_name = created
        .get("name")
        .and_then(Value::as_str)
        .unwrap_or(&name)
        .to_owned();
    let pending = created.get("pending").and_then(Value::as_bool) == Some(true);
    match created.get("path").and_then(Value::as_str).map(str::to_owned) {
        Some(path) if !pending => {
            let helper = forge::launch_helper(cx, &path, &reference)?;
            set_notice(shared, NoticeKind::Ok, "request-notice-forged");
            Ok(json!({ "forged": true, "bench": actual_name, "helper": helper }))
        }
        _ => {
            with_state(shared, |state| {
                state.pending_forge.push(PendingForge {
                    name: actual_name.clone(),
                    reference,
                });
            });
            set_notice(shared, NoticeKind::Info, "request-notice-forging");
            Ok(json!({ "forged": true, "bench": actual_name, "helper": false, "waiting": true }))
        }
    }
}

/// Starts an asynchronous bench preview; the view asks the bench module for its prepared result.
pub fn start_forge_preview(
    cx: &mut Ctx<'_>,
    shared: &Shared,
    preview_id: &str,
    counter: &str,
    key: &str,
) -> Result<()> {
    if !forge::bench_available(cx) {
        return Err(KernelError::message("the bench module is not available"));
    }
    let item = find_item(shared, counter, key)
        .ok_or_else(|| KernelError::message("request is not in the list"))?;
    let project = current_project(cx, shared)
        .ok_or_else(|| KernelError::message("open a project before forging"))?;
    let include_title = cx
        .setting("request.create.branch_from_title")
        .and_then(SettingValue::as_bool)
        .unwrap_or(true);
    let name = forge::bench_name(&item, include_title);
    let reference = item.request_ref();
    let mut args = Args::new();
    args.insert("preview_id", json!(preview_id));
    args.insert("project_id", json!(project.id));
    args.insert("name", json!(name));
    args.insert("from_request", serde_json::to_value(reference)?);
    let auto_start = cx
        .setting("helper.launch.auto_start")
        .and_then(SettingValue::as_bool)
        .unwrap_or(true);
    let helper_available = cx.registry().command("helper.launch").is_some();
    args.insert("helper", json!(auto_start && helper_available));
    cx.run_command("bench.preview_forge", args)?;
    Ok(())
}

/// Polls a prepared preview without touching project files on the interface thread.
pub fn forge_preview_status(cx: &mut Ctx<'_>, preview_id: &str) -> Result<Value> {
    let mut args = Args::new();
    args.insert("preview_id", json!(preview_id));
    cx.run_command("bench.preview_status", args)
}

/// Releases a prepared or pending preview when the user cancels it.
pub fn cancel_forge_preview(cx: &mut Ctx<'_>, preview_id: &str) {
    let mut args = Args::new();
    args.insert("preview_id", json!(preview_id));
    let _ = cx.run_command("bench.preview_cancel", args);
}

/// Comentário num pedido. Para balcão, o resultado chega em `counter.result`.
pub fn comment(cx: &mut Ctx<'_>, shared: &Shared, counter: &str, key: &str, body: &str) -> Result<Value> {
    let body = body.trim();
    if body.is_empty() {
        set_notice(shared, NoticeKind::Warn, "request-notice-empty-comment");
        return Err(KernelError::message("the comment is empty"));
    }
    if counter == LOCAL {
        let comment = forja_contracts::RequestComment {
            id: None,
            author: String::new(),
            body: body.to_owned(),
            created_at: Some(now_rfc3339()),
        };
        with_state(shared, |state| {
            if let Some(item) = state.local.iter_mut().find(|item| item.key == key) {
                item.comments.push(comment.clone());
            }
            if state.selected.as_ref().is_some_and(|(c, k)| c == counter && k == key) {
                if let Some(detail) = state.detail.as_mut() {
                    detail.comments.push(comment.clone());
                }
            }
        });
        persist_local(cx, shared);
        return Ok(json!({ "comment": comment }));
    }
    let module = module_of(cx, counter)?;
    let mut args = Args::new();
    args.insert("project_path", json!(project_path(shared)));
    args.insert("key", json!(key));
    args.insert("body", json!(body));
    args.insert("op_id", json!(format!("request:comment:{counter}:{key}")));
    cx.run_command(&format!("{module}.comment"), args)?;
    Ok(json!({ "started": true }))
}

/// Fecha ou reabre. Pedido local muda na hora; balcão responde em `counter.result`.
pub fn transition(cx: &mut Ctx<'_>, shared: &Shared, counter: &str, key: &str, to: RequestState) -> Result<Value> {
    if to == RequestState::Merged {
        return Err(KernelError::message("a request cannot be merged from here"));
    }
    if counter == LOCAL {
        let updated = with_state(shared, |state| {
            let item = state.local.iter_mut().find(|item| item.key == key)?;
            item.state = to;
            item.updated_at = Some(now_rfc3339());
            let copy = item.clone();
            if state.selected.as_ref().is_some_and(|(c, k)| c == counter && k == key) {
                state.detail = Some(copy.clone());
            }
            Some(copy)
        })
        .flatten();
        persist_local(cx, shared);
        return Ok(serde_json::to_value(updated)?);
    }
    let module = module_of(cx, counter)?;
    let mut args = Args::new();
    args.insert("project_path", json!(project_path(shared)));
    args.insert("key", json!(key));
    args.insert("to", serde_json::to_value(to)?);
    args.insert("op_id", json!(format!("request:transition:{counter}:{key}")));
    cx.run_command(&format!("{module}.transition"), args)?;
    Ok(json!({ "started": true }))
}

/// Abre a gaveta de detalhes. Mostra já o que a lista tem e pede o pedido completo.
pub fn open(cx: &mut Ctx<'_>, shared: &Shared, counter: &str, key: &str) -> Result<Value> {
    let cached = find_item(shared, counter, key);
    with_state(shared, |state| {
        state.selected = Some((counter.to_owned(), key.to_owned()));
        state.detail = cached.clone();
    });
    if counter == LOCAL {
        return Ok(json!({ "local": true }));
    }
    let module = module_of(cx, counter)?;
    let mut args = Args::new();
    args.insert("project_path", json!(project_path(shared)));
    args.insert("key", json!(key));
    args.insert("op_id", json!(format!("request:get:{counter}:{key}")));
    cx.run_command(&format!("{module}.get"), args)?;
    Ok(json!({ "started": true }))
}

/// Cria um pedido local. O título é obrigatório; o resto é opcional.
pub fn create_local(cx: &mut Ctx<'_>, shared: &Shared, title: &str, body: Option<&str>) -> Result<Value> {
    if title.trim().is_empty() {
        set_notice(shared, NoticeKind::Warn, "request-notice-empty-title");
        return Err(KernelError::message("the title is empty"));
    }
    let loaded = with_state(shared, |state| state.local_loaded).unwrap_or(false);
    if !loaded {
        set_notice(shared, NoticeKind::Info, "request-notice-local-loading");
        return Err(KernelError::message("local requests are still loading"));
    }
    let prefix = cx
        .setting("request.local.prefix")
        .and_then(SettingValue::as_str)
        .unwrap_or("LOCAL-")
        .to_owned();
    let start = cx
        .setting("request.local.counter_start")
        .and_then(SettingValue::as_i64)
        .unwrap_or(1)
        .clamp(1, 100_000) as u32;
    let assign_self = cx
        .setting("request.create.assign_self")
        .and_then(SettingValue::as_bool)
        .unwrap_or(true);
    let now = now_rfc3339();
    let created = with_state(shared, |state| {
        let mut item = local::new_item(&state.local, &prefix, start, title, body, &now);
        item.mine = assign_self;
        state.local.push(item.clone());
        item
    });
    let Some(created) = created else {
        return Err(KernelError::message("the request state is unavailable"));
    };
    persist_local(cx, shared);
    Ok(serde_json::to_value(created)?)
}

fn now_rfc3339() -> String {
    rfc3339_utc(
        SystemTime::now()
            .duration_since(SystemTime::UNIX_EPOCH)
            .map(|duration| duration.as_secs())
            .unwrap_or(0),
    )
}

/// Aplica um resultado de balcão ao estado. Não toca em rede nem em disco.
/// Devolve `true` quando a lista deve ser pedida de novo (por exemplo, após conectar).
pub fn apply_result(shared: &Shared, result: &CounterResult) -> bool {
    let mut refresh = false;
    with_state(shared, |state| {
        let counter = result.counter.clone();
        match result.op {
            CounterOp::List => {
                let scope = result.scope.unwrap_or_default();
                state.pending.remove(&(counter.clone(), scope));
                match &result.error {
                    Some(error) => {
                        state.remote.remove(&(counter.clone(), scope));
                        state.errors.insert(counter.clone(), error.clone());
                    }
                    None => {
                        state.errors.remove(&counter);
                        state.remote.insert((counter.clone(), scope), result.items.clone());
                    }
                }
            }
            CounterOp::Get => {
                if let (Some(item), None) = (&result.item, &result.error) {
                    let selected = state
                        .selected
                        .as_ref()
                        .is_some_and(|(c, k)| *c == counter && *k == item.key);
                    if selected {
                        state.detail = Some(item.clone());
                    }
                }
                if let Some(error) = &result.error {
                    notice(state, error);
                }
            }
            CounterOp::Comment => match (&result.comment, &result.error) {
                (Some(comment), None) => {
                    if let Some((c, key)) = op_key(result, "comment") {
                        if let Some(detail) = state.detail.as_mut() {
                            if detail.counter == c && detail.key == key {
                                detail.comments.push(comment.clone());
                            }
                        }
                    }
                }
                (_, Some(error)) => notice(state, error),
                _ => {}
            },
            CounterOp::Transition => {
                if let (Some(item), None) = (&result.item, &result.error) {
                    replace_cached(state, item);
                } else if let Some(error) = &result.error {
                    notice(state, error);
                }
            }
            CounterOp::Connect | CounterOp::Test | CounterOp::Disconnect => {
                if result.op == CounterOp::Connect && result.error.is_none() {
                    refresh = true;
                }
                if let Some(error) = &result.error {
                    notice(state, error);
                }
            }
        }
        if refresh {
            state.needs_refresh = true;
        }
    });
    refresh
}

fn notice(state: &mut State, error: &CounterError) {
    state.notice = Some(Notice {
        kind: NoticeKind::Error,
        key: error_key("request", error),
    });
}

/// Chave do pedido guardada no `op_id` (`request:<op>:<balcão>:<chave>`).
fn op_key(result: &CounterResult, op: &str) -> Option<(String, String)> {
    let op_id = result.op_id.as_deref()?;
    let rest = op_id.strip_prefix(&format!("request:{op}:"))?;
    let (counter, key) = rest.split_once(':')?;
    Some((counter.to_owned(), key.to_owned()))
}

fn replace_cached(state: &mut State, item: &RequestItem) {
    for list in state.remote.values_mut() {
        for cached in list.iter_mut() {
            if cached.counter == item.counter && cached.key == item.key {
                *cached = item.clone();
            }
        }
    }
    if state
        .selected
        .as_ref()
        .is_some_and(|(c, k)| *c == item.counter && *k == item.key)
    {
        state.detail = Some(item.clone());
    }
}

/// Pedidos de Forjar que esperavam a bancada. Devolve os que `bench.created` fechou.
pub fn take_pending_forge(shared: &Shared, bench_name: &str) -> Option<PendingForge> {
    with_state(shared, |state| {
        let index = state
            .pending_forge
            .iter()
            .position(|pending| pending.name == bench_name)?;
        Some(state.pending_forge.remove(index))
    })
    .flatten()
}

/// Projeto vindo de `bench.activated`.
pub fn set_project(shared: &Shared, id: &str, path: &str) {
    with_state(shared, |state| {
        state.project = Some(Project {
            id: id.to_owned(),
            path: path.to_owned(),
        });
    });
}
