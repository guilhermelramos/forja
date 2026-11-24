//! Seletor de esforço de raciocínio com nome acessível (Refs #207).

use forja_kernel::ui_contract::egui;
use forja_kernel::Ctx;

/// Desenha o seletor e devolve o novo valor quando a escolha muda.
/// `None` dentro do `Some` significa "padrão".
pub(crate) fn draw_effort(
    ui: &mut egui::Ui,
    cx: &Ctx<'_>,
    effort_values: &[String],
    current: Option<&str>,
    label: &str,
) -> Option<Option<String>> {
    let mut labels: Vec<String> = vec![label.to_owned()];
    for value in effort_values {
        labels.push(crate::view::effort_label(cx, value));
    }
    let refs: Vec<&str> = labels.iter().map(String::as_str).collect();
    let mut index = current
        .and_then(|value| effort_values.iter().position(|item| item == value))
        .map(|position| position + 1)
        .unwrap_or(0);
    if forja_ui_kit::select_labeled(ui, &mut index, label, &refs).changed() {
        let chosen = (index > 0).then(|| effort_values[index - 1].clone());
        Some(chosen)
    } else {
        None
    }
}
