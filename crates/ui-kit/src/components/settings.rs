//! Settings rows, search emphasis and full-width section navigation.
use super::{button, ButtonKind};
use crate::paint;
use eframe::egui::{self, text::LayoutJob, Align, Layout, RichText, TextFormat, Ui};

/// Copy and state for one settings row; all text is supplied by the caller.
pub struct SettingRow<'a> {
    pub label: &'a str,
    pub help: &'a str,
    pub query: &'a str,
    pub modified: bool,
    pub modified_label: &'a str,
    pub restore_label: &'a str,
    pub notice: Option<&'a str>,
    pub error: Option<&'a str>,
}

/// Draws the row and returns the control's result and whether restore was clicked.
pub fn setting_row<R>(
    ui: &mut Ui,
    row: SettingRow<'_>,
    control: impl FnOnce(&mut Ui) -> R,
) -> (R, bool) {
    let tokens = paint::tokens(ui);
    let control_width = (ui.available_width() * 0.38).clamp(110.0, 210.0);
    let text_width = (ui.available_width() - control_width - 16.0).max(60.0);
    let mut restore = false;
    let mut result = None;
    ui.vertical(|ui| {
        ui.add_space(8.0);
        ui.horizontal_top(|ui| {
            ui.allocate_ui_with_layout(
                egui::vec2(text_width, 0.0),
                Layout::top_down(Align::Min),
                |ui| {
                    ui.set_width(text_width);
                    ui.add(
                        egui::Label::new(highlighted_text(ui, row.label, row.query, true))
                            .selectable(false)
                            .wrap(),
                    );
                    ui.add(
                        egui::Label::new(highlighted_text(ui, row.help, row.query, false))
                            .selectable(false)
                            .truncate(),
                    )
                    .on_hover_text(row.help);
                    if let Some(notice) = row.notice {
                        ui.label(
                            RichText::new(notice)
                                .small()
                                .color(tokens.color.text_muted.to_color32()),
                        );
                    }
                    if let Some(error) = row.error {
                        ui.add(
                            egui::Label::new(
                                RichText::new(error)
                                    .small()
                                    .color(tokens.color.danger.to_color32()),
                            )
                            .wrap(),
                        );
                    }
                    if row.modified {
                        ui.horizontal_wrapped(|ui| {
                            let marker = ui.label(RichText::new(row.modified_label).small());
                            let centre = marker.rect.left_center() - egui::vec2(5.0, 0.0);
                            ui.painter().circle_filled(
                                centre,
                                2.5,
                                tokens.color.accent.to_color32(),
                            );
                            restore = button(ui, ButtonKind::Ghost, row.restore_label).clicked();
                        });
                    }
                },
            );
            ui.allocate_ui_with_layout(
                egui::vec2(control_width, 0.0),
                Layout::right_to_left(Align::Center),
                |ui| {
                    ui.set_width(control_width);
                    result = Some(control(ui));
                },
            );
        });
        ui.add_space(6.0);
        ui.separator();
    });
    (result.expect("control rendered"), restore)
}

/// Section navigation with an accessible selected state and a quiet accent rail.
pub fn settings_navigation(ui: &mut Ui, label: &str, selected: bool) -> egui::Response {
    let tokens = paint::tokens(ui);
    let (rect, fallback) = ui.allocate_exact_size(
        egui::vec2(ui.available_width(), tokens.metrics().control),
        egui::Sense::hover(),
    );
    let visible = rect.intersect(ui.clip_rect());
    if !visible.is_positive() {
        return fallback;
    }
    let response = ui.interact(visible, fallback.id, egui::Sense::click());
    response.widget_info(|| {
        egui::WidgetInfo::selected(
            egui::WidgetType::SelectableLabel,
            ui.is_enabled(),
            selected,
            label,
        )
    });
    if selected || response.hovered() {
        ui.painter().rect_filled(
            rect,
            6.0,
            if selected {
                tokens.color.accent_soft
            } else {
                tokens.color.surface_alt
            }
            .to_color32(),
        );
    }
    ui.painter().with_clip_rect(visible).text(
        rect.left_center() + egui::vec2(10.0, 0.0),
        egui::Align2::LEFT_CENTER,
        label,
        paint::body_font(&tokens.metrics()),
        tokens.color.text.to_color32(),
    );
    if selected {
        ui.painter().vline(
            response.rect.left() + 1.0,
            response.rect.y_range().shrink(6.0),
            egui::Stroke::new(2.0, tokens.color.accent.to_color32()),
        );
    }
    response.on_hover_text(label)
}

/// Text emphasis preserves original Unicode text and highlights every matching span.
pub fn highlighted_text(ui: &Ui, text: &str, query: &str, strong: bool) -> LayoutJob {
    let tokens = paint::tokens(ui);
    let font = paint::body_font(&tokens.metrics());
    let color = if strong {
        tokens.color.text
    } else {
        tokens.color.text_muted
    }
    .to_color32();
    let mut job = LayoutJob::default();
    let spans = highlight_spans(text, query);
    let mut start = 0;
    for (from, to) in spans {
        job.append(
            &text[start..from],
            0.0,
            TextFormat {
                font_id: font.clone(),
                color,
                ..Default::default()
            },
        );
        job.append(
            &text[from..to],
            0.0,
            TextFormat {
                font_id: font.clone(),
                color: tokens.color.text.to_color32(),
                background: tokens.color.accent_soft.to_color32(),
                ..Default::default()
            },
        );
        start = to;
    }
    job.append(
        &text[start..],
        0.0,
        TextFormat {
            font_id: font,
            color,
            ..Default::default()
        },
    );
    job
}

fn highlight_spans(text: &str, query: &str) -> Vec<(usize, usize)> {
    let query = query.trim().to_lowercase();
    if query.is_empty() {
        return Vec::new();
    }
    let mut folded = String::new();
    let mut positions = Vec::new();
    for (index, ch) in text.char_indices() {
        for lower in ch.to_lowercase() {
            for _ in 0..lower.len_utf8() {
                positions.push((index, index + ch.len_utf8()));
            }
            folded.push(lower);
        }
    }
    folded
        .match_indices(&query)
        .filter_map(|(from, value)| {
            Some((
                positions.get(from)?.0,
                positions.get(from + value.len() - 1)?.1,
            ))
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn highlighting_preserves_unicode_boundaries_and_all_matches() {
        assert_eq!(
            highlight_spans("Aparência aparência", "APARÊNCIA"),
            vec![(0, 10), (11, 21)]
        );
        assert_eq!(highlight_spans("İndice", "i"), vec![(0, 2), (4, 5)]);
        assert!(highlight_spans("Theme", "").is_empty());
    }
    #[test]
    fn row_and_navigation_fit_a_narrow_content_column() {
        let ctx = egui::Context::default();
        crate::apply(&ctx, &crate::Tokens::brasa_dark());
        let raw = egui::RawInput {
            screen_rect: Some(egui::Rect::from_min_size(
                egui::Pos2::ZERO,
                egui::vec2(376.0, 600.0),
            )),
            ..Default::default()
        };
        let _ = ctx.run(raw, |ctx| {
            egui::CentralPanel::default().show(ctx, |ui| {
                ui.set_width(360.0);
                let nav = settings_navigation(ui, "Appearance", true);
                assert!(nav.rect.width() <= 361.0);
                let (rect, _) = setting_row(
                    ui,
                    SettingRow {
                        label: "Theme",
                        help: "Choose a theme",
                        query: "theme",
                        modified: true,
                        modified_label: "Modified",
                        restore_label: "Restore default",
                        notice: None,
                        error: None,
                    },
                    |ui| ui.button("Dark").rect,
                );
                assert!(rect.right() <= ui.max_rect().right() + 1.0);
            });
        });
    }
}
