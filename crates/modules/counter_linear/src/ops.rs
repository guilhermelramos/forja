//! Operações do Linear. Rodam em `Ctx::spawn`, fora da thread da interface.
//! Cada operação é uma ou duas chamadas GraphQL; erros de GraphQL chegam com HTTP 200.

use crate::api::{self, MAX_PAGES};
use forja_contracts::{
    CommentArgs, ConnectArgs, CounterError, CounterOp, CounterResult, ItemArgs, ListArgs,
    RequestItem, RequestScope, TransitionArgs,
};
use forja_counter_kit::{
    error_for_status, fixtures_enabled, list_result, service_for, HttpRequest, HttpResponse,
    SecretStore, Transport,
};
use serde_json::{json, Value};
use std::sync::Arc;

pub const COUNTER: &str = "linear";
pub const MODULE: &str = "counter_linear";
/// Conta do cofre para o token do Linear. O Linear é um serviço só, sem host configurável.
const ACCOUNT: &str = "api.linear.app";
const LOGIN_SUFFIX: &str = "#login";
const FIXTURE_LOGIN: &str = "Ana Lima";
const FIXTURE_LIST: &str = include_str!("../tests/fixtures/issues.json");

#[derive(Clone)]
pub struct Env {
    pub transport: Arc<dyn Transport>,
    pub secrets: Arc<dyn SecretStore>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Config {
    /// Chave da equipe (`ENG`). Vazio mostra todas as equipes visíveis.
    pub team: String,
}

pub fn list(env: &Env, config: &Config, args: ListArgs) -> CounterResult {
    let scope = args.scope;
    let outcome = if fixtures_enabled() {
        fixture_items(scope)
    } else {
        list_items(env, config, scope)
    };
    match outcome {
        Ok(items) => list_result(COUNTER, args.op_id, scope, items),
        Err(error) => with_op_id(CounterResult::failed(COUNTER, CounterOp::List, error), args.op_id),
    }
}

fn list_items(env: &Env, config: &Config, scope: RequestScope) -> Result<Vec<RequestItem>, CounterError> {
    let token = token(env)?;
    let login = login(env);
    let mut items = Vec::new();
    let mut after: Option<String> = None;
    for _ in 0..MAX_PAGES {
        let variables = api::list_variables(scope, &config.team, after.as_deref())?;
        let request = api::graphql(&token, api::LIST_QUERY, variables);
        let response = execute(env, request)?;
        let data = api::graphql_error(&response)?;
        let (page, next) = api::parse_page(&data, login.as_deref())?;
        items.extend(page);
        match next {
            Some(cursor) => after = Some(cursor),
            None => break,
        }
    }
    Ok(items)
}

pub fn get(env: &Env, _config: &Config, args: ItemArgs) -> CounterResult {
    match get_item(env, &args) {
        Ok(item) => {
            let mut result = CounterResult::ok(COUNTER, CounterOp::Get);
            result.item = Some(item);
            with_op_id(result, args.op_id)
        }
        Err(error) => with_op_id(CounterResult::failed(COUNTER, CounterOp::Get, error), args.op_id),
    }
}

fn get_item(env: &Env, args: &ItemArgs) -> Result<RequestItem, CounterError> {
    api::check_identifier(&args.key)?;
    let token = token(env)?;
    let request = api::graphql(&token, api::GET_QUERY, api::get_variables(&args.key));
    let data = api::graphql_error(&execute(env, request)?)?;
    api::parse_issue(&data, login(env).as_deref())
}

pub fn comment(env: &Env, _config: &Config, args: CommentArgs) -> CounterResult {
    match comment_on(env, &args) {
        Ok(comment) => {
            let mut result = CounterResult::ok(COUNTER, CounterOp::Comment);
            result.comment = Some(comment);
            with_op_id(result, args.op_id)
        }
        Err(error) => with_op_id(CounterResult::failed(COUNTER, CounterOp::Comment, error), args.op_id),
    }
}

fn comment_on(env: &Env, args: &CommentArgs) -> Result<forja_contracts::RequestComment, CounterError> {
    let body = args.body.trim();
    if body.is_empty() {
        return Err(CounterError::Rejected {
            message: "the comment is empty".to_owned(),
        });
    }
    api::check_identifier(&args.key)?;
    let token = token(env)?;
    let issue = issue_id(env, &token, &args.key)?;
    let variables = json!({ "input": { "issueId": issue, "body": body } });
    let data = api::graphql_error(&execute(env, api::graphql(&token, api::COMMENT_MUTATION, variables))?)?;
    api::parse_comment(&data)
}

pub fn transition(env: &Env, _config: &Config, args: TransitionArgs) -> CounterResult {
    match transition_item(env, &args) {
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

fn transition_item(env: &Env, args: &TransitionArgs) -> Result<RequestItem, CounterError> {
    api::check_identifier(&args.key)?;
    let token = token(env)?;
    let states = api::graphql_error(&execute(
        env,
        api::graphql(&token, api::STATES_QUERY, api::get_variables(&args.key)),
    )?)?;
    let state_id = api::pick_state(&states, args.to)?;
    let issue = api::issue_id(&states)?;
    let variables = json!({ "id": issue, "input": { "stateId": state_id } });
    let data = api::graphql_error(&execute(env, api::graphql(&token, api::UPDATE_MUTATION, variables))?)?;
    api::parse_updated(&data, login(env).as_deref())
}

pub fn test(env: &Env, _config: &Config, op_id: Option<String>) -> CounterResult {
    match token(env).and_then(|token| verify(env, &token)) {
        Ok(account) => {
            let mut result = CounterResult::ok(COUNTER, CounterOp::Test);
            result.account = Some(account);
            with_op_id(result, op_id)
        }
        Err(error) => with_op_id(CounterResult::failed(COUNTER, CounterOp::Test, error), op_id),
    }
}

/// Valida o token antes de guardá-lo; só grava quando o Linear aceita.
pub fn connect(env: &Env, _config: &Config, args: ConnectArgs) -> CounterResult {
    let token = args.token.trim().to_owned();
    let outcome = verify(env, &token).and_then(|account| {
        env.secrets.set(&service_for(MODULE), ACCOUNT, &token)?;
        env.secrets.set(&service_for(MODULE), &login_account(), &account)?;
        Ok(account)
    });
    match outcome {
        Ok(account) => {
            let mut result = CounterResult::ok(COUNTER, CounterOp::Connect);
            result.account = Some(account);
            with_op_id(result, args.op_id)
        }
        Err(error) => with_op_id(CounterResult::failed(COUNTER, CounterOp::Connect, error), args.op_id),
    }
}

pub fn disconnect(env: &Env, _config: &Config, op_id: Option<String>) -> CounterResult {
    let outcome = env
        .secrets
        .delete(&service_for(MODULE), ACCOUNT)
        .and_then(|()| env.secrets.delete(&service_for(MODULE), &login_account()));
    match outcome {
        Ok(()) => with_op_id(CounterResult::ok(COUNTER, CounterOp::Disconnect), op_id),
        Err(error) => with_op_id(CounterResult::failed(COUNTER, CounterOp::Disconnect, error), op_id),
    }
}

fn verify(env: &Env, token: &str) -> Result<String, CounterError> {
    let request = api::graphql(token, api::VIEWER_QUERY, json!({}));
    let data = api::graphql_error(&execute(env, request)?)?;
    api::parse_viewer(&data)
}

fn issue_id(env: &Env, token: &str, key: &str) -> Result<String, CounterError> {
    let data = api::graphql_error(&execute(
        env,
        api::graphql(token, api::STATES_QUERY, api::get_variables(key)),
    )?)?;
    api::issue_id(&data)
}

fn token(env: &Env) -> Result<String, CounterError> {
    env.secrets
        .get(&service_for(MODULE), ACCOUNT)?
        .filter(|token| !token.trim().is_empty())
        .ok_or(CounterError::NotConnected)
}

fn login(env: &Env) -> Option<String> {
    env.secrets
        .get(&service_for(MODULE), &login_account())
        .ok()
        .flatten()
}

fn login_account() -> String {
    format!("{ACCOUNT}{LOGIN_SUFFIX}")
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

/// Lista de captura de tela. Sem rede.
pub(crate) fn fixture_items(scope: RequestScope) -> Result<Vec<RequestItem>, CounterError> {
    if scope == RequestScope::ToReview {
        return Err(CounterError::Unsupported);
    }
    let data: Value = serde_json::from_str(FIXTURE_LIST).map_err(|error| CounterError::Decode {
        message: error.to_string(),
    })?;
    let (items, _) = api::parse_page(&data, Some(FIXTURE_LOGIN))?;
    Ok(items
        .into_iter()
        .filter(|item| match scope {
            RequestScope::Mine => item.mine,
            RequestScope::Open => item.is_open(),
            _ => true,
        })
        .collect())
}
