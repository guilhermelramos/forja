//! Seção de Ajustes "Remoto": destinos SSH (testar, abrir, encaminhar, remover), importação do
//! `ssh_config`, e as distros do WSL (atualizar e abrir).
//!
//! Como nas outras telas: copia o estado, solta o lock e só então chama comandos.

use crate::actions;
use crate::commands;
use crate::forward::ForwardStatus;
use crate::state::{self, with, Forwards, ProbeResult, Shared};
use crate::target::{SshTarget, DEFAULT_SSH_PORT};
use crate::wsl::WslDistro;
use forja_kernel::ui_contract::{egui, View};
use forja_kernel::{Args, Ctx};
use forja_ui_kit::{button, text_field, ButtonKind};
use serde_json::json;
use std::collections::BTreeMap;
use std::time::Duration;

struct Snap {
    loaded: bool,
    targets: Vec<SshTarget>,
    probes: BTreeMap<String, ProbeResult>,
    statuses: Vec<ForwardStatus>,
    distros: Option<Vec<WslDistro>>,
    distros_loading: bool,
    busy: bool,
    notice: Option<String>,
}

fn snapshot(shared: &Shared, forwards: &Forwards) -> Snap {
    let targets = with(shared, |state| state.targets.clone()).unwrap_or_default();
    let probes = with(shared, |state| state.probes.clone()).unwrap_or_default();
    let (loaded, distros, distros_loading, busy, notice) = with(shared, |state| {
        (
            state.loaded,
            state.distros.clone(),
            state.distros_loading,
            !state.busy.is_empty(),
            state.notice.clone(),
        )
    })
    .unwrap_or((false, None, false, false, None));
    let statuses = forwards
        .lock()
        .map(|mut manager| manager.statuses(&targets))
        .unwrap_or_default();
    Snap {
        loaded,
        targets,
        probes,
        statuses,
        distros,
        distros_loading,
        busy,
        notice,
    }
}

pub struct RemoteView {
    shared: Shared,
    forwards: Forwards,
    started: bool,
    label: String,
    host: String,
    user: String,
    port: String,
    forward_port: String,
    forward_remote: String,
    forward_remote_port: String,
    error: Option<String>,
}

impl RemoteView {
    pub fn new(shared: Shared, forwards: Forwards) -> Self {
        Self {
            shared,
            forwards,
            started: false,
            label: String::new(),
            host: String::new(),
            user: String::new(),
            port: String::new(),
            forward_port: String::new(),
            forward_remote: String::new(),
            forward_remote_port: String::new(),
            error: None,
        }
    }

    fn ssh_section(&mut self, ui: &mut egui::Ui, cx: &mut Ctx<'_>, snap: &Snap) {
        ui.label(egui::RichText::new(cx.t("remote-ssh-title")).strong());
        if !state::ssh_enabled(cx) {
            ui.label(egui::RichText::new(cx.t("remote-ssh-off")).weak());
            return;
        }
        if snap.targets.is_empty() && snap.loaded {
            ui.label(egui::RichText::new(cx.t("remote-ssh-empty")).weak());
        }
        let warning = state::latency_warning_ms(cx);
        for target in &snap.targets {
            ui.horizontal(|ui| {
                ui.label(format!("{}  ·  {}", target.label, destination(target)));
                ui.label(egui::RichText::new(probe_text(cx, snap.probes.get(&target.id), warning)).small().weak());
            });
            ui.horizontal(|ui| {
                if button(ui, ButtonKind::Secondary, &cx.t("remote-action-test")).clicked() {
                    let mut args = Args::new();
                    args.insert("id", json!(target.id));
                    let _ = cx.run_command("remote.ssh.test", args);
                }
                if button(ui, ButtonKind::Secondary, &cx.t("remote-action-open")).clicked() {
                    let mut args = Args::new();
                    args.insert("id", json!(target.id));
                    let _ = cx.run_command("remote.ssh.open", args);
                }
                if button(ui, ButtonKind::Danger, &cx.t("remote-action-delete")).clicked() {
                    let mut args = Args::new();
                    args.insert("id", json!(target.id));
                    let _ = cx.run_command("remote.ssh.delete", args);
                }
            });
            for forward in &target.forwards {
                let running = snap
                    .statuses
                    .iter()
                    .any(|row| row.target_id == target.id && row.local_port == forward.local_port && matches!(row.state, crate::forward::ForwardState::Running));
                ui.horizontal(|ui| {
                    ui.label(
                        egui::RichText::new(format!(
                            "127.0.0.1:{} -> {}:{}",
                            forward.local_port, forward.remote_host, forward.remote_port
                        ))
                        .small(),
                    );
                    let (label, start) = if running {
                        (cx.t("remote-forward-stop"), false)
                    } else {
                        (cx.t("remote-forward-start"), true)
                    };
                    if button(ui, ButtonKind::Ghost, &label).clicked() {
                        let mut args = Args::new();
                        args.insert("target_id", json!(target.id));
                        args.insert("local_port", json!(forward.local_port));
                        let name = if start { "remote.forward.start" } else { "remote.forward.stop" };
                        let _ = cx.run_command(name, args);
                    }
                });
            }
            ui.add_space(4.0);
        }
        ui.horizontal(|ui| {
            if button(ui, ButtonKind::Ghost, &cx.t("remote-action-import")).clicked() {
                actions::import_config(cx, &self.shared);
            }
        });
        self.new_target_form(ui, cx);
        self.forward_form(ui, cx, snap);
    }

    fn new_target_form(&mut self, ui: &mut egui::Ui, cx: &mut Ctx<'_>) {
        ui.label(egui::RichText::new(cx.t("remote-new-target")).small().strong());
        text_field(ui, &mut self.label, &cx.t("remote-field-label"));
        text_field(ui, &mut self.host, &cx.t("remote-field-host"));
        text_field(ui, &mut self.user, &cx.t("remote-field-user"));
        text_field(ui, &mut self.port, &cx.t("remote-field-port"));
        if let Some(error) = &self.error {
            ui.colored_label(egui::Color32::from_rgb(0xE5, 0x53, 0x4B), error.as_str());
        }
        if button(ui, ButtonKind::Primary, &cx.t("remote-action-save")).clicked() {
            let port = self.port.trim().parse::<u16>().unwrap_or(DEFAULT_SSH_PORT);
            let target = json!({
                "label": self.label.trim(),
                "host": self.host.trim(),
                "user": self.user.trim(),
                "port": port,
            });
            let mut args = Args::new();
            args.insert("target", target);
            match cx.run_command("remote.ssh.save", args) {
                Ok(result) if result["saved"] == true => {
                    self.error = None;
                    self.label.clear();
                    self.host.clear();
                    self.user.clear();
                    self.port.clear();
                }
                Ok(result) => {
                    self.error = Some(match result["error"].as_str() {
                        Some("remote-error-validation") => result["message"].as_str().unwrap_or_default().to_owned(),
                        Some(key) => cx.t(key),
                        None => cx.t("remote-error-invalid"),
                    });
                }
                Err(_) => self.error = Some(cx.t("remote-error-invalid")),
            }
        }
    }

    fn forward_form(&mut self, ui: &mut egui::Ui, cx: &mut Ctx<'_>, snap: &Snap) {
        let Some(target) = snap.targets.first() else {
            return;
        };
        ui.label(egui::RichText::new(cx.t("remote-new-forward")).small().strong());
        text_field(ui, &mut self.forward_port, &cx.t("remote-field-local_port"));
        text_field(ui, &mut self.forward_remote, &cx.t("remote-field-remote_host"));
        text_field(ui, &mut self.forward_remote_port, &cx.t("remote-field-remote_port"));
        if button(ui, ButtonKind::Secondary, &cx.t("remote-action-add-forward")).clicked() {
            let local = self.forward_port.trim().parse::<u16>().unwrap_or(0);
            let remote = self.forward_remote_port.trim().parse::<u16>().unwrap_or(0);
            let mut forwards = target.forwards.clone();
            forwards.push(commands::forward_from(local, self.forward_remote.trim(), remote));
            let mut updated = target.clone();
            updated.forwards = forwards;
            let mut args = Args::new();
            args.insert("target", serde_json::to_value(&updated).unwrap_or_default());
            match cx.run_command("remote.ssh.save", args) {
                Ok(result) if result["saved"] == true => {
                    self.forward_port.clear();
                    self.forward_remote.clear();
                    self.forward_remote_port.clear();
                }
                _ => self.error = Some(cx.t("remote-error-forward")),
            }
        }
    }

    fn wsl_section(&mut self, ui: &mut egui::Ui, cx: &mut Ctx<'_>, snap: &Snap) {
        ui.separator();
        ui.label(egui::RichText::new(cx.t("remote-wsl-title")).strong());
        ui.horizontal(|ui| {
            if button(ui, ButtonKind::Secondary, &cx.t("remote-action-refresh-distros")).clicked() {
                actions::list_distros(cx, &self.shared);
            }
            if snap.distros_loading {
                forja_ui_kit::spinner_if_active(ui);
            }
        });
        match &snap.distros {
            None => {
                ui.label(egui::RichText::new(cx.t("remote-wsl-not-read")).weak());
            }
            Some(distros) if distros.is_empty() => {
                ui.label(egui::RichText::new(cx.t("remote-wsl-none")).weak());
            }
            Some(distros) => {
                for distro in distros {
                    ui.horizontal(|ui| {
                        let mark = if distro.is_default { cx.t("remote-wsl-default") } else { String::new() };
                        ui.label(format!("{}  ·  {}  ·  WSL {}  {}", distro.name, distro.state, distro.version, mark));
                        if button(ui, ButtonKind::Ghost, &cx.t("remote-action-open")).clicked() {
                            let mut args = Args::new();
                            args.insert("distro", json!(distro.name));
                            let _ = cx.run_command("remote.wsl.open", args);
                        }
                    });
                }
            }
        }
    }
}

impl View for RemoteView {
    fn ui(&mut self, ui: &mut egui::Ui, cx: &mut Ctx<'_>) {
        if !self.started {
            self.started = true;
            actions::ensure_loaded(cx, &self.shared, &self.forwards);
        }
        let snap = snapshot(&self.shared, &self.forwards);
        if snap.busy || snap.distros_loading {
            forja_ui_kit::request_repaint_after_active(ui.ctx(), Duration::from_secs(1));
        }
        ui.heading(cx.t("remote-name"));
        if let Some(notice) = &snap.notice {
            ui.colored_label(egui::Color32::from_rgb(0xFF, 0xC1, 0x4D), cx.t(notice));
        }
        if !snap.loaded {
            ui.label(cx.t("remote-loading"));
        }
        self.ssh_section(ui, cx, &snap);
        self.wsl_section(ui, cx, &snap);
    }
}

fn destination(target: &SshTarget) -> String {
    let who = if target.user.is_empty() {
        String::new()
    } else {
        format!("{}@", target.user)
    };
    format!("{who}{}:{}", target.host, target.port)
}

fn probe_text(cx: &Ctx<'_>, probe: Option<&ProbeResult>, warning_ms: u64) -> String {
    match probe {
        None => cx.t("remote-probe-never"),
        Some(result) if result.ok && result.elapsed_ms > warning_ms => {
            cx.t_args("remote-probe-slow", &[("ms", &result.elapsed_ms.to_string())])
        }
        Some(result) if result.ok => cx.t_args("remote-probe-ok", &[("ms", &result.elapsed_ms.to_string())]),
        Some(result) => match &result.error {
            Some(error) => cx.t(error.key()),
            None => cx.t("remote-error-other"),
        },
    }
}

/// Chaves de texto da seção, para o teste de contrato.
pub const USED_KEYS: &[&str] = &[
    "remote-name",
    "remote-loading",
    "remote-ssh-title",
    "remote-ssh-off",
    "remote-ssh-empty",
    "remote-action-test",
    "remote-action-open",
    "remote-action-delete",
    "remote-action-import",
    "remote-action-save",
    "remote-action-refresh-distros",
    "remote-action-add-forward",
    "remote-forward-start",
    "remote-forward-stop",
    "remote-new-target",
    "remote-new-forward",
    "remote-field-label",
    "remote-field-host",
    "remote-field-user",
    "remote-field-port",
    "remote-field-local_port",
    "remote-field-remote_host",
    "remote-field-remote_port",
    "remote-probe-never",
    "remote-probe-ok",
    "remote-probe-slow",
    "remote-wsl-title",
    "remote-wsl-not-read",
    "remote-wsl-none",
    "remote-wsl-default",
    "remote-error-invalid",
    "remote-error-forward",
    "remote-error-other",
];
