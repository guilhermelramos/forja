//! Máquina de estados das execuções. Não faz IO.
//!
//! Cada passo devolve [`Effect`]s: pedidos a outros módulos (`terminal.open`, `helper.launch`,
//! `bench.create`), à pré-checagem (um processo) ou ao aviso de falha. A camada de comandos
//! executa os efeitos e devolve os resultados por `on_*`. Assim o fluxo inteiro é testado com
//! comandos falsos, sem terminal nem processo.

use crate::model::{self, Action, Failure, Routine, Run, RunStatus, Skip, Store, Target, Trigger};
use crate::schedule::{due, Due};
use chrono::TimeZone;
use std::collections::VecDeque;

/// Política para um horário perdido (`routine.schedule.missed`).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum MissedPolicy {
    Skip,
    RunOnce,
    Ask,
}

/// Configurações que a máquina de estados usa, lidas da thread da interface a cada passo.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Settings {
    pub concurrency: usize,
    pub timeout_s: i64,
    pub notify_failure: bool,
    pub notify_success: bool,
    /// Data local (AAAA-MM-DD), para o nome das bancadas novas.
    pub today: String,
    pub grace_s: i64,
    pub missed: MissedPolicy,
}

/// Efeito pedido pela máquina. A camada de comandos executa e devolve o resultado.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Effect {
    Precheck { run_id: String, command: String, cwd: String },
    CreateBench { run_id: String, project_id: String, name: String },
    OpenTerminal { run_id: String, cwd: String, title: String },
    SendCommand { run_id: String, terminal_id: String, text: String },
    LaunchHelper { run_id: String, bench_path: String, profile_id: Option<String>, prompt: String },
    ReadSummary { run_id: String, terminal_id: String },
    Notify { run_id: String, failed: bool },
    /// Pedido de gravação do store em disco.
    Persist,
}

#[derive(Clone, Debug, PartialEq, Eq)]
enum Stage {
    Queued,
    Precheck,
    CreatingBench { name: String },
    WaitingBench { name: String },
    OpeningTerminal,
    SendingCommand { terminal_id: String },
    LaunchingHelper,
    Terminal { terminal_id: String },
    Helper { bench_path: String },
}

#[derive(Clone, Debug)]
struct Job {
    run_id: String,
    routine: Routine,
    stage: Stage,
    /// Instante em que a execução começou a rodar (unix), para o tempo limite.
    running_since: i64,
}

#[derive(Debug, Default)]
pub struct Runner {
    active: Vec<Job>,
    queue: VecDeque<Job>,
    seq: u64,
}

impl Runner {
    #[cfg(test)]
    pub fn new() -> Self {
        Self::default()
    }

    #[cfg(test)]
    pub fn is_busy(&self) -> bool {
        !self.active.is_empty() || !self.queue.is_empty()
    }

    pub fn running(&self) -> usize {
        self.active.len()
    }

    pub fn queued(&self) -> usize {
        self.queue.len()
    }

    /// Registra uma execução e a põe na fila. Devolve os efeitos do que pôde começar já.
    pub fn enqueue(&mut self, store: &mut Store, routine: &Routine, trigger: Trigger, now: i64, settings: &Settings) -> Vec<Effect> {
        let run_id = self.next_id(now);
        store.push_run(Run {
            id: run_id.clone(),
            routine_id: routine.id.clone(),
            routine_name: routine.name.clone(),
            trigger,
            status: RunStatus::Queued,
            started: now,
            ..Run::default()
        });
        self.queue.push_back(Job {
            run_id,
            routine: routine.clone(),
            stage: Stage::Queued,
            running_since: now,
        });
        self.pump(store, now, settings)
    }

    /// Registra um horário perdido sem rodar nada: "pular" ou "perguntar".
    pub fn record_finished(&mut self, store: &mut Store, routine: &Routine, trigger: Trigger, status: RunStatus, now: i64) {
        let run_id = self.next_id(now);
        store.push_run(Run {
            id: run_id,
            routine_id: routine.id.clone(),
            routine_name: routine.name.clone(),
            trigger,
            status,
            started: now,
            finished: Some(now),
            ..Run::default()
        });
    }

    /// Passo periódico: dispara o que venceu desde a última conferência e confere o tempo limite.
    pub fn check_schedules<Tz: TimeZone>(
        &mut self,
        store: &mut Store,
        zone: &Tz,
        now: i64,
        settings: &Settings,
    ) -> Vec<Effect> {
        let mut effects = Vec::new();
        let Some(now_at) = zone.timestamp_opt(now, 0).single() else {
            return effects;
        };
        // Primeira conferência: a janela começa agora, então nada antigo dispara por engano.
        let checked = store.checked_at.unwrap_or(now);
        let routines = store.routines.clone();
        for routine in routines.iter().filter(|routine| routine.enabled) {
            let Ok(spec) = routine.schedule.compile() else {
                continue;
            };
            let mut window = checked.max(routine.created_at);
            if let Some(last) = routine.last_fire {
                window = window.max(last);
            }
            let Some(window_start) = zone.timestamp_opt(window, 0).single() else {
                continue;
            };
            let grace = routine.tolerance_s.map_or(settings.grace_s, i64::from);
            match due(&spec, &window_start, &now_at, grace) {
                Due::Nothing => {}
                Due::OnTime { fire } => {
                    set_last_fire(store, &routine.id, fire.timestamp());
                    effects.extend(self.enqueue(store, routine, Trigger::Schedule, now, settings));
                }
                Due::Late { fire, .. } => {
                    set_last_fire(store, &routine.id, fire.timestamp());
                    match settings.missed {
                        MissedPolicy::Skip => self.record_finished(
                            store,
                            routine,
                            Trigger::Schedule,
                            RunStatus::Skipped {
                                reason: Skip::MissedSkipped,
                            },
                            now,
                        ),
                        MissedPolicy::RunOnce => {
                            effects.extend(self.enqueue(store, routine, Trigger::CatchUp, now, settings));
                        }
                        MissedPolicy::Ask => {
                            self.record_finished(store, routine, Trigger::Schedule, RunStatus::Missed, now);
                        }
                    }
                }
            }
        }
        store.checked_at = Some(now);
        effects.extend(self.check_timeouts(store, now, settings));
        effects
    }

    /// Termina as execuções que passaram do tempo limite e libera a fila.
    pub fn check_timeouts(&mut self, store: &mut Store, now: i64, settings: &Settings) -> Vec<Effect> {
        let expired: Vec<String> = self
            .active
            .iter()
            .filter(|job| now - job.running_since > settings.timeout_s)
            .map(|job| job.run_id.clone())
            .collect();
        let mut effects = Vec::new();
        for run_id in expired {
            effects.extend(self.finish(store, &run_id, RunStatus::Failed { reason: Failure::Timeout }, now, settings));
        }
        effects.extend(self.pump(store, now, settings));
        effects
    }

    /// Resultado da pré-checagem: `Ok(código)` do processo, ou erro ao iniciá-lo.
    pub fn on_precheck(&mut self, store: &mut Store, run_id: &str, outcome: Result<i32, String>, now: i64, settings: &Settings) -> Vec<Effect> {
        let mut effects = Vec::new();
        match outcome {
            Ok(0) => effects.extend(self.advance(store, run_id, now, settings)),
            Ok(code) => effects.extend(self.finish(
                store,
                run_id,
                RunStatus::Skipped {
                    reason: Skip::PrecheckFailed { code },
                },
                now,
                settings,
            )),
            Err(message) => effects.extend(self.finish(
                store,
                run_id,
                RunStatus::Failed {
                    reason: Failure::Detail { message },
                },
                now,
                settings,
            )),
        }
        effects.extend(self.pump(store, now, settings));
        effects
    }

    /// Resultado de `bench.create`: `Ok(Some(caminho))` quando a bancada já existe; `Ok(None)`
    /// quando ela chega depois, por `bench.created`; ou a falha.
    pub fn on_bench(&mut self, store: &mut Store, run_id: &str, outcome: Result<Option<String>, Failure>, now: i64, settings: &Settings) -> Vec<Effect> {
        let mut effects = Vec::new();
        match outcome {
            Ok(Some(path)) => effects.extend(self.bench_ready(store, run_id, path)),
            Ok(None) => {
                let waiting = match self.job(run_id).map(|job| job.stage.clone()) {
                    Some(Stage::CreatingBench { name }) => Stage::WaitingBench { name },
                    _ => Stage::WaitingBench { name: String::new() },
                };
                self.set_stage(run_id, waiting);
            }
            Err(reason) => effects.extend(self.finish(store, run_id, RunStatus::Failed { reason }, now, settings)),
        }
        effects.extend(self.pump(store, now, settings));
        effects
    }

    /// Chegou `bench.created` com o nome que esta execução pediu.
    pub fn on_bench_created(&mut self, store: &mut Store, name: &str, path: &str, now: i64, settings: &Settings) -> Vec<Effect> {
        let waiting = self
            .active
            .iter()
            .find(|job| matches!(&job.stage, Stage::WaitingBench { name: wanted } if wanted == name))
            .map(|job| job.run_id.clone());
        let Some(run_id) = waiting else {
            return Vec::new();
        };
        let mut effects = self.bench_ready(store, &run_id, path.to_owned());
        effects.extend(self.pump(store, now, settings));
        effects
    }

    /// Resultado de `terminal.open`.
    pub fn on_terminal_opened(&mut self, store: &mut Store, run_id: &str, outcome: Result<String, Failure>, now: i64, settings: &Settings) -> Vec<Effect> {
        let mut effects = Vec::new();
        match outcome {
            Ok(terminal_id) => {
                if let Some(run) = store.run_mut(run_id) {
                    run.terminal_id = Some(terminal_id.clone());
                }
                let text = match self.job_mut(run_id).map(|job| job.routine.action.clone()) {
                    Some(Action::Command { command }) => command,
                    _ => String::new(),
                };
                if let Some(job) = self.job_mut(run_id) {
                    job.stage = Stage::SendingCommand {
                        terminal_id: terminal_id.clone(),
                    };
                }
                effects.push(Effect::SendCommand {
                    run_id: run_id.to_owned(),
                    terminal_id,
                    text,
                });
            }
            Err(reason) => effects.extend(self.finish(store, run_id, RunStatus::Failed { reason }, now, settings)),
        }
        effects.extend(self.pump(store, now, settings));
        effects
    }

    /// Resultado de `terminal.send`. Daqui em diante a execução espera o fim do comando.
    pub fn on_command_sent(&mut self, store: &mut Store, run_id: &str, outcome: Result<(), Failure>, now: i64, settings: &Settings) -> Vec<Effect> {
        let mut effects = Vec::new();
        let terminal = self.job_mut(run_id).and_then(|job| match &job.stage {
            Stage::SendingCommand { terminal_id } => Some(terminal_id.clone()),
            _ => None,
        });
        match (outcome, terminal) {
            (Ok(()), Some(terminal_id)) => {
                if let Some(job) = self.job_mut(run_id) {
                    job.stage = Stage::Terminal { terminal_id };
                }
            }
            (Err(reason), _) => effects.extend(self.finish(store, run_id, RunStatus::Failed { reason }, now, settings)),
            (Ok(()), None) => {}
        }
        effects.extend(self.pump(store, now, settings));
        effects
    }

    /// Resultado de `helper.launch`. A execução espera o ajudante terminar naquela bancada.
    pub fn on_helper_launched(&mut self, store: &mut Store, run_id: &str, outcome: Result<(), Failure>, now: i64, settings: &Settings) -> Vec<Effect> {
        let mut effects = Vec::new();
        match outcome {
            Ok(()) => {
                let bench_path = store.run(run_id).and_then(|run| run.bench_path.clone()).unwrap_or_default();
                if let Some(job) = self.job_mut(run_id) {
                    job.stage = Stage::Helper { bench_path };
                }
            }
            Err(reason) => effects.extend(self.finish(store, run_id, RunStatus::Failed { reason }, now, settings)),
        }
        effects.extend(self.pump(store, now, settings));
        effects
    }

    /// Terminal de uma execução terminou. `exited` diz se foi o processo que saiu.
    pub fn on_terminal_finished(&mut self, store: &mut Store, terminal_id: &str, exit_code: Option<i32>, exited: bool, now: i64, settings: &Settings) -> Vec<Effect> {
        let run_id = self
            .active
            .iter()
            .find(|job| matches!(&job.stage, Stage::Terminal { terminal_id: id } if id == terminal_id))
            .map(|job| job.run_id.clone());
        let Some(run_id) = run_id else {
            return Vec::new();
        };
        let status = match (exit_code, exited) {
            (Some(0), _) => RunStatus::Succeeded,
            (Some(code), _) => RunStatus::Failed {
                reason: Failure::CommandExit { code },
            },
            (None, true) => RunStatus::Failed {
                reason: Failure::TerminalClosed,
            },
            (None, false) => RunStatus::Succeeded,
        };
        let mut effects = self.finish(store, &run_id, status, now, settings);
        effects.extend(self.pump(store, now, settings));
        effects
    }

    /// Status de um ajudante mudou. `Finished` na bancada desta execução a conclui.
    pub fn on_helper_status(&mut self, store: &mut Store, bench_path: &str, finished: bool, now: i64, settings: &Settings) -> Vec<Effect> {
        if !finished {
            return Vec::new();
        }
        let run_id = self
            .active
            .iter()
            .find(|job| matches!(&job.stage, Stage::Helper { bench_path: path } if path == bench_path))
            .map(|job| job.run_id.clone());
        let Some(run_id) = run_id else {
            return Vec::new();
        };
        let mut effects = self.finish(store, &run_id, RunStatus::Succeeded, now, settings);
        effects.extend(self.pump(store, now, settings));
        effects
    }

    /// Após `ReadSummary`: guarda o resumo no histórico.
    pub fn on_summary(&self, store: &mut Store, run_id: &str, output: &str) {
        if let Some(run) = store.run_mut(run_id) {
            run.summary = crate::execution::summary(output);
        }
    }

    fn pump(&mut self, store: &mut Store, now: i64, settings: &Settings) -> Vec<Effect> {
        let mut effects = Vec::new();
        while self.active.len() < settings.concurrency.max(1) {
            let Some(mut job) = self.queue.pop_front() else {
                break;
            };
            job.running_since = now;
            if let Some(run) = store.run_mut(&job.run_id) {
                run.status = RunStatus::Running;
            }
            effects.extend(self.start(store, job, now, settings));
        }
        effects
    }

    fn start(&mut self, store: &mut Store, job: Job, now: i64, settings: &Settings) -> Vec<Effect> {
        if matches!(job.routine.target, Target::Ask) {
            let run_id = job.run_id.clone();
            self.active.push(job);
            return self.finish(
                store,
                &run_id,
                RunStatus::Skipped {
                    reason: Skip::NeedsTarget,
                },
                now,
                settings,
            );
        }
        let run_id = job.run_id.clone();
        let precheck = job.routine.precheck.clone();
        let cwd = target_cwd(&job.routine.target);
        self.active.push(job);
        match (precheck, cwd) {
            (Some(command), Some(cwd)) => {
                self.set_stage(&run_id, Stage::Precheck);
                vec![Effect::Precheck { run_id, command, cwd }]
            }
            _ => self.advance(store, &run_id, now, settings),
        }
    }

    /// Resolve o alvo (criando a bancada, se for nova) e segue para a ação.
    fn advance(&mut self, store: &mut Store, run_id: &str, now: i64, settings: &Settings) -> Vec<Effect> {
        let Some(job) = self.job(run_id).cloned() else {
            return Vec::new();
        };
        match job.routine.target.clone() {
            Target::Ask => self.finish(
                store,
                run_id,
                RunStatus::Skipped {
                    reason: Skip::NeedsTarget,
                },
                now,
                settings,
            ),
            Target::Fixed { bench_path } => {
                if let Some(run) = store.run_mut(run_id) {
                    run.bench_path = Some(bench_path.clone());
                }
                self.act(run_id, bench_path)
            }
            Target::New {
                project_id,
                name_template,
                ..
            } => {
                let name = model::bench_name(&name_template, &job.routine.name, &settings.today);
                self.set_stage(run_id, Stage::CreatingBench { name: name.clone() });
                vec![Effect::CreateBench {
                    run_id: run_id.to_owned(),
                    project_id,
                    name,
                }]
            }
        }
    }

    /// Bancada pronta: segue para a ação dentro dela.
    fn bench_ready(&mut self, store: &mut Store, run_id: &str, path: String) -> Vec<Effect> {
        if let Some(run) = store.run_mut(run_id) {
            run.bench_path = Some(path.clone());
        }
        self.act(run_id, path)
    }

    fn act(&mut self, run_id: &str, bench_path: String) -> Vec<Effect> {
        let Some(job) = self.job(run_id).cloned() else {
            return Vec::new();
        };
        match job.routine.action {
            Action::Prompt { profile_id, prompt } => {
                self.set_stage(run_id, Stage::LaunchingHelper);
                vec![Effect::LaunchHelper {
                    run_id: run_id.to_owned(),
                    bench_path,
                    profile_id,
                    prompt,
                }]
            }
            Action::Command { .. } => {
                self.set_stage(run_id, Stage::OpeningTerminal);
                vec![Effect::OpenTerminal {
                    run_id: run_id.to_owned(),
                    cwd: bench_path,
                    title: job.routine.name.clone(),
                }]
            }
        }
    }

    /// Termina uma execução: remove da lista ativa, grava o status e pede avisos e resumo.
    fn finish(&mut self, store: &mut Store, run_id: &str, status: RunStatus, now: i64, settings: &Settings) -> Vec<Effect> {
        self.active.retain(|job| job.run_id != run_id);
        self.queue.retain(|job| job.run_id != run_id);
        let mut effects = Vec::new();
        let mut terminal = None;
        if let Some(run) = store.run_mut(run_id) {
            run.status = status.clone();
            run.finished = Some(now);
            terminal = run.terminal_id.clone();
        }
        if let Some(terminal_id) = terminal {
            effects.push(Effect::ReadSummary {
                run_id: run_id.to_owned(),
                terminal_id,
            });
        }
        match status {
            RunStatus::Failed { .. } if settings.notify_failure => effects.push(Effect::Notify {
                run_id: run_id.to_owned(),
                failed: true,
            }),
            RunStatus::Succeeded if settings.notify_success => effects.push(Effect::Notify {
                run_id: run_id.to_owned(),
                failed: false,
            }),
            _ => {}
        }
        effects.push(Effect::Persist);
        effects
    }

    fn job(&self, run_id: &str) -> Option<&Job> {
        self.active
            .iter()
            .find(|job| job.run_id == run_id)
            .or_else(|| self.queue.iter().find(|job| job.run_id == run_id))
    }

    fn job_mut(&mut self, run_id: &str) -> Option<&mut Job> {
        if let Some(index) = self.active.iter().position(|job| job.run_id == run_id) {
            return self.active.get_mut(index);
        }
        self.queue.iter_mut().find(|job| job.run_id == run_id)
    }

    fn set_stage(&mut self, run_id: &str, stage: Stage) {
        if let Some(job) = self.job_mut(run_id) {
            job.stage = stage;
        }
    }

    fn next_id(&mut self, now: i64) -> String {
        self.seq += 1;
        format!("run-{now}-{}", self.seq)
    }
}

fn target_cwd(target: &Target) -> Option<String> {
    match target {
        Target::Ask => None,
        Target::Fixed { bench_path } => Some(bench_path.clone()),
        Target::New { project_path, .. } => Some(project_path.clone()),
    }
}

fn set_last_fire(store: &mut Store, routine_id: &str, fire: i64) {
    if let Some(routine) = store.routine_mut(routine_id) {
        routine.last_fire = Some(fire);
    }
}

#[cfg(test)]
mod tests;
