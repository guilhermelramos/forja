//! Toasts e o centro na barra de status.

use crate::rules::visible_toast_indexes;
use forja_contracts::NotifyLevel;
use forja_kernel::ui_contract::egui::{self, Align2, Color32, Sense};
use forja_kernel::{Args, Ctx, SettingValue};
use forja_ui_kit::{button, card, toast, ButtonKind, ToastKind};
use serde_json::{json, Value};
use std::sync::{Arc, Mutex};
use std::time::Instant;

#[derive(Clone)]
pub struct Notice {
    pub id: u64,
    pub title: String,
    pub body: String,
    pub level: NotifyLevel,
    pub bench_path: Option<String>,
    pub at: Instant,
    pub read: bool,
}

#[derive(Default)]
struct Inner {
    notices: Vec<Notice>,
    next_id: u64,
    center_open: bool,
}

#[derive(Clone)]
pub struct Shared {
    inner: Arc<Mutex<Inner>>,
    focused: Arc<std::sync::atomic::AtomicBool>,
}

impl Shared {
    pub fn new() -> Self {
        Self {
            inner: Arc::new(Mutex::new(Inner::default())),
            focused: Arc::new(std::sync::atomic::AtomicBool::new(true)),
        }
    }

    pub fn focused(&self) -> bool {
        self.focused.load(std::sync::atomic::Ordering::Relaxed)
    }

    pub fn set_focused(&self, focused: bool) {
        self.focused
            .store(focused, std::sync::atomic::Ordering::Relaxed);
    }

    pub fn push(&self, mut notice: Notice) {
        if let Ok(mut inner) = self.inner.lock() {
            inner.next_id += 1;
            notice.id = inner.next_id;
            inner.notices.push(notice);
            if inner.notices.len() > 40 {
                let extra = inner.notices.len() - 40;
                inner.notices.drain(0..extra);
            }
        }
    }

    pub fn snapshot(&self) -> Vec<Notice> {
        self.inner
            .lock()
            .map(|inner| inner.notices.clone())
            .unwrap_or_default()
    }

    pub fn dismiss(&self, id: u64) {
        if let Ok(mut inner) = self.inner.lock() {
            inner.notices.retain(|notice| notice.id != id);
        }
    }

    pub fn toggle_center(&self) {
        if let Ok(mut inner) = self.inner.lock() {
            inner.center_open = !inner.center_open;
            if inner.center_open {
                for notice in &mut inner.notices {
                    notice.read = true;
                }
            }
        }
    }

    pub fn center_open(&self) -> bool {
        self.inner
            .lock()
            .map(|inner| inner.center_open)
            .unwrap_or(false)
    }

    pub fn mark_read(&self, id: u64) {
        if let Ok(mut inner) = self.inner.lock() {
            if let Some(notice) = inner.notices.iter_mut().find(|notice| notice.id == id) {
                notice.read = true;
            }
        }
    }
}

pub struct ToastView {
    pub state: Shared,
}

pub struct CenterView {
    pub state: Shared,
}

impl forja_kernel::ui_contract::View for ToastView {
    fn poll(&mut self, cx: &mut Ctx<'_>) {
        self.state.set_focused(cx.viewport_focused());
        let duration = match cx.setting("notifications.toast.duration_s") {
            Some(SettingValue::Number(value)) => *value as f32,
            _ => 5.0,
        };
        let max = match cx.setting("notifications.toast.max") {
            Some(SettingValue::Number(value)) => value.round().clamp(1.0, 5.0) as usize,
            _ => 3,
        };
        let ages: Vec<f32> = self
            .state
            .snapshot()
            .iter()
            .map(|notice| notice.at.elapsed().as_secs_f32())
            .collect();
        if visible_toast_indexes(&ages, duration, max).is_empty() {
            cx.close_modal("notify.toasts");
        } else {
            cx.open_modal("notify.toasts");
        }
    }

    fn ui(&mut self, ui: &mut egui::Ui, cx: &mut Ctx<'_>) {
        self.state.set_focused(cx.viewport_focused());
        draw_toasts(ui.ctx(), cx, &self.state);
    }
}

impl forja_kernel::ui_contract::View for CenterView {
    fn ui(&mut self, ui: &mut egui::Ui, cx: &mut Ctx<'_>) {
        let notices = self.state.snapshot();
        let unread = notices.iter().filter(|notice| !notice.read).count();
        let count = unread.to_string();
        let label = cx.t_args("notify.center.count", &[("count", &count)]);
        let color = if unread == 0 { muted(ui) } else { accent(ui) };
        let response = ui
            .horizontal(|ui| {
                let (rect, _) = ui.allocate_exact_size(egui::vec2(16.0, 16.0), Sense::hover());
                forja_ui_kit::paint_icon(ui.painter(), forja_ui_kit::Icon::Bell, rect, color);
                ui.label(egui::RichText::new(&label).color(color));
            })
            .response
            .interact(Sense::click());
        response.widget_info(|| {
            egui::WidgetInfo::labeled(egui::WidgetType::Button, ui.is_enabled(), &label)
        });
        if response.clicked() {
            self.state.toggle_center();
        }
        if self.state.center_open() && dismiss_key(ui.ctx()) {
            self.state.toggle_center();
        }
        if self.state.center_open() {
            draw_center(ui.ctx(), cx, &self.state, &notices);
        }
    }
}

fn draw_toasts(ctx: &egui::Context, cx: &mut Ctx<'_>, state: &Shared) {
    let notices = state.snapshot();
    let duration = number(cx, "notifications.toast.duration_s", 5.0) as f32;
    let max = number(cx, "notifications.toast.max", 3.0)
        .round()
        .clamp(1.0, 5.0) as usize;
    let ages: Vec<f32> = notices
        .iter()
        .map(|notice| notice.at.elapsed().as_secs_f32())
        .collect();
    let indexes = visible_toast_indexes(&ages, duration, max);
    if indexes.is_empty() {
        return;
    }
    if !state.center_open() && dismiss_key(ctx) {
        if let Some(index) = indexes.last() {
            state.dismiss(notices[*index].id);
        }
        return;
    }
    forja_ui_kit::request_repaint_after_active(ctx, std::time::Duration::from_millis(200));
    let position = text(cx, "notifications.toast.position", "bottom_right");
    let (align, offset) = anchor(position);
    let dismiss = cx.t("notify.toast.dismiss");
    let items: Vec<(u64, NotifyLevel, String)> = indexes
        .into_iter()
        .map(|index| {
            let notice = &notices[index];
            (notice.id, notice.level, brief(&notice.title, &notice.body))
        })
        .collect();
    if let Some(id) = draw_toast_area(ctx, &items, align, offset, &dismiss) {
        state.dismiss(id);
    }
}

fn draw_toast_area(
    ctx: &egui::Context,
    items: &[(u64, NotifyLevel, String)],
    align: Align2,
    offset: egui::Vec2,
    dismiss: &str,
) -> Option<u64> {
    let screen = ctx.content_rect();
    let bounds = screen.shrink(8.0);
    let area_width = 360.0_f32.min(bounds.width().max(120.0));
    let max_height = (bounds.height() - 48.0).max(80.0);
    let stack_height = estimate_toast_stack_height(ctx, items, area_width, dismiss).min(max_height);
    let x = if align.x() == egui::Align::RIGHT {
        (screen.right() + offset.x - area_width).max(bounds.left())
    } else {
        (screen.left() + offset.x).min((bounds.right() - area_width).max(bounds.left()))
    };
    let y = if align.y() == egui::Align::BOTTOM {
        (screen.bottom() + offset.y - stack_height).max(bounds.top())
    } else {
        (screen.top() + offset.y).min((bounds.bottom() - stack_height).max(bounds.top()))
    };
    let mut dismissed = None;
    egui::Area::new(egui::Id::new("forja.notify.toasts"))
        .order(egui::Order::Foreground)
        .fixed_pos(egui::pos2(x, y))
        .constrain_to(bounds)
        .show(ctx, |ui| {
            ui.set_min_width(area_width);
            ui.set_max_width(area_width);
            egui::ScrollArea::vertical()
                .id_salt("forja.notify.toasts.scroll")
                .max_height(max_height)
                .show(ui, |ui| {
                    for (id, level, message) in items {
                        ui.with_layout(egui::Layout::right_to_left(egui::Align::TOP), |ui| {
                            if button(ui, ButtonKind::Ghost, dismiss).clicked() {
                                dismissed = Some(*id);
                            }
                            ui.with_layout(egui::Layout::top_down(egui::Align::LEFT), |ui| {
                                let w = ui.available_width().max(1.0);
                                ui.set_max_width(w);
                                toast(ui, kind(*level), message);
                            });
                        });
                        ui.add_space(8.0);
                    }
                });
        });
    dismissed
}

fn estimate_toast_stack_height(
    ctx: &egui::Context,
    items: &[(u64, NotifyLevel, String)],
    area_width: f32,
    dismiss: &str,
) -> f32 {
    let tokens = forja_ui_kit::applied_tokens(ctx);
    let metrics = tokens.metrics();
    let dismiss_w = ctx.fonts_mut(|fonts| {
        fonts
            .layout_no_wrap(
                dismiss.to_owned(),
                egui::FontId::proportional(metrics.body),
                Color32::WHITE,
            )
            .size()
            .x
    }) + metrics.pad_x * 2.0
        + 12.0;
    let text_max = (area_width - dismiss_w - metrics.pad_x * 2.0 - 20.0).max(60.0);
    items
        .iter()
        .map(|(_, _, message)| {
            let text_h = ctx.fonts_mut(|fonts| {
                let mut job = egui::text::LayoutJob::single_section(
                    message.clone(),
                    egui::TextFormat {
                        font_id: egui::FontId::proportional(metrics.body),
                        color: Color32::WHITE,
                        ..Default::default()
                    },
                );
                job.wrap = egui::text::TextWrapping {
                    max_width: text_max,
                    max_rows: usize::MAX,
                    break_anywhere: true,
                    ..Default::default()
                };
                fonts.layout_job(job).size().y
            });
            (text_h.max(metrics.body + 4.0) + metrics.pad_y * 2.0 + 2.0).max(metrics.row) + 12.0
        })
        .sum()
}

fn dismiss_key(ctx: &egui::Context) -> bool {
    !egui::Popup::is_any_open(ctx)
        && ctx.memory(|memory| memory.top_modal_layer().is_none())
        && ctx.input_mut(|input| input.consume_key(egui::Modifiers::NONE, egui::Key::Escape))
}

fn draw_center(ctx: &egui::Context, cx: &mut Ctx<'_>, state: &Shared, notices: &[Notice]) {
    let title = cx.t("notify.center.title");
    let empty = cx.t("notify.center.empty");
    let mut activate: Option<(u64, String)> = None;
    egui::Area::new(egui::Id::new("forja.notify.center"))
        .order(egui::Order::Foreground)
        .anchor(Align2::RIGHT_BOTTOM, egui::vec2(-12.0, -40.0))
        .constrain_to(ctx.content_rect().shrink(8.0))
        .show(ctx, |ui| {
            card(ui, Some(&title), |ui| {
                egui::ScrollArea::vertical()
                    .max_height(280.0)
                    .show(ui, |ui| {
                        ui.set_min_width(280.0);
                        if notices.is_empty() {
                            ui.label(egui::RichText::new(empty).color(muted(ui)));
                        }
                        for notice in notices.iter().rev() {
                            let line = brief(&notice.title, &notice.body);
                            let response = ui.add(
                                egui::Label::new(egui::RichText::new(line).color(text_color(ui)))
                                    .sense(Sense::click()),
                            );
                            if response.clicked() {
                                if let Some(path) = &notice.bench_path {
                                    activate = Some((notice.id, path.clone()));
                                } else {
                                    state.mark_read(notice.id);
                                }
                            }
                        }
                    });
            });
        });
    if let Some((id, path)) = activate {
        state.mark_read(id);
        let mut args = Args::new();
        args.insert("bench_path", Value::String(path));
        let _ = cx.run_command("bench.activate", args);
    }
}

fn anchor(position: &str) -> (Align2, egui::Vec2) {
    let margin = 16.0;
    match position {
        "top_right" => (Align2::RIGHT_TOP, egui::vec2(-margin, 48.0)),
        "top_left" => (Align2::LEFT_TOP, egui::vec2(margin, 48.0)),
        "bottom_left" => (Align2::LEFT_BOTTOM, egui::vec2(margin, -40.0)),
        _ => (Align2::RIGHT_BOTTOM, egui::vec2(-margin, -40.0)),
    }
}

fn kind(level: NotifyLevel) -> ToastKind {
    match level {
        NotifyLevel::Info => ToastKind::Info,
        NotifyLevel::Success => ToastKind::Ok,
        NotifyLevel::Attention => ToastKind::Spark,
        NotifyLevel::Error => ToastKind::Danger,
    }
}

fn brief(title: &str, body: &str) -> String {
    let text = if body.is_empty() {
        title.to_owned()
    } else {
        format!("{title} — {body}")
    };
    let mut chars = text.chars();
    let short: String = chars.by_ref().take(140).collect();
    if chars.next().is_some() {
        format!("{short}…")
    } else {
        short
    }
}

fn number(cx: &Ctx<'_>, key: &str, default: f64) -> f64 {
    match cx.setting(key) {
        Some(SettingValue::Number(value)) => *value,
        Some(SettingValue::Integer(value)) => *value as f64,
        _ => default,
    }
}

fn text<'a>(cx: &'a Ctx<'_>, key: &str, default: &'a str) -> &'a str {
    cx.setting(key)
        .and_then(SettingValue::as_str)
        .unwrap_or(default)
}

fn tokens(ui: &egui::Ui) -> forja_ui_kit::Tokens {
    forja_ui_kit::applied_tokens(ui.ctx())
}

fn muted(ui: &egui::Ui) -> Color32 {
    tokens(ui).color.text_muted.to_color32()
}

fn accent(ui: &egui::Ui) -> Color32 {
    tokens(ui).color.accent.to_color32()
}

fn text_color(ui: &egui::Ui) -> Color32 {
    tokens(ui).color.text.to_color32()
}

pub fn level_name(level: NotifyLevel) -> &'static str {
    match level {
        NotifyLevel::Info => "info",
        NotifyLevel::Success => "success",
        NotifyLevel::Attention => "attention",
        NotifyLevel::Error => "error",
    }
}

pub fn notices_json(state: &Shared) -> Value {
    let items: Vec<Value> = state
        .snapshot()
        .into_iter()
        .rev()
        .map(|notice| {
            json!({
                "title": notice.title,
                "body": notice.body,
                "level": level_name(notice.level),
                "read": notice.read,
                "bench_path": notice.bench_path,
            })
        })
        .collect();
    json!({ "items": items })
}

#[cfg(test)]
mod keyboard_tests {
    use super::*;

    fn escape_input() -> egui::RawInput {
        let mut raw = egui::RawInput::default();
        raw.events.push(egui::Event::Key {
            key: egui::Key::Escape,
            physical_key: None,
            pressed: true,
            repeat: false,
            modifiers: egui::Modifiers::NONE,
        });
        raw
    }

    #[test]
    fn escape_is_consumed_once_and_does_not_cross_a_modal() {
        let ctx = egui::Context::default();
        let _ = ctx.run(escape_input(), |ctx| {
            assert!(dismiss_key(ctx));
            assert!(!dismiss_key(ctx));
        });

        let _ = ctx.run(egui::RawInput::default(), |ctx| {
            let layer = egui::LayerId::new(egui::Order::Foreground, egui::Id::new("dialog"));
            ctx.memory_mut(|memory| memory.set_modal_layer(layer));
        });
        let _ = ctx.run(escape_input(), |ctx| {
            assert!(!dismiss_key(ctx));
            assert!(ctx.input(|input| input.key_pressed(egui::Key::Escape)));
        });
    }

    #[test]
    fn long_toast_fits_inside_screen_on_first_frame() {
        for (w, h) in [(1920.0, 1080.0), (1280.0, 800.0)] {
            let ctx = egui::Context::default();
            forja_ui_kit::install_fonts(&ctx).expect("fontes do Windows");
            let screen = egui::Rect::from_min_size(egui::Pos2::ZERO, egui::vec2(w, h));
            let input = egui::RawInput {
                screen_rect: Some(screen),
                ..Default::default()
            };
            let items = vec![(
                1_u64,
                NotifyLevel::Error,
                "Não foi possível criar a bancada: D:\\a\\forja\\forja\\a-very-long-unbreakable-path-0123456789-abcdefghijklmnopqrstuvwxyz".to_owned(),
            )];
            let (align, offset) = anchor("bottom_right");
            let out = ctx.run(input, |ctx| {
                let _ = draw_toast_area(ctx, &items, align, offset, "Fechar");
            });
            let area_id = egui::Id::new("forja.notify.toasts");
            let rect = out
                .viewport_output
                .get(&egui::ViewportId::ROOT)
                .and(ctx.memory(|mem| mem.area_rect(area_id)))
                .expect("toast area rect");
            assert!(rect.left() >= 0.0, "{w}x{h} left={}", rect.left());
            assert!(rect.top() >= 0.0, "{w}x{h} top={}", rect.top());
            assert!(rect.right() <= w, "{w}x{h} right={}", rect.right());
            assert!(rect.bottom() <= h, "{w}x{h} bottom={}", rect.bottom());
        }
    }
}

#[cfg(test)]
mod plural_tests {
    #[test]
    fn notification_count_uses_real_localized_fluent_plural() {
        let source = forja_kernel::i18n::BundleSource {
            module: "notify".into(),
            en: include_str!("../i18n/en.ftl").into(),
            pt_br: include_str!("../i18n/pt-BR.ftl").into(),
        };
        let (i18n, warnings) = forja_kernel::I18n::build(&[source], forja_kernel::Locale::PtBr);
        assert!(warnings.is_empty());
        assert_eq!(
            i18n.t_args("notify.center.count", &[("count", "1")]),
            "1 aviso"
        );
        assert_eq!(
            i18n.t_args("notify.center.count", &[("count", "2")]),
            "2 avisos"
        );
    }
}
