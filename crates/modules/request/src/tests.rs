//! Testes do módulo `request`: contrato, o fluxo Forjar com `bench.create` falso,
//! o RPC `request.list`, pedidos locais em disco e a aplicação de resultados de balcão.

use super::*;
use crate::commands;
use crate::state::{Filter, State};
use forja_contracts::{
    names, BenchCreated, BenchRef, CounterOp, CounterResult, RequestItem, RequestScope,
};
use forja_kernel::forja_protocol::RpcError;
use forja_kernel::{contract, Args, Module, ModuleManifest, Registrar, Runtime, RuntimeOptions};
use serde_json::{json, Map, Value};
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};

/// O que o `bench` e o `helper` falsos receberam.
#[derive(Default)]
struct Calls {
    create: Vec<Value>,
    launch: Vec<Value>,
}

/// Bancada falsa: responde `bench.current` e `bench.create`. Com `async_mode`, a criação
/// não devolve o caminho; a bancada chega depois, pelo evento `bench.created`.
struct FakeBench {
    calls: Arc<Mutex<Calls>>,
    async_mode: bool,
}

impl Module for FakeBench {
    fn manifest(&self) -> ModuleManifest {
        ModuleManifest::new("bench", "bench.name", "0.0.0")
    }

    fn register(&mut self, r: &mut Registrar<'_>) {
        r.i18n(
            "bench-name = Fake benches\nbench-command-create = Create fake bench\nbench-command-current = Current fake bench\n",
            "bench-name = Bancadas falsas\nbench-command-create = Criar bancada falsa\nbench-command-current = Bancada atual falsa\n",
        );
        r.uses_i18n("bench-command-create");
        r.uses_i18n("bench-command-current");
        let calls = Arc::clone(&self.calls);
        let async_mode = self.async_mode;
        r.command("bench.create", "bench.command.create", move |_cx, args| {
            let name = args.str("name").unwrap_or_default().to_owned();
            calls.lock().unwrap().create.push(args.into_json());
            if async_mode {
                return Ok(json!({ "status": "queued", "name": name }));
            }
            Ok(json!({
                "project_id": "p1",
                "bench_id": "b-new",
                "name": name,
                "path": "C:/bench/novo",
                "branch": null
            }))
        });
        r.command("bench.current", "bench.command.current", |_cx, _args| {
            Ok(json!({
                "project_id": "p1",
                "bench_id": "b-main",
                "name": "main",
                "path": "C:/repo",
                "branch": "main"
            }))
        });
    }
}

/// Ajudante falso: só registra o que foi pedido para subir.
struct FakeHelper {
    calls: Arc<Mutex<Calls>>,
}

impl Module for FakeHelper {
    fn manifest(&self) -> ModuleManifest {
        ModuleManifest::new("helper", "helper.name", "0.0.0")
    }

    fn register(&mut self, r: &mut Registrar<'_>) {
        r.i18n(
            "helper-name = Fake helper\nhelper-command-launch = Launch fake helper\n",
            "helper-name = Ajudante falso\nhelper-command-launch = Subir ajudante falso\n",
        );
        r.uses_i18n("helper-command-launch");
        let calls = Arc::clone(&self.calls);
        r.command("helper.launch", "helper.command.launch", move |_cx, args| {
            calls.lock().unwrap().launch.push(args.into_json());
            Ok(json!({ "started": true }))
        });
    }
}

struct TempGuard(PathBuf);

impl std::ops::Deref for TempGuard {
    type Target = Path;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl Drop for TempGuard {
    fn drop(&mut self) {
        for attempt in 0..20 {
            if std::fs::remove_dir_all(&self.0).is_ok() || !self.0.exists() {
                return;
            }
            if attempt < 19 {
                std::thread::sleep(std::time::Duration::from_millis(15));
            }
        }
    }
}

fn dir(label: &str) -> TempGuard {
    let dir = std::env::temp_dir().join(format!(
        "forja-request-{label}-{}-{}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_nanos())
            .unwrap_or(0)
    ));
    std::fs::create_dir_all(&dir).unwrap();
    TempGuard(dir)
}

fn runtime(data: &Path, async_mode: bool, with_bench: bool) -> (Runtime, Arc<Mutex<Calls>>) {
    let calls = Arc::new(Mutex::new(Calls::default()));
    let mut modules: Vec<Box<dyn Module>> = vec![
        Box::new(RequestModule::new()),
        Box::new(FakeHelper {
            calls: Arc::clone(&calls),
        }),
    ];
    if with_bench {
        modules.push(Box::new(FakeBench {
            calls: Arc::clone(&calls),
            async_mode,
        }));
    }
    let runtime = Runtime::new(
        modules,
        RuntimeOptions {
            data_dir: Some(data.to_path_buf()),
            user_dir: Some(data.join("user")),
            locale: Some("pt-BR".to_owned()),
            wake: None,
        },
    )
    .expect("request module registers with the fakes");
    (runtime, calls)
}

/// Processa os resultados em resposta ao sinal de conclusão de cada tarefa.
fn settle(runtime: &mut Runtime, mut done: impl FnMut(&mut Runtime) -> bool) {
    let (sender, receiver) = std::sync::mpsc::channel();
    runtime.set_wake(Arc::new(move || { let _ = sender.send(()); }));
    loop {
        runtime.poll_tasks();
        if done(runtime) { return; }
        receiver.recv_timeout(std::time::Duration::from_secs(60)).expect("request task did not signal completion");
    }
}

/// Arquivo do store do módulo: o kernel guarda cada módulo em `<dados>/modules/<id>.json`.
fn store_file(data: &Path) -> PathBuf {
    data.join("modules").join("request.json")
}

fn args(pairs: &[(&str, Value)]) -> Args {
    let mut map = Map::new();
    for (key, value) in pairs {
        map.insert((*key).to_owned(), value.clone());
    }
    Args::from_map(map)
}

/// Cria um pedido local e espera a gravação em disco terminar.
fn create_local(runtime: &mut Runtime, data: &Path, title: &str) -> Value {
    settle(runtime, |rt| {
        rt.run_command("request.list", Args::new())
            .map(|v| v["local_loaded"] == true)
            .unwrap_or(false)
    });
    let created = runtime
        .run_command("request.local.create", args(&[("title", json!(title))]))
        .expect("local request is created");
    settle(runtime, |_| store_file(data).exists());
    created
}

#[test]
fn contract_holds_for_the_module_alone() {
    let data = dir("contract");
    let (runtime, _) = runtime(&data, false, false);
    contract::assert_ok(runtime.registry(), runtime.i18n());
}

#[test]
fn forjar_creates_the_bench_from_the_request_and_starts_the_helper() {
    let data = dir("forge");
    let (mut runtime, calls) = runtime(&data, false, true);
    let local = create_local(&mut runtime, &data, "Corrigir o cache do quadro");
    assert_eq!(local["key"], "LOCAL-1");

    let result = runtime
        .run_command("request.forge", args(&[("counter", json!("local")), ("key", json!("LOCAL-1"))]))
        .expect("forge succeeds");
    assert_eq!(result["forged"], true);
    assert_eq!(result["helper"], true);

    let calls = calls.lock().unwrap();
    let create = &calls.create[0];
    assert_eq!(create["project_id"], "p1", "the project comes from the current bench");
    assert_eq!(create["name"], "pedido-local-1-corrigir-o-cache-do-quadro");
    assert_eq!(create["from_request"]["key"], "LOCAL-1");
    assert_eq!(calls.launch[0]["bench_path"], "C:/bench/novo");
    assert_eq!(calls.launch[0]["from_request"]["counter"], "local");
    assert_eq!(calls.launch[0]["initial_context"], "request");
    drop(calls);
}

#[test]
fn forjar_waits_for_bench_created_when_the_bench_is_not_ready() {
    let data = dir("forge-async");
    let (mut runtime, calls) = runtime(&data, true, true);
    create_local(&mut runtime, &data, "Revisar textos");
    let result = runtime
        .run_command("request.forge", args(&[("counter", json!("local")), ("key", json!("LOCAL-1"))]))
        .unwrap();
    assert_eq!(result["waiting"], true);
    assert!(calls.lock().unwrap().launch.is_empty(), "nothing starts before the bench exists");

    let name = result["bench"].as_str().unwrap().to_owned();
    let created = BenchCreated(BenchRef {
        project_id: "p1".into(),
        bench_id: "b-new".into(),
        name,
        path: "C:/bench/chegou".into(),
        branch: None,
    });
    runtime.emit(&created);
    let calls = calls.lock().unwrap();
    assert_eq!(calls.launch.len(), 1);
    assert_eq!(calls.launch[0]["bench_path"], "C:/bench/chegou");
    drop(calls);
}

#[test]
fn forjar_degrades_when_the_bench_module_is_missing() {
    let data = dir("forge-missing");
    let (mut runtime, _) = runtime(&data, false, false);
    create_local(&mut runtime, &data, "Sem bancada");
    let result = runtime
        .run_command("request.forge", args(&[("counter", json!("local")), ("key", json!("LOCAL-1"))]))
        .unwrap();
    assert_eq!(result["forged"], false);
    assert_eq!(result["reason"], "bench_unavailable");
}

#[test]
fn forjar_refuses_a_request_that_is_not_in_the_list() {
    let data = dir("forge-unknown");
    let (mut runtime, _) = runtime(&data, false, true);
    let error = runtime
        .run_command("request.forge", args(&[("counter", json!("github")), ("key", json!("#999"))]))
        .unwrap_err();
    assert!(error.to_string().contains("not in the list"));
}

#[test]
fn the_rpc_lists_the_visible_requests_as_json() {
    let data = dir("rpc");
    let (mut runtime, _) = runtime(&data, false, true);
    create_local(&mut runtime, &data, "Primeiro");
    let mut params = Map::new();
    params.insert("filter".into(), json!("mine"));
    let value = runtime
        .call_method("request.list", params)
        .expect("request.list answers");
    assert_eq!(value["filter"], "mine");
    assert_eq!(value["items"][0]["key"], "LOCAL-1");
    assert_eq!(value["items"][0]["counter"], "local");

    let mut bad = Map::new();
    bad.insert("filter".into(), json!(42));
    assert!(matches!(
        runtime.call_method("request.list", bad),
        Err(RpcError { .. })
    ));
}

#[test]
fn local_requests_are_written_to_the_store_folder() {
    let data = dir("local-store");
    let (mut runtime, _) = runtime(&data, false, false);
    create_local(&mut runtime, &data, "Guardar no disco");
    let text = std::fs::read_to_string(store_file(&data)).unwrap();
    assert!(text.contains("LOCAL-1"));
    assert!(text.contains("Guardar no disco"));
}

#[test]
fn empty_titles_and_comments_are_refused_without_saving() {
    let data = dir("empty");
    let (mut runtime, _) = runtime(&data, false, false);
    settle(&mut runtime, |rt| {
        rt.run_command("request.list", Args::new())
            .map(|v| v["local_loaded"] == true)
            .unwrap_or(false)
    });
    assert!(runtime
        .run_command("request.local.create", args(&[("title", json!("   "))]))
        .is_err());
    assert!(!store_file(&data).exists());
}

#[test]
fn counter_results_fill_the_list_and_the_drawer() {
    let shared: Shared = Arc::new(Mutex::new(State {
        filter: Filter::Open,
        ..State::default()
    }));
    let item = RequestItem::new("github", "#41", "Login falha");
    let mut result = CounterResult::ok("github", CounterOp::List);
    result.scope = Some(RequestScope::Open);
    result.items = vec![item.clone()];
    commands::apply_result(&shared, &result);
    let snapshot = commands::snapshot(&shared);
    assert_eq!(snapshot["items"][0]["key"], "#41");

    let mut failed = CounterResult::failed(
        "github",
        CounterOp::List,
        forja_contracts::CounterError::Unauthorized,
    );
    failed.scope = Some(RequestScope::Open);
    commands::apply_result(&shared, &failed);
    let snapshot = commands::snapshot(&shared);
    assert_eq!(snapshot["items"].as_array().unwrap().len(), 0);
    assert_eq!(snapshot["errors"]["github"], "request-error-unauthorized");
}

#[test]
fn a_connect_result_asks_for_a_fresh_list() {
    let shared: Shared = Arc::new(Mutex::new(State::default()));
    let connected = CounterResult::ok("gitlab", CounterOp::Connect);
    assert!(commands::apply_result(&shared, &connected));
}

#[test]
fn the_filter_ids_round_trip_through_the_rpc_names() {
    for filter in Filter::ALL {
        assert_eq!(Filter::from_id(filter.id()), Some(filter));
    }
    assert_eq!(names::BENCH_CREATED, "bench.created");
}

#[test]
fn scopes_are_the_ones_the_counters_understand() {
    assert_eq!(Filter::Mine.scope(), RequestScope::Mine);
    assert_eq!(Filter::Recent.scope(), RequestScope::All);
}


#[test]
fn without_a_project_the_refresh_is_recorded_so_frames_do_not_retry() {
    let data = dir("no-project-refresh");
    let (mut runtime, _) = runtime(&data, false, false);
    let value = runtime.run_command("request.list", Args::new()).unwrap();
    assert_eq!(value["refreshed"], true, "the attempt is remembered even with no project");
    assert_eq!(value["items"], json!([]));
}

#[test]
fn rpc_local_create_and_forge_work_end_to_end() {
    let data = dir("rpc-forge");
    let (mut runtime, calls) = runtime(&data, false, true);
    settle(&mut runtime, |rt| {
        rt.call_method("request.list", Map::new())
            .map(|v| v["local_loaded"] == true)
            .unwrap_or(false)
    });
    let created = runtime
        .call_method(
            "request.local.create",
            Map::from_iter([
                ("title".into(), json!("Corrigir fluxo RPC")),
                ("body".into(), json!("Detalhes")),
            ]),
        )
        .expect("request.local.create rpc succeeds");
    assert_eq!(created["key"], "LOCAL-1");

    let forged = runtime
        .call_method(
            "request.forge",
            Map::from_iter([
                ("counter".into(), json!("local")),
                ("key".into(), json!("LOCAL-1")),
            ]),
        )
        .expect("request.forge rpc succeeds");
    assert_eq!(forged["forged"], true);
    assert_eq!(forged["helper"], true);
    assert_eq!(calls.lock().unwrap().launch.len(), 1);
}

#[test]
fn assign_self_and_default_filter_settings_apply() {
    let data = dir("settings-effect");
    let (mut runtime, _) = runtime(&data, false, true);
    runtime
        .set_setting("request.filters.default", SettingValue::Text("open".into()))
        .unwrap();
    runtime
        .set_setting("request.create.assign_self", SettingValue::Bool(false))
        .unwrap();
    settle(&mut runtime, |rt| {
        rt.run_command("request.list", Args::new())
            .map(|v| v["local_loaded"] == true)
            .unwrap_or(false)
    });
    let listed = runtime.run_command("request.list", Args::new()).unwrap();
    assert_eq!(listed["filter"], "open");
    let created = runtime
        .run_command("request.local.create", args(&[("title", json!("Sem dono"))]))
        .unwrap();
    assert_eq!(created["mine"], false);
}
