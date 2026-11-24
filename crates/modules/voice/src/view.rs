//! Telas de voz: a janela de gravação (camada `Modal`) e a seção "Voz" dos Ajustes.
//!
//! Como nas outras telas: copia o estado, solta o lock e só então chama ações. Inserir texto no
//! destino pede confirmação, e a chave da OpenAI é um campo de senha que não volta para a tela.

use crate::actions;
use crate::state::{
    fixtures_enabled, overlay_fixture_enabled, InsertKind, PendingInsert, Settings, Shared,
    TerminalChoice,
};
use forja_kernel::ui_contract::{egui, View};
use forja_kernel::{Ctx, SettingValue};
use forja_ui_kit::{button, progress_bar, secret_field, select_labeled, text_area, toggle, ButtonKind};

/// Chaves que as telas montam em tempo de execução.
pub const USED_KEYS: &[&str] = &[
    "voice-overlay-title",
    "voice-transcript-label",
    "voice-transcript-help",
    "voice-state-idle",
    "voice-state-recording",
    "voice-state-transcribing",
    "voice-state-captured",
    "voice-dictation-on",
    "voice-dictation-off",
    "voice-action-record",
    "voice-action-stop",
    "voice-action-transcribe",
    "voice-action-insert",
    "voice-action-discard",
    "voice-action-confirm",
    "voice-action-cancel",
    "voice-hint-no-key",
    "voice-target-label",
    "voice-target-editor",
    "voice-target-chat",
    "voice-target-none",
    "voice-confirm-title",
    "voice-confirm-body",
    "voice-section-title",
    "voice-section-intro",
    "voice-engine-windows",
    "voice-engine-whisper-cpp",
    "voice-engine-unavailable",
    "voice-language-auto",
    "voice-language-pt-br",
    "voice-language-en",
    "voice-key-label",
    "voice-key-present",
    "voice-key-absent",
    "voice-key-unknown",
    "voice-key-hint",
    "voice-key-save",
    "voice-key-delete",
    "voice-privacy-note",
    "setting-voice-mode-enabled-label",
    "setting-voice-input-engine-label",
    "setting-voice-input-language-label",
    "setting-voice-privacy-keep_audio-label",
];

/// Cópia do estado que a janela usa num quadro.
#[derive(Clone, Default)]
struct Snap {
    overlay: bool,
    recording: bool,
    dictating: bool,
    transcribing: bool,
    seconds: Option<f32>,
    transcript: String,
    targets: Vec<TerminalChoice>,
    key_present: Option<bool>,
    pending: Option<PendingInsert>,
    notice: Option<crate::state::Notice>,
}

/// Janela de gravação, ancorada no canto inferior direito.
pub struct OverlayView {
    shared: Shared,
    target: usize,
    applied: bool,
}

impl OverlayView {
    pub fn new(shared: Shared) -> Self {
        Self {
            shared,
            target: 0,
            applied: false,
        }
    }

    fn snapshot(&self) -> Snap {
        self.shared
            .with(|state| Snap {
                overlay: state.overlay,
                recording: state.recorder.is_some() || state.recording_fixture,
                dictating: state.gesture.is_active(),
                transcribing: state.transcribing,
                seconds: state.capture.as_ref().map(|capture| capture.seconds),
                transcript: state.transcript.clone(),
                targets: state.targets.clone(),
                key_present: state.key_present,
                pending: state.pending_insert.clone(),
                notice: state.notice.clone(),
            })
            .unwrap_or_default()
    }
}

impl View for OverlayView {
    fn ui(&mut self, ui: &mut egui::Ui, cx: &mut Ctx<'_>) {
        if overlay_fixture_enabled() && !self.applied {
            self.applied = true;
            actions::apply_fixtures(&self.shared);
        }
        let snap = self.snapshot();
        if !snap.overlay {
            return;
        }
        let level = if fixtures_enabled() {
            0.42
        } else {
            actions::current_level(&self.shared)
        };
        let title = cx.t("voice-overlay-title");
        egui::Window::new(title)
            .collapsible(false)
            .resizable(false)
            .anchor(egui::Align2::RIGHT_BOTTOM, egui::vec2(-16.0, -16.0))
            .default_width(380.0)
            .show(ui.ctx(), |ui| {
                self.body(ui, cx, &snap, level);
            });
        self.confirm(ui.ctx(), cx, &snap);
    }
}

impl OverlayView {
    fn body(&mut self, ui: &mut egui::Ui, cx: &mut Ctx<'_>, snap: &Snap, level: f32) {
        let state_key = if snap.transcribing {
            "voice-state-transcribing"
        } else if snap.recording {
            "voice-state-recording"
        } else if snap.seconds.is_some() {
            "voice-state-captured"
        } else {
            "voice-state-idle"
        };
        let mut state_text = cx.t(state_key);
        if let Some(seconds) = snap.seconds.filter(|_| !snap.recording) {
            state_text = format!("{state_text} · {seconds:.1} s");
        }
        ui.label(egui::RichText::new(state_text).strong());
        let dictation_key = if snap.dictating { "voice-dictation-on" } else { "voice-dictation-off" };
        ui.label(egui::RichText::new(cx.t(dictation_key)).weak().small());
        if snap.recording {
            progress_bar(ui, level);
        }

        ui.horizontal_wrapped(|ui| {
            if snap.recording {
                if button(ui, ButtonKind::Danger, &cx.t("voice-action-stop")).clicked() {
                    let _ = actions::stop_recording(cx, &self.shared);
                }
            } else if button(ui, ButtonKind::Primary, &cx.t("voice-action-record")).clicked() {
                let _ = actions::start_recording(cx, &self.shared);
            }
            if snap.seconds.is_some()
                && !snap.transcribing
                && button(ui, ButtonKind::Secondary, &cx.t("voice-action-transcribe")).clicked()
            {
                let _ = actions::transcribe(cx, &self.shared);
            }
            if button(ui, ButtonKind::Ghost, &cx.t("voice-action-discard")).clicked() {
                actions::discard(&self.shared);
            }
        });
        if snap.seconds.is_some() && snap.key_present == Some(false) {
            ui.label(egui::RichText::new(cx.t("voice-hint-no-key")).weak());
        }

        if !snap.transcript.is_empty() || snap.seconds.is_some() {
            let mut text = snap.transcript.clone();
            let changed = text_area(
                ui,
                &mut text,
                &cx.t("voice-transcript-label"),
                &cx.t("voice-transcript-help"),
                4,
            )
            .changed();
            if changed {
                self.shared.with(|state| state.transcript = text);
            }
            self.insert_row(ui, cx, snap);
        }

        if let Some(notice) = &snap.notice {
            let text = cx.t_args(&notice.key, &[("detail", notice.detail.as_str())]);
            let color = if notice.key.ends_with("error") || notice.key.contains("-error-") {
                ui.visuals().error_fg_color
            } else {
                ui.visuals().text_color()
            };
            ui.label(egui::RichText::new(text).color(color));
        }
    }

    /// Escolha do destino e o botão de inserir. Inserir só deixa a confirmação pendente.
    fn insert_row(&mut self, ui: &mut egui::Ui, cx: &mut Ctx<'_>, snap: &Snap) {
        let options = target_options(cx, snap);
        let labels: Vec<&str> = options.iter().map(|(label, _, _)| label.as_str()).collect();
        ui.horizontal_wrapped(|ui| {
            ui.label(cx.t("voice-target-label"));
            if labels.is_empty() {
                ui.label(egui::RichText::new(cx.t("voice-target-none")).weak());
                return;
            }
            let mut index = self.target.min(labels.len() - 1);
            select_labeled(ui, &mut index, &cx.t("voice-target-label"), &labels);
            self.target = index;
            let enabled = !snap.transcript.trim().is_empty();
            ui.add_enabled_ui(enabled, |ui| {
                if button(ui, ButtonKind::Primary, &cx.t("voice-action-insert")).clicked() {
                    let (_, kind, terminal_id) = options[index].clone();
                    let _ = actions::request_insert(&self.shared, kind, terminal_id);
                }
            });
        });
    }

    /// Confirmação antes de inserir: mostra o destino e o texto.
    fn confirm(&mut self, ctx: &egui::Context, cx: &mut Ctx<'_>, snap: &Snap) {
        let Some(pending) = &snap.pending else {
            return;
        };
        let target = target_label(cx, snap, pending);
        let mut confirm = false;
        let mut cancel = false;
        egui::Window::new(cx.t("voice-confirm-title"))
            .collapsible(false)
            .resizable(false)
            .anchor(egui::Align2::CENTER_CENTER, egui::Vec2::ZERO)
            .show(ctx, |ui| {
                ui.label(cx.t_args("voice-confirm-body", &[("target", target.as_str())]));
                ui.label(egui::RichText::new(&pending.text).monospace());
                ui.horizontal(|ui| {
                    confirm = button(ui, ButtonKind::Primary, &cx.t("voice-action-confirm")).clicked();
                    cancel = button(ui, ButtonKind::Ghost, &cx.t("voice-action-cancel")).clicked();
                });
            });
        if confirm {
            let _ = actions::confirm_insert(cx, &self.shared);
        } else if cancel {
            self.shared.with(|state| state.pending_insert = None);
        }
    }
}

/// Seção "Voz" dos Ajustes: as quatro configurações e o campo da chave da OpenAI.
pub struct SettingsView {
    shared: Shared,
    key_text: String,
    started: bool,
}

impl SettingsView {
    pub fn new(shared: Shared) -> Self {
        Self {
            shared,
            key_text: String::new(),
            started: false,
        }
    }
}

impl View for SettingsView {
    fn ui(&mut self, ui: &mut egui::Ui, cx: &mut Ctx<'_>) {
        if !self.started {
            self.started = true;
            actions::refresh_key_status(cx, &self.shared);
        }
        let settings = Settings::read(cx);
        ui.heading(cx.t("voice-section-title"));
        ui.label(egui::RichText::new(cx.t("voice-section-intro")).weak());

        let mut enabled = settings.enabled;
        if toggle(ui, &mut enabled, &cx.t("setting-voice-mode-enabled-label")).changed() {
            let _ = cx.set_setting("voice.mode.enabled", SettingValue::Bool(enabled));
        }

        let engine_options = [cx.t("voice-engine-windows"), cx.t("voice-engine-whisper-cpp")];
        let engine_refs: Vec<&str> = engine_options.iter().map(String::as_str).collect();
        let mut engine = usize::from(settings.engine == "whisper_cpp");
        ui.horizontal_wrapped(|ui| {
            ui.label(cx.t("setting-voice-input-engine-label"));
            if select_labeled(ui, &mut engine, &cx.t("setting-voice-input-engine-label"), &engine_refs).changed() {
                let value = if engine == 1 { "whisper_cpp" } else { "windows" };
                let _ = cx.set_setting("voice.input.engine", SettingValue::Text(value.to_owned()));
            }
        });
        if settings.engine == "whisper_cpp" {
            ui.label(egui::RichText::new(cx.t("voice-engine-unavailable")).weak());
        }

        let language_options = [
            cx.t("voice-language-auto"),
            cx.t("voice-language-pt-br"),
            cx.t("voice-language-en"),
        ];
        let language_refs: Vec<&str> = language_options.iter().map(String::as_str).collect();
        let language_values = ["auto", "pt_BR", "en"];
        let mut language = language_values
            .iter()
            .position(|value| *value == settings.language)
            .unwrap_or(0);
        ui.horizontal_wrapped(|ui| {
            ui.label(cx.t("setting-voice-input-language-label"));
            if select_labeled(ui, &mut language, &cx.t("setting-voice-input-language-label"), &language_refs).changed() {
                let value = language_values[language];
                let _ = cx.set_setting("voice.input.language", SettingValue::Text(value.to_owned()));
            }
        });

        let mut keep = settings.keep_audio;
        if toggle(ui, &mut keep, &cx.t("setting-voice-privacy-keep_audio-label")).changed() {
            let _ = cx.set_setting("voice.privacy.keep_audio", SettingValue::Bool(keep));
        }
        ui.label(egui::RichText::new(cx.t("voice-privacy-note")).weak().small());

        ui.separator();
        self.key(ui, cx);
    }
}

impl SettingsView {
    fn key(&mut self, ui: &mut egui::Ui, cx: &mut Ctx<'_>) {
        let snap = self
            .shared
            .with(|state| (state.key_present, state.notice.clone()))
            .unwrap_or((None, None));
        let status_key = match snap.0 {
            Some(true) => "voice-key-present",
            Some(false) => "voice-key-absent",
            None => "voice-key-unknown",
        };
        ui.label(cx.t("voice-key-label"));
        ui.label(egui::RichText::new(cx.t(status_key)).weak());
        ui.scope(|ui| {
            ui.set_max_width(320.0);
            secret_field(
                ui,
                &mut self.key_text,
                &cx.t("voice-key-label"),
                &cx.t("voice-key-hint"),
            );
        });
        ui.horizontal(|ui| {
            if button(ui, ButtonKind::Primary, &cx.t("voice-key-save")).clicked() && !self.key_text.is_empty() {
                let key = std::mem::take(&mut self.key_text);
                let _ = actions::save_key(cx, &self.shared, key);
            }
            if button(ui, ButtonKind::Danger, &cx.t("voice-key-delete")).clicked() {
                let _ = actions::delete_key(cx, &self.shared);
            }
        });
        if let Some(notice) = snap.1 {
            ui.label(egui::RichText::new(cx.t(&notice.key)).weak());
        }
    }
}

/// Destinos possíveis: terminais do projeto, e editor e chat quando os comandos existem.
fn target_options(cx: &Ctx<'_>, snap: &Snap) -> Vec<(String, InsertKind, Option<String>)> {
    let mut options: Vec<(String, InsertKind, Option<String>)> = snap
        .targets
        .iter()
        .map(|target| (target.title.clone(), InsertKind::Terminal, Some(target.id.clone())))
        .collect();
    if command_exists(cx, "editor.insert") {
        options.push((cx.t("voice-target-editor"), InsertKind::Editor, None));
    }
    if command_exists(cx, "chat.insert") {
        options.push((cx.t("voice-target-chat"), InsertKind::Chat, None));
    }
    options
}

fn target_label(cx: &Ctx<'_>, snap: &Snap, pending: &PendingInsert) -> String {
    match pending.kind {
        InsertKind::Terminal => snap
            .targets
            .iter()
            .find(|target| Some(&target.id) == pending.terminal_id.as_ref())
            .map(|target| target.title.clone())
            .unwrap_or_else(|| cx.t("voice-target-none")),
        InsertKind::Editor => cx.t("voice-target-editor"),
        InsertKind::Chat => cx.t("voice-target-chat"),
    }
}

fn command_exists(cx: &Ctx<'_>, id: &str) -> bool {
    cx.registry()
        .commands()
        .iter()
        .any(|command| command.id.as_str() == id)
}
