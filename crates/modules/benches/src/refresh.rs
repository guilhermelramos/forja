use crate::git;
use crate::model::{self, now_millis, BenchRecord};
use crate::service::{ensure_main, projects, setting_i64, setting_str};
use crate::stages;
use crate::state::Shared;
use forja_contracts::ProjectRef;
use forja_kernel::Ctx;
use std::path::Path;
pub fn prune(cx: &mut Ctx<'_>, shared: &Shared) -> Result<(), String> {
    let days = setting_i64(cx, "bench.worktree.auto_prune_days", 0);
    if days <= 0 {
        return Ok(());
    }
    let cutoff = now_millis().saturating_sub(days as u64 * 86_400_000);
    let has_old = shared
        .file()
        .benches
        .iter()
        .any(|bench| bench.archived && bench.created_at > 0 && bench.created_at < cutoff);
    if !has_old {
        return Ok(());
    }
    shared.mutate(cx, |file| {
        file.benches
            .retain(|bench| !(bench.archived && bench.created_at > 0 && bench.created_at < cutoff));
    })
}

pub fn refresh(cx: &mut Ctx<'_>, shared: &Shared) -> Result<(), String> {
    prune(cx, shared)?;
    ensure_main(cx, shared)?;
    let projects = projects(cx);
    if projects.is_empty() {
        return Ok(());
    }
    let state = shared.clone();
    cx.spawn(
        move || {
            projects
                .into_iter()
                .map(|project| {
                    let result = git::list_worktrees(Path::new(&project.path));
                    (project, result)
                })
                .collect::<Vec<_>>()
        },
        move |cx, results| finish_refresh(cx, &state, results),
    );
    Ok(())
}

fn finish_refresh(
    cx: &mut Ctx<'_>,
    shared: &Shared,
    results: Vec<(ProjectRef, Result<Vec<git::WorktreeEntry>, String>)>,
) {
    let default_stage = setting_str(cx, "bench.stage.creation_default", stages::DRAFT);
    let mut warnings = Vec::new();
    if let Err(error) =
        shared.mutate(cx, |file| {
            for (project, result) in &results {
                let entries = match result {
                    Ok(entries) => entries,
                    Err(error) => {
                        let human = crate::create::humanize_error(cx, error);
                        warnings.push(format!("{}: {}", project.name, human.summary));
                        continue;
                    }
                };
                for entry in entries {
                    if entry.bare {
                        continue;
                    }
                    let path = entry.path.to_string_lossy().into_owned();
                    if entry.is_main(Path::new(&project.path)) {
                        if let Some(bench) = file
                            .benches
                            .iter_mut()
                            .find(|bench| bench.project_id == project.id && bench.is_main)
                        {
                            bench.path = path;
                            bench.branch = entry.branch.clone();
                        }
                        continue;
                    }
                    if let Some(bench) = file
                        .benches
                        .iter_mut()
                        .find(|bench| git::same_path(Path::new(&bench.path), &entry.path))
                    {
                        bench.branch = entry.branch.clone();
                        bench.archived = false;
                        continue;
                    }
                    let bench_id = model::bench_id(&project.id, &entry.path, 0);
                    file.benches.push(BenchRecord {
                        bench_id,
                        project_id: project.id.clone(),
                        name: entry
                            .path
                            .file_name()
                            .and_then(|name| name.to_str())
                            .unwrap_or("bancada")
                            .to_owned(),
                        path,
                        branch: entry.branch.clone(),
                        is_main: false,
                        stage: default_stage.clone(),
                        archived: false,
                        pinned: false,
                        unread: false,
                        sleeping: false,
                        created_at: now_millis(),
                        activated_at: 0,
                        base_branch: None,
                        parent_bench_id: None,
                        created_head: entry.head.clone(),
                        request_command: None,
                        open_tabs: Vec::new(),
                        request_key: None,
                    });
                }
                let listed: Vec<&git::WorktreeEntry> = entries.iter().collect();
                for bench in file.benches.iter_mut().filter(|bench| {
                    bench.project_id == project.id && !bench.is_main && !bench.archived
                }) {
                    let present = listed
                        .iter()
                        .any(|entry| git::same_path(&entry.path, Path::new(&bench.path)));
                    if !present {
                        bench.archived = true;
                        bench.sleeping = false;
                    }
                }
            }
        })
    {
        shared.set_error(Some(error));
        return;
    }
    if !warnings.is_empty() {
        shared.set_notice(Some(warnings.join("; ")));
    }
}
