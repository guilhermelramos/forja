//! Headless agent text generation: commit messages, PR title/body and branch names.
//!
//! The agent comes from the helper profile the app would launch on the bench.
//! The profile supplies the headless argv, so the same CLIs used interactively
//! also answer these one-shot prompts.

use serde::Deserialize;
use std::io::Write;
use std::path::Path;
use std::process::Stdio;
use std::time::{Duration, Instant};

pub const TIMEOUT: Duration = Duration::from_secs(120);
pub const MAX_CONTEXT_CHARS: usize = 12_000;

pub const BUILTIN: &[&str] = &[
    include_str!("../../../../content/helpers/aider.toml"),
    include_str!("../../../../content/helpers/amp.toml"),
    include_str!("../../../../content/helpers/antigravity.toml"),
    include_str!("../../../../content/helpers/claude-code.toml"),
    include_str!("../../../../content/helpers/cline.toml"),
    include_str!("../../../../content/helpers/codex.toml"),
    include_str!("../../../../content/helpers/continue.toml"),
    include_str!("../../../../content/helpers/copilot.toml"),
    include_str!("../../../../content/helpers/crush.toml"),
    include_str!("../../../../content/helpers/cursor-agent.toml"),
    include_str!("../../../../content/helpers/gemini.toml"),
    include_str!("../../../../content/helpers/goose.toml"),
    include_str!("../../../../content/helpers/kimi-code.toml"),
    include_str!("../../../../content/helpers/kiro.toml"),
    include_str!("../../../../content/helpers/opencode.toml"),
    include_str!("../../../../content/helpers/qwen-code.toml"),
];

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Kind {
    Commit,
    PullRequest,
    Branch,
}

impl Kind {
    pub fn parse(value: &str) -> Option<Self> {
        match value.trim() {
            "commit" => Some(Self::Commit),
            "pr" | "pull_request" | "pull-request" => Some(Self::PullRequest),
            "branch" => Some(Self::Branch),
            _ => None,
        }
    }

    pub fn id(self) -> &'static str {
        match self {
            Self::Commit => "commit",
            Self::PullRequest => "pr",
            Self::Branch => "branch",
        }
    }
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct PrText {
    pub title: String,
    pub body: String,
}

#[derive(Clone, Debug, Deserialize)]
struct ProfileDoc {
    id: String,
    start: Start,
    #[serde(default)]
    prompt: PromptDoc,
}

#[derive(Clone, Debug, Deserialize)]
struct Start {
    #[serde(default)]
    command: String,
}

#[derive(Clone, Debug, Default, Deserialize)]
struct PromptDoc {
    #[serde(default)]
    headless: Vec<String>,
    #[serde(default)]
    headless_stdin: bool,
}

#[derive(Clone, Debug)]
pub struct Profile {
    pub program: String,
    pub args: Vec<String>,
    pub stdin: bool,
}

pub fn prompt(kind: Kind, diff: &str, commits: &str, extra: &str) -> String {
    let diff = truncate(diff);
    let commits = truncate(commits);
    match kind {
        Kind::Commit => format!(
            "Write a concise Git commit message for the staged changes below. \
             Use the imperative mood, keep the subject under 72 characters, and add a short \
             body only when it helps. Return only the message, without fences.\n\n{diff}"
        ),
        Kind::PullRequest => format!(
            "Write a pull request title on the first line and a Markdown body below it. \
             Explain what changed, why, and what reviewers should check. Do not use code \
             fences around the whole answer.\n\nCommits:\n{commits}\n\nBranch diff:\n{diff}\n\n{extra}"
        ),
        Kind::Branch => format!(
            "Create a short Git branch name for the work below. Use lowercase words separated \
             by hyphens, keep the issue key when present, and return only the branch name.\n\n{extra}\n\n{commits}"
        ),
    }
}

fn truncate(text: &str) -> String {
    if text.chars().count() <= MAX_CONTEXT_CHARS {
        return text.to_owned();
    }
    let head: String = text.chars().take(MAX_CONTEXT_CHARS).collect();
    format!("{head}\n[context truncated]")
}

pub fn clean(text: &str) -> String {
    let text = text.trim();
    let stripped = text
        .strip_prefix("```")
        .and_then(|rest| rest.split_once('\n').map(|(_, body)| body))
        .and_then(|body| body.strip_suffix("```"))
        .map(str::trim)
        .unwrap_or(text);
    stripped.trim().to_owned()
}

pub fn parse_pr(text: &str) -> PrText {
    let text = clean(text);
    let mut lines = text.lines();
    let title = lines
        .next()
        .unwrap_or_default()
        .trim()
        .trim_start_matches('#')
        .trim()
        .to_owned();
    let body = lines.collect::<Vec<_>>().join("\n").trim().to_owned();
    PrText { title, body }
}

pub fn branch_slug(text: &str) -> String {
    let line = clean(text);
    let line = line.lines().next().unwrap_or_default();
    let mut slug = String::new();
    let mut dash = false;
    for character in line.chars() {
        if character.is_ascii_alphanumeric() {
            slug.push(character.to_ascii_lowercase());
            dash = false;
        } else if !dash && !slug.is_empty() {
            slug.push('-');
            dash = true;
        }
    }
    let slug = slug.trim_matches('-');
    let slug: String = slug.chars().take(60).collect();
    if slug.is_empty() {
        "forja/work".to_owned()
    } else {
        slug
    }
}

/// Resolves the profile selected by the helper module, falling back to the
/// first built-in profile. Project and user profiles win over built-ins.
pub fn profile_for(bench: &Path, selected: Option<&str>) -> Option<Profile> {
    let mut candidates: Vec<(u8, ProfileDoc)> = Vec::new();
    let selected = selected.unwrap_or("claude-code");
    for text in BUILTIN {
        if let Ok(doc) = toml::from_str::<ProfileDoc>(text) {
            if doc.id == selected {
                candidates.push((0, doc));
            }
        }
    }
    for (layer, folder) in [
        (1, forja_kernel::paths::default_user_dir().join("helpers")),
        (2, bench.join(".forja").join("helpers")),
    ] {
        if let Ok(entries) = std::fs::read_dir(&folder) {
            for entry in entries.flatten() {
                let path = entry.path();
                if path.extension().and_then(|value| value.to_str()) != Some("toml") {
                    continue;
                }
                if let Ok(text) = std::fs::read_to_string(&path) {
                    if let Ok(doc) = toml::from_str::<ProfileDoc>(&text) {
                        if doc.id == selected {
                            candidates.push((layer, doc));
                        }
                    }
                }
            }
        }
    }
    let (_, doc) = candidates.into_iter().max_by_key(|(layer, _)| *layer)?;
    if doc.start.command.trim().is_empty() {
        return None;
    }
    if doc.prompt.headless.is_empty() && !doc.prompt.headless_stdin {
        return None;
    }
    let stdin = doc.prompt.headless_stdin
        && !doc
            .prompt
            .headless
            .iter()
            .any(|arg| arg.contains("{prompt}"));
    Some(Profile {
        program: doc.start.command,
        args: doc.prompt.headless,
        stdin,
    })
}

pub fn command_for(profile: &Profile, input: &str) -> (String, Vec<String>) {
    let mut args = Vec::new();
    let mut replaced = false;
    for arg in &profile.args {
        if arg.contains("{prompt}") {
            replaced = true;
            args.push(arg.replace("{prompt}", input));
        } else {
            args.push(arg.clone());
        }
    }
    if !replaced && !profile.stdin {
        args.push(input.to_owned());
    }
    if args.is_empty() && !profile.stdin {
        args.push(input.to_owned());
    }
    (profile.program.clone(), args)
}

pub fn run(profile: &Profile, input: &str, cwd: &Path) -> Result<String, String> {
    let (program, args) = command_for(profile, input);
    let mut child = forja_kernel::process::command(&program)
        .args(&args)
        .current_dir(cwd)
        .env("GIT_TERMINAL_PROMPT", "0")
        .stdin(if profile.stdin {
            Stdio::piped()
        } else {
            Stdio::null()
        })
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .map_err(|error| format!("could not start `{program}`: {error}"))?;
    if profile.stdin {
        if let Some(stdin) = child.stdin.as_mut() {
            stdin
                .write_all(input.as_bytes())
                .map_err(|error| format!("could not write the prompt: {error}"))?;
        }
    }
    let started = Instant::now();
    loop {
        match child.try_wait().map_err(|error| error.to_string())? {
            Some(_) => break,
            None if started.elapsed() >= TIMEOUT => {
                let _ = child.kill();
                let _ = child.wait();
                return Err(format!(
                    "the agent timed out after {} seconds",
                    TIMEOUT.as_secs()
                ));
            }
            None => std::thread::sleep(Duration::from_millis(100)),
        }
    }
    let output = child
        .wait_with_output()
        .map_err(|error| format!("could not read the agent output: {error}"))?;
    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        let detail = stderr
            .lines()
            .find(|line| !line.trim().is_empty())
            .unwrap_or("");
        return Err(if detail.is_empty() {
            "the agent failed without an error message".to_owned()
        } else {
            format!("the agent failed: {detail}")
        });
    }
    let text = clean(&String::from_utf8_lossy(&output.stdout));
    if text.is_empty() {
        return Err("the agent returned an empty response".to_owned());
    }
    Ok(text)
}

#[cfg(test)]
pub fn builtin_ids() -> Vec<&'static str> {
    BUILTIN
        .iter()
        .filter_map(|text| toml::from_str::<ProfileDoc>(text).ok())
        .map(|doc| Box::leak(doc.id.into_boxed_str()) as &'static str)
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn profile() -> Profile {
        Profile {
            program: "demo".into(),
            args: vec!["headless".into(), "{prompt}".into()],
            stdin: false,
        }
    }

    #[test]
    fn prompt_contains_the_diff_and_is_truncated() {
        let text = prompt(Kind::Commit, "diff --git a/a b/a", "", "");
        assert!(text.contains("diff --git"));
        let huge = "x".repeat(MAX_CONTEXT_CHARS + 50);
        assert!(prompt(Kind::Commit, &huge, "", "").contains("[context truncated]"));
    }

    #[test]
    fn pr_text_splits_title_and_body_and_removes_fences() {
        let parsed = parse_pr("```\nAdd login\n\nBody line\n```");
        assert_eq!(parsed.title, "Add login");
        assert_eq!(parsed.body, "Body line");
    }

    #[test]
    fn branch_slug_is_lowercase_and_safe() {
        assert_eq!(branch_slug("Feature: Add Login!"), "feature-add-login");
        assert_eq!(branch_slug("---"), "forja/work");
    }

    #[test]
    fn argv_replaces_the_prompt_placeholder() {
        assert_eq!(
            command_for(&profile(), "hello"),
            ("demo".into(), vec!["headless".into(), "hello".into()])
        );
        let mut stdin = profile();
        stdin.stdin = true;
        stdin.args = vec!["run".into()];
        assert_eq!(command_for(&stdin, "hello").1, vec!["run".to_string()]);
    }

    #[test]
    fn builtin_profiles_include_the_common_agents() {
        let ids = builtin_ids();
        assert!(ids.contains(&"claude-code"));
        assert!(ids.contains(&"codex"));
        assert!(ids.contains(&"opencode"));
    }

    #[test]
    fn builtin_profiles_with_headless_produce_the_right_argv() {
        let mut checked = 0;
        for text in BUILTIN {
            let doc: ProfileDoc = toml::from_str(text).expect("built-in profile parses");
            if doc.prompt.headless.is_empty() {
                continue;
            }
            let replaced = doc
                .prompt
                .headless
                .iter()
                .any(|arg| arg.contains("{prompt}"));
            let profile = Profile {
                program: doc.start.command.clone(),
                args: doc.prompt.headless,
                stdin: doc.prompt.headless_stdin && !replaced,
            };
            let (program, args) = command_for(&profile, "PROMPT");
            assert_eq!(program, doc.start.command);
            if replaced {
                assert!(args.iter().any(|arg| arg.contains("PROMPT")), "{}", doc.id);
            } else {
                assert!(profile.stdin, "{}", doc.id);
                assert!(!args.iter().any(|arg| arg.contains("PROMPT")), "{}", doc.id);
            }
            checked += 1;
        }
        assert!(checked >= 14, "only {checked} profiles have headless mode");
    }
}
