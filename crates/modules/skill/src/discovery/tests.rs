//! Testes de descoberta: dono e escopo, front matter bom e ruim, pastas que não são skill.

use super::*;
use crate::model::{Scope, FORJA_OWNER};
use std::fs;

fn temp(name: &str) -> PathBuf {
    let nanos = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_nanos())
        .unwrap_or(0);
    let dir = std::env::temp_dir().join(format!("forja-skill-scan-{name}-{}-{nanos}", std::process::id()));
    fs::create_dir_all(&dir).expect("temp");
    dir
}

fn root(owner: &str, scope: Scope, path: PathBuf) -> Root {
    Root {
        owner: owner.to_owned(),
        owner_name: owner.to_uppercase(),
        scope,
        path,
    }
}

fn skill_file(dir: &Path, text: &str) {
    fs::create_dir_all(dir).expect("skill dir");
    fs::write(dir.join("SKILL.md"), text).expect("skill file");
}

#[test]
fn skills_are_listed_by_owner_and_scope_in_folder_order() {
    let work = temp("owners");
    let forja_user = work.join("forja-user");
    let claude_project = work.join("claude-project");
    skill_file(&forja_user.join("b-skill"), "---\nname: b-skill\ndescription: segunda\n---\n");
    skill_file(&forja_user.join("a-skill"), "---\nname: a-skill\ndescription: primeira\n---\n");
    skill_file(&claude_project.join("so-do-projeto"), "---\nname: so-do-projeto\ndescription: x\n---\n");

    let roots = vec![
        root(FORJA_OWNER, Scope::User, forja_user),
        root("claude-code", Scope::Project, claude_project),
    ];
    let listing = scan_all(&roots, Some("/proj".to_owned()));
    let folders: Vec<(&str, Scope, &str)> = listing
        .skills
        .iter()
        .map(|s| (s.owner.as_str(), s.scope, s.folder.as_str()))
        .collect();
    assert_eq!(
        folders,
        [
            (FORJA_OWNER, Scope::User, "a-skill"),
            (FORJA_OWNER, Scope::User, "b-skill"),
            ("claude-code", Scope::Project, "so-do-projeto"),
        ]
    );
    assert_eq!(listing.skills[0].name.as_deref(), Some("a-skill"));
    assert_eq!(listing.skills[0].description.as_deref(), Some("primeira"));
    assert_eq!(listing.project.as_deref(), Some("/proj"));
    assert!(listing.roots.iter().all(|r| r.exists));
    let _ = fs::remove_dir_all(work);
}

#[test]
fn a_malformed_skill_is_listed_with_its_error() {
    let work = temp("malformed");
    let user = work.join("user");
    skill_file(&user.join("sem-cabecalho"), "# só markdown\n");
    skill_file(&user.join("nome-ruim"), "---\nname: Nome Ruim\ndescription: x\n---\n");
    skill_file(&user.join("ok"), "---\nname: ok\ndescription: certa\n---\n");

    let listing = scan_all(&[root(FORJA_OWNER, Scope::User, user)], None);
    let by_folder = |folder: &str| listing.skills.iter().find(|s| s.folder == folder).expect("listed");
    assert_eq!(by_folder("sem-cabecalho").error.as_deref(), Some("skill-error-frontmatter-missing"));
    assert_eq!(by_folder("sem-cabecalho").name, None);
    assert_eq!(by_folder("nome-ruim").error.as_deref(), Some("skill-error-name-invalid"));
    assert_eq!(by_folder("ok").error, None);
    let _ = fs::remove_dir_all(work);
}

#[test]
fn folders_without_skill_md_and_hidden_folders_are_not_skills() {
    let work = temp("not-skills");
    let user = work.join("user");
    fs::create_dir_all(user.join("so-uma-pasta")).expect("plain");
    skill_file(&user.join(".temporaria"), "---\nname: x\ndescription: y\n---\n");
    fs::write(user.join("solto.md"), "arquivo, não pasta").expect("file");
    skill_file(&user.join("real"), "---\nname: real\ndescription: ok\n---\n");

    let listing = scan_all(&[root(FORJA_OWNER, Scope::User, user)], None);
    let folders: Vec<&str> = listing.skills.iter().map(|s| s.folder.as_str()).collect();
    assert_eq!(folders, ["real"]);
    let _ = fs::remove_dir_all(work);
}

#[test]
fn a_missing_root_is_reported_and_empty() {
    let work = temp("missing");
    let listing = scan_all(&[root(FORJA_OWNER, Scope::User, work.join("nao-existe"))], None);
    assert!(listing.skills.is_empty());
    assert_eq!(listing.roots.len(), 1);
    assert!(!listing.roots[0].exists);
    let _ = fs::remove_dir_all(work);
}

#[test]
fn an_oversized_skill_file_is_not_read() {
    let work = temp("big");
    let user = work.join("user");
    let big = format!("---\nname: big\ndescription: x\n---\n{}", "a".repeat(300 * 1024));
    skill_file(&user.join("big"), &big);
    let listing = scan_all(&[root(FORJA_OWNER, Scope::User, user)], None);
    assert_eq!(listing.skills[0].error.as_deref(), Some("skill-error-too-large"));
    let _ = fs::remove_dir_all(work);
}
