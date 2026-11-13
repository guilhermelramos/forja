//! Operações do GitLab. Tudo roda em `Ctx::spawn`, fora da thread da interface.
//! Issues e merge requests são listados em duas consultas paginadas e juntados.

use crate::api::{self, Kind, ListQuery, MAX_PAGES};
use forja_contracts::{
    CommentArgs, ConnectArgs, CounterError, CounterOp, CounterResult, ItemArgs, ListArgs,
    RequestItem, RequestScope, RequestState, TransitionArgs,
};
use forja_counter_kit::{
    check_host, error_for_status, fixtures_enabled, list_result, next_page_number, parse_remote,
    read_origin, service_for, HttpRequest, HttpResponse, SecretStore, Transport,
};
use serde_json::Value;
use std::path::Path;
use std::sync::Arc;

pub const COUNTER: &str = "gitlab";
pub const MODULE: &str = "counter_gitlab";
const LOGIN_SUFFIX: &str = "#login";
const FIXTURE_LOGIN: &str = "ana.dev";
const FIXTURE_PROJECT: &str = "acme/tools";
const FIXTURE_ISSUES: &str = include_str!("../tests/fixtures/issues.json");
const FIXTURE_MERGE_REQUESTS: &str = include_str!("../tests/fixtures/merge_requests.json");

#[derive(Clone)]
pub struct Env {
    pub transport: Arc<dyn Transport>,
    pub secrets: Arc<dyn SecretStore>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Config {
    pub host: String,
}

pub fn list(env: &Env, config: &Config, args: ListArgs) -> CounterResult {
    let scope = args.scope;
    let outcome = if fixtures_enabled() {
        fixture_items(scope)
    } else {
        list_items(env, config, &args)
    };
    match outcome {
        Ok(items) => list_result(COUNTER, args.op_id, scope, items),
        Err(error) => with_op_id(CounterResult::failed(COUNTER, CounterOp::List, error), args.op_id),
    }
}

pub(crate) fn list_items(env: &Env, config: &Config, args: &ListArgs) -> Result<Vec<RequestItem>, CounterError> {
    let project = resolve_project(&args.project_path, &config.host)?;
    let token = token(env, &config.host)?;
    let login = login(env, &config.host);
    let kinds: &[Kind] = if args.scope == RequestScope::ToReview {
        &[Kind::Change]
    } else {
        &[Kind::Issue, Kind::Change]
    };
    let api_base = api::api_base(&config.host);
    let mut items = Vec::new();
    for &kind in kinds {
        let query = ListQuery {
            project: project.clone(),
            scope: args.scope,
            login: login.clone(),
            kind,
        };
        let mut page = 1;
        for _ in 0..MAX_PAGES {
            let request = api::list_request(&api_base, &query, page)?;
            let response = execute(env, authorized(request, &token))?;
            items.extend(api::parse_items(
                response.json()?,
                kind,
                &project,
                login.as_deref(),
                args.scope,
            )?);
            match next_page_number(&response.headers) {
                Some(next) => page = next,
                None => break,
            }
        }
    }
    items.sort_by(|a, b| b.updated_at.cmp(&a.updated_at));
    Ok(items)
}

pub fn get(env: &Env, config: &Config, args: ItemArgs) -> CounterResult {
    let outcome = get_item(env, config, &args);
    match outcome {
        Ok(item) => {
            let mut result = CounterResult::ok(COUNTER, CounterOp::Get);
            result.item = Some(item);
            with_op_id(result, args.op_id)
        }
        Err(error) => with_op_id(CounterResult::failed(COUNTER, CounterOp::Get, error), args.op_id),
    }
}

fn get_item(env: &Env, config: &Config, args: &ItemArgs) -> Result<RequestItem, CounterError> {
    let project = resolve_project(&args.project_path, &config.host)?;
    let (kind, number) = api::parse_key(&args.key)?;
    let token = token(env, &config.host)?;
    let api_base = api::api_base(&config.host);
    let login = login(env, &config.host);
    let response = execute(
        env,
        authorized(api::get_request(&api_base, &project, kind, number), &token),
    )?;
    let mut item = api::parse_item(response.json()?, kind, &project, login.as_deref())?;
    let notes = execute(
        env,
        authorized(api::notes_request(&api_base, &project, kind, number), &token),
    )?;
    item.comments = api::parse_notes(notes.json()?)?;
    Ok(item)
}

pub fn comment(env: &Env, config: &Config, args: CommentArgs) -> CounterResult {
    match comment_on(env, config, &args) {
        Ok(comment) => {
            let mut result = CounterResult::ok(COUNTER, CounterOp::Comment);
            result.comment = Some(comment);
            with_op_id(result, args.op_id)
        }
        Err(error) => with_op_id(CounterResult::failed(COUNTER, CounterOp::Comment, error), args.op_id),
    }
}

fn comment_on(env: &Env, config: &Config, args: &CommentArgs) -> Result<forja_contracts::RequestComment, CounterError> {
    let body = args.body.trim();
    if body.is_empty() {
        return Err(CounterError::Rejected {
            message: "the comment is empty".to_owned(),
        });
    }
    let project = resolve_project(&args.project_path, &config.host)?;
    let (kind, number) = api::parse_key(&args.key)?;
    let token = token(env, &config.host)?;
    let request = api::note_request(&api::api_base(&config.host), &project, kind, number, body);
    let response = execute(env, authorized(request, &token))?;
    api::parse_note(response.json()?)
}

pub fn transition(env: &Env, config: &Config, args: TransitionArgs) -> CounterResult {
    match transition_item(env, config, &args) {
        Ok(item) => {
            let mut result = CounterResult::ok(COUNTER, CounterOp::Transition);
            result.item = Some(item);
            with_op_id(result, args.op_id)
        }
        Err(error) => with_op_id(
            CounterResult::failed(COUNTER, CounterOp::Transition, error),
            args.op_id,
        ),
    }
}

fn transition_item(env: &Env, config: &Config, args: &TransitionArgs) -> Result<RequestItem, CounterError> {
    let open = match args.to {
        RequestState::Open => true,
        RequestState::Closed => false,
        RequestState::Merged => return Err(CounterError::Unsupported),
    };
    let project = resolve_project(&args.project_path, &config.host)?;
    let (kind, number) = api::parse_key(&args.key)?;
    let token = token(env, &config.host)?;
    let request = api::state_request(&api::api_base(&config.host), &project, kind, number, open);
    let response = execute(env, authorized(request, &token))?;
    let login = login(env, &config.host);
    api::parse_item(response.json()?, kind, &project, login.as_deref())
}

pub fn test(env: &Env, config: &Config, op_id: Option<String>) -> CounterResult {
    let outcome = token(env, &config.host).and_then(|token| verify(env, &config.host, &token));
    match outcome {
        Ok(login) => {
            let mut result = CounterResult::ok(COUNTER, CounterOp::Test);
            result.account = Some(login);
            with_op_id(result, op_id)
        }
        Err(error) => with_op_id(CounterResult::failed(COUNTER, CounterOp::Test, error), op_id),
    }
}

/// Valida o token antes de guardá-lo; só grava quando o GitLab aceita.
pub fn connect(env: &Env, config: &Config, args: ConnectArgs) -> CounterResult {
    let token = args.token.trim().to_owned();
    let outcome = verify(env, &config.host, &token).and_then(|login| {
        env.secrets.set(&service_for(MODULE), &config.host, &token)?;
        env.secrets.set(&service_for(MODULE), &login_account(&config.host), &login)?;
        Ok(login)
    });
    match outcome {
        Ok(login) => {
            let mut result = CounterResult::ok(COUNTER, CounterOp::Connect);
            result.account = Some(login);
            with_op_id(result, args.op_id)
        }
        Err(error) => with_op_id(CounterResult::failed(COUNTER, CounterOp::Connect, error), args.op_id),
    }
}

pub fn disconnect(env: &Env, config: &Config, op_id: Option<String>) -> CounterResult {
    let outcome = env
        .secrets
        .delete(&service_for(MODULE), &config.host)
        .and_then(|()| env.secrets.delete(&service_for(MODULE), &login_account(&config.host)));
    match outcome {
        Ok(()) => with_op_id(CounterResult::ok(COUNTER, CounterOp::Disconnect), op_id),
        Err(error) => with_op_id(CounterResult::failed(COUNTER, CounterOp::Disconnect, error), op_id),
    }
}

fn verify(env: &Env, host: &str, token: &str) -> Result<String, CounterError> {
    let request = authorized(api::user_request(&api::api_base(host)), token);
    let response = execute(env, request)?;
    api::parse_login(response.json()?)
}

/// Caminho do projeto (`grupo/projeto`) do remoto `origin`, conferido contra o host.
fn resolve_project(project_path: &str, host: &str) -> Result<String, CounterError> {
    let url = read_origin(Path::new(project_path))?;
    let remote = parse_remote(&url).ok_or_else(|| CounterError::Config {
        message: "origin is not a repository URL".to_owned(),
    })?;
    check_host(&remote.host, host)?;
    Ok(remote.path)
}

fn token(env: &Env, host: &str) -> Result<String, CounterError> {
    env.secrets
        .get(&service_for(MODULE), host)?
        .filter(|token| !token.trim().is_empty())
        .ok_or(CounterError::NotConnected)
}

fn login(env: &Env, host: &str) -> Option<String> {
    env.secrets
        .get(&service_for(MODULE), &login_account(host))
        .ok()
        .flatten()
}

fn login_account(host: &str) -> String {
    format!("{host}{LOGIN_SUFFIX}")
}

/// O GitLab aceita o token no cabeçalho `PRIVATE-TOKEN`.
pub(crate) fn authorized(request: HttpRequest, token: &str) -> HttpRequest {
    request
        .header("PRIVATE-TOKEN", token)
        .header("Accept", "application/json")
}

fn execute(env: &Env, request: HttpRequest) -> Result<HttpResponse, CounterError> {
    let response = env.transport.send(&request)?;
    match error_for_status(response.status, &response.headers, &response.body) {
        Some(error) => Err(error),
        None => Ok(response),
    }
}

fn with_op_id(mut result: CounterResult, op_id: Option<String>) -> CounterResult {
    result.op_id = op_id;
    result
}

/// Lista de captura de tela: as fixtures, filtradas pelo escopo. Sem rede.
pub(crate) fn fixture_items(scope: RequestScope) -> Result<Vec<RequestItem>, CounterError> {
    let mut items = Vec::new();
    let kinds: &[(Kind, &str)] = if scope == RequestScope::ToReview {
        &[(Kind::Change, FIXTURE_MERGE_REQUESTS)]
    } else {
        &[(Kind::Issue, FIXTURE_ISSUES), (Kind::Change, FIXTURE_MERGE_REQUESTS)]
    };
    for &(kind, text) in kinds {
        let body: Value = serde_json::from_str(text).map_err(|error| CounterError::Decode {
            message: error.to_string(),
        })?;
        items.extend(api::parse_items(body, kind, FIXTURE_PROJECT, Some(FIXTURE_LOGIN), scope)?);
    }
    Ok(items
        .into_iter()
        .filter(|item| match scope {
            RequestScope::Mine => item.mine,
            RequestScope::ToReview => item.review_requested && item.is_open(),
            RequestScope::Open => item.is_open(),
            RequestScope::All => true,
        })
        .collect())
}
