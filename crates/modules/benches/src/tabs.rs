//! Terminal tabs of a bench.
//!
//! A bench opens its initial tabs (`forja.toml` or `bench.layout.tabs`) as
//! terminals whose title is `<bench> · <tab>`. When the user leaves a bench,
//! its open tabs are remembered on the record; with `bench.layout.restore` on,
//! returning to the bench reopens the remembered tabs that are no longer open.

use crate::model::BenchRecord;
use forja_kernel::{Args, Ctx};
use serde_json::{json, Map, Value};

const TITLE_SEPARATOR: &str = " \u{00b7} ";

/// Terminal title for a tab of a bench.
pub(crate) fn tab_title(bench_name: &str, tab: &str) -> String {
    format!("{bench_name}{TITLE_SEPARATOR}{tab}")
}

/// Tab id encoded in a terminal title, when the title belongs to the bench.
pub(crate) fn tab_id_from_title(bench_name: &str, title: &str) -> Option<String> {
    let prefix = format!("{bench_name}{TITLE_SEPARATOR}");
    title
        .strip_prefix(&prefix)
        .filter(|tab| !tab.is_empty())
        .map(str::to_owned)
}

/// Tabs remembered for the bench that are not open now, in their original order.
pub(crate) fn missing_tabs(remembered: &[String], open: &[String]) -> Vec<String> {
    remembered
        .iter()
        .filter(|tab| !open.contains(tab))
        .cloned()
        .collect()
}

/// Opens one terminal per tab in the bench folder. Stops at the first failure.
pub(crate) fn open_tabs(cx: &mut Ctx<'_>, record: &BenchRecord, tabs: &[String]) {
    for tab in tabs {
        let mut args = Map::new();
        args.insert("cwd".to_owned(), json!(record.path));
        args.insert(
            "title".to_owned(),
            json!(tab_title(&record.name, tab)),
        );
        if cx
            .run_command("terminal.open", Args::from_map(args))
            .is_err()
        {
            return;
        }
    }
}

/// Tab ids of the terminals open in the bench folder right now.
pub(crate) fn open_tab_ids(cx: &mut Ctx<'_>, record: &BenchRecord) -> Vec<String> {
    let mut args = Map::new();
    args.insert("bench_path".to_owned(), json!(record.path));
    let Ok(value) = cx.run_command("terminal.list", Args::from_map(args)) else {
        return Vec::new();
    };
    value
        .get("terminals")
        .and_then(Value::as_array)
        .map(|terminals| {
            terminals
                .iter()
                .filter_map(|terminal| terminal.get("title").and_then(Value::as_str))
                .filter_map(|title| tab_id_from_title(&record.name, title))
                .collect()
        })
        .unwrap_or_default()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn title_round_trips_to_tab_id() {
        let title = tab_title("feat-login", "helper");
        assert_eq!(
            tab_id_from_title("feat-login", &title).as_deref(),
            Some("helper")
        );
    }

    #[test]
    fn foreign_or_empty_titles_are_not_tabs() {
        assert_eq!(tab_id_from_title("feat-login", "outra · terminal"), None);
        assert_eq!(tab_id_from_title("feat-login", "feat-login"), None);
        assert_eq!(
            tab_id_from_title("feat-login", &format!("feat-login{TITLE_SEPARATOR}")),
            None
        );
    }

    #[test]
    fn only_remembered_tabs_that_are_closed_are_restored() {
        let remembered = vec!["terminal".to_owned(), "helper".to_owned()];
        let open = vec!["helper".to_owned()];
        assert_eq!(
            missing_tabs(&remembered, &open),
            vec!["terminal".to_owned()]
        );
        assert!(missing_tabs(&remembered, &remembered).is_empty());
        assert!(missing_tabs(&[], &open).is_empty());
    }
}
