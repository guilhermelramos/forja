//! Página Extensões, painéis declarativos e fonte da Bigorna.

use crate::runtime::PanelSnapshot;
use crate::state::{self, EntrySnapshot, ExtensionCore, ExtensionState};
use forja_kernel::ui::AnvilHit;
use forja_kernel::ui_contract::{egui, View};
use forja_kernel::{Args, Ctx};
use serde_json::{json, Value};
use std::collections::{BTreeSet, HashMap};
use std::sync::Arc;

/// Página do trilho.
pub struct ExtensionPage {
    core: Arc<ExtensionCore>,
    open_logs: BTreeSet<String>,
    query: String,
    field_drafts: HashMap<(String, String), String>,
}

impl ExtensionPage {
    pub fn new(core: Arc<ExtensionCore>) -> Self {
        Self {
            core,
            open_logs: BTreeSet::new(),
            query: String::new(),
            field_drafts: HashMap::new(),
        }
    }

    fn filters(&mut self, ui: &mut egui::Ui, cx: &Ctx<'_>) {
        forja_ui_kit::text_field(ui, &mut self.query, &cx.t("extension.page.search"));
    }

    fn pick_folder(&self, cx: &mut Ctx<'_>) {
        let core = Arc::clone(&self.core);
        cx.spawn(
            || {
                forja_kernel::test_pick::folder()
                    .unwrap_or_else(|| rfd::FileDialog::new().pick_folder())
            },
            move |cx, picked: Option<std::path::PathBuf>| {
                if let Some(source) = picked {
                    crate::install::install_from_folder(&core, cx, source);
                }
            },
        );
    }

    fn install_notice(&self, ui: &mut egui::Ui, cx: &Ctx<'_>) {
        let Some(notice) = self.core.install_notice() else {
            return;
        };
        if notice.is_error {
            forja_ui_kit::page_state(
                ui,
                forja_ui_kit::PageState::Error,
                &cx.t("extension.page-state.error"),
                Some(&notice.summary),
            );
        } else {
            forja_ui_kit::text(ui, &notice.summary);
        }
        if let Some(detail) = notice.detail {
            forja_ui_kit::text(ui, egui::RichText::new(detail).weak().monospace());
        }
        ui.add_space(forja_ui_kit::Spacing::XS);
    }

    fn row(&mut self, ui: &mut egui::Ui, cx: &mut Ctx<'_>, entry: &EntrySnapshot) {
        let title = entry.name.resolve(cx.i18n().locale().id());
        let state_label = cx.t(&format!("extension.state.{}", entry.state.as_str()));
        ui.horizontal_wrapped(|ui| {
            ui.heading(title);
            forja_ui_kit::text(ui, egui::RichText::new(format!("{} {}", entry.version, state_label)).weak());
            if entry.dev {
                forja_ui_kit::text(ui, 
                    egui::RichText::new(cx.t("extension.badge.dev"))
                        .color(ui.visuals().warn_fg_color),
                );
            }
        });
        ui.collapsing(cx.t("extension.details"), |ui| {
            forja_ui_kit::text(ui, 
                egui::RichText::new(format!("{} · {}", entry.layer, entry.dir))
                    .monospace()
                    .weak(),
            );
            if !entry.hash.is_empty() {
                forja_ui_kit::text(ui, 
                    egui::RichText::new(format!(
                        "{}: {}",
                        cx.t("extension.hash"),
                        short(&entry.hash)
                    ))
                    .weak(),
                );
            }
        });
        if let Some(error) = &entry.error {
            forja_ui_kit::text(ui, egui::RichText::new(error).color(ui.visuals().error_fg_color));
        }
        ui.horizontal_wrapped(|ui| {
            let can_approve = matches!(
                entry.state,
                ExtensionState::Pending | ExtensionState::Changed | ExtensionState::Suspended
            );
            if can_approve
                && forja_ui_kit::button(
                    ui,
                    forja_ui_kit::ButtonKind::Primary,
                    &cx.t("extension.action.approve"),
                )
                .clicked()
            {
                self.call(cx, "extension.approve", &entry.id);
            }
            if entry.state == ExtensionState::Active
                && forja_ui_kit::button(
                    ui,
                    forja_ui_kit::ButtonKind::Secondary,
                    &cx.t("extension.action.reload"),
                )
                .clicked()
            {
                self.call(cx, "extension.reload", &entry.id);
            }
            if (entry.state != ExtensionState::Pending || entry.error.is_some())
                && forja_ui_kit::button(
                    ui,
                    forja_ui_kit::ButtonKind::Danger,
                    &cx.t("extension.action.revoke"),
                )
                .clicked()
            {
                self.call(cx, "extension.revoke", &entry.id);
            }
            let label = if self.open_logs.contains(&entry.id) {
                cx.t("extension.action.hide-logs")
            } else {
                cx.t("extension.action.show-logs")
            };
            if forja_ui_kit::button(ui, forja_ui_kit::ButtonKind::Secondary, &label).clicked() && !self.open_logs.remove(&entry.id) {
                self.open_logs.insert(entry.id.clone());
            }
        });
        if !entry.capabilities.is_empty() {
            forja_ui_kit::text(ui, egui::RichText::new(cx.t("extension.capabilities")).strong());
            for (key, scope) in &entry.capabilities {
                let label = cx.t(&format!("extension.capability.{key}"));
                let text = if scope.is_empty() {
                    format!("  {label}")
                } else {
                    format!("  {label}: {scope}")
                };
                forja_ui_kit::text(ui, egui::RichText::new(text).weak());
            }
        }
        if !entry.commands.is_empty() {
            forja_ui_kit::text(ui, egui::RichText::new(cx.t("extension.commands")).strong());
            for command in &entry.commands {
                forja_ui_kit::text(ui, format!(
                    "  {} · {}",
                    command.id,
                    command.title.resolve(cx.i18n().locale().id())
                ));
            }
        }
        self.settings(ui, cx, entry);
        if !entry.errors.is_empty() {
            forja_ui_kit::text(ui, egui::RichText::new(cx.t("extension.errors")).strong());
            for error in entry.errors.iter().rev().take(5) {
                forja_ui_kit::text(ui, 
                    egui::RichText::new(error)
                        .monospace()
                        .color(ui.visuals().error_fg_color),
                );
            }
        }
        if self.open_logs.contains(&entry.id) {
            let mut log = state::read_log(&self.core, &entry.id, 120);
            ui.add(
                egui::TextEdit::multiline(&mut log)
                    .desired_rows(8)
                    .desired_width(f32::INFINITY)
                    .font(egui::TextStyle::Monospace),
            );
        }
        ui.separator();
    }

    fn settings(&mut self, ui: &mut egui::Ui, cx: &mut Ctx<'_>, entry: &EntrySnapshot) {
        if entry.settings.is_empty() {
            return;
        }
        forja_ui_kit::text(ui, egui::RichText::new(cx.t("extension.settings")).strong());
        for setting in &entry.settings {
            let value = entry
                .values
                .get(&setting.name)
                .cloned()
                .unwrap_or(Value::Null);
            let label = setting.label.resolve(cx.i18n().locale().id());
            match setting.kind.as_str() {
                "toggle" => {
                    let mut checked = value.as_bool().unwrap_or(false);
                    if ui.checkbox(&mut checked, label).changed() {
                        state::set_setting(&self.core, &entry.id, &setting.name, json!(checked));
                    }
                }
                "choice" => {
                    let current = value.as_str().unwrap_or_default().to_owned();
                    let mut index = setting
                        .options
                        .iter()
                        .position(|option| option == &current)
                        .unwrap_or(0);
                    ui.horizontal(|ui| {
                        forja_ui_kit::text(ui, label);
                        let options: Vec<&str> =
                            setting.options.iter().map(String::as_str).collect();
                        if forja_ui_kit::select(ui, &mut index, &options).changed() {
                            if let Some(option) = setting.options.get(index) {
                                state::set_setting(
                                    &self.core,
                                    &entry.id,
                                    &setting.name,
                                    json!(option),
                                );
                            }
                        }
                    });
                }
                "number" => {
                    let mut text = value
                        .as_f64()
                        .map(|number| number.to_string())
                        .unwrap_or_default();
                    ui.horizontal(|ui| {
                        forja_ui_kit::text(ui, label);
                        if forja_ui_kit::text_field(ui, &mut text, "").changed() {
                            if let Ok(number) = text.parse::<f64>() {
                                state::set_setting(
                                    &self.core,
                                    &entry.id,
                                    &setting.name,
                                    json!(number),
                                );
                            }
                        }
                    });
                }
                _ => {
                    let key = (entry.id.clone(), setting.name.clone());
                    let mut text = self
                        .field_drafts
                        .get(&key)
                        .cloned()
                        .unwrap_or_else(|| value.as_str().unwrap_or_default().to_owned());
                    ui.horizontal(|ui| {
                        forja_ui_kit::text(ui, label);
                        if forja_ui_kit::text_field(ui, &mut text, "").changed() {
                            self.field_drafts.insert(key.clone(), text.clone());
                            state::set_setting(&self.core, &entry.id, &setting.name, json!(text));
                        }
                    });
                }
            }
            if !setting.help.en.is_empty() {
                forja_ui_kit::text(ui, egui::RichText::new(setting.help.resolve(cx.i18n().locale().id())).weak());
            }
        }
    }

    fn call(&self, cx: &mut Ctx<'_>, command: &str, id: &str) {
        let mut args = Args::new();
        args.insert("id", json!(id));
        if let Err(error) = cx.run_command(command, args) {
            eprintln!("forja: {error}");
        }
    }
}

impl View for ExtensionPage {
    fn ui(&mut self, ui: &mut egui::Ui, cx: &mut Ctx<'_>) {
        state::ensure(&self.core, cx);
        let entries = self.core.snapshot();
        let title = cx.t("extension.page.title");
        let refresh = cx.t("extension.page-state.refresh");
        let install = cx.t("extension.page.install");
        let count = cx.t_args(
            "extension.page.count",
            &[("count", &entries.len().to_string())],
        );
        let mut page = forja_ui_kit::page(
            ui,
            forja_ui_kit::PageSpec::new("extension.page", &title)
                .primary(&install)
                .refresh(&refresh)
                .footer(),
        );
        if page.response.primary_clicked {
            self.pick_folder(cx);
        }
        if page.response.refresh_clicked {
            let _ = cx.run_command("extension.rescan", Args::new());
        }
        page.filters(|ui| self.filters(ui, cx));
        page.body(|ui| {
            self.install_notice(ui, cx);
            if entries.is_empty() {
                forja_ui_kit::page_state(
                    ui,
                    forja_ui_kit::PageState::Empty,
                    &cx.t("extension.page-state.empty"),
                    Some(&cx.t("extension.page.empty")),
                );
                forja_ui_kit::text(ui, egui::RichText::new(cx.t("extension.page.empty-hint")).weak());
                return;
            }
            let query = self.query.to_ascii_lowercase();
            egui::ScrollArea::vertical().show(ui, |ui| {
                for entry in entries {
                    if !query.is_empty()
                        && !entry.id.to_ascii_lowercase().contains(&query)
                        && !entry
                            .name
                            .resolve(cx.i18n().locale().id())
                            .to_ascii_lowercase()
                            .contains(&query)
                    {
                        continue;
                    }
                    self.row(ui, cx, &entry);
                }
            });
        });
        page.footer(Some(&count), |_| {});
    }

    fn poll(&mut self, cx: &mut Ctx<'_>) {
        state::ensure(&self.core, cx);
    }
}

fn short(hash: &str) -> &str {
    hash.get(..12).unwrap_or(hash)
}

/// Painel declarativo de um slot.
pub struct ExtensionPanels {
    core: Arc<ExtensionCore>,
    slot: String,
    drafts: HashMap<String, String>,
}

impl ExtensionPanels {
    pub fn new(core: Arc<ExtensionCore>, slot: &str) -> Self {
        Self {
            core,
            slot: slot.to_owned(),
            drafts: HashMap::new(),
        }
    }

    fn panel(&mut self, ui: &mut egui::Ui, cx: &mut Ctx<'_>, panel: &PanelSnapshot) {
        ui.push_id(&panel.id, |ui| {
            ui.heading(panel.title.resolve(cx.i18n().locale().id()));
            let nodes = panel.tree.as_array().cloned().unwrap_or_default();
            for node in nodes {
                self.node(ui, cx, panel, &node);
            }
        });
        ui.separator();
    }

    fn node(&mut self, ui: &mut egui::Ui, cx: &mut Ctx<'_>, panel: &PanelSnapshot, node: &Value) {
        let kind = node.get("type").and_then(Value::as_str).unwrap_or_default();
        match kind {
            "text" => {
                let value = node
                    .get("value")
                    .and_then(Value::as_str)
                    .unwrap_or_default();
                let style = node
                    .get("style")
                    .and_then(Value::as_str)
                    .unwrap_or("normal");
                let mut text = egui::RichText::new(value);
                text = match style {
                    "title" => text.font(forja_ui_kit::TextRole::Section.font()),
                    "secondary" => text.weak(),
                    "warning" => text.color(ui.visuals().warn_fg_color),
                    "error" => text.color(ui.visuals().error_fg_color),
                    "code" => text.monospace(),
                    _ => text,
                };
                forja_ui_kit::text(ui, text);
            }
            "button" => {
                let label = node
                    .get("label")
                    .and_then(Value::as_str)
                    .unwrap_or_default();
                let primary = node
                    .get("primary")
                    .and_then(Value::as_bool)
                    .unwrap_or(false);
                let disabled = node
                    .get("disabled")
                    .and_then(Value::as_bool)
                    .unwrap_or(false);
                ui.add_enabled_ui(!disabled, |ui| {
                    let button = if primary {
                        forja_ui_kit::button(ui, forja_ui_kit::ButtonKind::Primary, label)
                    } else {
                        forja_ui_kit::button(ui, forja_ui_kit::ButtonKind::Secondary, label)
                    };
                    if button.clicked() {
                        self.action(cx, panel, node, "click", json!({}));
                    }
                });
            }
            "list" => {
                let items = node
                    .get("items")
                    .and_then(Value::as_array)
                    .cloned()
                    .unwrap_or_default();
                for item in items {
                    let id = item.get("id").and_then(Value::as_str).unwrap_or_default();
                    let label = item
                        .get("label")
                        .and_then(Value::as_str)
                        .unwrap_or_default();
                    let detail = item
                        .get("detail")
                        .and_then(Value::as_str)
                        .unwrap_or_default();
                    let response = ui.selectable_label(false, format!("{label}  {detail}"));
                    if response.clicked() {
                        self.action(cx, panel, node, "select", json!({ "item": id }));
                    }
                }
            }
            "table" => {
                let columns = node
                    .get("columns")
                    .and_then(Value::as_array)
                    .cloned()
                    .unwrap_or_default();
                let rows = node
                    .get("rows")
                    .and_then(Value::as_array)
                    .cloned()
                    .unwrap_or_default();
                egui::Grid::new(format!(
                    "{}:{}",
                    panel.id,
                    node.get("id").and_then(Value::as_str).unwrap_or_default()
                ))
                .striped(true)
                .show(ui, |ui| {
                    for column in &columns {
                        let label = column
                            .get("label")
                            .and_then(Value::as_str)
                            .unwrap_or_default();
                        forja_ui_kit::text(ui, egui::RichText::new(label).strong());
                    }
                    ui.end_row();
                    for row in rows.iter().take(1000) {
                        for column in &columns {
                            let key = column
                                .get("key")
                                .and_then(Value::as_str)
                                .unwrap_or_default();
                            let value = row
                                .get(key)
                                .map(|value| value.to_string())
                                .unwrap_or_default();
                            forja_ui_kit::text(ui, value.trim_matches('"'));
                        }
                        ui.end_row();
                    }
                });
                if node.get("action").and_then(Value::as_str).is_some() {
                    for row in rows.iter().take(1000) {
                        let id = row.get("id").and_then(Value::as_str).unwrap_or_default();
                        if ui.selectable_label(false, format!("↗ {id}")).clicked() {
                            self.action(cx, panel, node, "select", json!({ "row": id }));
                        }
                    }
                }
            }
            "form" => {
                let fields = node
                    .get("fields")
                    .and_then(Value::as_array)
                    .cloned()
                    .unwrap_or_default();
                for field in &fields {
                    self.field(ui, cx, panel, field);
                }
                if let Some(submit) = node.get("submit") {
                    let label = submit.get("label").and_then(Value::as_str).unwrap_or("OK");
                    if forja_ui_kit::button(ui, forja_ui_kit::ButtonKind::Primary, label).clicked()
                    {
                        let values: serde_json::Map<String, Value> = fields
                            .iter()
                            .filter_map(|field| {
                                let id = field.get("id").and_then(Value::as_str)?;
                                let key = format!("{}:{id}", panel.id);
                                Some((
                                    id.to_owned(),
                                    json!(self.drafts.get(&key).cloned().unwrap_or_default()),
                                ))
                            })
                            .collect();
                        self.action(cx, panel, node, "submit", json!({ "values": values }));
                    }
                }
            }
            "field" => self.field(ui, cx, panel, node),
            "divider" => {
                ui.separator();
            }
            "group" => {
                if let Some(title) = node.get("title").and_then(Value::as_str) {
                    forja_ui_kit::text(ui, egui::RichText::new(title).strong());
                }
                for child in node
                    .get("children")
                    .and_then(Value::as_array)
                    .cloned()
                    .unwrap_or_default()
                {
                    self.node(ui, cx, panel, &child);
                }
            }
            _ => {}
        }
    }

    fn field(
        &mut self,
        ui: &mut egui::Ui,
        _cx: &mut Ctx<'_>,
        panel: &PanelSnapshot,
        field: &Value,
    ) {
        let id = field.get("id").and_then(Value::as_str).unwrap_or_default();
        let label = field.get("label").and_then(Value::as_str).unwrap_or(id);
        let kind = field.get("type").and_then(Value::as_str).unwrap_or("text");
        let key = format!("{}:{id}", panel.id);
        let mut value = self.drafts.get(&key).cloned().unwrap_or_else(|| {
            field
                .get("value")
                .map(|value| value.to_string().trim_matches('"').to_owned())
                .unwrap_or_default()
        });
        ui.horizontal(|ui| {
            forja_ui_kit::text(ui, label);
            match kind {
                "boolean" => {
                    let mut checked = value == "true";
                    if ui.checkbox(&mut checked, "").changed() {
                        value = checked.to_string();
                        self.drafts.insert(key.clone(), value.clone());
                        self.action(
                            _cx,
                            panel,
                            field,
                            "change",
                            json!({ "node": id, "value": checked }),
                        );
                    }
                }
                "choice" => {
                    let options: Vec<String> = field
                        .get("options")
                        .and_then(Value::as_array)
                        .map(|items| {
                            items
                                .iter()
                                .filter_map(|item| {
                                    item.get("value")
                                        .or_else(|| item.get("label"))
                                        .and_then(Value::as_str)
                                        .map(str::to_owned)
                                })
                                .collect()
                        })
                        .unwrap_or_default();
                    let refs: Vec<&str> = options.iter().map(String::as_str).collect();
                    let mut index = options
                        .iter()
                        .position(|option| option == &value)
                        .unwrap_or(0);
                    if forja_ui_kit::select(ui, &mut index, &refs).changed() {
                        if let Some(option) = options.get(index) {
                            self.drafts.insert(key.clone(), option.clone());
                            self.action(
                                _cx,
                                panel,
                                field,
                                "change",
                                json!({ "node": id, "value": option }),
                            );
                        }
                    }
                }
                _ => {
                    if forja_ui_kit::text_field(ui, &mut value, "").changed() {
                        self.drafts.insert(key.clone(), value.clone());
                        self.action(
                            _cx,
                            panel,
                            field,
                            "change",
                            json!({ "node": id, "value": value }),
                        );
                    }
                }
            }
        });
    }

    fn action(
        &self,
        cx: &mut Ctx<'_>,
        panel: &PanelSnapshot,
        node: &Value,
        kind: &str,
        extra: Value,
    ) {
        let Some(action) = node.get("action").and_then(Value::as_str) else {
            return;
        };
        let node_id = node.get("id").and_then(Value::as_str).unwrap_or_default();
        let mut event = serde_json::Map::new();
        event.insert("node".to_owned(), json!(node_id));
        event.insert("kind".to_owned(), json!(kind));
        if let Value::Object(extra) = extra {
            for (key, value) in extra {
                event.insert(key, value);
            }
        }
        let mut args = Args::new();
        args.insert("extension", json!(panel.extension));
        args.insert("action", json!(action));
        args.insert("event", Value::Object(event));
        if let Err(error) = cx.run_command("extension.action", args) {
            eprintln!("forja: {error}");
        }
    }
}

impl View for ExtensionPanels {
    fn empty_by_nature(&self) -> bool {
        true
    }

    fn has_content(&self) -> bool {
        !state::panels(&self.core, &self.slot).is_empty()
    }

    fn ui(&mut self, ui: &mut egui::Ui, cx: &mut Ctx<'_>) {
        state::ensure(&self.core, cx);
        let panels = state::panels(&self.core, &self.slot);
        if panels.is_empty() {
            forja_ui_kit::text(ui, egui::RichText::new(cx.t("extension.panels.empty")).weak());
            return;
        }
        for panel in panels {
            self.panel(ui, cx, &panel);
        }
    }

    fn poll(&mut self, cx: &mut Ctx<'_>) {
        state::ensure(&self.core, cx);
    }
}

/// Fonte da Bigorna com os comandos das extensões.
pub struct ExtensionAnvil {
    core: Arc<ExtensionCore>,
}

impl ExtensionAnvil {
    pub fn new(core: Arc<ExtensionCore>) -> Self {
        Self { core }
    }
}

impl View for ExtensionAnvil {
    fn provides_anvil_hits(&self) -> bool {
        true
    }

    fn anvil_hits(&mut self, query: &str, cx: &mut Ctx<'_>) -> Vec<AnvilHit> {
        state::ensure(&self.core, cx);
        let locale = cx.i18n().locale().id().to_owned();
        let query = query.trim().to_ascii_lowercase();
        let mut hits = Vec::new();
        for command in state::commands(&self.core) {
            let title = command.title.resolve(&locale).to_owned();
            let search = format!("{} {}", title, command.id);
            if !query.is_empty() && !search.to_ascii_lowercase().contains(&query) {
                continue;
            }
            hits.push(AnvilHit {
                title,
                detail: command.id.clone(),
                icon_glyph: Some(forja_ui_kit::Icon::Configure.id().to_owned()),
                score: 25,
                command: "extension.run".to_owned(),
                args: json!({
                    "extension": command.extension,
                    "command": command.id,
                })
                .as_object()
                .cloned()
                .unwrap_or_default(),
            });
        }
        hits
    }

    fn poll(&mut self, cx: &mut Ctx<'_>) {
        state::ensure(&self.core, cx);
    }

    fn ui(&mut self, _ui: &mut egui::Ui, _cx: &mut Ctx<'_>) {}
}
