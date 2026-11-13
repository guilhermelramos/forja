//! Seção de Ajustes do GitHub: conectar, testar e desconectar.
//!
//! A tela não fala com a rede: cada botão chama um comando `counter_github.*`, que roda
//! em segundo plano. O token digitado sai do campo assim que o comando é enviado.

use crate::{Shared, Status};
use forja_counter_kit::error_key;
use forja_kernel::ui_contract::{egui, View};
use forja_kernel::{Args, Ctx};
use forja_ui_kit::{button, ButtonKind};
use forja_contracts::CounterError;
use serde_json::json;

pub struct ConnectionView {
    pub status: Shared,
    /// Token digitado. Vai para o cofre no comando `connect` e é apagado daqui.
    pub token: String,
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum Action {
    Connect,
    Test,
    Disconnect,
}

impl View for ConnectionView {
    fn ui(&mut self, ui: &mut egui::Ui, cx: &mut Ctx<'_>) {
        let status = self
            .status
            .lock()
            .map(|status| status.clone())
            .unwrap_or_default();
        ui.label(cx.t("counter_github-name"));
        ui.label(status_line(cx, &status));

        let mut action = None;
        ui.add_enabled_ui(!status.busy, |ui| {
            ui.add(
                egui::TextEdit::singleline(&mut self.token)
                    .password(true)
                    .hint_text(cx.t("counter_github-token-hint"))
                    .desired_width(f32::INFINITY),
            );
            ui.horizontal(|ui| {
                let can_connect = !self.token.trim().is_empty();
                ui.add_enabled_ui(can_connect, |ui| {
                    if button(ui, ButtonKind::Primary, &cx.t("counter_github-action-connect")).clicked() {
                        action = Some(Action::Connect);
                    }
                });
                if button(ui, ButtonKind::Secondary, &cx.t("counter_github-action-test")).clicked() {
                    action = Some(Action::Test);
                }
                let connected = status.account.is_some();
                ui.add_enabled_ui(connected, |ui| {
                    if button(ui, ButtonKind::Secondary, &cx.t("counter_github-action-disconnect")).clicked() {
                        action = Some(Action::Disconnect);
                    }
                });
            });
        });
        if let Some(action) = action {
            self.dispatch(cx, action);
        }
    }
}

impl ConnectionView {
    fn dispatch(&mut self, cx: &mut Ctx<'_>, action: Action) {
        if let Ok(mut status) = self.status.lock() {
            status.busy = true;
            status.error = None;
        }
        let outcome = match action {
            Action::Connect => {
                let mut args = Args::new();
                args.insert("token", json!(self.token.trim()));
                let outcome = cx.run_command("counter_github.connect", args);
                self.token.clear();
                outcome
            }
            Action::Test => cx.run_command("counter_github.test", Args::new()),
            Action::Disconnect => cx.run_command("counter_github.disconnect", Args::new()),
        };
        if let Err(error) = outcome {
            // O comando não chegou a rodar: a falha vem sem resultado de balcão.
            if let Ok(mut status) = self.status.lock() {
                status.busy = false;
                status.error = Some(CounterError::Config {
                    message: error.to_string(),
                });
            }
        }
    }
}

fn status_line(cx: &Ctx<'_>, status: &Status) -> String {
    if status.busy {
        return cx.t("counter_github-status-busy");
    }
    if let Some(error) = &status.error {
        return cx.t(&error_key("counter_github", error));
    }
    match &status.account {
        Some(account) => cx.t_args("counter_github-status-connected", &[("account", account.as_str())]),
        None => cx.t("counter_github-status-disconnected"),
    }
}

/// Chaves de texto da seção de Ajustes, declaradas com `uses_i18n` pelo módulo.
pub const I18N_KEYS: [&str; 7] = [
    "counter_github-token-hint",
    "counter_github-action-connect",
    "counter_github-action-test",
    "counter_github-action-disconnect",
    "counter_github-status-connected",
    "counter_github-status-disconnected",
    "counter_github-status-busy",
];
