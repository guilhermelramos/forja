//! `usage.spend`: the contract the Muster uses to read spend per bench.

use crate::model::{UsagePeriod, UsageSlice, UsageTotals};
use crate::state::Shared;
use crate::view::ensure_refresh;
use forja_kernel::{Args, Ctx};
use serde_json::{json, Value};

pub fn query(cx: &mut Ctx<'_>, shared: &Shared, args: &Args) -> Value {
    ensure_refresh(cx, shared);
    let summary = shared.summary();
    let period = UsagePeriod::parse(args.str("period").unwrap_or("today"));
    let project = args
        .str("project")
        .map(str::trim)
        .filter(|path| !path.is_empty());
    report(
        period,
        &summary.slice(period),
        project,
        shared.scanned_at_ms(),
        shared.with(|state| state.busy).unwrap_or(false),
    )
}

pub fn report(
    period: UsagePeriod,
    slice: &UsageSlice,
    project: Option<&str>,
    scanned_at_ms: Option<i64>,
    pending: bool,
) -> Value {
    let (selected, matches) = match project {
        Some(project) => totals_for_project(slice, project),
        None => (slice.totals, slice.by_project.len()),
    };
    let mut projects: Vec<Value> = slice
        .by_project
        .iter()
        .map(|(path, totals)| {
            json!({
                "project": path,
                "usd": totals.cost_usd,
                "tokens": totals.total_tokens,
                "unpriced_tokens": totals.unpriced_tokens,
            })
        })
        .collect();
    projects.sort_by(|left, right| {
        let left = left.get("usd").and_then(Value::as_f64).unwrap_or(0.0);
        let right = right.get("usd").and_then(Value::as_f64).unwrap_or(0.0);
        right
            .partial_cmp(&left)
            .unwrap_or(std::cmp::Ordering::Equal)
    });
    json!({
        "period": period.id(),
        "project": project,
        "matched_projects": matches,
        "usd": selected.cost_usd,
        "tokens": selected.total_tokens,
        "unpriced_tokens": selected.unpriced_tokens,
        "priced": selected.unpriced_tokens == 0,
        "projects": projects,
        "scanned_at_ms": scanned_at_ms,
        "pending": pending,
    })
}

/// Sums every log project inside `bench`, so a helper that ran in a subfolder
/// still belongs to the bench. Paths are compared like Windows paths.
pub fn totals_for_project(slice: &UsageSlice, bench: &str) -> (UsageTotals, usize) {
    let wanted = normalize_path(bench);
    let mut totals = UsageTotals::default();
    let mut matches = 0;
    if wanted.is_empty() {
        return (totals, matches);
    }
    let prefix = format!("{wanted}/");
    for (path, value) in &slice.by_project {
        let candidate = normalize_path(path);
        if candidate == wanted || candidate.starts_with(&prefix) {
            totals.add(value);
            matches += 1;
        }
    }
    (totals, matches)
}

fn normalize_path(path: &str) -> String {
    path.trim()
        .trim_end_matches(['/', '\\'])
        .replace('\\', "/")
        .to_ascii_lowercase()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn totals(cost: f64, tokens: u64, unpriced: u64) -> UsageTotals {
        UsageTotals {
            total_tokens: tokens,
            cost_usd: cost,
            unpriced_tokens: unpriced,
            ..UsageTotals::default()
        }
    }

    fn slice() -> UsageSlice {
        let mut slice = UsageSlice::default();
        slice
            .by_project
            .insert("C:/work/forja".into(), totals(1.5, 100, 0));
        slice
            .by_project
            .insert("C:/work/forja/sub".into(), totals(0.5, 50, 20));
        slice
            .by_project
            .insert("C:/work/other".into(), totals(2.0, 200, 0));
        slice.totals = totals(4.0, 350, 20);
        slice
    }

    #[test]
    fn bench_matching_includes_subfolders_and_ignores_separators() {
        let (totals, matches) = totals_for_project(&slice(), "c:\\work\\FORJA\\");
        assert_eq!(matches, 2);
        assert!((totals.cost_usd - 2.0).abs() < 1e-9);
        assert_eq!(totals.total_tokens, 150);
        assert_eq!(totals.unpriced_tokens, 20);
    }

    #[test]
    fn report_keeps_the_global_total_when_no_bench_is_given() {
        let value = report(UsagePeriod::All, &slice(), None, Some(123), false);
        assert_eq!(value["period"], "all");
        assert!((value["usd"].as_f64().unwrap() - 4.0).abs() < 1e-9);
        assert_eq!(value["matched_projects"], 3);
        assert_eq!(value["pending"], false);
        assert_eq!(value["scanned_at_ms"], 123);
    }

    #[test]
    fn report_marks_unpriced_tokens_instead_of_guessing() {
        let value = report(
            UsagePeriod::Today,
            &slice(),
            Some("C:/work/forja"),
            None,
            true,
        );
        assert_eq!(value["priced"], false);
        assert_eq!(value["unpriced_tokens"], 20);
        assert_eq!(value["matched_projects"], 2);
        assert_eq!(value["pending"], true);
    }

    #[test]
    fn empty_project_returns_zero_matches() {
        let (totals, matches) = totals_for_project(&slice(), "");
        assert_eq!(matches, 0);
        assert!(totals.is_empty());
    }
}
