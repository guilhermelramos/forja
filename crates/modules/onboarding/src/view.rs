//! Cartão do primeiro uso e o botão que o reabre nos Ajustes.

use crate::steps::{steps, Step};
use crate::Shared;
use forja_kernel::ui_contract::egui;
use forja_kernel::{Args, Ctx, SettingValue};
use forja_ui_kit::{button, modal_explicit_close, select, text_field, toggle, ButtonKind};
use serde_json::Value;

pub struct WizardView {
    pub state: Shared,
}

pub struct SettingsView {
    pub state: Shared,
}

impl forja_kernel::ui_contract::View for WizardView {
    fn poll(&mut self, cx: &mut Ctx<'_>) {
        load_progress(&self.state, cx);
        if self.state.should_show() {
            cx.open_modal("onboarding.wizard");
        } else {
            cx.close_modal("onboarding.wizard");
        }
    }

    fn ui(&mut self, ui: &mut egui::Ui, cx: &mut Ctx<'_>) {
        load_progress(&self.state, cx);
        let mut open = self.state.should_show();
        let was_open = open;
        if open {
            let title = cx.t("onboarding.title");
            let state = self.state.clone();
            // O clique fora não fecha: o guia só termina por Escape ou pelo
            // botão de fechar, para um clique perdido não descartá-lo.
            modal_explicit_close(ui.ctx(), &mut open, &title, |ui| {
                render(ui, cx, &state);
            });
        }
        if was_open && !open {
            self.state.dismiss();
        }
    }
}

impl forja_kernel::ui_contract::View for SettingsView {
    fn ui(&mut self, ui: &mut egui::Ui, cx: &mut Ctx<'_>) {
        ui.label(cx.t("onboarding.settings.hint"));
        if button(
            ui,
            ButtonKind::Secondary,
            &cx.t("onboarding.settings.reopen"),
        )
        .clicked()
        {
            self.state.reopen();
        }
    }
}

fn load_progress(state: &Shared, cx: &mut Ctx<'_>) {
    if state.loaded() {
        return;
    }
    if let forja_kernel::store::LoadResult::Loaded(progress) = cx.store().load() {
        state.set_progress(progress);
    }
    // Lê o Harbor em segundo plano (módulo import); sem o módulo, nada acontece.
    if cx.registry().command("import.harbor.preview").is_some() {
        let _ = cx.run_command("import.harbor.preview", Args::new());
    }
    state.mark_loaded();
}

fn render(ui: &mut egui::Ui, cx: &mut Ctx<'_>, state: &Shared) {
    let include_helpers = cx.registry().command("helper.detect").is_some();
    let include_import = import_pending(cx);
    let sequence = steps(include_helpers, include_import);
    let mut index = state.step().min(sequence.len().saturating_sub(1));
    let step = sequence[index];
    ui.set_min_width(420.0);
    ui.label(cx.t(step_key(step)));
    ui.add_space(8.0);
    match step {
        Step::Language => language(ui, cx),
        Step::Theme => theme(ui, cx),
        Step::Folder => folder(ui, cx, state),
        Step::Import => import(ui, cx),
        Step::Helpers => helpers(ui, cx, state),
        Step::Notices => notices(ui, cx),
        Step::Checklist => checklist(ui, cx, state),
    }
    ui.add_space(12.0);
    ui.push_id("wizard.navigation", |ui| {
        ui.horizontal(|ui| {
            if index > 0
                && ui
                    .push_id("back", |ui| {
                        button(ui, ButtonKind::Ghost, &cx.t("onboarding.back"))
                    })
                    .inner
                    .clicked()
            {
                index -= 1;
                state.set_step(index);
            }
            let last = index + 1 >= sequence.len();
            let label = cx.t(if last {
                "onboarding.finish"
            } else {
                "onboarding.next"
            });
            let forward = ui
                .push_id("forward", |ui| button(ui, ButtonKind::Primary, &label))
                .inner;
            let focus_id = egui::Id::new("forja.onboarding.focus_forward");
            if ui
                .ctx()
                .data_mut(|data| data.remove_temp::<bool>(focus_id))
                .unwrap_or(false)
            {
                forward.request_focus();
            }
            if forward.clicked() {
                if !last {
                    mark(state, step, cx);
                    index += 1;
                    state.set_step(index);
                    ui.ctx().data_mut(|data| data.insert_temp(focus_id, true));
                } else {
                    state.finish();
                    let path = cx.store().path();
                    let progress = state.progress();
                    let bytes = serde_json::to_vec(&progress).unwrap_or_default();
                    cx.spawn(
                        move || {
                            let _ = forja_kernel::store::write_atomic_bytes(&path, &bytes);
                        },
                        |_, _| {},
                    );
                    let mut args = Args::new();
                    args.insert("title", Value::String(cx.t("onboarding.done.title")));
                    args.insert("body", Value::String(cx.t("onboarding.done.body")));
                    args.insert("level", Value::String("success".into()));
                    let _ = cx.run_command("notify.show", args);
                }
            }
            if ui
                .push_id("close", |ui| {
                    button(ui, ButtonKind::Ghost, &cx.t("onboarding.close"))
                })
                .inner
                .clicked()
            {
                state.dismiss();
            }
        })
    });
}

fn mark(state: &Shared, step: Step, cx: &Ctx<'_>) {
    let mut progress = state.progress();
    match step {
        Step::Language => progress.language = true,
        Step::Theme => progress.theme = true,
        Step::Folder => {
            progress.folder = cx
                .setting("onboarding.projects.dir")
                .and_then(SettingValue::as_str)
                .is_some_and(|path| !path.is_empty());
        }
        Step::Notices => progress.notices = true,
        Step::Helpers | Step::Import | Step::Checklist => {}
    }
    state.set_progress(progress);
}

fn language(ui: &mut egui::Ui, cx: &mut Ctx<'_>) {
    let labels = [
        cx.t("onboarding.language.auto"),
        cx.t("onboarding.language.pt"),
        cx.t("onboarding.language.en"),
    ];
    let refs: Vec<&str> = labels.iter().map(String::as_str).collect();
    let current = setting_str(cx, "i18n.language.mode", "auto");
    let mut index = match current.as_str() {
        "pt_BR" => 1,
        "en" => 2,
        _ => 0,
    };
    select(ui, &mut index, &refs);
    let value = ["auto", "pt_BR", "en"][index];
    if value != current {
        let _ = cx.set_setting("i18n.language.mode", SettingValue::Text(value.to_owned()));
    }
}

fn theme(ui: &mut egui::Ui, cx: &mut Ctx<'_>) {
    let labels = [
        cx.t("onboarding.theme.system"),
        cx.t("onboarding.theme.dark"),
        cx.t("onboarding.theme.light"),
    ];
    let refs: Vec<&str> = labels.iter().map(String::as_str).collect();
    let current = setting_str(cx, "appearance.theme.mode", "system");
    let mut index = match current.as_str() {
        "dark" => 1,
        "light" => 2,
        _ => 0,
    };
    select(ui, &mut index, &refs);
    let value = ["system", "dark", "light"][index];
    if value != current {
        let _ = cx.set_setting(
            "appearance.theme.mode",
            SettingValue::Text(value.to_owned()),
        );
    }
}

fn folder(ui: &mut egui::Ui, cx: &mut Ctx<'_>, state: &Shared) {
    ui.label(cx.t("onboarding.folder.hint"));
    let placeholder = cx.t("onboarding.folder.placeholder");
    let mut path = setting_str(cx, "onboarding.projects.dir", "");
    if text_field(ui, &mut path, &placeholder).changed() {
        let _ = cx.set_setting("onboarding.projects.dir", SettingValue::Text(path));
    }
    if !state.picking()
        && button(ui, ButtonKind::Secondary, &cx.t("onboarding.folder.pick")).clicked()
    {
        state.set_picking(true);
        let shared = state.clone();
        cx.spawn(
            || {
                forja_kernel::test_pick::folder()
                    .unwrap_or_else(|| rfd::FileDialog::new().pick_folder())
                    .map(|path| path.display().to_string())
            },
            move |cx, picked| {
                shared.set_picking(false);
                if let Some(path) = picked {
                    let _ = cx.set_setting("onboarding.projects.dir", SettingValue::Text(path));
                }
            },
        );
    }
}

fn helpers(ui: &mut egui::Ui, cx: &mut Ctx<'_>, state: &Shared) {
    if state.helpers_text().is_none() {
        match cx.run_command("helper.detect", Args::new()) {
            Ok(value) => state.set_helpers(summarize(&value)),
            Err(error) => state.set_helpers(error.to_string()),
        }
    }
    ui.label(cx.t("onboarding.helpers.lead"));
    if let Some(text) = state.helpers_text() {
        ui.label(text);
    }
}

/// `true` quando o módulo `import` existe e tem itens do Harbor a decidir.
fn import_pending(cx: &mut Ctx<'_>) -> bool {
    if cx.registry().command("import.harbor.status").is_none() {
        return false;
    }
    cx.run_command("import.harbor.status", Args::new())
        .ok()
        .and_then(|value| value.get("pending").and_then(Value::as_bool))
        .unwrap_or(false)
}

/// Passo "Importar do Harbor": lista os itens pendentes (todos marcados por
/// padrão), e importa os marcados ou pula. A lista vem do módulo `import`.
fn import(ui: &mut egui::Ui, cx: &mut Ctx<'_>) {
    let status = cx
        .run_command("import.harbor.status", Args::new())
        .unwrap_or(Value::Null);
    ui.label(cx.t("onboarding.import.lead"));
    if let Some(key) = status.get("error").and_then(Value::as_str) {
        ui.label(egui::RichText::new(cx.t(key)).color(egui::Color32::from_rgb(0xE5, 0x53, 0x4B)));
    }
    let items: Vec<(String, String)> = status
        .get("items")
        .and_then(Value::as_array)
        .map(|list| {
            list.iter()
                .filter(|item| item.get("imported") != Some(&Value::Bool(true)))
                .map(|item| {
                    let text_of = |name: &str| item.get(name).and_then(Value::as_str).unwrap_or("");
                    let id = text_of("id").to_owned();
                    let kind = text_of("kind");
                    let label = text_of("label");
                    let detail = text_of("detail");
                    // Tema e fonte repetem o valor no rótulo; notas mostram só a primeira linha.
                    let show_detail = !detail.is_empty()
                        && detail != label
                        && !matches!(kind, "notes" | "theme" | "font");
                    let mut text = format!("{} · {label}", cx.t(&format!("import.kind.{kind}")));
                    if show_detail {
                        text.push_str(" · ");
                        text.push_str(detail);
                    }
                    (id, text)
                })
                .collect()
        })
        .unwrap_or_default();

    let selection_id = egui::Id::new("onboarding.import.deselected");
    let mut deselected: Vec<String> = ui.data(|d| d.get_temp(selection_id)).unwrap_or_default();
    let mut changed = false;
    for (id, text) in &items {
        let mut on = !deselected.contains(id);
        if toggle(ui, &mut on, text).changed() {
            changed = true;
            if on {
                deselected.retain(|item| item != id);
            } else {
                deselected.push(id.clone());
            }
        }
    }
    if changed {
        ui.data_mut(|d| d.insert_temp(selection_id, deselected.clone()));
    }

    let decided = status.get("decided").and_then(Value::as_str);
    if decided == Some("imported") {
        ui.label(cx.t("onboarding.import.done"));
    } else if items.is_empty() {
        ui.label(cx.t("onboarding.import.none"));
    }
    if !items.is_empty() && decided.is_none() {
        ui.add_space(6.0);
        ui.horizontal(|ui| {
            if button(ui, ButtonKind::Primary, &cx.t("onboarding.import.run")).clicked() {
                let selected: Vec<Value> = items
                    .iter()
                    .filter(|(id, _)| !deselected.contains(id))
                    .map(|(id, _)| Value::String(id.clone()))
                    .collect();
                let mut args = Args::new();
                args.insert("items", Value::Array(selected));
                let _ = cx.run_command("import.harbor.run", args);
            }
            if button(ui, ButtonKind::Ghost, &cx.t("onboarding.import.skip")).clicked() {
                let _ = cx.run_command("import.harbor.skip", Args::new());
            }
        });
    }
}

fn notices(ui: &mut egui::Ui, cx: &mut Ctx<'_>) {
    ui.label(cx.t("onboarding.notices.lead"));
    let mut toast = flag(cx, "notifications.toast.enabled", true);
    if toggle(ui, &mut toast, &cx.t("onboarding.notices.toast")).changed() {
        let _ = cx.set_setting("notifications.toast.enabled", SettingValue::Bool(toast));
    }
    let mut os = flag(cx, "notifications.os.enabled", true);
    if toggle(ui, &mut os, &cx.t("onboarding.notices.windows")).changed() {
        let _ = cx.set_setting("notifications.os.enabled", SettingValue::Bool(os));
    }
}

fn checklist(ui: &mut egui::Ui, cx: &mut Ctx<'_>, state: &Shared) {
    let mut progress = state.progress();
    let mut changed = false;
    changed |= row(ui, cx, "onboarding.check.language", &mut progress.language);
    changed |= row(ui, cx, "onboarding.check.theme", &mut progress.theme);
    changed |= row(ui, cx, "onboarding.check.folder", &mut progress.folder);
    changed |= row(ui, cx, "onboarding.check.notices", &mut progress.notices);
    if changed {
        state.set_progress(progress);
    }
}

fn row(ui: &mut egui::Ui, cx: &Ctx<'_>, key: &str, value: &mut bool) -> bool {
    let label = cx.t(key);
    toggle(ui, value, &label).changed()
}

fn step_key(step: Step) -> &'static str {
    match step {
        Step::Language => "onboarding.step.language",
        Step::Theme => "onboarding.step.theme",
        Step::Folder => "onboarding.step.folder",
        Step::Import => "onboarding.step.import",
        Step::Helpers => "onboarding.step.helpers",
        Step::Notices => "onboarding.step.notices",
        Step::Checklist => "onboarding.step.checklist",
    }
}

fn setting_str(cx: &Ctx<'_>, key: &str, default: &str) -> String {
    cx.setting(key)
        .and_then(SettingValue::as_str)
        .unwrap_or(default)
        .to_owned()
}

fn flag(cx: &Ctx<'_>, key: &str, default: bool) -> bool {
    cx.setting(key)
        .and_then(SettingValue::as_bool)
        .unwrap_or(default)
}

fn summarize(value: &Value) -> String {
    if let Some(text) = value.as_str() {
        return text.to_owned();
    }
    let list = value
        .get("helpers")
        .or_else(|| value.get("profiles"))
        .and_then(Value::as_array);
    if let Some(list) = list {
        let names: Vec<&str> = list
            .iter()
            .filter_map(|item| {
                item.as_str()
                    .or_else(|| item.get("name").and_then(Value::as_str))
                    .or_else(|| item.get("id").and_then(Value::as_str))
            })
            .collect();
        if names.is_empty() {
            return value.to_string();
        }
        return names.join(", ");
    }
    value.to_string()
}
