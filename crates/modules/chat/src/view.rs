//! Aba "Ajudante (chat)" e composer.

use crate::diff_mode::DiffMode;
use crate::helper_picker;
use crate::model::{
    filter_mentions, ChatBlock, ChatMessage, ChatRole, ChatUsage, SessionStatus, ToolStatus,
};
use crate::profile::ChatProfile;
use crate::state::{ChatSession, ChatState};
use crate::Shared;
use forja_kernel::ui_contract::{egui, View};
use forja_kernel::{Args, Ctx};
use serde_json::json;
use std::collections::BTreeSet;
use std::path::PathBuf;
use std::sync::Arc;

/// Aba de chat de uma sessão.
pub struct ChatView {
    core: Shared,
    composer: String,
    selected_mention: usize,
    show_mentions: bool,
    attach_error: Option<String>,
    copied_hint: bool,
}

impl ChatView {
    /// Cria a view ligada ao estado.
    pub fn new(core: Shared) -> Self {
        Self {
            core,
            composer: String::new(),
            selected_mention: 0,
            show_mentions: false,
            attach_error: None,
            copied_hint: false,
        }
    }

    fn seed_preview(&self, session: &mut ChatSession) {
        if std::env::var_os("FORJA_CHAT_PREVIEW").is_none() {
            return;
        }
        session.messages = vec![
            ChatMessage {
                role: ChatRole::Assistant,
                blocks: vec![ChatBlock::ToolCall {
                    name: "Bash".to_owned(),
                    input: "cargo test -p forja-module-chat".to_owned(),
                    output: Some("14 passed; 0 failed".to_owned()),
                    status: ToolStatus::Ok,
                }],
                timestamp_ms: 0,
                usage: Some(ChatUsage {
                    input_tokens: 1240,
                    output_tokens: 386,
                    total_tokens: 1626,
                }),
            },
            ChatMessage::text(ChatRole::Assistant, "Testes verdes."),
        ];
        session.model = Some("opus".to_owned());
        session.effort = Some("high".to_owned());
        session.usage = ChatUsage {
            input_tokens: 1240,
            output_tokens: 386,
            total_tokens: 1626,
        };
        session.session_id = Some("preview-session".to_owned());
        session.files = vec![
            "src/main.rs".to_owned(),
            "src/view.rs".to_owned(),
            "src/state.rs".to_owned(),
            "README.md".to_owned(),
        ];
        session.commands = vec!["review".to_owned(), "test".to_owned()];
        session.commands_loaded = true;
        session.status = SessionStatus::Idle;
    }

    fn draw_header(
        &mut self,
        ui: &mut egui::Ui,
        cx: &mut Ctx<'_>,
        session: &mut ChatSession,
        profiles: &[ChatProfile],
        installed: &BTreeSet<String>,
        preview: bool,
    ) -> Option<String> {
        let mut switch_to = None;
        let is_installed = preview || installed.contains(&session.profile.id);
        ui.horizontal(|ui| {
            switch_to = helper_picker::draw_helper_switcher(
                ui,
                cx,
                &session.profile.id,
                &session.profile.name,
                profiles,
                installed,
            );
            let (label, color) = if !is_installed && matches!(session.status, SessionStatus::Idle) {
                (
                    cx.t("chat.view.not-installed"),
                    egui::Color32::from_rgb(0xFF, 0xC1, 0x4D),
                )
            } else {
                match session.status {
                    SessionStatus::Idle => (cx.t("chat.view.idle"), egui::Color32::from_rgb(0x6C, 0xC4, 0x8A)),
                    SessionStatus::Working => (cx.t("chat.view.working"), egui::Color32::from_rgb(0x6F, 0xA8, 0xDC)),
                    SessionStatus::Cancelled => (cx.t("chat.view.cancelled"), egui::Color32::from_rgb(0xFF, 0xC1, 0x4D)),
                    SessionStatus::Error => (cx.t("chat.view.error"), egui::Color32::from_rgb(0xE5, 0x53, 0x4B)),
                }
            };
            ui.label(egui::RichText::new(label).color(color).strong());
            if matches!(session.status, SessionStatus::Working) {
                ui.label(
                    egui::RichText::new(format!(
                        "{}s",
                        session.turn_seconds()
                    ))
                    .weak(),
                );
            }
            if session.pending() > 0 {
                ui.label(
                    egui::RichText::new(format!(
                        "{}: {}",
                        cx.t("chat.view.queued"),
                        session.pending()
                    ))
                    .weak(),
                );
            }
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                if ui.button(cx.t("chat.view.terminal")).clicked() {
                    let mut args = Args::new();
                    args.insert("profile_id", json!(session.profile.id.clone()));
                    args.insert(
                        "bench_path",
                        json!(session.bench_path.display().to_string()),
                    );
                    let command = if session.session_id.is_some()
                        && cx.registry().command("archive.resume").is_some()
                    {
                        args.insert("session_id", json!(session.session_id.clone()));
                        "archive.resume"
                    } else {
                        "helper.launch"
                    };
                    if let Err(error) = cx.run_command(command, args) {
                        self.attach_error = Some(error.to_string());
                    }
                }
                let effort_values = &session.profile.models.effort_values;
                if !effort_values.is_empty() {
                    egui::ComboBox::from_id_salt("chat-effort")
                        .selected_text(
                            session.effort.as_deref().map(|value| effort_label(cx, value)).unwrap_or_else(|| cx.t("chat.view.effort")),
                        )
                        .width(110.0)
                        .show_ui(ui, |ui| {
                            ui.selectable_value(&mut session.effort, None, cx.t("chat.view.default"));
                            for value in effort_values {
                                ui.selectable_value(
                                    &mut session.effort,
                                    Some(value.clone()),
                                    effort_label(cx, value),
                                );
                            }
                        });
                }
                if session.profile.models.model_env.is_some() || !session.profile.models.model_args.is_empty() {
                    egui::ComboBox::from_label(cx.t("chat.view.model"))
                        .selected_text(session.model.clone().unwrap_or_else(|| cx.t("chat.view.model")))
                        .show_ui(ui, |ui| {
                            ui.selectable_value(&mut session.model, None, cx.t("chat.view.default"));
                            let mut values = session.profile.models.values.clone();
                            if let Some(current) = &session.model { if !values.contains(current) { values.push(current.clone()); } }
                            for value in values { ui.selectable_value(&mut session.model, Some(value.clone()), value); }
                            ui.separator();
                            let mut custom = session.model.clone().unwrap_or_default();
                            if ui.add(egui::TextEdit::singleline(&mut custom).hint_text(cx.t("chat.view.custom-model"))).changed() {
                                session.model = (!custom.trim().is_empty()).then_some(custom);
                            }
                        });
                }
            });
        });
        ui.horizontal(|ui| {
            ui.label(
                egui::RichText::new(short_folder(&session.bench_path))
                    .monospace()
                    .weak(),
            );
        });
        switch_to
    }

    fn draw_composer(
        &mut self,
        ui: &mut egui::Ui,
        cx: &mut Ctx<'_>,
        session: &mut ChatSession,
        installed: bool,
        install_hint: Option<&str>,
        any_installed: bool,
    ) {
        ui.separator();
        if !installed {
            helper_picker::draw_uninstalled_banner(
                ui,
                cx,
                &session.profile.id,
                &session.profile.name,
                install_hint,
                any_installed,
                &mut self.copied_hint,
            );
        }
        if let Some(error) = self
            .attach_error
            .clone()
            .or_else(|| session.error.clone())
        {
            let human = helper_picker::humanize_error(cx, &session.profile.name, &error);
            let details_label = cx.t("kernel.error.details");
            forja_ui_kit::error_banner(ui, &human.summary, human.detail.as_deref(), &details_label);
        }
        if let Some(notice) = &session.notice {
            ui.label(egui::RichText::new(notice).weak());
        }
        let enter_sends = cx
            .setting("chat.input.enter_sends")
            .and_then(|value| value.as_bool())
            .unwrap_or(true);
        let mut send = false;
        let mut cancel = false;
        ui.horizontal(|ui| {
            if ui.button(cx.t("chat.view.attach")).clicked() {
                let picked = forja_kernel::test_pick::file()
                    .unwrap_or_else(|| rfd::FileDialog::new().pick_file());
                if let Some(path) = picked {
                    let limit = cx
                        .setting("chat.attachments.max_mb")
                        .and_then(|value| value.as_i64())
                        .unwrap_or(20)
                        .max(1) as u64;
                    match std::fs::metadata(&path) {
                        Ok(metadata) if metadata.len() <= limit * 1024 * 1024 => {
                            session.attachments.push(path);
                        }
                        Ok(_) => {
                            self.attach_error = Some(format!(
                                "{}: {} MB",
                                cx.t("chat.view.attachment-too-large"),
                                limit
                            ))
                        }
                        Err(error) => self.attach_error = Some(error.to_string()),
                    }
                }
            }
            for path in &session.attachments {
                ui.label(
                    egui::RichText::new(
                        path.file_name()
                            .map(|name| name.to_string_lossy().into_owned())
                            .unwrap_or_else(|| path.display().to_string()),
                    )
                    .weak(),
                );
            }
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                if matches!(session.status, SessionStatus::Working) {
                    if ui.button(cx.t("chat.view.cancel")).clicked() {
                        cancel = true;
                    }
                } else if ui.button(cx.t("chat.view.send")).clicked() {
                    send = true;
                }
            });
        });
        let paste_images = cx
            .setting("chat.attachments.paste_images")
            .and_then(|value| value.as_bool())
            .unwrap_or(true);
        let response = ui.add(
            egui::TextEdit::multiline(&mut self.composer)
                .hint_text(cx.t("chat.view.composer-hint"))
                .desired_rows(3)
                .desired_width(f32::INFINITY),
        );
        response.widget_info(|| {
            egui::WidgetInfo::labeled(
                egui::WidgetType::TextEdit,
                true,
                cx.t("chat.view.composer-hint"),
            )
        });
        if paste_images
            && response.has_focus()
            && ui.input(|input| input.modifiers.command && input.key_pressed(egui::Key::V))
        {
            self.paste_clipboard_image(cx);
        }
        if enter_sends
            && response.has_focus()
            && ui.input(|input| input.key_pressed(egui::Key::Enter) && !input.modifiers.shift)
        {
            send = true;
        }
        if send {
            if !installed {
                self.attach_error = Some(cx.t("chat.view.send-no-helper"));
            } else {
                let mut prompt = self.composer.trim().to_owned();
                for path in &session.attachments {
                    prompt.push_str(&format!(
                        "\n@{}",
                        path.display().to_string().replace('\\', "/")
                    ));
                }
                if !prompt.is_empty() {
                    let _tokens = crate::model::composer_tokens(&prompt);
                    match session.send(prompt) {
                        Ok(_) => {
                            self.composer.clear();
                            session.attachments.clear();
                            self.attach_error = None;
                        }
                        Err(error) => {
                            self.attach_error = Some(error);
                        }
                    }
                }
            }
        }
        if cancel {
            session.cancel();
        }
        self.draw_selectors(ui, cx, session);
    }

    /// Attaches the clipboard image, when there is one. The PNG is written in the
    /// background under the module store, and then listed as an attachment.
    fn paste_clipboard_image(&mut self, cx: &mut Ctx<'_>) {
        let Some(png) = crate::clipboard::clipboard_png() else {
            return;
        };
        let limit = cx
            .setting("chat.attachments.max_mb")
            .and_then(|value| value.as_i64())
            .unwrap_or(20)
            .max(1) as usize;
        if png.len() > limit * 1024 * 1024 {
            self.attach_error = Some(format!(
                "{}: {} MB",
                cx.t("chat.view.attachment-too-large"),
                limit
            ));
            return;
        }
        let Some(dir) = cx.store().path().parent().map(|modules| modules.join("chat-pastes")) else {
            self.attach_error = Some(cx.t("chat.view.paste-failed"));
            return;
        };
        let path = dir.join(format!("paste-{}.png", chrono::Utc::now().timestamp_millis()));
        let core = Arc::clone(&self.core);
        cx.spawn(
            move || -> Result<PathBuf, String> {
                std::fs::create_dir_all(&dir).map_err(|error| error.to_string())?;
                std::fs::write(&path, &png).map_err(|error| error.to_string())?;
                Ok(path)
            },
            move |_cx, result| {
                let Ok(mut state) = core.state.lock() else {
                    return;
                };
                if let Some(session) = state.active.as_mut() {
                    match result {
                        Ok(path) => session.attachments.push(path),
                        Err(error) => session.error = Some(error),
                    }
                }
            },
        );
    }

    fn draw_selectors(&mut self, ui: &mut egui::Ui, cx: &Ctx<'_>, session: &mut ChatSession) {
        let last = self
            .composer
            .split_whitespace()
            .last()
            .unwrap_or("")
            .to_owned();
        if last.starts_with('@') && (self.show_mentions || session.files.is_empty()) {
            let query = last.trim_start_matches('@');
            let matches: Vec<String> = filter_mentions(&session.files, query, 6)
                .into_iter()
                .cloned()
                .collect();
            ui.horizontal_wrapped(|ui| {
                ui.label(egui::RichText::new(cx.t("chat.view.files")).weak());
                if session.files.is_empty() {
                    ui.label(egui::RichText::new(cx.t("chat.view.loading-files")).weak());
                }
                for (index, file) in matches.iter().enumerate() {
                    let selected = index == self.selected_mention.min(matches.len().saturating_sub(1));
                    if ui
                        .selectable_label(selected, egui::RichText::new(file).monospace())
                        .clicked()
                    {
                        replace_last_token(&mut self.composer, &format!("@{file} "));
                        self.selected_mention = 0;
                    }
                }
            });
        } else if last.starts_with('/') && !last.starts_with("//") {
            let query = last.trim_start_matches('/');
            let commands = session.commands.clone();
            let matches: Vec<&String> = commands
                .iter()
                .filter(|command| command.to_ascii_lowercase().contains(&query.to_ascii_lowercase()))
                .take(6)
                .collect();
            if !matches.is_empty() {
                ui.horizontal_wrapped(|ui| {
                    ui.label(egui::RichText::new(cx.t("chat.view.commands")).weak());
                    for command in matches {
                        if ui
                            .button(format!("/{command}"))
                            .clicked()
                        {
                            replace_last_token(&mut self.composer, &format!("/{command} "));
                        }
                    }
                });
            } else {
                ui.label(
                    egui::RichText::new(cx.t("chat.view.command-hint")).weak(),
                );
            }
        }
    }

    fn poll_idle(&self, cx: &mut Ctx<'_>) {
        let minutes = cx
            .setting("helper.monitor.idle_minutes")
            .and_then(|value| value.as_i64())
            .unwrap_or(10)
            .max(0) as u64;
        let Ok(mut state) = self.core.state.lock() else {
            return;
        };
        let Some(session) = &mut state.active else {
            return;
        };
        session.poll_turn(crate::state::STREAM_BATCH_LIMIT);
        session.poll_transcript();
        if session.idle_expired(minutes) {
            session.stop_idle();
        }
        while let Some(prompt) = session.take_next_prompt() {
            if let Err(error) = session.start_prompt(prompt) {
                session.error = Some(error);
                session.queue.finish();
                break;
            }
        }
    }

    fn maybe_load_lists(&self, cx: &mut Ctx<'_>) {
        let needs_files = {
            let Ok(state) = self.core.state.lock() else {
                return;
            };
            state.active.as_ref().is_some_and(|session| {
                self.composer.contains('@') && session.files.is_empty() && !session.files_loading
            })
        };
        if needs_files {
            let bench = {
                let Ok(mut state) = self.core.state.lock() else {
                    return;
                };
                let Some(session) = &mut state.active else {
                    return;
                };
                session.files_loading = true;
                session.bench_path.clone()
            };
            let core = Arc::clone(&self.core);
            cx.spawn(
                move || ChatState::list_files(&bench, 4000),
                move |_cx, files| {
                    if let Ok(mut state) = core.state.lock() {
                        if let Some(session) = &mut state.active {
                            session.files = files;
                            session.files_loading = false;
                        }
                    }
                },
            );
        }
        let needs_commands = {
            let Ok(state) = self.core.state.lock() else {
                return;
            };
            state
                .active
                .as_ref()
                .is_some_and(|session| self.composer.contains('/') && !session.commands_loaded)
        };
        if needs_commands {
            let bench = {
                let Ok(mut state) = self.core.state.lock() else {
                    return;
                };
                let Some(session) = &mut state.active else {
                    return;
                };
                session.commands_loaded = true;
                session.bench_path.clone()
            };
            let mut args = Args::new();
            args.insert("scope", serde_json::json!("chat"));
            let from_contract = cx
                .run_command("content.commands.list", args)
                .ok()
                .and_then(|value| {
                    value
                        .get("commands")
                        .and_then(serde_json::Value::as_array)
                        .map(|items| {
                            items
                                .iter()
                                .filter_map(|item| {
                                    item.get("name")
                                        .and_then(serde_json::Value::as_str)
                                        .map(str::to_owned)
                                })
                                .collect::<Vec<_>>()
                        })
                });
            if let Some(commands) = from_contract {
                if let Ok(mut state) = self.core.state.lock() {
                    if let Some(session) = &mut state.active {
                        session.commands = commands;
                    }
                }
            } else {
                let core = Arc::clone(&self.core);
                cx.spawn(
                    move || ChatState::list_commands(&bench),
                    move |_cx, commands| {
                        if let Ok(mut state) = core.state.lock() {
                            if let Some(session) = &mut state.active {
                                session.commands = commands;
                            }
                        }
                    },
                );
            }
        }
    }
}

impl View for ChatView {
    /// Runs every frame, also while the tab is hidden, so idle stop and queued turns advance.
    fn poll(&mut self, cx: &mut Ctx<'_>) {
        self.poll_idle(cx);
    }

    fn ui(&mut self, ui: &mut egui::Ui, cx: &mut Ctx<'_>) {
        let core = Arc::clone(&self.core);
        let bench = cx
            .run_command("bench.current", Args::new())
            .ok()
            .and_then(|value| {
                value
                    .get("path")
                    .and_then(serde_json::Value::as_str)
                    .map(PathBuf::from)
            })
            .or_else(|| std::env::current_dir().ok())
            .unwrap_or_else(|| PathBuf::from("."));
        let mut state = match core.state.lock() {
            Ok(state) => state,
            Err(_) => return,
        };
        state.ensure_profiles(Some(&bench));
        helper_picker::sync_helpers(cx, &mut state, &bench);
        if !state.preview_seeded {
            if let Some(session) = &mut state.active {
                if std::env::var_os("FORJA_CHAT_PREVIEW").is_some() {
                    self.seed_preview(session);
                    self.composer = "@src/".to_owned();
                    self.show_mentions = true;
                }
            }
            state.preview_seeded = true;
        }
        drop(state);
        self.poll_idle(cx);
        self.maybe_load_lists(cx);

        let preview = std::env::var_os("FORJA_CHAT_PREVIEW").is_some();
        let (messages, header, profiles, installed_helpers, install_hint, any_installed, collapse_output, show_calls, diff_mode) = {
            let Ok(state) = core.state.lock() else {
                return;
            };
            let Some(session) = &state.active else {
                ui.label(cx.t("chat.view.no-session"));
                return;
            };
            (
                session.messages.clone(),
                HeaderInfo {
                    profile_name: session.profile.name.clone(),
                    bench_path: session.bench_path.clone(),
                    usage: session.usage,
                },
                state.profiles.clone(),
                state.installed_helpers.clone(),
                state.install_hints.get(&session.profile.id).cloned(),
                preview
                    || state
                        .profiles
                        .iter()
                        .any(|p| p.supports_stream() && state.installed_helpers.contains(&p.id)),
                cx.setting("chat.tools.collapse_output")
                    .and_then(|value| value.as_bool())
                    .unwrap_or(true),
                cx.setting("chat.tools.show_calls")
                    .and_then(|value| value.as_bool())
                    .unwrap_or(true),
                DiffMode::from_setting(
                    cx.setting("chat.diff.apply_mode")
                        .and_then(|value| value.as_str()),
                ),
            )
        };
        let session_delay = {
            let Ok(state) = core.state.lock() else {
                return;
            };
            state
                .active
                .as_ref()
                .and_then(|session| session.repaint_delay())
        };
        let mut switch_profile = None;
        egui::TopBottomPanel::top("forja-chat-header")
            .resizable(false)
            .show_inside(ui, |ui| {
                ui.label(egui::RichText::new(cx.t("chat.view.title")).weak());
                if let Ok(mut state) = core.state.lock() {
                    if let Some(session) = &mut state.active {
                        switch_profile = self.draw_header(
                            ui,
                            cx,
                            session,
                            &profiles,
                            &installed_helpers,
                            preview,
                        );
                    }
                }
            });
        if let Some(target_id) = switch_profile {
            let permission = cx
                .setting("helper.default.permissions")
                .and_then(|value| value.as_str())
                .unwrap_or("ask")
                .to_owned();
            if let Ok(mut state) = core.state.lock() {
                state.user_picked_profile = true;
                let _ = state.open(&bench, &target_id, None, permission);
            }
            self.copied_hint = false;
            self.attach_error = None;
        }
        egui::TopBottomPanel::bottom("forja-chat-composer")
            .resizable(false)
            .show_inside(ui, |ui| {
                if let Ok(mut state) = core.state.lock() {
                    if let Some(session) = &mut state.active {
                        let installed = preview || installed_helpers.contains(&session.profile.id);
                        self.draw_composer(
                            ui,
                            cx,
                            session,
                            installed,
                            install_hint.as_deref(),
                            any_installed,
                        );
                    }
                }
            });
        egui::CentralPanel::default().show_inside(ui, |ui| {
            let font_size = cx
                .setting("chat.font.size")
                .and_then(|value| value.as_i64())
                .unwrap_or(14)
                .clamp(10, 24) as f32;
            ui.scope(|ui| {
                ui.style_mut().override_text_style = Some(egui::TextStyle::Body);
                ui.style_mut().text_styles.insert(
                    egui::TextStyle::Body,
                    egui::FontId::proportional(font_size),
                );
                draw_message_list(
                    ui,
                    cx,
                    &messages,
                    &header,
                    collapse_output,
                    show_calls,
                    diff_mode,
                    std::env::var_os("FORJA_CHAT_PREVIEW").is_some(),
                );
            });
        });
        if let Some(delay) = session_delay {
            forja_ui_kit::request_repaint_after_active(ui.ctx(), delay);
        }
    }
}

struct HeaderInfo {
    profile_name: String,
    bench_path: PathBuf,
    usage: ChatUsage,
}

#[allow(clippy::too_many_arguments)]
fn draw_message_list(
    ui: &mut egui::Ui,
    cx: &Ctx<'_>,
    messages: &[ChatMessage],
    header: &HeaderInfo,
    collapse_output: bool,
    show_calls: bool,
    diff_mode: DiffMode,
    preview: bool,
) {
    let max_width = cx
        .setting("chat.message.max_width")
        .and_then(|value| value.as_i64())
        .unwrap_or(760)
        .clamp(400, 1200) as f32;
    ui.separator();
    let scroll = egui::ScrollArea::vertical()
        .stick_to_bottom(!preview)
        .auto_shrink([false, false]);
    let scroll = if preview {
        scroll.vertical_scroll_offset(0.0)
    } else {
        scroll
    };
    scroll
        .max_height((ui.available_height() - 24.0).max(60.0))
        .show(ui, |ui| {
            for message in messages {
                draw_message(ui, cx, message, max_width, collapse_output, show_calls, diff_mode);
                ui.add_space(6.0);
            }
        });
    ui.horizontal(|ui| {
        ui.label(
            egui::RichText::new(format!(
                "{} · {}",
                header.profile_name,
                short_folder(&header.bench_path)
            ))
            .weak(),
        );
        if header.usage.total_tokens > 0 {
            ui.label(
                egui::RichText::new(format!(
                    "↑{} ↓{} Σ{}",
                    header.usage.input_tokens,
                    header.usage.output_tokens,
                    header.usage.total_tokens
                ))
                .weak(),
            );
        }
    });
}

fn draw_message(
    ui: &mut egui::Ui,
    cx: &Ctx<'_>,
    message: &ChatMessage,
    max_width: f32,
    collapse_output: bool,
    show_calls: bool,
    diff_mode: DiffMode,
) {
    let (name, color) = match message.role {
        ChatRole::User => (cx.t("chat.view.you"), egui::Color32::from_rgb(0xFF, 0xC1, 0x4D)),
        ChatRole::Assistant => (
            cx.t("chat.view.assistant"),
            egui::Color32::from_rgb(0x6F, 0xA8, 0xDC),
        ),
        ChatRole::Tool => (cx.t("chat.view.tool"), egui::Color32::from_rgb(0xA6, 0x9C, 0x93)),
        ChatRole::System => (cx.t("chat.view.system"), egui::Color32::GRAY),
    };
    ui.horizontal(|ui| {
        ui.label(egui::RichText::new(name).color(color).strong());
        if message.timestamp_ms > 0 {
            if let Some(time) = chrono::DateTime::from_timestamp_millis(message.timestamp_ms) {
                ui.label(
                    egui::RichText::new(time.format("%H:%M").to_string()).weak(),
                );
            }
        }
    });
    for block in &message.blocks {
        match block {
            ChatBlock::Text(text) => {
                if matches!(message.role, ChatRole::User) {
                    egui::Frame::group(ui.style())
                        .fill(ui.visuals().faint_bg_color)
                        .show(ui, |ui| {
                            ui.set_max_width(max_width);
                            ui.label(text);
                        });
                } else {
                    ui.scope(|ui| {
                        ui.set_max_width(max_width);
                        ui.label(text);
                    });
                }
            }
            ChatBlock::Code { lang, code } => {
                ui.label(egui::RichText::new(lang).weak().monospace());
                egui::Frame::group(ui.style())
                    .fill(ui.visuals().extreme_bg_color)
                    .show(ui, |ui| {
                        ui.add(
                            egui::Label::new(egui::RichText::new(code).monospace())
                                .wrap_mode(egui::TextWrapMode::Extend),
                        );
                    });
            }
            ChatBlock::ToolCall {
                name,
                input,
                output,
                status,
            } => {
                if !show_calls {
                    continue;
                }
                let status_text = match status {
                    ToolStatus::Running => cx.t("chat.view.running"),
                    ToolStatus::Ok => cx.t("chat.view.ok"),
                    ToolStatus::Error => cx.t("chat.view.failed"),
                };
                let status_color = match status {
                    ToolStatus::Running => egui::Color32::from_rgb(0x6F, 0xA8, 0xDC),
                    ToolStatus::Ok => egui::Color32::from_rgb(0x6C, 0xC4, 0x8A),
                    ToolStatus::Error => egui::Color32::from_rgb(0xE5, 0x53, 0x4B),
                };
                egui::Frame::group(ui.style()).show(ui, |ui| {
                    ui.set_max_width(max_width);
                    ui.horizontal(|ui| {
                        ui.label(
                            egui::RichText::new(format!("🔧 {name}"))
                                .strong()
                                .monospace(),
                        );
                        ui.label(egui::RichText::new(status_text).color(status_color));
                    });
                    if !input.trim().is_empty() {
                        if collapse_output {
                            egui::CollapsingHeader::new(cx.t("chat.view.input"))
                                .id_salt(format!("tool-in-{name}"))
                                .show(ui, |ui| {
                                    ui.add(
                                        egui::Label::new(
                                            egui::RichText::new(input).monospace().weak(),
                                        )
                                        .wrap_mode(egui::TextWrapMode::Truncate),
                                    );
                                });
                        } else {
                            ui.add(
                                egui::Label::new(egui::RichText::new(input).monospace().weak())
                                    .wrap_mode(egui::TextWrapMode::Truncate),
                            );
                        }
                    }
                    if let Some(output) = output {
                        if collapse_output && output.lines().count() > 3 {
                            egui::CollapsingHeader::new(format!(
                                "{} ({} linhas)",
                                cx.t("chat.view.output"),
                                output.lines().count()
                            ))
                            .id_salt(format!("tool-out-{name}"))
                            .show(ui, |ui| {
                                ui.add(
                                    egui::Label::new(
                                        egui::RichText::new(output).monospace().weak(),
                                    )
                                    .wrap_mode(egui::TextWrapMode::Truncate),
                                );
                            });
                        } else {
                            ui.add(
                                egui::Label::new(egui::RichText::new(output).monospace().weak())
                                    .wrap_mode(egui::TextWrapMode::Truncate),
                            );
                        }
                    }
                });
            }
            ChatBlock::FileEdit { path, diff } => {
                egui::Frame::group(ui.style())
                    .fill(ui.visuals().faint_bg_color)
                    .show(ui, |ui| {
                        ui.set_max_width(max_width);
                        let title = match diff_mode.status_key() {
                            Some(key) => format!("✎ {path} · {}", cx.t(key)),
                            None => format!("✎ {path}"),
                        };
                        ui.label(egui::RichText::new(title).strong());
                        egui::CollapsingHeader::new(cx.t("chat.view.diff"))
                            .id_salt(format!("diff-{path}"))
                            .default_open(diff_mode.starts_open())
                            .show(ui, |ui| {
                                for line in diff.lines() {
                                    let color = if line.starts_with('+') {
                                        egui::Color32::from_rgb(0x6C, 0xC4, 0x8A)
                                    } else if line.starts_with('-') {
                                        egui::Color32::from_rgb(0xE5, 0x53, 0x4B)
                                    } else {
                                        ui.visuals().text_color()
                                    };
                                    ui.label(
                                        egui::RichText::new(line).monospace().color(color),
                                    );
                                }
                            });
                    });
            }
            ChatBlock::Todo { items } => {
                egui::Frame::group(ui.style()).show(ui, |ui| {
                    ui.set_max_width(max_width);
                    ui.label(egui::RichText::new(cx.t("chat.view.todos")).strong());
                    for item in items {
                        let marker = match item.status.as_str() {
                            "completed" => "✔",
                            "in_progress" => "◐",
                            _ => "○",
                        };
                        ui.label(format!("{marker} {}", item.text));
                    }
                });
            }
            ChatBlock::Thinking(text) => {
                egui::CollapsingHeader::new(cx.t("chat.view.thinking"))
                    .id_salt("chat-thinking")
                    .show(ui, |ui| {
                        ui.add(
                            egui::Label::new(
                                egui::RichText::new(text).italics().weak(),
                            )
                            .wrap_mode(egui::TextWrapMode::Truncate),
                        );
                    });
            }
        }
    }
    if let Some(usage) = message.usage {
        ui.label(
            egui::RichText::new(format!(
                "↑{} ↓{} Σ{}",
                usage.input_tokens, usage.output_tokens, usage.total_tokens
            ))
            .weak(),
        );
    }
}

fn replace_last_token(text: &mut String, replacement: &str) {
    let trimmed = text.trim_end();
    let start = trimmed
        .rfind(char::is_whitespace)
        .map(|index| index + 1)
        .unwrap_or(0);
    let mut next = String::with_capacity(text.len() + replacement.len());
    next.push_str(&text[..start]);
    next.push_str(replacement);
    *text = next;
}


fn short_folder(path: &std::path::Path) -> String {
    let text = path.to_string_lossy();
    let leaf = text.trim_end_matches(['/', '\\']).rsplit(['/', '\\']).next().unwrap_or_default();
    format!("…\\{leaf}")
}

fn effort_label(cx: &Ctx<'_>, value: &str) -> String {
    match value {
        "low" | "medium" | "high" | "xhigh" | "max" => cx.t(&format!("chat.effort.{value}")),
        _ => value.to_owned(),
    }
}

#[cfg(test)]
mod presentation_tests {
    use super::*;
    #[test]
    fn folder_label_is_short_and_has_no_session_separator() {
        assert_eq!(short_folder(std::path::Path::new("D:/a/forja/forja")), "…\\forja");
        assert_eq!(short_folder(std::path::Path::new(r"D:\a\forja\forja")), "…\\forja");
    }
    #[test]
    fn model_selector_uses_profile_choices() {
        let profile: ChatProfile = toml::from_str(crate::profile::builtin_text("claude-code").unwrap()).unwrap();
        assert!(!profile.models.values.is_empty());
        assert!(profile.models.values.iter().all(|value| !value.contains("MODEL")));
    }
}
