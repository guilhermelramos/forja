//! The base branch declared by the active bench, read from the bench metadata
//! that the benches module already publishes. No git call here.

use crate::state::Shared;
use forja_kernel::{Args, Ctx};
use serde_json::Value;

/// Sets the declared bench base as the current base when the form has none,
/// and returns the base the comparison must start from.
pub(crate) fn resolve_for_refresh(
    cx: &mut Ctx<'_>,
    shared: &Shared,
    bench: &str,
) -> Option<String> {
    let bench_base = bench_base_branch(cx, bench);
    shared
        .with(|state| {
            let chosen = if state.base.trim().is_empty() {
                bench_base.clone().unwrap_or_default()
            } else {
                state.base.clone()
            };
            if state.base.trim().is_empty() {
                state.base = chosen.clone();
            }
            (!chosen.trim().is_empty()).then_some(chosen)
        })
        .flatten()
}

/// Reads the declared base of `bench` from `bench.list_cards`.
pub(crate) fn bench_base_branch(cx: &mut Ctx<'_>, bench: &str) -> Option<String> {
    let value = cx.run_command("bench.list_cards", Args::new()).ok()?;
    base_from_cards(&value, bench)
}

/// Picks the card matching `bench` and returns its non-empty `base_branch`.
pub(crate) fn base_from_cards(value: &Value, bench: &str) -> Option<String> {
    let wanted = normalise(bench);
    value
        .get("benches")?
        .as_array()?
        .iter()
        .filter(|card| {
            card.get("bench")
                .and_then(|bench| bench.get("path"))
                .and_then(Value::as_str)
                .is_some_and(|path| normalise(path) == wanted)
        })
        .find_map(|card| card.get("base_branch").and_then(Value::as_str))
        .map(str::trim)
        .filter(|base| !base.is_empty())
        .map(str::to_owned)
}

fn normalise(path: &str) -> String {
    path.trim().replace('\\', "/").trim_end_matches('/').to_lowercase()
}

/// Uses the bench's declared base when there is one; only then asks the caller
/// for the git-detected fallback (which reads git and must stay in a task).
pub(crate) fn prefer<F: FnOnce() -> String>(bench_base: Option<&str>, fallback: F) -> String {
    match bench_base {
        Some(base) if !base.trim().is_empty() => base.trim().to_owned(),
        _ => fallback(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn a_card_without_a_base_does_not_invent_one() {
        let cards = json!({ "benches": [{
            "bench": { "path": "C:/dados/bancadas/relatorio" },
            "base_branch": null
        }] });
        assert_eq!(base_from_cards(&cards, "C:/dados/bancadas/relatorio"), None);
    }

    #[test]
    fn the_declared_base_replaces_the_git_detected_fallback() {
        assert_eq!(
            prefer(Some(" trunk "), || panic!("git must not be read")),
            "trunk"
        );
        assert_eq!(prefer(None, || "main".to_owned()), "main");
        assert_eq!(prefer(Some("   "), || "main".to_owned()), "main");
    }

    #[test]
    fn the_declared_base_of_the_matching_bench_wins() {
        let cards = json!({ "benches": [
            { "bench": { "path": "c:/outra" }, "base_branch": "develop" },
            { "bench": { "path": "C:/dados/bancadas/relatorio/" }, "base_branch": "  trunk  " }
        ] });
        assert_eq!(
            base_from_cards(&cards, "c:/dados/bancadas/relatorio").as_deref(),
            Some("trunk"),
            "paths compare case-insensitively and ignore separators"
        );
    }
}
