//! Testes de integração do módulo: registro, comandos e resultado publicado.

use super::*;
use forja_kernel::{contract, Args, Runtime, RuntimeOptions};
use crate::ops::MODULE;
use forja_counter_kit::{service_for, FakeTransport, MemorySecretStore, SecretStore};

fn runtime_with(env: Env) -> Runtime {
    let dir = std::env::temp_dir().join(format!(
        "forja-counter-github-{}-{}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_nanos())
            .unwrap_or(0)
    ));
    Runtime::new(
        vec![Box::new(GithubCounter::with_env(env))],
        RuntimeOptions {
            data_dir: Some(dir.clone()),
            user_dir: Some(dir.join("user")),
            locale: Some("pt-BR".to_owned()),
            wake: None,
        },
    )
    .expect("counter_github registers cleanly")
}

#[test]
fn contract_holds_for_the_module() {
    let runtime = runtime_with(Env {
        transport: Arc::new(FakeTransport::new()),
        secrets: Arc::new(MemorySecretStore::new()),
    });
    contract::assert_ok(runtime.registry(), runtime.i18n());
}

#[test]
fn describe_announces_the_counter_without_io() {
    let mut runtime = runtime_with(Env {
        transport: Arc::new(FakeTransport::new()),
        secrets: Arc::new(MemorySecretStore::new()),
    });
    let value = runtime
        .run_command("counter_github.describe", Args::new())
        .unwrap();
    assert_eq!(value["counter"], "github");
    assert_eq!(value["module"], "counter_github");
}

// --- Operações com fixtures: parsing, paginação, erros e montagem de requisição. ---

use forja_contracts::{
    CommentArgs, ConnectArgs, CounterError, CounterOp, ItemArgs, ListArgs, RequestScope,
    RequestState, TransitionArgs,
};
use forja_counter_kit::HttpResponse;
use std::path::PathBuf;

const ISSUES: &str = include_str!("../tests/fixtures/issues.json");
const REVIEW: &str = include_str!("../tests/fixtures/review_search.json");
const DETAIL: &str = include_str!("../tests/fixtures/issue_detail.json");
const COMMENTS: &str = include_str!("../tests/fixtures/issue_comments.json");
const USER: &str = include_str!("../tests/fixtures/user.json");
const REJECTED: &str = include_str!("../tests/fixtures/error_rejected.json");

/// Pasta temporária com um `origin` apontando para `url`. Fica só em `temp_dir`.
fn project(url: &str, label: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!(
        "forja-counter-github-{label}-{}-{}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_nanos())
            .unwrap_or(0)
    ));
    std::fs::create_dir_all(&dir).unwrap();
    for args in [
        vec!["init", "-q"],
        vec!["config", "user.name", "Forja Test"],
        vec!["config", "user.email", "forja@example.invalid"],
        vec!["remote", "add", "origin", url],
    ] {
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
        host: "github.com".into(),
        include_drafts: false,
        labels: Vec::new(),
    }
}

fn env(fake: &Arc<FakeTransport>, secrets: &Arc<MemorySecretStore>) -> Env {
    Env {
        transport: fake.clone(),
        secrets: secrets.clone(),
    }
}

fn connected() -> Arc<MemorySecretStore> {
    let secrets = Arc::new(MemorySecretStore::new());
    secrets
        .set(&service_for(MODULE), "github.com", "ghp_test_token")
        .unwrap();
    secrets
        .set(&service_for(MODULE), "github.com#login", "ana-dev")
        .unwrap();
    secrets
}

fn list_args(dir: &std::path::Path, scope: RequestScope) -> ListArgs {
    ListArgs {
        project_path: dir.display().to_string(),
        scope,
        op_id: Some("op-1".into()),
        query: None,
    }
}

#[test]
fn list_parses_the_fixture_and_drops_drafts_by_default() {
    let dir = project("https://github.com/acme/tools.git", "list");
    let fake = Arc::new(FakeTransport::new().reply(HttpResponse::new(200, ISSUES)));
    let secrets = connected();
    let result = ops::list(&env(&fake, &secrets), &config(), list_args(&dir, RequestScope::Open));
    assert_eq!(result.error, None);
    assert_eq!(result.op_id.as_deref(), Some("op-1"));
    assert_eq!(result.items.len(), 4, "the draft #55 is hidden");
    assert!(result.items.iter().all(|item| item.counter == "github"));
    let merged = result.items.iter().find(|item| item.key == "#39").expect("#39");
    assert_eq!(merged.state, RequestState::Merged);
    let request = fake.last_request().unwrap();
    assert!(request
        .url
        .starts_with("https://api.github.com/repos/acme/tools/issues?state=open"));
    assert_eq!(request.header_value("Authorization"), Some("Bearer ghp_test_token"));
    assert_eq!(request.header_value("Accept"), Some("application/vnd.github+json"));
    let _ = std::fs::remove_dir_all(dir);
}

#[test]
fn drafts_appear_only_when_the_setting_asks_for_them() {
    let dir = project("https://github.com/acme/tools.git", "drafts");
    let fake = Arc::new(FakeTransport::new().reply(HttpResponse::new(200, ISSUES)));
    let secrets = connected();
    let mut config = config();
    config.include_drafts = true;
    let result = ops::list(&env(&fake, &secrets), &config, list_args(&dir, RequestScope::All));
    assert_eq!(result.items.len(), 5);
    let _ = std::fs::remove_dir_all(dir);
}

#[test]
fn required_labels_filter_the_list() {
    let dir = project("https://github.com/acme/tools.git", "labels");
    let fake = Arc::new(FakeTransport::new().reply(HttpResponse::new(200, ISSUES)));
    let secrets = connected();
    let mut config = config();
    config.labels = vec!["bug".into()];
    let result = ops::list(&env(&fake, &secrets), &config, list_args(&dir, RequestScope::All));
    assert_eq!(result.items.len(), 1);
    assert_eq!(result.items[0].key, "#41");
    assert!(fake.last_request().unwrap().url.contains("labels=bug"));
    let _ = std::fs::remove_dir_all(dir);
}

#[test]
fn pages_follow_the_link_header_until_the_end() {
    let dir = project("https://github.com/acme/tools.git", "pages");
    let first = HttpResponse::new(200, r#"[{"number":1,"title":"a","state":"open"}]"#).with_headers(vec![(
        "Link",
        "<https://api.github.com/repos/acme/tools/issues?page=2>; rel=\"next\"",
    )]);
    let second = HttpResponse::new(200, r#"[{"number":2,"title":"b","state":"open"}]"#);
    let fake = Arc::new(FakeTransport::new().reply(first).reply(second));
    let secrets = connected();
    let result = ops::list(&env(&fake, &secrets), &config(), list_args(&dir, RequestScope::All));
    let keys: Vec<&str> = result.items.iter().map(|item| item.key.as_str()).collect();
    assert_eq!(keys, ["#1", "#2"]);
    let requests = fake.requests();
    assert_eq!(requests.len(), 2);
    assert!(requests[1].url.ends_with("page=2"));
    assert_eq!(requests[1].header_value("Authorization"), Some("Bearer ghp_test_token"));
    let _ = std::fs::remove_dir_all(dir);
}

#[test]
fn http_errors_become_counter_error_kinds() {
    let dir = project("https://github.com/acme/tools.git", "errors");
    let secrets = connected();
    let cases: Vec<(HttpResponse, CounterError)> = vec![
        (HttpResponse::new(401, "Bad credentials"), CounterError::Unauthorized),
        (HttpResponse::new(403, "forbidden"), CounterError::Forbidden),
        (HttpResponse::new(404, "Not Found"), CounterError::NotFound),
        (
            HttpResponse::new(429, "slow down").with_headers(vec![("Retry-After", "30")]),
            CounterError::RateLimited {
                retry_after_s: Some(30),
            },
        ),
        (
            HttpResponse::new(403, "API rate limit exceeded")
                .with_headers(vec![("X-RateLimit-Remaining", "0")]),
            CounterError::RateLimited { retry_after_s: None },
        ),
        (
            HttpResponse::new(500, "boom"),
            CounterError::Server { status: 500 },
        ),
    ];
    for (response, expected) in cases {
        let fake = Arc::new(FakeTransport::new().reply(response));
        let result = ops::list(&env(&fake, &secrets), &config(), list_args(&dir, RequestScope::All));
        assert_eq!(result.error, Some(expected));
        assert!(result.items.is_empty());
    }
    let fake = Arc::new(FakeTransport::new().reply(HttpResponse::new(422, REJECTED)));
    let result = ops::list(&env(&fake, &secrets), &config(), list_args(&dir, RequestScope::All));
    assert_eq!(
        result.error,
        Some(CounterError::Rejected {
            message: "{".into()
        })
    );
    let _ = std::fs::remove_dir_all(dir);
}

#[test]
fn without_a_token_nothing_is_sent() {
    let dir = project("https://github.com/acme/tools.git", "no-token");
    let fake = Arc::new(FakeTransport::new());
    let secrets = Arc::new(MemorySecretStore::new());
    let result = ops::list(&env(&fake, &secrets), &config(), list_args(&dir, RequestScope::All));
    assert_eq!(result.error, Some(CounterError::NotConnected));
    assert!(fake.requests().is_empty());
    let _ = std::fs::remove_dir_all(dir);
}

#[test]
fn the_token_never_goes_to_a_remote_on_another_host() {
    let dir = project("https://evil.example/acme/tools.git", "host");
    let fake = Arc::new(FakeTransport::new());
    let secrets = connected();
    let result = ops::list(&env(&fake, &secrets), &config(), list_args(&dir, RequestScope::All));
    assert!(matches!(result.error, Some(CounterError::Config { .. })));
    assert!(fake.requests().is_empty(), "no request leaves with the token");
    let _ = std::fs::remove_dir_all(dir);
}

#[test]
fn review_scope_uses_search_and_marks_the_requests() {
    let dir = project("https://github.com/acme/tools.git", "review");
    let fake = Arc::new(FakeTransport::new().reply(HttpResponse::new(200, REVIEW)));
    let secrets = connected();
    let result = ops::list(&env(&fake, &secrets), &config(), list_args(&dir, RequestScope::ToReview));
    assert_eq!(result.items.len(), 1);
    assert!(result.items[0].review_requested);
    let url = fake.last_request().unwrap().url;
    assert!(url.contains("/search/issues?q="));
    assert!(url.contains("review-requested%3Aana-dev"));
    let _ = std::fs::remove_dir_all(dir);
}

#[test]
fn mine_without_a_known_login_is_a_config_error() {
    let dir = project("https://github.com/acme/tools.git", "mine");
    let fake = Arc::new(FakeTransport::new());
    let secrets = Arc::new(MemorySecretStore::new());
    secrets.set(&service_for(MODULE), "github.com", "ghp_x").unwrap();
    let result = ops::list(&env(&fake, &secrets), &config(), list_args(&dir, RequestScope::Mine));
    assert!(matches!(result.error, Some(CounterError::Config { .. })));
    assert!(fake.requests().is_empty());
    let _ = std::fs::remove_dir_all(dir);
}

#[test]
fn get_reads_the_issue_and_its_comments() {
    let dir = project("https://github.com/acme/tools.git", "get");
    let fake = Arc::new(
        FakeTransport::new()
            .reply(HttpResponse::new(200, DETAIL))
            .reply(HttpResponse::new(200, COMMENTS)),
    );
    let secrets = connected();
    let args = ItemArgs {
        project_path: dir.display().to_string(),
        key: "#41".into(),
        op_id: None,
    };
    let result = ops::get(&env(&fake, &secrets), &config(), args);
    let item = result.item.expect("item");
    assert_eq!(item.comments.len(), 2);
    assert_eq!(item.comments[0].author, "carlos");
    assert!(item.body.unwrap().contains("Ao expirar o token"));
    assert!(item.mine, "ana-dev is an assignee");
    let requests = fake.requests();
    assert!(requests[0].url.ends_with("/repos/acme/tools/issues/41"));
    assert!(requests[1].url.ends_with("/issues/41/comments?per_page=100"));
    let _ = std::fs::remove_dir_all(dir);
}

#[test]
fn a_bad_key_never_reaches_the_network() {
    let dir = project("https://github.com/acme/tools.git", "badkey");
    let fake = Arc::new(FakeTransport::new());
    let secrets = connected();
    let args = ItemArgs {
        project_path: dir.display().to_string(),
        key: "41".into(),
        op_id: None,
    };
    let result = ops::get(&env(&fake, &secrets), &config(), args);
    assert!(result.error.is_some());
    assert!(fake.requests().is_empty());
    let _ = std::fs::remove_dir_all(dir);
}

#[test]
fn comment_posts_the_body_and_refuses_an_empty_one() {
    let dir = project("https://github.com/acme/tools.git", "comment");
    let secrets = connected();
    let fake = Arc::new(FakeTransport::new().reply(HttpResponse::new(
        201,
        r#"{"id": 77, "user": {"login": "ana-dev"}, "body": "Obrigado", "created_at": "2026-11-12T10:00:00Z"}"#,
    )));
    let args = CommentArgs {
        project_path: dir.display().to_string(),
        key: "#41".into(),
        body: "Obrigado".into(),
        op_id: None,
    };
    let result = ops::comment(&env(&fake, &secrets), &config(), args);
    assert_eq!(result.op, CounterOp::Comment);
    assert_eq!(result.comment.unwrap().id.as_deref(), Some("77"));
    let request = fake.last_request().unwrap();
    assert_eq!(request.body.unwrap()["body"], "Obrigado");
    assert!(request.url.ends_with("/issues/41/comments"));

    let empty = Arc::new(FakeTransport::new());
    let args = CommentArgs {
        project_path: dir.display().to_string(),
        key: "#41".into(),
        body: "   ".into(),
        op_id: None,
    };
    let result = ops::comment(&env(&empty, &secrets), &config(), args);
    assert!(matches!(result.error, Some(CounterError::Rejected { .. })));
    assert!(empty.requests().is_empty());
    let _ = std::fs::remove_dir_all(dir);
}

#[test]
fn transition_patches_the_state_and_merged_is_refused() {
    let dir = project("https://github.com/acme/tools.git", "transition");
    let secrets = connected();
    let fake = Arc::new(FakeTransport::new().reply(HttpResponse::new(200, DETAIL)));
    let args = TransitionArgs {
        project_path: dir.display().to_string(),
        key: "#41".into(),
        to: RequestState::Closed,
        op_id: None,
    };
    let result = ops::transition(&env(&fake, &secrets), &config(), args);
    assert_eq!(result.item.unwrap().key, "#41");
    let request = fake.last_request().unwrap();
    assert_eq!(request.method.as_str(), "PATCH");
    assert_eq!(request.body.unwrap()["state"], "closed");

    let args = TransitionArgs {
        project_path: dir.display().to_string(),
        key: "#41".into(),
        to: RequestState::Merged,
        op_id: None,
    };
    let refused = Arc::new(FakeTransport::new());
    let result = ops::transition(&env(&refused, &secrets), &config(), args);
    assert_eq!(result.error, Some(CounterError::Unsupported));
    assert!(refused.requests().is_empty());
    let _ = std::fs::remove_dir_all(dir);
}

#[test]
fn connect_validates_before_it_stores_anything() {
    let secrets = Arc::new(MemorySecretStore::new());
    let accepted = Arc::new(FakeTransport::new().reply(HttpResponse::new(200, USER)));
    let args = ConnectArgs {
        token: " ghp_new ".into(),
        site: None,
        email: None,
        op_id: Some("c1".into()),
    };
    let result = ops::connect(&env(&accepted, &secrets), &config(), args);
    assert_eq!(result.account.as_deref(), Some("ana-dev"));
    assert_eq!(
        secrets.get(&service_for(MODULE), "github.com").unwrap().as_deref(),
        Some("ghp_new")
    );
    let sent = accepted.last_request().unwrap();
    assert_eq!(sent.header_value("Authorization"), Some("Bearer ghp_new"));

    let refused = Arc::new(FakeTransport::new().reply(HttpResponse::new(401, "Bad credentials")));
    let fresh = Arc::new(MemorySecretStore::new());
    let args = ConnectArgs {
        token: "ghp_bad".into(),
        site: None,
        email: None,
        op_id: None,
    };
    let result = ops::connect(&env(&refused, &fresh), &config(), args);
    assert_eq!(result.error, Some(CounterError::Unauthorized));
    assert_eq!(fresh.get(&service_for(MODULE), "github.com").unwrap(), None);
}

#[test]
fn disconnect_forgets_the_token_and_the_login() {
    let secrets = connected();
    let fake = Arc::new(FakeTransport::new());
    let result = ops::disconnect(&env(&fake, &secrets), &config(), None);
    assert_eq!(result.error, None);
    assert_eq!(secrets.get(&service_for(MODULE), "github.com").unwrap(), None);
    assert_eq!(secrets.get(&service_for(MODULE), "github.com#login").unwrap(), None);
}

#[test]
fn malformed_bodies_are_decode_errors() {
    let dir = project("https://github.com/acme/tools.git", "decode");
    let fake = Arc::new(FakeTransport::new().reply(HttpResponse::new(200, "<html>")));
    let secrets = connected();
    let result = ops::list(&env(&fake, &secrets), &config(), list_args(&dir, RequestScope::All));
    assert!(matches!(result.error, Some(CounterError::Decode { .. })));
    let _ = std::fs::remove_dir_all(dir);
}

#[test]
fn fixture_mode_builds_the_mine_and_review_lists_without_network() {
    let mine = ops::fixture_items(&config(), RequestScope::Mine).unwrap();
    assert!(!mine.is_empty() && mine.iter().all(|item| item.mine));
    let review = ops::fixture_items(&config(), RequestScope::ToReview).unwrap();
    assert_eq!(review.len(), 1);
}

#[test]
fn token_field_exposes_a_non_empty_name_and_a_tooltip() {
    let mut runtime = runtime_with(Env {
        transport: Arc::new(FakeTransport::new()),
        secrets: Arc::new(MemorySecretStore::new()),
    });
    let mut view = ConnectionView {
        status: Arc::new(Mutex::new(Status::default())),
        token: String::new(),
    };
    use forja_kernel::ui_contract::egui;
    let ctx = egui::Context::default();
    ctx.enable_accesskit();
    let _ = forja_ui_kit::install_fonts(&ctx);
    forja_ui_kit::apply(&ctx, &forja_ui_kit::Tokens::brasa_dark());
    ctx.style_mut(|style| style.interaction.tooltip_delay = 0.0);
    let mut cx = runtime.ctx("counter_github");
    let mut point = None;
    let mut output = None;
    for _ in 0..3 {
        let mut input = egui::RawInput {
            screen_rect: Some(egui::Rect::from_min_size(
                egui::Pos2::ZERO,
                egui::vec2(900.0, 600.0),
            )),
            ..Default::default()
        };
        if let Some(point) = point {
            input.events.push(egui::Event::PointerMoved(point));
        }
        output = Some(ctx.run(input, |ctx| {
            egui::CentralPanel::default().show(ctx, |ui| {
                forja_kernel::ui_contract::View::ui(&mut view, ui, &mut cx);
            });
        }));
        if point.is_none() {
            let tree = output.as_ref().unwrap().platform_output.accesskit_update.as_ref().unwrap();
            let node = tree
                .nodes
                .iter()
                .map(|(_, node)| node)
                .find(|node| node.label() == Some("Token do GitHub"))
                .expect("the token field has a non-empty accessible name");
            let bounds = node.bounds().unwrap();
            point = Some(egui::pos2(
                ((bounds.x0 + bounds.x1) * 0.5) as f32,
                ((bounds.y0 + bounds.y1) * 0.5) as f32,
            ));
        }
    }
    drop(cx);
    let output = output.unwrap();
    assert!(
        output.shapes.iter().any(|shape| matches!(
            &shape.shape,
            egui::Shape::Text(text) if text.galley.text() == "Token do GitHub"
        )),
        "hovering the field shows its accessible name as a tooltip"
    );
}

#[test]
fn secrets_stay_out_of_debug_output() {
    let args = ConnectArgs {
        token: "ghp_visible".into(),
        site: None,
        email: None,
        op_id: None,
    };
    assert!(!format!("{args:?}").contains("ghp_visible"));
}
