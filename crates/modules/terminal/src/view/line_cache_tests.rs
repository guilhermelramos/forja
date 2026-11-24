use super::*;
use crate::settings::TermSettings;
use crate::terminal::{Selection, Terminal};
use crate::theme::Palette;
use crate::view::render::draw_grid;
use forja_kernel::ui_contract::egui::{self, Color32, Rect};
use std::collections::HashMap;
use std::time::Instant;

fn test_palette(bg: Color32) -> Palette {
    Palette {
        background: bg,
        foreground: Color32::from_gray(230),
        cursor: Color32::from_rgb(242, 118, 46),
        selection: Color32::from_rgb(60, 90, 140),
        selection_foreground: Color32::WHITE,
        ansi: [Color32::WHITE; 16],
    }
}

fn draw_frame(
    ctx: &egui::Context,
    terminal: &mut Terminal,
    settings: &TermSettings,
    palette: &Palette,
    cache: &mut TerminalLineCache,
    body: Rect,
) {
    let mut textures = HashMap::new();
    let _ = ctx.run(egui::RawInput::default(), |ctx| {
        egui::CentralPanel::default().show(ctx, |ui| {
            let _ = draw_grid(
                ui,
                terminal,
                settings,
                palette,
                &mut textures,
                cache,
                body,
                true,
                "Terminal: teste",
            );
        });
    });
}

#[test]
fn cache_rebuilds_only_altered_lines() {
    let ctx = egui::Context::default();
    let mut terminal = Terminal::test_dummy(1);
    let settings = TermSettings::default();
    let palette = test_palette(Color32::BLACK);
    let mut cache = TerminalLineCache::default();
    let body = Rect::from_min_size(egui::pos2(0.0, 0.0), egui::vec2(720.0, 420.0));

    // Primeira passagem dimensiona a grade e popula o cache.
    draw_frame(&ctx, &mut terminal, &settings, &palette, &mut cache, body);
    let rows = terminal.visible_rows;
    assert!(rows >= 10);
    assert_eq!(cache.misses(), rows as u64);
    assert_eq!(cache.hits(), 0);

    // Quadro parado: nenhuma linha deve ser refeita.
    draw_frame(&ctx, &mut terminal, &settings, &palette, &mut cache, body);
    assert_eq!(cache.misses(), rows as u64);
    assert_eq!(cache.hits(), rows as u64);

    // Altera apenas a linha 4 (1-indexed: linha 5 no CSI H).
    terminal.surface.feed(b"\x1b[5;1Hlinha alterada");
    draw_frame(&ctx, &mut terminal, &settings, &palette, &mut cache, body);
    assert_eq!(cache.misses(), rows as u64 + 1);
    assert_eq!(cache.hits(), (rows * 2 - 1) as u64);

    // Seleção e movimento de cursor são camadas por cima: não invalidam o Galley.
    terminal.selection = Some(Selection {
        anchor: (0, 0),
        head: (2, 10),
        generation: terminal.surface.generation,
    });
    terminal.surface.feed(b"\x1b[2;3H");
    draw_frame(&ctx, &mut terminal, &settings, &palette, &mut cache, body);
    assert_eq!(cache.misses(), rows as u64 + 1);
    assert_eq!(cache.hits(), (rows * 3 - 1) as u64);
}

#[test]
fn cache_clears_on_resize_font_and_theme_change() {
    let ctx = egui::Context::default();
    let mut terminal = Terminal::test_dummy(2);
    let mut settings = TermSettings::default();
    let mut palette = test_palette(Color32::BLACK);
    let mut cache = TerminalLineCache::default();
    let body = Rect::from_min_size(egui::pos2(0.0, 0.0), egui::vec2(720.0, 420.0));

    draw_frame(&ctx, &mut terminal, &settings, &palette, &mut cache, body);
    let initial_clears = cache.clears();

    // Redimensionar altera colunas/linhas e limpa o cache.
    let smaller = Rect::from_min_size(egui::pos2(0.0, 0.0), egui::vec2(520.0, 320.0));
    draw_frame(&ctx, &mut terminal, &settings, &palette, &mut cache, smaller);
    assert_eq!(cache.clears(), initial_clears + 1);

    // Trocar tamanho da fonte limpa o cache.
    settings.font_size = 16.0;
    draw_frame(&ctx, &mut terminal, &settings, &palette, &mut cache, smaller);
    assert_eq!(cache.clears(), initial_clears + 2);

    // Trocar tema (paleta) limpa o cache.
    palette = test_palette(Color32::from_rgb(20, 22, 26));
    draw_frame(&ctx, &mut terminal, &settings, &palette, &mut cache, smaller);
    assert_eq!(cache.clears(), initial_clears + 3);
}

#[test]
fn cache_has_bounded_size() {
    let ctx = egui::Context::default();
    let mut terminal = Terminal::test_dummy(3);
    let settings = TermSettings::default();
    let palette = test_palette(Color32::BLACK);
    let mut cache = TerminalLineCache::with_capacity(32);
    let body = Rect::from_min_size(egui::pos2(0.0, 0.0), egui::vec2(720.0, 420.0));

    draw_frame(&ctx, &mut terminal, &settings, &palette, &mut cache, body);
    for idx in 0..200 {
        terminal.surface.feed(format!("linha {idx}\r\n").as_bytes());
        if idx % 10 == 0 {
            draw_frame(&ctx, &mut terminal, &settings, &palette, &mut cache, body);
            assert!(cache.len() <= 32, "cache exceeded cap: {}", cache.len());
        }
    }
}

#[test]
fn cache_preserves_mode_2026_and_sgr9_overline_wide() {
    let ctx = egui::Context::default();
    let mut terminal = Terminal::test_dummy(4);
    let settings = TermSettings::default();
    let palette = test_palette(Color32::BLACK);
    let mut cache = TerminalLineCache::default();
    let body = Rect::from_min_size(egui::pos2(0.0, 0.0), egui::vec2(720.0, 420.0));

    draw_frame(&ctx, &mut terminal, &settings, &palette, &mut cache, body);
    let rows = terminal.visible_rows as u64;
    terminal
        .surface
        .feed(b"\x1b[1;1H\x1b[9;53mtachado+sobrelinha \xe4\xb8\xad\xe6\x96\x87");
    draw_frame(&ctx, &mut terminal, &settings, &palette, &mut cache, body);
    assert_eq!(cache.misses(), rows + 1);
    // Se a fonte fallback CJK acabou de ser instalada, o próximo quadro reaquece o cache.
    draw_frame(&ctx, &mut terminal, &settings, &palette, &mut cache, body);
    let misses_before_sync = cache.misses();

    // Durante o modo 2026, display() mantém o quadro congelado e o cache segue acertando.
    terminal.surface.feed(b"\x1b[?2026h\x1b[2;1Hquadro privado");
    draw_frame(&ctx, &mut terminal, &settings, &palette, &mut cache, body);
    assert_eq!(cache.misses(), misses_before_sync);

    // Ao encerrar 2026, somente a linha alterada é refeita.
    terminal.surface.feed(b"\x1b[?2026l");
    draw_frame(&ctx, &mut terminal, &settings, &palette, &mut cache, body);
    assert_eq!(cache.misses(), misses_before_sync + 1);
}

#[test]
fn perf_5000_lines_cached_vs_uncached_and_multi_pane() {
    let ctx = egui::Context::default();
    let settings = TermSettings::default();
    let palette = test_palette(Color32::from_rgb(18, 18, 20));
    let body = Rect::from_min_size(egui::pos2(0.0, 0.0), egui::vec2(1080.0, 720.0));

    let mut payload = String::new();
    for idx in 1..=5_000 {
        payload.push_str(&format!(
            "linha {idx:04} \x1b[32mverde\x1b[0m \x1b[33;9mamarelo\x1b[0m texto normal\r\n"
        ));
    }

    let mut terminal = Terminal::test_dummy(10);
    let mut cache = TerminalLineCache::default();
    draw_frame(&ctx, &mut terminal, &settings, &palette, &mut cache, body);
    terminal.surface.feed(payload.as_bytes());
    draw_frame(&ctx, &mut terminal, &settings, &palette, &mut cache, body);

    let frames = 20u32;

    // Sem cache (simulando quadro a quadro antes da mudança: limpa antes de cada desenho).
    let uncached_start = Instant::now();
    for _ in 0..frames {
        let mut fresh = TerminalLineCache::default();
        draw_frame(&ctx, &mut terminal, &settings, &palette, &mut fresh, body);
    }
    let uncached_ms = uncached_start.elapsed().as_secs_f64() * 1000.0 / f64::from(frames);

    // Com cache aquecido e terminal parado.
    let cached_start = Instant::now();
    for _ in 0..frames {
        draw_frame(&ctx, &mut terminal, &settings, &palette, &mut cache, body);
    }
    let cached_ms = cached_start.elapsed().as_secs_f64() * 1000.0 / f64::from(frames);

    // Simula 8 vs 12 painéis visíveis (mesma área dividida em grade).
    let mut panes = Vec::new();
    let mut pane_caches = Vec::new();
    for id in 0..12u64 {
        let mut pane = Terminal::test_dummy(100 + id);
        let mut pcache = TerminalLineCache::default();
        let pane_body = Rect::from_min_size(
            egui::pos2((id % 4) as f32 * 270.0, (id / 4) as f32 * 240.0),
            egui::vec2(266.0, 236.0),
        );
        draw_frame(&ctx, &mut pane, &settings, &palette, &mut pcache, pane_body);
        pane.surface.feed(payload.as_bytes());
        draw_frame(&ctx, &mut pane, &settings, &palette, &mut pcache, pane_body);
        panes.push((pane, pane_body));
        pane_caches.push(pcache);
    }

    let measure_panes = |count: usize, panes: &mut [(Terminal, Rect)], caches: &mut [TerminalLineCache]| {
        let mut textures = HashMap::new();
        let start = Instant::now();
        for _ in 0..frames {
            let _ = ctx.run(egui::RawInput::default(), |ctx| {
                egui::CentralPanel::default().show(ctx, |ui| {
                    for idx in 0..count {
                        let (pane, pane_body) = &mut panes[idx];
                        let _ = draw_grid(
                            ui,
                            pane,
                            &settings,
                            &palette,
                            &mut textures,
                            &mut caches[idx],
                            *pane_body,
                            idx == 0,
                            "Terminal: teste",
                        );
                    }
                });
            });
        }
        start.elapsed().as_secs_f64() * 1000.0 / f64::from(frames)
    };

    let panes_8_ms = measure_panes(8, &mut panes, &mut pane_caches);
    let panes_12_ms = measure_panes(12, &mut panes, &mut pane_caches);

    println!(
        "PERF terminal 5000 linhas (debug): sem_cache={uncached_ms:.2} ms/quadro, com_cache={cached_ms:.2} ms/quadro, 8_paineis={panes_8_ms:.2} ms/quadro, 12_paineis={panes_12_ms:.2} ms/quadro"
    );
    assert!(cached_ms < uncached_ms);
}
