//! Assertions on external requests a test run recorded instead of performing.
//!
//! Split out of the already large `replay.rs`; the messages are unchanged.

use crate::replay::Expectation;
use crate::snapshot::Snapshot;

/// Checks `externals_contains` and `external_kinds` for one expectation.
pub(crate) fn check_external_expectations(
    exp: &Expectation,
    snap: &Snapshot,
    exp_idx: usize,
    target_idx: usize,
) -> Result<(), String> {
    if let Some(externals_contains) = &exp.externals_contains {
        for needle in externals_contains.as_slice() {
            if !snap
                .externals
                .iter()
                .any(|request| request.target.contains(needle))
            {
                return Err(format!(
                    "expectation {exp_idx} (step {target_idx}): expected a recorded external containing \"{needle}\", got {:?}",
                    snap.externals
                ));
            }
        }
    }
    if let Some(kinds) = &exp.external_kinds {
        for kind in kinds.as_slice() {
            if !snap.externals.iter().any(|request| request.kind == kind) {
                return Err(format!(
                    "expectation {exp_idx} (step {target_idx}): expected a recorded external of kind \"{kind}\", got {:?}",
                    snap.externals
                ));
            }
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::snapshot::ExternalRequest;

    fn snapshot_with(externals: Vec<ExternalRequest>) -> Snapshot {
        Snapshot {
            externals,
            ..Snapshot::default()
        }
    }

    #[test]
    fn checks_recorded_external_targets_and_kinds() {
        let expectations: Vec<Expectation> = serde_json::from_str(
            r#"[{"externals_contains":"https://","external_kinds":["open","copy"]}]"#,
        )
        .unwrap();
        let exp = &expectations[0];

        assert!(check_external_expectations(exp, &snapshot_with(Vec::new()), 0, 0).is_err());

        let open = snapshot_with(vec![ExternalRequest {
            kind: "open".to_owned(),
            target: "https://aider.chat".to_owned(),
        }]);
        assert!(check_external_expectations(exp, &open, 0, 0).is_err());

        let both = snapshot_with(vec![
            ExternalRequest {
                kind: "open".to_owned(),
                target: "https://aider.chat".to_owned(),
            },
            ExternalRequest {
                kind: "copy".to_owned(),
                target: "npm install -g @openai/codex".to_owned(),
            },
        ]);
        assert!(check_external_expectations(exp, &both, 0, 0).is_ok());
    }
}
