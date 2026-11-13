//! Digitação por voz do Windows: manda Win+H pelo `SendInput`, que liga e desliga o ditado do
//! sistema no campo que está com o foco. A sequência é uma lista pura, testada; o envio é a única
//! parte que fala com o sistema, e os testes usam uma versão que só registra.

/// Teclas usadas pela sequência.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Key {
    LeftWin,
    H,
}

/// Um passo da sequência: a tecla desce ou sobe.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Step {
    Down(Key),
    Up(Key),
}

/// Win+H: segura Win, aperta H, solta H, solta Win.
pub fn win_h() -> [Step; 4] {
    [
        Step::Down(Key::LeftWin),
        Step::Down(Key::H),
        Step::Up(Key::H),
        Step::Up(Key::LeftWin),
    ]
}

/// Quem envia os passos ao sistema. Trait para os testes não mexerem no teclado.
pub trait InputSender: Send + Sync {
    fn send(&self, steps: &[Step]) -> Result<(), &'static str>;
}

/// Envio real, pelo `SendInput`. Fora do Windows, responde que não há suporte.
#[derive(Clone, Copy, Debug, Default)]
pub struct SystemInput;

impl InputSender for SystemInput {
    fn send(&self, steps: &[Step]) -> Result<(), &'static str> {
        #[cfg(windows)]
        {
            windows_impl::send(steps)
        }
        #[cfg(not(windows))]
        {
            let _ = steps;
            Err("voice-error-unsupported")
        }
    }
}

#[cfg(windows)]
mod windows_impl {
    use super::{Key, Step};
    use windows_sys::Win32::UI::Input::KeyboardAndMouse::{
        SendInput, INPUT, INPUT_0, INPUT_KEYBOARD, KEYBDINPUT, KEYEVENTF_KEYUP,
    };

    /// Código de tecla virtual do Win esquerdo.
    const VK_LWIN: u16 = 0x5B;
    /// Código de tecla virtual de H.
    const VK_H: u16 = 0x48;

    pub fn send(steps: &[Step]) -> Result<(), &'static str> {
        let inputs: Vec<INPUT> = steps.iter().map(to_input).collect();
        let size = i32::try_from(std::mem::size_of::<INPUT>()).map_err(|_| "voice-error-input")?;
        let count = u32::try_from(inputs.len()).map_err(|_| "voice-error-input")?;
        // SAFETY: `inputs` é um vetor vivo com `count` entradas do tipo `INPUT`, e `size` é o
        // tamanho exato desse tipo. A função só lê esse buffer durante a chamada.
        let sent = unsafe { SendInput(count, inputs.as_ptr(), size) };
        if sent == count {
            Ok(())
        } else {
            Err("voice-error-input")
        }
    }

    fn to_input(step: &Step) -> INPUT {
        let (key, flags) = match step {
            Step::Down(key) => (*key, 0),
            Step::Up(key) => (*key, KEYEVENTF_KEYUP),
        };
        let vk = match key {
            Key::LeftWin => VK_LWIN,
            Key::H => VK_H,
        };
        INPUT {
            r#type: INPUT_KEYBOARD,
            Anonymous: INPUT_0 {
                ki: KEYBDINPUT {
                    wVk: vk,
                    wScan: 0,
                    dwFlags: flags,
                    time: 0,
                    dwExtraInfo: 0,
                },
            },
        }
    }
}

#[cfg(test)]
pub mod recording {
    use super::{InputSender, Step};
    use std::sync::Mutex;

    /// Envio de teste: guarda os passos que mandaria, sem tocar no teclado.
    #[derive(Default)]
    pub struct RecordingInput(pub Mutex<Vec<Step>>);

    impl InputSender for RecordingInput {
        fn send(&self, steps: &[Step]) -> Result<(), &'static str> {
            self.0.lock().map_err(|_| "voice-error-input")?.extend_from_slice(steps);
            Ok(())
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn win_h_presses_win_then_h_and_releases_in_reverse() {
        assert_eq!(
            win_h(),
            [
                Step::Down(Key::LeftWin),
                Step::Down(Key::H),
                Step::Up(Key::H),
                Step::Up(Key::LeftWin),
            ]
        );
    }
}
