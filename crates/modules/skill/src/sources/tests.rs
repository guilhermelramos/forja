//! Testes das fontes: os perfis embutidos e a montagem das raízes.

use super::*;

fn helper(id: &str, user: &[&str], project: &[&str]) -> HelperDirs {
    HelperDirs {
        id: id.to_owned(),
        name: id.to_uppercase(),
        user: user.iter().map(|s| (*s).to_owned()).collect(),
        project: project.iter().map(|s| (*s).to_owned()).collect(),
    }
}

#[test]
fn the_confirmed_profiles_declare_their_skill_folders() {
    let dirs = helper_dirs();
    let claude = dirs.iter().find(|d| d.id == "claude-code").expect("claude-code profile");
    assert_eq!(claude.user, vec!["~/.claude/skills"]);
    assert_eq!(claude.project, vec![".claude/skills"]);
    let codex = dirs.iter().find(|d| d.id == "codex").expect("codex profile");
    assert_eq!(codex.user, vec!["~/.codex/skills"]);
    assert_eq!(codex.project, vec![".agents/skills"]);
}

#[test]
fn profiles_not_confirmed_yet_declare_no_folders() {
    let dirs = helper_dirs();
    let aider = dirs.iter().find(|d| d.id == "aider").expect("aider profile");
    assert!(aider.user.is_empty());
    assert!(aider.project.is_empty());
}

#[test]
fn every_embedded_profile_is_read() {
    assert_eq!(helper_dirs().len(), 16);
}

#[test]
fn the_forja_roots_come_first_and_the_project_one_only_with_a_project() {
    let user = Path::new("/casa/.forja");
    let roots = build_roots(user, Some(Path::new("/casa")), None, &[]);
    assert_eq!(roots.len(), 1);
    assert_eq!(roots[0].owner, FORJA_OWNER);
    assert_eq!(roots[0].scope, Scope::User);
    assert_eq!(roots[0].path, user.join("skills"));

    let roots = build_roots(user, None, Some(Path::new("/proj")), &[]);
    assert_eq!(roots.len(), 2);
    assert_eq!(roots[1].scope, Scope::Project);
    assert_eq!(roots[1].path, Path::new("/proj").join(".forja").join("skills"));
}

#[test]
fn helper_user_dirs_hang_from_home_and_project_dirs_from_the_project() {
    let home = Path::new("/casa");
    let project = Path::new("/proj");
    let helpers = [helper("claude-code", &["~/.claude/skills"], &[".claude/skills"])];
    let roots = build_roots(Path::new("/casa/.forja"), Some(home), Some(project), &helpers);
    let claude: Vec<&Root> = roots.iter().filter(|r| r.owner == "claude-code").collect();
    assert_eq!(claude.len(), 2);
    assert_eq!(claude[0].scope, Scope::User);
    assert_eq!(claude[0].path, home.join(".claude").join("skills"));
    assert_eq!(claude[1].scope, Scope::Project);
    assert_eq!(claude[1].path, project.join(".claude").join("skills"));
    assert_eq!(claude[0].owner_name, "CLAUDE-CODE");
}

#[test]
fn without_home_or_project_the_helper_roots_are_skipped() {
    let helpers = [helper("x", &["~/.x"], &[".x"])];
    let roots = build_roots(Path::new("/f"), None, None, &helpers);
    assert_eq!(roots.len(), 1);
}

#[test]
fn join_relative_accepts_slashes_from_the_profiles() {
    let joined = join_relative(Path::new("/base"), "a/b//c");
    assert_eq!(joined, Path::new("/base").join("a").join("b").join("c"));
}
