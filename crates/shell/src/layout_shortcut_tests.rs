//! `Ctrl+Alt+L` abre o seletor mesmo quando o terminal ou o editor detém o
//! foco de texto. `design/keymap.toml` marca `layout.switch` como
//! `even_in_terminal = true`; a prova usa a mesma porta dos outros atalhos
//! "mesmo no terminal" (`always_shortcut`).

use crate::app::always_shortcut;
use eframe::egui;

#[test]
fn layout_switch_survives_a_focused_text_surface() {
    // Um campo de texto focado é a condição que `handle_shortcuts` consulta
    // (`ctx.wants_keyboard_input()`) para decidir se um atalho pode disparar.
    let ctx = egui::Context::default();
    let mut text = String::new();
    let mut focused = false;
    let _ = ctx.run(egui::RawInput::default(), |ctx| {
        egui::CentralPanel::default().show(ctx, |ui| {
            let response = ui.add(egui::TextEdit::singleline(&mut text));
            response.request_focus();
            focused = ctx.wants_keyboard_input();
        });
    });
    assert!(focused, "the text surface must own the keyboard in this probe");

    // Com o foco no texto, só os atalhos `always_shortcut` passam o filtro.
    assert!(always_shortcut("layout.switch"));
    assert!(always_shortcut("terminal.toggle"));
    assert!(!always_shortcut("editor.save"));
}
