//! Passos do primeiro uso. A detecção de ajudantes e a importação do Harbor
//! só entram se o comando existir (e, no caso da importação, se houver
//! algo pendente).

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Step {
    Language,
    Theme,
    Folder,
    Import,
    Helpers,
    Notices,
    Checklist,
}

pub fn steps(include_helpers: bool, include_import: bool) -> Vec<Step> {
    let mut steps = vec![Step::Language, Step::Theme, Step::Folder];
    if include_import {
        steps.push(Step::Import);
    }
    if include_helpers {
        steps.push(Step::Helpers);
    }
    steps.push(Step::Notices);
    steps.push(Step::Checklist);
    steps
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn helpers_step_is_skipped_when_the_command_is_missing() {
        let steps = steps(false, false);
        assert!(!steps.contains(&Step::Helpers));
        assert_eq!(steps.first().copied(), Some(Step::Language));
        assert_eq!(steps.last().copied(), Some(Step::Checklist));
    }

    #[test]
    fn helpers_step_sits_before_notices() {
        let steps = steps(true, false);
        let helpers = steps.iter().position(|step| *step == Step::Helpers).unwrap();
        let notices = steps.iter().position(|step| *step == Step::Notices).unwrap();
        assert!(helpers < notices);
    }

    #[test]
    fn import_step_is_skipped_unless_pending() {
        assert!(!steps(false, false).contains(&Step::Import));
        assert!(steps(false, true).contains(&Step::Import));
    }

    #[test]
    fn import_step_sits_right_after_folder() {
        let steps = steps(true, true);
        let folder = steps.iter().position(|step| *step == Step::Folder).unwrap();
        let import = steps.iter().position(|step| *step == Step::Import).unwrap();
        assert_eq!(import, folder + 1);
    }
}
