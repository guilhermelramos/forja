//! Modal dialogs used by the explorer panel (`New entry`, `Rename`, `Confirm trash`).

use forja_kernel::ui_contract::egui;
use forja_kernel::Ctx;
use std::path::Path;

/// How a small input dialog ended this frame.
#[derive(Debug, PartialEq, Eq)]
pub(crate) enum Outcome {
    Open,
    Submit,
    Cancel,
}

/// Shows the new file / new folder window, using the top keyboard layer.
pub(crate) fn new_entry_window(
    ctx: &egui::Context,
    cx: &Ctx<'_>,
    title: &str,
    name: &mut String,
    templates: &[(String, String)],
    template: &mut Option<String>,
) -> Outcome {
    let layer = egui::Id::new("forja.explorer.new_entry.keyboard");
    let focus_key = egui::Id::new("forja.explorer.new_entry.focus");
    if forja_ui_kit::consume_layer_escape(ctx, layer) {
        ctx.data_mut(|data| data.remove_temp::<bool>(focus_key));
        return Outcome::Cancel;
    }
    let focus_name = !ctx.data(|data| data.get_temp::<bool>(focus_key).unwrap_or(false));
    let label = cx.t("explorer.dialog.name.label");
    let template_label = cx.t("explorer.dialog.template.label");
    let none = cx.t("explorer.dialog.template.none");
    let ok = cx.t("explorer.dialog.confirm");
    let cancel = cx.t("explorer.dialog.cancel");
    let mut outcome = Outcome::Open;
    egui::Window::new(title)
        .id(egui::Id::new("forja.explorer.new_entry"))
        .collapsible(false)
        .resizable(false)
        .anchor(egui::Align2::CENTER_CENTER, [0.0, 0.0])
        .show(ctx, |ui| {
            forja_ui_kit::register_keyboard_layer(ui, layer);
            forja_ui_kit::text(ui, label);
            let edit = forja_ui_kit::text_field(ui, name, "");
            if focus_name {
                edit.request_focus();
                ctx.data_mut(|data| data.insert_temp(focus_key, true));
            }
            if !templates.is_empty() {
                forja_ui_kit::text(ui, template_label);
                let mut options = vec![none.as_str()];
                options.extend(templates.iter().map(|(_, name)| name.as_str()));
                let mut index = template
                    .as_ref()
                    .and_then(|selected| templates.iter().position(|(id, _)| id == selected))
                    .map(|index| index + 1)
                    .unwrap_or(0);
                if forja_ui_kit::select(ui, &mut index, &options).changed() {
                    *template = if index == 0 {
                        None
                    } else {
                        templates.get(index - 1).map(|(id, _)| id.clone())
                    };
                }
            }
            ui.horizontal(|ui| {
                if forja_ui_kit::button(ui, forja_ui_kit::ButtonKind::Primary, &ok).clicked() {
                    outcome = Outcome::Submit;
                }
                if forja_ui_kit::button(ui, forja_ui_kit::ButtonKind::Ghost, &cancel).clicked() {
                    outcome = Outcome::Cancel;
                }
            });
            if outcome == Outcome::Open
                && forja_ui_kit::keyboard_layer_active(ctx, layer)
                && !egui::Popup::is_any_open(ctx)
                && ui.input_mut(|input| input.consume_key(egui::Modifiers::NONE, egui::Key::Enter))
            {
                outcome = Outcome::Submit;
            }
        });
    if outcome != Outcome::Open {
        ctx.data_mut(|data| data.remove_temp::<bool>(focus_key));
    }
    outcome
}

/// Shows the rename window with one name field and OK / Cancel.
pub(crate) fn name_window(
    ctx: &egui::Context,
    cx: &Ctx<'_>,
    title: &str,
    name: &mut String,
) -> Outcome {
    let layer = egui::Id::new("forja.explorer.rename.keyboard");
    let focus_key = egui::Id::new("forja.explorer.rename.focus");
    if forja_ui_kit::consume_layer_escape(ctx, layer) {
        ctx.data_mut(|data| data.remove_temp::<bool>(focus_key));
        return Outcome::Cancel;
    }
    let focus_name = !ctx.data(|data| data.get_temp::<bool>(focus_key).unwrap_or(false));
    let label = cx.t("explorer.dialog.name.label");
    let ok = cx.t("explorer.dialog.confirm");
    let cancel = cx.t("explorer.dialog.cancel");
    let mut outcome = Outcome::Open;
    egui::Window::new(title)
        .id(egui::Id::new("forja.explorer.rename"))
        .collapsible(false)
        .resizable(false)
        .anchor(egui::Align2::CENTER_CENTER, [0.0, 0.0])
        .show(ctx, |ui| {
            forja_ui_kit::register_keyboard_layer(ui, layer);
            forja_ui_kit::text(ui, label);
            let edit = forja_ui_kit::text_field(ui, name, "");
            if focus_name {
                edit.request_focus();
                ctx.data_mut(|data| data.insert_temp(focus_key, true));
            }
            let enter = forja_ui_kit::keyboard_layer_active(ctx, layer)
                && edit.lost_focus()
                && ui.input(|input| input.key_pressed(egui::Key::Enter));
            ui.horizontal(|ui| {
                if forja_ui_kit::button(ui, forja_ui_kit::ButtonKind::Primary, &ok).clicked()
                    || enter
                {
                    outcome = Outcome::Submit;
                }
                if forja_ui_kit::button(ui, forja_ui_kit::ButtonKind::Ghost, &cancel).clicked() {
                    outcome = Outcome::Cancel;
                }
            });
        });
    if outcome != Outcome::Open {
        ctx.data_mut(|data| data.remove_temp::<bool>(focus_key));
    }
    outcome
}

/// Shows the confirmation window before moving an entry to trash.
pub(crate) fn confirm_trash_window(
    ctx: &egui::Context,
    cx: &Ctx<'_>,
    path: &Path,
) -> Option<bool> {
    let layer = egui::Id::new("forja.explorer.confirm_trash.keyboard");
    if forja_ui_kit::consume_layer_escape(ctx, layer) {
        return Some(false);
    }
    let name = path
        .file_name()
        .map(|n| n.to_string_lossy().into_owned())
        .unwrap_or_else(|| path.display().to_string());
    let title = cx.t("explorer.dialog.trash.title");
    let body = cx.t_args("explorer.dialog.trash.body", &[("name", &name)]);
    let yes = cx.t("explorer.dialog.trash.yes");
    let no = cx.t("explorer.dialog.cancel");
    let mut answer: Option<bool> = None;
    egui::Window::new(title)
        .id(egui::Id::new("forja.explorer.confirm_trash"))
        .collapsible(false)
        .resizable(false)
        .anchor(egui::Align2::CENTER_CENTER, [0.0, 0.0])
        .show(ctx, |ui| {
            forja_ui_kit::register_keyboard_layer(ui, layer);
            forja_ui_kit::text(ui, body);
            ui.horizontal(|ui| {
                if forja_ui_kit::button(ui, forja_ui_kit::ButtonKind::Primary, &yes).clicked() {
                    answer = Some(true);
                }
                if forja_ui_kit::button(ui, forja_ui_kit::ButtonKind::Ghost, &no).clicked() {
                    answer = Some(false);
                }
            });
        });
    answer
}

#[cfg(test)]
mod tests {
    use super::*;
    use forja_kernel::{Runtime, RuntimeOptions};
    use std::path::PathBuf;
    use std::time::{SystemTime, UNIX_EPOCH};

    fn test_runtime(tag: &str) -> (Runtime, PathBuf) {
        let nanos = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let dir = std::env::temp_dir().join(format!("forja-explorer-dialogs-{tag}-{nanos}"));
        std::fs::create_dir_all(&dir).unwrap();
        let runtime = Runtime::new(
            vec![Box::new(crate::ExplorerModule::new())],
            RuntimeOptions {
                data_dir: Some(dir.clone()),
                user_dir: Some(dir.join("user")),
                locale: Some("en".to_owned()),
                wake: None,
            },
        )
        .unwrap();
        (runtime, dir)
    }

    fn key_escape() -> egui::RawInput {
        egui::RawInput {
            events: vec![
                egui::Event::Key {
                    key: egui::Key::Escape,
                    physical_key: Some(egui::Key::Escape),
                    pressed: true,
                    repeat: false,
                    modifiers: egui::Modifiers::NONE,
                },
                egui::Event::Key {
                    key: egui::Key::Escape,
                    physical_key: Some(egui::Key::Escape),
                    pressed: false,
                    repeat: false,
                    modifiers: egui::Modifiers::NONE,
                },
            ],
            ..Default::default()
        }
    }

    #[test]
    fn escape_closes_only_the_top_stacked_dialog() {
        let (mut runtime, dir) = test_runtime("stacked-escape");
        let cx = runtime.ctx("explorer");
        let ctx = egui::Context::default();
        let top_layer = egui::Id::new("forja.projects.clone.keyboard");
        let mut top_open = true;
        let mut entry_open = true;
        let mut name = String::new();
        let mut template = None;

        // Frame 1: both dialogs render; projects modal registers last so it owns the top layer.
        let _ = ctx.run(Default::default(), |ctx| {
            let outcome = new_entry_window(ctx, &cx, "New folder", &mut name, &[], &mut template);
            assert_eq!(outcome, Outcome::Open);
            egui::Window::new("Clone repository")
                .id(egui::Id::new("forja.projects.clone"))
                .show(ctx, |ui| {
                    forja_ui_kit::register_keyboard_layer(ui, top_layer);
                });
        });

        // Frame 2: Escape closes only the top dialog; New folder remains open.
        let _ = ctx.run(key_escape(), |ctx| {
            let outcome = new_entry_window(ctx, &cx, "New folder", &mut name, &[], &mut template);
            entry_open = outcome == Outcome::Open;
            if forja_ui_kit::consume_layer_escape(ctx, top_layer) {
                top_open = false;
            } else if top_open {
                egui::Window::new("Clone repository")
                    .id(egui::Id::new("forja.projects.clone"))
                    .show(ctx, |ui| {
                        forja_ui_kit::register_keyboard_layer(ui, top_layer);
                    });
            }
        });
        assert!(!top_open, "first Escape should close the top dialog");
        assert!(entry_open, "first Escape must leave New folder open");

        // Frame 3: second Escape now reaches New folder's layer and closes it.
        let _ = ctx.run(key_escape(), |ctx| {
            let outcome = new_entry_window(ctx, &cx, "New folder", &mut name, &[], &mut template);
            entry_open = outcome == Outcome::Open;
        });
        assert!(!entry_open, "second Escape should close New folder");
        let _ = std::fs::remove_dir_all(dir);
    }
}
