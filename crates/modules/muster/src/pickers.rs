//! Seletores nomeados do Mutirão (Refs #207).
//!
//! Mantêm o nome acessível e a dica no seletor, sem `ComboBox` cru na view.

use forja_kernel::ui_contract::egui;
use forja_kernel::Ctx;

/// Desenha um seletor com nome acessível e devolve o índice quando muda.
pub(crate) fn named_select(
    ui: &mut egui::Ui,
    label: &str,
    options: &[&str],
    current: usize,
) -> Option<usize> {
    let mut index = current;
    if forja_ui_kit::select_labeled(ui, &mut index, label, options).changed() {
        Some(index)
    } else {
        None
    }
}

/// Seletor de perfil do ajudante. `helpers` é `(id, nome, instalado)`.
/// Devolve `Some(id)` quando muda; `Some("")` significa "Automático".
pub(crate) fn profile_picker(
    ui: &mut egui::Ui,
    cx: &Ctx<'_>,
    helpers: &[(String, String, bool)],
    current: &str,
) -> Option<String> {
    let not_installed = cx.t("muster.profile-not-installed");
    let mut options: Vec<String> = vec![cx.t("muster.profile-auto")];
    for (_, name, installed) in helpers {
        options.push(if *installed {
            name.clone()
        } else {
            format!("{name} ({not_installed})")
        });
    }
    let known = current.trim().is_empty() || helpers.iter().any(|(id, _, _)| id == current);
    if !known {
        options.push(current.to_owned());
    }
    let index = if current.trim().is_empty() {
        0
    } else if known {
        helpers
            .iter()
            .position(|(id, _, _)| id == current)
            .unwrap_or(0)
            + 1
    } else {
        options.len() - 1
    };
    let refs: Vec<&str> = options.iter().map(String::as_str).collect();
    named_select(ui, &cx.t("muster.profile"), &refs, index).map(|picked| {
        if picked == 0 {
            String::new()
        } else if picked <= helpers.len() {
            helpers[picked - 1].0.clone()
        } else {
            current.to_owned()
        }
    })
}
