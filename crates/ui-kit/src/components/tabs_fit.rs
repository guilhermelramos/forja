//! Planejamento de largura e excedente para faixas de abas.

use eframe::egui::{Color32, Ui};

use crate::paint::{self, body_font, caption_font};
use crate::tokens::Metrics;

pub(super) const CLOSE_RESERVE: f32 = 20.0;

/// Plano de quais abas cabem na faixa e quais ficam no menu de excedente.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct TabFitPlan {
    pub shown: Vec<usize>,
    pub hidden: Vec<usize>,
}

/// Mede quais índices cabem em `avail`; nunca deixa a aba ativa escondida no excedente.
pub fn plan_tab_fit(
    widths: &[f32],
    active: usize,
    avail: f32,
    gap: f32,
    overflow_width_for: impl Fn(usize) -> f32,
    trailing_width: f32,
) -> TabFitPlan {
    let count = widths.len();
    if count == 0 {
        return TabFitPlan {
            shown: Vec::new(),
            hidden: Vec::new(),
        };
    }

    let sum_widths = |indices: &[usize]| -> f32 {
        if indices.is_empty() {
            return 0.0;
        }
        let body: f32 = indices.iter().map(|&idx| widths[idx]).sum();
        body + gap * (indices.len().saturating_sub(1)) as f32
    };

    let all: Vec<usize> = (0..count).collect();
    let total_needed = sum_widths(&all)
        + if trailing_width > 0.0 {
            gap + trailing_width
        } else {
            0.0
        };
    if total_needed <= avail + 0.5 {
        return TabFitPlan {
            shown: all,
            hidden: Vec::new(),
        };
    }

    let trailing_reserve = if trailing_width > 0.0 {
        gap + trailing_width
    } else {
        0.0
    };
    let mut est_hidden = 1usize;
    let mut shown = Vec::new();
    for _ in 0..3 {
        let overflow_w = overflow_width_for(est_hidden) + gap;
        let budget = (avail - trailing_reserve - overflow_w).max(0.0);
        shown = pick_fitting_tabs(widths, active, budget, gap, &sum_widths);
        let next_hidden = count.saturating_sub(shown.len());
        if next_hidden == est_hidden || next_hidden == 0 {
            break;
        }
        est_hidden = next_hidden;
    }

    let hidden: Vec<usize> = (0..count).filter(|idx| !shown.contains(idx)).collect();
    TabFitPlan { shown, hidden }
}

fn pick_fitting_tabs(
    widths: &[f32],
    active: usize,
    budget: f32,
    gap: f32,
    sum_widths: &impl Fn(&[usize]) -> f32,
) -> Vec<usize> {
    let count = widths.len();
    let mut shown = Vec::new();
    let mut used = 0.0_f32;
    for (idx, &w) in widths.iter().enumerate() {
        let next = if shown.is_empty() { w } else { used + gap + w };
        if shown.is_empty() || next <= budget + 0.5 {
            shown.push(idx);
            used = next;
        } else {
            break;
        }
    }

    if active < count && !shown.contains(&active) {
        if let Some(last) = shown.last_mut() {
            *last = active;
        } else {
            shown.push(active);
        }
        while shown.len() > 1 && sum_widths(&shown) > budget + 0.5 {
            shown.remove(shown.len() - 2);
        }
    }
    shown
}

/// Calcula a largura natural de uma aba com ícone e reserva opcional de "×".
pub fn measure_tab_width(
    ui: &Ui,
    icon: &str,
    label: &str,
    closable: bool,
    metrics: &Metrics,
) -> f32 {
    let text_w = paint::text_size(ui, label, body_font(metrics), Color32::WHITE).x;
    let icon_w = if icon.is_empty() { 0.0 } else { 16.0 };
    let tail_w = if closable { CLOSE_RESERVE + 2.0 } else { 6.0 };
    (6.0 + icon_w + text_w + tail_w).max(metrics.control)
}

pub(super) fn measure_overflow_width(ui: &Ui, count: usize, metrics: &Metrics) -> f32 {
    let label = format!("+{count}");
    let text_w = paint::text_size(ui, &label, caption_font(metrics), Color32::WHITE).x;
    (text_w + 26.0).max(metrics.control + 12.0)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn plan_tab_fit_omits_overflow_when_all_tabs_fit() {
        let widths = [90.0, 90.0, 90.0, 90.0];
        let plan = plan_tab_fit(&widths, 0, 500.0, 4.0, |_| 44.0, 32.0);
        assert_eq!(plan.shown, vec![0, 1, 2, 3]);
        assert!(plan.hidden.is_empty());
    }

    #[test]
    fn plan_tab_fit_swaps_active_tab_into_last_visible_slot_on_overflow() {
        let widths = [100.0; 6];
        // Width 360 minus trailing (4+32=36) minus overflow (4+44=48) = 276 px -> fits 2 tabs (204).
        let plan = plan_tab_fit(&widths, 4, 360.0, 4.0, |_| 44.0, 32.0);
        assert_eq!(plan.shown, vec![0, 4]);
        assert_eq!(plan.hidden, vec![1, 2, 3, 5]);
    }

    #[test]
    fn four_panel_icons_fit_inside_180_px_minimum_width() {
        // Sidebar/right panel at min_width=180 has 180 - 2*12 = 156 px inner width.
        let widths = [32.0; 4];
        let plan = plan_tab_fit(&widths, 2, 156.0, 4.0, |_| 44.0, 0.0);
        assert_eq!(plan.shown, vec![0, 1, 2, 3]);
        assert!(plan.hidden.is_empty());
    }
}
