//! Seção de Ajustes do Jira: conectar, testar e desconectar.
//!
//! A tela não fala com a rede: cada botão chama um comando `counter_jira.*`, que roda
//! em segundo plano. O token digitado sai do campo assim que o comando é enviado.

use crate::{Shared, Status};
use forja_counter_kit::error_key;
use forja_kernel::ui_contract::{egui, View};
use forja_kernel::{Args, Ctx};
use forja_ui_kit::{button, secret_field, text_field_labeled, ButtonKind};
use forja_contracts::CounterError;
use serde_json::json;

pub struct ConnectionView {
    pub status: Shared,
    /// Site do Jira, por exemplo `acme.atlassian.net`.
    pub site: String,
    /// E-mail da conta que dá nome ao token.
    pub email: String,
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
        ui.label(cx.t("counter_jira-name"));
        ui.label(status_line(cx, &status));

        let mut action = None;
        ui.add_enabled_ui(!status.busy, |ui| {
            text_field_labeled(
                ui,
                &mut self.site,
                &cx.t("counter_jira-site-label"),
                &cx.t("counter_jira-site-hint"),
            );
            text_field_labeled(
                ui,
                &mut self.email,
                &cx.t("counter_jira-email-label"),
                &cx.t("counter_jira-email-hint"),
            );
            secret_field(
                ui,
                &mut self.token,
                &cx.t("counter_jira-token-label"),
                &cx.t("counter_jira-token-hint"),
            );
            ui.horizontal(|ui| {
                let can_connect = !self.site.trim().is_empty()
                    && !self.email.trim().is_empty()
                    && !self.token.trim().is_empty();
                ui.add_enabled_ui(can_connect, |ui| {
                    if button(ui, ButtonKind::Primary, &cx.t("counter_jira-action-connect")).clicked() {
                        action = Some(Action::Connect);
                    }
                });
                if button(ui, ButtonKind::Secondary, &cx.t("counter_jira-action-test")).clicked() {
                    action = Some(Action::Test);
                }
                let connected = status.account.is_some();
                ui.add_enabled_ui(connected, |ui| {
                    if button(ui, ButtonKind::Secondary, &cx.t("counter_jira-action-disconnect")).clicked() {
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
                args.insert("site", json!(self.site.trim()));
                args.insert("email", json!(self.email.trim()));
                let outcome = cx.run_command("counter_jira.connect", args);
                self.token.clear();
                outcome
            }
            Action::Test => cx.run_command("counter_jira.test", Args::new()),
            Action::Disconnect => cx.run_command("counter_jira.disconnect", Args::new()),
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
        return cx.t("counter_jira-status-busy");
    }
    if let Some(error) = &status.error {
        return cx.t(&error_key("counter_jira", error));
    }
    match &status.account {
        Some(account) => cx.t_args("counter_jira-status-connected", &[("account", account.as_str())]),
        None => cx.t("counter_jira-status-disconnected"),
    }
}

/// Chaves de texto da seção de Ajustes, declaradas com `uses_i18n` pelo módulo.
pub const I18N_KEYS: [&str; 12] = [
    "counter_jira-site-label",
    "counter_jira-site-hint",
    "counter_jira-email-label",
    "counter_jira-email-hint",
    "counter_jira-token-label",
    "counter_jira-token-hint",
    "counter_jira-action-connect",
    "counter_jira-action-test",
    "counter_jira-action-disconnect",
    "counter_jira-status-connected",
    "counter_jira-status-disconnected",
    "counter_jira-status-busy",
];
