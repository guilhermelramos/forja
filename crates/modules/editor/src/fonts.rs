//! Reads the code fonts named by `editor.font.family` from the system.
//!
//! The ui-kit installs Cascadia Code at startup. The other choices are loaded
//! on demand by the editor view through `egui::Context::add_font`, so a font
//! that is not installed simply falls back to the monospaced family.

use std::path::PathBuf;

/// Egui family name for a choice; `None` for `inherit` and unknown values.
pub fn family_name(choice: &str) -> Option<String> {
    let slug = match choice {
        "cascadia_code" => "cascadia_code",
        "cascadia_mono" => "cascadia_mono",
        "consolas" => "consolas",
        "jetbrains_mono" => "jetbrains_mono",
        _ => return None,
    };
    Some(format!("forja-editor-{slug}"))
}

/// Candidate files for a choice, most specific first.
pub fn candidates(choice: &str) -> Vec<PathBuf> {
    let files: &[&str] = match choice {
        "cascadia_code" => &["CascadiaCode.ttf"],
        "cascadia_mono" => &["CascadiaMono.ttf"],
        "consolas" => &["consola.ttf", "consola.ttf"],
        "jetbrains_mono" => &["JetBrainsMono-Regular.ttf", "JetBrainsMonoNL-Regular.ttf"],
        _ => return Vec::new(),
    };
    candidates_for(files)
}

/// Reads the first existing candidate, taking the user font folder into account.
pub fn read(choice: &str) -> Option<Vec<u8>> {
    candidates(choice)
        .into_iter()
        .find_map(|path| std::fs::read(path).ok().filter(|bytes| !bytes.is_empty()))
}

fn candidates_for(files: &[&str]) -> Vec<PathBuf> {
    let mut roots = Vec::new();
    if let Ok(windir) = std::env::var("WINDIR") {
        roots.push(PathBuf::from(windir).join("Fonts"));
    }
    if let Ok(local) = std::env::var("LOCALAPPDATA") {
        roots.push(
            PathBuf::from(local)
                .join("Microsoft")
                .join("Windows")
                .join("Fonts"),
        );
    }
    let mut paths = Vec::new();
    for root in roots {
        for file in files {
            paths.push(root.join(file));
        }
    }
    paths
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn known_choices_map_to_an_egui_family() {
        assert_eq!(
            family_name("cascadia_mono").as_deref(),
            Some("forja-editor-cascadia_mono")
        );
        assert_eq!(family_name("inherit"), None);
        assert_eq!(family_name("inventada"), None);
        assert!(candidates("consolas")
            .iter()
            .any(|path| path.ends_with("consola.ttf")));
    }
}
