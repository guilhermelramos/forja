//! Modo Foco: overlay de chrome em memória. Nenhuma preferência é gravada e
//! nenhuma view é destruída — só a apresentação das barras muda.

use crate::app::ForjaApp;
use eframe::egui::{self, Margin};
use forja_ui_kit::ButtonKind;

/// O que `appearance.focus.hide_bars` esconde.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub(crate) enum FocusHide {
    SidePanels,
    #[default]
    Chrome,
    All,
}

impl FocusHide {
    pub(crate) fn parse(raw: &str) -> Self {
        match raw {
            "side_panels" => Self::SidePanels,
            "all" => Self::All,
            _ => Self::Chrome,
        }
    }

    #[cfg(test)]
    pub(crate) fn as_str(self) -> &'static str {
        match self {
            Self::SidePanels => "side_panels",
            Self::Chrome => "chrome",
            Self::All => "all",
        }
    }

    /// Todos os modos recolhem as laterais elegíveis.
    pub(crate) fn hides_side_panels(self) -> bool {
        true
    }

    pub(crate) fn hides_chrome(self) -> bool {
        matches!(self, Self::Chrome | Self::All)
    }

    pub(crate) fn replaces_top_bar(self) -> bool {
        matches!(self, Self::All)
    }
}

/// Visibilidade do chrome no momento da entrada, para recompor ao sair.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub(crate) struct ChromeVisibility {
    pub rail: bool,
    pub sidebar: bool,
    pub right_panel: bool,
    pub status_bar: bool,
    pub top_bar: bool,
}

/// Estado transitório do overlay. `saved` e `return_focus` são consumidos na
/// saída; trocar de bancada também chama [`ForjaApp::exit_focus_mode`].
#[derive(Clone, Debug, Default)]
pub(crate) struct FocusOverlay {
    pub active: bool,
    pub hide: FocusHide,
    pub saved: Option<ChromeVisibility>,
    pub return_focus: Option<egui::Id>,
}

impl FocusOverlay {
    fn enter(&mut self, hide: FocusHide, visible: ChromeVisibility, focus: Option<egui::Id>) {
        self.active = true;
        self.hide = hide;
        self.saved = Some(visible);
        self.return_focus = focus;
    }

    fn leave(&mut self) -> Option<(ChromeVisibility, Option<egui::Id>)> {
        if !self.active {
            return None;
        }
        self.active = false;
        let visible = self.saved.take();
        let focus = self.return_focus.take();
        visible.map(|visible| (visible, focus))
    }
}

fn exit_hover_id() -> egui::Id {
    egui::Id::new("forja.focus.exit-hover")
}

fn exit_focus_id() -> egui::Id {
    egui::Id::new("forja.focus.exit-strip")
}

fn exit_focused_id() -> egui::Id {
    egui::Id::new("forja.focus.exit-focused")
}

/// A faixa abre ao apontar **ou** ao receber foco de teclado (Tab/F6).
fn exit_strip_expanded(ctx: &egui::Context, hovered: bool, button_focused: bool) -> bool {
    hovered
        || button_focused
        || ctx.memory(|memory| memory.focused() == Some(exit_focus_id()))
}

impl ForjaApp {
    pub(crate) fn focus_active(&self) -> bool {
        self.layout_state.focus.active
    }

    pub(crate) fn focus_hides_side_panels(&self) -> bool {
        self.focus_active() && self.layout_state.focus.hide.hides_side_panels()
    }

    pub(crate) fn focus_hides_rail(&self) -> bool {
        self.focus_active() && self.layout_state.focus.hide.hides_chrome()
    }

    pub(crate) fn focus_hides_status_bar(&self) -> bool {
        self.focus_active() && self.layout_state.focus.hide.hides_chrome()
    }

    pub(crate) fn focus_replaces_top_bar(&self) -> bool {
        self.focus_active() && self.layout_state.focus.hide.replaces_top_bar()
    }

    pub(crate) fn toggle_focus_mode(&mut self, ctx: &egui::Context) {
        if self.focus_active() {
            self.exit_focus_mode(ctx);
        } else {
            let hide = FocusHide::parse(&self.setting_text("appearance.focus.hide_bars", "chrome"));
            let visible = ChromeVisibility {
                rail: self.layout_rail_visible(),
                sidebar: self.show_sidebar,
                right_panel: self.show_right_panel,
                status_bar: self.layout_status_bar_visible(),
                top_bar: true,
            };
            let focus = ctx.memory(|memory| memory.focused());
            self.layout_state.focus.enter(hide, visible, focus);
            ctx.request_repaint();
        }
    }

    /// Repõe o chrome e devolve o foco a quem o tinha antes da entrada.
    pub(crate) fn exit_focus_mode(&mut self, ctx: &egui::Context) {
        let Some((visible, focus)) = self.layout_state.focus.leave() else {
            return;
        };
        self.show_sidebar = visible.sidebar;
        self.show_right_panel = visible.right_panel;
        if let Some(id) = focus {
            ctx.memory_mut(|memory| memory.request_focus(id));
        }
        ctx.request_repaint();
    }

    /// Trocar de bancada recompõe o chrome; não há foco de retorno a restaurar
    /// porque a própria bancada trocou de dono.
    pub(crate) fn exit_focus_for_bench_switch(&mut self) {
        if let Some((visible, _)) = self.layout_state.focus.leave() {
            self.show_sidebar = visible.sidebar;
            self.show_right_panel = visible.right_panel;
        }
    }

    /// Faixa de saída do modo `all`: fina em repouso, expande ao apontar ou
    /// focar, com nome acessível traduzido.
    pub(crate) fn focus_exit_strip(&mut self, ctx: &egui::Context) {
        let hovered = ctx
            .data(|data| data.get_temp::<bool>(exit_hover_id()))
            .unwrap_or(false);
        let button_focused = ctx
            .data(|data| data.get_temp::<bool>(exit_focused_id()))
            .unwrap_or(false);
        let anchor_focused = ctx.memory(|memory| memory.focused() == Some(exit_focus_id()));
        let expanded = exit_strip_expanded(ctx, hovered, button_focused);
        let height = if expanded { 24.0 } else { 6.0 };
        let frame = egui::Frame::NONE
            .fill(self.paint(self.tokens.color.surface_alt))
            .inner_margin(Margin::symmetric(8, 1));
        let label = self.text("shell.layout.focus-exit");
        let tip = self.text("shell.layout.focus-exit-tip");
        let response = egui::TopBottomPanel::top("forja.focus.exit")
            .exact_height(height)
            .frame(frame)
            .show(ctx, |ui| {
                if expanded {
                    ui.horizontal_centered(|ui| {
                        let button = forja_ui_kit::button(ui, ButtonKind::Ghost, &label);
                        if anchor_focused {
                            ctx.memory_mut(|memory| memory.request_focus(button.id));
                        }
                        let focused = button.has_focus();
                        let hovered = button.hovered();
                        if forja_ui_kit::tooltip(button, tip).clicked() {
                            self.exit_focus_mode(ctx);
                        }
                        ctx.data_mut(|data| {
                            data.insert_temp(exit_hover_id(), hovered);
                            data.insert_temp(exit_focused_id(), focused);
                        });
                    });
                } else {
                    // Alvo de foco sempre presente: o Tab/F6 precisa alcançar a
                    // faixa mesmo antes de ela se expandir. O nome acessível é
                    // o mesmo do botão exibido quando aberta.
                    let bounds = ui.max_rect();
                    let anchor = egui::Rect::from_min_size(
                        bounds.left_top(),
                        egui::vec2(16.0, bounds.height().max(1.0)),
                    );
                    let response = ui.interact(
                        anchor,
                        exit_focus_id(),
                        egui::Sense::focusable_noninteractive(),
                    );
                    response.widget_info(|| {
                        egui::WidgetInfo::labeled(egui::WidgetType::Button, ui.is_enabled(), &label)
                    });
                    ctx.data_mut(|data| {
                        data.insert_temp(exit_hover_id(), response.hovered());
                        data.insert_temp(exit_focused_id(), false);
                    });
                }
            });
        self.slot_rects.insert("top".into(), response.response.rect);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use forja_kernel::i18n::{BundleSource, I18n, Locale};

    fn shell_i18n(locale: Locale) -> I18n {
        let source = BundleSource {
            module: "shell".to_owned(),
            en: include_str!("../i18n/en.ftl").to_owned(),
            pt_br: include_str!("../i18n/pt-BR.ftl").to_owned(),
        };
        let (i18n, warnings) = I18n::build(&[source], locale);
        assert!(warnings.is_empty(), "{warnings:?}");
        i18n
    }

    #[test]
    fn triple_hide_bars_controls_the_right_chrome_parts() {
        let side = FocusHide::parse("side_panels");
        assert!(side.hides_side_panels());
        assert!(!side.hides_chrome());
        assert!(!side.replaces_top_bar());

        let chrome = FocusHide::parse("chrome");
        assert_eq!(chrome, FocusHide::default());
        assert_eq!(chrome.as_str(), "chrome");
        assert!(chrome.hides_side_panels());
        assert!(chrome.hides_chrome());
        assert!(!chrome.replaces_top_bar());

        let all = FocusHide::parse("all");
        assert!(all.hides_chrome());
        assert!(all.replaces_top_bar());
        assert_eq!(FocusHide::parse("unknown"), FocusHide::Chrome);
    }

    #[test]
    fn exit_strip_expands_from_keyboard_focus_without_the_pointer() {
        let ctx = egui::Context::default();
        assert!(!exit_strip_expanded(&ctx, false, false));
        assert!(exit_strip_expanded(&ctx, true, false));
        assert!(exit_strip_expanded(&ctx, false, true));

        // Tab/F6 can reach the strip while it is collapsed; the anchor is the
        // focus target, and the same accessible name is used when it opens.
        ctx.memory_mut(|memory| memory.request_focus(exit_focus_id()));
        assert!(exit_strip_expanded(&ctx, false, false));
        let label = shell_i18n(Locale::PtBr).t("shell.layout.focus-exit");
        assert_eq!(label, "Sair do Modo Foco");
    }

    #[test]
    fn leaving_focus_consumes_the_saved_chrome_and_return_focus() {
        let ctx = egui::Context::default();
        let search_id = egui::Id::new("forja.top.search");
        ctx.memory_mut(|memory| memory.request_focus(search_id));
        let focused = ctx.memory(|memory| memory.focused());
        let visible = ChromeVisibility {
            rail: true,
            sidebar: true,
            right_panel: false,
            status_bar: true,
            top_bar: true,
        };
        let mut overlay = FocusOverlay::default();
        overlay.enter(FocusHide::All, visible, focused);
        assert!(overlay.active);
        let (restored, returned) = overlay.leave().expect("saved chrome");
        assert_eq!(restored, visible);
        assert_eq!(returned, Some(search_id));
        assert!(overlay.leave().is_none(), "a second exit is a no-op");
    }

    #[test]
    fn exit_strip_has_a_translated_accessible_name() {
        assert_eq!(
            shell_i18n(Locale::PtBr).t("shell.layout.focus-exit"),
            "Sair do Modo Foco"
        );
        assert_eq!(
            shell_i18n(Locale::En).t("shell.layout.focus-exit"),
            "Exit Focus Mode"
        );
    }
}
