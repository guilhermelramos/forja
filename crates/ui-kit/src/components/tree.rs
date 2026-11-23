//! Árvore e tabela virtualizada.

use eframe::egui::{self, Align2, Color32, CornerRadius, ScrollArea, Sense, Ui};

use crate::paint::{self, body_font, caption_font, paint_text};

/// Nó da árvore. `open` só importa quando há filhos.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct TreeNode {
    /// Rótulo.
    pub label: String,
    /// Filhos visíveis.
    pub open: bool,
    /// Filhos.
    pub children: Vec<TreeNode>,
}

impl TreeNode {
    /// Nó sem filhos.
    pub fn leaf(label: impl Into<String>) -> Self {
        Self {
            label: label.into(),
            open: false,
            children: Vec::new(),
        }
    }

    /// Nó com filhos, aberto.
    pub fn branch(label: impl Into<String>, children: Vec<Self>) -> Self {
        Self {
            label: label.into(),
            open: true,
            children,
        }
    }
}

/// Árvore com recuo por nível.
pub fn tree(ui: &mut Ui, nodes: &mut [TreeNode]) {
    for node in nodes {
        tree_node(ui, node, 0);
    }
}

fn tree_node(ui: &mut Ui, node: &mut TreeNode, depth: usize) {
    let tokens = paint::tokens(ui);
    let metrics = tokens.metrics();
    let indent = metrics.indent * depth as f32;
    let width = ui.available_width();
    let (rect, _) = ui.allocate_exact_size(egui::vec2(width, metrics.row), Sense::hover());
    let toggle_rect = egui::Rect::from_min_size(
        egui::pos2(rect.left() + indent, rect.center().y - 8.0),
        egui::vec2(16.0, 16.0),
    );
    if !node.children.is_empty() {
        let toggle = ui.interact(
            toggle_rect,
            ui.id().with((&node.label, depth, "toggle")),
            Sense::click(),
        );
        toggle.widget_info(|| {
            egui::WidgetInfo::selected(
                egui::WidgetType::Button,
                ui.is_enabled(),
                node.open,
                &node.label,
            )
        });
        if toggle.clicked() {
            node.open = !node.open;
        }
        disclosure(
            ui,
            toggle_rect.center(),
            node.open,
            tokens.color.text_muted.to_color32(),
        );
        paint::paint_focus(
            ui,
            &toggle,
            toggle_rect,
            4.0,
            tokens.color.accent.to_color32(),
        );
    }
    let label_rect = egui::Rect::from_min_max(
        egui::pos2(toggle_rect.right() + 4.0, rect.top()),
        rect.right_bottom(),
    );
    paint_text(
        ui.painter(),
        label_rect,
        Align2::LEFT_CENTER,
        &node.label,
        body_font(&metrics),
        tokens.color.text.to_color32(),
    );
    if node.open {
        for child in &mut node.children {
            tree_node(ui, child, depth + 1);
        }
    }
}

fn disclosure(ui: &Ui, center: egui::Pos2, open: bool, color: Color32) {
    crate::paint_icon(
        ui.painter(),
        if open {
            crate::Icon::ChevronDown
        } else {
            crate::Icon::ChevronRight
        },
        egui::Rect::from_center_size(center, egui::vec2(16.0, 16.0)),
        color,
    );
}

/// Coluna da tabela.
pub struct Column<'a> {
    /// Título do cabeçalho.
    pub title: &'a str,
    /// Largura em px.
    pub width: f32,
}

/// Tabela que só pinta as linhas visíveis.
pub fn virtual_table(
    ui: &mut Ui,
    columns: &[Column<'_>],
    row_count: usize,
    viewport: f32,
    mut cell: impl FnMut(&mut Ui, usize, usize),
) {
    let tokens = paint::tokens(ui);
    let metrics = tokens.metrics();
    header(ui, columns, &tokens, &metrics);
    ScrollArea::vertical()
        .max_height(viewport)
        .auto_shrink([false, false])
        .show_rows(ui, metrics.row, row_count, |ui, range| {
            for row in range {
                table_row(ui, columns, row, &tokens, &metrics, &mut cell);
            }
        });
}

fn header(
    ui: &mut Ui,
    columns: &[Column<'_>],
    tokens: &crate::tokens::Tokens,
    metrics: &crate::tokens::Metrics,
) {
    let width = columns
        .iter()
        .map(|column| column.width)
        .sum::<f32>()
        .max(1.0);
    let (rect, _) = ui.allocate_exact_size(egui::vec2(width, metrics.row), Sense::hover());
    ui.painter().rect_filled(
        rect,
        CornerRadius::ZERO,
        tokens.color.surface_alt.to_color32(),
    );
    let mut x = rect.left();
    for column in columns {
        let cell = egui::Rect::from_min_size(
            egui::pos2(x, rect.top()),
            egui::vec2(column.width, rect.height()),
        );
        paint_text(
            ui.painter(),
            cell.shrink2(egui::vec2(8.0, 0.0)),
            Align2::LEFT_CENTER,
            column.title,
            caption_font(metrics),
            tokens.color.text_muted.to_color32(),
        );
        x += column.width;
    }
}

fn table_row(
    ui: &mut Ui,
    columns: &[Column<'_>],
    row: usize,
    tokens: &crate::tokens::Tokens,
    metrics: &crate::tokens::Metrics,
    cell: &mut impl FnMut(&mut Ui, usize, usize),
) {
    let width = columns
        .iter()
        .map(|column| column.width)
        .sum::<f32>()
        .max(1.0);
    let (rect, _) = ui.allocate_exact_size(egui::vec2(width, metrics.row), Sense::hover());
    if row % 2 == 1 {
        ui.painter()
            .rect_filled(rect, CornerRadius::ZERO, tokens.color.bg.to_color32());
    }
    let mut x = rect.left();
    for (index, column) in columns.iter().enumerate() {
        let cell_rect = egui::Rect::from_min_size(
            egui::pos2(x, rect.top()),
            egui::vec2(column.width, rect.height()),
        );
        ui.scope_builder(egui::UiBuilder::new().max_rect(cell_rect), |ui| {
            cell(ui, row, index);
        });
        x += column.width;
    }
}
