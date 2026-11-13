//! Gestos de uma tecla de ditado. Alternar: um toque liga, o próximo desliga. Segurar: liga ao
//! apertar e desliga ao soltar, mas só se foi o próprio segurar que ligou. Função pura: recebe o
//! evento e devolve o que fazer; quem executa é a camada de ações.

/// Como a tecla comanda: alternando ou segurando.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Mode {
    Toggle,
    Hold,
}

/// Borda do evento de tecla.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Edge {
    Press,
    Release,
}

/// O que a ação deve fazer em seguida.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Action {
    Start,
    Stop,
    Nothing,
}

/// Estado do gesto. `active` diz se o ditado está ligado; `by_hold` diz se foi o segurar que ligou.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Gesture {
    active: bool,
    by_hold: bool,
}

impl Gesture {
    pub fn is_active(&self) -> bool {
        self.active
    }

    /// Alimenta um evento e devolve a ação. Os dois modos seguem as regras do módulo.
    pub fn feed(&mut self, mode: Mode, edge: Edge) -> Action {
        match (mode, edge) {
            (Mode::Toggle, Edge::Press) => {
                if self.active {
                    self.stop()
                } else {
                    self.start(false)
                }
            }
            (Mode::Toggle, Edge::Release) => Action::Nothing,
            (Mode::Hold, Edge::Press) => {
                if self.active {
                    Action::Nothing
                } else {
                    self.start(true)
                }
            }
            (Mode::Hold, Edge::Release) => {
                if self.active && self.by_hold {
                    self.stop()
                } else {
                    Action::Nothing
                }
            }
        }
    }

    /// Desliga sem ação, quando o ditado acabou por outro motivo (erro, fim da gravação).
    pub fn reset(&mut self) {
        self.active = false;
        self.by_hold = false;
    }

    fn start(&mut self, by_hold: bool) -> Action {
        self.active = true;
        self.by_hold = by_hold;
        Action::Start
    }

    fn stop(&mut self) -> Action {
        self.reset();
        Action::Stop
    }
}

#[cfg(test)]
mod tests;
