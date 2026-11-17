//! Thirty days of sample usage for `forja-app --demo`.
//!
//! The module reads transcripts from `CLAUDE_CONFIG_DIR`; the shell sets that
//! variable to this demo folder, so nothing is written next to real sessions.
//! Models and projects are invented, and every line carries its own cost, so the
//! numbers show up without depending on the price table.

use std::path::Path;

/// One sample session: how many days ago it ran, its model and its lines.
struct Session {
    days_ago: i64,
    project: &'static str,
    model: &'static str,
    /// Cost of each assistant turn, in dollars.
    turns: &'static [f64],
}

const SESSIONS: [Session; 10] = [
    Session { days_ago: 0, project: "oficina", model: "demo-light", turns: &[0.12, 0.18] },
    Session { days_ago: 2, project: "estudio", model: "demo-wide", turns: &[0.42, 0.31, 0.27] },
    Session { days_ago: 4, project: "oficina", model: "demo-light", turns: &[0.09, 0.22] },
    Session { days_ago: 7, project: "oficina", model: "demo-wide", turns: &[0.64] },
    Session { days_ago: 10, project: "estudio", model: "demo-light", turns: &[0.15, 0.11, 0.19] },
    Session { days_ago: 13, project: "estudio", model: "demo-wide", turns: &[0.38, 0.26] },
    Session { days_ago: 16, project: "oficina", model: "demo-light", turns: &[0.21] },
    Session { days_ago: 20, project: "oficina", model: "demo-wide", turns: &[0.55, 0.33] },
    Session { days_ago: 25, project: "estudio", model: "demo-light", turns: &[0.14, 0.16] },
    Session { days_ago: 29, project: "oficina", model: "demo-light", turns: &[0.08, 0.24, 0.12] },
];

/// Writes the sample transcripts under `<root>/projects/<slug>/`.
pub fn demo_seed(root: &Path) -> Result<(), String> {
    let now = chrono::Utc::now();
    for (index, session) in SESSIONS.iter().enumerate() {
        let directory = root.join("projects").join(session.project);
        std::fs::create_dir_all(&directory)
            .map_err(|error| format!("demo: could not create {}: {error}", directory.display()))?;
        let day = now - chrono::Duration::days(session.days_ago);
        let mut text = String::new();
        for (turn, cost) in session.turns.iter().enumerate() {
            let at = day + chrono::Duration::minutes(turn as i64 * 9);
            let line = serde_json::json!({
                "type": "assistant",
                "sessionId": format!("demo-session-{index}"),
                "cwd": format!("C:/demo/{}", session.project),
                "timestamp": at.to_rfc3339(),
                "costUSD": cost,
                "message": {
                    "model": session.model,
                    "usage": {
                        "input_tokens": 900 + turn * 240,
                        "output_tokens": 320 + turn * 90,
                        "cache_read_input_tokens": 150,
                        "cache_creation_input_tokens": 40
                    }
                }
            });
            text.push_str(&line.to_string());
            text.push('\n');
        }
        let path = directory.join(format!("demo-session-{index}.jsonl"));
        std::fs::write(&path, text)
            .map_err(|error| format!("demo: could not write {}: {error}", path.display()))?;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_seed_writes_parsable_transcripts_with_costs() {
        let root = std::env::temp_dir().join(format!("forja-usage-demo-{}", std::process::id()));
        demo_seed(&root).unwrap();
        let roots = crate::scan::Roots {
            claude: vec![root.clone()],
            codex: Vec::new(),
            opencode: Vec::new(),
        };
        let files = crate::scan::discover(&roots);
        assert_eq!(files.len(), SESSIONS.len());
        let mut records = 0;
        for (_, path) in files {
            let text = std::fs::read_to_string(&path).unwrap();
            let lines: Vec<String> = text.lines().map(str::to_owned).collect();
            records += crate::parse::parse_claude(&lines, "demo", "demo").len();
        }
        assert_eq!(records, 21);
        let _ = std::fs::remove_dir_all(root);
    }
}
