//! Testes de integração: o módulo no kernel real, com terminal, ajudante e bancada falsos.
//! Nenhum processo roda de verdade; o arquivo do store vai para uma pasta temporária.

use super::*;
use crate::model::{Action, Failure, Routine, RunStatus, Store, Target, Trigger};
use crate::schedule::{Preset, Schedule};
use crate::state::Shared;
use forja_contracts::{HelperStatus, HelperStatusChanged, TerminalCommandFinished, TerminalRef};
use forja_kernel::{contract, Args, Module, ModuleManifest, Registrar, Runtime, RuntimeOptions};
use serde_json::{json, Map};
use std::path::PathBuf;
use std::sync::{Arc, Mutex};
use std::time::SystemTime;

/// O que os módulos falsos receberam.
#[derive(Default)]
struct Calls {
    opened: Vec<Value>,
    sent: Vec<Value>,
    helper: Vec<Value>,
    bench: Vec<Value>,
}

struct FakeTerminal(Arc<Mutex<Calls>>);

impl Module for FakeTerminal {
    fn manifest(&self) -> ModuleManifest {
        ModuleManifest::new("terminal", "terminal-name", "0.0.0")
    }

    fn register(&mut self, r: &mut Registrar<'_>) {
        r.i18n("terminal-name = Fake terminal\nterminal-open = Open\nterminal-send = Send\nterminal-read = Read\n", "terminal-name = Terminal falso\nterminal-open = Abrir\nterminal-send = Enviar\nterminal-read = Ler\n");
        r.uses_i18n("terminal-open");
        let calls = Arc::clone(&self.0);
        r.command("terminal.open", "terminal-open", move |_cx, args| {
            calls.lock().unwrap().opened.push(args.into_json());
            Ok(json!({ "terminal_id": "t-1" }))
        });
        let calls = Arc::clone(&self.0);
        r.command("terminal.send", "terminal-send", move |_cx, args| {
            calls.lock().unwrap().sent.push(args.into_json());
            Ok(json!({}))
        });
        r.command("terminal.read", "terminal-read", |_cx, _args| {
            Ok(json!({ "text": "compilando\npassou\n" }))
        });
    }
}

struct FakeHelper(Arc<Mutex<Calls>>);

impl Module for FakeHelper {
    fn manifest(&self) -> ModuleManifest {
        ModuleManifest::new("helper", "helper-name", "0.0.0")
    }

    fn register(&mut self, r: &mut Registrar<'_>) {
        r.i18n("helper-name = Fake helper\nhelper-launch = Launch\n", "helper-name = Ajudante falso\nhelper-launch = Subir\n");
        r.uses_i18n("helper-launch");
        let calls = Arc::clone(&self.0);
        r.command("helper.launch", "helper-launch", move |_cx, args| {
            calls.lock().unwrap().helper.push(args.into_json());
            Ok(json!({ "started": true }))
        });
    }
}

struct FakeBench(Arc<Mutex<Calls>>);

impl Module for FakeBench {
    fn manifest(&self) -> ModuleManifest {
        ModuleManifest::new("bench", "bench-name", "0.0.0")
    }

    fn register(&mut self, r: &mut Registrar<'_>) {
        r.i18n("bench-name = Fake bench\nbench-create = Create\n", "bench-name = Bancada falsa\nbench-create = Criar\n");
        r.uses_i18n("bench-create");
        let calls = Arc::clone(&self.0);
        r.command("bench.create", "bench-create", move |_cx, args| {
            calls.lock().unwrap().bench.push(args.into_json());
            Ok(json!({ "path": "C:/bench/nova" }))
        });
    }
}

struct TempGuard(PathBuf);

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

struct Harness {
    runtime: Runtime,
    shared: Shared,
    calls: Arc<Mutex<Calls>>,
    dir: PathBuf,
    _guard: TempGuard,
}

fn dir(label: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!(
        "forja-routine-{label}-{}-{}",
        std::process::id(),
        SystemTime::now()
            .duration_since(SystemTime::UNIX_EPOCH)
            .map(|d| d.as_nanos())
            .unwrap_or(0)
    ));
    std::fs::create_dir_all(&dir).unwrap();
    dir
}


/// Sobe o módulo com os falsos escolhidos.
fn harness(label: &str, helper: bool, bench: bool) -> Harness {
    let dir = dir(label);
    let calls = Arc::new(Mutex::new(Calls::default()));
    let module = RoutineModule::new();
    let shared = Arc::clone(&module.shared);
    let mut modules: Vec<Box<dyn Module>> = vec![Box::new(module), Box::new(FakeTerminal(Arc::clone(&calls)))];
    if helper {
        modules.push(Box::new(FakeHelper(Arc::clone(&calls))));
    }
    if bench {
        modules.push(Box::new(FakeBench(Arc::clone(&calls))));
    }
    let runtime = Runtime::new(
        modules,
        RuntimeOptions {
            data_dir: Some(dir.clone()),
            user_dir: Some(dir.join("user")),
            locale: Some("pt-BR".to_owned()),
            wake: None,
        },
    )
    .expect("routine registers with the fakes");
    Harness {
        runtime,
        shared,
        calls,
        dir: dir.clone(),
        _guard: TempGuard(dir),
    }
}

impl Harness {
    fn store_file(&self) -> PathBuf {
        self.dir.join("modules").join("routine.json")
    }

    /// Carrega o arquivo (ou um vazio) e espera a tarefa terminar.
    fn load(&mut self) {
        let mut cx = self.runtime.ctx("routine");
        actions::ensure_loaded(&mut cx, &self.shared);
        self.settle(|h| h.shared.lock().unwrap().loaded);
    }

    fn settle(&mut self, done: impl Fn(&Harness) -> bool) {
        let (sender, receiver) = std::sync::mpsc::channel();
        self.runtime.set_wake(Arc::new(move || { let _ = sender.send(()); }));
        loop {
            self.runtime.poll_tasks();
            if done(self) { return; }
            receiver.recv_timeout(std::time::Duration::from_secs(60)).expect("routine task did not signal completion");
        }
    }

    fn call(&mut self, id: &str, args: Args) -> Result<Value, String> {
        self.runtime.run_command(id, args).map_err(|e| e.to_string())
    }

    fn rpc(&mut self, method: &str, params: Map<String, Value>) -> Result<Value, String> {
        self.runtime.call_method(method, params).map_err(|e| e.message)
    }

    fn save(&mut self, routine: Value) -> Value {
        let mut args = Args::new();
        args.insert("routine", routine);
        self.call("routine.save", args).unwrap()
    }
}

fn args(pairs: &[(&str, Value)]) -> Args {
    let mut map = Map::new();
    for (key, value) in pairs {
        map.insert((*key).to_owned(), value.clone());
    }
    Args::from_map(map)
}

fn command_routine(name: &str, command: &str) -> Value {
    serde_json::to_value(Routine {
        name: name.into(),
        target: Target::Fixed {
            bench_path: "C:/repo".into(),
        },
        action: Action::Command {
            command: command.into(),
        },
        ..Routine::default()
    })
    .unwrap()
}

fn terminal_done(harness: &mut Harness, terminal_id: &str, exit: Option<i32>) {
    let event = TerminalCommandFinished {
        terminal: TerminalRef {
            terminal_id: terminal_id.into(),
            bench_path: "C:/repo".into(),
            title: "t".into(),
        },
        exit_code: exit,
    };
    harness.runtime.emit(&event);
}

#[test]
fn contract_holds_for_the_module_alone() {
    let h = harness("contract", false, false);
    contract::assert_ok(h.runtime.registry(), h.runtime.i18n());
}

#[test]
fn saving_validates_and_refuses_before_the_store_is_loaded() {
    let mut h = harness("save", false, false);
    let early = h.save(command_routine("Cedo", "x"));
    assert_eq!(early["saved"], false);
    assert_eq!(early["error"], "routine-error-loading");

    h.load();
    let blank = h.save(command_routine("   ", "x"));
    assert_eq!(blank["error"], "routine-error-name_empty");
    assert!(h.shared.lock().unwrap().store.routines.is_empty());

    let saved = h.save(command_routine("Resumo", "cargo test"));
    assert_eq!(saved["saved"], true);
    let id = saved["id"].as_str().unwrap().to_owned();
    h.settle(|h| h.store_file().exists());
    let on_disk = std::fs::read_to_string(h.store_file()).unwrap();
    assert!(on_disk.contains(&id), "the routine reached the file");

    let listed = h.rpc("routine.list", Map::new()).unwrap();
    assert_eq!(listed["routines"][0]["name"], "Resumo");
    assert_eq!(listed["routines"][0]["schedule"], "0 9 * * *");
    let _ = std::fs::remove_dir_all(h.dir.clone());
}

#[test]
fn concurrent_persist_requests_write_the_latest_snapshot_last() {
    let mut h = harness("persist-order", false, false);
    h.load();
    h.save(command_routine("First", "first"));
    h.save(command_routine("Second", "second"));
    {
        let state = h.shared.lock().unwrap();
        assert!(state.persisting && state.persist_queued);
    }
    h.settle(|h| !h.shared.lock().unwrap().persisting);
    let stored = actions::load_file(&h.store_file()).unwrap();
    let names: Vec<_> = stored.routines.iter().map(|routine| routine.name.as_str()).collect();
    assert_eq!(names, ["First", "Second"]);
    let _ = std::fs::remove_dir_all(h.dir.clone());
}

#[test]
fn a_command_routine_runs_end_to_end_and_its_terminal_result_is_recorded() {
    let mut h = harness("run", false, false);
    h.load();
    let saved = h.save(command_routine("Testes", "cargo test"));
    let id = saved["id"].as_str().unwrap().to_owned();

    let started = h.call("routine.run", args(&[("id", json!(id))])).unwrap();
    assert!(started["run_id"].is_string());
    {
        let calls = h.calls.lock().unwrap();
        assert_eq!(calls.opened[0]["cwd"], "C:/repo", "the bench is the working directory");
        assert_eq!(calls.opened[0]["title"], "Testes");
        assert_eq!(calls.sent[0]["terminal_id"], "t-1");
        assert_eq!(calls.sent[0]["text"], "cargo test");
        assert_eq!(calls.sent[0]["enter"], true);
    }

    terminal_done(&mut h, "t-1", Some(0));
    let runs = h.rpc("routine.runs", Map::new()).unwrap();
    assert_eq!(runs["runs"][0]["status"]["state"], "succeeded");
    assert_eq!(runs["runs"][0]["trigger"], "manual");
    assert_eq!(runs["runs"][0]["summary"], "compilando\npassou", "the output is summarized as data");

    h.settle(|h| std::fs::read_to_string(h.store_file()).map(|t| t.contains("succeeded")).unwrap_or(false));
    let _ = std::fs::remove_dir_all(h.dir.clone());
}

#[test]
fn a_failing_command_is_recorded_with_its_exit_code() {
    let mut h = harness("fail", false, false);
    h.load();
    let id = h.save(command_routine("Build", "build"))["id"].as_str().unwrap().to_owned();
    h.call("routine.run", args(&[("id", json!(id))])).unwrap();
    terminal_done(&mut h, "t-1", Some(3));
    let runs = h.rpc("routine.runs", Map::new()).unwrap();
    assert_eq!(runs["runs"][0]["status"]["state"], "failed");
    assert_eq!(runs["runs"][0]["status"]["reason"]["code"], 3);
    let _ = std::fs::remove_dir_all(h.dir.clone());
}

#[test]
fn a_prompt_runs_through_the_helper_and_finishes_with_its_status() {
    let mut h = harness("prompt", true, false);
    h.load();
    let routine = Routine {
        name: "Revisão".into(),
        target: Target::Fixed {
            bench_path: "C:/repo/feature".into(),
        },
        action: Action::Prompt {
            profile_id: Some("claude".into()),
            prompt: "Revise.".into(),
        },
        ..Routine::default()
    };
    let id = h.save(serde_json::to_value(routine).unwrap())["id"].as_str().unwrap().to_owned();
    h.call("routine.run", args(&[("id", json!(id))])).unwrap();
    assert_eq!(h.calls.lock().unwrap().helper[0]["bench_path"], "C:/repo/feature");
    assert_eq!(h.calls.lock().unwrap().helper[0]["profile_id"], "claude");

    h.runtime.emit(&HelperStatusChanged {
        bench_path: "C:/repo/feature".into(),
        terminal_id: Some("t-9".into()),
        profile_id: "claude".into(),
        status: HelperStatus::Finished,
    });
    let runs = h.rpc("routine.runs", Map::new()).unwrap();
    assert_eq!(runs["runs"][0]["status"]["state"], "succeeded");
    let _ = std::fs::remove_dir_all(h.dir.clone());
}

#[test]
fn a_missing_helper_fails_the_prompt_with_a_clear_reason() {
    let mut h = harness("no-helper", false, false);
    h.load();
    let routine = Routine {
        name: "Sem ajudante".into(),
        target: Target::Fixed {
            bench_path: "C:/repo".into(),
        },
        action: Action::Prompt {
            profile_id: None,
            prompt: "oi".into(),
        },
        ..Routine::default()
    };
    let id = h.save(serde_json::to_value(routine).unwrap())["id"].as_str().unwrap().to_owned();
    h.call("routine.run", args(&[("id", json!(id))])).unwrap();
    let runs = h.rpc("routine.runs", Map::new()).unwrap();
    assert_eq!(runs["runs"][0]["status"]["state"], "failed");
    assert_eq!(runs["runs"][0]["status"]["reason"]["kind"], "helper_unavailable");
    let _ = std::fs::remove_dir_all(h.dir.clone());
}

#[test]
fn a_new_bench_is_created_from_the_name_template_then_the_command_runs_there() {
    let mut h = harness("bench", false, true);
    h.load();
    let routine = Routine {
        name: "Diária".into(),
        target: Target::New {
            project_id: "p1".into(),
            project_path: "C:/projeto".into(),
            name_template: "rotina-{rotina}".into(),
        },
        action: Action::Command {
            command: "x".into(),
        },
        ..Routine::default()
    };
    let id = h.save(serde_json::to_value(routine).unwrap())["id"].as_str().unwrap().to_owned();
    h.call("routine.run", args(&[("id", json!(id))])).unwrap();
    {
        let calls = h.calls.lock().unwrap();
        assert_eq!(calls.bench[0]["project_id"], "p1");
        assert_eq!(calls.bench[0]["name"], "rotina-di-ria");
        assert_eq!(calls.opened[0]["cwd"], "C:/bench/nova");
    }
    let _ = std::fs::remove_dir_all(h.dir.clone());
}

#[test]
fn run_now_of_an_unknown_routine_is_an_error() {
    let mut h = harness("unknown", false, false);
    h.load();
    let error = h.call("routine.run", args(&[("id", json!("nope"))])).unwrap_err();
    assert!(error.contains("unknown routine"), "{error}");
    let rpc = h.rpc("routine.run", Map::new()).unwrap_err();
    assert!(rpc.contains("required"), "{rpc}");
    let _ = std::fs::remove_dir_all(h.dir.clone());
}

#[test]
fn the_tick_fires_a_due_routine_without_the_page_open() {
    let mut h = harness("tick", false, false);
    h.load();
    let now = crate::state::now_unix();
    let routine = Routine {
        id: "due".into(),
        name: "Minuto".into(),
        schedule: Schedule::Cron {
            expression: "* * * * *".into(),
        },
        target: Target::Fixed {
            bench_path: "C:/repo".into(),
        },
        action: Action::Command {
            command: "date".into(),
        },
        created_at: now - 3600,
        ..Routine::default()
    };
    {
        let mut state = h.shared.lock().unwrap();
        state.store.routines.push(routine);
        state.store.checked_at = Some(now - 90);
    }
    let mut cx = h.runtime.ctx("routine");
    actions::tick(&mut cx, &h.shared);
    let runs = h.rpc("routine.runs", Map::new()).unwrap();
    assert_eq!(runs["runs"].as_array().unwrap().len(), 1);
    assert_eq!(runs["runs"][0]["trigger"], "schedule");
    assert_eq!(h.calls.lock().unwrap().opened.len(), 1);
    let _ = std::fs::remove_dir_all(h.dir.clone());
}

#[test]
fn a_scheduled_routine_does_not_fire_twice_for_the_same_time() {
    let mut h = harness("dedupe", false, false);
    h.load();
    let now = crate::state::now_unix();
    {
        let mut state = h.shared.lock().unwrap();
        state.store.routines.push(Routine {
            id: "d".into(),
            name: "Dedup".into(),
            schedule: Schedule::Cron {
                expression: "* * * * *".into(),
            },
            target: Target::Fixed {
                bench_path: "C:/repo".into(),
            },
            action: Action::Command { command: "x".into() },
            created_at: now - 3600,
            ..Routine::default()
        });
        state.store.checked_at = Some(now - 90);
    }
    let mut cx = h.runtime.ctx("routine");
    actions::tick(&mut cx, &h.shared);
    // Mesmo horário, outra conferência logo depois: nada novo.
    h.shared.lock().unwrap().last_tick = None;
    actions::tick(&mut cx, &h.shared);
    assert_eq!(h.shared.lock().unwrap().store.runs.len(), 1);
    let _ = std::fs::remove_dir_all(h.dir.clone());
}

#[test]
fn a_file_that_cannot_be_read_is_never_overwritten() {
    let mut h = harness("corrupt", false, false);
    std::fs::create_dir_all(h.dir.join("modules")).unwrap();
    std::fs::write(h.store_file(), "{ isto não é json").unwrap();
    h.load();
    assert!(h.shared.lock().unwrap().load_failed);
    let refused = h.save(command_routine("Não grava", "x"));
    assert_eq!(refused["saved"], false, "saving is refused while the file is unreadable");
    let _ = std::fs::remove_dir_all(h.dir.clone());
}

#[test]
fn runs_left_open_when_the_app_closed_are_marked_interrupted() {
    let mut h = harness("interrupted", false, false);
    std::fs::create_dir_all(h.dir.join("modules")).unwrap();
    let mut store = Store::default();
    store.push_run(crate::model::Run {
        id: "old".into(),
        routine_id: "r".into(),
        status: RunStatus::Running,
        trigger: Trigger::Manual,
        started: 1,
        ..crate::model::Run::default()
    });
    std::fs::write(h.store_file(), serde_json::to_string(&store).unwrap()).unwrap();
    h.load();
    let state = h.shared.lock().unwrap();
    assert_eq!(
        state.store.runs[0].status,
        RunStatus::Failed {
            reason: Failure::Interrupted
        }
    );
    drop(state);
    let _ = std::fs::remove_dir_all(h.dir.clone());
}

#[test]
fn the_schedule_preset_round_trips_through_the_routine_json() {
    let routine = Routine {
        schedule: Schedule::Preset {
            preset: Preset::Weekly {
                weekday: 2,
                hour: 8,
                minute: 15,
            },
        },
        ..Routine::default()
    };
    let text = serde_json::to_string(&routine).unwrap();
    let back: Routine = serde_json::from_str(&text).unwrap();
    assert_eq!(back.schedule.expression(), "15 8 * * 2");
}

#[test]
fn rpc_save_and_run_work_end_to_end() {
    let mut h = harness("rpc-save", false, false);
    h.load();
    let saved = h
        .rpc(
            "routine.save",
            Map::from_iter([("routine".into(), command_routine("RPC Build", "echo ok"))]),
        )
        .expect("routine.save rpc succeeds");
    assert_eq!(saved["saved"], true);
    let id = saved["id"].as_str().unwrap().to_owned();
    let started = h
        .rpc("routine.run", Map::from_iter([("id".into(), json!(id))]))
        .expect("routine.run rpc succeeds");
    assert!(started["run_id"].is_string());
    terminal_done(&mut h, "t-1", Some(0));
    let runs = h.rpc("routine.runs", Map::new()).unwrap();
    assert_eq!(runs["runs"][0]["status"]["state"], "succeeded");
    let _ = std::fs::remove_dir_all(h.dir.clone());
}

