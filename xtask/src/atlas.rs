//! `cargo xtask atlas`: the visual atlas of Forja.
//!
//! ```text
//! cargo xtask atlas capture --exe <forja-app.exe> --out <dir> [--locales pt-BR,en] [--themes dark,light]
//! cargo xtask atlas status --out <dir>
//! ```
//!
//! `capture` asks the app for its views (`--dump-catalog`), opens every state
//! (rail page, bench tab, right tab, settings section, modal, overlay) in every
//! locale and theme, and saves `captures/<state>.png` plus the widget tree
//! `captures/<state>.json` (`--ui-tree`). It writes `manifest.json`.
//!
//! `status` compares the manifest with the pages in `pages/<state>.md`
//! (front matter `state` and `capture_sha`) and writes `todo.json`: states with
//! no page, pages whose capture changed, and how many named widgets each page
//! mentions. Pages are written by people or by the atlas agent (`atlas/agent/`).

use crate::util::write;
use serde_json::{json, Value};
use sha2::{Digest, Sha256};
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::time::{Duration, Instant};

pub fn run(args: &[String]) -> Result<(), String> {
    match args.first().map(String::as_str) {
        Some("capture") => capture(&Options::parse(&args[1..])?),
        Some("status") => status(&Options::parse(&args[1..])?),
        Some(command @ ("brief" | "accept" | "index")) => crate::atlas_agent::run(command, &args[1..]),
        _ => Err("use `cargo xtask atlas capture|status|brief|accept|index --out <dir> [...]`".to_owned()),
    }
}

struct Options {
    exe: Option<PathBuf>,
    out: PathBuf,
    locales: Vec<String>,
    themes: Vec<String>,
    /// Capture the demo state next to the empty one (default on).
    demo: bool,
}

impl Options {
    fn parse(args: &[String]) -> Result<Self, String> {
        let mut options = Options {
            exe: None,
            out: PathBuf::from("atlas-out"),
            locales: vec!["pt-BR".into(), "en".into()],
            themes: vec!["dark".into(), "light".into()],
            demo: true,
        };
        let mut iter = args.iter();
        while let Some(arg) = iter.next() {
            let mut value = || {
                iter.next()
                    .cloned()
                    .ok_or_else(|| format!("{arg} needs a value"))
            };
            match arg.as_str() {
                "--exe" => options.exe = Some(PathBuf::from(value()?)),
                "--out" => options.out = PathBuf::from(value()?),
                "--locales" => options.locales = split(&value()?),
                "--themes" => options.themes = split(&value()?),
                "--no-demo" => options.demo = false,
                other => return Err(format!("unknown atlas option `{other}`")),
            }
        }
        Ok(options)
    }
}

fn split(value: &str) -> Vec<String> {
    value
        .split(',')
        .map(str::trim)
        .filter(|part| !part.is_empty())
        .map(str::to_owned)
        .collect()
}

/// One screen the atlas visits: which flag opens it.
struct Target {
    kind: &'static str,
    id: String,
    args: Vec<String>,
}

fn targets(catalog: &Value) -> Vec<Target> {
    let mut targets = vec![Target {
        kind: "home",
        id: "default".into(),
        args: Vec::new(),
    }];
    for overlay in ["anvil", "onboarding", "toast"] {
        targets.push(Target {
            kind: "open",
            id: overlay.into(),
            args: vec!["--open".into(), overlay.into()],
        });
    }
    let views = catalog["views"].as_array().cloned().unwrap_or_default();
    let mut settings_modules = Vec::new();
    for view in &views {
        let id = view["id"].as_str().unwrap_or_default().to_owned();
        let flag = match view["slot"].as_str().unwrap_or_default() {
            "rail" => Some(("rail", "--rail")),
            "bench_tab" => Some(("bench-tab", "--bench-tab")),
            "right_panel" => Some(("right-tab", "--right-tab")),
            "modal" => Some(("modal", "--modal")),
            "settings_section" => {
                let module = view["module"].as_str().unwrap_or_default().to_owned();
                if !module.is_empty() && !settings_modules.contains(&module) {
                    settings_modules.push(module);
                }
                None
            }
            _ => None,
        };
        if let Some((kind, flag)) = flag {
            targets.push(Target {
                kind,
                id: id.clone(),
                args: vec![flag.into(), id],
            });
        }
    }
    for module in settings_modules {
        targets.push(Target {
            kind: "settings",
            id: module.clone(),
            args: vec!["--settings".into(), module],
        });
    }
    targets
}

fn state_id(locale: &str, theme: &str, target: &Target, demo: bool) -> String {
    // The empty state keeps its historic id; the demonstration one is prefixed
    // with `demo` so both live side by side in `captures/`.
    let kind = if demo {
        format!("demo.{}", target.kind)
    } else {
        target.kind.to_owned()
    };
    format!("{locale}.{theme}.{kind}.{}", target.id)
        .chars()
        .map(|c| if c.is_ascii_alphanumeric() || matches!(c, '.' | '-' | '_') { c } else { '_' })
        .collect()
}

fn capture(options: &Options) -> Result<(), String> {
    let exe = options
        .exe
        .clone()
        .ok_or_else(|| "atlas capture needs --exe <forja-app.exe>".to_owned())?;
    let out = &options.out;
    let captures = out.join("captures");
    std::fs::create_dir_all(&captures).map_err(|error| error.to_string())?;
    let scratch = std::env::temp_dir().join(format!("forja-atlas-{}", std::process::id()));

    let mut states = Vec::new();
    let mut failures = Vec::new();
    // First state seen for each capture hash: identical screens are documented once.
    let mut first_by_sha: std::collections::HashMap<String, String> = Default::default();
    for locale in &options.locales {
        let catalog = dump_catalog(&exe, locale, &scratch)?;
        let targets = targets(&catalog);
        for theme in &options.themes {
            let modes: &[bool] = if options.demo { &[false, true] } else { &[false] };
            for target in &targets {
                for &demo in modes {
                    let id = state_id(locale, theme, target, demo);
                    let png = captures.join(format!("{id}.png"));
                    let tree = captures.join(format!("{id}.json"));
                    let data = scratch.join(&id);
                    let mut command = Command::new(&exe);
                    command
                        .args(["--screenshot"])
                        .arg(&png)
                        .arg("--ui-tree")
                        .arg(&tree)
                        .args(["--locale", locale, "--theme", theme])
                        .args(&target.args)
                        .env("FORJA_DATA_DIR", data.join("data"))
                        .env("FORJA_CONTENT_DIR", data.join("user"))
                        .stdout(Stdio::null())
                        .stderr(Stdio::null());
                    if demo {
                        command.arg("--demo");
                    }
                    let ok = run_with_timeout(&mut command, Duration::from_secs(90))?;
                    if !ok || !png.exists() || !tree.exists() {
                        eprintln!("atlas: FAILED {id}");
                        failures.push(id);
                        continue;
                    }
                    let tree_json: Value = serde_json::from_str(
                        &std::fs::read_to_string(&tree).map_err(|error| error.to_string())?,
                    )
                    .map_err(|error| format!("{}: {error}", tree.display()))?;
                    let names = named_widgets(&tree_json);
                    let sha = short_sha(&std::fs::read(&png).map_err(|error| error.to_string())?);
                    let same_as = first_by_sha.get(&sha).cloned();
                    first_by_sha.entry(sha.clone()).or_insert_with(|| id.clone());
                    eprintln!("atlas: {id} ({} named widgets)", names.len());
                    states.push(json!({
                        "state": id,
                        "locale": locale,
                        "theme": theme,
                        "kind": target.kind,
                        "demo": demo,
                        "target": target.id,
                        "args": target.args,
                        "png": format!("captures/{id}.png"),
                        "tree": format!("captures/{id}.json"),
                        "capture_sha": sha,
                        "same_as": same_as,
                        "nodes": tree_json["nodes"].as_array().map_or(0, Vec::len),
                        "named_widgets": names,
                    }));
                }
            }
        }
    }
    let _ = std::fs::remove_dir_all(&scratch);
    let manifest = json!({ "format": "forja.atlas/1", "states": states, "failures": failures });
    write(
        &out.join("manifest.json"),
        &serde_json::to_string_pretty(&manifest).map_err(|error| error.to_string())?,
    )?;
    println!(
        "atlas: {} states captured, {} failed -> {}",
        manifest["states"].as_array().map_or(0, Vec::len),
        manifest["failures"].as_array().map_or(0, Vec::len),
        out.join("manifest.json").display()
    );
    Ok(())
}

fn dump_catalog(exe: &Path, locale: &str, scratch: &Path) -> Result<Value, String> {
    let output = Command::new(exe)
        .args(["--dump-catalog", "--locale", locale])
        .env("FORJA_DATA_DIR", scratch.join("catalog"))
        .env("FORJA_CONTENT_DIR", scratch.join("catalog-user"))
        .output()
        .map_err(|error| format!("{}: {error}", exe.display()))?;
    if !output.status.success() {
        return Err(format!(
            "--dump-catalog failed: {}",
            String::from_utf8_lossy(&output.stderr)
        ));
    }
    serde_json::from_slice(&output.stdout).map_err(|error| format!("--dump-catalog: {error}"))
}

fn run_with_timeout(command: &mut Command, limit: Duration) -> Result<bool, String> {
    let mut child = command.spawn().map_err(|error| error.to_string())?;
    let started = Instant::now();
    loop {
        if let Some(status) = child.try_wait().map_err(|error| error.to_string())? {
            return Ok(status.success());
        }
        if started.elapsed() > limit {
            let _ = child.kill();
            let _ = child.wait();
            return Ok(false);
        }
        std::thread::sleep(Duration::from_millis(200));
    }
}

/// Distinct visible names of widgets in a `--ui-tree` file (icons excluded).
fn named_widgets(tree: &Value) -> Vec<String> {
    let mut names: Vec<String> = tree["nodes"]
        .as_array()
        .into_iter()
        .flatten()
        .filter(|node| node["role"] != "TextRun")
        .filter_map(|node| node["name"].as_str())
        .map(str::trim)
        .filter(|name| name.chars().any(char::is_alphanumeric))
        .filter(|name| !name.chars().all(|c| ('\u{E000}'..='\u{F8FF}').contains(&c)))
        .map(str::to_owned)
        .collect();
    names.sort();
    names.dedup();
    names
}

fn short_sha(bytes: &[u8]) -> String {
    let digest = Sha256::digest(bytes);
    digest.iter().take(8).map(|byte| format!("{byte:02x}")).collect()
}

fn status(options: &Options) -> Result<(), String> {
    let out = &options.out;
    let manifest: Value = serde_json::from_str(&crate::util::read(&out.join("manifest.json"))?)
        .map_err(|error| format!("manifest.json: {error}"))?;
    let mut missing = Vec::new();
    let mut stale = Vec::new();
    let mut done = Vec::new();
    let mut widgets_total = 0usize;
    let mut widgets_covered = 0usize;
    for state in manifest["states"].as_array().into_iter().flatten() {
        let id = state["state"].as_str().unwrap_or_default();
        if state["same_as"].is_string() {
            // Identical pixels to an earlier state: its page covers this one.
            continue;
        }
        let page_path = out.join("pages").join(format!("{id}.md"));
        let names: Vec<&str> = state["named_widgets"]
            .as_array()
            .into_iter()
            .flatten()
            .filter_map(Value::as_str)
            .collect();
        let Ok(page) = std::fs::read_to_string(&page_path) else {
            missing.push(json!({ "state": id, "png": state["png"], "tree": state["tree"] }));
            continue;
        };
        let recorded = front_matter(&page, "capture_sha");
        let flat_page = flatten(&page);
        let uncovered: Vec<&str> = names
            .iter()
            .copied()
            .filter(|name| !flat_page.contains(&flatten(name)))
            .collect();
        widgets_total += names.len();
        widgets_covered += names.len() - uncovered.len();
        if recorded.as_deref() != state["capture_sha"].as_str() || !uncovered.is_empty() {
            stale.push(json!({
                "state": id,
                "png": state["png"],
                "tree": state["tree"],
                "page": format!("pages/{id}.md"),
                "capture_changed": recorded.as_deref() != state["capture_sha"].as_str(),
                "widgets_not_mentioned": uncovered,
            }));
        } else {
            done.push(id.to_owned());
        }
    }
    // pt-BR dark first: the reference variant is documented before the others.
    let rank = |item: &Value| {
        let id = item["state"].as_str().unwrap_or_default();
        (!id.starts_with("pt-BR.dark."), id.to_owned())
    };
    missing.sort_by_key(rank);
    stale.sort_by_key(rank);
    let total = manifest["states"]
        .as_array()
        .map_or(0, |states| states.iter().filter(|state| !state["same_as"].is_string()).count());
    let todo = json!({
        "states_total": total,
        "states_done": done.len(),
        "widgets_total": widgets_total,
        "widgets_covered": widgets_covered,
        "complete": missing.is_empty() && stale.is_empty() && total > 0,
        "stale": stale,
        "missing": missing,
    });
    write(
        &out.join("todo.json"),
        &serde_json::to_string_pretty(&todo).map_err(|error| error.to_string())?,
    )?;
    println!(
        "atlas: {}/{} states documented, {} stale, {} missing; widgets mentioned {}/{}; complete={}",
        done.len(),
        total,
        todo["stale"].as_array().map_or(0, Vec::len),
        todo["missing"].as_array().map_or(0, Vec::len),
        widgets_covered,
        widgets_total,
        todo["complete"]
    );
    Ok(())
}

/// Collapses whitespace and `<br>` so multi-line names match table cells.
fn flatten(text: &str) -> String {
    text.replace("<br>", " ")
        .replace("<br/>", " ")
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ")
}

/// Reads `key: value` from a `---` front matter block.
fn front_matter(page: &str, key: &str) -> Option<String> {
    let body = page.strip_prefix("---")?;
    let end = body.find("\n---")?;
    body[..end].lines().find_map(|line| {
        let (name, value) = line.split_once(':')?;
        (name.trim() == key).then(|| value.trim().trim_matches('"').to_owned())
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn front_matter_reads_keys() {
        let page = "---\nstate: a.b\ncapture_sha: \"abc\"\n---\n# x\n";
        assert_eq!(front_matter(page, "capture_sha").as_deref(), Some("abc"));
        assert_eq!(front_matter(page, "state").as_deref(), Some("a.b"));
        assert_eq!(front_matter("# no front matter", "state"), None);
    }

    #[test]
    fn flatten_matches_multiline_names() {
        assert_eq!(flatten("Nenhuma bancada ativa.
Crie uma"), flatten("Nenhuma bancada ativa.<br>Crie  uma"));
    }

    #[test]
    fn named_widgets_skip_icons_and_text_runs() {
        let tree = json!({ "nodes": [
            { "role": "Button", "name": "Nova bancada" },
            { "role": "TextRun", "name": "Nova bancada" },
            { "role": "Button", "name": "\u{E713}" },
            { "role": "Label", "name": "Quadro" },
            { "role": "Label", "name": "Quadro" },
        ]});
        assert_eq!(named_widgets(&tree), vec!["Nova bancada", "Quadro"]);
    }

    #[test]
    fn targets_cover_every_slot_kind() {
        let catalog = json!({ "views": [
            { "id": "board.page", "slot": "rail", "module": "board" },
            { "id": "editor.tab", "slot": "bench_tab", "module": "editor" },
            { "id": "git.changes", "slot": "right_panel", "module": "git" },
            { "id": "voice.overlay", "slot": "modal", "module": "voice" },
            { "id": "remote.settings", "slot": "settings_section", "module": "remote" },
            { "id": "remote.more", "slot": "settings_section", "module": "remote" },
        ]});
        let kinds: Vec<String> = targets(&catalog)
            .iter()
            .map(|target| format!("{}:{}", target.kind, target.id))
            .collect();
        assert!(kinds.contains(&"rail:board.page".to_owned()));
        assert!(kinds.contains(&"bench-tab:editor.tab".to_owned()));
        assert!(kinds.contains(&"right-tab:git.changes".to_owned()));
        assert!(kinds.contains(&"modal:voice.overlay".to_owned()));
        assert_eq!(kinds.iter().filter(|kind| kind.starts_with("settings:")).count(), 1);
        let home = &targets(&catalog)[0];
        assert_eq!(state_id("pt-BR", "dark", home, false), "pt-BR.dark.home.default");
        assert_eq!(
            state_id("pt-BR", "dark", home, true),
            "pt-BR.dark.demo.home.default"
        );
    }
}
