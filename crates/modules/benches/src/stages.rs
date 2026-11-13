//! Stage vocabulary, extra stages from `.forja/etapas.toml` and the
//! transition rules of `design/stages.md`.

use forja_kernel::{Ctx, Locale};
use serde::Deserialize;

pub const DRAFT: &str = "draft";
pub const FORGING: &str = "forging";
pub const REVIEW: &str = "review";
pub const READY: &str = "ready";
pub const DOT_TOKENS: [&str; 6] = ["text_muted", "accent", "spark", "ok", "info", "danger"];

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct StageDef {
    pub id: String,
    pub label_key: Option<String>,
    pub label_pt: String,
    pub label_en: String,
    pub order: i32,
    pub parent: String,
    pub locked: bool,
    pub dot: String,
}

impl StageDef {
    pub fn label(&self, cx: &Ctx<'_>) -> String {
        if let Some(key) = &self.label_key {
            cx.t(key)
        } else if cx.i18n().locale() == Locale::En {
            self.label_en.clone()
        } else {
            self.label_pt.clone()
        }
    }

    fn builtin(id: &str, key: &str, pt: &str, en: &str, order: i32, dot: &str) -> Self {
        Self {
            id: id.to_owned(),
            label_key: Some(key.to_owned()),
            label_pt: pt.to_owned(),
            label_en: en.to_owned(),
            order,
            parent: id.to_owned(),
            locked: true,
            dot: dot.to_owned(),
        }
    }
}

pub fn builtins() -> Vec<StageDef> {
    vec![
        StageDef::builtin(
            DRAFT,
            "bench.stage.draft",
            "Rascunho",
            "Draft",
            10,
            "text_muted",
        ),
        StageDef::builtin(
            FORGING,
            "bench.stage.forging",
            "Forjando",
            "Forging",
            20,
            "accent",
        ),
        StageDef::builtin(
            REVIEW,
            "bench.stage.review",
            "Revisão",
            "Review",
            30,
            "spark",
        ),
        StageDef::builtin(READY, "bench.stage.ready", "Pronto", "Ready", 40, "ok"),
    ]
}

#[derive(Clone, Debug, Default, Deserialize)]
struct StagesFile {
    #[serde(default)]
    stage: Vec<RawStage>,
}

#[derive(Clone, Debug, Default, Deserialize)]
struct RawStage {
    #[serde(default)]
    id: String,
    #[serde(default)]
    label_pt: String,
    #[serde(default)]
    label_en: String,
    #[serde(default)]
    order: i32,
    #[serde(default)]
    parent: String,
    #[serde(default)]
    dot: String,
    #[serde(default)]
    locked: bool,
}

/// Reads `<project>/.forja/etapas.toml` on top of the four canonical stages.
/// The project file wins over the defaults; invalid entries are ignored.
pub fn load(project_path: Option<&str>) -> (Vec<StageDef>, Vec<String>) {
    let mut stages = builtins();
    let mut warnings = Vec::new();
    let Some(project_path) = project_path else {
        return (stages, warnings);
    };
    let path = std::path::Path::new(project_path)
        .join(".forja")
        .join("etapas.toml");
    let text = match std::fs::read_to_string(&path) {
        Ok(text) => text,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return (stages, warnings),
        Err(error) => {
            warnings.push(format!("{}: {error}", path.display()));
            return (stages, warnings);
        }
    };
    let file: StagesFile = match toml::from_str(text.strip_prefix('\u{feff}').unwrap_or(&text)) {
        Ok(file) => file,
        Err(error) => {
            warnings.push(format!("{}: {error}", path.display()));
            return (stages, warnings);
        }
    };
    for raw in file.stage {
        if raw.id.trim().is_empty() {
            warnings.push("etapas.toml: stage without id ignored".to_owned());
            continue;
        }
        let id = raw.id.trim().to_owned();
        let raw_id = id.clone();
        if !valid_id(&id) {
            warnings.push(format!("etapas.toml: invalid stage id `{id}` ignored"));
            continue;
        }
        let dot = if raw.dot.trim().is_empty() {
            "text_muted".to_owned()
        } else {
            raw.dot.trim().to_owned()
        };
        if !DOT_TOKENS.contains(&dot.as_str()) {
            warnings.push(format!("etapas.toml: `{id}` has an unknown dot `{dot}`"));
            continue;
        }
        if let Some(canonical) = stages.iter_mut().find(|stage| stage.id == id) {
            if raw.locked != canonical.locked {
                warnings.push(format!("etapas.toml: `{id}` is locked and cannot change"));
            }
            if raw.order > 0 {
                canonical.order = raw.order;
            }
            if !raw.label_pt.trim().is_empty() || !raw.label_en.trim().is_empty() {
                canonical.label_key = None;
                canonical.label_pt = raw.label_pt;
                canonical.label_en = raw.label_en;
            }
            canonical.dot = dot;
            continue;
        }
        if id == "archived" {
            warnings.push("etapas.toml: `archived` is not a stage".to_owned());
            continue;
        }
        let parent = raw.parent.trim();
        if !matches!(parent, DRAFT | FORGING | REVIEW | READY) {
            warnings.push(format!("etapas.toml: `{id}` has an invalid parent"));
            continue;
        }
        if raw.order <= 0 || stages.iter().any(|stage| stage.order == raw.order) {
            warnings.push(format!("etapas.toml: `{id}` has a duplicate order"));
            continue;
        }
        stages.push(StageDef {
            id,
            label_key: None,
            label_pt: if raw.label_pt.trim().is_empty() {
                raw_id.clone()
            } else {
                raw.label_pt
            },
            label_en: if raw.label_en.trim().is_empty() {
                raw_id
            } else {
                raw.label_en
            },
            order: raw.order,
            parent: parent.to_owned(),
            locked: false,
            dot,
        });
    }
    stages.sort_by_key(|stage| stage.order);
    (stages, warnings)
}

fn valid_id(id: &str) -> bool {
    !id.is_empty()
        && id.chars().all(|character| {
            character.is_ascii_lowercase() || character.is_ascii_digit() || character == '_'
        })
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum StageError {
    UnknownStage,
    BackwardNeedsConfirmation,
    ForbiddenBackward,
    ForgingToDraftWithCommits,
    ReadyRequiresReview,
}

impl StageError {
    pub fn key(self) -> &'static str {
        match self {
            Self::UnknownStage => "bench.stage.error.unknown",
            Self::BackwardNeedsConfirmation => "bench.stage.error.backward",
            Self::ForbiddenBackward => "bench.stage.error.forbidden",
            Self::ForgingToDraftWithCommits => "bench.stage.error.commits",
            Self::ReadyRequiresReview => "bench.stage.error.ready",
        }
    }
}

pub fn find<'a>(stages: &'a [StageDef], id: &str) -> Option<&'a StageDef> {
    stages.iter().find(|stage| stage.id == id)
}

/// Validates one transition. `has_new_commits` is only consulted for
/// `forging -> draft`, where the design forbids going back after Forja wrote
/// commits in the bench.
pub fn check(
    stages: &[StageDef],
    from: &str,
    to: &str,
    confirm_backward: bool,
    has_new_commits: bool,
) -> Result<(), StageError> {
    let Some(from) = find(stages, from) else {
        return Err(StageError::UnknownStage);
    };
    let Some(to) = find(stages, to) else {
        return Err(StageError::UnknownStage);
    };
    if from.id == to.id {
        return Ok(());
    }
    if to.order < from.order {
        if matches!(to.id.as_str(), DRAFT) && matches!(from.id.as_str(), REVIEW | READY) {
            return Err(StageError::ForbiddenBackward);
        }
        if from.id == FORGING && to.id == DRAFT && has_new_commits {
            return Err(StageError::ForgingToDraftWithCommits);
        }
        if !confirm_backward {
            return Err(StageError::BackwardNeedsConfirmation);
        }
        return Ok(());
    }
    if to.parent == READY && from.id != REVIEW && from.parent != READY {
        return Err(StageError::ReadyRequiresReview);
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn forward_and_backward_rules_follow_the_table() {
        let stages = builtins();
        assert!(check(&stages, DRAFT, FORGING, false, false).is_ok());
        assert_eq!(
            check(&stages, DRAFT, READY, false, false),
            Err(StageError::ReadyRequiresReview)
        );
        assert_eq!(
            check(&stages, READY, DRAFT, true, false),
            Err(StageError::ForbiddenBackward)
        );
        assert_eq!(
            check(&stages, REVIEW, FORGING, false, false),
            Err(StageError::BackwardNeedsConfirmation)
        );
        assert!(check(&stages, REVIEW, FORGING, true, false).is_ok());
        assert_eq!(
            check(&stages, FORGING, DRAFT, true, true),
            Err(StageError::ForgingToDraftWithCommits)
        );
        assert!(check(&stages, FORGING, DRAFT, true, false).is_ok());
    }

    #[test]
    fn extras_load_and_keep_the_linear_rule() {
        let root = std::env::temp_dir().join(format!(
            "forja-stages-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        std::fs::create_dir_all(root.join(".forja")).unwrap();
        std::fs::write(
            root.join(".forja/etapas.toml"),
            "[[stage]]\nid = \"qa\"\nlabel_pt = \"QA\"\nlabel_en = \"QA\"\norder = 25\nparent = \"review\"\ndot = \"info\"\n",
        )
        .unwrap();
        let (stages, warnings) = load(Some(&root.to_string_lossy()));
        assert!(warnings.is_empty(), "{warnings:?}");
        assert_eq!(stages.len(), 5);
        assert_eq!(find(&stages, "qa").unwrap().parent, "review");
        assert!(check(&stages, FORGING, "qa", false, false).is_ok());
        let _ = std::fs::remove_dir_all(root);
    }
}
