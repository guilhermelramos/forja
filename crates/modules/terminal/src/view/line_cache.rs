//! Cache de desenho por linha para a grade do terminal.
//!
//! Cada `Row` carrega um `stamp` monotônico que muda apenas quando suas células
//! são alteradas. Enquanto largura, fonte, escala, tema e `stamp` não mudam,
//! reutilizamos o `Galley` já moldado pelo egui em vez de reconstruir um
//! `LayoutJob` por linha em todo quadro.

use crate::panes::tree::TerminalId;
use crate::theme::Palette;
use crate::view::render::GridMetrics;
use crate::vt::Row;
use forja_kernel::ui_contract::egui::{self, Color32, FontId};
use std::collections::HashMap;
use std::sync::Arc;

/// Máximo de linhas em cache por terminal (cobre a tela cheia + margem de rolagem).
pub const MAX_CACHED_LINES: usize = 512;

/// Configuração visual compartilhada pelas linhas em cache de um terminal.
#[derive(Clone, Debug, PartialEq)]
pub struct CacheConfig {
    pub cols: usize,
    pub rows: usize,
    pub font_id: FontId,
    pub line_height_bits: u32,
    pub char_width_bits: u32,
    pub pixels_per_point_bits: u32,
    pub palette: Palette,
    pub fonts_epoch: u64,
}

impl CacheConfig {
    /// Constrói a configuração a partir das medidas e parâmetros do quadro atual.
    pub fn from_metrics(
        metrics: &GridMetrics,
        rows: usize,
        font_id: FontId,
        pixels_per_point: f32,
        palette: &Palette,
        fonts_epoch: u64,
    ) -> Self {
        Self {
            cols: metrics.cols,
            rows,
            font_id,
            line_height_bits: metrics.line_height.to_bits(),
            char_width_bits: metrics.char_width.to_bits(),
            pixels_per_point_bits: pixels_per_point.to_bits(),
            palette: palette.clone(),
            fonts_epoch,
        }
    }
}

/// Glifo largo (CJK ou emoji) pré-moldado na coluna em que aparece.
pub struct CachedWideGlyph {
    pub col: usize,
    pub galley: Arc<egui::Galley>,
}

/// Traço de sobrelinha (SGR 53) em uma coluna da linha.
pub struct CachedOverline {
    pub col: usize,
    pub color: Color32,
}

/// Partes produzidas ao moldar uma linha fora do cache.
pub struct CachedLineParts {
    pub galley: Arc<egui::Galley>,
    pub wide_glyphs: Vec<CachedWideGlyph>,
    pub overlines: Vec<CachedOverline>,
    pub has_wide: bool,
}

/// Entrada em cache pronta para pintar.
pub struct CachedLine {
    pub galley: Arc<egui::Galley>,
    pub wide_glyphs: Vec<CachedWideGlyph>,
    pub overlines: Vec<CachedOverline>,
    pub has_wide: bool,
    last_used: u64,
}

/// Cache limitado de linhas moldadas para um único terminal.
pub struct TerminalLineCache {
    config: Option<CacheConfig>,
    entries: HashMap<u64, CachedLine>,
    tick: u64,
    max_entries: usize,
    hits: u64,
    misses: u64,
    clears: u64,
}

impl Default for TerminalLineCache {
    fn default() -> Self {
        Self::with_capacity(MAX_CACHED_LINES)
    }
}

impl TerminalLineCache {
    /// Cria um cache com teto explícito de entradas.
    pub fn with_capacity(max_entries: usize) -> Self {
        Self {
            config: None,
            entries: HashMap::new(),
            tick: 0,
            max_entries: max_entries.max(1),
            hits: 0,
            misses: 0,
            clears: 0,
        }
    }

    /// Garante que o cache corresponde à configuração atual; limpa se algo mudou.
    pub fn prepare(&mut self, config: CacheConfig) {
        if self.config.as_ref() != Some(&config) {
            if !self.entries.is_empty() {
                self.entries.clear();
                self.clears += 1;
            }
            self.config = Some(config);
        }
    }

    /// Devolve a linha em cache ou executa `build` apenas para esta linha.
    pub fn get_or_build(
        &mut self,
        row: &Row,
        build: impl FnOnce() -> CachedLineParts,
    ) -> &CachedLine {
        let stamp = row.stamp();
        self.tick = self.tick.wrapping_add(1);
        if self.entries.contains_key(&stamp) {
            self.hits += 1;
            let entry = self.entries.get_mut(&stamp).expect("checked");
            entry.last_used = self.tick;
            return entry;
        }
        self.misses += 1;
        if self.entries.len() >= self.max_entries {
            if let Some(&oldest) = self
                .entries
                .iter()
                .min_by_key(|(_, entry)| entry.last_used)
                .map(|(key, _)| key)
            {
                self.entries.remove(&oldest);
            }
        }
        let parts = build();
        self.entries.insert(
            stamp,
            CachedLine {
                galley: parts.galley,
                wide_glyphs: parts.wide_glyphs,
                overlines: parts.overlines,
                has_wide: parts.has_wide,
                last_used: self.tick,
            },
        );
        self.entries.get(&stamp).expect("just inserted")
    }

    /// Quantas linhas estão armazenadas agora.
    #[cfg(test)]
    pub fn len(&self) -> usize {
        self.entries.len()
    }

    /// Acertos acumulados desde a criação.
    #[cfg(test)]
    pub fn hits(&self) -> u64 {
        self.hits
    }

    /// Falhas acumuladas desde a criação.
    #[cfg(test)]
    pub fn misses(&self) -> u64 {
        self.misses
    }

    /// Quantas vezes o cache foi esvaziado por mudança de configuração.
    #[cfg(test)]
    pub fn clears(&self) -> u64 {
        self.clears
    }
}

/// Conjunto de caches por terminal mantido pela view.
#[derive(Default)]
pub struct LineCache {
    panes: HashMap<TerminalId, TerminalLineCache>,
}

impl LineCache {
    /// Obtém ou cria o cache do terminal indicado.
    pub fn for_terminal(&mut self, id: TerminalId) -> &mut TerminalLineCache {
        self.panes.entry(id).or_default()
    }

    /// Remove caches de terminais que foram fechados.
    pub fn retain_terminals(&mut self, mut keep: impl FnMut(TerminalId) -> bool) {
        self.panes.retain(|id, _| keep(*id));
    }
}

#[cfg(test)]
#[path = "line_cache_tests.rs"]
mod tests;
