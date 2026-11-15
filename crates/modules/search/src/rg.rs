//! Candidate files from ripgrep (`rg`) when it is on the PATH. ripgrep only
//! lists the files that match; the match lines and the context are still built
//! by the engine, so both engines show the same hits and limits.

use crate::engine::Plan;
use std::path::PathBuf;
use std::process::Stdio;

/// Whether an `rg` executable answers `--version`. Runs a process: call it from
/// the search thread, never from the interface thread.
pub fn available() -> bool {
    forja_kernel::process::command("rg")
        .arg("--version")
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status()
        .is_ok_and(|status| status.success())
}

/// The `rg` arguments for a plan. The pattern is escaped unless regex mode is on.
pub fn args(plan: &Plan) -> Vec<String> {
    let options = &plan.options;
    let mut args: Vec<String> = vec![
        "--files-with-matches".into(),
        "--null".into(),
        "--no-messages".into(),
        "--max-filesize".into(),
        format!("{}", crate::engine::MAX_FILE_BYTES),
    ];
    if plan.include_hidden {
        args.push("--hidden".into());
    }
    if plan.respect_gitignore {
        // Read .gitignore even outside a git repository, like the built-in walker.
        args.push("--no-require-git".into());
    } else {
        args.push("--no-ignore".into());
    }
    if !options.case_sensitive {
        args.push("--ignore-case".into());
    }
    if !options.regex {
        args.push("--fixed-strings".into());
    }
    if options.whole_word {
        args.push("--word-regexp".into());
    }
    for glob in crate::query::split_globs(&options.include) {
        args.push("--glob".into());
        args.push(glob);
    }
    for glob in crate::query::split_globs(&options.exclude) {
        args.push("--glob".into());
        args.push(format!("!{glob}"));
    }
    args.push("--glob".into());
    args.push("!.git".into());
    args.push("--regexp".into());
    args.push(options.pattern.clone());
    args.push("--".into());
    args.push(plan.root.display().to_string());
    args
}

/// Files under the root that contain a match, as paths joined to the root.
/// Exit code 1 (no match) is an empty list; any other failure is an error.
pub fn candidate_files(plan: &Plan) -> Result<Vec<PathBuf>, String> {
    let output = forja_kernel::process::command("rg")
        .args(args(plan))
        .stdin(Stdio::null())
        .output()
        .map_err(|error| error.to_string())?;
    match output.status.code() {
        Some(0) | Some(1) => {}
        _ => return Err(String::from_utf8_lossy(&output.stderr).trim().to_owned()),
    }
    Ok(output
        .stdout
        .split(|byte| *byte == 0)
        .filter(|chunk| !chunk.is_empty())
        .map(|chunk| PathBuf::from(String::from_utf8_lossy(chunk).into_owned()))
        .collect())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::query::SearchOptions;

    fn plan(options: SearchOptions) -> Plan {
        Plan {
            root: PathBuf::from("C:/repo"),
            options,
            context: 2,
            include_hidden: false,
            respect_gitignore: true,
            engine: crate::engine::Engine::Auto,
        }
    }

    #[test]
    fn literal_search_is_fixed_strings_and_case_insensitive_by_default() {
        let args = args(&plan(SearchOptions {
            pattern: "a.b".into(),
            ..SearchOptions::default()
        }));
        assert!(args.contains(&"--fixed-strings".to_owned()));
        assert!(args.contains(&"--ignore-case".to_owned()));
        assert!(!args.contains(&"--word-regexp".to_owned()));
        assert_eq!(args.last().map(String::as_str), Some("C:/repo"));
        let pattern_at = args.iter().position(|arg| arg == "--regexp").unwrap();
        assert_eq!(args[pattern_at + 1], "a.b");
    }

    #[test]
    fn options_map_to_flags_and_globs() {
        let args = args(&Plan {
            include_hidden: true,
            respect_gitignore: false,
            ..plan(SearchOptions {
                pattern: "x".into(),
                regex: true,
                case_sensitive: true,
                whole_word: true,
                include: "*.rs".into(),
                exclude: "target/**".into(),
            })
        });
        for flag in ["--hidden", "--no-ignore", "--word-regexp"] {
            assert!(args.contains(&flag.to_owned()), "missing {flag}");
        }
        assert!(!args.contains(&"--ignore-case".to_owned()));
        assert!(!args.contains(&"--fixed-strings".to_owned()));
        assert!(args.contains(&"*.rs".to_owned()));
        assert!(args.contains(&"!target/**".to_owned()));
    }
}
