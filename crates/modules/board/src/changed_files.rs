//! Changed-file counts for board cards, taken from the git status event that
//! the git module already publishes. The board never runs git or reads disk.

use crate::state::Shared;
use forja_contracts::GitStatusChanged;
use forja_kernel::ui_contract::egui;
use forja_kernel::Ctx;

/// Path key used to match an event with a bench card (Windows paths differ in
/// case and separator style between the store and the git event).
fn key(path: &str) -> String {
    path.trim().replace('\\', "/").trim_end_matches('/').to_lowercase()
}

/// Number of changed paths carried by one status event.
pub(crate) fn count_from_event(event: &GitStatusChanged) -> usize {
    event.files.iter().filter(|path| !path.trim().is_empty()).count()
}

/// Stores the count for the bench the event belongs to.
pub(crate) fn apply(state: &Shared, event: &GitStatusChanged) {
    state.set_changed_count(key(&event.bench_path), count_from_event(event));
}

/// Count for a card's bench path; zero when git has not published a status yet.
pub(crate) fn count(state: &Shared, path: &str) -> usize {
    state.changed_count(&key(path))
}

/// Renders nothing for a clean bench, one quiet tag otherwise.
pub(crate) fn render(ui: &mut egui::Ui, cx: &Ctx<'_>, changed: usize) {
    if changed == 0 {
        return;
    }
    forja_ui_kit::tag(ui, &cx.t_args("board.card.changed", &[("count", &changed.to_string())]))
        .on_hover_text(cx.t("board.card.changed-hint"));
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::state::Shared as BoardState;
    use forja_contracts::GitSummary;

    fn event(path: &str, files: &[&str]) -> GitStatusChanged {
        GitStatusChanged {
            bench_path: path.to_owned(),
            summary: GitSummary::default(),
            files: files.iter().map(|file| (*file).to_owned()).collect(),
        }
    }

    #[test]
    fn the_event_count_is_the_number_of_changed_paths() {
        assert_eq!(count_from_event(&event("C:/b", &["a", "b", ""])), 2);
        assert_eq!(count_from_event(&event("C:/b", &[])), 0);
    }

    #[test]
    fn a_status_event_updates_only_its_own_card() {
        let state = BoardState::new();
        apply(&state, &event("C:/bancadas/relatorio", &["src/login.ts"]));
        assert_eq!(count(&state, "c:/bancadas/relatorio/"), 1);
        assert_eq!(count(&state, "C:/bancadas/outro"), 0);
        apply(&state, &event("C:/bancadas/relatorio", &[]));
        assert_eq!(count(&state, "C:/bancadas/relatorio"), 0, "clean after commit");
    }
}
