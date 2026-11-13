//! Modelo das rotinas: a definição, cada execução, o histórico e a validação.
//!
//! O arquivo do store guarda rotinas, execuções e o último instante verificado. O histórico
//! tem no máximo [`HISTORY_LIMIT`] execuções e some depois de `routine.logs.retention_days`.

use crate::schedule::{Preset, Schedule};
use serde::{Deserialize, Serialize};

/// Execuções guardadas no histórico, da mais antiga para fora.
pub const HISTORY_LIMIT: usize = 200;

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum Target {
    /// Sem alvo definido: uma execução automática para aqui e registra "defina um alvo".
    Ask,
    /// Uma bancada que já existe.
    Fixed { bench_path: String },
    /// Uma bancada nova por execução, criada com `bench.create`.
    New {
        project_id: String,
        project_path: String,
        /// Modelo do nome. `{rotina}` vira o nome da rotina; `{data}`, a data local (AAAA-MM-DD).
        name_template: String,
    },
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum Action {
    /// Prompt para um ajudante, via `helper.launch`.
    Prompt { profile_id: Option<String>, prompt: String },
    /// Um comando num terminal: `terminal.open` e depois `terminal.send` com a linha.
    Command { command: String },
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct Routine {
    pub id: String,
    pub name: String,
    pub enabled: bool,
    pub schedule: Schedule,
    pub target: Target,
    pub action: Action,
    /// Comando de pré-checagem. Se sair com código diferente de zero, a execução é pulada.
    pub precheck: Option<String>,
    /// Tolerância desta rotina, em segundos. Vazio usa `routine.schedule.grace_s`.
    pub tolerance_s: Option<u32>,
    /// Quando a rotina foi criada (unix). Disparos antes disso não contam.
    pub created_at: i64,
    /// Último disparo tratado (unix). Impede repetir o mesmo horário após reinício.
    pub last_fire: Option<i64>,
}

impl Default for Routine {
    fn default() -> Self {
        Self {
            id: String::new(),
            name: String::new(),
            enabled: true,
            schedule: Schedule::Preset {
                preset: Preset::Daily { hour: 9, minute: 0 },
            },
            target: Target::Ask,
            action: Action::Command {
                command: String::new(),
            },
            precheck: None,
            tolerance_s: None,
            created_at: 0,
            last_fire: None,
        }
    }
}

/// Por que o disparo foi feito.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Trigger {
    /// No horário, dentro da tolerância.
    Schedule,
    /// Depois de um atraso, com a política "rodar uma vez".
    CatchUp,
    /// Pelo botão "rodar agora" ou pelo CLI.
    Manual,
}

/// Motivo de uma falha. A tela traduz a variante; `Detail` é texto técnico vindo de fora.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum Failure {
    Timeout,
    CommandExit { code: i32 },
    TerminalUnavailable,
    HelperUnavailable,
    BenchUnavailable,
    TerminalClosed,
    /// O app fechou com a execução em andamento; ao abrir de novo, ela vira falha.
    Interrupted,
    Detail { message: String },
}

/// Motivo de uma execução pulada.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum Skip {
    PrecheckFailed { code: i32 },
    NeedsTarget,
    /// Horário perdido com a política "pular".
    MissedSkipped,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "state", rename_all = "snake_case")]
pub enum RunStatus {
    Queued,
    Running,
    Succeeded,
    Failed { reason: Failure },
    Skipped { reason: Skip },
    /// Horário perdido à espera de decisão (política "perguntar").
    Missed,
}

impl RunStatus {
    pub fn is_finished(&self) -> bool {
        !matches!(self, Self::Queued | Self::Running)
    }
}

/// Uma execução no histórico.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct Run {
    pub id: String,
    pub routine_id: String,
    pub routine_name: String,
    pub trigger: Trigger,
    pub status: RunStatus,
    /// Instante em que a execução foi registrada (unix).
    pub started: i64,
    pub finished: Option<i64>,
    pub bench_path: Option<String>,
    pub terminal_id: Option<String>,
    /// Saída resumida. É dado de terminal: nunca é tratada como instrução.
    pub summary: String,
}

impl Default for Run {
    fn default() -> Self {
        Self {
            id: String::new(),
            routine_id: String::new(),
            routine_name: String::new(),
            trigger: Trigger::Schedule,
            status: RunStatus::Queued,
            started: 0,
            finished: None,
            bench_path: None,
            terminal_id: None,
            summary: String::new(),
        }
    }
}

#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct Store {
    pub routines: Vec<Routine>,
    pub runs: Vec<Run>,
    /// Último instante em que a agenda foi conferida (unix). Janela de disparos perdidos.
    pub checked_at: Option<i64>,
}

impl Store {
    /// Acrescenta uma execução e corta o histórico no limite. As mais antigas saem primeiro.
    pub fn push_run(&mut self, run: Run) {
        self.runs.push(run);
        if self.runs.len() > HISTORY_LIMIT {
            let excess = self.runs.len() - HISTORY_LIMIT;
            self.runs.drain(..excess);
        }
    }

    /// Tira do histórico o que passou da retenção, a partir de `now` (unix).
    pub fn prune(&mut self, now: i64, retention_days: i64) {
        let cutoff = now - retention_days.max(1) * 86_400;
        self.runs.retain(|run| run.started >= cutoff);
    }

    pub fn routine(&self, id: &str) -> Option<&Routine> {
        self.routines.iter().find(|routine| routine.id == id)
    }

    pub fn routine_mut(&mut self, id: &str) -> Option<&mut Routine> {
        self.routines.iter_mut().find(|routine| routine.id == id)
    }

    /// Execuções de uma rotina, da mais recente para a mais antiga.
    pub fn runs_of(&self, routine_id: &str, limit: usize) -> Vec<&Run> {
        self.runs
            .iter()
            .rev()
            .filter(|run| run.routine_id == routine_id)
            .take(limit)
            .collect()
    }

    pub fn run_mut(&mut self, run_id: &str) -> Option<&mut Run> {
        self.runs.iter_mut().find(|run| run.id == run_id)
    }

    pub fn run(&self, run_id: &str) -> Option<&Run> {
        self.runs.iter().find(|run| run.id == run_id)
    }
}

/// Motivo de uma rotina recusada. A tela traduz pela chave `routine-error-<código>`.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Invalid {
    NameEmpty,
    Schedule(String),
    FixedBenchEmpty,
    NewFieldsEmpty,
    PromptEmpty,
    CommandEmpty,
    PrecheckEmpty,
    ToleranceOutOfRange,
}

impl Invalid {
    pub fn key(&self) -> &'static str {
        match self {
            Self::NameEmpty => "routine-error-name_empty",
            Self::Schedule(_) => "routine-error-schedule",
            Self::FixedBenchEmpty => "routine-error-fixed_bench_empty",
            Self::NewFieldsEmpty => "routine-error-new_fields_empty",
            Self::PromptEmpty => "routine-error-prompt_empty",
            Self::CommandEmpty => "routine-error-command_empty",
            Self::PrecheckEmpty => "routine-error-precheck_empty",
            Self::ToleranceOutOfRange => "routine-error-tolerance_out_of_range",
        }
    }
}

/// Confere uma rotina antes de gravar. Campos vazios viram erro antes de qualquer execução.
pub fn validate(routine: &Routine) -> Result<(), Invalid> {
    if routine.name.trim().is_empty() {
        return Err(Invalid::NameEmpty);
    }
    routine
        .schedule
        .compile()
        .map_err(Invalid::Schedule)?;
    match &routine.target {
        Target::Ask => {}
        Target::Fixed { bench_path } if bench_path.trim().is_empty() => return Err(Invalid::FixedBenchEmpty),
        Target::Fixed { .. } => {}
        Target::New {
            project_id,
            project_path,
            name_template,
        } if project_id.trim().is_empty() || project_path.trim().is_empty() || name_template.trim().is_empty() => {
            return Err(Invalid::NewFieldsEmpty)
        }
        Target::New { .. } => {}
    }
    match &routine.action {
        Action::Prompt { prompt, .. } if prompt.trim().is_empty() => return Err(Invalid::PromptEmpty),
        Action::Command { command } if command.trim().is_empty() => return Err(Invalid::CommandEmpty),
        _ => {}
    }
    if routine.precheck.as_deref().is_some_and(|text| text.trim().is_empty()) {
        return Err(Invalid::PrecheckEmpty);
    }
    if routine.tolerance_s.is_some_and(|seconds| seconds > 3600) {
        return Err(Invalid::ToleranceOutOfRange);
    }
    Ok(())
}

/// Chaves de erro que a tela pode mostrar, além das de [`Invalid::key`].
pub const USED_KEYS: &[&str] = &[
    "routine-error-name_empty",
    "routine-error-schedule",
    "routine-error-fixed_bench_empty",
    "routine-error-new_fields_empty",
    "routine-error-prompt_empty",
    "routine-error-command_empty",
    "routine-error-precheck_empty",
    "routine-error-tolerance_out_of_range",
    "routine-error-invalid",
    "routine-error-loading",
    "routine-error-time",
    "routine-error-tolerance",
];

/// Nome da bancada nova: o modelo com `{rotina}` e `{data}` trocados e só caracteres seguros.
pub fn bench_name(template: &str, routine_name: &str, date: &str) -> String {
    let filled = template.replace("{rotina}", routine_name).replace("{data}", date);
    let slug: String = filled
        .chars()
        .map(|c| {
            if c.is_ascii_alphanumeric() || c == '-' || c == '_' {
                c.to_ascii_lowercase()
            } else {
                '-'
            }
        })
        .collect();
    let mut out = String::new();
    for c in slug.chars() {
        if c == '-' && out.ends_with('-') {
            continue;
        }
        out.push(c);
    }
    let trimmed = out.trim_matches('-');
    if trimmed.is_empty() {
        "rotina".to_owned()
    } else {
        trimmed.chars().take(60).collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn routine() -> Routine {
        Routine {
            id: "r1".into(),
            name: "Resumo diário".into(),
            action: Action::Command {
                command: "echo ok".into(),
            },
            target: Target::Fixed {
                bench_path: "C:/repo".into(),
            },
            ..Routine::default()
        }
    }

    fn run(id: usize, routine_id: &str, started: i64) -> Run {
        Run {
            id: id.to_string(),
            routine_id: routine_id.into(),
            started,
            ..Run::default()
        }
    }

    #[test]
    fn history_keeps_the_newest_200_runs() {
        let mut store = Store::default();
        for n in 0..205 {
            store.push_run(run(n, "r1", n as i64));
        }
        assert_eq!(store.runs.len(), HISTORY_LIMIT);
        assert_eq!(store.runs[0].id, "5", "the five oldest are gone");
        let restored: Store = serde_json::from_str(&serde_json::to_string(&store).unwrap()).unwrap();
        assert_eq!(restored.runs.len(), HISTORY_LIMIT);
    }

    #[test]
    fn retention_drops_runs_older_than_the_setting() {
        let mut store = Store::default();
        let now = 100 * 86_400;
        store.push_run(run(1, "r1", now - 40 * 86_400));
        store.push_run(run(2, "r1", now - 2 * 86_400));
        store.prune(now, 30);
        assert_eq!(store.runs.len(), 1);
        assert_eq!(store.runs[0].id, "2");
    }

    #[test]
    fn runs_of_a_routine_come_newest_first_with_a_limit() {
        let mut store = Store::default();
        for n in 0..5 {
            store.push_run(run(n, if n % 2 == 0 { "r1" } else { "r2" }, n as i64));
        }
        let ids: Vec<&str> = store.runs_of("r1", 2).iter().map(|r| r.id.as_str()).collect();
        assert_eq!(ids, ["4", "2"]);
    }

    #[test]
    fn valid_routines_pass_and_each_empty_field_is_named() {
        assert!(validate(&routine()).is_ok());

        let mut bad = routine();
        bad.name = "  ".into();
        assert_eq!(validate(&bad), Err(Invalid::NameEmpty));

        let mut bad = routine();
        bad.schedule = Schedule::Cron {
            expression: "0 0 9 * * *".into(),
        };
        assert!(matches!(validate(&bad), Err(Invalid::Schedule(_))));

        let mut bad = routine();
        bad.target = Target::Fixed { bench_path: String::new() };
        assert_eq!(validate(&bad), Err(Invalid::FixedBenchEmpty));

        let mut bad = routine();
        bad.action = Action::Prompt {
            profile_id: None,
            prompt: " ".into(),
        };
        assert_eq!(validate(&bad), Err(Invalid::PromptEmpty));

        let mut bad = routine();
        bad.precheck = Some(String::new());
        assert_eq!(validate(&bad), Err(Invalid::PrecheckEmpty));

        let mut bad = routine();
        bad.tolerance_s = Some(7200);
        assert_eq!(validate(&bad), Err(Invalid::ToleranceOutOfRange));
    }

    #[test]
    fn bench_names_are_safe_slugs_with_the_placeholders_filled() {
        // "á" não é ASCII: vira hífen, e as barras duplicadas colapsam.
        assert_eq!(
            bench_name("rotina-{rotina}-{data}", "Resumo diário", "2026-11-09"),
            "rotina-resumo-di-rio-2026-11-09"
        );
        assert_eq!(bench_name("{rotina}", "  a/b\\c  ", "x"), "a-b-c");
        assert_eq!(bench_name("{rotina}", "!!!", "x"), "rotina");
        assert!(bench_name("{rotina}", &"x".repeat(200), "d").len() <= 60);
    }

    #[test]
    fn statuses_know_when_they_are_finished() {
        assert!(!RunStatus::Running.is_finished());
        assert!(RunStatus::Succeeded.is_finished());
        assert!(RunStatus::Missed.is_finished());
    }
}
