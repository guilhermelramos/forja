//! Testes de integração do módulo `counter_jira`: registro, comandos e fixtures REST v3.

use super::*;
use crate::ops::{Config, Env, MODULE};
use forja_contracts::{
    CommentArgs, ConnectArgs, CounterError, CounterOp, ItemArgs, ListArgs, RequestScope,
    RequestState, TransitionArgs,
};
use forja_counter_kit::{service_for, FakeTransport, HttpResponse, MemorySecretStore, SecretStore};
use forja_kernel::{contract, Args, Runtime, RuntimeOptions};

const SEARCH: &str = include_str!("../tests/fixtures/search.json");
const ISSUE: &str = include_str!("../tests/fixtures/issue.json");
const TRANSITIONS: &str = include_str!("../tests/fixtures/transitions.json");
const MYSELF: &str = include_str!("../tests/fixtures/myself.json");
const COMMENT: &str = include_str!("../tests/fixtures/comment.json");
const ERROR_MESSAGES: &str = include_str!("../tests/fixtures/error_messages.json");

fn runtime_with(env: Env) -> Runtime {
    let dir = std::env::temp_dir().join(format!("forja-counter-jira-rt-{}", std::process::id()));
    Runtime::new(
        vec![Box::new(JiraCounter::with_env(env))],
        RuntimeOptions {
            data_dir: Some(dir.clone()),
            user_dir: Some(dir.join("user")),
            locale: Some("pt-BR".to_owned()),
            wake: None,
        },
    )
    .expect("counter_jira registers cleanly")
}

#[test]
fn contract_holds_and_describe_is_io_free() {
    let env = Env {
        transport: Arc::new(FakeTransport::new()),
        secrets: Arc::new(MemorySecretStore::new()),
    };
    let mut runtime = runtime_with(env);
    contract::assert_ok(runtime.registry(), runtime.i18n());
    let value = runtime.run_command("counter_jira.describe", Args::new()).unwrap();
    assert_eq!(value["counter"], "jira");
}

fn config(project: &str) -> Config {
    Config {
        project: project.to_owned(),
    }
}

fn connected() -> Arc<MemorySecretStore> {
    let secrets = Arc::new(MemorySecretStore::new());
    let service = service_for(MODULE);
    secrets.set(&service, "site", "acme.atlassian.net").unwrap();
    secrets.set(&service, "acme.atlassian.net", "jira_tok_test").unwrap();
    secrets.set(&service, "acme.atlassian.net#email", "ana@example.com").unwrap();
    secrets.set(&service, "acme.atlassian.net#login", "Ana Lima").unwrap();
    secrets
}

fn env(fake: &Arc<FakeTransport>, secrets: &Arc<MemorySecretStore>) -> Env {
    Env {
        transport: fake.clone(),
        secrets: secrets.clone(),
    }
}

fn list_args(scope: RequestScope) -> ListArgs {
    ListArgs {
        project_path: String::new(),
        scope,
        op_id: None,
        query: None,
    }
}

#[test]
fn list_posts_jql_with_basic_auth_and_never_the_raw_token() {
    let fake = Arc::new(FakeTransport::new().reply(HttpResponse::new(200, SEARCH)));
    let secrets = connected();
    let result = ops::list(&env(&fake, &secrets), &config("ENG"), list_args(RequestScope::Mine));
    assert_eq!(result.error, None);
    assert_eq!(result.items.len(), 3);
    assert!(result.items[0].mine, "assigned to Ana Lima");
    let request = fake.last_request().unwrap();
    assert!(request.url.ends_with("/rest/api/3/search/jql"));
    let auth = request.header_value("Authorization").unwrap();
    assert!(auth.starts_with("Basic "));
    assert!(!auth.contains("jira_tok_test"), "the token is encoded, never sent raw");
    let jql = request.body.unwrap()["jql"].as_str().unwrap().to_owned();
    assert!(jql.starts_with("project = ENG AND statusCategory != Done AND assignee = currentUser()"));
}

#[test]
fn pages_follow_next_page_token_until_is_last() {
    let first = r#"{"isLast": false, "nextPageToken": "tok-2", "issues": [{"key": "ENG-1", "fields": {"summary": "a"}}]}"#;
    let second = r#"{"isLast": true, "issues": [{"key": "ENG-2", "fields": {"summary": "b"}}]}"#;
    let fake = Arc::new(FakeTransport::new().reply(HttpResponse::new(200, first)).reply(HttpResponse::new(200, second)));
    let secrets = connected();
    let result = ops::list(&env(&fake, &secrets), &config(""), list_args(RequestScope::All));
    assert_eq!(result.items.len(), 2);
    assert_eq!(fake.requests()[1].body.as_ref().unwrap()["nextPageToken"], "tok-2");
}

#[test]
fn a_project_key_that_could_change_the_jql_is_refused() {
    let fake = Arc::new(FakeTransport::new());
    let secrets = connected();
    let result = ops::list(&env(&fake, &secrets), &config("ENG OR 1=1"), list_args(RequestScope::All));
    assert!(matches!(result.error, Some(CounterError::Config { .. })));
    assert!(fake.requests().is_empty());
}

#[test]
fn review_is_unsupported_and_status_errors_map() {
    let secrets = connected();
    let fake = Arc::new(FakeTransport::new());
    let result = ops::list(&env(&fake, &secrets), &config(""), list_args(RequestScope::ToReview));
    assert_eq!(result.error, Some(CounterError::Unsupported));
    assert!(fake.requests().is_empty());

    for (status, expected) in [
        (401, CounterError::Unauthorized),
        (403, CounterError::Forbidden),
        (404, CounterError::NotFound),
        (429, CounterError::RateLimited { retry_after_s: None }),
    ] {
        let fake = Arc::new(FakeTransport::new().reply(HttpResponse::new(status, ERROR_MESSAGES)));
        let result = ops::list(&env(&fake, &secrets), &config(""), list_args(RequestScope::All));
        assert_eq!(result.error, Some(expected));
    }
    let fake = Arc::new(FakeTransport::new().reply(HttpResponse::new(400, ERROR_MESSAGES)));
    let result = ops::list(&env(&fake, &secrets), &config(""), list_args(RequestScope::All));
    assert_eq!(
        result.error,
        Some(CounterError::Rejected {
            message: "The value 'ENG' does not exist for the field 'project'.".into()
        })
    );
}

#[test]
fn without_saved_credentials_nothing_is_sent() {
    let fake = Arc::new(FakeTransport::new());
    let secrets = Arc::new(MemorySecretStore::new());
    let result = ops::list(&env(&fake, &secrets), &config(""), list_args(RequestScope::All));
    assert_eq!(result.error, Some(CounterError::NotConnected));
    assert!(fake.requests().is_empty());
}

#[test]
fn get_flattens_the_description_and_comments() {
    let fake = Arc::new(FakeTransport::new().reply(HttpResponse::new(200, ISSUE)));
    let secrets = connected();
    let args = ItemArgs {
        project_path: String::new(),
        key: "ENG-42".into(),
        op_id: None,
    };
    let result = ops::get(&env(&fake, &secrets), &config(""), args);
    let item = result.item.expect("item");
    assert!(item.body.unwrap().contains("200 cartões"));
    assert_eq!(item.comments.len(), 1);
    assert_eq!(item.comments[0].body, "Medi: 2,4 s no lote grande.");
    assert_eq!(item.url.as_deref(), Some("https://acme.atlassian.net/browse/ENG-42"));
}

#[test]
fn comments_are_sent_as_adf_and_empty_ones_are_refused() {
    let fake = Arc::new(FakeTransport::new().reply(HttpResponse::new(201, COMMENT)));
    let secrets = connected();
    let args = CommentArgs {
        project_path: String::new(),
        key: "ENG-42".into(),
        body: "Obrigado".into(),
        op_id: None,
    };
    let result = ops::comment(&env(&fake, &secrets), &config(""), args);
    assert_eq!(result.comment.unwrap().author, "Ana Lima");
    let body = fake.last_request().unwrap().body.unwrap();
    assert_eq!(body["body"]["type"], "doc");
    assert_eq!(body["body"]["content"][0]["content"][0]["text"], "Obrigado");

    let silent = Arc::new(FakeTransport::new());
    let args = CommentArgs {
        project_path: String::new(),
        key: "ENG-42".into(),
        body: "  ".into(),
        op_id: None,
    };
    assert!(ops::comment(&env(&silent, &secrets), &config(""), args).error.is_some());
    assert!(silent.requests().is_empty());
}

#[test]
fn transition_applies_the_done_transition_by_category() {
    let fake = Arc::new(
        FakeTransport::new()
            .reply(HttpResponse::new(200, TRANSITIONS))
            .reply(HttpResponse::new(204, ""))
            .reply(HttpResponse::new(200, ISSUE)),
    );
    let secrets = connected();
    let args = TransitionArgs {
        project_path: String::new(),
        key: "ENG-42".into(),
        to: RequestState::Closed,
        op_id: None,
    };
    let result = ops::transition(&env(&fake, &secrets), &config(""), args);
    assert_eq!(result.op, CounterOp::Transition);
    let requests = fake.requests();
    assert!(requests[0].url.ends_with("/issue/ENG-42/transitions"));
    assert_eq!(requests[1].body.as_ref().unwrap()["transition"]["id"], "31");
}

#[test]
fn bad_issue_keys_never_reach_the_network() {
    let fake = Arc::new(FakeTransport::new());
    let secrets = connected();
    let args = ItemArgs {
        project_path: String::new(),
        key: "eng-42".into(),
        op_id: None,
    };
    assert!(ops::get(&env(&fake, &secrets), &config(""), args).error.is_some());
    assert!(fake.requests().is_empty());
}

#[test]
fn connect_validates_site_email_and_token_before_storing() {
    let secrets = Arc::new(MemorySecretStore::new());
    let fake = Arc::new(FakeTransport::new().reply(HttpResponse::new(200, MYSELF)));
    let args = ConnectArgs {
        token: "jira_tok_new".into(),
        site: Some("https://Acme.atlassian.net/".into()),
        email: Some("ana@example.com".into()),
        op_id: None,
    };
    let result = ops::connect(&env(&fake, &secrets), &config(""), args);
    assert_eq!(result.account.as_deref(), Some("Ana Lima"));
    let service = service_for(MODULE);
    assert_eq!(secrets.get(&service, "site").unwrap().as_deref(), Some("acme.atlassian.net"));
    assert_eq!(
        secrets.get(&service, "acme.atlassian.net").unwrap().as_deref(),
        Some("jira_tok_new")
    );
    assert!(fake.last_request().unwrap().url.starts_with("https://acme.atlassian.net/rest/api/3/myself"));

    let fresh = Arc::new(MemorySecretStore::new());
    let refused = Arc::new(FakeTransport::new().reply(HttpResponse::new(401, "")));
    let args = ConnectArgs {
        token: "jira_tok_bad".into(),
        site: Some("acme.atlassian.net".into()),
        email: Some("ana@example.com".into()),
        op_id: None,
    };
    let result = ops::connect(&env(&refused, &fresh), &config(""), args);
    assert_eq!(result.error, Some(CounterError::Unauthorized));
    assert_eq!(fresh.get(&service, "site").unwrap(), None, "nothing is stored after a refusal");

    let no_site = ConnectArgs {
        token: "x".into(),
        site: Some("not a site".into()),
        email: Some("a@b.c".into()),
        op_id: None,
    };
    let silent = Arc::new(FakeTransport::new());
    assert!(ops::connect(&env(&silent, &fresh), &config(""), no_site).error.is_some());
    assert!(silent.requests().is_empty());
}

#[test]
fn disconnect_removes_every_jira_entry() {
    let secrets = connected();
    let fake = Arc::new(FakeTransport::new());
    let result = ops::disconnect(&env(&fake, &secrets), &config(""), None);
    assert_eq!(result.error, None);
    let service = service_for(MODULE);
    assert_eq!(secrets.get(&service, "site").unwrap(), None);
    assert_eq!(secrets.get(&service, "acme.atlassian.net").unwrap(), None);
}

#[test]
fn fixture_mode_lists_without_network() {
    let mine = ops::fixture_items(RequestScope::Mine).unwrap();
    assert!(!mine.is_empty() && mine.iter().all(|item| item.mine));
    assert_eq!(ops::fixture_items(RequestScope::ToReview), Err(CounterError::Unsupported));
}
