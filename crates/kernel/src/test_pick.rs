//! Answers for native file pickers during automated play tests.
//!
//! Native Windows dialogs live outside egui, so a test driver cannot click
//! them. When `FORJA_TEST_PICK_DIR`, `FORJA_TEST_PICK_FILE` or
//! `FORJA_TEST_SAVE_DIR` is set, the matching picker returns that answer
//! without opening a window. The value `cancel` answers as if the user closed
//! the dialog. Outside tests the variables are unset and pickers behave
//! normally: callers do `if let Some(answer) = test_pick::folder() { .. }`.

use std::path::PathBuf;

fn answer(var: &str) -> Option<Option<PathBuf>> {
    let value = std::env::var_os(var)?;
    if value.is_empty() {
        return None;
    }
    if value == "cancel" {
        return Some(None);
    }
    Some(Some(PathBuf::from(value)))
}

/// Answer for a "pick folder" dialog, when a test set one.
pub fn folder() -> Option<Option<PathBuf>> {
    answer("FORJA_TEST_PICK_DIR")
}

/// Answer for a "pick file" dialog, when a test set one.
pub fn file() -> Option<Option<PathBuf>> {
    answer("FORJA_TEST_PICK_FILE")
}

/// Answer for a "save file" dialog: `<FORJA_TEST_SAVE_DIR>/<name>`.
pub fn save(name: &str) -> Option<Option<PathBuf>> {
    answer("FORJA_TEST_SAVE_DIR").map(|dir| dir.map(|dir| dir.join(name)))
}

#[cfg(test)]
mod tests {
    #[test]
    fn unset_means_no_override() {
        assert_eq!(super::answer("FORJA_TEST_PICK_NEVER_SET_IN_TESTS"), None);
    }
}
