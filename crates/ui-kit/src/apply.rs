//! Aplica os tokens no `egui::Context`.

use eframe::egui::{self, Color32, CornerRadius, FontFamily, FontId, Stroke, TextStyle, Visuals};

use crate::tokens::{Mode, Tokens};

const TOKENS_ID: &str = "forja_brasa_tokens";

/// Guarda os tokens no contexto e pinta o estilo do egui com eles.
pub fn apply(ctx: &egui::Context, tokens: &Tokens) {
    ctx.data_mut(|data| data.insert_temp(egui::Id::new(TOKENS_ID), tokens.clone()));
    let metrics = tokens.metrics();
    let color = tokens.color;
    let radius = radius_u8(metrics.radius_control);
    let hover = mix_color32(color.surface.to_color32(), hover_tint(tokens.mode), 0.08);
    let pressed = mix_color32(color.surface.to_color32(), hover_tint(tokens.mode), 0.14);

    ctx.style_mut(|style| {
        style.animation_time = tokens.motion_secs();
        style.spacing.item_spacing = egui::vec2(metrics.gap, metrics.gap);
        style.spacing.button_padding = egui::vec2(metrics.pad_x, metrics.pad_y);
        style.spacing.interact_size = egui::vec2(metrics.control, metrics.control);
        style.spacing.window_margin = egui::Margin::same(pad_i8(metrics.gap));
        // The painted separator stays 1 px. This only widens the invisible drag strip.
        style.interaction.resize_grab_radius_side = 8.0;

        style.text_styles.insert(
            TextStyle::Small,
            FontId::new(metrics.caption, FontFamily::Proportional),
        );
        style.text_styles.insert(
            TextStyle::Body,
            FontId::new(metrics.body, FontFamily::Proportional),
        );
        style.text_styles.insert(
            TextStyle::Button,
            FontId::new(metrics.body, FontFamily::Proportional),
        );
        style.text_styles.insert(
            TextStyle::Heading,
            FontId::new(metrics.title, semibold_family()),
        );
        style.text_styles.insert(
            TextStyle::Monospace,
            FontId::new(metrics.code, FontFamily::Monospace),
        );

        let mut visuals = if tokens.mode == Mode::Dark {
            Visuals::dark()
        } else {
            Visuals::light()
        };
        paint_visuals(&mut visuals, tokens, radius, hover, pressed);
        style.visuals = visuals;
    });
}

/// Tokens da última chamada de [`apply`]. Sem isso, volta o preset escuro.
pub fn applied_tokens(ctx: &egui::Context) -> Tokens {
    ctx.data(|data| data.get_temp(egui::Id::new(TOKENS_ID)))
        .unwrap_or_default()
}

pub(crate) fn semibold_family() -> FontFamily {
    FontFamily::Name("forja-semibold".into())
}

fn paint_visuals(
    visuals: &mut Visuals,
    tokens: &Tokens,
    radius: u8,
    hover: Color32,
    pressed: Color32,
) {
    let color = tokens.color;
    let text = color.text.to_color32();
    let muted = color.text_muted.to_color32();
    let surface = color.surface.to_color32();
    let border = Stroke::new(1.0, color.border.to_color32());
    let accent = color.accent.to_color32();

    visuals.dark_mode = tokens.mode == Mode::Dark;
    visuals.window_fill = surface;
    visuals.panel_fill = color.bg.to_color32();
    visuals.extreme_bg_color = color.bg.to_color32();
    visuals.faint_bg_color = color.surface_alt.to_color32();
    visuals.code_bg_color = color.surface_alt.to_color32();
    visuals.window_stroke = border;
    visuals.weak_text_color = Some(muted);
    visuals.hyperlink_color = color.info.to_color32();
    visuals.warn_fg_color = color.spark.to_color32();
    visuals.error_fg_color = color.danger.to_color32();
    visuals.selection.bg_fill = color.accent_soft.to_color32();
    visuals.selection.stroke = Stroke::new(1.0, accent);
    visuals.text_cursor.stroke = Stroke::new(2.0, accent);
    visuals.window_shadow = elevation_shadow(3, tokens.mode);
    visuals.popup_shadow = elevation_shadow(2, tokens.mode);

    for widget in [
        &mut visuals.widgets.noninteractive,
        &mut visuals.widgets.inactive,
        &mut visuals.widgets.hovered,
        &mut visuals.widgets.active,
        &mut visuals.widgets.open,
    ] {
        widget.fg_stroke = Stroke::new(1.0, text);
        widget.bg_stroke = border;
        widget.corner_radius = CornerRadius::same(radius);
    }
    visuals.widgets.noninteractive.bg_fill = color.bg.to_color32();
    visuals.widgets.inactive.bg_fill = surface;
    visuals.widgets.hovered.bg_fill = hover;
    visuals.widgets.active.bg_fill = pressed;
    visuals.widgets.open.bg_fill = hover;
    visuals.widgets.noninteractive.fg_stroke.color = text;
}

pub(crate) fn elevation_shadow(level: u8, mode: Mode) -> egui::Shadow {
    let (offset_y, blur, alpha) = match (level, mode) {
        (1, Mode::Dark) => (2, 8, 50),
        (2, Mode::Dark) => (4, 16, 80),
        (3, Mode::Dark) => (8, 24, 120),
        (1, Mode::Light) => (1, 6, 28),
        (2, Mode::Light) => (4, 16, 40),
        _ => (8, 28, 64),
    };
    egui::Shadow {
        offset: [0, offset_y],
        blur,
        spread: 0,
        color: Color32::from_black_alpha(alpha),
    }
}

pub(crate) fn hover_tint(mode: Mode) -> Color32 {
    if mode == Mode::Dark {
        Color32::WHITE
    } else {
        Color32::BLACK
    }
}

pub(crate) fn mix_color32(base: Color32, tint: Color32, amount: f32) -> Color32 {
    let amount = amount.clamp(0.0, 1.0);
    let blend = |from: u8, to: u8| -> u8 {
        let mixed = f32::from(from) * (1.0 - amount) + f32::from(to) * amount;
        mixed.round().clamp(0.0, 255.0) as u8
    };
    Color32::from_rgba_unmultiplied(
        blend(base.r(), tint.r()),
        blend(base.g(), tint.g()),
        blend(base.b(), tint.b()),
        blend(base.a(), tint.a()),
    )
}

pub(crate) fn radius_u8(value: f32) -> u8 {
    let rounded = value.round();
    if rounded <= 0.0 {
        0
    } else if rounded >= 255.0 {
        255
    } else {
        rounded as u8
    }
}

pub(crate) fn pad_i8(value: f32) -> i8 {
    let rounded = value.round();
    if rounded <= 0.0 {
        0
    } else if rounded >= 127.0 {
        127
    } else {
        rounded as i8
    }
}
