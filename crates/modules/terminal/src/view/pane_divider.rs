//! Divisor arrastável entre painéis do terminal, com nome e setas de teclado.

use forja_kernel::ui_contract::egui::{self, Rect, Sense};

/// Geometria e estado do divisor entre dois painéis.
pub(super) struct Divider<'a> {
    pub rect: Rect,
    pub id: egui::Id,
    pub name: &'a str,
    pub horizontal: bool,
    pub ratio: &'a mut f32,
    pub bounds: (f32, f32),
    pub available: f32,
}

impl Divider<'_> {
    /// Registra nome, dica, arrasto e teclado; devolve a resposta para pintar a linha.
    pub(super) fn draw(self, ui: &mut egui::Ui) -> egui::Response {
        let Self {
            rect,
            id,
            name,
            horizontal,
            ratio,
            bounds,
            available,
        } = self;
        let (lower, upper) = bounds;
        let response = ui.interact(rect, id, Sense::click_and_drag());
        response.widget_info(|| {
            egui::WidgetInfo::slider(ui.is_enabled(), f64::from(*ratio), name.to_owned())
        });
        if response.clicked() {
            response.request_focus();
        }
        // O splitter do ui-kit só arrasta em x e limita a 10–90%; aqui o
        // divisor cobre painéis empilhados e os mínimos das duas árvores.
        if response.has_focus() && ui.is_enabled() {
            let (less, more) = if horizontal {
                (egui::Key::ArrowLeft, egui::Key::ArrowRight)
            } else {
                (egui::Key::ArrowUp, egui::Key::ArrowDown)
            };
            if ui.input(|input| input.key_pressed(less)) {
                *ratio -= 0.01;
            }
            if ui.input(|input| input.key_pressed(more)) {
                *ratio += 0.01;
            }
            *ratio = (*ratio).clamp(lower, upper);
            ui.ctx().request_repaint();
        }
        let response = forja_ui_kit::tooltip(response, name.to_owned());
        if response.dragged() && available > 0.0 {
            let delta = response.drag_delta();
            *ratio = (*ratio + (if horizontal { delta.x } else { delta.y }) / available)
                .clamp(lower, upper);
            ui.ctx().request_repaint();
        }
        response.on_hover_cursor(if horizontal {
            egui::CursorIcon::ResizeHorizontal
        } else {
            egui::CursorIcon::ResizeVertical
        })
    }
}
