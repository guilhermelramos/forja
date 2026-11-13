//! `chat.diff.apply_mode`: how file edits proposed by the helper appear in the
//! transcript. The helper writes the files itself; the chat only shows the diff,
//! so the mode changes the presentation and its status label:
//!
//! - `review` (default): collapsed, marked as waiting for review;
//! - `inline`: open in the transcript;
//! - `auto`: collapsed, marked as applied.

/// Presentation of a file edit block.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum DiffMode {
    Review,
    Inline,
    Auto,
}

impl DiffMode {
    /// Parses the setting value. Unknown values fall back to `review`.
    pub fn from_setting(value: Option<&str>) -> Self {
        match value {
            Some("inline") => Self::Inline,
            Some("auto") => Self::Auto,
            _ => Self::Review,
        }
    }

    /// Whether the diff body starts expanded.
    pub fn starts_open(self) -> bool {
        self == Self::Inline
    }

    /// i18n key of the status shown next to the file path, when there is one.
    pub fn status_key(self) -> Option<&'static str> {
        match self {
            Self::Review => Some("chat.view.diff-review"),
            Self::Auto => Some("chat.view.diff-applied"),
            Self::Inline => None,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn setting_values_map_to_modes() {
        assert_eq!(DiffMode::from_setting(Some("review")), DiffMode::Review);
        assert_eq!(DiffMode::from_setting(Some("inline")), DiffMode::Inline);
        assert_eq!(DiffMode::from_setting(Some("auto")), DiffMode::Auto);
        assert_eq!(DiffMode::from_setting(Some("bogus")), DiffMode::Review);
        assert_eq!(DiffMode::from_setting(None), DiffMode::Review);
    }

    #[test]
    fn only_inline_starts_open_and_review_is_marked() {
        assert!(DiffMode::Inline.starts_open());
        assert!(!DiffMode::Review.starts_open());
        assert!(!DiffMode::Auto.starts_open());
        assert_eq!(DiffMode::Inline.status_key(), None);
        assert_eq!(DiffMode::Review.status_key(), Some("chat.view.diff-review"));
        assert_eq!(DiffMode::Auto.status_key(), Some("chat.view.diff-applied"));
    }
}
