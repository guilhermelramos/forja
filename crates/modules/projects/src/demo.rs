//! Sample projects for `forja-app --demo`.
//!
//! The shell creates the folders and calls [`demo_seed`] before the runtime
//! starts; the module keeps owning the file format, so nothing about
//! `project.json` leaks into the shell.

use crate::model::{project_id, ProjectsFile, Project, DEFAULT_ICON, SCHEMA_VERSION};
use forja_kernel::Store;
use std::path::Path;

/// Colors used by the demo projects, alternating per folder.
const COLORS: [&str; 2] = ["#6FA8DC", "#E8A33D"];

/// Writes one record per `(name, path)` and leaves the first one active.
pub fn demo_seed(store: &Store, projects: &[(String, String)]) -> Result<(), String> {
    let mut records = Vec::with_capacity(projects.len());
    for (index, (name, path)) in projects.iter().enumerate() {
        records.push(Project {
            id: project_id(Path::new(path)),
            name: name.clone(),
            path: path.clone(),
            icon: DEFAULT_ICON.to_owned(),
            color: COLORS[index % COLORS.len()].to_owned(),
            created_at: 0,
        });
    }
    let file = ProjectsFile {
        schema_version: SCHEMA_VERSION,
        active_id: records.first().map(|project| project.id.clone()),
        projects: records,
    };
    store
        .save("project", &file)
        .map_err(|error| format!("demo: could not write projects: {error}"))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_seed_writes_two_projects_with_the_first_active() {
        let dir = std::env::temp_dir().join(format!("forja-project-demo-{}", std::process::id()));
        let store = Store::new(&dir);
        demo_seed(
            &store,
            &[
                ("oficina".to_owned(), "C:/demo/oficina".to_owned()),
                ("estudio".to_owned(), "C:/demo/estudio".to_owned()),
            ],
        )
        .unwrap();
        let file: ProjectsFile = store.load("project").into_option().expect("projects");
        assert_eq!(file.projects.len(), 2);
        assert_eq!(
            file.active_id.as_deref(),
            Some(project_id(Path::new("C:/demo/oficina")).as_str())
        );
        let _ = std::fs::remove_dir_all(dir);
    }
}
