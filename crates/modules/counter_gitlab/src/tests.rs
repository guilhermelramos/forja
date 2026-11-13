//! Testes de integração do módulo `counter_gitlab`: registro, comandos e fixtures.

use super::*;
use crate::ops::{Config, Env, MODULE};
use forja_contracts::{
    CommentArgs, ConnectArgs, CounterError, CounterOp, ItemArgs, ListArgs, RequestScope,
    RequestState, TransitionArgs,
};
use forja_counter_kit::{service_for, FakeTransport, HttpResponse, MemorySecretStore, SecretStore};
use forja_kernel::{contract, Args, Runtime, RuntimeOptions};
use std::path::PathBuf;

const ISSUES: &str = include_str!("../tests/fixtures/issues.json");
const MERGE_REQUESTS: &str = include_str!("../tests/fixtures/merge_requests.json");
const DETAIL: &str = include_str!("../tests/fixtures/issue_detail.json");
const NOTES: &str = include_str!("../tests/fixtures/notes.json");
const USER: &str = include_str!("../tests/fixtures/user.json");

fn runtime_with(env: Env) -> Runtime {
    let dir = std::env::temp_dir().join(format!("forja-counter-gitlab-rt-{}", std::process::id()));
    Runtime::new(
        vec![Box::new(GitlabCounter::with_env(env))],
        RuntimeOptions {
            data_dir: Some(dir.clone()),
            user_dir: Some(dir.join("user")),
            locale: Some("pt-BR".to_owned()),
            wake: None,
        },
    )
    .expect("counter_gitlab registers cleanly")
}

#[test]
fn contract_holds_and_describe_is_io_free() {
    let env = Env {
        transport: Arc::new(FakeTransport::new()),
        secrets: Arc::new(MemorySecretStore::new()),
    };
    let mut runtime = runtime_with(env);
    contract::assert_ok(runtime.registry(), runtime.i18n());
    let value = runtime.run_command("counter_gitlab.describe", Args::new()).unwrap();
    assert_eq!(value["counter"], "gitlab");
}

/// Pasta temporária com um `origin` apontando para `url`.
fn project(url: &str, label: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!(
        "forja-counter-gitlab-{label}-{}-{}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_nanos())
            .unwrap_or(0)
    ));
    std::fs::create_dir_all(&dir).unwrap();
    for args in [vec!["init", "-q"], vec!["remote", "add", "origin", url]] {
        let status = std::process::Command::new("git")
            .arg("-C")
            .arg(&dir)
            .args(&args)
            .status()
            .expect("git is available for tests");
        assert!(status.success());
    }
    dir
}

fn config() -> Config {
    Config {
        host: "gitlab.com".into(),
    }
}

fn connected() -> Arc<MemorySecretStore> {
    let secrets = Arc::new(MemorySecretStore::new());
    secrets.set(&service_for(MODULE), "gitlab.com", "glpat_test").unwrap();
    secrets.set(&service_for(MODULE), "gitlab.com#login", "ana.dev").unwrap();
    secrets
}

fn env(fake: &Arc<FakeTransport>, secrets: &Arc<MemorySecretStore>) -> Env {
    Env {
        transport: fake.clone(),
        secrets: secrets.clone(),
    }
}

fn list_args(dir: &std::path::Path, scope: RequestScope) -> ListArgs {
    ListArgs {
        project_path: dir.display().to_string(),
        scope,
        op_id: None,
        query: None,
    }
}

#[test]
fn list_merges_issues_and_merge_requests_newest_first() {
    let dir = project("https://gitlab.com/acme/tools.git", "merge");
    let fake = Arc::new(
        FakeTransport::new()
            .reply(HttpResponse::new(200, ISSUES))
            .reply(HttpResponse::new(200, MERGE_REQUESTS)),
    );
    let secrets = connected();
    let result = ops::list(&env(&fake, &secrets), &config(), list_args(&dir, RequestScope::Open));
    assert_eq!(result.error, None);
    let keys: Vec<&str> = result.items.iter().map(|item| item.key.as_str()).collect();
    assert_eq!(keys[0], "!21", "updated most recently");
    assert!(keys.contains(&"#12") && keys.contains(&"!19"));
    let requests = fake.requests();
    assert!(requests[0].url.contains("/projects/acme%2Ftools/issues?state=opened"));
    assert!(requests[1].url.contains("/merge_requests?state=opened"));
    assert_eq!(requests[0].header_value("PRIVATE-TOKEN"), Some("glpat_test"));
    let _ = std::fs::remove_dir_all(dir);
}

#[test]
fn pages_follow_x_next_page() {
    let dir = project("https://gitlab.com/acme/tools.git", "pages");
    let first = HttpResponse::new(200, r#"[{"iid":1,"title":"a","state":"opened"}]"#)
        .with_headers(vec![("X-Next-Page", "2")]);
    let second = HttpResponse::new(200, r#"[{"iid":2,"title":"b","state":"opened"}]"#)
        .with_headers(vec![("X-Next-Page", "")]);
    let fake = Arc::new(FakeTransport::new().reply(first).reply(second).reply(HttpResponse::new(200, "[]")));
    let secrets = connected();
    let result = ops::list(&env(&fake, &secrets), &config(), list_args(&dir, RequestScope::Open));
    assert_eq!(result.items.len(), 2);
    let requests = fake.requests();
    assert!(requests[1].url.contains("page=2"));
    let _ = std::fs::remove_dir_all(dir);
}

#[test]
fn review_lists_only_merge_requests_asked_of_the_login() {
    let dir = project("https://gitlab.com/acme/tools.git", "review");
    let fake = Arc::new(FakeTransport::new().reply(HttpResponse::new(200, MERGE_REQUESTS)));
    let secrets = connected();
    let result = ops::list(&env(&fake, &secrets), &config(), list_args(&dir, RequestScope::ToReview));
    assert_eq!(fake.requests().len(), 1, "issues have no review scope");
    assert!(fake.last_request().unwrap().url.contains("reviewer_username=ana.dev"));
    assert!(result.items.iter().all(|item| item.kind == forja_contracts::RequestKind::Change));
    let _ = std::fs::remove_dir_all(dir);
}

#[test]
fn gitlab_errors_map_to_their_kinds() {
    let dir = project("https://gitlab.com/acme/tools.git", "errors");
    let secrets = connected();
    let cases = vec![
        (HttpResponse::new(401, "401 Unauthorized"), CounterError::Unauthorized),
        (HttpResponse::new(403, "403 Forbidden"), CounterError::Forbidden),
        (HttpResponse::new(404, "404 Project Not Found"), CounterError::NotFound),
        (
            HttpResponse::new(429, "Retry later").with_headers(vec![("Retry-After", "7")]),
            CounterError::RateLimited {
                retry_after_s: Some(7),
            },
        ),
    ];
    for (response, expected) in cases {
        let fake = Arc::new(FakeTransport::new().reply(response));
        let result = ops::list(&env(&fake, &secrets), &config(), list_args(&dir, RequestScope::Open));
        assert_eq!(result.error, Some(expected));
    }
    let _ = std::fs::remove_dir_all(dir);
}

#[test]
fn get_drops_system_notes() {
    let dir = project("https://gitlab.com/acme/tools.git", "get");
    let fake = Arc::new(
        FakeTransport::new()
            .reply(HttpResponse::new(200, DETAIL))
            .reply(HttpResponse::new(200, NOTES)),
    );
    let secrets = connected();
    let args = ItemArgs {
        project_path: dir.display().to_string(),
        key: "#12".into(),
        op_id: None,
    };
    let result = ops::get(&env(&fake, &secrets), &config(), args);
    let item = result.item.expect("item");
    assert_eq!(item.comments.len(), 1);
    assert_eq!(item.comments[0].author, "carlos");
    assert!(item.mine);
    assert!(fake.requests()[1].url.ends_with("/issues/12/notes?sort=asc&per_page=100"));
    let _ = std::fs::remove_dir_all(dir);
}

#[test]
fn comment_and_transition_use_the_right_collection() {
    let dir = project("https://gitlab.com/acme/tools.git", "write");
    let secrets = connected();
    let fake = Arc::new(FakeTransport::new().reply(HttpResponse::new(
        201,
        r#"{"id": 9, "author": {"username": "ana.dev"}, "body": "Ok", "created_at": "2026-11-12T10:00:00Z"}"#,
    )));
    let args = CommentArgs {
        project_path: dir.display().to_string(),
        key: "!21".into(),
        body: "Ok".into(),
        op_id: None,
    };
    let result = ops::comment(&env(&fake, &secrets), &config(), args);
    assert_eq!(result.op, CounterOp::Comment);
    assert!(fake.last_request().unwrap().url.ends_with("/merge_requests/21/notes"));

    let fake = Arc::new(FakeTransport::new().reply(HttpResponse::new(200, DETAIL)));
    let args = TransitionArgs {
        project_path: dir.display().to_string(),
        key: "#12".into(),
        to: RequestState::Closed,
        op_id: None,
    };
    ops::transition(&env(&fake, &secrets), &config(), args);
    let sent = fake.last_request().unwrap();
    assert_eq!(sent.method.as_str(), "PUT");
    assert_eq!(sent.body.unwrap()["state_event"], "close");

    let refused = Arc::new(FakeTransport::new());
    let args = TransitionArgs {
        project_path: dir.display().to_string(),
        key: "!21".into(),
        to: RequestState::Merged,
        op_id: None,
    };
    let result = ops::transition(&env(&refused, &secrets), &config(), args);
    assert_eq!(result.error, Some(CounterError::Unsupported));
    let _ = std::fs::remove_dir_all(dir);
}

#[test]
fn a_foreign_host_never_receives_the_token() {
    let dir = project("https://evil.example/acme/tools.git", "host");
    let fake = Arc::new(FakeTransport::new());
    let secrets = connected();
    let result = ops::list(&env(&fake, &secrets), &config(), list_args(&dir, RequestScope::Open));
    assert!(matches!(result.error, Some(CounterError::Config { .. })));
    assert!(fake.requests().is_empty());
    let _ = std::fs::remove_dir_all(dir);
}

#[test]
fn connect_stores_only_after_the_gitlab_accepts_the_token() {
    let secrets = Arc::new(MemorySecretStore::new());
    let fake = Arc::new(FakeTransport::new().reply(HttpResponse::new(200, USER)));
    let args = ConnectArgs {
        token: "glpat_new".into(),
        site: None,
        email: None,
        op_id: None,
    };
    let result = ops::connect(&env(&fake, &secrets), &config(), args);
    assert_eq!(result.account.as_deref(), Some("ana.dev"));
    assert_eq!(
        secrets.get(&service_for(MODULE), "gitlab.com").unwrap().as_deref(),
        Some("glpat_new")
    );
    let fresh = Arc::new(MemorySecretStore::new());
    let refused = Arc::new(FakeTransport::new().reply(HttpResponse::new(401, "401")));
    let args = ConnectArgs {
        token: "glpat_bad".into(),
        site: None,
        email: None,
        op_id: None,
    };
    let result = ops::connect(&env(&refused, &fresh), &config(), args);
    assert_eq!(result.error, Some(CounterError::Unauthorized));
    assert_eq!(fresh.get(&service_for(MODULE), "gitlab.com").unwrap(), None);
    let result = ops::disconnect(&env(&fake, &secrets), &config(), None);
    assert_eq!(result.error, None);
}

#[test]
fn fixture_mode_builds_lists_without_network() {
    let mine = ops::fixture_items(RequestScope::Mine).unwrap();
    assert!(!mine.is_empty() && mine.iter().all(|item| item.mine));
    let review = ops::fixture_items(RequestScope::ToReview).unwrap();
    assert_eq!(review.len(), 2, "the merged request is not a pending review");
}

#[test]
fn bad_keys_never_reach_the_network() {
    let dir = project("https://gitlab.com/acme/tools.git", "badkey");
    let fake = Arc::new(FakeTransport::new());
    let secrets = connected();
    let args = ItemArgs {
        project_path: dir.display().to_string(),
        key: "12".into(),
        op_id: None,
    };
    assert!(ops::get(&env(&fake, &secrets), &config(), args).error.is_some());
    assert!(fake.requests().is_empty());
    let _ = std::fs::remove_dir_all(dir);
}
