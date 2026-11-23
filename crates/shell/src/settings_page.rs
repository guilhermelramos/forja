//! Full-page settings: catalog groups, custom sections and a persistent search/footer.
use crate::app::ForjaApp;
use crate::overlays::{apply_page_scroll, reveal_focused_control};
use crate::settings_catalog::{catalog, matches};
use crate::settings_transfer::{Operation, Transfer};
use forja_ui_kit::{egui, ButtonKind};
use std::collections::BTreeMap;

#[derive(Default)]
pub(crate) struct State {
    pub selected: String,
    pub errors: BTreeMap<String, String>,
    was_open: bool,
    focus_pending: u8,
    pub reset_confirm: bool,
    pub transfer: Transfer,
}
const SEPARATE: &[&str] = &["accounts", "trust", "mcp", "voice", "import"];

impl ForjaApp {
    pub(crate) fn settings_page_ui(&mut self, ui: &mut egui::Ui) {
        let entry = forja_ui_kit::entry_motion(
            ui.ctx(),
            egui::Id::new("forja.settings.entry"),
            forja_ui_kit::Motion::Base,
        );
        let scale = egui::lerp(0.98..=1.0, entry);
        if scale < 1.0 {
            let center = ui.available_rect_before_wrap().center();
            ui.with_visual_transform(
                egui::emath::TSTransform {
                    scaling: scale,
                    translation: center.to_vec2() * (1.0 - scale),
                },
                |ui| self.settings_page_content(ui),
            );
        } else {
            self.settings_page_content(ui);
        }
    }

    fn settings_page_content(&mut self, ui: &mut egui::Ui) {
        self.poll_settings_transfer();
        if !self.settings_page.was_open {
            self.settings_page.focus_pending = 2;
        }
        self.settings_page.was_open = true;
        let focus_search = self.settings_page.focus_pending == 1;
        self.settings_page.focus_pending = self.settings_page.focus_pending.saturating_sub(1);
        if let Some(section) = self.settings_section.take() {
            self.settings_page.selected = if SEPARATE.contains(&section.as_str()) {
                format!("section:{section}")
            } else {
                catalog().module_group(&section).to_owned()
            };
        }
        if self.settings_page.selected.is_empty() {
            self.settings_page.selected = catalog().groups[0].id.clone();
        }
        let width = ui.available_width().min(960.0);
        ui.set_width(width);
        ui.horizontal(|ui| {
            ui.set_min_height(56.0);
            ui.add(
                egui::Label::new(
                    egui::RichText::new(self.text("shell.settings.title"))
                        .font(egui::FontId::new(
                            20.0,
                            egui::FontFamily::Name("forja-semibold".into()),
                        ))
                        .strong(),
                )
                .selectable(false),
            );
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                if forja_ui_kit::button(ui, ButtonKind::Ghost, &self.text("shell.settings.back"))
                    .clicked()
                {
                    self.settings_open = false;
                }
            });
        });
        ui.add_space(8.0);
        let search_label = self.text("shell.settings.search");
        let search = ui.add(
            egui::TextEdit::singleline(&mut self.settings_query)
                .hint_text(&search_label)
                .desired_width(width),
        );
        search.widget_info(|| {
            egui::WidgetInfo::labeled(egui::WidgetType::TextEdit, ui.is_enabled(), &search_label)
        });
        if focus_search {
            search.request_focus();
        }
        ui.add_space(10.0);
        let footer_height = if self.settings_page.transfer.notice.is_some() {
            84.0
        } else {
            58.0
        };
        let height = (ui.available_height() - footer_height).max(160.0);
        let nav_width = (width * 0.22).clamp(130.0, 190.0);
        let content_width = (width - nav_width - 20.0).max(150.0);
        let query = self.settings_query.trim().to_owned();
        ui.horizontal_top(|ui| {
            ui.allocate_ui_with_layout(
                egui::vec2(nav_width, height),
                egui::Layout::top_down(egui::Align::Min),
                |ui| {
                    ui.set_width(nav_width);
                    egui::ScrollArea::vertical()
                        .id_salt("forja.settings.navigation")
                        .max_height(height)
                        .auto_shrink([false, false])
                        .show(ui, |ui| {
                            ui.spacing_mut().item_spacing.y = 2.0;
                            for group in &catalog().groups {
                                let label = group.label(self.runtime.i18n().locale());
                                if forja_ui_kit::settings_navigation(
                                    ui,
                                    label,
                                    query.is_empty() && self.settings_page.selected == group.id,
                                )
                                .clicked()
                                {
                                    self.settings_page.selected = group.id.clone();
                                    self.settings_query.clear();
                                }
                            }
                            ui.separator();
                            for module in SEPARATE {
                                let title = self
                                    .settings_views
                                    .iter()
                                    .find(|view| view.module == *module)
                                    .map(|_| {
                                        self.text(&format!("shell.settings.section.{module}"))
                                    });
                                if let Some(title) = title {
                                    let id = format!("section:{module}");
                                    if forja_ui_kit::settings_navigation(
                                        ui,
                                        &title,
                                        query.is_empty() && self.settings_page.selected == id,
                                    )
                                    .clicked()
                                    {
                                        self.settings_page.selected = id;
                                        self.settings_query.clear();
                                    }
                                }
                            }
                        });
                },
            );
            let (divider, _) =
                ui.allocate_exact_size(egui::vec2(1.0, height), egui::Sense::hover());
            ui.painter().vline(
                divider.center().x,
                divider.y_range(),
                egui::Stroke::new(1.0_f32, self.paint(self.tokens.color.border)),
            );
            forja_ui_kit::settings_viewport(ui, content_width, height, |ui| {
                ui.set_width(content_width);
                let selected = self.settings_page.selected.clone();
                let mut scroll = egui::ScrollArea::both()
                    .id_salt(("forja.settings.rows", &selected, &query))
                    .max_height(height)
                    .auto_shrink([false, false]);
                if search.changed() {
                    scroll = scroll.vertical_scroll_offset(0.0);
                }
                scroll.show(ui, |ui| {
                    ui.set_width(content_width - 4.0);
                    self.settings_group_contents(ui, &query);
                    reveal_focused_control(ui);
                    apply_page_scroll(ui);
                });
            });
        });
        ui.add_space(8.0);
        ui.separator();
        ui.horizontal_wrapped(|ui| {
            let test = self.driver.is_some() || self.screenshot.is_some() || cfg!(test);
            ui.add_enabled_ui(!test, |ui| {
                if forja_ui_kit::button(
                    ui,
                    ButtonKind::Ghost,
                    &self.text("shell.settings.open-file"),
                )
                .clicked()
                {
                    self.open_settings_file();
                }
            });
            if forja_ui_kit::button(ui, ButtonKind::Ghost, &self.text("shell.settings.export"))
                .clicked()
            {
                self.start_settings_transfer(Operation::Export);
            }
            if forja_ui_kit::button(ui, ButtonKind::Ghost, &self.text("shell.settings.import"))
                .clicked()
            {
                self.start_settings_transfer(Operation::Import);
            }
            if forja_ui_kit::button(
                ui,
                ButtonKind::Ghost,
                &self.text("shell.settings.restore-all"),
            )
            .clicked()
            {
                self.settings_page.reset_confirm = true;
            }
        });
        if let Some(key) = self.settings_page.transfer.notice {
            ui.label(self.text(key));
        }
        self.slot_rects.insert("settings".into(), ui.min_rect());
    }

    fn settings_group_contents(&mut self, ui: &mut egui::Ui, query: &str) {
        let selected = self.settings_page.selected.clone();
        let specs = self.runtime.registry().settings().to_vec();
        let mut shown = 0;
        for group in &catalog().groups {
            if query.is_empty() && selected != group.id {
                continue;
            }
            let mut rows: Vec<_> = specs
                .iter()
                .filter(|spec| {
                    catalog().group_for(spec) == group.id
                        && (query.is_empty()
                            || matches(
                                query,
                                &spec.key,
                                &self.text(&spec.label_key),
                                &self.text(&spec.help_key),
                            ))
                })
                .collect();
            rows.sort_by(|a, b| {
                catalog()
                    .order(&a.key)
                    .cmp(&catalog().order(&b.key))
                    .then(a.order.cmp(&b.order))
                    .then(a.key.cmp(&b.key))
            });
            if !query.is_empty() && rows.is_empty() {
                continue;
            }
            ui.heading(group.label(self.runtime.i18n().locale()));
            if query.is_empty() && group.id == "helpers" {
                self.settings_custom_views(ui, &group.id, false);
            }
            if query.is_empty() && group.id == "appearance" {
                let featured = [
                    "appearance.theme.mode",
                    "appearance.theme.dark_name",
                    "appearance.theme.light_name",
                    "appearance.ui.density",
                    "appearance.ui.scale",
                    "appearance.colors.accent",
                ];
                let (mut first, remaining): (Vec<_>, Vec<_>) = rows
                    .into_iter()
                    .partition(|spec| featured.contains(&spec.key.as_str()));
                first.sort_by_key(|spec| {
                    featured
                        .iter()
                        .position(|key| *key == spec.key)
                        .unwrap_or(usize::MAX)
                });
                if ui.available_width() >= 700.0 {
                    ui.columns(2, |columns| {
                        for spec in &first {
                            self.settings_row(&mut columns[0], spec);
                        }
                        self.settings_appearance_preview(&mut columns[1]);
                    });
                } else {
                    self.settings_appearance_preview(ui);
                    for spec in &first {
                        self.settings_row(ui, spec);
                    }
                }
                shown += first.len();
                rows = remaining;
            }
            for spec in rows {
                self.settings_row(ui, spec);
                shown += 1;
            }
            if query.is_empty() {
                if group.id != "helpers" {
                    self.settings_custom_views(ui, &group.id, false);
                }
                if group.id == "advanced" {
                    let notice = self.storage_notice.clone().or_else(|| {
                        self.runtime
                            .storage_restart_pending()
                            .then(|| self.text("shell.storage.restart-required"))
                    });
                    if let Some(notice) = notice {
                        forja_ui_kit::text(
                            ui,
                            forja_ui_kit::TextRole::Caption
                                .rich(&notice)
                                .color(ui.visuals().warn_fg_color),
                        );
                    }
                    if forja_ui_kit::button(
                        ui,
                        ButtonKind::Ghost,
                        &self.text("shell.command.logs-open"),
                    )
                    .clicked()
                    {
                        self.run_command("shell.logs.open");
                    }
                }
            }
        }
        if query.is_empty() {
            if let Some(module) = selected.strip_prefix("section:") {
                self.settings_custom_views(ui, module, true);
            }
        } else if shown == 0 {
            ui.label(self.text("shell.settings.empty"));
        }
    }

    fn settings_custom_views(&mut self, ui: &mut egui::Ui, target: &str, separate: bool) {
        for view in self.settings_views.iter_mut() {
            let included = if separate {
                view.module == target
            } else {
                !SEPARATE.contains(&view.module.as_str())
                    && catalog().module_group(&view.module) == target
            };
            if included {
                let mut cx = self.runtime.ctx(&view.module);
                ui.push_id(&view.id, |ui| view.view.ui(ui, &mut cx));
            }
        }
    }

    fn settings_appearance_preview(&self, ui: &mut egui::Ui) {
        let title = self.text("shell.settings.preview");
        forja_ui_kit::card(ui, Some(&title), |ui| {
            ui.horizontal_wrapped(|ui| {
                forja_ui_kit::button(
                    ui,
                    ButtonKind::Primary,
                    &self.text("shell.settings.preview-action"),
                );
                forja_ui_kit::button(
                    ui,
                    ButtonKind::Secondary,
                    &self.text("shell.settings.preview-secondary"),
                );
            });
            let sample = self.text("shell.settings.preview-list");
            let mut selected = Some(0);
            forja_ui_kit::list(
                ui,
                &[forja_ui_kit::ListItem {
                    label: &sample,
                    detail: None,
                }],
                &mut selected,
            );
            egui::Frame::new()
                .fill(self.paint(self.tokens.color.bg))
                .inner_margin(8.0_f32)
                .show(ui, |ui| {
                    ui.label(
                        egui::RichText::new(self.text("shell.settings.preview-terminal"))
                            .monospace()
                            .color(self.paint(self.tokens.color.ok)),
                    );
                });
        });
    }

    // Retains the call site for shell overlays, but no floating settings window exists.
    pub(crate) fn settings_window(&mut self, ctx: &egui::Context) {
        if !self.settings_open {
            self.settings_page.was_open = false;
            return;
        }
        let can_escape = !self.anvil_open
            && !egui::Popup::is_any_open(ctx)
            && ctx.memory(|memory| memory.top_modal_layer().is_none());
        if can_escape
            && ctx.input_mut(|input| input.consume_key(egui::Modifiers::NONE, egui::Key::Escape))
        {
            self.settings_open = false;
            self.settings_section = None;
            self.settings_page.was_open = false;
            return;
        }
        self.settings_transfer_windows(ctx);
        let mut open = self.settings_page.reset_confirm;
        let mut restore = false;
        if open {
            forja_ui_kit::modal_explicit_close(
                ctx,
                &mut open,
                &self.text("shell.settings.restore-all"),
                |ui| {
                    ui.label(self.text("shell.settings.restore-confirm"));
                    restore = forja_ui_kit::button(
                        ui,
                        ButtonKind::DangerConfirm,
                        &self.text("shell.settings.confirm-restore"),
                    )
                    .clicked();
                },
            );
        }
        self.settings_page.reset_confirm = open && !restore;
        if restore {
            let specs = self.runtime.registry().settings().to_vec();
            for spec in specs {
                self.apply_setting(&spec.key, spec.default);
            }
            self.setting_drafts.clear();
            self.settings_page.errors.clear();
        }
    }
}
