//! Testes do módulo inteiro: contrato do kernel, recusas síncronas dos comandos e a forma da foto.
//! Nenhum teste lê as pastas reais de ajudantes nem grava fora de uma pasta temporária, e nenhum
//! chama `skill.list` (que começa uma leitura do disco).

use super::*;
use crate::model::{Root, Scope, FORJA_OWNER};
use crate::{actions, fixtures};
use forja_kernel::{contract, Runtime, RuntimeOptions};
use serde_json::Map;
use std::path::PathBuf;

fn temp_dir(name: &str) -> PathBuf {
    let nanos = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|duration| duration.as_nanos())
        .unwrap_or(0);
    std::env::temp_dir().join(format!("forja-skill-module-{name}-{}-{nanos}", std::process::id()))
}

fn runtime(dir: &std::path::Path) -> Runtime {
    Runtime::new(
        vec![Box::new(SkillModule::new())],
        RuntimeOptions {
            data_dir: Some(dir.to_path_buf()),
            user_dir: Some(dir.join("user")),
            locale: Some("pt-BR".to_owned()),
            wake: None,
        },
    )
    .expect("module registers cleanly")
}

fn args(pairs: &[(&str, serde_json::Value)]) -> Args {
    let mut map = Map::new();
    for (key, value) in pairs {
        map.insert((*key).to_owned(), value.clone());
    }
    Args::from_map(map)
}

#[test]
fn the_module_passes_the_kernel_contract() {
    let dir = temp_dir("contract");
    let runtime = runtime(&dir);
    contract::assert_ok(runtime.registry(), runtime.i18n());
    let _ = std::fs::remove_dir_all(dir);
}

#[test]
fn install_without_a_listed_root_is_refused_before_any_disk_work() {
    let dir = temp_dir("no-root");
    let mut runtime = runtime(&dir);
    let error = runtime
        .run_command(
            "skill.install",
            args(&[
                ("source", serde_json::json!("/nao/existe")),
                ("owner", serde_json::json!("forja")),
                ("scope", serde_json::json!("user")),
            ]),
        )
        .expect_err("refused");
    assert!(error.to_string().contains("skill-error-no-root"), "{error}");
    let _ = std::fs::remove_dir_all(dir);
}

#[test]
fn install_rejects_an_unknown_kind_and_scope() {
    let dir = temp_dir("bad-args");
    let mut runtime = runtime(&dir);
    let kind = runtime
        .run_command(
            "skill.install",
            args(&[
                ("source", serde_json::json!("x")),
                ("kind", serde_json::json!("tar")),
                ("owner", serde_json::json!("forja")),
                ("scope", serde_json::json!("user")),
            ]),
        )
        .expect_err("kind refused");
    assert!(kind.to_string().contains("folder or zip"), "{kind}");
    let scope = runtime
        .run_command(
            "skill.install",
            args(&[
                ("source", serde_json::json!("x")),
                ("owner", serde_json::json!("forja")),
                ("scope", serde_json::json!("global")),
            ]),
        )
        .expect_err("scope refused");
    assert!(scope.to_string().contains("user or project"), "{scope}");
    let _ = std::fs::remove_dir_all(dir);
}

#[test]
fn removing_a_path_that_is_not_listed_is_refused() {
    let dir = temp_dir("unknown");
    let mut runtime = runtime(&dir);
    let error = runtime
        .run_command(
            "skill.remove",
            args(&[("path", serde_json::json!("/qualquer/pasta"))]),
        )
        .expect_err("refused");
    assert!(error.to_string().contains("skill-error-unknown-skill"), "{error}");
    let _ = std::fs::remove_dir_all(dir);
}

#[test]
fn writing_to_another_helper_needs_consent() {
    let codex = Root {
        owner: "codex".to_owned(),
        owner_name: "Codex".to_owned(),
        scope: Scope::User,
        path: PathBuf::from("/exemplo/.codex/skills"),
    };
    let forja = Root {
        owner: FORJA_OWNER.to_owned(),
        owner_name: "Forja".to_owned(),
        scope: Scope::User,
        path: PathBuf::from("/exemplo/.forja/skills"),
    };
    assert_eq!(
        actions::require_consent(&codex, false).expect_err("needs consent").key,
        "skill-error-consent"
    );
    assert!(actions::require_consent(&codex, true).is_ok());
    assert!(actions::require_consent(&forja, false).is_ok(), "the Forja's own folders need no consent");
}

#[test]
fn the_snapshot_of_an_unread_module_is_empty_and_well_formed() {
    let shared = state::Shared::new();
    let value = commands::snapshot(&shared);
    assert_eq!(value["skills"], serde_json::json!([]));
    assert_eq!(value["roots"], serde_json::json!([]));
    assert_eq!(value["scanned"], serde_json::json!(false));
    assert_eq!(value["notice"], serde_json::Value::Null);
}

#[test]
fn the_fixture_listing_shows_every_owner_and_one_broken_skill() {
    let listing = fixtures::listing();
    let owners: std::collections::BTreeSet<&str> = listing.roots.iter().map(|r| r.owner.as_str()).collect();
    assert!(owners.contains(FORJA_OWNER));
    assert!(owners.contains("claude-code"));
    assert!(owners.contains("codex"));
    assert!(listing.skills.iter().any(|s| s.error.is_some()), "one skill shows a front matter error");
    assert!(listing.skills.iter().any(|s| s.scope == Scope::Project));
    assert!(fixtures::body("/x/revisar-pr").contains("name: revisar-pr"));
}
