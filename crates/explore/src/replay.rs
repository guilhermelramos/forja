//! Recorded regression runner (`forja-explore --replay <dir>`).

use crate::client::{call_ui, AppSession, CALL_TIMEOUT};
use crate::snapshot::{Element, Snapshot};
use serde::Deserialize;
use serde_json::{json, Map, Value};
use std::collections::BTreeSet;
use std::path::{Path, PathBuf};

/// One recorded regression file in `playtest/regressions/*.json`.
#[derive(Debug, Clone, Deserialize)]
pub struct RegressionFile {
    pub title: String,
    #[serde(default)]
    pub issue: Option<u64>,
    #[serde(default)]
    pub locale: Option<String>,
    /// Folder copied into the app data dir before it starts. It seeds stored
    /// layouts (a terminal pane tree, for example) that a click sequence
    /// cannot reach deterministically.
    #[serde(default)]
    pub seed: Option<String>,
    /// Test-only helper discovery override passed to the app process.
    #[serde(default)]
    pub test_helpers: Option<String>,
    /// Starts the app with `--demo`, the fixed sample data.
    #[serde(default)]
    pub demo: bool,
    pub steps: Vec<Map<String, Value>>,
    pub expect: Vec<Expectation>,
}

/// Accepts either a single string or an array of strings in JSON.
#[derive(Debug, Clone, Deserialize)]
#[serde(untagged)]
pub enum StringOrList {
    One(String),
    Many(Vec<String>),
}

impl StringOrList {
    fn as_slice(&self) -> Vec<&str> {
        match self {
            Self::One(s) => vec![s.as_str()],
            Self::Many(list) => list.iter().map(String::as_str).collect(),
        }
    }
}

/// Exact accessible identity, including semantic role rather than text alone.
#[derive(Debug, Clone, Deserialize)]
pub struct ElementExpectation {
    pub name: String,
    pub role: String,
}

/// Assertion checked against a specific step (or the final step when `step` is omitted).
#[derive(Debug, Clone, Deserialize)]
pub struct Expectation {
    #[serde(default)]
    pub present_elements: Vec<ElementExpectation>,
    #[serde(default)]
    pub max_primary_buttons: Option<usize>,
    /// Named controls must share a vertical center (within two physical pixels).
    #[serde(default)]
    pub same_row: Option<StringOrList>,
    #[serde(default)]
    pub step: Option<usize>,
    #[serde(default)]
    pub appeared: Option<StringOrList>,
    #[serde(default)]
    pub present: Option<StringOrList>,
    #[serde(default)]
    pub present_contains: Option<StringOrList>,
    /// Text typed into a field, checked against the element's current value.
    #[serde(default)]
    pub value_contains: Option<StringOrList>,
    #[serde(default)]
    pub absent: Option<StringOrList>,
    #[serde(default)]
    pub absent_contains: Option<StringOrList>,
    #[serde(default)]
    pub focused: Option<StringOrList>,
    #[serde(default)]
    pub window_count: Option<usize>,
    #[serde(default)]
    pub window_count_min: Option<usize>,
    #[serde(default)]
    pub window_count_max: Option<usize>,
    #[serde(default)]
    pub no_geometry_issues: Option<bool>,
    #[serde(default)]
    pub no_unnamed_buttons: Option<bool>,
    #[serde(default)]
    pub from_step: Option<usize>,
    #[serde(default)]
    pub max_frames_delta: Option<u64>,
    #[serde(default)]
    pub closed: Option<bool>,
    /// Every needle must appear in the target of a recorded external request.
    #[serde(default)]
    pub externals_contains: Option<StringOrList>,
    /// Every kind must be present among the recorded external requests.
    #[serde(default)]
    pub external_kinds: Option<StringOrList>,
}

/// Runs every `*.json` regression file in `dir` against a fresh clean app instance.
pub fn run_directory(dir: &Path, app_exe: &Path, default_locale: &str) -> Result<(), String> {
    let mut files = collect_json_files(dir)?;
    files.sort();
    if files.is_empty() {
        return Err(format!(
            "no *.json regression files found in {}",
            dir.display()
        ));
    }

    let mut failures = Vec::new();
    let total = files.len();

    for (idx, path) in files.iter().enumerate() {
        let name = path
            .file_name()
            .and_then(|n| n.to_str())
            .unwrap_or("regression.json");
        match run_file(path, app_exe, default_locale, idx) {
            Ok(title) => {
                println!("PASS [{}/{}] {name} — {title}", idx + 1, total);
            }
            Err(err) => {
                eprintln!("FAIL [{}/{}] {name}: {err}", idx + 1, total);
                failures.push(format!("{name}: {err}"));
            }
        }
    }

    if failures.is_empty() {
        println!("regressions: {total}/{total} passed");
        Ok(())
    } else {
        Err(format!(
            "{} of {total} regression(s) failed:\n  - {}",
            failures.len(),
            failures.join("\n  - ")
        ))
    }
}

fn collect_json_files(dir: &Path) -> Result<Vec<PathBuf>, String> {
    let entries =
        std::fs::read_dir(dir).map_err(|e| format!("could not read {}: {e}", dir.display()))?;
    let mut files = Vec::new();
    for entry in entries.flatten() {
        let path = entry.path();
        if path.is_file() && path.extension().and_then(|e| e.to_str()) == Some("json") {
            files.push(path);
        }
    }
    Ok(files)
}

/// Copies a seed folder into the app data dir used by one regression.
fn copy_tree(from: &Path, to: &Path) -> Result<(), String> {
    std::fs::create_dir_all(to)
        .map_err(|error| format!("could not create {}: {error}", to.display()))?;
    let entries = std::fs::read_dir(from)
        .map_err(|error| format!("could not read seed {}: {error}", from.display()))?;
    for entry in entries {
        let entry = entry.map_err(|error| error.to_string())?;
        let target = to.join(entry.file_name());
        let kind = entry.file_type().map_err(|error| error.to_string())?;
        if kind.is_dir() {
            copy_tree(&entry.path(), &target)?;
        } else {
            std::fs::copy(entry.path(), &target).map_err(|error| {
                format!(
                    "could not copy {} to {}: {error}",
                    entry.path().display(),
                    target.display()
                )
            })?;
        }
    }
    Ok(())
}

fn run_file(
    path: &Path,
    app_exe: &Path,
    default_locale: &str,
    index: usize,
) -> Result<String, String> {
    let raw = std::fs::read_to_string(path)
        .map_err(|e| format!("could not read {}: {e}", path.display()))?;
    let spec: RegressionFile = serde_json::from_str(&raw)
        .map_err(|e| format!("invalid regression JSON in {}: {e}", path.display()))?;
    if spec.steps.is_empty() {
        return Err("regression has no steps".to_owned());
    }

    let nanos = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_nanos())
        .unwrap_or(0);
    let temp_root = std::env::temp_dir().join(format!(
        "forja-regression-{}-{}-{nanos}",
        std::process::id(),
        index
    ));
    let pick_dir = temp_root.join("sample");
    let pipe = format!(r"\\.\pipe\forja-reg-{}-{index}-{nanos}", std::process::id());
    let locale = spec.locale.as_deref().unwrap_or(default_locale).to_owned();

    let result = (|| {
        if let Some(seed) = spec.seed.as_deref() {
            let base = path.parent().unwrap_or_else(|| Path::new("."));
            copy_tree(&base.join(seed), &temp_root.join("data"))?;
        }
        let mut session = AppSession::start(
            app_exe.to_path_buf(),
            pipe,
            locale,
            temp_root.clone(),
            pick_dir,
            spec.test_helpers.clone(),
            spec.demo,
        )?;
        let mut snapshots = Vec::with_capacity(spec.steps.len());

        for (step_idx, step) in spec.steps.iter().enumerate() {
            let method = step
                .get("m")
                .and_then(Value::as_str)
                .ok_or_else(|| format!("step {step_idx} missing string field `m`"))?;
            let snap = if method == "app.restart" {
                session
                    .spawn_and_wait()
                    .map_err(|err| format!("step {step_idx} (app.restart) failed: {err}"))?;
                let reply = call_ui(session.pipe(), "ui.snapshot", json!({}), CALL_TIMEOUT)
                    .map_err(|err| {
                        format!("step {step_idx} (app.restart snapshot) failed: {err}")
                    })?;
                Snapshot::from_rpc_result(&reply).map_err(|err| {
                    format!("step {step_idx} (app.restart) invalid snapshot: {err}")
                })?
            } else if method == "ui.open_tab" {
                let target = step
                    .get("target")
                    .and_then(Value::as_str)
                    .map(str::trim)
                    .filter(|s| !s.is_empty())
                    .ok_or_else(|| {
                        format!("step {step_idx} (ui.open_tab) missing string field `target`")
                    })?;
                open_tab(session.pipe(), target)
                    .map_err(|err| format!("step {step_idx} (ui.open_tab) failed: {err}"))?
            } else {
                let mut params = Map::new();
                for (k, v) in step {
                    if k != "m" && k != "view" && k != "allow_error" {
                        params.insert(k.clone(), v.clone());
                    }
                }
                let allow_error = step
                    .get("allow_error")
                    .and_then(Value::as_bool)
                    .unwrap_or(false);
                let reply = call_ui(session.pipe(), method, Value::Object(params), CALL_TIMEOUT);
                if method.starts_with("ui.") {
                    let reply = reply.map_err(|err| format!("step {step_idx} ({method}) failed: {err}"))?;
                    Snapshot::from_rpc_result(&reply).map_err(|err| {
                        format!("step {step_idx} ({method}) invalid snapshot: {err}")
                    })?
                } else {
                    if let (Err(err), false) = (&reply, allow_error) {
                        return Err(format!("step {step_idx} ({method}) failed: {err}"));
                    }
                    let snap_reply =
                        call_ui(session.pipe(), "ui.snapshot", json!({}), CALL_TIMEOUT).map_err(
                            |err| format!("step {step_idx} ({method} snapshot) failed: {err}"),
                        )?;
                    Snapshot::from_rpc_result(&snap_reply).map_err(|err| {
                        format!("step {step_idx} ({method}) invalid snapshot: {err}")
                    })?
                }
            };
            snapshots.push(snap);
        }

        evaluate_expectations(&spec.expect, &snapshots)?;
        session.kill();
        Ok(spec.title)
    })();

    let _ = std::fs::remove_dir_all(&temp_root);
    result
}

const OVERFLOW_PREFIXES: &[&str] = &["mais abas", "more tabs"];
const ADD_TOOL_LABELS: &[&str] = &["Abrir aba de ferramenta", "Open tool tab"];

fn is_overflow_button(name: &str) -> bool {
    let trimmed = name.trim();
    let lower = trimmed.to_lowercase();
    if OVERFLOW_PREFIXES
        .iter()
        .any(|prefix| lower.starts_with(prefix))
    {
        return true;
    }
    trimmed
        .strip_prefix('+')
        .is_some_and(|rest| !rest.is_empty() && rest.chars().all(|ch| ch.is_ascii_digit()))
}

fn is_add_tool_button(name: &str) -> bool {
    let trimmed = name.trim();
    ADD_TOOL_LABELS
        .iter()
        .any(|label| label.eq_ignore_ascii_case(trimmed))
}

/// Opens a slot tab directly when visible on the tab bar, through any
/// overflow menu (`"Mais abas (+N)"` / `"+N"`), or through the `"+"` tool-tab menu.
fn open_tab(pipe: &str, target: &str) -> Result<Snapshot, String> {
    open_tab_with(target, |method, params| {
        call_ui(pipe, method, params, CALL_TIMEOUT).map_err(|err| err.to_string())
    })
}

fn open_tab_with<F>(target: &str, mut call: F) -> Result<Snapshot, String>
where
    F: FnMut(&str, Value) -> Result<Value, String>,
{
    let snap_val = call("ui.snapshot", json!({}))?;
    let initial = Snapshot::from_rpc_result(&snap_val)?;
    // Rail navigation can share a title with a tool tab (Bancada/Bench).
    // A slot-tab request must not activate a rail item instead of its tool.
    let rail_right = initial
        .elements
        .iter()
        .find(|el| el.role == "Button" && ["Ajustes", "Settings"].contains(&el.name.as_str()))
        .map(|el| el.rect[2]);
    if let Some(el) = find_clickable_tab(&initial.elements, target, None)
        .filter(|el| rail_right.is_none_or(|right| el.rect[0] > right))
    {
        let reply = call("ui.click", json!({ "id": el.id }))?;
        return Snapshot::from_rpc_result(&reply);
    }

    let initial_ids: BTreeSet<u64> = initial.elements.iter().map(|el| el.id).collect();
    let overflow_ids: Vec<u64> = initial
        .elements
        .iter()
        .filter(|el| el.clickable && is_overflow_button(&el.name))
        .map(|el| el.id)
        .collect();
    let add_ids: Vec<u64> = initial
        .elements
        .iter()
        .filter(|el| el.clickable && is_add_tool_button(&el.name))
        .map(|el| el.id)
        .collect();

    for menu_id in overflow_ids.into_iter().chain(add_ids) {
        let menu_val = call("ui.click", json!({ "id": menu_id }))?;
        let menu_snap = Snapshot::from_rpc_result(&menu_val)?;
        if let Some(item) = find_clickable_tab(&menu_snap.elements, target, Some(&initial_ids))
            .or_else(|| find_clickable_tab(&menu_snap.elements, target, None))
        {
            let reply = call("ui.click", json!({ "id": item.id }))?;
            return Snapshot::from_rpc_result(&reply);
        }
        let _ = call("ui.key", json!({ "keys": "Escape" }))?;
    }

    Err(format!(
        "tab \"{target}\" not found directly, in overflow menu, or in \"+\" tool menu"
    ))
}

fn find_clickable_tab<'a>(
    elements: &'a [Element],
    target: &str,
    excluded_ids: Option<&BTreeSet<u64>>,
) -> Option<&'a Element> {
    let trimmed = target.trim();
    let lower = trimmed.to_lowercase();
    let allowed =
        |el: &&Element| el.clickable && excluded_ids.is_none_or(|ids| !ids.contains(&el.id));
    elements
        .iter()
        .find(|el| allowed(el) && el.name.trim() == trimmed)
        .or_else(|| {
            elements
                .iter()
                .find(|el| allowed(el) && el.name.trim().to_lowercase() == lower)
        })
}

/// Evaluates all expectations against the recorded step snapshots.
pub fn evaluate_expectations(
    expectations: &[Expectation],
    snapshots: &[Snapshot],
) -> Result<(), String> {
    if snapshots.is_empty() {
        return Err("no snapshots captured".to_owned());
    }
    let last_idx = snapshots.len() - 1;
    for (exp_idx, exp) in expectations.iter().enumerate() {
        let target_idx = exp.step.unwrap_or(last_idx);
        let snap = snapshots.get(target_idx).ok_or_else(|| {
            format!(
                "expectation {exp_idx} targets step {target_idx}, but only {} step(s) ran",
                snapshots.len()
            )
        })?;

        if let Some(appeared) = &exp.appeared {
            for needle in appeared.as_slice() {
                if !snap.changes.appeared.iter().any(|name| name == needle) {
                    return Err(format!(
                        "expectation {exp_idx} (step {target_idx}): expected \"{needle}\" in appeared, got {:?}",
                        snap.changes.appeared
                    ));
                }
            }
        }
        if let Some(present) = &exp.present {
            for needle in present.as_slice() {
                if !snap.elements.iter().any(|el| el.name.trim() == needle) {
                    return Err(format!(
                        "expectation {exp_idx} (step {target_idx}): expected \"{needle}\" to be present"
                    ));
                }
            }
        }
        if let Some(present_contains) = &exp.present_contains {
            for needle in present_contains.as_slice() {
                if !snap.elements.iter().any(|el| el.name.contains(needle)) {
                    return Err(format!(
                        "expectation {exp_idx} (step {target_idx}): expected element containing \"{needle}\""
                    ));
                }
            }
        }
        if let Some(value_contains) = &exp.value_contains {
            for needle in value_contains.as_slice() {
                if !snap.elements.iter().any(|el| el.value.contains(needle)) {
                    return Err(format!(
                        "expectation {exp_idx} (step {target_idx}): expected field value containing \"{needle}\""
                    ));
                }
            }
        }
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
        if let Some(absent) = &exp.absent {
            for needle in absent.as_slice() {
                if snap.elements.iter().any(|el| el.name.trim() == needle) {
                    return Err(format!(
                        "expectation {exp_idx} (step {target_idx}): expected \"{needle}\" to be absent, but it was visible"
                    ));
                }
            }
        }
        if let Some(absent_contains) = &exp.absent_contains {
            for needle in absent_contains.as_slice() {
                if snap.elements.iter().any(|el| el.name.contains(needle)) {
                    return Err(format!(
                        "expectation {exp_idx} (step {target_idx}): expected no element containing \"{needle}\""
                    ));
                }
            }
        }
        if let Some(focused) = &exp.focused {
            for needle in focused.as_slice() {
                if !snap
                    .elements
                    .iter()
                    .any(|element| element.name.trim() == needle && element.is_focused())
                {
                    return Err(format!(
                        "expectation {exp_idx} (step {target_idx}): expected \"{needle}\" to have keyboard focus"
                    ));
                }
            }
        }
        if let Some(expected_count) = exp.window_count {
            let actual = snap.window_count();
            if actual != expected_count {
                return Err(format!(
                    "expectation {exp_idx} (step {target_idx}): expected window_count == {expected_count}, got {actual}"
                ));
            }
        }
        if let Some(min_count) = exp.window_count_min {
            let actual = snap.window_count();
            if actual < min_count {
                return Err(format!(
                    "expectation {exp_idx} (step {target_idx}): expected window_count >= {min_count}, got {actual}"
                ));
            }
        }
        if let Some(max_count) = exp.window_count_max {
            let actual = snap.window_count();
            if actual > max_count {
                return Err(format!(
                    "expectation {exp_idx} (step {target_idx}): expected window_count <= {max_count}, got {actual}"
                ));
            }
        }
        if let Some(limit) = exp.max_primary_buttons {
            let count = snap
                .elements
                .iter()
                .filter(|element| {
                    element.role == "Button" && element.state.iter().any(|state| state == "primary")
                })
                .count();
            if count > limit {
                return Err(format!("expectation {exp_idx} (step {target_idx}): {count} primary buttons exceed {limit}"));
            }
        }
        if let Some(names) = &exp.same_row {
            let mut center: Option<f64> = None;
            for name in names.as_slice() {
                let element = snap
                    .elements
                    .iter()
                    .find(|element| element.name == name)
                    .ok_or_else(|| {
                        format!("expectation {exp_idx}: row control `{name}` is missing")
                    })?;
                let y = (element.rect[1] + element.rect[3]) / 2.0;
                if center.is_some_and(|previous| (previous - y).abs() > 2.0) {
                    return Err(format!(
                        "expectation {exp_idx}: `{name}` is not on the same filter row"
                    ));
                }
                center = Some(y);
            }
        }
        for expected in &exp.present_elements {
            if !snap
                .elements
                .iter()
                .any(|element| element.name == expected.name && element.role == expected.role)
            {
                return Err(format!(
                    "expectation {exp_idx} (step {target_idx}): missing {} named {:?}",
                    expected.role, expected.name
                ));
            }
        }
        if exp.no_unnamed_buttons == Some(true) {
            if let Some(element) = snap
                .elements
                .iter()
                .find(|element| element.role == "Button" && element.name.trim().is_empty())
            {
                return Err(format!(
                    "expectation {exp_idx} (step {target_idx}): button {} has no accessible name",
                    element.id
                ));
            }
        }
        if exp.no_geometry_issues == Some(true) {
            let issues = crate::invariants::geometry_issues(snap);
            if !issues.is_empty() {
                return Err(format!(
                    "expectation {exp_idx} (step {target_idx}): geometry issue(s): {}",
                    issues.join(" | ")
                ));
            }
        }
        if let Some(limit) = exp.max_frames_delta {
            let baseline_idx = match exp.from_step {
                Some(idx) => idx,
                None => target_idx.checked_sub(1).ok_or_else(|| {
                    format!(
                        "expectation {exp_idx} (step {target_idx}): max_frames_delta needs a previous step or from_step"
                    )
                })?,
            };
            let baseline = snapshots.get(baseline_idx).ok_or_else(|| {
                format!(
                    "expectation {exp_idx} targets from_step {baseline_idx}, but only {} step(s) ran",
                    snapshots.len()
                )
            })?;
            let delta = snap.frames.saturating_sub(baseline.frames);
            if delta > limit {
                return Err(format!(
                    "expectation {exp_idx} (step {target_idx}): frame delta {delta} exceeded max_frames_delta {limit} (from step {baseline_idx}: {} -> {})",
                    baseline.frames, snap.frames
                ));
            }
        }
        if let Some(expected_closed) = exp.closed {
            if snap.closed != expected_closed {
                return Err(format!(
                    "expectation {exp_idx} (step {target_idx}): expected closed={expected_closed}, got {}",
                    snap.closed
                ));
            }
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::snapshot::{Changes, Element, WindowSize};

    fn snap_with(names: &[&str], windows: usize, appeared: &[&str]) -> Snapshot {
        let mut elements = Vec::new();
        for i in 0..windows {
            elements.push(Element {
                id: i as u64 + 1,
                role: "Window".into(),
                name: format!("W{i}"),
                value: String::new(),
                rect: [0.0, 0.0, 400.0, 400.0],
                clickable: false,
                editable: false,
                state: vec![],
                window_id: None,
            });
        }
        for (i, n) in names.iter().enumerate() {
            elements.push(Element {
                id: 100 + i as u64,
                role: "Button".into(),
                name: (*n).to_owned(),
                value: String::new(),
                rect: [10.0, 10.0, 90.0, 40.0],
                clickable: true,
                editable: false,
                state: vec![],
                window_id: None,
            });
        }
        Snapshot {
            window: WindowSize {
                width: 1920.0,
                height: 1080.0,
            },
            changes: Changes {
                appeared: appeared.iter().map(|s| (*s).to_owned()).collect(),
                disappeared: vec![],
            },
            view: String::new(),
            elements,
            frames: 0,
            closed: false,
            externals: Vec::new(),
        }
    }

    #[test]
    fn filter_row_contract_rejects_a_second_row_and_missing_controls() {
        let expected: Vec<Expectation> =
            serde_json::from_str(r#"[{"same_row":["All","Local"]}]"#).unwrap();
        let mut snapshot = snap_with(&["All", "Local"], 0, &[]);
        snapshot.elements[1].rect = [100.0, 50.0, 180.0, 80.0];
        assert!(evaluate_expectations(&expected, &[snapshot.clone()]).is_err());
        snapshot.elements[1].rect = [100.0, 10.0, 180.0, 40.0];
        assert!(evaluate_expectations(&expected, &[snapshot]).is_ok());
        assert!(evaluate_expectations(&expected, &[snap_with(&["All"], 0, &[])]).is_err());
    }

    #[test]
    fn primary_limit_counts_the_actual_resolved_button_style() {
        let expected: Vec<Expectation> =
            serde_json::from_str(r#"[{"max_primary_buttons":1}]"#).unwrap();
        let mut snapshot = snap_with(&["Create", "Save"], 0, &[]);
        for element in &mut snapshot.elements {
            element.state.push("primary".into());
        }
        assert!(evaluate_expectations(&expected, &[snapshot.clone()]).is_err());
        snapshot.elements[1].state.clear();
        assert!(evaluate_expectations(&expected, &[snapshot]).is_ok());
    }

    #[test]
    fn element_expectations_require_both_name_and_role() {
        let expected: Vec<Expectation> = serde_json::from_str(
            r#"[{"present_elements":[{"name":"Shared state","role":"Label"}]}]"#,
        )
        .unwrap();
        let mut snap = snap_with(&["Shared state"], 0, &[]);
        assert!(
            evaluate_expectations(&expected, &[snap.clone()]).is_err(),
            "a Button must not satisfy a Label contract"
        );
        snap.elements[0].role = "Label".into();
        assert!(evaluate_expectations(&expected, &[snap]).is_ok());
    }

    #[test]
    fn value_contains_checks_typed_text_not_the_field_label() {
        let expectations: Vec<Expectation> = serde_json::from_str(
            r#"[{"present_contains":"C:\\dev\\projeto","value_contains":"99999999999999999999"}]"#,
        )
        .unwrap();
        let mut snap = snap_with(&["C:\\dev\\projeto"], 0, &[]);
        assert!(
            evaluate_expectations(&expectations, &[snap.clone()]).is_err(),
            "a field without the typed value must not satisfy value_contains"
        );
        snap.elements[0].role = "TextInput".into();
        snap.elements[0].value = "99999999999999999999".into();
        assert!(evaluate_expectations(&expectations, &[snap.clone()]).is_ok());

        snap.elements[0].value = "123".into();
        assert!(evaluate_expectations(&expectations, &[snap]).is_err());
    }

    #[test]
    fn rejects_icon_controls_without_accessible_names() {
        let expectations: Vec<Expectation> =
            serde_json::from_str(r#"[{"no_unnamed_buttons":true}]"#).unwrap();
        assert!(evaluate_expectations(&expectations, &[snap_with(&["Refresh"], 0, &[])]).is_ok());
        assert!(evaluate_expectations(&expectations, &[snap_with(&[""], 0, &[])]).is_err());
    }

    #[test]
    fn copy_tree_brings_nested_seed_files() {
        let nanos = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|duration| duration.as_nanos())
            .unwrap_or(0);
        let root = std::env::temp_dir().join(format!("forja-seed-test-{nanos}"));
        let from = root.join("seed");
        let to = root.join("data");
        std::fs::create_dir_all(from.join("modules")).unwrap();
        std::fs::write(from.join("modules").join("terminal.json"), b"{}").unwrap();
        copy_tree(&from, &to).unwrap();
        assert_eq!(
            std::fs::read_to_string(to.join("modules").join("terminal.json")).unwrap(),
            "{}"
        );
        let _ = std::fs::remove_dir_all(root);
    }

    #[test]
    fn evaluates_step_and_final_expectations() {
        let snapshots = vec![
            snap_with(&["Fechar janela"], 1, &["Fechar janela"]),
            snap_with(&["Projetos"], 0, &[]),
        ];
        let exps: Vec<Expectation> = serde_json::from_str(
            r#"[
                { "step": 0, "window_count": 1, "appeared": "Fechar janela" },
                { "window_count": 0, "absent": ["Fechar janela"], "present": "Projetos" }
            ]"#,
        )
        .unwrap();
        assert!(evaluate_expectations(&exps, &snapshots).is_ok());

        let bad: Vec<Expectation> = serde_json::from_str(r#"[{ "window_count": 1 }]"#).unwrap();
        assert!(evaluate_expectations(&bad, &snapshots).is_err());
    }

    #[test]
    fn evaluates_keyboard_focus_expectations() {
        let mut snapshot = snap_with(&["Ajustes"], 1, &[]);
        snapshot.elements[1].state.push("focused".to_owned());
        let expectations: Vec<Expectation> =
            serde_json::from_str(r#"[{"focused":"Ajustes"}]"#).unwrap();
        assert!(evaluate_expectations(&expectations, &[snapshot.clone()]).is_ok());

        let missing: Vec<Expectation> =
            serde_json::from_str(r#"[{"focused":"Comandos"}]"#).unwrap();
        assert!(evaluate_expectations(&missing, &[snapshot]).is_err());
    }

    #[test]
    fn regression_can_override_helper_detection_for_its_app() {
        let spec: RegressionFile = serde_json::from_str(
            r#"{"title":"no helper","test_helpers":"none","steps":[{"m":"ui.wait"}],"expect":[]}"#,
        )
        .unwrap();

        assert_eq!(spec.test_helpers.as_deref(), Some("none"));
    }

    fn rpc_snapshot(elements: Vec<Element>) -> Value {
        serde_json::to_value(Snapshot {
            window: WindowSize {
                width: 1280.0,
                height: 800.0,
            },
            changes: Changes::default(),
            view: String::new(),
            elements,
            frames: 0,
            closed: false,
            externals: Vec::new(),
        })
        .unwrap()
    }

    fn tab_el(id: u64, name: &str, clickable: bool) -> Element {
        Element {
            id,
            role: "Button".into(),
            name: name.into(),
            value: String::new(),
            rect: [10.0, 10.0, 90.0, 40.0],
            clickable,
            editable: false,
            state: vec![],
            window_id: None,
        }
    }

    #[test]
    fn open_tab_clicks_directly_when_visible_and_clickable() {
        let mut log = Vec::new();
        let result = open_tab_with("Bancada", |method, params| {
            log.push((method.to_owned(), params.clone()));
            match method {
                "ui.snapshot" => Ok(rpc_snapshot(vec![
                    tab_el(1, "Mais abas", true),
                    tab_el(2, "Terminal", true),
                    tab_el(3, "Bancada", true),
                    tab_el(4, "Nova bancada", true),
                ])),
                "ui.click" => {
                    assert_eq!(params["id"], 3);
                    Ok(rpc_snapshot(vec![tab_el(3, "Bancada", true)]))
                }
                other => panic!("unexpected method {other}"),
            }
        })
        .unwrap();

        assert_eq!(log.len(), 2);
        assert_eq!(result.elements[0].name, "Bancada");
    }

    #[test]
    fn open_tab_falls_back_through_more_tabs_menus_in_either_locale() {
        let mut log = Vec::new();
        let result = open_tab_with("Ajudante (chat)", |method, params| {
            log.push((method.to_owned(), params.clone()));
            match method {
                "ui.snapshot" => Ok(rpc_snapshot(vec![
                    tab_el(10, "Mais abas", true),
                    tab_el(11, "More tabs", true),
                    // Non-clickable truncated label must not block overflow lookup.
                    tab_el(12, "Ajudante (chat)", false),
                ])),
                "ui.click" => match params["id"].as_u64() {
                    Some(10) => Ok(rpc_snapshot(vec![
                        tab_el(10, "Mais abas", true),
                        tab_el(20, "Projetos", true),
                    ])),
                    Some(11) => Ok(rpc_snapshot(vec![
                        tab_el(11, "More tabs", true),
                        tab_el(30, "Ajudante (chat)", true),
                    ])),
                    Some(30) => Ok(rpc_snapshot(vec![tab_el(30, "Ajudante (chat)", true)])),
                    other => panic!("unexpected click id {other:?}"),
                },
                "ui.key" => {
                    assert_eq!(params["keys"], "Escape");
                    Ok(rpc_snapshot(vec![
                        tab_el(10, "Mais abas", true),
                        tab_el(11, "More tabs", true),
                    ]))
                }
                other => panic!("unexpected method {other}"),
            }
        })
        .unwrap();

        assert_eq!(
            log.iter().map(|(m, _)| m.as_str()).collect::<Vec<_>>(),
            ["ui.snapshot", "ui.click", "ui.key", "ui.click", "ui.click"]
        );
        assert_eq!(result.elements[0].id, 30);
    }

    #[test]
    fn open_tab_errors_after_checking_all_overflow_menus() {
        let mut keys_sent = 0;
        let err = open_tab_with("Inexistente", |method, params| match method {
            "ui.snapshot" => Ok(rpc_snapshot(vec![tab_el(1, "Mais abas", true)])),
            "ui.click" => {
                assert_eq!(params["id"], 1);
                Ok(rpc_snapshot(vec![tab_el(2, "Terminal", true)]))
            }
            "ui.key" => {
                keys_sent += 1;
                Ok(rpc_snapshot(vec![tab_el(1, "Mais abas", true)]))
            }
            other => panic!("unexpected method {other}"),
        })
        .unwrap_err();

        assert_eq!(keys_sent, 1);
        assert!(err.contains("Inexistente"));
    }

    #[test]
    fn open_tab_opens_tool_tab_via_plus_button_and_counts_overflow() {
        let mut log = Vec::new();
        let result = open_tab_with("Navegador", |method, params| {
            log.push((method.to_owned(), params.clone()));
            match method {
                "ui.snapshot" => Ok(rpc_snapshot(vec![
                    tab_el(10, "Mais abas (+2)", true),
                    tab_el(11, "Abrir aba de ferramenta", true),
                ])),
                "ui.click" => match params["id"].as_u64() {
                    Some(10) => Ok(rpc_snapshot(vec![tab_el(20, "Editor", true)])),
                    Some(11) => Ok(rpc_snapshot(vec![
                        tab_el(30, "Navegador", true),
                        tab_el(31, "Bancada", true),
                    ])),
                    Some(30) => Ok(rpc_snapshot(vec![tab_el(30, "Navegador", true)])),
                    other => panic!("unexpected click {other:?}"),
                },
                "ui.key" => Ok(rpc_snapshot(vec![
                    tab_el(10, "Mais abas (+2)", true),
                    tab_el(11, "Abrir aba de ferramenta", true),
                ])),
                other => panic!("unexpected {other}"),
            }
        })
        .unwrap();

        assert_eq!(result.elements[0].name, "Navegador");
    }

    #[test]
    fn evaluates_max_frames_delta_expectation() {
        let mut before = snap_with(&["Terminal"], 0, &[]);
        before.frames = 10;
        let mut ok_after = snap_with(&["Terminal"], 0, &[]);
        ok_after.frames = 18;
        let mut busy_after = snap_with(&["Terminal"], 0, &[]);
        busy_after.frames = 310;

        let exps: Vec<Expectation> = serde_json::from_str(r#"[{"max_frames_delta": 30}]"#).unwrap();
        assert!(evaluate_expectations(&exps, &[before.clone(), ok_after]).is_ok());
        assert!(evaluate_expectations(&exps, &[before.clone(), busy_after.clone()]).is_err());

        let explicit: Vec<Expectation> =
            serde_json::from_str(r#"[{"step": 2, "from_step": 0, "max_frames_delta": 30}]"#)
                .unwrap();
        let mut middle = snap_with(&["Terminal"], 0, &[]);
        middle.frames = 20;
        assert!(evaluate_expectations(&explicit, &[before, middle, busy_after]).is_err());
    }

    #[test]
    fn externals_expectations_check_recorded_requests() {
        let expectations: Vec<Expectation> = serde_json::from_str(
            r#"[{"externals_contains":"https://","external_kinds":["open","copy"]}]"#,
        )
        .unwrap();
        let mut snapshot = snap_with(&[], 0, &[]);
        assert!(evaluate_expectations(&expectations, &[snapshot.clone()]).is_err());

        snapshot.externals.push(crate::snapshot::ExternalRequest {
            kind: "open".to_owned(),
            target: "https://aider.chat".to_owned(),
        });
        assert!(evaluate_expectations(&expectations, &[snapshot.clone()]).is_err());
        snapshot.externals.push(crate::snapshot::ExternalRequest {
            kind: "copy".to_owned(),
            target: "npm install -g @openai/codex".to_owned(),
        });
        assert!(evaluate_expectations(&expectations, &[snapshot]).is_ok());
    }

    #[test]
    fn evaluates_closed_expectation() {
        let mut open_snap = snap_with(&["Trabalho em andamento"], 1, &[]);
        open_snap.closed = false;
        let mut closed_snap = snap_with(&[], 0, &[]);
        closed_snap.closed = true;

        let stay_open: Vec<Expectation> =
            serde_json::from_str(r#"[{"step": 0, "closed": false}]"#).unwrap();
        let did_close: Vec<Expectation> =
            serde_json::from_str(r#"[{"step": 1, "closed": true}]"#).unwrap();
        assert!(
            evaluate_expectations(&stay_open, &[open_snap.clone(), closed_snap.clone()]).is_ok()
        );
        assert!(evaluate_expectations(&did_close, &[open_snap, closed_snap]).is_ok());
    }
}
