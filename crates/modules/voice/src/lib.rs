//! Módulo `voice` da Forja: ditado pelo Windows (Win+H) com a tecla do keymap, gravação local com
//! medidor, transcrição opcional pela OpenAI (chave no Gerenciador de Credenciais do Windows), e
//! inserção no terminal, no editor e no chat, sempre com confirmação.
//!
//! Registra os comandos `voice.dictate`, `voice.hold`, `voice.record.start`, `voice.record.stop`,
//! `voice.transcribe`, `voice.insert`, `voice.discard`, `voice.key.save`, `voice.key.delete` e
//! `voice.targets.refresh`; a janela (`voice.overlay`, camada Modal) e a seção "Voz" dos Ajustes.
//! Não há método RPC. Contrato em `design/contracts.md`, seção "Voz".

mod actions;
mod commands;
mod gesture;
mod keystore;
mod multipart;
mod openai;
mod record;
mod state;
mod view;
mod wav;
mod winkeys;

#[cfg(test)]
mod tests;

use crate::keystore::{KeyStore, SystemKeyring};
use crate::state::{Backends, Shared};
use crate::winkeys::{InputSender, SystemInput};
use forja_kernel::ui_contract::View;
use forja_kernel::{Module, ModuleManifest, Registrar, SettingKind, SettingSpec, SettingValue, Slot};
use std::sync::Arc;

#[forja_kernel::module]
static VOICE: fn() -> Box<dyn Module> = || Box::new(VoiceModule::new());

/// Dono do estado de voz. A janela, a seção de Ajustes e os comandos compartilham o `Shared`.
pub struct VoiceModule {
    shared: Shared,
}

impl VoiceModule {
    /// O módulo de produção: cofre do Windows e envio real de teclas.
    pub fn new() -> Self {
        Self::with_backends(Arc::new(SystemKeyring), Arc::new(SystemInput))
    }

    /// Com backends escolhidos: os testes passam versões falsas do cofre e do teclado.
    pub fn with_backends(keys: Arc<dyn KeyStore>, input: Arc<dyn InputSender>) -> Self {
        Self {
            shared: Shared::new(Backends { keys, input }),
        }
    }
}

impl Default for VoiceModule {
    fn default() -> Self {
        Self::new()
    }
}

impl Module for VoiceModule {
    fn manifest(&self) -> ModuleManifest {
        ModuleManifest::new("voice", "voice.name", env!("CARGO_PKG_VERSION"))
    }

    fn register(&mut self, r: &mut Registrar<'_>) {
        r.i18n(
            include_str!("../i18n/en.ftl"),
            include_str!("../i18n/pt-BR.ftl"),
        );
        r.keymap(include_str!("../keymap.toml"));
        register_settings(r);
        for key in actions::USED_KEYS.iter().chain(view::USED_KEYS).copied() {
            r.uses_i18n(key);
        }

        let shared = self.shared.clone();
        r.command("voice.dictate", "voice-command-dictate", move |cx, _args| {
            commands::dictate(cx, &shared)
        });

        let shared = self.shared.clone();
        r.command("voice.hold", "voice-command-hold", move |cx, args| {
            commands::hold(cx, &shared, &args)
        });

        let shared = self.shared.clone();
        r.command("voice.record.start", "voice-command-record-start", move |cx, _args| {
            commands::record_start(cx, &shared)
        });

        let shared = self.shared.clone();
        r.command("voice.record.stop", "voice-command-record-stop", move |cx, _args| {
            commands::record_stop(cx, &shared)
        });

        let shared = self.shared.clone();
        r.command("voice.transcribe", "voice-command-transcribe", move |cx, _args| {
            commands::transcribe(cx, &shared)
        });

        let shared = self.shared.clone();
        r.command("voice.insert", "voice-command-insert", move |cx, args| {
            commands::insert(cx, &shared, &args)
        });

        let shared = self.shared.clone();
        r.command("voice.discard", "voice-command-discard", move |_cx, _args| {
            commands::discard(&shared)
        });

        let shared = self.shared.clone();
        r.command("voice.key.save", "voice-command-key-save", move |cx, args| {
            commands::key_save(cx, &shared, &args)
        });

        let shared = self.shared.clone();
        r.command("voice.key.delete", "voice-command-key-delete", move |cx, _args| {
            commands::key_delete(cx, &shared)
        });

        let shared = self.shared.clone();
        r.command("voice.targets.refresh", "voice-command-targets-refresh", move |cx, _args| {
            commands::targets_refresh(cx, &shared)
        });

        let shared = self.shared.clone();
        r.view(Slot::Modal, "voice.overlay", "voice-overlay-title", 20, move || {
            Box::new(view::OverlayView::new(shared.clone())) as Box<dyn View>
        });

        let shared = self.shared.clone();
        // Ordem 25: logo depois de "Ajudantes" (20), porque o catálogo põe as chaves de voz no grupo Ajudantes.
        r.view(Slot::SettingsSection, "voice.settings", "voice-section-title", 25, move || {
            Box::new(view::SettingsView::new(shared.clone())) as Box<dyn View>
        });
    }
}

/// Configurações de voz, com os padrões e as opções do catálogo (`design/settings.toml`).
fn register_settings(r: &mut Registrar<'_>) {
    r.setting(SettingSpec::new(
        "voice.mode.enabled",
        SettingKind::Bool,
        SettingValue::Bool(false),
        "setting-voice-mode-enabled-label",
        "setting-voice-mode-enabled-help",
    ));
    r.setting(SettingSpec::new(
        "voice.input.engine",
        SettingKind::Choice {
            options: vec!["windows".to_owned(), "whisper_cpp".to_owned()],
        },
        SettingValue::Text("windows".to_owned()),
        "setting-voice-input-engine-label",
        "setting-voice-input-engine-help",
    ));
    r.setting(SettingSpec::new(
        "voice.input.language",
        SettingKind::Choice {
            options: vec!["auto".to_owned(), "pt_BR".to_owned(), "en".to_owned()],
        },
        SettingValue::Text("auto".to_owned()),
        "setting-voice-input-language-label",
        "setting-voice-input-language-help",
    ));
    r.setting(SettingSpec::new(
        "voice.privacy.keep_audio",
        SettingKind::Bool,
        SettingValue::Bool(false),
        "setting-voice-privacy-keep_audio-label",
        "setting-voice-privacy-keep_audio-help",
    ));
}
