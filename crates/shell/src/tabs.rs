//! Bench tab order: which tabs are visible, what was closed and what reopens.
//!
//! Bench views are contributions and never disappear from the registry, so
//! closing a tab hides its index. `tab.close` and `tab.close_force` share the
//! same order; the difference is only which surfaces deliver the chord.

use std::collections::BTreeSet;

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct TabSet {
    hidden: BTreeSet<usize>,
    closed: Vec<usize>,
}

impl TabSet {
    pub fn is_visible(&self, index: usize, total: usize) -> bool {
        index < total && !self.hidden.contains(&index)
    }

    /// Visible indexes, in contribution order.
    pub fn visible(&self, total: usize) -> Vec<usize> {
        (0..total).filter(|index| !self.hidden.contains(index)).collect()
    }

    /// Picks a valid active index after the visible set changed.
    pub fn clamp(&self, active: usize, total: usize) -> Option<usize> {
        if self.is_visible(active, total) {
            return Some(active);
        }
        self.visible(total).first().copied()
    }

    pub fn next(&self, active: usize, total: usize) -> Option<usize> {
        let visible = self.visible(total);
        if visible.is_empty() {
            return None;
        }
        let position = visible.iter().position(|index| *index == active).unwrap_or(0);
        Some(visible[(position + 1) % visible.len()])
    }

    pub fn previous(&self, active: usize, total: usize) -> Option<usize> {
        let visible = self.visible(total);
        if visible.is_empty() {
            return None;
        }
        let position = visible.iter().position(|index| *index == active).unwrap_or(0);
        Some(visible[(position + visible.len() - 1) % visible.len()])
    }

    /// One-based tab number, matching `Ctrl+1`..`Ctrl+0` (where `10` is zero).
    pub fn switch(&self, number: usize, total: usize) -> Option<usize> {
        if number == 0 {
            return None;
        }
        self.visible(total).get(number - 1).copied()
    }

    /// Hides `index` and returns the next active index. Closing the last tab
    /// leaves no active index; the central area shows its empty state.
    pub fn close(&mut self, index: usize, total: usize) -> Option<usize> {
        if !self.is_visible(index, total) {
            return self.clamp(index, total);
        }
        self.hidden.insert(index);
        self.closed.retain(|closed| *closed != index);
        self.closed.push(index);
        let visible = self.visible(total);
        if visible.is_empty() {
            return None;
        }
        Some(
            visible
                .iter()
                .copied()
                .find(|candidate| *candidate > index)
                .unwrap_or_else(|| *visible.last().expect("visible")),
        )
    }

    /// Makes `index` visible again, even if it had been closed.
    pub fn show(&mut self, index: usize, total: usize) -> bool {
        if index >= total {
            return false;
        }
        self.closed.retain(|closed| *closed != index);
        self.hidden.remove(&index);
        true
    }

    /// Shows the most recently closed tab again.
    pub fn reopen(&mut self, total: usize) -> Option<usize> {
        while let Some(index) = self.closed.pop() {
            if index < total && self.hidden.remove(&index) {
                return Some(index);
            }
        }
        None
    }

    pub fn closed_indexes(&self) -> &[usize] {
        &self.closed
    }

    pub fn was_closed(&self, index: usize) -> bool {
        self.closed.contains(&index)
    }

    pub fn from_hidden_and_closed(
        hidden_indexes: impl IntoIterator<Item = usize>,
        closed_indexes: impl IntoIterator<Item = usize>,
        total: usize,
    ) -> Self {
        let mut hidden = BTreeSet::new();
        for index in hidden_indexes {
            if index < total {
                hidden.insert(index);
            }
        }
        let mut closed = Vec::new();
        for index in closed_indexes {
            if index < total {
                hidden.insert(index);
                if !closed.contains(&index) {
                    closed.push(index);
                }
            }
        }
        Self { hidden, closed }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn cycling_skips_closed_tabs() {
        let mut tabs = TabSet::default();
        assert_eq!(tabs.next(1, 3), Some(2));
        assert_eq!(tabs.previous(0, 3), Some(2));
        tabs.close(1, 3);
        assert_eq!(tabs.next(0, 3), Some(2));
        assert_eq!(tabs.previous(2, 3), Some(0));
    }

    #[test]
    fn switching_accepts_one_based_numbers_and_zero_is_ten() {
        let mut tabs = TabSet::default();
        assert_eq!(tabs.switch(1, 3), Some(0));
        assert_eq!(tabs.switch(2, 3), Some(1));
        assert_eq!(tabs.switch(10, 3), None);
        tabs.close(0, 3);
        assert_eq!(tabs.switch(1, 3), Some(1));
        assert_eq!(tabs.switch(10, 12), Some(10));
    }

    #[test]
    fn close_picks_the_next_tab_and_reopen_restores_the_last_one() {
        let mut tabs = TabSet::default();
        assert_eq!(tabs.close(1, 4), Some(2));
        assert_eq!(tabs.close(2, 4), Some(3));
        assert_eq!(tabs.reopen(4), Some(2));
        assert_eq!(tabs.reopen(4), Some(1));
        assert_eq!(tabs.reopen(4), None);
        assert_eq!(tabs.visible(4), vec![0, 1, 2, 3]);
    }

    #[test]
    fn show_reopens_a_closed_tab_and_ignores_missing_indexes() {
        let mut tabs = TabSet::default();
        tabs.close(1, 3);
        assert!(!tabs.is_visible(1, 3));
        assert!(tabs.show(1, 3));
        assert!(tabs.is_visible(1, 3));
        assert!(!tabs.show(5, 3));
    }

    #[test]
    fn closing_every_tab_leaves_no_active_index() {
        let mut tabs = TabSet::default();
        assert_eq!(tabs.close(0, 1), None);
        assert_eq!(tabs.clamp(0, 1), None);
        assert_eq!(tabs.next(0, 1), None);
        assert_eq!(tabs.visible(1), Vec::<usize>::new());
        assert_eq!(tabs.reopen(1), Some(0));
        assert_eq!(tabs.clamp(0, 1), Some(0));
    }
}
