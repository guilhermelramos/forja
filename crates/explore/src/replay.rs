//! Recorded regression runner (`forja-explore --replay <dir>`).

use crate::client::{call_ui, AppSession, CALL_TIMEOUT};
use crate::snapshot::Snapshot;
use serde::Deserialize;
use serde_json::{Map, Value};
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

/// Assertion checked against a specific step (or the final step when `step` is omitted).
#[derive(Debug, Clone, Deserialize)]
pub struct Expectation {
    #[serde(default)]
    pub step: Option<usize>,
    #[serde(default)]
    pub appeared: Option<StringOrList>,
    #[serde(default)]
    pub present: Option<StringOrList>,
    #[serde(default)]
    pub present_contains: Option<StringOrList>,
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
    pub from_step: Option<usize>,
    #[serde(default)]
    pub max_frames_delta: Option<u64>,
}

/// Runs every `*.json` regression file in `dir` against a fresh clean app instance.
pub fn run_directory(
    dir: &Path,
    app_exe: &Path,
    default_locale: &str,
) -> Result<(), String> {
    let mut files = collect_json_files(dir)?;
    files.sort();
    if files.is_empty() {
        return Err(format!("no *.json regression files found in {}", dir.display()));
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
    let entries = std::fs::read_dir(dir)
        .map_err(|e| format!("could not read {}: {e}", dir.display()))?;
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
        )?;
        let mut snapshots = Vec::with_capacity(spec.steps.len());

        for (step_idx, step) in spec.steps.iter().enumerate() {
            let method = step
                .get("m")
                .and_then(Value::as_str)
                .ok_or_else(|| format!("step {step_idx} missing string field `m`"))?;
            let mut params = Map::new();
            for (k, v) in step {
                if k != "m" && k != "view" {
                    params.insert(k.clone(), v.clone());
                }
            }
            let reply = call_ui(session.pipe(), method, Value::Object(params), CALL_TIMEOUT)
                .map_err(|err| format!("step {step_idx} ({method}) failed: {err}"))?;
            let snap = Snapshot::from_rpc_result(&reply)
                .map_err(|err| format!("step {step_idx} ({method}) invalid snapshot: {err}"))?;
            snapshots.push(snap);
        }

        evaluate_expectations(&spec.expect, &snapshots)?;
        session.kill();
        Ok(spec.title)
    })();

    let _ = std::fs::remove_dir_all(&temp_root);
    result
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
                rect: [10.0, 10.0, 90.0, 40.0],
                clickable: true,
                editable: false,
                state: vec![],
                window_id: None,
            });
        }
        Snapshot {
            window: WindowSize { width: 1920.0, height: 1080.0 },
            changes: Changes {
                appeared: appeared.iter().map(|s| (*s).to_owned()).collect(),
                disappeared: vec![],
            },
            view: String::new(),
            elements,
            frames: 0,
        }
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

    #[test]
    fn evaluates_max_frames_delta_expectation() {
        let mut before = snap_with(&["Terminal"], 0, &[]);
        before.frames = 10;
        let mut ok_after = snap_with(&["Terminal"], 0, &[]);
        ok_after.frames = 18;
        let mut busy_after = snap_with(&["Terminal"], 0, &[]);
        busy_after.frames = 310;

        let exps: Vec<Expectation> =
            serde_json::from_str(r#"[{"max_frames_delta": 30}]"#).unwrap();
        assert!(evaluate_expectations(&exps, &[before.clone(), ok_after]).is_ok());
        assert!(evaluate_expectations(&exps, &[before.clone(), busy_after.clone()]).is_err());

        let explicit: Vec<Expectation> =
            serde_json::from_str(r#"[{"step": 2, "from_step": 0, "max_frames_delta": 30}]"#)
                .unwrap();
        let mut middle = snap_with(&["Terminal"], 0, &[]);
        middle.frames = 20;
        assert!(evaluate_expectations(&explicit, &[before, middle, busy_after]).is_err());
    }
}
