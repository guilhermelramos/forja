//! `--demo`: fixed sample data for the atlas and for screenshots.
//!
//! The shell owns the scenario (folders, branches, stages) and the two sample
//! git repositories; each module owns the shape of its own file and receives
//! only the data it needs. Nothing here starts a helper, opens a window or
//! touches the person's real data.
//!
//! ```text
//! forja-app --demo --test-driver
//! forja-app --demo --screenshot <arquivo.png>
//! ```

use std::path::{Path, PathBuf};

/// Folder with the demo repositories, shared between runs so repeated captures
/// (the atlas) do not pay for git every time.
pub(crate) fn repos_root() -> PathBuf {
    std::env::temp_dir().join("forja-demo-repos")
}

/// One sample bench: which repo it comes from, its stage and its branch.
struct Bench {
    project: usize,
    name: &'static str,
    branch: &'static str,
    stage: &'static str,
    pinned: bool,
    sleeping: bool,
    request: Option<&'static str>,
}

const PROJECTS: [(&str, &str); 2] = [("oficina", "oficina-de-projetos"), ("estudio", "estudio-de-texto")];

const BENCHES: [Bench; 6] = [
    Bench { project: 0, name: "relatorio", branch: "feature/relatorio", stage: "review", pinned: true, sleeping: false, request: Some("LOCAL-1") },
    Bench { project: 0, name: "graficos", branch: "feature/graficos", stage: "forging", pinned: false, sleeping: false, request: Some("LOCAL-3") },
    Bench { project: 0, name: "anexo", branch: "feature/anexo", stage: "draft", pinned: false, sleeping: true, request: Some("LOCAL-7") },
    Bench { project: 0, name: "orcamento", branch: "feature/orcamento", stage: "draft", pinned: false, sleeping: false, request: Some("LOCAL-6") },
    Bench { project: 0, name: "pagina-final", branch: "feature/pagina-final", stage: "ready", pinned: false, sleeping: false, request: Some("LOCAL-9") },
    Bench { project: 1, name: "correcao", branch: "feature/correcao", stage: "review", pinned: true, sleeping: false, request: Some("LOCAL-4") },
];

/// Prepares the repositories and writes the sample state of every module that
/// knows how to receive it. Call before the runtime starts.
pub fn seed(data_dir: &Path) -> Result<(), String> {
    std::env::set_var("FORJA_TEST_HELPERS", "none");
    std::env::set_var("FORJA_DEMO", "1");
    let repos = repos_root();
    let project_paths: Vec<PathBuf> = PROJECTS
        .iter()
        .map(|(folder, _)| repos.join(folder))
        .collect();
    let bench_paths: Vec<PathBuf> = BENCHES
        .iter()
        .enumerate()
        .map(|(index, bench)| repos.join("benches").join(format!("{index}-{}", bench.name)))
        .collect();
    let prepared = project_paths
        .iter()
        .chain(bench_paths.iter())
        .all(|path| path.join(".git").exists());
    if !prepared {
        // A missing repository still lets the rest of the demo show: the board
        // draws its cards from the module files, not from git.
        if let Err(error) = create_repositories(&repos, &project_paths) {
            forja_kernel::logging::log(
                forja_kernel::logging::LogLevel::Warn,
                format!("forja-app: demo repositories incomplete: {error}"),
            );
        }
    }

    // Paths go to the modules with forward slashes, the form the store keeps.
    let display = |path: &Path| path.to_string_lossy().replace('\\', "/");
    let projects: Vec<(String, String)> = PROJECTS
        .iter()
        .zip(&project_paths)
        .map(|((folder, _), path)| (title(folder), display(path)))
        .collect();
    let store = forja_kernel::Store::new(data_dir);
    forja_module_projects::demo::demo_seed(&store, &projects)?;

    let project_ids: Vec<String> = project_paths
        .iter()
        .map(|path| forja_module_projects::project_id(path))
        .collect();
    let mains: Vec<(String, String, String)> = project_ids
        .iter()
        .zip(&projects)
        .map(|(id, (name, path))| (id.clone(), name.clone(), path.clone()))
        .collect();
    let benches: Vec<forja_module_benches::demo::DemoBench> = BENCHES
        .iter()
        .enumerate()
        .map(|(index, bench)| forja_module_benches::demo::DemoBench {
            project_id: project_ids[bench.project].clone(),
            name: title(bench.name),
            path: display(&bench_paths[index]),
            branch: bench.branch.to_owned(),
            stage: bench.stage.to_owned(),
            pinned: bench.pinned,
            sleeping: bench.sleeping,
            request: bench.request.map(str::to_owned),
        })
        .collect();
    forja_module_benches::demo::demo_seed(&store, &mains, &benches, Some("b-demo-0"))?;

    let helper_benches: Vec<String> = bench_paths.iter().take(2).map(|path| display(path)).collect();
    forja_module_muster::demo::demo_seed(&store, &display(&bench_paths[0]))?;
    forja_module_request::demo::demo_seed(&store)?;
    forja_module_helper::demo::demo_seed(&store, &helper_benches)?;

    let claude = data_dir.join("demo").join("claude");
    forja_module_usage::demo::demo_seed(&claude)?;
    std::env::set_var("CLAUDE_CONFIG_DIR", &claude);
    Ok(())
}

/// A folder name turned into something readable: `pagina-final` -> `Pagina final`.
fn title(name: &str) -> String {
    let mut chars = name.chars();
    match chars.next() {
        Some(first) => first.to_uppercase().collect::<String>() + &chars.as_str().replace('-', " "),
        None => String::new(),
    }
}

fn create_repositories(repos: &Path, project_paths: &[PathBuf]) -> Result<(), String> {
    std::fs::create_dir_all(repos).map_err(|error| error.to_string())?;
    for (index, (_, name)) in PROJECTS.iter().enumerate() {
        let path = &project_paths[index];
        std::fs::create_dir_all(path).map_err(|error| error.to_string())?;
        write_files(path, name, index)?;
        git(path, &["init", "-b", "main"])?;
        git(path, &["add", "-A"])?;
        commit(path, "Primeiro esboço")?;
        std::fs::write(
            path.join("notas.md"),
            "# Notas\n\n- conferir os números\n- revisar o texto\n",
        )
        .map_err(|error| error.to_string())?;
        git(path, &["add", "-A"])?;
        commit(path, "Anotar o que falta")?;
    }
    for (index, bench) in BENCHES.iter().enumerate() {
        let worktree = repos.join("benches").join(format!("{index}-{}", bench.name));
        if worktree.join(".git").exists() {
            continue;
        }
        let project = &project_paths[bench.project];
        let worktree_text = worktree.to_string_lossy().to_string();
        // One broken worktree must not hide the rest of the demo.
        if let Err(error) = git(
            project,
            &["worktree", "add", "-b", bench.branch, &worktree_text],
        ) {
            forja_kernel::logging::log(
                forja_kernel::logging::LogLevel::Warn,
                format!("forja-app: demo worktree {}: {error}", bench.name),
            );
        }
    }
    Ok(())
}

fn write_files(path: &Path, name: &str, index: usize) -> Result<(), String> {
    std::fs::write(
        path.join("README.md"),
        format!("# {name}\n\nProjeto de demonstração da Forja, sem dados de ninguém.\n"),
    )
    .map_err(|error| error.to_string())?;
    std::fs::create_dir_all(path.join("src")).map_err(|error| error.to_string())?;
    std::fs::write(
        path.join("src").join("resumo.md"),
        format!(
            "# Resumo\n\nLinha {} do relatório de demonstração.\n",
            index + 1
        ),
    )
    .map_err(|error| error.to_string())?;
    Ok(())
}

fn commit(path: &Path, message: &str) -> Result<(), String> {
    git(
        path,
        &[
            "-c",
            "user.name=Forja",
            "-c",
            "user.email=forja@localhost",
            "-c",
            "commit.gpgsign=false",
            "commit",
            "-q",
            "-m",
            message,
        ],
    )
}

fn git(path: &Path, args: &[&str]) -> Result<(), String> {
    let output = forja_kernel::process::command("git")
        .args(args)
        .current_dir(path)
        .stdin(std::process::Stdio::null())
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::piped())
        .output()
        .map_err(|error| format!("git {}: {error}", args.join(" ")))?;
    if output.status.success() {
        return Ok(());
    }
    Err(format!(
        "git {}: {}",
        args.join(" "),
        String::from_utf8_lossy(&output.stderr).trim()
    ))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn folder_names_become_readable_titles() {
        assert_eq!(title("pagina-final"), "Pagina final");
        assert_eq!(title("relatorio"), "Relatorio");
        assert_eq!(title(""), "");
    }

    #[test]
    fn the_scenario_has_two_projects_and_six_benches_across_stages() {
        assert_eq!(PROJECTS.len(), 2);
        assert_eq!(BENCHES.len(), 6);
        // The board filters by the active project; five demo benches there plus
        // the project root make the six cards the atlas photographs.
        assert_eq!(BENCHES.iter().filter(|bench| bench.project == 0).count(), 5);
        for stage in ["draft", "forging", "review", "ready"] {
            assert!(
                BENCHES.iter().any(|bench| bench.stage == stage),
                "no demo bench in stage {stage}"
            );
        }
    }
}
