//! In-memory state and mutations for the project list.

use crate::model::{self, NewProject, Project, ProjectsFile};
use forja_contracts::{ProjectActivated, ProjectAdded, ProjectRef, ProjectRemoved};
use forja_kernel::Ctx;
use std::collections::BTreeSet;
use std::sync::{Arc, Mutex};

#[derive(Clone, Debug, Default)]
pub enum Dialog {
    #[default]
    None,
    Add(ProjectForm),
    Create(ProjectForm),
    Clone(CloneForm),
    Edit(EditForm),
    Remove {
        project_id: String,
        name: String,
    },
}

#[derive(Clone, Debug)]
pub struct ProjectForm {
    pub path: String,
    pub name: String,
    pub icon: String,
    pub color: String,
    pub busy: bool,
    pub error: Option<String>,
}

impl Default for ProjectForm {
    fn default() -> Self {
        Self {
            path: String::new(),
            name: String::new(),
            icon: model::DEFAULT_ICON.to_owned(),
            color: model::DEFAULT_COLOR.to_owned(),
            busy: false,
            error: None,
        }
    }
}

#[derive(Clone, Debug, Default)]
pub struct CloneForm {
    pub url: String,
    pub form: ProjectForm,
}

#[derive(Clone, Debug)]
pub struct EditForm {
    pub project_id: String,
    pub name: String,
    pub icon: String,
    pub color: String,
    pub error: Option<String>,
}

#[derive(Default)]
struct State {
    loaded: bool,
    projects: Vec<Project>,
    recent: Vec<Project>,
    active_id: Option<String>,
    expanded: BTreeSet<String>,
    dialog: Dialog,
    status: Option<String>,
    error: Option<String>,
    benches_missing: bool,
}

#[derive(Clone)]
pub struct Shared(Arc<Mutex<State>>);

impl Shared {
    pub fn new() -> Self {
        Self(Arc::new(Mutex::new(State::default())))
    }

    pub fn ensure_loaded(&self, cx: &Ctx<'_>) {
        let mut state = match self.0.lock() {
            Ok(state) => state,
            Err(_) => return,
        };
        if state.loaded {
            return;
        }
        state.loaded = true;
        let open_last = !matches!(
            cx.setting("app.startup.open_last"),
            Some(forja_kernel::SettingValue::Bool(false))
        );
        match cx.store().load::<ProjectsFile>() {
            forja_kernel::LoadResult::Loaded(file) => {
                let file = file.sorted();
                let mut projects = file.projects;
                state.recent = file.recent;
                if state.recent.is_empty() {
                    state.recent = projects.clone();
                }
                for project in &mut projects {
                    project.path = forja_kernel::paths::clean_path_str(&project.path);
                }
                state.active_id = if open_last {
                    file.active_id
                        .filter(|id| projects.iter().any(|project| project.id == *id))
                        .or_else(|| projects.first().map(|project| project.id.clone()))
                } else {
                    None
                };
                state.projects = projects;
            }
            forja_kernel::LoadResult::Quarantined(path) => {
                state.error = Some(format!(
                    "project file was corrupted and moved to {}",
                    path.display()
                ));
            }
            forja_kernel::LoadResult::Missing => {}
        }
    }

    pub fn recent(&self) -> Vec<Project> {
        self.0
            .lock()
            .map(|state| state.recent.clone())
            .unwrap_or_default()
    }

    pub fn close(&self, cx: &mut Ctx<'_>) -> Result<(), String> {
        let Some(id) = self.active_id() else {
            return Ok(());
        };
        self.remove_open(cx, &id, false)
    }

    pub fn projects(&self) -> Vec<Project> {
        self.0
            .lock()
            .map(|state| state.projects.clone())
            .unwrap_or_default()
    }

    pub fn active_id(&self) -> Option<String> {
        self.0
            .lock()
            .ok()
            .and_then(|state| state.active_id.clone())
    }

    pub fn active(&self) -> Option<ProjectRef> {
        self.0.lock().ok().and_then(|state| {
            let active_id = state.active_id.as_deref()?;
            state
                .projects
                .iter()
                .find(|project| project.id == active_id)
                .map(reference)
        })
    }

    pub fn is_expanded(&self, id: &str) -> bool {
        self.0
            .lock()
            .map(|state| state.expanded.contains(id))
            .unwrap_or(false)
    }

    pub fn toggle_expanded(&self, id: &str) {
        if let Ok(mut state) = self.0.lock() {
            if !state.expanded.remove(id) {
                state.expanded.insert(id.to_owned());
            }
        }
    }

    pub fn dialog(&self) -> Dialog {
        self.0
            .lock()
            .map(|state| state.dialog.clone())
            .unwrap_or_default()
    }

    pub fn set_dialog(&self, dialog: Dialog) {
        if let Ok(mut state) = self.0.lock() {
            state.dialog = dialog;
        }
    }

    pub fn finish_dialog(&self, error: Option<String>) {
        if let Ok(mut state) = self.0.lock() {
            match &mut state.dialog {
                Dialog::Create(form) => {
                    form.busy = false;
                    form.error = error.clone();
                }
                Dialog::Clone(clone) => {
                    clone.form.busy = false;
                    clone.form.error = error.clone();
                }
                _ => {}
            }
            if error.is_none() {
                state.dialog = Dialog::None;
            }
        }
    }

    pub fn status(&self) -> Option<String> {
        self.0.lock().ok().and_then(|state| state.status.clone())
    }

    pub fn error(&self) -> Option<String> {
        self.0.lock().ok().and_then(|state| state.error.clone())
    }

    pub fn set_status(&self, value: Option<String>) {
        if let Ok(mut state) = self.0.lock() {
            state.status = value;
        }
    }

    pub fn set_error(&self, value: Option<String>) {
        if let Ok(mut state) = self.0.lock() {
            state.error = value;
        }
    }

    pub fn benches_missing(&self) -> bool {
        self.0
            .lock()
            .map(|state| state.benches_missing)
            .unwrap_or(false)
    }

    pub fn mark_benches_missing(&self, missing: bool) {
        if let Ok(mut state) = self.0.lock() {
            state.benches_missing = missing;
        }
    }

    fn save(&self, cx: &Ctx<'_>) -> Result<(), String> {
        let file = {
            let state = self.0.lock().map_err(|_| "project state is poisoned")?;
            ProjectsFile {
                schema_version: model::SCHEMA_VERSION,
                projects: state.projects.clone(),
                recent: state.recent.clone(),
                active_id: state.active_id.clone(),
            }
        };
        cx.store()
            .save(&file)
            .map_err(|error| format!("could not save projects: {error}"))
    }

    pub fn add(&self, cx: &mut Ctx<'_>, form: &NewProject) -> Result<Project, String> {
        let canonical = model::existing_folder(&form.path)?;
        {
            let state = self.0.lock().map_err(|_| "project state is poisoned")?;
            if state.projects.iter().any(|project| {
                model::normalize(std::path::Path::new(&project.path))
                    == model::normalize(&canonical)
            }) {
                return Err("this folder is already registered".to_owned());
            }
        }
        let project = {
            let state = self.0.lock().map_err(|_| "project state is poisoned")?;
            model::build_project(form, &canonical, state.projects.len())?
        };
        self.push_project(cx, project.clone())?;
        self.set_error(None);
        Ok(project)
    }

    /// Adds an already-validated record (used by the create and clone tasks).
    fn push_project(&self, cx: &mut Ctx<'_>, project: Project) -> Result<(), String> {
        let previous_active = {
            let mut state = self.0.lock().map_err(|_| "project state is poisoned")?;
            let previous_active = state.active_id.clone();
            state.recent.retain(|item| item.id != project.id);
            state.recent.insert(0, project.clone());
            state.recent.truncate(8);
            state.projects.push(project.clone());
            state.projects.sort_by_key(|a| a.name.to_lowercase());
            state.active_id = Some(project.id.clone());
            previous_active
        };
        if let Err(error) = self.save(cx) {
            if let Ok(mut state) = self.0.lock() {
                state.projects.retain(|existing| existing.id != project.id);
                state.active_id = previous_active;
            }
            return Err(error);
        }
        let reference = ProjectRef {
            id: project.id,
            name: project.name,
            path: project.path,
        };
        cx.emit(&ProjectAdded(reference.clone()));
        cx.emit(&ProjectActivated(reference));
        Ok(())
    }

    pub fn activate(&self, cx: &mut Ctx<'_>, id: &str) -> Result<ProjectRef, String> {
        let (changed, active) = {
            let mut state = self.0.lock().map_err(|_| "project state is poisoned")?;
            let project = state
                .projects
                .iter()
                .find(|project| project.id == id)
                .ok_or_else(|| "project not found".to_owned())?
                .clone();
            let changed = state.active_id.as_deref() != Some(id);
            state.active_id = Some(id.to_owned());
            (changed, reference(&project))
        };
        if changed {
            self.save(cx)?;
            cx.emit(&ProjectActivated(active.clone()));
        }
        Ok(active)
    }

    pub fn update(
        &self,
        cx: &mut Ctx<'_>,
        id: &str,
        name: Option<&str>,
        icon: Option<&str>,
        color: Option<&str>,
    ) -> Result<Project, String> {
        let updated = {
            let mut state = self.0.lock().map_err(|_| "project state is poisoned")?;
            let project = state
                .projects
                .iter_mut()
                .find(|project| project.id == id)
                .ok_or_else(|| "project not found".to_owned())?;
            if let Some(name) = name {
                if !name.trim().is_empty() {
                    project.name = name.trim().to_owned();
                }
            }
            if let Some(icon) = icon {
                project.icon = model::normalize_icon(icon);
            }
            if let Some(color) = color {
                project.color = model::normalize_color(color)
                    .ok_or_else(|| "color must be #RRGGBB".to_owned())?;
            }
            let updated = project.clone();
            if let Some(recent) = state.recent.iter_mut().find(|item| item.id == id) {
                *recent = updated.clone();
            }
            updated
        };
        self.save(cx)?;
        Ok(updated)
    }

    pub fn remove(&self, cx: &mut Ctx<'_>, id: &str) -> Result<(), String> {
        self.remove_open(cx, id, true)
    }

    fn remove_open(&self, cx: &mut Ctx<'_>, id: &str, forget: bool) -> Result<(), String> {
        let next_active = {
            let mut state = self.0.lock().map_err(|_| "project state is poisoned")?;
            state.projects.retain(|project| project.id != id);
            if forget {
                state.recent.retain(|project| project.id != id);
            }
            state.expanded.remove(id);
            if state.active_id.as_deref() == Some(id) {
                state.active_id = state.projects.first().map(|project| project.id.clone());
                state.projects.first().map(reference)
            } else {
                None
            }
        };
        self.save(cx)?;
        cx.emit(&ProjectRemoved {
            project_id: id.to_owned(),
        });
        if let Some(active) = next_active {
            cx.emit(&ProjectActivated(active));
        }
        Ok(())
    }
}

impl Default for Shared {
    fn default() -> Self {
        Self::new()
    }
}

pub fn reference(project: &Project) -> ProjectRef {
    ProjectRef {
        id: project.id.clone(),
        name: project.name.clone(),
        path: project.path.clone(),
    }
}

/// Creates a folder and initializes a git repository in it.
pub fn create_folder(path: &str) -> Result<std::path::PathBuf, String> {
    let path = std::path::PathBuf::from(path.trim());
    if path.as_os_str().is_empty() {
        return Err("path is empty".to_owned());
    }
    if path.exists() {
        let mut entries = std::fs::read_dir(&path)
            .map_err(|error| format!("could not read the folder: {error}"))?;
        if entries.next().is_some() {
            return Err("the folder is not empty".to_owned());
        }
    }
    std::fs::create_dir_all(&path)
        .map_err(|error| format!("could not create the folder: {error}"))?;
    let mut command = forja_kernel::process::command("git");
    command
        .arg("init")
        .current_dir(&path)
        .env("GIT_TERMINAL_PROMPT", "0")
        .stdin(std::process::Stdio::null());
    let output = command
        .output()
        .map_err(|error| format!("could not run git init: {error}"))?;
    if !output.status.success() {
        return Err(format!(
            "git init failed: {}",
            String::from_utf8_lossy(&output.stderr).trim()
        ));
    }
    path.canonicalize()
        .map(forja_kernel::paths::clean_path)
        .map_err(|error| format!("could not resolve the new folder: {error}"))
}

/// Clones a repository into `path`. Never prompts for credentials.
pub fn clone_repository(url: &str, path: &str) -> Result<std::path::PathBuf, String> {
    let path = std::path::PathBuf::from(path.trim());
    if url.trim().is_empty() || path.as_os_str().is_empty() {
        return Err("repository URL and destination are required".to_owned());
    }
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)
            .map_err(|error| format!("could not create the destination: {error}"))?;
    }
    let mut command = forja_kernel::process::command("git");
    command
        .arg("clone")
        .arg("--")
        .arg(url)
        .arg(&path)
        .env("GIT_TERMINAL_PROMPT", "0")
        .stdin(std::process::Stdio::null());
    let output = command
        .output()
        .map_err(|error| format!("could not run git clone: {error}"))?;
    if !output.status.success() {
        return Err(format!(
            "git clone failed: {}",
            String::from_utf8_lossy(&output.stderr).trim()
        ));
    }
    path.canonicalize()
        .map(forja_kernel::paths::clean_path)
        .map_err(|error| format!("could not resolve the clone: {error}"))
}
