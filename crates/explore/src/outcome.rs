//! Exit-code policy: findings are output, not failure.

use crate::runner::ExploreSummary;

/// One completed CLI invocation, kept separate from printing so it can be tested.
#[derive(Debug)]
pub enum Outcome {
    Help,
    Explore(Result<ExploreSummary, String>),
    Replay(Result<(), String>),
    Usage(String),
}

impl Outcome {
    /// `0` when the work ran (even with findings), `1` for a real failure,
    /// and `2` for invalid arguments.
    pub fn exit_code(&self) -> i32 {
        match self {
            Self::Help | Self::Explore(Ok(_)) | Self::Replay(Ok(())) => 0,
            Self::Explore(Err(_)) | Self::Replay(Err(_)) => 1,
            Self::Usage(_) => 2,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn summary(findings: usize) -> ExploreSummary {
        ExploreSummary {
            actions: 10,
            screens: 4,
            elements: 20,
            keybinds: 2,
            findings,
            crashes_or_hangs: 0,
            restarts: 0,
        }
    }

    #[test]
    fn findings_do_not_make_the_exploration_fail() {
        assert_eq!(Outcome::Explore(Ok(summary(36))).exit_code(), 0);
        assert_eq!(Outcome::Explore(Ok(summary(0))).exit_code(), 0);
        assert_eq!(Outcome::Help.exit_code(), 0);
    }

    #[test]
    fn real_errors_keep_non_zero_codes() {
        assert_eq!(
            Outcome::Explore(Err("timed out waiting for pipe".to_owned())).exit_code(),
            1
        );
        assert_eq!(Outcome::Replay(Ok(())).exit_code(), 0);
        assert_eq!(
            Outcome::Replay(Err("1 of 3 regression(s) failed".to_owned())).exit_code(),
            1
        );
        assert_eq!(Outcome::Usage("unknown argument".to_owned()).exit_code(), 2);
    }
}
