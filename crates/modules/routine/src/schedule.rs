//! Agenda das rotinas: presets, cron de cinco campos e a decisão de disparo com tolerância.
//!
//! Tudo é calculado em horário de parede do fuso escolhido (sistema ou UTC). Na virada do
//! horário de verão, um horário que não existe é pulado; um horário repetido dispara nas duas
//! ocorrências, na ordem em que acontecem. Nenhum disparo é feito aqui: isto só responde
//! "quando" e "se está devido".

use chrono::{DateTime, Datelike, Duration, LocalResult, NaiveDate, NaiveDateTime, TimeZone, Timelike};
use serde::{Deserialize, Serialize};

/// Quantos dias para frente a busca procura o próximo horário (cinco anos).
const FORWARD_DAYS: i64 = 366 * 5;
/// Quantos dias para trás a busca do último disparo olha (dois anos).
const BACKWARD_DAYS: i64 = 366 * 2;

const MONTHS: [&str; 12] = ["JAN", "FEB", "MAR", "APR", "MAY", "JUN", "JUL", "AUG", "SEP", "OCT", "NOV", "DEC"];
const WEEKDAYS: [&str; 7] = ["SUN", "MON", "TUE", "WED", "THU", "FRI", "SAT"];

/// Presets que cobrem a maioria dos casos. Cada um vira uma expressão cron.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "name", rename_all = "snake_case")]
pub enum Preset {
    Hourly,
    Daily { hour: u8, minute: u8 },
    Weekdays { hour: u8, minute: u8 },
    /// `weekday`: 0 é domingo, 6 é sábado.
    Weekly { weekday: u8, hour: u8, minute: u8 },
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum Schedule {
    Preset { preset: Preset },
    Cron { expression: String },
}

impl Schedule {
    /// Expressão cron equivalente, de cinco campos.
    pub fn expression(&self) -> String {
        match self {
            Self::Cron { expression } => expression.trim().to_owned(),
            Self::Preset { preset } => match preset {
                Preset::Hourly => "0 * * * *".to_owned(),
                Preset::Daily { hour, minute } => format!("{minute} {hour} * * *"),
                Preset::Weekdays { hour, minute } => format!("{minute} {hour} * * 1-5"),
                Preset::Weekly { weekday, hour, minute } => format!("{minute} {hour} * * {weekday}"),
            },
        }
    }

    /// Compila a agenda. Erros dizem o campo que está errado.
    pub fn compile(&self) -> Result<CronSpec, String> {
        if let Self::Preset { preset } = self {
            let (hour, minute, weekday) = match preset {
                Preset::Hourly => (0, 0, None),
                Preset::Daily { hour, minute } | Preset::Weekdays { hour, minute } => (*hour, *minute, None),
                Preset::Weekly { weekday, hour, minute } => (*hour, *minute, Some(*weekday)),
            };
            if hour > 23 || minute > 59 || weekday.is_some_and(|day| day > 6) {
                return Err("preset time is out of range".into());
            }
        }
        CronSpec::parse(&self.expression())
    }

    /// Próximas `count` execuções a partir de `after` (estritamente depois).
    pub fn next_runs<Tz: TimeZone>(&self, after: &DateTime<Tz>, count: usize) -> Result<Vec<DateTime<Tz>>, String> {
        Ok(self.compile()?.next_after(after, count))
    }
}

/// Agenda compilada: conjuntos de valores válidos para cada campo.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct CronSpec {
    minutes: Vec<u32>,
    hours: Vec<u32>,
    /// Índice é o dia do mês (1 a 31).
    days_of_month: [bool; 32],
    /// Índice é o mês (1 a 12).
    months: [bool; 13],
    /// Índice é o dia da semana, 0 = domingo.
    weekdays: [bool; 7],
    days_of_month_restricted: bool,
    weekdays_restricted: bool,
}

impl CronSpec {
    /// Cron de cinco campos: minuto, hora, dia do mês, mês e dia da semana. Aceita `*`, listas,
    /// intervalos, passos (`*/15`, `1-5/2`) e nomes (`MON`, `JAN`). Domingo é 0 ou 7.
    pub fn parse(expression: &str) -> Result<Self, String> {
        let fields: Vec<&str> = expression.split_whitespace().collect();
        if fields.len() != 5 {
            return Err("use five fields: minute hour day month weekday".into());
        }
        let (minutes, _) = parse_field(fields[0], 0, 59, &[]).map_err(|e| format!("minute: {e}"))?;
        let (hours, _) = parse_field(fields[1], 0, 23, &[]).map_err(|e| format!("hour: {e}"))?;
        let (dom, dom_restricted) = parse_field(fields[2], 1, 31, &[]).map_err(|e| format!("day: {e}"))?;
        let (month, _) = parse_field(fields[3], 1, 12, &MONTHS).map_err(|e| format!("month: {e}"))?;
        let (dow, dow_restricted) = parse_field(fields[4], 0, 7, &WEEKDAYS).map_err(|e| format!("weekday: {e}"))?;

        let mut days_of_month = [false; 32];
        for day in dom {
            days_of_month[day as usize] = true;
        }
        let mut months = [false; 13];
        for m in month {
            months[m as usize] = true;
        }
        let mut weekdays = [false; 7];
        for day in dow {
            // 7 também é domingo.
            weekdays[(day % 7) as usize] = true;
        }
        Ok(Self {
            minutes: minutes.into_iter().collect(),
            hours: hours.into_iter().collect(),
            days_of_month,
            months,
            weekdays,
            days_of_month_restricted: dom_restricted,
            weekdays_restricted: dow_restricted,
        })
    }

    /// Um dia casa se o mês casa e (dia do mês OU dia da semana). Se só um campo é restrito,
    /// vale só ele; é a regra do cron tradicional.
    pub fn matches_date(&self, date: NaiveDate) -> bool {
        if !self.months[date.month() as usize] {
            return false;
        }
        let day_of_month = self.days_of_month[date.day() as usize];
        let weekday = self.weekdays[date.weekday().num_days_from_sunday() as usize];
        if self.days_of_month_restricted && self.weekdays_restricted {
            day_of_month || weekday
        } else {
            day_of_month && weekday
        }
    }

    /// Próximas `count` ocorrências estritamente depois de `after`, em ordem crescente.
    pub fn next_after<Tz: TimeZone>(&self, after: &DateTime<Tz>, count: usize) -> Vec<DateTime<Tz>> {
        let zone = after.timezone();
        let start = floor_to_minute(after.naive_local()) + Duration::minutes(1);
        let mut out = Vec::new();
        let mut day = start.date();
        let last_day = day + Duration::days(FORWARD_DAYS);
        while day <= last_day && out.len() < count {
            if self.matches_date(day) {
                'day: for &hour in &self.hours {
                    for &minute in &self.minutes {
                        let Some(naive) = day.and_hms_opt(hour, minute, 0) else { continue };
                        if naive < start {
                            continue;
                        }
                        for instant in instants(&zone, naive).into_iter().flatten() {
                            if instant > *after {
                                out.push(instant);
                            }
                        }
                        if out.len() >= count {
                            break 'day;
                        }
                    }
                }
            }
            match day.succ_opt() {
                Some(next) => day = next,
                None => break,
            }
        }
        out.truncate(count);
        out
    }

    /// Último horário da agenda que não passa de `now` (olhando para trás no calendário).
    pub fn last_at_or_before<Tz: TimeZone>(&self, now: &DateTime<Tz>) -> Option<DateTime<Tz>> {
        let zone = now.timezone();
        let now_naive = now.naive_local();
        let mut day = now_naive.date();
        for _ in 0..=BACKWARD_DAYS {
            if self.matches_date(day) {
                for &hour in self.hours.iter().rev() {
                    for &minute in self.minutes.iter().rev() {
                        let Some(naive) = day.and_hms_opt(hour, minute, 0) else { continue };
                        if naive > now_naive + Duration::minutes(1) {
                            continue;
                        }
                        // Horário repetido: a segunda ocorrência é a mais recente.
                        let mut candidates: Vec<DateTime<Tz>> = instants(&zone, naive).into_iter().flatten().collect();
                        candidates.reverse();
                        if let Some(found) = candidates.into_iter().find(|t| t <= now) {
                            return Some(found);
                        }
                    }
                }
            }
            match day.pred_opt() {
                Some(previous) => day = previous,
                None => break,
            }
        }
        None
    }
}

/// Decisão sobre um disparo devido.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Due<Tz: TimeZone> {
    /// Nada venceu desde a última verificação.
    Nothing,
    /// Venceu há pouco, dentro da tolerância: roda agora.
    OnTime { fire: DateTime<Tz> },
    /// Venceu há mais tempo que a tolerância (o PC dormiu, o app estava fechado).
    Late { fire: DateTime<Tz>, late_s: i64 },
}

/// Decide se a agenda venceu na janela `(window_start, now]`.
///
/// Só o disparo mais recente conta: horários perdidos no meio do caminho não são repetidos.
/// `grace_s` é a tolerância; além dela o disparo é `Late`, e a política de horário perdido decide.
pub fn due<Tz: TimeZone>(spec: &CronSpec, window_start: &DateTime<Tz>, now: &DateTime<Tz>, grace_s: i64) -> Due<Tz> {
    let Some(fire) = spec.last_at_or_before(now) else {
        return Due::Nothing;
    };
    if fire <= *window_start {
        return Due::Nothing;
    }
    let late_s = now.clone().signed_duration_since(fire.clone()).num_seconds().max(0);
    if late_s <= grace_s {
        Due::OnTime { fire }
    } else {
        Due::Late { fire, late_s }
    }
}

/// Instantes que correspondem a um horário de parede: zero (pulado), um ou dois (repetido).
fn instants<Tz: TimeZone>(zone: &Tz, naive: NaiveDateTime) -> Vec<Option<DateTime<Tz>>> {
    match zone.from_local_datetime(&naive) {
        LocalResult::None => vec![],
        LocalResult::Single(instant) => vec![Some(instant)],
        LocalResult::Ambiguous(first, second) => vec![Some(first), Some(second)],
    }
}

fn floor_to_minute(naive: NaiveDateTime) -> NaiveDateTime {
    naive.with_second(0).and_then(|t| t.with_nanosecond(0)).unwrap_or(naive)
}

/// Um campo do cron. Devolve os valores válidos e se o campo é restrito (não começa com `*`).
fn parse_field(text: &str, min: u32, max: u32, names: &[&str]) -> Result<(Vec<u32>, bool), String> {
    if text.is_empty() {
        return Err("empty field".into());
    }
    let restricted = !text.starts_with('*');
    let mut values = Vec::new();
    for part in text.split(',') {
        let (range, step) = match part.split_once('/') {
            Some((range, step)) => {
                let step: u32 = step.parse().map_err(|_| format!("invalid step `{step}`"))?;
                if step == 0 {
                    return Err("step must be at least 1".into());
                }
                (range, Some(step))
            }
            None => (part, None),
        };
        let (low, high) = if range == "*" {
            (min, max)
        } else if let Some((a, b)) = range.split_once('-') {
            (value(a, min, max, names)?, value(b, min, max, names)?)
        } else {
            let single = value(range, min, max, names)?;
            // `5/10` começa em 5 e vai até o fim do campo.
            (single, if step.is_some() { max } else { single })
        };
        if low > high {
            return Err(format!("range `{range}` runs backwards"));
        }
        values.extend((low..=high).step_by(step.unwrap_or(1) as usize));
    }
    values.sort_unstable();
    values.dedup();
    Ok((values, restricted))
}

fn value(text: &str, min: u32, max: u32, names: &[&str]) -> Result<u32, String> {
    if text.chars().all(|c| c.is_ascii_alphabetic()) && !names.is_empty() {
        let upper = text.to_ascii_uppercase();
        let found = names.iter().position(|name| *name == upper.get(..3).unwrap_or(&upper));
        return found
            .map(|index| index as u32 + min)
            .ok_or_else(|| format!("unknown name `{text}`"));
    }
    let number: u32 = text.parse().map_err(|_| format!("`{text}` is not a number"))?;
    if number < min || number > max {
        return Err(format!("{number} is outside {min}-{max}"));
    }
    Ok(number)
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::{FixedOffset, Utc};

    /// Fuso de regras fixas, independente da máquina: horário do leste dos EUA em 2026.
    /// Início do horário de verão em 8 de março às 2h (vira para 3h); fim em 1 de novembro às 2h.
    #[derive(Clone, Debug)]
    struct Eastern;

    fn at(month: u32, day: u32, hour: u32) -> NaiveDateTime {
        NaiveDate::from_ymd_opt(2026, month, day).unwrap().and_hms_opt(hour, 0, 0).unwrap()
    }

    impl TimeZone for Eastern {
        type Offset = FixedOffset;

        fn from_offset(_: &FixedOffset) -> Self {
            Eastern
        }

        fn offset_from_local_date(&self, local: &NaiveDate) -> LocalResult<FixedOffset> {
            self.offset_from_local_datetime(&local.and_hms_opt(12, 0, 0).unwrap())
        }

        fn offset_from_local_datetime(&self, local: &NaiveDateTime) -> LocalResult<FixedOffset> {
            let winter = FixedOffset::west_opt(5 * 3600).unwrap();
            let summer = FixedOffset::west_opt(4 * 3600).unwrap();
            if *local >= at(3, 8, 2) && *local < at(3, 8, 3) {
                LocalResult::None
            } else if *local >= at(11, 1, 1) && *local < at(11, 1, 2) {
                LocalResult::Ambiguous(summer, winter)
            } else if *local >= at(3, 8, 3) && *local < at(11, 1, 1) {
                LocalResult::Single(summer)
            } else {
                LocalResult::Single(winter)
            }
        }

        fn offset_from_utc_date(&self, utc: &NaiveDate) -> FixedOffset {
            self.offset_from_utc_datetime(&utc.and_hms_opt(12, 0, 0).unwrap())
        }

        fn offset_from_utc_datetime(&self, utc: &NaiveDateTime) -> FixedOffset {
            // Transições em UTC: 8 de março às 7h e 1 de novembro às 6h.
            if *utc >= at(3, 8, 7) && *utc < at(11, 1, 6) {
                FixedOffset::west_opt(4 * 3600).unwrap()
            } else {
                FixedOffset::west_opt(5 * 3600).unwrap()
            }
        }
    }

    fn utc(y: i32, mo: u32, d: u32, h: u32, mi: u32, s: u32) -> DateTime<Utc> {
        Utc.with_ymd_and_hms(y, mo, d, h, mi, s).unwrap()
    }

    #[test]
    fn presets_and_cron_agree_on_the_next_wall_time() {
        let now = utc(2026, 11, 9, 8, 59, 59);
        for schedule in [
            Schedule::Preset { preset: Preset::Daily { hour: 9, minute: 0 } },
            Schedule::Preset { preset: Preset::Weekdays { hour: 9, minute: 0 } },
            Schedule::Preset { preset: Preset::Weekly { weekday: 1, hour: 9, minute: 0 } },
            Schedule::Cron { expression: "0 9 * * MON".into() },
        ] {
            let next = schedule.next_runs(&now, 3).unwrap();
            assert_eq!(next[0], utc(2026, 11, 9, 9, 0, 0), "{schedule:?}");
            assert!(next.windows(2).all(|w| w[0] < w[1]));
        }
    }

    #[test]
    fn strictly_after_means_a_run_at_the_same_minute_is_not_repeated() {
        let spec = CronSpec::parse("0 9 * * *").unwrap();
        assert_eq!(spec.next_after(&utc(2026, 11, 9, 9, 0, 0), 1)[0], utc(2026, 11, 10, 9, 0, 0));
        assert_eq!(spec.next_after(&utc(2026, 11, 9, 9, 0, 30), 1)[0], utc(2026, 11, 10, 9, 0, 0));
    }

    #[test]
    fn weekday_names_numbers_and_steps_follow_standard_cron() {
        // Segunda, 9 de novembro de 2026.
        let monday = utc(2026, 11, 9, 0, 0, 0);
        for expression in ["0 9 * * 1", "0 9 * * MON", "0 9 * * 8-1"] {
            if expression == "0 9 * * 8-1" {
                assert!(CronSpec::parse(expression).is_err(), "backwards range is refused");
                continue;
            }
            assert_eq!(CronSpec::parse(expression).unwrap().next_after(&monday, 1)[0], utc(2026, 11, 9, 9, 0, 0));
        }
        // Domingo como 0 e como 7.
        let sunday = CronSpec::parse("0 9 * * 0").unwrap();
        let seven = CronSpec::parse("0 9 * * 7").unwrap();
        assert_eq!(sunday.next_after(&monday, 1), seven.next_after(&monday, 1));
        // Passos: segunda, quarta e sexta.
        let mwf = CronSpec::parse("0 9 * * 1-5/2").unwrap();
        let fires: Vec<_> = mwf.next_after(&monday, 3).iter().map(|t| t.date_naive().to_string()).collect();
        assert_eq!(fires, ["2026-11-09", "2026-11-11", "2026-11-13"]);
    }

    #[test]
    fn invalid_expressions_are_refused_with_the_field_name() {
        assert!(CronSpec::parse("0 0 9 * * *").is_err(), "six fields");
        let error = CronSpec::parse("70 25 * * *").unwrap_err();
        assert!(error.starts_with("minute"), "{error}");
        assert!(CronSpec::parse("*/0 * * * *").is_err());
        assert!(CronSpec::parse("0 9 * * FOO").is_err());
        assert!(Schedule::Preset { preset: Preset::Daily { hour: 24, minute: 0 } }.compile().is_err());
    }

    #[test]
    fn spring_gap_skips_the_wall_time_that_does_not_exist() {
        // 8 de março, 2h não existe no leste dos EUA. O 2h30 diário pula esse dia.
        let after = Eastern.with_ymd_and_hms(2026, 3, 7, 3, 0, 0).unwrap();
        let runs = Schedule::Preset { preset: Preset::Daily { hour: 2, minute: 30 } }
            .compile()
            .unwrap()
            .next_after(&after, 2);
        assert_eq!(runs[0], Eastern.with_ymd_and_hms(2026, 3, 9, 2, 30, 0).unwrap());
    }

    #[test]
    fn fall_fold_fires_both_occurrences_in_increasing_order() {
        // 1 de novembro, 1h acontece duas vezes. A regra horária dispara nas duas.
        let after = Eastern.with_ymd_and_hms(2026, 11, 1, 0, 30, 0).unwrap();
        let runs = Schedule::Preset { preset: Preset::Hourly }.compile().unwrap().next_after(&after, 3);
        assert_eq!(runs[0].naive_local(), runs[1].naive_local(), "same wall clock");
        assert_eq!(runs[1].timestamp() - runs[0].timestamp(), 3600, "one real hour apart");
        assert!(runs.windows(2).all(|w| w[0] < w[1]));
    }

    #[test]
    fn due_inside_the_tolerance_runs_on_time() {
        let spec = CronSpec::parse("0 9 * * *").unwrap();
        let window = utc(2026, 11, 9, 8, 59, 0);
        let now = utc(2026, 11, 9, 9, 1, 0);
        assert_eq!(due(&spec, &window, &now, 120), Due::OnTime { fire: utc(2026, 11, 9, 9, 0, 0) });
    }

    #[test]
    fn due_beyond_the_tolerance_is_late_and_reports_the_delay() {
        let spec = CronSpec::parse("0 9 * * *").unwrap();
        let window = utc(2026, 11, 9, 8, 0, 0);
        let now = utc(2026, 11, 9, 11, 0, 0);
        assert_eq!(
            due(&spec, &window, &now, 120),
            Due::Late {
                fire: utc(2026, 11, 9, 9, 0, 0),
                late_s: 7200
            }
        );
    }

    #[test]
    fn nothing_is_due_before_the_window_or_when_already_handled() {
        let spec = CronSpec::parse("0 9 * * *").unwrap();
        let now = utc(2026, 11, 9, 9, 0, 30);
        // Janela começa depois do disparo: já foi tratado.
        assert_eq!(due(&spec, &utc(2026, 11, 9, 9, 0, 0), &now, 120), Due::Nothing);
        // Antes do primeiro disparo do dia, nada venceu.
        assert_eq!(due(&spec, &utc(2026, 11, 9, 8, 0, 0), &utc(2026, 11, 9, 8, 30, 0), 120), Due::Nothing);
    }

    #[test]
    fn a_long_gap_is_one_late_decision_for_the_latest_fire() {
        // Hora em hora, fechado por 10 horas: um único disparo atrasado, o mais recente.
        let spec = CronSpec::parse("0 * * * *").unwrap();
        let window = utc(2026, 11, 9, 0, 0, 0);
        let now = utc(2026, 11, 9, 10, 20, 0);
        assert_eq!(
            due(&spec, &window, &now, 120),
            Due::Late {
                fire: utc(2026, 11, 9, 10, 0, 0),
                late_s: 1200
            }
        );
    }

    #[test]
    fn the_fall_fold_last_fire_is_the_second_occurrence() {
        let spec = CronSpec::parse("30 1 * * *").unwrap();
        let now = Eastern.with_ymd_and_hms(2026, 11, 1, 2, 0, 0).unwrap();
        let last = spec.last_at_or_before(&now).unwrap();
        // 1h30 do horário de verão (primeira ocorrência, mais cedo) é a última antes de 2h? Não:
        // a segunda ocorrência (1h30 padrão) acontece depois, então a mais recente é ela.
        assert_eq!(last.naive_local(), at(11, 1, 1) + Duration::minutes(30));
        assert_eq!(last.offset().local_minus_utc(), -5 * 3600);
    }
}
