//! Regras do catálogo de notificações: silêncio, foco e horário.

use chrono::Timelike;

/// O que fazer com um aviso depois de ler o catálogo.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Decision {
    /// Mostra um toast dentro da janela.
    pub show_toast: bool,
    /// Envia uma notificação do Windows.
    pub show_os: bool,
    /// Toca o som próprio do aviso.
    pub play_sound: bool,
}

/// Entrada já resolvida das configurações e do relógio.
#[derive(Clone, Copy, Debug)]
pub struct Policy {
    pub toast_enabled: bool,
    pub os_enabled: bool,
    pub only_unfocused: bool,
    pub sound: bool,
    pub quiet: bool,
    pub quiet_start: (u8, u8),
    pub quiet_end: (u8, u8),
    pub focused: bool,
    pub now: (u8, u8),
}

/// `HH:MM` em 24 horas. Fora disso, `None`.
pub fn parse_hhmm(text: &str) -> Option<(u8, u8)> {
    let (hour, minute) = text.trim().split_once(':')?;
    let hour = hour.parse::<u8>().ok()?;
    let minute = minute.parse::<u8>().ok()?;
    if hour < 24 && minute < 60 {
        Some((hour, minute))
    } else {
        None
    }
}

/// Hora local para o horário de silêncio.
pub fn local_hm() -> (u8, u8) {
    let now = chrono::Local::now();
    (now.hour() as u8, now.minute() as u8)
}

fn minutes(hm: (u8, u8)) -> u16 {
    u16::from(hm.0) * 60 + u16::from(hm.1)
}

/// Intervalo que pode cruzar a meia-noite. Início igual ao fim não silencia nada.
pub fn in_quiet(now: (u8, u8), start: (u8, u8), end: (u8, u8)) -> bool {
    let now = minutes(now);
    let start = minutes(start);
    let end = minutes(end);
    if start == end {
        return false;
    }
    if start < end {
        now >= start && now < end
    } else {
        now >= start || now < end
    }
}

/// Aplica o catálogo. O som acompanha a notificação do Windows, não o toast.
pub fn decide(policy: &Policy) -> Decision {
    let quiet = policy.quiet && in_quiet(policy.now, policy.quiet_start, policy.quiet_end);
    if quiet {
        return Decision {
            show_toast: false,
            show_os: false,
            play_sound: false,
        };
    }
    let show_os = policy.os_enabled && !(policy.only_unfocused && policy.focused);
    Decision {
        show_toast: policy.toast_enabled,
        show_os,
        play_sound: policy.sound && show_os,
    }
}

/// Índices dos toasts ainda dentro da duração, do mais recente, até `max`.
pub fn visible_toast_indexes(age_secs: &[f32], duration_secs: f32, max: usize) -> Vec<usize> {
    if max == 0 || duration_secs <= 0.0 {
        return Vec::new();
    }
    let mut live: Vec<usize> = age_secs
        .iter()
        .enumerate()
        .filter(|(_, age)| **age < duration_secs)
        .map(|(index, _)| index)
        .collect();
    if live.len() > max {
        live = live.split_off(live.len() - max);
    }
    live
}

#[cfg(test)]
mod tests {
    use super::*;

    fn policy() -> Policy {
        Policy {
            toast_enabled: true,
            os_enabled: true,
            only_unfocused: true,
            sound: true,
            quiet: false,
            quiet_start: (22, 0),
            quiet_end: (7, 0),
            focused: true,
            now: (15, 0),
        }
    }

    #[test]
    fn quiet_hours_wrap_past_midnight() {
        assert!(in_quiet((23, 30), (22, 0), (7, 0)));
        assert!(in_quiet((6, 59), (22, 0), (7, 0)));
        assert!(!in_quiet((12, 0), (22, 0), (7, 0)));
        assert!(!in_quiet((7, 0), (22, 0), (7, 0)));
        assert!(!in_quiet((22, 0), (22, 0), (22, 0)));
    }

    #[test]
    fn parse_clock_rejects_bad_values() {
        assert_eq!(parse_hhmm("22:00"), Some((22, 0)));
        assert_eq!(parse_hhmm(" 7:05 "), Some((7, 5)));
        assert_eq!(parse_hhmm("24:00"), None);
        assert_eq!(parse_hhmm("nope"), None);
    }

    #[test]
    fn focused_window_skips_os_and_sound() {
        let decision = decide(&policy());
        assert!(decision.show_toast);
        assert!(!decision.show_os);
        assert!(!decision.play_sound);
    }

    #[test]
    fn unfocused_window_notifies_and_plays() {
        let mut input = policy();
        input.focused = false;
        let decision = decide(&input);
        assert!(decision.show_toast);
        assert!(decision.show_os);
        assert!(decision.play_sound);
    }

    #[test]
    fn sound_stays_off_when_the_toggle_is_off() {
        let mut input = policy();
        input.focused = false;
        input.sound = false;
        assert!(!decide(&input).play_sound);
        assert!(decide(&input).show_os);
    }

    #[test]
    fn quiet_hours_mute_everything() {
        let mut input = policy();
        input.quiet = true;
        input.now = (23, 0);
        input.focused = false;
        let decision = decide(&input);
        assert!(!decision.show_toast);
        assert!(!decision.show_os);
        assert!(!decision.play_sound);
    }

    #[test]
    fn toast_stack_keeps_the_newest() {
        let ages = [1.0, 2.0, 9.0, 3.0];
        assert_eq!(visible_toast_indexes(&ages, 5.0, 2), vec![1, 3]);
        assert!(visible_toast_indexes(&ages, 5.0, 0).is_empty());
    }
}
