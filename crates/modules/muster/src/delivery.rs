//! Quando um recado pode ser digitado no terminal do ajudante.
//!
//! Digitar termina com Enter. Num ajudante esperando aprovação, esse Enter
//! responderia o pedido; por isso recados ficam na fila enquanto o ajudante
//! está ocupado ou há um prompt de aprovação visível.

use crate::model::AssignmentState;

/// Decisão para um recado.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Delivery {
    /// Pode digitar e enviar Enter.
    Send,
    /// Fica na fila; o texto diz o motivo para o coordenador.
    Hold(&'static str),
}

/// Decide pelo estado da atribuição e pelo fim do terminal.
pub fn decide(state: AssignmentState, recent_output: &str) -> Delivery {
    match state {
        AssignmentState::NeedsYou => Delivery::Hold("o ajudante está esperando aprovação"),
        AssignmentState::Starting | AssignmentState::Running => Delivery::Hold("o ajudante está ocupado"),
        AssignmentState::Delivered
        | AssignmentState::Failed
        | AssignmentState::Blocked
        | AssignmentState::Stopped => Delivery::Hold("a atribuição já terminou"),
        AssignmentState::Idle => {
            if looks_like_approval_prompt(recent_output) {
                Delivery::Hold("há um pedido de aprovação na tela")
            } else {
                Delivery::Send
            }
        }
    }
}

/// Últimas linhas não vazias, onde o CLI desenha o prompt.
pub fn recent_lines(text: &str) -> String {
    let lines: Vec<&str> = text.lines().filter(|line| !line.trim().is_empty()).collect();
    let start = lines.len().saturating_sub(12);
    lines[start..].join("\n")
}

/// Textos de confirmação usados pelos CLIs suportados. Um falso positivo só
/// segura o recado, que é a direção segura.
pub fn looks_like_approval_prompt(text: &str) -> bool {
    let lower = text.to_ascii_lowercase();
    const MARKERS: &[&str] = &[
        "(y/n)",
        "[y/n]",
        "(y/n/",
        "[y/n/",
        "[y/n",
        "yes, allow",
        "allow once",
        "allow this session",
        "allow command",
        "do you want to proceed",
        "do you want to make this edit",
        "do you want to run",
        "press enter to confirm",
        "esc to cancel",
        "trust this folder",
    ];
    MARKERS.iter().any(|marker| lower.contains(marker))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_helper_waiting_for_approval_never_receives_a_note() {
        assert!(matches!(
            decide(AssignmentState::NeedsYou, "Ready\n> "),
            Delivery::Hold(_)
        ));
    }

    #[test]
    fn a_busy_helper_holds_and_an_idle_helper_accepts() {
        assert!(matches!(
            decide(AssignmentState::Running, ""),
            Delivery::Hold(_)
        ));
        let screen = "Task finished.\n\n> \n? for shortcuts";
        assert_eq!(decide(AssignmentState::Idle, screen), Delivery::Send);
    }

    #[test]
    fn a_visible_approval_prompt_holds_even_when_idle() {
        let claude = "Bash command\n  rm -rf build\n\nDo you want to proceed?\n? 1. Yes\n  2. No\nEsc to cancel";
        assert!(matches!(
            decide(AssignmentState::Idle, claude),
            Delivery::Hold(_)
        ));
        let codex = "Allow command: cargo test (y/n)";
        assert!(matches!(
            decide(AssignmentState::Idle, codex),
            Delivery::Hold(_)
        ));
    }

    #[test]
    fn an_old_prompt_no_longer_holds_the_note() {
        let old = "Do you want to proceed?\n1. Yes";
        let mut screen = String::from(old);
        for i in 0..20 {
            screen.push_str(&format!("\nagent output line {i}"));
        }
        assert_eq!(
            decide(AssignmentState::Idle, &recent_lines(&screen)),
            Delivery::Send
        );
    }
}

