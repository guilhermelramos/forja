//! Invariant checks run after each explorer action and deduplicated JSONL finding log.

use crate::ftl::TranslationDict;
use crate::snapshot::{Element, Snapshot};
use serde::{Deserialize, Serialize};
use std::collections::HashSet;
use std::io::Write as _;
use std::path::PathBuf;

/// Finding record matching `forja-mcp --player`'s `report_finding` JSONL schema.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct Finding {
    pub title: String,
    pub kind: String,
    pub severity: String,
    pub steps: Vec<String>,
    pub expected: String,
    pub actual: String,
    pub screen: String,
}

/// Violation discovered by checking invariants, paired with its deduplication key.
#[derive(Debug, Clone)]
pub struct Violation {
    pub dedup_key: String,
    pub finding: Finding,
}

/// Appends deduplicated findings to an optional JSONL file.
pub struct FindingLog {
    path: Option<PathBuf>,
    seen: HashSet<String>,
    recorded: Vec<Finding>,
}

impl FindingLog {
    pub fn new(path: Option<PathBuf>) -> Self {
        Self {
            path,
            seen: HashSet::new(),
            recorded: Vec::new(),
        }
    }

    /// Records a violation if its `dedup_key` has not been seen yet. Returns `true` if newly recorded.
    pub fn record(&mut self, violation: Violation) -> std::io::Result<bool> {
        if !self.seen.insert(violation.dedup_key) {
            return Ok(false);
        }
        if let Some(path) = &self.path {
            if let Some(parent) = path.parent() {
                if !parent.as_os_str().is_empty() {
                    std::fs::create_dir_all(parent)?;
                }
            }
            let mut file = std::fs::OpenOptions::new()
                .create(true)
                .append(true)
                .open(path)?;
            let line = serde_json::to_string(&violation.finding)
                .map_err(std::io::Error::other)?;
            writeln!(file, "{line}")?;
            file.flush()?;
        }
        self.recorded.push(violation.finding);
        Ok(true)
    }

    pub fn count(&self) -> usize {
        self.recorded.len()
    }

    pub fn findings(&self) -> &[Finding] {
        &self.recorded
    }
}

/// Checks invariants 2, 3, 4, and 6 on `snapshot`.
pub fn check_snapshot(
    snapshot: &Snapshot,
    locale: &str,
    dict: &TranslationDict,
    recent_steps: &[String],
) -> Vec<Violation> {
    let mut out = Vec::new();
    let screen = snapshot.screen_label();

    check_untranslated(snapshot, &screen, locale, dict, recent_steps, &mut out);
    check_raw_errors(snapshot, &screen, locale, dict, recent_steps, &mut out);
    check_geometry(snapshot, &screen, recent_steps, &mut out);
    check_nameless_clickable(snapshot, &screen, recent_steps, &mut out);

    out
}

/// Returns human-readable descriptions of any geometry violations in `snapshot`.
pub(crate) fn geometry_issues(snapshot: &Snapshot) -> Vec<String> {
    let mut out = Vec::new();
    check_geometry(snapshot, &snapshot.screen_label(), &[], &mut out);
    out.into_iter()
        .map(|v| format!("{}: {}", v.finding.title, v.finding.actual))
        .collect()
}

/// Checks invariant 5: pressing `Escape` when at least one `Window` is open must close exactly one.
pub fn check_escape_window(
    before: &Snapshot,
    after: &Snapshot,
    recent_steps: &[String],
) -> Option<Violation> {
    let before_windows = before.window_count();
    if before_windows == 0 {
        return None;
    }
    let after_windows = after.window_count();
    if after_windows == before_windows - 1 {
        return None;
    }
    let screen = before.screen_label();
    let title = if after_windows >= before_windows {
        format!("Escape não fechou janela aberta em '{screen}'")
    } else {
        format!("Escape fechou janelas demais em '{screen}'")
    };
    Some(Violation {
        dedup_key: format!("escape_window:{screen}:{before_windows}->{after_windows}"),
        finding: Finding {
            title,
            kind: "bug".to_owned(),
            severity: "high".to_owned(),
            steps: recent_steps.to_vec(),
            expected: format!(
                "Com {before_windows} janela(s) aberta(s), Escape deveria deixar {} janela(s).",
                before_windows - 1
            ),
            actual: format!("Continuaram {after_windows} janela(s) aberta(s)."),
            screen,
        },
    })
}

/// Builds a violation for invariant 1 (crash or hang > 10s).
pub fn crash_or_hang_violation(
    is_hang: bool,
    screen: &str,
    detail: &str,
    recent_steps: &[String],
) -> Violation {
    let last = recent_steps.last().map(String::as_str).unwrap_or("inicio");
    let kind = if is_hang { "hang" } else { "crash" };
    let title = if is_hang {
        format!("App travou (>10 s) em '{screen}' após {last}")
    } else {
        format!("App fechou inesperadamente em '{screen}' após {last}")
    };
    Violation {
        dedup_key: format!("{kind}:{screen}:{last}"),
        finding: Finding {
            title,
            kind: kind.to_owned(),
            severity: "critical".to_owned(),
            steps: recent_steps.to_vec(),
            expected: "O aplicativo continua vivo e responde em menos de 10 s.".to_owned(),
            actual: detail.to_owned(),
            screen: screen.to_owned(),
        },
    }
}

fn check_untranslated(
    snapshot: &Snapshot,
    screen: &str,
    locale: &str,
    dict: &TranslationDict,
    recent_steps: &[String],
    out: &mut Vec<Violation>,
) {
    for el in &snapshot.elements {
        let name = el.name.trim();
        if name.is_empty() {
            continue;
        }
        if let Some(matched) = dict.check_untranslated(locale, name) {
            out.push(Violation {
                dedup_key: format!("untranslated:{screen}:{matched}"),
                finding: Finding {
                    title: format!("Texto sem tradução ({locale}): \"{matched}\" em '{screen}'"),
                    kind: "text".to_owned(),
                    severity: "medium".to_owned(),
                    steps: recent_steps.to_vec(),
                    expected: format!("Todos os textos da tela traduzidos para {locale}."),
                    actual: format!("Elemento {} exibindo \"{matched}\".", el.role),
                    screen: screen.to_owned(),
                },
            });
        }
    }
}

fn check_raw_errors(
    snapshot: &Snapshot,
    screen: &str,
    locale: &str,
    dict: &TranslationDict,
    recent_steps: &[String],
    out: &mut Vec<Violation>,
) {
    for el in &snapshot.elements {
        let name = el.name.trim();
        if name.is_empty() || is_inside_technical_details(snapshot, el) {
            continue;
        }
        if let Some(reason) = detect_raw_error(name, locale, dict) {
            out.push(Violation {
                dedup_key: format!("raw_error:{screen}:{reason}:{name}"),
                finding: Finding {
                    title: format!("Erro técnico cru na tela ({reason}) em '{screen}'"),
                    kind: "bug".to_owned(),
                    severity: "high".to_owned(),
                    steps: recent_steps.to_vec(),
                    expected: "Mensagens de interface claras para o usuário, sem detalhes técnicos crus.".to_owned(),
                    actual: format!("{} exibindo \"{name}\" ({reason}).", el.role),
                    screen: screen.to_owned(),
                },
            });
        }
    }
}

fn is_details_label(name: &str) -> bool {
    matches!(
        name.trim(),
        "Detalhes técnicos" | "Technical details" | "Detalhes" | "Details"
    )
}

fn rect_contains(outer: [f64; 4], inner: [f64; 4], margin: f64) -> bool {
    inner[0] >= outer[0] - margin
        && inner[1] >= outer[1] - margin
        && inner[2] <= outer[2] + margin
        && inner[3] <= outer[3] + margin
}

/// Technical details drawers (`forja_ui_kit::error_banner`, terminal details)
/// and the status-bar details modal intentionally reveal raw diagnostics only
/// after explicit expansion.
fn is_inside_technical_details(snapshot: &Snapshot, el: &Element) -> bool {
    if el.role != "Label" {
        return false;
    }
    let in_modal = snapshot.elements.iter().any(|w| {
        w.role == "Window" && is_details_label(&w.name) && rect_contains(w.rect, el.rect, 4.0)
    });
    if in_modal {
        return true;
    }
    snapshot.elements.iter().any(|header| {
        if header.role != "Button" || !is_details_label(&header.name) {
            return false;
        }
        let indented = el.rect[0] >= header.rect[0] + 6.0 && el.rect[0] <= header.rect[0] + 64.0;
        let directly_below =
            el.rect[1] >= header.rect[3] - 2.0 && el.rect[1] <= header.rect[3] + 36.0;
        if !indented || !directly_below {
            return false;
        }
        let blocked_between = snapshot.elements.iter().any(|other| {
            other.id != header.id
                && other.id != el.id
                && other.rect[0] <= header.rect[0] + 4.0
                && other.rect[2] >= header.rect[0]
                && other.rect[1] >= header.rect[3] - 2.0
                && other.rect[3] <= el.rect[1] + 2.0
        });
        !blocked_between
    })
}

/// Detects raw technical error patterns in visible UI text.
pub fn detect_raw_error(text: &str, locale: &str, dict: &TranslationDict) -> Option<&'static str> {
    let lower = text.to_ascii_lowercase();
    let is_en = locale.trim().to_ascii_lowercase().starts_with("en");

    if text.contains("{\"") {
        return Some("JSON cru");
    }
    if text.contains(r"\\?\") {
        return Some(r"prefixo \\?\");
    }
    if lower.contains("os error") {
        return Some("os error");
    }
    if lower.contains("invalid setting") {
        return Some("invalid setting");
    }
    if has_backtick_internal_id(text) {
        return Some("id interno entre crases");
    }
    if lower.contains("error:") && !(is_en && dict.is_known_en_label(text)) {
        return Some("error:");
    }
    if has_ascii_word(&lower, "failed") && !(is_en && dict.is_known_en_label(text)) {
        return Some("failed");
    }
    None
}

fn has_ascii_word(haystack: &str, word: &str) -> bool {
    let bytes = haystack.as_bytes();
    let mut start = 0;
    while let Some(offset) = haystack[start..].find(word) {
        let idx = start + offset;
        let end = idx + word.len();
        let left_ok = idx == 0 || !bytes[idx - 1].is_ascii_alphanumeric();
        let right_ok = end == bytes.len() || !bytes[end].is_ascii_alphanumeric();
        if left_ok && right_ok {
            return true;
        }
        start = idx + 1;
    }
    false
}

fn has_backtick_internal_id(text: &str) -> bool {
    let mut rest = text;
    while let Some(open) = rest.find('`') {
        let after = &rest[open + 1..];
        let Some(close) = after.find('`') else {
            break;
        };
        let inner = &after[..close];
        if inner.len() >= 3
            && inner.len() <= 64
            && !inner.contains(char::is_whitespace)
            && (inner.contains('.') || inner.contains('_'))
            && inner
                .chars()
                .all(|c| c.is_ascii_alphanumeric() || matches!(c, '.' | '_' | '-' | ':'))
        {
            return true;
        }
        rest = &after[close + 1..];
    }
    false
}

fn check_geometry(
    snapshot: &Snapshot,
    screen: &str,
    recent_steps: &[String],
    out: &mut Vec<Violation>,
) {
    let win_w = snapshot.window.width;
    let win_h = snapshot.window.height;
    if win_w > 0.0 && win_h > 0.0 {
        for el in &snapshot.elements {
            let [x0, y0, x1, y1] = el.rect;
            if x0 < -2.0 || y0 < -2.0 || x1 > win_w + 2.0 || y1 > win_h + 2.0 {
                let label = if el.name.is_empty() {
                    "(sem nome)".to_owned()
                } else {
                    el.name.clone()
                };
                out.push(Violation {
                    dedup_key: format!("outside:{screen}:{}:{label}", el.role),
                    finding: Finding {
                        title: format!("Elemento fora da janela: {} \"{label}\" em '{screen}'", el.role),
                        kind: "bug".to_owned(),
                        severity: "medium".to_owned(),
                        steps: recent_steps.to_vec(),
                        expected: format!("Elemento contido na janela ({win_w:.0}x{win_h:.0})."),
                        actual: format!(
                            "Retângulo ({x0:.0},{y0:.0})-({x1:.0},{y1:.0}) ultrapassa {win_w:.0}x{win_h:.0}."
                        ),
                        screen: screen.to_owned(),
                    },
                });
            }
        }
    }

    // Collect open Window and Popup menu layers so foreground popups aren't paired with background tabs.
    let window_rects: Vec<[f64; 4]> = snapshot
        .elements
        .iter()
        .filter(|el| el.role == "Window")
        .map(|el| el.rect)
        .collect();
    let popup_layers = detect_popup_layers(&snapshot.elements);

    let tab_candidates: Vec<(usize, &Element)> = snapshot
        .elements
        .iter()
        .enumerate()
        .filter(|(_, el)| {
            el.clickable
                && matches!(el.role.as_str(), "Button" | "Tab")
                && !el.name.trim().is_empty()
        })
        .collect();

    for i in 0..tab_candidates.len() {
        for j in (i + 1)..tab_candidates.len() {
            let (idx_a, a) = tab_candidates[i];
            let (idx_b, b) = tab_candidates[j];
            if (a.rect[1] - b.rect[1]).abs() > 4.0 || (a.rect[3] - b.rect[3]).abs() > 4.0 {
                continue;
            }
            let overlap_x = a.rect[2].min(b.rect[2]) - a.rect[0].max(b.rect[0]);
            if overlap_x <= 4.0 {
                continue;
            }
            if a.window_id != b.window_id
                || popup_layers[idx_a] != popup_layers[idx_b]
                || (a.window_id.is_none()
                    && enclosing_windows(a, &window_rects) != enclosing_windows(b, &window_rects))
            {
                continue;
            }
            let (first, second) = if a.name <= b.name {
                (a.name.trim(), b.name.trim())
            } else {
                (b.name.trim(), a.name.trim())
            };
            out.push(Violation {
                dedup_key: format!("overlap:{screen}:{first}:{second}"),
                finding: Finding {
                    title: format!("Abas/botões sobrepostos na mesma faixa: \"{first}\" e \"{second}\""),
                    kind: "bug".to_owned(),
                    severity: "medium".to_owned(),
                    steps: recent_steps.to_vec(),
                    expected: "Abas da mesma faixa sem sobreposição horizontal.".to_owned(),
                    actual: format!(
                        "\"{}\" ({:.0},{:.0})-({:.0},{:.0}) sobrepõe \"{}\" ({:.0},{:.0})-({:.0},{:.0}) em {overlap_x:.0} px.",
                        a.name, a.rect[0], a.rect[1], a.rect[2], a.rect[3],
                        b.name, b.rect[0], b.rect[1], b.rect[2], b.rect[3],
                    ),
                    screen: screen.to_owned(),
                },
            });
        }
    }
}

fn enclosing_windows(el: &Element, windows: &[[f64; 4]]) -> Vec<usize> {
    let (cx, cy) = el.centre();
    windows
        .iter()
        .enumerate()
        .filter(|(_, w)| cx >= w[0] && cx <= w[2] && cy >= w[1] && cy <= w[3])
        .map(|(idx, _)| idx)
        .collect()
}

/// Identifies floating vertical popup containers (nameless clickable `Unknown` boxes holding
/// full-width stacked menu buttons) and assigns each menu row its tightest container index.
fn detect_popup_layers(elements: &[Element]) -> Vec<Option<usize>> {
    let mut layers = vec![None; elements.len()];
    let mut containers: Vec<(usize, f64)> = elements
        .iter()
        .enumerate()
        .filter(|(_, el)| el.role == "Unknown" && el.name.trim().is_empty() && el.clickable)
        .map(|(idx, el)| (idx, el.width().max(0.0) * el.height().max(0.0)))
        .collect();
    // Smallest containers first so a scrolled popup assigns rows to the inner container consistently.
    containers.sort_by(|a, b| a.1.partial_cmp(&b.1).unwrap_or(std::cmp::Ordering::Equal));

    for (c_idx, _) in containers {
        let c = &elements[c_idx];
        let mut rows: Vec<(usize, f64, f64)> = elements
            .iter()
            .enumerate()
            .filter(|(idx, el)| {
                *idx != c_idx
                    && el.clickable
                    && matches!(el.role.as_str(), "Button" | "SelectableLabel")
                    && !el.name.trim().is_empty()
                    && el.rect[0] >= c.rect[0] - 2.0
                    && el.rect[2] <= c.rect[2] + 2.0
                    && el.rect[1] >= c.rect[1] - 2.0
                    && el.rect[3] <= c.rect[3] + 2.0
                    && el.width() >= (c.width() * 0.7).max(c.width() - 64.0)
            })
            .map(|(idx, el)| (idx, el.rect[1], el.rect[3]))
            .collect();
        if rows.is_empty() {
            continue;
        }
        rows.sort_by(|a, b| a.1.partial_cmp(&b.1).unwrap_or(std::cmp::Ordering::Equal));
        let vertical_stack = rows.windows(2).all(|pair| pair[1].1 >= pair[0].2 - 4.0);
        if !vertical_stack {
            continue;
        }
        for (r_idx, _, _) in rows {
            if layers[r_idx].is_none() {
                layers[r_idx] = Some(c_idx);
            }
        }
    }
    layers
}

fn check_nameless_clickable(
    snapshot: &Snapshot,
    screen: &str,
    recent_steps: &[String],
    out: &mut Vec<Violation>,
) {
    let nameless: Vec<String> = snapshot
        .elements
        .iter()
        .filter(|el| el.clickable && el.name.trim().is_empty())
        .map(|el| {
            format!(
                "{} ({:.0},{:.0})-({:.0},{:.0})",
                el.role, el.rect[0], el.rect[1], el.rect[2], el.rect[3]
            )
        })
        .collect();
    if nameless.is_empty() {
        return;
    }
    let sample: Vec<&str> = nameless.iter().take(8).map(String::as_str).collect();
    out.push(Violation {
        dedup_key: format!("nameless_clickable:{screen}"),
        finding: Finding {
            title: format!("Elemento clicável sem nome acessível em '{screen}'"),
            kind: "accessibility".to_owned(),
            severity: "low".to_owned(),
            steps: recent_steps.to_vec(),
            expected: "Todo elemento clicável possui nome acessível.".to_owned(),
            actual: format!(
                "{} elemento(s) clicável(is) sem nome: {}",
                nameless.len(),
                sample.join("; ")
            ),
            screen: screen.to_owned(),
        },
    });
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::snapshot::WindowSize;

    fn el(id: u64, role: &str, name: &str, rect: [f64; 4], clickable: bool) -> Element {
        Element {
            id,
            role: role.to_owned(),
            name: name.to_owned(),
            rect,
            clickable,
            editable: false,
            state: Vec::new(),
            window_id: None,
        }
    }

    fn el_in_window(
        id: u64,
        role: &str,
        name: &str,
        rect: [f64; 4],
        clickable: bool,
        window_id: Option<u64>,
    ) -> Element {
        Element {
            window_id,
            ..el(id, role, name, rect, clickable)
        }
    }

    fn snap(elements: Vec<Element>) -> Snapshot {
        Snapshot {
            window: WindowSize {
                width: 1920.0,
                height: 1080.0,
            },
            changes: Default::default(),
            view: String::new(),
            elements,
            frames: 0,
            closed: false,
        }
    }

    #[test]
    fn detects_untranslated_text() {
        let dict = TranslationDict::from_ftl_pairs(&[(
            "k = Settings\n".to_owned(),
            "k = Ajustes\n".to_owned(),
        )]);
        let s = snap(vec![el(1, "Button", "Settings", [10.0, 10.0, 90.0, 40.0], true)]);
        let v = check_snapshot(&s, "pt-BR", &dict, &[]);
        assert_eq!(v.len(), 1);
        assert_eq!(v[0].finding.kind, "text");
    }

    #[test]
    fn detects_raw_errors() {
        let dict = TranslationDict::from_ftl_pairs(&[(
            "ok = Notify failed checks\n".to_owned(),
            "ok = Avisar check quebrado\n".to_owned(),
        )]);
        let s = snap(vec![
            el(1, "Label", "command `bench.create` failed: os error 2", [10.0, 10.0, 300.0, 30.0], false),
            el(2, "Label", "{\"error\":\"bad\"}", [10.0, 40.0, 300.0, 60.0], false),
            el(3, "Label", r"\\?\C:\foo", [10.0, 70.0, 300.0, 90.0], false),
            el(4, "Label", "invalid setting foo", [10.0, 100.0, 300.0, 120.0], false),
        ]);
        let v = check_snapshot(&s, "pt-BR", &dict, &[]);
        assert_eq!(v.len(), 4);

        // Legitimate localized English label is allowed in `en` locale.
        let s_en = snap(vec![el(1, "Label", "Notify failed checks", [10.0, 10.0, 200.0, 30.0], false)]);
        let v_en = check_snapshot(&s_en, "en", &dict, &[]);
        assert!(v_en.is_empty());

        // Intentional details inside an expanded CollapsingHeader or details Window are allowed,
        // while a raw error in the summary label outside details is still caught.
        let expanded_drawer = snap(vec![
            el(10, "Label", "A busca falhou.", [483.0, 183.0, 700.0, 231.0], false),
            el(11, "Button", "Detalhes técnicos", [483.0, 243.0, 666.0, 291.0], true),
            el(12, "Label", "invalid query: Syntax Error: <>", [510.0, 303.0, 793.0, 327.0], false),
        ]);
        assert!(
            check_snapshot(&expanded_drawer, "pt-BR", &dict, &[]).is_empty(),
            "raw detail inside expanded CollapsingHeader should be allowed"
        );

        let leaked_summary = snap(vec![
            el(10, "Label", "command `helper.launch` failed: bad", [483.0, 183.0, 800.0, 231.0], false),
            el(11, "Button", "Detalhes técnicos", [483.0, 243.0, 666.0, 291.0], true),
            el(12, "Label", "command `helper.launch` failed: bad", [510.0, 303.0, 800.0, 327.0], false),
        ]);
        let leaked = check_snapshot(&leaked_summary, "pt-BR", &dict, &[]);
        assert_eq!(leaked.len(), 1);
        assert!(leaked[0].dedup_key.starts_with("raw_error:"));

        let details_modal = snap(vec![
            el(20, "Window", "Technical details", [500.0, 200.0, 1100.0, 500.0], false),
            el(21, "Label", "content.run: command `helper.launch` failed: missing", [520.0, 260.0, 1050.0, 300.0], false),
        ]);
        assert!(
            check_snapshot(&details_modal, "en", &dict, &[]).is_empty(),
            "raw detail inside Technical details modal window should be allowed"
        );
    }

    #[test]
    fn detects_outside_window_and_overlapping_tabs() {
        let s = snap(vec![
            el(1, "Button", "Tab A", [100.0, 75.0, 250.0, 123.0], true),
            el(2, "Button", "Tab B", [200.0, 75.0, 350.0, 123.0], true),
            el(3, "Button", "Tab C", [1850.0, 75.0, 2050.0, 123.0], true),
        ]);
        let v = check_snapshot(&s, "pt-BR", &TranslationDict::default(), &[]);
        assert!(v.iter().any(|x| x.dedup_key.starts_with("overlap:")));
        assert!(v.iter().any(|x| x.dedup_key.starts_with("outside:")));
    }

    #[test]
    fn popup_menu_rows_do_not_collide_with_background_tabs() {
        let s = snap(vec![
            el(1, "Button", "Painéis de extensões", [1057.0, 52.0, 1208.0, 84.0], true),
            el(2, "Button", "+", [1244.0, 48.0, 1277.0, 80.0], true),
            el(3, "Unknown", "", [1174.0, 42.0, 1390.0, 148.0], true),
            el(4, "Button", "Seguir o Windows", [1180.0, 48.0, 1384.0, 80.0], true),
            el(5, "Button", "Escuro", [1180.0, 82.0, 1384.0, 114.0], true),
        ]);
        let v = check_snapshot(&s, "pt-BR", &TranslationDict::default(), &[]);
        assert!(
            !v.iter().any(|x| x.dedup_key.starts_with("overlap:")),
            "unexpected overlap findings for foreground popup: {v:?}"
        );
    }

    #[test]
    fn foreground_window_buttons_do_not_collide_with_background_buttons() {
        let s = snap(vec![
            el_in_window(1, "Window", "Clonar repositório", [300.0, 200.0, 800.0, 400.0], false, None),
            el_in_window(2, "Button", "Criar", [340.0, 320.0, 410.0, 352.0], true, None),
            el_in_window(3, "Button", "Clonar", [337.0, 320.0, 412.0, 352.0], true, Some(1)),
        ]);
        let v = check_snapshot(&s, "pt-BR", &TranslationDict::default(), &[]);
        assert!(
            !v.iter().any(|x| x.dedup_key.starts_with("overlap:")),
            "background button should not collide with foreground window button: {v:?}"
        );
    }

    #[test]
    fn popup_menu_with_scrollbar_gap_does_not_collide_with_background_button() {
        let s = snap(vec![
            el(1, "Button", "Receitas", [1391.0, 979.0, 1464.0, 1011.0], true),
            el(2, "Unknown", "", [1305.0, 860.0, 1939.0, 1020.0], true),
            el(3, "Button", "PowerShell 7", [1311.0, 944.0, 1897.0, 976.0], true),
            el(4, "Button", "WSL  ·  Distribuição padrão", [1311.0, 978.0, 1897.0, 1010.0], true),
        ]);
        let v = check_snapshot(&s, "pt-BR", &TranslationDict::default(), &[]);
        assert!(
            !v.iter().any(|x| x.dedup_key.starts_with("overlap:")),
            "scrolled popup item should not collide with narrow background button: {v:?}"
        );
    }

    #[test]
    fn detects_escape_window_count_mismatch() {
        let before = snap(vec![el(1, "Window", "Ajustes", [100.0, 100.0, 500.0, 500.0], false)]);
        let still_open = snap(vec![el(1, "Window", "Ajustes", [100.0, 100.0, 500.0, 500.0], false)]);
        let closed = snap(vec![]);

        assert!(check_escape_window(&before, &still_open, &[]).is_some());
        assert!(check_escape_window(&before, &closed, &[]).is_none());
    }

    #[test]
    fn records_nameless_clickable_once_per_screen_and_deduplicates_log() {
        let s = snap(vec![
            el(1, "Button", "", [10.0, 10.0, 40.0, 40.0], true),
            el(2, "Unknown", "", [50.0, 10.0, 80.0, 40.0], true),
        ]);
        let violations = check_snapshot(&s, "pt-BR", &TranslationDict::default(), &[]);
        let nameless: Vec<_> = violations
            .iter()
            .filter(|v| v.dedup_key.starts_with("nameless_clickable:"))
            .collect();
        assert_eq!(nameless.len(), 1);

        let mut log = FindingLog::new(None);
        for v in check_snapshot(&s, "pt-BR", &TranslationDict::default(), &[]) {
            assert!(log.record(v).unwrap());
        }
        for v in check_snapshot(&s, "pt-BR", &TranslationDict::default(), &[]) {
            assert!(!log.record(v).unwrap());
        }
        assert_eq!(log.count(), 1);

        // Changing an unrelated label on the same logical screen must remain deduplicated.
        let s2 = snap(vec![
            el(1, "Button", "", [10.0, 10.0, 40.0, 40.0], true),
            el(2, "Unknown", "", [50.0, 10.0, 80.0, 40.0], true),
            el(3, "Label", "Updated search count: 5", [10.0, 60.0, 180.0, 80.0], false),
        ]);
        for v in check_snapshot(&s2, "pt-BR", &TranslationDict::default(), &[]) {
            assert!(!log.record(v).unwrap());
        }
        assert_eq!(log.count(), 1);
    }
}
