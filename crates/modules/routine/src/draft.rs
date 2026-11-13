//! Rascunho do editor: o que a tela digita, convertido de e para uma [`Routine`].
//!
//! O editor trabalha com texto (horário "09:00", data, campos vazios). Aqui isso vira a
//! rotina de verdade, com erros que dizem o campo. A validação final continua no modelo.

use crate::model::{Action, Routine, Target};
use crate::schedule::{Preset, Schedule};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ScheduleKind {
    Preset,
    Cron,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PresetKind {
    Hourly,
    Daily,
    Weekdays,
    Weekly,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum TargetKind {
    Ask,
    Fixed,
    New,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ActionKind {
    Command,
    Prompt,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Draft {
    pub id: String,
    pub name: String,
    pub enabled: bool,
    pub schedule_kind: ScheduleKind,
    pub preset: PresetKind,
    /// "HH:MM", horário local.
    pub time: String,
    /// 0 = domingo.
    pub weekday: usize,
    pub cron: String,
    pub target: TargetKind,
    pub bench_path: String,
    pub project_id: String,
    pub project_path: String,
    pub name_template: String,
    pub action: ActionKind,
    pub command: String,
    pub profile: String,
    pub prompt: String,
    pub precheck: String,
    pub tolerance: String,
}

impl Draft {
    /// Rascunho de rotina nova. `target` vem da configuração `routine.run.target`.
    pub fn new(target: TargetKind) -> Self {
        Self {
            id: String::new(),
            name: String::new(),
            enabled: true,
            schedule_kind: ScheduleKind::Preset,
            preset: PresetKind::Daily,
            time: "09:00".to_owned(),
            weekday: 1,
            cron: "0 9 * * 1-5".to_owned(),
            target,
            bench_path: String::new(),
            project_id: String::new(),
            project_path: String::new(),
            name_template: "rotina-{rotina}-{data}".to_owned(),
            action: ActionKind::Command,
            command: String::new(),
            profile: String::new(),
            prompt: String::new(),
            precheck: String::new(),
            tolerance: String::new(),
        }
    }

    /// Rascunho a partir de uma rotina salva, para edição.
    pub fn from_routine(routine: &Routine) -> Self {
        let mut draft = Self::new(TargetKind::Ask);
        draft.id = routine.id.clone();
        draft.name = routine.name.clone();
        draft.enabled = routine.enabled;
        match &routine.schedule {
            Schedule::Preset { preset } => {
                draft.schedule_kind = ScheduleKind::Preset;
                match preset {
                    Preset::Hourly => {
                        draft.preset = PresetKind::Hourly;
                    }
                    Preset::Daily { hour, minute } => {
                        draft.preset = PresetKind::Daily;
                        draft.time = format!("{hour:02}:{minute:02}");
                    }
                    Preset::Weekdays { hour, minute } => {
                        draft.preset = PresetKind::Weekdays;
                        draft.time = format!("{hour:02}:{minute:02}");
                    }
                    Preset::Weekly { weekday, hour, minute } => {
                        draft.preset = PresetKind::Weekly;
                        draft.weekday = usize::from(*weekday);
                        draft.time = format!("{hour:02}:{minute:02}");
                    }
                }
            }
            Schedule::Cron { expression } => {
                draft.schedule_kind = ScheduleKind::Cron;
                draft.cron = expression.clone();
            }
        }
        match &routine.target {
            Target::Ask => draft.target = TargetKind::Ask,
            Target::Fixed { bench_path } => {
                draft.target = TargetKind::Fixed;
                draft.bench_path = bench_path.clone();
            }
            Target::New {
                project_id,
                project_path,
                name_template,
            } => {
                draft.target = TargetKind::New;
                draft.project_id = project_id.clone();
                draft.project_path = project_path.clone();
                draft.name_template = name_template.clone();
            }
        }
        match &routine.action {
            Action::Command { command } => {
                draft.action = ActionKind::Command;
                draft.command = command.clone();
            }
            Action::Prompt { profile_id, prompt } => {
                draft.action = ActionKind::Prompt;
                draft.profile = profile_id.clone().unwrap_or_default();
                draft.prompt = prompt.clone();
            }
        }
        draft.precheck = routine.precheck.clone().unwrap_or_default();
        draft.tolerance = routine.tolerance_s.map(|s| s.to_string()).unwrap_or_default();
        draft
    }

    /// Agenda escrita pelo usuário. Horário inválido vira erro com o motivo.
    pub fn schedule(&self) -> Result<Schedule, String> {
        match self.schedule_kind {
            ScheduleKind::Cron => Ok(Schedule::Cron {
                expression: self.cron.trim().to_owned(),
            }),
            ScheduleKind::Preset => {
                let preset = match self.preset {
                    PresetKind::Hourly => Preset::Hourly,
                    PresetKind::Daily => {
                        let (hour, minute) = parse_time(&self.time)?;
                        Preset::Daily { hour, minute }
                    }
                    PresetKind::Weekdays => {
                        let (hour, minute) = parse_time(&self.time)?;
                        Preset::Weekdays { hour, minute }
                    }
                    PresetKind::Weekly => {
                        let (hour, minute) = parse_time(&self.time)?;
                        let weekday = u8::try_from(self.weekday).map_err(|_| "weekday".to_owned())?;
                        Preset::Weekly { weekday, hour, minute }
                    }
                };
                Ok(Schedule::Preset { preset })
            }
        }
    }

    /// Rotina pronta para gravar. Não valida: quem grava chama `model::validate`.
    pub fn to_routine(&self, created_at: i64, last_fire: Option<i64>) -> Result<Routine, String> {
        let schedule = self.schedule()?;
        let target = match self.target {
            TargetKind::Ask => Target::Ask,
            TargetKind::Fixed => Target::Fixed {
                bench_path: self.bench_path.trim().to_owned(),
            },
            TargetKind::New => Target::New {
                project_id: self.project_id.trim().to_owned(),
                project_path: self.project_path.trim().to_owned(),
                name_template: self.name_template.trim().to_owned(),
            },
        };
        let action = match self.action {
            ActionKind::Command => Action::Command {
                command: self.command.trim().to_owned(),
            },
            ActionKind::Prompt => Action::Prompt {
                profile_id: Some(self.profile.trim().to_owned()).filter(|p| !p.is_empty()),
                prompt: self.prompt.trim().to_owned(),
            },
        };
        let precheck = Some(self.precheck.trim().to_owned()).filter(|text| !text.is_empty());
        let tolerance_s = match self.tolerance.trim() {
            "" => None,
            text => Some(text.parse::<u32>().map_err(|_| "tolerance".to_owned())?),
        };
        Ok(Routine {
            id: self.id.clone(),
            name: self.name.trim().to_owned(),
            enabled: self.enabled,
            schedule,
            target,
            action,
            precheck,
            tolerance_s,
            created_at,
            last_fire,
        })
    }
}

/// "09:00" ou "9:5" para (hora, minuto). Só aceita o que cabe no relógio.
pub fn parse_time(text: &str) -> Result<(u8, u8), String> {
    let (hour, minute) = text.trim().split_once(':').ok_or_else(|| "time".to_owned())?;
    let hour: u8 = hour.trim().parse().map_err(|_| "time".to_owned())?;
    let minute: u8 = minute.trim().parse().map_err(|_| "time".to_owned())?;
    if hour > 23 || minute > 59 {
        return Err("time".to_owned());
    }
    Ok((hour, minute))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn times_are_parsed_and_bounded() {
        assert_eq!(parse_time("09:00"), Ok((9, 0)));
        assert_eq!(parse_time(" 9:5 "), Ok((9, 5)));
        assert!(parse_time("24:00").is_err());
        assert!(parse_time("09:60").is_err());
        assert!(parse_time("nove").is_err());
    }

    #[test]
    fn a_routine_survives_a_round_trip_through_the_editor() {
        let routine = Routine {
            id: "r1".into(),
            name: "Resumo".into(),
            schedule: Schedule::Preset {
                preset: Preset::Weekly {
                    weekday: 5,
                    hour: 17,
                    minute: 30,
                },
            },
            target: Target::Fixed {
                bench_path: "C:/repo".into(),
            },
            action: Action::Prompt {
                profile_id: Some("claude".into()),
                prompt: "Resuma os commits.".into(),
            },
            precheck: Some("git status".into()),
            tolerance_s: Some(300),
            created_at: 10,
            ..Routine::default()
        };
        let draft = Draft::from_routine(&routine);
        assert_eq!(draft.time, "17:30");
        assert_eq!(draft.weekday, 5);
        let back = draft.to_routine(10, None).unwrap();
        assert_eq!(back, routine);
    }

    #[test]
    fn a_bad_time_is_reported_before_saving() {
        let mut draft = Draft::new(TargetKind::Ask);
        draft.time = "25:00".into();
        assert_eq!(draft.to_routine(0, None), Err("time".to_owned()));
        draft.schedule_kind = ScheduleKind::Cron;
        draft.cron = "0 9 * * *".into();
        assert!(draft.to_routine(0, None).is_ok());
    }
}
