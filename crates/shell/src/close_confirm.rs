//! Close confirmation modal and active-work detection for `app.close.confirm`.

use crate::app::ForjaApp;
use crate::tray::{self, TrayAction};
use eframe::egui::{self, ViewportCommand};
use forja_kernel::{Args, Runtime};
use forja_ui_kit::{button, modal_explicit_close, wrapped_label, ButtonKind};
use serde_json::Value;

/// Summary of work currently running in the app.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct ActiveWork {
    pub helpers: usize,
    pub routines: usize,
}

fn query_json(runtime: &mut Runtime, name: &str) -> Option<Value> {
    if let Ok(value) = runtime.call_method(name, serde_json::Map::new()) {
        return Some(value);
    }
    if runtime.registry().command(name).is_some() {
        return runtime.run_command(name, Args::new()).ok();
    }
    None
}

impl ActiveWork {
    pub fn any(self) -> bool {
        self.helpers > 0 || self.routines > 0
    }

    /// Queries registered modules for active helpers, active chat work, or running/queued routines.
    pub fn query(runtime: &mut Runtime) -> Self {
        let mut helpers = 0usize;
        let mut routines = 0usize;

        if let Some(value) = query_json(runtime, "helper.status") {
            if let Some(runs) = value.get("runs").and_then(Value::as_array) {
                helpers += runs
                    .iter()
                    .filter(|run| run.get("status").and_then(Value::as_str) != Some("finished"))
                    .count();
            }
        }

        if let Some(value) = query_json(runtime, "chat.status") {
            if value.get("status").and_then(Value::as_str) == Some("working") {
                helpers += 1;
            }
        }

        if let Some(value) = query_json(runtime, "routine.list") {
            let running = value.get("running").and_then(Value::as_u64).unwrap_or(0);
            let queued = value.get("queued").and_then(Value::as_u64).unwrap_or(0);
            routines += (running + queued) as usize;
        }

        Self { helpers, routines }
    }
}

/// Returns `true` when closing the window should pause and ask the user first.
pub fn should_prompt_close(
    confirm_setting: bool,
    work: ActiveWork,
    quitting: bool,
    confirmed: bool,
    screenshot: bool,
) -> bool {
    !screenshot && !quitting && !confirmed && confirm_setting && work.any()
}

/// Result chosen inside the close confirmation modal.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum CloseChoice {
    None,
    CloseAnyway,
    MinimizeToTray,
    Cancel,
}

/// Localized labels passed from the shell i18n bundle into [`render_close_confirm_modal`].
pub struct CloseConfirmTexts {
    pub title: String,
    pub body: String,
    pub helpers_line: String,
    pub routines_line: String,
    pub close_anyway: String,
    pub minimize_to_tray: String,
    pub cancel: String,
}

/// Draws the in-app egui close-confirmation dialog via `forja_ui_kit::modal_explicit_close`.
pub fn render_close_confirm_modal(
    ctx: &egui::Context,
    work: ActiveWork,
    tray_enabled: bool,
    texts: &CloseConfirmTexts,
) -> CloseChoice {
    let mut open = true;
    let mut choice = CloseChoice::None;
    modal_explicit_close(ctx, &mut open, &texts.title, |ui| {
        ui.set_min_width(400.0);
        ui.set_max_width(480.0);
        let font = ui
            .style()
            .text_styles
            .get(&egui::TextStyle::Body)
            .cloned()
            .unwrap_or_else(|| egui::FontId::proportional(14.0));
        let color = ui.visuals().text_color();
        wrapped_label(ui, &texts.body, font, color);
        if work.helpers > 0 || work.routines > 0 {
            ui.add_space(8.0);
            if work.helpers > 0 {
                ui.label(&texts.helpers_line);
            }
            if work.routines > 0 {
                ui.label(&texts.routines_line);
            }
        }
        ui.add_space(12.0);
        ui.horizontal(|ui| {
            if button(ui, ButtonKind::Danger, &texts.close_anyway).clicked() {
                choice = CloseChoice::CloseAnyway;
            }
            if tray_enabled && button(ui, ButtonKind::Secondary, &texts.minimize_to_tray).clicked()
            {
                choice = CloseChoice::MinimizeToTray;
            }
            if button(ui, ButtonKind::Ghost, &texts.cancel).clicked() {
                choice = CloseChoice::Cancel;
            }
        });
    });
    if choice == CloseChoice::None && !open {
        CloseChoice::Cancel
    } else {
        choice
    }
}

impl ForjaApp {
    pub(crate) fn hide_to_tray(&mut self, ctx: &egui::Context) {
        ctx.send_viewport_cmd(ViewportCommand::Visible(false));
        if self.driver.is_none() {
            let open = self.text("shell.tray.show");
            let quit = self.text("shell.tray.quit");
            let tip = self.text("shell.tray.tip");
            tray::ensure(&open, &quit, &tip);
        }
    }

    pub(crate) fn close_confirm_modal(&mut self, ctx: &egui::Context) {
        let Some(work) = self.close_confirm else {
            return;
        };
        let tray_enabled = self.setting_bool("app.tray.enabled", false) && self.screenshot.is_none();
        let helpers = work.helpers.to_string();
        let routines = work.routines.to_string();
        let texts = CloseConfirmTexts {
            title: self.text("shell.close.confirm.title"),
            body: self.text("shell.close.confirm.body"),
            helpers_line: self.text_args("shell.close.confirm.helpers", &[("count", &helpers)]),
            routines_line: self.text_args("shell.close.confirm.routines", &[("count", &routines)]),
            close_anyway: self.text("shell.close.confirm.close"),
            minimize_to_tray: self.text("shell.close.confirm.tray"),
            cancel: self.text("shell.close.confirm.cancel"),
        };
        match render_close_confirm_modal(ctx, work, tray_enabled, &texts) {
            CloseChoice::None => {}
            CloseChoice::CloseAnyway => {
                self.close_confirm = None;
                self.close_confirmed = true;
                self.quitting = true;
                self.close_pending = true;
                ctx.request_repaint();
            }
            CloseChoice::MinimizeToTray => {
                self.close_confirm = None;
                self.hide_to_tray(ctx);
            }
            CloseChoice::Cancel => {
                self.close_confirm = None;
            }
        }
    }

    pub(crate) fn handle_close(&mut self, ctx: &egui::Context) {
        let tray_enabled = self.setting_bool("app.tray.enabled", false) && self.screenshot.is_none();
        if !tray_enabled {
            tray::remove();
        } else if self.driver.is_none() {
            match tray::poll() {
                Some(TrayAction::Show) => {
                    ctx.send_viewport_cmd(ViewportCommand::Visible(true));
                    ctx.send_viewport_cmd(ViewportCommand::Focus);
                }
                Some(TrayAction::Quit) => {
                    self.quitting = true;
                    self.close_confirm = None;
                    self.close_pending = true;
                    tray::remove();
                }
                None => {}
            }
        }

        let viewport_close = ctx.input(|input| input.viewport().close_requested());
        let driver_close = self
            .driver
            .as_mut()
            .is_some_and(|driver| driver.take_close_requested());
        let closing = viewport_close || driver_close || self.close_pending;

        // Test-driver windows sit off-screen on purpose: never remember that position.
        if self.screenshot.is_none()
            && self.driver.is_none()
            && self.setting_bool("appearance.window.remember_geometry", true)
            && (closing || self.geometry_checked.elapsed() >= std::time::Duration::from_secs(1))
        {
            self.geometry_checked = std::time::Instant::now();
            if let Some(geometry) =
                ctx.input(|input| crate::window_geometry::Geometry::from_viewport(input.viewport()))
            {
                if geometry.on_screen() {
                    self.geometry_writer.record(&mut self.runtime, geometry);
                }
            }
        }

        if self.close_allowed || !closing {
            return;
        }

        self.sync_shell_store();

        let confirm_setting = self.setting_bool("app.close.confirm", true);
        let work = if confirm_setting && !self.quitting && !self.close_confirmed && self.screenshot.is_none() {
            ActiveWork::query(&mut self.runtime)
        } else {
            ActiveWork::default()
        };
        if should_prompt_close(
            confirm_setting,
            work,
            self.quitting,
            self.close_confirmed,
            self.screenshot.is_some(),
        ) {
            if viewport_close {
                ctx.send_viewport_cmd(ViewportCommand::CancelClose);
            }
            self.close_pending = false;
            self.close_confirm = Some(work);
            return;
        }

        if tray_enabled && !self.quitting {
            if viewport_close {
                ctx.send_viewport_cmd(ViewportCommand::CancelClose);
            }
            self.close_pending = false;
            self.hide_to_tray(ctx);
            return;
        }

        let flush_driver = !self.driver_close_flushed
            && self
                .driver
                .as_mut()
                .is_some_and(|driver| driver.finish_closing_if_pending(ctx));
        if flush_driver {
            self.driver_close_flushed = true;
        }

        if self.geometry_writer.busy() || self.shell_writer.busy() || flush_driver {
            if viewport_close {
                ctx.send_viewport_cmd(ViewportCommand::CancelClose);
            }
            self.close_pending = true;
            ctx.request_repaint_after(std::time::Duration::from_millis(25));
        } else if driver_close || self.close_pending {
            self.close_pending = false;
            self.close_allowed = true;
            ctx.send_viewport_cmd(ViewportCommand::Close);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use forja_kernel::{Module, ModuleManifest, Registrar, RuntimeOptions};
    use serde_json::json;

    struct HelperStub {
        runs: Value,
    }

    impl Module for HelperStub {
        fn manifest(&self) -> ModuleManifest {
            ModuleManifest::new("helper", "test.helper.name", "0.1.0")
        }

        fn register(&mut self, r: &mut Registrar<'_>) {
            r.i18n("test-helper-name = Helper\n", "test-helper-name = Ajudante\n");
            let runs = self.runs.clone();
            r.command("helper.status", "test.helper.name", move |_cx, _| {
                Ok(json!({ "runs": runs }))
            });
        }
    }

    struct RoutineStub {
        running: usize,
        queued: usize,
    }

    impl Module for RoutineStub {
        fn manifest(&self) -> ModuleManifest {
            ModuleManifest::new("routine", "test.routine.name", "0.1.0")
        }

        fn register(&mut self, r: &mut Registrar<'_>) {
            use forja_kernel::forja_protocol::{Method, MethodEffect, ResultSchema};
            r.i18n("test-routine-name = Routine\n", "test-routine-name = Rotina\n");
            let running = self.running;
            let queued = self.queued;
            r.rpc(
                Method::new("routine.list", "test.routine.name")
                    .effect(MethodEffect::Read)
                    .idempotent(true)
                    .result(ResultSchema::new("test.routine.name")),
                move |_cx, _| {
                    Ok(json!({ "routines": [], "running": running, "queued": queued, "loaded": true }))
                },
            );
        }
    }

    struct ChatStub {
        status: &'static str,
    }

    impl Module for ChatStub {
        fn manifest(&self) -> ModuleManifest {
            ModuleManifest::new("chat", "test.chat.name", "0.1.0")
        }

        fn register(&mut self, r: &mut Registrar<'_>) {
            r.i18n("test-chat-name = Chat\n", "test-chat-name = Conversa\n");
            let status = self.status;
            r.command("chat.status", "test.chat.name", move |_cx, _| {
                Ok(json!({ "status": status }))
            });
        }
    }

    fn texts() -> CloseConfirmTexts {
        CloseConfirmTexts {
            title: "Trabalho em andamento".into(),
            body: "Há ajudantes ou rotinas em execução.".into(),
            helpers_line: "Ajudantes ativos: 1".into(),
            routines_line: "Rotinas ativas: 1".into(),
            close_anyway: "Fechar mesmo assim".into(),
            minimize_to_tray: "Minimizar para a bandeja".into(),
            cancel: "Cancelar".into(),
        }
    }

    fn modal_button_labels(tray_enabled: bool) -> Vec<String> {
        let ctx = egui::Context::default();
        let _ = forja_ui_kit::install_fonts(&ctx);
        ctx.enable_accesskit();
        let work = ActiveWork {
            helpers: 1,
            routines: 1,
        };
        let t = texts();
        let mut last_output = None;
        for _ in 0..2 {
            let raw = egui::RawInput {
                screen_rect: Some(egui::Rect::from_min_size(
                    egui::Pos2::ZERO,
                    egui::vec2(960.0, 640.0),
                )),
                ..Default::default()
            };
            last_output = Some(ctx.run(raw, |ctx| {
                let _ = render_close_confirm_modal(ctx, work, tray_enabled, &t);
            }));
        }
        last_output
            .and_then(|out| out.platform_output.accesskit_update)
            .map(|update| {
                update
                    .nodes
                    .into_iter()
                    .filter_map(|(_, node)| node.label().map(str::to_owned))
                    .collect()
            })
            .unwrap_or_default()
    }

    #[test]
    fn query_counts_active_helpers_chat_and_routines_ignoring_finished() {
        let mut rt = Runtime::new(
            vec![
                Box::new(HelperStub {
                    runs: json!([
                        { "terminal_id": "t1", "status": "working" },
                        { "terminal_id": "t2", "status": "idle" },
                        { "terminal_id": "t3", "status": "finished" },
                    ]),
                }),
                Box::new(RoutineStub {
                    running: 1,
                    queued: 2,
                }),
                Box::new(ChatStub { status: "working" }),
            ],
            RuntimeOptions {
                data_dir: None,
                user_dir: None,
                locale: Some("pt-BR".to_owned()),
                wake: None,
            },
        )
        .expect("runtime boots");

        let work = ActiveWork::query(&mut rt);
        assert_eq!(work.helpers, 3);
        assert_eq!(work.routines, 3);
        assert!(work.any());
    }

    #[test]
    fn should_prompt_close_gates_on_setting_and_work() {
        let busy = ActiveWork {
            helpers: 1,
            routines: 0,
        };
        let idle = ActiveWork::default();
        assert!(should_prompt_close(true, busy, false, false, false));
        assert!(!should_prompt_close(false, busy, false, false, false));
        assert!(!should_prompt_close(true, idle, false, false, false));
        assert!(!should_prompt_close(true, busy, true, false, false));
        assert!(!should_prompt_close(true, busy, false, true, false));
        assert!(!should_prompt_close(true, busy, false, false, true));
    }

    #[test]
    fn modal_shows_tray_button_only_when_tray_is_enabled() {
        let without_tray = modal_button_labels(false);
        assert!(without_tray.iter().any(|l| l == "Fechar mesmo assim"));
        assert!(without_tray.iter().any(|l| l == "Cancelar"));
        assert!(!without_tray.iter().any(|l| l == "Minimizar para a bandeja"));

        let with_tray = modal_button_labels(true);
        assert!(with_tray.iter().any(|l| l == "Fechar mesmo assim"));
        assert!(with_tray.iter().any(|l| l == "Minimizar para a bandeja"));
        assert!(with_tray.iter().any(|l| l == "Cancelar"));
    }

    #[test]
    fn escape_cancels_close_confirm_modal() {
        let ctx = egui::Context::default();
        let _ = forja_ui_kit::install_fonts(&ctx);
        let work = ActiveWork {
            helpers: 0,
            routines: 1,
        };
        let t = texts();
        let frame = || egui::RawInput {
            screen_rect: Some(egui::Rect::from_min_size(
                egui::Pos2::ZERO,
                egui::vec2(960.0, 640.0),
            )),
            ..Default::default()
        };
        for _ in 0..2 {
            let _ = ctx.run(frame(), |ctx| {
                assert_eq!(
                    render_close_confirm_modal(ctx, work, false, &t),
                    CloseChoice::None
                );
            });
        }
        let mut esc = frame();
        esc.events.push(egui::Event::Key {
            key: egui::Key::Escape,
            physical_key: None,
            pressed: true,
            repeat: false,
            modifiers: egui::Modifiers::NONE,
        });
        let mut choice = CloseChoice::None;
        let _ = ctx.run(esc, |ctx| {
            choice = render_close_confirm_modal(ctx, work, false, &t);
        });
        assert_eq!(choice, CloseChoice::Cancel);
    }
}
