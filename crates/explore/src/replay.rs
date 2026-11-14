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
    pub window_count: Option<usize>,
    #[serde(default)]
    pub window_count_min: Option<usize>,
    #[serde(default)]
    pub window_count_max: Option<usize>,
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
        let mut session = AppSession::start(
            app_exe.to_path_buf(),
            pipe,
            locale,
            temp_root.clone(),
            pick_dir,
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
        }
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
}
