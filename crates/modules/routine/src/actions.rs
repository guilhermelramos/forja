//! Executor: roda os efeitos da máquina de estados e devolve os resultados.
//!
//! Comandos de outros módulos (`bench.create`, `terminal.*`, `helper.launch`) rodam na thread
//! da interface, como todo comando; eles respondem na hora. A pré-checagem é um processo, então
//! roda em `Ctx::spawn`. Disco também fica em `spawn`.

use crate::model::{Failure, RunStatus, Store};
use crate::runner::{Effect, Settings};
use crate::state::{self, retention_days, settings, with, Shared, State};
use forja_contracts::{NotifyLevel, NotifyRequested};
use forja_kernel::{Args, Ctx, KernelError};
use serde_json::{json, Value};
use std::path::Path;
use std::process::{Command, Stdio};
use std::sync::Arc;
use std::time::{Duration, SystemTime};

/// Linhas de saída pedidas ao terminal quando a execução termina.
const SUMMARY_READ_LINES: u64 = 40;
/// Intervalo entre gravações automáticas quando nada mudou de forma relevante.
const PERSIST_EVERY: Duration = Duration::from_secs(30);
/// Intervalo mínimo entre passos periódicos.
const TICK_EVERY: Duration = Duration::from_millis(900);

/// Aplica uma mudança ao estado e executa os efeitos que ela pede.
pub fn apply(cx: &mut Ctx<'_>, shared: &Shared, change: impl FnOnce(&mut State, i64, &Settings) -> Vec<Effect>) {
    let settings = settings(cx);
    let now = state::now_unix();
    let effects = with(shared, |state| change(state, now, &settings)).unwrap_or_default();
    execute(cx, shared, effects);
}

/// Executa os efeitos em ordem. Cada resultado é devolvido ao estado por `apply`.
pub fn execute(cx: &mut Ctx<'_>, shared: &Shared, effects: Vec<Effect>) {
    for effect in effects {
        match effect {
            Effect::Precheck { run_id, command, cwd } => {
                let shared = Arc::clone(shared);
                cx.spawn(move || run_precheck(&command, &cwd), move |cx, outcome| {
                    apply(cx, &shared, move |state, now, settings| {
                        state.runner.on_precheck(&mut state.store, &run_id, outcome, now, settings)
                    });
                });
            }
            Effect::CreateBench { run_id, project_id, name } => {
                let mut args = Args::new();
                args.insert("project_id", json!(project_id));
                args.insert("name", json!(name));
                let outcome = match cx.run_command("bench.create", args) {
                    Ok(value) => Ok(value.get("path").and_then(Value::as_str).map(str::to_owned)),
                    Err(error) => Err(unavailable(error, Failure::BenchUnavailable)),
                };
                apply(cx, shared, move |state, now, settings| {
                    state.runner.on_bench(&mut state.store, &run_id, outcome, now, settings)
                });
            }
            Effect::OpenTerminal { run_id, cwd, title } => {
                let mut args = Args::new();
                args.insert("cwd", json!(cwd));
                args.insert("title", json!(title));
                let outcome = match cx.run_command("terminal.open", args) {
                    Ok(value) => id_text(&value["terminal_id"]).ok_or(Failure::Detail {
                        message: "terminal.open returned no terminal id".into(),
                    }),
                    Err(error) => Err(unavailable(error, Failure::TerminalUnavailable)),
                };
                apply(cx, shared, move |state, now, settings| {
                    state.runner.on_terminal_opened(&mut state.store, &run_id, outcome, now, settings)
                });
            }
            Effect::SendCommand { run_id, terminal_id, text } => {
                let mut args = Args::new();
                args.insert("terminal_id", json!(terminal_id));
                args.insert("text", json!(text));
                args.insert("enter", json!(true));
                let outcome = match cx.run_command("terminal.send", args) {
                    Ok(_) => Ok(()),
                    Err(error) => Err(unavailable(error, Failure::TerminalUnavailable)),
                };
                apply(cx, shared, move |state, now, settings| {
                    state.runner.on_command_sent(&mut state.store, &run_id, outcome, now, settings)
                });
            }
            Effect::LaunchHelper {
                run_id,
                bench_path,
                profile_id,
                prompt,
            } => {
                let mut args = Args::new();
                args.insert("bench_path", json!(bench_path));
                args.insert("prompt", json!(prompt));
                if let Some(profile) = profile_id {
                    args.insert("profile_id", json!(profile));
                }
                let outcome = match cx.run_command("helper.launch", args) {
                    Ok(_) => Ok(()),
                    Err(error) => Err(unavailable(error, Failure::HelperUnavailable)),
                };
                apply(cx, shared, move |state, now, settings| {
                    state.runner.on_helper_launched(&mut state.store, &run_id, outcome, now, settings)
                });
            }
            Effect::ReadSummary { run_id, terminal_id } => {
                let mut args = Args::new();
                args.insert("terminal_id", json!(terminal_id));
                args.insert("lines", json!(SUMMARY_READ_LINES));
                if let Ok(value) = cx.run_command("terminal.read", args) {
                    let text = value["text"].as_str().unwrap_or_default().to_owned();
                    with(shared, |state| state.runner.on_summary(&mut state.store, &run_id, &text));
                }
            }
            Effect::Notify { run_id, failed } => notify(cx, shared, &run_id, failed),
            Effect::Persist => persist(cx, shared),
        }
    }
}

/// Aviso de conclusão ou falha, pelo barramento de notificações.
fn notify(cx: &mut Ctx<'_>, shared: &Shared, run_id: &str, failed: bool) {
    let Some((name, status, bench_path)) = with(shared, |state| {
        state
            .store
            .run(run_id)
            .map(|run| (run.routine_name.clone(), run.status.clone(), run.bench_path.clone()))
    })
    .flatten() else {
        return;
    };
    let title = cx.t(if failed {
        "routine-notify-failed-title"
    } else {
        "routine-notify-success-title"
    });
    let status_text = cx.t(state::status_key(&status));
    let body = cx.t_args(
        "routine-notify-body",
        &[("routine", name.as_str()), ("status", status_text.as_str())],
    );
    let event = NotifyRequested {
        title,
        body,
        level: if failed {
            NotifyLevel::Attention
        } else {
            NotifyLevel::Success
        },
        bench_path,
    };
    let _ = cx.emit(&event);
}

/// Carrega o arquivo do store uma vez, em segundo plano. Execuções que estavam em andamento
/// quando o app fechou viram falha: não existe nada rodando de verdade para elas.
pub fn ensure_loaded(cx: &mut Ctx<'_>, shared: &Shared) {
    let start = with(shared, |state| {
        if state.loaded || state.loading {
            false
        } else {
            state.loading = true;
            true
        }
    })
    .unwrap_or(false);
    if !start {
        return;
    }
    let path = cx.store().path();
    let shared = Arc::clone(shared);
    cx.spawn(move || load_file(&path), move |_cx, result| {
        let now = state::now_unix();
        with(&shared, |state| {
            state.loading = false;
            state.loaded = true;
            match result {
                Ok(store) if store.routines.is_empty() && state::fixtures_enabled() => {
                    state.store = crate::fixtures::store(now);
                    state.fixture = true;
                }
                Ok(store) => state.store = store,
                Err(_) => {
                    state.load_failed = true;
                    state.notice = Some("routine-notice-load-failed".to_owned());
                }
            }
            for run in state.store.runs.iter_mut().filter(|run| !run.status.is_finished()) {
                run.status = RunStatus::Failed {
                    reason: Failure::Interrupted,
                };
                run.finished = Some(now);
            }
        });
    });
}

/// Passo periódico, chamado pela barra de status a cada quadro. Limita a um passo por segundo.
pub fn tick(cx: &mut Ctx<'_>, shared: &Shared) {
    ensure_loaded(cx, shared);
    let ready = with(shared, |state| {
        state.loaded
            && !state.load_failed
            && state
                .last_tick
                .is_none_or(|at| at.elapsed().map_or(true, |age| age >= TICK_EVERY))
    })
    .unwrap_or(false);
    if !ready {
        return;
    }
    let settings = settings(cx);
    let now = state::now_unix();
    let scheduling = state::scheduling_enabled(cx);
    let utc = state::schedule_in_utc(cx);
    let effects = with(shared, |state| {
        state.last_tick = Some(SystemTime::now());
        if !scheduling {
            return state.runner.check_timeouts(&mut state.store, now, &settings);
        }
        if utc {
            state.runner.check_schedules(&mut state.store, &chrono::Utc, now, &settings)
        } else {
            state.runner.check_schedules(&mut state.store, &chrono::Local, now, &settings)
        }
    })
    .unwrap_or_default();
    let throttled = with(shared, |state| {
        state
            .last_persist
            .is_none_or(|at| at.elapsed().map_or(true, |age| age >= PERSIST_EVERY))
    })
    .unwrap_or(false);
    let changed = !effects.is_empty();
    execute(cx, shared, effects);
    if changed || throttled {
        persist(cx, shared);
    }
}

/// Grava o store em segundo plano, com a retenção aplicada. Não grava se a leitura falhou.
pub fn persist(cx: &mut Ctx<'_>, shared: &Shared) {
    let retention = retention_days(cx);
    let now = state::now_unix();
    let Some(snapshot) = with(shared, |state| {
        if state.load_failed || !state.loaded || state.fixture {
            return None;
        }
        if state.persisting {
            state.persist_queued = true;
            return None;
        }
        state.store.prune(now, retention);
        state.last_persist = Some(SystemTime::now());
        state.persisting = true;
        Some(state.store.clone())
    }).flatten() else {
        return;
    };
    let path = cx.store().path();
    let shared = Arc::clone(shared);
    cx.spawn(move || save_file(&path, &snapshot), move |cx, result| {
        let queued = with(&shared, |state| {
            state.persisting = false;
            if result.is_err() {
                state.notice = Some("routine-notice-save-failed".to_owned());
            }
            std::mem::take(&mut state.persist_queued)
        }).unwrap_or(false);
        if queued {
            persist(cx, &shared);
        }
    });
}

/// Evento de terminal: um comando terminou (ou o terminal fechou).
pub fn terminal_done(cx: &mut Ctx<'_>, shared: &Shared, terminal_id: &str, exit_code: Option<i32>, exited: bool) {
    apply(cx, shared, |state, now, settings| {
        state
            .runner
            .on_terminal_finished(&mut state.store, terminal_id, exit_code, exited, now, settings)
    });
}

/// Evento de ajudante: o status mudou na bancada.
pub fn helper_status(cx: &mut Ctx<'_>, shared: &Shared, bench_path: &str, finished: bool) {
    apply(cx, shared, |state, now, settings| {
        state.runner.on_helper_status(&mut state.store, bench_path, finished, now, settings)
    });
}

/// Evento de bancada criada: destrava a execução que esperava por esse nome.
pub fn bench_created(cx: &mut Ctx<'_>, shared: &Shared, name: &str, path: &str) {
    apply(cx, shared, |state, now, settings| {
        state.runner.on_bench_created(&mut state.store, name, path, now, settings)
    });
}

/// Roda o comando de pré-checagem no diretório da bancada. Código de saída ou erro ao iniciar.
pub fn run_precheck(command: &str, cwd: &str) -> Result<i32, String> {
    let mut process = if cfg!(windows) {
        let mut process = forja_kernel::process::command("cmd.exe");
        process.arg("/C").arg(command);
        process
    } else {
        let mut process = forja_kernel::process::command("sh");
        process.arg("-c").arg(command);
        process
    };
    process
        .current_dir(cwd)
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null());
    no_window(&mut process);
    process
        .status()
        .map(|status| status.code().unwrap_or(-1))
        .map_err(|error| error.to_string())
}

#[cfg(windows)]
fn no_window(command: &mut Command) {
    use std::os::windows::process::CommandExt;
    const CREATE_NO_WINDOW: u32 = 0x0800_0000;
    command.creation_flags(CREATE_NO_WINDOW);
}

#[cfg(not(windows))]
fn no_window(_command: &mut Command) {}

/// Erro de um comando: "não existe" vira a falha de módulo ausente; o resto vira detalhe.
fn unavailable(error: KernelError, missing: Failure) -> Failure {
    match error {
        KernelError::UnknownCommand(_) => missing,
        other => Failure::Detail {
            message: other.to_string(),
        },
    }
}

/// Id de terminal vem como texto ou número, conforme o módulo que o criou.
fn id_text(value: &Value) -> Option<String> {
    match value {
        Value::String(text) if !text.is_empty() => Some(text.clone()),
        Value::Number(number) => Some(number.to_string()),
        _ => None,
    }
}

pub fn load_file(path: &Path) -> Result<Store, String> {
    match std::fs::read(path) {
        Ok(bytes) => serde_json::from_slice(&bytes).map_err(|error| error.to_string()),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(Store::default()),
        Err(error) => Err(error.to_string()),
    }
}

pub fn save_file(path: &Path, store: &Store) -> Result<(), String> {
    let bytes = serde_json::to_vec_pretty(store).map_err(|error| error.to_string())?;
    forja_kernel::store::write_atomic_bytes(path, &bytes).map_err(|error| error.to_string())
}
