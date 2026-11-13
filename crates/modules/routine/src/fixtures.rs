//! Dados de captura de tela (`FORJA_FIXTURES=1`): rotinas e histórico de exemplo.
//! Só aparecem quando o arquivo real não tem rotinas, e nunca são gravados no arquivo real.

use crate::model::{Action, Failure, Routine, Run, RunStatus, Skip, Store, Target, Trigger};
use crate::schedule::{Preset, Schedule};

/// Loja de exemplo, com horários relativos a `now` (unix) para o histórico parecer recente.
pub fn store(now: i64) -> Store {
    let day = 86_400;
    let routines = vec![
        Routine {
            id: "fixture-report".into(),
            name: "Resumo diário do quadro".into(),
            enabled: true,
            schedule: Schedule::Preset {
                preset: Preset::Daily { hour: 9, minute: 0 },
            },
            target: Target::Fixed {
                bench_path: "C:/projetos/forja/principal".into(),
            },
            action: Action::Command {
                command: "npm run report".into(),
            },
            created_at: now - 30 * day,
            ..Routine::default()
        },
        Routine {
            id: "fixture-deps".into(),
            name: "Revisão de dependências".into(),
            enabled: true,
            schedule: Schedule::Cron {
                expression: "0 8 * * 1-5".into(),
            },
            target: Target::New {
                project_id: "forja".into(),
                project_path: "C:/projetos/forja".into(),
                name_template: "deps-{data}".into(),
            },
            action: Action::Prompt {
                profile_id: Some("claude".into()),
                prompt: "Liste dependências com versões antigas e proponha a atualização.".into(),
            },
            created_at: now - 12 * day,
            ..Routine::default()
        },
        Routine {
            id: "fixture-nightly".into(),
            name: "Testes noturnos".into(),
            enabled: false,
            schedule: Schedule::Cron {
                expression: "30 2 * * *".into(),
            },
            target: Target::Fixed {
                bench_path: "C:/projetos/forja/principal".into(),
            },
            action: Action::Command {
                command: "cargo test --workspace".into(),
            },
            precheck: Some("git diff --quiet".into()),
            tolerance_s: Some(600),
            created_at: now - 40 * day,
            ..Routine::default()
        },
    ];
    let run = |id: &str, routine: &str, name: &str, ago: i64, status: RunStatus, summary: &str, trigger: Trigger| Run {
        id: id.into(),
        routine_id: routine.into(),
        routine_name: name.into(),
        trigger,
        status,
        started: now - ago,
        finished: Some(now - ago + 40),
        bench_path: Some("C:/projetos/forja/principal".into()),
        terminal_id: Some("t-3".into()),
        summary: summary.into(),
    };
    let runs = vec![
        run(
            "fixture-run-4",
            "fixture-report",
            "Resumo diário do quadro",
            4 * 3600,
            RunStatus::Succeeded,
            "2 cartões sem responsável\nrelatório gravado em relatorios/quadro.md",
            Trigger::Schedule,
        ),
        run(
            "fixture-run-3",
            "fixture-deps",
            "Revisão de dependências",
            26 * 3600,
            RunStatus::Failed {
                reason: Failure::HelperUnavailable,
            },
            "",
            Trigger::Schedule,
        ),
        run(
            "fixture-run-2",
            "fixture-nightly",
            "Testes noturnos",
            50 * 3600,
            RunStatus::Skipped {
                reason: Skip::PrecheckFailed { code: 1 },
            },
            "",
            Trigger::Schedule,
        ),
        run(
            "fixture-run-1",
            "fixture-report",
            "Resumo diário do quadro",
            52 * 3600,
            RunStatus::Failed {
                reason: Failure::CommandExit { code: 3 },
            },
            "erro: relatório sem o campo de data\nsaindo com código 3",
            Trigger::Manual,
        ),
    ];
    Store {
        routines,
        runs,
        checked_at: Some(now),
    }
}
