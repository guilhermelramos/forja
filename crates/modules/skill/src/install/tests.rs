//! Testes de instalação, cópia, remoção e zip. Tudo em pastas temporárias; a lixeira é falsa.

use super::*;
use std::cell::RefCell;
use std::io::Write;

/// Lixeira falsa: registra o que mandaram e apaga a pasta (como a lixeira faria).
#[derive(Default)]
struct FakeTrash {
    trashed: RefCell<Vec<PathBuf>>,
}

impl Trasher for FakeTrash {
    fn trash(&self, path: &Path) -> Result<(), String> {
        self.trashed.borrow_mut().push(path.to_path_buf());
        fs::remove_dir_all(path).map_err(|error| error.to_string())
    }
}

fn temp(name: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("forja-skill-{name}-{}", unique_suffix()));
    fs::create_dir_all(&dir).expect("temp dir");
    dir
}

fn write(path: &Path, text: &str) {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).expect("parent");
    }
    fs::write(path, text).expect("write");
}

fn skill_text(name: &str) -> String {
    format!("---\nname: {name}\ndescription: Uma skill de teste.\n---\n# {name}\n")
}

fn make_skill_folder(root: &Path, folder: &str, name: &str) -> PathBuf {
    let dir = root.join(folder);
    write(&dir.join("SKILL.md"), &skill_text(name));
    write(&dir.join("notes").join("extra.md"), "apoio\n");
    dir
}

/// Monta um zip com as entradas dadas (nome, conteúdo). Nomes são gravados como estão.
fn make_zip(path: &Path, entries: &[(&str, &str)]) {
    let file = fs::File::create(path).expect("zip file");
    let mut zip = zip::ZipWriter::new(file);
    for (name, body) in entries {
        zip.start_file(*name, zip::write::SimpleFileOptions::default())
            .expect("start entry");
        zip.write_all(body.as_bytes()).expect("write entry");
    }
    zip.finish().expect("finish zip");
}

#[test]
fn a_folder_with_a_valid_skill_is_installed_as_a_new_skill() {
    let work = temp("install-new");
    let source = make_skill_folder(&work, "pdf-tools", "pdf-tools");
    let target_root = work.join("destino");
    let trash = FakeTrash::default();
    let placed = install_folder(&source, "pdf-tools", &target_root, false, &trash).expect("installed");
    assert_eq!(placed, Placed::Installed(target_root.join("pdf-tools")));
    assert!(target_root.join("pdf-tools").join("SKILL.md").is_file());
    assert!(target_root.join("pdf-tools").join("notes").join("extra.md").is_file());
    assert!(trash.trashed.borrow().is_empty());
    let _ = fs::remove_dir_all(work);
}

#[test]
fn an_existing_skill_is_a_conflict_and_nothing_changes() {
    let work = temp("install-conflict");
    let source = make_skill_folder(&work, "pdf-tools", "pdf-tools");
    let target_root = work.join("destino");
    let existing = make_skill_folder(&target_root, "pdf-tools", "pdf-tools");
    write(&existing.join("SKILL.md"), &skill_text("versao-antiga"));
    let trash = FakeTrash::default();

    let placed = install_folder(&source, "pdf-tools", &target_root, false, &trash).expect("no error");
    assert_eq!(placed, Placed::Conflict(target_root.join("pdf-tools")));
    let kept = fs::read_to_string(existing.join("SKILL.md")).expect("still there");
    assert!(kept.contains("versao-antiga"), "the old version must stay");
    assert!(trash.trashed.borrow().is_empty());
    let _ = fs::remove_dir_all(work);
}

#[test]
fn overwriting_sends_the_old_version_to_the_trash_first() {
    let work = temp("install-overwrite");
    let source = make_skill_folder(&work, "pdf-tools", "pdf-tools");
    let target_root = work.join("destino");
    let existing = make_skill_folder(&target_root, "pdf-tools", "pdf-tools");
    write(&existing.join("SKILL.md"), &skill_text("versao-antiga"));
    write(&existing.join("so-no-antigo.md"), "x");
    let trash = FakeTrash::default();

    let placed = install_folder(&source, "pdf-tools", &target_root, true, &trash).expect("overwritten");
    assert_eq!(placed, Placed::Installed(target_root.join("pdf-tools")));
    assert_eq!(trash.trashed.borrow().as_slice(), [target_root.join("pdf-tools")]);
    assert!(!target_root.join("pdf-tools").join("so-no-antigo.md").exists());
    let now = fs::read_to_string(target_root.join("pdf-tools").join("SKILL.md")).expect("new");
    assert!(now.contains("pdf-tools"));
    let _ = fs::remove_dir_all(work);
}

#[test]
fn a_folder_without_skill_md_is_refused() {
    let work = temp("install-no-file");
    let source = work.join("vazia");
    fs::create_dir_all(&source).expect("dir");
    let error = install_folder(&source, "vazia", &work.join("destino"), false, &FakeTrash::default())
        .expect_err("refused");
    assert_eq!(error.key, "skill-error-no-skill-file");
    let _ = fs::remove_dir_all(work);
}

#[test]
fn malformed_front_matter_stops_the_install() {
    let work = temp("install-bad-front");
    let source = work.join("ruim");
    write(&source.join("SKILL.md"), "sem front matter\n");
    let target_root = work.join("destino");
    let error = install_folder(&source, "ruim", &target_root, false, &FakeTrash::default())
        .expect_err("refused");
    assert_eq!(error.key, "skill-error-frontmatter-missing");
    assert!(!target_root.join("ruim").exists());
    let _ = fs::remove_dir_all(work);
}

#[test]
fn a_zip_with_the_skill_at_its_root_takes_the_zip_name() {
    let work = temp("zip-root");
    let archive = work.join("Minha Skill.zip");
    make_zip(&archive, &[("SKILL.md", &skill_text("minha-skill")), ("apoio.md", "x")]);
    let target_root = work.join("destino");
    let staging = work.join("staging");
    let placed = install_zip(&archive, &staging, &target_root, false, &FakeTrash::default()).expect("installed");
    assert_eq!(placed, Placed::Installed(target_root.join("Minha Skill")));
    assert!(target_root.join("Minha Skill").join("apoio.md").is_file());
    assert!(!staging.exists() || fs::read_dir(&staging).expect("staging").next().is_none());
    let _ = fs::remove_dir_all(work);
}

#[test]
fn a_zip_with_one_top_folder_installs_that_folder() {
    let work = temp("zip-folder");
    let archive = work.join("pacote.zip");
    make_zip(
        &archive,
        &[
            ("revisao/SKILL.md", &skill_text("revisao")),
            ("revisao/exemplos/a.md", "a"),
        ],
    );
    let target_root = work.join("destino");
    let placed = install_zip(&archive, &work.join("staging"), &target_root, false, &FakeTrash::default())
        .expect("installed");
    assert_eq!(placed, Placed::Installed(target_root.join("revisao")));
    assert!(target_root.join("revisao").join("exemplos").join("a.md").is_file());
    let _ = fs::remove_dir_all(work);
}

#[test]
fn a_zip_entry_that_leaves_the_folder_refuses_the_whole_install() {
    let work = temp("zip-slip");
    let archive = work.join("mau.zip");
    make_zip(
        &archive,
        &[("SKILL.md", &skill_text("mau")), ("../fora.txt", "escapou")],
    );
    let target_root = work.join("destino");
    let error = install_zip(&archive, &work.join("staging"), &target_root, false, &FakeTrash::default())
        .expect_err("refused");
    assert_eq!(error.key, "skill-error-unsafe-entry");
    assert!(!work.join("fora.txt").exists(), "nothing may be written outside the staging folder");
    assert!(!target_root.join("mau").exists());
    let _ = fs::remove_dir_all(work);
}

#[test]
fn a_zip_without_a_skill_is_refused() {
    let work = temp("zip-none");
    let archive = work.join("sem.zip");
    make_zip(&archive, &[("readme.md", "oi")]);
    let error = install_zip(&archive, &work.join("staging"), &work.join("destino"), false, &FakeTrash::default())
        .expect_err("refused");
    assert_eq!(error.key, "skill-error-no-skill-in-zip");
    let _ = fs::remove_dir_all(work);
}

#[test]
fn safe_relative_refuses_every_way_out_of_the_folder() {
    for bad in ["../x", "a/../../x", "/etc/x", "\\windows\\x", "C:/x", "a:b", "", "./", "a\0b"] {
        assert_eq!(safe_relative(bad), None, "{bad:?} must be refused");
    }
    for bad in ["..\\..\\x", "a\\..\\..\\x"] {
        assert_eq!(safe_relative(bad), None, "{bad:?} must be refused");
    }
    assert_eq!(safe_relative("a/b/c.md"), Some(PathBuf::from("a").join("b").join("c.md")));
    assert_eq!(safe_relative("a\\b.md"), Some(PathBuf::from("a").join("b.md")));
    assert_eq!(safe_relative("./a//b"), Some(PathBuf::from("a").join("b")));
}

#[test]
fn folder_names_must_be_plain() {
    assert!(valid_folder("pdf-tools"));
    assert!(valid_folder("Minha Skill"));
    for bad in ["", ".", "..", ".oculta", "a/b", "a\\b", "a:b", "a*b", "a?b"] {
        assert!(!valid_folder(bad), "{bad:?} must be refused");
    }
    assert_eq!(folder_name_from("Minha: Skill?"), "Minha- Skill");
    assert_eq!(folder_name_from("..."), "skill");
}

#[cfg(windows)]
#[test]
fn a_symbolic_link_inside_the_source_is_refused() {
    let work = temp("install-link");
    let source = make_skill_folder(&work, "com-link", "com-link");
    let outside = work.join("fora");
    fs::create_dir_all(&outside).expect("outside");
    if std::os::windows::fs::symlink_dir(&outside, source.join("atalho")).is_err() {
        // Sem permissão para criar link (modo desenvolvedor desligado): nada a testar aqui.
        let _ = fs::remove_dir_all(work);
        return;
    }
    let error = install_folder(&source, "com-link", &work.join("destino"), false, &FakeTrash::default())
        .expect_err("refused");
    assert_eq!(error.key, "skill-error-symlink");
    let _ = fs::remove_dir_all(work);
}

#[test]
fn removing_only_touches_folders_that_are_skills() {
    let work = temp("remove");
    let trash = FakeTrash::default();
    let plain = work.join("solta");
    fs::create_dir_all(&plain).expect("plain");
    assert_eq!(
        remove_folder(&plain, &trash).expect_err("refused").key,
        "skill-error-not-a-skill"
    );
    let skill = make_skill_folder(&work, "remover-me", "remover-me");
    remove_folder(&skill, &trash).expect("removed");
    assert_eq!(trash.trashed.borrow().as_slice(), std::slice::from_ref(&skill));
    assert!(!skill.exists());
    let _ = fs::remove_dir_all(work);
}

#[test]
fn copying_a_skill_to_another_helper_is_an_install_there() {
    let work = temp("copy");
    let claude = make_skill_folder(&work.join("claude"), "revisar", "revisar");
    let codex_root = work.join("codex");
    let placed = install_folder(&claude, "revisar", &codex_root, false, &FakeTrash::default()).expect("copied");
    assert_eq!(placed, Placed::Installed(codex_root.join("revisar")));
    assert!(codex_root.join("revisar").join("notes").join("extra.md").is_file());
    assert!(claude.join("SKILL.md").is_file(), "the source stays where it was");
    let _ = fs::remove_dir_all(work);
}
