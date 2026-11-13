//! Testes de integração do módulo `counter_linear`: registro, comandos e fixtures GraphQL.

use super::*;
use crate::ops::{Config, Env, MODULE};
use forja_contracts::{
    CommentArgs, ConnectArgs, CounterError, CounterOp, ItemArgs, ListArgs, RequestScope,
    RequestState, TransitionArgs,
};
use forja_counter_kit::{service_for, FakeTransport, HttpResponse, MemorySecretStore, SecretStore};
use forja_kernel::{contract, Args, Runtime, RuntimeOptions};

const ISSUES: &str = include_str!("../tests/fixtures/issues.json");
const DETAIL: &str = include_str!("../tests/fixtures/issue_detail.json");
const STATES: &str = include_str!("../tests/fixtures/states.json");
const VIEWER: &str = include_str!("../tests/fixtures/viewer.json");
const COMMENT: &str = include_str!("../tests/fixtures/comment.json");
const UPDATE: &str = include_str!("../tests/fixtures/update.json");
const ERROR_AUTH: &str = include_str!("../tests/fixtures/error_auth.json");
const ERROR_REJECTED: &str = include_str!("../tests/fixtures/error_rejected.json");

fn runtime_with(env: Env) -> Runtime {
    let dir = std::env::temp_dir().join(format!("forja-counter-linear-rt-{}", std::process::id()));
    Runtime::new(
        vec![Box::new(LinearCounter::with_env(env))],
        RuntimeOptions {
            data_dir: Some(dir.clone()),
            user_dir: Some(dir.join("user")),
            locale: Some("pt-BR".to_owned()),
            wake: None,
        },
    )
    .expect("counter_linear registers cleanly")
}

#[test]
fn contract_holds_and_describe_is_io_free() {
    let env = Env {
        transport: Arc::new(FakeTransport::new()),
        secrets: Arc::new(MemorySecretStore::new()),
    };
    let mut runtime = runtime_with(env);
    contract::assert_ok(runtime.registry(), runtime.i18n());
    let value = runtime.run_command("counter_linear.describe", Args::new()).unwrap();
    assert_eq!(value["counter"], "linear");
}

fn config(team: &str) -> Config {
    Config {
        team: team.to_owned(),
    }
}

fn connected() -> Arc<MemorySecretStore> {
    let secrets = Arc::new(MemorySecretStore::new());
    secrets.set(&service_for(MODULE), "api.linear.app", "lin_api_test").unwrap();
    secrets.set(&service_for(MODULE), "api.linear.app#login", "Ana Lima").unwrap();
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
        op_id: Some("l1".into()),
        query: None,
    }
}

#[test]
fn list_sends_the_raw_key_and_the_team_filter() {
    let fake = Arc::new(FakeTransport::new().reply(HttpResponse::new(200, ISSUES)));
    let secrets = connected();
    let result = ops::list(&env(&fake, &secrets), &config("ENG"), list_args(RequestScope::Open));
    assert_eq!(result.error, None);
    assert_eq!(result.op_id.as_deref(), Some("l1"));
    assert_eq!(result.items.len(), 3);
    assert_eq!(result.items[0].key, "ENG-42");
    assert!(result.items[0].mine, "assigned to Ana Lima");
    let request = fake.last_request().unwrap();
    assert_eq!(request.url, "https://api.linear.app/graphql");
    assert_eq!(request.header_value("Authorization"), Some("lin_api_test"));
    let body = request.body.unwrap();
    assert_eq!(body["variables"]["filter"]["team"]["key"]["eq"], "ENG");
}

#[test]
fn pages_follow_the_cursor() {
    let first = r#"{"data":{"issues":{"nodes":[{"id":"1","identifier":"ENG-1","title":"a","state":{"type":"started"}}],"pageInfo":{"hasNextPage":true,"endCursor":"c2"}}}}"#;
    let second = r#"{"data":{"issues":{"nodes":[{"id":"2","identifier":"ENG-2","title":"b","state":{"type":"backlog"}}],"pageInfo":{"hasNextPage":false,"endCursor":null}}}}"#;
    let fake = Arc::new(FakeTransport::new().reply(HttpResponse::new(200, first)).reply(HttpResponse::new(200, second)));
    let secrets = connected();
    let result = ops::list(&env(&fake, &secrets), &config(""), list_args(RequestScope::All));
    assert_eq!(result.items.len(), 2);
    let requests = fake.requests();
    assert_eq!(requests[1].body.as_ref().unwrap()["variables"]["after"], "c2");
}

#[test]
fn review_is_unsupported_and_sends_nothing() {
    let fake = Arc::new(FakeTransport::new());
    let secrets = connected();
    let result = ops::list(&env(&fake, &secrets), &config("ENG"), list_args(RequestScope::ToReview));
    assert_eq!(result.error, Some(CounterError::Unsupported));
    assert!(fake.requests().is_empty());
}

#[test]
fn graphql_errors_arrive_with_http_200_and_still_map() {
    let secrets = connected();
    let fake = Arc::new(FakeTransport::new().reply(HttpResponse::new(200, ERROR_AUTH)));
    let result = ops::list(&env(&fake, &secrets), &config(""), list_args(RequestScope::All));
    assert_eq!(result.error, Some(CounterError::Unauthorized));

    let fake = Arc::new(FakeTransport::new().reply(HttpResponse::new(200, ERROR_REJECTED)));
    let result = ops::list(&env(&fake, &secrets), &config(""), list_args(RequestScope::All));
    assert_eq!(
        result.error,
        Some(CounterError::Rejected {
            message: "Team not found".into()
        })
    );
}

#[test]
fn http_status_errors_map_to_their_kinds() {
    let secrets = connected();
    for (status, expected) in [
        (401, CounterError::Unauthorized),
        (403, CounterError::Forbidden),
        (429, CounterError::RateLimited { retry_after_s: None }),
    ] {
        let fake = Arc::new(FakeTransport::new().reply(HttpResponse::new(status, "")));
        let result = ops::list(&env(&fake, &secrets), &config(""), list_args(RequestScope::All));
        assert_eq!(result.error, Some(expected));
    }
}

#[test]
fn get_reads_the_issue_with_its_comments() {
    let secrets = connected();
    let fake = Arc::new(FakeTransport::new().reply(HttpResponse::new(200, DETAIL)));
    let args = ItemArgs {
        project_path: String::new(),
        key: "ENG-42".into(),
        op_id: None,
    };
    let result = ops::get(&env(&fake, &secrets), &config(""), args);
    let item = result.item.expect("item");
    assert_eq!(item.comments.len(), 1);
    assert_eq!(item.comments[0].author, "Bruno Teles");
    assert!(item.body.unwrap().contains("200 cartões"));
}

#[test]
fn a_malformed_key_is_refused_before_the_network() {
    let secrets = connected();
    let fake = Arc::new(FakeTransport::new());
    let args = ItemArgs {
        project_path: String::new(),
        key: "eng-42".into(),
        op_id: None,
    };
    assert!(ops::get(&env(&fake, &secrets), &config(""), args).error.is_some());
    assert!(fake.requests().is_empty());
}

#[test]
fn comment_resolves_the_uuid_then_creates_the_comment() {
    let secrets = connected();
    let fake = Arc::new(FakeTransport::new().reply(HttpResponse::new(200, STATES)).reply(HttpResponse::new(200, COMMENT)));
    let args = CommentArgs {
        project_path: String::new(),
        key: "ENG-42".into(),
        body: "Obrigado".into(),
        op_id: None,
    };
    let result = ops::comment(&env(&fake, &secrets), &config(""), args);
    assert_eq!(result.op, CounterOp::Comment);
    assert_eq!(result.comment.unwrap().id.as_deref(), Some("c-9"));
    let requests = fake.requests();
    let body = requests[1].body.clone().unwrap();
    assert_eq!(body["variables"]["input"]["issueId"], "9f1d0c2e-1111-4a6b-9b1a-000000000001");
    assert_eq!(body["variables"]["input"]["body"], "Obrigado");
}

#[test]
fn closing_picks_the_completed_state() {
    let secrets = connected();
    let fake = Arc::new(FakeTransport::new().reply(HttpResponse::new(200, STATES)).reply(HttpResponse::new(200, UPDATE)));
    let args = TransitionArgs {
        project_path: String::new(),
        key: "ENG-42".into(),
        to: RequestState::Closed,
        op_id: None,
    };
    let result = ops::transition(&env(&fake, &secrets), &config(""), args);
    assert_eq!(result.item.unwrap().state, RequestState::Closed);
    let body = fake.requests()[1].body.clone().unwrap();
    assert_eq!(body["variables"]["input"]["stateId"], "st-done");
}

#[test]
fn test_and_connect_use_the_viewer_and_store_only_when_accepted() {
    let secrets = Arc::new(MemorySecretStore::new());
    let fake = Arc::new(FakeTransport::new().reply(HttpResponse::new(200, VIEWER)));
    let args = ConnectArgs {
        token: "lin_api_new".into(),
        site: None,
        email: None,
        op_id: None,
    };
    let result = ops::connect(&env(&fake, &secrets), &config(""), args);
    assert_eq!(result.account.as_deref(), Some("Ana Lima"));
    assert_eq!(
        secrets.get(&service_for(MODULE), "api.linear.app").unwrap().as_deref(),
        Some("lin_api_new")
    );

    let fresh = Arc::new(MemorySecretStore::new());
    let refused = Arc::new(FakeTransport::new().reply(HttpResponse::new(200, ERROR_AUTH)));
    let args = ConnectArgs {
        token: "lin_api_bad".into(),
        site: None,
        email: None,
        op_id: None,
    };
    let result = ops::connect(&env(&refused, &fresh), &config(""), args);
    assert_eq!(result.error, Some(CounterError::Unauthorized));
    assert_eq!(fresh.get(&service_for(MODULE), "api.linear.app").unwrap(), None);

    let result = ops::disconnect(&env(&fake, &secrets), &config(""), None);
    assert_eq!(result.error, None);
}

#[test]
fn fixture_mode_lists_without_network() {
    let open = ops::fixture_items(RequestScope::Open).unwrap();
    assert!(open.iter().all(|item| item.is_open()));
    assert_eq!(ops::fixture_items(RequestScope::ToReview), Err(CounterError::Unsupported));
}
